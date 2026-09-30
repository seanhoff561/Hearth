//! Background terrain streaming for the in-process world: generates, lights and meshes the
//! cubes around a moving point, nearest first, and unloads the ones left behind. The render
//! thread only uploads the finished meshes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use glam::DVec3;
use hearth_math::{CubePos, Planet, PlanetSize};
use hearth_render::atlas::TextureArray;
use hearth_render::mesh::{CubeMesh, MeshOptions};
use hearth_render::models::BlockModels;
use rayon::prelude::*;
use rustc_hash::FxHashSet;

use crate::scene::LocalWorld;

/// Cubes generated per batch (bounded so nearby terrain appears quickly while moving).
const BATCH: usize = 384;

/// What the streamer should keep loaded.
#[derive(Debug, Clone, Copy)]
pub struct StreamTarget {
    pub center: DVec3,
    /// Horizontal radius in cubes.
    pub radius: i32,
    /// Vertical radius in cubes.
    pub vertical: i32,
}

/// Messages from the streamer thread.
pub enum StreamEvent {
    /// The world is ready; `spawn` is a good starting camera position.
    Ready {
        planet: Planet,
        spawn: DVec3,
    },
    Mesh(Box<CubeMesh>),
    Unload(CubePos),
    Failed(String),
}

/// Handle to the streaming thread; stops it when dropped.
pub struct Streamer {
    target: Arc<Mutex<StreamTarget>>,
    events: Receiver<StreamEvent>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// World settings for a streamed world.
#[derive(Debug, Clone)]
pub struct StreamWorld {
    pub seed: u64,
    pub planet: PlanetSize,
    pub cache_dir: Option<std::path::PathBuf>,
}

impl Streamer {
    pub fn start(world: StreamWorld, atlas: Arc<TextureArray>, target: StreamTarget) -> Self {
        let target = Arc::new(Mutex::new(target));
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, events) = channel();
        let thread = {
            let target = target.clone();
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("terrain streamer".into())
                .spawn(move || {
                    if let Err(e) = run(world, atlas, target, stop, &tx) {
                        let _ = tx.send(StreamEvent::Failed(format!("{e:#}")));
                    }
                })
                .expect("spawn streamer thread")
        };
        Self {
            target,
            events,
            stop,
            thread: Some(thread),
        }
    }

    pub fn set_target(&self, t: StreamTarget) {
        if let Ok(mut g) = self.target.lock() {
            *g = t;
        }
    }

    /// Next pending event, if any.
    pub fn poll(&self) -> Option<StreamEvent> {
        self.events.try_recv().ok()
    }
}

impl Drop for Streamer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(
    world: StreamWorld,
    atlas: Arc<TextureArray>,
    target: Arc<Mutex<StreamTarget>>,
    stop: Arc<AtomicBool>,
    tx: &Sender<StreamEvent>,
) -> anyhow::Result<()> {
    let mut lw = LocalWorld::create(world.seed, world.planet, 0, world.cache_dir.as_deref())?;
    let planet = *lw.map.planet();
    let (sx, sz) = lw.terrain().find_spawn(false);
    let spawn = DVec3::new(
        sx as f64 + 0.5,
        lw.surface_y(sx as f64, sz as f64) + 12.0,
        sz as f64 + 0.5,
    );
    if tx.send(StreamEvent::Ready { planet, spawn }).is_err() {
        return Ok(());
    }
    let models = BlockModels::build(&lw.reg, &atlas);
    let opts = MeshOptions::default();
    let mut loaded: FxHashSet<CubePos> = FxHashSet::default();
    let mut meshed: FxHashSet<CubePos> = FxHashSet::default();
    let mut wanted: Vec<(i64, CubePos)> = Vec::new();
    let mut last_center: Option<CubePos> = None;
    let mut last_radius = (0, 0);
    let mut idle_since = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        let t = match target.lock() {
            Ok(g) => *g,
            Err(_) => break,
        };
        let center = planet.wrap_cube(CubePos::containing(t.center));
        if last_center != Some(center) || last_radius != (t.radius, t.vertical) {
            last_center = Some(center);
            last_radius = (t.radius, t.vertical);
            // Unload what fell out of range (with a margin so small moves don't thrash).
            let far: Vec<CubePos> = loaded
                .iter()
                .copied()
                .filter(|p| {
                    let d = planet.cube_delta(center, *p);
                    d.x.abs() > t.radius + 2
                        || d.z.abs() > t.radius + 2
                        || d.y.abs() > t.vertical + 2
                })
                .collect();
            for p in far {
                loaded.remove(&p);
                lw.map.remove_cube(p);
                if meshed.remove(&p) && tx.send(StreamEvent::Unload(p)).is_err() {
                    return Ok(());
                }
            }
            // Everything in range that is missing, nearest first.
            wanted.clear();
            for dy in -t.vertical..=t.vertical {
                for dz in -t.radius..=t.radius {
                    for dx in -t.radius..=t.radius {
                        let p = planet.wrap_cube(CubePos::new(
                            center.x + dx,
                            center.y + dy,
                            center.z + dz,
                        ));
                        if !loaded.contains(&p) {
                            let d = (dx * dx + dz * dz) as i64 + (dy * dy) as i64 * 2;
                            wanted.push((d, p));
                        }
                    }
                }
            }
            // Farthest first so the nearest can be popped off the end.
            wanted.sort_unstable_by_key(|(d, _)| std::cmp::Reverse(*d));
        }
        if wanted.is_empty() {
            if idle_since.elapsed() > Duration::from_millis(2) {
                std::thread::sleep(Duration::from_millis(5));
            }
            continue;
        }
        idle_since = Instant::now();
        let take = wanted.len().min(BATCH);
        let batch: Vec<CubePos> = wanted
            .drain(wanted.len() - take..)
            .map(|(_, p)| p)
            .collect();
        let generator = lw.generator.clone();
        let cubes: Vec<_> = batch
            .par_iter()
            .map(|p| (*p, generator.generate_cube(*p)))
            .collect();
        for (p, cube) in cubes {
            let data = lw.generator.column(p.column());
            lw.map.ensure_column(p.column(), || {
                let mut est = [0i32; hearth_math::CUBE_AREA];
                for (e, s) in est.iter_mut().zip(&data.samples) {
                    *e = s.height_i().max(s.water_i()) - 1;
                }
                est
            });
            lw.map.insert_cube(p, Arc::new(cube), &lw.reg);
            loaded.insert(p);
        }
        lw.light.light_new_cubes(&mut lw.map, &lw.reg, &batch);
        // Mesh every cube around the batch whose 26 neighbours are now all present; cubes that
        // were already meshed are redone because their lighting may have changed.
        let mut ready: FxHashSet<CubePos> = FxHashSet::default();
        for p in &batch {
            for dz in -1..=1 {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let q = planet.wrap_cube(CubePos::new(p.x + dx, p.y + dy, p.z + dz));
                        if loaded.contains(&q)
                            && !ready.contains(&q)
                            && neighbours_loaded(&planet, &loaded, q)
                        {
                            ready.insert(q);
                        }
                    }
                }
            }
        }
        let ready: Vec<CubePos> = ready.into_iter().collect();
        let meshes = lw.mesh(&models, &ready, opts);
        for m in meshes {
            meshed.insert(m.pos);
            if tx.send(StreamEvent::Mesh(Box::new(m))).is_err() {
                return Ok(());
            }
        }
    }
    Ok(())
}

fn neighbours_loaded(planet: &Planet, loaded: &FxHashSet<CubePos>, p: CubePos) -> bool {
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                if (dx, dy, dz) != (0, 0, 0)
                    && !loaded.contains(&planet.wrap_cube(CubePos::new(
                        p.x + dx,
                        p.y + dy,
                        p.z + dz,
                    )))
                {
                    return false;
                }
            }
        }
    }
    true
}

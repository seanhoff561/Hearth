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
use hearth_render::precip::SkyHeights;
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
    /// Date whose seasonal snow and ice new terrain gets.
    pub year_frac: f64,
}

/// Messages from the streamer thread.
pub enum StreamEvent {
    /// The world is ready; `spawn` is a good starting camera position.
    Ready {
        planet: Planet,
        spawn: DVec3,
        grid: Arc<hearth_worldgen::PlanetGrid>,
    },
    Mesh(Box<CubeMesh>),
    Unload(CubePos),
    /// What covers the sky around the camera (hides rain and snow under cover).
    SkyHeights(Box<SkyHeights>),
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
    if tx
        .send(StreamEvent::Ready {
            planet,
            spawn,
            grid: lw.grid(),
        })
        .is_err()
    {
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
    // The sky-height map is rebuilt when the camera has moved a few blocks or terrain arrived,
    // at most a few times a second.
    let mut cover_step: Option<i64> = None;
    let mut heights_at: Option<(i32, i32)> = None;
    let mut heights_dirty = true;
    let mut heights_sent = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        let t = match target.lock() {
            Ok(g) => *g,
            Err(_) => break,
        };
        let column = (t.center.x.floor() as i32, t.center.z.floor() as i32);
        let moved =
            heights_at.is_none_or(|(x, z)| (x - column.0).abs().max((z - column.1).abs()) >= 8);
        if (moved || heights_dirty) && heights_sent.elapsed() > Duration::from_millis(250) {
            let map = &lw.map;
            let heights = SkyHeights::build(column.0, column.1, |x, z| map.sky_top(x, z));
            if tx.send(StreamEvent::SkyHeights(Box::new(heights))).is_err() {
                return Ok(());
            }
            heights_at = Some(column);
            heights_dirty = false;
            heights_sent = Instant::now();
        }
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
        // As the calendar moves on (every five days of the year), loaded terrain gets the
        // date's snow and ice.
        let step = (t.year_frac * COVER_STEPS).floor() as i64;
        if cover_step != Some(step) {
            if cover_step.is_some()
                && refresh_cover(&mut lw, &models, opts, &loaded, &meshed, t.year_frac, tx).is_err()
            {
                return Ok(());
            }
            cover_step = Some(step);
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
        // Seasonal snow and ice on the new terrain (idempotent: snow already lying on a column
        // is not a full block, so it isn't covered twice).
        let mut cols: Vec<hearth_math::ColumnPos> = batch.iter().map(|p| p.column()).collect();
        cols.sort_unstable_by_key(|c| (c.x, c.z));
        cols.dedup();
        crate::season_cover::apply(
            &mut lw.map,
            &lw.reg,
            &lw.cover,
            &lw.generator,
            &mut lw.buried,
            &cols,
            t.year_frac,
        );
        lw.light.light_new_cubes(&mut lw.map, &lw.reg, &batch);
        heights_dirty = true;
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

/// Seasonal cover steps per year (as the year-scale snow model).
const COVER_STEPS: f64 = 73.0;

/// Re-lays the date's snow and ice on all loaded terrain; relights and remeshes what changed.
fn refresh_cover(
    lw: &mut LocalWorld,
    models: &BlockModels,
    opts: MeshOptions,
    loaded: &FxHashSet<CubePos>,
    meshed: &FxHashSet<CubePos>,
    year_frac: f64,
    tx: &Sender<StreamEvent>,
) -> Result<(), ()> {
    let t0 = Instant::now();
    let mut cols: Vec<hearth_math::ColumnPos> = loaded.iter().map(|p| p.column()).collect();
    cols.sort_unstable_by_key(|c| (c.x, c.z));
    cols.dedup();
    let planet = *lw.map.planet();
    lw.buried.retain_columns(|c| {
        cols.binary_search_by_key(&(c.x, c.z), |k| (k.x, k.z))
            .is_ok()
    });
    let changed = crate::season_cover::refresh(
        &mut lw.map,
        &lw.reg,
        &lw.cover,
        &lw.generator,
        &mut lw.buried,
        &cols,
        year_frac,
    );
    let mut dirty: FxHashSet<CubePos> = FxHashSet::default();
    for &p in &changed {
        // Snow layers don't change light; ice and water do, a little.
        lw.light.block_changed(&mut lw.map, &lw.reg, p);
        // The block's cube and, on a cube face, the neighbour that shows it.
        let p = planet.wrap_block(p);
        let local = p.local();
        dirty.insert(p.cube());
        for (d, edge) in [
            ((-1, 0, 0), local.x == 0),
            ((1, 0, 0), local.x == 15),
            ((0, -1, 0), local.y == 0),
            ((0, 1, 0), local.y == 15),
            ((0, 0, -1), local.z == 0),
            ((0, 0, 1), local.z == 15),
        ] {
            if edge {
                let c = p.cube();
                dirty.insert(planet.wrap_cube(CubePos::new(c.x + d.0, c.y + d.1, c.z + d.2)));
            }
        }
    }
    let dirty: Vec<CubePos> = dirty.into_iter().filter(|c| meshed.contains(c)).collect();
    let meshes = lw.mesh(models, &dirty, opts);
    log::info!(
        "seasonal cover for year {:.3}: {} blocks changed, {} cubes remeshed in {:.0} ms",
        year_frac,
        changed.len(),
        meshes.len(),
        t0.elapsed().as_secs_f64() * 1e3
    );
    for m in meshes {
        tx.send(StreamEvent::Mesh(Box::new(m))).map_err(|_| ())?;
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

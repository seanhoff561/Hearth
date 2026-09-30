//! Streams distant terrain (v1 §8) around the camera: re-selects the LOD quadtree as the camera
//! moves, builds missing tiles nearest first — those in view before those behind — on a small
//! thread pool of its own (so it never starves the full-detail cube generation), and uploads
//! finished tiles a few per frame. Until a new tile is ready, the tiles it replaces stay drawn
//! (`hearth_lod::cover`), so moving never opens holes in the distant land.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use glam::DVec3;
use hearth_lod::{LodGen, TileKey, TileMesh};
use hearth_math::Planet;
use hearth_render::GpuContext;
use hearth_render::lod::LodRenderer;
use hearth_worldgen::WorldGenerator;
use rustc_hash::FxHashSet;

/// Tiles being built at once (more wait in the queue, so a moving camera reprioritises).
const IN_FLIGHT: usize = 48;
/// The camera moves this far (blocks) before the selection is redone.
const RESELECT: f64 = 16.0;
/// Tiles behind the camera wait as if this many times farther away.
const BEHIND_WEIGHT: f64 = 4.0;

pub struct LodStream {
    generator: Arc<WorldGenerator>,
    lod: Arc<LodGen>,
    pool: rayon::ThreadPool,
    /// LOD distance setting (chunks) and the world's vertical scale.
    chunks: u32,
    vertical_scale: f64,
    wanted: Vec<TileKey>,
    wanted_ids: FxHashSet<u64>,
    /// Wanted tiles not built or building yet, nearest last (popped first).
    queue: Vec<TileKey>,
    in_flight: FxHashSet<u64>,
    done_tx: Sender<TileMesh>,
    done_rx: Receiver<TileMesh>,
    last: Option<(DVec3, [f64; 4])>,
    /// The tiles drawn: the selection where built, stand-ins elsewhere.
    ids: Vec<u64>,
}

impl LodStream {
    pub fn new(
        generator: Arc<WorldGenerator>,
        lod: Arc<LodGen>,
        distance_chunks: u32,
        vertical_scale: f64,
    ) -> Self {
        let threads = (std::thread::available_parallelism().map_or(4, |n| n.get()) / 4).max(2);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .thread_name(|i| format!("lod-{i}"))
            .build()
            .expect("LOD thread pool");
        let (done_tx, done_rx) = channel();
        Self {
            generator,
            lod,
            pool,
            chunks: distance_chunks,
            vertical_scale,
            wanted: Vec::new(),
            wanted_ids: FxHashSet::default(),
            queue: Vec::new(),
            in_flight: FxHashSet::default(),
            done_tx,
            done_rx,
            last: None,
            ids: Vec::new(),
        }
    }

    /// Re-selects the tiles around the camera when it has moved, then keeps the builders busy.
    /// `forward` is the camera's view direction.
    pub fn update(
        &mut self,
        planet: &Planet,
        camera: DVec3,
        forward: DVec3,
        near: [f64; 4],
        renderer: &mut LodRenderer,
    ) {
        let moved = self.last.is_none_or(|(c, n)| {
            planet.delta_x(c.x, camera.x).abs() > RESELECT
                || (c.z - camera.z).abs() > RESELECT
                || (c.y - camera.y).abs() > RESELECT * 4.0
                || n != near
        });
        if moved && self.chunks > 0 {
            self.last = Some((camera, near));
            let reach = hearth_lod::draw_distance(self.chunks, camera.y, self.vertical_scale);
            self.wanted = hearth_lod::select(planet, camera.x, camera.z, reach, Some(near));
            self.wanted_ids = self.wanted.iter().map(|k| k.id()).collect();
            self.refresh(renderer);
            // Build order: nearest (and finest) first, what is in view before what is behind.
            let (fx, fz) = {
                let l = forward.x.hypot(forward.z).max(1e-9);
                (forward.x / l, forward.z / l)
            };
            let dist = |k: &TileKey| {
                let (x, z) = k.min_block();
                let h = k.size() as f64 * 0.5;
                let dx = planet.delta_x(camera.x, x as f64 + h);
                let dz = z as f64 + h - camera.z;
                let d2 = dx * dx + dz * dz;
                let ahead = (dx * fx + dz * fz) / d2.sqrt().max(1.0) > -0.2 || d2 < 4.0 * h * h;
                if ahead {
                    d2
                } else {
                    d2 * BEHIND_WEIGHT * BEHIND_WEIGHT
                }
            };
            self.queue = self
                .wanted
                .iter()
                .filter(|k| !renderer.contains(k.id()) && !self.in_flight.contains(&k.id()))
                .copied()
                .collect();
            self.queue.sort_by(|a, b| dist(b).total_cmp(&dist(a)));
        }
        while self.in_flight.len() < IN_FLIGHT {
            let Some(key) = self.queue.pop() else {
                break;
            };
            if renderer.contains(key.id()) {
                continue;
            }
            self.in_flight.insert(key.id());
            let (generator, lod, tx) = (
                self.generator.clone(),
                self.lod.clone(),
                self.done_tx.clone(),
            );
            self.pool.spawn(move || {
                let _ = tx.send(lod.build(&generator, key));
            });
        }
    }

    /// Uploads up to `max` finished tiles that are still wanted.
    pub fn pump(&mut self, ctx: &GpuContext, renderer: &mut LodRenderer, max: usize) {
        let mut uploaded = false;
        for _ in 0..max {
            let Ok(mesh) = self.done_rx.try_recv() else {
                break;
            };
            let id = mesh.key.id();
            self.in_flight.remove(&id);
            if self.wanted_ids.contains(&id) {
                upload(ctx, renderer, &mesh);
                uploaded = true;
            }
        }
        if uploaded {
            self.refresh(renderer);
        }
    }

    /// Chooses what to draw — the selection where built, the tiles it replaces elsewhere — and
    /// lets go of tiles neither wanted nor standing in.
    fn refresh(&mut self, renderer: &mut LodRenderer) {
        let draw = hearth_lod::cover(&self.wanted, |k| renderer.contains(k.id()));
        self.ids.clear();
        self.ids.extend(draw.iter().map(|k| k.id()));
        let drawn: FxHashSet<u64> = self.ids.iter().copied().collect();
        let wanted = &self.wanted_ids;
        renderer.retain(|id| wanted.contains(&id) || drawn.contains(&id));
    }

    /// Ids of the tiles of the current selection (the ones to draw).
    pub fn show(&self) -> &[u64] {
        &self.ids
    }

    /// Tiles wanted, and tiles still to build.
    pub fn progress(&self, renderer: &LodRenderer) -> (usize, usize) {
        let built = self
            .wanted
            .iter()
            .filter(|k| renderer.contains(k.id()))
            .count();
        (self.wanted.len(), self.wanted.len() - built)
    }
}

/// Uploads one tile mesh.
pub fn upload(ctx: &GpuContext, renderer: &mut LodRenderer, mesh: &TileMesh) {
    renderer.upload(
        ctx,
        mesh.key.id(),
        mesh.origin,
        mesh.key.size(),
        (mesh.min_y, mesh.max_y),
        bytemuck::cast_slice(&mesh.quads),
    );
}

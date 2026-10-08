//! Streams distant terrain (v1 §8) around the camera: re-selects the LOD quadtree as the camera
//! moves, builds missing tiles nearest first — those in view before those behind — on a small
//! thread pool of its own (so it never starves the full-detail cube generation), and uploads
//! finished tiles a few per frame. Until a new tile is ready, the tiles it replaces stay drawn
//! (`hearth_lod::cover`), so moving never opens holes in the distant land. Rough tiles are
//! refined by their error on screen (`hearth_lod::select_refined`): a tile that arrives with
//! steps standing out by more than the error allowed calls for a new selection.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use glam::DVec3;
use hearth_lod::{LodGen, Refine, TileKey, TileMesh};
use hearth_math::Planet;
use hearth_render::GpuContext;
use hearth_render::lod::LodRenderer;
use hearth_worldgen::WorldGenerator;
use hearth_worldgen::vegetation::Vegetation;
use rustc_hash::{FxHashMap, FxHashSet};

/// Tiles being built at once (more wait in the queue, so a moving camera reprioritises).
const IN_FLIGHT: usize = 48;
/// The camera moves this far (blocks) before the selection is redone.
const RESELECT: f64 = 16.0;
/// Tiles behind the camera wait as if this many times farther away.
const BEHIND_WEIGHT: f64 = 4.0;
/// Frames between selections called for by rough tiles arriving (a selection costs a little).
const REFINE_EVERY: u32 = 15;
/// Frames between looks at the memory budget.
const BUDGET_EVERY: u32 = 60;
/// The coarsest the budget makes the selection.
const MIN_DETAIL: f64 = 0.3;

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
    /// Vertical error and top of the built tiles the selection keeps or splits.
    errors: FxHashMap<TileKey, (f32, i32)>,
    /// The tiles the selection split (for hysteresis).
    split: FxHashSet<TileKey>,
    /// Tiles uploaded since the last selection, and frames since it.
    arrived: Vec<TileKey>,
    since_select: u32,
    /// Pixels per radian of the view, and the vertical error allowed on screen (pixels; 0 keeps
    /// the distance rule alone).
    px_per_rad: f64,
    max_error_px: f64,
    /// The vegetation and the player's changes the tiles are grown with.
    world: hearth_lod::LodWorld,
    /// Tiles kept on disk.
    cache: Option<Arc<hearth_lod::cache::TileCache>>,
    /// Tiles built with an older vegetation, drawn until they are built again.
    stale: FxHashSet<u64>,
    /// Video memory the tiles may take (bytes), and the detail the selection keeps to stay
    /// within it (1 as usual, less coarser), looked at every so many frames.
    budget: u64,
    detail: f64,
    since_budget: u32,
}

impl LodStream {
    /// `max_error_px`: the vertical error allowed on screen (0: no refinement).
    pub fn new(
        generator: Arc<WorldGenerator>,
        lod: Arc<LodGen>,
        distance_chunks: u32,
        vertical_scale: f64,
        max_error_px: f64,
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
            errors: FxHashMap::default(),
            split: FxHashSet::default(),
            arrived: Vec::new(),
            since_select: 0,
            px_per_rad: hearth_lod::px_per_rad(1080, 70.0),
            max_error_px,
            world: hearth_lod::LodWorld::default(),
            cache: None,
            stale: FxHashSet::default(),
            budget: 1024 << 20,
            detail: 1.0,
            since_budget: 0,
        }
    }

    /// The video memory the tiles may take (MiB).
    pub fn set_budget(&mut self, mib: u32) {
        self.budget = (mib.max(16) as u64) << 20;
    }

    /// The detail the selection keeps for the memory budget (1 as usual).
    pub fn detail(&self) -> f64 {
        self.detail
    }

    /// Coarsens the selection when the tiles outgrow the budget (the far tiles first), and
    /// lets it fine again when they are well within it.
    fn keep_budget(&mut self, renderer: &LodRenderer) {
        self.since_budget += 1;
        if self.since_budget < BUDGET_EVERY {
            return;
        }
        self.since_budget = 0;
        let bytes = renderer.bytes();
        let before = self.detail;
        // Within the budget, and within what the device's buffer can hold (from high up the
        // far tiles would fill it before the budget: E4).
        let budget = self.budget.min(renderer.max_bytes() / 100 * 85);
        if bytes > budget && self.detail > MIN_DETAIL {
            self.detail = (self.detail * 0.8).max(MIN_DETAIL);
        } else if (bytes as f64) < 0.6 * budget as f64 && self.detail < 1.0 {
            self.detail = (self.detail / 0.85).min(1.0);
        }
        if self.detail != before {
            log::info!(
                "distant terrain: {} MiB of tiles for a budget of {} MiB: detail {:.2}",
                bytes >> 20,
                budget >> 20,
                self.detail
            );
            self.last = None;
        }
    }

    /// Keeps the tiles built on disk in `dir` (and reads them back).
    pub fn set_cache(&mut self, dir: std::path::PathBuf) {
        self.cache = Some(Arc::new(hearth_lod::cache::TileCache::new(dir)));
    }

    /// New changes by the player: the tiles holding columns whose top changed are built again.
    pub fn set_edits(&mut self, edits: Arc<hearth_lod::EditTops>, renderer: &LodRenderer) {
        let old = std::mem::replace(&mut self.world.edits, edits);
        let changed: Vec<(i32, i32)> = self
            .world
            .edits
            .iter()
            .filter(|(p, v)| old.get(p) != Some(v))
            .map(|(p, _)| *p)
            .chain(
                old.keys()
                    .filter(|p| !self.world.edits.contains_key(p))
                    .copied(),
            )
            .collect();
        for k in &self.wanted {
            let (x, z) = k.min_block();
            let size = k.size();
            if changed
                .iter()
                .any(|(cx, cz)| (x..x + size).contains(cx) && (z..z + size).contains(cz))
            {
                self.stale.insert(k.id());
            }
        }
        self.stale.retain(|id| renderer.contains(*id));
        if !self.stale.is_empty() {
            self.last = None;
        }
    }

    /// New vegetation: the tiles its new disturbances reach are built again, and every tile when
    /// the year has turned (the trees have grown); the old tiles are drawn until then.
    pub fn set_vegetation(&mut self, veg: Vegetation, renderer: &LodRenderer) {
        // Tiles grow trees by whole years.
        let veg = veg.at_year(veg.year.floor());
        let old = &self.world.veg;
        let turned = veg.year != old.year;
        let added: Vec<hearth_worldgen::vegetation::Disturbance> = veg.added_since(old).to_vec();
        let fresh = !old.same_disturbances(&veg) && added.is_empty();
        self.world.veg = veg;
        if turned || fresh {
            self.stale.extend(self.wanted.iter().map(|k| k.id()));
        } else if !added.is_empty() {
            for k in &self.wanted {
                let (x, z) = k.min_block();
                let h = k.size() as f64 * 0.5;
                let (cx, cz) = (x as f64 + h, z as f64 + h);
                let touched = added.iter().any(|d| {
                    let reach = d.radius as f64 * hearth_worldgen::vegetation::EDGE as f64
                        + h * std::f64::consts::SQRT_2
                        + 24.0;
                    self.world.veg.distance(d, cx as i32, cz as i32) as f64 <= reach
                });
                if touched {
                    self.stale.insert(k.id());
                }
            }
        }
        self.stale.retain(|id| renderer.contains(*id));
        if !self.stale.is_empty() {
            // Queued again at the next update.
            self.last = None;
        }
    }

    /// New settings: the distance (chunks) and the vertical error allowed on screen.
    pub fn set_settings(&mut self, distance_chunks: u32, max_error_px: f64) {
        if distance_chunks != self.chunks || max_error_px != self.max_error_px {
            self.chunks = distance_chunks;
            self.max_error_px = max_error_px;
            self.last = None;
        }
    }

    /// The view the tiles are seen in: its height in pixels and vertical field of view (degrees).
    pub fn set_view(&mut self, height_px: u32, fov_y_deg: f32) {
        let ppr = hearth_lod::px_per_rad(height_px.max(1), fov_y_deg);
        // Small changes (a sprint's widening view) keep the selection.
        if (ppr / self.px_per_rad - 1.0).abs() > 0.1 {
            self.px_per_rad = ppr;
            self.last = None;
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
        self.keep_budget(renderer);
        self.since_select = self.since_select.saturating_add(1);
        let refining = self.max_error_px > 0.0;
        // Tiles that arrived rough enough to split call for finer ones.
        let rough = refining && self.since_select >= REFINE_EVERY && !self.arrived.is_empty() && {
            let errors = &self.errors;
            let built = |k: TileKey| errors.get(&k).copied();
            let refine = Refine {
                px_per_rad: self.px_per_rad,
                max_error_px: self.max_error_px,
                camera_y: camera.y,
                built: &built,
                split_before: &self.split,
            };
            let rough = self
                .arrived
                .iter()
                .any(|&k| hearth_lod::refines(planet, camera.x, camera.z, k, &refine));
            self.arrived.clear();
            rough
        };
        let moved = rough
            || self.last.is_none_or(|(c, n)| {
                planet.delta_x(c.x, camera.x).abs() > RESELECT
                    || (c.z - camera.z).abs() > RESELECT
                    || (c.y - camera.y).abs() > RESELECT * 4.0
                    || n != near
            });
        if moved && self.chunks > 0 {
            self.last = Some((camera, near));
            self.since_select = 0;
            self.arrived.clear();
            let reach = hearth_lod::draw_distance(self.chunks, camera.y, self.vertical_scale);
            let errors = &self.errors;
            let built = |k: TileKey| errors.get(&k).copied();
            let refine = refining.then_some(Refine {
                px_per_rad: self.px_per_rad,
                max_error_px: self.max_error_px,
                camera_y: camera.y,
                built: &built,
                split_before: &self.split,
            });
            self.wanted = hearth_lod::select_detailed(
                planet,
                camera.x,
                camera.z,
                reach,
                Some(near),
                refine.as_ref(),
                self.detail,
            );
            self.wanted_ids = self.wanted.iter().map(|k| k.id()).collect();
            if refining {
                self.split = hearth_lod::split_nodes(&self.wanted);
                let (split, wanted) = (&self.split, &self.wanted_ids);
                self.errors
                    .retain(|k, _| split.contains(k) || wanted.contains(&k.id()));
            }
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
            let stale = &self.stale;
            self.queue = self
                .wanted
                .iter()
                .filter(|k| {
                    (!renderer.contains(k.id()) || stale.contains(&k.id()))
                        && !self.in_flight.contains(&k.id())
                })
                .copied()
                .collect();
            self.queue.sort_by(|a, b| dist(b).total_cmp(&dist(a)));
        }
        while self.in_flight.len() < IN_FLIGHT {
            let Some(key) = self.queue.pop() else {
                break;
            };
            if renderer.contains(key.id()) && !self.stale.contains(&key.id()) {
                continue;
            }
            self.in_flight.insert(key.id());
            let (generator, lod, tx, world, cache) = (
                self.generator.clone(),
                self.lod.clone(),
                self.done_tx.clone(),
                self.world.clone(),
                self.cache.clone(),
            );
            self.pool.spawn(move || {
                hearth_core::zone!("lod.tile");
                let _c = hearth_core::prof::caller("lod");
                // From disk where it was kept as it would be built now; else built (and kept).
                let mesh = match &cache {
                    Some(c) => {
                        let stamp = c.stamp(&lod, &generator, &world, key);
                        c.load(key, stamp).unwrap_or_else(|| {
                            let m = lod.build_in(&generator, &world, key);
                            c.store(&m, stamp);
                            m
                        })
                    }
                    None => lod.build_in(&generator, &world, key),
                };
                let _ = tx.send(mesh);
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
            self.stale.remove(&id);
            if self.wanted_ids.contains(&id) {
                upload(ctx, renderer, &mesh);
                uploaded = true;
                if self.max_error_px > 0.0 {
                    self.errors.insert(mesh.key, (mesh.error, mesh.max_y));
                    self.arrived.push(mesh.key);
                }
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

/// Vertical error and top of built tiles (`hearth_lod::Refine::built`).
pub type Errors = FxHashMap<TileKey, (f32, i32)>;

/// Builds up front — for tools; the game streams them — the tiles `select` wants, refined in
/// rounds: a tile's error is known once it is built, so each round selects with the errors known
/// so far and builds what is new (`rounds` = 1 builds the selection without refinement). `build`
/// builds a batch of tiles; each is handed to `upload`. Returns the errors of all tiles built.
pub fn build_refined(
    rounds: u32,
    mut select: impl FnMut(&Errors) -> Vec<TileKey>,
    mut build: impl FnMut(&[TileKey]) -> anyhow::Result<Vec<TileMesh>>,
    mut upload: impl FnMut(&TileMesh),
) -> anyhow::Result<Errors> {
    let mut errors = Errors::default();
    for _ in 0..rounds {
        let mut new = select(&errors);
        new.retain(|k| !errors.contains_key(k));
        new.sort_unstable();
        new.dedup();
        if new.is_empty() {
            break;
        }
        for t in build(&new)? {
            errors.insert(t.key, (t.error, t.max_y));
            upload(&t);
        }
    }
    Ok(errors)
}

/// Uploads one tile mesh.
pub fn upload(ctx: &GpuContext, renderer: &mut LodRenderer, mesh: &TileMesh) {
    renderer.upload(
        ctx,
        mesh.key.id(),
        mesh.origin,
        mesh.key.size(),
        (mesh.min_y, mesh.max_y),
        bytemuck::cast_slice(&mesh.ground),
        mesh.skirt,
        bytemuck::cast_slice(&mesh.canopy),
        bytemuck::cast_slice(&mesh.quads),
        mesh.groups,
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn mesher_and_renderer_agree_on_the_quad_groups() {
        assert_eq!(hearth_lod::GROUP_FACES, hearth_render::lod::GROUP_FACES);
        assert_eq!(
            hearth_lod::GROUND_SIDE as u32,
            hearth_render::lod::GROUND_SIDE
        );
        assert_eq!(
            std::mem::size_of::<hearth_lod::GroundVertex>() as u64,
            hearth_render::lod::GROUND_BYTES
        );
    }
}

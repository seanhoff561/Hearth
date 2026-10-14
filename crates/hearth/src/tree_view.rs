//! The natural trees about the player drawn as their meshes (S5): the trees the server leaves
//! out of the cubes' meshes (`Features::drawn_trees_about`), found a column at a time off the
//! main thread from the generator and the vegetation, each drawn at the detail its distance asks
//! for (a coarser mesh standing in while a finer one is made), its mesh made off the main thread
//! and kept on the GPU (`hearth_render::trees`).

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use glam::{DVec3, Vec3};
use hearth_env::climate::SeasonalCover;
use hearth_flora::Stage;
use hearth_flora::mesh::{Detail, TreeMesh};
use hearth_math::{BlockPos, ColumnPos, Planet};
use hearth_render::GpuContext;
use hearth_render::camera::Frustum;
use hearth_render::trees::{TreeInstance, TreeRenderer};
use hearth_world::CubeMap;
use hearth_worldgen::WorldGenerator;
use hearth_worldgen::cubegen::features::{PlacedTree, Remains};
use hearth_worldgen::vegetation::Vegetation;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::tree_draw::{self, MeshOf, TreeLight, TreeLook};

/// Columns being found at once.
const FINDING: usize = 6;
/// Meshes being made at once.
const MAKING: usize = 6;
/// Meshes uploaded a frame.
const UPLOADS: usize = 6;
/// Frames a mesh is kept undrawn.
const KEEP_FRAMES: u64 = 900;
/// Columns are let go this far (m) past the distance trees are drawn to.
const MARGIN: f64 = 48.0;
/// Snow (m) on the open ground for the trees to bear their most.
const FULL_SNOW_M: f64 = 0.25;
/// A felled tree is drawn lying this long (s) after its fall, till its blocks come.
const LYING_S: f32 = 1.0;
/// Trees this near (m) keep this frame's instance, for the highlight of one looked at.
const NEAR_M: f64 = 16.0;

/// A tree found, as it is drawn.
#[derive(Debug, Clone)]
struct Tree {
    species: usize,
    stage: Stage,
    variant: u8,
    remains: Remains,
    /// Where its skeleton stands and its turn (`PlacedTree::frame`).
    origin: DVec3,
    turn: [f32; 4],
    /// Its foot's block, and the block over its top (for the light).
    foot: BlockPos,
    top: i32,
    /// Its height and reach (m), for culling.
    height: f32,
    reach: f32,
    /// The climate code where it stands.
    climate: u32,
    phase: u8,
    /// Its stem's radius (m).
    radius: f32,
    /// As the generator placed it (its blocks).
    placed: PlacedTree,
}

impl Tree {
    fn mesh_of(&self) -> MeshOf {
        MeshOf::standing(self.species, self.stage, self.variant, self.remains)
    }
}

/// A tree felled, falling as its mesh: turning about the hinge (its felling cut's edge toward
/// the fall) round the level axis, faster as it goes.
struct Fall {
    tree: Tree,
    light: TreeLight,
    snow: u8,
    hinge: DVec3,
    axis: DVec3,
    started: std::time::Instant,
    seconds: f32,
}

/// A column's trees (by their feet).
struct Column {
    trees: Vec<Tree>,
    light: Vec<TreeLight>,
    /// Its light to be read again (its cubes changed).
    stale_light: bool,
    cover: SeasonalCover,
    /// The vegetation it was found with ([`TreeView::generation`]).
    generation: u64,
    /// A box about all its trees: its middle and half its size (m), for culling them together.
    middle: DVec3,
    half: Vec3,
}

/// A column found off the main thread.
struct Found {
    col: ColumnPos,
    generation: u64,
    trees: Vec<Tree>,
    cover: SeasonalCover,
}

/// What a mesh is: of which tree, at which detail.
type MeshSpec = (MeshOf, Detail);

/// The trees about the player.
pub struct TreeView {
    generator: Arc<WorldGenerator>,
    looks: Vec<TreeLook>,
    /// The vegetation the trees grow by (none found till the server's comes).
    vegetation: Option<Vegetation>,
    /// Bumped when the vegetation changes: the columns found before are found again.
    generation: u64,
    columns: FxHashMap<ColumnPos, Column>,
    asked: FxHashSet<ColumnPos>,
    found: (Sender<Found>, Receiver<Found>),
    making: FxHashSet<u64>,
    made: (Sender<(u64, TreeMesh)>, Receiver<(u64, TreeMesh)>),
    /// Meshes made with nothing in them (nothing to draw).
    empty: FxHashSet<u64>,
    /// The last frame drew every tree in view with its own mesh, every column in reach found.
    settled: bool,
    /// Feet of trees (x wrapped) not to draw until their columns are found again (a tree
    /// felled or cut into).
    hidden: FxHashSet<(i32, i32)>,
    /// Trees felled, falling.
    falls: Vec<Fall>,
    batches: Vec<(u64, Vec<TreeInstance>)>,
    slot: FxHashMap<u64, usize>,
    /// The batches' lists of instances kept for the next frame's.
    spare: Vec<Vec<TreeInstance>>,
    /// The time of year last drawn at.
    year_frac: f64,
    /// The columns last looked over for those to find: from the eye's column, with the
    /// vegetation's generation and the reach then, and how many were not found yet. Looked over
    /// again when one of those changes or a column found leaves room for more.
    scanned: Option<((i32, i32, u64, i64), usize)>,
    /// This frame's trees near the eye, by foot (x wrapped): the mesh each is drawn with and its
    /// instance.
    near: FxHashMap<(i32, i32), (u64, TreeInstance)>,
    /// The falls drawn this far (s) into them, whatever the time (a picture's).
    fall_clock: Option<f32>,
    /// Trees drawn in the last frame.
    pub drawn: usize,
}

impl TreeView {
    pub fn new(generator: Arc<WorldGenerator>, colors: &hearth_lod::BlockColors) -> Self {
        let looks = tree_draw::looks(&generator.forest, colors);
        Self {
            generator,
            looks,
            vegetation: None,
            generation: 0,
            columns: FxHashMap::default(),
            asked: FxHashSet::default(),
            found: channel(),
            making: FxHashSet::default(),
            made: channel(),
            empty: FxHashSet::default(),
            settled: false,
            hidden: FxHashSet::default(),
            falls: Vec::new(),
            batches: Vec::new(),
            slot: FxHashMap::default(),
            spare: Vec::new(),
            year_frac: 0.0,
            scanned: None,
            near: FxHashMap::default(),
            fall_clock: None,
            drawn: 0,
        }
    }

    /// The vegetation now: where its disturbances or the trees cut into changed, or a season
    /// has passed, the columns are found again (drawn as they were meanwhile, but the trees cut
    /// into: their blocks are drawn from now on).
    pub fn set_vegetation(&mut self, v: Vegetation) {
        let changed = self.vegetation.as_ref().is_some_and(|old| {
            for foot in v.cut_since(old) {
                self.hidden.insert(foot);
            }
            !v.same_disturbances(old) || !v.same_cut(old) || (v.year - old.year).abs() > 1e-6
        });
        self.vegetation = Some(v);
        if changed {
            self.generation += 1;
        }
    }

    /// The foot of the nearest tree standing within `within` (m) of a place, ahead along
    /// `ahead` (level), as drawn.
    pub fn nearest(&self, at: DVec3, ahead: DVec3, within: f64) -> Option<BlockPos> {
        let planet = self.generator.planet();
        self.columns
            .values()
            .flat_map(|c| c.trees.iter())
            .filter(|t| t.remains == Remains::Living && t.stage >= Stage::Young)
            .map(|t| (t, planet.delta(at, t.origin)))
            .filter(|(_, d)| {
                let level = DVec3::new(d.x, 0.0, d.z);
                level.length() <= within && level.dot(ahead) > 0.0
            })
            .min_by(|a, b| a.1.length().total_cmp(&b.1.length()))
            .map(|(t, _)| t.foot)
    }

    /// The highlight of a block looked at that a tree drawn as its mesh holds (T §2.3, its own
    /// shape): the tree as this frame draws it, its leaves where the block is of its leaves,
    /// else its wood. None where no such tree holds the block as it grew there (a block of a
    /// tree cut into is drawn, and highlighted, as itself).
    pub fn highlight(
        &self,
        mirror: &CubeMap,
        pos: BlockPos,
    ) -> Option<hearth_render::outline::Highlight> {
        let planet = *mirror.planet();
        let state = mirror.block(pos)?;
        let forest = &self.generator.forest;
        let (t, part) = self
            .columns
            .values()
            .flat_map(|c| c.trees.iter())
            .filter(|t| {
                let r = t.reach as i32;
                let dx = planet.delta(t.origin, pos.center()).x.abs();
                dx <= r as f64 + 1.0
                    && (pos.z - t.foot.z).abs() <= r + 1
                    && pos.y >= t.foot.y + t.placed.template.min[1]
                    && pos.y <= t.foot.y + t.placed.template.max[1]
            })
            .find_map(|t| {
                t.placed
                    .blocks()
                    .find(|(p, part)| {
                        planet.wrap_block(*p) == planet.wrap_block(pos)
                            && t.placed.state(forest, *part) == state
                    })
                    .map(|(_, part)| (t, part))
            })?;
        let (key, instance) = *self.near.get(&(planet.wrap_x(t.foot.x), t.foot.z))?;
        let o = glam::Vec3::from(instance.origin);
        let r = glam::Vec3::new(t.reach, 0.0, t.reach);
        Some(hearth_render::outline::Highlight::Tree {
            key,
            instance,
            leaves: matches!(part, hearth_flora::Part::Leaves),
            bounds: (
                o - r - glam::Vec3::Y,
                o + r + glam::Vec3::Y * (t.height + 1.0),
            ),
        })
    }

    /// Draws the falls this far (s) into them from now on, whatever the time (a picture's).
    pub fn hold_falls_at(&mut self, seconds: f32) {
        self.fall_clock = Some(seconds);
    }

    /// A tree felled at its foot, falling toward `toward` (level) over `seconds`: drawn
    /// falling as its mesh if it is drawn as one (false: it is not, and its blocks fall).
    pub fn fell(&mut self, foot: BlockPos, toward: DVec3, seconds: f32) -> bool {
        let planet = self.generator.planet();
        let at = (planet.wrap_x(foot.x), foot.z);
        let year_frac = self.year_frac;
        let found = self.columns.values().find_map(|c| {
            c.trees
                .iter()
                .zip(&c.light)
                .find(|(t, _)| (planet.wrap_x(t.foot.x), t.foot.z) == at && t.foot.y == foot.y)
                .map(|(t, l)| (t.clone(), *l, snow_on(&c.cover, year_frac)))
        });
        let Some((tree, light, snow)) = found else {
            return false;
        };
        if tree.remains == Remains::Stump || self.hidden.contains(&at) {
            return false;
        }
        self.hidden.insert(at);
        let n = DVec3::new(toward.x, 0.0, toward.z).normalize_or(DVec3::X);
        self.falls.push(Fall {
            hinge: tree.origin + n * tree.radius as f64 + DVec3::Y * tree_draw::STUMP_M as f64,
            axis: DVec3::Y.cross(n),
            light,
            snow,
            tree,
            started: std::time::Instant::now(),
            seconds: seconds.max(0.1),
        });
        true
    }

    /// A column's cubes changed: the light on its trees is read again.
    pub fn cubes_changed(&mut self, col: ColumnPos) {
        if let Some(c) = self.columns.get_mut(&col) {
            c.stale_light = true;
        }
    }

    /// Prepares this frame's trees within `radius` (m) of the eye: the columns found and the
    /// meshes made since taken in, those wanted asked for, each tree in view drawn where its
    /// foot's cube is `loaded`.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        ctx: &GpuContext,
        renderer: &mut TreeRenderer,
        mirror: &CubeMap,
        eye: DVec3,
        frustum: &Frustum,
        radius: f64,
        year_frac: f64,
        wind: Vec3,
        loaded: impl Fn(BlockPos) -> bool,
    ) {
        let planet = *mirror.planet();
        self.take_found(planet, eye, radius);
        self.take_made(ctx, renderer);
        let unfound = self.ask_columns(planet, eye, radius);
        let mut missing = unfound > 0;
        self.year_frac = year_frac;
        for c in self.columns.values_mut().filter(|c| c.stale_light) {
            c.light = c.trees.iter().map(|t| light_of(mirror, t)).collect();
            c.stale_light = false;
        }
        for (_, mut list) in self.batches.drain(..) {
            list.clear();
            self.spare.push(list);
        }
        self.slot.clear();
        self.near.clear();
        let mut wanted: Vec<(u64, MeshSpec)> = Vec::new();
        let mut drawn = 0;
        for col in self.columns.values() {
            if col.trees.is_empty() {
                continue;
            }
            // The column's trees together: none of them within reach or in view, none drawn.
            let rel = planet.delta(eye, col.middle);
            let across = col.half.x as f64 * std::f64::consts::SQRT_2;
            if (rel.x * rel.x + rel.z * rel.z).sqrt() - across > radius {
                continue;
            }
            let rel = rel.as_vec3();
            if !frustum.intersects_aabb(rel - col.half, rel + col.half) {
                continue;
            }
            let snow = snow_on(&col.cover, year_frac);
            for (t, light) in col.trees.iter().zip(&col.light) {
                let rel = planet.delta(eye, t.origin);
                let distance = rel.length();
                if distance > radius
                    || (!self.hidden.is_empty()
                        && self.hidden.contains(&(planet.wrap_x(t.foot.x), t.foot.z)))
                {
                    continue;
                }
                let rel = rel.as_vec3();
                let r = Vec3::new(t.reach, 0.0, t.reach);
                if !frustum.intersects_aabb(rel - r, rel + r + Vec3::Y * t.height) {
                    continue;
                }
                if !loaded(t.foot) {
                    continue;
                }
                let detail = tree_draw::detail_at(distance as f32);
                let (key, own) = self.drawable(renderer, t.mesh_of(), detail, &mut wanted);
                missing |= !own;
                let Some(key) = key else {
                    continue;
                };
                let one = tree_draw::instance(
                    self.looks[t.species],
                    rel,
                    t.turn,
                    t.climate,
                    if t.remains == Remains::Stump { 0 } else { snow },
                    *light,
                    t.phase,
                    t.remains == Remains::Charred,
                );
                if distance < NEAR_M {
                    self.near
                        .insert((planet.wrap_x(t.foot.x), t.foot.z), (key, one));
                }
                Self::add(&mut self.batches, &mut self.slot, &mut self.spare, key, one);
                drawn += 1;
            }
        }
        // The trees falling: what falls of each turned down about its hinge, lying a moment
        // when down.
        let clock = self.fall_clock;
        let into = |f: &Fall| clock.unwrap_or_else(|| f.started.elapsed().as_secs_f32());
        self.falls.retain(|f| into(f) < f.seconds + LYING_S);
        for k in 0..self.falls.len() {
            let f = &self.falls[k];
            let t = (into(f) / f.seconds).min(1.0);
            let angle = std::f64::consts::FRAC_PI_2 * (t * t) as f64;
            let turned = glam::DQuat::from_axis_angle(f.axis, angle);
            let origin = f.hinge + turned * (f.tree.origin - f.hinge);
            let rel = planet.delta(eye, origin);
            let detail = tree_draw::detail_at(rel.length() as f32);
            let of = MeshOf {
                felled: true,
                ..f.tree.mesh_of()
            };
            // Its own mesh (another of its details while that is made), else the whole tree's.
            let (own_key, own) = self.drawable(renderer, of, detail, &mut wanted);
            missing |= !own;
            let key = own_key.or_else(|| {
                self.drawable(renderer, f.tree.mesh_of(), detail, &mut wanted)
                    .0
            });
            let Some(key) = key else {
                continue;
            };
            let f = &self.falls[k];
            let mut one = tree_draw::instance(
                self.looks[f.tree.species],
                rel.as_vec3(),
                f.tree.turn,
                f.tree.climate,
                f.snow,
                f.light,
                f.tree.phase,
                f.tree.remains == Remains::Charred,
            );
            one.tilt = tree_draw::tilt(angle as f32, f.axis.as_vec3());
            Self::add(&mut self.batches, &mut self.slot, &mut self.spare, key, one);
            drawn += 1;
        }
        self.drawn = drawn;
        self.settled = !missing && self.asked.is_empty() && self.making.is_empty();
        // The coarsest first: every tree drawn soon, the nearer ones finer after.
        wanted.sort_by_key(|w| std::cmp::Reverse(w.1.1));
        for (key, spec) in wanted {
            self.make(key, spec);
        }
        renderer.prepare(ctx, &self.batches, wind);
        renderer.forget_unused(KEEP_FRAMES);
    }

    /// Prepares the trees as [`frame`](Self::frame) does, waiting until every column in reach
    /// is found and every tree in view drawn with its own mesh (a picture's; at most a minute).
    #[allow(clippy::too_many_arguments)]
    pub fn settle(
        &mut self,
        ctx: &GpuContext,
        renderer: &mut TreeRenderer,
        mirror: &CubeMap,
        eye: DVec3,
        frustum: &Frustum,
        radius: f64,
        year_frac: f64,
        wind: Vec3,
        loaded: impl Fn(BlockPos) -> bool,
    ) {
        let t0 = std::time::Instant::now();
        loop {
            self.frame(
                ctx, renderer, mirror, eye, frustum, radius, year_frac, wind, &loaded,
            );
            if self.settled || t0.elapsed().as_secs() >= 60 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        log::info!(
            "{} trees drawn as meshes, settled in {:.2} s",
            self.drawn,
            t0.elapsed().as_secs_f64()
        );
    }

    /// One tree drawn with a mesh, in its batch.
    fn add(
        batches: &mut Vec<(u64, Vec<TreeInstance>)>,
        slot: &mut FxHashMap<u64, usize>,
        spare: &mut Vec<Vec<TreeInstance>>,
        key: u64,
        one: TreeInstance,
    ) {
        let i = *slot.entry(key).or_insert_with(|| {
            batches.push((key, spare.pop().unwrap_or_default()));
            batches.len() - 1
        });
        batches[i].1.push(one);
    }

    /// The mesh to draw a tree with at a detail, and whether it is its own: its own if made,
    /// else another of its details (the coarser first) while its own is made; none till one is
    /// (or where its own has nothing in it).
    fn drawable(
        &self,
        renderer: &TreeRenderer,
        of: MeshOf,
        detail: Detail,
        wanted: &mut Vec<(u64, MeshSpec)>,
    ) -> (Option<u64>, bool) {
        let key_of = |d: Detail| tree_draw::mesh_key(of, d);
        let key = key_of(detail);
        if renderer.has(key) {
            return (Some(key), true);
        }
        if self.empty.contains(&key) {
            return (None, true);
        }
        if !self.making.contains(&key) && !wanted.iter().any(|w| w.0 == key) {
            wanted.push((key, (of, detail)));
        }
        let others = match detail {
            Detail::Full => [Detail::Reduced, Detail::Low],
            Detail::Reduced => [Detail::Low, Detail::Full],
            Detail::Low => [Detail::Reduced, Detail::Full],
        };
        let other = others.into_iter().map(key_of).find(|k| renderer.has(*k));
        if other.is_none() {
            // Nothing made yet: the coarsest first.
            let low = key_of(Detail::Low);
            if !self.making.contains(&low)
                && !self.empty.contains(&low)
                && !wanted.iter().any(|w| w.0 == low)
            {
                wanted.push((low, (of, Detail::Low)));
            }
        }
        (other, false)
    }

    /// Makes a mesh on a worker thread, while fewer than `MAKING` are being made.
    fn make(&mut self, key: u64, (of, detail): MeshSpec) {
        if self.making.len() >= MAKING || !self.making.insert(key) {
            return;
        }
        let generator = self.generator.clone();
        let tx = self.made.0.clone();
        hearth_core::jobs::spawn(hearth_core::jobs::Priority::NearField, move || {
            let mesh = tree_draw::mesh(&generator.forest, of, detail);
            let _ = tx.send((key, mesh));
        });
    }

    /// Takes the meshes made, a few a frame.
    fn take_made(&mut self, ctx: &GpuContext, renderer: &mut TreeRenderer) {
        for (key, mesh) in self.made.1.try_iter().take(UPLOADS) {
            self.making.remove(&key);
            if mesh.indices.is_empty() {
                self.empty.insert(key);
            } else {
                renderer.upload(ctx, key, &mesh);
            }
        }
    }

    /// Takes the columns found, and lets go of those far off.
    fn take_found(&mut self, planet: Planet, eye: DVec3, radius: f64) {
        for f in self.found.1.try_iter() {
            self.asked.remove(&f.col);
            // Room for more to be found.
            self.scanned = None;
            if f.generation != self.generation {
                continue;
            }
            // Found again with the vegetation now: the trees felled or cut into here are as it
            // has them.
            if !self.hidden.is_empty() {
                let planet = self.generator.planet();
                self.hidden
                    .retain(|&(x, z)| ColumnPos::new(planet.wrap_x(x) >> 4, z >> 4) != f.col);
            }
            let n = f.trees.len();
            let (lo, hi, reach) = f.trees.iter().fold(
                (f64::INFINITY, f64::NEG_INFINITY, 0.0f32),
                |(lo, hi, r), t| {
                    (
                        lo.min(t.origin.y),
                        hi.max(t.origin.y + t.height as f64),
                        r.max(t.reach),
                    )
                },
            );
            let (x0, z0) = f.col.min_block_xz();
            let (lo, hi) = if n == 0 { (0.0, 0.0) } else { (lo, hi) };
            self.columns.insert(
                f.col,
                Column {
                    trees: f.trees,
                    light: vec![TreeLight::OPEN; n],
                    stale_light: true,
                    cover: f.cover,
                    generation: f.generation,
                    middle: DVec3::new(x0 as f64 + 8.0, (lo + hi) * 0.5, z0 as f64 + 8.0),
                    half: Vec3::new(8.0 + reach, ((hi - lo) * 0.5) as f32 + 1.0, 8.0 + reach),
                },
            );
        }
        let far = radius + MARGIN;
        self.columns
            .retain(|col, _| column_distance(&planet, eye, *col) <= far);
    }

    /// Asks for the columns within reach not found (or found with an older vegetation), the
    /// nearest first.
    fn ask_columns(&mut self, planet: Planet, eye: DVec3, radius: f64) -> usize {
        let Some(vegetation) = self.vegetation.as_ref() else {
            return 1;
        };
        let r = (radius / 16.0).ceil() as i32 + 1;
        let (cx, cz) = ((eye.x / 16.0).floor() as i32, (eye.z / 16.0).floor() as i32);
        // Nothing changed since the columns were last looked over: as many still unfound.
        let key = (cx, cz, self.generation, radius as i64);
        if let Some((k, unfound)) = self.scanned
            && k == key
        {
            return unfound;
        }
        let vegetation = vegetation.clone();
        let mut want: Vec<(f64, ColumnPos)> = Vec::new();
        for dz in -r..=r {
            for dx in -r..=r {
                let col = ColumnPos::new(planet.wrap_x((cx + dx) * 16) >> 4, cz + dz);
                let d = column_distance(&planet, eye, col);
                if d > radius + 8.0 {
                    continue;
                }
                if self
                    .columns
                    .get(&col)
                    .is_some_and(|c| c.generation == self.generation)
                {
                    continue;
                }
                want.push((d, col));
            }
        }
        let unfound = want.len();
        self.scanned = Some((key, unfound));
        want.retain(|w| !self.asked.contains(&w.1));
        want.sort_by(|a, b| a.0.total_cmp(&b.0));
        let room = FINDING.saturating_sub(self.asked.len());
        for (_, col) in want.into_iter().take(room) {
            self.asked.insert(col);
            let generator = self.generator.clone();
            let vegetation = vegetation.clone();
            let generation = self.generation;
            let tx = self.found.0.clone();
            hearth_core::jobs::spawn(hearth_core::jobs::Priority::NearField, move || {
                let (trees, cover) = find(&generator, &vegetation, col);
                let _ = tx.send(Found {
                    col,
                    generation,
                    trees,
                    cover,
                });
            });
        }
        unfound
    }
}

/// The snow on a column's trees at a time of year (0..200 of `TreeInstance::climate`'s byte).
fn snow_on(cover: &SeasonalCover, year_frac: f64) -> u8 {
    let snow = cover.snow_depth_m(year_frac);
    ((snow / FULL_SNOW_M).min(1.0) * 200.0) as u8
}

/// Horizontal distance (m) from the eye to a column's middle, across the seam.
fn column_distance(planet: &Planet, eye: DVec3, col: ColumnPos) -> f64 {
    let mid = DVec3::new(col.x as f64 * 16.0 + 8.0, eye.y, col.z as f64 * 16.0 + 8.0);
    let d = planet.delta(eye, mid);
    (d.x * d.x + d.z * d.z).sqrt()
}

/// The trees drawn as meshes with their foot in a column, and its seasons' snow.
fn find(
    generator: &WorldGenerator,
    vegetation: &Vegetation,
    col: ColumnPos,
) -> (Vec<Tree>, SeasonalCover) {
    let (x0, z0) = col.min_block_xz();
    let placed: Vec<PlacedTree> = generator
        .features()
        .trees_in(generator, vegetation, (x0, z0), (x0 + 15, z0 + 15))
        .into_iter()
        .filter(|t| !t.is_cut(vegetation))
        .collect();
    let tints = crate::scene::column_tints(generator, col);
    let trees = placed
        .iter()
        .map(|t| {
            let (origin, turn) = t.frame();
            let (lx, lz) = (
                (t.foot[0] - x0).rem_euclid(16) as usize,
                (t.foot[2] - z0).rem_euclid(16) as usize,
            );
            let h = hearth_math::hash::hash_2d(0x7ee5_f00d, t.foot[0], t.foot[2]);
            Tree {
                species: t.species,
                stage: t.stage,
                variant: t.variant,
                remains: t.remains,
                origin: DVec3::from_array(origin),
                turn,
                foot: BlockPos::new(t.foot[0], t.foot[1], t.foot[2]),
                top: t.foot[1] + t.template.max[1] + 1,
                height: t.template.max[1] as f32 + 1.0,
                reach: t.template.reach() as f32,
                climate: tints.climate[lz * 16 + lx],
                phase: (h >> 24) as u8,
                radius: t.template.diameter_m * 0.5,
                placed: t.clone(),
            }
        })
        .collect();
    let cover = SeasonalCover::compute(&crate::season_cover::column_normals(generator, col));
    (trees, cover)
}

/// The light where a tree stands: the sky's and the blocks' about its foot (beside its trunk,
/// whose own blocks are dark within) and the sky's over its crown.
fn light_of(mirror: &CubeMap, t: &Tree) -> TreeLight {
    let beside = [(-3, 0), (3, 0), (0, -3), (0, 3)]
        .map(|(dx, dz)| BlockPos::new(t.foot.x + dx, t.foot.y + 1, t.foot.z + dz));
    TreeLight {
        foot: beside
            .iter()
            .map(|p| mirror.sky_light(*p))
            .max()
            .unwrap_or(15),
        crown: mirror.sky_light(BlockPos::new(t.foot.x, t.top, t.foot.z)),
        block: beside
            .iter()
            .map(|p| mirror.block_light(*p))
            .max()
            .unwrap_or(0),
    }
}

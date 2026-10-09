//! The animals of the world on the server (V2-7): the populations of the regions about the
//! player, advanced with the calendar, their groups and small animals brought into the world
//! near the player and folded back as the player leaves, saved with the world.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use glam::{DVec2, DVec3};
use hearth_content::butchery::{Carcass, carcass_id};
use hearth_fauna::ecology::{Cause, Ecology, REGION_CELLS, Region, Remains};
use hearth_fauna::habitat::{GenLand, TreeYields};
use hearth_fauna::live::{AnimalView, Cell, Footing, Ground, Live, Now, Stage};
use hearth_fauna::mind::Presence;
use hearth_fauna::species::Catalog;
use hearth_fauna::voices::Called;
use hearth_items::{Items, Stack};
use hearth_math::BlockPos;
use hearth_physics::{Mover, Stance};
use hearth_world::{BlockRegistry, BlockStateId, CubeMap};
use rustc_hash::FxHashMap;

use crate::scene::LocalWorld;

/// The file the populations are saved in.
const FILE: &str = "fauna.json.zst";
/// Regions made on workers at once.
const MAKERS: usize = 3;
/// Regions this many regions or more from the player's own are set aside (E4.1 §4.7): kept as
/// they are saved (the land's part let go, made again when they are brought back), beyond the
/// animals' share of memory written into the world's folder, so a journey keeps in memory only
/// the regions about the player.
const ASIDE_STEPS: i64 = 2;
/// The folder in the world's folder the regions set aside beyond memory are written to.
const ASIDE_DIR: &str = "fauna";

/// The loaded terrain as animals walk on it.
pub struct MapGround<'a> {
    pub map: &'a CubeMap,
    pub reg: &'a BlockRegistry,
    pub lw: &'a LocalWorld,
    /// What each block state is to an animal ([`cells_of`]).
    pub cells: &'a [Cell],
}

/// What each block state is to an animal: a tree's trunk, limbs and foliage by their names
/// (`_log`, `_branch`, `_leaves`), water, the solid, and the open (air and the plants a body
/// pushes through).
pub fn cells_of(reg: &BlockRegistry) -> Vec<Cell> {
    (0..reg.state_count())
        .map(|i| {
            let s = BlockStateId(i as u16);
            if s.is_air() {
                return Cell::Open;
            }
            let path = reg.block_of(s).name.path();
            if reg.fluid_amount(s) > 0 {
                Cell::Water
            } else if path.ends_with("_log") {
                Cell::Trunk
            } else if path.ends_with("_branch") {
                Cell::Limb
            } else if path.ends_with("_leaves") {
                Cell::Leaves
            } else if reg.collision_shape(s).is_empty() {
                Cell::Plant
            } else {
                Cell::Solid
            }
        })
        .collect()
}

/// The surface at a foot's place in a map as it takes signs: how plainly it takes a print and
/// its height — snow lying where the feet are (its top), otherwise the ground under them (not
/// the plants on it).
pub fn sign_surface(map: &CubeMap, reg: &BlockRegistry, x: f64, z: f64, y: f64) -> (f32, f64) {
    let planet = map.planet();
    let (bx, bz) = (planet.wrap_x(x.floor() as i32), z.floor() as i32);
    let at = |yy: i32| {
        map.block(BlockPos::new(bx, yy, bz))
            .filter(|s| !s.is_air())
            .map(|s| (s, reg.block_of(s).def.sound.clone()))
    };
    let feet = (y + 0.05).floor() as i32;
    if let Some((s, sound)) = at(feet)
        && sound == "snow"
    {
        let top = reg
            .outline_shape(s)
            .boxes
            .iter()
            .map(|b| b.max.y)
            .fold(0.0, f64::max);
        return (1.0, feet as f64 + top);
    }
    let plain = match at((y - 0.05).floor() as i32).map(|(_, s)| s).as_deref() {
        Some("snow") => 1.0,
        Some("sand") => 0.8,
        Some("soil") => 0.7,
        Some("moss") => 0.3,
        Some("grass") | Some("wet_grass") => 0.2,
        _ => 0.0,
    };
    (plain, y)
}

/// The surface of the ground in the column at (x, z) of a map, searching down from `from` for
/// `depth` blocks: the top of a block that stops a foot with room above it.
pub fn surface_in(
    map: &CubeMap,
    reg: &BlockRegistry,
    x: f64,
    z: f64,
    from: i32,
    depth: i32,
) -> Option<Footing> {
    let planet = map.planet();
    let bx = planet.wrap_x(x.floor() as i32);
    let bz = z.floor() as i32;
    let mut above = map.block(BlockPos::new(bx, from + 1, bz))?;
    for y in (from - depth..=from).rev() {
        let s = map.block(BlockPos::new(bx, y, bz))?;
        let shape = reg.collision_shape(s);
        let open_above = reg.collision_shape(above).is_empty();
        if !shape.is_empty() && open_above {
            // The water over it, as deep as it stands.
            let mut depth = 0.0;
            if reg.fluid_amount(above) > 0 {
                for k in 1..6 {
                    match map.block(BlockPos::new(bx, y + k, bz)) {
                        Some(w) if reg.fluid_amount(w) > 0 => {
                            depth = k as f64 + 1.0 - shape.top();
                        }
                        _ => break,
                    }
                }
            }
            // Natural ground stands where its smooth surface is (S §8.1), within its voxels.
            let top = if reg.has(s, hearth_world::StateFlags::NATURAL) {
                smooth_top(map, reg, x, z, y).unwrap_or(y as f64 + shape.top())
            } else {
                y as f64 + shape.top()
            };
            let depth = if depth > 0.0 {
                depth + (y as f64 + shape.top() - top)
            } else {
                0.0
            };
            return Some(Footing {
                y: top,
                water: depth > 0.0,
                depth,
            });
        }
        // Standing water: the ground under it is wet footing.
        above = s;
    }
    None
}

/// The smooth ground's surface in a column whose natural voxel at `y` is the top one: its
/// fill's depth from the voxel's middle (one lookup, so a herd's paths stay cheap; on a slope
/// the fill's depth is across it, a few centimetres short of the height).
pub fn smooth_top(map: &CubeMap, reg: &BlockRegistry, x: f64, z: f64, y: i32) -> Option<f64> {
    let p = BlockPos::new(map.planet().wrap_x(x.floor() as i32), y, z.floor() as i32);
    let q = map.fill(p, reg)?;
    let depth = hearth_world::fill::Fill::depth(q) as f64;
    Some((y as f64 + 0.5 + depth).clamp(y as f64, y as f64 + 1.0))
}

impl MapGround<'_> {
    fn surface(&self, x: f64, z: f64, from: i32, depth: i32) -> Option<Footing> {
        surface_in(self.map, self.reg, x, z, from, depth)
    }
}

/// The ground under an animal's feet in a map of blocks, for its pose: heights in its frame
/// (it stands at `at`, facing `yaw`).
pub struct CubeFooting<'a> {
    pub map: &'a CubeMap,
    pub reg: &'a BlockRegistry,
    pub at: DVec3,
    pub yaw: f32,
}

impl hearth_fauna::anim::Footing for CubeFooting<'_> {
    fn ground(&self, p: glam::Vec3) -> Option<f32> {
        let w = glam::Quat::from_rotation_y(self.yaw) * p;
        let (x, z) = (self.at.x + w.x as f64, self.at.z + w.z as f64);
        let from = (self.at.y + p.y as f64).floor() as i32 + 2;
        surface_in(self.map, self.reg, x, z, from, 5).map(|f| (f.y - self.at.y) as f32)
    }
}

impl MapGround<'_> {
    /// The generated ground where the blocks are not loaded (beyond the near terrain, where
    /// the distant terrain shows the same heights): the top of the ground or the water.
    fn generated(&self, x: f64, z: f64) -> Footing {
        let s = self.lw.terrain().sample(x.floor() as i32, z.floor() as i32);
        let depth = s.water_i().saturating_sub(s.height_i()).max(0) as f64;
        Footing {
            y: s.height_i() as f64,
            water: depth > 0.0,
            depth,
        }
    }

    fn loaded(&self, x: f64, z: f64, y: f64) -> bool {
        let planet = self.map.planet();
        let p = BlockPos::new(
            planet.wrap_x(x.floor() as i32),
            y.floor() as i32,
            z.floor() as i32,
        );
        self.map.block(p).is_some()
    }
}

impl Ground for MapGround<'_> {
    fn footing(&self, x: f64, z: f64, y: f64) -> Option<Footing> {
        if !self.loaded(x, z, y) {
            return Some(self.generated(x, z));
        }
        self.surface(x, z, y.floor() as i32 + 2, 8)
    }

    fn cell(&self, x: i32, y: i32, z: i32) -> Option<Cell> {
        let p = BlockPos::new(self.map.planet().wrap_x(x), y, z);
        match self.map.block(p) {
            Some(s) => self.cells.get(s.0 as usize).copied(),
            None => {
                // Not loaded: the generated ground, its water, and air.
                let f = self.generated(x as f64 + 0.5, z as f64 + 0.5);
                let y = y as f64;
                Some(if y < f.y {
                    Cell::Solid
                } else if y < f.level() {
                    Cell::Water
                } else {
                    Cell::Open
                })
            }
        }
    }

    fn sign_surface(&self, x: f64, z: f64, y: f64) -> (f32, f64) {
        sign_surface(self.map, self.reg, x, z, y)
    }

    fn top(&self, x: f64, z: f64) -> Option<Footing> {
        // The generated ground, then the blocks about it (the player's changes, the trees).
        let h = self.lw.surface_y(x, z);
        if !self.loaded(x, z, h) {
            return Some(self.generated(x, z));
        }
        self.surface(x, z, h.floor() as i32 + 3, 10)
    }
}

/// The animals of the world.
pub struct Fauna {
    pub eco: Ecology,
    pub live: Live,
    /// What each block state is to an animal.
    pub cells: Vec<Cell>,
    yields: TreeYields,
    /// Years since the world began the populations are simulated to.
    pub years: f64,
    /// Years since the world began now (the populations go on a few days at a time behind).
    pub now: f64,
    /// For the calls of the animals not in the world.
    rng: hearth_math::hash::Rng,
    /// Regions being made on workers: their keys, and where each will come.
    making: Vec<((i64, i64), std::sync::mpsc::Receiver<Region>)>,
    /// Regions set aside, as saved, by key: when each was (to write out the longest set aside
    /// first) and the region.
    aside: FxHashMap<(i64, i64), (u64, Region)>,
    /// Regions set aside so far.
    asides: u64,
    /// The world's folder, if it is saved.
    dir: Option<PathBuf>,
    /// What befell the kept animals since this was last taken (V2-12).
    pub tidings: Vec<hearth_fauna::herd::Tiding>,
}

/// What is saved of the populations.
#[derive(serde::Deserialize)]
struct Saved {
    years: f64,
    next_id: u64,
    regions: Vec<Region>,
    /// The animals people keep (V2-12), apart from the populations' numbers.
    #[serde(default)]
    kept: Vec<hearth_fauna::live::KeptSaved>,
}

/// [`Saved`] as written: the regions where they are.
#[derive(serde::Serialize)]
struct Saving<'a> {
    years: f64,
    next_id: u64,
    regions: Vec<&'a Region>,
    kept: Vec<hearth_fauna::live::KeptSaved>,
}

/// A catch-up this long (years: a week of the world a tick, the Observer's year a second and
/// faster) is lived at the animals' demographic tier (D210).
const FAST_YEARS: f64 = 1.0 / 48.0;
/// The longest step the animals about the player live at a time (s): the world going faster
/// than lived (a rest, the time speed) is lived by them in steps no longer.
const LIVE_STEP_S: f32 = 0.5;
/// The most steps they live in a tick: up to a hundred times as fast as lived (faster, they lag
/// the world's clock until a catch-up passes to the populations' tier).
const LIVE_STEPS: f32 = 10.0;

impl Fauna {
    /// The world's animals: as saved in `dir`, or new.
    pub fn new(
        lw: &LocalWorld,
        seed: u64,
        year_offset: f64,
        years: f64,
        dir: Option<&Path>,
    ) -> Self {
        let catalog = Arc::new(Catalog::new(&lw.content));
        let yields = TreeYields::new(&lw.generator, &lw.content);
        let land = GenLand {
            wg: &lw.generator,
            veg: &lw.vegetation,
            catalog: &catalog,
            trees: &yields,
        };
        let mut eco = Ecology::new(catalog.clone(), seed, year_offset, &land);
        let mut at = years;
        let mut live = Live::new(seed);
        let mut aside = FxHashMap::default();
        let mut asides = 0;
        if let Some(d) = dir
            && let Some(saved) = load(&d.join(FILE))
        {
            eco.next_id = saved.next_id;
            at = saved.years;
            // Set aside, each brought back on a worker as the player comes near (E4.1 §4.4: a
            // save opens without making its animals' land again first). A region written out
            // since it was saved is not as saved: the save's stands.
            for r in saved.regions {
                if let Some(path) = aside_path(Some(d), r.key) {
                    let _ = std::fs::remove_file(path);
                }
                asides += 1;
                aside.insert(r.key, (asides, r));
            }
            live.restore_kept(&catalog, saved.kept);
        }
        Self {
            eco,
            live,
            cells: cells_of(&lw.reg),
            yields,
            years: at,
            now: years.max(at),
            rng: hearth_math::hash::Rng::new(seed ^ 0x000c_a115),
            making: Vec::new(),
            tidings: Vec::new(),
            aside,
            asides,
            dir: dir.map(Path::to_path_buf),
        }
    }

    /// The regions about a place: its own and the eight about it (fewer on a small planet).
    fn regions_about(&self, at: DVec3) -> Vec<(i64, i64)> {
        let (ki, kj) = self.eco.region_key(at.x, at.z);
        let around = (self.eco.cells_around / REGION_CELLS).max(1);
        let rows = self.eco.rows / REGION_CELLS;
        let mut keys = vec![(ki, kj)];
        for dj in -1..=1 {
            for di in -1..=1 {
                let k = ((ki + di).rem_euclid(around), kj + dj);
                if k.1 >= -rows - 1 && k.1 <= rows && !keys.contains(&k) {
                    keys.push(k);
                }
            }
        }
        keys
    }

    /// Makes every region about a place now (tools: the screenshot's animals).
    pub fn ensure_about(&mut self, lw: &LocalWorld, at: DVec3) {
        let catalog = self.eco.catalog.clone();
        let land = GenLand {
            wg: &lw.generator,
            veg: &lw.vegetation,
            catalog: &catalog,
            trees: &self.yields,
        };
        for key in self.regions_about(at) {
            match self.aside.remove(&key) {
                Some((_, r)) => self.eco.restore(&land, r),
                None => self.eco.ensure_region(&land, key, self.years),
            }
        }
    }

    /// Sets aside the regions far from `at` (none of their animals in the world), and writes
    /// out the longest set aside beyond the animals' share of memory (a world not saved lets
    /// them go: made again, they hold their animals as the land does).
    fn set_aside(&mut self, at: DVec3) {
        let (ki, kj) = self.eco.region_key(at.x, at.z);
        let around = (self.eco.cells_around / REGION_CELLS).max(1);
        let far: Vec<(i64, i64)> = self
            .eco
            .regions
            .iter()
            .filter(|(k, r)| {
                let di = (k.0 - ki).rem_euclid(around);
                di.min(around - di).max((k.1 - kj).abs()) >= ASIDE_STEPS
                    && r.groups.iter().all(|g| !g.live)
            })
            .map(|(k, _)| *k)
            .collect();
        for key in far {
            if let Some(mut r) = self.eco.regions.remove(&key) {
                r.strip();
                self.asides += 1;
                self.aside.insert(key, (self.asides, r));
            }
        }
        let budget = hearth_core::memory::budget(hearth_core::memory::Kind::Animals) as usize;
        let mut held: usize = self.aside.values().map(|(_, r)| r.bytes()).sum();
        while held > budget {
            let Some(key) = self
                .aside
                .iter()
                .min_by_key(|(_, (n, _))| *n)
                .map(|(k, _)| *k)
            else {
                break;
            };
            let Some((_, r)) = self.aside.remove(&key) else {
                break;
            };
            held = held.saturating_sub(r.bytes());
            match aside_path(self.dir.as_deref(), key) {
                Some(path) => hearth_core::jobs::spawn(hearth_core::jobs::Priority::Background, {
                    move || write_region(&path, &r)
                }),
                None => log::debug!("region {key:?} of the populations let go (not saved)"),
            }
        }
    }

    /// The memory the populations hold (bytes): the regions about the player, those set aside.
    pub fn memory(&self) -> u64 {
        self.eco
            .regions
            .values()
            .chain(self.aside.values().map(|(_, r)| r))
            .map(|r| r.bytes() as u64)
            .sum()
    }

    /// How the animals sense a person from how they move: the noise of their going (by gait
    /// and the ground underfoot, as the player hears their own steps), how plain they stand
    /// (upright, crouched or crawling; moving or still; among plants), how high.
    #[allow(clippy::too_many_arguments)]
    pub fn presence_of(
        &self,
        mover: &Mover,
        facing: f32,
        shouting: bool,
        hurt: usize,
        light: f32,
        map: &CubeMap,
        reg: &BlockRegistry,
    ) -> Presence {
        use hearth_audio::Surface as S;
        let speed = glam::DVec2::new(mover.vel.x, mover.vel.z).length() as f32;
        let (gait, plain, height) = match mover.stance {
            Stance::Crouching => (if speed > 0.2 { 0.3 } else { 0.05 }, 0.5, 1.0),
            Stance::Crawling => (if speed > 0.1 { 0.3 } else { 0.03 }, 0.3, 0.4),
            Stance::Swimming => (0.45, 0.6, 0.5),
            Stance::Climbing => (0.45, 1.0, 1.7),
            Stance::Standing => (
                if speed < 0.2 {
                    0.05
                } else if speed < 2.0 {
                    0.55
                } else if speed < 4.0 {
                    0.8
                } else {
                    1.0
                },
                1.0,
                1.7,
            ),
        };
        let loud: f32 = match crate::hearing::surface_under(map, reg, mover.pos) {
            S::Leaves => 1.35,
            S::Gravel | S::Shallow => 1.2,
            S::Stone | S::Wood | S::Ice => 1.0,
            S::Soil | S::Mud | S::Sand => 0.85,
            S::Grass => 0.8,
            S::Snow => 0.6,
            S::Moss => 0.5,
        };
        let still = if speed < 0.2 { 0.45 } else { 1.0 };
        // Among plants up to the body: hidden in part, the more as it is low.
        let among = map
            .block(BlockPos::containing(mover.pos + DVec3::Y * 0.5))
            .and_then(|s| self.cells.get(s.0 as usize))
            .is_some_and(|c| matches!(c, Cell::Plant | Cell::Leaves));
        let cover = match (among, height < 1.2) {
            (true, true) => 0.45,
            (true, false) => 0.8,
            _ => 1.0,
        };
        let crouched = match mover.stance {
            Stance::Crouching => 0.35,
            Stance::Crawling => 0.5,
            _ => 0.1,
        };
        // By a fire (or another light): the light where they stand.
        let by_fire = map.block_light(BlockPos::containing(mover.pos + DVec3::Y)) >= 8;
        Presence {
            pos: mover.pos,
            noise: if shouting {
                1.0
            } else {
                (gait * loud).min(1.0)
            },
            plain: plain * still * cover,
            height,
            facing,
            upright: mover.stance == Stance::Standing,
            shouting,
            // Small and weak to a hunter: crouched or crawling, in the dark, hurt.
            vulnerable: (crouched + (1.0 - light) * 0.35 + hurt.min(3) as f32 * 0.12).min(1.0),
            by_fire,
            running: speed > 3.5,
        }
    }

    /// A tick of `dt` seconds at `years` (since the world began) and the local `hour` (0–1)
    /// with the player at `player`; `tick` counts ticks.
    pub fn tick(
        &mut self,
        lw: &LocalWorld,
        presence: &Presence,
        now: &Now,
        years: f64,
        dt: f32,
        tick: u64,
    ) {
        let player = presence.pos;
        self.now = years;
        // The years passing at a stroke (the Observer's fast-forward, D210): the populations as
        // numbers every tick, so that the warp does not save them up, and none of the animals
        // drawn out about the eye while they do (the view is the globe's).
        let fast = years - self.years > FAST_YEARS;
        if tick.is_multiple_of(40) || fast {
            // The regions about the player, the player's own first, made on workers (each takes a
            // tenth of a second or more), a few at a time, taken in when made.
            let mut made = Vec::new();
            self.making.retain(|(_, rx)| match rx.try_recv() {
                Ok(r) => {
                    made.push(r);
                    false
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
                Err(std::sync::mpsc::TryRecvError::Empty) => true,
            });
            for r in made {
                log::info!(
                    "region {:?} of the populations made: {} groups",
                    r.key,
                    r.groups.len()
                );
                // Brought back: no longer set aside.
                self.aside.remove(&r.key);
                self.eco.adopt(r);
            }
            self.set_aside(player);
            let wanted: Vec<(i64, i64)> = self
                .regions_about(player)
                .into_iter()
                .filter(|k| {
                    !self.eco.regions.contains_key(k) && self.making.iter().all(|(m, _)| m != k)
                })
                .take(MAKERS.saturating_sub(self.making.len()))
                .collect();
            for (n, key) in wanted.into_iter().enumerate() {
                // Ids of their own, apart from the others' and from those born meanwhile.
                let ids = self.eco.next_id + 1_000_000 * (n as u64 + 1 + self.making.len() as u64);
                let mut maker = self.eco.maker(ids);
                let (wg, veg, trees) = (
                    lw.generator.clone(),
                    lw.vegetation.clone(),
                    self.yields.clone(),
                );
                let years = self.years;
                // Set aside (a copy: it stays aside until it is back, and is saved so), written
                // out, or new.
                let kept = self.aside.get(&key).map(|(_, r)| r.clone());
                let written = aside_path(self.dir.as_deref(), key).filter(|p| p.exists());
                let (tx, rx) = std::sync::mpsc::channel();
                let spawned = std::thread::Builder::new()
                    .name("fauna region".into())
                    .spawn(move || {
                        let cat = maker.catalog.clone();
                        let land = GenLand {
                            wg: &wg,
                            veg: &veg,
                            catalog: &cat,
                            trees: &trees,
                        };
                        match kept.or_else(|| written.and_then(|p| read_region(&p))) {
                            Some(r) => {
                                maker.restore(&land, r);
                                // Long away: the years it missed as numbers (D210).
                                if maker.regions.values().any(|r| years - r.time > FAST_YEARS) {
                                    maker.advance_coarse(years);
                                }
                            }
                            None => maker.ensure_region(&land, key, years),
                        }
                        if let Some(r) = maker.regions.into_values().next() {
                            let _ = tx.send(r);
                        }
                    });
                if spawned.is_ok() {
                    self.making.push((key, rx));
                }
            }
            // The populations to the calendar every few days, in steps of at most 1/32 year — as
            // numbers alone when years pass at a stroke (the Observer's fast-forward, D210).
            if years - self.years >= 1.0 / 128.0 {
                let t0 = std::time::Instant::now();
                if years - self.years > FAST_YEARS {
                    self.eco.advance_coarse(years);
                } else {
                    self.eco.advance(years, 1.0 / 32.0);
                }
                // A long catch-up (years skipped) told of, for the profiler.
                if t0.elapsed().as_secs_f64() > 0.01 {
                    log::debug!(
                        "the animals caught up {:.2} years in {:.3} s over {} regions",
                        years - self.years,
                        t0.elapsed().as_secs_f64(),
                        self.eco.regions.len()
                    );
                }
                self.years = years;
            }
            let ground = MapGround {
                map: &lw.map,
                reg: &lw.reg,
                lw,
                cells: &self.cells,
            };
            if !fast {
                self.live.fold(&mut self.eco, player);
                self.live.materialize(&mut self.eco, &ground, player);
            }
        }
        let ground = MapGround {
            map: &lw.map,
            reg: &lw.reg,
            lw,
            cells: &self.cells,
        };
        if !fast {
            let lived = dt.min(LIVE_STEP_S * LIVE_STEPS);
            let n = (lived / LIVE_STEP_S).ceil().max(1.0);
            for _ in 0..n as usize {
                self.live
                    .step(&self.eco, &ground, Some(presence), now, lived / n);
            }
        }
        // The kept animals through the calendar: born, grown, in young, dying of age.
        if tick.is_multiple_of(40) || fast || self.live.years.is_none_or(|y| years - y > 0.01) {
            let cat = self.eco.catalog.clone();
            let tidings = self.live.tend(&cat, years, now.year_frac, now.southern);
            self.tidings.extend(tidings);
        }
    }

    /// The animals near the player, for the client.
    pub fn views(&self) -> Vec<AnimalView> {
        self.live.views()
    }

    /// The calls of the animals about the player that are not in the world, this step of `dt`
    /// seconds ([`hearth_fauna::voices::chorus`]).
    pub fn chorus(&mut self, at: DVec3, now: &Now, dt: f32) -> Vec<Called> {
        hearth_fauna::voices::chorus(&self.eco, at, now, dt, &mut self.rng)
    }

    /// The carcass an animal leaves, if it is big enough to work (a male of a species whose
    /// sexes are alike leaves a grown one's).
    pub fn carcass_of(
        &self,
        items: &Items,
        species: u16,
        stage: Stage,
        female: bool,
    ) -> Option<String> {
        let sp = self.eco.catalog.species.get(species as usize)?;
        let which = match (stage, female) {
            (Stage::Adult, true) => Carcass::Grown,
            (Stage::Adult, false) => Carcass::Male,
            _ => Carcass::Young,
        };
        let id = carcass_id(&sp.id, which);
        if items.get(&id).is_some() {
            return Some(id);
        }
        let grown = carcass_id(&sp.id, Carcass::Grown);
        (which == Carcass::Male && items.get(&grown).is_some()).then_some(grown)
    }

    /// The dead taken out of the world as the carcasses they leave: what is left of each
    /// (a hunter's kill eaten from), where, which way it lies; and, in words, those the person
    /// at `near` brought down that fell within sight of them.
    pub fn carcasses(
        &mut self,
        items: &Items,
        near: DVec3,
    ) -> (Vec<(Stack, DVec3, f32)>, Vec<String>) {
        let cat = self.eco.catalog.clone();
        let mut out = Vec::new();
        let mut words = Vec::new();
        for b in self.live.take_bodies(&cat) {
            let sp = &cat.species[b.species as usize];
            let how = match b.killed_by {
                Some(h) => format!(
                    "killed by a {}",
                    cat.species[h as usize].name.to_lowercase()
                ),
                None if b.by_person => "killed by the person".to_owned(),
                None => "dead".to_owned(),
            };
            if b.by_person && (b.pos - near).length() < 150.0 {
                words.push(format!("The {} falls.", sp.name.to_lowercase()));
            }
            log::info!(
                "A {} lies {how}, {:.0}% left",
                sp.name.to_lowercase(),
                b.left * 100.0
            );
            if let Some(id) = self.carcass_of(items, b.species, b.stage, b.female) {
                let mut s = Stack::one(&id);
                s.condition = b.left;
                out.push((s, b.pos, b.yaw));
            }
        }
        (out, words)
    }

    /// The remains of the populations' dead lying within `radius` of the player, taken into the
    /// world as carcasses: with what is left of them and how far they have gone off at
    /// `air_c`, where (on the ground of the column), which way they lie. The rotted are gone.
    /// The time since a death, in the populations' years, is reckoned in days (`days_per_year`
    /// of them to the year).
    #[allow(clippy::too_many_arguments)]
    pub fn found(
        &mut self,
        items: &Items,
        content: &hearth_content::Content,
        lw: &LocalWorld,
        at: DVec3,
        radius: f64,
        air_c: f32,
        days_per_year: f64,
    ) -> Vec<(Stack, DVec3, f32)> {
        let mut out = Vec::new();
        for m in self.eco.take_remains([at.x, at.z], radius, self.now) {
            let Some(id) = self.carcass_of(items, m.species, m.stage, m.female) else {
                continue;
            };
            let Some(kind) = items.get(&id) else {
                continue;
            };
            let hours = ((self.now - m.time).max(0.0) * days_per_year * 24.0) as f32;
            let decay = hearth_craft::food::keeps_days(content, kind).map_or(0.0, |keeps| {
                hearth_craft::food::decay_per_hour(keeps, air_c, 0.0) * hours
            });
            if decay >= 1.5 {
                continue;
            }
            let mut s = Stack::one(&id);
            s.condition = m.left;
            s.decay = decay;
            let col = lw
                .terrain()
                .sample(m.at[0].floor() as i32, m.at[1].floor() as i32);
            let pos = DVec3::new(m.at[0], col.height as f64 + 1.0, m.at[1]);
            let yaw = ((m.time * 7919.0).fract() * std::f64::consts::TAU) as f32;
            out.push((s, pos, yaw));
        }
        out
    }

    /// Where ravens circle over fresh remains within sight of the player: the way to the
    /// nearest not told of within the hour, on the ground (each flock in its turn).
    pub fn ravens(&mut self, at: DVec3) -> Option<DVec2> {
        let m = self.eco.raven(
            [at.x, at.z],
            2000.0,
            self.now,
            4.0 / 365.0,
            1.0 / (365.0 * 24.0),
        )?;
        Some(DVec2::new(m.at[0] - at.x, m.at[1] - at.z))
    }

    /// Tests and bots: a grown animal of a species dies at a place (a natural death a test may
    /// force), to lie there when the player comes near.
    pub fn die(&mut self, species: &str, at: DVec3) {
        let Some(si) = self.eco.catalog.index(species) else {
            return;
        };
        let key = self.eco.region_key(at.x, at.z);
        if let Some(r) = self.eco.regions.get_mut(&key) {
            r.remains.push(Remains {
                species: si as u16,
                stage: Stage::Adult,
                female: true,
                at: [at.x, at.z],
                time: self.now,
                left: 1.0,
                half_days: hearth_fauna::ecology::half_days(self.eco.catalog.species[si].mass_kg),
                cause: Cause::Natural,
                told: -1.0,
            });
        }
    }

    /// A wild animal of a species stands at a place, of no group (tests and bots: a young one
    /// alone, as if its mother were gone). Its id.
    pub fn bring(&mut self, species: &str, young: bool, female: bool, at: DVec3) -> Option<u64> {
        let si = self.eco.catalog.index(species)?;
        let stage = if young { Stage::Young } else { Stage::Adult };
        Some(self.live.place(si as u16, stage, female, at, 0.0))
    }

    /// The groups of the loaded regions: species, where, how many.
    pub fn census(&self) -> Vec<(u16, glam::DVec2, u32)> {
        self.eco
            .regions
            .values()
            .flat_map(|r| r.groups.iter())
            .filter(|g| g.size() > 0)
            .map(|g| (g.species, glam::DVec2::new(g.pos[0], g.pos[1]), g.size()))
            .collect()
    }

    /// Saves the populations into `dir` (the animals in the world folded back first, in a
    /// copy).
    pub fn save(&self, dir: &Path) {
        let mut eco = self.eco.clone();
        let mut live = self.live.clone();
        // Everything folds, as if the player were far away.
        live.fold(&mut eco, DVec3::new(f64::MAX / 4.0, 0.0, f64::MAX / 4.0));
        // With those set aside (those written out stay where they are).
        let mut regions: Vec<&Region> = eco
            .regions
            .values()
            .chain(self.aside.values().map(|(_, r)| r))
            .collect();
        regions.sort_by_key(|r| r.key);
        let keys: Vec<(i64, i64)> = regions.iter().map(|r| r.key).collect();
        let saved = Saving {
            years: self.years,
            next_id: eco.next_id,
            regions,
            kept: self.live.kept_saved(&self.eco.catalog),
        };
        let json = match serde_json::to_vec(&saved) {
            Ok(j) => j,
            Err(e) => {
                log::error!("fauna not saved: {e}");
                return;
            }
        };
        match zstd::encode_all(json.as_slice(), 3) {
            Ok(z) => {
                if let Err(e) = std::fs::write(dir.join(FILE), z) {
                    log::error!("fauna not saved: {e}");
                    return;
                }
            }
            Err(e) => {
                log::error!("fauna not saved: {e}");
                return;
            }
        }
        // Written out before, saved now: the file is old.
        for key in keys {
            if let Some(path) = aside_path(Some(dir), key)
                && path.exists()
            {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

/// Where a region set aside beyond memory is written in a world's folder.
fn aside_path(dir: Option<&Path>, key: (i64, i64)) -> Option<PathBuf> {
    Some(
        dir?.join(ASIDE_DIR)
            .join(format!("{}_{}.json.zst", key.0, key.1)),
    )
}

/// Writes a region set aside (whole or not at all).
fn write_region(path: &Path, r: &Region) {
    let written = (|| -> std::io::Result<()> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let json = serde_json::to_vec(r)?;
        let z = zstd::encode_all(json.as_slice(), 3)?;
        let part = path.with_extension("part");
        std::fs::write(&part, z)?;
        std::fs::rename(&part, path)
    })();
    if let Err(e) = written {
        log::error!("{} not written: {e}", path.display());
    }
}

/// A region written out.
fn read_region(path: &Path) -> Option<Region> {
    let bytes = std::fs::read(path).ok()?;
    let json = zstd::decode_all(bytes.as_slice()).ok()?;
    match serde_json::from_slice(&json) {
        Ok(r) => Some(r),
        Err(e) => {
            log::error!("{} unreadable ({e}): made anew", path.display());
            None
        }
    }
}

fn load(path: &Path) -> Option<Saved> {
    let bytes = std::fs::read(path).ok()?;
    let json = zstd::decode_all(bytes.as_slice()).ok()?;
    match serde_json::from_slice(&json) {
        Ok(s) => Some(s),
        Err(e) => {
            log::error!(
                "{} unreadable ({e}); the animals start anew",
                path.display()
            );
            None
        }
    }
}

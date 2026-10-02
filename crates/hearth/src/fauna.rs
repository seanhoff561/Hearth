//! The animals of the world on the server (V2-7): the populations of the regions about the
//! player, advanced with the calendar, their groups and small animals brought into the world
//! near the player and folded back as the player leaves, saved with the world.

use std::path::Path;
use std::sync::Arc;

use glam::DVec3;
use hearth_fauna::ecology::{Ecology, REGION_CELLS, Region};
use hearth_fauna::habitat::{GenLand, TreeYields};
use hearth_fauna::live::{AnimalView, Footing, Ground, Live};
use hearth_fauna::species::Catalog;
use hearth_math::BlockPos;
use hearth_world::{BlockRegistry, CubeMap};

use crate::scene::LocalWorld;

/// The file the populations are saved in.
const FILE: &str = "fauna.json.zst";

/// The loaded terrain as animals walk on it.
pub struct MapGround<'a> {
    pub map: &'a CubeMap,
    pub reg: &'a BlockRegistry,
    pub lw: &'a LocalWorld,
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
            return Some(Footing {
                y: y as f64 + shape.top(),
                water: reg.fluid_amount(above) > 0,
            });
        }
        // Standing water: the ground under it is wet footing.
        above = s;
    }
    None
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
        Footing {
            y: s.height_i().max(s.water_i()) as f64,
            water: s.water_i() > s.height_i(),
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
        self.surface(x, z, y.floor() as i32 + 2, 6)
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
    yields: TreeYields,
    /// Years since the world began the populations are simulated to.
    pub years: f64,
}

/// What is saved of the populations.
#[derive(serde::Serialize, serde::Deserialize)]
struct Saved {
    years: f64,
    next_id: u64,
    regions: Vec<Region>,
}

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
        if let Some(saved) = dir.and_then(|d| load(&d.join(FILE))) {
            eco.next_id = saved.next_id;
            at = saved.years;
            for r in saved.regions {
                eco.restore(&land, r);
            }
        }
        Self {
            eco,
            live: Live::new(seed),
            yields,
            years: at,
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
            self.eco.ensure_region(&land, key, self.years);
        }
    }

    /// A tick of `dt` seconds at `years` (since the world began) and the local `hour` (0–1)
    /// with the player at `player`; `tick` counts ticks.
    pub fn tick(
        &mut self,
        lw: &LocalWorld,
        player: DVec3,
        years: f64,
        hour: f32,
        dt: f32,
        tick: u64,
    ) {
        if tick.is_multiple_of(40) {
            let land = GenLand {
                wg: &lw.generator,
                veg: &lw.vegetation,
                catalog: &self.eco.catalog.clone(),
                trees: &self.yields,
            };
            // The player's region first, then the rest, one a time (each takes a moment to
            // make).
            for key in self.regions_about(player) {
                if !self.eco.regions.contains_key(&key) {
                    self.eco.ensure_region(&land, key, self.years);
                    break;
                }
            }
            // The populations to the calendar every few days, in steps of at most 1/32 year.
            if years - self.years >= 1.0 / 128.0 {
                self.eco.advance(years, 1.0 / 32.0);
                self.years = years;
            }
            let ground = MapGround {
                map: &lw.map,
                reg: &lw.reg,
                lw,
            };
            self.live.fold(&mut self.eco, player);
            self.live.materialize(&mut self.eco, &ground, player);
        }
        let ground = MapGround {
            map: &lw.map,
            reg: &lw.reg,
            lw,
        };
        self.live.step(&self.eco, &ground, Some(player), hour, dt);
    }

    /// The animals near the player, for the client.
    pub fn views(&self) -> Vec<AnimalView> {
        self.live.views()
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
        let mut regions: Vec<Region> = eco.regions.into_values().collect();
        regions.sort_by_key(|r| r.key);
        let saved = Saved {
            years: self.years,
            next_id: eco.next_id,
            regions,
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
                }
            }
            Err(e) => log::error!("fauna not saved: {e}"),
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

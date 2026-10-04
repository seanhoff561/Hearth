//! The history grid (D191): square cells laid over the planet's map, each reading the planet at a
//! few points — its ground (so that the sea's level says how much of it is land), its climate,
//! what its biomes give foragers, its realm and landmass, its rivers and its sunlight.

use hearth_content::schema::history::HistorySettings;
use hearth_worldgen::region::Terrain;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

/// The points a cell is read at (4 × 4).
pub const SAMPLES: usize = 16;
/// No landmass.
pub const NO_LANDMASS: u32 = u32::MAX;

/// A cell of the history grid.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoCell {
    /// Its points' ground (real metres against today's sea), lowest first.
    pub ground: [f32; SAMPLES],
    /// Today's yearly mean and coldest month (°C) and rain (mm a year), over its land (or the
    /// whole cell where it has none).
    pub temp_c: f32,
    pub coldest_c: f32,
    pub rain_mm: f32,
    /// What its land gives foragers today (its biomes' factors), and what the land the sea
    /// leaves bare in a glacial would give (by its climate).
    pub land_yield: f32,
    pub shelf_yield: f32,
    /// Its realm (`Realm as u8`) and landmass (its index, or [`NO_LANDMASS`]).
    pub realm: u8,
    pub landmass: u32,
    /// A river runs through it.
    pub river: bool,
    /// Its sunlight, 0 at the poles to 1 at the equator (as the gene pools read it).
    pub sun: f32,
}

impl GeoCell {
    /// Its share that is land with the sea at `sea_m` against today's.
    pub fn land(&self, sea_m: f32) -> f32 {
        self.ground.iter().filter(|&&g| g > sea_m).count() as f32 / SAMPLES as f32
    }

    /// Its share that is land today.
    pub fn land_today(&self) -> f32 {
        self.land(0.0)
    }

    /// What its land gives foragers with the sea at `sea_m`: today's land at its biomes', the
    /// land the sea left bare at its climate's.
    pub fn yields(&self, sea_m: f32) -> f32 {
        let now = self.land(sea_m);
        if now <= 0.0 {
            return 0.0;
        }
        let today = self.land_today().min(now);
        (today * self.land_yield + (now - today) * self.shelf_yield) / now
    }
}

/// The history grid over a planet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Geography {
    /// Cells along the circumference (x wraps) and from pole to pole (z).
    pub n: usize,
    /// A cell's edge in blocks (metres).
    pub cell_m: f64,
    pub cells: Vec<GeoCell>,
}

/// The eight neighbours' offsets and weights (the diagonals half).
const AROUND: [(i64, i64, f32); 8] = [
    (1, 0, 1.0),
    (-1, 0, 1.0),
    (0, 1, 1.0),
    (0, -1, 1.0),
    (1, 1, 0.5),
    (1, -1, 0.5),
    (-1, 1, 0.5),
    (-1, -1, 0.5),
];

impl Geography {
    /// How many cells along a planet of this circumference (m).
    pub fn cells_for(circumference_m: f64, s: &HistorySettings) -> usize {
        let km = circumference_m / 1000.0;
        ((km / s.cell_km.max(0.1) as f64).round() as usize)
            .clamp(s.cells.0 as usize, s.cells.1.max(s.cells.0) as usize)
    }

    /// A cell's edge (km).
    pub fn cell_km(&self) -> f64 {
        self.cell_m / 1000.0
    }

    /// A cell's area (km²).
    pub fn cell_km2(&self) -> f64 {
        self.cell_km() * self.cell_km()
    }

    /// The planet's circumference (m).
    pub fn circumference_m(&self) -> f64 {
        self.cell_m * self.n as f64
    }

    /// The cell holding a world position.
    pub fn cell_at(&self, x: f64, z: f64) -> usize {
        let c = self.circumference_m();
        let i = ((x.rem_euclid(c)) / self.cell_m).floor() as i64;
        let j = ((z + c * 0.5) / self.cell_m).floor() as i64;
        let n = self.n as i64;
        (j.clamp(0, n - 1) * n + i.clamp(0, n - 1)) as usize
    }

    /// A cell's middle (x, z in blocks).
    pub fn centre(&self, cell: usize) -> (f64, f64) {
        let (i, j) = (cell % self.n, cell / self.n);
        (
            (i as f64 + 0.5) * self.cell_m,
            -self.circumference_m() * 0.5 + (j as f64 + 0.5) * self.cell_m,
        )
    }

    /// The cells about one (wrapping east–west, not over the poles), with their weights.
    pub fn around(&self, cell: usize) -> impl Iterator<Item = (usize, f32)> + '_ {
        let n = self.n as i64;
        let (i, j) = ((cell % self.n) as i64, (cell / self.n) as i64);
        AROUND.iter().filter_map(move |&(di, dj, w)| {
            let jj = j + dj;
            (0..n)
                .contains(&jj)
                .then(|| ((jj * n + (i + di).rem_euclid(n)) as usize, w))
        })
    }

    /// The distance between two cells' middles (km), the short way round.
    pub fn km(&self, a: usize, b: usize) -> f64 {
        let n = self.n as i64;
        let (ai, aj) = ((a % self.n) as i64, (a / self.n) as i64);
        let (bi, bj) = ((b % self.n) as i64, (b / self.n) as i64);
        let di = (ai - bi).rem_euclid(n).min((bi - ai).rem_euclid(n));
        let dj = aj - bj;
        ((di * di + dj * dj) as f64).sqrt() * self.cell_km()
    }

    /// Reads the planet: each cell at 4 × 4 points of the terrain (its biome, climate, realm and
    /// rivers) and of the planet model (its ground in real metres); `sun` gives the sunlight of
    /// a latitude (degrees).
    pub fn of_terrain(
        terrain: &Terrain,
        settings: &HistorySettings,
        sun: &(dyn Fn(f64) -> f32 + Sync),
    ) -> Self {
        let planet = *terrain.planet();
        let circ = planet.circumference() as f64;
        let n = Self::cells_for(circ, settings);
        let cell_m = circ / n as f64;
        let grid = &terrain.grid;
        let yield_of = |biome: &str| {
            settings
                .biomes
                .iter()
                .find(|(b, _)| b == biome)
                .map_or(0.3, |(_, f)| *f)
        };
        let cells: Vec<GeoCell> = (0..n * n)
            .into_par_iter()
            .map(|cell| {
                let (i, j) = (cell % n, cell / n);
                let mut ground = [0.0f32; SAMPLES];
                let (mut t, mut cold, mut rain, mut y, mut lands) = (0.0, 0.0, 0.0, 0.0, 0.0f32);
                let (mut t_all, mut cold_all, mut rain_all) = (0.0, 0.0, 0.0);
                let mut river = false;
                let mut realms = [0u16; 8];
                let mut masses: Vec<(u32, u16)> = Vec::new();
                for (k, ground_k) in ground.iter_mut().enumerate() {
                    let (a, b) = ((k % 4) as f64, (k / 4) as f64);
                    let x = (i as f64 + (a + 0.5) / 4.0) * cell_m;
                    let z = -circ * 0.5 + (j as f64 + (b + 0.5) / 4.0) * cell_m;
                    let s = terrain.sample(x as i32, z as i32);
                    let g = grid.field_at(&grid.elevation, grid.cell_at(x, z));
                    *ground_k = g;
                    let coldest = 2.0 * s.temperature - s.t_warm;
                    t_all += s.temperature;
                    cold_all += coldest;
                    rain_all += s.precipitation;
                    if g > 0.0 {
                        lands += 1.0;
                        t += s.temperature;
                        cold += coldest;
                        rain += s.precipitation;
                        y += yield_of(s.biome.name());
                        realms[s.realm as usize & 7] += 1;
                        if let Some(m) = terrain.realms.landmass_index_at(x, z) {
                            match masses.iter_mut().find(|(id, _)| *id == m) {
                                Some(e) => e.1 += 1,
                                None => masses.push((m, 1)),
                            }
                        }
                    }
                    river |= s.river.is_some_and(|r| r.distance < r.width * 0.5 + 32.0);
                }
                ground.sort_by(f32::total_cmp);
                let all = SAMPLES as f32;
                let (temp_c, coldest_c, rain_mm) = if lands > 0.0 {
                    (t / lands, cold / lands, rain / lands)
                } else {
                    (t_all / all, cold_all / all, rain_all / all)
                };
                let (cx, cz) = (
                    (i as f64 + 0.5) * cell_m,
                    -circ * 0.5 + (j as f64 + 0.5) * cell_m,
                );
                // The sea's land, bare in a glacial: what its climate grows, a little less
                // than old land of the same climate.
                let shelf_yield = (hearth_fauna::habitat::miami_npp(temp_c, rain_mm) / 1200.0)
                    .clamp(0.05, 1.0)
                    * 0.8;
                let realm = if lands > 0.0 {
                    realms
                        .iter()
                        .enumerate()
                        .max_by_key(|(r, k)| (**k, std::cmp::Reverse(*r)))
                        .map_or(0, |(r, _)| r as u8)
                } else {
                    terrain.realms.realm_at(cx, cz) as u8
                };
                let landmass = masses
                    .iter()
                    .max_by_key(|(id, k)| (*k, std::cmp::Reverse(*id)))
                    .map(|(id, _)| *id)
                    .or_else(|| terrain.realms.landmass_index_at(cx, cz))
                    .unwrap_or(NO_LANDMASS);
                GeoCell {
                    ground,
                    temp_c,
                    coldest_c,
                    rain_mm,
                    land_yield: if lands > 0.0 { y / lands } else { shelf_yield },
                    shelf_yield,
                    realm,
                    landmass,
                    river,
                    sun: sun(planet.latitude_deg(cz)),
                }
            })
            .collect();
        Self { n, cell_m, cells }
    }
}

//! Groundwater, springs and water quality (v2 §5.4).
//!
//! **Water table.** A subdued copy of the land. From the drainage base (the lowest ground or
//! water surface within a quarter kilometre) it rises under the land, smoothed over a couple
//! of hundred metres, by a share of the relief that depends on the rock (tight crystalline rock
//! carries it up under the hills, karst drains it to the valley floors) and on the climate
//! (wet climates hold it high, dry ones sink it below the valley floors). Because it follows
//! the smoothed land, it meets the ground in valley bottoms and at the foot of slopes, where
//! springs and seeps rise. It meets rivers, lakes and the sea at their surfaces and stays below
//! the ground. Cave voids below it are flooded.
//!
//! **Springs** issue where the table meets sloping ground (spring lines at the foot of hills),
//! as rare oases in dry country, and over salt beds, hot rock and travertine. Each feeds a pool
//! and a brook that runs downhill until it reaches other water, finds a hollow or soaks away.
//!
//! **Water quality** of natural water: salinity, how likely it is to make a drinker ill,
//! temperature and taste, per water body (sea, salt or fresh lake, stream, spring, groundwater).

use std::sync::Arc;

use hearth_content::Content;
use hearth_content::schema::geology::Permeability;
use hearth_math::ColumnPos;
use hearth_math::hash::{derive_seed, hash_2d, hash2, unit_f32};
use hearth_world::{BlockRegistry, BlockStateId};
use rustc_hash::FxHashMap;

use crate::cubegen::cache::Cache;
use crate::cubegen::{CubeBuf, WorldGenerator};
use crate::planet::flags;
use crate::region::ColumnSample;

/// Side of a drainage-base tile (blocks).
const TILE: i32 = 128;
/// The drainage base is the lowest ground or water surface within this reach (blocks); the
/// smoothed land the mean of the dry ground within the smaller one.
const BASE_REACH: i32 = 256;
const SMOOTH_REACH: i32 = 96;
/// Side of a spring placement cell (blocks), and candidate points tried per cell.
pub const CELL: i32 = 256;
const CANDIDATES: u64 = 32;
/// Springs per km² of spring line (ground where the water table meets a slope): one every
/// hundred-odd metres along it.
const SPRINGS_PER_KM2: f32 = 120.0;
/// Oasis springs per km² of hollows in dry country.
const OASES_PER_KM2: f32 = 0.6;
/// Longest brook (blocks).
const MAX_BROOK: usize = 96;

/// What a spring's water carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpringKind {
    Fresh,
    /// Brine from salt beds.
    Salt,
    /// Warm, sulfurous water at fumaroles.
    Sulfur,
    /// Hot water near volcanoes.
    Hot,
    /// Hard, mineral water (travertine springs).
    Mineral,
}

/// A spring: its pool and the brook it feeds.
#[derive(Debug, Clone)]
pub struct Spring {
    pub x: i32,
    pub z: i32,
    /// Water surface of the pool (the first block above the water).
    pub level: i32,
    pub kind: SpringKind,
    /// The brook from the pool downhill: columns and their water surface.
    pub brook: Vec<(i32, i32, i32)>,
    min: [i32; 2],
    max: [i32; 2],
}

impl Spring {
    /// Whether the pool or the brook covers a column.
    fn contains(&self, planet: &hearth_math::Planet, x: i32, z: i32) -> bool {
        (planet.delta_block_x(self.x, x).abs() <= 1 && (z - self.z).abs() <= 1)
            || self
                .brook
                .iter()
                .any(|&(bx, bz, _)| bz == z && planet.delta_block_x(bx, x) == 0)
    }
}

/// How natural water tastes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taste {
    Clean,
    Salty,
    Sulfurous,
    Mineral,
    Muddy,
}

/// The quality of a body of water (v2 §9.3 drinks it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterQuality {
    /// Dissolved salts, g/L: fresh below 0.5, brackish to 30, the sea about 35, brine above.
    pub salinity_g_l: f32,
    /// 0–1: how likely an untreated drink is to make one ill (a cold spring ≈ 0, a warm
    /// stagnant pond ≈ 0.7).
    pub pathogen_risk: f32,
    pub temperature_c: f32,
    pub taste: Taste,
}

/// Groundwater and springs of one world.
pub struct Hydrology {
    /// Share of the relief above the drainage base the water table follows, and how readily
    /// groundwater flows, by rock.
    follow: FxHashMap<BlockStateId, f32>,
    permeability: FxHashMap<BlockStateId, Permeability>,
    /// Drainage base and smoothed land per tile.
    bases: Cache<(i32, i32), (f32, f32)>,
    springs: Cache<(i32, i32), Vec<Spring>>,
    cells_around: i32,
    seed: u64,
}

/// Share of the relief the water table follows in a rock of this permeability.
fn follow_of(p: Permeability) -> f32 {
    match p {
        Permeability::Tight => 0.9,
        Permeability::Poor => 0.8,
        Permeability::Fair => 0.7,
        Permeability::Good => 0.5,
        Permeability::Karst => 0.12,
    }
}

/// 0 (desert) … 1 (wet): the water left for the ground after evaporation.
fn wetness(s: &ColumnSample) -> f32 {
    wetness_of(s.precipitation, s.temperature)
}

/// Wetness from annual precipitation (mm) and mean temperature (°C): 0 (desert) … 1 (wet).
pub fn wetness_of(precipitation: f32, temperature: f32) -> f32 {
    let t = ((precipitation - 20.0 * temperature.max(0.0) - 150.0) / 850.0).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Hydrology {
    pub fn new(content: &Content, reg: &BlockRegistry, seed: u64, circumference: i32) -> Self {
        use hearth_content::schema::Entry;
        let mut follow = FxHashMap::default();
        let mut permeability = FxHashMap::default();
        for r in content.rocks.iter() {
            let id = r.id();
            let path = id.split_once(':').map_or(id, |(_, p)| p);
            if let Ok(state) = reg.parse_state(path) {
                follow.insert(state, follow_of(r.permeability));
                permeability.insert(state, r.permeability);
            }
        }
        Self {
            follow,
            permeability,
            bases: Cache::new(4096),
            springs: Cache::new(512),
            cells_around: (circumference / CELL).max(1),
            seed: derive_seed(seed, "springs"),
        }
    }

    /// The water table under a column (block Y of the water surface in the ground; the water
    /// surface itself under water).
    pub fn water_table(&self, wg: &WorldGenerator, x: i32, z: i32) -> f32 {
        // One sample: callers ask about scattered places (cave systems, wells).
        let s = wg.terrain.sample(x, z);
        self.table_at(wg, &s, x, z)
    }

    fn table_at(&self, wg: &WorldGenerator, s: &ColumnSample, x: i32, z: i32) -> f32 {
        self.table_parts(wg, s, x, z).1
    }

    /// The water table before and after keeping it below the ground (the first rises above
    /// the ground where groundwater comes out: springs and seeps).
    fn table_parts(&self, wg: &WorldGenerator, s: &ColumnSample, x: i32, z: i32) -> (f32, f32) {
        if s.is_underwater() {
            return (s.water, s.water);
        }
        let (base, smooth) = self.drainage_base(wg, x, z);
        let base = base.min(s.height);
        let rock = wg
            .geology
            .column(x, z)
            .rock_at(s.height_i() - 1 - s.soil_depth as i32);
        let follow = self.follow.get(&rock).copied().unwrap_or(0.7);
        let w = wetness(s);
        let f = follow * (0.3 + 0.7 * w);
        let drop = (1.0 - w).powi(2) * 24.0 + if follow < 0.2 { 3.0 } else { 0.0 };
        // Under hills the table follows the smoothed land; in hollows the land itself.
        let land = s.height.min(smooth.max(base));
        let mut t = base - drop + f * (land - base);
        // Near the sea the fresh water stands on the salt water, above sea level.
        if base <= 0.5 {
            t = t.max(0.0);
        }
        (t, t.min(s.height - (0.6 + 2.5 * (1.0 - w))))
    }

    /// The drainage base and the smoothed land, interpolated between tile centres.
    fn drainage_base(&self, wg: &WorldGenerator, x: i32, z: i32) -> (f32, f32) {
        // In f64 (exact at any x), the tiles keyed by their place around the planet (E4.1 §4.5).
        let fx = (x as f64 - TILE as f64 * 0.5) / TILE as f64;
        let fz = (z as f64 - TILE as f64 * 0.5) / TILE as f64;
        let (tx, tz) = (fx.floor() as i32, fz.floor() as i32);
        let (ux, uz) = ((fx - tx as f64) as f32, (fz - tz as f64) as f32);
        let around = (wg.planet().circumference() / TILE).max(1);
        let b = |dx: i32, dz: i32| {
            let tx = (tx + dx).rem_euclid(around);
            *self
                .bases
                .get_or_insert_with((tx, tz + dz), || Self::tile_base(wg, tx, tz + dz))
        };
        let (b00, b10, b01, b11) = (b(0, 0), b(1, 0), b(0, 1), b(1, 1));
        let lerp2 = |a: f32, b: f32, c: f32, d: f32| {
            let top = a + (b - a) * ux;
            let bottom = c + (d - c) * ux;
            top + (bottom - top) * uz
        };
        (
            lerp2(b00.0, b10.0, b01.0, b11.0),
            lerp2(b00.1, b10.1, b01.1, b11.1),
        )
    }

    /// Lowest ground or water surface around a tile centre, and the mean level of the dry land
    /// near it, from the planet grid (whose cells are tens of blocks across).
    fn tile_base(wg: &WorldGenerator, tx: i32, tz: i32) -> (f32, f32) {
        let g = &wg.terrain.grid;
        let v = wg.terrain.vertical_scale();
        let (cx, cz) = ((tx * TILE + TILE / 2) as f64, (tz * TILE + TILE / 2) as f64);
        let (gx, gz) = g.geom.grid_coords(wg.planet().wrap_xf(cx), cz);
        let n = g.geom.n as isize;
        let reach = (BASE_REACH as f64 / g.geom.cell).ceil() as isize;
        let near = (SMOOTH_REACH as f64 / g.geom.cell).ceil() as isize;
        let (ci, cj) = (gx.round() as isize, gz.round() as isize);
        let mut low = f32::INFINITY;
        let (mut sum, mut count) = (0.0, 0.0);
        for dj in -reach..=reach {
            let j = cj + dj;
            if !(0..n).contains(&j) {
                continue;
            }
            for di in -reach..=reach {
                let i = (ci + di).rem_euclid(n);
                let ground = g.elevation.cell(i as usize, j as usize) * v;
                let water = g.water.cell(i as usize, j as usize) * v;
                let wet = water > ground;
                low = low.min(if wet { water } else { ground });
                // The smoothed land: the dry ground only, so the sea does not pull it down.
                if !wet && di.abs() <= near && dj.abs() <= near {
                    sum += ground;
                    count += 1.0;
                }
            }
        }
        (low, if count > 0.0 { sum / count } else { low })
    }

    /// Springs of a placement cell.
    pub fn cell(&self, wg: &WorldGenerator, cx: i32, cz: i32) -> Arc<Vec<Spring>> {
        let cx = cx.rem_euclid(self.cells_around);
        self.springs
            .get_or_insert_with((cx, cz), || self.compute_cell(wg, cx, cz))
    }

    fn compute_cell(&self, wg: &WorldGenerator, cx: i32, cz: i32) -> Vec<Spring> {
        let mut out = Vec::new();
        let area = (CELL as f32 / 1000.0).powi(2) / CANDIDATES as f32;
        for k in 0..CANDIDATES {
            let h = hash2(hash_2d(self.seed, cx, cz), k);
            let x = cx * CELL + (unit_f32(hash2(h, 1)) * CELL as f32) as i32;
            let z = cz * CELL + (unit_f32(hash2(h, 2)) * CELL as f32) as i32;
            let s = wg.terrain.sample(x, z);
            if s.is_underwater()
                || s.river.is_some_and(|r| r.distance < r.width + 4.0)
                || s.temperature < -2.0
            {
                continue;
            }
            let w = wetness(&s);
            let roll = unit_f32(hash2(h, 3));
            // Groundwater comes out where the table would rise to the surface.
            let (raw, _) = self.table_parts(wg, &s, x, z);
            let meets = raw >= s.height - 0.3;
            let spring_line = meets && s.slope > 0.02 && roll < SPRINGS_PER_KM2 * area * w.sqrt();
            let oasis = w < 0.25
                && s.height - self.drainage_base(wg, x, z).0 < 2.0
                && roll < OASES_PER_KM2 * area;
            if spring_line || oasis {
                out.push(self.spring(wg, x, z, &s, SpringKind::Fresh, w));
            }
        }
        // Mineral springs over the deposits that show as springs.
        for b in wg.deposits.cell(wg, cx, cz).iter() {
            let m = &wg.deposits.models[b.model as usize];
            if let Some(kind) = m.spring {
                let (x, z) = (b.center[0], b.center[2]);
                let s = wg.terrain.sample(x, z);
                if !s.is_underwater() {
                    out.push(self.spring(wg, x, z, &s, kind, wetness(&s)));
                }
            }
        }
        out
    }

    /// A spring at a place: a pool and a brook down the steepest way.
    fn spring(
        &self,
        wg: &WorldGenerator,
        x: i32,
        z: i32,
        s: &ColumnSample,
        kind: SpringKind,
        w: f32,
    ) -> Spring {
        let level = s.height_i();
        let mut brook = Vec::new();
        let max_len = (12.0 + (MAX_BROOK as f32 - 12.0) * w) as usize;
        let (mut bx, mut bz, mut bh) = (x, z, s.height);
        let planet = wg.planet();
        'walk: while brook.len() < max_len {
            let mut best: Option<(i32, i32, ColumnSample)> = None;
            for (dx, dz) in [
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
                (1, 1),
                (1, -1),
                (-1, 1),
                (-1, -1),
            ] {
                let (nx, nz) = (planet.wrap_x(bx + dx), bz + dz);
                let n = wg.terrain.sample(nx, nz);
                if best.as_ref().is_none_or(|(_, _, b)| n.height < b.height) {
                    best = Some((nx, nz, n));
                }
            }
            let Some((nx, nz, n)) = best else {
                break;
            };
            if n.height >= bh {
                break 'walk; // a hollow: the brook ends in the ground
            }
            if n.is_underwater() {
                break 'walk; // it reaches a stream, lake or the sea
            }
            brook.push((nx, nz, n.height_i()));
            (bx, bz, bh) = (nx, nz, n.height);
        }
        let mut min = [x - 2, z - 2];
        let mut max = [x + 2, z + 2];
        for &(px, pz, _) in &brook {
            let dx = planet.delta_block_x(x, px);
            min[0] = min[0].min(x + dx);
            max[0] = max[0].max(x + dx);
            min[1] = min[1].min(pz);
            max[1] = max[1].max(pz);
        }
        Spring {
            x,
            z,
            level,
            kind,
            brook,
            min,
            max,
        }
    }

    /// Draws the springs whose pools or brooks reach a cube: water in a hollow of the ground at
    /// the spring, and a channel a block deep along the brook.
    pub fn apply(&self, buf: &mut CubeBuf, wg: &WorldGenerator) {
        let o = buf.origin;
        let (x0, z0) = (o.x, o.z);
        let (x1, z1) = (x0 + 15, z0 + 15);
        let margin = MAX_BROOK as i32 + 4;
        let (ca, cb) = (
            (x0 - margin).div_euclid(CELL),
            (x1 + margin).div_euclid(CELL),
        );
        let (za, zb) = (
            (z0 - margin).div_euclid(CELL),
            (z1 + margin).div_euclid(CELL),
        );
        let water = wg.blocks.water;
        let wet = |buf: &mut CubeBuf, x: i32, y: i32, z: i32| {
            if let Some(g) = buf.get(x, y, z)
                && (wg.blocks.is_base_rock(g) || wg.blocks.is_carvable(g))
                && buf
                    .get(x, y + 1, z)
                    .is_none_or(|a| a.is_air() || a == water)
            {
                buf.set(x, y, z, water);
            }
        };
        let planet = wg.planet();
        for cz in za..=zb {
            for cx in ca..=cb {
                for sp in self.cell(wg, cx, cz).iter() {
                    // Shift the spring to this cube's side of the seam.
                    let shift = planet.delta_block_x(sp.x, x0) - (x0 - sp.x);
                    if sp.max[0] - shift < x0
                        || sp.min[0] - shift > x1
                        || sp.max[1] < z0
                        || sp.min[1] > z1
                    {
                        continue;
                    }
                    // The pool: a hollow of 3×3 around the eye of the spring, deeper in the middle.
                    for dz in -1..=1 {
                        for dx in -1..=1 {
                            let (x, z) = (sp.x - shift + dx, sp.z + dz);
                            if x < x0 || x > x1 || z < z0 || z > z1 {
                                continue;
                            }
                            let top = wg.terrain.sample(x, z).height_i();
                            if top > sp.level + 1 {
                                continue;
                            }
                            wet(buf, x, top - 1, z);
                            if dx == 0 && dz == 0 {
                                wet(buf, x, top - 2, z);
                            }
                        }
                    }
                    for &(bx, bz, top) in &sp.brook {
                        let x = sp.x - shift + planet.delta_block_x(sp.x, bx);
                        if x < x0 || x > x1 || bz < z0 || bz > z1 {
                            continue;
                        }
                        wet(buf, x, top - 1, bz);
                    }
                }
            }
        }
    }

    /// The spring whose pool or brook covers a column, if any.
    pub fn spring_at(&self, wg: &WorldGenerator, x: i32, z: i32) -> Option<SpringKind> {
        let margin = MAX_BROOK as i32 + 4;
        for cz in (z - margin).div_euclid(CELL)..=(z + margin).div_euclid(CELL) {
            for cx in (x - margin).div_euclid(CELL)..=(x + margin).div_euclid(CELL) {
                for sp in self.cell(wg, cx, cz).iter() {
                    if sp.contains(wg.planet(), x, z) {
                        return Some(sp.kind);
                    }
                }
            }
        }
        None
    }

    /// How fast groundwater seeps into a hole dug below the water table (litres a day for a
    /// block of hole), by the permeability of the rock around it: a few litres in tight
    /// crystalline rock or clay, a cubic metre or more in sandstone and gravel, several in
    /// karst — what hand-dug wells yield.
    pub fn seepage(&self, wg: &WorldGenerator, x: i32, y: i32, z: i32) -> f32 {
        let rock = wg.geology.column(x, z).rock_at(y);
        match self.permeability.get(&rock).copied().unwrap_or_default() {
            Permeability::Tight => 10.0,
            Permeability::Poor => 60.0,
            Permeability::Fair => 300.0,
            Permeability::Good => 1500.0,
            Permeability::Karst => 4000.0,
        }
    }

    /// The quality of the natural water at a block: the sea, a lake, a stream, a spring, or the
    /// groundwater below the water table. `None` where there is no water.
    pub fn quality(&self, wg: &WorldGenerator, x: i32, y: i32, z: i32) -> Option<WaterQuality> {
        let col = wg.column(ColumnPos::new(x >> 4, z >> 4));
        let s = *col.at((x & 15) as usize, (z & 15) as usize);
        let grid = &wg.terrain.grid;
        let idx = grid.cell_at(x as f64, z as f64);
        let cell_flags = grid.flags[idx];
        let fresh = |risk: f32, t: f32, taste: Taste| WaterQuality {
            salinity_g_l: 0.2,
            pathogen_risk: risk.clamp(0.0, 1.0),
            temperature_c: t,
            taste,
        };
        let warm = |t: f32| {
            let u = ((t - 8.0) / 17.0).clamp(0.0, 1.0);
            u * u * (3.0 - 2.0 * u)
        };
        // Surface water.
        if s.is_underwater() && y < s.water_i() && y >= s.height_i() - 1 {
            if s.ocean {
                let delta = cell_flags & flags::DELTA != 0;
                return Some(WaterQuality {
                    salinity_g_l: if delta { 12.0 } else { 35.0 },
                    pathogen_risk: 0.1,
                    temperature_c: s.sea_temperature,
                    taste: Taste::Salty,
                });
            }
            if s.lake {
                if cell_flags & flags::ENDORHEIC != 0 {
                    // Closed basins keep what evaporation leaves: brackish to brine.
                    let arid = 1.0 - wetness(&s);
                    return Some(WaterQuality {
                        salinity_g_l: 5.0 + 150.0 * arid * arid,
                        pathogen_risk: 0.15,
                        temperature_c: s.temperature,
                        taste: Taste::Salty,
                    });
                }
                return Some(fresh(
                    0.15 + 0.5 * warm(s.t_warm),
                    s.temperature,
                    Taste::Clean,
                ));
            }
            if let Some(r) = s.river {
                // Small cold streams are safe; broad, warm lowland rivers carry disease and silt.
                let size = (r.width / 56.0).clamp(0.0, 1.0);
                let taste = if size > 0.5 {
                    Taste::Muddy
                } else {
                    Taste::Clean
                };
                return Some(fresh(
                    0.05 + 0.45 * size * (0.3 + 0.7 * warm(s.t_warm)),
                    s.temperature,
                    taste,
                ));
            }
            return Some(fresh(
                0.3 + 0.4 * warm(s.t_warm),
                s.temperature,
                Taste::Clean,
            ));
        }
        // Springs and their brooks.
        if y >= s.height_i() - 2
            && y < s.height_i()
            && let Some(kind) = self.spring_at(wg, x, z)
        {
            let t = s.temperature.max(4.0);
            return Some(match kind {
                SpringKind::Fresh => fresh(0.02, t, Taste::Clean),
                SpringKind::Salt => WaterQuality {
                    salinity_g_l: 60.0,
                    pathogen_risk: 0.02,
                    temperature_c: t,
                    taste: Taste::Salty,
                },
                SpringKind::Sulfur => WaterQuality {
                    salinity_g_l: 2.0,
                    pathogen_risk: 0.0,
                    temperature_c: 45.0,
                    taste: Taste::Sulfurous,
                },
                SpringKind::Hot => WaterQuality {
                    salinity_g_l: 1.5,
                    pathogen_risk: 0.0,
                    temperature_c: 55.0,
                    taste: Taste::Mineral,
                },
                SpringKind::Mineral => WaterQuality {
                    salinity_g_l: 1.0,
                    pathogen_risk: 0.02,
                    temperature_c: t + 6.0,
                    taste: Taste::Mineral,
                },
            });
        }
        // Groundwater: what a well reaches.
        if (y as f32) < self.table_at(wg, &s, x, z) {
            let arid = 1.0 - wetness(&s);
            return Some(WaterQuality {
                salinity_g_l: 0.3 + 2.5 * arid * arid,
                pathogen_risk: 0.03,
                temperature_c: s.temperature.max(1.0),
                taste: if arid > 0.7 {
                    Taste::Mineral
                } else {
                    Taste::Clean
                },
            });
        }
        None
    }
}

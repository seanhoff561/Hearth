//! Terrain refinement (E §5.2): nested levels between the planet grid and the blocks.
//!
//! The planet grid's cells are some 20 km across on Earth and a block is a metre. Between them
//! lie refinement levels, each [`RATIO`] times finer than the one above (2.4 km, 306 m and 38 m
//! on Earth), as many as stay coarser than [`MIN_CELL`]; below the last, the blocks take their
//! shape from its surface and a little noise. A small test planet, whose grid is already fine,
//! has none.
//!
//! Each level is made a tile at a time ([`TILE`] × [`TILE`] cells and a margin of [`MARGIN`]
//! all round), conditioned on the level above:
//! - its surface, interpolated, with relief added at the level's own wavelengths, as much as
//!   the place allows: mountains where the rock is uplifted, hills on high land, plains low and
//!   flat, abyssal hills on the deep sea floor, the shelf smooth;
//! - its rivers, kept as channels at their own levels, falling downstream, meandering below the
//!   parent's cell size by a fractal of each reach that the tiles on either side share;
//! - then its own drainage, found by Priority-Flood+ε (Barnes et al. 2014) and cut by implicit
//!   stream-power erosion (Braun and Willett 2013), so valleys, ridges and tributaries form at
//!   the level's scale and run into the parent's rivers; slopes past their angle of repose slump;
//!   hollows that do not drain hold lakes.
//!
//! A cell's value is its tiles' blend: the tile whose core holds it, faded into those whose
//! margins reach it. A value so never depends on which tile was asked for first, and tiles meet
//! without a seam. Tiles are kept in an LRU cache per level.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::sync::Arc;

use hearth_math::hash::{derive_seed, hash_2d, hash2, unit_f64};

use crate::cubegen::cache::Cache;
use crate::noise::BlockFbm;
use crate::planet::{PlanetGrid, flags};

/// Each level's cells are this many times finer than its parent's.
pub const RATIO: i64 = 8;
/// Core cells along a tile's side.
pub const TILE: i64 = 64;
/// Cells of margin on every side of a tile's core, computed and blended into its neighbours.
pub const MARGIN: i64 = 16;
/// Cells along the side of a tile's computed span.
const SPAN: i64 = TILE + 2 * MARGIN;
/// The finest level's cells are no smaller than this (blocks).
pub const MIN_CELL: f64 = 24.0;
/// Tiles kept per level.
const TILES_KEPT: usize = 512;
/// Seconds in a year.
const YEAR_S: f64 = 31_557_600.0;
/// Discharge of the grid's drainage per unit of its measure (a square degree at the equator of
/// an Earth-sized planet, a metre of rain a year), as runoff: m³/s. The grid counts the rain;
/// some 36 % of the rain on land runs off (40,000 of 111,000 km³ a year).
const GRID_Q_UNIT: f64 = 392.7 * 0.36;
/// D8 offsets, the receiver codes 0..8; 8 is no receiver (an outlet).
const D8: [(i64, i64); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];
const OUTLET: u8 = 8;

/// One refinement level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Level {
    /// Cell size (blocks).
    pub cell: f64,
    /// Cells around the planet, and from pole edge to pole edge.
    pub count: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TileKey {
    level: u8,
    tx: i64,
    tz: i64,
}

/// A tile's results over its span (core and margin), row-major from its span's corner.
pub struct Tile {
    /// Surface heights (blocks).
    pub h: Vec<f32>,
    /// Lake surfaces (blocks), NaN where none.
    pub lake: Vec<f32>,
    /// Discharge through each cell (m³/s).
    pub q: Vec<f32>,
    /// Receiver codes (into [`D8`]; [`OUTLET`] for none).
    pub recv: Vec<u8>,
    /// Whether a cell is a channel kept from the parent.
    pub channel: Vec<bool>,
    /// Whether a cell is the sea's.
    pub sea: Vec<bool>,
}

/// What a level knows of one cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    /// The blended surface (blocks).
    pub h: f32,
    /// The lake surface (blocks) in the cell, or NaN.
    pub lake: f32,
    /// Discharge (m³/s).
    pub q: f32,
    /// The cell it drains to (its global indices), if any.
    pub receiver: Option<(i64, i64)>,
    /// Whether the cell is the sea's.
    pub sea: bool,
    /// Whether the cell carries a river kept from the level above (always so at the grid's).
    pub channel: bool,
}

/// Noise for one level: an octave per wavelength, with its amplitude in metres at full relief.
#[derive(Debug, Clone)]
struct LevelNoise {
    octaves: Vec<(BlockFbm, f64)>,
    warp: BlockFbm,
    /// The octaves' amplitudes summed (m at full relief): how far the level's relief can reach.
    reach: f64,
}

/// How far below the parent's surface a river's bed lies at a level, as a share of the level's
/// reach: rivers are the low ground of the relief the level adds, so it rises from them.
const VALLEY: f64 = 0.35;
/// How far apart (blocks, along z) the relief's warp takes its two parts.
const WARP_APART: f64 = 3.7e7;

/// The planet's refinement levels.
pub struct Relief {
    grid: Arc<PlanetGrid>,
    /// Blocks per metre.
    v: f64,
    c: f64,
    seed: u64,
    levels: Vec<Level>,
    noise: Vec<LevelNoise>,
    tiles: Vec<Cache<TileKey, Tile>>,
}

impl std::fmt::Debug for Relief {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Relief")
            .field("levels", &self.levels)
            .finish()
    }
}

/// Relief amplitude (metres, ±) at full relief for a wavelength: rough as mountain ranges are,
/// some 300 m at 2.5 km and 70 m at 300 m (amplitude ∝ λ^0.7).
fn amplitude(wavelength_m: f64) -> f64 {
    160.0 * (wavelength_m / 1000.0).powf(0.7)
}

#[inline]
fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Relief {
    pub fn new(grid: Arc<PlanetGrid>) -> Self {
        let c = grid.geom.c;
        let n = grid.n() as i64;
        let mut levels = Vec::new();
        let mut cell = grid.geom.cell;
        let mut count = n;
        while cell / RATIO as f64 >= MIN_CELL {
            cell /= RATIO as f64;
            count *= RATIO;
            levels.push(Level { cell, count });
        }
        let seed = derive_seed(grid.seed, "relief");
        let noise = levels
            .iter()
            .enumerate()
            .map(|(l, lv)| {
                // Wavelengths from the parent's cell (twice the grid's, below the first level,
                // as finer than that the grid's surface holds nothing) down to twice the
                // level's own.
                let mut octaves = Vec::new();
                let mut wl = lv.cell * RATIO as f64 * if l == 0 { 2.0 } else { 1.0 };
                let mut k = 0u64;
                while wl >= 2.0 * lv.cell - 1e-6 {
                    let s = hash2(seed, (l as u64) << 8 | k);
                    octaves.push((BlockFbm::new(s, c as i64, wl, 1, 0.5), amplitude(wl)));
                    wl *= 0.5;
                    k += 1;
                }
                LevelNoise {
                    reach: octaves.iter().map(|o| o.1).sum(),
                    octaves,
                    warp: BlockFbm::new(
                        hash2(seed, 0xa11 + l as u64),
                        c as i64,
                        4.0 * lv.cell * RATIO as f64,
                        2,
                        0.5,
                    ),
                }
            })
            .collect();
        let tiles = levels.iter().map(|_| Cache::new(TILES_KEPT)).collect();
        Self {
            v: grid.vertical_scale,
            c,
            seed,
            levels,
            noise,
            tiles,
            grid,
        }
    }

    /// The refinement levels, coarsest first (none on a small test planet).
    pub fn levels(&self) -> &[Level] {
        &self.levels
    }

    /// Cell size of a level (0 is the grid's).
    fn cell_size(&self, level: usize) -> f64 {
        if level == 0 {
            self.grid.geom.cell
        } else {
            self.levels[level - 1].cell
        }
    }

    fn count(&self, level: usize) -> i64 {
        if level == 0 {
            self.grid.n() as i64
        } else {
            self.levels[level - 1].count
        }
    }

    /// World (x, z) of a cell's centre at a level.
    fn centre(&self, level: usize, i: i64, j: i64) -> (f64, f64) {
        let s = self.cell_size(level);
        ((i as f64 + 0.5) * s, -self.c * 0.5 + (j as f64 + 0.5) * s)
    }

    /// A river's node in a cell: its centre, moved by up to 0.3 of a cell (the same however the
    /// cell's index is wrapped round the planet).
    fn node(&self, level: usize, i: i64, j: i64) -> (f64, f64) {
        let i = i.rem_euclid(self.count(level));
        let s = self.cell_size(level);
        let (x, z) = self.centre(level, i, j);
        let hsh = hash_2d(hash2(self.seed, 0x40de + level as u64), i as i32, j as i32);
        let jx = unit_f64(hsh) - 0.5;
        let jz = unit_f64(hash2(hsh, 1)) - 0.5;
        (x + jx * 0.6 * s, z + jz * 0.6 * s)
    }

    /// The grid cell holding a level-0 index (wrapping in x; `None` past the poles).
    fn grid_index(&self, i: i64, j: i64) -> Option<usize> {
        let n = self.grid.n() as i64;
        if j < 0 || j >= n {
            return None;
        }
        Some((j * n + i.rem_euclid(n)) as usize)
    }

    /// A cell's blended value at a level ≥ 1, or the grid's at level 0.
    pub fn cell(&self, level: usize, i: i64, j: i64) -> Cell {
        if level == 0 {
            return self.grid_cell(i, j);
        }
        let count = self.count(level);
        let i = i.rem_euclid(count);
        let j = j.clamp(0, count - 1);
        let tx = i.div_euclid(TILE);
        let tz = j.div_euclid(TILE);
        let own = self.tile(level, tx, tz);
        let (u, w) = (i - tx * TILE + MARGIN, j - tz * TILE + MARGIN);
        let k = (w * SPAN + u) as usize;
        // The tiles whose margins reach the cell, faded by how far it lies past their core: the
        // surface, and the depth of any lake over it (so a lake one tile holds and another does
        // not thins out across the margin rather than stopping at a line).
        let depth = |t: &Tile, k: usize| {
            if t.lake[k].is_finite() {
                (t.lake[k] - t.h[k]).max(0.0) as f64
            } else {
                0.0
            }
        };
        let mut sum = own.h[k] as f64;
        let mut water = depth(&own, k);
        let mut weight = 1.0;
        let (ci, cj) = (i - tx * TILE, j - tz * TILE);
        for dz in -1..=1i64 {
            for dx in -1..=1i64 {
                if dx == 0 && dz == 0 {
                    continue;
                }
                // Distance (cells) from the cell to the neighbour's core along each axis.
                let ex = match dx {
                    -1 => ci as f64 + 0.5,
                    1 => (TILE - ci) as f64 - 0.5,
                    _ => 0.0,
                };
                let ez = match dz {
                    -1 => cj as f64 + 0.5,
                    1 => (TILE - cj) as f64 - 0.5,
                    _ => 0.0,
                };
                if ex >= MARGIN as f64 || ez >= MARGIN as f64 {
                    continue;
                }
                let ntz = tz + dz;
                if ntz < 0 || ntz * TILE >= count {
                    continue;
                }
                let d = (ex * ex + ez * ez).sqrt();
                let wgt = 1.0 - smoothstep(0.0, MARGIN as f64, d);
                if wgt <= 0.0 {
                    continue;
                }
                let ntx = (tx + dx).rem_euclid(count.div_euclid(TILE).max(1));
                let t = self.tile(level, ntx, ntz);
                let nu = ci + MARGIN - dx * TILE;
                let nw = cj + MARGIN - dz * TILE;
                let nk = (nw * SPAN + nu) as usize;
                sum += wgt * t.h[nk] as f64;
                water += wgt * depth(&t, nk);
                weight += wgt;
            }
        }
        let r = own.recv[k];
        let (h, water) = (sum / weight, water / weight);
        Cell {
            h: h as f32,
            lake: if water > 0.1 * self.v {
                (h + water) as f32
            } else {
                f32::NAN
            },
            q: own.q[k],
            receiver: (r != OUTLET).then(|| {
                let (di, dj) = D8[r as usize];
                ((i + di).rem_euclid(count), j + dj)
            }),
            sea: own.sea[k],
            channel: own.channel[k],
        }
    }

    /// The grid as level 0: its surface in blocks, every cell's drainage.
    fn grid_cell(&self, i: i64, j: i64) -> Cell {
        let n = self.grid.n() as i64;
        let Some(idx) = self.grid_index(i, j.clamp(0, n - 1)) else {
            return Cell {
                h: 0.0,
                lake: f32::NAN,
                q: 0.0,
                receiver: None,
                sea: false,
                channel: false,
            };
        };
        let g = &*self.grid;
        let lake = if g.flags[idx] & flags::LAKE != 0 {
            (g.water.data[idx] as f64 * self.v) as f32
        } else {
            f32::NAN
        };
        let code = g.flow[idx];
        Cell {
            h: (g.elevation.data[idx] as f64 * self.v) as f32,
            lake,
            q: (g.discharge[idx] as f64 * GRID_Q_UNIT) as f32,
            receiver: (code != crate::planet::NO_FLOW).then(|| {
                let (di, dj) = crate::planet::FLOW_D8[code as usize];
                ((i + di).rem_euclid(n), j.clamp(0, n - 1) + dj)
            }),
            sea: g.flags[idx] & flags::OCEAN != 0,
            channel: true,
        }
    }

    /// The level of the water a cell's river flows on to: the first lake's surface or the sea's
    /// level (0) down its course, followed up to a hundred cells; −∞ if none comes.
    fn base_level(&self, level: usize, mut i: i64, mut j: i64) -> f32 {
        for _ in 0..100 {
            let c = self.cell(level, i, j);
            if c.sea {
                return 0.0;
            }
            if c.lake.is_finite() {
                return c.lake;
            }
            match c.receiver {
                Some((ri, rj)) => (i, j) = (ri, rj),
                None => break,
            }
        }
        f32::NEG_INFINITY
    }

    /// A tile, from the cache or made.
    fn tile(&self, level: usize, tx: i64, tz: i64) -> Arc<Tile> {
        let key = TileKey {
            level: level as u8,
            tx,
            tz,
        };
        self.tiles[level - 1].get_or_insert_with(key, || self.build(level, tx, tz))
    }

    /// The surface at a level (0: the grid's), interpolated bicubically between its cells.
    pub fn height(&self, level: usize, x: f64, z: f64) -> f32 {
        let s = self.cell_size(level);
        let gx = x / s - 0.5;
        let gz = (z + self.c * 0.5) / s - 0.5;
        let (i0, j0) = (gx.floor() as i64, gz.floor() as i64);
        let (fx, fz) = (gx - i0 as f64, gz - j0 as f64);
        let mut rows = [0.0f64; 4];
        for (r, row) in rows.iter_mut().enumerate() {
            let j = j0 - 1 + r as i64;
            let p = [
                self.cell(level, i0 - 1, j).h as f64,
                self.cell(level, i0, j).h as f64,
                self.cell(level, i0 + 1, j).h as f64,
                self.cell(level, i0 + 2, j).h as f64,
            ];
            *row = catmull(p, fx);
        }
        catmull(rows, fz) as f32
    }

    /// The finest surface (blocks) at a point, or the grid's where there are no levels.
    pub fn finest_height(&self, x: f64, z: f64) -> f32 {
        self.height(self.levels.len(), x, z)
    }

    /// How much relief a place takes (0 flat … 1 the roughest mountains), from the grid.
    fn relief_at(&self, gx: f64, gz: f64) -> f64 {
        let g = &*self.grid;
        let e = g.elevation.bilinear(gx, gz) as f64;
        let uplift = g.uplift.bilinear(gx, gz).max(0.0) as f64;
        if e < 0.0 {
            // The shelf smooth, the slope and the deep floor in abyssal hills.
            return 0.008 + 0.07 * smoothstep(-150.0, -2500.0, e);
        }
        let mountains = smoothstep(300.0, 3500.0, uplift).powf(0.8);
        // Coasts and lowlands nearly flat, higher land hillier.
        let land = 0.012 + 0.10 * smoothstep(80.0, 1500.0, e);
        mountains.max(land)
    }

    /// Builds a tile of `level` (≥ 1).
    fn build(&self, level: usize, tx: i64, tz: i64) -> Tile {
        let lv = self.levels[level - 1];
        let s = lv.cell;
        let ps = self.cell_size(level - 1);
        let n = SPAN as usize;
        let len = n * n;
        let i0 = tx * TILE - MARGIN;
        let j0 = tz * TILE - MARGIN;
        // ------------------------------------------------ the parent's cells about the span
        let pi0 = i0.div_euclid(RATIO) - 2;
        let pj0 = j0.div_euclid(RATIO) - 2;
        let pw = SPAN / RATIO + 5;
        let mut parent = Vec::with_capacity((pw * pw) as usize);
        for pj in pj0..pj0 + pw {
            for pi in pi0..pi0 + pw {
                parent.push(self.cell(level - 1, pi, pj));
            }
        }
        let pcell = |pi: i64, pj: i64| -> &Cell {
            let u = (pi - pi0).clamp(0, pw - 1);
            let w = (pj - pj0).clamp(0, pw - 1);
            &parent[(w * pw + u) as usize]
        };
        // ------------------------------------------------ the surface with this level's relief
        let noise = &self.noise[level - 1];
        let g = &*self.grid;
        let mut h = vec![0.0f32; len];
        let mut rain = vec![0.0f32; len];
        let mut erodibility = vec![1.0f32; len];
        let mut sea = vec![false; len];
        let mut outside = vec![false; len];
        // The parent's lake surface about each cell, NaN where it has none, and the lakes' share
        // of the four parent cells about it.
        let mut parent_lake = vec![f32::NAN; len];
        let mut lake_share = vec![0.0f32; len];
        let count = lv.count;
        for w in 0..n {
            let j = j0 + w as i64;
            for u in 0..n {
                let i = i0 + u as i64;
                let k = w * n + u;
                if j < 0 || j >= count {
                    outside[k] = true;
                    continue;
                }
                let (x, z) = self.centre(level, i, j);
                // Bicubic from the parent's cells.
                let gx = x / ps - 0.5;
                let gz = (z + self.c * 0.5) / ps - 0.5;
                let (ci, cj) = (gx.floor() as i64, gz.floor() as i64);
                let (fx, fz) = (gx - ci as f64, gz - cj as f64);
                let mut rows = [0.0f64; 4];
                for (r, row) in rows.iter_mut().enumerate() {
                    let pj = cj - 1 + r as i64;
                    let p = [
                        pcell(ci - 1, pj).h as f64,
                        pcell(ci, pj).h as f64,
                        pcell(ci + 1, pj).h as f64,
                        pcell(ci + 2, pj).h as f64,
                    ];
                    *row = catmull(p, fx);
                }
                let base = catmull(rows, fz);
                // The grid's view of the place.
                let (ggx, ggz) = g.geom.grid_coords(x.rem_euclid(self.c), z);
                let relief = self.relief_at(ggx, ggz);
                // Both of the warp's parts periodic round the planet (the second taken far off
                // along the other axis).
                let wx = x + noise.warp.sample2(x, z) * s * RATIO as f64 * 0.5;
                let wz = z + noise.warp.sample2(x, z + WARP_APART) * s * RATIO as f64 * 0.5;
                let mut add = 0.0;
                for (oct, amp) in &noise.octaves {
                    add += oct.sample2(wx, wz) * amp;
                }
                let mut hk = base + add * relief * self.v;
                let idx = g.cell_at(x.rem_euclid(self.c), z);
                // The parent's sea, where this level's surface stays under it, is the sea; the
                // low ground joined to it is found below.
                let (pi, pj) = (
                    (x / ps).floor() as i64,
                    ((z + self.c * 0.5) / ps).floor() as i64,
                );
                sea[k] = hk < 0.0 && pcell(pi, pj).sea;
                // The parent's lakes keep their extent. About their cells' outline (where their
                // share of the four parent cells about this one is a half) the ground is drawn
                // towards a slope through their surface, down within and up without, with this
                // level's relief on it: the shore wanders about the outline, and the finer
                // relief cannot spill a lake over a plain or drain it.
                let mut share = 0.0;
                let mut surface = f32::NEG_INFINITY;
                for (dj, wz) in [(0, 1.0 - fz), (1, fz)] {
                    for (di, wx) in [(0, 1.0 - fx), (1, fx)] {
                        let lake = pcell(ci + di, cj + dj).lake;
                        if lake.is_finite() {
                            share += wx * wz;
                            surface = surface.max(lake);
                        }
                    }
                }
                if share > 0.0 {
                    let rough = 0.3 * noise.reach * relief * self.v + 0.5 * self.v;
                    let shore =
                        surface as f64 + (0.5 - share) * 2.0 * rough + add * relief * self.v;
                    let t = 1.0 - (2.0 * share - 1.0).abs();
                    hk += (shore - hk) * t;
                    lake_share[k] = share as f32;
                    parent_lake[k] = surface;
                }
                h[k] = hk as f32;
                let p = g.precipitation.bilinear(ggx, ggz) as f64;
                let t = g.temperature.bilinear(ggx, ggz) as f64;
                // Runoff (mm/yr): what the rain leaves after evaporation, more of it in cool
                // wet climates.
                let share = (0.12 + 0.45 * smoothstep(300.0, 2500.0, p)
                    - 0.15 * smoothstep(10.0, 28.0, t))
                .clamp(0.03, 0.75);
                // A cell's real area: Mercator's blocks are cos φ metres apart.
                let lat = g.geom.planet().latitude(z);
                let area_m2 = (s * lat.cos() / self.v).powi(2);
                rain[k] = (area_m2 * p * share / 1000.0 / YEAR_S) as f32;
                // Hard old rock wears slowly, soft young rock fast.
                erodibility[k] = match g.province[idx] {
                    crate::planet::province::SHIELD => 0.5,
                    crate::planet::province::BASIN | crate::planet::province::RIFT => 1.4,
                    _ => 1.0,
                };
            }
        }
        // The parent's lakes stand where they stood, over the ground below their surface within
        // their outline and joined to it about the shore: every tile sees the same water there.
        let mut held = vec![f32::NAN; len];
        let mut reach: VecDeque<usize> = VecDeque::new();
        for k in 0..len {
            if !outside[k] && lake_share[k] > 0.5 && h[k] < parent_lake[k] {
                held[k] = parent_lake[k];
                reach.push_back(k);
            }
        }
        while let Some(k) = reach.pop_front() {
            let (u, w) = ((k % n) as i64, (k / n) as i64);
            for &(du, dw) in &D8 {
                let (nu, nw) = (u + du, w + dw);
                if nu < 0 || nw < 0 || nu >= n as i64 || nw >= n as i64 {
                    continue;
                }
                let nb = (nw * n as i64 + nu) as usize;
                if held[nb].is_nan() && !outside[nb] && lake_share[nb] > 0.0 && h[nb] < held[k] {
                    held[nb] = held[k];
                    reach.push_back(nb);
                }
            }
        }
        // The sea floods the low ground joined to it, its bays and inlets; hollows below the
        // sea's level that it cannot reach stay land.
        let mut channel = vec![false; len];
        for k in 0..len {
            sea[k] &= held[k].is_nan();
        }
        flood_sea(&mut sea, &h, &channel, &outside, &held, n);
        // ------------------------------------------------ the parent's rivers as channels
        let mut recv = vec![OUTLET; len];
        let mut inflow = vec![0.0f32; len];
        let min_q = channel_threshold(level);
        // The water each parent cell's river flows on to.
        let mut floors: rustc_hash::FxHashMap<(i64, i64), f32> = Default::default();
        // The lowest a channel's bed may be cut: where the reach it drains along ends.
        let mut bottom = vec![f32::NEG_INFINITY; len];
        // A river runs on its lake's surface where it crosses one.
        let top = |c: &Cell| {
            if c.lake.is_finite() {
                c.h.max(c.lake)
            } else {
                c.h
            }
        };
        for pj in pj0..pj0 + pw {
            for pi in pi0..pi0 + pw {
                let pc = *pcell(pi, pj);
                let Some((ri, rj)) = pc.receiver else {
                    continue;
                };
                if pc.q < min_q {
                    continue;
                }
                let rc = self.cell(level - 1, ri, rj);
                if pc.lake.is_finite() && rc.lake.is_finite() {
                    // Water crossing a lake is the lake's, not a river.
                    continue;
                }
                let a = self.node(level - 1, pi, pj);
                let b = self.node(level - 1, ri, rj);
                // A river's bed lies below the parent's surface by a share of the relief this
                // level adds there (keeping half its height above the sea, so lowland rivers
                // still fall to it), but never below the water it flows on to: a river on land
                // falls to the sea's level, not to the sea floor, and to a lake's surface. A
                // lake's outflow leaves at its surface, and an inflow ends there.
                let land = !pc.sea;
                let mut floor_of = |i: i64, j: i64| -> f32 {
                    *floors
                        .entry((i, j))
                        .or_insert_with(|| self.base_level(level - 1, i, j))
                };
                let (fa, fb) = (floor_of(pi, pj), floor_of(ri, rj));
                let bed = |c: &Cell, at: (f64, f64), floor: f32| -> f32 {
                    if c.lake.is_finite() {
                        return top(c);
                    }
                    let (gx, gz) = g.geom.grid_coords(at.0.rem_euclid(self.c), at.1);
                    let mut low = VALLEY * noise.reach * self.relief_at(gx, gz) * self.v;
                    if land {
                        low = low.min(0.5 * (top(c) as f64).max(0.0));
                    }
                    let b = ((top(c) as f64 - low) as f32).max(floor.min(top(c)));
                    if land { b.max(0.0) } else { b }
                };
                let rh = bed(&rc, b, fb);
                let ha = bed(&pc, a, fa).max(rh);
                let hb = rh;
                let path = meander(
                    a,
                    b,
                    ps,
                    s,
                    hash_2d(
                        hash2(self.seed, 0x5ee + level as u64),
                        pi.rem_euclid(self.count(level - 1)) as i32,
                        pj as i32,
                    ),
                    self.c,
                );
                self.rasterize(
                    &path,
                    (ha, hb),
                    pc.q,
                    (i0, j0, count),
                    s,
                    &sea,
                    &mut h,
                    &mut channel,
                    &mut recv,
                    &mut inflow,
                    &mut bottom,
                );
            }
        }
        // A river never runs round in a ring (should crossing reaches close one, it ends there).
        break_rings(&mut recv, &channel, n);
        // Rivers fall all the way: where reaches meet or cross, a bed standing above the one
        // before it is cut down to it, though never below where its reach ends.
        for _ in 0..4 * SPAN {
            let mut cut = false;
            for k in 0..len {
                if !channel[k] {
                    continue;
                }
                if let Some(r) = target(k, recv[k], n)
                    && channel[r]
                    && h[r] > h[k].max(bottom[r])
                {
                    h[r] = h[k].max(bottom[r]);
                    cut = true;
                }
            }
            if !cut {
                break;
            }
        }
        // A river running in at a lake's surface joins the lake; one beside it below its surface
        // (the lake spills into it) or above it stays a river.
        for k in 0..len {
            if channel[k] && (h[k] - held[k]).abs() > 0.01 {
                held[k] = f32::NAN;
            }
        }
        // The sea comes up the rivers whose beds lie at its level: estuaries.
        flood_sea(&mut sea, &h, &channel, &outside, &held, n);
        // ------------------------------------------------ drainage and erosion
        let mut base = vec![false; len];
        for w in 0..n {
            for u in 0..n {
                let k = w * n + u;
                base[k] = outside[k]
                    || sea[k]
                    || channel[k]
                    || held[k].is_finite()
                    || u == 0
                    || w == 0
                    || u == n - 1
                    || w == n - 1;
            }
        }
        // `z` is the ground, `wet` the surface water drains over: the ground where it is dry, a
        // hollow's brim where it holds water. Each round the hollows fill (or are cut through),
        // the water runs, and the ground wears towards where it drains: a hollow's outlet wears
        // down with all its water through it, so a lake drains as its outlet is cut.
        let k_level = 0.08;
        let talus = (0.84 * self.v * s) as f32;
        let breach = breach_depth(level) * self.v as f32;
        // A cell's runoff at 300 mm a year: the discharge the wearing is reckoned against.
        let q_cell = ((s / self.v).powi(2) * 0.3 / YEAR_S) as f32;
        let mut z = h;
        let mut wet = z.clone();
        for it in 0..8 {
            // The parent's lakes stand at their surface, which the water about them drains to.
            for k in 0..len {
                wet[k] = if held[k].is_finite() { held[k] } else { z[k] };
            }
            fill(&mut wet, &base, n, 1e-3, breach);
            // A cut is the ground's.
            for (zk, &wk) in z.iter_mut().zip(&wet) {
                *zk = zk.min(wk);
            }
            if it == 7 {
                break;
            }
            let rec = steepest(&wet, &base, &channel, &recv, n, s as f32);
            let order = stack(&rec, n);
            let q = accumulate(&rec, &order, &rain, &inflow, &channel, n);
            // Implicit stream power (n = 1): f = K · ((q / q_cell)^0.5 − 1) / (distance in
            // cells), so ground that drains only itself (a ridge, a summit) is left to slump.
            for &c in &order {
                let c = c as usize;
                let r = rec[c];
                if r == OUTLET || base[c] || wet[c] > z[c] + 1e-2 {
                    continue;
                }
                let (du, dw) = D8[r as usize];
                let rk = (c as i64 + dw * n as i64 + du) as usize;
                let dist = if du != 0 && dw != 0 {
                    std::f32::consts::SQRT_2
                } else {
                    1.0
                };
                let f = k_level * erodibility[c] * ((q[c] / q_cell).sqrt() - 1.0).max(0.0) / dist;
                // Land wears down to the sea's level at most.
                let hr = if z[c] >= 0.0 {
                    wet[rk].max(0.0)
                } else {
                    wet[rk]
                };
                if z[c] > hr {
                    z[c] = (z[c] + f * hr) / (1.0 + f);
                    wet[c] = z[c];
                }
            }
            slump(&mut z, &base, n, talus);
        }
        // ------------------------------------------------ results
        let rec = steepest(&wet, &base, &channel, &recv, n, s as f32);
        let order = stack(&rec, n);
        let q = accumulate(&rec, &order, &rain, &inflow, &channel, n);
        let lake = lakes(
            &wet,
            &mut z,
            &base,
            &held,
            n,
            lake_depth(level) * self.v as f32,
        );
        Tile {
            h: z,
            lake,
            q,
            recv: rec,
            channel,
            sea,
        }
    }

    /// Marks the cells a channel's path crosses, from `a` to `b` falling `levels.0` to
    /// `levels.1`, each draining to the next; it ends where it meets the sea. Where reaches
    /// cross, a cell drains along the one that ends lowest, and `bottom` keeps that end: the
    /// lowest its bed may be cut.
    #[allow(clippy::too_many_arguments)]
    fn rasterize(
        &self,
        path: &[(f64, f64)],
        levels: (f32, f32),
        q: f32,
        (i0, j0, count): (i64, i64, i64),
        s: f64,
        sea: &[bool],
        h: &mut [f32],
        channel: &mut [bool],
        recv: &mut [u8],
        inflow: &mut [f32],
        bottom: &mut [f32],
    ) {
        let n = SPAN;
        let total: f64 = path
            .windows(2)
            .map(|p| ((p[1].0 - p[0].0).powi(2) + (p[1].1 - p[0].1).powi(2)).sqrt())
            .sum();
        if total <= 0.0 {
            return;
        }
        let mut walked = 0.0;
        let mut prev: Option<(i64, i64)> = None;
        let span_i = |x: f64| -> i64 {
            // The cell index along x relative to the span, nearest to it across the wrap.
            let i = (x / s).floor() as i64;
            let d = (i - i0).rem_euclid(count);
            if d > count / 2 { d - count } else { d }
        };
        for p in path.windows(2) {
            let seg = ((p[1].0 - p[0].0).powi(2) + (p[1].1 - p[0].1).powi(2)).sqrt();
            let steps = (seg / (0.35 * s)).ceil().max(1.0) as usize;
            for t in 0..=steps {
                let f = t as f64 / steps as f64;
                let x = p[0].0 + (p[1].0 - p[0].0) * f;
                let z = p[0].1 + (p[1].1 - p[0].1) * f;
                let at = (walked + seg * f) / total;
                let u = span_i(x);
                let w = ((z + self.c * 0.5) / s).floor() as i64 - j0;
                if (u, w) == prev.unwrap_or((i64::MIN, i64::MIN)) {
                    continue;
                }
                let level = levels.0 + (levels.1 - levels.0) * at as f32;
                if let Some((pu, pw)) = prev
                    && (0..n).contains(&pu)
                    && (0..n).contains(&pw)
                {
                    // The previous cell drains here (a diagonal or straight step), unless a
                    // reach through it falling lower takes its water.
                    let (du, dw) = ((u - pu).clamp(-1, 1), (w - pw).clamp(-1, 1));
                    let pk = (pw * n + pu) as usize;
                    // (A reach crossing its own path drains on from its last visit, so its
                    // loops hold no water.)
                    if let Some(code) = D8.iter().position(|&o| o == (du, dw))
                        && (recv[pk] == OUTLET || levels.1 <= bottom[pk])
                    {
                        recv[pk] = code as u8;
                        bottom[pk] = levels.1;
                    }
                }
                prev = Some((u, w));
                if !(0..n).contains(&u) || !(0..n).contains(&w) {
                    continue;
                }
                let k = (w * n + u) as usize;
                if sea[k] {
                    continue;
                }
                if channel[k] {
                    // Two reaches meet: the lower bed and the larger flow.
                    h[k] = h[k].min(level);
                    inflow[k] = inflow[k].max(q);
                } else {
                    channel[k] = true;
                    h[k] = level;
                    inflow[k] = q;
                    bottom[k] = levels.1;
                }
            }
            walked += seg;
        }
    }
}

/// The lakes over ground `z` whose hollows the water's surface `wet` fills, with the parent's
/// lakes `held` (their surfaces, NaN elsewhere): lake surfaces, NaN elsewhere. A hollow joined
/// to a parent's lake is that lake's; one of this level's own is a lake if it fits in a tile's
/// margin, so every tile that sees it sees it whole and agrees on it, and somewhere it is deeper
/// than `deep` or its bed lies below the sea's level (the water table standing at it). A larger
/// one of its own (the parent saw no lake there) is filled to its brim with what washed into
/// it, a basin's flat floor.
fn lakes(wet: &[f32], z: &mut [f32], base: &[bool], held: &[f32], n: usize, deep: f32) -> Vec<f32> {
    const DAMP: f32 = 0.01;
    let len = wet.len();
    let water = |k: usize| held[k].is_finite() || (!base[k] && wet[k] - z[k] > DAMP);
    let mut lake = vec![f32::NAN; len];
    let mut seen = vec![false; len];
    let mut hollow = Vec::new();
    let mut basins = Vec::new();
    for start in 0..len {
        if seen[start] || !water(start) || wet[start] <= 0.0 {
            continue;
        }
        // One hollow: the water-covered cells joined to this one.
        hollow.clear();
        hollow.push(start);
        seen[start] = true;
        let mut i = 0;
        let (mut lo, mut hi) = ((i64::MAX, i64::MAX), (i64::MIN, i64::MIN));
        let (mut parents, mut own) = (false, false);
        while i < hollow.len() {
            let k = hollow[i];
            i += 1;
            parents |= held[k].is_finite();
            own |= wet[k] - z[k] > deep || z[k] < 0.0;
            let (u, w) = ((k % n) as i64, (k / n) as i64);
            lo = (lo.0.min(u), lo.1.min(w));
            hi = (hi.0.max(u), hi.1.max(w));
            for &(du, dw) in &D8 {
                let (nu, nw) = (u + du, w + dw);
                if nu < 0 || nw < 0 || nu >= n as i64 || nw >= n as i64 {
                    continue;
                }
                let nb = (nw * n as i64 + nu) as usize;
                if !seen[nb] && water(nb) {
                    seen[nb] = true;
                    hollow.push(nb);
                }
            }
        }
        let small = hi.0 - lo.0 < MARGIN && hi.1 - lo.1 < MARGIN;
        if parents || (own && small) {
            for &k in &hollow {
                lake[k] = if held[k].is_finite() { held[k] } else { wet[k] };
            }
        } else if !small {
            basins.extend_from_slice(&hollow);
        }
    }
    for k in basins {
        z[k] = wet[k];
    }
    lake
}

/// Ends any ring the channels' receivers close: following each channel's course, a cell met
/// twice on one walk gives up its receiver.
fn break_rings(recv: &mut [u8], channel: &[bool], n: usize) {
    let len = recv.len();
    // 0: not yet walked; walk number + 1 while on a walk, or once its course is known to end.
    let mut mark = vec![0u32; len];
    for (walk, start) in (0..len).filter(|&k| channel[k]).enumerate() {
        let walk = walk as u32 + 1;
        let mut k = start;
        while channel[k] && mark[k] == 0 {
            mark[k] = walk;
            match target(k, recv[k], n) {
                Some(r) if mark[r] == walk => {
                    recv[k] = OUTLET;
                    break;
                }
                Some(r) => k = r,
                None => break,
            }
        }
    }
}

/// Spreads the sea from its cells over the ground below its level joined to them (side by side,
/// not corner to corner), and up channels whose beds lie at its level; a lake (`held`) keeps it
/// out.
fn flood_sea(
    sea: &mut [bool],
    h: &[f32],
    channel: &[bool],
    outside: &[bool],
    held: &[f32],
    n: usize,
) {
    let mut reach: VecDeque<usize> = (0..sea.len()).filter(|&k| sea[k]).collect();
    while let Some(k) = reach.pop_front() {
        let (u, w) = ((k % n) as i64, (k / n) as i64);
        for (du, dw) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nu, nw) = (u + du, w + dw);
            if nu < 0 || nw < 0 || nu >= n as i64 || nw >= n as i64 {
                continue;
            }
            let nb = (nw * n as i64 + nu) as usize;
            let low = h[nb] < 0.0 || (channel[nb] && h[nb] <= 0.0);
            if !sea[nb] && !outside[nb] && held[nb].is_nan() && low {
                sea[nb] = true;
                reach.push_back(nb);
            }
        }
    }
}

/// Discharge (m³/s) from which a level's parent's rivers are kept as its channels: a river
/// draining some hundreds of km² from the grid, some tens from the first level, a few from
/// the second.
fn channel_threshold(level: usize) -> f32 {
    match level {
        1 => 3.0,
        2 => 1.0,
        _ => 0.15,
    }
}

/// How deep (m) a hollow must be to hold a lake at a level.
fn lake_depth(level: usize) -> f32 {
    match level {
        1 => 3.0,
        2 => 1.5,
        _ => 0.8,
    }
}

/// The path of a reach from `a` to `b` (a parent cell of size `ps` apart), wandering by a
/// fractal of midpoints displaced across it, down to the child's cell size `s`. The same reach
/// always takes the same path (`hash`).
fn meander(a: (f64, f64), b: (f64, f64), ps: f64, s: f64, hash: u64, c: f64) -> Vec<(f64, f64)> {
    // Unwrap b near a.
    let mut dx = b.0 - a.0;
    if dx > c * 0.5 {
        dx -= c;
    } else if dx < -c * 0.5 {
        dx += c;
    }
    let b = (a.0 + dx, b.1);
    let mut path = vec![a, b];
    let mut depth = 0u64;
    let mut seg = ps * 1.5;
    while seg > s * 1.5 && depth < 6 {
        let mut next = Vec::with_capacity(path.len() * 2);
        for (k, w) in path.windows(2).enumerate() {
            let (p, q) = (w[0], w[1]);
            let (mx, mz) = ((p.0 + q.0) * 0.5, (p.1 + q.1) * 0.5);
            let (lx, lz) = (q.0 - p.0, q.1 - p.1);
            let len = (lx * lx + lz * lz).sqrt().max(1e-9);
            let r = unit_f64(hash2(hash, depth << 32 | k as u64)) - 0.5;
            let off = r * 0.45 * len;
            next.push(p);
            next.push((mx - lz / len * off, mz + lx / len * off));
        }
        next.push(*path.last().expect("two points"));
        path = next;
        seg *= 0.5;
        depth += 1;
    }
    path
}

#[inline]
fn catmull(p: [f64; 4], t: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p[1])
        + (-p[0] + p[2]) * t
        + (2.0 * p[0] - 5.0 * p[1] + 4.0 * p[2] - p[3]) * t2
        + (-p[0] + 3.0 * p[1] - 3.0 * p[2] + p[3]) * t3)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Ord32(f32);
impl Eq for Ord32 {}
impl PartialOrd for Ord32 {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Ord32 {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&o.0)
    }
}

/// Priority-Flood+ε over an n × n span from its base cells, so every other cell drains. A
/// hollow up to `breach` deep is drained by cutting its way out (the path the flood came by
/// lowered below it, as a stream would have cut it: Lindsay 2016); a deeper one is filled to
/// its brim, a lake.
fn fill(h: &mut [f32], base: &[bool], n: usize, eps: f32, breach: f32) {
    let mut closed = base.to_vec();
    let mut from = vec![u32::MAX; h.len()];
    let mut open: BinaryHeap<Reverse<(Ord32, u32)>> = BinaryHeap::new();
    let mut pit: VecDeque<u32> = VecDeque::new();
    for k in 0..h.len() {
        if base[k] {
            open.push(Reverse((Ord32(h[k]), k as u32)));
        }
    }
    while let Some(c) = pit
        .pop_front()
        .or_else(|| open.pop().map(|Reverse((_, c))| c))
    {
        let c = c as usize;
        let (u, w) = ((c % n) as i64, (c / n) as i64);
        for &(du, dw) in &D8 {
            let (nu, nw) = (u + du, w + dw);
            if nu < 0 || nw < 0 || nu >= n as i64 || nw >= n as i64 {
                continue;
            }
            let nb = (nw * n as i64 + nu) as usize;
            if closed[nb] {
                continue;
            }
            closed[nb] = true;
            from[nb] = c as u32;
            let hc = h[c];
            if h[nb] <= hc + eps {
                // A hollow below the sea's level is not cut through, which would let the sea in.
                if h[nb] >= 0.0
                    && hc - h[nb] <= breach
                    && can_cut(h, base, &from, c, h[nb] - eps, eps)
                {
                    // Cut the way out: down the flood's path, each cell below the last.
                    let mut level = h[nb] - eps;
                    let mut p = c;
                    while !base[p] && h[p] > level {
                        h[p] = level;
                        level -= eps;
                        if from[p] == u32::MAX {
                            break;
                        }
                        p = from[p] as usize;
                    }
                    open.push(Reverse((Ord32(h[nb]), nb as u32)));
                } else {
                    h[nb] = hc + eps;
                    pit.push_back(nb as u32);
                }
            } else {
                open.push(Reverse((Ord32(h[nb]), nb as u32)));
            }
        }
    }
}

/// Whether the flood's path back from `c` can be cut to fall from `level`: it must reach a fixed
/// cell (the sea, a channel, the span's edge) that lies lower still, as fixed cells are not cut.
fn can_cut(h: &[f32], base: &[bool], from: &[u32], c: usize, mut level: f32, eps: f32) -> bool {
    let mut p = c;
    loop {
        if base[p] {
            return h[p] <= level;
        }
        if h[p] <= level {
            return true;
        }
        level -= eps;
        if from[p] == u32::MAX {
            return true;
        }
        p = from[p] as usize;
    }
}

/// How deep (m) a hollow may be and still be cut through at a level, rather than hold a lake:
/// as deep as the level's relief makes its hollows, so only the deepest hold water. Rivers keep
/// pace with the land's rise, so a mountain range drains through its valleys; lakes are where
/// the ground sank or the parent held one.
fn breach_depth(level: usize) -> f32 {
    match level {
        1 => 400.0,
        2 => 100.0,
        _ => 25.0,
    }
}

/// Steepest-descent receivers; channels keep theirs, base cells drain out.
fn steepest(h: &[f32], base: &[bool], channel: &[bool], fixed: &[u8], n: usize, s: f32) -> Vec<u8> {
    let mut rec = vec![OUTLET; h.len()];
    for k in 0..h.len() {
        if channel[k] {
            rec[k] = fixed[k];
            continue;
        }
        if base[k] {
            continue;
        }
        let (u, w) = ((k % n) as i64, (k / n) as i64);
        let mut best = OUTLET;
        let mut best_slope = 0.0f32;
        for (code, &(du, dw)) in D8.iter().enumerate() {
            let (nu, nw) = (u + du, w + dw);
            if nu < 0 || nw < 0 || nu >= n as i64 || nw >= n as i64 {
                continue;
            }
            let nb = (nw * n as i64 + nu) as usize;
            let drop = h[k] - h[nb];
            if drop > 0.0 {
                let d = if du != 0 && dw != 0 { s * 1.414 } else { s };
                if drop / d > best_slope {
                    best_slope = drop / d;
                    best = code as u8;
                }
            }
        }
        rec[k] = best;
    }
    rec
}

/// The receiver's index of a cell, if any.
#[inline]
fn target(k: usize, code: u8, n: usize) -> Option<usize> {
    if code == OUTLET {
        return None;
    }
    let (du, dw) = D8[code as usize];
    let (u, w) = ((k % n) as i64 + du, (k / n) as i64 + dw);
    (u >= 0 && w >= 0 && u < n as i64 && w < n as i64).then(|| (w * n as i64 + u) as usize)
}

/// Outlets first, then every cell after the cell it drains to.
fn stack(rec: &[u8], n: usize) -> Vec<u32> {
    let len = rec.len();
    let mut donors: Vec<Vec<u32>> = vec![Vec::new(); len];
    let mut roots = Vec::new();
    for (k, &code) in rec.iter().enumerate() {
        match target(k, code, n) {
            Some(r) => donors[r].push(k as u32),
            None => roots.push(k as u32),
        }
    }
    let mut order = Vec::with_capacity(len);
    let mut queue: VecDeque<u32> = roots.into();
    while let Some(c) = queue.pop_front() {
        order.push(c);
        for &d in &donors[c as usize] {
            queue.push_back(d);
        }
    }
    order
}

/// Discharge: each cell's own runoff carried downstream; a channel carries at least its
/// parent river's.
fn accumulate(
    rec: &[u8],
    order: &[u32],
    rain: &[f32],
    inflow: &[f32],
    channel: &[bool],
    n: usize,
) -> Vec<f32> {
    let mut q = rain.to_vec();
    for &c in order.iter().rev() {
        let c = c as usize;
        if channel[c] {
            q[c] = q[c].max(inflow[c]);
        }
        if let Some(r) = target(c, rec[c], n) {
            q[r] += q[c];
        }
    }
    q
}

/// Slopes steeper than `max_drop` per cell slump half their excess onto the lower neighbour.
fn slump(h: &mut [f32], base: &[bool], n: usize, max_drop: f32) {
    let src = h.to_vec();
    for k in 0..h.len() {
        if base[k] {
            continue;
        }
        let (u, w) = ((k % n) as i64, (k / n) as i64);
        let mut best: Option<(usize, f32)> = None;
        for &(du, dw) in &D8 {
            let (nu, nw) = (u + du, w + dw);
            if nu < 0 || nw < 0 || nu >= n as i64 || nw >= n as i64 {
                continue;
            }
            let nb = (nw * n as i64 + nu) as usize;
            let d = if du != 0 && dw != 0 { 1.414 } else { 1.0 };
            let excess = src[k] - src[nb] - max_drop * d;
            if excess > best.map_or(0.0, |b| b.1) {
                best = Some((nb, excess));
            }
        }
        if let Some((nb, excess)) = best {
            h[k] -= excess * 0.25;
            if !base[nb] {
                h[nb] += excess * 0.25;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WorldGenSettings;
    use hearth_math::PlanetSize;

    fn earth(seed: u64) -> Relief {
        let s = WorldGenSettings {
            seed,
            planet_size: PlanetSize::Earth,
            grid_resolution: 256,
        };
        Relief::new(Arc::new(PlanetGrid::build(&s.sanitized(), &|_, _| {})))
    }

    #[test]
    fn earth_has_levels_down_to_tens_of_metres_and_a_test_planet_none() {
        let r = earth(3);
        let cells: Vec<f64> = r.levels().iter().map(|l| l.cell).collect();
        assert!(cells.len() >= 3, "{cells:?}");
        assert!(*cells.last().unwrap() >= MIN_CELL && *cells.last().unwrap() < MIN_CELL * 8.0);
        let s = WorldGenSettings {
            seed: 3,
            planet_size: PlanetSize::Standard,
            grid_resolution: 2048,
        };
        let std = Relief::new(Arc::new(PlanetGrid::build(&s.sanitized(), &|_, _| {})));
        assert!(std.levels().is_empty(), "the grid is already fine");
    }

    /// A land cell of the grid away from the poles: (its centre's x, z) at the first level's
    /// indices.
    fn land(r: &Relief, nth: usize) -> (i64, i64) {
        let g = &r.grid;
        let n = g.n();
        let idx = (n * n / 4..n * n * 3 / 4)
            .filter(|&i| g.flags[i] & flags::OCEAN == 0 && g.elevation.data[i] > 50.0)
            .nth(nth * 97)
            .expect("land");
        let (x, z) = g.geom.world_xz(idx % n, idx / n);
        let cell = r.levels()[0].cell;
        ((x / cell) as i64, ((z + r.c * 0.5) / cell) as i64)
    }

    #[test]
    fn rivers_fall_all_the_way_down_through_every_level() {
        let r = earth(11);
        for nth in 0..3 {
            let (mut i0, mut j0) = land(&r, nth);
            for level in 1..=r.levels().len() {
                if level > 1 {
                    (i0, j0) = (i0 * RATIO, j0 * RATIO);
                }
                for j in j0 - 40..j0 + 40 {
                    for i in i0 - 40..i0 + 40 {
                        let c = r.cell(level, i, j);
                        let Some((ri, rj)) = c.receiver else {
                            continue;
                        };
                        if !c.channel || c.sea || c.lake.is_finite() {
                            continue;
                        }
                        let next = r.cell(level, ri, rj);
                        let surface = if next.lake.is_finite() {
                            next.lake
                        } else {
                            next.h
                        };
                        assert!(
                            next.sea || surface <= c.h + 0.5,
                            "level {level}: ({i}, {j}) at {} m runs up to ({ri}, {rj}) at {surface} m",
                            c.h
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn each_level_keeps_its_parents_surface_on_the_whole() {
        let r = earth(13);
        let (mut i0, mut j0) = land(&r, 1);
        for level in 1..=r.levels().len() {
            if level > 1 {
                (i0, j0) = (i0 * RATIO, j0 * RATIO);
            }
            let cell = r.levels()[level - 1].cell;
            let (mut mean, mut parent) = (0.0, 0.0);
            for j in j0 - 30..j0 + 30 {
                for i in i0 - 30..i0 + 30 {
                    mean += r.cell(level, i, j).h as f64;
                    let x = (i as f64 + 0.5) * cell;
                    let z = -r.c * 0.5 + (j as f64 + 0.5) * cell;
                    parent += r.height(level - 1, x, z) as f64;
                }
            }
            let (mean, parent) = (mean / 3600.0, parent / 3600.0);
            // Within a tenth of the height, or a few metres on low ground.
            assert!(
                (mean - parent).abs() <= 0.1 * parent.abs() + 5.0,
                "level {level}: {mean:.1} m against its parent's {parent:.1} m"
            );
        }
    }

    #[test]
    fn a_cell_has_one_value_whichever_tile_is_made_first() {
        let a = earth(5);
        let b = earth(5);
        let lv = a.levels()[0];
        // Cells along a tile's edge, asked for from either side first.
        let j = 3 * TILE + 5;
        let i = 40 * TILE;
        let va: Vec<f32> = (i - 3..i + 3).map(|ii| a.cell(1, ii, j).h).collect();
        let vb: Vec<f32> = (i - 3..i + 3).rev().map(|ii| b.cell(1, ii, j).h).collect();
        let vb: Vec<f32> = vb.into_iter().rev().collect();
        assert_eq!(va, vb);
        assert!(lv.cell > 0.0);
    }
}

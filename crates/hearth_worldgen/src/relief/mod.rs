//! Terrain refinement (E §5.2): nested levels between the planet grid and the blocks.
//!
//! The planet grid's cells are some 20 km across on Earth and a block is a metre. Between them
//! lie refinement levels, each [`RATIO`] times finer than the one above (2.4 km, 306 m, 38 m and
//! 4.8 m on Earth), as many as stay coarser than [`MIN_CELL`]; below the last, the blocks take
//! their shape from its surface and the ground's own roughness. The finest, the walking scale's
//! (G1b), is made only about the players: a coarser read stands in for it
//! ([`Relief::walking_proxy`]). A small test planet, whose grid is already fine, has none.
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
//!
//! The walking level's own parts (its second drainage pass, its tiles made ahead of the cubes,
//! its stand-in) are in `walking`.

use std::collections::VecDeque;
use std::sync::{Arc, OnceLock};

use hearth_math::hash::{derive_seed, hash_2d, hash2, unit_f64};

use crate::cubegen::cache::Cache;
use crate::noise::BlockFbm;
use crate::planet::{PlanetGrid, flags};
use drainage::{
    Routing, accumulate, breach_depth, break_rings, diffuse, fill, flood_sea, lakes, slump, stack,
    steepest, target,
};

mod courses;
mod drainage;
pub(crate) mod gullies;
mod roughness;
mod walking;

pub use roughness::Roughness;

/// Each level's cells are this many times finer than its parent's.
pub const RATIO: i64 = 8;
/// Core cells along a tile's side.
pub const TILE: i64 = 64;
/// Cells of margin on every side of a tile's core, computed and blended into its neighbours.
pub const MARGIN: i64 = 16;
/// Cells along the side of a tile's computed span.
const SPAN: i64 = TILE + 2 * MARGIN;
/// The finest level's cells are no smaller than this (blocks): the walking scale's ~5 m.
pub const MIN_CELL: f64 = 4.0;
/// A grid whose cells are smaller than this (blocks) has no refinement levels.
pub const MIN_GRID_CELL: f64 = 1000.0;
/// Tiles kept per level at least (the rest as the memory budget allows, E4.1 §4.7).
const TILES_KEPT_MIN: usize = 64;
/// A tile's memory (bytes): its six arrays over the span.
const TILE_BYTES: usize = (SPAN * SPAN) as usize * 15;
/// The callers that work at a coarse scale (`hearth_core::prof::caller`) and must never make
/// tiles of the finest level (E4.1 §4.1): the globe's map and its hovering, the places' search
/// across the planet, the animals' habitats, the distant LOD tiles and the far field.
pub const COARSE_CALLERS: [&str; 6] = [
    "globe.map",
    "globe.hover",
    "places.search",
    "fauna.habitat",
    "lod.far",
    "farfield",
];
/// The zones a tile's making is timed in, by level.
const BUILD_ZONES: [&str; 4] = [
    "relief.build.L1",
    "relief.build.L2",
    "relief.build.L3",
    "relief.build.L4+",
];
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
/// The four neighbours along the cells' rows and columns.
const D4: [(i64, i64); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
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

impl Tile {
    /// The memory it takes (bytes).
    pub fn bytes(&self) -> u64 {
        (self.h.len() * 4
            + self.lake.len() * 4
            + self.q.len() * 4
            + self.recv.len()
            + self.channel.len()
            + self.sea.len()) as u64
    }
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

/// A rectangle of one level's cells, read once for the many columns sampled over it.
#[derive(Debug, Clone)]
pub struct Patch {
    /// The level (0 is the grid's).
    pub level: usize,
    cell: f64,
    half_c: f64,
    c: f64,
    /// The level's cells around the planet.
    count: i64,
    i0: i64,
    j0: i64,
    w: i64,
    h: i64,
    cells: Vec<Cell>,
}

/// A river's reach at a level, between two cells' nodes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reach {
    /// World (x, z) of its ends, upstream first.
    pub a: (f64, f64),
    pub b: (f64, f64),
    /// Its water's level at either end (blocks).
    pub level_a: f32,
    pub level_b: f32,
    /// Discharge (m³/s) at either end.
    pub q_a: f32,
    pub q_b: f32,
    /// The grid cell it starts in.
    pub grid: u32,
}

/// `d` (blocks along x) taken the short way round a planet of circumference `c`.
fn wrap_delta(d: f64, c: f64) -> f64 {
    let d = d.rem_euclid(c);
    if d > c * 0.5 { d - c } else { d }
}

impl Patch {
    /// The memory its cells take (bytes).
    pub fn bytes(&self) -> u64 {
        (self.cells.len() * std::mem::size_of::<Cell>()) as u64
    }

    /// The level's cell size (blocks).
    /// The level whose cells these are.
    pub fn level(&self) -> usize {
        self.level
    }

    pub fn cell_size(&self) -> f64 {
        self.cell
    }

    /// The cell at a level's indices, if the patch holds it (wrapping in x).
    fn get(&self, i: i64, j: i64) -> Option<&Cell> {
        let u = (i - self.i0).rem_euclid(self.count);
        let v = j - self.j0;
        (u < self.w && (0..self.h).contains(&v)).then(|| &self.cells[(v * self.w + u) as usize])
    }

    /// The cell at a level's indices, or the patch's nearest edge cell.
    fn at(&self, i: i64, j: i64) -> &Cell {
        let mut u = (i - self.i0).rem_euclid(self.count);
        if u >= self.w {
            u = if u > self.count / 2 { 0 } else { self.w - 1 };
        }
        let v = (j - self.j0).clamp(0, self.h - 1);
        &self.cells[(v * self.w + u) as usize]
    }

    /// Cell-centre coordinates of a world point.
    fn coords(&self, x: f64, z: f64) -> (f64, f64) {
        (x / self.cell - 0.5, (z + self.half_c) / self.cell - 0.5)
    }

    /// The surface (blocks) at a point, interpolated bicubically between the cells, never
    /// leaving the range of the cells about it ([`monotone`]).
    pub fn height(&self, x: f64, z: f64) -> f32 {
        let (gx, gz) = self.coords(x.rem_euclid(self.c), z);
        let (i0, j0) = (gx.floor() as i64, gz.floor() as i64);
        let (fx, fz) = (gx - i0 as f64, gz - j0 as f64);
        let mut rows = [0.0f64; 4];
        for (r, row) in rows.iter_mut().enumerate() {
            let j = j0 - 1 + r as i64;
            let p = [
                self.at(i0 - 1, j).h as f64,
                self.at(i0, j).h as f64,
                self.at(i0 + 1, j).h as f64,
                self.at(i0 + 2, j).h as f64,
            ];
            *row = monotone(p, fx);
        }
        monotone(rows, fz) as f32
    }

    /// The water about a point: the sea's share and the lakes' share of the four cells about
    /// it (weighted by nearness), and the highest of their lake surfaces (blocks).
    pub fn water(&self, x: f64, z: f64) -> (f32, f32, Option<f32>) {
        let (gx, gz) = self.coords(x.rem_euclid(self.c), z);
        let (i0, j0) = (gx.floor() as i64, gz.floor() as i64);
        let (fx, fz) = ((gx - i0 as f64) as f32, (gz - j0 as f64) as f32);
        let (mut sea, mut lakes) = (0.0f32, 0.0f32);
        let mut level: Option<f32> = None;
        for (dj, wz) in [(0, 1.0 - fz), (1, fz)] {
            for (di, wx) in [(0, 1.0 - fx), (1, fx)] {
                let c = self.at(i0 + di, j0 + dj);
                if c.sea {
                    sea += wx * wz;
                }
                if c.lake.is_finite() {
                    lakes += wx * wz;
                    level = Some(level.map_or(c.lake, |l| l.max(c.lake)));
                }
            }
        }
        (sea, lakes, level)
    }

    /// The channels' share of the four cells about a point (weighted by nearness).
    pub fn channel(&self, x: f64, z: f64) -> f32 {
        let (gx, gz) = self.coords(x.rem_euclid(self.c), z);
        let (i0, j0) = (gx.floor() as i64, gz.floor() as i64);
        let (fx, fz) = ((gx - i0 as f64) as f32, (gz - j0 as f64) as f32);
        let mut share = 0.0;
        for (dj, wz) in [(0, 1.0 - fz), (1, fz)] {
            for (di, wx) in [(0, 1.0 - fx), (1, fx)] {
                if self.at(i0 + di, j0 + dj).channel {
                    share += wx * wz;
                }
            }
        }
        share
    }
}

/// Noise for one level: an octave per wavelength, longest first, halving.
#[derive(Debug, Clone)]
struct LevelNoise {
    octaves: Vec<BlockFbm>,
    /// Each octave's dissected form (`gullies`): its seed, its lattice's cells round the planet
    /// and their size (blocks).
    dissect: Vec<(u64, i64, f64)>,
    /// The longest octave's wavelength (blocks).
    longest: f64,
    warp: BlockFbm,
}

/// How far below the parent's surface a river's bed lies at a level, as a share of the level's
/// reach: rivers are the low ground of the relief the level adds, so it rises from them.
const VALLEY: f64 = 0.6;
/// Cells between the points at which a tile's relief is reckoned (`build`).
const LATTICE: i64 = 8;
/// How deep a hollow may be cut through, as a share of the relief the level adds there.
const BREACH_REACH: f64 = 1.5;
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
    /// What the grid says of its land's relief (empty without levels).
    land: roughness::Land,
    /// Each level's tiles, made once: a tile asked for by several threads at once is made by
    /// the first and waited for by the others (a tile needs only coarser levels' tiles, so none
    /// waits on itself).
    tiles: Vec<Cache<TileKey, OnceLock<Tile>>>,
    /// The walking level's second pass (`Relief::drained_tile`): its blended surface drained
    /// over tiles offset half a tile from the first pass's.
    drained: Cache<TileKey, OnceLock<Vec<f32>>>,
    /// The tiles kept on disk too, once given a folder ([`Relief::keep_on_disk`]).
    store: std::sync::OnceLock<crate::relief_store::TileStore>,
}

impl std::fmt::Debug for Relief {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Relief")
            .field("levels", &self.levels)
            .finish()
    }
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
        // Only a grid of kilometre cells is refined: a small test planet's is fine enough.
        while grid.geom.cell >= MIN_GRID_CELL && cell / RATIO as f64 >= MIN_CELL {
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
                // level's own; the finest level's to four times, as the blocks interpolate it
                // and add their own below (relief at its cells' own size would show their grid).
                // The walking level's from twice its parent's cell, its parent keeping the finest
                // level's rule: the relief a hill's view must show, which a far read stands in
                // for (`walking_proxy`), is the walking level's, not its parent's cells' noise.
                let mut octaves = Vec::new();
                let mut dissect = Vec::new();
                let walking =
                    |l: usize| levels.get(l).is_some_and(|v: &Level| v.cell < WALKING_CELL);
                let longest = lv.cell * RATIO as f64 * if l == 0 || walking(l) { 2.0 } else { 1.0 };
                let mut wl = longest;
                let shortest = if l + 1 == levels.len() || walking(l + 1) {
                    4.0
                } else {
                    2.0
                } * lv.cell;
                let mut k = 0u64;
                while wl >= shortest - 1e-6 {
                    let s = hash2(seed, (l as u64) << 8 | k);
                    octaves.push(BlockFbm::new(s, c as i64, wl, 1, 0.5));
                    // The dissected octave's lattice, a whole number of cells round the planet.
                    let cells = ((c / wl).round() as i64).max(1);
                    dissect.push((hash2(s, 0xd155), cells, c / cells as f64));
                    wl *= 0.5;
                    k += 1;
                }
                LevelNoise {
                    octaves,
                    dissect,
                    longest,
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
        // Each level its share of the tiles' memory budget.
        let kept = (hearth_core::memory::budget(hearth_core::memory::Kind::TerrainTiles) as usize
            / levels.len().max(1)
            / TILE_BYTES)
            .max(TILES_KEPT_MIN);
        let tiles = levels.iter().map(|_| Cache::new(kept)).collect();
        let drained = Cache::new(kept);
        let land = if levels.is_empty() {
            roughness::Land::none()
        } else {
            roughness::Land::new(&grid)
        };
        Self {
            v: grid.vertical_scale,
            c,
            seed,
            levels,
            noise,
            land,
            tiles,
            drained,
            grid,
            store: std::sync::OnceLock::new(),
        }
    }

    /// Keeps the tiles made in `dir` too (at most `cap` bytes), read back rather than made again
    /// the next time they are wanted: when the world opens again, or the player comes back.
    pub fn keep_on_disk(&self, dir: &std::path::Path, cap: u64) {
        match crate::relief_store::TileStore::open(dir, cap) {
            Ok(store) => {
                log::info!(
                    "terrain tiles kept in {} ({} MiB there)",
                    dir.display(),
                    store.used() >> 20
                );
                let _ = self.store.set(store);
            }
            Err(e) => log::warn!("terrain tiles not kept on disk ({e})"),
        }
    }

    /// The refinement levels, coarsest first (none on a small test planet).
    pub fn levels(&self) -> &[Level] {
        &self.levels
    }

    /// Tiles made so far at each level, coarsest first.
    pub fn tiles_made(&self) -> Vec<u64> {
        self.tiles.iter().map(|t| t.stats().1).collect()
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

    /// A cell's value at a level ≥ 1, or the grid's at level 0: its tiles' blend, at the walking
    /// level drained again where the blend dams it (`Relief::drained_tile`).
    pub fn cell(&self, level: usize, i: i64, j: i64) -> Cell {
        self.cell_in(level, i, j, &mut Runs::default())
    }

    /// [`Relief::cell`] in a run of reads close together, which asks the cache for each of
    /// their tiles once.
    fn cell_in(&self, level: usize, i: i64, j: i64, runs: &mut Runs) -> Cell {
        let c = self.blended_in(level, i, j, &mut runs.tiles);
        if level == 0 || !self.walking(level) {
            return c;
        }
        Cell {
            h: self.drained_h_in(level, i, j, &mut runs.drained),
            ..c
        }
    }

    /// A cell's blended value at a level ≥ 1, or the grid's at level 0, its tiles taken from
    /// `run` (of reads at this level).
    fn blended_in(&self, level: usize, i: i64, j: i64, run: &mut Run<Tile>) -> Cell {
        if level == 0 {
            return self.grid_cell(i, j);
        }
        let count = self.count(level);
        let i = i.rem_euclid(count);
        let j = j.clamp(0, count - 1);
        let tx = i.div_euclid(TILE);
        let tz = j.div_euclid(TILE);
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
        let own = run.at(tx, tz, || self.tile(level, tx, tz));
        let mut sum = own.h[k] as f64;
        let mut water = depth(own, k);
        let (q, r, sea, channel) = (own.q[k], own.recv[k], own.sea[k], own.channel[k]);
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
                let t = run.at(ntx, ntz, || self.tile(level, ntx, ntz));
                let nu = ci + MARGIN - dx * TILE;
                let nw = cj + MARGIN - dz * TILE;
                let nk = (nw * SPAN + nu) as usize;
                sum += wgt * t.h[nk] as f64;
                water += wgt * depth(t, nk);
                weight += wgt;
            }
        }
        let (h, water) = (sum / weight, water / weight);
        Cell {
            h: h as f32,
            lake: if water > 0.1 * self.v {
                (h + water) as f32
            } else {
                f32::NAN
            },
            q,
            receiver: (r != OUTLET).then(|| {
                let (di, dj) = D8[r as usize];
                ((i + di).rem_euclid(count), j + dj)
            }),
            sea,
            channel,
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

    /// A tile, from the cache, from disk ([`Relief::keep_on_disk`]: `relief.disk.L<level>`) or
    /// made: each made counted by its level and the caller it was made for
    /// (`relief.tile.L<level>.<caller>`, E4.1 §3), and timed. A tile of the finest level wanted
    /// by a caller that works at a coarse scale ([`COARSE_CALLERS`]) is counted again as such
    /// (`relief.fine.<caller>`) and said once in the log: the guard of E4.1 §4.1.
    fn tile(&self, level: usize, tx: i64, tz: i64) -> Arc<OnceLock<Tile>> {
        let key = TileKey {
            level: level as u8,
            tx,
            tz,
        };
        let tile = self.tiles[level - 1].get_or_insert_with(key, OnceLock::new);
        tile.get_or_init(|| {
            let caller = hearth_core::prof::current_caller();
            if level == self.levels.len() && COARSE_CALLERS.contains(&caller) {
                let name = format!("relief.fine.{caller}");
                if hearth_core::prof::counter(&name) == 0 {
                    log::warn!("{caller} built a tile of the finest refinement level");
                }
                hearth_core::prof::count(&name, 1);
            }
            let store = self.store.get();
            if let Some(s) = store {
                let _zone = hearth_core::prof::Zone::new("relief.disk");
                if let Some(t) = s.load(level, tx, tz, SPAN as usize) {
                    hearth_core::prof::count(&format!("relief.disk.L{level}"), 1);
                    return t;
                }
            }
            let _zone = hearth_core::prof::Zone::new(BUILD_ZONES[(level - 1).min(3)]);
            hearth_core::prof::count(&format!("relief.tile.L{level}.{caller}"), 1);
            let t = self.build(level, tx, tz);
            if let Some(s) = store {
                s.store(level, tx, tz, &t);
            }
            t
        });
        tile
    }

    /// Tiles kept now at each level, coarsest first.
    pub fn tiles_kept(&self) -> Vec<usize> {
        self.tiles.iter().map(Cache::len).collect()
    }

    /// The memory the tiles kept take at each level, coarsest first (bytes).
    pub fn tiles_bytes(&self) -> Vec<u64> {
        self.tiles
            .iter()
            .map(|c| c.sum(|t| t.get().map_or(0, Tile::bytes)))
            .collect()
    }

    /// The surface at a level (0: the grid's), interpolated bicubically between its cells, as
    /// [`Patch::height`] reads it.
    pub fn height(&self, level: usize, x: f64, z: f64) -> f32 {
        let s = self.cell_size(level);
        let gx = x / s - 0.5;
        let gz = (z + self.c * 0.5) / s - 0.5;
        let (i0, j0) = (gx.floor() as i64, gz.floor() as i64);
        let (fx, fz) = (gx - i0 as f64, gz - j0 as f64);
        let mut runs = Runs::default();
        let mut rows = [0.0f64; 4];
        for (r, row) in rows.iter_mut().enumerate() {
            let j = j0 - 1 + r as i64;
            let p = [
                self.cell_in(level, i0 - 1, j, &mut runs).h as f64,
                self.cell_in(level, i0, j, &mut runs).h as f64,
                self.cell_in(level, i0 + 1, j, &mut runs).h as f64,
                self.cell_in(level, i0 + 2, j, &mut runs).h as f64,
            ];
            *row = monotone(p, fx);
        }
        monotone(rows, fz) as f32
    }

    /// The finest surface (blocks) at a point, or the grid's where there are no levels.
    pub fn finest_height(&self, x: f64, z: f64) -> f32 {
        self.height(self.levels.len(), x, z)
    }

    /// The level for a query standing for `footprint` blocks of ground (E4.1 §4.1): the
    /// coarsest whose cells are no wider than about the footprint (a quarter more allowed, so
    /// the ~300 m level answers for a 256 m ecology cell): the grid's for the globe and the
    /// climate's summaries, the ~2.5 km and ~300 m levels for regional searches and ecology,
    /// the finest for footprints of some tens of metres and less.
    pub fn level_at(&self, footprint: f64) -> usize {
        let fits = |cell: f64| cell <= 1.25 * footprint;
        if fits(self.grid.geom.cell) {
            return 0;
        }
        (1..=self.levels.len())
            .find(|&l| fits(self.levels[l - 1].cell))
            .unwrap_or(self.levels.len())
    }

    /// The level to read for columns `scale` blocks apart: the coarsest whose cells are at
    /// most four columns wide (finer relief would not show between them), the finest but the
    /// walking level for the closest, the grid's for the widest. A far tile so reads coarse cells
    /// and makes few tiles.
    pub fn level_for(&self, scale: f64) -> usize {
        let span = 4.0 * scale;
        if span >= self.grid.geom.cell {
            return 0;
        }
        // Never the walking scale's level: its tiles are made for the blocks about the players
        // alone (`Terrain::nearby`), and a distant tile, however fine, reads its stand-in.
        let deepest = if self.walking(self.levels.len()) {
            self.levels.len() - 1
        } else {
            self.levels.len()
        };
        (1..=deepest)
            .find(|&l| self.levels[l - 1].cell <= span)
            .unwrap_or(deepest)
    }

    /// The cells of `level` over a rectangle of world (x, z), `margin` cells more on every side
    /// (and enough for a bicubic surface over all of it).
    pub fn patch(
        &self,
        level: usize,
        (x0, z0): (f64, f64),
        (x1, z1): (f64, f64),
        margin: i64,
    ) -> Patch {
        let s = self.cell_size(level);
        let half = self.c * 0.5;
        let i0 = (x0 / s - 0.5).floor() as i64 - 1 - margin;
        let i1 = (x1 / s - 0.5).floor() as i64 + 2 + margin;
        let j0 = ((z0 + half) / s - 0.5).floor() as i64 - 1 - margin;
        let j1 = ((z1 + half) / s - 0.5).floor() as i64 + 2 + margin;
        let mut cells = Vec::with_capacity(((i1 - i0 + 1) * (j1 - j0 + 1)) as usize);
        let mut runs = Runs::default();
        for j in j0..=j1 {
            for i in i0..=i1 {
                cells.push(self.cell_in(level, i, j, &mut runs));
            }
        }
        Patch {
            level,
            cell: s,
            half_c: half,
            c: self.c,
            count: self.count(level),
            i0,
            j0,
            w: i1 - i0 + 1,
            h: j1 - j0 + 1,
            cells,
        }
    }

    /// The rivers of a patch: a reach from each cell carrying at least `min_q` (m³/s), not the
    /// sea's or a lake's, to the cell it drains to, between their nodes. Positions are unwrapped
    /// about the patch.
    pub fn reaches(&self, patch: &Patch, min_q: f32) -> Vec<Reach> {
        let level = patch.level;
        let centre = (patch.i0 as f64 + patch.w as f64 * 0.5) * patch.cell;
        let unwrap = |x: f64| centre + wrap_delta(x - centre, self.c);
        let mut out = Vec::new();
        for v in 0..patch.h {
            for u in 0..patch.w {
                let c = &patch.cells[(v * patch.w + u) as usize];
                if c.q < min_q || c.sea || c.lake.is_finite() {
                    continue;
                }
                let Some((ri, rj)) = c.receiver else {
                    continue;
                };
                let (i, j) = (patch.i0 + u, patch.j0 + v);
                let r = patch
                    .get(ri, rj)
                    .copied()
                    .unwrap_or_else(|| self.cell(level, ri, rj));
                let a = self.node(level, i, j);
                let b = self.node(level, ri, rj);
                let to = if r.sea {
                    0.0
                } else if r.lake.is_finite() {
                    r.lake
                } else {
                    r.h
                };
                out.push(Reach {
                    a: (unwrap(a.0), a.1),
                    b: (unwrap(b.0), b.1),
                    level_a: c.h,
                    level_b: to.min(c.h),
                    q_a: c.q,
                    q_b: r.q.max(c.q),
                    grid: self.grid.cell_at(a.0.rem_euclid(self.c), a.1) as u32,
                });
            }
        }
        out
    }

    /// A place's height above its base level (m, at grid coordinates; [`roughness`]).
    pub fn above_base(&self, gx: f64, gz: f64) -> f32 {
        self.land.above.bilinear(gx, gz)
    }

    /// The relief a place takes at the levels (at grid coordinates): by its kind of land and its
    /// height above base level ([`roughness`]).
    pub fn roughness_at(&self, gx: f64, gz: f64) -> Roughness {
        roughness::roughness(&self.grid, &self.land, gx, gz)
    }

    /// The amplitudes (blocks) of a level's octaves at a place, longest first, into `out`; their
    /// sum, how far the level's relief reaches there.
    /// Below [`SMOOTH_M`] soil-mantled ground's relief falls away a power faster, as creep
    /// rounds it (the spectra's roll-over below the hillslopes' scale: Perron et al. 2008), as
    /// much as the ground is not `bare` (0 … 1: rock, young glacial ground, dry land, [`bare`]).
    fn amplitudes(&self, level: usize, r: &Roughness, bare: f64, out: &mut [f64]) -> f64 {
        let noise = &self.noise[level - 1];
        let mut wl = noise.longest / self.v;
        let mut sum = 0.0;
        let smooth = SMOOTH_M * (1.0 - bare);
        for a in out.iter_mut().take(noise.octaves.len()) {
            *a = r.at(wl) * self.v * if wl < smooth { wl / smooth } else { 1.0 };
            sum += *a;
            wl *= 0.5;
        }
        sum
    }

    /// The relief a level's noise adds at a point (blocks), before its drainage: its octaves as
    /// columns `min_wl` blocks apart see them (0: all) at amplitudes `amps` (longest first),
    /// each laid across the slope beneath it — the parent's surface's (`slope`, blocks a block)
    /// with the longer octaves' on it — where that slope falls further over its wavelength than
    /// its relief reaches (`gullies`), isotropic on gentler ground.
    fn octaves_at(
        &self,
        level: usize,
        x: f64,
        z: f64,
        amps: &[f64],
        slope: (f64, f64),
        min_wl: f64,
    ) -> f64 {
        let noise = &self.noise[level - 1];
        let reach = self.cell_size(level) * RATIO as f64 * 0.5;
        // Both of the warp's parts periodic round the planet (the second taken far off along
        // the other axis).
        let wx = x + noise.warp.sample2(x, z) * reach;
        let wz = z + noise.warp.sample2(x, z + WARP_APART) * reach;
        let mut add = 0.0;
        let mut grad = slope;
        for ((oct, amp), &(seed, cells, wl)) in noise.octaves.iter().zip(amps).zip(&noise.dissect) {
            // How much of the octave columns `min_wl` apart see: all of it from two spacings up,
            // none below one (a column stands for its square's mean, as a 30 m DEM's sample
            // does: finer relief averages away rather than showing as noise).
            let seen = if min_wl > 0.0 {
                smoothstep(1.0, 2.0, wl / min_wl)
            } else {
                1.0
            };
            if seen <= 0.0 {
                break;
            }
            let iso = oct.sample2(wx, wz);
            let fall = grad.0.hypot(grad.1);
            let led = smoothstep(0.5, 2.0, fall * wl / amp.max(1e-6));
            if led <= 0.0 {
                add += iso * amp * seen;
                continue;
            }
            let along = (grad.0 / fall, grad.1 / fall);
            let (g, (gu, gv)) = gullies::octave(seed, cells, (wx / wl, wz / wl), along);
            add += (led * g * gullies::GAIN + (1.0 - led) * iso) * amp * seen;
            let k = led * gullies::GAIN * amp * seen / wl;
            grad = (grad.0 + k * gu, grad.1 + k * gv);
        }
        add
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
        let mut runs = Runs::default();
        for pj in pj0..pj0 + pw {
            for pi in pi0..pi0 + pw {
                parent.push(self.cell_in(level - 1, pi, pj, &mut runs));
            }
        }
        let pcell = |pi: i64, pj: i64| -> &Cell {
            let u = (pi - pi0).clamp(0, pw - 1);
            let w = (pj - pj0).clamp(0, pw - 1);
            &parent[(w * pw + u) as usize]
        };
        // The parent's ground about each of its cells (the 25 within two of it): its mean,
        // lowest and highest.
        let ground: Vec<(f64, f64, f64)> = (0..pw * pw)
            .map(|q| {
                let (pi, pj) = (pi0 + q % pw, pj0 + q / pw);
                let (mut mean, mut lo, mut hi) = (0.0, f64::MAX, f64::MIN);
                for dj in -2..=2 {
                    for di in -2..=2 {
                        let v = pcell(pi + di, pj + dj).h as f64;
                        mean += v / 25.0;
                        lo = lo.min(v);
                        hi = hi.max(v);
                    }
                }
                (mean, lo, hi)
            })
            .collect();
        let ground_about = |pi: i64, pj: i64| {
            let u = (pi - pi0).clamp(0, pw - 1);
            let w = (pj - pj0).clamp(0, pw - 1);
            ground[(w * pw + u) as usize]
        };
        // ------------------------------------------------ the surface with this level's relief
        let g = &*self.grid;
        let mut h = vec![0.0f32; len];
        let mut rain = vec![0.0f32; len];
        let mut storm = vec![0.0f32; len];
        let mut erodibility = vec![1.0f32; len];
        let mut rock = vec![0.0f32; len];
        let mut sea = vec![false; len];
        let mut outside = vec![false; len];
        // The parent's lake surface about each cell, NaN where it has none, and the lakes' share
        // of the four parent cells about it.
        let mut parent_lake = vec![f32::NAN; len];
        let mut lake_share = vec![0.0f32; len];
        let mut amps = [0.0f64; 16];
        // The relief the place takes, on a lattice every LATTICE cells (at the same cells for every
        // tile: the spans start on it), between its points interpolated; it changes only over
        // the grid's cells.
        let side = (n as i64 - 1) / LATTICE + 2;
        let lattice: Vec<([f64; 16], f64, f64, f64, f64)> = (0..side * side)
            .map(|q| {
                let (lu, lw) = (q % side, q / side);
                let (x, z) = self.centre(
                    level,
                    i0 + lu * LATTICE,
                    (j0 + lw * LATTICE).clamp(0, lv.count - 1),
                );
                let (gx, gz) = g.geom.grid_coords(x.rem_euclid(self.c), z);
                let rough = self.roughness_at(gx, gz);
                let p = g.precipitation.bilinear(gx, gz) as f64;
                let t = g.temperature.bilinear(gx, gz) as f64;
                let mut a = [0.0; 16];
                self.amplitudes(level, &rough, bare(p, t), &mut a);
                // Runoff (mm/yr): what the rain leaves after evaporation, more of it in cool
                // wet climates.
                let share = (0.12 + 0.45 * smoothstep(300.0, 2500.0, p)
                    - 0.15 * smoothstep(10.0, 28.0, t))
                .clamp(0.03, 0.75);
                // A cell's real area: Mercator's blocks are cos φ metres apart.
                let lat = g.geom.planet().latitude(z);
                let area_m2 = (s * lat.cos() / self.v).powi(2);
                // The walking level's washes and gullies are cut by the storms that run off a
                // dry land's bare ground, not by the year's mean (Wolman and Gerson 1978; dry
                // lands drain densest, Melton 1957): its erosion reckons with their runoff.
                let storm = if self.walking(level) {
                    share.max(STORM_SHARE * (1.0 - smoothstep(150.0, 600.0, p)))
                } else {
                    share
                };
                // Bare rock where it is cold above the trees (frost-shattered, ice-scoured): at
                // the walking level it stands in cliffs and steps, steeper than scree.
                let alpine = if level == self.levels.len() && self.levels.len() >= 4 {
                    smoothstep(0.0, -6.0, t)
                } else {
                    0.0
                };
                let per = area_m2 * p / 1000.0 / YEAR_S;
                (a, rough.fill, per * share, alpine, per * storm)
            })
            .collect();
        // How deep a hollow may be and still be cut through: deeper where the relief is rougher.
        let mut breach = vec![breach_depth(level) * self.v as f32; len];
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
                let mut rows_dx = [0.0f64; 4];
                for (r, (row, row_dx)) in rows.iter_mut().zip(&mut rows_dx).enumerate() {
                    let pj = cj - 1 + r as i64;
                    let p = [
                        pcell(ci - 1, pj).h as f64,
                        pcell(ci, pj).h as f64,
                        pcell(ci + 1, pj).h as f64,
                        pcell(ci + 2, pj).h as f64,
                    ];
                    *row = catmull(p, fx);
                    *row_dx = catmull_dt(p, fx);
                }
                let base = catmull(rows, fz);
                // The parent's slope here (blocks a block).
                let slope_base = (catmull(rows_dx, fz) / ps, catmull_dt(rows, fz) / ps);
                // The parent's ground about the place, its mean and its range: the four cells'
                // about it, bilinear (no step where the place passes from one cell to the next).
                let (mut mean, mut lo, mut hi) = (0.0, 0.0, 0.0);
                for (dj, wz) in [(0, 1.0 - fz), (1, fz)] {
                    for (di, wx) in [(0, 1.0 - fx), (1, fx)] {
                        let (m, l, h) = ground_about(ci + di, cj + dj);
                        mean += m * wx * wz;
                        lo += l * wx * wz;
                        hi += h * wx * wz;
                    }
                }
                // The relief the place takes, from the lattice about it.
                let (lu, lw) = (u as i64 / LATTICE, w as i64 / LATTICE);
                let (fu, fw) = (
                    (u as i64 % LATTICE) as f64 / LATTICE as f64,
                    (w as i64 % LATTICE) as f64 / LATTICE as f64,
                );
                let corner = |du: i64, dw: i64| &lattice[((lw + dw) * side + lu + du) as usize];
                let weights = [
                    (corner(0, 0), (1.0 - fu) * (1.0 - fw)),
                    (corner(1, 0), fu * (1.0 - fw)),
                    (corner(0, 1), (1.0 - fu) * fw),
                    (corner(1, 1), fu * fw),
                ];
                let (mut fill, mut runoff, mut alpine, mut flood) = (0.0, 0.0, 0.0, 0.0);
                amps.fill(0.0);
                for (c, wgt) in weights {
                    for (a, ca) in amps.iter_mut().zip(&c.0) {
                        *a += ca * wgt;
                    }
                    fill += c.1 * wgt;
                    runoff += c.2 * wgt;
                    alpine += c.3 * wgt;
                    flood += c.4 * wgt;
                }
                storm[k] = flood as f32;
                rock[k] = alpine as f32;
                let reach: f64 = amps.iter().sum();
                breach[k] = breach[k].max((BREACH_REACH * reach) as f32);
                // Both of the warp's parts periodic round the planet (the second taken far off
                // along the other axis).
                let mut add = self.octaves_at(level, x, z, &amps, slope_base, 0.0);
                // Dry land's basins, the ground lying low among the parent's, are filled with
                // what the ranges about them shed: the finer levels' relief laid only thinly
                // there, their floors and fans buried. The first level's (its cells some
                // kilometres) is the ranges' within a basin, which stand above the fill; the
                // walking level's is the fill's own surface, its washes, bars and swales (Bull
                // 1977), as rough on a fan as off it.
                if level > 1 && !self.walking(level) && fill > 0.0 && hi > lo {
                    let low = smoothstep(0.25, -0.2, (base - mean) / (hi - lo));
                    add *= 1.0 - fill * low;
                }
                let mut hk = base + add;
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
                    let rough = 0.3 * reach + 0.5 * self.v;
                    let shore = surface as f64 + (0.5 - share) * 2.0 * rough + add;
                    let t = 1.0 - (2.0 * share - 1.0).abs();
                    hk += (shore - hk) * t;
                    lake_share[k] = share as f32;
                    parent_lake[k] = surface;
                }
                h[k] = hk as f32;
                rain[k] = runoff as f32;
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
        let mut courses = courses::Courses::new(self, level);
        // The lowest a channel's bed may be cut: where the reach it drains along ends.
        let mut bottom = vec![f32::NEG_INFINITY; len];
        for pj in pj0..pj0 + pw {
            for pi in pi0..pi0 + pw {
                let pc = *pcell(pi, pj);
                let Some((ri, rj)) = pc.receiver else {
                    continue;
                };
                let rc = courses.cell(ri, rj);
                if pc.q < min_q {
                    continue;
                }
                if pc.lake.is_finite() && rc.lake.is_finite() {
                    // Water crossing a lake is the lake's, not a river.
                    continue;
                }
                let a = self.node(level - 1, pi, pj);
                let b = self.node(level - 1, ri, rj);
                // The main stream above and the reach below, which the course bends towards.
                let above = courses
                    .upstream(pi, pj, min_q)
                    .map(|(ui, uj)| self.node(level - 1, ui, uj));
                let below = rc.receiver.map(|(ci, cj)| self.node(level - 1, ci, cj));
                // The beds at either end ([`courses::Courses::bed`]), falling along the reach.
                let land = !pc.sea;
                let hb = courses.bed(ri, rj, land);
                let ha = courses.bed(pi, pj, land).max(hb);
                let length = (wrap_delta(b.0 - a.0, self.c)).hypot(b.1 - a.1).max(1e-6);
                let path = meander(
                    (above, a, b, below),
                    (ps, s),
                    hash_2d(
                        hash2(self.seed, 0x5ee + level as u64),
                        pi.rem_euclid(self.count(level - 1)) as i32,
                        pj as i32,
                    ),
                    self.c,
                    wander((ha - hb) as f64 / length, pc.q),
                );
                self.rasterize(
                    &path,
                    (ha, hb),
                    pc.q,
                    (i0, j0, count),
                    (s, self.walking(level)),
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
        // Where a floor held a bed up, the beds above it rise to it: each lies at or above the
        // next down its course.
        for _ in 0..4 * SPAN {
            let mut raised = false;
            for k in 0..len {
                if !channel[k] {
                    continue;
                }
                if let Some(r) = target(k, recv[k], n)
                    && channel[r]
                    && h[k] < h[r]
                {
                    h[k] = h[r];
                    raised = true;
                }
            }
            if !raised {
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
        let talus = (0.84 * self.v * s) as f32;
        // A cell's runoff at 300 mm a year: the discharge the wearing is reckoned against.
        let q_cell = ((s / self.v).powi(2) * 0.3 / YEAR_S) as f32;
        let k_power: Vec<f32> = erodibility.iter().map(|e| K_POWER * e).collect();
        let creep = creep_rate(level);
        let mut z = h;
        let mut wet = z.clone();
        let mut routing = Routing::new(len);
        // The walking level drains along its rows and columns, as the blocks read between its
        // cells: a cell draining only across a corner, between two higher, would hold a hollow.
        let ways: &[(i64, i64)] = if self.walking(level) { &D4 } else { &D8 };
        for it in 0..8 {
            // The parent's lakes stand at their surface, which the water about them drains to.
            for k in 0..len {
                wet[k] = if held[k].is_finite() { held[k] } else { z[k] };
            }
            fill(&mut wet, &base, n, 1e-3, &breach, ways);
            // A cut is the ground's.
            for (zk, &wk) in z.iter_mut().zip(&wet) {
                *zk = zk.min(wk);
            }
            if it == 7 {
                break;
            }
            // The water spread over the slopes and gathered in the hollows, the ground worn
            // where it gathers, the slopes crept and slumped.
            routing.route(&wet, &base, &channel, &recv, n);
            let q = routing.accumulate(&storm, &inflow, &channel, n);
            routing.erode(&mut z, &mut wet, &q, &k_power, &base, q_cell, n);
            diffuse(&mut z, &base, n, creep);
            slump(&mut z, &base, n, talus, &rock);
        }
        // The parent's lakes keep an outlet: where this level's relief rings one with ground above
        // its surface, the lowest way out is cut down through the ring to the lake's level, as its
        // outflow would cut it (rather than the lake standing below its own brim).
        if held.iter().any(|h| h.is_finite()) {
            let mut out = z.clone();
            let mut open_base = base.clone();
            for k in 0..len {
                if held[k].is_finite() {
                    out[k] = held[k];
                    open_base[k] = outside[k] || sea[k] || channel[k];
                }
            }
            fill(&mut out, &open_base, n, 1e-3, &breach, ways);
            for k in 0..len {
                if held[k].is_nan() && out[k] < z[k] {
                    z[k] = out[k];
                    wet[k] = wet[k].min(out[k]);
                }
            }
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
    /// lowest its bed may be cut. With `straight` (the walking level's, whose cells the blocks
    /// read between) it steps only along rows and columns: a channel stepping across a corner
    /// between two banks would rise between its cells there, damming itself.
    #[allow(clippy::too_many_arguments)]
    fn rasterize(
        &self,
        path: &[(f64, f64)],
        levels: (f32, f32),
        q: f32,
        (i0, j0, count): (i64, i64, i64),
        (s, straight): (f64, bool),
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
        // Where the path was last sampled (blocks).
        let mut last = (f64::NAN, f64::NAN);
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
                let from = std::mem::replace(&mut last, (x, z));
                if (u, w) == prev.unwrap_or((i64::MIN, i64::MIN)) {
                    continue;
                }
                let level = levels.0 + (levels.1 - levels.0) * at as f32;
                // The cells the step enters: across a corner, first the one of the two beside
                // it that the path crosses into first.
                let mut cells = [(u, w), (u, w)];
                let mut visits = 1;
                if straight
                    && let Some((pu, pw)) = prev
                    && (u - pu).abs() == 1
                    && (w - pw).abs() == 1
                {
                    let (dx, dz) = (x - from.0, z - from.1);
                    // How far along the step it crosses into the next column, and row.
                    let cross = |a: f64, d: f64, up: bool| {
                        let into = (a / s).rem_euclid(1.0);
                        let gap = if up { 1.0 - into } else { into };
                        if d == 0.0 {
                            f64::MAX
                        } else {
                            gap * s / d.abs()
                        }
                    };
                    let tx = cross(from.0, dx, u > pu);
                    let tz = cross(from.1 + self.c * 0.5, dz, w > pw);
                    cells[0] = if tx < tz { (u, pw) } else { (pu, w) };
                    visits = 2;
                }
                for &(u, w) in &cells[..visits] {
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
            }
            walked += seg;
        }
    }
}

/// Discharge (m³/s) from which a level's parent's rivers are kept as its channels: a river
/// draining some hundreds of km² from the grid, some tens from the first level, a few from
/// the second; at the walking level the brooks and first-order channels its parent drains (some
/// tenths of a km² in humid land), which every tile lays alike, so that its hillslopes drain to
/// the same channels whichever tile holds them.
fn channel_threshold(level: usize) -> f32 {
    match level {
        1 => 3.0,
        2 => 1.0,
        3 => 0.15,
        _ => 0.0005,
    }
}

/// The share of a dry land's rain its storms run off (Hortonian flow on bare and crusted ground,
/// a fifth to a half of a heavy storm's: Yair and Lavee 1985), for the walking level's erosion.
const STORM_SHARE: f64 = 0.3;

/// The stream power's strength (per round, at a level's cells; times the rock's erodibility).
const K_POWER: f32 = 0.08;

/// How far each round the slopes creep towards their neighbours' mean at a level (hillslope
/// diffusion, `drainage::diffuse`): at the ~300 m and ~40 m levels, where hillslopes are a few
/// cells long; none at the coarsest, whose cells are wider than a hillslope; most at the walking
/// level, whose cells are a hillslope's metres (soil-mantled slopes smooth, hilltops round).
fn creep_rate(level: usize) -> f32 {
    match level {
        1 => 0.0,
        2 => 0.05,
        3 => 0.12,
        _ => 0.25,
    }
}

/// How deep (m) a hollow must be to hold a lake at a level (at the walking level, only a pond
/// deep enough to stay one; shallower hollows fill with what washes in).
fn lake_depth(level: usize) -> f32 {
    match level {
        1 => 3.0,
        2 => 1.5,
        3 => 0.8,
        _ => 2.0,
    }
}

/// How far a river wanders from the straight line, as a share of each stretch's length (its
/// midpoint displaced across it by up to half this): by its slope (blocks a block) and its
/// discharge (m³/s). A river on a gentle floodplain meanders, one falling steeply runs straight
/// (Leopold and Wolman 1957; sinuosity falls with the valley's slope), and large rivers more
/// than small.
fn wander(slope: f64, q: f32) -> f64 {
    0.2 + 0.3 * (1.0 - smoothstep(0.001, 0.03, slope)) + 0.05 * smoothstep(1.0, 100.0, q as f64)
}

/// The path of a reach from node `a` to node `b` (parent cells of size `ps` apart; `cells` =
/// (ps, s)), with the main stream's node above `a` and the node below `b` where there are:
/// a curve through the nodes (Hermite, leaving `a` along the line from the node above to `b` and
/// reaching `b` along the line from `a` to the node below, each tangent the reach's length: the
/// main stream bends through its nodes instead of turning there), wandering about it by a
/// fractal of midpoints displaced across it by up to half of `wander` of their length, down to
/// the child's cell size `s`. The same reach always takes the same path (`hash`).
fn meander(
    (above, a, b, below): (
        Option<(f64, f64)>,
        (f64, f64),
        (f64, f64),
        Option<(f64, f64)>,
    ),
    (ps, s): (f64, f64),
    hash: u64,
    c: f64,
    wander: f64,
) -> Vec<(f64, f64)> {
    // Every node unwrapped near a.
    let near = |p: (f64, f64)| {
        let mut dx = p.0 - a.0;
        if dx > c * 0.5 {
            dx -= c;
        } else if dx < -c * 0.5 {
            dx += c;
        }
        (a.0 + dx, p.1)
    };
    let b = near(b);
    let chord = (b.0 - a.0, b.1 - a.1);
    let length = chord.0.hypot(chord.1).max(1e-9);
    let tangent = |from: (f64, f64), to: (f64, f64)| {
        let (dx, dz) = (to.0 - from.0, to.1 - from.1);
        let d = dx.hypot(dz);
        if d < 1e-9 {
            chord
        } else {
            (dx / d * length, dz / d * length)
        }
    };
    let ta = above.map_or(chord, |z| tangent(near(z), b));
    let tb = below.map_or(chord, |c| tangent(a, near(c)));
    let spine = |t: f64| {
        let (t2, t3) = (t * t, t * t * t);
        let (h00, h10, h01, h11) = (
            2.0 * t3 - 3.0 * t2 + 1.0,
            t3 - 2.0 * t2 + t,
            -2.0 * t3 + 3.0 * t2,
            t3 - t2,
        );
        (
            h00 * a.0 + h10 * ta.0 + h01 * b.0 + h11 * tb.0,
            h00 * a.1 + h10 * ta.1 + h01 * b.1 + h11 * tb.1,
        )
    };
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
            let off = r * wander * len;
            next.push(p);
            next.push((mx - lz / len * off, mz + lx / len * off));
        }
        next.push(*path.last().expect("two points"));
        path = next;
        seg *= 0.5;
        depth += 1;
    }
    // The wander laid on the curve instead of the chord.
    let last = (path.len() - 1) as f64;
    for (k, p) in path.iter_mut().enumerate() {
        let t = k as f64 / last;
        let (cx, cz) = spine(t);
        *p = (
            cx + p.0 - (a.0 + chord.0 * t),
            cz + p.1 - (a.1 + chord.1 * t),
        );
    }
    path
}

/// The cells (blocks) below which a level is the walking scale's: its relief is laid with its
/// drainage only about the players, and stood in for elsewhere (`Relief::walking_proxy`).
pub const WALKING_CELL: f64 = 10.0;

/// The wavelength (m) below which soil-mantled ground's relief falls away faster
/// (`Relief::amplitudes`): two of the ~40 m level's cells, about a hillslope's length.
const SMOOTH_M: f64 = 76.0;

/// How bare a place's ground is of the soil that creep smooths (0 … 1), by its rain (mm a
/// year) and mean temperature (°C): alpine rock (frost-shattered, ice-scoured), young glacial
/// ground (knobbly till and bedrock where the last ice sheets lay, some 7 °C and colder now) and
/// dry land (thin stony soils, sparse plants) keep their relief below the hillslopes' scale.
fn bare(p: f64, t: f64) -> f64 {
    let alpine = smoothstep(0.0, -6.0, t);
    let young = smoothstep(7.0, 3.0, t);
    let arid = 1.0 - smoothstep(250.0, 700.0, p);
    alpine.max(young).max(arid)
}

/// A tile from its cache, made by [`Relief::tile`] or [`Relief::drained_tile`] before either
/// returns it.
/// The tiles of one level a run of reads close together wants (a patch's cells, a tile's
/// parent cells, a second pass's span), each asked of the cache once: its lock and clock are
/// shared by every thread, and a read of one cell would otherwise ask for up to four tiles.
struct Run<T> {
    held: smallvec::SmallVec<[((i64, i64), Arc<OnceLock<T>>); 8]>,
}

impl<T> Default for Run<T> {
    fn default() -> Self {
        Self {
            held: smallvec::SmallVec::new(),
        }
    }
}

impl<T> Run<T> {
    /// The tile at (tx, tz), from `get` the first time the run wants it.
    fn at(&mut self, tx: i64, tz: i64, get: impl FnOnce() -> Arc<OnceLock<T>>) -> &T {
        let i = match self.held.iter().position(|(k, _)| *k == (tx, tz)) {
            Some(i) => i,
            None => {
                self.held.push(((tx, tz), get()));
                self.held.len() - 1
            }
        };
        made(&self.held[i].1)
    }
}

/// A run of reads' tiles at one level: the first pass's and, at the walking level, the
/// second's.
#[derive(Default)]
struct Runs {
    tiles: Run<Tile>,
    drained: Run<Vec<f32>>,
}

fn made<T>(tile: &OnceLock<T>) -> &T {
    tile.get().expect("a tile is made before it is handed out")
}

/// The derivative of [`catmull`] by `t`.
#[inline]
fn catmull_dt(p: [f64; 4], t: f64) -> f64 {
    0.5 * ((-p[0] + p[2])
        + 2.0 * (2.0 * p[0] - 5.0 * p[1] + 4.0 * p[2] - p[3]) * t
        + 3.0 * (-p[0] + 3.0 * p[1] - 3.0 * p[2] + p[3]) * t * t)
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

/// [`catmull`] with its slopes held in so that it never leaves the range of the two values it
/// lies between (Fritsch and Carlson 1980; Hyman 1983); the same where the values rise or fall
/// steadily. A drained surface's cells so read between them dam no hollow: plain Catmull-Rom
/// dips below a valley's floor two cells wide between steep sides, by an eighth of their rise.
#[inline]
fn monotone(p: [f64; 4], t: f64) -> f64 {
    let held = |a: f64, b: f64| {
        if a * b <= 0.0 {
            0.0
        } else {
            let m = 0.5 * (a + b);
            m.signum() * m.abs().min(3.0 * a.abs().min(b.abs()))
        }
    };
    let (m1, m2) = (
        held(p[1] - p[0], p[2] - p[1]),
        held(p[2] - p[1], p[3] - p[2]),
    );
    let t2 = t * t;
    let t3 = t2 * t;
    (2.0 * t3 - 3.0 * t2 + 1.0) * p[1]
        + (t3 - 2.0 * t2 + t) * m1
        + (3.0 * t2 - 2.0 * t3) * p[2]
        + (t3 - t2) * m2
}

#[cfg(test)]
mod tests;

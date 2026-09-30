//! Cube generation: turns the pure surface model into 16³ cubes of blocks, independently for
//! every cube, in any order, on any thread.
//!
//! Each cube is first classified cheaply:
//! * **Empty** — entirely above terrain, water and any feature: air, no noise evaluated.
//! * **Deep** — entirely below the surface band: the geology's rock columns, then any caves
//!   whose bounding volumes intersect it. No surface noise.
//! * **Surface** — everything else: full per-column evaluation.

pub mod blocks;
pub mod cache;
pub mod caves;
pub mod features;

use std::cell::RefCell;
use std::sync::Arc;

use hearth_math::{CUBE_SIZE, CUBE_VOLUME, ColumnPos, CubePos, Planet};
use hearth_world::{BlockRegistry, BlockStateId, Cube};

use crate::geology::{Geology, RockColumn};
use crate::region::{ColumnSample, Terrain};
use blocks::{GenBlocks, MissingBlock};
use cache::Cache;

/// Maximum height any feature (the tallest trees) reaches above the surface.
pub const MAX_FEATURE_HEIGHT: i32 = 48;
/// Maximum amplitude of 3D cliff shaping around the 2D surface.
pub const CLIFF_AMPLITUDE: i32 = 8;
/// Maximum soil/filler depth under the surface (dune seas are the thickest).
pub const MAX_SOIL_DEPTH: i32 = 12;

/// Per-16×16-column cached surface data.
#[derive(Debug, Clone)]
pub struct ColumnData {
    pub pos: ColumnPos,
    /// Samples in (z * 16 + x) order.
    pub samples: Vec<ColumnSample>,
    pub h_min: i32,
    pub h_max: i32,
    pub water_max: i32,
    pub cliffy: bool,
    pub any_trees: bool,
}

impl ColumnData {
    #[inline]
    pub fn at(&self, lx: usize, lz: usize) -> &ColumnSample {
        &self.samples[lz * 16 + lx]
    }
}

/// How a cube will be generated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubeClass {
    Empty,
    Deep,
    Surface,
}

/// Counters for benchmarks and the debug overlay.
#[derive(Debug, Default)]
pub struct GenStats {
    pub empty: std::sync::atomic::AtomicU64,
    pub deep: std::sync::atomic::AtomicU64,
    pub surface: std::sync::atomic::AtomicU64,
}

/// Scratch buffer for one cube (reused per thread: no allocation per cube).
pub struct CubeBuf {
    pub origin: hearth_math::BlockPos,
    pub states: [BlockStateId; CUBE_VOLUME],
}

impl CubeBuf {
    #[inline]
    pub fn idx(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        let lx = x - self.origin.x;
        let ly = y - self.origin.y;
        let lz = z - self.origin.z;
        if (0..16).contains(&lx) && (0..16).contains(&ly) && (0..16).contains(&lz) {
            Some(((ly as usize) << 8) | ((lz as usize) << 4) | lx as usize)
        } else {
            None
        }
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> Option<BlockStateId> {
        self.idx(x, y, z).map(|i| self.states[i])
    }

    #[inline]
    pub fn set(&mut self, x: i32, y: i32, z: i32, s: BlockStateId) {
        if let Some(i) = self.idx(x, y, z) {
            self.states[i] = s;
        }
    }
}

thread_local! {
    static BUF: RefCell<Box<CubeBuf>> = RefCell::new(Box::new(CubeBuf {
        origin: hearth_math::BlockPos::ORIGIN,
        states: [BlockStateId::AIR; CUBE_VOLUME],
    }));
}

/// The world generator for one world.
pub struct WorldGenerator {
    pub terrain: Arc<Terrain>,
    pub blocks: GenBlocks,
    pub geology: Geology,
    columns: Cache<ColumnPos, ColumnData>,
    /// Rock columns of 16×16 block columns (index `z * 16 + x`).
    rocks: Cache<ColumnPos, Vec<RockColumn>>,
    caves: caves::CaveGen,
    features: features::FeatureGen,
    planet: Planet,
    seed: u64,
    pub stats: GenStats,
}

impl WorldGenerator {
    /// A generator for the planet in `terrain`, with rocks and provinces from `content`.
    pub fn new(
        terrain: Arc<Terrain>,
        reg: &BlockRegistry,
        content: &hearth_content::Content,
    ) -> Result<Self, MissingBlock> {
        let blocks = GenBlocks::resolve(reg)?;
        let geology = Geology::new(&terrain.grid, content, reg)?;
        let seed = terrain.seed();
        let v = terrain.vertical_scale();
        Ok(Self {
            caves: caves::CaveGen::new(seed, v),
            features: features::FeatureGen::new(seed),
            planet: *terrain.planet(),
            columns: Cache::new(8192),
            rocks: Cache::new(1024),
            geology,
            blocks,
            seed,
            terrain,
            stats: GenStats::default(),
        })
    }

    /// Sets the giant-cavern frequency multiplier (from the world's feature rarity).
    pub fn set_cavern_frequency(&mut self, f: f64) {
        self.caves.set_cavern_frequency(f);
    }

    pub fn planet(&self) -> &Planet {
        &self.planet
    }

    /// Cached surface data of a 16×16 column.
    pub fn column(&self, pos: ColumnPos) -> Arc<ColumnData> {
        let pos = self.planet.wrap_column(pos);
        self.columns
            .get_or_insert_with(pos, || self.compute_column(pos))
    }

    /// Rock columns of a 16×16 column (index `z * 16 + x`).
    pub fn rock_columns(&self, pos: ColumnPos) -> Arc<Vec<RockColumn>> {
        let pos = self.planet.wrap_column(pos);
        self.rocks.get_or_insert_with(pos, || {
            let (x0, z0) = pos.min_block_xz();
            let mut v = Vec::with_capacity(256);
            for lz in 0..16 {
                for lx in 0..16 {
                    v.push(self.geology.column(x0 + lx, z0 + lz));
                }
            }
            v
        })
    }

    /// The bedrock (ignoring soil, caves and features) at a block.
    pub fn rock_at(&self, x: i32, y: i32, z: i32) -> BlockStateId {
        let col = self.rock_columns(ColumnPos::new(x >> 4, z >> 4));
        col[((z & 15) * 16 + (x & 15)) as usize].rock_at(y)
    }

    fn compute_column(&self, pos: ColumnPos) -> ColumnData {
        let (x0, z0) = pos.min_block_xz();
        let segs = self.terrain.river_segments(x0, z0, x0 + 15, z0 + 15);
        let mut samples = Vec::with_capacity(256);
        let (mut h_min, mut h_max, mut water_max) = (i32::MAX, i32::MIN, i32::MIN);
        let mut cliffy = false;
        let mut any_trees = false;
        for lz in 0..16 {
            for lx in 0..16 {
                let s = self.terrain.sample_with(x0 + lx, z0 + lz, &segs);
                h_min = h_min.min(s.height_i());
                h_max = h_max.max(s.height_i());
                water_max = water_max.max(s.water_i());
                cliffy |= s.cliffiness > 0.0;
                any_trees |= s.tree_density > 0.0;
                samples.push(s);
            }
        }
        ColumnData {
            pos,
            samples,
            h_min,
            h_max,
            water_max,
            cliffy,
            any_trees,
        }
    }

    /// Cheap classification of a cube.
    pub fn classify(&self, pos: CubePos) -> CubeClass {
        let col = self.column(pos.column());
        let y0 = pos.y * CUBE_SIZE;
        let y1 = y0 + CUBE_SIZE - 1;
        // Tallest thing that can reach into this column: terrain, water, or a tree whose origin
        // is in this or a neighbouring column.
        let mut top = col.h_max.max(col.water_max);
        if col.cliffy {
            top += CLIFF_AMPLITUDE;
        }
        let mut tree_top = if col.any_trees {
            col.h_max + MAX_FEATURE_HEIGHT
        } else {
            col.h_max + 3
        };
        if y0 <= tree_top.max(top) + 2 {
            // Check neighbours only when the cube is near the band.
            for dz in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dz == 0 {
                        continue;
                    }
                    let n = self.column(ColumnPos::new(pos.x + dx, pos.z + dz));
                    if n.any_trees {
                        tree_top = tree_top.max(n.h_max + MAX_FEATURE_HEIGHT);
                    }
                }
            }
        }
        if y0 > top.max(tree_top) + 1 {
            return CubeClass::Empty;
        }
        let deep_limit =
            col.h_min - MAX_SOIL_DEPTH - if col.cliffy { CLIFF_AMPLITUDE } else { 0 } - 2;
        if y1 < deep_limit {
            return CubeClass::Deep;
        }
        CubeClass::Surface
    }

    /// Generates one cube. Pure: the same position always yields the same cube.
    pub fn generate_cube(&self, pos: CubePos) -> Cube {
        use std::sync::atomic::Ordering::Relaxed;
        let pos = self.planet.wrap_cube(pos);
        let class = self.classify(pos);
        match class {
            CubeClass::Empty => {
                self.stats.empty.fetch_add(1, Relaxed);
                return Cube::filled(BlockStateId::AIR);
            }
            CubeClass::Deep => self.stats.deep.fetch_add(1, Relaxed),
            CubeClass::Surface => self.stats.surface.fetch_add(1, Relaxed),
        };
        BUF.with(|cell| {
            let mut buf = cell.borrow_mut();
            buf.origin = pos.min_block();
            let col = self.column(pos.column());
            let rocks = self.rock_columns(pos.column());
            match class {
                CubeClass::Deep => self.fill_deep(&mut buf, &rocks),
                _ => self.fill_surface(&mut buf, &col, &rocks),
            }
            self.caves
                .carve(&mut buf, pos, &col, &self.terrain, &self.blocks);
            if class == CubeClass::Surface {
                self.features.place(&mut buf, pos, self, &col);
            }
            Cube::from_states(&buf.states)
        })
    }

    fn fill_deep(&self, buf: &mut CubeBuf, rocks: &[RockColumn]) {
        let o = buf.origin;
        for lz in 0..16 {
            for lx in 0..16 {
                let rc = &rocks[(lz * 16 + lx) as usize];
                for ly in 0..16 {
                    let i = ((ly as usize) << 8) | ((lz as usize) << 4) | lx as usize;
                    buf.states[i] = rc.rock_at(o.y + ly);
                }
            }
        }
    }

    fn fill_surface(&self, buf: &mut CubeBuf, col: &ColumnData, rocks: &[RockColumn]) {
        let o = buf.origin;
        let b = &self.blocks;
        for lz in 0..16i32 {
            for lx in 0..16i32 {
                let s = col.at(lx as usize, lz as usize);
                let rc = &rocks[(lz * 16 + lx) as usize];
                let (x, z) = (o.x + lx, o.z + lz);
                let top = s.height_i();
                let water_top = s.water_i();
                // Seasonal snow and ice are applied by the environment at the current date
                // (`hearth_env`); the generator only lays down perennial snow and ice.
                let surface_state = b.surface(s.surface, false);
                let filler = b.filler(s.filler);
                let soil = s.soil_depth as i32;
                for ly in 0..16i32 {
                    let y = o.y + ly;
                    let i = ((ly as usize) << 8) | ((lz as usize) << 4) | lx as usize;
                    let mut ground = y < top;
                    if s.cliffiness > 0.0 && (y - top).abs() <= CLIFF_AMPLITUDE {
                        // 3D shaping on cliffs: overhangs and ledges.
                        let n = self.features.cliff_noise(x, y, z);
                        ground = (top as f32 - y as f32 - 0.5)
                            + n * s.cliffiness * CLIFF_AMPLITUDE as f32
                            > 0.0;
                    }
                    buf.states[i] = if ground {
                        let depth = top - 1 - y;
                        if depth <= 0 && (s.cliffiness == 0.0 || self.is_exposed(x, y, z, top, s)) {
                            surface_state.unwrap_or_else(|| rc.rock_at(y))
                        } else if depth < soil {
                            filler.unwrap_or_else(|| rc.rock_at(y))
                        } else {
                            rc.rock_at(y)
                        }
                    } else if y < water_top {
                        // Multi-year ice where the water never thaws.
                        if y == water_top - 1 && s.temperature < -10.0 {
                            b.ice
                        } else {
                            b.water
                        }
                    } else {
                        b.air
                    };
                }
            }
        }
    }

    /// Whether a cliff-shaped ground block has air above it (so it gets the top material).
    fn is_exposed(&self, x: i32, y: i32, z: i32, top: i32, s: &ColumnSample) -> bool {
        let yy = y + 1;
        let n = self.features.cliff_noise(x, yy, z);
        (top as f32 - yy as f32 - 0.5) + n * s.cliffiness * CLIFF_AMPLITUDE as f32 <= 0.0
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// True if the 1024-block cavern region (rx, rz) holds a giant cavern.
    pub fn has_giant_cavern(&self, rx: i32, rz: i32) -> bool {
        self.caves.has_cavern(rx, rz, &self.terrain)
    }

    /// Column cache (hits, misses).
    pub fn cache_stats(&self) -> (u64, u64) {
        self.columns.stats()
    }
}

#[cfg(test)]
mod tests;

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
pub mod succession;

use std::cell::RefCell;
use std::sync::Arc;

use hearth_math::{CUBE_SIZE, CUBE_VOLUME, ColumnPos, CubePos, Planet};
use hearth_world::{BlockRegistry, BlockStateId, Cube, Fill};

use crate::deposits::Deposits;
use crate::geology::{Geology, RockColumn};
use crate::hydro::Hydrology;
use crate::region::{ColumnSample, Terrain};
use crate::soil::{Profile, Soils};
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
    /// Each voxel's depth inside the ground in voxels (Amendment S §2.2), where a continuous
    /// test says it; NaN where the states alone say it.
    pub depth: [f32; CUBE_VOLUME],
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

    /// Cuts the ground at voxel `i` by a space whose surface is `outside` voxels away (negative
    /// inside the space): the ground's depth there is the lesser.
    #[inline]
    pub fn cut(&mut self, i: usize, outside: f32) {
        let d = self.depth[i];
        let d = if d.is_nan() {
            hearth_world::fill::RANGE
        } else {
            d
        };
        self.depth[i] = d.min(outside);
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
        depth: [f32::NAN; CUBE_VOLUME],
    }));
}

/// The world generator for one world.
pub struct WorldGenerator {
    pub terrain: Arc<Terrain>,
    pub blocks: GenBlocks,
    pub geology: Geology,
    pub soils: Soils,
    pub deposits: Deposits,
    pub hydro: Hydrology,
    /// The tree species and their growth templates.
    pub forest: Arc<crate::trees::Forest>,
    columns: Cache<ColumnPos, ColumnData>,
    /// Rock columns and soil profiles of 16×16 block columns (index `z * 16 + x`).
    rocks: Cache<ColumnPos, Vec<(RockColumn, Profile)>>,
    caves: caves::CaveGen,
    features: features::FeatureGen,
    planet: Planet,
    seed: u64,
    /// Which states are natural ground (by state id), for the fill.
    natural: Vec<bool>,
    pub stats: GenStats,
}

impl WorldGenerator {
    /// What the generator keeps in memory, by kind (bytes): the terrain's, and its own caches
    /// of columns and rock (E4.1 §4.7).
    pub fn memory(&self) -> Vec<(&'static str, u64)> {
        let mut out = self.terrain.memory();
        out.push((
            "generated columns",
            self.columns.sum(|c| {
                (std::mem::size_of::<ColumnData>()
                    + c.samples.capacity() * std::mem::size_of::<ColumnSample>())
                    as u64
            }),
        ));
        out.push((
            "rock columns",
            self.rocks
                .sum(|r| (r.capacity() * std::mem::size_of::<(RockColumn, Profile)>()) as u64),
        ));
        out
    }

    /// A generator for the planet in `terrain`, with rocks and provinces from `content`.
    pub fn new(
        terrain: Arc<Terrain>,
        reg: &BlockRegistry,
        content: &hearth_content::Content,
    ) -> Result<Self, MissingBlock> {
        let mut blocks = GenBlocks::resolve(reg)?;
        let geology = Geology::new(&terrain.grid, content, reg)?;
        let seed = terrain.seed();
        let soils = Soils::new(content, reg, seed, terrain.planet().circumference() as i64)?;
        let province_ids: Vec<String> = geology.provinces().iter().map(|p| p.id.clone()).collect();
        let deposits = Deposits::new(
            content,
            reg,
            &province_ids,
            seed,
            terrain.planet().circumference(),
            terrain.vertical_scale() as f64,
        )?;
        let hydro = Hydrology::new(content, reg, seed, terrain.planet().circumference());
        blocks.add_ground(soils.plantable());
        for extra in [
            blocks.grass,
            blocks.grass_snowy,
            blocks.podzol,
            blocks.podzol_snowy,
            blocks.moss_block,
        ] {
            blocks.add_ground(std::iter::once(extra));
        }
        let v = terrain.vertical_scale();
        let forest = Arc::new(crate::trees::Forest::new(reg, content));
        blocks.add_water_wood(forest.water_wood());
        let natural = (0..reg.state_count())
            .map(|i| reg.has(BlockStateId(i as u16), hearth_world::StateFlags::NATURAL))
            .collect();
        Ok(Self {
            natural,
            caves: caves::CaveGen::new(seed, v),
            features: features::FeatureGen::new(seed),
            planet: *terrain.planet(),
            columns: Cache::new({
                // As many as the columns' memory budget holds (E4.1 §4.7).
                let each = std::mem::size_of::<ColumnData>()
                    + hearth_math::CUBE_AREA * std::mem::size_of::<ColumnSample>();
                (hearth_core::memory::budget(hearth_core::memory::Kind::Columns) as usize / each)
                    .clamp(1024, 16384)
            }),
            rocks: Cache::new(1024),
            geology,
            soils,
            deposits,
            hydro,
            forest,
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

    /// Surface features (trees for the distant terrain).
    pub fn features(&self) -> &features::FeatureGen {
        &self.features
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

    /// Rock columns and soil profiles of a 16×16 column (index `z * 16 + x`).
    pub fn rock_columns(&self, pos: ColumnPos) -> Arc<Vec<(RockColumn, Profile)>> {
        let pos = self.planet.wrap_column(pos);
        self.rocks.get_or_insert_with(pos, || {
            let col = self.column(pos);
            let (x0, z0) = pos.min_block_xz();
            let mut v = Vec::with_capacity(256);
            for lz in 0..16 {
                for lx in 0..16 {
                    let (x, z) = (x0 + lx, z0 + lz);
                    let rock = self.geology.column(x, z);
                    let s = col.at(lx as usize, lz as usize);
                    let parent = rock.rock_at(s.height_i() - 1 - s.soil_depth as i32);
                    let profile = self.soils.profile(s, parent, x, z);
                    v.push((rock, profile));
                }
            }
            v
        })
    }

    /// The bedrock (ignoring soil, caves and features) at a block.
    pub fn rock_at(&self, x: i32, y: i32, z: i32) -> BlockStateId {
        let col = self.rock_columns(ColumnPos::new(x >> 4, z >> 4));
        col[((z & 15) * 16 + (x & 15)) as usize].0.rock_at(y)
    }

    fn compute_column(&self, pos: ColumnPos) -> ColumnData {
        let (x0, z0) = pos.min_block_xz();
        let near = self.terrain.nearby(x0, z0, x0 + 15, z0 + 15);
        let mut samples = Vec::with_capacity(256);
        let (mut h_min, mut h_max, mut water_max) = (i32::MAX, i32::MIN, i32::MIN);
        let mut cliffy = false;
        let mut any_trees = false;
        for lz in 0..16 {
            for lx in 0..16 {
                let s = self.terrain.sample_with(x0 + lx, z0 + lz, &near);
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

    /// Generates one cube of the world as it began (the trees as they stood, nothing
    /// disturbed). Pure: the same position always yields the same cube.
    pub fn generate_cube(&self, pos: CubePos) -> Cube {
        self.generate_cube_in(pos, &crate::vegetation::Vegetation::default())
            .0
    }

    /// Generates one cube as the vegetation has grown, and the year its trees or ground next
    /// change (infinity: never). Pure for a vegetation.
    pub fn generate_cube_in(
        &self,
        pos: CubePos,
        veg: &crate::vegetation::Vegetation,
    ) -> (Cube, f64) {
        use std::sync::atomic::Ordering::Relaxed;
        let pos = self.planet.wrap_cube(pos);
        let class = self.classify(pos);
        match class {
            CubeClass::Empty => {
                self.stats.empty.fetch_add(1, Relaxed);
                return (Cube::filled(BlockStateId::AIR), f64::INFINITY);
            }
            CubeClass::Deep => self.stats.deep.fetch_add(1, Relaxed),
            CubeClass::Surface => self.stats.surface.fetch_add(1, Relaxed),
        };
        BUF.with(|cell| {
            let mut buf = cell.borrow_mut();
            buf.origin = pos.min_block();
            buf.depth.fill(f32::NAN);
            let col = self.column(pos.column());
            let rocks = self.rock_columns(pos.column());
            match class {
                CubeClass::Deep => self.fill_deep(&mut buf, &rocks),
                _ => self.fill_surface(&mut buf, &col, &rocks),
            }
            self.deposits.apply(&mut buf, pos, self);
            if class == CubeClass::Surface {
                self.hydro.apply(&mut buf, self);
            }
            self.caves.carve(&mut buf, pos, &col, self, &self.blocks);
            let next = if class == CubeClass::Surface {
                self.features.place(&mut buf, self, &col, veg)
            } else {
                f64::INFINITY
            };
            (Cube::from_states_fill(&buf.states, self.fill(&buf)), next)
        })
    }

    /// The cube's fill: each voxel's depth where a continuous test gave it, made to agree with
    /// what the voxel ended as (deposits, water and trees change states after the tests);
    /// what the states say elsewhere.
    fn fill(&self, buf: &CubeBuf) -> Fill {
        let mut f = Box::new([hearth_world::fill::EMPTY; CUBE_VOLUME]);
        for (i, q) in f.iter_mut().enumerate() {
            let natural = self.natural[buf.states[i].0 as usize];
            let d = buf.depth[i];
            *q = if d.is_nan() {
                if natural {
                    hearth_world::fill::FULL
                } else {
                    hearth_world::fill::EMPTY
                }
            } else {
                Fill::agree(Fill::quantize(d), natural)
            };
        }
        Fill(f)
    }

    fn fill_deep(&self, buf: &mut CubeBuf, rocks: &[(RockColumn, Profile)]) {
        let o = buf.origin;
        for lz in 0..16 {
            for lx in 0..16 {
                let rc = &rocks[(lz * 16 + lx) as usize].0;
                for ly in 0..16 {
                    let i = ((ly as usize) << 8) | ((lz as usize) << 4) | lx as usize;
                    buf.states[i] = rc.rock_at(o.y + ly);
                }
            }
        }
    }

    fn fill_surface(&self, buf: &mut CubeBuf, col: &ColumnData, rocks: &[(RockColumn, Profile)]) {
        let o = buf.origin;
        let b = &self.blocks;
        for lz in 0..16i32 {
            for lx in 0..16i32 {
                let s = col.at(lx as usize, lz as usize);
                let (rc, profile) = &rocks[(lz * 16 + lx) as usize];
                let (x, z) = (o.x + lx, o.z + lz);
                let top = s.height_i();
                let water_top = s.water_i();
                let across = 1.0 / (1.0 + s.slope * s.slope).sqrt();
                // Tide pools in the rock of stony shores, just above the sea.
                let tide_pool = s.biome == crate::region::biome::Biome::StonyShore
                    && (0..=2).contains(&top)
                    && s.slope < 0.4
                    && hearth_math::hash::unit_f32(hearth_math::hash::hash_2d(
                        self.seed ^ 0x71de,
                        x,
                        z,
                    )) < 0.07;
                // Seasonal snow and ice are applied by the environment at the current date
                // (`hearth_env`); the generator only lays down perennial snow and ice.
                for ly in 0..16i32 {
                    let y = o.y + ly;
                    let i = ((ly as usize) << 8) | ((lz as usize) << 4) | lx as usize;
                    // How far inside the ground: down from the surface's continuous height,
                    // across the slope (the distance to an inclined plane).
                    let mut depth = (s.height - y as f32 - 0.5) * across;
                    let mut ground = y < top;
                    if s.cliffiness > 0.0 && (y - top).abs() <= CLIFF_AMPLITUDE {
                        // 3D shaping on cliffs: overhangs and ledges.
                        let n = self.features.cliff_noise(x, y, z);
                        depth = (top as f32 - y as f32 - 0.5)
                            + n * s.cliffiness * CLIFF_AMPLITUDE as f32;
                        ground = depth > 0.0;
                    }
                    buf.depth[i] = depth;
                    buf.states[i] = if ground {
                        // Depth below the top block (0 = the top block).
                        let depth = top - 1 - y;
                        let exposed =
                            depth <= 0 && (s.cliffiness == 0.0 || self.is_exposed(x, y, z, top, s));
                        let layer = if exposed { 0 } else { depth.max(1) as usize };
                        if depth == 0 && tide_pool {
                            b.water
                        } else {
                            profile.get(layer).copied().unwrap_or_else(|| rc.rock_at(y))
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
        self.caves.has_cavern(rx, rz, self)
    }

    /// Column cache (hits, misses).
    pub fn cache_stats(&self) -> (u64, u64) {
        self.columns.stats()
    }
}

#[cfg(test)]
mod tests;

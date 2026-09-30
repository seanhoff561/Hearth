//! Surface features: trees (procedural per species and climate), plants, underwater flora,
//! fallen logs, boulders and cacti.
//!
//! Every feature is a pure function of its origin. A cube gathers the features whose bounds
//! intersect it and writes only its own blocks. Overlaps are resolved by a priority lattice
//! (terrain > logs > boulders > leaves by distance > plants > air, ties broken by state id), so
//! the result is identical in any order.

use hearth_math::hash::{Rng, derive_seed, hash_2d, hash_3d, unit_f32};
use hearth_math::{ColumnPos, CubePos};
use hearth_world::BlockStateId;

use super::blocks::{GenBlocks, Wood};
use super::{ColumnData, CubeBuf, WorldGenerator};
use crate::noise::Perlin;
use crate::region::biome::Biome;
use crate::region::{ColumnSample, Surface};

/// Size of the tree placement grid cells.
const TREE_CELL: i32 = 5;
/// Horizontal reach of the widest tree from its origin.
const TREE_REACH: i32 = 11;
/// Grid for fallen logs and boulders.
const DEBRIS_CELL: i32 = 12;

/// Feature placement.
#[derive(Debug, Clone)]
pub struct FeatureGen {
    seed: u64,
    cliff: Perlin,
    flower_patch: Perlin,
}

/// Tree shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TreeKind {
    Oak,
    BigOak,
    GiantOak,
    SavannaOak,
    ShrubOak,
    SwampOak,
    Birch,
    Spruce,
    GiantSpruce,
    Krummholz,
    Bush,
}

impl TreeKind {
    fn wood(self) -> Wood {
        match self {
            TreeKind::Birch => Wood::Birch,
            TreeKind::Spruce | TreeKind::GiantSpruce | TreeKind::Krummholz => Wood::Spruce,
            _ => Wood::Oak,
        }
    }

    /// Conservative (horizontal reach, height) of the shape.
    fn extent(self) -> (i32, i32) {
        match self {
            TreeKind::Oak => (4, 10),
            TreeKind::BigOak => (8, 20),
            TreeKind::GiantOak => (TREE_REACH, 36),
            TreeKind::SavannaOak => (7, 10),
            TreeKind::ShrubOak => (3, 6),
            TreeKind::SwampOak => (5, 10),
            TreeKind::Birch => (3, 11),
            TreeKind::Spruce => (4, 17),
            TreeKind::GiantSpruce => (7, 36),
            TreeKind::Krummholz => (2, 5),
            TreeKind::Bush => (2, 3),
        }
    }
}

/// Priority of a state for merging overlapping features.
struct Priorities<'a> {
    b: &'a GenBlocks,
}

impl Priorities<'_> {
    #[inline]
    fn of(&self, s: BlockStateId) -> u32 {
        let b = self.b;
        if s.is_air() {
            return 0;
        }
        for w in [&b.oak, &b.birch, &b.spruce] {
            if s == w.log_y || s == w.log_x || s == w.log_z {
                return 60;
            }
            if let Some(d) = w.leaves.iter().position(|l| *l == s) {
                return 40 - d as u32; // distance 1 → 40, distance 7 → 34
            }
        }
        if s == b.mossy_cobblestone || s == b.cobblestone || s == b.andesite || s == b.cactus {
            return 50;
        }
        if is_plant(b, s) {
            return 10;
        }
        // Terrain, water and anything else: never replaced by features.
        1000
    }
}

fn is_plant(b: &GenBlocks, s: BlockStateId) -> bool {
    s == b.short_grass
        || s == b.fern
        || s == b.short_dry_grass
        || s == b.dead_bush
        || b.tall_grass.contains(&s)
        || b.large_fern.contains(&s)
        || b.tall_dry_grass.contains(&s)
        || b.flowers_meadow.contains(&s)
        || b.flowers_forest.contains(&s)
        || b.flowers_alpine.contains(&s)
        || s == b.brown_mushroom
        || s == b.red_mushroom
        || s == b.moss_carpet
        || b.snow_layers.contains(&s)
        || b.vines.contains(&s)
}

/// Writes blocks into the cube using the priority lattice.
struct Writer<'a> {
    buf: &'a mut CubeBuf,
    prio: Priorities<'a>,
}

impl Writer<'_> {
    #[inline]
    fn put(&mut self, x: i32, y: i32, z: i32, s: BlockStateId) {
        let Some(i) = self.buf.idx(x, y, z) else {
            return;
        };
        let cur = self.buf.states[i];
        let (pc, pn) = (self.prio.of(cur), self.prio.of(s));
        if pn > pc || (pn == pc && pc < 1000 && s.0 > cur.0) {
            self.buf.states[i] = s;
        }
    }
}

fn soil_ok(s: &ColumnSample) -> bool {
    matches!(
        s.surface,
        Surface::Grass
            | Surface::Podzol
            | Surface::Dirt
            | Surface::CoarseDirt
            | Surface::Moss
            | Surface::SnowGrass
            | Surface::Mud
    )
}

impl FeatureGen {
    pub fn new(seed: u64) -> Self {
        Self {
            seed: derive_seed(seed, "features"),
            cliff: Perlin::new(derive_seed(seed, "cliff3d")),
            flower_patch: Perlin::new(derive_seed(seed, "flowers")),
        }
    }

    /// 3D noise in [-1, 1] used for cliff overhangs.
    #[inline]
    pub fn cliff_noise(&self, x: i32, y: i32, z: i32) -> f32 {
        let f = 1.0 / 11.0;
        self.cliff
            .noise3(x as f64 * f, y as f64 * f * 1.4, z as f64 * f, 0) as f32
    }

    /// Places all features intersecting the cube.
    pub fn place(&self, buf: &mut CubeBuf, pos: CubePos, wg: &WorldGenerator, col: &ColumnData) {
        let b = &wg.blocks;
        let o = buf.origin;
        let mut w = Writer {
            buf,
            prio: Priorities { b },
        };
        // Plants and underwater flora of this cube's own columns.
        for lz in 0..16 {
            for lx in 0..16 {
                let s = col.at(lx as usize, lz as usize);
                self.decorate_column(&mut w, wg, col, o.x + lx, o.z + lz, s);
            }
        }
        // Trees whose origin cell is near the cube.
        let (x0, x1) = (o.x - TREE_REACH, o.x + 15 + TREE_REACH);
        let (z0, z1) = (o.z - TREE_REACH, o.z + 15 + TREE_REACH);
        for fz in z0.div_euclid(TREE_CELL)..=z1.div_euclid(TREE_CELL) {
            for fx in x0.div_euclid(TREE_CELL)..=x1.div_euclid(TREE_CELL) {
                self.tree_cell(&mut w, wg, pos, fx, fz);
            }
        }
        for fz in (o.z - 8).div_euclid(DEBRIS_CELL)..=(o.z + 23).div_euclid(DEBRIS_CELL) {
            for fx in (o.x - 8).div_euclid(DEBRIS_CELL)..=(o.x + 23).div_euclid(DEBRIS_CELL) {
                self.debris_cell(&mut w, wg, fx, fz);
            }
        }
    }

    fn sample_at(wg: &WorldGenerator, x: i32, z: i32) -> ColumnSample {
        let c = wg.column(ColumnPos::new(x >> 4, z >> 4));
        *c.at((x & 15) as usize, (z & 15) as usize)
    }

    fn decorate_column(
        &self,
        w: &mut Writer<'_>,
        wg: &WorldGenerator,
        col: &ColumnData,
        x: i32,
        z: i32,
        s: &ColumnSample,
    ) {
        let b = &wg.blocks;
        let top = s.height_i();
        let o = w.buf.origin;
        // Quick reject: nothing this column places can reach the cube.
        let reach_top = if s.is_underwater() {
            s.water_i()
        } else {
            top + 2
        };
        if reach_top < o.y || top > o.y + 15 {
            return;
        }
        let h = hash_2d(self.seed, x, z);
        let r = unit_f32(h);
        let r2 = unit_f32(h.rotate_left(21));
        if s.is_underwater() {
            let depth = s.water_i() - top;
            let floor_ok = matches!(
                s.surface,
                Surface::Sand | Surface::Gravel | Surface::Dirt | Surface::Clay
            );
            match s.biome {
                Biome::ColdSea | Biome::TemperateSea
                    if floor_ok && depth > 4 && depth < 40 && r < 0.12 =>
                {
                    // Kelp forests in cool water.
                    let len = (4 + (r2 * 18.0) as i32).min(depth - 2);
                    for k in 0..len {
                        let st = if k == len - 1 { b.kelp } else { b.kelp_plant };
                        w.put(x, top + k, z, st);
                    }
                }
                Biome::WarmShallows
                | Biome::TemperateSea
                | Biome::ColdSea
                | Biome::Lake
                | Biome::River
                    if floor_ok && (2..25).contains(&depth) && r < 0.3 =>
                {
                    if r2 < 0.2 && depth >= 3 {
                        w.put(x, top, z, b.tall_seagrass[0]);
                        w.put(x, top + 1, z, b.tall_seagrass[1]);
                    } else {
                        w.put(x, top, z, b.seagrass);
                    }
                }
                Biome::Wetland | Biome::Lake if depth <= 2 && r < 0.08 && s.temperature > 4.0 => {
                    w.put(x, s.water_i(), z, b.lily_pad);
                }
                _ => {}
            }
            return;
        }
        // Only plant on exposed soil (caves or cliffs may have removed it).
        let ground = w.buf.get(x, top - 1, z);
        let above_air = w.buf.get(x, top, z).is_none_or(|s| s.is_air());
        let soil_block = ground.is_none_or(|g| {
            g == b.grass
                || g == b.grass_snowy
                || g == b.podzol
                || g == b.moss_block
                || g == b.coarse_dirt
                || g == b.dirt
                || g == b.sand
                || g == b.red_sand
                || g == b.mud
        });
        if !above_air || !soil_block || s.temperature < -0.5 {
            return;
        }
        let flower_n = self
            .flower_patch
            .noise2(x as f64 / 24.0, z as f64 / 24.0, 0) as f32;
        let grassy = matches!(s.surface, Surface::Grass | Surface::Podzol | Surface::Moss);
        let tall = |w: &mut Writer<'_>, pair: [BlockStateId; 2]| {
            w.put(x, top, z, pair[0]);
            w.put(x, top + 1, z, pair[1]);
        };
        match s.biome {
            Biome::HotDesert | Biome::DuneSea | Biome::Mesa | Biome::ColdDesert => {
                if r < 0.012 {
                    w.put(x, top, z, b.dead_bush);
                }
            }
            Biome::Steppe | Biome::Savanna => {
                if grassy {
                    if r < 0.28 {
                        w.put(x, top, z, b.short_dry_grass);
                    } else if r < 0.36 {
                        tall(w, b.tall_dry_grass);
                    } else if r < 0.37 && s.biome == Biome::Steppe {
                        w.put(x, top, z, b.dead_bush);
                    }
                }
            }
            Biome::SaltFlat
            | Biome::Beach
            | Biome::StonyShore
            | Biome::Glacier
            | Biome::IceSheet
            | Biome::AlpineRock
            | Biome::Volcanic => {}
            Biome::AlpineMeadow => {
                if grassy {
                    if flower_n > 0.35 && r < 0.35 {
                        w.put(x, top, z, b.flowers_alpine[(r2 * 2.0) as usize % 2]);
                    } else if r < 0.4 {
                        w.put(x, top, z, b.short_grass);
                    }
                }
            }
            Biome::Tundra => {
                if grassy && r < 0.15 {
                    w.put(x, top, z, b.short_grass);
                }
            }
            Biome::TropicalRainforest | Biome::TemperateRainforest => {
                if grassy || s.surface == Surface::Moss {
                    if r < 0.2 {
                        w.put(x, top, z, b.fern);
                    } else if r < 0.3 {
                        tall(w, b.large_fern);
                    } else if r < 0.45 {
                        w.put(x, top, z, b.short_grass);
                    } else if r < 0.5 {
                        w.put(x, top, z, b.moss_carpet);
                    } else if r < 0.505 {
                        w.put(
                            x,
                            top,
                            z,
                            if r2 < 0.5 {
                                b.brown_mushroom
                            } else {
                                b.red_mushroom
                            },
                        );
                    }
                }
            }
            Biome::BorealForest | Biome::SnowyTaiga | Biome::Krummholz | Biome::MontaneForest => {
                if r < 0.12 {
                    w.put(x, top, z, b.fern);
                } else if r < 0.16 {
                    tall(w, b.large_fern);
                } else if r < 0.26 {
                    w.put(x, top, z, b.short_grass);
                } else if r < 0.28 {
                    w.put(x, top, z, b.brown_mushroom);
                }
            }
            Biome::Wetland => {
                if r < 0.35 {
                    w.put(x, top, z, b.short_grass);
                } else if r < 0.45 {
                    tall(w, b.tall_grass);
                }
                self.sugar_cane(w, wg, col, x, z, top, r2);
            }
            _ => {
                // Temperate grasslands and forests: grass, flowers in clusters.
                let forest = matches!(
                    s.biome,
                    Biome::BroadleafForest | Biome::BirchForest | Biome::MixedForest
                );
                if grassy {
                    if flower_n > 0.42 && r < if forest { 0.12 } else { 0.3 } {
                        let f = if forest {
                            b.flowers_forest[(r2 * 2.0) as usize % 2]
                        } else {
                            // One species per cluster, from the patch position.
                            let cluster =
                                hash_2d(self.seed ^ 0xf10, x.div_euclid(24), z.div_euclid(24));
                            b.flowers_meadow[(cluster % 5) as usize]
                        };
                        w.put(x, top, z, f);
                    } else if r < 0.34 {
                        w.put(x, top, z, b.short_grass);
                    } else if r < 0.4 {
                        tall(w, b.tall_grass);
                    } else if forest && r < 0.43 {
                        w.put(x, top, z, b.fern);
                    } else if forest && r > 0.997 {
                        w.put(
                            x,
                            top,
                            z,
                            if r2 < 0.5 {
                                b.brown_mushroom
                            } else {
                                b.red_mushroom
                            },
                        );
                    }
                }
                if s.biome == Biome::Oasis || s.river.is_some() {
                    self.sugar_cane(w, wg, col, x, z, top, r2);
                }
            }
        }
    }

    /// Sugar cane on banks directly next to water.
    #[allow(clippy::too_many_arguments)]
    fn sugar_cane(
        &self,
        w: &mut Writer<'_>,
        wg: &WorldGenerator,
        col: &ColumnData,
        x: i32,
        z: i32,
        top: i32,
        r: f32,
    ) {
        if r > 0.16 {
            return;
        }
        let near_water = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dz)| {
            let (nx, nz) = (x + dx, z + dz);
            let n = if nx >> 4 == col.pos.x && nz >> 4 == col.pos.z {
                *col.at((nx & 15) as usize, (nz & 15) as usize)
            } else {
                Self::sample_at(wg, nx, nz)
            };
            n.is_underwater() && n.water_i() == top
        });
        if near_water {
            let len = 1 + (r * 20.0) as i32 % 3;
            for k in 0..len {
                w.put(x, top + k, z, wg.blocks.sugar_cane);
            }
        }
    }

    fn tree_cell(&self, w: &mut Writer<'_>, wg: &WorldGenerator, pos: CubePos, fx: i32, fz: i32) {
        let h = hash_2d(self.seed ^ 0x7ee5, fx, fz);
        let ox = fx * TREE_CELL + (h % TREE_CELL as u64) as i32;
        let oz = fz * TREE_CELL + ((h >> 8) % TREE_CELL as u64) as i32;
        let u = unit_f32(h >> 16);
        let s = Self::sample_at(wg, ox, oz);
        // Trees are denser than one per cell only where density is ~1.
        if u >= s.tree_density * 0.95 || s.is_underwater() || !soil_ok(&s) || s.slope > 0.95 {
            return;
        }
        let kind = self.choose_tree(&s, h);
        let (reach, height) = kind.extent();
        let base = s.height_i();
        let y0 = pos.y * 16;
        let o = w.buf.origin;
        if base + height < y0 || base - 3 > y0 + 15 {
            return;
        }
        if ox + reach < o.x || ox - reach > o.x + 15 || oz + reach < o.z || oz - reach > o.z + 15 {
            return;
        }
        let mut rng = Rng::new(hash_3d(self.seed, ox, base, oz));
        self.grow(w, &wg.blocks, kind, ox, base, oz, &mut rng, &s);
    }

    fn choose_tree(&self, s: &ColumnSample, h: u64) -> TreeKind {
        let r = unit_f32(h.rotate_left(33));
        let r2 = unit_f32(h.rotate_left(47));
        use TreeKind::*;
        match s.biome {
            Biome::TropicalRainforest => {
                if r < 0.32 {
                    GiantOak
                } else if r < 0.55 {
                    BigOak
                } else if r < 0.75 {
                    Bush
                } else {
                    Oak
                }
            }
            Biome::Savanna => SavannaOak,
            Biome::MediterraneanScrub => {
                if r < 0.8 {
                    ShrubOak
                } else {
                    Oak
                }
            }
            Biome::Wetland => {
                if s.temperature < 3.0 {
                    Spruce
                } else {
                    SwampOak
                }
            }
            Biome::BirchForest => {
                if r < 0.85 {
                    Birch
                } else {
                    Oak
                }
            }
            Biome::TemperateRainforest => {
                if r < 0.45 {
                    GiantSpruce
                } else if r < 0.85 {
                    Spruce
                } else {
                    BigOak
                }
            }
            Biome::MixedForest => {
                if r < 0.35 {
                    Oak
                } else if r < 0.6 {
                    Birch
                } else {
                    Spruce
                }
            }
            Biome::BorealForest | Biome::SnowyTaiga => {
                if r < 0.9 {
                    Spruce
                } else {
                    Birch
                }
            }
            Biome::MontaneForest => {
                if r < 0.6 {
                    Spruce
                } else if r < 0.8 {
                    Oak
                } else {
                    Birch
                }
            }
            Biome::Krummholz | Biome::AlpineMeadow | Biome::Tundra => {
                if r < 0.8 {
                    Krummholz
                } else {
                    Bush
                }
            }
            Biome::Steppe => Oak,
            _ => {
                // Temperate broadleaf, plains, oases.
                if r < 0.05 && r2 < 0.7 {
                    BigOak
                } else if r < 0.28 && s.temperature < 15.0 {
                    Birch
                } else if r < 0.34 {
                    Bush
                } else {
                    Oak
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn grow(
        &self,
        w: &mut Writer<'_>,
        b: &GenBlocks,
        kind: TreeKind,
        x: i32,
        y: i32,
        z: i32,
        rng: &mut Rng,
        s: &ColumnSample,
    ) {
        let wood = b.wood(kind.wood());
        // Collect logs first; leaves take their distance from the nearest log.
        let mut logs: smallvec::SmallVec<[(i32, i32, i32, BlockStateId); 64]> =
            smallvec::SmallVec::new();
        let mut blobs: smallvec::SmallVec<[(f32, f32, f32, f32, f32); 12]> =
            smallvec::SmallVec::new(); // cx, cy, cz, r, ry
        let mut vines = false;
        // Moisture makes trees taller.
        let vigor = ((s.precipitation - 400.0) / 1600.0).clamp(0.0, 1.0);
        let trunk = |logs: &mut smallvec::SmallVec<[(i32, i32, i32, BlockStateId); 64]>,
                     h: i32,
                     wide: bool| {
            for k in 0..h {
                logs.push((x, y + k, z, wood.log_y));
                if wide {
                    logs.push((x + 1, y + k, z, wood.log_y));
                    logs.push((x, y + k, z + 1, wood.log_y));
                    logs.push((x + 1, y + k, z + 1, wood.log_y));
                }
            }
        };
        match kind {
            TreeKind::Oak => {
                let h = rng.range_i32(4, 6) + (vigor * 1.5) as i32;
                trunk(&mut logs, h, false);
                let top = (y + h) as f32;
                blobs.push((
                    x as f32 + 0.5,
                    top - 0.5,
                    z as f32 + 0.5,
                    rng.range_f32(2.3, 2.9),
                    2.0,
                ));
                for _ in 0..2 {
                    blobs.push((
                        x as f32 + 0.5 + rng.range_f32(-1.2, 1.2),
                        top - rng.range_f32(1.0, 2.2),
                        z as f32 + 0.5 + rng.range_f32(-1.2, 1.2),
                        rng.range_f32(1.8, 2.4),
                        1.6,
                    ));
                }
            }
            TreeKind::BigOak => {
                let h = rng.range_i32(9, 15);
                trunk(&mut logs, h, false);
                let branches = rng.range_i32(3, 6);
                for k in 0..branches {
                    let start = y + h / 2 + rng.range_i32(0, h / 2);
                    let a = k as f32 / branches as f32 * std::f32::consts::TAU
                        + rng.range_f32(-0.4, 0.4);
                    let len = rng.range_f32(3.0, 5.5);
                    let (mut bx, mut by, mut bz) =
                        (x as f32 + 0.5, start as f32 + 0.5, z as f32 + 0.5);
                    let steps = len.ceil() as i32;
                    for _ in 0..steps {
                        bx += a.cos();
                        bz += a.sin();
                        by += 0.6;
                        let axis = if a.cos().abs() > a.sin().abs() {
                            wood.log_x
                        } else {
                            wood.log_z
                        };
                        logs.push((
                            bx.floor() as i32,
                            by.floor() as i32,
                            bz.floor() as i32,
                            axis,
                        ));
                    }
                    blobs.push((bx, by + 0.5, bz, rng.range_f32(2.4, 3.0), 2.0));
                }
                blobs.push((x as f32 + 0.5, (y + h) as f32, z as f32 + 0.5, 3.0, 2.2));
            }
            TreeKind::GiantOak => {
                let h = rng.range_i32(18, 26) + (vigor * 6.0) as i32;
                trunk(&mut logs, h, true);
                // Buttress roots.
                for (dx, dz, axis) in [
                    (-1, 0, wood.log_x),
                    (2, 1, wood.log_x),
                    (0, -1, wood.log_z),
                    (1, 2, wood.log_z),
                ] {
                    logs.push((x + dx, y, z + dz, axis));
                    if rng.chance(0.5) {
                        logs.push((x + dx, y + 1, z + dz, wood.log_y));
                    }
                }
                let top = (y + h) as f32;
                let r = rng.range_f32(6.0, 8.5);
                blobs.push((x as f32 + 1.0, top, z as f32 + 1.0, r, 2.6));
                for _ in 0..3 {
                    let a = rng.range_f32(0.0, std::f32::consts::TAU);
                    let d = rng.range_f32(2.0, 4.0);
                    let bx = x as f32 + 1.0 + a.cos() * d;
                    let bz = z as f32 + 1.0 + a.sin() * d;
                    let by = top - rng.range_f32(4.0, 7.0);
                    // Branch to the secondary crown.
                    let steps = d.ceil() as i32;
                    for k in 1..=steps {
                        let t = k as f32 / steps as f32;
                        let lx = x as f32 + 1.0 + a.cos() * d * t;
                        let lz = z as f32 + 1.0 + a.sin() * d * t;
                        let axis = if a.cos().abs() > a.sin().abs() {
                            wood.log_x
                        } else {
                            wood.log_z
                        };
                        logs.push((
                            lx.floor() as i32,
                            (by - 1.0) as i32,
                            lz.floor() as i32,
                            axis,
                        ));
                    }
                    blobs.push((bx, by, bz, rng.range_f32(3.0, 4.0), 1.8));
                }
                vines = true;
            }
            TreeKind::SavannaOak => {
                let h = rng.range_i32(4, 6);
                let lean = rng.range_i32(1, 2);
                let a = rng.below(4);
                let (ldx, ldz) = [(1, 0), (-1, 0), (0, 1), (0, -1)][a as usize];
                for k in 0..h {
                    let off = if k >= h / 2 { lean } else { 0 };
                    logs.push((x + ldx * off, y + k, z + ldz * off, wood.log_y));
                }
                let (tx, tz) = (x + ldx * lean, z + ldz * lean);
                let r = rng.range_f32(3.6, 4.6);
                blobs.push((
                    tx as f32 + 0.5,
                    (y + h) as f32 + 0.3,
                    tz as f32 + 0.5,
                    r,
                    0.9,
                ));
                if rng.chance(0.5) {
                    blobs.push((
                        tx as f32 + 0.5 - ldx as f32 * 3.0,
                        (y + h) as f32 - 1.4,
                        tz as f32 + 0.5 - ldz as f32 * 3.0,
                        2.2,
                        0.7,
                    ));
                }
            }
            TreeKind::ShrubOak => {
                let h = rng.range_i32(2, 3);
                trunk(&mut logs, h, false);
                blobs.push((
                    x as f32 + 0.5,
                    (y + h) as f32,
                    z as f32 + 0.5,
                    rng.range_f32(1.9, 2.5),
                    1.5,
                ));
            }
            TreeKind::SwampOak => {
                let h = rng.range_i32(4, 6);
                trunk(&mut logs, h, false);
                blobs.push((
                    x as f32 + 0.5,
                    (y + h) as f32 - 0.5,
                    z as f32 + 0.5,
                    rng.range_f32(3.0, 3.6),
                    1.4,
                ));
                vines = true;
            }
            TreeKind::Birch => {
                let h = rng.range_i32(5, 7) + (vigor * 1.5) as i32;
                trunk(&mut logs, h, false);
                blobs.push((
                    x as f32 + 0.5,
                    (y + h) as f32 - 1.0,
                    z as f32 + 0.5,
                    2.1,
                    2.6,
                ));
            }
            TreeKind::Spruce => {
                let h = rng.range_i32(7, 11) + (vigor * 3.0) as i32;
                trunk(&mut logs, h, false);
                self.spruce_crown(w, wood.leaves, x, y, z, h, 2.8 + vigor, false);
            }
            TreeKind::GiantSpruce => {
                let h = rng.range_i32(22, 30);
                trunk(&mut logs, h, true);
                self.spruce_crown(w, wood.leaves, x, y, z, h, 5.5, true);
            }
            TreeKind::Krummholz => {
                let h = rng.range_i32(1, 2);
                trunk(&mut logs, h, false);
                blobs.push((
                    x as f32 + 0.5,
                    (y + h) as f32 - 0.2,
                    z as f32 + 0.5,
                    rng.range_f32(1.4, 2.1),
                    1.0,
                ));
            }
            TreeKind::Bush => {
                logs.push((x, y, z, wood.log_y));
                blobs.push((
                    x as f32 + 0.5,
                    y as f32 + 0.8,
                    z as f32 + 0.5,
                    rng.range_f32(1.4, 2.0),
                    1.2,
                ));
            }
        }
        // Leaves from blobs: distance = Manhattan distance to the nearest log (1..=6).
        let o = w.buf.origin;
        for &(cx, cy, cz, r, ry) in &blobs {
            let (x0, x1) = ((cx - r).floor() as i32, (cx + r).ceil() as i32);
            let (y0, y1) = ((cy - ry).floor() as i32, (cy + ry).ceil() as i32);
            let (z0, z1) = ((cz - r).floor() as i32, (cz + r).ceil() as i32);
            if x1 < o.x || x0 > o.x + 15 || y1 < o.y || y0 > o.y + 15 || z1 < o.z || z0 > o.z + 15 {
                continue;
            }
            for ly in y0.max(o.y)..=y1.min(o.y + 15) {
                for lz in z0.max(o.z)..=z1.min(o.z + 15) {
                    for lx in x0.max(o.x)..=x1.min(o.x + 15) {
                        let dx = (lx as f32 + 0.5 - cx) / r;
                        let dy = (ly as f32 + 0.5 - cy) / ry;
                        let dz = (lz as f32 + 0.5 - cz) / r;
                        let d = dx * dx + dy * dy + dz * dz;
                        if d > 1.0 {
                            continue;
                        }
                        // Ragged edges.
                        if d > 0.65 && unit_f32(hash_3d(self.seed ^ 0x1eaf, lx, ly, lz)) < 0.35 {
                            continue;
                        }
                        let dist = logs
                            .iter()
                            .map(|(bx, by, bz, _)| {
                                (bx - lx).abs() + (by - ly).abs() + (bz - lz).abs()
                            })
                            .min()
                            .unwrap_or(7)
                            .clamp(1, 7);
                        if dist >= 7 {
                            continue;
                        }
                        w.put(lx, ly, lz, wood.leaves[dist as usize - 1]);
                        if vines && d > 0.55 && ly <= cy as i32 {
                            self.hang_vine(w, lx, ly, lz, cx, cz);
                        }
                    }
                }
            }
        }
        for (lx, ly, lz, st) in logs {
            w.put(lx, ly, lz, st);
        }
    }

    /// Conical spruce crown: ragged layers shrinking toward a spike.
    #[allow(clippy::too_many_arguments)]
    fn spruce_crown(
        &self,
        w: &mut Writer<'_>,
        leaves: [BlockStateId; 7],
        x: i32,
        y: i32,
        z: i32,
        h: i32,
        max_r: f32,
        wide: bool,
    ) {
        let start = y + (h as f32 * if wide { 0.4 } else { 0.25 }) as i32;
        let top = y + h + 1;
        let (cx, cz) = if wide {
            (x as f32 + 1.0, z as f32 + 1.0)
        } else {
            (x as f32 + 0.5, z as f32 + 0.5)
        };
        let o = w.buf.origin;
        if top < o.y || start > o.y + 15 {
            return;
        }
        for ly in start..=top {
            let t = (ly - start) as f32 / (top - start).max(1) as f32;
            // Alternate wider and narrower layers.
            let ragged = if (ly - start) % 2 == 0 { 1.0 } else { 0.72 };
            let r = (max_r * (1.0 - t) * ragged).max(if ly >= top - 1 { 0.5 } else { 0.9 });
            let ri = r.ceil() as i32;
            for lz in (cz - 0.5) as i32 - ri..=(cz - 0.5) as i32 + ri + i32::from(wide) {
                for lx in (cx - 0.5) as i32 - ri..=(cx - 0.5) as i32 + ri + i32::from(wide) {
                    let dx = lx as f32 + 0.5 - cx;
                    let dz = lz as f32 + 0.5 - cz;
                    if dx * dx + dz * dz > r * r + 0.3 {
                        continue;
                    }
                    let trunk_d = if wide {
                        (lx - x).abs().min((lx - x - 1).abs())
                            + (lz - z).abs().min((lz - z - 1).abs())
                    } else {
                        (lx - x).abs() + (lz - z).abs()
                    };
                    let above_trunk = if ly > y + h - 1 { ly - (y + h - 1) } else { 0 };
                    let dist = (trunk_d + above_trunk).clamp(1, 6);
                    w.put(lx, ly, lz, leaves[dist as usize - 1]);
                }
            }
        }
    }

    /// Vines hanging from the outer face of a crown leaf.
    fn hang_vine(&self, w: &mut Writer<'_>, lx: i32, ly: i32, lz: i32, cx: f32, cz: f32) {
        let hv = hash_3d(self.seed ^ 0x517e, lx, ly, lz);
        if unit_f32(hv) > 0.22 {
            return;
        }
        let dx = lx as f32 + 0.5 - cx;
        let dz = lz as f32 + 0.5 - cz;
        // The vine sits outside the leaf, attached to it (attachment side faces the crown).
        let (vx, vz, attach) = if dx.abs() > dz.abs() {
            if dx > 0.0 {
                (lx + 1, lz, 3)
            } else {
                (lx - 1, lz, 1)
            }
        } else if dz > 0.0 {
            (lx, lz + 1, 0)
        } else {
            (lx, lz - 1, 2)
        };
        let len = 2 + (hv >> 20) as i32 % 7;
        for k in 0..len {
            let yy = ly - k;
            match w.buf.get(vx, yy, vz) {
                Some(s) if s.is_air() => w.put(vx, yy, vz, w.prio.b.vines[attach]),
                Some(_) => break,
                None => {}
            }
        }
    }

    /// Fallen logs, boulders and cacti on a coarse grid.
    fn debris_cell(&self, w: &mut Writer<'_>, wg: &WorldGenerator, fx: i32, fz: i32) {
        let b = &wg.blocks;
        let h = hash_2d(self.seed ^ 0xdeb, fx, fz);
        let ox = fx * DEBRIS_CELL + (h % DEBRIS_CELL as u64) as i32;
        let oz = fz * DEBRIS_CELL + ((h >> 8) % DEBRIS_CELL as u64) as i32;
        let u = unit_f32(h >> 20);
        let s = Self::sample_at(wg, ox, oz);
        if s.is_underwater() || s.slope > 0.7 {
            return;
        }
        let y = s.height_i();
        let o = w.buf.origin;
        if y + 4 < o.y || y - 2 > o.y + 15 {
            return;
        }
        let mut rng = Rng::new(hash_3d(self.seed ^ 0xdeb, ox, y, oz));
        match s.biome {
            Biome::HotDesert | Biome::DuneSea
                if u < 0.55 && matches!(s.surface, Surface::Sand | Surface::RedSand) =>
            {
                // Cacti: a few per cell, each on its own column.
                for k in 0..3 {
                    let cx = ox + rng.range_i32(-4, 4);
                    let cz = oz + rng.range_i32(-4, 4);
                    if k > 0 && !rng.chance(0.5) {
                        continue;
                    }
                    let cs = Self::sample_at(wg, cx, cz);
                    if cs.is_underwater() || !matches!(cs.surface, Surface::Sand | Surface::RedSand)
                    {
                        continue;
                    }
                    let ch = rng.range_i32(1, 4);
                    for j in 0..ch {
                        w.put(cx, cs.height_i() + j, cz, b.cactus);
                    }
                }
            }
            Biome::BroadleafForest
            | Biome::MixedForest
            | Biome::BorealForest
            | Biome::TemperateRainforest
            | Biome::BirchForest
            | Biome::TropicalRainforest
                if u < 0.12 && soil_ok(&s) =>
            {
                let wood = match s.biome {
                    Biome::BorealForest | Biome::TemperateRainforest => b.wood(Wood::Spruce),
                    Biome::BirchForest => b.wood(Wood::Birch),
                    _ => b.wood(Wood::Oak),
                };
                let len = rng.range_i32(3, 6);
                let along_x = rng.chance(0.5);
                for k in 0..len {
                    let (lx, lz) = if along_x { (ox + k, oz) } else { (ox, oz + k) };
                    let gs = Self::sample_at(wg, lx, lz);
                    if (gs.height_i() - y).abs() > 1 {
                        break;
                    }
                    w.put(lx, y, lz, if along_x { wood.log_x } else { wood.log_z });
                    if rng.chance(0.5) {
                        w.put(lx, y + 1, lz, b.moss_carpet);
                    } else if rng.chance(0.1) {
                        w.put(lx, y + 1, lz, b.brown_mushroom);
                    }
                }
            }
            Biome::Tundra
            | Biome::AlpineMeadow
            | Biome::BorealForest
            | Biome::TemperatePlains
            | Biome::Steppe
            | Biome::ColdDesert
            | Biome::Krummholz
                if u < 0.08 =>
            {
                let rock = if s.precipitation > 900.0 {
                    b.mossy_cobblestone
                } else if rng.chance(0.5) {
                    b.andesite
                } else {
                    b.cobblestone
                };
                let r = rng.range_f32(1.2, 2.4);
                let ri = r.ceil() as i32;
                for dy in -1..=ri {
                    for dz in -ri..=ri {
                        for dx in -ri..=ri {
                            let d = (dx * dx + dz * dz) as f32 + (dy as f32 * 1.3).powi(2);
                            if d <= r * r {
                                w.put(ox + dx, y + dy, oz + dz, rock);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

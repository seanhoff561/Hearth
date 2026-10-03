//! Surface features: trees (procedural per species and climate), plants, underwater flora,
//! fallen logs, boulders and cacti.
//!
//! Every feature is a pure function of its origin. A cube gathers the features whose bounds
//! intersect it and writes only its own blocks. Overlaps are resolved by a priority lattice
//! (terrain > logs > boulders > leaves by distance > plants > air, ties broken by state id), so
//! the result is identical in any order.

use hearth_math::ColumnPos;
use hearth_math::hash::{Rng, derive_seed, hash_2d, hash_3d, unit_f32};
use hearth_world::BlockStateId;

use super::blocks::{GenBlocks, Wood};
pub use super::succession::Remains;
use super::{ColumnData, CubeBuf, WorldGenerator};
use crate::noise::Perlin;
use crate::region::biome::Biome;
use crate::region::{ColumnSample, Surface};
use crate::vegetation::{BARE_YEARS, DisturbanceKind, Vegetation};

/// Size of the tree placement grid cells.
const TREE_CELL: i32 = 5;
/// Horizontal reach of the widest tree from its origin (an ancient oak's crown).
const TREE_REACH: i32 = 22;
/// Grid for fallen logs and boulders.
const DEBRIS_CELL: i32 = 12;
/// Years after a clearing before the shrubs come up.
const SHRUB_YEARS: f32 = 2.0;
/// Years ground counts as broken after a disturbance.
const DISTURBED_YEARS: f32 = 6.0;
/// Years the young trees on cleared ground take to close their canopy (after the first three).
const CLOSING_YEARS: f32 = 40.0;
/// Years a felled tree's gap lets more light onto the ground (from its second year).
const GAP_YEARS: f32 = 12.0;

/// The year a disturbed column's ground next changes: its phases (bare, herbs, shrubs, broken
/// ground) and the canopy closing over it a year at a time.
fn ground_change(kind: DisturbanceKind, year: f64, since: f32) -> f64 {
    let at = year - since as f64;
    let mut next = f64::INFINITY;
    for p in [BARE_YEARS, 1.0, SHRUB_YEARS, DISTURBED_YEARS] {
        if since < p {
            next = next.min(at + p as f64);
        }
    }
    let horizon = match kind {
        DisturbanceKind::Felled => GAP_YEARS,
        _ => 3.0 + CLOSING_YEARS,
    };
    if since < horizon {
        next = next.min(at + since.floor() as f64 + 1.0);
    }
    next
}

/// Feature placement.
#[derive(Debug, Clone)]
pub struct FeatureGen {
    seed: u64,
    cliff: Perlin,
    flower_patch: Perlin,
    /// The age of the stands across the land.
    stands: Perlin,
}

/// A tree of a real species where the generator grows one.
#[derive(Debug, Clone)]
pub struct PlacedTree {
    /// Index into the generator's forest.
    pub species: usize,
    pub stage: hearth_flora::Stage,
    pub template: std::sync::Arc<hearth_flora::TreeTemplate>,
    pub turn: hearth_flora::Turn,
    /// The block over the ground the trunk stands in.
    pub foot: [i32; 3],
    /// What of it is left: all of it, a snag, a charred trunk or a stump.
    pub remains: Remains,
    /// A young tree of the understory, where no canopy tree stands.
    pub understory: bool,
}

impl PlacedTree {
    /// Whether a part of the template is drawn, for what is left of the tree.
    pub fn shows(&self, c: [i16; 3], part: hearth_flora::Part) -> bool {
        use hearth_flora::Part;
        match self.remains {
            Remains::Living => true,
            Remains::Snag => !matches!(part, Part::Leaves),
            Remains::Charred => match part {
                Part::Leaves => false,
                Part::Branch { thickness, .. } => thickness > 2,
                Part::Log { .. } => true,
            },
            Remains::Stump => c[1] <= 0 && !matches!(part, Part::Leaves),
        }
    }

    /// The block a part of it is drawn with (charred where fire killed it).
    pub fn state(&self, forest: &crate::trees::Forest, part: hearth_flora::Part) -> BlockStateId {
        match (&forest.charred, self.remains) {
            (Some(c), Remains::Charred) => c.state(part),
            _ => forest.blocks[self.species].state(part),
        }
    }

    /// Its blocks where they stand (what is left of it), as template parts turned with it.
    pub fn blocks(&self) -> impl Iterator<Item = (hearth_math::BlockPos, hearth_flora::Part)> + '_ {
        let shown = self
            .template
            .blocks
            .iter()
            .filter(|(c, part)| self.shows(*c, *part));
        shown.map(|(c, part)| {
            let (dx, dz) = self
                .turn
                .apply(c[0] as i32, c[2] as i32, self.template.corner);
            (
                hearth_math::BlockPos::new(
                    self.foot[0] + dx,
                    self.foot[1] + c[1] as i32,
                    self.foot[2] + dz,
                ),
                self.turn.part(*part),
            )
        })
    }
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
    /// On prop roots in the shallows of tropical coasts.
    Mangrove,
}

impl TreeKind {
    fn wood(self) -> Wood {
        match self {
            TreeKind::Birch => Wood::Birch,
            TreeKind::Spruce | TreeKind::GiantSpruce | TreeKind::Krummholz => Wood::Spruce,
            TreeKind::Mangrove => Wood::Mangrove,
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
            TreeKind::Mangrove => (5, 11),
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
        if b.mangrove_roots.contains(&s) {
            return 60;
        }
        for w in [&b.oak, &b.birch, &b.spruce, &b.mangrove] {
            if s == w.log_y || s == w.log_x || s == w.log_z {
                return 60;
            }
            if let Some(d) = w.leaves.iter().position(|l| *l == s) {
                return 40 - d as u32; // distance 1 → 40, distance 7 → 34
            }
        }
        match b.tree_part.get(s.0 as usize) {
            Some(1) => return 60,
            Some(2) => return 40,
            Some(3) => return 10,
            _ => {}
        }
        if s == b.mossy_cobblestone || s == b.cobblestone || s == b.cactus {
            return 50;
        }
        if is_plant(b, s) || b.is_loose_stone(s) {
            return 10;
        }
        // Terrain, water and anything else: never replaced by features.
        1000
    }
}

/// Chance of loose stones on a column's surface.
fn loose_stone_chance(s: &ColumnSample) -> f32 {
    let base = match s.surface {
        Surface::Stone
        | Surface::Gravel
        | Surface::Sandstone
        | Surface::RedSandstone
        | Surface::Tuff => 0.1,
        Surface::CoarseDirt => 0.06,
        Surface::Sand | Surface::RedSand => 0.004,
        Surface::Snow
        | Surface::Ice
        | Surface::Mud
        | Surface::Clay
        | Surface::Dirt
        | Surface::Moss => 0.0,
        _ => 0.006,
    };
    let place = match s.biome {
        Biome::HotDesert | Biome::ColdDesert | Biome::Mesa => 2.5,
        Biome::AlpineMeadow | Biome::AlpineRock | Biome::Krummholz | Biome::Tundra => 2.5,
        Biome::StonyShore => 3.0,
        Biome::DuneSea | Biome::Beach => 0.3,
        _ => 1.0,
    };
    (base * place * (1.0 + 4.0 * s.slope.min(1.0))).min(0.35)
}

fn is_plant(b: &GenBlocks, s: BlockStateId) -> bool {
    s == b.short_grass
        || s == b.fern
        || s == b.short_dry_grass
        || s == b.dead_bush
        || s == b.nettle
        || s == b.bramble
        || b.hazel.contains(&s)
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

/// Blocks that may take the place of water: water plants, and mangrove trunks and roots.
fn is_aquatic(b: &GenBlocks, s: BlockStateId) -> bool {
    b.is_water_wood(s)
        || s == b.seagrass
        || b.tall_seagrass.contains(&s)
        || s == b.kelp
        || s == b.kelp_plant
        || s == b.coral
        || s == b.seaweed
        || s == b.sea_pen
        || s == b.mangrove_roots[1]
        || s == b.mangrove.log_y
}

/// Where a tree's blocks go: a cube being generated, or the distant terrain's map of the
/// canopy. Trees are grown by the same code for both, so the trees of the distant terrain are
/// the ones the cubes will have.
pub trait TreeSink {
    /// Inclusive bounds (min, max) of the blocks the sink takes; trees and their parts wholly
    /// outside are skipped.
    fn bounds(&self) -> ([i32; 3], [i32; 3]);
    /// The block already at a position, where the sink knows it (vines hang only into air).
    fn get(&self, x: i32, y: i32, z: i32) -> Option<BlockStateId>;
    fn put(&mut self, x: i32, y: i32, z: i32, s: BlockStateId);
    /// The sink wants only each column's crown and trunk (the distant terrain): trees grown
    /// from templates then give `crown` and `trunk` per column instead of every block.
    fn crowns_only(&self) -> bool {
        false
    }
    /// Foliage over a column from `bottom` to under `top`.
    fn crown(&mut self, _x: i32, _z: i32, _bottom: i32, _top: i32, _leaves: BlockStateId) {}
    /// An upright trunk over a column from `bottom` to under `top`.
    fn trunk(&mut self, _x: i32, _z: i32, _bottom: i32, _top: i32, _log: BlockStateId) {}
}

/// Writes blocks into the cube using the priority lattice.
struct Writer<'a> {
    buf: &'a mut CubeBuf,
    prio: Priorities<'a>,
}

impl TreeSink for Writer<'_> {
    fn bounds(&self) -> ([i32; 3], [i32; 3]) {
        let o = self.buf.origin;
        ([o.x, o.y, o.z], [o.x + 15, o.y + 15, o.z + 15])
    }

    fn get(&self, x: i32, y: i32, z: i32) -> Option<BlockStateId> {
        self.buf.get(x, y, z)
    }

    #[inline]
    fn put(&mut self, x: i32, y: i32, z: i32, s: BlockStateId) {
        let Some(i) = self.buf.idx(x, y, z) else {
            return;
        };
        let cur = self.buf.states[i];
        // Water plants grow in water, and mangroves stand in it; nothing else replaces it.
        if cur == self.prio.b.water {
            if is_aquatic(self.prio.b, s) {
                self.buf.states[i] = s;
            }
            return;
        }
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
    ) || (s.biome == Biome::Beach && s.surface == Surface::Sand)
}

impl FeatureGen {
    pub fn new(seed: u64) -> Self {
        Self {
            seed: derive_seed(seed, "features"),
            cliff: Perlin::new(derive_seed(seed, "cliff3d")),
            flower_patch: Perlin::new(derive_seed(seed, "flowers")),
            stands: Perlin::new(derive_seed(seed, "stands")),
        }
    }

    /// 3D noise in [-1, 1] used for cliff overhangs.
    #[inline]
    pub fn cliff_noise(&self, x: i32, y: i32, z: i32) -> f32 {
        let f = 1.0 / 11.0;
        self.cliff
            .noise3(x as f64 * f, y as f64 * f * 1.4, z as f64 * f, 0) as f32
    }

    /// Places all features intersecting the cube, as the vegetation has grown. Returns the year
    /// the cube's trees or ground next change (infinity: never).
    pub fn place(
        &self,
        buf: &mut CubeBuf,
        wg: &WorldGenerator,
        col: &ColumnData,
        veg: &Vegetation,
    ) -> f64 {
        let b = &wg.blocks;
        let o = buf.origin;
        let mut w = Writer {
            buf,
            prio: Priorities { b },
        };
        let mut next = f64::INFINITY;
        // Plants and underwater flora of this cube's own columns.
        for lz in 0..16 {
            for lx in 0..16 {
                let s = col.at(lx as usize, lz as usize);
                let n = self.decorate_column(&mut w, wg, col, o.x + lx, o.z + lz, s, veg);
                next = next.min(n);
            }
        }
        // Trees whose origin cell is near the cube.
        let (x0, x1) = (o.x - TREE_REACH, o.x + 15 + TREE_REACH);
        let (z0, z1) = (o.z - TREE_REACH, o.z + 15 + TREE_REACH);
        let sample = |x: i32, z: i32| Self::sample_at(wg, x, z);
        for fz in z0.div_euclid(TREE_CELL)..=z1.div_euclid(TREE_CELL) {
            for fx in x0.div_euclid(TREE_CELL)..=x1.div_euclid(TREE_CELL) {
                next = next.min(self.tree_cell(&mut w, wg, veg, fx, fz, &sample));
            }
        }
        for fz in (o.z - 8).div_euclid(DEBRIS_CELL)..=(o.z + 23).div_euclid(DEBRIS_CELL) {
            for fx in (o.x - 8).div_euclid(DEBRIS_CELL)..=(o.x + 23).div_euclid(DEBRIS_CELL) {
                self.debris_cell(&mut w, wg, fx, fz);
            }
        }
        next
    }

    fn sample_at(wg: &WorldGenerator, x: i32, z: i32) -> ColumnSample {
        let c = wg.column(ColumnPos::new(x >> 4, z >> 4));
        *c.at((x & 15) as usize, (z & 15) as usize)
    }

    /// The plants of a column; returns the year its ground next changes.
    #[allow(clippy::too_many_arguments)]
    fn decorate_column(
        &self,
        w: &mut Writer<'_>,
        wg: &WorldGenerator,
        col: &ColumnData,
        x: i32,
        z: i32,
        s: &ColumnSample,
        veg: &Vegetation,
    ) -> f64 {
        let b = &wg.blocks;
        let top = s.height_i();
        let o = w.buf.origin;
        // Cleared and burned ground (and the gap of a felled tree), as long ago as it was.
        let disturbed = if s.is_underwater() {
            None
        } else {
            veg.ground(x, z)
        };
        let mut next = f64::INFINITY;
        if let Some((kind, since)) = disturbed
            && top - 1 <= o.y + 15
            && top + 2 >= o.y
        {
            next = ground_change(kind, veg.year, since);
        }
        let bare =
            matches!(disturbed, Some((DisturbanceKind::Burned, since)) if since < BARE_YEARS);
        if bare
            && let Some(g) = w.buf.get(x, top - 1, z)
            && b.is_plantable(g)
        {
            w.buf.set(x, top - 1, z, b.burnt_ground);
        }
        // Quick reject: nothing this column places can reach the cube.
        let reach_top = if s.is_underwater() {
            s.water_i()
        } else {
            top + 2
        };
        if reach_top < o.y || top > o.y + 15 || bare {
            return next;
        }
        let h = hash_2d(self.seed, x, z);
        let r = unit_f32(h);
        let r2 = unit_f32(h.rotate_left(21));
        if s.is_underwater() {
            let depth = s.water_i() - top;
            // Reeds and lilies in fresh water's shallows.
            let fresh = matches!(s.biome, Biome::Wetland | Biome::Lake | Biome::River);
            if fresh && depth <= 3 && self.aquatic(w, wg, x, z, top, s, depth) {
                return next;
            }
            let floor_ok = matches!(
                s.surface,
                Surface::Sand | Surface::Gravel | Surface::Dirt | Surface::Clay
            );
            // Kelp forests hold to rocky floors in cool, clear water, thickest in patches.
            let rocky = matches!(s.surface, Surface::Gravel | Surface::Stone);
            let kelp_water = (5.0..20.0).contains(&s.sea_temperature);
            let kelp_patch = self
                .flower_patch
                .noise2(x as f64 / 40.0, z as f64 / 40.0, 3) as f32;
            let kelp_chance = if rocky {
                if kelp_patch > 0.1 { 0.4 } else { 0.08 }
            } else {
                0.02
            };
            match s.biome {
                Biome::WarmShallows if s.surface == Surface::Coral => {
                    // Fans and branching corals on the reef crest and slopes.
                    if r < 0.3 {
                        w.put(x, top, z, b.coral);
                    }
                }
                Biome::ColdSea | Biome::TemperateSea | Biome::PolarSea
                    if rocky && depth <= 2 && s.temperature > -2.0 && r < 0.3 =>
                {
                    // Wrack on the rocks of the shore.
                    w.put(x, top, z, b.seaweed);
                }
                Biome::ColdSea | Biome::TemperateSea
                    if kelp_water && depth > 3 && depth < 40 && r < kelp_chance =>
                {
                    let len = (3 + (r2 * 18.0) as i32).min(depth - 2);
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
                // In the dark of the deep floor, sea pens glowing where they are touched.
                Biome::DeepOcean | Biome::Trench if floor_ok && r < 0.06 => {
                    w.put(x, top, z, b.sea_pen);
                }
                _ => {}
            }
            return next;
        }
        // Only plant on exposed soil (caves or cliffs may have removed it).
        let ground = w.buf.get(x, top - 1, z);
        let above_air = w.buf.get(x, top, z).is_none_or(|s| s.is_air());
        // Loose stones of the local bedrock: on bare and thin ground, scree, deserts and high
        // ground, now and then anywhere.
        if above_air
            && ground.is_some_and(|g| !g.is_air() && !b.is_loose_stone(g))
            && unit_f32(h.rotate_left(43)) < loose_stone_chance(s)
            && let Some(c) = b.cobbles_for(wg.rock_at(x, top - 1, z))
        {
            w.put(x, top, z, c);
            return next;
        }
        let soil_block = ground.is_none_or(|g| b.is_plantable(g));
        if !above_air || !soil_block {
            return next;
        }
        // Where the year is mostly frozen only the cold's own species grow (the content's, by
        // their climates: dwarf shrubs, sedges, mosses, lichens); the generic grasses, ferns and
        // flowers do not.
        let frozen = s.temperature < -0.5;
        let flower_n = self
            .flower_patch
            .noise2(x as f64 / 24.0, z as f64 / 24.0, 0) as f32;
        let grassy = matches!(s.surface, Surface::Grass | Surface::Podzol | Surface::Moss);
        let tall = |w: &mut Writer<'_>, pair: [BlockStateId; 2]| {
            w.put(x, top, z, pair[0]);
            w.put(x, top + 1, z, pair[1]);
        };
        if frozen
            && !matches!(
                s.biome,
                Biome::Tundra | Biome::BorealForest | Biome::SnowyTaiga
            )
        {
            return next;
        }
        match s.biome {
            Biome::HotDesert | Biome::DuneSea | Biome::Mesa | Biome::ColdDesert => {
                // The desert's own shrubs, cacti and tufts where anything grows; dead bushes
                // between.
                if self.understory(w, wg, x, z, top, s, flower_n, disturbed) {
                    return next;
                }
                if r < 0.012 {
                    w.put(x, top, z, b.dead_bush);
                }
            }
            Biome::SaltMarsh => {
                // The marsh's own: cordgrass meadows, glasswort on the open mud, sea lavender.
                if self.understory(w, wg, x, z, top, s, flower_n, disturbed) {
                    return next;
                }
                if r < 0.3 {
                    tall(w, b.cordgrass);
                }
            }
            // The dunes' grasses and creepers, the rocks' cushions, sparse.
            Biome::Beach | Biome::StonyShore => {
                if r < 0.25 && self.understory(w, wg, x, z, top, s, flower_n, disturbed) {
                    return next;
                }
            }
            Biome::Mangrove => {}
            Biome::Steppe | Biome::Savanna => {
                // The steppe's and the savanna's own grasses, shrubs and flowers first; dry
                // grass between.
                if (grassy || matches!(s.surface, Surface::CoarseDirt))
                    && self.understory(w, wg, x, z, top, s, flower_n, disturbed)
                {
                    return next;
                }
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
            Biome::SaltFlat | Biome::Glacier | Biome::IceSheet | Biome::Volcanic => {}
            // Cushions and the flowers of the screes, in the rock's gravelly pockets.
            Biome::AlpineRock => {
                if (grassy || matches!(s.surface, Surface::Gravel | Surface::CoarseDirt))
                    && r < 0.35
                    && self.understory(w, wg, x, z, top, s, flower_n, disturbed)
                {
                    return next;
                }
            }
            Biome::AlpineMeadow => {
                // The meadow's own grasses, cushions and flowers first.
                if (grassy || matches!(s.surface, Surface::CoarseDirt | Surface::SnowGrass))
                    && self.understory(w, wg, x, z, top, s, flower_n, disturbed)
                {
                    return next;
                }
                if grassy {
                    if flower_n > 0.35 && r < 0.35 {
                        w.put(x, top, z, b.flowers_alpine[(r2 * 2.0) as usize % 2]);
                    } else if r < 0.4 {
                        w.put(x, top, z, b.short_grass);
                    }
                }
            }
            Biome::Tundra => {
                // The tundra's own: dwarf shrubs, sedges, cushions, mosses and lichens; grass
                // where it is mild enough.
                let open = grassy || matches!(s.surface, Surface::CoarseDirt | Surface::SnowGrass);
                if open && self.understory(w, wg, x, z, top, s, flower_n, disturbed) {
                    return next;
                }
                if grassy && !frozen && r < 0.15 {
                    w.put(x, top, z, b.short_grass);
                }
            }
            Biome::TropicalRainforest | Biome::TemperateRainforest => {
                // The tropical forest's own floor first (ferns, gingers, broad leaves).
                if s.biome == Biome::TropicalRainforest
                    && (grassy || s.surface == Surface::Moss)
                    && self.understory(w, wg, x, z, top, s, flower_n, disturbed)
                {
                    return next;
                }
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
                // The taiga's own floor first (berries, heaths, feather moss, lichens); ferns
                // and grass where it is mild enough.
                if (grassy || s.surface == Surface::SnowGrass)
                    && self.understory(w, wg, x, z, top, s, flower_n, disturbed)
                {
                    return next;
                }
                if frozen {
                    return next;
                }
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
                // The marsh's and the bog's own plants first.
                if self.understory(w, wg, x, z, top, s, flower_n, disturbed) {
                    return next;
                }
                if r < 0.35 {
                    w.put(x, top, z, b.short_grass);
                } else if r < 0.45 {
                    tall(w, b.tall_grass);
                }
                self.sugar_cane(w, wg, col, x, z, top, r2);
            }
            _ => {
                // Temperate grasslands and forests: the understory's species, then grass and
                // flowers in clusters (the Mediterranean's maquis on its stony ground too).
                let forest = matches!(
                    s.biome,
                    Biome::BroadleafForest | Biome::BirchForest | Biome::MixedForest
                );
                let open = grassy
                    || (s.biome == Biome::MediterraneanScrub
                        && matches!(s.surface, Surface::CoarseDirt));
                if open && self.understory(w, wg, x, z, top, s, flower_n, disturbed) {
                    return next;
                }
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
        next
    }

    /// A plant of the understory's species on a column, if one grows there. Whether one did.
    /// On cleared and burned ground the herbs of open, broken ground come at once and the
    /// shrubs from the second year, until the young trees close over it; a felled tree's gap
    /// lets light onto the ground for some years.
    #[allow(clippy::too_many_arguments)]
    fn understory(
        &self,
        w: &mut Writer<'_>,
        wg: &WorldGenerator,
        x: i32,
        z: i32,
        top: i32,
        s: &ColumnSample,
        flower_n: f32,
        disturbed: Option<(DisturbanceKind, f32)>,
    ) -> bool {
        let forest = &wg.forest;
        if forest.understory.is_empty() {
            return false;
        }
        let climate = crate::trees::PlaceClimate {
            mean_c: s.temperature,
            warm_c: s.t_warm,
            cold_c: 2.0 * s.temperature - s.t_warm,
            precip_mm: s.precipitation,
            class: s.climate,
            biome: s.biome,
            wet: s.biome == Biome::Wetland,
            realm: s.realm,
        };
        let wet = matches!(s.biome, Biome::Wetland | Biome::Oasis)
            || s.river
                .is_some_and(|r| r.distance < r.width * 0.5 + 14.0 && s.height - r.level < 2.5);
        // Light under the canopy: closed where trees are dense and the stand has grown.
        let stand = self.stand_age(x, z);
        let density = (s.tree_density * 1.9).min(1.0);
        let mut closure = density * (stand / 40.0).min(1.0);
        let mut broken = false;
        let mut shrubs = true;
        match disturbed {
            Some((DisturbanceKind::Felled, since)) if (1.0..GAP_YEARS).contains(&since) => {
                closure = closure.min(0.3 + 0.05 * since.floor());
                broken = since < DISTURBED_YEARS;
            }
            Some((DisturbanceKind::Cleared | DisturbanceKind::Burned, since)) => {
                closure = density * ((since.floor() - 3.0) / CLOSING_YEARS).clamp(0.0, 1.0);
                broken = since < DISTURBED_YEARS;
                shrubs = since >= SHRUB_YEARS;
            }
            _ => {}
        }
        let light = 1.0 - 0.85 * closure;
        let ground = crate::trees::PlaceGround {
            wet,
            rich: flower_n > 0.1 || wet,
            acid: matches!(s.surface, Surface::Podzol | Surface::Moss) || s.precipitation > 1100.0,
            disturbed: broken || s.slope > 0.35 || (s.tree_density > 0.3 && closure < 0.45),
            shrubs,
        };
        let seed = self.seed;
        let roll = unit_f32(hash_2d(seed ^ 0x0de5, x, z));
        let patch = |i: usize, size: f32| {
            let size = size.max(2.0) as i32;
            unit_f32(hash_3d(
                seed ^ 0x9a7c,
                x.div_euclid(size),
                i as i32,
                z.div_euclid(size),
            ))
        };
        let Some(i) = forest.choose_under(&climate, &ground, light, roll, patch, None) else {
            return false;
        };
        let p = &forest.understory[i];
        w.put(x, top, z, p.lower);
        if let Some(up) = p.upper {
            w.put(x, top + 1, z, up);
        }
        true
    }

    /// A plant of fresh water's shallows over a bottom at `top`, the water `depth` blocks deep:
    /// one afloat on the surface, or one standing up out of it on its stalks. Whether one grew.
    #[allow(clippy::too_many_arguments)]
    fn aquatic(
        &self,
        w: &mut Writer<'_>,
        wg: &WorldGenerator,
        x: i32,
        z: i32,
        top: i32,
        s: &ColumnSample,
        depth: i32,
    ) -> bool {
        use hearth_content::schema::flora::WaterHabit;
        let forest = &wg.forest;
        let climate = crate::trees::PlaceClimate {
            mean_c: s.temperature,
            warm_c: s.t_warm,
            cold_c: 2.0 * s.temperature - s.t_warm,
            precip_mm: s.precipitation,
            class: s.climate,
            biome: s.biome,
            wet: true,
            realm: s.realm,
        };
        let ground = crate::trees::PlaceGround {
            wet: true,
            rich: true,
            acid: matches!(s.surface, Surface::Podzol | Surface::Moss),
            disturbed: false,
            shrubs: true,
        };
        let seed = self.seed;
        let roll = unit_f32(hash_2d(seed ^ 0xa9a7, x, z));
        let patch = |i: usize, size: f32| {
            let size = size.max(2.0) as i32;
            unit_f32(hash_3d(
                seed ^ 0x9a7c,
                x.div_euclid(size),
                i as i32,
                z.div_euclid(size),
            ))
        };
        let Some(i) = forest.choose_under(&climate, &ground, 1.0, roll, patch, Some(depth as f32))
        else {
            return false;
        };
        let p = &forest.understory[i];
        let surface = s.water_i();
        match p.understory.water {
            Some(WaterHabit::Floating { .. }) => w.put(x, surface, z, p.lower),
            Some(WaterHabit::Emergent { .. }) => {
                let Some(stem) = p.stem else {
                    return false;
                };
                for y in top..surface {
                    w.put(x, y, z, stem);
                }
                w.put(x, surface, z, p.lower);
                if let Some(up) = p.upper {
                    w.put(x, surface + 1, z, up);
                }
            }
            None => return false,
        }
        true
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

    /// Grows every tree that reaches into the sink's bounds, as the cubes grow them (distant
    /// terrain: the sink maps the canopy).
    pub fn grow_trees<S: TreeSink>(&self, sink: &mut S, wg: &WorldGenerator, veg: &Vegetation) {
        let (lo, hi) = sink.bounds();
        let (x0, x1) = (lo[0] - TREE_REACH, hi[0] + TREE_REACH);
        let (z0, z1) = (lo[2] - TREE_REACH, hi[2] + TREE_REACH);
        let planet = wg.planet();
        let sample = |x: i32, z: i32| wg.terrain.sample(planet.wrap_x(x), z);
        for fz in z0.div_euclid(TREE_CELL)..=z1.div_euclid(TREE_CELL) {
            for fx in x0.div_euclid(TREE_CELL)..=x1.div_euclid(TREE_CELL) {
                self.tree_cell(sink, wg, veg, fx, fz, &sample);
            }
        }
    }

    /// The tree of a tree cell, written into the sink; returns the year it next changes.
    fn tree_cell<S: TreeSink>(
        &self,
        w: &mut S,
        wg: &WorldGenerator,
        veg: &Vegetation,
        fx: i32,
        fz: i32,
        sample: &impl Fn(i32, i32) -> ColumnSample,
    ) -> f64 {
        let Some((h, ox, oz, s)) = self.cell_site(fx, fz, sample) else {
            return f64::INFINITY;
        };
        let base = s.height_i();
        let (lo, hi) = w.bounds();
        if let Some((tree, next)) = self.species_cell(wg, veg, ox, oz, &s, h) {
            if let Some(t) = &tree {
                self.place_template(w, wg, t);
            }
            // Its changes matter where the tallest tree could grow into the sink.
            let reaches =
                base - 4 <= hi[1] && base + wg.forest.tallest_m.ceil() as i32 + 4 >= lo[1];
            return if reaches { next } else { f64::INFINITY };
        }
        // The old shapes (where no species fits) are gone where the ground was cleared or
        // burned, and do not grow back; the beaches have none.
        if s.biome == Biome::Beach {
            return f64::INFINITY;
        }
        if veg
            .at(ox, oz)
            .iter()
            .any(|e| e.kind != DisturbanceKind::Felled)
        {
            return f64::INFINITY;
        }
        let kind = self.choose_tree(&s, h);
        let (reach, height) = kind.extent();
        if base + height < lo[1] || base - 3 > hi[1] {
            return f64::INFINITY;
        }
        if ox + reach < lo[0] || ox - reach > hi[0] || oz + reach < lo[2] || oz - reach > hi[2] {
            return f64::INFINITY;
        }
        let mut rng = Rng::new(hash_3d(self.seed, ox, base, oz));
        self.grow(w, &wg.blocks, kind, ox, base, oz, &mut rng, &s);
        f64::INFINITY
    }

    /// Where a tree cell's tree would stand, if the place takes a tree: its hash, origin and
    /// column.
    fn cell_site(
        &self,
        fx: i32,
        fz: i32,
        sample: &impl Fn(i32, i32) -> ColumnSample,
    ) -> Option<(u64, i32, i32, ColumnSample)> {
        let h = hash_2d(self.seed ^ 0x7ee5, fx, fz);
        let ox = fx * TREE_CELL + (h % TREE_CELL as u64) as i32;
        let oz = fz * TREE_CELL + ((h >> 8) % TREE_CELL as u64) as i32;
        // The upper hash bits (the lower ones placed the tree in its cell).
        let u = unit_f32(h.rotate_left(24));
        let s = sample(ox, oz);
        // Trees are denser than one per cell only where density is ~1. Mangroves stand in up
        // to two blocks of water.
        let wet =
            s.is_underwater() && !(s.biome == Biome::Mangrove && s.water_i() - s.height_i() <= 2);
        if u >= s.tree_density * 0.95 || wet || !soil_ok(&s) || s.slope > 0.95 {
            return None;
        }
        Some((h, ox, oz, s))
    }

    /// The tree of a real species (or what is left of one) with wood at `p`, as the cubes grow
    /// it.
    pub fn tree_at(
        &self,
        wg: &WorldGenerator,
        veg: &Vegetation,
        p: hearth_math::BlockPos,
    ) -> Option<PlacedTree> {
        let planet = wg.planet();
        let sample = |x: i32, z: i32| wg.terrain.sample(planet.wrap_x(x), z);
        for fz in
            (p.z - TREE_REACH).div_euclid(TREE_CELL)..=(p.z + TREE_REACH).div_euclid(TREE_CELL)
        {
            for fx in
                (p.x - TREE_REACH).div_euclid(TREE_CELL)..=(p.x + TREE_REACH).div_euclid(TREE_CELL)
            {
                let Some((h, ox, oz, s)) = self.cell_site(fx, fz, &sample) else {
                    continue;
                };
                let Some((Some(t), _)) = self.species_cell(wg, veg, ox, oz, &s, h) else {
                    continue;
                };
                let wood = t
                    .blocks()
                    .any(|(q, part)| q == p && !matches!(part, hearth_flora::Part::Leaves));
                if wood {
                    return Some(t);
                }
            }
        }
        None
    }

    /// The trees of real species (and what is left of them) with their foot in a rectangle
    /// (inclusive), as the vegetation has grown them.
    pub fn trees_in(
        &self,
        wg: &WorldGenerator,
        veg: &Vegetation,
        min: (i32, i32),
        max: (i32, i32),
    ) -> Vec<PlacedTree> {
        let planet = wg.planet();
        let sample = |x: i32, z: i32| wg.terrain.sample(planet.wrap_x(x), z);
        let mut out = Vec::new();
        for fz in min.1.div_euclid(TREE_CELL)..=max.1.div_euclid(TREE_CELL) {
            for fx in min.0.div_euclid(TREE_CELL)..=max.0.div_euclid(TREE_CELL) {
                let Some((h, ox, oz, s)) = self.cell_site(fx, fz, &sample) else {
                    continue;
                };
                if ox < min.0 || ox > max.0 || oz < min.1 || oz > max.1 {
                    continue;
                }
                if let Some((Some(t), _)) = self.species_cell(wg, veg, ox, oz, &s, h) {
                    out.push(t);
                }
            }
        }
        out
    }

    /// Age (years) of the stand at a place: young woods and old growth in patches a few
    /// hundred metres across.
    pub fn stand_age(&self, x: i32, z: i32) -> f32 {
        let f = 1.0 / 380.0;
        let n = self.stands.noise2(x as f64 * f, z as f64 * f, 0) as f32;
        let u = ((n + 1.0) * 0.5).clamp(0.0, 1.0);
        15.0 + 285.0 * u * u
    }

    /// What the distant terrain shows where it does not grow each tree: the species most
    /// likely at a place (by `roll`), the height of its crown's top (blocks) and how much of
    /// the ground the crowns cover. None where no species fits the climate.
    pub fn expected_canopy(
        &self,
        wg: &WorldGenerator,
        veg: &Vegetation,
        s: &ColumnSample,
        x: i32,
        z: i32,
        roll: f32,
    ) -> Option<(usize, i32, f32)> {
        let forest = &wg.forest;
        let climate = crate::trees::PlaceClimate {
            mean_c: s.temperature,
            warm_c: s.t_warm,
            cold_c: 2.0 * s.temperature - s.t_warm,
            precip_mm: s.precipitation,
            class: s.climate,
            biome: s.biome,
            wet: s.biome == Biome::Wetland,
            realm: s.realm,
        };
        let mut sp = forest.choose(&climate, 0.0, roll)?;
        let species = &forest.templates.species[sp];
        let mut age =
            (self.stand_age(x, z) * 0.9 + veg.year as f32).min(species.lifespan_years * 0.9);
        // Crowns are spaced to close where the trees are dense (`species_tree`).
        let mut cover = (s.tree_density * 0.95 * 2.0).min(1.0);
        // Cleared and burned land: the young stand growing back.
        if let Some((kind, since)) = veg.ground(x, z)
            && kind != DisturbanceKind::Felled
        {
            sp = forest.choose_open(&climate, roll).unwrap_or(sp);
            age = (since - 2.5).max(0.0);
            cover *= (age / 12.0).min(1.0);
        }
        let h = forest.templates.species[sp].height_at(age).round() as i32;
        Some((sp, h.max(2), cover))
    }

    /// The tree of a real species in a tree cell, as the vegetation has grown, and the year it
    /// next changes: None where no species fits the place (the old shapes stand there), a
    /// tree of None where one fits but no tree stands in the cell.
    fn species_cell(
        &self,
        wg: &WorldGenerator,
        veg: &Vegetation,
        ox: i32,
        oz: i32,
        s: &ColumnSample,
        h: u64,
    ) -> Option<(Option<PlacedTree>, f64)> {
        let forest = &wg.forest;
        if forest.niches.is_empty() {
            return None;
        }
        let wet = matches!(s.biome, Biome::Wetland | Biome::Oasis)
            || s.river
                .is_some_and(|r| r.distance < r.width * 0.5 + 14.0 && s.height - r.level < 2.5);
        let climate = crate::trees::PlaceClimate {
            mean_c: s.temperature,
            warm_c: s.t_warm,
            cold_c: 2.0 * s.temperature - s.t_warm,
            precip_mm: s.precipitation,
            class: s.climate,
            biome: s.biome,
            wet,
            realm: s.realm,
        };
        let site = super::succession::SiteInput {
            forest,
            climate: &climate,
            veg,
            h,
            stand: self.stand_age(ox, oz),
            x: ox,
            z: oz,
            cell_area: (TREE_CELL * TREE_CELL) as f32,
        }
        .now()?;
        let foot = [ox, s.height_i(), oz];
        let tree = site.tree.map(|t| PlacedTree {
            species: t.species,
            stage: t.stage,
            template: forest.templates.get(t.species, t.stage, t.variant),
            turn: t.turn,
            foot,
            remains: t.remains,
            understory: t.understory,
        });
        Some((tree, site.next_change))
    }

    /// Writes a tree from its template, turned, with its foot where it stands.
    fn place_template<S: TreeSink>(&self, w: &mut S, wg: &WorldGenerator, tree: &PlacedTree) {
        let t = &tree.template;
        let turn = tree.turn;
        let [x, y, z] = tree.foot;
        let (lo, hi) = w.bounds();
        let reach = t.reach();
        if y + t.max[1] < lo[1] || y + t.min[1] > hi[1] {
            return;
        }
        if x + reach < lo[0] || x - reach > hi[0] || z + reach < lo[2] || z - reach > hi[2] {
            return;
        }
        let forest = &wg.forest;
        if w.crowns_only() {
            if tree.remains == Remains::Living {
                let leaves = forest.blocks[tree.species].leaves;
                for c in &t.crowns {
                    let (dx, dz) = turn.apply(c[0] as i32, c[1] as i32, t.corner);
                    w.crown(x + dx, z + dz, y + c[2] as i32, y + c[3] as i32 + 1, leaves);
                }
            }
            if tree.remains != Remains::Stump {
                let log = tree.state(forest, hearth_flora::Part::Log { axis: 1 });
                for c in &t.trunks {
                    let (dx, dz) = turn.apply(c[0] as i32, c[1] as i32, t.corner);
                    w.trunk(x + dx, z + dz, y + c[2] as i32, y + c[3] as i32 + 1, log);
                }
            }
            return;
        }
        for (c, part) in &t.blocks {
            if !tree.shows(*c, *part) {
                continue;
            }
            let by = y + c[1] as i32;
            if by < lo[1] || by > hi[1] {
                continue;
            }
            let (dx, dz) = turn.apply(c[0] as i32, c[2] as i32, t.corner);
            let (bx, bz) = (x + dx, z + dz);
            if bx < lo[0] || bx > hi[0] || bz < lo[2] || bz > hi[2] {
                continue;
            }
            w.put(bx, by, bz, tree.state(forest, turn.part(*part)));
        }
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
            Biome::Mangrove => Mangrove,
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
    fn grow<S: TreeSink>(
        &self,
        w: &mut S,
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
            TreeKind::Mangrove => {
                // The trunk starts above the tide on a cage of arching prop roots.
                let water_top = s.water_i();
                let root = |yy: i32| {
                    if yy < water_top {
                        b.mangrove_roots[1]
                    } else {
                        b.mangrove_roots[0]
                    }
                };
                let base = y + 2;
                for k in 0..2 {
                    logs.push((x, y + k, z, root(y + k)));
                }
                let h = rng.range_i32(3, 5) + (vigor * 1.5) as i32;
                for k in 0..h {
                    logs.push((x, base + k, z, wood.log_y));
                }
                let roots = rng.range_i32(4, 7);
                for r in 0..roots {
                    let a =
                        r as f32 / roots as f32 * std::f32::consts::TAU + rng.range_f32(-0.3, 0.3);
                    let reach = rng.range_f32(1.6, 3.2);
                    let start = base + rng.range_i32(0, 2);
                    // An arch from the trunk out and down into the mud.
                    let steps = (reach * 2.0).ceil() as i32;
                    for k in 1..=steps {
                        let t = k as f32 / steps as f32;
                        let rx = (x as f32 + 0.5 + a.cos() * reach * t).floor() as i32;
                        let rz = (z as f32 + 0.5 + a.sin() * reach * t).floor() as i32;
                        let ry = start - ((start - y + 1) as f32 * t * t).round() as i32;
                        logs.push((rx, ry, rz, root(ry)));
                    }
                }
                let top = (base + h) as f32;
                blobs.push((
                    x as f32 + 0.5,
                    top,
                    z as f32 + 0.5,
                    rng.range_f32(2.6, 3.4),
                    1.7,
                ));
                blobs.push((
                    x as f32 + 0.5 + rng.range_f32(-1.5, 1.5),
                    top - 1.0,
                    z as f32 + 0.5 + rng.range_f32(-1.5, 1.5),
                    rng.range_f32(2.0, 2.6),
                    1.3,
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
        let (lo, hi) = w.bounds();
        for &(cx, cy, cz, r, ry) in &blobs {
            let (x0, x1) = ((cx - r).floor() as i32, (cx + r).ceil() as i32);
            let (y0, y1) = ((cy - ry).floor() as i32, (cy + ry).ceil() as i32);
            let (z0, z1) = ((cz - r).floor() as i32, (cz + r).ceil() as i32);
            if x1 < lo[0] || x0 > hi[0] || y1 < lo[1] || y0 > hi[1] || z1 < lo[2] || z0 > hi[2] {
                continue;
            }
            for ly in y0.max(lo[1])..=y1.min(hi[1]) {
                for lz in z0.max(lo[2])..=z1.min(hi[2]) {
                    for lx in x0.max(lo[0])..=x1.min(hi[0]) {
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
                            self.hang_vine(w, b, lx, ly, lz, cx, cz);
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
    fn spruce_crown<S: TreeSink>(
        &self,
        w: &mut S,
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
        let (lo, hi) = w.bounds();
        if top < lo[1] || start > hi[1] {
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
    #[allow(clippy::too_many_arguments)]
    fn hang_vine<S: TreeSink>(
        &self,
        w: &mut S,
        b: &GenBlocks,
        lx: i32,
        ly: i32,
        lz: i32,
        cx: f32,
        cz: f32,
    ) {
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
            match w.get(vx, yy, vz) {
                Some(s) if s.is_air() => w.put(vx, yy, vz, b.vines[attach]),
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
        let u = unit_f32(h.rotate_left(24));
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
                if u < 0.55
                    && matches!(s.surface, Surface::Sand | Surface::RedSand)
                    && matches!(
                        s.realm,
                        crate::realms::Realm::Nearctic | crate::realms::Realm::Neotropical
                    ) =>
            {
                // Cacti, the New World's (none in the Sahara or the Gobi): a few per cell, each
                // on its own column.
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
                // Boulders of the local bedrock, mossy in wet climates.
                let rock = if s.precipitation > 900.0 {
                    b.mossy_cobblestone
                } else if rng.chance(0.6) {
                    wg.rock_at(ox, y - 8, oz)
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

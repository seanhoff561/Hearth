//! Seasonal snow and ice on loaded terrain (v2 §4.3): the world generator lays down only
//! perennial snow and ice; the snowpack and lake ice of the current date (year-scale model in
//! `hearth_env::climate`) are laid on top when terrain loads, and brought up to date on loaded
//! terrain as the calendar moves on (`refresh`).
//!
//! Placement: snow lies on full blocks and buries low plants (remembered and restored when it
//! melts); bare deciduous crowns let it through to the ground, a little thinner; conifer crowns
//! hold a couple of layers and shed the rest. Still water and rivers freeze, and the sea
//! where winters are cold enough for sea ice.
//!
//! Rivers rise and fall with the flow of their basins through the year
//! (`hearth_env::rivers`): spring floods of snowmelt rivers and wet-season high water spill
//! over the banks onto the floodplain (drowning low plants, which come back), dry seasons lower
//! them, and small rivers of dry climates run dry.

use std::sync::Arc;

use hearth_env::climate::{Normals, SeasonalCover};
use hearth_env::rivers::RiverRegimes;
use hearth_math::hash::hash_2d;
use hearth_math::{BlockPos, ColumnPos};
use hearth_world::{BlockRegistry, BlockStateId, CubeMap, StateFlags, TintKind};
use hearth_worldgen::region::ColumnSample;
use hearth_worldgen::region::rivers::{BANK_HEIGHT, bank_width};
use hearth_worldgen::{PlanetGrid, WorldGenerator};
use rustc_hash::FxHashMap;

/// Block states the cover uses.
#[derive(Debug, Clone)]
pub struct CoverStates {
    pub snow_layers: [BlockStateId; 8],
    pub ice: BlockStateId,
    pub water: BlockStateId,
    pub grass: BlockStateId,
    pub grass_snowy: BlockStateId,
    pub podzol: BlockStateId,
    pub podzol_snowy: BlockStateId,
}

impl CoverStates {
    pub fn resolve(reg: &BlockRegistry) -> anyhow::Result<Self> {
        let s = |n: &str| reg.parse_state(n).map_err(|e| anyhow::anyhow!("{n}: {e}"));
        let mut snow_layers = [BlockStateId::AIR; 8];
        for (i, l) in snow_layers.iter_mut().enumerate() {
            *l = s(&format!("snow[layers={}]", i + 1))?;
        }
        Ok(Self {
            snow_layers,
            ice: s("ice")?,
            water: reg.water_source(),
            grass: s("grass_block[snowy=false]")?,
            grass_snowy: s("grass_block[snowy=true]")?,
            podzol: s("podzol[snowy=false]")?,
            podzol_snowy: s("podzol[snowy=true]")?,
        })
    }

    fn is_snow(&self, b: BlockStateId) -> bool {
        self.snow_layers.contains(&b)
    }
}

/// Plants buried under seasonal snow or drowned by a flood (and water plants left dry by a
/// falling river), restored when the season turns.
#[derive(Debug, Default)]
pub struct Buried(FxHashMap<BlockPos, BlockStateId>);

impl Buried {
    /// Forgets plants in columns that are no longer loaded.
    pub fn retain_columns(&mut self, keep: impl Fn(ColumnPos) -> bool) {
        self.0.retain(|p, _| keep(p.column()));
    }
}

/// Block edits with the state each touched block had before, so a caller can tell what really
/// changed.
struct Edit<'a> {
    map: &'a mut CubeMap,
    reg: &'a BlockRegistry,
    before: FxHashMap<BlockPos, BlockStateId>,
}

impl<'a> Edit<'a> {
    fn new(map: &'a mut CubeMap, reg: &'a BlockRegistry) -> Self {
        Self {
            map,
            reg,
            before: FxHashMap::default(),
        }
    }

    fn get(&self, p: BlockPos) -> Option<BlockStateId> {
        self.map.block(p)
    }

    fn set(&mut self, p: BlockPos, b: BlockStateId) {
        let Some(old) = self.map.block(p) else {
            return;
        };
        self.before.entry(p).or_insert(old);
        self.map.set_block(p, b, self.reg);
    }

    /// Positions whose state differs from before the edits.
    fn changed(self) -> Vec<BlockPos> {
        let map = &*self.map;
        self.before
            .into_iter()
            .filter(|(p, b)| map.block(*p) != Some(*b))
            .map(|(p, _)| p)
            .collect()
    }
}

/// The climate normals of a column at its own surface (the terrain sampler has already applied
/// the lapse rate for its height).
pub fn column_normals(generator: &WorldGenerator, col: ColumnPos) -> Normals {
    let data = generator.column(col);
    let s = data.at(8, 8);
    let (x0, z0) = col.min_block_xz();
    let base = Normals::sample(&generator.terrain.grid, x0 as f64 + 8.0, z0 as f64 + 8.0);
    Normals::new(
        base.lat_deg,
        s.temperature as f64,
        (2.0 * (s.t_warm - s.temperature)).max(0.0) as f64,
        s.precipitation as f64,
        base.winter_dry,
        base.summer_dry,
    )
}

/// What the date lays on a column: snow depth (m), and still-water and sea ice thickness (m).
#[derive(Debug, Clone, Copy)]
pub struct DateCover {
    pub snow_m: f64,
    pub ice_m: f64,
    pub sea_ice_m: f64,
}

/// The snow and ice of a column at a year fraction.
pub fn column_cover(generator: &WorldGenerator, col: ColumnPos, year_frac: f64) -> DateCover {
    let cover = SeasonalCover::compute(&column_normals(generator, col));
    DateCover {
        snow_m: cover.snow_depth_m(year_frac),
        ice_m: cover.ice_m(year_frac),
        sea_ice_m: cover.sea_ice_m(year_frac),
    }
}

/// A river column's water on a date.
#[derive(Debug, Clone, Copy)]
struct RiverTarget {
    /// First block above the ground: water fills up from here.
    bed: i32,
    /// First block above the water at mean flow (the ground for a dry bank).
    normal: i32,
    /// First block above the water on the date.
    date: i32,
    /// Above the highest the water ever reaches here.
    reach: i32,
}

/// A river's depth grows with its discharge to this power (Manning's equation for a wide
/// channel).
const STAGE_EXPONENT: f32 = 0.6;
/// Over its banks a river spreads across the floodplain and rises more slowly…
const FLOODPLAIN_RISE: f32 = 0.5;
/// …and the deepest floods stand at most this far over the banks: half a block plus a quarter
/// of the channel's depth (a block or so on small rivers, a few on great ones).
const OVERBANK_BASE: f32 = 0.5;
const OVERBANK_PER_DEPTH: f32 = 0.25;
/// A flood higher than the land around the river spreads over it in a film too thin to show:
/// toward the edge of the river's banks its surface falls to the height of that land, by this
/// much per block, so it never stands in walls above lower ground.
const FLOOD_THINNING: f32 = 0.5;

/// A river column's water level on a date with its reach flowing at `flow` times the mean.
fn river_target(s: &ColumnSample, flow: f64) -> Option<RiverTarget> {
    let r = s.river?;
    if s.ocean || s.lake {
        return None;
    }
    let flow = flow as f32;
    let dry = -r.depth - 1.0;
    let max_rise = BANK_HEIGHT + OVERBANK_BASE + OVERBANK_PER_DEPTH * r.depth;
    let mut rise = r.depth * (flow.powf(STAGE_EXPONENT) - 1.0);
    if rise > BANK_HEIGHT {
        rise = BANK_HEIGHT + (rise - BANK_HEIGHT) * FLOODPLAIN_RISE;
    }
    // Small rivers stop flowing in the dry season; large ones only when nothing comes at all.
    if flow < 0.02 || (flow < 0.15 && r.width < 8.0) {
        rise = dry;
    }
    let rise = rise.clamp(dry, max_rise);
    // The sea holds up the level at river mouths.
    let mut level = (r.level + rise).max(r.level.min(0.0));
    if rise > 0.0 {
        let edge = r.width * 0.5 + bank_width(r.width);
        let thin = r.plain + (edge - r.distance).max(0.0) * FLOOD_THINNING;
        level = level.min(thin.max(r.level));
    }
    let bed = s.height_i();
    Some(RiverTarget {
        bed,
        normal: if s.is_underwater() { s.water_i() } else { bed },
        date: (level.round() as i32).max(bed),
        reach: (r.level + max_rise).ceil() as i32 + 1,
    })
}

/// What the date asks of one block column.
#[derive(Debug, Clone, Copy)]
struct Target {
    x: i32,
    z: i32,
    /// Scan range around the terrain surface.
    floor: i32,
    top: i32,
    /// Snow layers on open ground (before canopy rules).
    layers: usize,
    /// Freeze still water at its surface.
    freeze: bool,
    /// Y of the top water block for water columns (where ice may lie).
    water_top: Option<i32>,
    /// River water rising and falling with the date.
    river: Option<RiverTarget>,
}

fn target(s: &ColumnSample, x: i32, z: i32, c: &DateCover, river_flow: f64) -> Target {
    let (snow_m, ice_m, sea_ice_m) = (c.snow_m, c.ice_m, c.sea_ice_m);
    let top_guess = s.height_i().max(s.water_i());
    // Drifting varies the depth a little from place to place.
    let jitter = (hash_2d(0x5a0e, x, z) & 3) as f64 * 0.04 - 0.06;
    let layers = if snow_m > 0.0 {
        ((snow_m + jitter) / 0.125).round().clamp(0.0, 8.0) as usize
    } else {
        0
    };
    // Rivers freeze to about half the thickness of still water; the sea by its own rule.
    let thickness = if s.ocean {
        sea_ice_m
    } else if s.river.is_some() {
        ice_m * 0.5
    } else {
        ice_m
    };
    Target {
        x,
        z,
        floor: top_guess - 8,
        top: top_guess + 48,
        layers,
        freeze: thickness > 0.03,
        water_top: s.is_underwater().then(|| s.water_i() - 1),
        river: river_target(s, river_flow),
    }
}

/// The seasonal cover of loaded terrain: the block states it uses, the planet's river regimes
/// and what it has buried.
pub struct SeasonCover {
    pub states: CoverStates,
    pub rivers: Arc<RiverRegimes>,
    pub buried: Buried,
}

impl SeasonCover {
    pub fn new(reg: &BlockRegistry, grid: &PlanetGrid) -> anyhow::Result<Self> {
        Ok(Self {
            states: CoverStates::resolve(reg)?,
            rivers: Arc::new(RiverRegimes::build(grid)),
            buried: Buried::default(),
        })
    }

    /// Lays the snow, ice and river levels of `year_frac` on freshly loaded columns (on the
    /// blocks already in `map`). Call before lighting new cubes; returns how many blocks
    /// changed.
    pub fn apply(
        &mut self,
        map: &mut CubeMap,
        reg: &BlockRegistry,
        generator: &WorldGenerator,
        cols: &[ColumnPos],
        year_frac: f64,
    ) -> usize {
        let mut edit = Edit::new(map, reg);
        let (states, buried) = (&self.states, &mut self.buried);
        for_each_target(generator, &self.rivers, cols, year_frac, |t| {
            cover(&mut edit, states, buried, t)
        });
        edit.changed().len()
    }

    /// Brings loaded columns up to date with `year_frac`: the old cover is taken off (buried
    /// plants come back, ice thaws, rivers return to their mean level) and the date's cover laid
    /// again. Returns the blocks that changed (for relighting and remeshing). Columns with
    /// perennial snow keep the generator's.
    pub fn refresh(
        &mut self,
        map: &mut CubeMap,
        reg: &BlockRegistry,
        generator: &WorldGenerator,
        cols: &[ColumnPos],
        year_frac: f64,
    ) -> Vec<BlockPos> {
        let mut edit = Edit::new(map, reg);
        let seasonal: Vec<ColumnPos> = cols
            .iter()
            .copied()
            .filter(|c| !SeasonalCover::compute(&column_normals(generator, *c)).perennial)
            .collect();
        let (states, buried) = (&self.states, &mut self.buried);
        for_each_target(generator, &self.rivers, &seasonal, year_frac, |t| {
            strip(&mut edit, states, buried, t);
            cover(&mut edit, states, buried, t);
        });
        edit.changed()
    }
}

fn for_each_target(
    generator: &WorldGenerator,
    rivers: &RiverRegimes,
    cols: &[ColumnPos],
    year_frac: f64,
    mut f: impl FnMut(&Target),
) {
    for &col in cols {
        let cover = column_cover(generator, col, year_frac);
        let (x0, z0) = col.min_block_xz();
        let data = generator.column(col);
        for lz in 0..16 {
            for lx in 0..16 {
                let (x, z) = (x0 + lx as i32, z0 + lz as i32);
                let s = data.at(lx, lz);
                let flow = s.river.map_or(1.0, |r| rivers.flow(r.cell, year_frac));
                f(&target(s, x, z, &cover, flow));
            }
        }
    }
}

/// Takes seasonal cover off a column: snow layers (restoring buried plants), snowy ground,
/// ice on the water surface, and a river's seasonal rise or fall (back to its mean level).
fn strip(edit: &mut Edit<'_>, states: &CoverStates, buried: &mut Buried, t: &Target) {
    strip_snow_and_ice(edit, states, buried, t);
    let Some(r) = t.river else {
        return;
    };
    // Water back where the dry season took it (with the water plants it left dry), and off the
    // banks where the flood left it (with the plants it drowned).
    for y in r.bed..r.reach.max(r.normal) {
        let p = BlockPos::new(t.x, y, t.z);
        let Some(b) = edit.get(p) else {
            continue;
        };
        if !(b.is_air() || b == states.water || b == states.ice) {
            continue;
        }
        let back = match buried.0.remove(&p) {
            Some(plant) => plant,
            None if y < r.normal => states.water,
            None => BlockStateId::AIR,
        };
        edit.set(p, back);
    }
}

fn strip_snow_and_ice(edit: &mut Edit<'_>, states: &CoverStates, buried: &mut Buried, t: &Target) {
    for y in t.floor..=t.top {
        let p = BlockPos::new(t.x, y, t.z);
        let Some(b) = edit.get(p) else {
            continue;
        };
        let bare = if states.is_snow(b) {
            buried.0.remove(&p).unwrap_or(BlockStateId::AIR)
        } else if b == states.grass_snowy {
            states.grass
        } else if b == states.podzol_snowy {
            states.podzol
        } else if b == states.ice && t.water_top == Some(y) {
            states.water
        } else {
            continue;
        };
        edit.set(p, bare);
    }
}

/// Lays the target's river level, then its snow or ice, on a column.
fn cover(edit: &mut Edit<'_>, states: &CoverStates, buried: &mut Buried, t: &Target) {
    if let Some(r) = t.river {
        set_river_level(edit, states, buried, r, t.x, t.z);
    }
    let Some((p, b)) = highest_block(edit, t.x, t.z, t.top, t.floor, |_| false) else {
        return;
    };
    if b == states.water {
        if t.freeze {
            edit.set(p, states.ice);
        }
        return;
    }
    if t.layers > 0 {
        lay_snow(edit, states, buried, p, b, t.layers, t.floor);
    }
}

/// Raises or lowers a river column's water from its mean level to the date's.
fn set_river_level(
    edit: &mut Edit<'_>,
    states: &CoverStates,
    buried: &mut Buried,
    r: RiverTarget,
    x: i32,
    z: i32,
) {
    let reg = edit.reg;
    // Plants and other things without a body drown (and come back when the water falls); tree
    // trunks and the like stand in the flood.
    let drowns = |b: BlockStateId| {
        !b.is_air()
            && !reg.has(b, StateFlags::HAS_COLLISION)
            && !reg.has(b, StateFlags::FLUID)
            && !states.is_snow(b)
    };
    if r.date > r.normal {
        // High water over the banks.
        for y in r.normal..r.date {
            let p = BlockPos::new(x, y, z);
            match edit.get(p) {
                Some(b) if b.is_air() => edit.set(p, states.water),
                Some(b) if drowns(b) => {
                    buried.0.insert(p, b);
                    edit.set(p, states.water);
                }
                _ => {}
            }
        }
        // The top half of a tall plant whose foot drowned goes under with it.
        let (p, below) = (BlockPos::new(x, r.date, z), BlockPos::new(x, r.date - 1, z));
        if let (Some(b), Some(&foot)) = (edit.get(p), buried.0.get(&below))
            && drowns(b)
            && reg.block_id_of(b) == reg.block_id_of(foot)
        {
            buried.0.insert(p, b);
            edit.set(p, BlockStateId::AIR);
        }
    } else if r.date < r.normal {
        // Low water: the river falls and bares its bed and the water plants on it.
        for y in r.date..r.normal {
            let p = BlockPos::new(x, y, z);
            match edit.get(p) {
                Some(b) if b == states.water => edit.set(p, BlockStateId::AIR),
                Some(b) if reg.block_of(b).def.water_filled => {
                    buried.0.insert(p, b);
                    edit.set(p, BlockStateId::AIR);
                }
                _ => {}
            }
        }
    }
}

/// The highest loaded block in a column between `top` and `floor` that is neither air nor
/// `skip`.
fn highest_block(
    edit: &Edit<'_>,
    x: i32,
    z: i32,
    top: i32,
    floor: i32,
    skip: impl Fn(BlockStateId) -> bool,
) -> Option<(BlockPos, BlockStateId)> {
    (floor..=top).rev().find_map(|y| {
        let p = BlockPos::new(x, y, z);
        edit.get(p)
            .filter(|b| !b.is_air() && !skip(*b))
            .map(|b| (p, b))
    })
}

fn deciduous(reg: &BlockRegistry, b: BlockStateId) -> bool {
    matches!(
        reg.block_of(b).def.tint,
        TintKind::Foliage | TintKind::Birch
    )
}

/// Snow on a column whose highest block is `b` at `p`.
fn lay_snow(
    edit: &mut Edit<'_>,
    states: &CoverStates,
    buried: &mut Buried,
    p: BlockPos,
    b: BlockStateId,
    layers: usize,
    floor: i32,
) {
    let reg = edit.reg;
    let (p, b, layers) = if deciduous(reg, b) {
        // Bare crowns let the snow through to the ground, a little less of it.
        match highest_block(edit, p.x, p.z, p.y - 1, floor, |c| deciduous(reg, c)) {
            Some((q, c)) => (q, c, (layers * 3 / 4).max(1)),
            None => return,
        }
    } else if reg.block_of(b).def.tint == TintKind::Spruce {
        // Conifer crowns hold a couple of layers and shed the rest.
        (p, b, layers.min(2))
    } else {
        (p, b, layers)
    };
    let supports = |g: BlockStateId| {
        reg.has(g, StateFlags::FULL_CUBE_SHAPE) || reg.has(g, StateFlags::SOLID_TOP)
    };
    let (snow_at, ground, g) =
        if reg.has(b, StateFlags::REPLACEABLE) && !reg.has(b, StateFlags::FLUID) {
            // A low plant (or older snow) on the ground: buried under the snow.
            let below = p.down();
            match edit.get(below) {
                Some(g) if supports(g) => {
                    if !states.is_snow(b) {
                        buried.0.insert(p, b);
                    }
                    (p, below, g)
                }
                _ => return,
            }
        } else {
            if !supports(b) || edit.get(p.up()).is_none_or(|a| !a.is_air()) {
                return;
            }
            (p.up(), p, b)
        };
    edit.set(snow_at, states.snow_layers[layers - 1]);
    if g == states.grass {
        edit.set(ground, states.grass_snowy);
    } else if g == states.podzol {
        edit.set(ground, states.podzol_snowy);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use hearth_math::{CubePos, PlanetSize};
    use hearth_world::Cube;

    use super::*;

    struct Fixture {
        reg: BlockRegistry,
        states: CoverStates,
        map: CubeMap,
    }

    fn fixture() -> Fixture {
        let reg = hearth_world::datapack::load_builtin_registry().expect("registry");
        let states = CoverStates::resolve(&reg).expect("cover states");
        let mut map =
            CubeMap::new(hearth_math::Planet::from_size(PlanetSize::Tiny).expect("planet"));
        map.insert_cube(
            CubePos::new(0, 0, 0),
            Arc::new(Cube::filled(BlockStateId::AIR)),
            &reg,
        );
        Fixture { reg, states, map }
    }

    fn put(f: &mut Fixture, x: i32, y: i32, z: i32, state: &str) {
        let s = f.reg.parse_state(state).expect(state);
        f.map.set_block(BlockPos::new(x, y, z), s, &f.reg);
    }

    fn at(f: &Fixture, x: i32, y: i32, z: i32) -> BlockStateId {
        f.map.block(BlockPos::new(x, y, z)).expect("loaded")
    }

    fn target(x: i32, z: i32, layers: usize) -> Target {
        Target {
            x,
            z,
            floor: 0,
            top: 15,
            layers,
            freeze: false,
            water_top: None,
            river: None,
        }
    }

    #[test]
    fn snow_buries_low_plants_and_they_come_back() {
        let mut f = fixture();
        put(&mut f, 1, 4, 1, "grass_block[snowy=false]");
        put(&mut f, 1, 5, 1, "short_grass");
        let tuft = at(&f, 1, 5, 1);
        let mut buried = Buried::default();
        let t = target(1, 1, 3);
        let mut edit = Edit::new(&mut f.map, &f.reg);
        cover(&mut edit, &f.states, &mut buried, &t);
        assert_eq!(edit.changed().len(), 2);
        assert_eq!(at(&f, 1, 5, 1), f.states.snow_layers[2]);
        assert_eq!(at(&f, 1, 4, 1), f.states.grass_snowy);
        // The thaw brings the tuft back and the grass loses its snow.
        let mut edit = Edit::new(&mut f.map, &f.reg);
        strip(&mut edit, &f.states, &mut buried, &t);
        let changed = edit.changed();
        assert_eq!(changed.len(), 2);
        assert_eq!(at(&f, 1, 5, 1), tuft);
        assert_eq!(at(&f, 1, 4, 1), f.states.grass);
        // Re-covering to the same depth changes nothing.
        let mut edit = Edit::new(&mut f.map, &f.reg);
        cover(&mut edit, &f.states, &mut buried, &t);
        strip(&mut edit, &f.states, &mut buried, &t);
        cover(&mut edit, &f.states, &mut buried, &t);
        assert_eq!(
            edit.changed().len(),
            2,
            "only the first cover counts against the bare state"
        );
    }

    #[test]
    fn canopies_shed_or_pass_snow() {
        let mut f = fixture();
        // A bare oak: snow falls through to the ground, a quarter thinner.
        put(&mut f, 2, 4, 2, "loam");
        for y in 8..=10 {
            put(
                &mut f,
                2,
                y,
                2,
                "oak_leaves[distance=1,persistent=false,waterlogged=false]",
            );
        }
        // A spruce: a couple of layers on the crown.
        put(&mut f, 5, 4, 5, "loam");
        put(
            &mut f,
            5,
            10,
            5,
            "spruce_leaves[distance=1,persistent=false,waterlogged=false]",
        );
        let mut buried = Buried::default();
        let mut edit = Edit::new(&mut f.map, &f.reg);
        cover(&mut edit, &f.states, &mut buried, &target(2, 2, 8));
        cover(&mut edit, &f.states, &mut buried, &target(5, 5, 8));
        drop(edit);
        assert_eq!(
            at(&f, 2, 5, 2),
            f.states.snow_layers[5],
            "6 of 8 layers under the oak"
        );
        assert!(at(&f, 2, 11, 2).is_air(), "nothing on the bare crown");
        assert_eq!(
            at(&f, 5, 11, 5),
            f.states.snow_layers[1],
            "2 layers on the spruce"
        );
        assert!(at(&f, 5, 5, 5).is_air(), "the spruce shelters the ground");
    }

    /// A river channel at (4, 4) (bed y 4, water to y 7) with banks at y 9 east of it: short
    /// grass at (5, 4), a tall plant at (6, 4), seagrass on the channel floor.
    fn river_fixture() -> Fixture {
        let mut f = fixture();
        for y in 0..4 {
            put(&mut f, 4, y, 4, "granite");
        }
        put(&mut f, 4, 4, 4, "seagrass");
        for y in 5..8 {
            f.map
                .set_block(BlockPos::new(4, y, 4), f.states.water, &f.reg);
        }
        for x in 5..7 {
            for y in 0..9 {
                put(&mut f, x, y, 4, "loam");
            }
        }
        put(&mut f, 5, 9, 4, "short_grass");
        put(&mut f, 6, 9, 4, "tall_grass[half=lower]");
        put(&mut f, 6, 10, 4, "tall_grass[half=upper]");
        f
    }

    fn river_targets(date: i32) -> [Target; 3] {
        let mut out = [target(4, 4, 0), target(5, 4, 0), target(6, 4, 0)];
        for (t, (bed, normal)) in out.iter_mut().zip([(4, 8), (9, 9), (9, 9)]) {
            t.river = Some(RiverTarget {
                bed,
                normal,
                date: date.max(bed),
                reach: 14,
            });
        }
        out
    }

    fn column(f: &Fixture, x: i32) -> Vec<BlockStateId> {
        (0..16).map(|y| at(f, x, y, 4)).collect()
    }

    fn season(f: &mut Fixture, buried: &mut Buried, date: i32, strip_first: bool) -> usize {
        let mut edit = Edit::new(&mut f.map, &f.reg);
        for t in &river_targets(date) {
            if strip_first {
                strip(&mut edit, &f.states, buried, t);
            }
            cover(&mut edit, &f.states, buried, t);
        }
        edit.changed().len()
    }

    #[test]
    fn floods_drown_the_floodplain_and_recede() {
        let mut f = river_fixture();
        let before: Vec<_> = (4..7).map(|x| column(&f, x)).collect();
        let mut buried = Buried::default();
        // High water to y 11: two blocks over the banks.
        season(&mut f, &mut buried, 12, false);
        for x in 4..7 {
            for y in 9..12 {
                assert_eq!(at(&f, x, y, 4), f.states.water, "flooded at ({x}, {y})");
            }
        }
        assert!(at(&f, 6, 12, 4).is_air());
        // Laying the same date again changes nothing.
        assert_eq!(season(&mut f, &mut buried, 12, false), 0);
        // A lower flood: only the tall plant's foot drowns, and its top goes with it.
        season(&mut f, &mut buried, 9, true);
        let mut edit = Edit::new(&mut f.map, &f.reg);
        for t in &river_targets(10) {
            strip(&mut edit, &f.states, &mut buried, t);
            cover(&mut edit, &f.states, &mut buried, t);
        }
        drop(edit);
        assert_eq!(at(&f, 6, 9, 4), f.states.water);
        assert!(
            at(&f, 6, 10, 4).is_air(),
            "no top half floating on the flood"
        );
        // Back to the mean level: every plant returns.
        season(&mut f, &mut buried, 8, true);
        let after: Vec<_> = (4..7).map(|x| column(&f, x)).collect();
        assert_eq!(after, before);
        assert!(buried.0.is_empty());
    }

    #[test]
    fn low_water_bares_the_bed_and_comes_back() {
        let mut f = river_fixture();
        let before = column(&f, 4);
        let mut buried = Buried::default();
        // Low water: one block left over the seagrass.
        season(&mut f, &mut buried, 6, false);
        assert_eq!(at(&f, 4, 5, 4), f.states.water);
        assert!(at(&f, 4, 6, 4).is_air() && at(&f, 4, 7, 4).is_air());
        // A dry bed, snowed on in winter.
        let mut edit = Edit::new(&mut f.map, &f.reg);
        let mut t = river_targets(4)[0];
        t.layers = 2;
        strip(&mut edit, &f.states, &mut buried, &t);
        cover(&mut edit, &f.states, &mut buried, &t);
        drop(edit);
        assert_eq!(
            at(&f, 4, 4, 4),
            f.states.snow_layers[1],
            "snow on the dry bed"
        );
        assert!(at(&f, 4, 5, 4).is_air());
        // The river comes back with its seagrass.
        season(&mut f, &mut buried, 8, true);
        assert_eq!(column(&f, 4), before);
        assert!(buried.0.is_empty());
    }

    #[test]
    fn still_water_freezes_and_thaws() {
        let mut f = fixture();
        put(&mut f, 3, 3, 3, "granite");
        f.map
            .set_block(BlockPos::new(3, 4, 3), f.states.water, &f.reg);
        let mut t = target(3, 3, 0);
        t.freeze = true;
        t.water_top = Some(4);
        let mut buried = Buried::default();
        let mut edit = Edit::new(&mut f.map, &f.reg);
        cover(&mut edit, &f.states, &mut buried, &t);
        drop(edit);
        assert_eq!(at(&f, 3, 4, 3), f.states.ice);
        let mut edit = Edit::new(&mut f.map, &f.reg);
        strip(&mut edit, &f.states, &mut buried, &t);
        drop(edit);
        assert_eq!(at(&f, 3, 4, 3), f.states.water);
    }
}

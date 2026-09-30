//! Seasonal snow and ice on loaded terrain (v2 §4.3): the world generator lays down only
//! perennial snow and ice; the snowpack and lake ice of the current date (year-scale model in
//! `hearth_env::climate`) are laid on top when terrain loads, and brought up to date on loaded
//! terrain as the calendar moves on (`refresh`).
//!
//! Placement: snow lies on full blocks and buries low plants (remembered and restored when it
//! melts); bare deciduous crowns let it through to the ground, a little thinner; conifer crowns
//! hold a couple of layers and shed the rest. Still water and rivers freeze; the sea is left
//! open until sea ice arrives with the coasts (V2-2).

use hearth_env::climate::{Normals, SeasonalCover};
use hearth_math::hash::hash_2d;
use hearth_math::{BlockPos, ColumnPos};
use hearth_world::{BlockRegistry, BlockStateId, CubeMap, StateFlags, TintKind};
use hearth_worldgen::WorldGenerator;
use hearth_worldgen::region::ColumnSample;
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

/// Plants buried under seasonal snow, restored when it melts.
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

/// Snow depth (m) and ice thickness (m) for a column at a year fraction.
pub fn column_cover(generator: &WorldGenerator, col: ColumnPos, year_frac: f64) -> (f64, f64) {
    let cover = SeasonalCover::compute(&column_normals(generator, col));
    (cover.snow_depth_m(year_frac), cover.ice_m(year_frac))
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
}

fn target(s: &ColumnSample, x: i32, z: i32, snow_m: f64, ice_m: f64) -> Target {
    let top_guess = s.height_i().max(s.water_i());
    // Drifting varies the depth a little from place to place.
    let jitter = (hash_2d(0x5a0e, x, z) & 3) as f64 * 0.04 - 0.06;
    let layers = if snow_m > 0.0 {
        ((snow_m + jitter) / 0.125).round().clamp(0.0, 8.0) as usize
    } else {
        0
    };
    // Rivers freeze to about half the thickness of still water; the sea not at all (yet).
    let thickness = if s.ocean {
        0.0
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
    }
}

/// Lays the snow and ice of `year_frac` on freshly loaded columns (on the blocks already in
/// `map`). Call before lighting new cubes; returns how many blocks changed.
pub fn apply(
    map: &mut CubeMap,
    reg: &BlockRegistry,
    states: &CoverStates,
    generator: &WorldGenerator,
    buried: &mut Buried,
    cols: &[ColumnPos],
    year_frac: f64,
) -> usize {
    let mut edit = Edit::new(map, reg);
    for_each_target(generator, cols, year_frac, |t| {
        cover(&mut edit, states, buried, t)
    });
    edit.changed().len()
}

/// Brings loaded columns up to date with the snow and ice of `year_frac`: the old cover is
/// taken off (buried plants come back, ice thaws) and the date's cover laid again. Returns the
/// blocks that changed (for relighting and remeshing). Columns with perennial snow keep the
/// generator's.
pub fn refresh(
    map: &mut CubeMap,
    reg: &BlockRegistry,
    states: &CoverStates,
    generator: &WorldGenerator,
    buried: &mut Buried,
    cols: &[ColumnPos],
    year_frac: f64,
) -> Vec<BlockPos> {
    let mut edit = Edit::new(map, reg);
    let seasonal: Vec<ColumnPos> = cols
        .iter()
        .copied()
        .filter(|c| !SeasonalCover::compute(&column_normals(generator, *c)).perennial)
        .collect();
    for_each_target(generator, &seasonal, year_frac, |t| {
        strip(&mut edit, states, buried, t);
        cover(&mut edit, states, buried, t);
    });
    edit.changed()
}

fn for_each_target(
    generator: &WorldGenerator,
    cols: &[ColumnPos],
    year_frac: f64,
    mut f: impl FnMut(&Target),
) {
    for &col in cols {
        let (snow_m, ice_m) = column_cover(generator, col, year_frac);
        let (x0, z0) = col.min_block_xz();
        let data = generator.column(col);
        for lz in 0..16 {
            for lx in 0..16 {
                let (x, z) = (x0 + lx as i32, z0 + lz as i32);
                f(&target(data.at(lx, lz), x, z, snow_m, ice_m));
            }
        }
    }
}

/// Takes seasonal cover off a column: snow layers (restoring buried plants), snowy ground and
/// ice on the water surface.
fn strip(edit: &mut Edit<'_>, states: &CoverStates, buried: &mut Buried, t: &Target) {
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

/// Lays the target's snow or ice on a column.
fn cover(edit: &mut Edit<'_>, states: &CoverStates, buried: &mut Buried, t: &Target) {
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
        put(&mut f, 2, 4, 2, "dirt");
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
        put(&mut f, 5, 4, 5, "dirt");
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

    #[test]
    fn still_water_freezes_and_thaws() {
        let mut f = fixture();
        put(&mut f, 3, 3, 3, "stone");
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

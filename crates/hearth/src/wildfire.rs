//! Wildfire (V2-6, docs/design/flora.md): fire in the vegetation.
//!
//! Near the player a fire spreads block by block through turf and litter, plants, foliage and
//! wood, as readily as its fuel is dry (the air's humidity and warmth, the rain of the last two
//! days, and how much of the growth has cured in a dry season), faster downwind and upward.
//! Turf burns to bare ground; plants, foliage and twigs to nothing; limbs and trunks char.
//! Squares of ground the fire has left are kept, a few at a time, as Burned disturbances of
//! the vegetation, so the burned land enters succession and stays burned when its terrain is
//! generated again. Beyond the loaded terrain a fire spreads over the ecological cells by their
//! fuel, the dryness and the wind, and burns out in hours; each cell it burned is kept the same
//! way.

use glam::{DVec3, Vec2};
use hearth_math::BlockPos;
use hearth_math::hash::Rng;
use hearth_world::{BlockRegistry, BlockStateId, RenderKind};
use hearth_worldgen::region::biome::Biome;
use hearth_worldgen::vegetation::{Disturbance, DisturbanceKind, ECO_CELL, PATCH};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::environment::EnvSampler;

/// The near fire steps this often (ticks).
pub const STEP_TICKS: u64 = 10;
/// The game seconds a neighbour's chance of catching ([`Fuel::catches`], the conditions
/// applied) and the rain's of putting a flame out are reckoned over; a step takes its share.
const CATCH_S: f32 = 15.0;
/// At most this many blocks burn at once.
const MAX_BURNING: usize = 6000;
/// At most this many ecological cells burn at once far away.
const MAX_CELLS: usize = 200;
/// The far fire is followed this far from the player (m); beyond, the land is not kept.
const FAR_REACH: f64 = 12_000.0;

/// What burns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fuel {
    /// Grass turf, forest litter, moss: the ground's surface.
    Turf,
    /// Grasses, herbs, ferns, shrubs.
    Herb,
    Leaves,
    /// Limbs up to 4 px through.
    Twig,
    /// Limbs 8 px and more.
    Limb,
    Trunk,
}

impl Fuel {
    /// Game seconds it burns (drawn between).
    fn burns_s(self) -> (f32, f32) {
        match self {
            Fuel::Turf => (40.0, 90.0),
            Fuel::Herb => (30.0, 60.0),
            Fuel::Leaves => (40.0, 100.0),
            Fuel::Twig => (120.0, 300.0),
            Fuel::Limb => (900.0, 1800.0),
            Fuel::Trunk => (2400.0, 4800.0),
        }
    }

    /// How readily it catches from a burning neighbour in a step, in fully cured, dry fuel and
    /// still air.
    fn catches(self) -> f32 {
        match self {
            Fuel::Turf => 0.2,
            Fuel::Herb => 0.25,
            Fuel::Leaves => 0.09,
            Fuel::Twig => 0.07,
            Fuel::Limb => 0.025,
            Fuel::Trunk => 0.012,
        }
    }

    /// How hard it burns: the heat it gives its neighbours, and its radiant heat.
    fn heat(self) -> f32 {
        match self {
            Fuel::Turf => 1.0,
            Fuel::Herb => 0.9,
            Fuel::Leaves => 1.4,
            Fuel::Twig => 1.1,
            Fuel::Limb => 1.2,
            Fuel::Trunk => 1.3,
        }
    }
}

/// The fuel in each block state, what it burns to and the flames drawn.
pub struct FuelTable {
    fuel: Vec<Option<Fuel>>,
    burned_to: Vec<BlockStateId>,
    pub flames: BlockStateId,
}

impl FuelTable {
    pub fn new(reg: &BlockRegistry) -> Self {
        let n = reg.state_count();
        let mut fuel = vec![None; n];
        let mut burned_to = vec![BlockStateId::AIR; n];
        let flames = reg
            .parse_state("hearth:flames")
            .unwrap_or(BlockStateId::AIR);
        let burnt = reg.parse_state("hearth:burnt_ground").ok();
        // The charred block with the same properties (axis, thickness and joins).
        let charred = |s: BlockStateId, block: &str| -> Option<BlockStateId> {
            let props = reg.state_string(s);
            let props = props.split_once('[').map(|(_, p)| p).unwrap_or("]");
            reg.parse_state(&format!("hearth:{block}[{props}"))
                .or_else(|_| reg.parse_state(&format!("hearth:{block}")))
                .ok()
        };
        for block in reg.blocks() {
            let path = block.name.path();
            let def = &block.def;
            let first = block.first_state.0 as usize;
            for k in 0..block.state_count as usize {
                let s = BlockStateId((first + k) as u16);
                let snowy = reg.get(s, "snowy") == Some("true");
                let (f, to) = if path.starts_with("charred_") || path == "burnt_ground" {
                    (None, s)
                } else if matches!(path, "grass_block" | "podzol" | "moss_block") {
                    if snowy || burnt.is_none() {
                        (None, s)
                    } else {
                        (Some(Fuel::Turf), burnt.unwrap_or(s))
                    }
                } else if path.ends_with("_leaves") {
                    (Some(Fuel::Leaves), BlockStateId::AIR)
                } else if path.ends_with("_branch") {
                    match reg.get(s, "thickness") {
                        Some("2") | Some("4") => (Some(Fuel::Twig), BlockStateId::AIR),
                        _ => match charred(s, "charred_branch") {
                            Some(c) => (Some(Fuel::Limb), c),
                            None => (Some(Fuel::Limb), BlockStateId::AIR),
                        },
                    }
                } else if path.ends_with("_log") || path.ends_with("_wood") {
                    match charred(s, "charred_log") {
                        Some(c) => (Some(Fuel::Trunk), c),
                        None => (None, s),
                    }
                } else if (def.render == RenderKind::Cross
                    && def.fluid.is_none()
                    && !def.collision
                    && path != "flames")
                    || path == "moss_carpet"
                {
                    (Some(Fuel::Herb), BlockStateId::AIR)
                } else {
                    (None, s)
                };
                fuel[s.0 as usize] = f;
                burned_to[s.0 as usize] = to;
            }
        }
        Self {
            fuel,
            burned_to,
            flames,
        }
    }

    pub fn fuel(&self, s: BlockStateId) -> Option<Fuel> {
        self.fuel.get(s.0 as usize).copied().flatten()
    }

    pub fn burned_to(&self, s: BlockStateId) -> BlockStateId {
        self.burned_to
            .get(s.0 as usize)
            .copied()
            .unwrap_or(BlockStateId::AIR)
    }
}

/// How ready a place is to burn, and what drives a fire there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Danger {
    /// 0 nothing spreads … 1 fully cured fuel in the driest air.
    pub level: f32,
    /// The wind (m/s, toward; x east, y south).
    pub wind: Vec2,
    /// Rain falling (mm/h).
    pub rain_mm_h: f32,
}

impl Danger {
    pub const NONE: Danger = Danger {
        level: 0.0,
        wind: Vec2::ZERO,
        rain_mm_h: 0.0,
    };
}

/// Equilibrium moisture of fine dead fuel (% of dry weight) in air of a relative humidity
/// (%) and temperature (°C), after Simard (1968).
pub fn equilibrium_moisture(rh: f64, t: f64) -> f64 {
    let h = rh.clamp(0.0, 100.0);
    if h < 10.0 {
        0.03229 + 0.281_073 * h - 0.000_578 * h * t
    } else if h < 50.0 {
        2.22749 + 0.160_107 * h - 0.01478 * t
    } else {
        21.0606 + 0.005_565 * h * h - 0.00035 * h * t - 0.483_199 * h
    }
    .max(1.0)
}

/// The fire danger at a place now: how dry the fine fuel is (the air, and the rain of the last
/// two days) times how much of the growth has cured (in a dry month, when the rain falls short
/// of twice the temperature, all of it; otherwise a fifth).
pub fn danger(env: &EnvSampler, ticks: u64, at: DVec3) -> Danger {
    let m = env.calendar.at(ticks);
    let w = env.weather_at(&m, at);
    let tpd = env.calendar.ticks_per_day();
    let mut recent_mm = 0.0;
    for k in 1..=16u64 {
        let back = (k as f64 * 3.0 / 24.0 * tpd) as u64;
        let past = env.calendar.at(ticks.saturating_sub(back));
        let p = env.weather_at(&past, at).precip_mm_h;
        recent_mm += p * 3.0 * (-(k as f64) * 3.0 / 12.0).exp();
    }
    let mut moisture = equilibrium_moisture(w.humidity * 100.0, w.temperature_c);
    moisture += (4.0 * recent_mm).min(30.0);
    if w.precip_mm_h > 0.3 {
        moisture = moisture.max(35.0);
    }
    let dryness = ((30.0 - moisture) / 22.0).clamp(0.0, 1.0);
    let normals = hearth_env::climate::Normals::sample(&env.grid, at.x, at.z);
    let t = normals.temperature(m.year_frac);
    let p_month = normals.precip_mm_per_day(m.year_frac) * 30.4;
    let dry_month = if t > 2.0 {
        ((2.0 * t - p_month) / (2.0 * t)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    // A dry month by Gaussen's rule (rain under twice the warmth) has cured the grass.
    let x = (dry_month / 0.3).min(1.0);
    let cured = 0.2 + 0.8 * x * x * (3.0 - 2.0 * x);
    let (s, c) = w.wind_dir.sin_cos();
    Danger {
        level: (dryness * cured) as f32,
        wind: Vec2::new(s as f32, -c as f32) * w.wind_speed_m_s as f32,
        rain_mm_h: w.precip_mm_h as f32,
    }
}

#[derive(Debug, Clone, Copy)]
struct Burning {
    fuel: Fuel,
    /// The block as it was.
    was: BlockStateId,
    until: u64,
    /// Where its flames are drawn.
    flames: Option<BlockPos>,
}

/// What the near fire does to the world in a step.
pub trait FireWorld {
    /// The block at a place, if its terrain is loaded.
    fn block(&self, p: BlockPos) -> Option<BlockStateId>;
    /// Sets a block (as nature changes it; the player's changes stay the player's).
    fn set(&mut self, p: BlockPos, s: BlockStateId);
    /// Whether a place may not burn (a hearth, a rack, a bed).
    fn spared(&self, p: BlockPos) -> bool;
}

/// A smoke plume for the client: where it rises from and how thick it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plume {
    pub at: DVec3,
    /// 0–1 (a few burning blocks … a front tens of metres long; a cell of burning forest).
    pub strength: f32,
    /// Far (a cell's fire, its smoke kilometres high) or near.
    pub far: bool,
}

/// The fire near the player.
pub struct Wildfire {
    burning: FxHashMap<BlockPos, Burning>,
    /// Per square: its blocks burning, and whether any has burned.
    squares: FxHashMap<(i32, i32), (u32, bool)>,
    /// Squares the fire has left, not yet kept.
    left: Vec<(i32, i32)>,
    /// The tick of the last step (the time since it is this step's share of the chances).
    last_step: Option<u64>,
    rng: Rng,
}

/// The 26 neighbours and the block two above, with how near each is.
fn neighbours() -> Vec<(BlockPos, f32)> {
    let mut v = Vec::with_capacity(27);
    for dy in -1..=1 {
        for dz in -1..=1 {
            for dx in -1..=1 {
                if (dx, dy, dz) == (0, 0, 0) {
                    continue;
                }
                let n = dx * dx + dy * dy + dz * dz;
                let near = match n {
                    1 => 1.0,
                    2 => 0.7,
                    _ => 0.5,
                };
                v.push((BlockPos::new(dx, dy, dz), near));
            }
        }
    }
    v.push((BlockPos::new(0, 2, 0), 0.5));
    v
}

fn square_of(p: BlockPos) -> (i32, i32) {
    (p.x.div_euclid(PATCH), p.z.div_euclid(PATCH))
}

impl Wildfire {
    pub fn new(seed: u64) -> Self {
        Self {
            burning: FxHashMap::default(),
            squares: FxHashMap::default(),
            left: Vec::new(),
            last_step: None,
            rng: Rng::new(seed ^ 0xf17e),
        }
    }

    pub fn is_burning(&self) -> bool {
        !self.burning.is_empty()
    }

    pub fn burning_count(&self) -> usize {
        self.burning.len()
    }

    /// Sets a block alight; whether it caught (it has fuel and is not burning already).
    pub fn ignite(
        &mut self,
        world: &mut impl FireWorld,
        table: &FuelTable,
        p: BlockPos,
        ticks: u64,
        tick_s: f32,
    ) -> bool {
        if self.burning.contains_key(&p) || world.spared(p) {
            return false;
        }
        let Some(was) = world.block(p) else {
            return false;
        };
        let Some(fuel) = table.fuel(was) else {
            return false;
        };
        let (lo, hi) = fuel.burns_s();
        let until = ticks + (self.rng.range_f32(lo, hi) / tick_s.max(1e-3)) as u64;
        // Flames where they show: in place of what burns away, over turf, beside wood.
        let flames = match fuel {
            Fuel::Herb | Fuel::Leaves | Fuel::Twig => {
                world.set(p, table.flames);
                Some(p)
            }
            Fuel::Turf | Fuel::Limb | Fuel::Trunk => {
                let around = [
                    p.up(),
                    BlockPos::new(p.x + 1, p.y, p.z),
                    BlockPos::new(p.x - 1, p.y, p.z),
                    BlockPos::new(p.x, p.y, p.z + 1),
                    BlockPos::new(p.x, p.y, p.z - 1),
                ];
                let n = if fuel == Fuel::Turf { 1 } else { 5 };
                around[..n]
                    .iter()
                    .copied()
                    .find(|q| world.block(*q).is_some_and(|s| s.is_air()))
                    .inspect(|q| world.set(*q, table.flames))
            }
        };
        self.burning.insert(
            p,
            Burning {
                fuel,
                was,
                until,
                flames,
            },
        );
        self.squares.entry(square_of(p)).or_default().0 += 1;
        true
    }

    /// One step of the fire: what burns out is left as it burns to; burning blocks set their
    /// neighbours alight as the fuel, the dryness, the wind and the slope have it. Places the
    /// fire would reach in terrain not loaded go to `beyond` (for the far fire).
    pub fn step(
        &mut self,
        world: &mut impl FireWorld,
        table: &FuelTable,
        danger: Danger,
        ticks: u64,
        tick_s: f32,
        beyond: &mut Vec<BlockPos>,
    ) {
        if self.burning.is_empty() {
            self.last_step = None;
            return;
        }
        let since = ticks.saturating_sub(self.last_step.unwrap_or(ticks - STEP_TICKS.min(ticks)));
        if since < STEP_TICKS {
            return;
        }
        self.last_step = Some(ticks);
        // A chance over `CATCH_S` as this step's share of it (more of it when the world goes
        // faster than lived: a step a tick, each tick many).
        let share = (since as f32 * tick_s / CATCH_S).min(1.0);
        let per_step = |chance: f32| 1.0 - (1.0 - chance.clamp(0.0, 0.999)).powf(share);
        // Rain puts fires out.
        let doused = per_step(danger.rain_mm_h * 0.15) as f64;
        let mut out: Vec<BlockPos> = Vec::new();
        let mut keys: Vec<BlockPos> = self.burning.keys().copied().collect();
        keys.sort_unstable();
        for p in &keys {
            let b = self.burning[p];
            if ticks >= b.until || (doused > 0.0 && self.rng.chance(doused)) {
                out.push(*p);
            }
        }
        // Over the cap, the longest burning go first.
        if self.burning.len() > MAX_BURNING {
            let mut by_end: Vec<(u64, BlockPos)> =
                keys.iter().map(|p| (self.burning[p].until, *p)).collect();
            by_end.sort_unstable();
            out.extend(
                by_end
                    .into_iter()
                    .take(self.burning.len() - MAX_BURNING)
                    .map(|(_, p)| p),
            );
            out.sort_unstable();
            out.dedup();
        }
        for p in &out {
            self.burn_out(world, table, *p);
        }
        // Spread.
        let level = danger.level.clamp(0.0, 1.0);
        let dry = level * level.sqrt();
        if dry <= 0.0 {
            return;
        }
        let wind = danger.wind;
        let speed = wind.length();
        let along = if speed > 0.01 {
            wind / speed
        } else {
            Vec2::ZERO
        };
        let near = neighbours();
        let sources: Vec<(BlockPos, Fuel)> =
            self.burning.iter().map(|(p, b)| (*p, b.fuel)).collect();
        let mut sources = sources;
        sources.sort_unstable_by_key(|(p, _)| *p);
        for (p, src) in sources {
            for (o, closeness) in &near {
                let q = BlockPos::new(p.x + o.x, p.y + o.y, p.z + o.z);
                if self.burning.contains_key(&q) {
                    continue;
                }
                let Some(s) = world.block(q) else {
                    beyond.push(q);
                    continue;
                };
                let Some(f) = table.fuel(s) else {
                    continue;
                };
                // Downwind faster, upwind hardly (a fire backs into the wind at a tenth of its
                // pace or less); flames climb.
                let flat = Vec2::new(o.x as f32, o.z as f32);
                let wind_f = if flat.length_squared() > 0.0 {
                    let cos = along.dot(flat.normalize());
                    let k = if cos >= 0.0 { 0.22 } else { 0.5 };
                    (k * speed * cos).exp().clamp(0.05, 5.0)
                } else {
                    1.0
                };
                let climb = match o.y {
                    y if y > 0 => 2.5,
                    y if y < 0 => 0.35,
                    _ => 1.0,
                };
                let chance = f.catches() * src.heat() * dry * wind_f * climb * closeness;
                if self.rng.chance(per_step(chance) as f64) {
                    self.ignite(world, table, q, ticks, tick_s);
                }
            }
        }
    }

    fn burn_out(&mut self, world: &mut impl FireWorld, table: &FuelTable, p: BlockPos) {
        let Some(b) = self.burning.remove(&p) else {
            return;
        };
        if let Some(f) = b.flames
            && f != p
            && world.block(f) == Some(table.flames)
        {
            world.set(f, BlockStateId::AIR);
        }
        world.set(p, table.burned_to(b.was));
        let sq = square_of(p);
        let e = self.squares.entry(sq).or_default();
        e.0 = e.0.saturating_sub(1);
        e.1 = true;
        if e.0 == 0 {
            self.left.push(sq);
            self.squares.remove(&sq);
        }
    }

    /// The squares the fire has burned and left since the last call, as a Burned disturbance
    /// of the year (none if there are none).
    pub fn take_burned(&mut self, year: f64) -> Option<Disturbance> {
        if self.left.is_empty() {
            return None;
        }
        let mut patches: Vec<[i32; 2]> = self.left.drain(..).map(|(x, z)| [x, z]).collect();
        patches.sort_unstable();
        patches.dedup();
        let n = patches.len() as f64;
        let cx = patches.iter().map(|p| p[0] as f64).sum::<f64>() / n;
        let cz = patches.iter().map(|p| p[1] as f64).sum::<f64>() / n;
        let (x, z) = (
            (cx * PATCH as f64) as i32 + PATCH / 2,
            (cz * PATCH as f64) as i32 + PATCH / 2,
        );
        let radius = patches
            .iter()
            .map(|p| {
                let dx = (p[0] * PATCH + PATCH / 2 - x) as f32;
                let dz = (p[1] * PATCH + PATCH / 2 - z) as f32;
                (dx * dx + dz * dz).sqrt()
            })
            .fold(0.0, f32::max)
            + PATCH as f32;
        Some(Disturbance {
            kind: DisturbanceKind::Burned,
            year,
            x,
            z,
            radius,
            severity: 1.0,
            patches,
        })
    }

    /// Radiant heat (W/m², as a body absorbs it) from the blocks burning about a point.
    pub fn radiant_w_m2(&self, at: DVec3) -> f32 {
        let c = BlockPos::containing(at);
        let mut w = 0.0f32;
        for dy in -3..=4 {
            for dz in -6..=6 {
                for dx in -6..=6 {
                    let p = BlockPos::new(c.x + dx, c.y + dy, c.z + dz);
                    if let Some(b) = self.burning.get(&p) {
                        let d2 = (DVec3::new(p.x as f64 + 0.5, p.y as f64 + 0.5, p.z as f64 + 0.5)
                            - at)
                            .length_squared()
                            .max(0.25) as f32;
                        // A burning block radiates some 20 kW (turf) to 40 kW (a crown).
                        w += 20_000.0 * b.fuel.heat() / (4.0 * std::f32::consts::PI * d2);
                    }
                }
            }
        }
        (0.45 * w).min(2500.0)
    }

    /// Whether a place is in flames.
    pub fn in_flames(&self, p: BlockPos) -> bool {
        self.burning.values().any(|b| b.flames == Some(p))
    }

    /// The smoke over the fire: one plume per 32 m square with blocks burning (the dozen
    /// thickest).
    pub fn plumes(&self) -> Vec<Plume> {
        let mut by: FxHashMap<(i32, i32), (DVec3, u32)> = FxHashMap::default();
        for p in self.burning.keys() {
            let e = by
                .entry((p.x.div_euclid(32), p.z.div_euclid(32)))
                .or_insert((DVec3::ZERO, 0));
            e.0 += DVec3::new(p.x as f64 + 0.5, p.y as f64 + 1.0, p.z as f64 + 0.5);
            e.1 += 1;
        }
        let mut v: Vec<Plume> = by
            .into_values()
            .map(|(sum, n)| Plume {
                at: sum / n.max(1) as f64,
                strength: ((n as f32).sqrt() / 12.0).min(1.0),
                far: false,
            })
            .collect();
        v.sort_by(|a, b| b.strength.total_cmp(&a.strength));
        v.truncate(12);
        v
    }
}

/// A cell of the far fire.
#[derive(Debug, Clone, Copy)]
struct CellFire {
    until: u64,
    fuel: f32,
}

/// Fire far from the player, over the ecological cells.
pub struct FarFire {
    cells: FxHashMap<(i32, i32), CellFire>,
    /// Cells burned out lately (they do not burn again soon).
    burned: FxHashSet<(i32, i32)>,
    /// Cells burned out and not yet kept.
    left: Vec<((i32, i32), f32)>,
    next_hour: u64,
    rng: Rng,
}

impl FarFire {
    pub fn new(seed: u64) -> Self {
        Self {
            cells: FxHashMap::default(),
            burned: FxHashSet::default(),
            left: Vec::new(),
            next_hour: 0,
            rng: Rng::new(seed ^ 0x00fa_7f1e),
        }
    }

    pub fn is_burning(&self) -> bool {
        !self.cells.is_empty()
    }

    pub fn cell_of(x: f64, z: f64) -> (i32, i32) {
        (
            (x.floor() as i32).div_euclid(ECO_CELL),
            (z.floor() as i32).div_euclid(ECO_CELL),
        )
    }

    fn centre(cell: (i32, i32)) -> DVec3 {
        DVec3::new(
            (cell.0 * ECO_CELL + ECO_CELL / 2) as f64,
            0.0,
            (cell.1 * ECO_CELL + ECO_CELL / 2) as f64,
        )
    }

    /// How much a cell's land would carry a fire, 0–1: grass and scrub most, forest less,
    /// nothing on water, rock, ice and desert.
    pub fn fuel(env: &EnvSampler, terrain: &hearth_worldgen::Terrain, cell: (i32, i32)) -> f32 {
        let c = Self::centre(cell);
        let planet = env.planet;
        let s = terrain.sample(planet.wrap_x(c.x as i32), c.z as i32);
        if s.is_underwater() {
            return 0.0;
        }
        let base = match s.biome {
            Biome::Steppe | Biome::Savanna | Biome::MediterraneanScrub => 1.0,
            Biome::BroadleafForest | Biome::MixedForest | Biome::BirchForest => 0.6,
            Biome::BorealForest | Biome::MontaneForest | Biome::TemperateRainforest => 0.7,
            Biome::Wetland | Biome::SaltMarsh | Biome::Mangrove => 0.15,
            Biome::TropicalRainforest => 0.2,
            Biome::AlpineMeadow | Biome::Tundra | Biome::Krummholz => 0.4,
            Biome::HotDesert
            | Biome::ColdDesert
            | Biome::DuneSea
            | Biome::Mesa
            | Biome::SaltFlat
            | Biome::Glacier
            | Biome::IceSheet
            | Biome::AlpineRock
            | Biome::Volcanic
            | Biome::Beach
            | Biome::StonyShore => 0.0,
            _ => 0.8,
        };
        base * (0.4 + 0.6 * s.tree_density.max(0.3)).min(1.0)
    }

    /// A cell catches fire (if it has fuel and has not burned lately).
    pub fn ignite(&mut self, cell: (i32, i32), fuel: f32, ticks: u64, ticks_per_day: f64) -> bool {
        if fuel <= 0.05 || self.cells.contains_key(&cell) || self.burned.contains(&cell) {
            return false;
        }
        let hours = self.rng.range_f64(6.0, 18.0) * (0.5 + fuel as f64);
        self.cells.insert(
            cell,
            CellFire {
                until: ticks + (hours / 24.0 * ticks_per_day) as u64,
                fuel,
            },
        );
        true
    }

    /// A game hour of the far fire: cells burn out, burning cells set their neighbours alight
    /// as their fuel, the danger there and the wind have it, as far as `FAR_REACH` from the
    /// player at `from`. Cells under `near` (the loaded terrain) are not burned here: they are
    /// given back to light the near fire.
    pub fn hour(
        &mut self,
        env: &EnvSampler,
        terrain: &hearth_worldgen::Terrain,
        ticks: u64,
        from: DVec3,
        near: impl Fn((i32, i32)) -> bool,
        handed_back: &mut Vec<(i32, i32)>,
    ) {
        let tpd = env.calendar.ticks_per_day();
        if self.cells.is_empty() || ticks < self.next_hour {
            return;
        }
        self.next_hour = ticks + (tpd / 24.0) as u64;
        let mut keys: Vec<(i32, i32)> = self.cells.keys().copied().collect();
        keys.sort_unstable();
        for cell in &keys {
            let c = self.cells[cell];
            if ticks >= c.until {
                self.cells.remove(cell);
                self.burned.insert(*cell);
                self.left.push((*cell, c.fuel));
            }
        }
        let burning: Vec<(i32, i32)> = {
            let mut v: Vec<(i32, i32)> = self.cells.keys().copied().collect();
            v.sort_unstable();
            v
        };
        for cell in burning {
            let d = danger(env, ticks, Self::centre(cell));
            if d.rain_mm_h > 1.0 {
                // Rain puts the cell out.
                if self.rng.chance((d.rain_mm_h as f64 * 0.1).min(0.8)) {
                    let f = self.cells.remove(&cell).map_or(0.5, |c| c.fuel);
                    self.burned.insert(cell);
                    self.left.push((cell, f * 0.5));
                }
                continue;
            }
            let speed = d.wind.length();
            let along = if speed > 0.01 {
                d.wind / speed
            } else {
                Vec2::ZERO
            };
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
                let next = (cell.0 + dx, cell.1 + dz);
                if self.cells.contains_key(&next) || self.burned.contains(&next) {
                    continue;
                }
                let c = Self::centre(next);
                let far = env.planet.delta_x(from.x, c.x).hypot(c.z - from.z);
                if far > FAR_REACH || self.cells.len() >= MAX_CELLS {
                    continue;
                }
                let fuel = Self::fuel(env, terrain, next);
                if fuel <= 0.05 {
                    continue;
                }
                let dir = Vec2::new(dx as f32, dz as f32).normalize();
                let wind_f = (0.2 * speed * along.dot(dir)).exp().clamp(0.2, 4.0);
                let diag = if dx != 0 && dz != 0 { 0.6 } else { 1.0 };
                let p = 0.15 * fuel * d.level * d.level * wind_f * diag;
                if self.rng.chance(p.min(1.0) as f64) {
                    if near(next) {
                        handed_back.push(next);
                    } else {
                        self.ignite(next, fuel, ticks, tpd);
                    }
                }
            }
        }
    }

    /// The cells burned out since the last call, each as a Burned disturbance of the year.
    pub fn take_burned(&mut self, year: f64) -> Vec<Disturbance> {
        self.left
            .drain(..)
            .map(|(cell, fuel)| {
                let c = Self::centre(cell);
                Disturbance {
                    kind: DisturbanceKind::Burned,
                    year,
                    x: c.x as i32,
                    z: c.z as i32,
                    radius: ECO_CELL as f32 * 0.62,
                    severity: (0.55 + 0.4 * fuel).min(0.95),
                    patches: Vec::new(),
                }
            })
            .collect()
    }

    /// The smoke of the cells burning.
    pub fn plumes(
        &self,
        terrain: &hearth_worldgen::Terrain,
        planet: hearth_math::Planet,
    ) -> Vec<Plume> {
        let mut v: Vec<Plume> = self
            .cells
            .iter()
            .map(|(cell, c)| {
                let mut at = Self::centre(*cell);
                at.y = terrain
                    .sample(planet.wrap_x(at.x as i32), at.z as i32)
                    .height as f64;
                Plume {
                    at,
                    strength: (0.5 + 0.5 * c.fuel).min(1.0),
                    far: true,
                }
            })
            .collect();
        v.sort_by(|a, b| a.at.x.total_cmp(&b.at.x).then(a.at.z.total_cmp(&b.at.z)));
        v.truncate(32);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg() -> BlockRegistry {
        let defs = hearth_world::datapack::load_block_defs(&[crate::scene::data_pack_dir()])
            .expect("blocks");
        BlockRegistry::build(defs).expect("registry")
    }

    /// A flat world of grass turf with tall grass on it, a pond in the middle of the west half.
    struct Field {
        blocks: FxHashMap<BlockPos, BlockStateId>,
    }

    impl FireWorld for Field {
        fn block(&self, p: BlockPos) -> Option<BlockStateId> {
            if p.x.abs() > 60 || p.z.abs() > 60 {
                return None;
            }
            Some(self.blocks.get(&p).copied().unwrap_or(BlockStateId::AIR))
        }
        fn set(&mut self, p: BlockPos, s: BlockStateId) {
            self.blocks.insert(p, s);
        }
        fn spared(&self, _: BlockPos) -> bool {
            false
        }
    }

    fn field(reg: &BlockRegistry) -> Field {
        let grass = reg
            .parse_state("hearth:grass_block[snowy=false]")
            .expect("grass");
        let short = reg.parse_state("hearth:short_grass").expect("short grass");
        let water = reg.parse_state("hearth:water[level=0]").expect("water");
        let mut blocks = FxHashMap::default();
        for z in -60..=60 {
            for x in -60..=60 {
                // A ditch of water from north to south at x = -20.
                if (-22..=-20).contains(&x) {
                    blocks.insert(BlockPos::new(x, 0, z), water);
                    continue;
                }
                blocks.insert(BlockPos::new(x, 0, z), grass);
                if (x * 7 + z * 13).rem_euclid(3) == 0 {
                    blocks.insert(BlockPos::new(x, 1, z), short);
                }
            }
        }
        Field { blocks }
    }

    fn burnt(f: &Field, reg: &BlockRegistry) -> Vec<BlockPos> {
        let b = reg
            .parse_state("hearth:burnt_ground")
            .expect("burnt ground");
        f.blocks
            .iter()
            .filter(|(_, s)| **s == b)
            .map(|(p, _)| *p)
            .collect()
    }

    fn run(danger: Danger, steps: u64) -> (Field, BlockRegistry, Wildfire) {
        let reg = reg();
        let table = FuelTable::new(&reg);
        let mut f = field(&reg);
        let mut fire = Wildfire::new(3);
        let tick_s = 1.5;
        assert!(fire.ignite(&mut f, &table, BlockPos::new(0, 0, 0), 0, tick_s));
        let mut beyond = Vec::new();
        for k in 0..steps {
            fire.step(&mut f, &table, danger, k * STEP_TICKS, tick_s, &mut beyond);
        }
        (f, reg, fire)
    }

    #[test]
    fn grass_burns_downwind_and_stops_at_water() {
        // Dry, with the wind blowing east.
        let danger = Danger {
            level: 0.95,
            wind: Vec2::new(6.0, 0.0),
            rain_mm_h: 0.0,
        };
        let (f, reg, fire) = run(danger, 240);
        let burnt = burnt(&f, &reg);
        let east = burnt.iter().filter(|p| p.x > 5).count();
        let west = burnt.iter().filter(|p| p.x < -5).count();
        println!("{} burned, {east} east, {west} west", burnt.len());
        assert!(burnt.len() > 300, "{} burned", burnt.len());
        assert!(east > 2 * west, "downwind {east} against upwind {west}");
        assert!(
            burnt.iter().all(|p| p.x > -20),
            "the fire crossed the water"
        );
        // An hour on, it has burned out (the field's edge is the edge of the world).
        let _ = fire;
    }

    #[test]
    fn damp_grass_does_not_carry_a_fire() {
        let danger = Danger {
            level: 0.12,
            wind: Vec2::new(2.0, 0.0),
            rain_mm_h: 0.0,
        };
        let (f, reg, fire) = run(danger, 240);
        let burnt = burnt(&f, &reg);
        assert!(burnt.len() < 30, "{} burned in damp grass", burnt.len());
        assert!(!fire.is_burning(), "it went out");
    }

    #[test]
    fn the_burned_ground_is_kept_as_squares() {
        let danger = Danger {
            level: 1.0,
            wind: Vec2::ZERO,
            rain_mm_h: 0.0,
        };
        let (_, _, mut fire) = run(danger, 60);
        let d = fire.take_burned(2.5).expect("burned squares");
        assert_eq!(d.kind, DisturbanceKind::Burned);
        assert!(!d.patches.is_empty());
        assert!(d.patches.iter().all(|p| {
            let (dx, dz) = (
                (p[0] * PATCH + PATCH / 2 - d.x) as f32,
                (p[1] * PATCH + PATCH / 2 - d.z) as f32,
            );
            (dx * dx + dz * dz).sqrt() <= d.radius
        }));
        assert!(fire.take_burned(2.5).is_none(), "taken once");
    }

    #[test]
    fn fine_fuel_dries_in_dry_air() {
        // About 4 % in hot dry air, 12 % at 60 %, near 20 % in damp air.
        assert!(equilibrium_moisture(20.0, 30.0) < 6.0);
        let m60 = equilibrium_moisture(60.0, 20.0);
        assert!((9.0..15.0).contains(&m60), "{m60}");
        assert!(equilibrium_moisture(90.0, 10.0) > 18.0);
    }
}

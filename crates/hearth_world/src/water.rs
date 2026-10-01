//! Water the player moves: finite and conserved, falling, spreading to level out, and settling
//! (v2 §5.4), with what is in it (v2 §9.3).
//!
//! Natural water — the sea, lakes, rivers and springs (`water[level=0]`, and plants standing in
//! it) — is a sustained reservoir that the hydrology holds at its level, and is never simulated.
//! It feeds an open block beside it at its height or under it (a dug channel fills to the
//! river's level; a pit dug under a lake bed floods) and takes in water that comes down onto it
//! (a channel drains into the sea). Groundwater seeps into holes dug below the water table, so a
//! well fills to it. Water the player moves is finite: each block of it holds 1–1000 litres and
//! their quality, kept here, and shows its volume in eighths (`water[level=1..8]`).
//!
//! Each tick the water falls, spreads by evening out with lower neighbours, and is fed or
//! drained by the natural water it touches. Evening out stops at steps of a litre and cannot
//! pass through full water, so once a stretch of water is at rest, each connected body of it
//! takes one level (`level_bodies`): connected wells reach the same height, a channel stands at
//! its river's. Only blocks that changed and their neighbours are looked at, so water at rest
//! costs nothing. Over days (`weather`) neighbouring water mixes, groundwater seeps in, and
//! water under the open sky evaporates — its salt stays, and is left as a crust when the water
//! is gone — takes the air's temperature, and grows germs when still and warm.
//!
//! The simulation keeps the litres and qualities (they are the truth; the blocks show them), and
//! puts its blocks back into cubes that load again (`restore`).

use std::collections::BTreeSet;

use hearth_math::{BlockPos, CubePos};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{BlockError, BlockRegistry, BlockStateId, CubeMap, StateFlags};

/// Litres in a full block (1 m³).
pub const FULL: u32 = 1000;
/// Litres in each of the eighths a block shows.
const EIGHTH: u32 = FULL / 8;
/// Water no deeper than this (litres on a square metre, i.e. mm) spreads no further: a film
/// that wets the ground until it evaporates.
pub const FILM: u32 = 5;
/// The most salt water can hold (g/L, halite saturation); evaporation past it leaves the rest
/// as crust.
pub const SATURATION_G_L: f32 = 360.0;
/// Salt (kg) that a block of water drying out must leave to show a crust; less stays in the
/// ground.
pub const CRUST_MIN_KG: f32 = 1.0;
/// Bodies larger than this (blocks) are not levelled at once (a sea is natural water).
const LEVEL_MAX: usize = 1 << 16;
/// Days for neighbouring water to mix halfway, by stirring and convection.
const MIX_DAYS: f32 = 0.5;

/// What is in a body or block of water (v2 §9.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quality {
    /// Dissolved salts, g/L: fresh below 0.5, the sea about 35.
    pub salinity_g_l: f32,
    /// 0–1: how likely an untreated drink is to make one ill.
    pub pathogen_risk: f32,
    pub temperature_c: f32,
}

impl Quality {
    /// Clean rain or well water.
    pub const FRESH: Quality = Quality {
        salinity_g_l: 0.2,
        pathogen_risk: 0.02,
        temperature_c: 15.0,
    };

    /// `a` litres of this mixed with `b` litres of `other`.
    pub fn mix(self, a: u32, other: Quality, b: u32) -> Quality {
        let (fa, fb) = (a as f32, b as f32);
        if fa + fb <= 0.0 {
            return self;
        }
        let m = |x: f32, y: f32| (x * fa + y * fb) / (fa + fb);
        Quality {
            salinity_g_l: m(self.salinity_g_l, other.salinity_g_l),
            pathogen_risk: m(self.pathogen_risk, other.pathogen_risk),
            temperature_c: m(self.temperature_c, other.temperature_c),
        }
    }
}

/// A block of finite water.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Parcel {
    pub litres: u32,
    pub quality: Quality,
    /// Evaporated or seeped litres not yet taken off or added (fractions).
    pending: f32,
    /// Days since the water last moved.
    still_days: f32,
}

impl Parcel {
    fn new(litres: u32, quality: Quality) -> Self {
        Self {
            litres,
            quality,
            pending: 0.0,
            still_days: 0.0,
        }
    }
}

/// Groundwater around a block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ground {
    /// Height of the water table (Y).
    pub table: f32,
    /// How fast it seeps into an open hole below the table (litres a day).
    pub seep_l_per_day: f32,
    pub quality: Quality,
}

/// What the water needs to know about the world around it.
pub trait WaterEnv {
    /// The quality of the natural water at a block (a reservoir feeding finite water).
    fn natural(&self, pos: BlockPos) -> Quality;
    /// Air temperature (°C) and the evaporation from open water (mm a day) at a block now.
    fn weather(&self, pos: BlockPos) -> (f32, f32);
    /// The groundwater around a block, where the rock holds some.
    fn ground(&self, pos: BlockPos) -> Option<Ground>;
}

/// Water that crossed the simulation's bounds, in litres.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WaterBudget {
    /// Fed in by natural water and groundwater.
    pub sourced: u64,
    /// Taken in by natural water.
    pub drained: u64,
    /// Lost to the air.
    pub evaporated: u64,
    /// Poured in and taken out by the player.
    pub poured: u64,
    pub taken: u64,
    /// Pushed out by a block put in its place, with nowhere to go.
    pub spilled: u64,
    /// Salt left as crust (kg).
    pub salt_kg: f64,
}

/// What a block is to the water.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cell {
    /// Solid, or in a cube not loaded: water cannot enter.
    Solid,
    /// Air or something water washes away (plants, crusts).
    Open,
    /// Finite water: its litres.
    Finite(u32),
    /// A natural reservoir.
    Natural,
}

const SIDES: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// The six blocks sharing a face with `p`.
fn faces(p: BlockPos) -> [BlockPos; 6] {
    [
        BlockPos::new(p.x + 1, p.y, p.z),
        BlockPos::new(p.x - 1, p.y, p.z),
        BlockPos::new(p.x, p.y, p.z + 1),
        BlockPos::new(p.x, p.y, p.z - 1),
        p.up(),
        p.down(),
    ]
}

/// The finite water of a world.
pub struct WaterSim {
    parcels: FxHashMap<BlockPos, Parcel>,
    /// Salt crusts the water left (kg per block).
    crusts: FxHashMap<BlockPos, f32>,
    /// Holes below the water table that groundwater seeps into.
    seeps: BTreeSet<BlockPos>,
    /// Blocks to look at next tick.
    active: BTreeSet<BlockPos>,
    /// `water[level=0..8]` and the salt crust.
    water: [BlockStateId; 9],
    crust: BlockStateId,
    pub budget: WaterBudget,
    /// Blocks set by the last call (for relighting and remeshing).
    changed: Vec<BlockPos>,
    /// The blocks whose water changed; those changed since the water last rested.
    touched: FxHashSet<BlockPos>,
    recent: FxHashSet<BlockPos>,
    ticks: u64,
}

impl WaterSim {
    pub fn new(reg: &BlockRegistry) -> Result<Self, BlockError> {
        let mut water = [BlockStateId::AIR; 9];
        for (level, s) in water.iter_mut().enumerate() {
            *s = reg.parse_state(&format!("water[level={level}]"))?;
        }
        Ok(Self {
            parcels: FxHashMap::default(),
            crusts: FxHashMap::default(),
            seeps: BTreeSet::new(),
            active: BTreeSet::new(),
            water,
            crust: reg.parse_state("salt_crust")?,
            budget: WaterBudget::default(),
            changed: Vec::new(),
            touched: FxHashSet::default(),
            recent: FxHashSet::default(),
            ticks: 0,
        })
    }

    /// The finite water in a block.
    pub fn parcel(&self, map: &CubeMap, pos: BlockPos) -> Option<&Parcel> {
        self.parcels.get(&map.planet().wrap_block(pos))
    }

    /// All finite water (litres).
    pub fn total_litres(&self) -> u64 {
        self.parcels.values().map(|p| p.litres as u64).sum()
    }

    /// The salt crust in a block (kg).
    pub fn crust_kg(&self, map: &CubeMap, pos: BlockPos) -> f32 {
        self.crusts
            .get(&map.planet().wrap_block(pos))
            .copied()
            .unwrap_or(0.0)
    }

    /// Whether anything is still moving.
    pub fn is_settled(&self) -> bool {
        self.active.is_empty()
    }

    /// Blocks the last call set (to relight and remesh).
    pub fn changed(&self) -> &[BlockPos] {
        &self.changed
    }

    fn cell(&self, map: &CubeMap, reg: &BlockRegistry, pos: BlockPos) -> Cell {
        let Some(s) = map.block(pos) else {
            return Cell::Solid;
        };
        if let Some(p) = self.parcels.get(&pos) {
            return Cell::Finite(p.litres);
        }
        if s == BlockStateId::AIR {
            return Cell::Open;
        }
        if reg.has(s, StateFlags::WATER) {
            // The litres kept here are the truth: a finite water block without any (its water
            // just moved on, its block not set yet) is open.
            return if self.water[1..].contains(&s) {
                Cell::Open
            } else {
                Cell::Natural
            };
        }
        if reg.has(s, StateFlags::REPLACEABLE)
            && !reg.has(s, StateFlags::FLUID)
            && !reg.has(s, StateFlags::FULL_COLLISION)
        {
            return Cell::Open;
        }
        Cell::Solid
    }

    /// Adds water to a block (no budget): what did not fit.
    fn add(
        &mut self,
        map: &CubeMap,
        reg: &BlockRegistry,
        pos: BlockPos,
        litres: u32,
        quality: Quality,
    ) -> u32 {
        if litres == 0 {
            return 0;
        }
        match self.cell(map, reg, pos) {
            Cell::Open => {
                let t = litres.min(FULL);
                let mut p = Parcel::new(t, quality);
                // Water dissolves a salt crust it covers.
                if let Some(kg) = self.crusts.remove(&pos) {
                    p.quality.salinity_g_l += kg * 1000.0 / t as f32;
                    self.budget.salt_kg -= kg as f64;
                }
                self.parcels.insert(pos, p);
                self.touched.insert(pos);
                litres - t
            }
            Cell::Finite(l) => {
                let t = litres.min(FULL - l);
                if t == 0 {
                    return litres;
                }
                let p = self.parcels.get_mut(&pos).expect("finite water has litres");
                p.quality = p.quality.mix(p.litres, quality, t);
                p.litres += t;
                self.touched.insert(pos);
                litres - t
            }
            Cell::Natural => {
                self.budget.drained += litres as u64;
                0
            }
            Cell::Solid => litres,
        }
    }

    /// Pours water into a block (a container emptied, rain caught): what did not fit.
    pub fn pour(
        &mut self,
        map: &mut CubeMap,
        reg: &BlockRegistry,
        pos: BlockPos,
        litres: u32,
        quality: Quality,
    ) -> u32 {
        self.changed.clear();
        let pos = map.planet().wrap_block(pos);
        let left = self.add(map, reg, pos, litres, quality);
        self.budget.poured += (litres - left) as u64;
        self.settle_blocks(map, reg);
        left
    }

    /// Takes up to `litres` from a block of water (a container filled): the litres taken and
    /// their quality. Natural water gives without running out.
    pub fn take(
        &mut self,
        map: &mut CubeMap,
        reg: &BlockRegistry,
        env: &dyn WaterEnv,
        pos: BlockPos,
        litres: u32,
    ) -> Option<(u32, Quality)> {
        self.changed.clear();
        let pos = map.planet().wrap_block(pos);
        match self.cell(map, reg, pos) {
            Cell::Natural => Some((litres, env.natural(pos))),
            Cell::Finite(_) => {
                let p = self.parcels.get_mut(&pos).expect("finite water has litres");
                let t = litres.min(p.litres);
                let q = p.quality;
                p.litres -= t;
                if p.litres == 0 {
                    self.parcels.remove(&pos);
                }
                self.budget.taken += t as u64;
                self.touched.insert(pos);
                self.settle_blocks(map, reg);
                Some((t, q))
            }
            _ => None,
        }
    }

    /// Tells the water a block changed (dug out, built, a dam opened): it wakes there and
    /// around. Finite water a new block took the place of rises into the block above if it
    /// can, and is otherwise spilled.
    pub fn block_changed(&mut self, map: &mut CubeMap, reg: &BlockRegistry, pos: BlockPos) {
        self.changed.clear();
        let pos = map.planet().wrap_block(pos);
        if let Some(p) = self.parcels.get(&pos).copied() {
            let s = map.block(pos);
            if s.is_none_or(|s| !self.water[1..].contains(&s)) {
                self.parcels.remove(&pos);
                let up = map.planet().wrap_block(pos.up());
                let left = self.add(map, reg, up, p.litres, p.quality);
                self.budget.spilled += left as u64;
            }
        }
        self.wake(map, pos);
        self.settle_blocks(map, reg);
    }

    /// Puts the water's blocks back into a cube that was loaded again.
    pub fn restore(&mut self, map: &mut CubeMap, reg: &BlockRegistry, cube: CubePos) {
        self.changed.clear();
        let mut here: Vec<BlockPos> = self
            .parcels
            .keys()
            .chain(self.crusts.keys())
            .filter(|p| p.cube() == cube)
            .copied()
            .collect();
        here.sort_unstable();
        here.dedup();
        for p in here {
            self.touched.insert(p);
        }
        self.settle_blocks(map, reg);
    }

    /// Wakes a block and its neighbours for the next tick.
    fn wake(&mut self, map: &CubeMap, pos: BlockPos) {
        let planet = map.planet();
        self.active.insert(pos);
        for n in faces(pos) {
            self.active.insert(planet.wrap_block(n));
        }
    }

    /// Sets the blocks of the water that changed, and wakes them.
    fn settle_blocks(&mut self, map: &mut CubeMap, reg: &BlockRegistry) {
        let mut touched: Vec<BlockPos> = self.touched.drain().collect();
        touched.sort_unstable();
        for p in touched {
            self.recent.insert(p);
            let state = match self.parcels.get_mut(&p) {
                Some(par) => {
                    par.still_days = 0.0;
                    self.water[par.litres.div_ceil(EIGHTH).clamp(1, 8) as usize]
                }
                None => {
                    let crust = self.crusts.get(&p).copied().unwrap_or(0.0) >= CRUST_MIN_KG;
                    match map.block(p) {
                        Some(s) if self.water[1..].contains(&s) || s == self.crust => {
                            if crust {
                                self.crust
                            } else {
                                BlockStateId::AIR
                            }
                        }
                        Some(s) if crust && s == BlockStateId::AIR => self.crust,
                        _ => {
                            self.wake(map, p);
                            continue;
                        }
                    }
                }
            };
            if map.set_block(p, state, reg).is_some_and(|old| old != state) {
                self.changed.push(p);
            }
            self.wake(map, p);
        }
    }

    /// Moves the water one step (about ten a second): it falls; it spreads, each block evening
    /// out with each lower neighbour in turn (the order alternating from tick to tick, so it
    /// spreads alike every way); natural water feeds and drains what it touches; and once the
    /// water is at rest, each body of it that changed takes one level (`level_bodies`).
    /// Returns the blocks it set.
    pub fn tick(
        &mut self,
        map: &mut CubeMap,
        reg: &BlockRegistry,
        env: &dyn WaterEnv,
    ) -> &[BlockPos] {
        self.changed.clear();
        if self.active.is_empty() {
            return &self.changed;
        }
        self.ticks += 1;
        let planet = *map.planet();
        let active: Vec<BlockPos> = std::mem::take(&mut self.active).into_iter().collect();
        // Falling, lowest first, so a column of water comes down together.
        let mut low_first = active.clone();
        low_first.sort_unstable_by_key(|p| (p.y, p.z, p.x));
        for &p in &low_first {
            let Some(par) = self.parcels.get(&p).copied() else {
                continue;
            };
            let below = planet.wrap_block(p.down());
            let moved = match self.cell(map, reg, below) {
                Cell::Open | Cell::Finite(_) => {
                    par.litres - self.add(map, reg, below, par.litres, par.quality)
                }
                Cell::Natural => {
                    self.budget.drained += par.litres as u64;
                    par.litres
                }
                Cell::Solid => 0,
            };
            if moved > 0 {
                let rest = par.litres - moved;
                if rest == 0 {
                    self.parcels.remove(&p);
                } else if let Some(q) = self.parcels.get_mut(&p) {
                    q.litres = rest;
                }
                self.touched.insert(p);
            }
        }
        // Spreading: half of each difference to a lower neighbour, in turn, so nothing gives
        // more than evens it out.
        let forward = self.ticks.is_multiple_of(2);
        let first_side = (self.ticks / 2 % 4) as usize;
        for k in 0..active.len() {
            let p = active[if forward { k } else { active.len() - 1 - k }];
            if !self.parcels.contains_key(&p) || !self.held(map, reg, p) {
                continue;
            }
            for j in 0..4 {
                let (dx, dz) = SIDES[(first_side + j) % 4];
                let Some(par) = self.parcels.get(&p).copied() else {
                    break;
                };
                if par.litres <= FILM {
                    break;
                }
                let n = planet.wrap_block(BlockPos::new(p.x + dx, p.y, p.z + dz));
                let ln = match self.cell(map, reg, n) {
                    Cell::Open => 0,
                    Cell::Finite(l) => l,
                    Cell::Solid | Cell::Natural => continue,
                };
                if par.litres >= ln + 2 {
                    let t = (par.litres - ln) / 2;
                    self.parcels.get_mut(&p).expect("checked").litres -= t;
                    self.touched.insert(p);
                    let left = self.add(map, reg, n, t, par.quality);
                    debug_assert_eq!(left, 0, "evening out never overfills");
                }
            }
        }
        // Natural water feeds open and finite blocks beside it at its height, and under it;
        // holes below the water table take in groundwater (over days, `weather`).
        let mut check: Vec<BlockPos> = active;
        check.extend(self.touched.iter().copied());
        check.sort_unstable();
        check.dedup();
        for p in check {
            let have = match self.cell(map, reg, p) {
                Cell::Open => 0,
                Cell::Finite(l) if l < FULL => l,
                _ => continue,
            };
            let below = planet.wrap_block(p.down());
            // Water that would only pass on into natural water below is the hydrology's.
            if self.cell(map, reg, below) == Cell::Natural {
                continue;
            }
            if let Some(n) = self.natural_beside(map, reg, p) {
                let t = FULL - have;
                self.add(map, reg, p, t, env.natural(n));
                self.budget.sourced += t as u64;
            } else if env.ground(p).is_some_and(|g| (p.y as f32) < g.table)
                && faces(p)
                    .iter()
                    .any(|&n| self.cell(map, reg, planet.wrap_block(n)) == Cell::Solid)
            {
                self.seeps.insert(p);
            }
        }
        // Water at rest takes its level.
        if self.touched.is_empty() && !self.recent.is_empty() {
            self.level_bodies(map, reg, env);
        }
        self.settle_blocks(map, reg);
        &self.changed
    }

    /// Whether water in a block is held up: on a floor, natural water, or full water.
    fn held(&self, map: &CubeMap, reg: &BlockRegistry, p: BlockPos) -> bool {
        match self.cell(map, reg, map.planet().wrap_block(p.down())) {
            Cell::Solid | Cell::Natural => true,
            Cell::Finite(l) => l >= FULL,
            Cell::Open => false,
        }
    }

    /// Natural water beside a block at its height, or over it.
    fn natural_beside(&self, map: &CubeMap, reg: &BlockRegistry, p: BlockPos) -> Option<BlockPos> {
        let planet = map.planet();
        std::iter::once(planet.wrap_block(p.up()))
            .chain(
                SIDES
                    .iter()
                    .map(|&(dx, dz)| planet.wrap_block(BlockPos::new(p.x + dx, p.y, p.z + dz))),
            )
            .find(|&n| self.cell(map, reg, n) == Cell::Natural)
    }

    /// The top (Y) of the natural water a block belongs to.
    fn natural_top(&self, map: &CubeMap, reg: &BlockRegistry, mut n: BlockPos) -> i32 {
        while self.cell(map, reg, map.planet().wrap_block(n.up())) == Cell::Natural {
            n = n.up();
        }
        n.y
    }

    /// Levels each body of resting water the last ticks changed. Evening out stops at steps of
    /// a litre (which add up along a channel) and cannot pass through full water (so connected
    /// wells would not level out); at rest, a body — water connected through its faces —
    /// takes one level: its water fills its blocks, and the open blocks right over them, from
    /// the bottom up, each level shared evenly (so it rises at most a block over its old top
    /// at a time, and settles again before rising further), with one mixed quality. A body
    /// touching natural water stands at that water's surface instead: full up to it, drained
    /// above it.
    fn level_bodies(&mut self, map: &CubeMap, reg: &BlockRegistry, env: &dyn WaterEnv) {
        let planet = *map.planet();
        let mut starts: Vec<BlockPos> = self.recent.drain().collect();
        starts.sort_unstable();
        let mut seen: FxHashSet<BlockPos> = FxHashSet::default();
        for s in starts {
            if seen.contains(&s) || !self.parcels.contains_key(&s) {
                continue;
            }
            let mut body = vec![s];
            seen.insert(s);
            let mut i = 0;
            while i < body.len() && body.len() <= LEVEL_MAX {
                let p = body[i];
                i += 1;
                for n in faces(p) {
                    let n = planet.wrap_block(n);
                    if !seen.contains(&n) && self.parcels.contains_key(&n) {
                        seen.insert(n);
                        body.push(n);
                    }
                }
            }
            if body.len() > LEVEL_MAX {
                continue;
            }
            // Where it may stand: its blocks, and the open blocks right over them.
            let mut room = body.clone();
            for &p in &body {
                let up = planet.wrap_block(p.up());
                if self.cell(map, reg, up) == Cell::Open {
                    room.push(up);
                }
            }
            room.sort_unstable_by_key(|p| (p.y, p.z, p.x));
            room.dedup();
            let (mut total, mut q) = (0u32, Quality::FRESH);
            for p in &body {
                let par = self.parcels[p];
                q = q.mix(total, par.quality, par.litres);
                total += par.litres;
            }
            let natural = body
                .iter()
                .filter_map(|&p| self.natural_beside(map, reg, p))
                .map(|n| (self.natural_top(map, reg, n), n))
                .max();
            let mut level: Vec<u32> = vec![0; room.len()];
            match natural {
                Some((top, _)) => {
                    for (l, p) in level.iter_mut().zip(&room) {
                        if p.y <= top {
                            *l = FULL;
                        }
                    }
                }
                None => {
                    let mut left = total;
                    let mut k = 0;
                    while k < room.len() && left > 0 {
                        let y = room[k].y;
                        let end = room[k..]
                            .iter()
                            .position(|p| p.y != y)
                            .map_or(room.len(), |e| k + e);
                        let n = (end - k) as u32;
                        if left >= n * FULL {
                            level[k..end].fill(FULL);
                            left -= n * FULL;
                        } else {
                            for (j, l) in level[k..end].iter_mut().enumerate() {
                                *l = left / n + u32::from((j as u32) < left % n);
                            }
                            left = 0;
                        }
                        k = end;
                    }
                }
            }
            // Only rewrite where it changes more than a litre (the remainders' places aside).
            let now = |p: &BlockPos| self.parcels.get(p).map_or(0, |par| par.litres);
            if room
                .iter()
                .zip(&level)
                .all(|(p, &l)| now(p).abs_diff(l) <= 1)
            {
                continue;
            }
            let new_total: u32 = level.iter().sum();
            if let Some((_, n)) = natural {
                if new_total > total {
                    q = q.mix(total, env.natural(n), new_total - total);
                    self.budget.sourced += (new_total - total) as u64;
                } else {
                    self.budget.drained += (total - new_total) as u64;
                }
            }
            for (p, &l) in room.iter().zip(&level) {
                if l == 0 {
                    if self.parcels.remove(p).is_some() {
                        self.touched.insert(*p);
                    }
                } else {
                    let par = self.parcels.entry(*p).or_insert_with(|| Parcel::new(l, q));
                    par.litres = l;
                    par.quality = q;
                    self.touched.insert(*p);
                }
            }
        }
    }

    /// The slow processes over `days`: neighbouring water mixes (halfway in about half a day);
    /// groundwater seeps into holes below the water table; water under the open sky evaporates
    /// (its salt stays — the excess over saturation, and all of it once the water is gone, left
    /// as crust); water takes the air's temperature (a full block in about a day, shallow water
    /// faster); and water still for half a day grows germs (toward a level that rises with
    /// warmth, in about three days). Call every few game minutes.
    pub fn weather(
        &mut self,
        map: &mut CubeMap,
        reg: &BlockRegistry,
        env: &dyn WaterEnv,
        days: f32,
    ) {
        self.changed.clear();
        let planet = *map.planet();
        let mut positions: Vec<BlockPos> = self.parcels.keys().copied().collect();
        positions.sort_unstable();
        // Mixing: each touching pair trades water, salt and heat both ways.
        let f = 1.0 - (-days / MIX_DAYS).exp();
        let mut trade: FxHashMap<BlockPos, [f32; 3]> = FxHashMap::default();
        for &p in &positions {
            let a = self.parcels[&p];
            for n in faces(p) {
                let n = planet.wrap_block(n);
                let Some(b) = self.parcels.get(&n).filter(|_| n > p) else {
                    continue;
                };
                let m = a.litres.min(b.litres) as f32 * f * 0.5;
                let d = [
                    m * (b.quality.salinity_g_l - a.quality.salinity_g_l),
                    m * (b.quality.pathogen_risk - a.quality.pathogen_risk),
                    m * (b.quality.temperature_c - a.quality.temperature_c),
                ];
                for (k, dk) in d.into_iter().enumerate() {
                    trade.entry(p).or_default()[k] += dk;
                    trade.entry(n).or_default()[k] -= dk;
                }
            }
        }
        for (p, d) in trade {
            let par = self.parcels.get_mut(&p).expect("traded");
            let l = par.litres as f32;
            par.quality.salinity_g_l += d[0] / l;
            par.quality.pathogen_risk += d[1] / l;
            par.quality.temperature_c += d[2] / l;
        }
        // Groundwater into the holes below the table, up to the table.
        let seeps: Vec<BlockPos> = self.seeps.iter().copied().collect();
        for p in seeps {
            let ground = env.ground(p).filter(|g| (p.y as f32) < g.table);
            let have = match self.cell(map, reg, p) {
                Cell::Open => 0,
                Cell::Finite(l) => l,
                _ => {
                    self.seeps.remove(&p);
                    continue;
                }
            };
            let Some(g) = ground else {
                self.seeps.remove(&p);
                continue;
            };
            if have >= FULL {
                continue;
            }
            let pending = self.parcels.get(&p).map_or(0.0, |par| par.pending);
            let due = pending.max(0.0) + g.seep_l_per_day * days;
            let t = (due.floor() as u32).min(FULL - have);
            if t > 0 {
                self.add(map, reg, p, t, g.quality);
                self.budget.sourced += t as u64;
            }
            if let Some(par) = self.parcels.get_mut(&p) {
                par.pending = (due - t as f32).min(1.0);
            }
        }
        for p in positions {
            let (air_c, evap_mm) = env.weather(p);
            let up = planet.wrap_block(p.up());
            let sky = if self.cell(map, reg, up) == Cell::Open {
                map.sky_light(up) as f32 / 15.0
            } else {
                0.0
            };
            let Some(par) = self.parcels.get_mut(&p) else {
                continue;
            };
            let mut evaporated = par.pending.min(0.0) - evap_mm.max(0.0) * days * sky;
            let lost = ((-evaporated).floor().max(0.0) as u32).min(par.litres);
            evaporated += lost as f32;
            par.pending = if par.pending > 0.0 {
                par.pending
            } else {
                evaporated.min(0.0)
            };
            let mut salt_out_kg = 0.0;
            if lost > 0 {
                let salt_g = par.quality.salinity_g_l * par.litres as f32;
                par.litres -= lost;
                self.budget.evaporated += lost as u64;
                if par.litres == 0 {
                    salt_out_kg = salt_g / 1000.0;
                } else {
                    let s = salt_g / par.litres as f32;
                    if s > SATURATION_G_L {
                        salt_out_kg = (s - SATURATION_G_L) * par.litres as f32 / 1000.0;
                    }
                    par.quality.salinity_g_l = s.min(SATURATION_G_L);
                }
                self.touched.insert(p);
            }
            let tau = 0.1 + par.litres as f32 / FULL as f32;
            par.quality.temperature_c +=
                (air_c - par.quality.temperature_c) * (1.0 - (-days / tau).exp());
            par.still_days += days;
            if par.still_days > 0.5 {
                let u = ((par.quality.temperature_c - 8.0) / 17.0).clamp(0.0, 1.0);
                let level = 0.1 + 0.6 * u * u * (3.0 - 2.0 * u);
                if level > par.quality.pathogen_risk {
                    par.quality.pathogen_risk +=
                        (level - par.quality.pathogen_risk) * (1.0 - (-days / 3.0).exp());
                }
            }
            if par.litres == 0 {
                self.parcels.remove(&p);
            }
            if salt_out_kg > 0.0 {
                *self.crusts.entry(p).or_default() += salt_out_kg;
                self.budget.salt_kg += salt_out_kg as f64;
            }
        }
        // Setting the blocks resets stillness: keep the days of water that only evaporated.
        let still: Vec<(BlockPos, f32)> = self
            .touched
            .iter()
            .filter_map(|p| self.parcels.get(p).map(|q| (*p, q.still_days)))
            .collect();
        self.settle_blocks(map, reg);
        for (p, d) in still {
            if let Some(q) = self.parcels.get_mut(&p) {
                q.still_days = d;
            }
        }
    }

    /// Takes the salt crust from a block (kg), leaving air.
    pub fn take_crust(&mut self, map: &mut CubeMap, reg: &BlockRegistry, pos: BlockPos) -> f32 {
        self.changed.clear();
        let pos = map.planet().wrap_block(pos);
        let kg = self.crusts.remove(&pos).unwrap_or(0.0);
        self.budget.salt_kg -= kg as f64;
        if map.block(pos) == Some(self.crust)
            && map.set_block(pos, BlockStateId::AIR, reg).is_some()
        {
            self.changed.push(pos);
            self.wake(map, pos);
        }
        kg
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use hearth_math::Planet;

    use super::*;
    use crate::Cube;

    struct Env {
        natural: Quality,
        air_c: f32,
        evap_mm: f32,
        ground: Option<Ground>,
    }

    impl WaterEnv for Env {
        fn natural(&self, _: BlockPos) -> Quality {
            self.natural
        }
        fn weather(&self, _: BlockPos) -> (f32, f32) {
            (self.air_c, self.evap_mm)
        }
        fn ground(&self, _: BlockPos) -> Option<Ground> {
            self.ground
        }
    }

    const SEA: Quality = Quality {
        salinity_g_l: 35.0,
        pathogen_risk: 0.1,
        temperature_c: 20.0,
    };

    fn env() -> Env {
        Env {
            natural: SEA,
            air_c: 20.0,
            evap_mm: 0.0,
            ground: None,
        }
    }

    fn reg() -> BlockRegistry {
        crate::datapack::load_builtin_registry().expect("registry")
    }

    /// A world of granite below y = 16 and air above, in a 3×3 patch of cube columns around
    /// the origin.
    fn world(reg: &BlockRegistry) -> CubeMap {
        let planet = Planet::new(hearth_math::PlanetSize::Tiny.circumference()).expect("planet");
        let mut map = CubeMap::new(planet);
        let granite = reg.parse_state("granite").expect("granite");
        for cx in -1..=1 {
            for cz in -1..=1 {
                map.insert_cube(
                    CubePos::new(cx, 0, cz),
                    Arc::new(Cube::filled(granite)),
                    reg,
                );
                map.insert_cube(
                    CubePos::new(cx, 1, cz),
                    Arc::new(Cube::filled(BlockStateId::AIR)),
                    reg,
                );
            }
        }
        map
    }

    /// Digs a box (inclusive corners) out of the rock.
    fn dig(map: &mut CubeMap, reg: &BlockRegistry, a: (i32, i32, i32), b: (i32, i32, i32)) {
        for x in a.0..=b.0 {
            for y in a.1..=b.1 {
                for z in a.2..=b.2 {
                    map.set_block(BlockPos::new(x, y, z), BlockStateId::AIR, reg);
                }
            }
        }
    }

    fn run(sim: &mut WaterSim, map: &mut CubeMap, reg: &BlockRegistry, env: &Env, ticks: usize) {
        for _ in 0..ticks {
            sim.tick(map, reg, env);
            if sim.is_settled() {
                break;
            }
        }
    }

    fn litres(sim: &WaterSim, map: &CubeMap, x: i32, y: i32, z: i32) -> u32 {
        sim.parcel(map, BlockPos::new(x, y, z))
            .map_or(0, |p| p.litres)
    }

    #[test]
    fn poured_water_levels_out_and_keeps_its_volume() {
        let reg = reg();
        let mut map = world(&reg);
        let (e, mut sim) = (env(), WaterSim::new(&reg).expect("sim"));
        // A 4×4 basin one block deep.
        dig(&mut map, &reg, (0, 15, 0), (3, 15, 3));
        let at = |x, z| BlockPos::new(x, 15, z);
        assert_eq!(sim.pour(&mut map, &reg, at(0, 0), 1000, Quality::FRESH), 0);
        assert_eq!(sim.pour(&mut map, &reg, at(1, 0), 1000, Quality::FRESH), 0);
        assert_eq!(sim.pour(&mut map, &reg, at(0, 1), 1000, Quality::FRESH), 0);
        assert_eq!(
            sim.pour(&mut map, &reg, at(0, 0), 1000, Quality::FRESH),
            1000
        );
        run(&mut sim, &mut map, &reg, &e, 2000);
        assert!(sim.is_settled(), "settles");
        assert_eq!(sim.total_litres(), 3000, "conserved");
        for x in 0..4 {
            for z in 0..4 {
                let l = litres(&sim, &map, x, 15, z);
                assert!((187..=188).contains(&l), "level at ({x}, {z}): {l}");
                assert_eq!(map.block(at(x, z)), Some(sim.water[2]));
            }
        }
    }

    #[test]
    fn water_runs_downhill_and_pools() {
        let reg = reg();
        let mut map = world(&reg);
        let (e, mut sim) = (env(), WaterSim::new(&reg).expect("sim"));
        // A staircase down to a pit: steps at y 15, 14, 13, then a 2×1 pit at y 11..12, all
        // under a roof so nothing spreads over the ground.
        let granite = reg.parse_state("granite").expect("granite");
        dig(&mut map, &reg, (0, 15, 0), (0, 15, 0));
        dig(&mut map, &reg, (1, 14, 0), (1, 15, 0));
        dig(&mut map, &reg, (2, 13, 0), (2, 15, 0));
        dig(&mut map, &reg, (3, 11, 0), (4, 15, 0));
        sim.pour(
            &mut map,
            &reg,
            BlockPos::new(0, 15, 0),
            1000,
            Quality::FRESH,
        );
        sim.pour(&mut map, &reg, BlockPos::new(1, 15, 0), 600, Quality::FRESH);
        for x in 0..=4 {
            map.set_block(BlockPos::new(x, 16, 0), granite, &reg);
        }
        run(&mut sim, &mut map, &reg, &e, 5000);
        assert!(sim.is_settled());
        assert_eq!(sim.total_litres(), 1600);
        // Films stay on the steps; the pit's bottom holds the rest, level.
        let films: u32 = (0..=2)
            .flat_map(|x| (13..=15).map(move |y| (x, y)))
            .map(|(x, y)| litres(&sim, &map, x, y, 0))
            .sum();
        assert!(films <= 3 * FILM, "films {films}");
        let (a, b) = (litres(&sim, &map, 3, 11, 0), litres(&sim, &map, 4, 11, 0));
        assert!(
            a.abs_diff(b) <= 1 && a + b + films == 1600,
            "{a} {b} {films}"
        );
        assert_eq!(
            litres(&sim, &map, 3, 12, 0) + litres(&sim, &map, 4, 12, 0),
            0
        );
    }

    #[test]
    fn connected_vessels_reach_one_level() {
        let reg = reg();
        let mut map = world(&reg);
        let (e, mut sim) = (env(), WaterSim::new(&reg).expect("sim"));
        // Two wells four deep joined by a tunnel at the bottom; the left one filled.
        dig(&mut map, &reg, (0, 12, 0), (0, 15, 0));
        dig(&mut map, &reg, (2, 12, 0), (2, 15, 0));
        dig(&mut map, &reg, (1, 12, 0), (1, 12, 0));
        for y in 12..=15 {
            sim.pour(&mut map, &reg, BlockPos::new(0, y, 0), 1000, Quality::FRESH);
        }
        run(&mut sim, &mut map, &reg, &e, 20_000);
        assert!(sim.is_settled());
        assert_eq!(sim.total_litres(), 4000);
        // The tunnel fills; the rest stands at the same height in both wells.
        for x in 0..=2 {
            assert_eq!(litres(&sim, &map, x, 12, 0), FULL, "bottom row at x {x}");
        }
        let (a, b) = (litres(&sim, &map, 0, 13, 0), litres(&sim, &map, 2, 13, 0));
        assert!(a.abs_diff(b) <= 1 && a + b == 1000, "{a} {b}");
        assert_eq!(litres(&sim, &map, 0, 14, 0), 0);
    }

    #[test]
    fn natural_water_feeds_a_channel_to_its_level_and_drains_what_reaches_it() {
        let reg = reg();
        let mut map = world(&reg);
        let (e, mut sim) = (env(), WaterSim::new(&reg).expect("sim"));
        let sea = reg.water_source();
        // A sea at x ≤ −2 up to y 15; a channel dug at y 15 from it.
        for z in -3..=3 {
            for x in -8..=-2 {
                map.set_block(BlockPos::new(x, 15, z), sea, &reg);
            }
        }
        dig(&mut map, &reg, (-1, 15, 0), (12, 15, 0));
        sim.block_changed(&mut map, &reg, BlockPos::new(-1, 15, 0));
        run(&mut sim, &mut map, &reg, &e, 20_000);
        assert!(sim.is_settled());
        for x in -1..=12 {
            let p = sim.parcel(&map, BlockPos::new(x, 15, 0)).expect("water");
            assert_eq!(p.litres, FULL, "x {x}");
            assert!((p.quality.salinity_g_l - 35.0).abs() < 1e-3, "sea water");
        }
        assert_eq!(sim.budget.sourced, 14 * FULL as u64);
        // Water poured on the shore above the sea runs into it.
        let drained = sim.budget.drained;
        sim.pour(
            &mut map,
            &reg,
            BlockPos::new(-3, 16, 0),
            500,
            Quality::FRESH,
        );
        run(&mut sim, &mut map, &reg, &e, 1000);
        assert_eq!(sim.budget.drained, drained + 500);
        assert!(sim.parcel(&map, BlockPos::new(-3, 16, 0)).is_none());
        // Natural water is never touched.
        assert_eq!(map.block(BlockPos::new(-2, 15, 0)), Some(sea));
    }

    #[test]
    fn a_well_below_the_water_table_fills_to_it() {
        let reg = reg();
        let mut map = world(&reg);
        let mut e = env();
        e.ground = Some(Ground {
            table: 12.5,
            seep_l_per_day: 400.0,
            quality: Quality::FRESH,
        });
        let mut sim = WaterSim::new(&reg).expect("sim");
        // A shaft from the surface down to y 9.
        dig(&mut map, &reg, (0, 9, 0), (0, 15, 0));
        for y in 9..=15 {
            sim.block_changed(&mut map, &reg, BlockPos::new(0, y, 0));
        }
        for _ in 0..40 {
            run(&mut sim, &mut map, &reg, &e, 100);
            sim.weather(&mut map, &reg, &e, 0.25);
        }
        run(&mut sim, &mut map, &reg, &e, 1000);
        // Full below the table (y 9..12), dry above.
        for y in 9..=12 {
            assert_eq!(litres(&sim, &map, 0, y, 0), FULL, "y {y}");
        }
        assert_eq!(litres(&sim, &map, 0, 13, 0), 0);
        // Drawn down, it seeps back.
        let env_no = env();
        sim.take(&mut map, &reg, &env_no, BlockPos::new(0, 12, 0), 500);
        for _ in 0..8 {
            run(&mut sim, &mut map, &reg, &e, 100);
            sim.weather(&mut map, &reg, &e, 0.25);
        }
        assert_eq!(litres(&sim, &map, 0, 12, 0), FULL, "refilled");
    }

    #[test]
    fn mixing_evaporation_and_salt_crust() {
        let reg = reg();
        let mut map = world(&reg);
        let mut e = env();
        let mut sim = WaterSim::new(&reg).expect("sim");
        dig(&mut map, &reg, (0, 15, 0), (1, 15, 0));
        sim.pour(&mut map, &reg, BlockPos::new(0, 15, 0), 1000, SEA);
        sim.pour(
            &mut map,
            &reg,
            BlockPos::new(1, 15, 0),
            1000,
            Quality::FRESH,
        );
        run(&mut sim, &mut map, &reg, &e, 20_000);
        // Side by side at one level the two mix in a few days: salt spreads by volume.
        for _ in 0..4 {
            sim.weather(&mut map, &reg, &e, 1.0);
        }
        for x in 0..=1 {
            let s = sim
                .parcel(&map, BlockPos::new(x, 15, 0))
                .expect("water")
                .quality
                .salinity_g_l;
            assert!((s - 17.6).abs() < 0.2, "salinity {s}");
        }
        let salt_kg = (SEA.salinity_g_l + Quality::FRESH.salinity_g_l) as f64;
        // A hot dry month: the brine thickens, then dries to a crust.
        e.evap_mm = 40.0;
        e.air_c = 30.0;
        for _ in 0..15 {
            sim.weather(&mut map, &reg, &e, 1.0);
        }
        assert_eq!(
            sim.total_litres(),
            800,
            "40 mm a day for 15 days off each block"
        );
        let s = sim
            .parcel(&map, BlockPos::new(0, 15, 0))
            .expect("water")
            .quality;
        assert!(
            (s.salinity_g_l - 17.6 * 2.5).abs() < 1.0,
            "concentrated {}",
            s.salinity_g_l
        );
        assert!((s.temperature_c - 30.0).abs() < 0.5, "warmed to the air");
        assert!(
            s.pathogen_risk > 0.5,
            "still warm water: {}",
            s.pathogen_risk
        );
        for _ in 0..15 {
            sim.weather(&mut map, &reg, &e, 1.0);
        }
        assert_eq!(sim.total_litres(), 0);
        let crust = sim.crust_kg(&map, BlockPos::new(0, 15, 0))
            + sim.crust_kg(&map, BlockPos::new(1, 15, 0));
        assert!(
            (crust as f64 - salt_kg).abs() < 0.05,
            "crust {crust} of {salt_kg} kg"
        );
        assert!((sim.budget.salt_kg - salt_kg).abs() < 0.05);
        assert_eq!(map.block(BlockPos::new(0, 15, 0)), Some(sim.crust));
        // Water poured back dissolves the crust.
        sim.pour(
            &mut map,
            &reg,
            BlockPos::new(0, 15, 0),
            1000,
            Quality::FRESH,
        );
        let s = sim
            .parcel(&map, BlockPos::new(0, 15, 0))
            .expect("water")
            .quality
            .salinity_g_l;
        assert!(s > 17.0, "the crust dissolves: {s}");
        assert_eq!(sim.crust_kg(&map, BlockPos::new(0, 15, 0)), 0.0);
        // A crust can be taken.
        let left = sim.crust_kg(&map, BlockPos::new(1, 15, 0));
        assert!((sim.take_crust(&mut map, &reg, BlockPos::new(1, 15, 0)) - left).abs() < 1e-6);
        assert_eq!(map.block(BlockPos::new(1, 15, 0)), Some(BlockStateId::AIR));
    }

    #[test]
    fn displaced_water_rises_and_settled_water_costs_nothing() {
        let reg = reg();
        let mut map = world(&reg);
        let (e, mut sim) = (env(), WaterSim::new(&reg).expect("sim"));
        dig(&mut map, &reg, (0, 14, 0), (0, 15, 0));
        sim.pour(
            &mut map,
            &reg,
            BlockPos::new(0, 14, 0),
            1000,
            Quality::FRESH,
        );
        run(&mut sim, &mut map, &reg, &e, 100);
        assert!(sim.is_settled());
        assert!(
            sim.tick(&mut map, &reg, &e).is_empty(),
            "nothing to do at rest"
        );
        // A block put into the water pushes it up.
        let granite = reg.parse_state("granite").expect("granite");
        map.set_block(BlockPos::new(0, 14, 0), granite, &reg);
        sim.block_changed(&mut map, &reg, BlockPos::new(0, 14, 0));
        assert_eq!(litres(&sim, &map, 0, 15, 0), FULL);
        assert_eq!(sim.total_litres(), 1000);
        // And with nowhere to go, it is spilled (counted).
        map.set_block(BlockPos::new(0, 16, 0), granite, &reg);
        map.set_block(BlockPos::new(0, 15, 0), granite, &reg);
        sim.block_changed(&mut map, &reg, BlockPos::new(0, 15, 0));
        assert_eq!(sim.total_litres(), 0);
        assert_eq!(sim.budget.spilled, 1000);
    }

    #[test]
    fn taking_water_from_finite_and_natural_water() {
        let reg = reg();
        let mut map = world(&reg);
        let (e, mut sim) = (env(), WaterSim::new(&reg).expect("sim"));
        dig(&mut map, &reg, (0, 15, 0), (0, 15, 0));
        map.set_block(BlockPos::new(2, 15, 0), reg.water_source(), &reg);
        sim.pour(&mut map, &reg, BlockPos::new(0, 15, 0), 300, Quality::FRESH);
        assert_eq!(
            sim.take(&mut map, &reg, &e, BlockPos::new(0, 15, 0), 10),
            Some((10, Quality::FRESH))
        );
        assert_eq!(litres(&sim, &map, 0, 15, 0), 290);
        assert_eq!(
            map.block(BlockPos::new(0, 15, 0)),
            Some(sim.water[3]),
            "290 L show 3/8"
        );
        let (t, _) = sim
            .take(&mut map, &reg, &e, BlockPos::new(0, 15, 0), 500)
            .expect("water");
        assert_eq!(t, 290);
        assert_eq!(map.block(BlockPos::new(0, 15, 0)), Some(BlockStateId::AIR));
        // The sea gives without running out.
        assert_eq!(
            sim.take(&mut map, &reg, &e, BlockPos::new(2, 15, 0), 10),
            Some((10, SEA))
        );
        assert_eq!(map.block(BlockPos::new(2, 15, 0)), Some(reg.water_source()));
        assert_eq!(sim.budget.taken, 300);
    }
}

//! Populations (v2 §7.4): the land in regions of 64 × 64 ecological cells of 256 m, each cell
//! holding stocks of forage and carrion that grow with the season and are eaten; large animals
//! as groups that keep their members (a herd of red deer, a wolf pack, a sow with her young),
//! small ones as numbers per cell (young and adults, with a shared condition).
//!
//! A step of `dt` years: each cell offers each kind of forage at the rate its vegetation and
//! the season give (grass and twigs standing through the winter thinner, nuts falling in
//! autumn and lasting into spring, fruit in its weeks), the carrion of the dead lies until it
//! is eaten or rots. Predators hunt first, by a functional
//! response whose attack rate is calibrated so that each meets its need for meat at the prey
//! densities its species' data give; it is of type III (predators turn from a prey that has
//! grown scarce to others), and prey that keeps to cover is partly hidden. Then everyone eats
//! what the hunt did not give them from the forage and carrion in reach, shared out cell by
//! cell when there is not enough (scramble competition; an omnivore short of meat eats more
//! plants). Condition follows what was eaten; animals die of what the simulation does not model
//! (from the species' survival), of hunger, of winter, and of disease and stress where they
//! crowd past what their habitat holds; the dead feed the scavengers. Young are born in their
//! season, more the better their mothers' condition, only to holders of a territory where the
//! species keeps one; they grow up, and the dispersing ones leave to settle where there is
//! room, in this region or the next.

use std::sync::Arc;

use hearth_content::schema::fauna::{BodyPlan, Dispersers, Social};
use hearth_math::hash::{Rng, derive_seed, hash2};
use hearth_worldgen::realms::{Realm, native, stand_in};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::habitat::{CELL_KM2, CELL_M, Habitat, Land};
use crate::live::Stage;
use crate::species::{Catalog, FORAGE_KINDS, Forage, Species};

/// Cells on a side of a region.
pub const REGION_CELLS: i64 = 64;
/// Cells in a region.
pub const REGION_LEN: usize = (REGION_CELLS * REGION_CELLS) as usize;
/// A region's side, m.
pub const REGION_M: f64 = REGION_CELLS as f64 * CELL_M;
/// The food slot of carrion, after the forage kinds.
pub const CARRION: usize = FORAGE_KINDS;
/// Food slots: the forage kinds and carrion.
pub const FOODS: usize = FORAGE_KINDS + 1;

/// How fast carrion rots, per year (a carcass lasts some weeks).
const ROT: f32 = 30.0;
/// How much of the carrion lying scavengers find in a year.
const FIND_CARRION: f32 = 20.0;

/// How fast condition moves toward what the food allows, per year (a deer on short winter
/// rations thins over a few months, and fattens again in a summer).
const LEAN: f32 = 3.0;
/// The death rate from disease, stress and want of room when animals crowd past what their
/// habitat holds, per year at twice that.
const CROWDING: f32 = 2.0;
/// The death rate of adults of a territorial species that hold no territory, per year.
const FLOATERS: f32 = 1.0;
/// Prey grown scarcer than this share of its usual density is passed over for others (the
/// type III response).
const SWITCH: f32 = 1.0;
/// The share of its prey's usual density at which a predator's attack rate is set to meet its
/// needs (with two fifths to spare).
const CALIBRATE_AT: f32 = 1.0;
/// The cover of the wood the species' densities describe.
const REFERENCE_COVER: f32 = 0.6;

/// A group of a large species.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub id: u64,
    pub species: u16,
    /// The centre of its home range, and where it is now (world x, z).
    pub home: [f64; 2],
    pub pos: [f64; 2],
    /// Young of the year, older young not yet grown, grown females and males.
    pub young: u16,
    pub juveniles: u16,
    pub females: u16,
    pub males: u16,
    /// Body condition, 0 (starving) to 1 (fat).
    pub condition: f32,
    /// A kill being eaten, kg.
    pub food: f32,
    /// Its animals stand in the world near the player; the abstract step leaves it be.
    #[serde(default)]
    pub live: bool,
}

impl Group {
    pub fn size(&self) -> u32 {
        self.young as u32 + self.juveniles as u32 + self.females as u32 + self.males as u32
    }

    pub fn adults(&self) -> u32 {
        self.females as u32 + self.males as u32
    }

    /// Mouths as adults (young eat less).
    pub fn mouths(&self) -> f32 {
        self.young as f32 * 0.4 + self.juveniles as f32 * 0.75 + self.adults() as f32
    }

    /// How easily a predator takes one of it: the young, the young-grown and the weak first.
    fn vulnerability(&self) -> f32 {
        self.young as f32 * 2.0
            + self.juveniles as f32 * 1.2
            + self.adults() as f32 * (1.0 + 1.5 * (1.0 - self.condition))
    }
}

/// Why animals died (tallied for tests and the log).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Cause {
    /// Disease, accident, old age: the species' survival.
    Natural,
    Hunger,
    Winter,
    /// Disease and stress of crowding, and adults without a territory.
    Crowding,
    Predation,
    Hunting,
    /// Left for elsewhere and did not settle.
    Lost,
}

/// What is left where a large animal died in the abstract step: its species, age and sex,
/// where (world x, z), when (years), how much of it was left then (a kill the hunters ate from,
/// a death whole), how many days half of what is left lasts (the bigger the body, the longer),
/// why it died, and when the ravens over it were last told of (years; never below zero).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Remains {
    pub species: u16,
    pub stage: Stage,
    pub female: bool,
    pub at: [f64; 2],
    pub time: f64,
    pub left: f32,
    #[serde(default = "five")]
    pub half_days: f32,
    pub cause: Cause,
    #[serde(default = "never")]
    pub told: f64,
}

fn never() -> f64 {
    -1.0
}

fn five() -> f32 {
    5.0
}

/// How many days half of a body of `kg` lasts once dead: the hunters come back to their kill,
/// the scavengers find the dead (a piglet is gone in a couple of days, a red deer lasts a week
/// or so, an aurochs a fortnight and more).
pub fn half_days(kg: f32) -> f32 {
    1.0 + 6.0 * (kg.max(0.0) / 100.0).sqrt()
}

/// How long remains lie before the scavengers have them all (years).
const REMAINS_KEPT: f64 = 30.0 / 365.0;
/// The most remains a region keeps (the newest).
const MAX_REMAINS: usize = 512;

impl Remains {
    /// What is left of it at a time.
    pub fn left_at(&self, now: f64) -> f32 {
        let days = ((now - self.time).max(0.0) * 365.0) as f32;
        self.left * 0.5f32.powf(days / self.half_days.max(0.1))
    }
}

/// A region of cells and the animals living in it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub key: (i64, i64),
    /// Years since the world began to which it is simulated.
    pub time: f64,
    #[serde(skip)]
    pub habitat: Vec<Habitat>,
    /// Per cell: the yearly means of each kind's availability (to scale the season by).
    #[serde(skip)]
    pub avail_mean: Vec<[f32; FORAGE_KINDS]>,
    /// Carrion lying in each cell, kg.
    pub carrion: Vec<f32>,
    /// Where large animals died lately.
    #[serde(default)]
    pub remains: Vec<Remains>,
    /// The small species simulated here, and per species and cell (species-major): young,
    /// adults and condition.
    pub pool_species: Vec<u16>,
    pub young: Vec<f32>,
    pub adults: Vec<f32>,
    pub cond: Vec<f32>,
    pub groups: Vec<Group>,
    /// The mast factor of the year it was last computed for, and that year's weather: how
    /// well plants grew (1 an ordinary year) and how hard the winter was (1 ordinary).
    pub mast_year: i64,
    pub mast: f32,
    #[serde(default = "one")]
    pub growth: f32,
    #[serde(default = "one")]
    pub winter: f32,
    /// Per species: the animals the region's habitat holds at the species' density.
    #[serde(skip)]
    pub capacity: Vec<f32>,
    /// Per species and block of 8 × 8 cells (2 km): the same, to judge crowding by.
    #[serde(skip)]
    pub block_capacity: Vec<f32>,
    /// Per species and cell (species-major): how well the cell feeds the species against the
    /// reference wood, 0 where it cannot live ([`Ecology::qualities`]).
    #[serde(skip)]
    pub quality: Vec<f32>,
    /// Per species and block: how rich the land about is in a hunter's prey against the
    /// reference wood (1 for the rest).
    #[serde(skip)]
    pub prey: Vec<f32>,
}

/// Cells on a side of a block (crowding is judged over the blocks about a group).
const BLOCK: i64 = 8;
/// Blocks on a side of a region.
const BLOCKS: i64 = REGION_CELLS / BLOCK;

fn block_of(cell: usize) -> usize {
    let (i, j) = (cell as i64 % REGION_CELLS, cell as i64 / REGION_CELLS);
    ((j / BLOCK) * BLOCKS + i / BLOCK) as usize
}

/// A young animal that left home for elsewhere.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Disperser {
    species: u16,
    female: bool,
    from: [f64; 2],
}

/// The whole simulation.
#[derive(Debug, Clone)]
pub struct Ecology {
    pub catalog: Arc<Catalog>,
    pub seed: u64,
    /// Year fraction at time 0 (the calendar's offset).
    pub year_offset: f64,
    pub cells_around: i64,
    pub rows: i64,
    pub regions: FxHashMap<(i64, i64), Region>,
    pub next_id: u64,
    /// Per species and realm: the predator's attack rate, km² a year per animal.
    pub attack: Vec<[f32; 8]>,
    /// Deaths by species and cause since the tally was cleared.
    pub deaths: FxHashMap<(u16, Cause), f64>,
    /// How well each species was fed (the sum of its consumers' food ratios, and their
    /// number) since the tally was cleared.
    pub fed: FxHashMap<u16, (f64, f64)>,
    /// The temperate wood the species' densities describe.
    pub reference: Habitat,
    /// Per kind of forage: the factor that anchors production to what the reference wood's
    /// animals eat at their usual densities ([`forage_scale`]).
    pub forage_scale: [f32; FORAGE_KINDS],
    /// Years a new region is run before it is used, to settle its numbers and structure.
    pub spin_up: f64,
    /// Young bound for another region, delivered after the step.
    emigrants: Vec<Disperser>,
}

/// The mast factor of a year in a region: heavy crops every few years, synchronized over a
/// wide area, little between (a mean of 1).
pub fn mast_factor(seed: u64, key: (i64, i64), year: i64) -> f32 {
    // Masting is synchronized over about 50 km.
    let wide = (key.0.div_euclid(3), key.1.div_euclid(3));
    let h = seed_of(seed, "mast", &[wide.0 as u64, wide.1 as u64, year as u64]);
    let u = hearth_math::hash::unit_f32(h);
    if u < 0.3 {
        2.2
    } else if u < 0.6 {
        0.8
    } else {
        0.25
    }
}

/// A year's weather over a wide area: how well plants grow (about 0.7 in a cold wet or dry
/// summer to 1.3 in a kind one) and how hard the winter is (snow half to twice the usual).
pub fn weather(seed: u64, key: (i64, i64), year: i64) -> (f32, f32) {
    let wide = (key.0.div_euclid(4), key.1.div_euclid(4));
    let h = seed_of(
        seed,
        "weather",
        &[wide.0 as u64, wide.1 as u64, year as u64],
    );
    let u = |k: u64| hearth_math::hash::unit_f32(hash2(h, k));
    // The sum of three uniforms: about normal, a standard deviation of a sixth.
    let n = (u(1) + u(2) + u(3) - 1.5) * 2.0 / 3.0_f32.sqrt();
    let growth = (1.0 + 0.15 * n).clamp(0.6, 1.4);
    let winter = 2.0f32.powf((u(4) - 0.5) * 2.0);
    (growth, winter)
}

fn one() -> f32 {
    1.0
}

/// A seed for a purpose and some numbers.
fn seed_of(seed: u64, purpose: &str, parts: &[u64]) -> u64 {
    parts
        .iter()
        .fold(derive_seed(seed, purpose), |h, p| hash2(h, *p))
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A cold-blooded animal's need in the warmest weeks against its yearly need.
const ECTOTHERM_PEAK: f32 = 1.6;
/// The month's mean temperature below which a migrant is away, °C.
const AWAY_BELOW_C: f32 = -5.0;

/// Asleep in its den for the winter at year fraction `f`: a hibernator while the month is cold.
pub fn asleep(sp: &Species, h: &Habitat, f: f32) -> bool {
    sp.hibernates && h.temp_at(f) < 3.0
}

/// Wintering elsewhere at year fraction `f`: a migrant while its place lies frozen hard. It is
/// fed where it is and comes back with the thaw; nothing here eats or meets it meanwhile.
pub fn away(sp: &Species, h: &Habitat, f: f32) -> bool {
    sp.migrates && h.temp_at(f) < AWAY_BELOW_C
}

/// About in its place at year fraction `f` (neither asleep nor away).
pub fn about(sp: &Species, h: &Habitat, f: f32) -> bool {
    !asleep(sp, h, f) && !away(sp, h, f)
}

/// The share of breeding females that carry young to term, from their condition.
fn fecundity(c: f32) -> f32 {
    smoothstep(0.2, 0.6, c)
}

/// The condition an animal comes to on a share of its need: fat on all of it, thin but sound
/// on nine tenths, starving on half.
fn condition_for(ratio: f32) -> f32 {
    smoothstep(0.5, 1.0, ratio)
}

/// Condition after `dt` years on `ratio` of the need (or asleep, living on its fat).
fn update_condition(c: f32, ratio: f32, sleeping: bool, slow: bool, dt: f32) -> f32 {
    // A cold-blooded animal burns little and fasts long.
    let pace = if slow { 0.3 } else { 1.0 };
    if sleeping {
        return (c - 0.35 * pace * dt).max(0.0);
    }
    let target = condition_for(ratio);
    target + (c - target) * (-LEAN * pace * dt).exp()
}

/// Hunger's death rate, per year.
fn starving(c: f32) -> f32 {
    5.0 * smoothstep(0.3, 0.0, c)
}

/// The share of its yearly need an animal has now: the warm-blooded need less in winter (less
/// active, a slower metabolism) and more in deep snow; the cold-blooded eat with the warmth.
fn seasonal_need(sp: &Species, h: &Habitat, f: f32, snow: f32) -> f32 {
    if sp.ectotherm {
        ECTOTHERM_PEAK * ((h.temp_at(f) - 4.0) / 12.0).clamp(0.0, 1.0)
    } else {
        (0.75 + 0.25 * h.growth_at(f)) * (1.0 + 0.25 * snow)
    }
}

/// Draws from a binomial (exact for small n, normal beyond).
fn binomial(rng: &mut Rng, n: u32, p: f32) -> u32 {
    if n == 0 || p <= 0.0 {
        return 0;
    }
    if p >= 1.0 {
        return n;
    }
    if n <= 40 {
        return (0..n).filter(|_| rng.next_f32() < p).count() as u32;
    }
    let mean = n as f32 * p;
    let sd = (mean * (1.0 - p)).sqrt();
    (mean + sd * normal(rng)).round().clamp(0.0, n as f32) as u32
}

fn normal(rng: &mut Rng) -> f32 {
    let u1 = rng.next_f32().max(1e-7);
    let u2 = rng.next_f32();
    (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos()
}

/// Draws from a Poisson distribution.
pub fn poisson(rng: &mut Rng, mean: f32) -> u32 {
    if mean <= 0.0 {
        return 0;
    }
    if mean > 30.0 {
        return (mean + mean.sqrt() * normal(rng)).round().max(0.0) as u32;
    }
    let l = (-mean).exp();
    let mut k = 0;
    let mut p = 1.0;
    loop {
        p *= rng.next_f32();
        if p <= l {
            return k;
        }
        k += 1;
    }
}

/// How much of the years `[a0, a1]` (year fractions, unwrapped) falls in a season `len` long
/// about the year fraction `centre`, each year.
fn season_overlap(a0: f64, a1: f64, centre: f64, len: f64) -> f64 {
    let (s0, s1) = (centre - 0.5 * len, centre + 0.5 * len);
    let first = (a0 - s1).floor() as i64;
    let last = (a1 - s0).ceil() as i64;
    (first..=last)
        .map(|k| {
            let (lo, hi) = (s0 + k as f64, s1 + k as f64);
            (a1.min(hi) - a0.max(lo)).max(0.0)
        })
        .sum()
}

/// Whether the year fraction `b` falls in `(a0, a1]` (wrapping).
fn crossed(a0: f64, a1: f64, b: f64) -> bool {
    if a1 - a0 >= 1.0 {
        return true;
    }
    let (s, e) = (a0.rem_euclid(1.0), a1.rem_euclid(1.0));
    if s <= e {
        b > s && b <= e
    } else {
        b > s || b <= e
    }
}

/// The realm whose animals live in a cell: its own where the catalog has a fauna of that realm
/// for the cell's ecosystems, else the stand-in's for its climate (the Afrotropical's in the
/// tropics, the Palearctic's elsewhere) — a realm with only a species or two of its own for the
/// place (a honey bee in a southern temperate wood) has none yet.
pub fn fauna_realm(cat: &Catalog, h: &Habitat) -> Realm {
    let realm = h.realm();
    let tropical = h.temp_c - (h.warm_c - h.temp_c) > 15.0;
    let other = stand_in(if tropical {
        hearth_worldgen::planet::climate::ClimateClass::TropicalRainforest
    } else {
        hearth_worldgen::planet::climate::ClimateClass::Oceanic
    });
    let natives = |r: Realm| {
        cat.species
            .iter()
            .filter(|s| s.habitats & h.ecosystems != 0 && s.realms != 0 && native(s.realms, r))
            .count()
    };
    if realm == other || natives(realm) * 4 >= natives(other) {
        realm
    } else {
        other
    }
}

/// Distance between two world positions, wrapping east–west.
pub fn dist(a: [f64; 2], b: [f64; 2], wrap: f64) -> f64 {
    let mut dx = (a[0] - b[0]).rem_euclid(wrap);
    if dx > wrap * 0.5 {
        dx = wrap - dx;
    }
    (dx * dx + (a[1] - b[1]).powi(2)).sqrt()
}

/// The part of a prey animal a predator eats, kg.
fn edible(prey: &Species) -> f32 {
    if prey.mass_kg < 5.0 {
        prey.mass_kg * 0.8
    } else {
        prey.mass_kg * 0.6
    }
}

/// How visible prey at density `d` (per km²) is to a predator: the type III response, prey
/// passed over as it grows scarce against its usual density `usual`.
fn noticed(pred: &Species, d: f32, usual: f32) -> f32 {
    let s = switching(pred) * usual;
    if d + s <= 0.0 { 0.0 } else { d * d / (d + s) }
}

/// How readily a predator turns from a prey grown scarce: a generalist (with plants to eat,
/// or many prey) at once, a specialist (a snake with its voles and frogs) not at all — it
/// searches the harder.
fn switching(pred: &Species) -> f32 {
    if specialist(pred) { 0.0 } else { SWITCH }
}

/// A predator with nothing but a prey or two to live on.
fn specialist(pred: &Species) -> bool {
    pred.forage_share() < 0.05 && pred.prey.len() <= 2
}

/// The density the hunt sees of a prey species at its usual numbers, per km².
fn usual_density(prey: &Species) -> f32 {
    if prey.grouped() {
        prey.density * 1.4
    } else {
        prey.density * 1.15 * (1.0 - 0.5 * REFERENCE_COVER * prey.cover)
    }
}

impl Region {
    /// The local index of the cell at a world position, if it is in this region.
    pub fn cell_at(&self, eco_cells_around: i64, x: f64, z: f64) -> Option<usize> {
        let ci = (x / CELL_M).floor() as i64;
        let cj = (z / CELL_M).floor() as i64;
        let (i0, j0) = (self.key.0 * REGION_CELLS, self.key.1 * REGION_CELLS);
        let di = (ci - i0).rem_euclid(eco_cells_around);
        let dj = cj - j0;
        (di < REGION_CELLS && (0..REGION_CELLS).contains(&dj))
            .then(|| (dj * REGION_CELLS + di) as usize)
    }

    /// The world position of a local cell's centre.
    pub fn cell_centre(&self, idx: usize) -> [f64; 2] {
        let (i, j) = (idx as i64 % REGION_CELLS, idx as i64 / REGION_CELLS);
        [
            ((self.key.0 * REGION_CELLS + i) as f64 + 0.5) * CELL_M,
            ((self.key.1 * REGION_CELLS + j) as f64 + 0.5) * CELL_M,
        ]
    }

    /// The local cells within `r` metres of a world position (clipped to the region).
    pub fn cells_within(&self, around: i64, p: [f64; 2], r: f64, out: &mut Vec<usize>) {
        out.clear();
        let i0 = self.key.0 * REGION_CELLS;
        let j0 = self.key.1 * REGION_CELLS;
        let ci = (p[0] / CELL_M).floor() as i64;
        let cj = (p[1] / CELL_M).floor() as i64;
        let k = (r / CELL_M).ceil() as i64;
        let rr = (r / CELL_M).powi(2);
        for dj in -k..=k {
            let j = cj + dj - j0;
            if !(0..REGION_CELLS).contains(&j) {
                continue;
            }
            for di in -k..=k {
                if (di * di + dj * dj) as f64 > rr + 0.5 {
                    continue;
                }
                let i = (ci + di - i0).rem_euclid(around);
                if i < REGION_CELLS {
                    out.push((j * REGION_CELLS + i) as usize);
                }
            }
        }
        if out.is_empty()
            && let Some(c) = self.cell_at(around, p[0], p[1])
        {
            out.push(c);
        }
    }

    /// Individuals of a pool species (by slot) in the region.
    pub fn pool_total(&self, slot: usize) -> f64 {
        let r = slot * REGION_LEN..(slot + 1) * REGION_LEN;
        self.young[r.clone()]
            .iter()
            .chain(&self.adults[r])
            .map(|v| *v as f64)
            .sum()
    }
}

/// What the hunt gave: meat eaten by each group (by index) and each pool (slot × cell), kg.
struct Hunted {
    groups: Vec<f32>,
    pools: Vec<f32>,
}

impl Ecology {
    pub fn new(catalog: Arc<Catalog>, seed: u64, year_offset: f64, land: &dyn Land) -> Self {
        let attack = calibrate_attack(&catalog);
        let reference = crate::habitat::temperate_wood(&catalog);
        let scale = forage_scale(&catalog, &reference);
        Self {
            catalog,
            seed,
            year_offset,
            cells_around: land.cells_around(),
            rows: land.rows(),
            regions: FxHashMap::default(),
            next_id: 1,
            attack,
            deaths: FxHashMap::default(),
            fed: FxHashMap::default(),
            reference,
            forage_scale: scale,
            spin_up: 3.0,
            emigrants: Vec::new(),
        }
    }

    /// The key of the region holding a world position.
    pub fn region_key(&self, x: f64, z: f64) -> (i64, i64) {
        let ci = ((x / CELL_M).floor() as i64).rem_euclid(self.cells_around);
        let cj = (z / CELL_M).floor() as i64;
        (ci.div_euclid(REGION_CELLS), cj.div_euclid(REGION_CELLS))
    }

    pub fn wrap_m(&self) -> f64 {
        self.cells_around as f64 * CELL_M
    }

    /// Whether a species can live in a cell's habitat (its ecosystems, its fauna's realm).
    pub fn suits(&self, sp: &Species, h: &Habitat) -> bool {
        if sp.habitats & h.ecosystems == 0 || !native(sp.realms, h.fauna()) {
            return false;
        }
        if sp.aquatic {
            h.fresh > 0.0
        } else {
            h.land > 0.0
        }
    }

    /// The living area of a species in a cell, km² (land, or water for fish).
    fn area(sp: &Species, h: &Habitat) -> f32 {
        if sp.aquatic {
            h.fresh_km2()
        } else {
            h.land_km2()
        }
    }

    /// How much of a species' worth a cell keeps for lack of the cover it keeps to, and, for a
    /// cold-blooded animal of the land, for want of warm weeks: a frog feeds, grows and breeds
    /// in the months above 8 °C, a snake or a lizard in those above 10 °C; where they are
    /// fewer than a third of the year (the far north, the mountains) it holds on in fewer
    /// numbers, and where they are a tenth or less, not at all (the adder's northern limit is
    /// a July of about 13 °C, the common frog's the tundra's edge).
    fn cover_factor(sp: &Species, h: &Habitat) -> f32 {
        let cover = 1.0 - (1.0 - h.cover).max(0.0) * sp.cover * 0.5;
        if !sp.ectotherm || sp.aquatic {
            return cover;
        }
        let (above, full) = match sp.plan {
            BodyPlan::Snake | BodyPlan::Lizard | BodyPlan::Turtle | BodyPlan::Crocodilian => {
                (10.0, 0.3)
            }
            _ => (8.0, 0.35),
        };
        let t = ((h.share_above(above) - 0.1) / (full - 0.1)).clamp(0.0, 1.0);
        cover * t * t * (3.0 - 2.0 * t)
    }

    /// How well a cell's plants (and worms, grubs and fungi) feed a species against the
    /// reference wood, if it eats them.
    fn fare(&self, sp: &Species, h: &Habitat) -> Option<f32> {
        if !sp.forages() {
            return None;
        }
        let (mut num, mut den) = (0.0, 0.0);
        for k in 0..FORAGE_KINDS {
            num += sp.forage[k] * h.forage[k];
            den += sp.forage[k] * self.reference.forage[k];
        }
        (den > 0.0).then(|| (num / den).clamp(0.0, 3.0))
    }

    /// How well a cell feeds a plant-eater against the reference wood (about 1 in good
    /// habitat), less where it lacks the cover the species keeps to. A hunter's is reckoned
    /// with its prey, a region at a time ([`Self::qualities`]).
    pub fn quality(&self, sp: &Species, h: &Habitat) -> f32 {
        self.fare(sp, h).unwrap_or(1.0) * Self::cover_factor(sp, h)
    }

    /// Per species and cell of a region's habitat (species-major): how well the cell feeds the
    /// species against the reference wood, 0 where it cannot live; and per species and block,
    /// how rich the land about is in a hunter's prey (1 for the rest). A plant-eater's quality
    /// is its plants' ([`Self::quality`]). A hunter's is its prey's: the meat its prey of the
    /// realm offer about the cell (their usual numbers there, block by block, by preference)
    /// against what they offer in the reference wood (a wolf of the tundra has a few reindeer
    /// and musk oxen where a wolf of the oak woods has deer and boar by the dozen), with its
    /// plants for their share of its food (a fox's berries do not make the berryless tundra a
    /// desert to it).
    pub fn qualities(&self, habitat: &[Habitat]) -> (Vec<f32>, Vec<f32>) {
        let cat = &self.catalog;
        let n = cat.len();
        let nb = (BLOCKS * BLOCKS) as usize;
        let mut q = vec![0.0f32; n * REGION_LEN];
        for sp in &cat.species {
            for (c, h) in habitat.iter().enumerate() {
                if self.suits(sp, h) {
                    q[sp.index * REGION_LEN + c] = self.quality(sp, h);
                }
            }
        }
        // Each species' worth over a block (as its hunters range over it).
        let mut block = vec![0.0f32; n * nb];
        for s in 0..n {
            for c in 0..REGION_LEN {
                block[s * nb + block_of(c)] += q[s * REGION_LEN + c] / (BLOCK * BLOCK) as f32;
            }
        }
        let mut prey = vec![1.0f32; n * nb];
        for sp in cat.species.iter().filter(|s| s.hunts()) {
            let share = sp.forage_share();
            for (c, h) in habitat.iter().enumerate() {
                let b = block_of(c);
                let (mut here, mut usual) = (0.0f32, 0.0f32);
                for &(pj, pref) in &sp.prey {
                    let p = &cat.species[pj];
                    if !native(p.realms, h.fauna()) {
                        continue;
                    }
                    let meat = pref * usual_density(p) * edible(p);
                    usual += meat;
                    here += meat * block[pj * nb + b];
                }
                let rich = if usual > 0.0 { here / usual } else { 0.0 };
                prey[sp.index * nb + b] = rich;
                if self.suits(sp, h) {
                    let plants = self.fare(sp, h).unwrap_or(rich);
                    q[sp.index * REGION_LEN + c] =
                        ((1.0 - share) * rich + share * plants) * Self::cover_factor(sp, h);
                }
            }
        }
        (q, prey)
    }

    /// The animals of a species a cell of a region holds at its usual density.
    fn capacity_in(r: &Region, sp: &Species, c: usize) -> f32 {
        sp.density * Self::area(sp, &r.habitat[c]) * r.quality[sp.index * REGION_LEN + c]
    }

    /// How well a cell of a region feeds a species against the reference wood.
    pub fn quality_in(r: &Region, species: usize, c: usize) -> f32 {
        r.quality[species * REGION_LEN + c]
    }

    /// How rich the land about a cell of a region is in a hunter's prey against the reference
    /// wood, for the hunt: a hunter ranges the wider (its attack rate rises) as its prey are
    /// the fewer, so that it meets its needs at its prey's usual numbers there, while the land
    /// holds the fewer of it ([`Self::qualities`]) — within a tenth and three times.
    fn prey_in(r: &Region, species: usize, c: usize) -> f32 {
        let nb = (BLOCKS * BLOCKS) as usize;
        r.prey[species * nb + block_of(c)].clamp(0.1, 3.0)
    }

    /// The animals of each species the region's habitat holds at the species' density.
    pub fn compute_capacity(&self, r: &mut Region) {
        let (quality, prey) = self.qualities(&r.habitat);
        r.quality = quality;
        r.prey = prey;
        let nb = (BLOCKS * BLOCKS) as usize;
        r.block_capacity = vec![0.0; self.catalog.len() * nb];
        for sp in &self.catalog.species {
            for c in 0..REGION_LEN {
                r.block_capacity[sp.index * nb + block_of(c)] += Self::capacity_in(r, sp, c);
            }
        }
        r.capacity = (0..self.catalog.len())
            .map(|s| r.block_capacity[s * nb..(s + 1) * nb].iter().sum())
            .collect();
    }

    /// A maker of regions apart from this one (on another thread, [`Ecology::adopt`] taking
    /// in what it makes): the same species, seed, calendar and tables, no regions, the ids of
    /// its new groups from `ids` on.
    pub fn maker(&self, ids: u64) -> Ecology {
        Ecology {
            catalog: self.catalog.clone(),
            seed: self.seed,
            year_offset: self.year_offset,
            cells_around: self.cells_around,
            rows: self.rows,
            regions: FxHashMap::default(),
            next_id: ids,
            attack: self.attack.clone(),
            deaths: FxHashMap::default(),
            fed: FxHashMap::default(),
            reference: self.reference,
            forage_scale: self.forage_scale,
            spin_up: self.spin_up,
            emigrants: Vec::new(),
        }
    }

    /// Takes in a region made apart: its groups' ids are beyond any here from now on.
    pub fn adopt(&mut self, r: Region) {
        self.next_id = self
            .next_id
            .max(r.groups.iter().map(|g| g.id + 1).max().unwrap_or(1));
        self.regions.insert(r.key, r);
    }

    /// Loads (creating at equilibrium the first time) the region at `key`, simulated to `time`.
    pub fn ensure_region(&mut self, land: &dyn Land, key: (i64, i64), time: f64) {
        let key = (
            key.0.rem_euclid((self.cells_around / REGION_CELLS).max(1)),
            key.1,
        );
        if self.regions.contains_key(&key) {
            return;
        }
        let r = self.create_region(land, key, time);
        self.regions.insert(key, r);
    }

    /// The habitats of a region's cells, with the realm of their animals.
    fn habitats(&self, land: &dyn Land, key: (i64, i64)) -> Vec<Habitat> {
        let (i0, j0) = (key.0 * REGION_CELLS, key.1 * REGION_CELLS);
        let mut habitat: Vec<Habitat> = (0..REGION_LEN as i64)
            .map(|idx| land.habitat((i0 + idx % REGION_CELLS, j0 + idx / REGION_CELLS)))
            .collect();
        let mut known: FxHashMap<(u8, u32), u8> = FxHashMap::default();
        for h in habitat.iter_mut() {
            h.fauna = *known
                .entry((h.realm, h.ecosystems))
                .or_insert_with(|| fauna_realm(&self.catalog, h) as u8);
        }
        habitat
    }

    /// Puts back a saved region: its habitats made again from the land (they are not saved),
    /// its animals as they were.
    pub fn restore(&mut self, land: &dyn Land, mut r: Region) {
        r.habitat = self.habitats(land, r.key);
        r.avail_mean = r
            .habitat
            .iter()
            .map(|h| Forage::ALL.map(|k| h.avail_mean(k)))
            .collect();
        if r.carrion.len() != REGION_LEN {
            r.carrion = vec![0.0; REGION_LEN];
        }
        self.compute_capacity(&mut r);
        for g in r.groups.iter_mut() {
            g.live = false;
        }
        self.next_id = self
            .next_id
            .max(r.groups.iter().map(|g| g.id + 1).max().unwrap_or(1));
        self.regions.insert(r.key, r);
    }

    fn create_region(&mut self, land: &dyn Land, key: (i64, i64), time: f64) -> Region {
        let cat = self.catalog.clone();
        let (i0, j0) = (key.0 * REGION_CELLS, key.1 * REGION_CELLS);
        let habitat = self.habitats(land, key);
        let mut rng = Rng::new(seed_of(self.seed, "fauna", &[key.0 as u64, key.1 as u64]));
        // The small species: each cell's share of the species' density.
        let pool_species: Vec<u16> = cat
            .species
            .iter()
            .filter(|s| !s.grouped())
            .filter(|s| habitat.iter().any(|h| self.suits(s, h)))
            .map(|s| s.index as u16)
            .collect();
        let n = pool_species.len() * REGION_LEN;
        let (quality, prey) = self.qualities(&habitat);
        let capacity_of = |sp: &Species, c: usize| {
            sp.density * Self::area(sp, &habitat[c]) * quality[sp.index * REGION_LEN + c]
        };
        let (mut young, mut adults) = (vec![0.0; n], vec![0.0; n]);
        for (slot, &si) in pool_species.iter().enumerate() {
            let sp = &cat.species[si as usize];
            for c in 0..REGION_LEN {
                let k = capacity_of(sp, c);
                adults[slot * REGION_LEN + c] = 0.7 * k;
                young[slot * REGION_LEN + c] = 0.3 * k;
            }
        }
        // The large species as groups, placed in suitable cells by quality.
        let mut groups = Vec::new();
        let wrap = self.wrap_m();
        for sp in cat.species.iter().filter(|s| s.grouped()) {
            let weights: Vec<f32> = (0..REGION_LEN).map(|c| capacity_of(sp, c)).collect();
            let total: f32 = weights.iter().sum();
            if total <= 0.0 {
                continue;
            }
            let mean_group = 0.5 * (sp.group.0 + sp.group.1) as f32;
            let count = poisson(&mut rng, total / mean_group.max(1.0));
            let spacing = if sp.territorial { spacing(sp) } else { 0.0 };
            let mut homes: Vec<[f64; 2]> = Vec::new();
            for _ in 0..count {
                for _attempt in 0..12 {
                    let mut x = rng.next_f32() * total;
                    let mut cell = REGION_LEN - 1;
                    for (c, w) in weights.iter().enumerate() {
                        if x < *w {
                            cell = c;
                            break;
                        }
                        x -= w;
                    }
                    let p = [
                        (((i0 + cell as i64 % REGION_CELLS) as f64 + rng.next_f64()) * CELL_M)
                            .rem_euclid(wrap),
                        ((j0 + cell as i64 / REGION_CELLS) as f64 + rng.next_f64()) * CELL_M,
                    ];
                    if homes.iter().any(|h| dist(*h, p, wrap) < spacing) {
                        continue;
                    }
                    homes.push(p);
                    groups.push(founding_group(sp, &mut rng, self.next_id, p));
                    self.next_id += 1;
                    break;
                }
            }
        }
        let avail_mean = habitat
            .iter()
            .map(|h| Forage::ALL.map(|k| h.avail_mean(k)))
            .collect();
        let mut region = Region {
            key,
            time: time - self.spin_up,
            habitat,
            avail_mean,
            carrion: vec![0.0; REGION_LEN],
            remains: Vec::new(),
            pool_species,
            young,
            adults,
            cond: vec![0.7; n],
            groups,
            mast_year: i64::MIN,
            mast: 1.0,
            growth: 1.0,
            winter: 1.0,
            capacity: Vec::new(),
            block_capacity: Vec::new(),
            quality,
            prey,
        };
        self.compute_capacity(&mut region);
        // Three years at monthly steps settle the structure and the numbers.
        while region.time < time - 1e-9 {
            let dt = (1.0 / 12.0f64).min(time - region.time);
            self.step_region(&mut region, dt);
            let out = std::mem::take(&mut self.emigrants);
            for d in out {
                self.settle_one(&mut region, d, &mut rng, true);
            }
        }
        region
    }

    /// Steps every loaded region to `time` (years), together, in steps of at most `max_dt`;
    /// young bound for another loaded region arrive there after each step.
    pub fn advance(&mut self, time: f64, max_dt: f64) {
        let mut keys: Vec<(i64, i64)> = self.regions.keys().copied().collect();
        keys.sort_unstable();
        loop {
            let now = keys
                .iter()
                .map(|k| self.regions[k].time)
                .fold(f64::INFINITY, f64::min);
            if !now.is_finite() || now >= time - 1e-9 {
                break;
            }
            let next = (now + max_dt).min(time);
            for key in &keys {
                let mut r = self.regions.remove(key).expect("region");
                if r.time < next - 1e-9 {
                    let dt = next - r.time;
                    self.step_region(&mut r, dt);
                    self.immigrate(&mut r, dt);
                }
                self.regions.insert(*key, r);
            }
            self.deliver();
        }
    }

    /// The land beyond the simulated regions holds its animals at their usual densities, and
    /// some come in over the edges: the small species' young drift into the edge cells, and now
    /// and then a disperser of the large ones arrives to settle (the rescue that keeps a small
    /// population of wolves, lynx or owls from dying out for good).
    fn immigrate(&mut self, r: &mut Region, dt: f64) {
        let cat = self.catalog.clone();
        let dtf = dt as f32;
        let wrap_regions = (self.cells_around / REGION_CELLS).max(1);
        let open: Vec<(i64, i64)> = [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)]
            .into_iter()
            .filter(|(di, dj)| {
                let k = ((r.key.0 + di).rem_euclid(wrap_regions), r.key.1 + dj);
                !self.regions.contains_key(&k)
            })
            .collect();
        if open.is_empty() {
            return;
        }
        let edge_cells = |(di, dj): (i64, i64)| -> Vec<usize> {
            (0..REGION_CELLS)
                .map(|k| {
                    let (i, j) = match (di, dj) {
                        (-1, _) => (0, k),
                        (1, _) => (REGION_CELLS - 1, k),
                        (_, -1) => (k, 0),
                        _ => (k, REGION_CELLS - 1),
                    };
                    (j * REGION_CELLS + i) as usize
                })
                .collect()
        };
        let mut rng = Rng::new(seed_of(
            self.seed,
            "immigrants",
            &[
                r.key.0 as u64,
                r.key.1 as u64,
                (r.time * 4096.0).round() as u64,
            ],
        ));
        for &side in &open {
            let cells = edge_cells(side);
            // The small species: what would drift in from a cell at its usual numbers.
            for (slot, &si) in r.pool_species.iter().enumerate() {
                let sp = &cat.species[si as usize];
                let rate = ((sp.dispersal_km / 0.256).clamp(0.5, 20.0) * 0.5 * dtf).min(0.4) * 0.15;
                for &c in &cells {
                    r.adults[slot * REGION_LEN + c] += Self::capacity_in(r, sp, c) * rate;
                }
            }
            // The large species: dispersers crossing the edge, by the density beyond.
            for sp in cat.species.iter().filter(|s| s.grouped()) {
                let k: f32 = cells.iter().map(|&c| Self::capacity_in(r, sp, c)).sum();
                if k <= 0.0 {
                    continue;
                }
                // A strip a dispersal distance deep sends a tenth of its young-grown each year.
                let depth = (sp.dispersal_km * 1000.0 / CELL_M as f32).max(1.0);
                let births = sp.life.births_per_year.min(1.0) * sp.life.litter_mean() * 0.5;
                let yearly = k * depth * births * 0.1 / sp.life.maturity_years.max(1.0);
                for _ in 0..poisson(&mut rng, yearly * dtf) {
                    let c = cells[rng.below(cells.len() as u32) as usize];
                    let from = r.cell_centre(c);
                    let d = Disperser {
                        species: sp.index as u16,
                        female: rng.next_f32() < 0.5,
                        from,
                    };
                    self.settle_one(r, d, &mut rng, true);
                }
            }
        }
    }

    /// Delivers the young bound for other regions: settled there when it is loaded, sent back
    /// home when it is not (the edge of the simulated land turns them).
    fn deliver(&mut self) {
        let out = std::mem::take(&mut self.emigrants);
        let mut rng = Rng::new(seed_of(self.seed, "deliver", &[self.next_id]));
        for d in out {
            let key = self.region_key(d.from[0], d.from[1]);
            if let Some(mut r) = self.regions.remove(&key) {
                self.settle_one(&mut r, d, &mut rng, true);
                self.regions.insert(key, r);
            } else {
                *self.deaths.entry((d.species, Cause::Lost)).or_default() += 1.0;
            }
        }
    }

    /// One step of a region.
    pub fn step_region(&mut self, r: &mut Region, dt: f64) {
        let t0 = r.time;
        let t1 = t0 + dt;
        let f0 = t0 + self.year_offset;
        let f1 = t1 + self.year_offset;
        let fmid = (0.5 * (f0 + f1)).rem_euclid(1.0) as f32;
        let mut rng = Rng::new(seed_of(
            self.seed,
            "fauna step",
            &[r.key.0 as u64, r.key.1 as u64, (t0 * 4096.0).round() as u64],
        ));
        let year = f0.floor() as i64;
        if r.mast_year != year {
            r.mast_year = year;
            r.mast = mast_factor(self.seed, r.key, year);
            (r.growth, r.winter) = weather(self.seed, r.key, year);
        }
        self.grow(r, dt);
        let snow: Vec<f32> = r
            .habitat
            .iter()
            .map(|h| h.snow_at(fmid) * r.winter)
            .collect();
        let needs = self.needs(r, dt, fmid, &snow);
        let hunted = self.hunt(r, dt, &needs, &mut rng);
        let ate = self.forage(r, dt, &needs, &hunted, &snow);
        self.step_groups(r, dt, fmid, (f0, f1), &snow, &needs, &ate, &mut rng);
        self.step_pools(r, dt, fmid, f0, &snow, &needs, &ate);
        r.time = t1;
        // Remains the scavengers have had go; the oldest first when there are many.
        r.remains
            .retain(|m| t1 - m.time < REMAINS_KEPT && m.left_at(t1) > 0.1);
        if r.remains.len() > MAX_REMAINS {
            r.remains.sort_by(|a, b| b.time.total_cmp(&a.time));
            r.remains.truncate(MAX_REMAINS);
        }
    }

    /// Takes the remains lying within `radius` of a place out of the populations (to lie in the
    /// world as carcasses), each with what is left of it now.
    pub fn take_remains(&mut self, at: [f64; 2], radius: f64, now: f64) -> Vec<Remains> {
        let wrap = self.wrap_m();
        let mut out = Vec::new();
        for r in self.regions.values_mut() {
            r.remains.retain(|m| {
                if dist(m.at, at, wrap) > radius || m.time > now {
                    return true;
                }
                out.push(Remains {
                    left: m.left_at(now),
                    ..*m
                });
                false
            });
        }
        out
    }

    /// Remains of animals dead within `fresh` years and within `radius` of a place, that ravens
    /// circle over, not told of for `again` years: told of now.
    pub fn ravens(
        &mut self,
        at: [f64; 2],
        radius: f64,
        now: f64,
        fresh: f64,
        again: f64,
    ) -> Vec<Remains> {
        let wrap = self.wrap_m();
        let mut out = Vec::new();
        for r in self.regions.values_mut() {
            for m in r.remains.iter_mut() {
                let age = now - m.time;
                if (0.0..fresh).contains(&age)
                    && now - m.told >= again
                    && dist(m.at, at, wrap) <= radius
                    && m.left_at(now) > 0.2
                {
                    m.told = now;
                    out.push(*m);
                }
            }
        }
        out
    }

    /// Records the remains of a large animal that died in a step from the region's time over
    /// `dt` years, near `at`: the place and the time drawn from a stream of their own (the
    /// populations' draws stay as they were).
    #[allow(clippy::too_many_arguments)]
    fn remains(
        &self,
        r: &mut Region,
        species: u16,
        stage: Stage,
        female: bool,
        at: [f64; 2],
        left: f32,
        cause: Cause,
        dt: f64,
    ) {
        let mut rng = Rng::new(seed_of(
            self.seed,
            "remains",
            &[
                r.key.0 as u64,
                r.key.1 as u64,
                (r.time * 4096.0).round() as u64,
                r.remains.len() as u64,
                species as u64,
            ],
        ));
        let wrap = self.wrap_m();
        let a = rng.next_f64() * std::f64::consts::TAU;
        let d = rng.next_f64() * 120.0;
        // The young of the year die small, mostly in their first weeks.
        let kg = self.catalog.species[species as usize].mass_kg
            * match stage {
                Stage::Adult => 1.0,
                Stage::Juvenile => 0.55,
                Stage::Young => 0.1,
            };
        r.remains.push(Remains {
            species,
            stage,
            female,
            at: [(at[0] + a.cos() * d).rem_euclid(wrap), at[1] + a.sin() * d],
            time: r.time + rng.next_f64() * dt,
            left: left.clamp(0.0, 1.0),
            half_days: half_days(kg),
            cause,
            told: never(),
        });
    }

    /// Carrion rots.
    fn grow(&self, r: &mut Region, dt: f64) {
        let k = (-ROT * dt as f32).exp();
        for c in r.carrion.iter_mut() {
            *c *= k;
        }
    }

    /// What each consumer needs this step (kg), awake: by group index, then by pool slot and
    /// cell.
    fn needs(&self, r: &Region, dt: f64, fmid: f32, snow: &[f32]) -> Needs {
        let dtf = dt as f32;
        let cat = &self.catalog;
        let groups = r
            .groups
            .iter()
            .map(|g| {
                let sp = &cat.species[g.species as usize];
                let Some(c) = r.cell_at(self.cells_around, g.pos[0], g.pos[1]) else {
                    return 0.0;
                };
                let h = &r.habitat[c];
                if g.live || !about(sp, h, fmid) {
                    return 0.0;
                }
                g.mouths() * sp.need_kg * 365.0 * dtf * seasonal_need(sp, h, fmid, snow[c])
            })
            .collect();
        let mut pools = vec![0.0f32; r.pool_species.len() * REGION_LEN];
        for (slot, &si) in r.pool_species.iter().enumerate() {
            let sp = &cat.species[si as usize];
            let yw = sp.young_appetite();
            for (c, (h, &sn)) in r.habitat.iter().zip(snow).enumerate() {
                let i = slot * REGION_LEN + c;
                let n = r.adults[i] + yw * r.young[i];
                if n <= 1e-7 || !about(sp, h, fmid) {
                    continue;
                }
                pools[i] = n * sp.need_kg * 365.0 * dtf * seasonal_need(sp, h, fmid, sn);
            }
        }
        Needs { groups, pools }
    }

    /// Predators hunt toward their need for meat: groups over the prey in their range, pools
    /// in their own cell. The remains of large kills are carrion.
    fn hunt(&mut self, r: &mut Region, dt: f64, needs: &Needs, rng: &mut Rng) -> Hunted {
        let cat = self.catalog.clone();
        let dtf = dt as f32;
        let wrap = self.wrap_m();
        let mut out = Hunted {
            groups: vec![0.0; r.groups.len()],
            pools: vec![0.0; needs.pools.len()],
        };
        let slot_of: FxHashMap<u16, usize> = r
            .pool_species
            .iter()
            .enumerate()
            .map(|(k, s)| (*s, k))
            .collect();
        let fmid = ((r.time + 0.5 * dt + self.year_offset).rem_euclid(1.0)) as f32;
        let mut by_species: FxHashMap<u16, Vec<usize>> = FxHashMap::default();
        for (gi, g) in r.groups.iter().enumerate() {
            let here = r
                .cell_at(self.cells_around, g.pos[0], g.pos[1])
                .is_none_or(|c| !away(&cat.species[g.species as usize], &r.habitat[c], fmid));
            if !g.live && g.size() > 0 && here {
                by_species.entry(g.species).or_default().push(gi);
            }
        }
        let mut cells = Vec::new();
        for gi in 0..r.groups.len() {
            let sp = &cat.species[r.groups[gi].species as usize];
            let want = needs.groups[gi] * (1.0 - sp.forage_share());
            if !sp.hunts() || want <= 0.0 {
                continue;
            }
            let (mouths, pos) = (r.groups[gi].mouths(), r.groups[gi].home);
            // A kill in hand first.
            let have = r.groups[gi].food.min(want);
            r.groups[gi].food -= have;
            let mut eaten = have;
            let reach = sp.range_radius_m();
            r.cells_within(self.cells_around, pos, reach, &mut cells);
            // The prey in reach, as the hunt sees it: its numbers over the area it lives in (not
            // the land of another realm or another kind it cannot live on).
            let mut seen: Vec<(usize, f32, f32)> = Vec::new();
            for &(pj, pref) in &sp.prey {
                let prey = &cat.species[pj];
                // Where it can live in reach, and its usual numbers there.
                let (mut reach_km2, mut worth) = (0.0f32, 0.0f32);
                for &c in &cells {
                    let h = &r.habitat[c];
                    if self.suits(prey, h) {
                        let a = Self::area(prey, h);
                        reach_km2 += a;
                        worth += a * Self::quality_in(r, pj, c);
                    }
                }
                let usual = usual_density(prey) * worth / reach_km2.max(1e-9);
                let reach_km2 = reach_km2.max(CELL_KM2 * 0.01);
                let mut n = 0.0f32;
                if let Some(list) = by_species.get(&(pj as u16)) {
                    for &gj in list {
                        let g = &r.groups[gj];
                        if dist(g.pos, pos, wrap) <= reach {
                            n += g.vulnerability();
                        }
                    }
                }
                if let Some(&slot) = slot_of.get(&(pj as u16)) {
                    let (wa, wy) = stage_weights(sp, prey);
                    for &c in &cells {
                        if away(prey, &r.habitat[c], fmid) {
                            continue;
                        }
                        let i = slot * REGION_LEN + c;
                        let hide = 1.0 - 0.5 * r.habitat[c].cover * prey.cover;
                        n += (wa * r.adults[i] + wy * r.young[i]) * hide;
                    }
                }
                let d = noticed(sp, n / reach_km2, usual);
                if d > 0.0 {
                    seen.push((pj, pref, d));
                }
            }
            let home = r.cell_at(self.cells_around, pos[0], pos[1]);
            let realm = home.map_or(0, |c| r.habitat[c].fauna as usize & 7);
            // As it hunts in its own realm (a home on the border of another is still its own),
            // ranging the wider as its prey are the fewer.
            let alpha = match self.attack[sp.index][realm] {
                a if a > 0.0 => a,
                _ => self.attack[sp.index].iter().copied().fold(0.0, f32::max),
            } / home.map_or(1.0, |c| Self::prey_in(r, sp.index, c));
            let most = most_eaten(sp);
            let handling = |pj: usize| edible(&cat.species[pj]) / most;
            let denom = 1.0
                + seen
                    .iter()
                    .map(|(pj, pref, d)| alpha * pref * d * handling(*pj))
                    .sum::<f32>();
            for &(pj, pref, d) in &seen {
                if eaten >= want {
                    break;
                }
                let rate = mouths * alpha * pref * d / denom;
                let prey = &cat.species[pj];
                let e = edible(prey);
                if prey.grouped() {
                    for _ in 0..poisson(rng, rate * dtf) {
                        if eaten >= want {
                            break;
                        }
                        let Some((stage, female, at)) =
                            kill_from_groups(r, pj as u16, pos, reach, wrap, rng)
                        else {
                            break;
                        };
                        let take = e.min(want - eaten);
                        eaten += take;
                        let left = e - take;
                        self.remains(
                            r,
                            pj as u16,
                            stage,
                            female,
                            at,
                            left / e.max(1e-6),
                            Cause::Predation,
                            dt,
                        );
                        // What it cannot eat now it comes back to; the rest is carrion.
                        r.groups[gi].food += left * 0.5;
                        if let Some(c) = r.cell_at(self.cells_around, pos[0], pos[1]) {
                            r.carrion[c] += left * 0.5;
                        }
                        *self
                            .deaths
                            .entry((pj as u16, Cause::Predation))
                            .or_default() += 1.0;
                    }
                } else if let Some(&slot) = slot_of.get(&(pj as u16)) {
                    // Small prey are taken as they come, in proportion through the reach (counted
                    // as grown ones: a tadpole is a mouthful of a frog).
                    let kills = (rate * dtf).min((want - eaten) / e.max(1e-9));
                    let (wa, wy) = stage_weights(sp, prey);
                    let here: Vec<usize> = cells
                        .iter()
                        .copied()
                        .filter(|&c| !away(prey, &r.habitat[c], fmid))
                        .collect();
                    let total: f32 = here
                        .iter()
                        .map(|&c| {
                            let i = slot * REGION_LEN + c;
                            wa * r.adults[i] + wy * r.young[i]
                        })
                        .sum();
                    if total <= 0.0 || kills <= 0.0 {
                        continue;
                    }
                    // Each stage is taken as it is offered; a young one is part of a meal.
                    let frac = (kills / total).min(0.5);
                    let mut taken = 0.0f32;
                    for &c in &here {
                        let i = slot * REGION_LEN + c;
                        taken += take_stages(&mut r.adults[i], &mut r.young[i], frac, wa, wy);
                    }
                    let mass = taken.min(kills);
                    eaten += mass * e;
                    *self
                        .deaths
                        .entry((pj as u16, Cause::Predation))
                        .or_default() += mass as f64;
                }
            }
            out.groups[gi] = eaten;
            let t = self.fed.entry(1000 + sp.index as u16).or_default();
            t.0 += (eaten / want).min(1.0) as f64;
            t.1 += 1.0;
        }
        // Small predators hunt small prey in their own cell.
        for slot in 0..r.pool_species.len() {
            let sp = &cat.species[r.pool_species[slot] as usize];
            if !sp.hunts() {
                continue;
            }
            let most = most_eaten(sp);
            let handling = |pj: usize| edible(&cat.species[pj]) / most;
            for c in 0..REGION_LEN {
                let i = slot * REGION_LEN + c;
                let alpha = self.attack[sp.index][r.habitat[c].fauna as usize & 7]
                    / Self::prey_in(r, sp.index, c);
                let want = needs.pools[i] * (1.0 - sp.forage_share());
                if want <= 0.0 {
                    continue;
                }
                let preds = r.adults[i] + sp.young_appetite() * r.young[i];
                let mut seen: Vec<(usize, usize, f32, f32)> = Vec::new();
                for &(pj, pref) in &sp.prey {
                    if let Some(&ps) = slot_of.get(&(pj as u16))
                        && !away(&cat.species[pj], &r.habitat[c], fmid)
                    {
                        let j = ps * REGION_LEN + c;
                        let prey = &cat.species[pj];
                        let hide = 1.0 - 0.5 * r.habitat[c].cover * prey.cover;
                        let (wa, wy) = stage_weights(sp, prey);
                        let n = (wa * r.adults[j] + wy * r.young[j]) * hide;
                        let area = Self::area(prey, &r.habitat[c]).max(1e-4);
                        let usual = usual_density(prey) * Self::quality_in(r, pj, c);
                        let d = noticed(sp, n / area, usual);
                        if d > 0.0 {
                            seen.push((pj, j, pref, d));
                        }
                    }
                }
                let denom = 1.0
                    + seen
                        .iter()
                        .map(|(pj, _, pref, d)| alpha * pref * d * handling(*pj))
                        .sum::<f32>();
                let mut eaten = 0.0;
                for &(pj, j, pref, d) in &seen {
                    let prey = &cat.species[pj];
                    let e = edible(prey);
                    let (wa, wy) = stage_weights(sp, prey);
                    let total = wa * r.adults[j] + wy * r.young[j];
                    let kills = (preds * alpha * pref * d / denom * dtf)
                        .min((want - eaten).max(0.0) / e.max(1e-9))
                        .min(total * 0.5);
                    if total > 0.0 && kills > 0.0 {
                        let (mut a, mut y) = (r.adults[j], r.young[j]);
                        let taken = take_stages(&mut a, &mut y, kills / total, wa, wy).min(kills);
                        r.adults[j] = a;
                        r.young[j] = y;
                        eaten += taken * e;
                        *self
                            .deaths
                            .entry((pj as u16, Cause::Predation))
                            .or_default() += taken as f64;
                    }
                }
                out.pools[i] = eaten;
                let t = self.fed.entry(1000 + sp.index as u16).or_default();
                t.0 += (eaten / want).min(1.0) as f64;
                t.1 += 1.0;
            }
        }
        out
    }

    /// Everyone eats from the forage and carrion in reach what the hunt did not give them (a
    /// plant-eater all its need; a predator its share of plants, more when the hunt fell
    /// short). Returns the food eaten by each consumer, meat included, kg.
    fn forage(
        &mut self,
        r: &mut Region,
        dt: f64,
        needs: &Needs,
        hunted: &Hunted,
        snow: &[f32],
    ) -> Needs {
        let cat = self.catalog.clone();
        let dtf = dt as f32;
        // What each cell offers of each food this step, kg.
        let fmid = ((r.time + 0.5 * dt + self.year_offset).rem_euclid(1.0)) as f32;
        let supply: Vec<[f32; FOODS]> = (0..REGION_LEN)
            .map(|c| {
                let h = &r.habitat[c];
                let mut s = [0.0f32; FOODS];
                for (k, &kind) in Forage::ALL.iter().enumerate() {
                    if h.forage[k] <= 0.0 {
                        continue;
                    }
                    s[k] = h.forage[k]
                        * self.forage_scale[k]
                        * h.area_of(kind)
                        * h.availability(kind, fmid, snow[c], r.mast, r.avail_mean[c][k])
                        * if matches!(kind, Forage::Mast | Forage::Aquatic) {
                            1.0
                        } else {
                            r.growth
                        }
                        * dtf;
                }
                s[CARRION] = r.carrion[c] * (FIND_CARRION * dtf).min(1.0);
                s
            })
            .collect();
        let avail = |c: usize, k: usize| supply[c][k];
        let prefs = |sp: &Species| -> [f32; FOODS] {
            let mut p = [0.0; FOODS];
            p[..FORAGE_KINDS].copy_from_slice(&sp.forage);
            p[CARRION] = sp.carrion;
            p
        };
        // Who asks for how much.
        let mut asks: Vec<(Who, f32)> = Vec::new();
        for (gi, g) in r.groups.iter().enumerate() {
            let sp = &cat.species[g.species as usize];
            let d = needs.groups[gi] - hunted.groups[gi];
            if d > 0.0 && (sp.forages() || sp.carrion > 0.0) {
                asks.push((Who::Group(gi), d));
            }
        }
        for (slot, &si) in r.pool_species.iter().enumerate() {
            let sp = &cat.species[si as usize];
            if !sp.forages() && sp.carrion <= 0.0 {
                continue;
            }
            for c in 0..REGION_LEN {
                let i = slot * REGION_LEN + c;
                let d = needs.pools[i] - hunted.pools[i];
                if d > 0.0 {
                    asks.push((Who::Pool(slot, c), d));
                }
            }
        }
        let cells_around = self.cells_around;
        let mut cells: Vec<usize> = Vec::new();
        // A consumer's spread of its demand over cells and kinds, by what is there.
        let spread = |who: Who,
                      demand: f32,
                      r: &Region,
                      cells: &mut Vec<usize>,
                      f: &mut dyn FnMut(usize, usize, f32)| {
            let sp = match who {
                Who::Group(gi) => {
                    let g = &r.groups[gi];
                    let sp = &cat.species[g.species as usize];
                    r.cells_within(cells_around, g.home, sp.range_radius_m().min(3000.0), cells);
                    sp
                }
                Who::Pool(slot, c) => {
                    cells.clear();
                    cells.push(c);
                    &cat.species[r.pool_species[slot] as usize]
                }
            };
            let w = prefs(sp);
            let mut total = 0.0f32;
            for &c in cells.iter() {
                for (k, wk) in w.iter().enumerate() {
                    total += wk * avail(c, k);
                }
            }
            if total <= 0.0 {
                return;
            }
            for &c in cells.iter() {
                for (k, wk) in w.iter().enumerate() {
                    let a = wk * avail(c, k);
                    if a > 0.0 {
                        f(c, k, demand * a / total);
                    }
                }
            }
        };
        let mut want = vec![[0.0f32; FOODS]; REGION_LEN];
        for &(who, d) in &asks {
            spread(who, d, r, &mut cells, &mut |c, k, x| want[c][k] += x);
        }
        let mut scale = vec![[1.0f32; FOODS]; REGION_LEN];
        for c in 0..REGION_LEN {
            for k in 0..FOODS {
                if want[c][k] > supply[c][k] && want[c][k] > 0.0 {
                    scale[c][k] = supply[c][k] / want[c][k];
                }
            }
        }
        let mut ate = Needs {
            groups: hunted.groups.clone(),
            pools: hunted.pools.clone(),
        };
        for &(who, d) in &asks {
            let (mut plants, mut carrion) = (0.0f32, 0.0f32);
            spread(who, d, r, &mut cells, &mut |c, k, x| {
                if k == CARRION {
                    carrion += x * scale[c][k];
                } else {
                    plants += x * scale[c][k];
                }
            });
            let (si, need, meat) = match who {
                Who::Group(gi) => (r.groups[gi].species, needs.groups[gi], hunted.groups[gi]),
                Who::Pool(slot, c) => {
                    let i = slot * REGION_LEN + c;
                    (r.pool_species[slot], needs.pools[i], hunted.pools[i])
                }
            };
            let sp = &cat.species[si as usize];
            let got = carrion + plants.min(plant_cap(sp, need, meat));
            match who {
                Who::Group(gi) => ate.groups[gi] += got,
                Who::Pool(slot, c) => ate.pools[slot * REGION_LEN + c] += got,
            }
            let t = self.fed.entry(si).or_default();
            t.0 += (got / d.max(1e-9)) as f64;
            t.1 += 1.0;
        }
        for c in 0..REGION_LEN {
            r.carrion[c] = (r.carrion[c] - want[c][CARRION] * scale[c][CARRION]).max(0.0);
        }
        ate
    }

    /// The groups: condition from what they ate, deaths, the year's births and growing up,
    /// where they are today, herds too big splitting, the young settling.
    #[allow(clippy::too_many_arguments)]
    fn step_groups(
        &mut self,
        r: &mut Region,
        dt: f64,
        fmid: f32,
        (f0, f1): (f64, f64),
        snow: &[f32],
        needs: &Needs,
        ate: &Needs,
        rng: &mut Rng,
    ) {
        let cat = self.catalog.clone();
        let dtf = dt as f32;
        let wrap = self.wrap_m();
        // Crowding of each group's kind past what the land about it holds: its block of 2 km
        // and those about it, out to its home range. Where the land holds fewer than one
        // ordinary group, a group of that size is not crowded by its own numbers: it lives or
        // starves by its food.
        let nb = (BLOCKS * BLOCKS) as usize;
        let mut counts = vec![0.0f32; cat.len() * nb];
        let block_at =
            |r: &Region, p: [f64; 2]| r.cell_at(self.cells_around, p[0], p[1]).map(block_of);
        for g in &r.groups {
            if let Some(b) = block_at(r, g.home) {
                counts[g.species as usize * nb + b] += g.size() as f32;
            }
        }
        let crowding_at = |species: usize, b: usize| -> f32 {
            let (bi, bj) = ((b as i64) % BLOCKS, (b as i64) / BLOCKS);
            let w = (cat.species[species].range_radius_m() / (BLOCK as f64 * CELL_M)).ceil() as i64;
            let w = w.max(1);
            let (mut n, mut k) = (0.0, 0.0);
            for dj in -w..=w {
                for di in -w..=w {
                    let (i, j) = (bi + di, bj + dj);
                    if (0..BLOCKS).contains(&i) && (0..BLOCKS).contains(&j) {
                        let x = species * nb + (j * BLOCKS + i) as usize;
                        n += counts[x];
                        k += r.block_capacity[x];
                    }
                }
            }
            let (lo, hi) = cat.species[species].group;
            CROWDING * (n / k.max(0.5 * (lo + hi) as f32).max(1e-6) - 1.0).max(0.0)
        };
        let crowds: Vec<f32> = r
            .groups
            .iter()
            .map(|g| block_at(r, g.home).map_or(0.0, |b| crowding_at(g.species as usize, b)))
            .collect();
        let mut leaving: Vec<Disperser> = Vec::new();
        let mut bred: Vec<usize> = Vec::new();
        for (gi, &crowd) in crowds.iter().enumerate() {
            if r.groups[gi].live {
                continue;
            }
            let sp = &cat.species[r.groups[gi].species as usize];
            let c0 = r.cell_at(self.cells_around, r.groups[gi].pos[0], r.groups[gi].pos[1]);
            let h = c0.map(|c| r.habitat[c]).unwrap_or_default();
            let sleeping = asleep(sp, &h, fmid);
            let gone = away(sp, &h, fmid);
            let ratio = if needs.groups[gi] > 0.0 {
                ate.groups[gi] / needs.groups[gi]
            } else {
                1.0
            };
            let g = &mut r.groups[gi];
            g.condition = update_condition(g.condition, ratio, sleeping, sp.ectotherm, dtf);
            let c = g.condition;
            let winter = if sleeping || gone {
                0.0
            } else {
                0.6 * c0.map_or(0.0, |c| snow[c]) * smoothstep(0.6, 0.1, c)
            };
            let natural = -(sp.life.adult_survival.ln()) * if sleeping { 0.5 } else { 1.0 };
            let young_rate = -(sp.life.young_survival.ln()) * if sleeping { 0.5 } else { 1.0 };
            let hunger = starving(c);
            let p = |rate: f32| 1.0 - (-rate * dtf).exp();
            // Crowding falls on the young and the half-grown first, as density does in the wild:
            // the grown hold their ground (a pair keeps its territory while its cubs find none).
            let p_adult = p(natural + hunger + winter + crowd * 0.3);
            let p_juv = p(natural * 1.3 + hunger + winter + crowd * 1.5);
            let p_young = p(young_rate + 2.0 * hunger + 2.0 * winter + crowd * 1.5);
            let mut dead = 0u32;
            let mut died = [0u32; 4];
            for (k, (n, pr)) in [
                (&mut g.females, p_adult),
                (&mut g.males, p_adult),
                (&mut g.juveniles, p_juv),
                (&mut g.young, p_young),
            ]
            .into_iter()
            .enumerate()
            {
                let x = binomial(rng, *n as u32, pr);
                *n -= x as u16;
                dead += x;
                died[k] = x;
            }
            if dead > 0 {
                let (species, at, id) = (g.species, g.pos, g.id);
                let total = (natural + hunger + winter + crowd).max(1e-9);
                let parts = [
                    (Cause::Natural, natural),
                    (Cause::Hunger, hunger),
                    (Cause::Winter, winter),
                    (Cause::Crowding, crowd),
                ];
                for (cause, part) in parts {
                    *self.deaths.entry((species, cause)).or_default() +=
                        dead as f64 * (part / total) as f64;
                }
                // The dead feed the scavengers, and lie where they died (of the likeliest
                // cause).
                if let Some(c) = c0 {
                    r.carrion[c] += dead as f32 * edible(sp) * 0.7;
                }
                let cause = parts
                    .iter()
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .map_or(Cause::Natural, |p| p.0);
                for (k, &x) in died.iter().enumerate() {
                    let (stage, female) = match k {
                        0 => (Stage::Adult, true),
                        1 => (Stage::Adult, false),
                        2 => (Stage::Juvenile, id.is_multiple_of(2)),
                        _ => (Stage::Young, !id.is_multiple_of(2)),
                    };
                    for _ in 0..x {
                        self.remains(r, species, stage, female, at, 1.0, cause, dt);
                    }
                }
            }
            let g = &mut r.groups[gi];
            // The young of the year are born, the year's growing up done first.
            let birth = if h.southern {
                (sp.life.birth_frac as f64 + 0.5).rem_euclid(1.0)
            } else {
                sp.life.birth_frac as f64
            };
            if crossed(f0, f1, birth) {
                let grown = if sp.life.maturity_years <= 1.25 {
                    let n = g.young as u32 + g.juveniles as u32;
                    g.young = 0;
                    g.juveniles = 0;
                    n
                } else {
                    let p = 1.0 / (sp.life.maturity_years - 1.0).max(1.0);
                    let n = binomial(rng, g.juveniles as u32, p);
                    g.juveniles = g.juveniles - n as u16 + g.young;
                    g.young = 0;
                    n
                };
                for _ in 0..grown {
                    let female = rng.next_f32() < 0.5;
                    let leaves = match (sp.social, sp.dispersers) {
                        (Social::Solitary, _) => true,
                        (Social::Pack { .. }, _) => rng.next_f32() < 0.75,
                        (_, Dispersers::Both) => rng.next_f32() < 0.7,
                        (_, Dispersers::Males) => !female,
                        (_, Dispersers::Females) => female,
                    };
                    if leaves {
                        leaving.push(Disperser {
                            species: g.species,
                            female,
                            from: g.home,
                        });
                    } else if female {
                        g.females += 1;
                    } else {
                        g.males += 1;
                    }
                }
                let breeders = match sp.social {
                    Social::Pack { .. } => (g.females > 0 && g.males > 0) as u32,
                    _ => g.females as u32,
                };
                let litters = binomial(
                    rng,
                    breeders,
                    sp.life.births_per_year.min(1.0) * fecundity(c),
                );
                let (lo, hi) = sp.life.litter;
                let mut born = 0u32;
                for _ in 0..litters {
                    let n = (lo + rng.next_f32() * (hi - lo + 1.0)).floor();
                    born += n.clamp(lo, hi.max(lo)) as u32;
                }
                g.young = (g.young as u32 + born).min(u16::MAX as u32) as u16;
                bred.push(gi);
            }
            // Where it is today: somewhere in its range.
            let rad = sp.range_radius_m();
            let a = rng.next_f64() * std::f64::consts::TAU;
            let d = rad * rng.next_f64().sqrt();
            let cand = [
                (g.home[0] + a.cos() * d).rem_euclid(wrap),
                g.home[1] + a.sin() * d,
            ];
            if r.cell_at(self.cells_around, cand[0], cand[1]).is_some() {
                r.groups[gi].pos = cand;
            }
        }
        self.bud(r, &bred, rng);
        // Herds too big split (when there is land for the half that leaves); a territorial
        // group sends its surplus adults to find room.
        for gi in 0..r.groups.len() {
            let g = &r.groups[gi];
            let sp = &cat.species[g.species as usize];
            if g.live || (g.size() as f32) <= sp.group.1 as f32 * 1.5 {
                continue;
            }
            if sp.territorial {
                let g = &mut r.groups[gi];
                while g.size() as u16 > sp.group.1 && g.adults() > 2 {
                    let female = g.females > g.males;
                    if female {
                        g.females -= 1;
                    } else {
                        g.males -= 1;
                    }
                    leaving.push(Disperser {
                        species: g.species,
                        female,
                        from: g.home,
                    });
                }
                continue;
            }
            if g.adults() < 4 {
                continue;
            }
            let a = rng.next_f64() * std::f64::consts::TAU;
            let d = sp.range_radius_m() * 1.5;
            let home = [
                (g.home[0] + a.cos() * d).rem_euclid(wrap),
                g.home[1] + a.sin() * d,
            ];
            match r.cell_at(self.cells_around, home[0], home[1]) {
                Some(c) if self.suits(sp, &r.habitat[c]) => {}
                _ => continue,
            }
            let g = &mut r.groups[gi];
            let mut h = g.clone();
            h.id = self.next_id;
            self.next_id += 1;
            h.females = g.females / 2;
            h.males = g.males / 2;
            h.juveniles = g.juveniles / 2;
            h.young = g.young / 2;
            g.females -= h.females;
            g.males -= h.males;
            g.juveniles -= h.juveniles;
            g.young -= h.young;
            h.home = home;
            h.pos = home;
            r.groups.push(h);
        }
        r.groups.retain(|g| g.size() > 0 || g.live);
        for d in leaving {
            self.settle_one(r, d, rng, false);
        }
    }

    /// Herds and families crowded past what their range holds send some of their mothers with
    /// their young away, to land nearby with room (the way deer spread back into land emptied
    /// by hunting or a hard winter). Looked at once a year, when the young are born.
    fn bud(&mut self, r: &mut Region, bred: &[usize], rng: &mut Rng) {
        let cat = self.catalog.clone();
        let wrap = self.wrap_m();
        let mut cells = Vec::new();
        let mut buds = Vec::new();
        // Animals of a species, and what the land holds of it, about a point.
        let local = |r: &Region, cells: &mut Vec<usize>, sp: &Species, p: [f64; 2]| {
            let rad = sp.range_radius_m();
            r.cells_within(self.cells_around, p, rad, cells);
            let cap: f32 = cells.iter().map(|&c| Self::capacity_in(r, sp, c)).sum();
            let n: u32 = r
                .groups
                .iter()
                .filter(|g| g.species == sp.index as u16 && dist(g.home, p, wrap) <= rad)
                .map(Group::size)
                .sum();
            (n as f32, cap)
        };
        for &gi in bred {
            let g = &r.groups[gi];
            let sp = &cat.species[g.species as usize];
            if g.live
                || sp.territorial
                || g.females < 2
                || !matches!(sp.social, Social::Herd { .. } | Social::Family)
            {
                continue;
            }
            let (n, cap) = local(r, &mut cells, sp, g.home);
            if n <= cap * 0.9 {
                continue;
            }
            // The roomiest of a few places within reach.
            let mut best: Option<([f64; 2], f32)> = None;
            for _ in 0..6 {
                let a = rng.next_f64() * std::f64::consts::TAU;
                let d = (sp.dispersal_km as f64 * 1000.0).max(sp.range_radius_m() * 2.0)
                    * (0.4 + 0.6 * rng.next_f64());
                let p = [
                    (g.home[0] + a.cos() * d).rem_euclid(wrap),
                    g.home[1] + a.sin() * d,
                ];
                match r.cell_at(self.cells_around, p[0], p[1]) {
                    Some(c) if self.suits(sp, &r.habitat[c]) => {}
                    _ => continue,
                }
                let (m, k) = local(r, &mut cells, sp, p);
                let room = k - m;
                if room > (g.size() as f32 / 3.0).max(1.0) && best.is_none_or(|(_, b)| room > b) {
                    best = Some((p, room));
                }
            }
            if let Some((p, _)) = best {
                buds.push((gi, p));
            }
        }
        for (gi, p) in buds {
            let g = &mut r.groups[gi];
            let mut h = g.clone();
            h.id = self.next_id;
            self.next_id += 1;
            h.females = g.females / 3;
            h.young = g.young / 3;
            h.juveniles = g.juveniles / 3;
            h.males = 0;
            g.females -= h.females;
            g.young -= h.young;
            g.juveniles -= h.juveniles;
            h.home = p;
            h.pos = p;
            if h.size() > 0 {
                r.groups.push(h);
            }
        }
    }

    /// Settles a young animal that left home: in a suitable cell within its species'
    /// dispersal distance with room (territorial species keep their distance from others'
    /// homes), joining a group of its kind with room or founding one; a pack's young pair up
    /// with another of the other sex. One bound for another region leaves for it, unless
    /// `stay` (then the edge turns it back).
    fn settle_one(&mut self, r: &mut Region, d: Disperser, rng: &mut Rng, stay: bool) {
        let cat = self.catalog.clone();
        let sp = &cat.species[d.species as usize];
        let wrap = self.wrap_m();
        let rad = sp.range_radius_m();
        for attempt in 0..8 {
            let a = rng.next_f64() * std::f64::consts::TAU;
            // One leaving home goes its species' distance; one arriving (from another region,
            // or over the edge) looks for room about where it came in.
            let reach = if stay {
                (rad * 2.0)
                    .max(2_000.0)
                    .min(sp.dispersal_km as f64 * 1000.0)
            } else {
                sp.dispersal_km as f64 * 1000.0
            };
            let dd = reach * (0.3 + 0.7 * rng.next_f64());
            let p = [
                (d.from[0] + a.cos() * dd).rem_euclid(wrap),
                d.from[1] + a.sin() * dd,
            ];
            let Some(c) = r.cell_at(self.cells_around, p[0], p[1]) else {
                if !stay && attempt == 0 {
                    // Off to the next region.
                    self.emigrants.push(Disperser { from: p, ..d });
                    return;
                }
                continue;
            };
            if !self.suits(sp, &r.habitat[c]) || Self::quality_in(r, sp.index, c) < 0.2 {
                continue;
            }
            if sp.territorial
                && r.groups.iter().any(|g| {
                    g.species == d.species
                        && dist(g.home, p, wrap) < spacing(sp)
                        && !matches!(sp.social, Social::Pack { .. } if g.adults() < 2)
                })
            {
                // Packs take in a lone wolf of the other sex.
                if let Social::Pack { .. } = sp.social
                    && let Some(g) = r.groups.iter_mut().find(|g| {
                        g.species == d.species
                            && !g.live
                            && g.adults() == 1
                            && (g.females == 1) != d.female
                            && dist(g.home, p, wrap) < rad * 3.0
                    })
                {
                    if d.female {
                        g.females += 1;
                    } else {
                        g.males += 1;
                    }
                    return;
                }
                continue;
            }
            if matches!(sp.social, Social::Herd { .. } | Social::Family)
                && let Some(g) = r.groups.iter_mut().find(|g| {
                    g.species == d.species
                        && !g.live
                        && (g.size() as u16) < sp.group.1
                        && dist(g.home, p, wrap) < rad * 2.0
                })
            {
                if d.female {
                    g.females += 1;
                } else {
                    g.males += 1;
                }
                return;
            }
            let mut g = founding_group(sp, rng, self.next_id, p);
            self.next_id += 1;
            g.young = 0;
            g.juveniles = 0;
            g.females = d.female as u16;
            g.males = (!d.female) as u16;
            r.groups.push(g);
            return;
        }
        *self.deaths.entry((d.species, Cause::Lost)).or_default() += 1.0;
    }

    /// The small species of a region: condition, deaths, births, growing up and spreading.
    #[allow(clippy::too_many_arguments)]
    fn step_pools(
        &mut self,
        r: &mut Region,
        dt: f64,
        fmid: f32,
        f0: f64,
        snow: &[f32],
        needs: &Needs,
        ate: &Needs,
    ) {
        let cat = self.catalog.clone();
        let dtf = dt as f32;
        for (slot, &si) in r.pool_species.iter().enumerate() {
            let sp = &cat.species[si as usize];
            let natural = -(sp.life.adult_survival.ln());
            // The young die at their own rate in their first year and, for those that take
            // longer to grow, as the half-grown of the groups do after it (a third more than the
            // grown): over the years they are young, the mean.
            let young_rate = {
                let first = -(sp.life.young_survival.ln());
                let years = sp.life.maturity_years.max(1.0);
                (first + natural * 1.3 * (years - 1.0)) / years
            };
            let mature = 1.0 / sp.life.maturity_years.max(0.05);
            // Births through a breeding season about the birth date, as long as the litters
            // need; a colony sends out swarms (no sexes to halve).
            let season_len = (0.1 * sp.life.births_per_year).clamp(0.08, 0.45);
            let mothers = if sp.colony { 1.0 } else { 0.5 };
            let litters = sp.life.births_per_year * sp.life.litter_mean() * mothers;
            let mut tally = [0.0f64; 4];
            for (c, &sn) in snow.iter().enumerate() {
                let i = slot * REGION_LEN + c;
                let (a, y) = (r.adults[i], r.young[i]);
                if a + y <= 1e-7 {
                    continue;
                }
                let h = &r.habitat[c];
                let sleeping = asleep(sp, h, fmid);
                let gone = away(sp, h, fmid);
                let fed = if needs.pools[i] > 0.0 {
                    ate.pools[i] / needs.pools[i]
                } else {
                    1.0
                };
                let cnd = update_condition(r.cond[i], fed, sleeping, sp.ectotherm, dtf);
                r.cond[i] = cnd;
                let winter = if sleeping || gone {
                    0.0
                } else {
                    0.8 * sn * smoothstep(0.6, 0.1, cnd)
                };
                let slow = if sleeping { 0.5 } else { 1.0 };
                // Crowding: the adults against what the place holds, the young (eggs, larvae,
                // the year's litters) among themselves, by what they eat.
                let q = Self::quality_in(r, sp.index, c);
                let k_cap = (sp.density * Self::area(sp, h) * q).max(1e-6);
                let crowd = CROWDING * (a / k_cap - 1.0).max(0.0);
                let crowd_y =
                    CROWDING * ((a + y * sp.young_appetite()) / k_cap - 1.0).max(0.0) * 2.0;
                let holders = if sp.territorial {
                    2.0 * Self::area(sp, h) * q / sp.home_range_km2
                } else {
                    f32::INFINITY
                };
                let floaters = (a - holders).max(0.0);
                let hunger = starving(cnd);
                let mu_a = natural * slow + hunger + winter + crowd;
                let mu_y = young_rate * slow + 2.0 * hunger + 2.0 * winter + crowd_y;
                let da = (a * (1.0 - (-mu_a * dtf).exp())
                    + floaters * (1.0 - (-FLOATERS * dtf).exp()))
                .min(a);
                let dy = y * (1.0 - (-mu_y * dtf).exp());
                let dead = (da + dy) as f64;
                if dead > 0.0 {
                    let f_float = (floaters * (1.0 - (-FLOATERS * dtf).exp())) as f64;
                    let rest = (dead - f_float).max(0.0);
                    let tot = mu_a.max(1e-9) as f64;
                    tally[0] += rest * (natural * slow) as f64 / tot;
                    tally[1] += rest * hunger as f64 / tot;
                    tally[2] += rest * winter as f64 / tot;
                    tally[3] += rest * crowd as f64 / tot + f_float;
                    r.carrion[c] += (da + dy) * edible(sp) * 0.5;
                }
                let mut a2 = a - da;
                let mut y2 = y - dy;
                // Growing up, the fewer the more grown ones the place already holds (the young
                // find no room, no territory, no food of their own).
                let room = 1.0 / (1.0 + (a2 / k_cap).powi(4));
                let m = y2 * (1.0 - (-mature * dtf).exp()) * room;
                y2 -= m;
                a2 += m;
                // Births through the part of the breeding season this step spans, to the holders
                // of a territory.
                let shift = if h.southern { 0.5 } else { 0.0 };
                let centre = sp.life.birth_frac as f64 + shift;
                let span = season_overlap(f0, f0 + dt, centre, season_len as f64) as f32;
                if !sleeping && span > 0.0 {
                    y2 += a2.min(holders) * litters / season_len * fecundity(cnd) * span;
                }
                r.adults[i] = a2.max(0.0);
                r.young[i] = y2.max(0.0);
            }
            for (cause, n) in [
                Cause::Natural,
                Cause::Hunger,
                Cause::Winter,
                Cause::Crowding,
            ]
            .into_iter()
            .zip(tally)
            {
                *self.deaths.entry((si, cause)).or_default() += n;
            }
            // Spreading: some of the young move to the neighbouring cells, more where food is
            // short, toward the suitable.
            let rate = (sp.dispersal_km / 0.256).clamp(0.5, 20.0) * 0.5;
            let q = (rate * dtf).min(0.4);
            let base = slot * REGION_LEN;
            let mut moved_y = vec![0.0f32; REGION_LEN];
            let mut moved_a = vec![0.0f32; REGION_LEN];
            for c in 0..REGION_LEN {
                let i = base + c;
                let y = r.young[i] + 0.15 * r.adults[i];
                if y <= 1e-6 {
                    continue;
                }
                let fed = if needs.pools[i] > 0.0 {
                    ate.pools[i] / needs.pools[i]
                } else {
                    1.0
                };
                let leave = y * q * (0.5 + (1.0 - fed).max(0.0)).min(1.25);
                let (ci, cj) = (c as i64 % REGION_CELLS, c as i64 / REGION_CELLS);
                let mut targets = [(c, 0.0f32); 4];
                let mut tw = 0.0;
                for (k, (di, dj)) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)].iter().enumerate() {
                    let (ni, nj) = (ci + di, cj + dj);
                    if !(0..REGION_CELLS).contains(&ni) || !(0..REGION_CELLS).contains(&nj) {
                        continue;
                    }
                    let n = (nj * REGION_CELLS + ni) as usize;
                    let h = &r.habitat[n];
                    let w = if self.suits(sp, h) {
                        Self::area(sp, h) * (0.2 + Self::quality_in(r, sp.index, n))
                    } else {
                        0.0
                    };
                    targets[k] = (n, w);
                    tw += w;
                }
                if tw <= 0.0 {
                    continue;
                }
                let share_y = r.young[i] / y;
                let (ly, la) = (leave * share_y, leave * (1.0 - share_y));
                for (n, w) in targets {
                    if w > 0.0 {
                        moved_y[n] += ly * w / tw;
                        moved_a[n] += la * w / tw;
                    }
                }
                r.young[i] = (r.young[i] - ly).max(0.0);
                r.adults[i] = (r.adults[i] - la).max(0.0);
            }
            for c in 0..REGION_LEN {
                r.young[base + c] += moved_y[c];
                r.adults[base + c] += moved_a[c];
            }
        }
    }

    /// Individuals of a species in the loaded regions.
    pub fn count(&self, species: usize) -> f64 {
        let s = species as u16;
        let mut n = 0.0;
        for r in self.regions.values() {
            n += r
                .groups
                .iter()
                .filter(|g| g.species == s)
                .map(|g| g.size() as f64)
                .sum::<f64>();
            if let Some(slot) = r.pool_species.iter().position(|x| *x == s) {
                // The spawn of fish and frogs is not counted.
                let young = if self.catalog.species[species].census_young() >= 0.5 {
                    1.0
                } else {
                    0.0
                };
                let range = slot * REGION_LEN..(slot + 1) * REGION_LEN;
                n += r.adults[range.clone()]
                    .iter()
                    .map(|v| *v as f64)
                    .sum::<f64>()
                    + young * r.young[range].iter().map(|v| *v as f64).sum::<f64>();
            }
        }
        n
    }

    /// Individuals of a species within `radius` metres of a world position.
    pub fn count_within(&self, species: usize, at: [f64; 2], radius: f64) -> f64 {
        let s = species as u16;
        let wrap = self.wrap_m();
        let young = if self.catalog.species[species].census_young() >= 0.5 {
            1.0
        } else {
            0.0
        };
        let mut n = 0.0;
        for r in self.regions.values() {
            n += r
                .groups
                .iter()
                .filter(|g| g.species == s && dist(g.pos, at, wrap) <= radius)
                .map(|g| g.size() as f64)
                .sum::<f64>();
            if let Some(slot) = r.pool_species.iter().position(|x| *x == s) {
                for c in 0..REGION_LEN {
                    if dist(r.cell_centre(c), at, wrap) <= radius {
                        let i = slot * REGION_LEN + c;
                        n += r.adults[i] as f64 + young * r.young[i] as f64;
                    }
                }
            }
        }
        n
    }

    /// Hunters take up to `n` animals of a species within `radius` metres of a world position:
    /// the grown ones of the groups there (the nearest first), or of the small species'
    /// numbers there. Returns how many were taken.
    pub fn cull(&mut self, species: usize, at: [f64; 2], radius: f64, n: u32) -> u32 {
        let s = species as u16;
        let wrap = self.wrap_m();
        let mut taken = 0u32;
        let mut keys: Vec<(i64, i64)> = self.regions.keys().copied().collect();
        keys.sort_unstable();
        let mut stag = false;
        for key in keys {
            let r = self.regions.get_mut(&key).expect("region");
            let mut near: Vec<usize> = (0..r.groups.len())
                .filter(|&gi| {
                    let g = &r.groups[gi];
                    g.species == s && !g.live && dist(g.pos, at, wrap) <= radius
                })
                .collect();
            near.sort_by(|a, b| {
                dist(r.groups[*a].pos, at, wrap).total_cmp(&dist(r.groups[*b].pos, at, wrap))
            });
            for gi in near {
                let g = &mut r.groups[gi];
                while taken < n && g.adults() + g.juveniles as u32 > 0 {
                    stag = !stag;
                    if g.males > 0 && (stag || g.females == 0) {
                        g.males -= 1;
                    } else if g.females > 0 {
                        g.females -= 1;
                    } else {
                        g.juveniles -= 1;
                    }
                    taken += 1;
                }
            }
            r.groups.retain(|g| g.size() > 0 || g.live);
            if let Some(slot) = r.pool_species.iter().position(|x| *x == s) {
                for c in 0..REGION_LEN {
                    if taken >= n {
                        break;
                    }
                    if dist(r.cell_centre(c), at, wrap) <= radius {
                        let i = slot * REGION_LEN + c;
                        let k = r.adults[i].min((n - taken) as f32).floor();
                        r.adults[i] -= k;
                        taken += k as u32;
                    }
                }
            }
        }
        *self.deaths.entry((s, Cause::Hunting)).or_default() += taken as f64;
        taken
    }

    /// The area a species can live in across the loaded regions, km².
    pub fn range_km2(&self, species: usize) -> f64 {
        let sp = &self.catalog.species[species];
        let mut a = 0.0;
        for r in self.regions.values() {
            for h in &r.habitat {
                if self.suits(sp, h) {
                    a += Self::area(sp, h) as f64;
                }
            }
        }
        a
    }

    /// The animals of a species its range across the loaded regions holds at its usual
    /// density, by the habitat's quality.
    pub fn capacity(&self, species: usize) -> f64 {
        self.regions
            .values()
            .map(|r| r.capacity.get(species).copied().unwrap_or(0.0) as f64)
            .sum()
    }
}

/// The most of its need a hunter can make of plants (and worms and grubs): its share, and of the
/// meat the hunt did not give it, all for an omnivore (a badger, a bear) and little for a
/// hunter (an owl in a vole-poor year lives on worms and beetles, but thinly).
fn plant_cap(sp: &Species, need: f32, meat: f32) -> f32 {
    let share = sp.forage_share();
    let missing = (need * (1.0 - share) - meat).max(0.0);
    need * share + (0.6 + 0.4 * smoothstep(0.2, 0.7, share)) * missing
}

/// How far apart the homes of a territorial species keep: the hunters' ranges hardly overlap,
/// the rest hold a core of their range.
fn spacing(sp: &Species) -> f64 {
    sp.range_radius_m() * if sp.hunts() { 1.5 } else { 1.0 }
}

/// Food needed (or eaten) this step, kg: by group index, and by pool slot × cell.
struct Needs {
    groups: Vec<f32>,
    pools: Vec<f32>,
}

/// One consumer: a group or a pool in a cell.
#[derive(Debug, Clone, Copy)]
enum Who {
    Group(usize),
    Pool(usize, usize),
}

/// How a predator weighs a small prey's grown ones and young (its young count by what they
/// weigh, thrice over for being easier to catch); a fish reaches only the young of a land
/// animal that grows up in the water (tadpoles), not the grown ones on the land.
fn stage_weights(pred: &Species, prey: &Species) -> (f32, f32) {
    let wy = 3.0 * prey.young_appetite();
    if pred.aquatic && !prey.aquatic {
        (0.0, wy)
    } else {
        (1.0, wy)
    }
}

/// Takes `frac` of what a predator is offered of a pool (grown ones weighted `wa`, young
/// `wy`) out of it; returns the meat taken, in grown animals' worth.
fn take_stages(adults: &mut f32, young: &mut f32, frac: f32, wa: f32, wy: f32) -> f32 {
    let frac = frac.clamp(0.0, 0.5);
    let ka = *adults * wa * frac;
    let ky = *young * (wy > 0.0) as u8 as f32 * frac;
    *adults -= ka;
    *young -= ky;
    // A young one is a third of its catch weight in meat (`wy` counted it thrice).
    ka + ky * wy / 3.0
}

/// Kills one animal of a prey species from a group in reach, the weakest most likely.
fn kill_from_groups(
    r: &mut Region,
    prey: u16,
    pos: [f64; 2],
    reach: f64,
    wrap: f64,
    rng: &mut Rng,
) -> Option<(Stage, bool, [f64; 2])> {
    let in_reach = |g: &Group| g.species == prey && !g.live && dist(g.pos, pos, wrap) <= reach;
    let total: f32 = r
        .groups
        .iter()
        .filter(|g| in_reach(g))
        .map(Group::vulnerability)
        .sum();
    if total <= 0.0 {
        return None;
    }
    let mut x = rng.next_f32() * total;
    for g in r.groups.iter_mut() {
        if !in_reach(g) {
            continue;
        }
        let w = g.vulnerability();
        if x > w {
            x -= w;
            continue;
        }
        let weak = 1.0 + 1.5 * (1.0 - g.condition);
        let parts = [
            g.young as f32 * 2.0,
            g.juveniles as f32 * 1.2,
            g.females as f32 * weak,
            g.males as f32 * weak,
        ];
        let mut y = rng.next_f32() * parts.iter().sum::<f32>();
        for (k, p) in parts.iter().enumerate() {
            if *p > 0.0 && y <= *p {
                // Of the young, as many females as males.
                let half = (g.id ^ g.size() as u64).is_multiple_of(2);
                let (stage, female) = match k {
                    0 => {
                        g.young -= 1;
                        (Stage::Young, half)
                    }
                    1 => {
                        g.juveniles -= 1;
                        (Stage::Juvenile, half)
                    }
                    2 => {
                        g.females -= 1;
                        (Stage::Adult, true)
                    }
                    _ => {
                        g.males -= 1;
                        (Stage::Adult, false)
                    }
                };
                return Some((stage, female, g.pos));
            }
            y -= p;
        }
        return None;
    }
    None
}

/// A new group as a species lives: a herd of mothers with young and some males, a family, a
/// pack, a solitary adult (a female often with young).
fn founding_group(sp: &Species, rng: &mut Rng, id: u64, p: [f64; 2]) -> Group {
    let (lo, hi) = sp.group;
    let size = (lo as f32 + rng.next_f32() * (hi - lo + 1) as f32)
        .floor()
        .max(1.0) as u16;
    let (mut young, mut juveniles, mut females, mut males) = (0u16, 0u16, 0u16, 0u16);
    match sp.social {
        Social::Solitary => {
            if rng.next_f32() < 0.5 {
                females = 1;
                if rng.next_f32() < 0.5 {
                    young = sp.life.litter_mean().round().max(1.0) as u16;
                }
            } else {
                males = 1;
            }
        }
        Social::Pack { .. } => {
            females = 1;
            males = 1;
            let rest = size.saturating_sub(2);
            juveniles = rest / 2;
            young = rest - juveniles;
        }
        _ => {
            for _ in 0..size {
                let u = rng.next_f32();
                if u < 0.42 {
                    females += 1;
                } else if u < 0.62 {
                    males += 1;
                } else if u < 0.8 {
                    juveniles += 1;
                } else {
                    young += 1;
                }
            }
            females = females.max(1);
        }
    }
    Group {
        id,
        species: sp.index as u16,
        home: p,
        pos: p,
        young,
        juveniles,
        females,
        males,
        condition: 0.75,
        food: 0.0,
        live: false,
    }
}

/// The share of the production of a kind of forage the reference wood's animals eat at their
/// usual densities: what is left feeds the lean season, rots, and is eaten by what the
/// simulation does not count (insects, slugs, the animals of other realms).
const USED: f32 = 1.0;

/// Per kind of forage, the factor that makes the reference wood produce what its animals (the
/// Palearctic's, at their usual densities) eat over [`USED`]: the habitat's formulas give the
/// relative amounts from place to place, the animals' needs the absolute.
pub fn forage_scale(cat: &Catalog, reference: &Habitat) -> [f32; FORAGE_KINDS] {
    let mut prod = reference.forage;
    for _ in 0..4 {
        let mut demand = [0.0f32; FORAGE_KINDS];
        for sp in &cat.species {
            if !native(sp.realms, Realm::Palearctic) || sp.habitats & reference.ecosystems == 0 {
                continue;
            }
            let total: f32 = (0..FORAGE_KINDS).map(|k| sp.forage[k] * prod[k]).sum();
            if total <= 0.0 {
                continue;
            }
            // Per km² of the reference wood (of its water for fish).
            let area = if sp.aquatic {
                reference.fresh / reference.land.max(1e-6)
            } else {
                1.0
            };
            let season = if sp.ectotherm { 0.6 } else { 0.85 };
            let d = sp.density * area * sp.need_kg * 365.0 * season * sp.forage_share();
            for k in 0..FORAGE_KINDS {
                demand[k] += d * sp.forage[k] * prod[k] / total;
            }
        }
        for k in 0..FORAGE_KINDS {
            if demand[k] > 0.0 {
                let per_area = if Forage::ALL[k] == Forage::Aquatic {
                    reference.land / reference.fresh.max(1e-6)
                } else {
                    1.0
                };
                prod[k] = demand[k] * per_area / USED;
            }
        }
    }
    let mut scale = [1.0; FORAGE_KINDS];
    for k in 0..FORAGE_KINDS {
        if reference.forage[k] > 0.0 {
            scale[k] = prod[k] / reference.forage[k];
        }
    }
    scale
}

/// Each predator's attack rate in each realm (km² a year per animal): the rate at which, at
/// the usual densities of its prey species of that realm, it would meet its need for meat with
/// two fifths to spare.
pub fn calibrate_attack(cat: &Catalog) -> Vec<[f32; 8]> {
    cat.species
        .iter()
        .map(|sp| {
            let mut out = [0.0f32; 8];
            for (ri, realm) in Realm::ALL.into_iter().enumerate() {
                out[ri] = attack_in(cat, sp, realm);
            }
            out
        })
        .collect()
}

/// The most meat a hunter can eat in a year, at the rate of its hungriest season (its handling
/// time's limit): half as much again as its need — a cold-blooded hunter's need in the warm
/// months, when it eats its year's food, more than half again its yearly mean.
fn most_eaten(sp: &Species) -> f32 {
    let peak = if sp.ectotherm { ECTOTHERM_PEAK } else { 1.0 };
    (sp.need_kg * 365.0 * (1.0 - sp.forage_share()) * 1.5 * peak).max(1e-6)
}

fn attack_in(cat: &Catalog, sp: &Species, realm: Realm) -> f32 {
    if !sp.hunts() {
        return 0.0;
    }
    let need = sp.need_kg * 365.0 * (1.0 - sp.forage_share());
    let target = 1.4 * need;
    let (mut s, mut t) = (0.0f32, 0.0f32);
    for &(pj, pref) in &sp.prey {
        let prey = &cat.species[pj];
        if !native(prey.realms, realm) {
            continue;
        }
        let usual = usual_density(prey);
        // A specialist finds its prey where others would starve: it is set to live on a fifth
        // of its prey's usual numbers.
        let at = if specialist(sp) { 0.2 } else { CALIBRATE_AT };
        let d = noticed(sp, at * usual, usual);
        let e = edible(prey);
        let h = e / most_eaten(sp);
        s += pref * d * e;
        t += pref * d * h;
    }
    if s <= 0.0 {
        0.0
    } else if s <= target * t {
        1.0e6
    } else {
        target / (s - target * t)
    }
}

//! The run of deep time (D191–D194). Each step — two centuries in the far past, a generation in
//! the last sixty thousand years — every deme grows toward what its cell leaves it (the species
//! in a cell sharing its land, each counting the others by their own needs there) and sends a
//! share of itself to its neighbours by how much room they have, and across water within its
//! reach. Every few centuries the slower things are reckoned: lineages split, are cut off and
//! meet; techniques are invented, taken up from neighbours and lost; gene pools come toward
//! their sunlight, drift and mix; and the chronicle notes what is new. Every random draw comes
//! from a stream of the world's seed, the step and the cell, and every update reads the state
//! before it, so neither the order nor the threads change what happens.

use std::collections::{BTreeMap, BTreeSet};

use hearth_math::hash::{Rng, hash2};
use rayon::prelude::*;

use super::geo::NO_LANDMASS;
use super::{
    Climate, Deme, Event, Geography, Happening, History, Lineage, Mask, POOL_DIMS, Pool, Setup,
    VERSION, to_words,
};

/// Fewer than this (persons) is none.
const PRESENT: f32 = 1.0e-3;
/// The most of a deme that leaves it in a step.
const MOST_OUT: f32 = 0.5;
/// The crowding past which a deme's growth is reckoned (a cell it cannot live in).
const MOST_CROWD: f32 = 5.0;
/// The farthest (cells) a crossing of the sea is looked for.
const LINK_CELLS: i64 = 8;
/// Years between censuses.
const CENSUS_YEARS: f64 = 10_000.0;

/// A run, from the first people to the era's date.
pub struct Run<'a> {
    s: &'a Setup,
    geo: &'a Geography,
    climate: Climate,
    cells: usize,
    kinds: usize,
    /// Years ago now.
    t: f64,
    sea: f32,
    /// Per kind and cell (kind-major): people, lineage (0 none), knowledge, pool.
    n: Vec<f32>,
    lin: Vec<u32>,
    know: Vec<Mask>,
    pool: Vec<Pool>,
    /// Per cell: its land share now, whether it is on the sea now, what it gives a people
    /// before what they know (persons a unit of density), and its coldest month now.
    land: Vec<f32>,
    coast: Vec<bool>,
    base: Vec<f32>,
    cold: Vec<f32>,
    /// What each knowledge set known now brings (sorted by set).
    ways: Vec<(Mask, Ways)>,
    /// Per cell: the cells of other landmasses within crossing distance, and how far (km).
    links: Vec<Vec<(u32, f32)>>,
    lineages: Vec<Lineage>,
    contacts: BTreeMap<(u32, u32), f32>,
    chronicle: Vec<Event>,
    appeared: Vec<bool>,
    gone: Vec<bool>,
    realms_now: Vec<u16>,
    realms_seen: Vec<u16>,
    realms_left: Vec<u16>,
    census: Vec<super::Census>,
    masses_seen: Vec<BTreeSet<u32>>,
    invented: Vec<Mask>,
    known_on: BTreeMap<(usize, u32), Mask>,
    met: u64,
    /// Per kind: Earth's numbers over the planet's (what its peoples can keep is reckoned by).
    scale: Vec<f32>,
    /// How much denser than real its peoples live (a small planet's).
    denser: f32,
    step: u64,
}

/// What a set of techniques brings: the food it wins from the land, the more from water, the
/// coldest month it lets a people winter in.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Ways {
    food: f32,
    fish: f32,
    limit: f32,
}

/// A standard normal draw.
fn normal(rng: &mut Rng) -> f32 {
    let u = rng.next_f64().max(1e-12);
    let v = rng.next_f64();
    ((-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()) as f32
}

impl<'a> Run<'a> {
    pub fn new(s: &'a Setup, geo: &'a Geography) -> Self {
        let cells = geo.cells.len();
        let kinds = s.kinds.len();
        let start = s
            .kinds
            .iter()
            .map(|k| k.appears_ya)
            .fold(s.until_ya, f64::max);
        let mut r = Self {
            s,
            geo,
            climate: Climate::new(&s.settings),
            cells,
            kinds,
            t: start,
            sea: 0.0,
            n: vec![0.0; kinds * cells],
            lin: vec![0; kinds * cells],
            know: vec![0; kinds * cells],
            pool: vec![Pool::default(); kinds * cells],
            land: vec![0.0; cells],
            coast: vec![false; cells],
            base: vec![0.0; cells],
            cold: vec![0.0; cells],
            ways: Vec::new(),
            links: Vec::new(),
            lineages: Vec::new(),
            contacts: BTreeMap::new(),
            chronicle: Vec::new(),
            appeared: vec![false; kinds],
            gone: vec![false; kinds],
            realms_now: vec![0; kinds],
            realms_seen: vec![0; kinds],
            realms_left: vec![0; kinds],
            census: Vec::new(),
            masses_seen: vec![BTreeSet::new(); kinds],
            invented: vec![0; kinds],
            known_on: BTreeMap::new(),
            met: 0,
            scale: vec![1.0; kinds],
            denser: 1.0,
            step: 0,
        };
        let land_km2: f64 = geo
            .cells
            .iter()
            .map(|c| c.land_today() as f64 * geo.cell_km2())
            .sum();
        r.denser = (s.settings.people_km2 as f64 / land_km2.max(1.0))
            .clamp(1.0, s.settings.most_denser.max(1.0) as f64) as f32;
        r.shore();
        r.links = r.crossings();
        r
    }

    /// The length of a step `ya` years ago.
    fn step_years(&self) -> f64 {
        let mut years = 100.0;
        for &(from, y) in &self.s.settings.steps {
            if self.t <= from {
                years = y;
            }
        }
        years.max(1.0)
    }

    /// The cells of other landmasses about each cell near the sea, within [`LINK_CELLS`] and
    /// the longest reach any technique gives.
    fn crossings(&self) -> Vec<Vec<(u32, f32)>> {
        let geo = self.geo;
        let reach = self
            .s
            .techniques
            .iter()
            .map(|t| t.reach_km)
            .fold(self.s.settings.drift_reach_km, f32::max) as f64;
        let r = ((reach / geo.cell_km()).ceil() as i64).clamp(0, LINK_CELLS);
        let n = geo.n as i64;
        // Land at the lowest stands of the sea, and its landmass.
        let ever: Vec<bool> = geo.cells.iter().map(|c| c.land(-140.0) > 0.0).collect();
        (0..self.cells)
            .into_par_iter()
            .map(|c| {
                let mut out = Vec::new();
                if r < 2 || !ever[c] {
                    return out;
                }
                let here = geo.cells[c].landmass;
                let (i, j) = ((c % geo.n) as i64, (c / geo.n) as i64);
                for dj in -r..=r {
                    for di in -r..=r {
                        if di.abs().max(dj.abs()) < 2 || !(0..n).contains(&(j + dj)) {
                            continue;
                        }
                        let o = ((j + dj) * n + (i + di).rem_euclid(n)) as usize;
                        if !ever[o] || geo.cells[o].landmass == here {
                            continue;
                        }
                        let km = geo.km(c, o) as f32;
                        if km <= reach as f32 {
                            out.push((o as u32, km));
                        }
                    }
                }
                out
            })
            .collect()
    }

    /// The land, the shore and the climate now: each cell's land, what its land gives before
    /// what a people knows (the cold of a glacial making it poorer, ice-bound land nothing), and
    /// its coldest month.
    fn shore(&mut self) {
        self.sea = self.climate.sea_m(self.t);
        let (sea, d, rain) = (
            self.sea,
            self.climate.degrees(self.t),
            self.climate.rain(self.t),
        );
        let geo = self.geo;
        self.land = geo.cells.iter().map(|c| c.land(sea)).collect();
        let land = &self.land;
        self.coast = (0..self.cells)
            .map(|c| land[c] > 0.0 && (land[c] < 1.0 || geo.around(c).any(|(o, _)| land[o] <= 0.0)))
            .collect();
        let km2 = geo.cell_km2() as f32 * self.denser;
        self.base = geo
            .cells
            .iter()
            .zip(land)
            .map(|(g, &l)| {
                if l <= 0.0 || g.temp_c + d < -10.0 {
                    0.0
                } else {
                    km2 * l * g.yields(sea) * (1.0 + 0.03 * d).clamp(0.5, 1.0) * rain
                }
            })
            .collect();
        self.cold = geo.cells.iter().map(|g| g.coldest_c + d).collect();
    }

    /// What a set of techniques brings.
    fn ways_now(&self, know: Mask) -> Ways {
        let (mut food, mut fish) = (1.0f32, 1.0f32);
        for (i, t) in self.s.techniques.iter().enumerate() {
            if know & (1 << i) != 0 {
                food *= t.food;
                fish *= t.fishing;
            }
        }
        let limit = self
            .s
            .cold
            .iter()
            .filter(|(m, _)| know & m == *m && *m != 0)
            .fold(self.s.settings.bare_coldest_c, |l, (_, c)| l.min(*c));
        Ways {
            food: food.min(8.0),
            fish: fish.min(4.0),
            limit,
        }
    }

    /// What a set of techniques brings, from those known this step.
    fn ways_of(&self, know: Mask) -> Ways {
        match self.ways.binary_search_by_key(&know, |w| w.0) {
            Ok(i) => self.ways[i].1,
            Err(_) => self.ways_now(know),
        }
    }

    /// What a cell gives a people of a kind knowing `know`, in persons, now.
    fn capacity(&self, k: usize, c: usize, know: Mask) -> f32 {
        let base = self.base[c];
        if base <= 0.0 {
            return 0.0;
        }
        let w = self.ways_of(know);
        let s = &self.s.settings;
        // Water's foods, the richer for the gear it knows.
        let mut water = 1.0;
        if self.coast[c] {
            water += s.coast * w.fish;
        }
        if self.geo.cells[c].river {
            water += s.river * w.fish;
        }
        // The cold it can winter in.
        let kind = &self.s.kinds[k];
        let ok = ((self.cold[c] - (w.limit + kind.cold_c)) / 4.0 + 0.5).clamp(0.0, 1.0);
        kind.density * base * w.food * water * ok
    }

    /// Everyone in each cell, of every kind.
    fn totals(&self) -> Vec<f32> {
        (0..self.cells)
            .map(|c| (0..self.kinds).map(|k| self.n[k * self.cells + c]).sum())
            .collect()
    }

    /// How crowded a cell is for a kind with `cap` there: everyone in it against what the land
    /// would feed of that kind (the better equipped, needing less land each, are less crowded
    /// by the same numbers, and in the end crowd the others out).
    fn crowd(total: f32, cap: f32) -> f32 {
        if cap > 0.0 {
            (total / cap).min(MOST_CROWD)
        } else {
            MOST_CROWD
        }
    }

    /// The share of a deme that goes to a neighbour in a step, before the most that may leave.
    fn rate(&self, k: usize, dt: f64) -> f32 {
        let kind = &self.s.kinds[k];
        let r = kind.growth.max(1e-5) as f64;
        let v = kind.spread_km_year as f64;
        let d = v * v / (4.0 * r);
        let l = self.geo.cell_km();
        ((d * dt / (l * l)) as f32).min(MOST_OUT / 6.0)
    }

    /// How far across water a people knowing `know` reaches (km).
    fn reach(&self, know: Mask) -> f32 {
        self.s
            .techniques
            .iter()
            .enumerate()
            .filter(|(i, t)| t.reach_km > 0.0 && know & (1 << i) != 0)
            .fold(self.s.settings.drift_reach_km, |r, (_, t)| {
                r.max(t.reach_km)
            })
    }

    /// The people of a kind going from cell `i` to `j` in a step (before the most that may
    /// leave `i`): toward room, by the crowding between.
    #[allow(clippy::too_many_arguments)]
    fn flow(
        &self,
        k: usize,
        i: usize,
        j: usize,
        w: f32,
        m: f32,
        cap: &[f32],
        total: &[f32],
    ) -> f32 {
        let a = k * self.cells + i;
        if self.n[a] <= PRESENT || self.land[j] <= 0.0 {
            return 0.0;
        }
        let b = k * self.cells + j;
        // An empty cell is judged by what the comers know.
        let room = if self.n[b] > PRESENT {
            cap[b]
        } else {
            self.capacity(k, j, self.know[a])
        };
        if room <= 0.0 {
            return 0.0;
        }
        let push = Self::crowd(total[i], cap[a]) - Self::crowd(total[j], room);
        if push <= 0.0 {
            return 0.0;
        }
        let along = (self.coast[i] && self.coast[j])
            || (self.geo.cells[i].river && self.geo.cells[j].river);
        let m = if along {
            (m * 2.0).min(MOST_OUT / 6.0)
        } else {
            m
        };
        m * w * self.n[a] * push.min(1.0)
    }

    /// One step of `dt` years.
    fn step(&mut self, dt: f64) {
        self.step += 1;
        let (cells, kinds) = (self.cells, self.kinds);
        // What each knowledge set known now brings.
        let mut sets: Vec<Mask> = (0..kinds * cells)
            .filter(|&i| self.n[i] > PRESENT)
            .map(|i| self.know[i])
            .collect();
        sets.sort_unstable();
        sets.dedup();
        self.ways = sets.into_iter().map(|m| (m, self.ways_now(m))).collect();
        // What each deme's cell gives it, and the crowding.
        let cap: Vec<f32> = (0..kinds * cells)
            .into_par_iter()
            .map(|i| {
                if self.n[i] > PRESENT {
                    self.capacity(i / cells, i % cells, self.know[i])
                } else {
                    0.0
                }
            })
            .collect();
        let total = self.totals();
        // Growth toward what the land leaves.
        for i in 0..kinds * cells {
            if self.n[i] <= PRESENT {
                continue;
            }
            let r = self.s.kinds[i / cells].growth as f64;
            let crowd = Self::crowd(total[i % cells], cap[i]);
            let g = (r * dt * (1.0 - crowd) as f64).clamp(-30.0, 5.0);
            self.n[i] = (self.n[i] as f64 * g.exp()) as f32;
        }
        let total = self.totals();
        // Spread: what each deme sends (and the share of it that may go), then what each cell
        // takes in, both from the state before.
        let rates: Vec<f32> = (0..kinds).map(|k| self.rate(k, dt)).collect();
        let share = self.s.settings.crossing_share;
        let out: Vec<f32> = (0..kinds * cells)
            .into_par_iter()
            .map(|a| {
                let (k, i) = (a / cells, a % cells);
                if self.n[a] <= PRESENT {
                    return 1.0;
                }
                let m = rates[k];
                let mut sent: f32 = self
                    .geo
                    .around(i)
                    .map(|(j, w)| self.flow(k, i, j, w, m, &cap, &total))
                    .sum();
                let reach = self.reach(self.know[a]);
                for &(j, km) in &self.links[i] {
                    if km <= reach {
                        sent += share
                            * (1.0 - km / reach.max(1e-3))
                            * self.flow(k, i, j as usize, 1.0, m, &cap, &total);
                    }
                }
                let most = MOST_OUT * self.n[a];
                if sent > most { most / sent } else { 1.0 }
            })
            .collect();
        struct Took {
            n: f32,
            lin: u32,
            know: Mask,
            pool: Pool,
            founded: Option<f32>,
        }
        let took: Vec<Took> = (0..kinds * cells)
            .into_par_iter()
            .map(|b| {
                let (k, j) = (b / cells, b % cells);
                let m = rates[k];
                // What leaves it.
                let mut n = self.n[b];
                if n > PRESENT {
                    let mut sent: f32 = self
                        .geo
                        .around(j)
                        .map(|(o, w)| self.flow(k, j, o, w, m, &cap, &total))
                        .sum();
                    let reach = self.reach(self.know[b]);
                    for &(o, km) in &self.links[j] {
                        if km <= reach {
                            sent += share
                                * (1.0 - km / reach.max(1e-3))
                                * self.flow(k, j, o as usize, 1.0, m, &cap, &total);
                        }
                    }
                    n -= sent * out[b];
                }
                // What comes in, and from whom most.
                let mut came = Vec::new();
                for (i, w) in self.geo.around(j) {
                    let f = self.flow(k, i, j, w, m, &cap, &total) * out[k * cells + i];
                    if f > 0.0 {
                        came.push((i, f));
                    }
                }
                for &(i, km) in &self.links[j] {
                    let a = k * cells + i as usize;
                    let reach = self.reach(self.know[a]);
                    if km <= reach && self.n[a] > PRESENT {
                        let f = share
                            * (1.0 - km / reach.max(1e-3))
                            * self.flow(k, i as usize, j, 1.0, m, &cap, &total)
                            * out[a];
                        if f > 0.0 {
                            came.push((i as usize, f));
                        }
                    }
                }
                let inflow: f32 = came.iter().map(|c| c.1).sum();
                let mut lin = self.lin[b];
                let mut know = self.know[b];
                let mut pool = self.pool[b];
                let mut founded = None;
                if inflow > 0.0 {
                    let total = n.max(0.0) + inflow;
                    let mut mixed = Pool {
                        sun: pool.sun * n.max(0.0),
                        archaic: pool.archaic * n.max(0.0),
                        drift: pool.drift.map(|d| d * n.max(0.0)),
                    };
                    let mut most = (0usize, 0.0f32);
                    for &(i, f) in &came {
                        let p = &self.pool[k * cells + i];
                        mixed.sun += p.sun * f;
                        mixed.archaic += p.archaic * f;
                        for d in 0..POOL_DIMS {
                            mixed.drift[d] += p.drift[d] * f;
                        }
                        if f > most.1 || (f == most.1 && i < most.0) {
                            most = (i, f);
                        }
                    }
                    pool = Pool {
                        sun: mixed.sun / total,
                        archaic: mixed.archaic / total,
                        drift: mixed.drift.map(|d| d / total),
                    };
                    let from = k * cells + most.0;
                    if n <= PRESENT {
                        // Founded by the comers: the most numerous's ways and tongue.
                        lin = self.lin[from];
                        know = self.know[from];
                        founded = Some(inflow);
                    } else if most.1 > 0.25 * total {
                        know |= self.know[from] & self.s.kinds[k].repertoire;
                    }
                }
                Took {
                    n: n.max(0.0) + inflow,
                    lin,
                    know,
                    pool,
                    founded,
                }
            })
            .collect();
        // Founders' drift: a few founders carry a chance sample of their people's genes.
        let s = &self.s.settings;
        for (b, t) in took.into_iter().enumerate() {
            let k = b / cells;
            self.n[b] = if t.n > PRESENT { t.n } else { 0.0 };
            self.lin[b] = if self.n[b] > 0.0 { t.lin } else { 0 };
            self.know[b] = if self.n[b] > 0.0 { t.know } else { 0 };
            self.pool[b] = t.pool;
            if let Some(f) = t.founded
                && self.n[b] > 0.0
            {
                let founders = (f * self.scale[k]).max(1.0);
                let mut rng = Rng::new(hash2(
                    self.s.seed ^ 0xf0_0d,
                    self.step * 1_000_003 + b as u64,
                ));
                let sd = (s.drift / (2.0 * founders)).sqrt();
                for d in &mut self.pool[b].drift {
                    *d += sd * normal(&mut rng);
                }
            }
        }
    }

    /// A kind comes to be, `ya` years ago: from its forebears' people in its realm, or in the
    /// best country of its realm (the cradle).
    fn appear(&mut self, k: usize) {
        self.appeared[k] = true;
        let kind = &self.s.kinds[k];
        let cells = self.cells;
        // What it knows from its first day: what its forebears knew before it.
        let old = self
            .s
            .techniques
            .iter()
            .enumerate()
            .filter(|(_, t)| t.years_bp >= kind.appears_ya)
            .fold(0 as Mask, |m, (i, _)| m | (1 << i))
            & kind.repertoire;
        let in_realm = |c: usize| kind.realm.is_none_or(|r| self.geo.cells[c].realm == r);
        let mut first = None;
        if let Some(f) = kind.from {
            // The forebears' people there become it, lineage by lineage.
            let mut daughters: BTreeMap<u32, u32> = BTreeMap::new();
            for c in 0..cells {
                let a = f * cells + c;
                if self.n[a] <= PRESENT || !in_realm(c) {
                    continue;
                }
                let b = k * cells + c;
                let parent = self.lin[a];
                let lin = match daughters.get(&parent) {
                    Some(&l) => l,
                    None => {
                        let l = self.new_lineage(k, Some(parent).filter(|&p| p != 0), c);
                        daughters.insert(parent, l);
                        l
                    }
                };
                self.n[b] += self.n[a];
                self.lin[b] = lin;
                self.know[b] = (self.know[a] | old) & kind.repertoire;
                self.pool[b] = self.pool[a];
                self.n[a] = 0.0;
                self.lin[a] = 0;
                self.know[a] = 0;
                first.get_or_insert(c);
            }
        }
        if first.is_none() {
            // The cradle: the realm's best country, and the land about it.
            let best = (0..cells)
                .filter(|&c| in_realm(c))
                .map(|c| (self.capacity(k, c, old), c))
                .filter(|(cap, _)| *cap > 0.0)
                .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
            let Some((_, c0)) = best else {
                log::warn!("deep time: no country for {} to appear in", kind.id);
                return;
            };
            let lin = self.new_lineage(k, None, c0);
            let around: Vec<usize> = std::iter::once(c0)
                .chain(self.geo.around(c0).map(|(c, _)| c))
                .collect();
            for c in around {
                let cap = self.capacity(k, c, old);
                if cap <= 0.0 {
                    continue;
                }
                let b = k * cells + c;
                self.n[b] = 0.5 * cap;
                self.lin[b] = lin;
                self.know[b] = old;
                self.pool[b] = Pool {
                    sun: self.geo.cells[c].sun,
                    ..Pool::default()
                };
            }
            first = Some(c0);
        }
        let cell = first.unwrap_or(0) as u32;
        self.chronicle.push(Event {
            ya: self.t,
            cell,
            what: Happening::Appeared { species: k as u8 },
        });
    }

    /// A kind is gone.
    fn end(&mut self, k: usize) {
        self.gone[k] = true;
        let cells = self.cells;
        let mut last = None;
        for c in 0..cells {
            let b = k * cells + c;
            if self.n[b] > PRESENT {
                last = Some(c);
            }
            self.n[b] = 0.0;
            self.lin[b] = 0;
            self.know[b] = 0;
        }
        if let Some(c) = last {
            self.chronicle.push(Event {
                ya: self.t,
                cell: c as u32,
                what: Happening::Gone { species: k as u8 },
            });
        }
    }

    fn new_lineage(&mut self, k: usize, parent: Option<u32>, cell: usize) -> u32 {
        let id = self.lineages.len() as u32 + 1;
        self.lineages.push(Lineage {
            id,
            species: k as u8,
            parent,
            since_ya: self.t,
            cell: cell as u32,
            peak: 0.0,
            contacts: Vec::new(),
        });
        id
    }

    /// The slower things, every few centuries (`dt` years since the last).
    fn reckon(&mut self, dt: f64) {
        let cells = self.cells;
        let kinds = self.kinds;
        self.shore();
        // The census, every ten thousand years.
        if self
            .census
            .last()
            .is_none_or(|c| c.ya - self.t >= CENSUS_YEARS - 1.0)
        {
            self.census.push(super::Census {
                ya: self.t,
                people: (0..kinds)
                    .map(|k| (0..cells).map(|c| self.n[k * cells + c]).sum())
                    .collect(),
                sea_m: self.sea,
            });
        }
        // Earth's numbers over the planet's, per kind.
        for k in 0..kinds {
            let total: f64 = (0..cells).map(|c| self.n[k * cells + c] as f64).sum();
            self.scale[k] = if total > 0.0 {
                (self.s.kinds[k].people as f64 / total) as f32
            } else {
                1.0
            };
        }
        self.lineages_reckoned(dt);
        self.knowledge(dt);
        self.pools(dt);
        self.chronicled();
    }

    /// Lineages: their census, contact between neighbours, and splits — a part cut off from
    /// the rest, or a people grown past what one holds.
    fn lineages_reckoned(&mut self, dt: f64) {
        let cells = self.cells;
        let geo = self.geo;
        let mut by: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
        for b in 0..self.kinds * cells {
            if self.n[b] > PRESENT && self.lin[b] != 0 {
                by.entry(self.lin[b]).or_default().push(b);
            }
        }
        // Contact: neighbours of one kind and two lineages.
        for (&l, members) in &by {
            for &b in members {
                let (k, c) = (b / cells, b % cells);
                for (o, w) in geo.around(c) {
                    let ob = k * cells + o;
                    let lo = self.lin[ob];
                    if self.n[ob] > PRESENT && lo != 0 && lo > l {
                        *self.contacts.entry((l, lo)).or_default() +=
                            w * self.n[b].min(self.n[ob]) * dt as f32;
                    }
                }
            }
        }
        let apart = self.s.settings.apart_people;
        for (l, members) in by {
            let k = members[0] / cells;
            let people: f32 = members.iter().map(|&b| self.n[b]).sum();
            let li = l as usize - 1;
            self.lineages[li].peak = self.lineages[li].peak.max(people);
            // Its parts, by land.
            let set: BTreeSet<usize> = members.iter().map(|&b| b % cells).collect();
            let mut parts: Vec<Vec<usize>> = Vec::new();
            let mut seen: BTreeSet<usize> = BTreeSet::new();
            for &c in &set {
                if seen.contains(&c) {
                    continue;
                }
                let mut part = vec![c];
                seen.insert(c);
                let mut q = 0;
                while q < part.len() {
                    let x = part[q];
                    q += 1;
                    for (o, _) in geo.around(x) {
                        if set.contains(&o) && seen.insert(o) {
                            part.push(o);
                        }
                    }
                }
                parts.push(part);
            }
            let n = &self.n;
            let sum = |p: &[usize]| p.iter().map(|&c| n[k * cells + c]).sum::<f32>();
            let mut parts: Vec<(f32, Vec<usize>)> =
                parts.into_iter().map(|p| (sum(&p), p)).collect();
            parts.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1[0].cmp(&b.1[0])));
            for (people, p) in parts.iter().skip(1) {
                if *people >= apart {
                    self.split(k, l, p);
                }
            }
            // Grown past what one people holds: the half farthest from where it began.
            let (held, main) = (parts[0].0, &parts[0].1);
            if held > self.s.kinds[k].lineage_people && main.len() >= 2 {
                let origin = self.lineages[li].cell as usize;
                let far = *main
                    .iter()
                    .max_by(|&&a, &&b| {
                        geo.km(a, origin)
                            .total_cmp(&geo.km(b, origin))
                            .then(b.cmp(&a))
                    })
                    .unwrap_or(&main[0]);
                let inside: BTreeSet<usize> = main.iter().copied().collect();
                let mut part = vec![far];
                let mut taken: BTreeSet<usize> = [far].into_iter().collect();
                let mut got = self.n[k * cells + far];
                let mut q = 0;
                while q < part.len() && got < held * 0.5 {
                    let x = part[q];
                    q += 1;
                    for (o, _) in geo.around(x) {
                        if inside.contains(&o) && taken.insert(o) && got < held * 0.5 {
                            part.push(o);
                            got += self.n[k * cells + o];
                        }
                    }
                }
                if part.len() < main.len() {
                    self.split(k, l, &part);
                }
            }
        }
    }

    fn split(&mut self, k: usize, from: u32, part: &[usize]) {
        let cells = self.cells;
        let at = *part
            .iter()
            .max_by(|&&a, &&b| {
                self.n[k * cells + a]
                    .total_cmp(&self.n[k * cells + b])
                    .then(b.cmp(&a))
            })
            .unwrap_or(&part[0]);
        let d = self.new_lineage(k, Some(from), at);
        for &c in part {
            self.lin[k * cells + c] = d;
        }
        self.chronicle.push(Event {
            ya: self.t,
            cell: at as u32,
            what: Happening::Split {
                lineage: from,
                daughter: d,
            },
        });
    }

    /// The techniques a deme could take up now: in its kind's reach, dated by now, resting on
    /// what it knows.
    fn open(&self, k: usize, know: Mask) -> Mask {
        let rep = self.s.kinds[k].repertoire;
        self.s
            .techniques
            .iter()
            .enumerate()
            .filter(|(i, t)| rep & (1 << i) != 0 && self.t <= t.years_bp && t.requires & !know == 0)
            .fold(0 as Mask, |m, (i, _)| m | (1 << i))
            & !know
    }

    /// Knowledge (D194): invention by lineages, diffusion between neighbours, loss where too
    /// few keep a technique.
    fn knowledge(&mut self, dt: f64) {
        let cells = self.cells;
        let s = &self.s.settings;
        let geo = self.geo;
        let seed = self.s.seed;
        let step = self.step;
        // Invention: each lineage, at its largest deme, by its numbers reckoned as Earth's.
        let mut lineage_people: BTreeMap<u32, (f32, usize)> = BTreeMap::new();
        for b in 0..self.kinds * cells {
            if self.n[b] > PRESENT && self.lin[b] != 0 {
                let e = lineage_people.entry(self.lin[b]).or_insert((0.0, b));
                e.0 += self.n[b];
                if self.n[b] > self.n[e.1] {
                    e.1 = b;
                }
            }
        }
        for (&l, &(people, b)) in &lineage_people {
            let k = b / cells;
            let open = self.open(k, self.know[b]);
            if open == 0 {
                continue;
            }
            let earth = people * self.scale[k];
            let lambda = s.invent * earth / s.invent_people.max(1.0) * (dt / 100.0) as f32;
            let p = 1.0 - (-lambda).exp();
            let mut rng = Rng::new(hash2(seed ^ 0x001e_7e17, step * 1_000_003 + l as u64));
            for i in 0..self.s.techniques.len() {
                if open & (1 << i) != 0 && rng.next_f32() < p {
                    self.know[b] |= 1 << i;
                    if self.invented[k] & (1 << i) == 0 {
                        self.invented[k] |= 1 << i;
                        self.chronicle.push(Event {
                            ya: self.t,
                            cell: (b % cells) as u32,
                            what: Happening::Invented {
                                species: k as u8,
                                technique: i as u16,
                                lineage: l,
                            },
                        });
                    }
                }
            }
        }
        // Diffusion: passes enough for the techniques to travel `diffuse` km a year.
        let passes = ((s.diffuse as f64 * dt / geo.cell_km()).round() as usize).clamp(1, 16);
        for pass in 0..passes {
            let next: Vec<Mask> = (0..self.kinds * cells)
                .into_par_iter()
                .map(|b| {
                    let (k, c) = (b / cells, b % cells);
                    let know = self.know[b];
                    if self.n[b] <= PRESENT {
                        return know;
                    }
                    let open = self.open(k, know);
                    if open == 0 {
                        return know;
                    }
                    let mut rng = Rng::new(hash2(
                        seed ^ 0xd1ff_u64,
                        (step * 17 + pass as u64) * 1_000_003 + b as u64,
                    ));
                    let mut got = know;
                    for (o, w) in geo.around(c) {
                        let ob = k * cells + o;
                        if self.n[ob] <= PRESENT {
                            continue;
                        }
                        let theirs = self.know[ob] & open;
                        if theirs == 0 {
                            continue;
                        }
                        let same = self.lin[ob] == self.lin[b];
                        let p = 0.9 * w * if same { 1.0 } else { s.across_lineages };
                        for i in 0..self.s.techniques.len() {
                            if theirs & (1 << i) != 0 && rng.next_f32() < p {
                                got |= 1 << i;
                            }
                        }
                    }
                    got
                })
                .collect();
            self.know = next;
        }
        // Loss: what too few keep, deepest first, never what something kept rests on. Those
        // who can keep a technique among them are everyone of the kind joined to the deme by
        // land: a continent's people keep what an island's lose (Henrich 2004's Tasmania).
        let joined = self.joined();
        let scale = self.scale.clone();
        let techniques = &self.s.techniques;
        let next: Vec<Mask> = (0..self.kinds * cells)
            .into_par_iter()
            .map(|b| {
                let k = b / cells;
                let mut know = self.know[b];
                if self.n[b] <= PRESENT || know == 0 {
                    return know;
                }
                let earth = joined[b] * scale[k];
                let mut rng = Rng::new(hash2(seed ^ 0x105e, step * 1_000_003 + b as u64));
                for i in (0..techniques.len()).rev() {
                    if know & (1 << i) == 0 {
                        continue;
                    }
                    let keep = s.keep_people * (s.keep_depth * techniques[i].depth as f32).exp();
                    if earth >= keep {
                        continue;
                    }
                    let needed = techniques
                        .iter()
                        .enumerate()
                        .any(|(j, t)| know & (1 << j) != 0 && t.requires & (1 << i) != 0);
                    if needed {
                        continue;
                    }
                    let p = s.lose * (1.0 - earth / keep) * (dt / 100.0) as f32;
                    if rng.next_f32() < p {
                        know &= !(1 << i);
                    }
                }
                know
            })
            .collect();
        self.know = next;
    }

    /// For each deme, the people of its kind joined to it by land now (its part of the world).
    fn joined(&self) -> Vec<f32> {
        let cells = self.cells;
        let mut out = vec![0.0f32; self.kinds * cells];
        let mut part = vec![u32::MAX; self.kinds * cells];
        let mut next = 0u32;
        let mut sums: Vec<f32> = Vec::new();
        for start in 0..self.kinds * cells {
            if self.n[start] <= PRESENT || part[start] != u32::MAX {
                continue;
            }
            let k = start / cells;
            let mut queue = vec![start % cells];
            part[start] = next;
            let mut sum = 0.0;
            while let Some(c) = queue.pop() {
                sum += self.n[k * cells + c];
                for (o, _) in self.geo.around(c) {
                    let ob = k * cells + o;
                    if self.n[ob] > PRESENT && part[ob] == u32::MAX && self.land[o] > 0.0 {
                        part[ob] = next;
                        queue.push(o);
                    }
                }
            }
            sums.push(sum);
            next += 1;
        }
        for b in 0..self.kinds * cells {
            if part[b] != u32::MAX {
                out[b] = sums[part[b] as usize];
            }
        }
        out
    }

    /// Gene pools (D192): selection toward the sunlight, drift by how few they are, mixing
    /// with neighbours, and another species' ancestry where two live together.
    fn pools(&mut self, dt: f64) {
        let cells = self.cells;
        let s = &self.s.settings;
        let geo = self.geo;
        let seed = self.s.seed;
        let step = self.step;
        let toward = (1.0 - (-dt / s.selection_years.max(1.0)).exp()) as f32;
        let back = (-dt / s.drift_return_years.max(1.0)).exp() as f32;
        let flow = 0.3f32;
        let scale = self.scale.clone();
        let next: Vec<Pool> = (0..self.kinds * cells)
            .into_par_iter()
            .map(|b| {
                let (k, c) = (b / cells, b % cells);
                let mut p = self.pool[b];
                let n = self.n[b];
                if n <= PRESENT {
                    return p;
                }
                // Gene flow from the neighbours of its kind.
                let (mut sun, mut arch, mut drift, mut wsum) = (0.0, 0.0, [0.0f32; POOL_DIMS], 0.0);
                for (o, w) in geo.around(c) {
                    let ob = k * cells + o;
                    let m = self.n[ob];
                    if m <= PRESENT {
                        continue;
                    }
                    let q = &self.pool[ob];
                    let x = w * m / (n + m);
                    sun += x * (q.sun - p.sun);
                    arch += x * (q.archaic - p.archaic);
                    for (d, (qd, pd)) in drift.iter_mut().zip(q.drift.iter().zip(&p.drift)) {
                        *d += x * (qd - pd);
                    }
                    wsum += w;
                }
                if wsum > 0.0 {
                    let f = flow / wsum.max(1.0);
                    p.sun += f * sun;
                    p.archaic += f * arch;
                    for (pd, d) in p.drift.iter_mut().zip(&drift) {
                        *pd += f * d;
                    }
                }
                // Selection toward its own sun.
                p.sun += (geo.cells[c].sun - p.sun) * toward;
                // Drift, by how few they are (reckoned as Earth's numbers), slowly undone.
                let mut rng = Rng::new(hash2(seed ^ 0xd21f7, step * 1_000_003 + b as u64));
                let earth = (n * scale[k]).max(1.0);
                let sd = (s.drift * (dt / 25.0) as f32 / (2.0 * earth)).sqrt();
                for d in &mut p.drift {
                    *d = *d * back + sd * normal(&mut rng);
                }
                // Another species' ancestry, where one lives here too.
                let others: f32 = (0..self.kinds)
                    .filter(|&o| o != k)
                    .map(|o| self.n[o * cells + c])
                    .sum();
                if others > PRESENT {
                    p.archaic = (p.archaic
                        + s.archaic_mixing * others / (n + others) * (dt / 100.0) as f32)
                        .min(0.5);
                }
                p
            })
            .collect();
        self.pool = next;
    }

    /// What is new for the chronicle: realms reached and left, landmasses reached, species
    /// meeting, techniques lost from a landmass.
    fn chronicled(&mut self) {
        let cells = self.cells;
        let geo = self.geo;
        for k in 0..self.kinds {
            let mut realms = 0u16;
            let mut best: BTreeMap<u8, (f32, usize)> = BTreeMap::new();
            let mut masses: BTreeMap<u32, (f32, usize)> = BTreeMap::new();
            let mut known: BTreeMap<u32, Mask> = BTreeMap::new();
            for c in 0..cells {
                let b = k * cells + c;
                if self.n[b] <= PRESENT {
                    continue;
                }
                let g = &geo.cells[c];
                realms |= 1 << (g.realm & 15);
                let e = best.entry(g.realm).or_insert((0.0, c));
                if self.n[b] > e.0 {
                    *e = (self.n[b], c);
                }
                if g.landmass != NO_LANDMASS {
                    let e = masses.entry(g.landmass).or_insert((0.0, c));
                    if self.n[b] > e.0 {
                        *e = (self.n[b], c);
                    }
                    *known.entry(g.landmass).or_default() |= self.know[b];
                }
            }
            for (&r, &(_, c)) in &best {
                if self.realms_seen[k] & (1 << (r & 15)) == 0 {
                    self.chronicle.push(Event {
                        ya: self.t,
                        cell: c as u32,
                        what: Happening::Reached {
                            species: k as u8,
                            realm: r,
                        },
                    });
                }
            }
            for r in 0..16u8 {
                if self.realms_now[k] & (1 << r) != 0
                    && realms & (1 << r) == 0
                    && self.realms_left[k] & (1 << r) == 0
                {
                    self.realms_left[k] |= 1 << r;
                    self.chronicle.push(Event {
                        ya: self.t,
                        cell: 0,
                        what: Happening::Left {
                            species: k as u8,
                            realm: r,
                        },
                    });
                }
            }
            self.realms_seen[k] |= realms;
            self.realms_now[k] = realms;
            for (&m, &(_, c)) in &masses {
                if self.masses_seen[k].contains(&m) {
                    continue;
                }
                // Over land if a neighbour of its people's cells there is land of a landmass
                // its kind already held.
                let by_land = (0..cells).any(|x| {
                    geo.cells[x].landmass == m
                        && self.n[k * cells + x] > PRESENT
                        && geo.around(x).any(|(o, _)| {
                            self.land[o] > 0.0
                                && geo.cells[o].landmass != m
                                && self.masses_seen[k].contains(&geo.cells[o].landmass)
                        })
                });
                let first = self.masses_seen[k].is_empty();
                self.masses_seen[k].insert(m);
                if !first {
                    self.chronicle.push(Event {
                        ya: self.t,
                        cell: c as u32,
                        what: Happening::Crossed {
                            species: k as u8,
                            landmass: m,
                            by_sea: !by_land,
                        },
                    });
                }
            }
            for (&m, &now) in &known {
                if let Some(&before) = self.known_on.get(&(k, m)) {
                    let lost = before & !now;
                    for i in 0..self.s.techniques.len() {
                        if lost & (1 << i) != 0 && self.s.techniques[i].depth >= 2 {
                            let c = masses.get(&m).map_or(0, |e| e.1);
                            self.chronicle.push(Event {
                                ya: self.t,
                                cell: c as u32,
                                what: Happening::Lost {
                                    species: k as u8,
                                    technique: i as u16,
                                    landmass: m,
                                },
                            });
                        }
                    }
                }
                self.known_on.insert((k, m), now);
            }
            self.known_on
                .retain(|(kk, m), _| *kk != k || known.contains_key(m));
        }
        // Species meeting.
        for c in 0..cells {
            for a in 0..self.kinds {
                if self.n[a * cells + c] <= PRESENT {
                    continue;
                }
                for b in a + 1..self.kinds {
                    let bit = 1u64 << (a * 8 + b).min(63);
                    if self.n[b * cells + c] > PRESENT && self.met & bit == 0 {
                        self.met |= bit;
                        self.chronicle.push(Event {
                            ya: self.t,
                            cell: c as u32,
                            what: Happening::Met {
                                a: a as u8,
                                b: b as u8,
                            },
                        });
                    }
                }
            }
        }
    }

    /// Runs to the era's date, telling `progress` the share done.
    pub fn run(mut self, progress: &(dyn Fn(f32) + Sync)) -> History {
        let start = self.t;
        let until = self.s.until_ya;
        let every = self.s.settings.lineage_years.max(1.0);
        let mut since = 0.0;
        let mut last_told = -1.0f32;
        while self.t > until {
            for k in 0..self.kinds {
                if !self.appeared[k] && self.t <= self.s.kinds[k].appears_ya {
                    self.appear(k);
                }
                if !self.gone[k] && self.s.kinds[k].gone_ya.is_some_and(|g| self.t <= g) {
                    self.end(k);
                }
            }
            let dt = self.step_years().min(self.t - until);
            self.step(dt);
            self.t -= dt;
            since += dt;
            if since >= every || self.t <= until {
                self.reckon(since);
                since = 0.0;
            }
            let done = ((start - self.t) / (start - until).max(1.0)) as f32;
            if done - last_told >= 0.01 {
                last_told = done;
                progress(done.min(1.0));
            }
        }
        self.finish()
    }

    fn finish(mut self) -> History {
        self.shore();
        let cells = self.cells;
        let mut demes = Vec::new();
        for c in 0..cells {
            for k in 0..self.kinds {
                let b = k * cells + c;
                if self.n[b] > PRESENT {
                    demes.push(Deme {
                        cell: c as u32,
                        species: k as u8,
                        people: self.n[b],
                        lineage: self.lin[b],
                        know: to_words(
                            self.know[b] | (self.s.baseline & self.s.kinds[k].repertoire),
                        ),
                        pool: self.pool[b],
                    });
                }
            }
        }
        // Each lineage's closest contacts.
        let mut lineages = std::mem::take(&mut self.lineages);
        for (&(a, b), &v) in &self.contacts {
            for (x, y) in [(a, b), (b, a)] {
                if let Some(l) = lineages.get_mut(x as usize - 1) {
                    l.contacts.push((y, v));
                }
            }
        }
        for l in &mut lineages {
            l.contacts
                .sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
            l.contacts.truncate(8);
        }
        History {
            version: VERSION,
            seed: self.s.seed,
            era: self.s.era.clone(),
            years_ago: self.s.until_ya,
            n: self.geo.n,
            cell_m: self.geo.cell_m,
            denser: self.denser,
            branch_years: self.s.settings.branch_years_max,
            species: self.s.kinds.iter().map(|k| k.id.clone()).collect(),
            populations: self.s.kinds.iter().map(|k| k.population.clone()).collect(),
            techniques: self.s.techniques.iter().map(|t| t.id.clone()).collect(),
            land: self.land.clone(),
            demes,
            lineages,
            chronicle: self.chronicle,
            census: self.census,
        }
    }
}

/// Deep time from a setup over a geography.
pub fn run(s: &Setup, geo: &Geography, progress: &(dyn Fn(f32) + Sync)) -> History {
    Run::new(s, geo).run(progress)
}

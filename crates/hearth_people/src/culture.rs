//! Cultures (V2.1 §9; H5). Every band carries its culture: what it knows and practises (H2–H4)
//! and, drawn from its people's generator (`humans/culture/generators.ron`), its ways — its
//! values, where a new pair lives and how descent is reckoned, how much polygyny it allows, the
//! share of each work its women do, how it lays its dead to rest and greets, the foods it
//! forbids, its motif and its ways with strangers and quarrels. A band that splits off, or leaves
//! after a feud, or is cast out, takes a daughter of its culture, its lineage kept.
//!
//! Cultures live (V2.1 §9.2): each year a culture drifts a little at random — its values wander,
//! now and then a custom changes, its motif — and meets its neighbours' as their bands range, the
//! nearer the more, each drawing the other's values toward its own and now and then taking up a
//! custom of theirs; and each month a band's people take its culture in — their values moving
//! toward what their temperament and their culture make of them together, the young quickly, the
//! grown slowly, the conforming the more.

use hearth_content::Content;
use hearth_content::schema::culture::{Burial, Descent, Drift, Greeting, Residence, Span};
use hearth_math::hash::{Rng, hash2};
use serde::{Deserialize, Serialize};

use crate::conflict::Ways;
use crate::psyche::{PsycheDefs, Tendency, Value};
use crate::sim::People;
use crate::world::Now;

/// The salt of the streams cultures are drawn and drift on (the world's seed and a band's id).
pub(crate) const STREAM: u64 = 0x0c17_70e5_0000_0000;
/// How far apart two bands' homes may be and their cultures still touch as they range (m).
const CONTACT_M: f64 = 20_000.0;
/// The age (years) to which one takes in its culture as the young do.
const YOUNG: f64 = 15.0;

/// A culture's values (V2.1 §9.1), each 0–1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CultureValues {
    /// Egalitarian (0) … hierarchical (1).
    pub hierarchy: f32,
    /// Kin first (0) … wider cooperation (1).
    pub wide: f32,
    /// Conciliation (0) … honour (1) in conflict.
    pub honour: f32,
    /// Loose (0) … tight (1) norms.
    pub tight: f32,
}

impl Default for CultureValues {
    fn default() -> Self {
        Self {
            hierarchy: 0.2,
            wide: 0.5,
            honour: 0.35,
            tight: 0.45,
        }
    }
}

/// A people's culture generator, resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct Generator {
    pub species: String,
    pub residence: Vec<(Residence, f32)>,
    pub descent: Vec<(Descent, f32)>,
    pub polygyny: Span,
    pub hierarchy: Span,
    pub wide: Span,
    pub honour: Span,
    pub tight: Span,
    pub labour: Vec<(String, Span)>,
    pub burial: Vec<(Burial, f32)>,
    pub greeting: Vec<(Greeting, f32)>,
    pub taboos: (u8, u8),
    pub taboo_foods: Vec<String>,
    pub ways_spread: f32,
    pub drift: Drift,
}

/// Every people's generator.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Generators {
    pub list: Vec<Generator>,
}

fn key(id: &str) -> &str {
    id.rsplit(':').next().unwrap_or(id)
}

impl Generators {
    pub fn from_content(c: &Content) -> Self {
        Self {
            list: c
                .culture_generators
                .iter()
                .map(|g| Generator {
                    species: g.species.to_string(),
                    residence: g.residence.clone(),
                    descent: g.descent.clone(),
                    polygyny: g.polygyny,
                    hierarchy: g.values.hierarchy,
                    wide: g.values.wide,
                    honour: g.values.honour,
                    tight: g.values.tight,
                    labour: g.labour.clone(),
                    burial: g.burial.clone(),
                    greeting: g.greeting.clone(),
                    taboos: g.taboos,
                    taboo_foods: g.taboo_foods.iter().map(|f| f.to_string()).collect(),
                    ways_spread: g.ways_spread,
                    drift: g.drift,
                })
                .collect(),
        }
    }

    /// A species' generator, if its people have cultures.
    pub fn of(&self, species: &str) -> Option<&Generator> {
        self.list.iter().find(|g| key(&g.species) == key(species))
    }
}

/// One of a weighted list.
fn pick<T: Copy>(options: &[(T, f32)], rng: &mut Rng) -> Option<T> {
    let total: f32 = options.iter().map(|(_, w)| w.max(0.0)).sum();
    if total <= 0.0 {
        return options.first().map(|(t, _)| *t);
    }
    let mut r = rng.next_f32() * total;
    for (t, w) in options {
        r -= w.max(0.0);
        if r <= 0.0 {
            return Some(*t);
        }
    }
    options.last().map(|(t, _)| *t)
}

/// Somewhere in a span.
fn within(span: Span, rng: &mut Rng) -> f32 {
    span.0 + (span.1 - span.0) * rng.next_f32()
}

/// About a standard normal draw (the sum of three uniforms, rescaled).
fn normal(rng: &mut Rng) -> f32 {
    (rng.next_f32() + rng.next_f32() + rng.next_f32() - 1.5) * 2.0
}

impl crate::band::Culture {
    /// Whether its ways have been drawn.
    pub fn drawn(&self) -> bool {
        self.id != 0
    }

    /// Draws its ways (its knowledge and techniques kept) from its people's generator and their
    /// ways with strangers, as culture `id` begun on `day`.
    pub fn draw(&mut self, id: u64, g: &Generator, ways: Option<&Ways>, rng: &mut Rng, day: f64) {
        self.id = id;
        self.parent = None;
        self.since = day;
        self.values = crate::culture::CultureValues {
            hierarchy: within(g.hierarchy, rng),
            wide: within(g.wide, rng),
            honour: within(g.honour, rng),
            tight: within(g.tight, rng),
        };
        self.residence = pick(&g.residence, rng).unwrap_or_default();
        self.descent = pick(&g.descent, rng).unwrap_or_default();
        self.polygyny = within(g.polygyny, rng);
        self.labour = g
            .labour
            .iter()
            .map(|(work, span)| (work.clone(), within(*span, rng)))
            .collect();
        self.burial = pick(&g.burial, rng).unwrap_or_default();
        self.greeting = pick(&g.greeting, rng).unwrap_or_default();
        let (least, most) = (g.taboos.0 as usize, g.taboos.1.max(g.taboos.0) as usize);
        let n = least + (rng.next_f32() * (most - least + 1) as f32) as usize;
        let mut foods = g.taboo_foods.clone();
        self.taboos = Vec::new();
        while self.taboos.len() < n.min(g.taboo_foods.len()) && !foods.is_empty() {
            let k = ((rng.next_f32() * foods.len() as f32) as usize).min(foods.len() - 1);
            self.taboos.push(foods.swap_remove(k));
        }
        self.motif = rng.next_u32();
        self.ways = ways.map(|w| ways_for(w, &self.values, g.ways_spread, rng));
    }

    /// A daughter of it, for a band gone off on its own: the same ways, its own lineage.
    pub fn daughter(&self, id: u64, day: f64) -> Self {
        let mut d = self.clone();
        if self.drawn() {
            d.id = id;
            d.parent = Some(self.id);
            d.since = day;
            d.language = self.language.as_ref().map(|l| l.daughter(id));
        }
        d
    }

    /// A year of drift (V2.1 §9.2): its values wander a little, the shares of its work too
    /// within what its people's way of life allows; now and then a custom changes (where a pair
    /// lives, descent, how the dead are laid to rest, the greeting, a taboo taken up or let go),
    /// and its motif.
    pub fn drift(&mut self, g: &Generator, rng: &mut Rng) {
        if !self.drawn() {
            return;
        }
        let step = g.drift.value_step;
        for v in [
            &mut self.values.hierarchy,
            &mut self.values.wide,
            &mut self.values.honour,
            &mut self.values.tight,
        ] {
            *v = (*v + step * normal(rng)).clamp(0.0, 1.0);
        }
        for (work, share) in &mut self.labour {
            let k = key(work);
            let span = g
                .labour
                .iter()
                .find(|(w, _)| key(w) == k)
                .map_or((0.0, 1.0), |(_, s)| *s);
            *share = (*share + 0.5 * step * normal(rng)).clamp(span.0, span.1);
        }
        if rng.next_f32() < g.drift.custom {
            match (rng.next_f32() * 5.0) as u32 {
                0 => self.residence = pick(&g.residence, rng).unwrap_or(self.residence),
                1 => self.descent = pick(&g.descent, rng).unwrap_or(self.descent),
                2 => self.burial = pick(&g.burial, rng).unwrap_or(self.burial),
                3 => self.greeting = pick(&g.greeting, rng).unwrap_or(self.greeting),
                _ => {
                    let room = (self.taboos.len() as u8) < g.taboos.1;
                    if !self.taboos.is_empty() && (!room || rng.next_f32() < 0.5) {
                        let k = ((rng.next_f32() * self.taboos.len() as f32) as usize)
                            .min(self.taboos.len() - 1);
                        self.taboos.swap_remove(k);
                    } else if room && let Some(f) = g.taboo_foods.iter().find(|f| !self.forbids(f))
                    {
                        self.taboos.push(f.clone());
                    }
                }
            }
        }
        if rng.next_f32() < g.drift.motif {
            self.motif = rng.next_u32();
        }
    }

    /// A year's meeting with a neighbour's culture (V2.1 §9.2), as close as `closeness` (0–1):
    /// each draws the other's values toward its own, and now and then one takes up a custom of
    /// the other's.
    pub fn meet(&mut self, other: &mut Self, g: &Generator, closeness: f32, rng: &mut Rng) {
        if !self.drawn() || !other.drawn() {
            return;
        }
        let k = 0.5 * g.drift.contact * closeness.clamp(0.0, 1.0);
        for (a, b) in [
            (&mut self.values.hierarchy, &mut other.values.hierarchy),
            (&mut self.values.wide, &mut other.values.wide),
            (&mut self.values.honour, &mut other.values.honour),
            (&mut self.values.tight, &mut other.values.tight),
        ] {
            let m = (*a - *b) * k;
            *a -= m;
            *b += m;
        }
        // Neighbours lend each other words.
        if rng.next_f32() < g.drift.borrow * closeness
            && let (Some(a), Some(b)) = (&mut self.language, &mut other.language)
        {
            a.lend(b, rng);
        }
        if rng.next_f32() < g.drift.borrow * closeness {
            let ours = rng.next_f32() < 0.5;
            match (rng.next_f32() * 3.0) as u32 {
                0 if ours => other.burial = self.burial,
                0 => self.burial = other.burial,
                1 if ours => other.greeting = self.greeting,
                1 => self.greeting = other.greeting,
                _ if ours => other.motif = self.motif,
                _ => self.motif = other.motif,
            }
        }
    }

    /// How far its values lie from another culture's: the sum of their differences (0–4).
    pub fn value_gap(&self, other: &Self) -> f32 {
        (self.values.hierarchy - other.values.hierarchy).abs()
            + (self.values.wide - other.values.wide).abs()
            + (self.values.honour - other.values.honour).abs()
            + (self.values.tight - other.values.tight).abs()
    }

    /// How many of its customs another culture does not share (where a pair lives, descent,
    /// burial, greeting, taboos, motif).
    pub fn customs_apart(&self, other: &Self) -> usize {
        [
            self.residence != other.residence,
            self.descent != other.descent,
            self.burial != other.burial,
            self.greeting != other.greeting,
            self.taboos != other.taboos,
            self.motif != other.motif,
        ]
        .iter()
        .filter(|d| **d)
        .count()
    }

    /// The share of a work its women do (half for a work it has no custom for).
    pub fn women_share(&self, work: &str) -> f32 {
        let work = key(work);
        self.labour
            .iter()
            .find(|(w, _)| key(w) == work)
            .map_or(0.5, |(_, s)| *s)
    }

    /// How tight its norms are (a middling half before it is drawn).
    pub fn tightness(&self) -> f32 {
        if self.drawn() { self.values.tight } else { 0.5 }
    }

    /// How much it holds to honour in a quarrel (a middling 0.35 before it is drawn).
    pub fn honour(&self) -> f32 {
        if self.drawn() {
            self.values.honour
        } else {
            0.35
        }
    }

    /// Whether it forbids a food (a material).
    pub fn forbids(&self, material: &str) -> bool {
        let m = key(material);
        self.taboos.iter().any(|t| key(t) == m)
    }
}

/// A culture's own ways with strangers and quarrels: its people's, tilted by its values and a
/// little at random.
fn ways_for(w: &Ways, v: &CultureValues, spread: f32, rng: &mut Rng) -> Ways {
    let mut jitter = || 1.0 + spread * (2.0 * rng.next_f32() - 1.0);
    Ways {
        species: w.species.clone(),
        wary_m: w.wary_m * jitter() as f64,
        greet_m: (w.greet_m * jitter() as f64).min(w.wary_m),
        greeting_trust: (w.greeting_trust * jitter()).clamp(0.05, 0.9),
        // The more widely they cooperate, the more hospitable.
        hospitality: (w.hospitality * (0.7 + 0.6 * v.wide) * jitter()).clamp(0.0, 1.0),
        // The more they hold to honour, the sooner strangers are warned off.
        warn_off_crowding: w.warn_off_crowding * (1.3 - 0.6 * v.honour as f64) * jitter() as f64,
        // The tighter, the slower a stranger is taken in.
        take_in_trust: (w.take_in_trust * (0.8 + 0.4 * v.tight) * jitter()).clamp(0.1, 0.95),
        take_in_days: w.take_in_days * (0.5 + v.tight as f64) * jitter() as f64,
        argue_s: w.argue_s * jitter(),
        threat_s: w.threat_s * jitter(),
        blows_s: w.blows_s,
        leave_rivalry: (w.leave_rivalry * jitter()).clamp(0.3, 0.95),
        leave_quarrels: w.leave_quarrels,
    }
}

impl People {
    /// A year of a band's culture, as its life course reaches it: its drift and its language's,
    /// and its meeting with its neighbours' of the same people, the nearer the more.
    pub(crate) fn culture_year(&mut self, bi: usize, sp: &crate::species::Species, year: u64) {
        let Some(g) = &sp.culture else {
            return;
        };
        if !self.bands[bi].culture.drawn() {
            return;
        }
        let (id, home) = (self.bands[bi].id, self.bands[bi].home);
        let year_salt = year.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let mut rng = Rng::new(hash2(self.seed ^ STREAM ^ year_salt, id));
        self.bands[bi].culture.drift(g, &mut rng);
        if let (Some(l), Some(d)) = (&mut self.bands[bi].culture.language, &sp.language) {
            l.drift(d, &mut rng);
        }
        let kind = self.bands[bi].species.clone();
        let near: Vec<(usize, f32)> = (0..self.bands.len())
            .filter(|&b| {
                b != bi
                    && self.bands[b].species == kind
                    && self.bands[b].culture.drawn()
                    && !self.bands[b].members.is_empty()
            })
            .filter_map(|b| {
                let d = (self.bands[b].home - home).length();
                (d < CONTACT_M).then_some((b, (1.0 - d / CONTACT_M) as f32))
            })
            .collect();
        for (b, closeness) in near {
            let mut other = std::mem::take(&mut self.bands[b].culture);
            self.bands[bi]
                .culture
                .meet(&mut other, g, closeness, &mut rng);
            self.bands[b].culture = other;
        }
    }

    /// A month of a band's people taking its culture in (V2.1 §9.2): each one's values move
    /// toward what its temperament and its culture make of them together — six parts its own,
    /// four its culture's — the young a tenth of the way a month, the grown a hundredth, the
    /// conforming the more. Honour from the culture's honour; deference and autonomy from its
    /// hierarchy; kin loyalty and generosity from how widely it cooperates.
    pub(crate) fn enculturate(&mut self, bi: usize, defs: &PsycheDefs, now: &Now) {
        let c = &self.bands[bi].culture;
        if !c.drawn() {
            return;
        }
        let v = c.values;
        let targets = [
            (Value::Honor, v.honour),
            (Value::Deference, v.hierarchy),
            (Value::Autonomy, 1.0 - v.hierarchy),
            (Value::KinLoyalty, 1.0 - v.wide),
            (Value::Generosity, 0.3 + 0.5 * v.wide),
        ];
        for i in self.band_members(bi) {
            let p = &mut self.persons[i];
            if p.player.is_some() || !p.psyche.formed {
                continue;
            }
            let young = p.age(now) < YOUNG;
            let rate =
                if young { 0.1 } else { 0.01 } * (0.5 + p.psyche.tendency(Tendency::Conformity));
            for (value, ours) in targets {
                let own = defs.values[value as usize].of(p.phenotype.as_ref());
                let target = 0.6 * own + 0.4 * ours;
                let x = &mut p.psyche.values[value];
                *x += (target - *x) * rate;
            }
        }
    }
}

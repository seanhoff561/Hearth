//! Cultures (V2.1 §9; H5). Every band carries its culture: what it knows and practises (H2–H4)
//! and, drawn from its people's generator (`humans/culture/generators.ron`), its ways — its
//! values, where a new pair lives and how descent is reckoned, how much polygyny it allows, the
//! share of each work its women do, how it lays its dead to rest and greets, the foods it
//! forbids, its motif and its ways with strangers and quarrels. A band that splits off, or leaves
//! after a feud, or is cast out, takes a daughter of its culture, its lineage kept.

use hearth_content::Content;
use hearth_content::schema::culture::{Burial, Descent, Drift, Greeting, Residence, Span};
use hearth_math::hash::Rng;
use serde::{Deserialize, Serialize};

use crate::conflict::Ways;

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
        }
        d
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

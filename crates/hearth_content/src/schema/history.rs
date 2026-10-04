//! Deep time (`humans/history/`), V2.1 §15.1; H8: how the history grid is laid over the planet,
//! how the climate swings with the ice ages, what each biome gives foragers, what the cold and
//! the sea ask of a people, and how gene pools, knowledge and lineages change through it.

use serde::{Deserialize, Serialize};

use super::entry;
use crate::IdRef;

/// A stretch of the ice ages' stylised cycles: from `from_ya` (years ago) on, glacials come every
/// `period_years`, lowering the sea `low_m` at their depth (negative).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cycles {
    pub from_ya: f64,
    pub period_years: f64,
    pub low_m: f32,
}

/// The coldest month (°C) a people can winter in when it knows all of `needs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColdStep {
    pub needs: Vec<IdRef>,
    pub coldest_c: f32,
}

/// A technique and what it brings: water's foods (a factor on a coast's or a river's yield) or
/// a reach across the sea (km).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnownGives {
    pub knowledge: IdRef,
    pub gives: f32,
}

entry! {
    /// How deep time runs (V2.1 §15.1).
    pub struct HistorySettings in "humans/history/settings", schema 1, name name {
        pub name: String,
        /// A history cell's edge (km) where the planet allows, and the fewest and the most cells
        /// along the planet's circumference.
        pub cell_km: f32,
        pub cells: (u16, u16),
        /// How long a step is (years): `(from_ya, years)` — from that many years ago on, steps of
        /// that length; oldest first.
        pub steps: Vec<(f64, f64)>,
        /// The ice ages' stylised cycles, oldest first, until `curve` begins.
        pub cycles: Vec<Cycles>,
        /// The sea against today's (m) at points of the last glacial cycle: `(years_ago, m)`,
        /// oldest first; between points, the straight line.
        pub curve: Vec<(f64, f32)>,
        /// The air's temperature against today's for each metre the sea stands lower (°C), and
        /// the rain's share lost for each degree colder.
        pub degrees_per_metre: f32,
        pub rain_per_degree: f32,
        /// What each biome (by its generator name) gives foragers: a factor on a people's
        /// density, 1 for open savanna–woodland.
        pub biomes: Vec<(String, f32)>,
        /// What a coast and a river add (as a share of the cell's own) to a people who take
        /// nothing but shellfish and stranded fish, and what each fishing technique multiplies it
        /// by.
        pub coast: f32,
        pub river: f32,
        pub fishing: Vec<KnownGives>,
        /// What each technique that wins more food from the land multiplies a people's density
        /// by (D194): the better equipped live thicker on the same land, and crowd the others
        /// out where they meet.
        pub food: Vec<KnownGives>,
        /// The land (km²) one people's country spans: on a planet with less land, its peoples
        /// live denser than real by as much, at most `most_denser` times (D196).
        pub people_km2: f32,
        pub most_denser: f32,
        /// The coldest month (°C) a people without fire or clothing can winter in, and what
        /// knowing more allows.
        pub bare_coldest_c: f32,
        pub cold: Vec<ColdStep>,
        /// How far across water (km) a people drifts without craft, and what craft reaches.
        pub drift_reach_km: f32,
        pub craft: Vec<KnownGives>,
        /// The share of a step's spread that goes over water, against over land.
        pub crossing_share: f32,
        /// Gene pools: the years in which a pool's pigmentation comes most of the way to its
        /// sunlight (selection), how strongly few founders move it (drift), the years in which
        /// drift is undone (mutation and the species' gene flow), and how much of a neighbouring
        /// species' ancestry a step of living beside it brings.
        pub selection_years: f64,
        pub drift: f32,
        pub drift_return_years: f64,
        pub archaic_mixing: f32,
        /// Knowledge: the chance a century that a people of `invent_people` invents a technique
        /// whose groundwork it has; how far a technique travels between neighbouring peoples (km
        /// a year), less across lineages by `across_lineages`; the people (reckoned as Earth's)
        /// a technique of depth 0 needs to be kept by those joined to it by land (rising by
        /// `keep_depth` a level), and the chance a century of losing it when there are none.
        pub invent: f32,
        pub invent_people: f32,
        pub diffuse: f32,
        pub across_lineages: f32,
        pub keep_people: f32,
        pub keep_depth: f32,
        pub lose: f32,
        /// Lineages are reckoned (split, cut off, in contact) every so many years; a branch's
        /// culture and language are replayed for at most so many years.
        pub lineage_years: f64,
        pub branch_years_max: f64,
        /// The fewest people of a part cut off from its lineage that become a lineage of their
        /// own.
        pub apart_people: f32,
    }
}

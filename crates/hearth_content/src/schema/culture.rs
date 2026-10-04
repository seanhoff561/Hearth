//! Cultures (`humans/culture/`), V2.1 §9; H5: what a people's culture is drawn from — each of
//! its traits' options and how often the world's peoples of that way of life have them — and how
//! fast cultures change.

use serde::{Deserialize, Serialize};

use super::entry;
use crate::IdRef;

/// Where a pair lives once paired (V2.1 §9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Residence {
    /// With either's people, as suits them.
    #[default]
    Multilocal,
    /// With the husband's people.
    Patrilocal,
    /// With the wife's people.
    Matrilocal,
    /// Apart from both, a household of their own in either's band.
    Neolocal,
}

/// How descent is reckoned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Descent {
    /// Through both parents.
    #[default]
    Bilateral,
    /// Through the father.
    Patrilineal,
    /// Through the mother.
    Matrilineal,
}

/// How the dead are laid to rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Burial {
    /// In a grave.
    #[default]
    Buried,
    /// In a grave, with the things they carried.
    BuriedWithGoods,
    /// Laid out away from camp, under brush.
    LaidOut,
    /// Burned (where fire is kept).
    Burned,
}

/// How a people greets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Greeting {
    /// An embrace.
    #[default]
    Embrace,
    /// Hands touched or clasped.
    Hands,
    /// A call from afar, answered, before coming near.
    Call,
    /// A small gift held out.
    Gift,
}

/// A range a value is drawn from.
pub type Span = (f32, f32);

/// The spans of a culture's values (V2.1 §9.1), each 0–1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueSpans {
    /// Egalitarian (0) … hierarchical (1).
    pub hierarchy: Span,
    /// Kin first (0) … wider cooperation (1).
    pub wide: Span,
    /// Conciliation (0) … honour (1) in conflict.
    pub honour: Span,
    /// Loose (0) … tight (1) norms.
    pub tight: Span,
}

/// How fast a culture changes: the chances a year of the life course.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Drift {
    /// A value moves, by up to this much a year (its standard step).
    pub value_step: f32,
    /// A custom (residence, descent, burial, greeting, a taboo) changes.
    pub custom: f32,
    /// Its motif changes.
    pub motif: f32,
    /// What a year's contact with a neighbour moves its values toward theirs, and the chance it
    /// takes up one of their customs.
    pub contact: f32,
    pub borrow: f32,
}

entry! {
    /// What a people's culture is drawn from (V2.1 §9.1).
    pub struct CultureGenerator in "humans/culture/generators", schema 1, name name {
        pub name: String,
        pub species: IdRef,
        pub residence: Vec<(Residence, f32)>,
        pub descent: Vec<(Descent, f32)>,
        /// The share of men who may have a second wife.
        pub polygyny: Span,
        pub values: ValueSpans,
        /// For each skill (and gathering, hunting and child care), the share of it its women
        /// do: the only place a person's sex shapes its work (ground rule 6).
        pub labour: Vec<(String, Span)>,
        pub burial: Vec<(Burial, f32)>,
        pub greeting: Vec<(Greeting, f32)>,
        /// How many foods it may forbid, and which may be forbidden.
        pub taboos: (u8, u8),
        pub taboo_foods: Vec<IdRef>,
        /// How far each of its ways with strangers and quarrels may lie from its people's, a
        /// share either side.
        pub ways_spread: f32,
        pub drift: Drift,
    }
}

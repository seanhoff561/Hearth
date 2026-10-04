//! The life course (`humans/life/`), V2.1 §7 and §14.3; H3: a people's life table — how they
//! die at each age, how often their women conceive, how long they nurse, when they pair, how
//! large their bands grow — as data, with its sources.

use serde::{Deserialize, Serialize};

use super::entry;
use crate::IdRef;

/// Deaths by age: a Siler hazard, `a1·e^(−b1·x) + a2 + a3·e^(b3·x)` a year at age `x` — the
/// infant and child deaths falling away, a constant adult risk, and ageing's rising one.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Siler {
    pub a1: f32,
    pub b1: f32,
    pub a2: f32,
    pub a3: f32,
    pub b3: f32,
}

impl Siler {
    /// The yearly hazard of death at an age.
    pub fn hazard(&self, age: f64) -> f64 {
        self.a1 as f64 * (-(self.b1 as f64) * age).exp()
            + self.a2 as f64
            + self.a3 as f64 * (self.b3 as f64 * age).exp()
    }
}

/// How crowding tells on a people (see [`LifeTable::crowding`]).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Crowding {
    pub children: f32,
    pub conception: f32,
}

entry! {
    /// A people's life table (V2.1 §7, §14.3).
    pub struct LifeTable in "humans/life/tables", schema 1, name name {
        pub name: String,
        pub species: IdRef,
        pub mortality: Siler,
        /// A paired woman's chance to conceive in a month, by age (between the ages listed, the
        /// straight line; outside them, none).
        pub fecundability: Vec<(f32, f32)>,
        /// Years a child is nursed, and the factor nursing puts on its mother's conceiving.
        pub nursing_years: f32,
        pub nursing_factor: f32,
        /// Twins in a birth; a mother's death in giving birth.
        pub twins: f32,
        pub maternal_death: f32,
        /// The ages women and men first pair.
        pub pairing_age: (f32, f32),
        /// The age the young are counted grown, and a player's childhood ends (Addendum A).
        pub coming_of_age: f32,
        /// The people a square kilometre of their land feeds well (where more live about,
        /// children die more and fewer are born), and the number at which a band splits in two.
        pub density: f32,
        pub band_splits_at: u16,
        /// How crowding past what the land feeds tells: the children's deaths multiplied by one
        /// and this for each part crowded, conceptions divided by the crowding to this power.
        pub crowding: Crowding,
    }
}

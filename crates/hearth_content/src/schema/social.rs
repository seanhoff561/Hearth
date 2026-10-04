//! Social norms (`humans/social/`), V2.1 §8.4; H4: what a people holds one must and must not do,
//! what breaks it, how heavily, and the sanctions those who see or hear of it apply — the
//! foragers' until cultures carry their own (H5).

use serde::{Deserialize, Serialize};

use super::entry;
use crate::IdRef;

/// What breaks a norm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Breach {
    /// Taking a thing that is another's.
    Taking,
    /// Keeping food from one hungry near (demand sharing among foragers).
    Withholding,
}

/// What those who know of a breach do to the one who broke it, from the lightest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Sanction {
    /// Mocked to their face: they feel shame.
    Ridicule,
    /// Kept away from: less company, less trust.
    Avoidance,
    /// Food and help not shared with them.
    Withholding,
    /// Cast out of the band.
    Ostracism,
}

entry! {
    /// A norm of a people (V2.1 §8.4).
    pub struct Norm in "humans/social/norms", schema 1, name name {
        pub name: String,
        pub species: IdRef,
        pub breach: Breach,
        /// How heavily a breach weighs, 0–1: how far it moves what others believe of the one
        /// who broke it.
        pub severity: f32,
        /// The sanctions it brings, each from how bad a name (−1 … 0) the one who broke it has
        /// among those who apply it.
        pub sanctions: Vec<(Sanction, f32)>,
    }
}

//! Social norms and ways (`humans/social/`), V2.1 §8.4–8.7; H4: what a people holds one must and
//! must not do, what breaks it, how heavily, and the sanctions those who see or hear of it apply;
//! how it meets a stranger and how its quarrels run — the foragers' until cultures carry their
//! own (H5).

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

entry! {
    /// A people's ways with strangers and with quarrels (V2.1 §8.6–8.7).
    pub struct Ways in "humans/social/ways", schema 1, name name {
        pub name: String,
        pub species: IdRef,
        /// How near a stranger comes before it is watched, and before it is met (m).
        pub wary_m: f32,
        pub greet_m: f32,
        /// How far a greeting takes trust in a stranger (0–1): a guest's first trust.
        pub greeting_trust: f32,
        /// How readily a hungry guest is fed, to one of their own (0–1).
        pub hospitality: f32,
        /// How crowded their country is (its people to the number it feeds well) before
        /// strangers are warned off instead of met.
        pub warn_off_crowding: f32,
        /// A guest whom most of the grown trust this far, among them this many days, is taken
        /// in.
        pub take_in_trust: f32,
        pub take_in_days: f32,
        /// Seconds a quarrel stays an argument, and a threat, before it eases or rises; and the
        /// seconds of the scuffle it may come to.
        pub argue_s: f32,
        pub threat_s: f32,
        pub blows_s: f32,
        /// A feud: rivalry this deep after this many quarrels, and one side leaves the band.
        pub leave_rivalry: f32,
        pub leave_quarrels: u8,
    }
}

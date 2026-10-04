//! The psyche (`humans/psyche/`), V2.1 §5; H2: how personality shows in behaviour, the feelings
//! and how they are felt, caught and shown, and the values people hold — as data. The kinds are
//! fixed by the people's minds (`hearth_people::psyche`); the numbers are here.

use serde::{Deserialize, Serialize};

use super::{Range, entry};
use crate::IdRef;

/// A heritable trait a tendency, a feeling or a value leans on, and how much.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lean {
    pub of: IdRef,
    pub weight: f32,
}

entry! {
    /// A behaviour tendency (V2.1 §5.1): its middle, its range, and how far it moves per
    /// standard deviation of the traits it leans on. Kept small: personality tilts choices, the
    /// situation decides them.
    pub struct Tendency in "humans/psyche/tendencies", schema 1, name name {
        pub name: String,
        pub middle: f32,
        pub range: Range,
        pub spread: f32,
        pub from: Vec<Lean>,
    }
}

/// How a feeling shows on a body: the figure's posture and face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Display {
    #[default]
    None,
    /// Hunched and drawn in, brows up, watching the threat.
    Cower,
    /// Upright and wide, brows down, leaning in.
    Bristle,
    /// Head bowed, shoulders down, slow.
    Slump,
    /// Upright and loose, the head up.
    Bright,
    /// Turned half away, the head back.
    Recoil,
    /// Turned toward the other, the head tilted.
    Warm,
    /// Head down and turned aside, the eyes low.
    Hang,
}

/// How fast a feeling fades: a moment's (in seconds of play, as the player sees it pass) or a
/// lasting one's (in days of the world).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Fade {
    Seconds(f32),
    Days(f32),
}

entry! {
    /// A feeling (V2.1 §5.2): its weight in the mood, how fast it fades, how much of another's is
    /// caught, how it shows, and the traits that make it stronger.
    pub struct Feeling in "humans/psyche/feelings", schema 1, name name {
        pub name: String,
        /// −1 unpleasant … 1 pleasant.
        pub valence: f32,
        /// How long for half of it to fade.
        pub half_life: Fade,
        /// How much of another's, seen at full strength, is caught in a second (0–1).
        pub contagion: f32,
        pub display: Display,
        /// How much stronger it is felt per standard deviation of these traits.
        #[serde(default)]
        pub reactivity: Vec<Lean>,
    }
}

entry! {
    /// A value (V2.1 §5.3): its weight where a culture says nothing of it (H5 gives cultures
    /// theirs), and the traits that tilt it in a person.
    pub struct Value in "humans/psyche/values", schema 1, name name {
        pub name: String,
        pub default: f32,
        pub spread: f32,
        #[serde(default)]
        pub from: Vec<Lean>,
    }
}

//! Global configuration files: `units.ron`, `time.ron` and the balance layer (`balance/`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::entry;

/// A unit a quantity can be displayed in: `display = si * factor + offset`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayUnit {
    pub unit: String,
    pub factor: f64,
    #[serde(default)]
    pub offset: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quantity {
    pub name: String,
    /// The SI unit used internally.
    pub si: String,
    pub display: Vec<DisplayUnit>,
}

/// `units.ron`: SI internally everywhere, conversions only at the UI (v2 §3.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Units {
    pub schema: u32,
    pub quantities: Vec<Quantity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SleepAcceleration {
    /// Maximum time acceleration while sleeping.
    pub max_factor: f64,
    /// Seconds to ramp up to it (and back down on waking).
    pub ramp_s: f64,
}

/// `time.ron`: how sleep and rest speed time up (E §4.3). The clock itself is Earth's
/// (`hearth_env::calendar`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeConfig {
    pub schema: u32,
    pub sleep: SleepAcceleration,
}

entry! {
    /// A tunable global multiplier (hunger rate, predator aggression, growth rate...).
    pub struct BalanceKey in "balance/keys", schema 1, name name {
        pub name: String,
        pub description: String,
        pub default: f32,
        pub min: f32,
        pub max: f32,
    }
}

entry! {
    /// A realism preset: values for balance keys (unlisted keys use their defaults).
    pub struct BalancePreset in "balance/presets", schema 1, name name {
        pub name: String,
        pub description: String,
        /// Offered in world creation (Custom is implied and not a preset).
        #[serde(default = "yes")]
        pub selectable: bool,
        pub values: BTreeMap<String, f32>,
    }
}

entry! {
    /// A game mode (Amendment P §2): what it says of itself and the rules it sets — the realism
    /// preset, how knowledge is gained, what death means, hints, the clock, whether the world
    /// may be watched and its time and weather changed, Creative's powers, and how a life starts.
    pub struct GameMode in "balance/modes", schema 1, name name {
        pub name: String,
        /// The summary Create World shows.
        pub summary: String,
        /// Its place among the modes (Create World's order).
        pub order: u32,
        /// How strict it is: a world's mode may change only toward a lower strictness.
        pub strictness: u32,
        /// The realism preset (`balance/presets`) its needs and injuries follow; empty: none (no
        /// needs and no harm, Creative).
        pub realism: String,
        /// Discovery, Guided or Open.
        pub knowledge: String,
        /// What a new life keeps of what earlier lives knew (theirs_only, head_start,
        /// keep_everything; Amendment E §6.6).
        pub after_death: String,
        /// Predators' ways (authentic, wild, tranquil).
        pub predators: String,
        /// First-time hints on unless turned off.
        pub hints: bool,
        /// The clock shown: none (the time in words), optional (a small clock and day counter
        /// may be shown), exact (an exact clock and date, editable).
        pub clock: String,
        /// Watching the world, the Observer, and changing time and weather.
        pub observer: bool,
        /// Creative's powers: needs always full and no harm, flight, the creative inventory,
        /// instant actions, clear view.
        pub creative: bool,
    }
}

fn yes() -> bool {
    true
}

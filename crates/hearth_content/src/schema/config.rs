//! Global configuration files: `units.ron`, `time.ron` and the balance layer (`balance/`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Season, entry};

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

/// An integer world setting with its default and allowed range.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct IntSetting {
    pub default: u32,
    pub min: u32,
    pub max: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FloatSetting {
    pub default: f64,
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SleepAcceleration {
    /// Maximum time acceleration while sleeping.
    pub max_factor: f64,
    /// Seconds to ramp up to it (and back down on waking).
    pub ramp_s: f64,
}

/// `time.ron`: calendar defaults and the constants of the two time scales (v2 §4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeConfig {
    pub schema: u32,
    /// Real minutes per game day.
    pub day_length_min: IntSetting,
    pub days_per_season: IntSetting,
    pub axial_tilt_deg: FloatSetting,
    pub starting_season: Season,
    /// Seconds in a real day and days in a real (tropical) year.
    pub real_day_s: f64,
    pub real_year_days: f64,
    pub synodic_month_days: f64,
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
        /// After death: whom one may live on as (anyone, kin_group_region, kin_only, none), what
        /// is kept of what was known (theirs_only, head_start, keep_everything), and whether one
        /// may be born again.
        pub inhabit: String,
        pub after_death: String,
        pub born_again: bool,
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
        /// A first life is born (Addendum A), or appears grown at the place chosen.
        pub born: bool,
    }
}

fn yes() -> bool {
    true
}

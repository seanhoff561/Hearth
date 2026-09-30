//! Ecosystems (`ecosystems/`): where species live together, food-web defaults, succession and
//! fire regimes. Food webs themselves come from the animals' diets.

use serde::{Deserialize, Serialize};

use super::{Range, entry};
use crate::IdRef;

/// A stage of succession after disturbance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuccessionStage {
    pub name: String,
    pub plants: Vec<IdRef>,
    /// Years after disturbance when this stage dominates.
    pub years: Range,
}

entry! {
    /// An ecosystem type.
    pub struct Ecosystem in "ecosystems", schema 1, name name {
        pub name: String,
        /// World-generator biome names this ecosystem occupies.
        pub biomes: Vec<String>,
        /// Primary producers (plants, algae).
        pub producers: Vec<IdRef>,
        /// Animals typical of it.
        #[serde(default)]
        pub consumers: Vec<IdRef>,
        #[serde(default)]
        pub succession: Vec<SuccessionStage>,
        /// Typical years between fires.
        #[serde(default)]
        pub fire_return_years: Option<Range>,
    }
}

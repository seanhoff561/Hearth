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

/// The land an ecosystem's animals' densities describe (V2-10): its climate and its trees. A
/// species' density is that of the richest of the reference lands of the ecosystems it lives in;
/// an ecosystem without one is measured against the reference temperate wood.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ReferenceLand {
    /// Yearly mean and warmest month, °C; rain a year, mm.
    pub temp_c: f32,
    pub warm_c: f32,
    pub precip_mm: f32,
    /// The canopy's cover, 0–1 (none on open land).
    #[serde(default)]
    pub canopy: f32,
    /// Young regrowth in reach, 0–1.
    #[serde(default)]
    pub young: f32,
    /// Mast its trees bear in a year (nuts, cone seed), kg a km².
    #[serde(default)]
    pub mast_kg: f32,
    /// Fruit its trees bear in a year, kg a km².
    #[serde(default)]
    pub fruit_kg: f32,
    /// Its trees flower for the bees.
    #[serde(default)]
    pub flowers: bool,
    /// The realms whose animals live on it, each anchoring its forage to their needs as a land
    /// of its own (the African, the American and the Asian rainforest); the Palearctic when
    /// none are named.
    #[serde(default)]
    pub realms: Vec<String>,
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
        /// The land its animals' densities describe.
        #[serde(default)]
        pub reference: Option<ReferenceLand>,
    }
}

//! Eras (`eras/`) and hominin species (`hominins/`), v2 §8 and §17.

use super::{Range, entry};
use crate::IdRef;

entry! {
    /// A historical setting a world can be created in.
    pub struct Era in "eras", schema 1, name name {
        pub name: String,
        pub description: String,
        /// Order in the era selector.
        pub order: u8,
        /// Selectable now; unavailable eras are shown as "coming later".
        pub available: bool,
        /// Years before present the world represents; `None` = timeless present-day wild Earth.
        #[serde(default)]
        pub years_bp: Option<f64>,
        /// Include wild ancestors of domesticates even if extinct or rare today.
        #[serde(default)]
        pub wild_ancestors: bool,
        /// Sea level change in metres (scaled by the world's vertical scale).
        #[serde(default)]
        pub sea_level_offset_m: f32,
        /// Mean temperature offset.
        #[serde(default)]
        pub temperature_offset_c: f32,
        #[serde(default)]
        pub hominins: Vec<IdRef>,
        /// Knowledge every human of the era starts with.
        #[serde(default)]
        pub knowledge_baseline: Vec<IdRef>,
        /// Game systems active in this era.
        #[serde(default)]
        pub systems: Vec<String>,
    }
}

entry! {
    /// An early human species simulated as agents.
    pub struct Hominin in "hominins", schema 1, name name {
        pub name: String,
        #[serde(default)]
        pub scientific: Option<String>,
        pub height_m: Range,
        pub mass_kg: Range,
        pub group_size: Range,
        /// Ecosystems it lives in.
        pub habitat: Vec<IdRef>,
        /// Knowledge nodes it practises.
        pub knowledge: Vec<IdRef>,
        /// Behaviours in words the agent framework implements.
        pub behaviors: Vec<String>,
        #[serde(default)]
        pub first_appearance_ya: Option<f64>,
        #[serde(default)]
        pub extinction_ya: Option<f64>,
    }
}

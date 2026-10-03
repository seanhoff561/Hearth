//! Eras (`eras/`), v2 §17 (V2.1 §15.3 extends them into era profiles). The species of person
//! are in [`super::humans`].

use super::entry;
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
        /// The species of person living in it (`humans/species/`).
        #[serde(default)]
        pub species: Vec<IdRef>,
        /// Knowledge every human of the era starts with.
        #[serde(default)]
        pub knowledge_baseline: Vec<IdRef>,
        /// Game systems active in this era.
        #[serde(default)]
        pub systems: Vec<String>,
    }
}

//! Eras (`eras/`), v2 §17 (V2.1 §15.3 extends them into era profiles). The species of person
//! are in [`super::humans`].

use serde::{Deserialize, Serialize};

use super::{Range, Season, entry};
use crate::IdRef;

/// Where a band's camp goes in a season of its round (V2.1 §15.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Toward {
    /// By a river, a lake or the shore.
    Water,
    /// Up on the open high ground.
    Uplands,
    /// Down in the shelter of the woods.
    Shelter,
}

/// A season of a people's round: where its bands keep camp then.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoundStop {
    pub season: Season,
    pub toward: Toward,
}

/// A people's bands of one culture gathering in one camp for a while each year (V2.1 §15.3):
/// the season, how many days, and how far (km) bands come to it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Aggregation {
    pub season: Season,
    pub days: f32,
    pub reach_km: f32,
}

/// One of an era's peoples (V2.1 §15.3): a species and how its bands live in the era.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EraPeople {
    pub species: IdRef,
    /// How many live in one of its bands.
    pub band_size: Range,
    /// What its people may come to know in the era (the deep-time layer's repertoire, within
    /// its species' reach); none listed: what its species profile lists.
    #[serde(default)]
    pub repertoire: Vec<IdRef>,
    /// Instead of a list: every technique of the knowledge graph's eras up to this one (within
    /// its species' reach), each no earlier than the record dates it.
    #[serde(default)]
    pub reach: Option<u8>,
    /// A fire kept at camp, by those who know how to keep one.
    #[serde(default)]
    pub hearth: bool,
    /// Beds about the camp's fire: grass heaped, or furs where they know fur bedding.
    #[serde(default)]
    pub bedding: bool,
    /// Where its camps go season by season; none: where the food is.
    #[serde(default)]
    pub round: Vec<RoundStop>,
    /// Its bands' yearly gathering, if they have one.
    #[serde(default)]
    pub aggregation: Option<Aggregation>,
}

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
        /// Its peoples (V2.1 §15.3), whom the deep-time layer runs to its date (H8); none: Wild
        /// Earth's hominins and families as they are.
        #[serde(default)]
        pub peoples: Vec<EraPeople>,
        /// How its peoples live, as the era selector tells it.
        #[serde(default)]
        pub way_of_life: Option<String>,
    }
}

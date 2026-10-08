//! The knowledge graph (`knowledge/`), v2 §12.

use serde::{Deserialize, Serialize};

use super::entry;
use crate::IdRef;

/// How a node can be discovered (v2 §12.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Route {
    /// Doing related things with relevant materials.
    Experiment,
    /// Watching animals or natural phenomena.
    Observation,
    /// Combining two known techniques.
    Inference,
    /// Studying found evidence (tool scatters, remains).
    Evidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Discovery {
    pub route: Route,
    /// Event id the game emits (e.g. `strike:flint`, `watch:hyena_cracking_bones`,
    /// `heat:clay`) that grants insight.
    pub trigger: String,
    /// Insight granted per trigger (1.0 total discovers the node).
    pub insight: f32,
    /// Hunch shown in the journal before discovery.
    #[serde(default)]
    pub hint: Option<String>,
}

/// Something that must be at hand or reachable, besides prior knowledge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Need {
    Material(IdRef),
    Item(IdRef),
    Station(IdRef),
    /// A kind of place or phenomenon ("wildfire", "river", "chalk outcrop").
    Environment(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct History {
    /// Journal-style description of the real history.
    pub summary: String,
    /// Human-readable approximate date ("c. 3.3 million years ago", "c. 6000 BCE").
    pub date: String,
    /// Years before present (for sorting and eras).
    pub years_bp: f64,
}

entry! {
    /// A technique or concept.
    pub struct Knowledge in "knowledge", schema 1, name name {
        pub name: String,
        /// 0 Lower Paleolithic … 8 Modern.
        pub era: u8,
        /// Prerequisite knowledge.
        #[serde(default)]
        pub requires: Vec<IdRef>,
        #[serde(default)]
        pub needs: Vec<Need>,
        /// Processes it enables.
        #[serde(default)]
        pub enables: Vec<IdRef>,
        #[serde(default)]
        pub discovery: Vec<Discovery>,
        #[serde(default)]
        pub skill: Option<String>,
        pub history: History,
    }
}

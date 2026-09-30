//! Processes (`processes/`): transformations of materials by actions, tools and conditions
//! (v2 §11.3). The same data drives the player, hominins and future simulated humans.

use serde::{Deserialize, Serialize};

use super::material::MaterialFilter;
use super::{Duration, Range, entry};
use crate::IdRef;

/// What an input or output slot accepts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Match {
    /// A specific item (explicit or generated `form/material`).
    Item(IdRef),
    /// Any item of a form, optionally restricted by material.
    Form {
        form: IdRef,
        #[serde(default)]
        materials: Option<MaterialFilter>,
    },
    /// Bulk material by mass (clay, sand, water, meat).
    Material(IdRef),
    /// Any item carrying a tag.
    Tag(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Input {
    pub item: Match,
    /// Count for items, kilograms for materials.
    pub amount: f32,
    /// False for things that are used but not used up (an anvil stone).
    #[serde(default = "yes")]
    pub consumed: bool,
}

fn yes() -> bool {
    true
}

/// A tool requirement: something in hand with a property at or above `min`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolReq {
    pub property: String,
    pub min: f32,
}

/// Environmental or workstation condition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Condition {
    /// A heat source at least this hot.
    HeatAtLeastC(f32),
    /// Standing water or a water container at hand.
    Water,
    Dry,
    Daylight,
    /// Air temperature below (e.g. freezing meat).
    ColdBelowC(f32),
    /// Inside a structure with a roof.
    Sheltered,
    /// Next to a named feature of the world ("river", "cliff", "clay bank").
    Near(String),
}

/// Quality of the output: base plus skill and tool influences, clamped 0–1.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Quality {
    #[serde(default)]
    pub base: f32,
    /// Added at skill 1.
    #[serde(default)]
    pub from_skill: f32,
    /// Added in proportion to the input material's knapping/workability.
    #[serde(default)]
    pub from_material: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Output {
    pub item: Match,
    /// Count range for items, kg range for materials.
    pub amount: Range,
    #[serde(default = "certain")]
    pub chance: f32,
    #[serde(default)]
    pub quality: Quality,
}

fn certain() -> f32 {
    1.0
}

/// A way the process can go wrong.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Failure {
    /// Chance at skill 0; skill reduces it towards `min_chance`.
    pub chance: f32,
    #[serde(default)]
    pub min_chance: f32,
    /// What happens, in words for the journal.
    pub outcome: String,
    /// Inputs lost on failure.
    #[serde(default = "yes")]
    pub loses_inputs: bool,
    /// Injury risk description, if any.
    #[serde(default)]
    pub injury: Option<String>,
}

entry! {
    /// A process.
    pub struct Process in "processes", schema 1, name name {
        pub name: String,
        /// Verb phrase offered as a contextual action ("strike off a flake").
        pub action: String,
        pub inputs: Vec<Input>,
        #[serde(default)]
        pub tools: Vec<ToolReq>,
        #[serde(default)]
        pub station: Option<IdRef>,
        #[serde(default)]
        pub conditions: Vec<Condition>,
        pub duration: Duration,
        /// Knowledge needed to attempt it.
        #[serde(default)]
        pub knowledge: Option<IdRef>,
        /// Skill track practised by it.
        #[serde(default)]
        pub skill: Option<String>,
        pub outputs: Vec<Output>,
        #[serde(default)]
        pub byproducts: Vec<Output>,
        #[serde(default)]
        pub failures: Vec<Failure>,
    }
}

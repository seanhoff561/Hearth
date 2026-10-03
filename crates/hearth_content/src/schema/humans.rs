//! People (`humans/`), V2.1 §2: species profiles — the ranges every person's components take.

use serde::{Deserialize, Serialize};

use super::{Range, entry};
use crate::IdRef;

/// What a species does, of what the people's minds know how to do (v2 §8.2): its capabilities,
/// which H2's routines and methods are gated by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Behavior {
    /// Picks fruit, nuts, pods and seeds, turns over stones for grubs.
    Forage,
    /// Digs roots and tubers with a stick.
    DigTubers,
    /// Cracks hard nuts with a hammerstone on an anvil.
    CrackNuts,
    /// Fishes termites from their mound with a twig.
    FishTermites,
    /// Cuts meat from a carcass and cracks its bones for the marrow.
    Scavenge,
    /// Strikes sharp flakes from a stone.
    KnapFlakes,
    /// Bends branches into a nest in a tree to sleep.
    TreeNest,
    /// Calls out at a hunter.
    AlarmCall,
    /// Faces a threat together: shouting, brandishing sticks, throwing stones.
    MobThreat,
    /// Runs up into the trees.
    FleeToTrees,
    /// Comes to tolerate a calm, patient person.
    Habituate,
}

/// How a species speaks (V2.1 §10.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    /// Calls and gestures only.
    Calls,
    /// A simple, gesture-rich proto-language.
    ProtoLanguage,
    /// Full language.
    Full,
}

/// How much a species teaches (V2.1 §11.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Teaching {
    None,
    /// Tolerating the young watching up close, now and then slowing to let them see.
    Minimal,
    /// Demonstrating and correcting.
    Real,
}

/// What a species can do with fire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FireUse {
    None,
    /// Keeping and carrying a fire taken from a wildfire.
    Keeping,
    /// Making fire at will.
    Making,
}

/// The body plan a species' figures are drawn by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodyPlan {
    /// Long arms, short legs, a funnel chest: a climber that walks upright.
    Australopith,
    /// Tall and long-legged, narrow-hipped.
    Erectus,
    /// Short, broad and barrel-chested, short in the forearm and shin.
    Neanderthal,
    /// A modern human's.
    Modern,
}

/// A species' coat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Coat {
    /// Hair over the body, as an ape's.
    Hair,
    /// Bare skin.
    Bare,
}

/// Which sex leaves its birth group to live in another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Disperser {
    Females,
    Males,
    Both,
}

/// A range by sex.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BySex {
    pub female: Range,
    pub male: Range,
}

impl BySex {
    pub fn of(&self, female: bool) -> Range {
        if female { self.female } else { self.male }
    }
}

/// A species' body: grown sizes by sex, its plan and coat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeciesBody {
    pub height_m: BySex,
    pub mass_kg: BySex,
    pub plan: BodyPlan,
    pub coat: Coat,
    #[serde(default)]
    pub climbs: bool,
}

/// What a species' minds can do (V2.1 §6.1, §10–11).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cognition {
    pub language: Language,
    /// How many steps deep its plans go.
    pub planning_depth: u8,
    /// Symbols: ornament, pigment, marks that mean.
    pub symbolic: bool,
    pub teaching: Teaching,
    pub fire: FireUse,
    /// The highest era of the knowledge graph it can reach.
    pub era_ceiling: u8,
}

/// A species' life history (years unless said).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifeParams {
    pub gestation_days: f32,
    pub weaning_years: f32,
    /// Grown: a female's first birth, a male's full size.
    pub maturity_years: f32,
    /// The ages by which most of those who outlive childhood die, and the oldest few.
    pub adult_death_years: Range,
    pub birth_interval_years: f32,
}

/// How a species lives together, before culture (V2.1 §8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SocialDefaults {
    pub group_size: Range,
    pub disperses: Disperser,
}

entry! {
    /// A species of person (V2.1 §2): the ranges its persons' components take, what it knows and
    /// does, where and when it lived.
    pub struct Species in "humans/species", schema 1, name name {
        pub name: String,
        #[serde(default)]
        pub scientific: Option<String>,
        pub body: SpeciesBody,
        pub cognition: Cognition,
        pub life: LifeParams,
        pub social: SocialDefaults,
        /// Ecosystems it lives in.
        pub habitat: Vec<IdRef>,
        /// Knowledge nodes its groups practise (what a grown one knows).
        pub knowledge: Vec<IdRef>,
        /// What it does, of what the people's minds know how to do.
        pub behaviors: Vec<Behavior>,
        /// Its population in the ecological cells: the animal entry that says what it eats, how
        /// it lives and dies and how far it ranges (its bands are drawn out as persons near the
        /// player).
        #[serde(default)]
        pub population: Option<IdRef>,
        #[serde(default)]
        pub first_appearance_ya: Option<f64>,
        #[serde(default)]
        pub extinction_ya: Option<f64>,
    }
}

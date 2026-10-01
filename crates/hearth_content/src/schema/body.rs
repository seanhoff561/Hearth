//! Human physiology (`body/`) and clothing (`clothing/`), v2 §9–§10.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::material::MaterialFilter;
use super::{Duration, Range, entry};
use crate::IdRef;

/// Physiology parameters of an adult human (`body/human.ron`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BodyParams {
    pub schema: u32,
    pub height_m: Range,
    pub mass_kg: Range,
    /// Basal metabolic rate per kg of body mass.
    pub bmr_kcal_per_kg_day: f32,
    /// Metabolic equivalents of activities (1 MET = resting).
    pub activity_met: BTreeMap<String, f32>,
    pub stomach_capacity_l: f32,
    /// Water needed per day at rest in mild weather.
    pub water_l_per_day: f32,
    /// Fraction of body water lost that is fatal.
    pub fatal_water_loss: f32,
    pub core_temp_c: f32,
    pub hypothermia_c: f32,
    pub heat_stroke_c: f32,
    pub blood_l: f32,
    /// Load as a fraction of body mass carried comfortably / at most.
    pub comfortable_load: f32,
    pub max_load: f32,
    pub walk_m_s: f32,
    pub jog_m_s: f32,
    pub sprint_m_s: f32,
    pub swim_m_s: f32,
    /// Short-term stamina, in seconds of play (movement is not compressed by the day scale).
    #[serde(default)]
    pub stamina: StaminaParams,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub realism_source: Option<String>,
}

/// Short-term stamina: how long an all-out effort lasts and how long a full recovery takes, in
/// seconds of play.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StaminaParams {
    pub all_out_s: f32,
    pub recover_s: f32,
}

impl Default for StaminaParams {
    fn default() -> Self {
        Self {
            all_out_s: 15.0,
            recover_s: 30.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodyRegion {
    Head,
    Neck,
    Chest,
    Abdomen,
    Pelvis,
    UpperArm,
    LowerArm,
    Hand,
    UpperLeg,
    LowerLeg,
    Foot,
}

entry! {
    /// An injury type.
    pub struct Injury in "body/injuries", schema 1, name name {
        pub name: String,
        pub regions: Vec<BodyRegion>,
        #[serde(default)]
        pub bleeding_ml_per_min: Range,
        /// 0–1.
        pub pain: f32,
        /// Chance of infection if untreated.
        #[serde(default)]
        pub infection_risk: f32,
        pub heal: Duration,
        /// Effects in words the body model understands (slow_walk, no_two_hands...).
        #[serde(default)]
        pub effects: Vec<String>,
        #[serde(default)]
        pub treatments: Vec<String>,
    }
}

entry! {
    /// An illness.
    pub struct Illness in "body/illnesses", schema 1, name name {
        pub name: String,
        /// Causes in words (spoiled_food, bad_water, wound_infection, mosquito).
        pub causes: Vec<String>,
        pub onset: Duration,
        pub lasts: Duration,
        pub effects: Vec<String>,
        #[serde(default)]
        pub treatments: Vec<String>,
        /// Chance of death if untreated (0 for most).
        #[serde(default)]
        pub lethality: f32,
    }
}

/// Clothing layers, inside out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClothingLayer {
    Under,
    Main,
    Outer,
    Feet,
    Hands,
    Head,
    Belt,
    Back,
}

entry! {
    /// A garment or worn gear.
    pub struct Garment in "clothing", schema 1, name name {
        /// Display name pattern with `{material}`.
        pub name: String,
        pub layer: ClothingLayer,
        pub regions: Vec<BodyRegion>,
        pub materials: MaterialFilter,
        /// Insulation in clo.
        pub clo: f32,
        /// 0–1 wind and water resistance.
        pub wind: f32,
        pub water: f32,
        pub mass_kg: f32,
        #[serde(default)]
        pub capacity_l: f32,
        /// Attachment points it provides (belt loop, sheath, strap...).
        #[serde(default)]
        pub attachments: Vec<String>,
        /// 0–1 noise when moving.
        #[serde(default)]
        pub noise: f32,
        #[serde(default)]
        pub knowledge: Option<IdRef>,
        /// Inventory grid cells when carried rather than worn.
        #[serde(default = "garment_footprint")]
        pub footprint: (u8, u8),
    }
}

fn garment_footprint() -> (u8, u8) {
    (2, 2)
}

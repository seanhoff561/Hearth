//! Physical materials (`materials/`): everything else takes its behaviour from these.

use serde::{Deserialize, Serialize};

use super::{Color, Range, entry};

/// Broad material classes, used by filters ("any rock", "any wood").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaterialCategory {
    Rock,
    Mineral,
    Ore,
    Sediment,
    Soil,
    Clay,
    Wood,
    Bark,
    PlantFibre,
    PlantTissue,
    AnimalTissue,
    AnimalFibre,
    Bone,
    Metal,
    Ceramic,
    Glass,
    Fuel,
    Resin,
    Pigment,
    Salt,
    Water,
    Ice,
    Snow,
    Other,
}

/// How a material breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fracture {
    /// Smooth shell-like fracture: predictable sharp flakes (flint, obsidian, chert).
    Conchoidal,
    SubConchoidal,
    Uneven,
    Granular,
    Splintery,
    Fibrous,
    /// Along planes (slate, shale, mica).
    Cleavage,
    /// Deforms instead of breaking (metals, clay, hide).
    Plastic,
}

/// Visual pattern of the procedural texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Pattern {
    #[default]
    Plain,
    Grainy,
    Speckled,
    Crystalline,
    Layered,
    Banded,
    Veined,
    Glassy,
    Porous,
    Fibrous,
    WoodGrain,
    Bark,
    Clumpy,
    Powder,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Strength {
    #[serde(default)]
    pub compressive_mpa: Option<f32>,
    #[serde(default)]
    pub tensile_mpa: Option<f32>,
    /// Modulus of rupture.
    #[serde(default)]
    pub bending_mpa: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Thermal {
    #[serde(default)]
    pub conductivity_w_mk: Option<f32>,
    #[serde(default)]
    pub specific_heat_j_kgk: Option<f32>,
    #[serde(default)]
    pub melting_c: Option<f32>,
    /// Piloted ignition temperature for combustibles.
    #[serde(default)]
    pub ignition_c: Option<f32>,
    /// Temperature window for firing (clays) or smelting/reduction (ores).
    #[serde(default)]
    pub firing_c: Option<Range>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Appearance {
    pub color: Color,
    #[serde(default)]
    pub color2: Option<Color>,
    #[serde(default)]
    pub pattern: Pattern,
    /// 0 = glossy, 1 = matte.
    #[serde(default)]
    pub roughness: Option<f32>,
}

entry! {
    /// A physical material with real properties.
    pub struct Material in "materials", schema 1, name name {
        pub name: String,
        pub category: MaterialCategory,
        #[serde(default)]
        pub tags: Vec<String>,
        pub density_kg_m3: f32,
        #[serde(default)]
        pub hardness_mohs: Option<f32>,
        #[serde(default)]
        pub fracture: Option<Fracture>,
        /// 0–1: how predictably flakes come off when knapped (flint/obsidian ≈ 0.95).
        #[serde(default)]
        pub knapping: Option<f32>,
        #[serde(default)]
        pub strength: Strength,
        /// Young's modulus.
        #[serde(default)]
        pub elastic_modulus_gpa: Option<f32>,
        #[serde(default)]
        pub thermal: Thermal,
        /// Lower heating value.
        #[serde(default)]
        pub fuel_mj_kg: Option<f32>,
        /// 0–1: ease of shaping with period tools.
        #[serde(default)]
        pub workability: Option<f32>,
        /// 0–1: resistance to rot, insects and weathering.
        #[serde(default)]
        pub durability: Option<f32>,
        /// 0–1 pore fraction (soils, sediments, porous rock).
        #[serde(default)]
        pub porosity: Option<f32>,
        /// Food energy for edible materials.
        #[serde(default)]
        pub kcal_per_kg: Option<f32>,
        pub appearance: Appearance,
    }
}

/// Selects materials by category, tag or id (an empty filter matches nothing).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MaterialFilter {
    #[serde(default)]
    pub categories: Vec<MaterialCategory>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub ids: Vec<crate::IdRef>,
    /// Tags a matching material must *not* have.
    #[serde(default)]
    pub exclude_tags: Vec<String>,
}

impl MaterialFilter {
    pub fn matches(&self, id: &str, m: &Material) -> bool {
        if self.exclude_tags.iter().any(|t| m.tags.contains(t)) {
            return false;
        }
        self.categories.contains(&m.category)
            || self.tags.iter().any(|t| m.tags.contains(t))
            || self.ids.iter().any(|r| r.as_str() == id)
    }
}

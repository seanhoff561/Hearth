//! Physical materials (`materials/`): everything else takes its behaviour from these.

use serde::{Deserialize, Serialize};

use super::{Color, Range, entry};
use crate::id::IdRef;

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
    /// Twigs and sticks piled or laid, with gaps between (brush walls, bough roofs).
    Brush,
    /// Rods woven between upright stakes (wattle).
    Wattle,
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

/// What a medicinal plant does when taken (eaten, chewed, drunk), modestly (v2 §6.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Medicine {
    /// `analgesic`: eases pain.
    pub kind: String,
    /// 0–1 how much.
    pub strength: f32,
    /// For how long (hours of the day scale).
    pub hours: f32,
}

/// What a kilogram of a food gives: grams of protein, fat and carbohydrate, its water, the
/// fresh-food vitamins in it, and what eating it as it is risks.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Nutrition {
    #[serde(default)]
    pub protein_g: f32,
    #[serde(default)]
    pub fat_g: f32,
    #[serde(default)]
    pub carb_g: f32,
    /// Water (kg per kg).
    #[serde(default)]
    pub water: f32,
    /// Days of fresh-food vitamins a kilogram supplies (greens, berries, organ meat).
    #[serde(default)]
    pub fresh_days: f32,
    /// The cause of illness eating it risks (`raw_meat`), with the chance for a kilogram.
    #[serde(default)]
    pub risk: Option<(String, f32)>,
}

impl Nutrition {
    /// Food energy (kcal per kg) from the macronutrients.
    pub fn kcal(&self) -> f32 {
        4.0 * self.protein_g + 9.0 * self.fat_g + 4.0 * self.carb_g
    }
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
        /// How wide an opening it roofs over, unsupported, as natural ground (m): where its kind's
        /// rule is wrong (loess stands in cave dwellings where loam falls in).
        #[serde(default)]
        pub span_m: Option<f32>,
        /// 0–1 pore fraction (soils, sediments, porous rock).
        #[serde(default)]
        pub porosity: Option<f32>,
        /// Food energy for edible materials.
        #[serde(default)]
        pub kcal_per_kg: Option<f32>,
        /// What it gives as food (its energy agrees with `kcal_per_kg`).
        #[serde(default)]
        pub nutrition: Option<Nutrition>,
        /// How many days it keeps at 20 °C before it spoils; none: it does not spoil.
        #[serde(default)]
        pub keeps_days: Option<f32>,
        /// What it does as medicine, taken.
        #[serde(default)]
        pub medicine: Option<Medicine>,
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

/// `materials/reference.ron`: measured ranges per material family (Amendment Q §2.1). The lint
/// holds each family's members to its ranges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialReference {
    pub schema: u32,
    pub families: Vec<ReferenceFamily>,
}

/// A material family's measured ranges, their source and the materials held to them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceFamily {
    pub id: String,
    pub name: String,
    /// Diffuse albedo: the linear luminance of the base colour (visible band).
    pub albedo: (f32, f32),
    /// Perceptual roughness, 0 a mirror … 1 matte.
    pub roughness: (f32, f32),
    /// Index of refraction (the specular at normal incidence follows from it).
    pub ior: f32,
    /// How much light passes through, 0 opaque … 1.
    pub translucency: (f32, f32),
    /// Materials held to the ranges.
    #[serde(default)]
    pub members: Vec<IdRef>,
    pub source: String,
    /// Ranges estimated rather than measured.
    #[serde(default)]
    pub uncertain: bool,
}

impl Color {
    /// The colour's linear luminance (sRGB decoded, Rec. 709 weights): a surface's diffuse
    /// albedo when the colour is its base colour.
    pub fn albedo(&self) -> f32 {
        let lin = |v: u8| {
            let c = v as f32 / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let [r, g, b] = self.0;
        0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
    }
}

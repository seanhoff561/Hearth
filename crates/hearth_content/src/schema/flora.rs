//! Plant species (`flora/`), v2 §6.1.

use serde::{Deserialize, Serialize};

use super::{Color, Range, Season, entry};
use crate::IdRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GrowthForm {
    Tree,
    Shrub,
    Herb,
    Grass,
    Sedge,
    Vine,
    Succulent,
    Fern,
    Moss,
    Lichen,
    Fungus,
    Aquatic,
    Alga,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Lifecycle {
    Annual,
    Biennial,
    #[default]
    Perennial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Dispersal {
    #[default]
    Gravity,
    Wind,
    Animal,
    Water,
    Explosive,
    Spores,
}

/// Climate the species grows in. Unset limits are unconstrained.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ClimateEnvelope {
    /// Mean annual temperature.
    #[serde(default)]
    pub temp_c: Option<Range>,
    /// Coldest month mean it survives.
    #[serde(default)]
    pub min_coldest_month_c: Option<f32>,
    /// Warmest month mean it needs.
    #[serde(default)]
    pub min_warmest_month_c: Option<f32>,
    /// Annual precipitation.
    #[serde(default)]
    pub precip_mm: Option<Range>,
    /// Köppen classes or groups it typically occurs in.
    #[serde(default)]
    pub koppen: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SoilPreference {
    #[serde(default)]
    pub ph: Option<Range>,
    /// 0 = waterlogged … 1 = excessively drained.
    #[serde(default)]
    pub drainage: Option<Range>,
    #[serde(default)]
    pub min_fertility: Option<f32>,
    #[serde(default)]
    pub soils: Vec<IdRef>,
}

/// Timing of the yearly cycle as fractions of the year from the start of spring (0–1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Phenology {
    #[serde(default)]
    pub leaf_out: Option<f32>,
    #[serde(default)]
    pub flowering: Option<Range>,
    #[serde(default)]
    pub fruiting: Option<Range>,
    #[serde(default)]
    pub leaf_fall: Option<Range>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PartKind {
    Fruit,
    Nut,
    Seed,
    Root,
    Tuber,
    Bulb,
    Leaf,
    Shoot,
    Flower,
    Bark,
    InnerBark,
    Sap,
    Resin,
    Fibre,
    Wood,
    Cap,
    Whole,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Edibility {
    Edible,
    /// Edible only after the given process (e.g. leaching acorns, cooking tubers).
    AfterProcess(IdRef),
    /// Poisonous; severity 0–1.
    Toxic(f32),
    Inedible,
}

/// A usable part of a plant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlantPart {
    pub part: PartKind,
    /// Material the gathered part is made of.
    pub material: IdRef,
    #[serde(default)]
    pub seasons: Vec<Season>,
    pub edibility: Edibility,
    /// Typical yield per mature plant.
    #[serde(default)]
    pub yield_kg: Option<Range>,
    /// Uses in words (medicine, dye, tinder, cordage...).
    #[serde(default)]
    pub uses: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PlantAppearance {
    #[serde(default)]
    pub foliage: Option<Color>,
    #[serde(default)]
    pub autumn: Option<Color>,
    #[serde(default)]
    pub flower: Option<Color>,
    #[serde(default)]
    pub bark: Option<Color>,
    /// Crown shape keyword for the growth model (spreading, conical, columnar, umbrella...).
    #[serde(default)]
    pub crown: Option<String>,
}

entry! {
    /// A plant species.
    pub struct Plant in "flora", schema 1, name name {
        pub name: String,
        #[serde(default)]
        pub scientific: Option<String>,
        pub form: GrowthForm,
        #[serde(default)]
        pub deciduous: bool,
        pub climate: ClimateEnvelope,
        #[serde(default)]
        pub soil: SoilPreference,
        /// 0 = needs full sun (pioneer), 1 = grows in deep shade.
        #[serde(default)]
        pub shade_tolerance: f32,
        #[serde(default)]
        pub lifecycle: Lifecycle,
        #[serde(default)]
        pub phenology: Phenology,
        #[serde(default)]
        pub dispersal: Dispersal,
        /// Height growth when young, metres per year.
        #[serde(default)]
        pub growth_m_per_year: Option<f32>,
        pub max_height_m: f32,
        #[serde(default)]
        pub max_trunk_diameter_m: Option<f32>,
        #[serde(default)]
        pub lifespan_years: Option<f32>,
        /// Wood material for trees and woody shrubs.
        #[serde(default)]
        pub wood: Option<IdRef>,
        #[serde(default)]
        pub parts: Vec<PlantPart>,
        /// 0–1 flammability of the living plant.
        #[serde(default)]
        pub flammability: f32,
        #[serde(default)]
        pub appearance: PlantAppearance,
        /// Biogeographic realms (Palearctic, Nearctic, ...) it belongs to.
        #[serde(default)]
        pub realms: Vec<String>,
        /// Years before present; `None` = always present in the Wild Earth era.
        #[serde(default)]
        pub first_appearance_ya: Option<f64>,
        #[serde(default)]
        pub extinction_ya: Option<f64>,
        /// Wild ancestor this is a domesticated form of.
        #[serde(default)]
        pub domesticated_from: Option<IdRef>,
    }
}

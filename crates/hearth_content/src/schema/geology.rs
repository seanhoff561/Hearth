//! Geology (`geology/`): rock types, minerals, provinces with their stratigraphy, deposit
//! models and soils.

use serde::{Deserialize, Serialize};

use super::{Range, entry};
use crate::IdRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RockClass {
    IgneousIntrusive,
    IgneousExtrusive,
    Sedimentary,
    Metamorphic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Grain {
    Glassy,
    #[default]
    Fine,
    Medium,
    Coarse,
    /// Made of fragments (conglomerate, breccia).
    Clastic,
}

entry! {
    /// A rock type; physical properties come from its material.
    pub struct Rock in "geology/rocks", schema 1, name name {
        pub name: String,
        pub material: IdRef,
        pub class: RockClass,
        #[serde(default)]
        pub grain: Grain,
        /// Materials it weathers into (soil parent material, sand, clay).
        #[serde(default)]
        pub weathers_to: Vec<IdRef>,
    }
}

entry! {
    /// A mineral (ores, gems, pigments, salts).
    pub struct Mineral in "geology/minerals", schema 1, name name {
        pub name: String,
        pub material: IdRef,
        #[serde(default)]
        pub formula: Option<String>,
        /// Materials (usually metals) that can be won from it, as mass fractions.
        #[serde(default)]
        pub yields: Vec<Yield>,
        /// How it shows at the surface, in words a prospector would use.
        #[serde(default)]
        pub indicators: Vec<String>,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Yield {
    pub material: IdRef,
    pub fraction: f32,
}

/// The words `Formation::vegetation` may use (the world generator derives them from the biome
/// and the river network).
pub const SOIL_VEGETATION: [&str; 15] = [
    "broadleaf_forest",
    "mixed_forest",
    "boreal_forest",
    "montane_forest",
    "tropical_forest",
    "temperate_rainforest",
    "grassland",
    "steppe",
    "savanna",
    "scrub",
    "desert",
    "tundra",
    "alpine",
    "wetland",
    "floodplain",
];

/// The words `Province::conditions` may use (the world generator evaluates them per region).
pub const PROVINCE_CONDITIONS: [&str; 8] = [
    "arid", "humid", "warm", "cold", "coastal", "inland", "old", "young",
];

/// Tectonic setting a province comes from (matches the planet model's history).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TectonicSetting {
    Craton,
    FoldBelt,
    VolcanicArc,
    Hotspot,
    Rift,
    SedimentaryBasin,
    PassiveMargin,
    OceanFloor,
}

impl TectonicSetting {
    pub const ALL: [TectonicSetting; 8] = [
        TectonicSetting::Craton,
        TectonicSetting::FoldBelt,
        TectonicSetting::VolcanicArc,
        TectonicSetting::Hotspot,
        TectonicSetting::Rift,
        TectonicSetting::SedimentaryBasin,
        TectonicSetting::PassiveMargin,
        TectonicSetting::OceanFloor,
    ];
}

/// One layer of a stratigraphic sequence (listed top to bottom).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub rock: IdRef,
    pub thickness_m: Range,
    /// Probability that the layer is present at a given place.
    #[serde(default = "one")]
    pub presence: f32,
}

fn one() -> f32 {
    1.0
}

entry! {
    /// A geological province: rocks, structure and deposits of a region.
    pub struct Province in "geology/provinces", schema 1, name name {
        pub name: String,
        pub setting: TectonicSetting,
        /// Layers from the surface down, above the basement.
        pub sequence: Vec<Layer>,
        /// Crystalline basement under the sequence.
        pub basement: IdRef,
        /// Typical dip of the layers (0 = flat).
        #[serde(default)]
        pub dip_deg: Range,
        /// Intrusive bodies (granite plutons, dykes).
        #[serde(default)]
        pub intrusions: Vec<IdRef>,
        /// Layers folded into anticlines and synclines (collision belts); otherwise they lie
        /// flat or dip gently.
        #[serde(default)]
        pub folded: bool,
        /// Where among the regions of its setting this province is chosen: all must hold.
        /// Understood: "arid", "humid", "warm", "cold", "coastal", "inland", "old", "young".
        #[serde(default)]
        pub conditions: Vec<String>,
        /// Relative share among the provinces eligible for a region.
        #[serde(default = "one")]
        pub weight: f32,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DepositGeometry {
    Vein,
    Seam,
    /// Heavy grains concentrated in stream or beach gravels.
    Placer,
    Disseminated,
    Nodules,
    /// Surface crust or weathering cap (laterite, gossan, desert varnish).
    Crust,
    Pipe,
    /// Precipitated in wetlands (bog iron).
    Bog,
    Evaporite,
    /// Exposed outcrop or flow (obsidian).
    Flow,
}

/// A clue at the surface that a deposit lies below or upstream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Indicator {
    pub kind: IndicatorKind,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndicatorKind {
    Stain,
    Gossan,
    Float,
    PanConcentrate,
    Plant,
    Spring,
    Outcrop,
    Landform,
}

entry! {
    /// A deposit model: where and in what shape a resource occurs.
    pub struct Deposit in "geology/deposits", schema 1, name name {
        pub name: String,
        /// Mineral, rock or material id.
        pub resource: IdRef,
        pub geometry: DepositGeometry,
        pub provinces: Vec<IdRef>,
        #[serde(default)]
        pub host_rocks: Vec<IdRef>,
        /// Extra conditions in words the generator understands (e.g. "wetland", "arid").
        #[serde(default)]
        pub conditions: Vec<String>,
        /// Depth below the local surface.
        pub depth_m: Range,
        /// Typical extent of one body.
        pub size_m: Range,
        /// Mass fraction of the resource within the body.
        pub grade: Range,
        /// Bodies per square kilometre of suitable ground.
        pub frequency_per_km2: f32,
        #[serde(default)]
        pub indicators: Vec<Indicator>,
        /// First technological era that makes use of it.
        #[serde(default)]
        pub era: u8,
    }
}

/// One horizon of a soil profile, top down.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Horizon {
    /// O, A, E, B, C…
    pub name: String,
    pub material: IdRef,
    pub thickness_m: Range,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Fertility {
    /// 0–1 each (simplified N/P/K and organic matter).
    pub n: f32,
    pub p: f32,
    pub k: f32,
    pub organic: f32,
}

/// Where a soil forms.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Formation {
    /// Köppen classes (e.g. "Dfb") or groups ("D").
    #[serde(default)]
    pub climates: Vec<String>,
    #[serde(default)]
    pub parent_rocks: Vec<IdRef>,
    #[serde(default)]
    pub vegetation: Vec<String>,
    /// 0 = waterlogged, 1 = excessively drained.
    #[serde(default)]
    pub drainage: Option<Range>,
    #[serde(default)]
    pub max_slope_deg: Option<f32>,
}

entry! {
    /// A soil type with its profile.
    pub struct Soil in "geology/soils", schema 1, name name {
        pub name: String,
        pub horizons: Vec<Horizon>,
        pub fertility: Fertility,
        /// 0 = waterlogged, 1 = excessively drained.
        pub drainage: f32,
        pub ph: f32,
        /// 0–1 ease of digging and tilling.
        pub workability: f32,
        #[serde(default)]
        pub formation: Formation,
    }
}

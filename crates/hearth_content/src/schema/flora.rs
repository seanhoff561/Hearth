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
    /// A second bark colour (the birch's black marks, the pine's orange upper bark).
    #[serde(default)]
    pub bark2: Option<Color>,
    /// Crown shape in words (the growth model reads `TreeForm::crown`).
    #[serde(default)]
    pub crown: Option<String>,
    /// The colour of its fruit or berries.
    #[serde(default)]
    pub fruit: Option<Color>,
    /// How its block is drawn (herbs, shrubs, ferns and fungi).
    #[serde(default)]
    pub sprite: Option<Sprite>,
}

/// The drawings of the understory's plants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sprite {
    /// A leafy bush hung with fruit.
    Bush,
    /// A low, dense, twiggy shrub (heather, bilberry).
    Heath,
    /// Leaves in a low ring on the ground.
    Rosette,
    /// A tall stem crowned with an umbrella of small flowers (the carrot family).
    Umbel,
    /// A tall stem of flowers up its length (foxglove).
    Spike,
    Fern,
    /// A cap on a stem.
    Mushroom,
    /// Strap-shaped leaves in a clump (wild garlic).
    Clump,
    /// Low runners with leaves and fruit (strawberry).
    Creeper,
    /// Grassy leaves in a tuft, its flowers or seed heads on stalks above (sedges, cottongrass).
    Tuft,
    /// A mat over the ground, seen from above (mosses, lichens): its block is a carpet.
    Carpet,
    /// Spined green paddles one on another (a prickly pear).
    Cactus,
}

/// Ground the understory's plants need.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ground {
    Wet,
    /// Rich in nitrogen (old camps, middens, woodland on good soil).
    Rich,
    /// Poor and acid (heath, pine and birch woods on sand).
    Acid,
    /// On lime.
    Lime,
    /// Broken ground: clearings, burns, banks.
    Disturbed,
}

/// How a herb, shrub, fern or fungus of the understory is drawn and where it grows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Understory {
    /// The block it is drawn as.
    pub block: IdRef,
    /// How common it is where it fits best, 0–1 (a carpet of wild garlic 1, a lone foxglove
    /// a few hundredths).
    pub abundance: f32,
    /// The light it grows in: 0 deep shade … 1 full sun.
    pub light: Range,
    #[serde(default)]
    pub ground: Vec<Ground>,
    /// It grows in patches this wide (m); 0 scattered.
    #[serde(default)]
    pub patch_m: f32,
}

/// Crown shapes of the growth model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Crown {
    /// Broad, with low limbs forking wide (oak, chestnut, walnut).
    Spreading,
    /// Egg-shaped (beech, lime, ash, cherry).
    Ovoid,
    /// About as wide as tall (field maple, apple, hawthorn).
    Rounded,
    /// A spire, widest at the bottom (spruce, alder).
    Conical,
    /// Narrow and tall (aspen).
    Columnar,
    /// Branches hanging (white willow, silver birch's twigs).
    Weeping,
    /// Flat-topped on a long clear stem (old pine).
    Umbrella,
    /// Several stems from the ground (hazel, elder, osier).
    MultiStemmed,
    /// One stem, unbranched, crowned by a rosette of great fronds (palms; `droop` how far they
    /// arch down, `crown_width` their spread).
    Palm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LeafKind {
    #[default]
    Broad,
    Needle,
    Scale,
}

/// The colour deciduous foliage turns in autumn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AutumnColor {
    #[default]
    Brown,
    Yellow,
    Red,
}

/// Bark patterns for the procedural textures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BarkPattern {
    /// Deep vertical furrows (oak, chestnut in spirals).
    #[default]
    Furrowed,
    /// Smooth and grey (beech, hornbeam).
    Smooth,
    /// Papery bands peeling (birch).
    Peeling,
    /// Plates and scales (spruce, pine, apple).
    Scaly,
    /// Long shallow fissures (willow, lime).
    Fissured,
    /// Horizontal bands of lenticels (cherry).
    Banded,
    /// Interlacing ridges (ash, elm, walnut).
    Ridged,
    /// Thin flakes (yew).
    Flaky,
}

/// How a tree or woody shrub grows: the parameters of the growth model (v2 §6.2,
/// `docs/design/flora.md`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TreeForm {
    pub crown: Crown,
    /// Live crown length ÷ height, mature.
    pub crown_ratio: f32,
    /// Crown width ÷ height, mature.
    pub crown_width: f32,
    /// 0 = forks into limbs (a broad decurrent crown); 1 = one straight leader.
    pub apical_dominance: f32,
    /// Branch angle from the stem, degrees.
    pub branch_angle_deg: f32,
    /// 0–1 how much branches hang.
    #[serde(default)]
    pub droop: f32,
    /// Branches in whorls (conifers).
    #[serde(default)]
    pub whorled: bool,
    /// Stems from the ground.
    #[serde(default = "one_stem")]
    pub stems: u8,
    /// 0–1 how the foot spreads into roots.
    #[serde(default)]
    pub root_flare: f32,
    #[serde(default)]
    pub leaf: LeafKind,
    /// 0–1 how thickly foliage fills its clusters.
    pub foliage_density: f32,
    #[serde(default)]
    pub autumn: AutumnColor,
    #[serde(default)]
    pub bark: BarkPattern,
    /// Blocks: the trunk, the limbs, the foliage.
    pub log: IdRef,
    pub branch: IdRef,
    pub leaves: IdRef,
}

fn one_stem() -> u8 {
    1
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
        /// How a tree or woody shrub grows (the growth model).
        #[serde(default)]
        pub tree: Option<TreeForm>,
        /// How a plant of the understory is drawn and where it grows.
        #[serde(default)]
        pub understory: Option<Understory>,
        /// Plants of one group (umbellifers, white mushrooms, dark berries) look alike until
        /// the player has learned to tell them apart (`knowledge` `telling_<group>`).
        #[serde(default)]
        pub look_alike: Option<String>,
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

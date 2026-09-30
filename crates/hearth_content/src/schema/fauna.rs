//! Animal species (`fauna/`), v2 §7.

use serde::{Deserialize, Serialize};

use super::{Range, Season, entry};
use crate::IdRef;

/// Shared skeleton families (v2 §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodyPlan {
    Ungulate,
    Carnivore,
    Rodent,
    Lagomorph,
    Bear,
    Elephant,
    Giraffe,
    Hippo,
    Primate,
    BirdPerching,
    BirdGround,
    Waterfowl,
    Raptor,
    Seabird,
    Penguin,
    FishFusiform,
    FishFlat,
    Eel,
    Seal,
    Cetacean,
    Lizard,
    Crocodilian,
    Turtle,
    Snake,
    Amphibian,
    Crab,
    Insect,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Speeds {
    pub walk_m_s: f32,
    #[serde(default)]
    pub run_m_s: Option<f32>,
    #[serde(default)]
    pub swim_m_s: Option<f32>,
    #[serde(default)]
    pub fly_m_s: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Senses {
    /// Distance at which it notices a moving human in daylight.
    pub vision_m: f32,
    pub hearing_m: f32,
    /// Downwind scent detection distance.
    pub smell_m: f32,
    /// 0–1 relative night vision.
    #[serde(default)]
    pub night_vision: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DietKind {
    Grazer,
    Browser,
    MixedFeeder,
    Frugivore,
    Granivore,
    Carnivore,
    Omnivore,
    Insectivore,
    Piscivore,
    Scavenger,
    FilterFeeder,
}

/// Something eaten: a plant species, an animal species or a material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Food {
    pub food: IdRef,
    /// Relative preference 0–1.
    #[serde(default = "one")]
    pub preference: f32,
}

fn one() -> f32 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diet {
    pub kind: DietKind,
    pub foods: Vec<Food>,
    /// Food needed per day (dry matter for herbivores, meat for carnivores).
    #[serde(default)]
    pub daily_food_kg: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Social {
    Solitary,
    Pair,
    Family,
    Herd { size: Range },
    Pack { size: Range },
    Pride { size: Range },
    Flock { size: Range },
    School { size: Range },
    Colony { size: Range },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Activity {
    Diurnal,
    Nocturnal,
    Crepuscular,
    Cathemeral,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SeasonalBehavior {
    Migration { seasons: Vec<Season> },
    Hibernation,
    Rut { season: Season },
    Birthing { season: Season },
    WinterCoat,
    Spawning { season: Season },
}

/// How and why it can be dangerous to people.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Danger {
    /// Will hunt a human under the right conditions.
    #[serde(default)]
    pub predatory: bool,
    /// Defends itself, young or food with force.
    #[serde(default)]
    pub defensive: bool,
    #[serde(default)]
    pub venomous: bool,
    /// 0–1 baseline aggression (scaled by the Predator Behavior setting).
    #[serde(default)]
    pub aggression: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Domestication {
    /// Name of the domestic form (dog, sheep, cattle...).
    pub domestic_form: String,
    /// Knowledge era in which it becomes possible.
    pub era: u8,
    /// 0 = easy, 1 = only tameable, never domesticated.
    pub difficulty: f32,
}

entry! {
    /// An animal species.
    pub struct Animal in "fauna", schema 1, name name {
        pub name: String,
        #[serde(default)]
        pub scientific: Option<String>,
        pub body_plan: BodyPlan,
        /// Adult mass range.
        pub mass_kg: Range,
        pub length_m: f32,
        #[serde(default)]
        pub shoulder_height_m: Option<f32>,
        pub speed: Speeds,
        pub senses: Senses,
        pub diet: Diet,
        pub social: Social,
        pub activity: Activity,
        /// Ecosystems it lives in.
        pub habitat: Vec<IdRef>,
        #[serde(default)]
        pub seasonal: Vec<SeasonalBehavior>,
        #[serde(default)]
        pub danger: Danger,
        #[serde(default)]
        pub domestication: Option<Domestication>,
        /// Yearly population growth rate at low density (r).
        #[serde(default)]
        pub growth_rate: Option<f32>,
        /// Individuals per km² in good habitat.
        #[serde(default)]
        pub density_per_km2: Option<f32>,
        #[serde(default)]
        pub realms: Vec<String>,
        #[serde(default)]
        pub first_appearance_ya: Option<f64>,
        #[serde(default)]
        pub extinction_ya: Option<f64>,
    }
}

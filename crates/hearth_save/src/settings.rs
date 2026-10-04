//! World settings chosen at world creation (v2 §16.1) and persisted in `level.json`.

use std::collections::BTreeMap;

use hearth_content::schema::Season;
use hearth_worldgen::WorldGenSettings;
use serde::{Deserialize, Serialize};

/// How aggressive predators are (scales motivation, never the mechanics; v2 §7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredatorBehavior {
    #[default]
    Authentic,
    Wild,
    Tranquil,
}

/// How knowledge is gained (v2 §12.3). `Open` makes everything known (sandbox, testing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeMode {
    #[default]
    Discovery,
    Guided,
    Open,
}

/// Where hominin groups live (v2 §8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HomininRange {
    #[default]
    AllSuitableHabitat,
    SingleCradleRegion,
}

/// What happens when the character dies (v2 §9.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeathRules {
    #[default]
    Legacy,
    Permadeath,
    Hardy,
}

/// Who a player may live on as after their death (Addendum B §2.3): the world's (and server's)
/// scope for inhabiting another living person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InhabitScope {
    /// Any eligible living person (single-player's default).
    #[default]
    Anyone,
    /// The dead one's kin, their group, or anyone of the region where they died (multiplayer's).
    KinGroupRegion,
    /// Living kin only.
    KinOnly,
    /// No one: being born again, restarting and watching remain.
    None,
}

/// Realism preset (a `balance/presets` id) plus Custom overrides per balance key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Realism {
    pub preset: String,
    #[serde(default)]
    pub overrides: BTreeMap<String, f32>,
}

impl Default for Realism {
    fn default() -> Self {
        Self {
            preset: "authentic".into(),
            overrides: BTreeMap::new(),
        }
    }
}

/// Life & time settings (v2 §16.1 step 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LifeSettings {
    /// Real minutes per game day.
    pub day_length_min: u32,
    pub days_per_season: u32,
    pub starting_season: Season,
    pub axial_tilt_deg: f64,
    #[serde(default)]
    pub realism: Realism,
    #[serde(default)]
    pub predator_behavior: PredatorBehavior,
    #[serde(default)]
    pub knowledge_mode: KnowledgeMode,
    #[serde(default)]
    pub hominin_range: HomininRange,
    #[serde(default)]
    pub death_rules: DeathRules,
    /// Reveal the whole map instead of only what the character has seen.
    #[serde(default)]
    pub full_map_knowledge: bool,
    /// Who a player may live on as after death (Addendum B §2.3).
    #[serde(default)]
    pub inhabit: InhabitScope,
    /// The players the world expects (Addendum B §3.5): Wild Earth holds as many times its
    /// wandering families, so that each may be born into one.
    #[serde(default = "one")]
    pub players: u8,
}

fn one() -> u8 {
    1
}

impl Default for LifeSettings {
    fn default() -> Self {
        Self {
            day_length_min: 48,
            days_per_season: 8,
            starting_season: Season::Spring,
            axial_tilt_deg: 23.44,
            realism: Realism::default(),
            predator_behavior: PredatorBehavior::default(),
            knowledge_mode: KnowledgeMode::default(),
            hominin_range: HomininRange::default(),
            death_rules: DeathRules::default(),
            full_map_knowledge: false,
            inhabit: InhabitScope::default(),
            players: 1,
        }
    }
}

impl LifeSettings {
    /// Defaults taken from the content's `time.ron`.
    pub fn from_content(time: &hearth_content::schema::config::TimeConfig) -> Self {
        Self {
            day_length_min: time.day_length_min.default,
            days_per_season: time.days_per_season.default,
            starting_season: time.starting_season,
            axial_tilt_deg: time.axial_tilt_deg.default,
            ..Self::default()
        }
    }
}

/// Everything decided on the Create World screens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldSettings {
    pub planet: WorldGenSettings,
    pub life: LifeSettings,
    /// Era id (`eras/`).
    pub era: String,
}

impl WorldSettings {
    pub fn new(planet: WorldGenSettings) -> Self {
        Self {
            planet,
            life: LifeSettings::default(),
            era: "hearth:wild_earth".into(),
        }
    }
}

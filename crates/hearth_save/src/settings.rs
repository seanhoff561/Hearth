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

/// What a player keeps of what they knew when they begin a new life (Amendment E §6.6; the
/// mode sets it: Realistic their own only, Easy everything).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AfterDeath {
    /// Exactly what the new person knows; the old journal readable as notes from a past life.
    #[default]
    TheirsOnly,
    /// What earlier lives discovered comes back quickly: legends, learned again by doing.
    HeadStart,
    /// What earlier lives discovered is known, at a beginner's skill.
    KeepEverything,
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
    /// What is kept of what was known when a new life begins.
    #[serde(default)]
    pub after_death: AfterDeath,
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
            after_death: AfterDeath::default(),
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
    /// Where the first life is born (world x, z), chosen on the globe (Amendment P §4.3);
    /// `None`: where the world finds a place. The world's calendar starts by it.
    #[serde(default)]
    pub birthplace: Option<[f64; 2]>,
    /// The game mode (`balance/modes.ron`, Amendment P §2) the world's rules were set by; `None`:
    /// a world from before the modes, or one set rule by rule (tests and tools).
    #[serde(default)]
    pub mode: Option<String>,
    /// The world was ever played in Creative (shown in the world list; it cannot be undone).
    #[serde(default)]
    pub played_in_creative: bool,
}

impl WorldSettings {
    pub fn new(planet: WorldGenSettings) -> Self {
        Self {
            planet,
            life: LifeSettings::default(),
            era: "hearth:wild_earth".into(),
            birthplace: None,
            mode: None,
            played_in_creative: false,
        }
    }
}

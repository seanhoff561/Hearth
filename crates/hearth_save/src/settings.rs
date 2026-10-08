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

/// What a player keeps of what they knew when they live on as another or are born again
/// (Addendum B §2.4, the world setting "Knowledge after death").
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

/// The ways of death as one choice (Addendum B §2.5, D167): v2's three rules, retired in H9, live
/// on as presets of the settings that replaced them, beside the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathPreset {
    /// Anyone may be lived on as, knowing only what they know; being born again allowed.
    Authentic,
    /// v2's Legacy: as Authentic, with a head start on what earlier lives discovered.
    Legacy,
    /// v2's Hardy: what earlier lives discovered is kept.
    Hardy,
    /// No one may be lived on as and no one born again: one may watch, begin again, or end.
    Permadeath,
}

impl DeathPreset {
    pub const ALL: [Self; 4] = [Self::Authentic, Self::Legacy, Self::Hardy, Self::Permadeath];

    /// The settings the preset stands for: inhabiting's scope, what is kept, being born again.
    pub fn settings(self) -> (InhabitScope, AfterDeath, bool) {
        match self {
            Self::Authentic => (InhabitScope::Anyone, AfterDeath::TheirsOnly, true),
            Self::Legacy => (InhabitScope::Anyone, AfterDeath::HeadStart, true),
            Self::Hardy => (InhabitScope::Anyone, AfterDeath::KeepEverything, true),
            Self::Permadeath => (InhabitScope::None, AfterDeath::TheirsOnly, false),
        }
    }

    /// The preset these settings are, if any.
    pub fn of(inhabit: InhabitScope, after: AfterDeath, born_again: bool) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|p| p.settings() == (inhabit, after, born_again))
    }
}

/// What death means in a world (Addendum B §2.3–2.5): whom one may live on as, what is kept of
/// what was known, whether one may be born again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Death {
    pub inhabit: InhabitScope,
    pub after: AfterDeath,
    pub born_again: bool,
}

impl Default for Death {
    fn default() -> Self {
        Self::preset(DeathPreset::Authentic)
    }
}

impl Death {
    pub fn preset(p: DeathPreset) -> Self {
        let (inhabit, after, born_again) = p.settings();
        Self {
            inhabit,
            after,
            born_again,
        }
    }

    /// The preset these settings are, if any.
    pub fn is(&self) -> Option<DeathPreset> {
        DeathPreset::of(self.inhabit, self.after, self.born_again)
    }

    /// Death ends the world: no one to live on as, no being born again.
    pub fn ends_the_world(&self) -> bool {
        !self.born_again && self.inhabit == InhabitScope::None
    }
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
    /// What is kept of what was known after death (Addendum B §2.4).
    #[serde(default)]
    pub after_death: AfterDeath,
    /// Whether a dead player may be born again (Permadeath: not).
    #[serde(default = "yes")]
    pub born_again: bool,
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

fn yes() -> bool {
    true
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
            after_death: AfterDeath::default(),
            born_again: true,
            full_map_knowledge: false,
            inhabit: InhabitScope::default(),
            players: 1,
        }
    }
}

impl LifeSettings {
    /// What death means in this world.
    pub fn death(&self) -> Death {
        Death {
            inhabit: self.inhabit,
            after: self.after_death,
            born_again: self.born_again,
        }
    }

    pub fn set_death(&mut self, d: Death) {
        self.inhabit = d.inhabit;
        self.after_death = d.after;
        self.born_again = d.born_again;
    }

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
}

impl WorldSettings {
    pub fn new(planet: WorldGenSettings) -> Self {
        Self {
            planet,
            life: LifeSettings::default(),
            era: "hearth:wild_earth".into(),
            birthplace: None,
        }
    }
}

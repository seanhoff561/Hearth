//! `level.json`: the world's metadata, versioned by `format`.

use serde::{Deserialize, Serialize};

use crate::settings::WorldSettings;

/// Save format written by this build. Bump it (and add a step to `migrate`) whenever any
/// persisted structure changes.
///
/// * 1 — v1 of the game (never released with saves); refused.
/// * 2 — first v2 format: flat seed/planet fields, `time.ticks`.
/// * 3 — settings grouped into `settings.{planet,life,era}`, `clock` replaces `time`.
pub const FORMAT: u32 = 3;

/// Oldest format this build can migrate from.
pub const OLDEST_SUPPORTED: u32 = 2;

/// World time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Clock {
    /// Simulation ticks since the world was created (20 per second of play).
    pub ticks: u64,
}

/// The persisted world metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldMeta {
    pub format: u32,
    pub name: String,
    /// Game version that last wrote the world.
    pub game_version: String,
    pub created_unix: u64,
    pub last_played_unix: u64,
    pub settings: WorldSettings,
    pub clock: Clock,
    /// Block state names indexed by the ids used in the region files.
    pub block_states: Vec<String>,
    /// Data packs enabled for the world, in load order (the base pack is implicit).
    #[serde(default)]
    pub data_packs: Vec<String>,
}

impl WorldMeta {
    pub fn new(name: &str, settings: WorldSettings, block_states: Vec<String>) -> Self {
        let now = unix_now();
        Self {
            format: FORMAT,
            name: name.to_owned(),
            game_version: hearth_core::GAME_VERSION.to_owned(),
            created_unix: now,
            last_played_unix: now,
            settings,
            clock: Clock::default(),
            block_states,
            data_packs: Vec::new(),
        }
    }
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

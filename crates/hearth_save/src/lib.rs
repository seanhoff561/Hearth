//! World saves (v1 M4 saves, extended by v2 §3.5): a world directory with versioned metadata
//! (`level.json`), the block-state palette of the save, region files of cubes, and migrations
//! from every supported older format.
//!
//! ```text
//! saves/<world>/
//!   level.json      WorldMeta (format, settings, clock, block state palette, packs)
//!   region/         r.<x>.<y>.<z>.hrg — 8×8×8 cubes each
//! ```

pub mod meta;
pub mod migrate;
pub mod region;
pub mod settings;
pub mod states;

use std::path::{Path, PathBuf};

pub use meta::{Clock, FORMAT, WorldMeta};
pub use migrate::MigrationReport;
pub use region::RegionStore;
pub use settings::{
    AfterDeath, KnowledgeMode, LifeSettings, PredatorBehavior, Realism, Start, WorldSettings,
};

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Incompatible(String),
    #[error("corrupt save: {0}")]
    Corrupt(String),
    #[error("content error: {0}")]
    Content(String),
    #[error("a world named {0:?} already exists")]
    Exists(String),
}

/// An opened world directory.
#[derive(Debug, Clone)]
pub struct WorldDir {
    pub root: PathBuf,
}

impl WorldDir {
    pub fn level_file(&self) -> PathBuf {
        self.root.join("level.json")
    }

    pub fn region_dir(&self) -> PathBuf {
        self.root.join("region")
    }

    pub fn regions(&self) -> RegionStore {
        RegionStore::new(self.region_dir())
    }

    /// Creates a new world directory under `saves` (a folder name derived from the world name).
    pub fn create(saves: &Path, meta: &WorldMeta) -> Result<WorldDir, SaveError> {
        let folder: String = meta
            .name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let root = saves.join(if folder.is_empty() {
            "world".into()
        } else {
            folder
        });
        if root.join("level.json").exists() {
            return Err(SaveError::Exists(meta.name.clone()));
        }
        std::fs::create_dir_all(root.join("region"))?;
        let dir = WorldDir { root };
        dir.save_meta(meta)?;
        Ok(dir)
    }

    /// Opens a world, migrating its metadata to the current format in memory (it is written
    /// in the new format on the next save). Refuses incompatible formats with a clear message.
    pub fn open(root: &Path) -> Result<(WorldDir, WorldMeta, MigrationReport), SaveError> {
        let dir = WorldDir {
            root: root.to_path_buf(),
        };
        let text = std::fs::read_to_string(dir.level_file())?;
        let mut value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| SaveError::Corrupt(format!("level.json: {e}")))?;
        let report = migrate::migrate(&mut value)?;
        let meta: WorldMeta = serde_json::from_value(value)
            .map_err(|e| SaveError::Corrupt(format!("level.json: {e}")))?;
        if report.migrated() {
            log::info!(
                "world {:?} migrated from save format {} to {}: {}",
                meta.name,
                report.from,
                report.to,
                report.steps.join("; ")
            );
        }
        Ok((dir, meta, report))
    }

    /// Writes a JSON file of the world (`player.json`...) atomically, keeping a backup of the
    /// old one.
    pub fn write_json<T: serde::Serialize>(&self, name: &str, value: &T) -> Result<(), SaveError> {
        let path = self.root.join(name);
        let tmp = path.with_extension("json.tmp");
        let text =
            serde_json::to_string_pretty(value).map_err(|e| SaveError::Corrupt(e.to_string()))?;
        std::fs::write(&tmp, text)?;
        if path.exists() {
            let _ = std::fs::copy(&path, path.with_extension("json.bak"));
        }
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Reads a JSON file of the world, `None` when it does not exist.
    pub fn read_json<T: serde::de::DeserializeOwned>(
        &self,
        name: &str,
    ) -> Result<Option<T>, SaveError> {
        let path = self.root.join(name);
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)?;
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| SaveError::Corrupt(format!("{name}: {e}")))
    }

    /// Writes `level.json` atomically (temp file + rename), keeping a backup of the old one.
    pub fn save_meta(&self, meta: &WorldMeta) -> Result<(), SaveError> {
        let path = self.level_file();
        let tmp = path.with_extension("json.tmp");
        let text =
            serde_json::to_string_pretty(meta).map_err(|e| SaveError::Corrupt(e.to_string()))?;
        std::fs::write(&tmp, text)?;
        if path.exists() {
            let _ = std::fs::copy(&path, path.with_extension("json.bak"));
        }
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}

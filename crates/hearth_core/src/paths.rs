//! Locations of the game directory and its well-known subfolders.

use std::path::{Path, PathBuf};

/// Well-known folders inside the game directory. Everything the game writes lives here.
#[derive(Debug, Clone)]
pub struct GameDirs {
    pub root: PathBuf,
}

impl GameDirs {
    /// Uses `root` as the game directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Resolves the game directory: an explicit override, then the `HEARTH_GAME_DIR`
    /// environment variable, then a `portable` marker file next to the executable (portable
    /// installs keep everything beside the binary), then the platform data directory.
    pub fn resolve(explicit: Option<&Path>, platform_data_dir: Option<PathBuf>) -> Self {
        if let Some(p) = explicit {
            return Self::new(p);
        }
        if let Some(p) = std::env::var_os("HEARTH_GAME_DIR") {
            return Self::new(PathBuf::from(p));
        }
        if let Some(exe_dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            && exe_dir.join("portable").exists()
        {
            return Self::new(exe_dir);
        }
        match platform_data_dir {
            Some(p) => Self::new(p),
            None => Self::new(std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
        }
    }

    pub fn options_file(&self) -> PathBuf {
        self.root.join("options.toml")
    }
    pub fn saves(&self) -> PathBuf {
        self.root.join("saves")
    }
    pub fn screenshots(&self) -> PathBuf {
        self.root.join("screenshots")
    }
    pub fn resource_packs(&self) -> PathBuf {
        self.root.join("resourcepacks")
    }
    pub fn data_packs(&self) -> PathBuf {
        self.root.join("datapacks")
    }
    pub fn mods(&self) -> PathBuf {
        self.root.join("mods")
    }
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
    pub fn crash_reports(&self) -> PathBuf {
        self.root.join("crash-reports")
    }
    /// Regenerable data (planet analysis grids, shader caches); safe to delete.
    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }

    /// Creates the root and the standard subfolders if they don't exist yet.
    pub fn ensure_created(&self) -> std::io::Result<()> {
        for dir in [
            self.root.clone(),
            self.saves(),
            self.screenshots(),
            self.resource_packs(),
            self.mods(),
            self.logs(),
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}

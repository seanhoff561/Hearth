//! Core building blocks shared by every Hearth crate: the game identity constants, resource
//! locations (`namespace:path` ids), registries, the game's folders and the persisted player
//! options.
//!
//! This crate deliberately has no dependency on rendering, windowing or world code so that
//! everything else can build on it.

pub mod disk_cache;
pub mod hardware;
pub mod jobs;
pub mod memory;
pub mod options;
pub mod paths;
pub mod prof;
pub mod registry;
pub mod resource;

pub use registry::{RawId, Registry, RegistryError};
pub use resource::{ResourceLocation, ResourceLocationError};

/// Human-readable game name shown in window titles and menus. The project codename lives in
/// exactly this one place so the game can be renamed by editing it.
pub const GAME_NAME: &str = "Hearth";

/// Namespace of the base game's content ("vanilla is a mod": the base game registers its
/// content through the same API as mods, under this namespace).
pub const GAME_NAMESPACE: &str = "hearth";

/// Semantic version of the game build.
pub const GAME_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Simulation rate of the server: the one tick rate every crate reckons in.
pub const TICKS_PER_SECOND: u32 = 20;

/// Window title for the current build.
pub fn window_title() -> String {
    format!("{GAME_NAME} {GAME_VERSION}")
}

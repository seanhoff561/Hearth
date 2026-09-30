//! Core building blocks shared by every Hearth crate: the game identity constants, resource
//! locations (`namespace:path` ids), registries, events, the tick scheduler and the persisted
//! player options.
//!
//! This crate deliberately has no dependency on rendering, windowing or world code so that
//! everything else can build on it.

pub mod events;
pub mod options;
pub mod paths;
pub mod registry;
pub mod resource;
pub mod tick;

pub use events::EventQueue;
pub use registry::{IdMapping, RawId, Registry, RegistryError};
pub use resource::{ResourceLocation, ResourceLocationError};
pub use tick::{FixedTimestep, ScheduledTicks};

/// Human-readable game name shown in window titles and menus. The project codename lives in
/// exactly this one place so the game can be renamed by editing it.
pub const GAME_NAME: &str = "Hearth";

/// Namespace of the base game's content ("vanilla is a mod": the base game registers its
/// content through the same API as mods, under this namespace).
pub const GAME_NAMESPACE: &str = "hearth";

/// Semantic version of the game build.
pub const GAME_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Simulation rate of the integrated server.
pub const TICKS_PER_SECOND: u32 = 20;

/// Duration of one simulation tick in seconds.
pub const SECONDS_PER_TICK: f64 = 1.0 / TICKS_PER_SECOND as f64;

/// Length of one full day/night cycle in ticks (20 real minutes).
pub const TICKS_PER_DAY: i64 = 24_000;

/// Window title for the current build.
pub fn window_title() -> String {
    format!("{GAME_NAME} {GAME_VERSION}")
}

//! The Hearth game: launcher, main loop and settings. The binary in `main.rs` is a thin
//! wrapper so tools (benchmarks, screenshot mode) can reuse the pieces.

pub mod alloc_count;
pub mod app;
pub mod bench;
pub mod body_panel;
pub mod building;
pub mod client;
pub mod content_cli;
pub mod content_state;
pub mod crafting_ui;
pub mod edits;
pub mod environment;
pub mod fauna;
pub mod frame_limiter;
pub mod gamepad;
pub mod globe;
pub mod hearing;
pub mod interface;
pub mod inventory_ui;
pub mod journal_ui;
pub mod knapping_ui;
pub mod lod_stream;
pub mod menus;
pub mod people;
pub mod profiles;
pub mod scene;
pub mod screenshot;
pub mod season_cover;
pub mod server;
pub mod signs;
pub mod structure;
pub mod water_env;
pub mod wildfire;
pub mod workshop;

pub use app::{LaunchConfig, resolve_dirs, run};

//! The Hearth game: launcher, main loop and settings. The binary in `main.rs` is a thin
//! wrapper so tools (benchmarks, screenshot mode) can reuse the pieces.

pub mod alloc_count;
pub mod app;
pub mod bench;
pub mod client;
pub mod content_cli;
pub mod content_state;
pub mod environment;
pub mod frame_limiter;
pub mod gamepad;
pub mod globe;
pub mod hearing;
pub mod interface;
pub mod lod_stream;
pub mod menus;
pub mod scene;
pub mod screenshot;
pub mod season_cover;
pub mod server;
pub mod water_env;

pub use app::{LaunchConfig, resolve_dirs, run};

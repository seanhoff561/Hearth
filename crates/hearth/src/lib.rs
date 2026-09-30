//! The Hearth game: launcher, main loop and settings. The binary in `main.rs` is a thin
//! wrapper so tools (benchmarks, screenshot mode) can reuse the pieces.

pub mod app;
pub mod content_cli;
pub mod content_state;
pub mod environment;
pub mod frame_limiter;
pub mod lod_stream;
pub mod preview;
pub mod scene;
pub mod screenshot;
pub mod season_cover;
pub mod streamer;

pub use app::{LaunchConfig, resolve_dirs, run};

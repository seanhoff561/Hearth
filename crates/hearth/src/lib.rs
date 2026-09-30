//! The Hearth game: launcher, main loop and settings. The binary in `main.rs` is a thin
//! wrapper so tools (benchmarks, screenshot mode) can reuse the pieces.

pub mod app;
pub mod frame_limiter;

pub use app::{LaunchConfig, run};

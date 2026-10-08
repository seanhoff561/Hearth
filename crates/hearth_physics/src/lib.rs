//! Bodies moving through the block world (v2 §9.2): a box swept against the blocks' collision
//! shapes one axis at a time (so it slides along walls and never tunnels), gravity and air drag,
//! human gaits with quick starts and stops (walk 1.4 m/s, jog 3, sprint 6.5 in bursts), standing
//! jumps of about 0.45 m, steps taken freely up to 0.6 m and scrambled up to a block, ledges
//! climbed up to head height with both hands, crouching and crawling through low gaps, wading
//! and swimming with breath held under water, and the speed of every landing for the body to
//! judge (`hearth_body::Body::land`). Animals will move with the same code (V2-7).
//!
//! Movement is not compressed by the day scale: it runs in seconds of play.

mod mover;
pub mod testing;
mod world;

pub use mover::{Ability, Gait, Intent, Motion, Mover, Report, Stance, step};
pub use world::{BlockWorld, Ground, Plant, Terrain};

/// Gravity (m/s²).
pub const GRAVITY: f64 = 9.81;

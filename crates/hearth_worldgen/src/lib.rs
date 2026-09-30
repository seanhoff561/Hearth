//! World generation for a whole Earth-like planet.
//!
//! Pipeline (coarse → fine), every stage a pure deterministic function of the seed:
//! 1. [`planet`] — the planet model on the analysis grid: tectonics, elevation, erosion,
//!    drainage, climate (built once per world, saved with it).
//! 2. Regional sampling — block-resolution surface height, water, biome, rivers, materials.
//! 3. Cube generation — terrain, caves, ores, water and features for one 16³ cube.

pub mod noise;
pub mod planet;
pub mod settings;

pub use planet::PlanetGrid;
pub use settings::{FeatureRarity, SpawnClimate, WorldGenSettings};

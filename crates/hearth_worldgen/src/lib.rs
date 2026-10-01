//! World generation for a whole Earth-like planet.
//!
//! Pipeline (coarse → fine), every stage a pure deterministic function of the seed:
//! 1. [`planet`] — the planet model on the analysis grid: tectonics, elevation, erosion,
//!    drainage, climate (built once per world, saved with it).
//! 2. Regional sampling — block-resolution surface height, water, biome, rivers, materials.
//! 3. Cube generation — terrain, caves, ores, water and features for one 16³ cube.

pub mod coverage;
pub mod cubegen;
pub mod deposits;
pub mod geology;
pub mod hydro;
pub mod noise;
pub mod planet;
pub mod realms;
pub mod region;
pub mod settings;
pub mod soil;
pub mod trees;
pub mod vegetation;

pub use cubegen::{CubeClass, WorldGenerator};
pub use planet::PlanetGrid;
pub use region::{ColumnSample, Surface, Terrain};
pub use settings::{FeatureRarity, SpawnClimate, WorldGenSettings};

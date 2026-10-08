//! World-creation settings for the generator.

use hearth_math::{Planet, PlanetError, PlanetSize};
use serde::{Deserialize, Serialize};

/// How often the spectacular features (great ranges, trenches, giant caverns) appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureRarity {
    Rare,
    Standard,
    Common,
}

impl FeatureRarity {
    /// Multiplier on the frequency of spectacular features.
    pub fn frequency(self) -> f64 {
        match self {
            FeatureRarity::Rare => 1.0,
            FeatureRarity::Standard => 1.8,
            FeatureRarity::Common => 3.0,
        }
    }
}

/// Where the player spawns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpawnClimate {
    Temperate,
    Random,
}

/// Everything the generator needs to know about a world.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldGenSettings {
    pub seed: u64,
    pub planet_size: PlanetSize,
    /// Multiplier on the planet size's auto vertical scale (0.25–2).
    pub vertical_scale_factor: f64,
    pub rarity: FeatureRarity,
    /// Target fraction of the planet's surface that is land (0.2–0.5).
    pub land_fraction: f64,
    pub spawn_climate: SpawnClimate,
    /// Resolution of the planet analysis grid (N×N cells); 0 picks the recommended resolution
    /// for the planet size. Tests use small explicit grids.
    pub grid_resolution: usize,
}

impl Default for WorldGenSettings {
    fn default() -> Self {
        Self {
            seed: 0,
            planet_size: PlanetSize::Standard,
            vertical_scale_factor: 1.0,
            rarity: FeatureRarity::Rare,
            land_fraction: 0.3,
            spawn_climate: SpawnClimate::Temperate,
            grid_resolution: 0,
        }
    }
}

impl WorldGenSettings {
    pub fn planet(&self) -> Result<Planet, PlanetError> {
        Planet::from_size(self.planet_size)
    }

    /// Blocks per real metre of relief.
    pub fn vertical_scale(&self) -> f64 {
        self.planet_size.auto_vertical_scale() * self.vertical_scale_factor.clamp(0.25, 2.0)
    }

    /// Clamps values into their valid ranges.
    pub fn sanitized(mut self) -> Self {
        self.vertical_scale_factor = self.vertical_scale_factor.clamp(0.25, 2.0);
        self.land_fraction = self.land_fraction.clamp(0.2, 0.5);
        if self.grid_resolution == 0 {
            self.grid_resolution = Self::recommended_resolution(self.planet_size);
        }
        self.grid_resolution = self.grid_resolution.clamp(64, 4096).next_power_of_two();
        self
    }

    /// Grid resolution for a planet size: cells of at most ~32 blocks where affordable.
    pub fn recommended_resolution(size: hearth_math::PlanetSize) -> usize {
        use hearth_math::PlanetSize as P;
        match size {
            P::Tiny | P::Small => 1024,
            _ => 2048,
        }
    }
}

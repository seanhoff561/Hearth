//! World-creation settings for the generator.

use hearth_math::{Planet, PlanetError, PlanetSize};
use serde::{Deserialize, Serialize};

/// The share of the planet's surface that is land: Earth's, 29 % (Kossinna 1931 gives 29.2 %,
/// Eakins and Sharman 2012 29.05 %).
pub const LAND_FRACTION: f64 = 0.29;

/// Everything the generator needs to know about a world. The planet is Earth; the small test
/// planets are for tests, bots and benchmarks (Developer mode, E §5.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldGenSettings {
    pub seed: u64,
    pub planet_size: PlanetSize,
    /// Resolution of the planet analysis grid (N×N cells); 0 picks the recommended resolution
    /// for the planet size. Tests use small explicit grids.
    pub grid_resolution: usize,
}

impl Default for WorldGenSettings {
    fn default() -> Self {
        Self {
            seed: 0,
            planet_size: PlanetSize::Earth,
            grid_resolution: 0,
        }
    }
}

impl WorldGenSettings {
    pub fn planet(&self) -> Result<Planet, PlanetError> {
        Planet::from_size(self.planet_size)
    }

    /// Blocks per real metre of relief: one on Earth, less on the small test planets.
    pub fn vertical_scale(&self) -> f64 {
        self.planet_size.auto_vertical_scale()
    }

    /// Clamps values into their valid ranges.
    pub fn sanitized(mut self) -> Self {
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

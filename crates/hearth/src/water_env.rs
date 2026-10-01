//! The world as its finite water sees it (`hearth_world::water::WaterEnv`): natural water's
//! quality from the hydrology, open-water evaporation from the weather, and the groundwater a
//! hole dug below the water table takes in.

use hearth_math::BlockPos;
use hearth_world::water::{Ground, Quality, WaterEnv};
use hearth_worldgen::WorldGenerator;
use hearth_worldgen::hydro::WaterQuality;

/// A world's surroundings for its finite water, with the weather of the moment.
pub struct WorldWater<'a> {
    pub generator: &'a WorldGenerator,
    /// Air temperature (°C), relative humidity (0–1) and wind speed (m/s) now.
    pub air_c: f32,
    pub humidity: f32,
    pub wind_m_s: f32,
}

/// The finite water's view of a natural water quality.
pub fn quality(q: WaterQuality) -> Quality {
    Quality {
        salinity_g_l: q.salinity_g_l,
        pathogen_risk: q.pathogen_risk,
        temperature_c: q.temperature_c,
    }
}

/// Evaporation from open water (mm a day): a share from the warmth the sun brings (0.06 mm per
/// °C) and the drying power of the air by the mass-transfer law, (0.06 + 0.04 u) mm per hPa of
/// vapour-pressure deficit, u the wind (m/s) — about 10 mm in hot, dry, windy weather (lakes in
/// hot deserts), 1–2 mm in mild humid weather, under 1 mm in cool damp weather, none from frozen
/// water.
pub fn open_water_evaporation(air_c: f32, humidity: f32, wind_m_s: f32) -> f32 {
    if air_c <= 0.0 {
        return 0.0;
    }
    let saturation_hpa = 6.11 * (17.27 * air_c / (air_c + 237.3)).exp();
    let deficit_hpa = saturation_hpa * (1.0 - humidity.clamp(0.0, 1.0));
    0.06 * air_c + (0.06 + 0.04 * wind_m_s.max(0.0)) * deficit_hpa
}

impl WaterEnv for WorldWater<'_> {
    fn natural(&self, p: BlockPos) -> Quality {
        let wg = self.generator;
        wg.hydro
            .quality(wg, p.x, p.y, p.z)
            .map(quality)
            .unwrap_or(Quality::FRESH)
    }

    fn weather(&self, _: BlockPos) -> (f32, f32) {
        (
            self.air_c,
            open_water_evaporation(self.air_c, self.humidity, self.wind_m_s),
        )
    }

    fn ground(&self, p: BlockPos) -> Option<Ground> {
        let wg = self.generator;
        let table = wg.hydro.water_table(wg, p.x, p.z);
        if p.y as f32 >= table {
            return None;
        }
        Some(Ground {
            table,
            seep_l_per_day: wg.hydro.seepage(wg, p.x, p.y, p.z),
            quality: wg
                .hydro
                .quality(wg, p.x, p.y, p.z)
                .map(quality)
                .unwrap_or(Quality::FRESH),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_water_evaporates_by_heat_dryness_and_wind() {
        let desert = open_water_evaporation(32.0, 0.2, 4.0);
        let temperate = open_water_evaporation(15.0, 0.7, 2.0);
        let damp = open_water_evaporation(8.0, 0.9, 1.0);
        assert!((8.0..14.0).contains(&desert), "desert {desert}");
        assert!((1.0..3.0).contains(&temperate), "temperate {temperate}");
        assert!(damp < 0.8, "damp {damp}");
        assert_eq!(open_water_evaporation(-5.0, 0.5, 3.0), 0.0);
    }
}

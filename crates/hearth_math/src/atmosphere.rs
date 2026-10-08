//! The air at a height (standard atmosphere) and water's boiling point there.

/// Air pressure at sea level (kPa).
pub const SEA_LEVEL_KPA: f64 = 101.325;

/// Air pressure (kPa) at a height (m above the sea), in the standard atmosphere (ISO 2533).
pub fn pressure_kpa(height_m: f64) -> f64 {
    SEA_LEVEL_KPA
        * (1.0 - 2.25577e-5 * height_m.max(-500.0))
            .max(0.01)
            .powf(5.25588)
}

/// Water's boiling point (°C) at a height: about a degree lower for each 300 m
/// (Clausius–Clapeyron, water's heat of vaporisation 40.66 kJ/mol).
pub fn boiling_c(height_m: f64) -> f64 {
    const L_OVER_R: f64 = 40_660.0 / 8.314;
    let p = pressure_kpa(height_m) / SEA_LEVEL_KPA;
    1.0 / (1.0 / 373.15 - p.ln() / L_OVER_R) - 273.15
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_air_thins_and_water_boils_cooler_with_height() {
        assert!((pressure_kpa(0.0) - 101.325).abs() < 1e-9);
        // About half the sea's pressure at 5.5 km, a third on Everest (33.7 kPa measured).
        assert!((pressure_kpa(5500.0) / 101.325 - 0.5).abs() < 0.02);
        assert!((pressure_kpa(8848.0) - 31.4).abs() < 2.5);
        assert!((boiling_c(0.0) - 100.0).abs() < 0.1);
        // Some 90 °C at 3 km, 70 °C on Everest's summit (about a degree per 300 m).
        assert!(
            (boiling_c(3000.0) - 90.0).abs() < 1.0,
            "{}",
            boiling_c(3000.0)
        );
        assert!(
            (boiling_c(8848.0) - 70.5).abs() < 2.0,
            "{}",
            boiling_c(8848.0)
        );
    }
}

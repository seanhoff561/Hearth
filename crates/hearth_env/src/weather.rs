//! Day-scale weather (v2 §4.3): what the player sees and feels — clouds, rain, snow, storms,
//! wind and temperature swings. Weather systems are fields on the planet's sphere that drift
//! with the prevailing winds (westerlies in mid-latitudes, trade winds and polar easterlies)
//! and evolve over time; the local seasonal climate decides how often they bring precipitation
//! and how much, so a month's weather averages to the month's normals.
//!
//! The model is a pure function of (seed, place, time): server, client and headless tools all
//! agree without sending weather around.

use std::f64::consts::TAU;

use glam::DVec3;
use hearth_math::Planet;
use hearth_worldgen::noise::SphereFbm;
use hearth_worldgen::planet::climate::prevailing_wind;

use crate::climate::Normals;

/// Precipitation type at the ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precip {
    None,
    Rain,
    Sleet,
    Snow,
}

/// Weather at one place and moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherState {
    /// Fraction of the sky covered by cloud (0..1).
    pub cloud_cover: f64,
    /// Precipitation rate (mm of water per hour).
    pub precip_mm_h: f64,
    pub precip: Precip,
    /// Chance per minute of a lightning strike nearby (0..1).
    pub thunder: f64,
    /// Air temperature (°C) including the synoptic anomaly and the time of day.
    pub temperature_c: f64,
    /// Direction the wind blows toward (radians; 0 = toward north, π/2 = toward east).
    pub wind_dir: f64,
    pub wind_speed_m_s: f64,
    /// Relative humidity (0..1).
    pub humidity: f64,
}

/// Weather held as given wherever it is sampled (tests and bots): what is set replaces the
/// model's.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WeatherHold {
    pub humidity: Option<f64>,
    pub temperature_c: Option<f64>,
    pub precip_mm_h: Option<f64>,
    pub wind_speed_m_s: Option<f64>,
    /// Toward (radians; 0 north, π/2 east).
    pub wind_dir: Option<f64>,
}

impl WeatherHold {
    pub fn apply(&self, w: &mut WeatherState) {
        if let Some(h) = self.humidity {
            w.humidity = h.clamp(0.0, 1.0);
        }
        if let Some(t) = self.temperature_c {
            w.temperature_c = t;
        }
        if let Some(p) = self.precip_mm_h {
            w.precip_mm_h = p.max(0.0);
            if p <= 0.0 {
                w.precip = Precip::None;
                w.thunder = 0.0;
            } else if w.precip == Precip::None {
                w.precip = Precip::Rain;
            }
        }
        if let Some(s) = self.wind_speed_m_s {
            w.wind_speed_m_s = s.max(0.0);
        }
        if let Some(d) = self.wind_dir {
            w.wind_dir = d;
        }
    }
}

/// Inverse of the standard normal CDF (Acklam's rational approximation, |error| < 1.2e-9).
fn inv_norm(p: f64) -> f64 {
    let p = p.clamp(1e-9, 1.0 - 1e-9);
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    let q = if p < 0.02425 {
        (-2.0 * p.ln()).sqrt()
    } else if p > 1.0 - 0.02425 {
        (-2.0 * (1.0 - p).ln()).sqrt()
    } else {
        0.0
    };
    if p < 0.02425 {
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p > 1.0 - 0.02425 {
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    }
}

/// The weather of one world.
#[derive(Debug, Clone)]
pub struct WeatherModel {
    planet: Planet,
    storms: SphereFbm,
    anomaly: SphereFbm,
    gusts: SphereFbm,
    /// Small convective cells (showers, thunderstorms).
    cells: SphereFbm,
    /// Standard deviations of the storm and cell fields (measured at construction).
    sigma: f64,
    cell_sigma: f64,
}

/// Fraction of a wet area covered by convective cells at any moment.
const CELL_COVER: f64 = 0.15;

/// Fraction of the planet's circumference weather systems travel per day in the westerlies.
const WESTERLY_DRIFT: f64 = 1.0 / 45.0;
/// Rate at which the storm field changes shape, in noise units per day.
const EVOLUTION: f64 = 0.35;

impl WeatherModel {
    pub fn new(seed: u64, planet: Planet) -> Self {
        // Synoptic systems are roughly a twentieth of the circumference across.
        let storms = SphereFbm::new(
            hearth_math::hash::derive_seed(seed, "weather/storms"),
            3.2,
            4,
            0.5,
            2.1,
        );
        let anomaly = SphereFbm::new(
            hearth_math::hash::derive_seed(seed, "weather/anomaly"),
            2.0,
            3,
            0.5,
            2.0,
        );
        let gusts = SphereFbm::new(
            hearth_math::hash::derive_seed(seed, "weather/gusts"),
            6.0,
            2,
            0.5,
            2.0,
        );
        let cells = SphereFbm::new(
            hearth_math::hash::derive_seed(seed, "weather/cells"),
            40.0,
            2,
            0.5,
            2.0,
        );
        let mut m = Self {
            planet,
            storms,
            anomaly,
            gusts,
            cells,
            sigma: 0.25,
            cell_sigma: 0.25,
        };
        // Measure the storm field's spread so wet-day frequencies come out right.
        let (mut sum2, mut cell2) = (0.0, 0.0);
        let n = 2000;
        let mut rng = hearth_math::hash::Rng::new(seed ^ 0x5eed);
        for i in 0..n {
            let p = DVec3::new(
                rng.range_f64(-1.0, 1.0),
                rng.range_f64(-1.0, 1.0),
                rng.range_f64(-1.0, 1.0),
            )
            .normalize_or(DVec3::Y);
            let v = m.storm_value(p, i as f64 * 0.37);
            sum2 += v * v;
            let c = m.cells.sample(p);
            cell2 += c * c;
        }
        m.sigma = (sum2 / n as f64).sqrt().max(1e-3);
        m.cell_sigma = (cell2 / n as f64).sqrt().max(1e-3);
        m
    }

    fn storm_value(&self, p: DVec3, days: f64) -> f64 {
        self.storms
            .sample(p + DVec3::new(0.31, 0.57, 0.77) * (days * EVOLUTION))
    }

    /// Rotates a sphere point about the polar axis by `angle` radians (toward east when positive).
    fn advect(p: DVec3, angle: f64) -> DVec3 {
        let (s, c) = angle.sin_cos();
        // Longitude increases from +X toward the direction of +X motion (east); rotate about Y.
        DVec3::new(p.x * c + p.z * s, p.y, -p.x * s + p.z * c)
    }

    /// Weather at world position (x, z) at `days` (game days since the calendar origin).
    pub fn sample(
        &self,
        n: &Normals,
        x: f64,
        z: f64,
        days: f64,
        year_frac: f64,
        local_time: f64,
    ) -> WeatherState {
        let p = self.planet.sphere_point(x, z);
        let lat = n.lat_deg;
        let zonal = prevailing_wind(lat); // +1 westerly (toward east), −1 easterly
        // Systems drift with the wind: sample the field where the air came from.
        let drift = -zonal * WESTERLY_DRIFT * TAU * days;
        let q = Self::advect(p, drift);
        let s = self.storm_value(q, days) / self.sigma;
        // How often it precipitates at this time of year.
        let mm_day = n.precip_mm_per_day(year_frac);
        let wet_frac = (mm_day / (mm_day + 4.0)).clamp(0.02, 0.85);
        let threshold = inv_norm(1.0 - wet_frac);
        let excess = s - threshold;
        let precip_mm_h = if excess > 0.0 && mm_day > 0.01 {
            // Mean intensity while wet, higher in the core of a system.
            let mean_rate = mm_day / (24.0 * wet_frac);
            let steady = mean_rate * (0.4 + 1.2 * excess.min(3.0));
            // In warm climates much of the rain falls in short, intense convective cells that
            // cover a small part of the wet area (same total, far higher rates).
            let convective = ((n.temperature(year_frac) - 14.0) / 10.0).clamp(0.0, 0.85);
            let cell = self.cells.sample(q + DVec3::Z * days * 3.0) / self.cell_sigma;
            let in_cell = cell > inv_norm(1.0 - CELL_COVER);
            (1.0 - convective) * steady
                + if in_cell {
                    convective * steady / CELL_COVER
                } else {
                    0.0
                }
        } else {
            0.0
        };
        let cloud_cover = {
            let frontal = ((s - (threshold - 1.1)) / 1.3).clamp(0.0, 1.0);
            // Fair-weather cumulus on warm humid afternoons.
            let humid = (mm_day / 4.0).min(1.0);
            let afternoon = (1.0 - ((local_time - 0.6) / 0.2).powi(2)).max(0.0);
            (frontal + 0.25 * humid * afternoon).min(1.0)
        };
        let base_t = n.temperature_at(year_frac, local_time);
        // Synoptic swings: larger in continental mid-latitudes and in winter (a few °C
        // typical, rarely beyond ±8 °C); cloudy days have a smaller day–night range.
        let winterness = 0.5 * (1.0 - n.seasonal_wave(year_frac));
        let swing = (1.5 + 3.0 * (n.t_range / 40.0).min(1.0)) * (0.6 + 0.4 * winterness);
        let anomaly = self
            .anomaly
            .sample(Self::advect(p, drift * 0.8) + DVec3::X * days * 0.2)
            * swing
            * 1.6;
        let cloud_damp = 1.0 - 0.5 * cloud_cover;
        let temperature_c =
            n.temperature(year_frac) + (base_t - n.temperature(year_frac)) * cloud_damp + anomaly
                - if precip_mm_h > 0.0 { 1.5 } else { 0.0 };
        let precip = if precip_mm_h <= 0.0 {
            Precip::None
        } else if temperature_c < 0.0 {
            Precip::Snow
        } else if temperature_c < 2.0 {
            Precip::Sleet
        } else {
            Precip::Rain
        };
        // Convective storms: heavy rain in warm, humid air.
        let thunder = if precip == Precip::Rain && temperature_c > 16.0 {
            ((precip_mm_h - 2.0) / 8.0).clamp(0.0, 1.0)
                * ((temperature_c - 16.0) / 10.0).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let gust = self.gusts.sample(q + DVec3::Y * days * 1.3);
        let wind_speed_m_s =
            (3.0 + 4.0 * zonal.abs() + 6.0 * excess.max(0.0) + 3.0 * gust).max(0.0);
        // Mostly zonal, turning with the storm field's gradient direction.
        let wind_dir = if zonal >= 0.0 {
            TAU / 4.0
        } else {
            3.0 * TAU / 4.0
        } + gust * 0.8;
        let humidity = (0.45 + 0.4 * (mm_day / 6.0).min(1.0) + 0.3 * cloud_cover).min(1.0);
        WeatherState {
            cloud_cover,
            precip_mm_h,
            precip,
            thunder,
            temperature_c,
            wind_dir: wind_dir.rem_euclid(TAU),
            wind_speed_m_s,
            humidity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_normal_is_accurate() {
        assert!(inv_norm(0.5).abs() < 1e-9);
        assert!((inv_norm(0.975) - 1.959_964).abs() < 1e-5);
        assert!((inv_norm(0.1) + 1.281_552).abs() < 1e-5);
    }

    #[test]
    fn weather_averages_to_the_climate() {
        let planet = Planet::from_size(hearth_math::PlanetSize::Standard).unwrap();
        let model = WeatherModel::new(7, planet);
        let wet = Normals::new(50.0, 10.0, 15.0, 1400.0, 0.0, 0.0);
        let dry = Normals::new(25.0, 24.0, 15.0, 150.0, 0.0, 0.0);
        let z = planet.z_for_latitude(50f64.to_radians());
        let mut total = [0.0f64; 2];
        let mut wet_hours = [0usize; 2];
        let hours = 24 * 365;
        for h in 0..hours {
            let days = h as f64 / 24.0;
            let yf = (days / 365.2422).fract();
            for (k, n) in [wet, dry].iter().enumerate() {
                let w = model.sample(n, 1234.0, z, days, yf, days.fract());
                total[k] += w.precip_mm_h;
                if w.precip_mm_h > 0.0 {
                    wet_hours[k] += 1;
                }
                assert!((0.0..=1.0).contains(&w.cloud_cover));
            }
        }
        // Annual totals come out near the normals (weather is random: allow ±35%).
        assert!(
            (total[0] / 1400.0 - 1.0).abs() < 0.35,
            "wet climate total {}",
            total[0]
        );
        assert!(
            (total[1] / 150.0 - 1.0).abs() < 0.5,
            "dry climate total {}",
            total[1]
        );
        assert!(wet_hours[0] > 3 * wet_hours[1], "{wet_hours:?}");
    }

    #[test]
    fn cold_weather_snows_and_warm_weather_storms() {
        let planet = Planet::from_size(hearth_math::PlanetSize::Standard).unwrap();
        let model = WeatherModel::new(3, planet);
        let arctic = Normals::new(70.0, -12.0, 30.0, 400.0, 0.0, 0.0);
        let tropics = Normals::new(5.0, 27.0, 2.0, 3000.0, 0.0, 0.0);
        let (mut snow, mut rain, mut thunder) = (0, 0, 0);
        for h in 0..24 * 120 {
            let days = h as f64 / 24.0;
            let a = model.sample(&arctic, 500.0, -9000.0, days, 0.8, days.fract());
            if a.precip == Precip::Snow {
                snow += 1;
            }
            if a.precip == Precip::Rain {
                rain += 1;
            }
            let t = model.sample(&tropics, 500.0, 0.0, days, 0.3, days.fract());
            if t.thunder > 0.0 {
                thunder += 1;
            }
        }
        assert!(
            snow > 10 * (rain + 1),
            "arctic winter precipitation is snow ({snow} vs {rain})"
        );
        assert!(thunder > 0, "tropical storms");
    }
}

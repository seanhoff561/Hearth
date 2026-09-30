//! Seasonal climate from the planet's climate normals (v2 §4.1, §4.3).
//!
//! The planet model gives, per place, the annual mean temperature, the annual range (warmest
//! minus coldest month), annual precipitation and dry-season strengths. This module turns them
//! into the course of a year: daily mean and hourly temperature, precipitation seasonality (wet
//! and dry seasons, monsoons, mediterranean summers, the equatorial double rainy season), and —
//! on the year (calendar) scale — the seasonal snowpack and lake ice.

use std::f64::consts::TAU;

use hearth_worldgen::PlanetGrid;

/// Base temperature for growth (°C).
pub const GROWTH_BASE_C: f64 = 5.0;

/// Climate normals of one place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Normals {
    pub lat_deg: f64,
    /// Annual mean temperature (°C) at the place's elevation.
    pub t_mean: f64,
    /// Warmest minus coldest month (°C).
    pub t_range: f64,
    /// Annual precipitation (mm).
    pub precip: f64,
    /// Strength of a winter dry season (savanna, monsoon) 0..1.
    pub winter_dry: f64,
    /// Strength of a summer dry season (mediterranean) 0..1.
    pub summer_dry: f64,
    /// Mean of the raw precipitation seasonality weight over the year (normaliser).
    norm: f64,
}

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Normals {
    pub fn new(
        lat_deg: f64,
        t_mean: f64,
        t_range: f64,
        precip: f64,
        winter_dry: f64,
        summer_dry: f64,
    ) -> Self {
        let mut n = Self {
            lat_deg,
            t_mean,
            t_range: t_range.max(0.0),
            precip: precip.max(0.0),
            winter_dry: winter_dry.clamp(0.0, 1.0),
            summer_dry: summer_dry.clamp(0.0, 1.0),
            norm: 1.0,
        };
        let samples = 48;
        let sum: f64 = (0..samples)
            .map(|i| n.raw_precip_weight((i as f64 + 0.5) / samples as f64))
            .sum();
        n.norm = (sum / samples as f64).max(1e-6);
        n
    }

    /// Normals of a place on the planet (smooth fields are bilinearly interpolated).
    pub fn sample(grid: &PlanetGrid, x: f64, z: f64) -> Self {
        let planet = grid.planet();
        let (gx, gz) = grid.geom.grid_coords(planet.wrap_xf(x), z);
        let lat = planet.latitude_deg(z);
        Self::new(
            lat,
            grid.temperature.bilinear(gx, gz) as f64,
            grid.temp_range.bilinear(gx, gz) as f64,
            grid.precipitation.bilinear(gx, gz) as f64,
            grid.winter_dry.bilinear(gx, gz) as f64,
            grid.summer_dry.bilinear(gx, gz) as f64,
        )
    }

    /// Normals with the mean temperature shifted by an elevation difference (lapse rate).
    pub fn at_elevation_offset(mut self, metres: f64) -> Self {
        self.t_mean -= hearth_worldgen::planet::climate::LAPSE_RATE * metres;
        self
    }

    pub fn southern(&self) -> bool {
        self.lat_deg < 0.0
    }

    /// Year fraction as seen from this hemisphere (0 = local spring equinox).
    fn local_year(&self, year_frac: f64) -> f64 {
        if self.southern() {
            (year_frac + 0.5).fract()
        } else {
            year_frac
        }
    }

    /// Seasonal wave: +1 at the warmest time of the year, −1 at the coldest. The peak lags the
    /// solstice by about a month over land (more at sea, where the range is small).
    pub fn seasonal_wave(&self, year_frac: f64) -> f64 {
        let lag = 0.07 + 0.05 * (1.0 - smoothstep(8.0, 25.0, self.t_range));
        (TAU * (self.local_year(year_frac) - 0.25 - lag)).cos()
    }

    /// Daily mean temperature (°C).
    pub fn temperature(&self, year_frac: f64) -> f64 {
        self.t_mean + 0.5 * self.t_range * self.seasonal_wave(year_frac)
    }

    /// Day–night temperature range (°C): large in dry continental climates, small in humid ones.
    pub fn diurnal_range(&self) -> f64 {
        let aridity = 1.0 - smoothstep(250.0, 1800.0, self.precip);
        5.0 + 11.0 * aridity + 3.0 * smoothstep(15.0, 40.0, self.t_range)
    }

    /// Temperature at a local solar time (0.5 = noon; warmest mid-afternoon, coldest near dawn).
    pub fn temperature_at(&self, year_frac: f64, local_time: f64) -> f64 {
        self.temperature(year_frac)
            + 0.5 * self.diurnal_range() * (TAU * (local_time - 0.625)).cos()
    }

    fn raw_precip_weight(&self, year_frac: f64) -> f64 {
        let y = self.local_year(year_frac);
        // +1 in high summer (the rain belt follows the sun, peaking a little after it).
        let summer = (TAU * (y - 0.3)).cos();
        let mut w = 1.0;
        // Savanna and monsoon climates: wet summer, dry winter.
        w *= 1.0 + 0.95 * self.winter_dry * summer;
        // Mediterranean climates: dry summer, wet winter.
        w *= 1.0 - 0.85 * self.summer_dry * summer;
        // Continental interiors get more rain in summer (convection).
        let continental = smoothstep(18.0, 40.0, self.t_range);
        w *= 1.0 + 0.35 * continental * summer;
        // Near the equator the rain belt passes twice: two rainy seasons after the equinoxes.
        let equatorial = 1.0 - smoothstep(4.0, 10.0, self.lat_deg.abs());
        w *= 1.0 + 0.35 * equatorial * (2.0 * TAU * (year_frac - 0.06)).cos();
        w.max(0.0)
    }

    /// Precipitation relative to the annual mean at this time of year (mean 1 over a year).
    pub fn precip_weight(&self, year_frac: f64) -> f64 {
        self.raw_precip_weight(year_frac) / self.norm
    }

    /// Mean precipitation rate at this time of year (mm per real day).
    pub fn precip_mm_per_day(&self, year_frac: f64) -> f64 {
        self.precip / 365.2422 * self.precip_weight(year_frac)
    }

    /// Whether it is the dry season: precipitation well below the annual mean where the climate
    /// has a marked dry season.
    pub fn is_dry_season(&self, year_frac: f64) -> bool {
        self.winter_dry.max(self.summer_dry) > 0.3 && self.precip_weight(year_frac) < 0.4
    }

    /// Mean temperature below −2 °C: permanently frozen ground at depth.
    pub fn permafrost(&self) -> bool {
        self.t_mean < -2.0
    }

    /// Growing degree days accumulated over a year (base 5 °C).
    pub fn growing_degree_days(&self) -> f64 {
        let n = 73;
        (0..n)
            .map(|i| (self.temperature((i as f64 + 0.5) / n as f64) - GROWTH_BASE_C).max(0.0))
            .sum::<f64>()
            * 365.2422
            / n as f64
    }
}

/// Snowpack and lake ice over the year (calendar scale), integrated from the normals.
#[derive(Debug, Clone)]
pub struct SeasonalCover {
    /// Snow water equivalent (mm) at each step of the year.
    swe_mm: Vec<f32>,
    /// Still-water ice thickness (cm) at each step.
    ice_cm: Vec<f32>,
    /// Sea ice thickness (cm) at each step.
    sea_ice_cm: Vec<f32>,
    /// Snow never melts completely: glacier or ice-sheet conditions.
    pub perennial: bool,
}

/// Steps per year for the seasonal integration (5-day steps).
const STEPS: usize = 73;
/// Degree-day melt factor (mm water per °C·day).
const MELT_MM_PER_DEGREE_DAY: f64 = 3.5;
/// Stefan's law coefficient for lake ice (cm per sqrt(°C·day)), snow-covered ice.
const STEFAN_CM: f64 = 2.4;
/// Ice melt rate (cm per °C·day of thaw).
const ICE_MELT_CM: f64 = 1.2;
/// Sea water freezes at −1.8 °C, and the heat stored in the sea keeps it open until the air is
/// colder still: sea ice grows only below this air temperature (°C).
const SEA_FREEZE_C: f64 = -4.0;
/// Settled seasonal snow density (kg/m³).
pub const SNOW_DENSITY: f64 = 280.0;
/// Snowpack water equivalent is capped here (permanent snow does not grow forever).
const SWE_CAP_MM: f64 = 3000.0;

impl SeasonalCover {
    pub fn compute(n: &Normals) -> Self {
        let dt = 365.2422 / STEPS as f64;
        let mut swe = 0.0f64;
        let mut ice = 0.0f64;
        let mut sea = 0.0f64;
        let mut swe_out = vec![0f32; STEPS];
        let mut ice_out = vec![0f32; STEPS];
        let mut sea_out = vec![0f32; STEPS];
        let mut min_swe = f64::MAX;
        // Three years of spin-up from the coldest time so the state is periodic.
        for year in 0..4 {
            for (k, (s_out, i_out)) in swe_out.iter_mut().zip(ice_out.iter_mut()).enumerate() {
                let f = (k as f64 + 0.5) / STEPS as f64;
                let t = n.temperature(f);
                let p = n.precip_mm_per_day(f) * dt;
                // Snow below −0.5 °C, rain above 2 °C, a mix between.
                let snow_frac = ((2.0 - t) / 2.5).clamp(0.0, 1.0);
                swe += snow_frac * p;
                swe -= (MELT_MM_PER_DEGREE_DAY * t.max(0.0) * dt).min(swe);
                swe = swe.min(SWE_CAP_MM);
                if t < 0.0 {
                    let fdd = (ice / STEFAN_CM).powi(2) + (-t) * dt;
                    ice = STEFAN_CM * fdd.sqrt();
                } else {
                    ice = (ice - ICE_MELT_CM * t * dt).max(0.0);
                }
                // Sea ice: the same growth law below the sea's freezing threshold; it melts once
                // the air is above the freezing point of sea water.
                if t < SEA_FREEZE_C {
                    let fdd = (sea / STEFAN_CM).powi(2) + (SEA_FREEZE_C - t) * dt;
                    sea = STEFAN_CM * fdd.sqrt();
                } else if t > -1.8 {
                    sea = (sea - ICE_MELT_CM * (t + 1.8) * dt).max(0.0);
                }
                if year == 3 {
                    *s_out = swe as f32;
                    *i_out = ice as f32;
                    sea_out[k] = sea as f32;
                    min_swe = min_swe.min(swe);
                }
            }
        }
        Self {
            swe_mm: swe_out,
            ice_cm: ice_out,
            sea_ice_cm: sea_out,
            perennial: min_swe > 1.0,
        }
    }

    fn sample(v: &[f32], year_frac: f64) -> f64 {
        let x = year_frac.rem_euclid(1.0) * STEPS as f64 - 0.5;
        let i0 = x.floor().rem_euclid(STEPS as f64) as usize;
        let i1 = (i0 + 1) % STEPS;
        let t = x - x.floor();
        v[i0] as f64 * (1.0 - t) + v[i1] as f64 * t
    }

    /// Snow water equivalent (mm).
    pub fn swe_mm(&self, year_frac: f64) -> f64 {
        Self::sample(&self.swe_mm, year_frac)
    }

    /// Snow depth on open ground (m).
    pub fn snow_depth_m(&self, year_frac: f64) -> f64 {
        self.swe_mm(year_frac) / SNOW_DENSITY
    }

    /// Ice thickness on still water (m); rivers freeze to about half of it.
    pub fn ice_m(&self, year_frac: f64) -> f64 {
        Self::sample(&self.ice_cm, year_frac) / 100.0
    }

    /// Sea ice thickness (m).
    pub fn sea_ice_m(&self, year_frac: f64) -> f64 {
        Self::sample(&self.sea_ice_cm, year_frac) / 100.0
    }

    /// Year fraction with the deepest snow.
    pub fn peak_snow(&self) -> f64 {
        let (i, _) = self
            .swe_mm
            .iter()
            .enumerate()
            .fold(
                (0, f32::MIN),
                |acc, (i, &v)| if v > acc.1 { (i, v) } else { acc },
            );
        (i as f64 + 0.5) / STEPS as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mean_over(f: impl Fn(f64) -> f64, from: f64, to: f64) -> f64 {
        let n = 50;
        (0..n)
            .map(|i| f(from + (to - from) * (i as f64 + 0.5) / n as f64))
            .sum::<f64>()
            / n as f64
    }

    #[test]
    fn temperature_seasons_follow_the_hemisphere() {
        let north = Normals::new(50.0, 8.0, 20.0, 700.0, 0.0, 0.0);
        let south = Normals::new(-50.0, 8.0, 20.0, 700.0, 0.0, 0.0);
        // Northern July (year_frac ≈ 0.3) is warm, January (≈ 0.8) cold; the south is opposite.
        assert!(north.temperature(0.32) > 16.0 && north.temperature(0.82) < 0.0);
        assert!(south.temperature(0.82) > 16.0 && south.temperature(0.32) < 0.0);
        let annual = mean_over(|f| north.temperature(f), 0.0, 1.0);
        assert!((annual - 8.0).abs() < 0.05, "annual mean preserved");
        // Afternoons are warmer than dawn.
        assert!(north.temperature_at(0.3, 0.62) > north.temperature_at(0.3, 0.22) + 5.0);
    }

    #[test]
    fn precipitation_seasonality() {
        let savanna_n = Normals::new(12.0, 26.0, 5.0, 1000.0, 0.9, 0.0);
        let savanna_s = Normals::new(-12.0, 26.0, 5.0, 1000.0, 0.9, 0.0);
        let med = Normals::new(38.0, 16.0, 14.0, 600.0, 0.0, 0.9);
        let annual = mean_over(|f| savanna_n.precip_weight(f), 0.0, 1.0);
        assert!((annual - 1.0).abs() < 0.02, "weights average to 1");
        // Northern savanna: wet in June–September, dry in December–March.
        let wet = mean_over(|f| savanna_n.precip_weight(f), 0.2, 0.45);
        let dry = mean_over(|f| savanna_n.precip_weight(f), 0.7, 0.95);
        assert!(wet > 3.0 * dry, "wet {wet} dry {dry}");
        assert!(savanna_n.is_dry_season(0.8) && !savanna_n.is_dry_season(0.3));
        // The southern savanna is the other way round.
        assert!(savanna_s.is_dry_season(0.3) && !savanna_s.is_dry_season(0.8));
        // Mediterranean: dry summers.
        assert!(mean_over(|f| med.precip_weight(f), 0.25, 0.45) < 0.4);
        assert!(mean_over(|f| med.precip_weight(f), 0.7, 0.95) > 1.4);
    }

    #[test]
    fn snowpack_and_ice_by_latitude() {
        // Continental subarctic (e.g. central Siberia / interior Canada).
        let subarctic = SeasonalCover::compute(&Normals::new(62.0, -4.0, 38.0, 450.0, 0.0, 0.0));
        let late_winter = subarctic.snow_depth_m(0.95);
        let summer = subarctic.snow_depth_m(0.35);
        assert!(
            late_winter > 0.35,
            "deep snow in late winter: {late_winter}"
        );
        assert!(summer == 0.0, "no snow in summer: {summer}");
        assert!(
            subarctic.ice_m(0.0) > 0.6,
            "thick lake ice by spring: {}",
            subarctic.ice_m(0.0)
        );
        assert_eq!(subarctic.ice_m(0.4), 0.0, "lakes open in summer");
        assert!(!subarctic.perennial);
        // Mild maritime temperate: at most a little snow, no lasting lake ice.
        let maritime = SeasonalCover::compute(&Normals::new(51.0, 10.0, 12.0, 800.0, 0.0, 0.0));
        assert!(maritime.snow_depth_m(0.85) < 0.05);
        assert!(maritime.ice_m(0.85) < 0.02);
        // Tropics: never.
        let tropical = SeasonalCover::compute(&Normals::new(5.0, 26.0, 2.0, 2500.0, 0.0, 0.0));
        for i in 0..20 {
            let f = i as f64 / 20.0;
            assert_eq!(tropical.snow_depth_m(f), 0.0);
            assert_eq!(tropical.ice_m(f), 0.0);
        }
        // Ice cap: snow never melts.
        let cap = SeasonalCover::compute(&Normals::new(80.0, -25.0, 30.0, 200.0, 0.0, 0.0));
        assert!(cap.perennial);
        // The snowpack peaks in late winter / early spring in the north, six months later south.
        let south = SeasonalCover::compute(&Normals::new(-62.0, -4.0, 38.0, 450.0, 0.0, 0.0));
        let dn = subarctic.peak_snow();
        let ds = south.peak_snow();
        let diff = (dn - ds).rem_euclid(1.0);
        assert!((diff - 0.5).abs() < 0.08, "north peak {dn} south peak {ds}");
    }

    #[test]
    fn sea_ice_by_climate() {
        // High Arctic: pack ice all year.
        let arctic = SeasonalCover::compute(&Normals::new(82.0, -18.0, 30.0, 150.0, 0.0, 0.0));
        for i in 0..20 {
            let f = i as f64 / 20.0;
            assert!(
                arctic.sea_ice_m(f) > 0.5,
                "perennial pack at {f}: {}",
                arctic.sea_ice_m(f)
            );
        }
        // Subarctic bay (Hudson Bay, the northern Baltic): frozen in winter, open in summer.
        let bay = SeasonalCover::compute(&Normals::new(60.0, -3.0, 32.0, 400.0, 0.0, 0.0));
        assert!(
            bay.sea_ice_m(0.05) > 0.4,
            "winter sea ice: {}",
            bay.sea_ice_m(0.05)
        );
        assert_eq!(bay.sea_ice_m(0.55), 0.0, "open water in late summer");
        // Sea ice needs colder winters than lake ice: a coast with winters just below freezing
        // keeps its sea open while its ponds freeze.
        let mild = SeasonalCover::compute(&Normals::new(66.0, 2.0, 12.0, 900.0, 0.0, 0.0));
        assert!(mild.ice_m(0.05) > 0.0, "ponds freeze");
        assert_eq!(mild.sea_ice_m(0.05), 0.0, "the sea stays open");
        // Temperate seas never freeze.
        let maritime = SeasonalCover::compute(&Normals::new(51.0, 10.0, 12.0, 800.0, 0.0, 0.0));
        assert_eq!(maritime.sea_ice_m(0.05), 0.0);
    }

    #[test]
    fn growing_season_and_permafrost() {
        let boreal = Normals::new(62.0, -4.0, 38.0, 450.0, 0.0, 0.0);
        let temperate = Normals::new(48.0, 10.0, 18.0, 800.0, 0.0, 0.0);
        assert!(boreal.permafrost() && !temperate.permafrost());
        assert!(temperate.growing_degree_days() > boreal.growing_degree_days() * 1.5);
    }
}

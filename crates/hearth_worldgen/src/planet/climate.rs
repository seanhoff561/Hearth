//! Climate from physical causes: latitude, altitude (lapse rate), ocean currents per basin,
//! continentality, prevailing winds per latitude band, moisture advection with orographic lift
//! and rain shadows, dry seasons, and a simplified Köppen classification.

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::grid::GridGeom;

/// Simplified Köppen-style climate classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ClimateClass {
    Ocean = 0,
    TropicalRainforest = 1,
    TropicalSavanna = 2,
    HotDesert = 3,
    HotSteppe = 4,
    ColdDesert = 5,
    ColdSteppe = 6,
    Mediterranean = 7,
    HumidSubtropical = 8,
    Oceanic = 9,
    HumidContinental = 10,
    Subarctic = 11,
    Tundra = 12,
    IceCap = 13,
}

impl ClimateClass {
    pub const ALL: [ClimateClass; 14] = [
        ClimateClass::Ocean,
        ClimateClass::TropicalRainforest,
        ClimateClass::TropicalSavanna,
        ClimateClass::HotDesert,
        ClimateClass::HotSteppe,
        ClimateClass::ColdDesert,
        ClimateClass::ColdSteppe,
        ClimateClass::Mediterranean,
        ClimateClass::HumidSubtropical,
        ClimateClass::Oceanic,
        ClimateClass::HumidContinental,
        ClimateClass::Subarctic,
        ClimateClass::Tundra,
        ClimateClass::IceCap,
    ];

    pub fn from_u8(v: u8) -> ClimateClass {
        Self::ALL
            .get(v as usize)
            .copied()
            .unwrap_or(ClimateClass::Ocean)
    }

    /// Köppen-style code for maps and the debug overlay.
    pub fn code(self) -> &'static str {
        match self {
            ClimateClass::Ocean => "Ocean",
            ClimateClass::TropicalRainforest => "Af",
            ClimateClass::TropicalSavanna => "Aw",
            ClimateClass::HotDesert => "BWh",
            ClimateClass::HotSteppe => "BSh",
            ClimateClass::ColdDesert => "BWk",
            ClimateClass::ColdSteppe => "BSk",
            ClimateClass::Mediterranean => "Cs",
            ClimateClass::HumidSubtropical => "Cfa",
            ClimateClass::Oceanic => "Cfb",
            ClimateClass::HumidContinental => "Dfb",
            ClimateClass::Subarctic => "Dfc",
            ClimateClass::Tundra => "ET",
            ClimateClass::IceCap => "EF",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ClimateClass::Ocean => "ocean",
            ClimateClass::TropicalRainforest => "tropical rainforest",
            ClimateClass::TropicalSavanna => "tropical savanna",
            ClimateClass::HotDesert => "hot desert",
            ClimateClass::HotSteppe => "hot steppe",
            ClimateClass::ColdDesert => "cold desert",
            ClimateClass::ColdSteppe => "cold steppe",
            ClimateClass::Mediterranean => "mediterranean",
            ClimateClass::HumidSubtropical => "humid subtropical",
            ClimateClass::Oceanic => "oceanic",
            ClimateClass::HumidContinental => "humid continental",
            ClimateClass::Subarctic => "subarctic",
            ClimateClass::Tundra => "tundra",
            ClimateClass::IceCap => "ice cap",
        }
    }

    /// Map colour (the usual Köppen palette).
    pub fn color(self) -> [u8; 3] {
        match self {
            ClimateClass::Ocean => [30, 60, 120],
            ClimateClass::TropicalRainforest => [0, 0, 254],
            ClimateClass::TropicalSavanna => [70, 169, 250],
            ClimateClass::HotDesert => [254, 0, 0],
            ClimateClass::HotSteppe => [245, 165, 0],
            ClimateClass::ColdDesert => [254, 150, 149],
            ClimateClass::ColdSteppe => [255, 220, 100],
            ClimateClass::Mediterranean => [255, 255, 0],
            ClimateClass::HumidSubtropical => [150, 255, 150],
            ClimateClass::Oceanic => [100, 255, 80],
            ClimateClass::HumidContinental => [0, 200, 255],
            ClimateClass::Subarctic => [0, 125, 125],
            ClimateClass::Tundra => [178, 178, 178],
            ClimateClass::IceCap => [240, 240, 255],
        }
    }
}

/// Dry-season type.
pub mod dry_season {
    pub const NONE: u8 = 0;
    /// Winter-dry (savanna, monsoon margins).
    pub const WINTER: u8 = 1;
    /// Summer-dry (mediterranean).
    pub const SUMMER: u8 = 2;

    /// Dry-season type from continuous strengths.
    pub fn from_strengths(winter: f32, summer: f32) -> u8 {
        if winter > 0.5 {
            WINTER
        } else if summer > 0.5 {
            SUMMER
        } else {
            NONE
        }
    }
}

/// Maritime (sea-surface / coastal) annual mean temperature (°C) by |latitude| in degrees.
/// Continental interiors are derived from this with extra seasonality and winter cooling.
pub fn latitude_temperature(lat_deg: f64) -> f64 {
    const T: [(f64, f64); 10] = [
        (0.0, 27.0),
        (10.0, 26.8),
        (20.0, 25.0),
        (30.0, 21.0),
        (40.0, 15.5),
        (50.0, 9.5),
        (60.0, 4.0),
        (70.0, -2.5),
        (80.0, -10.0),
        (90.0, -18.0),
    ];
    interp(&T, lat_deg.abs())
}

/// Zonal-mean precipitation (mm/yr) by |latitude|: equatorial low, subtropical highs,
/// mid-latitude westerlies, dry polar highs.
pub fn latitude_precipitation(lat_deg: f64) -> f64 {
    const P: [(f64, f64); 14] = [
        (0.0, 2300.0),
        (5.0, 2300.0),
        (10.0, 1850.0),
        (15.0, 1150.0),
        (20.0, 650.0),
        (25.0, 420.0),
        (30.0, 480.0),
        (35.0, 680.0),
        (40.0, 900.0),
        (45.0, 1050.0),
        (55.0, 950.0),
        (65.0, 600.0),
        (75.0, 300.0),
        (90.0, 150.0),
    ];
    interp(&P, lat_deg.abs())
}

fn interp(table: &[(f64, f64)], x: f64) -> f64 {
    if x <= table[0].0 {
        return table[0].1;
    }
    for w in table.windows(2) {
        if x <= w[1].0 {
            let t = (x - w[0].0) / (w[1].0 - w[0].0);
            return w[0].1 + (w[1].1 - w[0].1) * t;
        }
    }
    table[table.len() - 1].1
}

#[inline]
fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Prevailing wind direction along X at a latitude: −1 = blowing toward −X (easterlies, trade
/// winds and polar easterlies), +1 = toward +X (westerlies). Smooth across band edges.
pub fn prevailing_wind(lat_deg: f64) -> f64 {
    let a = lat_deg.abs();
    let to_west = 1.0 - smoothstep(26.0, 34.0, a);
    let polar = smoothstep(58.0, 66.0, a);
    let westerly = 1.0 - to_west - polar;
    westerly - to_west - polar
}

/// Output climate fields.
#[derive(Debug, Clone)]
pub struct ClimateFields {
    /// Mean annual temperature at the cell's surface elevation (°C).
    pub temperature: Vec<f32>,
    /// Mean annual temperature reduced to sea level (°C).
    pub sea_level_temperature: Vec<f32>,
    /// Warmest minus coldest month (°C).
    pub temp_range: Vec<f32>,
    /// Annual precipitation (mm).
    pub precipitation: Vec<f32>,
    pub dry_season: Vec<u8>,
    /// Strength (0..1) of a winter dry season (savanna belts).
    pub winter_dry: Vec<f32>,
    /// Strength (0..1) of a summer dry season (mediterranean coasts).
    pub summer_dry: Vec<f32>,
    pub class: Vec<u8>,
    /// Sea-surface temperature anomaly from currents (°C).
    pub current: Vec<f32>,
    /// Continentality 0 (maritime) .. 1 (deep interior).
    pub continentality: Vec<f32>,
}

/// Lapse rate in °C per real metre.
pub const LAPSE_RATE: f64 = 0.0065;

/// Computes all climate fields. `elev` is in real metres; `ocean` marks ocean cells.
pub fn compute(geom: &GridGeom, elev: &[f32], ocean: &[bool]) -> ClimateFields {
    let n = geom.n;
    let lat_deg: Vec<f64> = geom.lat.iter().map(|l| l.to_degrees()).collect();

    // ---------------------------------------------------------------- ocean currents
    // Per row: distance (physical radians) to land toward the west and toward the east.
    let mut current = vec![0f32; geom.len()];
    current.par_chunks_mut(n).enumerate().for_each(|(j, row)| {
        let row_ocean = &ocean[j * n..(j + 1) * n];
        if row_ocean.iter().all(|o| *o) || row_ocean.iter().all(|o| !*o) {
            return;
        }
        let step = geom.phys_cell(j) / geom.radius();
        let dw = run_distance(row_ocean, -1);
        let de = run_distance(row_ocean, 1);
        let a = lat_deg[j].abs();
        let subtropical = smoothstep(8.0, 18.0, a) * (1.0 - smoothstep(42.0, 50.0, a));
        let subpolar = smoothstep(42.0, 50.0, a) * (1.0 - smoothstep(66.0, 74.0, a));
        let l = 0.10;
        for i in 0..n {
            if !row_ocean[i] {
                continue;
            }
            let west = (-(dw[i] as f64) * step / l).exp();
            let east = (-(de[i] as f64) * step / l).exp();
            let far_east = (-(de[i] as f64) * step / (l * 3.0)).exp();
            // Subtropical gyres: warm western-boundary currents (east coasts), cold
            // eastern-boundary currents (west coasts).
            let st = 6.0 * west - 7.0 * east;
            // Subpolar: warm drift reaching the eastern side of the basin (west coasts),
            // cold currents on the western side.
            let sp = 7.0 * far_east - 5.0 * west;
            row[i] = (st * subtropical + sp * subpolar) as f32;
        }
    });
    super::fields::blur(geom, &mut current, 2, 2);
    for (c, o) in current.iter_mut().zip(ocean) {
        if !*o {
            *c = 0.0;
        }
    }
    // Bleed the anomaly inland over coastal land (maritime influence).
    let mut influence = current.clone();
    let mut weight: Vec<f32> = ocean.iter().map(|o| if *o { 1.0 } else { 0.0 }).collect();
    super::fields::blur(geom, &mut influence, 6, 2);
    super::fields::blur(geom, &mut weight, 6, 2);

    // ---------------------------------------------------------------- continentality
    // Upwind land fetch along each row (how far the prevailing wind has travelled over land).
    let mut fetch = vec![0f32; geom.len()];
    fetch.par_chunks_mut(n).enumerate().for_each(|(j, row)| {
        let wind = prevailing_wind(lat_deg[j]);
        let dir: isize = if wind >= 0.0 { 1 } else { -1 };
        let row_ocean = &ocean[j * n..(j + 1) * n];
        if row_ocean.iter().all(|o| !*o) {
            row.fill(10.0);
            return;
        }
        let step = (geom.phys_cell(j) / geom.radius()) as f32;
        // Walk downwind twice around the row so every land run starts from its upwind ocean.
        let start = (0..n).find(|&i| row_ocean[i]).unwrap_or(0) as isize;
        let mut run = 0.0f32;
        for k in 0..2 * n as isize {
            let i = (start + dir * k).rem_euclid(n as isize) as usize;
            if row_ocean[i] {
                run = 0.0;
            } else {
                run += step;
            }
            row[i] = run;
        }
    });

    // Rows are independent sweeps; smooth across rows so no latitude striping remains.
    super::fields::blur(geom, &mut fetch, 3, 2);

    // ---------------------------------------------------------------- temperature
    let mut sea_t = vec![0f32; geom.len()];
    let mut temp = vec![0f32; geom.len()];
    let mut range = vec![0f32; geom.len()];
    let mut cont = vec![0f32; geom.len()];
    sea_t
        .par_chunks_mut(n)
        .zip(temp.par_chunks_mut(n))
        .zip(range.par_chunks_mut(n))
        .zip(cont.par_chunks_mut(n))
        .enumerate()
        .for_each(|(j, (((st, t), r), k))| {
            let a = lat_deg[j].abs();
            let base = latitude_temperature(a);
            for i in 0..n {
                let idx = j * n + i;
                let w = weight[idx].max(1e-4);
                let maritime = influence[idx] / w;
                let fetch_c = smoothstep(0.0, 0.45, fetch[idx] as f64);
                let continental = if ocean[idx] { 0.0 } else { fetch_c };
                k[i] = continental as f32;
                // Interiors are colder on average at high latitudes (long, bitter winters) and
                // a little hotter in the subtropics.
                let cont_shift = -11.0 * continental * smoothstep(35.0, 65.0, a)
                    + 2.5 * continental * (1.0 - smoothstep(25.0, 40.0, a));
                let anomaly = if ocean[idx] {
                    current[idx] as f64
                } else {
                    maritime as f64 * weight[idx].min(1.0) as f64 * (1.0 - continental)
                };
                let t_sl = base + anomaly + cont_shift;
                st[i] = t_sl as f32;
                let e = elev[idx].max(0.0) as f64;
                t[i] = (t_sl - LAPSE_RATE * e) as f32;
                // Seasonal amplitude: small at sea, large in high-latitude interiors.
                let s = (a.to_radians()).sin();
                let amp = 3.0 + 12.0 * s * s + 45.0 * s * continental;
                r[i] = if ocean[idx] {
                    (amp * 0.35) as f32
                } else {
                    amp as f32
                };
            }
        });

    // ---------------------------------------------------------------- precipitation
    let mut precip = vec![0f32; geom.len()];
    precip.par_chunks_mut(n).enumerate().for_each(|(j, row)| {
        let a = lat_deg[j].abs();
        let band = latitude_precipitation(a);
        let wind = prevailing_wind(lat_deg[j]);
        let dir: isize = if wind >= 0.0 { 1 } else { -1 };
        let wind_strength = wind.abs().max(0.25);
        let row_ocean = &ocean[j * n..(j + 1) * n];
        let step_blocks = geom.phys_cell(j);
        let step_rad = step_blocks / geom.radius();
        let start = (0..n).find(|&i| row_ocean[i]).unwrap_or(0) as isize;
        // Moisture carried by the wind, relative to the saturation of this latitude's typical
        // sea surface (so the latitude band table sets the level and the sweep the pattern).
        let sat_of = |t: f64| (0.068 * (t - 25.0)).exp().clamp(0.03, 1.4);
        let sat_ref = sat_of(latitude_temperature(a));
        let mut m = sat_ref;
        let mut last_h = 0.0f64;
        let tropical = 1.0 - smoothstep(10.0, 18.0, a);
        // Rain-out length (radians): cyclonic mid-latitude rain travels far inland.
        let rain_len = 0.6 + 0.5 * smoothstep(25.0, 40.0, a) - 0.2 * smoothstep(60.0, 75.0, a);
        for k in 0..2 * n as isize {
            let i = (start + dir * k).rem_euclid(n as isize) as usize;
            let idx = j * n + i;
            let t = sea_t[idx] as f64;
            if row_ocean[i] {
                // Evaporation: warm water loads the air quickly; cold currents barely.
                let sat = sat_of(t);
                m += (sat - m) * (1.0 - (-step_rad / 0.05).exp());
                last_h = 0.0;
                if k >= n as isize {
                    let cold = (-(current[idx] as f64)).max(0.0);
                    row[i] =
                        (band * (0.5 + 0.5 * m / sat_ref) * (1.0 - 0.06 * cold).max(0.3)) as f32;
                }
            } else {
                let h = elev[idx].max(0.0) as f64;
                let rise = (h - last_h).max(0.0);
                last_h = h;
                // Rain-out: a background rate per distance plus orographic lift.
                let base_rate = step_rad / rain_len;
                let oro = (rise / 1400.0) * wind_strength;
                let out = (m * (base_rate + oro)).min(m * 0.9);
                // Evapotranspiration recycles part of the rain back into the air.
                let recycle = 0.6 + 0.25 * tropical;
                m = (m - out * (1.0 - recycle)).max(0.02);
                if k >= n as isize {
                    let availability = (m / sat_ref).clamp(0.05, 1.8);
                    let orographic = 1.0 + (oro / base_rate.max(1e-6)).min(4.0) * 0.35;
                    let mut p = band * availability * orographic;
                    // Convective tropical rain keeps continental interiors wet.
                    p = p.max(band * 0.75 * tropical);
                    row[i] = p as f32;
                }
            }
        }
    });
    // Coastal currents: cold currents suppress rain (stable, foggy air); warm currents feed
    // humid east coasts (the summer-monsoon flank of the subtropical highs).
    for idx in 0..geom.len() {
        if !ocean[idx] {
            let w = weight[idx].max(1e-4);
            let maritime = (influence[idx] / w) as f64 * weight[idx].min(1.0) as f64;
            let f = if maritime < 0.0 {
                (1.0 + maritime * 0.09).max(0.25)
            } else {
                1.0 + maritime * 0.13
            };
            precip[idx] = (precip[idx] as f64 * f) as f32;
        }
    }
    super::fields::blur(geom, &mut precip, 3, 2);

    // ---------------------------------------------------------------- dry seasons & classes
    // Dry-season strengths are continuous so block-level sampling can interpolate them and
    // climate borders stay smooth.
    let belt_noise = crate::noise::SphereFbm::new(0x6e17_b0a1, 9.0, 3, 0.5, 2.0);
    let mut winter_dry = vec![0f32; geom.len()];
    let mut summer_dry = vec![0f32; geom.len()];
    winter_dry
        .par_chunks_mut(n)
        .zip(summer_dry.par_chunks_mut(n))
        .enumerate()
        .for_each(|(j, (wd, sd))| {
            let wind = prevailing_wind(lat_deg[j]);
            for i in 0..n {
                let idx = j * n + i;
                let p = precip[idx] as f64;
                // Belt edges wander a few degrees so climate borders aren't latitude lines.
                let a = lat_deg[j].abs() + belt_noise.sample(geom.sphere(i, j)) * 3.5;
                // Savanna belt: winter-dry as the rain belt migrates.
                let savanna = smoothstep(6.0, 11.0, a) * (1.0 - smoothstep(22.0, 27.0, a));
                wd[i] = (savanna * (1.0 - smoothstep(2000.0, 2600.0, p))) as f32;
                // Mediterranean: subtropical west coasts (westerly side, maritime air).
                let west =
                    smoothstep(0.0, 0.3, wind) * (1.0 - smoothstep(0.08, 0.16, fetch[idx] as f64));
                let med = smoothstep(28.0, 32.0, a) * (1.0 - smoothstep(42.0, 46.0, a));
                sd[i] = (med * west) as f32;
            }
        });
    super::fields::blur(geom, &mut winter_dry, 2, 1);
    super::fields::blur(geom, &mut summer_dry, 2, 1);
    let mut dry = vec![dry_season::NONE; geom.len()];
    let mut class = vec![0u8; geom.len()];
    for idx in 0..geom.len() {
        if ocean[idx] {
            class[idx] = ClimateClass::Ocean as u8;
            continue;
        }
        dry[idx] = dry_season::from_strengths(winter_dry[idx], summer_dry[idx]);
        class[idx] = classify(
            temp[idx] as f64,
            range[idx] as f64,
            precip[idx] as f64,
            dry[idx],
        ) as u8;
    }

    ClimateFields {
        temperature: temp,
        sea_level_temperature: sea_t,
        temp_range: range,
        precipitation: precip,
        dry_season: dry,
        winter_dry,
        summer_dry,
        class,
        current,
        continentality: cont,
    }
}

/// Distance in cells to the nearest land in direction `dir` along a wrapping row (0 on land).
fn run_distance(row_ocean: &[bool], dir: isize) -> Vec<u32> {
    let n = row_ocean.len();
    let mut out = vec![u32::MAX; n];
    let Some(start) = (0..n).find(|&i| !row_ocean[i]) else {
        return out;
    };
    // Walk opposite to `dir` so that each cell sees the land lying in direction `dir`.
    let mut d = u32::MAX;
    for k in 0..2 * n as isize {
        let i = (start as isize - dir * k).rem_euclid(n as isize) as usize;
        if !row_ocean[i] {
            d = 0;
        } else {
            d = d.saturating_add(1);
        }
        out[i] = d;
    }
    out
}

/// Köppen-style classification from mean temperature, annual range, precipitation and dry
/// season.
pub fn classify(t_mean: f64, t_range: f64, p: f64, dry: u8) -> ClimateClass {
    let t_warm = t_mean + t_range * 0.5;
    let t_cold = t_mean - t_range * 0.5;
    if t_warm < 0.0 {
        return ClimateClass::IceCap;
    }
    if t_warm < 10.0 {
        return ClimateClass::Tundra;
    }
    // Aridity threshold (mm): more evaporation when warm; summer rain is less effective.
    let add = match dry {
        dry_season::SUMMER => 0.0,
        dry_season::WINTER => 280.0,
        _ => 140.0,
    };
    let threshold = 20.0 * t_mean.max(0.0) + add;
    if p < threshold {
        let hot = t_mean >= 18.0;
        return match (p < threshold * 0.5, hot) {
            (true, true) => ClimateClass::HotDesert,
            (true, false) => ClimateClass::ColdDesert,
            (false, true) => ClimateClass::HotSteppe,
            (false, false) => ClimateClass::ColdSteppe,
        };
    }
    if t_cold >= 18.0 {
        return if dry == dry_season::WINTER && p < 2300.0 {
            ClimateClass::TropicalSavanna
        } else {
            ClimateClass::TropicalRainforest
        };
    }
    if t_cold > -3.0 {
        if dry == dry_season::SUMMER {
            return ClimateClass::Mediterranean;
        }
        return if t_warm >= 22.0 {
            ClimateClass::HumidSubtropical
        } else {
            ClimateClass::Oceanic
        };
    }
    // Continental climates: boreal where summers are short.
    if t_warm >= 17.5 {
        ClimateClass::HumidContinental
    } else {
        ClimateClass::Subarctic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_examples() {
        // Rough real-world climates.
        assert_eq!(
            classify(26.5, 2.0, 2500.0, 0),
            ClimateClass::TropicalRainforest
        ); // Manaus
        assert_eq!(
            classify(27.0, 5.0, 1100.0, 1),
            ClimateClass::TropicalSavanna
        );
        assert_eq!(classify(25.0, 15.0, 30.0, 0), ClimateClass::HotDesert); // Sahara
        assert_eq!(classify(11.0, 13.0, 600.0, 0), ClimateClass::Oceanic); // London
        assert_eq!(classify(17.5, 14.0, 500.0, 2), ClimateClass::Mediterranean); // Rome-ish
        assert_eq!(
            classify(5.8, 27.0, 700.0, 0),
            ClimateClass::HumidContinental
        ); // Moscow
        assert_eq!(classify(-2.0, 38.0, 450.0, 0), ClimateClass::Subarctic);
        assert_eq!(classify(-12.0, 25.0, 250.0, 0), ClimateClass::Tundra);
        assert_eq!(classify(-30.0, 25.0, 100.0, 0), ClimateClass::IceCap);
        assert_eq!(classify(8.0, 30.0, 120.0, 0), ClimateClass::ColdDesert);
        assert_eq!(classify(8.0, 30.0, 200.0, 0), ClimateClass::ColdSteppe);
    }

    #[test]
    fn winds_by_band() {
        assert!(prevailing_wind(15.0) < -0.9, "trade winds blow west");
        assert!(prevailing_wind(-45.0) > 0.9, "westerlies blow east");
        assert!(prevailing_wind(75.0) < -0.9, "polar easterlies");
    }

    #[test]
    fn run_distance_wraps() {
        let row = [false, true, true, true, false, true];
        let east = run_distance(&row, 1);
        assert_eq!(east[1], 3); // land at index 4 is three cells east
        let west = run_distance(&row, -1);
        assert_eq!(west[1], 1);
        assert_eq!(west[5], 1);
        assert_eq!(east[5], 1); // wraps to index 0
    }
}

//! Statistical checks over many seeds: the planet model must reproduce Earth's recognisable
//! climate and hypsometry patterns (spec §14, M2).

use hearth_math::PlanetSize;

use super::PlanetGrid;
use super::climate::ClimateClass;
use super::fields::row_area_weights;
use crate::settings::WorldGenSettings;

fn planets() -> Vec<PlanetGrid> {
    (1..=6u64)
        .map(|seed| {
            let s = WorldGenSettings {
                seed,
                planet_size: PlanetSize::Standard,
                grid_resolution: 256,
                ..WorldGenSettings::default()
            };
            PlanetGrid::build(&s, &|_, _| {})
        })
        .collect()
}

/// Area-weighted share of a class's land area whose |latitude| lies in [lo, hi].
fn share_in_band(
    planets: &[PlanetGrid],
    class: ClimateClass,
    lo: f64,
    hi: f64,
    max_elev: f32,
) -> f64 {
    let (mut inside, mut total) = (0.0, 0.0);
    for g in planets {
        let w = row_area_weights(&g.geom);
        let n = g.n();
        for idx in 0..n * n {
            if g.climate[idx] != class as u8 || g.elevation.data[idx] > max_elev {
                continue;
            }
            let j = idx / n;
            let lat = g.geom.lat[j].to_degrees().abs();
            total += w[j];
            if (lo..=hi).contains(&lat) {
                inside += w[j];
            }
        }
    }
    if total == 0.0 { 1.0 } else { inside / total }
}

#[test]
fn climate_zones_sit_where_they_do_on_earth() {
    let ps = planets();
    let hot_desert = share_in_band(&ps, ClimateClass::HotDesert, 12.0, 38.0, f32::MAX);
    let rainforest = share_in_band(&ps, ClimateClass::TropicalRainforest, 0.0, 13.0, f32::MAX);
    let boreal = share_in_band(&ps, ClimateClass::Subarctic, 44.0, 72.0, f32::MAX);
    let ice = share_in_band(&ps, ClimateClass::IceCap, 64.0, 90.0, 1500.0);
    assert!(
        hot_desert >= 0.6,
        "hot deserts in the subtropics: {hot_desert:.2}"
    );
    assert!(
        rainforest >= 0.7,
        "rainforest near the equator: {rainforest:.2}"
    );
    assert!(
        boreal >= 0.7,
        "boreal forest at high mid-latitudes: {boreal:.2}"
    );
    assert!(ice >= 0.7, "lowland ice caps toward the poles: {ice:.2}");
}

#[test]
fn deserts_lie_west_and_inland_humid_subtropics_east() {
    let ps = planets();
    // Position of a land cell within its land run along the row: 0 = west coast, 1 = east coast.
    let (mut desert_pos, mut desert_n) = (0.0f64, 0.0f64);
    let (mut humid_pos, mut humid_n) = (0.0f64, 0.0f64);
    for g in &ps {
        let n = g.n();
        for j in 0..n {
            let lat = g.geom.lat[j].to_degrees().abs();
            if !(15.0..=35.0).contains(&lat) {
                continue;
            }
            let row = &g.climate[j * n..(j + 1) * n];
            let land: Vec<bool> = row
                .iter()
                .map(|c| *c != ClimateClass::Ocean as u8)
                .collect();
            if land.iter().all(|l| *l) || land.iter().all(|l| !*l) {
                continue;
            }
            // Walk runs of land (wrapping).
            let start = (0..n).find(|&i| !land[i]).unwrap_or(0);
            let mut i = 0;
            while i < n {
                let a = (start + i) % n;
                if !land[a] {
                    i += 1;
                    continue;
                }
                let mut len = 0;
                while i + len < n && land[(start + i + len) % n] {
                    len += 1;
                }
                if len >= 12 {
                    for k in 0..len {
                        let c = row[(start + i + k) % n];
                        let pos = (k as f64 + 0.5) / len as f64;
                        if c == ClimateClass::HotDesert as u8 || c == ClimateClass::ColdDesert as u8
                        {
                            desert_pos += pos;
                            desert_n += 1.0;
                        } else if c == ClimateClass::HumidSubtropical as u8 {
                            humid_pos += pos;
                            humid_n += 1.0;
                        }
                    }
                }
                i += len;
            }
        }
    }
    let d = desert_pos / desert_n.max(1.0);
    let h = humid_pos / humid_n.max(1.0);
    assert!(d < 0.6, "deserts sit west/inland: mean position {d:.2}");
    assert!(
        h > 0.5,
        "humid subtropics sit on east coasts: mean position {h:.2}"
    );
    assert!(h > d, "humid east of deserts: {h:.2} vs {d:.2}");
}

#[test]
fn windward_coasts_are_wetter_than_leeward_interiors() {
    let ps = planets();
    let (mut coast, mut coast_n, mut inland, mut inland_n) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for g in &ps {
        let n = g.n();
        for j in 0..n {
            let lat = g.geom.lat[j].to_degrees().abs();
            if !(40.0..=58.0).contains(&lat) {
                continue;
            }
            // Westerlies: distance (cells) to the ocean on the upwind (west) side.
            let row = &g.climate[j * n..(j + 1) * n];
            let mut run = usize::MAX;
            let start = (0..n).find(|&i| row[i] == ClimateClass::Ocean as u8);
            let Some(start) = start else { continue };
            for k in 0..2 * n {
                let i = (start + k) % n;
                if row[i] == ClimateClass::Ocean as u8 {
                    run = 0;
                    continue;
                }
                run = run.saturating_add(1);
                if k < n {
                    continue;
                }
                let p = g.field_at(&g.precipitation, j * n + i) as f64;
                if run <= 3 {
                    coast += p;
                    coast_n += 1.0;
                } else if run >= 30 {
                    inland += p;
                    inland_n += 1.0;
                }
            }
        }
    }
    let c = coast / coast_n.max(1.0);
    let i = inland / inland_n.max(1.0);
    assert!(
        c > i * 1.2,
        "windward coast {c:.0} mm vs interior {i:.0} mm"
    );
}

#[test]
fn hypsometry_matches_the_vertical_profile() {
    let ps = planets();
    let mut land = Vec::new();
    let mut abyss = Vec::new();
    let mut max_land = Vec::new();
    let mut deepest = Vec::new();
    for g in &ps {
        let v = g.vertical_scale as f32;
        let mut peak = f32::MIN;
        let mut low = f32::MAX;
        for (idx, e) in g.elevation.data.iter().enumerate() {
            let j = idx / g.n();
            if g.geom.lat[j].to_degrees().abs() > 78.0 {
                continue; // polar caps
            }
            let b = e * v;
            if *e > 0.0 {
                land.push(b);
                peak = peak.max(b);
            } else if *e < -2000.0 {
                abyss.push(b);
            }
            low = low.min(b);
        }
        max_land.push(peak);
        deepest.push(low);
    }
    land.sort_by(f32::total_cmp);
    abyss.sort_by(f32::total_cmp);
    let median_land = land[land.len() / 2];
    let median_abyss = abyss[abyss.len() / 2];
    let high = land.iter().filter(|h| **h > 1000.0).count() as f64 / land.len() as f64;
    assert!(
        median_land < 120.0,
        "most land below Y 120: median {median_land}"
    );
    assert!(
        (-1150.0..=-750.0).contains(&median_abyss),
        "abyssal floor median {median_abyss}"
    );
    assert!(high < 0.02, "great heights are rare: {high:.3}");
    let tall = max_land.iter().filter(|m| **m > 900.0).count();
    assert!(tall >= ps.len() / 2, "great peaks present: {max_land:?}");
    let trenches = deepest.iter().filter(|d| **d < -2000.0).count();
    assert!(trenches >= ps.len() / 2, "trenches present: {deepest:?}");
}

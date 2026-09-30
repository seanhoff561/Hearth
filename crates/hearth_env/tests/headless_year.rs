//! V2-1 acceptance: simulate a year headlessly on a generated planet and check that snow, lake
//! ice and wet/dry seasons arrive at the right times at each latitude, in both hemispheres.

use hearth_env::climate::{Normals, SeasonalCover};
use hearth_env::weather::{Precip, WeatherModel};
use hearth_math::PlanetSize;
use hearth_worldgen::{PlanetGrid, WorldGenSettings};

struct Place {
    x: f64,
    z: f64,
    n: Normals,
}

/// Land cells of the planet with their normals.
fn land_places(grid: &PlanetGrid) -> Vec<Place> {
    let n = grid.geom.n;
    let mut out = Vec::new();
    for j in (0..n).step_by(3) {
        for i in (0..n).step_by(3) {
            let idx = grid.geom.idx(i, j);
            if grid.elevation.at_index(idx, n) <= 0.0 {
                continue;
            }
            let (x, z) = grid.geom.world_xz(i, j);
            let place = Normals::sample(grid, x, z);
            if place.lat_deg.abs() < 80.0 {
                out.push(Place { x, z, n: place });
            }
        }
    }
    out
}

/// Year fraction for a *local* season time (0 = local spring equinox).
fn local(n: &Normals, f: f64) -> f64 {
    if n.lat_deg < 0.0 {
        (f + 0.5).fract()
    } else {
        f
    }
}

#[test]
fn a_year_of_seasons_by_latitude() {
    let settings = WorldGenSettings {
        seed: 11,
        planet_size: PlanetSize::Small,
        grid_resolution: 256,
        ..WorldGenSettings::default()
    };
    let grid = PlanetGrid::build(&settings, &|_, _| {});
    let places = land_places(&grid);
    assert!(places.len() > 500, "{} land samples", places.len());

    // --- Snow and ice: cold continental places have them in late winter, never in summer.
    let cold: Vec<&Place> = places
        .iter()
        .filter(|p| p.n.t_mean < -1.0 && p.n.t_range > 20.0)
        .collect();
    assert!(cold.len() > 20, "{} cold continental samples", cold.len());
    let (mut winter_snow, mut summer_snow, mut winter_ice) = (0, 0, 0);
    let mut hemispheres = [0usize; 2];
    for p in &cold {
        let cover = SeasonalCover::compute(&p.n);
        if cover.perennial {
            continue;
        }
        hemispheres[usize::from(p.n.lat_deg < 0.0)] += 1;
        if cover.snow_depth_m(local(&p.n, 0.95)) > 0.1 {
            winter_snow += 1;
        }
        if cover.snow_depth_m(local(&p.n, 0.4)) > 0.0 {
            summer_snow += 1;
        }
        if cover.ice_m(local(&p.n, 0.95)) > 0.3 {
            winter_ice += 1;
        }
    }
    let seasonal = cold.len().max(1);
    assert!(
        winter_snow * 10 >= seasonal * 7,
        "snow in late winter at {winter_snow}/{seasonal}"
    );
    assert!(
        summer_snow * 20 <= seasonal,
        "snow in summer at {summer_snow}/{seasonal}"
    );
    assert!(
        winter_ice * 10 >= seasonal * 7,
        "thick ice in late winter at {winter_ice}/{seasonal}"
    );

    // --- Warm places never see snow or ice.
    for p in places.iter().filter(|p| p.n.t_mean > 20.0) {
        let cover = SeasonalCover::compute(&p.n);
        for k in 0..12 {
            let f = k as f64 / 12.0;
            assert_eq!(cover.snow_depth_m(f), 0.0, "snow at t_mean {}", p.n.t_mean);
            assert_eq!(cover.ice_m(f), 0.0);
        }
    }

    // --- Wet and dry seasons in the tropics follow the local summer, opposite across the
    // equator.
    let savanna: Vec<&Place> = places
        .iter()
        .filter(|p| p.n.winter_dry > 0.5 && p.n.lat_deg.abs() > 5.0 && p.n.lat_deg.abs() < 25.0)
        .collect();
    assert!(savanna.len() > 10, "{} savanna samples", savanna.len());
    let mut correct = 0;
    let mut by_hemisphere = [0usize; 2];
    for p in &savanna {
        let wet = (0..10)
            .map(|k| p.n.precip_weight(local(&p.n, 0.2 + 0.025 * k as f64)))
            .sum::<f64>();
        let dry = (0..10)
            .map(|k| p.n.precip_weight(local(&p.n, 0.7 + 0.025 * k as f64)))
            .sum::<f64>();
        if wet > 2.0 * dry {
            correct += 1;
            by_hemisphere[usize::from(p.n.lat_deg < 0.0)] += 1;
        }
    }
    assert!(
        correct * 10 >= savanna.len() * 9,
        "wet summers at {correct}/{}",
        savanna.len()
    );

    // --- Day-scale weather over the year: winter precipitation in cold places falls as snow,
    // summer precipitation as rain.
    let model = WeatherModel::new(settings.seed, *grid.planet());
    // Seasonal climates only (warmest month above 10 °C); in polar climates summer snow is real.
    let seasonal_cold: Vec<&Place> = cold
        .iter()
        .copied()
        .filter(|p| p.n.t_mean + p.n.t_range / 2.0 > 10.0)
        .collect();
    assert!(
        seasonal_cold.len() > 5,
        "{} seasonal cold samples",
        seasonal_cold.len()
    );
    let sample: Vec<&Place> = seasonal_cold
        .iter()
        .copied()
        .step_by((seasonal_cold.len() / 12).max(1))
        .collect();
    let (mut snow_in_winter, mut precip_in_winter, mut snow_in_summer, mut precip_in_summer) =
        (0, 0, 0, 0);
    for p in &sample {
        for day in 0..365 {
            let yf = day as f64 / 365.2422;
            let lf = local(&p.n, yf);
            for hour in [3.0, 9.0, 15.0, 21.0] {
                let w = model.sample(&p.n, p.x, p.z, day as f64 + hour / 24.0, yf, hour / 24.0);
                if w.precip == Precip::None {
                    continue;
                }
                let winter = (0.8..0.95).contains(&lf);
                let summer = (0.3..0.45).contains(&lf);
                if winter {
                    precip_in_winter += 1;
                    snow_in_winter += usize::from(w.precip == Precip::Snow);
                }
                if summer {
                    precip_in_summer += 1;
                    snow_in_summer += usize::from(w.precip == Precip::Snow);
                }
            }
        }
    }
    assert!(precip_in_winter > 0 && precip_in_summer > 0);
    assert!(
        snow_in_winter * 10 >= precip_in_winter * 8,
        "winter snow {snow_in_winter}/{precip_in_winter}"
    );
    assert!(
        snow_in_summer * 10 <= precip_in_summer,
        "summer snow {snow_in_summer}/{precip_in_summer}"
    );
    eprintln!(
        "cold samples {} (N {}, S {}), savanna {} (N {}, S {} correct)",
        cold.len(),
        hemispheres[0],
        hemispheres[1],
        savanna.len(),
        by_hemisphere[0],
        by_hemisphere[1]
    );
}

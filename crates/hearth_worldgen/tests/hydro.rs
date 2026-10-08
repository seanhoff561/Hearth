//! Groundwater, springs and water quality (v2 §5.4): the water table is a subdued copy of the
//! land, deeper under hills, in dry country and in karst; voids below it are full of water;
//! springs rise where it meets the land and their brooks run downhill; natural water has the
//! quality of its body.

use std::sync::{Arc, OnceLock};

use hearth_content::Content;
use hearth_math::{CubePos, LocalPos, PlanetSize};
use hearth_worldgen::hydro::{CELL, SpringKind, Taste};
use hearth_worldgen::{PlanetGrid, Terrain, WorldGenSettings, WorldGenerator};
use rayon::prelude::*;

fn content() -> &'static Content {
    static C: OnceLock<Content> = OnceLock::new();
    C.get_or_init(Content::load_base)
}

fn generator() -> &'static WorldGenerator {
    static G: OnceLock<WorldGenerator> = OnceLock::new();
    G.get_or_init(|| {
        let s = WorldGenSettings {
            seed: 11,
            planet_size: PlanetSize::Standard,
            grid_resolution: 512,
        };
        let terrain = Arc::new(Terrain::new(Arc::new(PlanetGrid::build(&s, &|_, _| {}))));
        let reg = hearth_world::datapack::load_builtin_registry().expect("base pack");
        WorldGenerator::new(terrain, &reg, content()).expect("all generator blocks exist")
    })
}

/// Land columns on a coarse lattice over the planet (skipping the poles).
fn land_columns(step: i32) -> Vec<(i32, i32)> {
    let wg = generator();
    let c = wg.planet().circumference();
    (-c / 3..c / 3)
        .step_by(step as usize)
        .collect::<Vec<_>>()
        .into_par_iter()
        .flat_map_iter(|z| {
            (0..c)
                .step_by(step as usize)
                .filter(move |&x| !wg.terrain.sample(x, z).is_underwater())
                .map(move |x| (x, z))
        })
        .collect()
}

#[test]
fn water_table_is_a_subdued_copy_of_the_land() {
    let wg = generator();
    let cols = land_columns(197);
    assert!(cols.len() > 2000, "{} land columns", cols.len());
    // (depth to the table, height above sea, wetness proxy)
    let samples: Vec<(f32, f32, f32)> = cols
        .par_iter()
        .map(|&(x, z)| {
            let s = wg.terrain.sample(x, z);
            let t = wg.hydro.water_table(wg, x, z);
            assert!(
                t < s.height,
                "table above the ground at {x},{z}: {t} vs {}",
                s.height
            );
            (
                s.height - t,
                s.height,
                s.precipitation - 20.0 * s.temperature.max(0.0),
            )
        })
        .collect();
    let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len().max(1) as f32;
    let depth_where = |f: &dyn Fn(&(f32, f32, f32)) -> bool| {
        let v: Vec<f32> = samples.iter().filter(|s| f(s)).map(|s| s.0).collect();
        (mean(&v), v.len())
    };
    let (wet, n_wet) = depth_where(&|s| s.2 > 900.0);
    let (dry, n_dry) = depth_where(&|s| s.2 < 250.0);
    assert!(
        n_wet > 100 && n_dry > 100,
        "{n_wet} wet and {n_dry} dry columns"
    );
    assert!(
        dry > wet * 2.0,
        "dry country has deeper water: {dry:.1} vs {wet:.1} blocks"
    );
    let (low, _) = depth_where(&|s| s.1 < 20.0 && s.2 > 600.0);
    let (high, _) = depth_where(&|s| s.1 > 150.0 && s.2 > 600.0);
    assert!(
        high > low,
        "deeper under high ground: {high:.1} vs {low:.1}"
    );
    let mut wet_low: Vec<f32> = samples
        .iter()
        .filter(|s| s.2 > 900.0 && s.1 < 60.0)
        .map(|s| s.0)
        .collect();
    wet_low.sort_by(f32::total_cmp);
    let median = wet_low[wet_low.len() / 2];
    assert!(
        median < 4.0,
        "humid lowland tables are shallow: median {median:.1} blocks"
    );
}

#[test]
fn water_table_meets_surface_water() {
    let wg = generator();
    let c = wg.planet().circumference();
    let mut checked = 0;
    for z in (-c / 4..c / 4).step_by(313) {
        for x in (0..c).step_by(313) {
            let s = wg.terrain.sample(x, z);
            if s.is_underwater() {
                assert_eq!(wg.hydro.water_table(wg, x, z), s.water);
                checked += 1;
            }
        }
    }
    assert!(checked > 100);
}

/// No dry void lies below the water table: caves, ravines and caverns there hold water.
#[test]
fn voids_below_the_water_table_are_flooded() {
    let wg = generator();
    let cols = land_columns(1531);
    let (air, water) = cols
        .par_iter()
        .map(|&(x, z)| {
            let s = wg.terrain.sample(x, z);
            let table = wg.hydro.water_table(wg, x, z).floor() as i32;
            let top = table.min(s.height_i() - 12) - 1;
            let (mut air, mut water) = (0usize, 0usize);
            let mut cube = None;
            for y in (top - 160..=top).rev() {
                let pos = CubePos::new(x >> 4, y >> 4, z >> 4);
                if cube.as_ref().is_none_or(|(p, _)| *p != pos) {
                    cube = Some((pos, wg.generate_cube(pos)));
                }
                let (_, c) = cube.as_ref().expect("cube");
                let st = c.get(LocalPos::new(
                    (x & 15) as u8,
                    (y & 15) as u8,
                    (z & 15) as u8,
                ));
                if st.is_air() {
                    air += 1;
                } else if st == wg.blocks.water {
                    water += 1;
                }
            }
            (air, water)
        })
        .reduce(|| (0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
    assert_eq!(
        air, 0,
        "{air} dry blocks below the water table ({water} flooded)"
    );
    assert!(water > 0, "some flooded voids");
}

#[test]
fn springs_rise_where_the_table_meets_the_land() {
    let wg = generator();
    let n = wg.deposits.cells_around();
    let springs: Vec<_> = (-n / 3..n / 3)
        .into_par_iter()
        .flat_map_iter(|cz| {
            (0..n).flat_map(move |cx| {
                wg.hydro
                    .cell(wg, cx, cz)
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let fresh = springs
        .iter()
        .filter(|s| s.kind == SpringKind::Fresh)
        .count();
    let mineral = springs.len() - fresh;
    assert!(fresh > 200, "{fresh} fresh springs");
    assert!(mineral > 5, "{mineral} mineral springs");
    for sp in springs
        .iter()
        .filter(|s| s.kind == SpringKind::Fresh)
        .take(400)
    {
        let s = wg.terrain.sample(sp.x, sp.z);
        assert!(
            !s.is_underwater(),
            "a spring under water at {},{}",
            sp.x,
            sp.z
        );
        // Brooks run downhill.
        let mut last = sp.level;
        for &(_, _, y) in &sp.brook {
            assert!(y <= last, "a brook runs uphill near {},{}", sp.x, sp.z);
            last = y;
        }
    }
    // A spring's pool holds water in the generated world.
    let sp = springs
        .iter()
        .find(|s| s.kind == SpringKind::Fresh)
        .expect("a spring");
    let y = sp.level - 1;
    let cube = wg.generate_cube(CubePos::new(sp.x >> 4, y >> 4, sp.z >> 4));
    let st = cube.get(LocalPos::new(
        (sp.x & 15) as u8,
        (y & 15) as u8,
        (sp.z & 15) as u8,
    ));
    assert_eq!(
        st, wg.blocks.water,
        "the eye of the spring at {},{},{}",
        sp.x, y, sp.z
    );
    let q = wg.hydro.quality(wg, sp.x, y, sp.z).expect("spring water");
    assert!(q.pathogen_risk < 0.05 && q.salinity_g_l < 1.0, "{q:?}");
    let _ = CELL;
}

#[test]
fn natural_water_has_the_quality_of_its_body() {
    let wg = generator();
    let c = wg.planet().circumference();
    let (mut sea, mut river, mut ground) = (None, None, None);
    for z in (-c / 4..c / 4).step_by(97) {
        for x in (0..c).step_by(389) {
            let s = wg.terrain.sample(x, z);
            if sea.is_none() && s.ocean && s.water_i() - s.height_i() > 3 {
                sea = wg.hydro.quality(wg, x, s.water_i() - 1, z);
            }
            if river.is_none()
                && !s.ocean
                && !s.lake
                && s.river.is_some_and(|r| r.distance < r.width * 0.3)
                && s.is_underwater()
            {
                river = wg.hydro.quality(wg, x, s.water_i() - 1, z);
            }
            if ground.is_none() && !s.is_underwater() && s.precipitation > 800.0 {
                let t = wg.hydro.water_table(wg, x, z);
                ground = wg.hydro.quality(wg, x, t as i32 - 3, z);
                assert!(
                    wg.hydro.quality(wg, x, s.height_i() + 2, z).is_none(),
                    "air is not water"
                );
            }
        }
    }
    let sea = sea.expect("sea water");
    assert!(
        sea.salinity_g_l >= 10.0 && sea.taste == Taste::Salty,
        "{sea:?}"
    );
    let river = river.expect("river water");
    assert!(
        river.salinity_g_l < 1.0 && river.pathogen_risk < 0.6,
        "{river:?}"
    );
    let ground = ground.expect("groundwater");
    assert!(
        ground.salinity_g_l < 1.0 && ground.pathogen_risk < 0.05,
        "{ground:?}"
    );
}

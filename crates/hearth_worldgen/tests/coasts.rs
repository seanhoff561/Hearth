//! Coasts (v2 §5.5): coral reefs only in warm, clear, shallow sea; mangroves on low sheltered
//! tropical coasts and salt marsh on those of cooler climates; mud in sheltered shallows; kelp
//! and seaweed in cool water.

use std::sync::{Arc, OnceLock};

use hearth_content::Content;
use hearth_math::{CubePos, LocalPos, PlanetSize};
use hearth_worldgen::region::biome::Biome;
use hearth_worldgen::{PlanetGrid, Surface, Terrain, WorldGenSettings, WorldGenerator};
use rayon::prelude::*;

fn generator() -> &'static WorldGenerator {
    static G: OnceLock<WorldGenerator> = OnceLock::new();
    G.get_or_init(|| {
        let s = WorldGenSettings {
            seed: 11,
            planet_size: PlanetSize::Standard,
            grid_resolution: 512,
            ..WorldGenSettings::default()
        };
        let terrain = Arc::new(Terrain::new(Arc::new(PlanetGrid::build(&s, &|_, _| {}))));
        let reg = hearth_world::datapack::load_builtin_registry().expect("base pack");
        WorldGenerator::new(terrain, &reg, &Content::load_base())
            .expect("all generator blocks exist")
    })
}

/// Samples of the planet's columns on a lattice.
fn lattice(step: i32) -> Vec<(i32, i32, hearth_worldgen::ColumnSample)> {
    let wg = generator();
    let c = wg.planet().circumference();
    (-c / 2 + step..c / 2 - step)
        .step_by(step as usize)
        .collect::<Vec<_>>()
        .into_par_iter()
        .flat_map_iter(|z| {
            (0..c)
                .step_by(step as usize)
                .map(move |x| (x, z, wg.terrain.sample(x, z)))
        })
        .collect()
}

#[test]
fn reefs_grow_in_warm_clear_shallow_sea() {
    let cols = lattice(41);
    let reefs: Vec<_> = cols
        .iter()
        .filter(|c| c.2.surface == Surface::Coral)
        .collect();
    assert!(reefs.len() > 50, "only {} reef columns", reefs.len());
    for (x, z, s) in &reefs {
        assert!(
            s.ocean && s.sea_temperature > 21.0,
            "a reef in cool water at {x},{z}: {:?}",
            s.sea_temperature
        );
        assert!(
            s.water_i() - s.height_i() <= 4,
            "a reef top deep under water at {x},{z}"
        );
    }
    // Warm shallow sea is not all reef: lagoons and sandy floors between.
    let warm_shallow = cols
        .iter()
        .filter(|c| c.2.ocean && c.2.sea_temperature > 21.0 && c.2.water_i() - c.2.height_i() <= 6)
        .count();
    assert!(
        reefs.len() < warm_shallow,
        "{} reefs of {warm_shallow} warm shallows",
        reefs.len()
    );
}

#[test]
fn low_sheltered_coasts_are_marsh_or_mangrove() {
    let cols = lattice(23);
    let mangrove: Vec<_> = cols
        .iter()
        .filter(|c| c.2.biome == Biome::Mangrove)
        .collect();
    let marsh: Vec<_> = cols
        .iter()
        .filter(|c| c.2.biome == Biome::SaltMarsh)
        .collect();
    assert!(mangrove.len() > 20, "{} mangrove columns", mangrove.len());
    assert!(marsh.len() > 20, "{} salt marsh columns", marsh.len());
    for (x, z, s) in &mangrove {
        assert!(
            s.temperature > 20.0,
            "mangroves in the cold at {x},{z}: {}",
            s.temperature
        );
        assert!(s.height < 1.5, "mangroves up the hill at {x},{z}");
        assert_eq!(s.surface, Surface::Dirt, "mangroves stand in mud");
    }
    for (x, z, s) in &marsh {
        assert!(
            s.height < 1.5 && s.slope < 0.05,
            "salt marsh off the flats at {x},{z}"
        );
        assert!(
            s.temperature <= 20.0 || s.sea_temperature <= 22.0,
            "salt marsh in the tropics at {x},{z}"
        );
    }
    // Mangrove trees stand in the generated world.
    let wg = generator();
    let found = mangrove.iter().take(60).any(|(x, z, s)| {
        (s.height_i()..s.height_i() + 12).any(|y| {
            let cube = wg.generate_cube(CubePos::new(x >> 4, y >> 4, z >> 4));
            (0..16).any(|lz| {
                (0..16).any(|lx| {
                    let st = cube.get(LocalPos::new(lx, (y & 15) as u8, lz));
                    st == wg.blocks.mangrove.log_y || wg.blocks.mangrove_roots.contains(&st)
                })
            })
        })
    });
    assert!(found, "no mangrove trees near the mangrove columns");
}

#[test]
fn kelp_and_seaweed_keep_to_cool_water() {
    let wg = generator();
    let cols = lattice(211);
    let (kelp_cool, kelp_elsewhere) = cols
        .par_iter()
        .filter(|c| c.2.ocean && (3..30).contains(&(c.2.water_i() - c.2.height_i())))
        .map(|(x, z, s)| {
            let y = s.height_i();
            let cube = wg.generate_cube(CubePos::new(x >> 4, y >> 4, z >> 4));
            let st = cube.get(LocalPos::new(
                (x & 15) as u8,
                (y & 15) as u8,
                (z & 15) as u8,
            ));
            let kelp = st == wg.blocks.kelp_plant || st == wg.blocks.kelp;
            let cool = (5.0..20.0).contains(&s.sea_temperature);
            (usize::from(kelp && cool), usize::from(kelp && !cool))
        })
        .reduce(|| (0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
    assert!(kelp_cool > 0, "some kelp in cool seas");
    assert_eq!(kelp_elsewhere, 0, "kelp only in cool water");
}

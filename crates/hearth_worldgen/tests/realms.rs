//! Biogeographic realms (V2-7, v2 §7.4): the landmasses of a world are grouped into realms by
//! isolation and climate, and the trees of a realm are its own.

use std::sync::Arc;

use hearth_math::PlanetSize;
use hearth_worldgen::realms::{Realm, Realms};
use hearth_worldgen::{PlanetGrid, Terrain, WorldGenSettings};

fn terrain(seed: u64, size: PlanetSize) -> Terrain {
    let s = WorldGenSettings {
        seed,
        planet_size: size,
        grid_resolution: 256,
        ..WorldGenSettings::default()
    };
    Terrain::new(Arc::new(PlanetGrid::build(&s, &|_, _| {})))
}

#[test]
fn continents_are_zoned_into_realms_by_latitude_and_size() {
    for (seed, size) in [
        (7u64, PlanetSize::Standard),
        (7, PlanetSize::Tiny),
        (11, PlanetSize::Standard),
        (23, PlanetSize::Standard),
    ] {
        let t = terrain(seed, size);
        let r = &t.realms;
        let names: Vec<String> = r
            .areas
            .iter()
            .map(|(realm, a)| format!("{} {:.0} km²", realm.name(), a))
            .collect();
        let continents = r.landmasses.iter().filter(|l| l.continent).count();
        println!(
            "seed {seed}: {continents} continents, {} islands; {}",
            r.landmasses.len() - continents,
            names.join(", ")
        );
        assert!(continents >= 1);
        // The largest landmass is the Old World: north of the tropics, the Palearctic.
        let biggest = r
            .landmasses
            .iter()
            .map(|l| l.area_km2)
            .fold(0.0f64, f64::max);
        let (sx, sz) = t.find_spawn(false);
        let here = r.realm_at(sx as f64, sz as f64);
        let lat = t.planet().latitude_deg(sz as f64 + 0.5);
        println!("  spawn at latitude {lat:.1}°: {}", here.name());
        for (k, l) in r.landmasses.iter().enumerate().filter(|(_, l)| l.continent) {
            println!("  continent {k}: {:.0} km²", l.area_km2);
        }
        println!(
            "  spawn's landmass: {:?}",
            r.landmass_at(sx as f64, sz as f64)
                .map(|l| l.area_km2 as i64)
        );
        let home = r
            .landmass_at(sx as f64, sz as f64)
            .map_or(0.0, |l| l.area_km2);
        if lat >= 23.5 && home >= biggest {
            assert_eq!(here, Realm::Palearctic, "seed {seed}");
        }
        // Every land column of a continent has a realm consistent with its latitude.
        let c = t.planet().circumference() as f64;
        let mut checked = 0;
        for k in 0..400 {
            let x = (k as f64 * 0.618_033_988_75).fract() * c;
            let z = ((k as f64 * 0.414_213_562_37).fract() - 0.5) * c * 0.8;
            let s = t.sample(x as i32, z as i32);
            if s.ocean || !r.landmass_at(x, z).is_some_and(|l| l.continent) {
                continue;
            }
            let lat = t.planet().latitude_deg(z);
            let realm = s.realm;
            let ok = if lat >= 23.5 {
                matches!(realm, Realm::Palearctic | Realm::Nearctic)
            } else if lat > -23.5 {
                matches!(
                    realm,
                    Realm::Afrotropical | Realm::Indomalayan | Realm::Neotropical
                )
            } else if lat >= -60.0 {
                matches!(
                    realm,
                    Realm::Afrotropical
                        | Realm::Indomalayan
                        | Realm::Neotropical
                        | Realm::Australasian
                )
            } else {
                realm == Realm::Antarctic
            };
            // Columns within a grid cell of a zone's edge may read the neighbour's realm.
            let edge = (lat.abs() - 23.5).abs() < 1.5 || (lat + 60.0).abs() < 1.5;
            assert!(
                ok || edge,
                "seed {seed}: {} at latitude {lat:.1}",
                realm.name()
            );
            checked += 1;
        }
        assert!(
            checked > 20,
            "seed {seed}: only {checked} land columns sampled"
        );
    }
}

#[test]
fn a_uniform_world_has_one_realm() {
    let r = Realms::uniform(Realm::Nearctic);
    assert_eq!(r.realm_at(1234.0, -567.0), Realm::Nearctic);
    assert!(!r.island_at(0.0, 0.0));
}

#[test]
fn realm_sets_read_the_species_lists() {
    use hearth_worldgen::realms::{native, set_of};
    let pal = set_of(&["Palearctic".to_string()]);
    assert!(native(pal, Realm::Palearctic));
    assert!(!native(pal, Realm::Nearctic));
    assert!(
        native(0, Realm::Australasian),
        "no realms listed: everywhere"
    );
    let both = set_of(&["Palearctic".into(), "Nearctic".into(), "Atlantis".into()]);
    assert!(native(both, Realm::Nearctic) && !native(both, Realm::Neotropical));
}

#[test]
fn each_realm_grows_its_own_trees() {
    use hearth_worldgen::WorldGenerator;
    use hearth_worldgen::realms::native;
    use hearth_worldgen::region::biome::Biome;
    use hearth_worldgen::vegetation::Vegetation;

    let s = WorldGenSettings {
        seed: 7,
        planet_size: PlanetSize::Standard,
        grid_resolution: 256,
        ..WorldGenSettings::default()
    };
    let terrain = Arc::new(Terrain::new(Arc::new(PlanetGrid::build(&s, &|_, _| {}))));
    let reg = hearth_world::datapack::load_builtin_registry().expect("base pack");
    let content = hearth_content::Content::load_base();
    let wg = WorldGenerator::new(terrain, &reg, &content).expect("generator blocks");
    let veg = Vegetation::default();
    let c = wg.planet().circumference();
    // Forest columns of each northern realm, on a coarse lattice over the planet.
    let mut counted: Vec<(Realm, usize, usize)> = Vec::new();
    for want in [Realm::Palearctic, Realm::Nearctic] {
        let (mut trees, mut natives) = (0, 0);
        'scan: for j in 0..64 {
            for i in 0..128 {
                let (x, z) = (i * c / 128, (j - 32) * c / 128);
                let col = wg.terrain.sample(x, z);
                if col.realm != want
                    || !matches!(
                        col.biome,
                        Biome::BroadleafForest | Biome::MixedForest | Biome::BirchForest
                    )
                    || col.tree_density < 0.15
                {
                    continue;
                }
                for t in wg
                    .features()
                    .trees_in(&wg, &veg, (x - 24, z - 24), (x + 24, z + 24))
                {
                    trees += 1;
                    if native(wg.forest.niches[t.species].realms, want) {
                        natives += 1;
                    }
                }
                if trees > 400 {
                    break 'scan;
                }
            }
        }
        println!("{}: {natives} of {trees} trees native", want.name());
        counted.push((want, trees, natives));
    }
    // The Palearctic is all its own; in the Nearctic, which has only three trees of its own so
    // far, Palearctic stand-ins grow where none of them suits the place.
    for (realm, trees, natives) in counted {
        assert!(trees >= 40, "{}: only {trees} trees found", realm.name());
        let share = if realm == Realm::Palearctic { 1.0 } else { 0.6 };
        assert!(
            natives as f32 >= share * trees as f32,
            "{}: only {natives} of {trees} trees are native",
            realm.name()
        );
    }
}

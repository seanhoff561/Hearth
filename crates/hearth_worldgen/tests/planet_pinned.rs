//! The planets made, pinned to the generator's version (`planet::GENERATOR`, part of the name a
//! planet is cached under): a change to how planets are made fails here until the version is
//! bumped and the new planets pinned, so that no machine goes on using a planet an older build
//! cached. Run on another system (Windows, macOS), it also tells whether its maths make the same
//! planet as Linux's.

use hearth_math::PlanetSize;
use hearth_worldgen::planet::GENERATOR;
use hearth_worldgen::{PlanetGrid, WorldGenSettings};

fn settings(seed: u64, planet_size: PlanetSize, grid_resolution: usize) -> WorldGenSettings {
    WorldGenSettings {
        seed,
        planet_size,
        grid_resolution,
    }
    .sanitized()
}

#[test]
fn the_planets_made_are_pinned_to_the_generators_version() {
    // (seed, size, grid resolution, the hash of all the planet holds)
    let pinned = [
        (77, PlanetSize::Small, 128, 0xacde_d7ac_bd6b_88d0),
        (7, PlanetSize::Earth, 256, 0xd468_057e_61c3_3845),
    ];
    let mut made = Vec::new();
    for (seed, size, n, hash) in pinned {
        let t = std::time::Instant::now();
        let got = PlanetGrid::build(&settings(seed, size, n), &|_, _| {}).content_hash();
        println!(
            "seed {seed} {size:?} {n}: {got:#018x} in {:.1} s",
            t.elapsed().as_secs_f64()
        );
        made.push((seed, size, n, got, hash));
    }
    assert_eq!(
        GENERATOR, 1,
        "pin the planets made by the new version below"
    );
    for (seed, size, n, got, hash) in made {
        assert_eq!(
            got, hash,
            "seed {seed}, {size:?}, {n}: the planet made has changed; bump \
             hearth_worldgen::planet::GENERATOR (the caches' names) and pin {got:#018x}"
        );
    }
}

#[test]
fn the_number_of_threads_does_not_change_the_planet() {
    let s = settings(77, PlanetSize::Small, 128);
    let hashes: Vec<u64> = [1, 4]
        .iter()
        .map(|&n| {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(n)
                .build()
                .expect("a pool");
            pool.install(|| PlanetGrid::build(&s, &|_, _| {}).content_hash())
        })
        .collect();
    assert_eq!(
        hashes[0], hashes[1],
        "one thread and four make the same planet"
    );
}

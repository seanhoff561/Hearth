//! The guard of E4.1 §4.1: the callers that work at a coarse scale (the globe's hovering, the
//! places' search across the planet, the animals' habitats, the distant LOD tiles) make no
//! tiles of the finest refinement level; the near field (the start spot) does, and is counted.

use hearth::scene::LocalWorld;
use hearth_core::prof;
use hearth_fauna::habitat::Land;
use hearth_math::PlanetSize;
use hearth_worldgen::relief::COARSE_CALLERS;

#[test]
fn coarse_callers_make_no_tiles_of_the_finest_level() {
    // An Earth-sized planet on a coarse grid (quick to make), with all its refinement levels.
    let lw = LocalWorld::create(5, PlanetSize::Earth, 256, None).expect("world");
    let terrain = lw.terrain();
    let finest = terrain.finest_level();
    assert!(finest >= 3, "refinement levels: {finest}");
    let fine = |caller: &str| prof::counter(&format!("relief.tile.L{finest}.{caller}"));
    let (sx, sz) = terrain.find_spawn(false);

    // The globe: hovering over points all over the planet.
    let mut rng = hearth_math::hash::Rng::new(5);
    for _ in 0..60 {
        let lat = (rng.next_f32() * 2.0 - 1.0).asin();
        let lon = (rng.next_f32() * 2.0 - 1.0) * std::f32::consts::PI;
        let _c = prof::caller("globe.hover");
        std::hint::black_box(hearth::globe::describe(terrain, lat, lon));
    }
    // The places suggested: the search across the planet, each candidate then looked at
    // closely where its life would begin (that may read the finest level).
    let finder =
        hearth::places::Finder::new(lw.generator.clone(), lw.content.clone(), lw.reg.clone());
    let places = finder.suggest(hearth::places::When::Spring, 5);
    assert!(!places.is_empty(), "places to begin");
    // The animals' habitats over a region about the spawn.
    let catalog = hearth_fauna::species::Catalog::new(&lw.content);
    let yields = hearth_fauna::habitat::TreeYields::new(&lw.generator, &lw.content);
    let land = hearth_fauna::habitat::GenLand {
        wg: &lw.generator,
        veg: &lw.vegetation,
        catalog: &catalog,
        trees: &yields,
    };
    let (ci, cj) = ((sx as i64).div_euclid(256), (sz as i64).div_euclid(256));
    for dj in -8..8 {
        for di in -8..8 {
            std::hint::black_box(land.habitat((ci + di, cj + dj)));
        }
    }
    // Distant LOD tiles over the spawn.
    let lod = hearth_lod::LodGen::new(&lw.reg, &hearth_texgen::textures_for(Some(&lw.content)));
    for level in 7..=hearth_lod::MAX_LEVEL {
        let size = hearth_lod::TILE << level;
        let key = hearth_lod::TileKey {
            level,
            x: sx.div_euclid(size),
            z: sz.div_euclid(size),
        };
        std::hint::black_box(lod.build(&lw.generator, key));
    }
    for caller in COARSE_CALLERS {
        assert_eq!(fine(caller), 0, "{caller} made tiles of the finest level");
        assert_eq!(
            prof::counter(&format!("relief.fine.{caller}")),
            0,
            "{caller}"
        );
    }
    // The near field makes them, and they are counted: a start spot settled block by block,
    // somewhere nothing has been read yet.
    {
        let _c = prof::caller("spawn");
        let x = lw.map.planet().wrap_x(sx + 400_000);
        std::hint::black_box(terrain.spawn_near(x, sz));
    }
    assert!(
        fine("spawn") > 0,
        "the counter counts the near field's tiles"
    );
}

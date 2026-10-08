//! Habitats from the generated world (V2-7): a temperate region near the spawn of seed 7 is
//! Palearctic, its woods feed the deer and its waters the fish, and its animals settle into
//! numbers the habitat holds.

use std::sync::Arc;
use std::time::Instant;

use hearth_fauna::ecology::{Ecology, REGION_CELLS};
use hearth_fauna::habitat::{CELL_M, GenLand, Land, TreeYields};
use hearth_fauna::species::{Catalog, Forage};
use hearth_math::PlanetSize;
use hearth_worldgen::realms::Realm;
use hearth_worldgen::vegetation::Vegetation;
use hearth_worldgen::{PlanetGrid, Terrain, WorldGenSettings, WorldGenerator};

#[test]
fn a_generated_temperate_region_feeds_its_animals() {
    let s = WorldGenSettings {
        seed: 7,
        planet_size: PlanetSize::Standard,
        grid_resolution: 256,
    };
    let terrain = Arc::new(Terrain::new(Arc::new(PlanetGrid::build(&s, &|_, _| {}))));
    let reg = hearth_world::datapack::load_builtin_registry().expect("base pack");
    let content = hearth_content::Content::load_base();
    let wg = WorldGenerator::new(terrain, &reg, &content).expect("generator blocks");
    let catalog = Arc::new(Catalog::new(&content));
    let trees = TreeYields::new(&wg, &content);
    let veg = Vegetation::default();
    let land = GenLand {
        wg: &wg,
        veg: &veg,
        catalog: &catalog,
        trees: &trees,
    };
    let (sx, sz) = wg.terrain.find_spawn(false);
    // The spawn's cell: land in a Palearctic temperate wood or near one.
    let cell = (
        (sx as f64 / CELL_M).floor() as i64,
        (sz as f64 / CELL_M).floor() as i64,
    );
    let h = land.habitat(cell);
    // On land (a coastal spawn's cell may be half sea).
    assert!(h.land >= 0.5, "the spawn is on land: {h:?}");
    assert_eq!(h.realm(), Realm::Palearctic);
    let mut eco = Ecology::new(catalog.clone(), 7, 0.0, &land);
    let key = eco.region_key(sx as f64, sz as f64);
    let t = Instant::now();
    eco.ensure_region(&land, key, 0.0);
    println!(
        "region {key:?} made in {:.1} s ({} cells)",
        t.elapsed().as_secs_f64(),
        REGION_CELLS * REGION_CELLS
    );
    let r = &eco.regions[&key];
    // The cells' production: grass in the open, browse and mast in the woods, invertebrates in
    // the water.
    let (mut open, mut wood, mut water) = (0, 0, 0);
    for h in &r.habitat {
        if h.fresh > 0.5 {
            water += 1;
            assert!(h.forage[Forage::Aquatic as usize] > 0.0);
        } else if h.land > 0.9 {
            if h.cover > 0.5 {
                wood += 1;
            } else {
                open += 1;
            }
        }
    }
    let sea = r.habitat.iter().filter(|h| h.sea > 0.5).count();
    let covers: Vec<f32> = r
        .habitat
        .iter()
        .filter(|h| h.land > 0.9)
        .map(|h| h.cover)
        .collect();
    let mean_cover = covers.iter().sum::<f32>() / covers.len().max(1) as f32;
    println!(
        "spawn ({sx}, {sz}): {wood} wooded cells, {open} open, {water} fresh water, {sea} sea;          mean cover {mean_cover:.2}"
    );
    let palearctic = r
        .habitat
        .iter()
        .filter(|h| h.land > 0.5 && h.fauna() == Realm::Palearctic)
        .count();
    println!("{palearctic} land cells of Palearctic animals");
    assert!(palearctic > 50);
    assert!(wood + open > 300, "the region holds land enough");
    assert!(water > 0, "and has fresh water");
    // The animals the region holds, and what it holds of them after a few years.
    eco.advance(5.0, 1.0 / 32.0);
    let r = &eco.regions[&key];
    for id in [
        "red_deer",
        "roe_deer",
        "wild_boar",
        "red_fox",
        "bank_vole",
        "brown_trout",
    ] {
        let s = catalog.index(id).expect(id);
        let (cap, n) = (eco.capacity(s), eco.count(s));
        println!("{id:<12} capacity {cap:>9.0} now {n:>9.0}");
        assert!(cap > 0.0, "{id} finds a home in the region");
        assert!(
            n > 0.05 * cap && n < 5.0 * cap + 5.0,
            "{id}: {n:.0} of {cap:.0}"
        );
    }
    // The Nearctic's animals live only on its land.
    let wtd = catalog
        .index("white_tailed_deer")
        .expect("white-tailed deer");
    for g in r.groups.iter().filter(|g| g.species as usize == wtd) {
        let c = r
            .cell_at(eco.cells_around, g.home[0], g.home[1])
            .expect("home");
        assert_eq!(r.habitat[c].fauna(), Realm::Nearctic);
    }
}

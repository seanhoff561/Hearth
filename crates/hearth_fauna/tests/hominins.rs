//! V2-11 (a): *Australopithecus* as a population of the ecological cells. Its groups live on for
//! fifty years in the savanna–woodland of Africa, India and South America (the savanna runs of
//! `tropical_biomes.rs` hold them); here, the Hominin range setting's single cradle keeps them to
//! Africa, and the population is the hominin's, drawn out near the player as its agents.

use std::sync::Arc;

use hearth_fauna::ecology::Ecology;
use hearth_fauna::habitat::{Uniform, reference_land};
use hearth_fauna::species::Catalog;
use hearth_worldgen::realms::Realm;

/// What a uniform savanna–woodland of a realm holds of the hominin, with a catalog.
fn hominins_held(catalog: Arc<Catalog>, realm: Realm) -> f64 {
    let e = catalog
        .ecosystems
        .iter()
        .find(|e| e.id.ends_with("savanna_woodland"))
        .expect("the savanna–woodland");
    let r = e.reference.as_ref().expect("its land");
    let land = Uniform {
        habitat: reference_land(&catalog, r, &e.id, realm),
        cells_around: 4096,
    };
    let mut eco = Ecology::new(catalog.clone(), 7, 0.0, &land);
    eco.ensure_region(&land, (3, 0), 0.0);
    let s = catalog
        .index("australopithecus")
        .expect("the hominin's population");
    eco.capacity(s)
}

#[test]
fn the_population_is_the_hominins() {
    let content = hearth_content::Content::load_base();
    let catalog = Catalog::new(&content);
    let s = catalog
        .get("australopithecus")
        .expect("the hominin's population");
    assert!(s.hominin, "its groups are the hominin's agents");
    assert!(!hearth_fauna::live::drawn(s), "not drawn as animals");
    let h = content
        .hominins
        .iter()
        .find(|h| h.id.ends_with("australopithecus"))
        .expect("the hominin");
    assert!(
        h.population
            .as_ref()
            .is_some_and(|p| p.as_str().ends_with("australopithecus")),
        "its population: {:?}",
        h.population
    );
    // The hunters that take them know them.
    let leopard = catalog.get("leopard").expect("the leopard");
    let i = catalog.index("australopithecus").expect("index");
    assert!(
        leopard.prey.iter().any(|(p, _)| *p == i),
        "the leopard takes them"
    );
}

#[test]
fn the_cradle_keeps_them_to_africa() {
    let content = hearth_content::Content::load_base();
    let everywhere = Arc::new(Catalog::new(&content));
    let mut cradle = Catalog::new(&content);
    cradle.hominins_in_cradle();
    let cradle = Arc::new(cradle);
    let africa = hominins_held(everywhere.clone(), Realm::Afrotropical);
    let india = hominins_held(everywhere, Realm::Indomalayan);
    assert!(africa > 30.0, "Africa's savanna holds {africa:.0}");
    assert!(india > 30.0, "India's savanna holds {india:.0} by default");
    let africa = hominins_held(cradle.clone(), Realm::Afrotropical);
    let india = hominins_held(cradle, Realm::Indomalayan);
    assert!(africa > 30.0, "the cradle holds {africa:.0}");
    assert!(india < 1.0, "outside the cradle: {india:.0}");
}

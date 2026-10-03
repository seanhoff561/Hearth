//! V2-10 (a) acceptance (PLAN.md): fifty years of generated boreal forest, snowy taiga and
//! tundra, and of the polar desert (the tundra's land in the high arctic's climate), stay within
//! plausible bounds for every species each holds, and each holds the animals that belong to it.

mod biomes;

use biomes::{about_the_heart, check, fifty_years, uniform, world};
use hearth_fauna::habitat::reference_land;
use hearth_fauna::species::Forage;
use hearth_worldgen::realms::Realm;
use hearth_worldgen::region::biome::Biome;

#[test]
fn fifty_years_of_boreal_forest() {
    let (mut eco, heart) = about_the_heart(Biome::BorealForest, |_| true);
    // The conifers' cone crops are mast for the seed-eaters.
    assert!(heart.forage[Forage::Mast as usize] > 0.0, "{heart:?}");
    let (present, failures) = fifty_years(&mut eco, Biome::BorealForest);
    check(
        "boreal forest",
        &present,
        &failures,
        &[
            &["moose"],
            &["snowshoe_hare", "mountain_hare"],
            &["american_red_squirrel", "red_squirrel"],
            &["american_marten", "sable"],
            &["ermine"],
            &["spruce_grouse", "capercaillie", "willow_ptarmigan"],
            &["canada_jay", "siberian_jay"],
        ],
    );
}

#[test]
fn fifty_years_of_snowy_taiga() {
    let (mut eco, _) = about_the_heart(Biome::SnowyTaiga, |_| true);
    let (present, failures) = fifty_years(&mut eco, Biome::SnowyTaiga);
    check(
        "snowy taiga",
        &present,
        &failures,
        &[&["moose"], &["snowshoe_hare", "mountain_hare"], &["ermine"]],
    );
}

#[test]
fn fifty_years_of_tundra() {
    let (mut eco, heart) = about_the_heart(Biome::Tundra, |s| s.temperature > -12.0);
    // The dwarf shrubs are browse.
    let (graze, browse) = (
        heart.forage[Forage::Graze as usize],
        heart.forage[Forage::Browse as usize],
    );
    assert!(browse > 0.3 * graze, "browse {browse:.0}, graze {graze:.0}");
    let (present, failures) = fifty_years(&mut eco, Biome::Tundra);
    check(
        "tundra",
        &present,
        &failures,
        &[
            &["reindeer"],
            &["musk_ox"],
            &["norway_lemming", "brown_lemming"],
            &["arctic_fox"],
            &["willow_ptarmigan", "rock_ptarmigan"],
            &["arctic_hare", "mountain_hare"],
            &["ermine"],
        ],
    );
}

#[test]
fn fifty_years_of_polar_desert() {
    // The high arctic of seed 7's vast planet lies under its polar ice, and the cold high
    // plateaus that were its coldest tundra are the mountains' alpine (D134): the tundra's land
    // in the high arctic's climate, some -13 °C, the warmest month 5 °C, 200 mm.
    let w = world();
    let e = w
        .catalog
        .ecosystems
        .iter()
        .find(|e| e.id.ends_with("tundra_ecosystem"))
        .expect("the tundra");
    let mut land = e.reference.clone().expect("its land");
    (land.temp_c, land.warm_c, land.precip_mm) = (-13.0, 5.0, 200.0);
    let mut eco = uniform(reference_land(&w.catalog, &land, &e.id, Realm::Palearctic));
    let (present, failures) = fifty_years(&mut eco, Biome::Tundra);
    check(
        "polar desert",
        &present,
        &failures,
        &[
            &["norway_lemming", "brown_lemming"],
            &["arctic_fox"],
            &["rock_ptarmigan"],
        ],
    );
}

//! V2-10 (a) acceptance (PLAN.md): fifty years of generated boreal forest, snowy taiga, tundra
//! and polar desert (the coldest tundra) stay within plausible bounds for every species each
//! holds, and each holds the animals that belong to it.

mod biomes;

use biomes::{about_the_heart, check, fifty_years};
use hearth_fauna::species::Forage;
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
    let (mut eco, _) = about_the_heart(Biome::Tundra, |s| s.temperature < -12.0);
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

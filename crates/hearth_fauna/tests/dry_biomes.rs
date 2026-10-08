//! V2-10 (b) acceptance (PLAN.md): fifty years of generated steppe, cold desert, hot desert and
//! dune sea stay within plausible bounds for every species each holds, and each holds the
//! animals that belong to it; and so do a uniform prairie and a uniform hot desert of the
//! Nearctic (the generated world of seed 7 has no dry land in the Nearctic).

mod biomes;

use biomes::{about_the_heart, check, fifty_years, uniform, world};
use hearth_fauna::habitat::open_land;
use hearth_fauna::species::Forage;
use hearth_worldgen::realms::Realm;
use hearth_worldgen::region::biome::Biome;

#[test]
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn fifty_years_of_steppe() {
    let (mut eco, _) = about_the_heart(Biome::Steppe, |_| true);
    let (present, failures) = fifty_years(&mut eco, Biome::Steppe);
    check(
        "steppe",
        &present,
        &failures,
        &[
            &["saiga", "wild_horse", "kulan"],
            &["european_souslik", "bobak_marmot", "common_vole"],
            &["corsac_fox", "red_fox", "steppe_polecat"],
            &["great_bustard", "eurasian_skylark"],
        ],
    );
}

#[test]
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn fifty_years_of_cold_desert() {
    let (mut eco, heart) = about_the_heart(Biome::ColdDesert, |_| true);
    // The desert's shrubs are browse.
    let (graze, browse) = (
        heart.forage[Forage::Graze as usize],
        heart.forage[Forage::Browse as usize],
    );
    assert!(browse > 0.3 * graze, "browse {browse:.0}, graze {graze:.0}");
    let (present, failures) = fifty_years(&mut eco, Biome::ColdDesert);
    check(
        "cold desert",
        &present,
        &failures,
        &[
            &["wild_bactrian_camel", "kulan", "saiga", "wild_horse"],
            &["lesser_egyptian_jerboa"],
        ],
    );
}

#[test]
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn fifty_years_of_hot_desert() {
    let (mut eco, _) = about_the_heart(Biome::HotDesert, |_| true);
    let (present, failures) = fifty_years(&mut eco, Biome::HotDesert);
    check(
        "hot desert",
        &present,
        &failures,
        &[
            &["dromedary", "addax", "dorcas_gazelle", "kulan"],
            &["lesser_egyptian_jerboa"],
        ],
    );
}

#[test]
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn fifty_years_of_dune_sea() {
    let (mut eco, _) = about_the_heart(Biome::DuneSea, |_| true);
    let (present, failures) = fifty_years(&mut eco, Biome::DuneSea);
    check(
        "dune sea",
        &present,
        &failures,
        &[&["dromedary", "addax", "dorcas_gazelle", "kulan"]],
    );
}

#[test]
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn fifty_years_of_a_nearctic_prairie() {
    let h = open_land(
        &world().catalog,
        8.0,
        24.0,
        500.0,
        &["temperate_grassland"],
        Realm::Nearctic,
    );
    let mut eco = uniform(h);
    let (present, failures) = fifty_years(&mut eco, Biome::Steppe);
    check(
        "prairie",
        &present,
        &failures,
        &[
            &["american_bison"],
            &["pronghorn"],
            &["black_tailed_prairie_dog"],
            &["meadow_vole"],
            &["coyote"],
            &["swift_fox", "american_badger"],
            &["burrowing_owl"],
            &["western_meadowlark"],
        ],
    );
}

#[test]
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn fifty_years_of_a_nearctic_hot_desert() {
    let h = open_land(
        &world().catalog,
        21.0,
        33.0,
        200.0,
        &["hot_desert"],
        Realm::Nearctic,
    );
    let mut eco = uniform(h);
    let (present, failures) = fifty_years(&mut eco, Biome::HotDesert);
    check(
        "Nearctic hot desert",
        &present,
        &failures,
        &[
            &["black_tailed_jackrabbit"],
            &["merriams_kangaroo_rat"],
            &["kit_fox", "coyote"],
            &["greater_roadrunner"],
        ],
    );
}

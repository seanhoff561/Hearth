//! V2-10 (d) acceptance (PLAN.md): fifty years of generated montane forest about the Old World's
//! heart of it stay within plausible bounds for every species it holds, and it holds the animals
//! that belong to it; and so do uniform lands of the alpine ecosystem in the Old World, North and
//! South America and the Ethiopian highlands, and of the montane forest in the Americas (the
//! mountain belts of seed 7's vast planet are narrow: their hearts' regions hold other lands as
//! much as their own).

mod biomes;

use biomes::{about_the_heart_in, check, fifty_years, fifty_years_in, uniform, world};
use hearth_fauna::ecology::Ecology;
use hearth_fauna::habitat::reference_land;
use hearth_worldgen::realms::Realm;
use hearth_worldgen::region::biome::Biome;

/// Uniform land of an ecosystem's reference, with a realm's animals.
fn reference(ecosystem: &str, realm: Realm) -> Ecology {
    let w = world();
    let e = w
        .catalog
        .ecosystems
        .iter()
        .find(|e| e.id.ends_with(ecosystem))
        .expect("the ecosystem");
    let r = e.reference.as_ref().expect("its land");
    uniform(reference_land(&w.catalog, r, &e.id, realm))
}

#[test]
fn fifty_years_of_montane_forest() {
    let (mut eco, _) = about_the_heart_in(Biome::MontaneForest, Realm::Palearctic);
    let (present, failures) = fifty_years_in(&mut eco, Biome::MontaneForest, Realm::Palearctic);
    check(
        "montane forest",
        &present,
        &failures,
        &[
            &["chamois", "himalayan_tahr", "alpine_musk_deer"],
            &["red_deer", "roe_deer"],
            &["red_squirrel", "himalayan_monal"],
        ],
    );
}

#[test]
fn fifty_years_of_old_world_alpine() {
    let mut eco = reference("alpine_ecosystem", Realm::Palearctic);
    let (present, failures) = fifty_years(&mut eco, Biome::AlpineMeadow);
    check(
        "Old World alpine",
        &present,
        &failures,
        &[
            &["alpine_ibex", "bharal", "argali"],
            &["chamois", "himalayan_tahr"],
            &["wild_yak", "kiang", "tibetan_antelope"],
            &["alpine_marmot", "plateau_pika", "snow_vole"],
            &["snow_leopard", "gray_wolf"],
            &["tibetan_fox", "red_fox"],
            &["alpine_chough", "himalayan_snowcock"],
        ],
    );
}

#[test]
fn fifty_years_of_rocky_mountain_alpine() {
    let mut eco = reference("alpine_ecosystem", Realm::Nearctic);
    let (present, failures) = fifty_years(&mut eco, Biome::AlpineMeadow);
    check(
        "Rocky Mountain alpine",
        &present,
        &failures,
        &[
            &["mountain_goat"],
            &["bighorn_sheep"],
            &["hoary_marmot", "american_pika"],
            &["white_tailed_ptarmigan"],
            &["cougar", "gray_wolf", "red_fox"],
        ],
    );
}

#[test]
fn fifty_years_of_rocky_mountain_forest() {
    let mut eco = reference("montane_forest_ecosystem", Realm::Nearctic);
    let (present, failures) = fifty_years(&mut eco, Biome::MontaneForest);
    check(
        "Rocky Mountain forest",
        &present,
        &failures,
        &[
            &["mule_deer", "moose"],
            &["golden_mantled_ground_squirrel", "american_red_squirrel"],
            &["clarks_nutcracker"],
            &["cougar", "canada_lynx", "gray_wolf"],
        ],
    );
}

#[test]
fn fifty_years_of_andean_puna() {
    let mut eco = reference("alpine_ecosystem", Realm::Neotropical);
    let (present, failures) = fifty_years(&mut eco, Biome::AlpineMeadow);
    check(
        "Andean puna",
        &present,
        &failures,
        &[
            &["vicuna", "guanaco"],
            &["mountain_viscacha"],
            &["andean_leaf_eared_mouse"],
            &["culpeo"],
        ],
    );
}

#[test]
fn fifty_years_of_andean_forest() {
    let mut eco = reference("montane_forest_ecosystem", Realm::Neotropical);
    let (present, failures) = fifty_years(&mut eco, Biome::MontaneForest);
    check(
        "Andean forest",
        &present,
        &failures,
        &[
            &["taruca", "northern_pudu"],
            &["spectacled_bear"],
            &["mountain_tapir"],
            &["culpeo", "cougar"],
        ],
    );
}

#[test]
fn fifty_years_of_ethiopian_highlands() {
    // The tropics' alpine moors are cold all year round rather than in a winter: about 6 °C,
    // the warmest month hardly warmer, frost most nights.
    let w = world();
    let e = w
        .catalog
        .ecosystems
        .iter()
        .find(|e| e.id.ends_with("alpine_ecosystem"))
        .expect("the alpine");
    let mut land = e.reference.clone().expect("its land");
    (land.temp_c, land.warm_c, land.precip_mm) = (6.0, 8.0, 1200.0);
    let mut eco = uniform(reference_land(
        &w.catalog,
        &land,
        &e.id,
        Realm::Afrotropical,
    ));
    let (present, failures) = fifty_years(&mut eco, Biome::AlpineMeadow);
    check(
        "Ethiopian highlands",
        &present,
        &failures,
        &[
            &["gelada"],
            &["giant_mole_rat"],
            &["ethiopian_wolf"],
            &["rock_hyrax", "klipspringer"],
            &["walia_ibex", "mountain_nyala"],
        ],
    );
}

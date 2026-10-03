//! V2-10 (c) acceptance (PLAN.md): fifty years of generated savanna in the realms of Africa,
//! India and South America, of rainforest in Africa's and South America's, and of Mediterranean
//! scrub about the Mediterranean, stay within plausible bounds for every species each holds, and
//! each holds the animals that belong to it; and so do a uniform rainforest of tropical Asia and
//! a uniform chaparral of the Nearctic (the generated world of seed 7 has only specks of the one
//! and none of the other).

mod biomes;

use biomes::{about_the_heart_in, check, fifty_years, fifty_years_in, uniform, world};
use hearth_fauna::ecology::Ecology;
use hearth_fauna::habitat::reference_land;
use hearth_worldgen::realms::Realm;
use hearth_worldgen::region::biome::Biome;

/// The regions about the heart of a biome in a realm.
fn heart(biome: Biome, realm: Realm) -> Ecology {
    about_the_heart_in(biome, realm).0
}

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
fn fifty_years_of_african_savanna() {
    let mut eco = heart(Biome::Savanna, Realm::Afrotropical);
    let (present, failures) = fifty_years_in(&mut eco, Biome::Savanna, Realm::Afrotropical);
    check(
        "African savanna",
        &present,
        &failures,
        &[
            &["plains_zebra", "blue_wildebeest"],
            &["african_buffalo"],
            &["impala", "thomsons_gazelle"],
            &["african_bush_elephant"],
            &["giraffe"],
            &["lion"],
            &["spotted_hyena"],
            &["leopard", "cheetah", "african_wild_dog"],
            &["ostrich", "helmeted_guineafowl"],
            &["natal_multimammate_mouse"],
        ],
    );
}

#[test]
fn fifty_years_of_indian_savanna() {
    let mut eco = heart(Biome::Savanna, Realm::Indomalayan);
    let (present, failures) = fifty_years_in(&mut eco, Biome::Savanna, Realm::Indomalayan);
    check(
        "Indian savanna",
        &present,
        &failures,
        &[
            &["chital", "sambar"],
            &["gaur"],
            &["asian_elephant"],
            &["tiger", "leopard"],
            &["dhole"],
            &["indian_peafowl", "red_junglefowl"],
            &["ricefield_rat"],
            &["rhesus_macaque"],
        ],
    );
}

#[test]
fn fifty_years_of_american_savanna() {
    let mut eco = heart(Biome::Savanna, Realm::Neotropical);
    let (present, failures) = fifty_years_in(&mut eco, Biome::Savanna, Realm::Neotropical);
    check(
        "American savanna",
        &present,
        &failures,
        &[
            &["giant_anteater"],
            &["greater_rhea"],
            &["collared_peccary"],
            &["hairy_tailed_bolo_mouse"],
            &["jaguar", "ocelot", "maned_wolf"],
        ],
    );
}

#[test]
fn fifty_years_of_african_rainforest() {
    let mut eco = heart(Biome::TropicalRainforest, Realm::Afrotropical);
    let (present, failures) =
        fifty_years_in(&mut eco, Biome::TropicalRainforest, Realm::Afrotropical);
    check(
        "African rainforest",
        &present,
        &failures,
        &[
            &["african_forest_elephant"],
            &["okapi", "bongo"],
            &["blue_duiker"],
            &["red_river_hog"],
            &["chimpanzee"],
            &["western_lowland_gorilla"],
            &["mantled_guereza"],
            &["leopard", "african_golden_cat"],
            &["crowned_eagle"],
            &["african_grey_parrot"],
            &["gaboon_viper"],
        ],
    );
}

#[test]
fn fifty_years_of_asian_rainforest() {
    let mut eco = reference("tropical_rainforest", Realm::Indomalayan);
    let (present, failures) = fifty_years(&mut eco, Biome::TropicalRainforest);
    check(
        "Asian rainforest",
        &present,
        &failures,
        &[
            &["asian_elephant"],
            &["gaur", "sambar"],
            &["indian_muntjac"],
            &["tiger", "clouded_leopard"],
            &["lar_gibbon"],
            &["great_hornbill"],
            &["sun_bear"],
            &["red_spiny_rat"],
            &["reticulated_python", "king_cobra"],
        ],
    );
}

#[test]
fn fifty_years_of_american_rainforest() {
    let mut eco = heart(Biome::TropicalRainforest, Realm::Neotropical);
    let (present, failures) =
        fifty_years_in(&mut eco, Biome::TropicalRainforest, Realm::Neotropical);
    check(
        "American rainforest",
        &present,
        &failures,
        &[
            &["lowland_tapir"],
            &["collared_peccary"],
            &["red_brocket"],
            &["brown_throated_sloth"],
            &["mantled_howler"],
            &["jaguar", "ocelot"],
            &["harpy_eagle"],
            &["scarlet_macaw", "keel_billed_toucan"],
            &["red_rumped_agouti", "lowland_paca"],
            &["tomes_spiny_rat"],
            &["boa_constrictor"],
        ],
    );
}

#[test]
fn fifty_years_of_mediterranean_scrub() {
    let mut eco = heart(Biome::MediterraneanScrub, Realm::Palearctic);
    let (present, failures) =
        fifty_years_in(&mut eco, Biome::MediterraneanScrub, Realm::Palearctic);
    check(
        "Mediterranean scrub",
        &present,
        &failures,
        &[
            &["european_rabbit"],
            &["iberian_lynx"],
            &["red_legged_partridge"],
            &["wild_boar", "red_deer"],
            &["red_fox"],
            &["wood_mouse"],
            &["hermanns_tortoise"],
            &["montpellier_snake"],
        ],
    );
}

#[test]
fn fifty_years_of_a_chaparral() {
    let mut eco = reference("mediterranean_scrub", Realm::Nearctic);
    let (present, failures) = fifty_years(&mut eco, Biome::MediterraneanScrub);
    check(
        "chaparral",
        &present,
        &failures,
        &[
            &["mule_deer"],
            &["california_quail"],
            &["coyote", "bobcat"],
            &["deer_mouse"],
            &["black_tailed_jackrabbit"],
        ],
    );
}

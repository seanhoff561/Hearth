//! V2-10 (e) acceptance (PLAN.md): fifty years of generated wetland about the Old World's heart
//! of it (its reed beds, pools and bog, with the lakes' fish in them) stay within plausible bounds
//! for every species it holds, and it holds the animals that belong to it; and so do uniform lands
//! of the freshwater ecosystems' references for the realms whose wetlands seed 7's vast planet has
//! only in the cold of their southern lands, or not at all (the Nearctic's, Africa's, the
//! Americas' and tropical Asia's).

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
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn fifty_years_of_old_world_wetland() {
    let (mut eco, _) = about_the_heart_in(Biome::Wetland, Realm::Palearctic);
    let (present, failures) = fifty_years_in(&mut eco, Biome::Wetland, Realm::Palearctic);
    check(
        "Old World wetland",
        &present,
        &failures,
        &[
            &["eurasian_beaver", "water_vole"],
            &["eurasian_otter"],
            &["mallard", "greylag_goose", "mute_swan"],
            &["grey_heron", "white_stork", "common_crane"],
            &["northern_pike", "european_perch", "common_carp"],
            &["common_frog", "common_toad"],
        ],
    );
}

#[test]
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn fifty_years_of_north_american_marsh() {
    let mut eco = reference("temperate_freshwater", Realm::Nearctic);
    let (present, failures) = fifty_years(&mut eco, Biome::Wetland);
    check(
        "North American marsh",
        &present,
        &failures,
        &[
            &["american_beaver", "muskrat"],
            &["north_american_river_otter", "american_mink"],
            &["mallard", "canada_goose"],
            &["great_blue_heron", "osprey"],
            &["northern_pike", "yellow_perch", "channel_catfish"],
            &["american_bullfrog", "wood_frog"],
        ],
    );
}

#[test]
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn fifty_years_of_african_swamp() {
    let mut eco = reference("tropical_freshwater", Realm::Afrotropical);
    let (present, failures) = fifty_years(&mut eco, Biome::Wetland);
    check(
        "African swamp",
        &present,
        &failures,
        &[
            &["hippopotamus"],
            &["nile_crocodile"],
            &["sitatunga"],
            &["african_fish_eagle", "shoebill", "grey_heron"],
            &["nile_tilapia", "african_sharptooth_catfish"],
        ],
    );
}

#[test]
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn fifty_years_of_south_american_floodplain() {
    let mut eco = reference("tropical_freshwater", Realm::Neotropical);
    let (present, failures) = fifty_years(&mut eco, Biome::Wetland);
    check(
        "South American floodplain",
        &present,
        &failures,
        &[
            &["capybara"],
            &["spectacled_caiman"],
            &["green_anaconda", "giant_otter"],
            &["marsh_deer", "wattled_jacana"],
            &["red_bellied_piranha", "arapaima"],
        ],
    );
}

#[test]
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn fifty_years_of_asian_floodplain() {
    let mut eco = reference("tropical_freshwater", Realm::Indomalayan);
    let (present, failures) = fifty_years(&mut eco, Biome::Wetland);
    check(
        "Asian floodplain",
        &present,
        &failures,
        &[
            &["wild_water_buffalo"],
            &["mugger_crocodile", "gharial"],
            &["smooth_coated_otter", "fishing_cat"],
            &["sarus_crane", "grey_heron"],
            &["snakehead"],
        ],
    );
}

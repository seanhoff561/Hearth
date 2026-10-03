//! V2-10 (f) acceptance (PLAN.md): fifty years of the generated seas about their hearts on seed
//! 7's vast planet (the North Atlantic's cold shelf, the temperate shelves of the Old World and
//! Africa, the Indo-Pacific's reefs, the open ocean, the Arctic and the Antarctic seas) stay
//! within plausible bounds for every species they hold, and they hold the animals that belong to
//! them; and so do uniform seas of the marine ecosystems' references for the realms whose seas the
//! world lacks or has only in patches (the Nearctic's Pacific shelf, the Caribbean's reefs, the
//! mangroves of the Old World and the New).

mod biomes;

use biomes::{about_the_heart_in, check, fifty_years, fifty_years_in, uniform, world};
use hearth_fauna::ecology::Ecology;
use hearth_fauna::habitat::reference_land;
use hearth_worldgen::realms::Realm;
use hearth_worldgen::region::biome::Biome;

/// A uniform sea (or land) of an ecosystem's reference, with a realm's animals.
fn reference(ecosystem: &str, realm: Realm) -> Ecology {
    let w = world();
    let e = w
        .catalog
        .ecosystems
        .iter()
        .find(|e| e.id.ends_with(ecosystem))
        .expect("the ecosystem");
    let r = e.reference.as_ref().expect("its sea");
    uniform(reference_land(&w.catalog, r, &e.id, realm))
}

#[test]
fn fifty_years_of_north_atlantic_shelf() {
    let (mut eco, _) = about_the_heart_in(Biome::ColdSea, Realm::Palearctic);
    let (present, failures) = fifty_years_in(&mut eco, Biome::ColdSea, Realm::Palearctic);
    check(
        "North Atlantic shelf",
        &present,
        &failures,
        &[
            &["atlantic_herring", "sandeel"],
            &["atlantic_cod"],
            &["harbour_seal", "grey_seal"],
            &["harbour_porpoise"],
            &["atlantic_puffin", "northern_gannet", "herring_gull"],
        ],
    );
}

#[test]
fn fifty_years_of_north_pacific_shelf() {
    let mut eco = reference("cold_shelf_sea", Realm::Nearctic);
    let (present, failures) = fifty_years(&mut eco, Biome::ColdSea);
    check(
        "North Pacific shelf",
        &present,
        &failures,
        &[
            &["atlantic_herring"],
            &["sea_otter"],
            &["steller_sea_lion", "harbour_seal"],
            &["red_king_crab"],
        ],
    );
}

#[test]
fn fifty_years_of_temperate_shelf() {
    let (mut eco, _) = about_the_heart_in(Biome::TemperateSea, Realm::Afrotropical);
    let (present, failures) = fifty_years_in(&mut eco, Biome::TemperateSea, Realm::Afrotropical);
    check(
        "Benguela shelf",
        &present,
        &failures,
        &[
            &["sardine", "chub_mackerel"],
            &["cape_fur_seal"],
            &["african_penguin"],
            &["common_dolphin", "bottlenose_dolphin"],
        ],
    );
}

#[test]
fn fifty_years_of_california_current() {
    let mut eco = reference("temperate_shelf_sea", Realm::Nearctic);
    let (present, failures) = fifty_years(&mut eco, Biome::TemperateSea);
    check(
        "California current",
        &present,
        &failures,
        &[
            &["sardine", "chub_mackerel"],
            &["california_sea_lion"],
            &["brown_pelican"],
            &["common_dolphin", "bottlenose_dolphin"],
        ],
    );
}

#[test]
fn fifty_years_of_indo_pacific_reef() {
    let (mut eco, _) = about_the_heart_in(Biome::WarmShallows, Realm::Indomalayan);
    let (present, failures) = fifty_years_in(&mut eco, Biome::WarmShallows, Realm::Indomalayan);
    check(
        "Indo-Pacific reef",
        &present,
        &failures,
        &[
            &["bluestripe_snapper"],
            &["bumphead_parrotfish"],
            &["blacktip_reef_shark", "giant_moray", "giant_grouper"],
            &["green_turtle", "hawksbill_turtle"],
        ],
    );
}

#[test]
fn fifty_years_of_caribbean_reef() {
    let mut eco = reference("coral_reef", Realm::Neotropical);
    let (present, failures) = fifty_years(&mut eco, Biome::WarmShallows);
    check(
        "Caribbean reef",
        &present,
        &failures,
        &[
            &["yellowtail_snapper", "stoplight_parrotfish"],
            &["nassau_grouper", "caribbean_reef_shark"],
            &["green_turtle", "hawksbill_turtle"],
            &["west_indian_manatee"],
        ],
    );
}

#[test]
fn fifty_years_of_open_ocean() {
    let (mut eco, _) = about_the_heart_in(Biome::DeepOcean, Realm::Palearctic);
    let (present, failures) = fifty_years_in(&mut eco, Biome::DeepOcean, Realm::Palearctic);
    check(
        "Open ocean",
        &present,
        &failures,
        &[
            &["lanternfish"],
            &["blue_shark", "yellowfin_tuna"],
            &["common_dolphin"],
        ],
    );
}

#[test]
fn fifty_years_of_arctic_sea() {
    let (mut eco, _) = about_the_heart_in(Biome::PolarSea, Realm::Palearctic);
    let (present, failures) = fifty_years_in(&mut eco, Biome::PolarSea, Realm::Palearctic);
    check(
        "Arctic sea",
        &present,
        &failures,
        &[
            &["arctic_cod"],
            &["ringed_seal"],
            &["walrus"],
            &["beluga", "narwhal"],
        ],
    );
}

#[test]
fn fifty_years_of_antarctic_sea() {
    let (mut eco, _) = about_the_heart_in(Biome::PolarSea, Realm::Antarctic);
    let (present, failures) = fifty_years_in(&mut eco, Biome::PolarSea, Realm::Antarctic);
    check(
        "Antarctic sea",
        &present,
        &failures,
        &[
            &["antarctic_silverfish"],
            &["adelie_penguin", "emperor_penguin"],
            &["crabeater_seal"],
            &["weddell_seal", "leopard_seal"],
        ],
    );
}

#[test]
fn fifty_years_of_asian_mangroves() {
    let mut eco = reference("mangrove_forest", Realm::Indomalayan);
    let (present, failures) = fifty_years(&mut eco, Biome::Mangrove);
    check(
        "Asian mangroves",
        &present,
        &failures,
        &[
            &["fiddler_crab"],
            &["proboscis_monkey"],
            &["saltwater_crocodile"],
        ],
    );
}

#[test]
fn fifty_years_of_american_mangroves() {
    let mut eco = reference("mangrove_forest", Realm::Neotropical);
    let (present, failures) = fifty_years(&mut eco, Biome::Mangrove);
    check(
        "American mangroves",
        &present,
        &failures,
        &[&["fiddler_crab"], &["scarlet_ibis"]],
    );
}

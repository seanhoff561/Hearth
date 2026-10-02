//! Carcasses and butchering by species (V2-7 (h)): every species big enough to work has its
//! carcasses at their real masses, and ways of working them that give its yields.

use hearth_content::Content;
use hearth_content::butchery::{Carcass, butcher_id, carcass_id, carcasses, hack_id};
use hearth_content::schema::Season;
use hearth_content::schema::process::{Match, Output};

fn kg_of(outputs: &[Output], material: &str) -> (f32, f32) {
    outputs
        .iter()
        .filter(|o| matches!(&o.item, Match::Material(m) if m.as_str() == material))
        .fold((0.0, 0.0), |(lo, hi), o| (lo + o.amount.0, hi + o.amount.1))
}

#[test]
fn every_species_big_enough_has_carcasses_and_their_butchering() {
    let c = Content::load_base();
    let mut n = 0;
    for a in c.animals.iter() {
        for which in carcasses(a) {
            let id = carcass_id(&a.id, which);
            let item = c.items.get(&id).unwrap_or_else(|| panic!("{id}"));
            let butcher = c.processes.get(&butcher_id(&id)).expect("butchering");
            let hack = c.processes.get(&hack_id(&id)).expect("hacking");
            let (meat, _) = kg_of(&butcher.outputs, "hearth:meat");
            let (fish, _) = kg_of(&butcher.outputs, "hearth:raw_fish");
            let (hacked, _) = kg_of(&hack.outputs, "hearth:meat");
            println!(
                "{:<36} {:>7.2} kg  {:<40} {:>4.1} h  meat {:.2} kg ({:.2} hacked)",
                item.name,
                item.mass_kg,
                butcher.name,
                butcher.duration.hours,
                meat + fish,
                hacked
            );
            assert!(item.tags.iter().any(|t| t == "carcass"));
            assert!(meat + fish > 0.0, "{id} gives flesh");
            assert!(hacked < meat + 1e-6, "hacking wastes meat");
            assert!(butcher.knowledge.is_some() && hack.knowledge.is_none());
            n += 1;
        }
    }
    assert!(n >= 40, "{n} carcasses");
    // Songbirds, mice and frogs are too small to work.
    for small in [
        "european_robin",
        "deer_mouse",
        "common_frog",
        "western_honey_bee",
    ] {
        assert!(
            c.items.get(&format!("hearth:{small}_carcass")).is_none(),
            "{small}"
        );
    }
}

#[test]
fn a_red_deer_by_sex_age_and_season() {
    let c = Content::load_base();
    let hind = c.items.get("hearth:red_deer_carcass").expect("hind");
    let stag = c.items.get("hearth:red_deer_carcass_male").expect("stag");
    let calf = c.items.get("hearth:red_deer_carcass_young").expect("calf");
    // Hinds about 130 kg, stags half again, calves in their first autumn about 50.
    assert!((120.0..145.0).contains(&hind.mass_kg), "{}", hind.mass_kg);
    assert!((stag.mass_kg / hind.mass_kg - 1.5).abs() < 0.01);
    assert!((45.0..60.0).contains(&calf.mass_kg), "{}", calf.mass_kg);
    let butcher = |id: &str| c.processes.get(&butcher_id(id)).expect("butcher");
    let (meat, _) = kg_of(&butcher(&hind.id).outputs, "hearth:meat");
    assert!(
        (meat / hind.mass_kg - 0.34).abs() < 0.02,
        "{meat} kg of meat"
    );
    let (hide, _) = kg_of(&butcher(&hind.id).outputs, "hearth:rawhide");
    assert!(hide > 8.0, "{hide} kg of hide");
    // Antlers only from the stag, and only in the seasons he carries them.
    let antlers = |id: &str| -> Vec<Output> {
        butcher(id)
            .byproducts
            .iter()
            .filter(|o| matches!(&o.item, Match::Material(m) if m.as_str() == "hearth:antler"))
            .cloned()
            .collect()
    };
    assert!(antlers(&hind.id).is_empty());
    assert!(antlers(&calf.id).is_empty());
    let a = antlers(&stag.id);
    assert_eq!(a.len(), 1);
    assert!(!a[0].seasons.contains(&Season::Spring), "cast in spring");
    // Fat by the season: most in autumn.
    let fat = |season: Season| -> f32 {
        butcher(&hind.id)
            .byproducts
            .iter()
            .filter(|o| matches!(&o.item, Match::Material(m) if m.as_str() == "hearth:animal_fat"))
            .filter(|o| o.seasons.contains(&season))
            .map(|o| o.amount.1)
            .sum()
    };
    assert!(fat(Season::Autumn) > 2.0 * fat(Season::Spring));
    // The bigger the animal, the longer the work.
    assert!(butcher(&stag.id).duration.hours > butcher(&hind.id).duration.hours);
    let hare = c
        .processes
        .get(&butcher_id(&carcass_id(
            "hearth:brown_hare",
            Carcass::Grown,
        )))
        .expect("hare");
    assert!(
        hare.duration.hours < 0.5,
        "{} h for a hare",
        hare.duration.hours
    );
    assert!(
        (1.5..4.0).contains(&butcher(&hind.id).duration.hours),
        "{} h for a hind",
        butcher(&hind.id).duration.hours
    );
}

#[test]
fn butchery_enables_every_species_butchering() {
    let c = Content::load_base();
    let k = c.knowledge.get("hearth:butchery").expect("butchery");
    assert!(
        k.enables
            .iter()
            .any(|p| p.as_str() == "hearth:butcher_roe_deer")
    );
    assert!(
        k.enables
            .iter()
            .all(|p| c.processes.get(p.as_str()).is_some())
    );
}

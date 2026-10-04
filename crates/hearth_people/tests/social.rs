//! H4 (V2.1 §8.1): kinship read from the pedigree and the bonds, and households — a pair's
//! hearth with the children they raise — kept through decades of the life course.

mod common;

use common::*;
use glam::DVec3;
use hearth_people::kin::{Kin, kin_of};
use hearth_people::{Now, People};

#[test]
fn households_and_kin_hold_through_the_decades() {
    let b = base();
    let species = &b.species;
    let k = species.index_of("homo_sapiens").expect("our species");
    let mut w = Savanna::new();
    let mut p = People::new(21);
    let mut now = w.now();
    for (x, z) in [(0.0, 0.0), (15_000.0, 0.0)] {
        let at = DVec3::new(x, GROUND, z);
        p.spawn_band(
            species,
            &b.graph,
            &b.items,
            &mut w,
            k,
            [8, 7, 9, 3],
            at,
            now,
        );
    }
    let start = now.day;
    for year in 1..=60 {
        now = Now {
            day: start + year as f64 * YEAR_DAYS,
            ..now
        };
        p.live_course(species, &b.items, &w, now);
    }
    let living: Vec<&hearth_people::Person> = p.persons.iter().filter(|q| q.alive()).collect();
    assert!(living.len() > 30, "{} living", living.len());
    // Everyone keeps a hearth; a pair keeps one together; a young child is in its mother's.
    assert!(living.iter().all(|q| q.social.household.is_some()));
    let mut pairs = 0;
    let mut young = 0;
    for q in &living {
        if let Some(r) = q.social.bond.and_then(|r| p.get(r))
            && r.alive()
            && r.social.band == q.social.band
        {
            assert_eq!(q.social.household, r.social.household, "a pair's hearth");
            pairs += 1;
        }
        let age = q.age(&now);
        if age < 10.0
            && let Some(m) = q.life.mother.and_then(|m| p.get(m))
            && m.alive()
            && m.social.band == q.social.band
        {
            assert_eq!(
                q.social.household, m.social.household,
                "a child with its mother"
            );
            young += 1;
        }
    }
    println!(
        "{} living, {pairs} in pairs, {young} young with their mothers",
        living.len()
    );
    assert!(pairs > 4 && young > 4);
    // Kin read rightly: a mother and her child, two of her children, a grandmother.
    let child = living
        .iter()
        .find(|q| {
            q.life
                .mother
                .and_then(|m| p.get(m))
                .and_then(|m| m.life.mother)
                .is_some()
        })
        .expect("a child with a grandmother in the record");
    let mother = child.life.mother.expect("its mother");
    let grandmother = p.get(mother).and_then(|m| m.life.mother).expect("hers");
    assert_eq!(kin_of(&p, child.id, mother), Some(Kin::Parent));
    assert_eq!(kin_of(&p, mother, child.id), Some(Kin::Child));
    assert_eq!(kin_of(&p, child.id, grandmother), Some(Kin::Grandparent));
    assert_eq!(kin_of(&p, grandmother, child.id), Some(Kin::Grandchild));
    if let Some(sib) = p
        .persons
        .iter()
        .find(|q| q.id != child.id && q.life.mother == Some(mother))
    {
        let k = kin_of(&p, child.id, sib.id);
        assert!(matches!(k, Some(Kin::Sibling | Kin::HalfSibling)), "{k:?}");
    }
}

#[test]
fn ties_grow_in_company_kin_begin_closer_and_giving_is_owed() {
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    live(&mut p, &mut world, 300.0, &[]);
    let members: Vec<u64> = p.full().map(|q| q.id).collect();
    // Each knows the others of its band.
    for q in p.full() {
        assert!(
            q.social.ties.len() + 1 >= members.len(),
            "#{} knows {} of {}",
            q.id,
            q.social.ties.len(),
            members.len()
        );
    }
    // A mother is fonder of her child than of one of the band not her kin.
    let tie = |p: &People, a: u64, b: u64| {
        p.get(a)
            .and_then(|q| q.social.ties.iter().find(|t| t.who == b))
            .map(|t| t.affection)
            .unwrap_or(0.0)
    };
    let child = p
        .full()
        .find(|q| q.life.mother.is_some())
        .expect("a child with its mother");
    let mother = child.life.mother.expect("its mother");
    let stranger = p
        .full()
        .find(|q| q.id != mother && kin_of(&p, mother, q.id).is_none())
        .map(|q| q.id)
        .expect("one not her kin");
    assert!(tie(&p, mother, child.id) > tie(&p, mother, stranger) + 0.2);
    // What one gives the other owes, and is the fonder for.
    let before = tie(&p, stranger, mother);
    let day = world.now().day;
    p.give(mother, stranger, 1.0, day);
    let ledger = |p: &People, a: u64, b: u64| {
        p.get(a)
            .and_then(|q| q.social.ties.iter().find(|t| t.who == b))
            .map(|t| (t.given, t.owed))
            .unwrap_or_default()
    };
    assert!(ledger(&p, mother, stranger).0 >= 1.0, "she gave");
    assert!(ledger(&p, stranger, mother).1 >= 1.0, "it is owed her");
    assert!(tie(&p, stranger, mother) > before, "and the fonder");
}

#[test]
fn a_mother_with_food_feeds_her_hungry_child() {
    let b = base();
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    let sp = &b.species.list[b.species.index_of("australopithecus").expect("the hominin")];
    let now = world.now();
    // A weaned child of the band and its mother.
    let child = p
        .full()
        .find(|q| {
            q.life.mother.is_some() && q.life_stage(sp, &now) != hearth_people::LifeStage::Infant
        })
        .map(|q| q.id)
        .expect("a weaned child with its mother");
    let mother = p
        .get(child)
        .and_then(|q| q.life.mother)
        .expect("its mother");
    // Something good to eat in her hand; the child hungry.
    let food = b
        .items
        .iter()
        .find(|k| {
            hearth_craft::food::bite_of(&b.content, k, &hearth_items::Stack::of(&k.id, 1))
                .is_some_and(|f| f.kcal > 20.0)
        })
        .map(|k| k.id.clone())
        .expect("a food");
    for q in p.persons.iter_mut() {
        if q.id == mother {
            q.possessions.carry.right = Some(hearth_items::Stack::of(&food, 4));
        }
        if q.id == child {
            q.body.energy.glycogen_kcal = 0.0;
            q.body.energy.fat_kcal *= 0.4;
        }
    }
    live(&mut p, &mut world, 120.0, &[]);
    let gave = p
        .get(mother)
        .and_then(|q| q.social.ties.iter().find(|t| t.who == child))
        .map_or(0.0, |t| t.given);
    println!("she gave {gave:.2} of {food}");
    assert!(gave > 0.0, "she fed her child");
}

#[test]
fn one_hurt_is_tended_by_those_fond_of_them() {
    let b = base();
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    let sp = &b.species.list[b.species.index_of("australopithecus").expect("the hominin")];
    let now = world.now();
    // A mother hurt: a deep cut to the leg.
    let mother = p
        .full()
        .find(|q| p.full().any(|c| c.life.mother == Some(q.id)))
        .map(|q| q.id)
        .expect("a mother");
    for q in p.persons.iter_mut().filter(|q| q.id == mother) {
        let cfg = q.body_config(sp, &now).into_owned();
        q.body
            .injure(
                &cfg,
                "cut",
                hearth_content::schema::body::BodyRegion::LowerLeg,
                hearth_body::Side::Left,
                0.5,
            )
            .expect("a cut");
    }
    live(&mut p, &mut world, 120.0, &[]);
    let tended: f32 = p
        .full()
        .filter(|q| q.id != mother)
        .filter_map(|q| q.social.ties.iter().find(|t| t.who == mother))
        .map(|t| t.given)
        .sum();
    println!("tended her for {tended:.2}");
    assert!(tended > 0.0, "someone stayed by her");
}

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

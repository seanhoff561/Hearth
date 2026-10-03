//! H0 (V2.1 §2, §17): persons persist. A save round-trips them exactly and they live on alike after
//! it; a band laid to rest and woken again is the same persons — those its numbers lost while the
//! player was away dead, those they gained born to its mothers — a half year older.

mod common;

use common::*;
use glam::DVec3;
use hearth_fauna::live::Stage;
use hearth_people::save::{from_json, to_json};
use hearth_people::{Cause, Event, Numbers, People, PersonId, Tier};

#[test]
fn persons_round_trip_through_a_save() {
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    live(&mut p, &mut world, 600.0, &[]);
    let json = to_json(&p.to_save()).expect("saved");
    let back = from_json(&json).expect("read back");
    let mut q = People::from_save(7, back);
    assert_eq!(to_json(&q.to_save()).expect("saved again"), json);
    assert_eq!(q.full().count(), 10);
    // Both live on alike.
    let mut world2 = world.clone();
    live(&mut p, &mut world, 300.0, &[]);
    live(&mut q, &mut world2, 300.0, &[]);
    assert_eq!(
        to_json(&p.to_save()).expect("saved"),
        to_json(&q.to_save()).expect("saved")
    );
}

#[test]
fn a_newer_save_is_refused_and_a_broken_one_said_so() {
    let mut world = Savanna::new();
    let p = band(&mut world);
    let mut v: serde_json::Value =
        serde_json::from_slice(&to_json(&p.to_save()).expect("saved")).expect("JSON");
    v["format"] = serde_json::Value::from(hearth_people::FORMAT + 1);
    let newer = serde_json::to_vec(&v).expect("JSON");
    assert!(from_json(&newer).is_err(), "a newer format is refused");
    assert!(
        from_json(b"{}").is_err(),
        "a save without its format is refused"
    );
}

#[test]
fn a_band_woken_again_is_the_same_people() {
    let b = base();
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    live(&mut p, &mut world, 60.0, &[]);
    let band = p.bands[0].id;
    let living = |p: &People| -> Vec<PersonId> {
        p.members(band)
            .filter(|q| q.alive())
            .map(|q| q.id)
            .collect()
    };
    let before = living(&p);
    assert_eq!(before.len(), 10);
    let age0 = p.get(before[0]).expect("there").age(&world.now());
    let (n, _) = p.rest(band, &b.species, world.now()).expect("laid to rest");
    assert_eq!(
        n,
        Numbers {
            young: 1,
            juveniles: 2,
            females: 4,
            males: 3
        }
    );
    assert_eq!(p.full().count(), 0, "all dormant");
    assert!(p.persons.iter().all(|q| q.tier == Tier::Dormant));

    // A quarter year away; the cells lost two of them.
    for _ in 0..8 {
        world.day += 1.0;
    }
    let here = DVec3::new(4.0, GROUND, 4.0);
    let fewer = Numbers {
        young: 0,
        juveniles: 2,
        females: 3,
        males: 3,
    };
    let now = world.now();
    p.wake(
        band, &b.species, &b.graph, &mut world, &b.items, fewer, here, now,
    );
    let after = living(&p);
    assert_eq!(after.len(), 8);
    assert!(
        after.iter().all(|id| before.contains(id)),
        "the same persons"
    );
    let dead: Vec<_> = p.members(band).filter(|q| !q.alive()).collect();
    assert_eq!(dead.len(), 2);
    for d in &dead {
        assert_eq!(
            d.life.died.as_ref().map(|x| &x.cause),
            Some(&Cause::WhileAway)
        );
    }
    // The frailest died: the young, before any grown one in its prime.
    let sp = b.species.get("australopithecus").expect("the hominin");
    assert!(
        dead.iter().all(|d| d.stage(sp, &now) != Stage::Adult),
        "the young died first"
    );
    assert!(
        p.full().all(|q| (q.place.pos - here).length() < 12.0),
        "back about the place"
    );

    // Away again, a half year; the cells gained two young.
    p.rest(band, &b.species, world.now()).expect("laid to rest");
    for _ in 0..16 {
        world.day += 1.0;
    }
    let more = Numbers {
        young: 2,
        juveniles: 2,
        females: 3,
        males: 3,
    };
    let now = world.now();
    p.wake(
        band, &b.species, &b.graph, &mut world, &b.items, more, here, now,
    );
    let again = living(&p);
    assert_eq!(again.len(), 10);
    let born: Vec<_> = p
        .members(band)
        .filter(|q| q.alive() && !before.contains(&q.id))
        .collect();
    assert_eq!(born.len(), 2);
    for c in &born {
        assert!(c.id > *before.iter().max().expect("ids"), "new ids");
        assert!(
            c.life
                .events
                .iter()
                .any(|e| matches!(e.event, Event::Born { .. })),
            "born"
        );
        assert!(c.age(&now) < 1.0, "a baby: {:.2} years", c.age(&now));
        let mother = c.life.mother.and_then(|m| p.get(m)).expect("a mother");
        assert!(mother.alive() && mother.life.female);
        assert_eq!(mother.stage(sp, &now), Stage::Adult);
    }
    // Everyone is older by the time away: three quarters of a year.
    let first = p.get(before[0]).expect("still there");
    assert!(
        (first.age(&now) - age0 - 0.75).abs() < 0.01,
        "{:.3} years on",
        first.age(&now) - age0
    );
}

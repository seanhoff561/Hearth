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

#[test]
fn a_band_left_as_households_and_come_back_to_has_lived_on() {
    let b = base();
    let species = &b.species;
    let k = species.index_of("homo_sapiens").expect("our species");
    let mut world = Savanna::new();
    let mut p = People::new(80);
    let now = world.now();
    let band = p.spawn_band(
        species,
        &b.graph,
        &b.items,
        &mut world,
        k,
        [6, 6, 4, 0],
        DVec3::new(2.0, GROUND, 2.0),
        now,
    );
    live(&mut p, &mut world, 10.0, &[]);
    let now = world.now();
    let before: Vec<(PersonId, f64)> = p
        .members(band)
        .filter(|q| q.alive())
        .map(|q| (q.id, q.life.born))
        .collect();
    let bi = p.bands.iter().position(|x| x.id == band).expect("the band");
    // The player goes away: the band goes on as households, five years.
    p.to_households(bi, now);
    assert!(
        p.members(band)
            .all(|q| !q.alive() || q.tier == Tier::Household)
    );
    p.live_course(species, &b.items, &world, now);
    let later = hearth_people::Now {
        day: now.day + 5.0 * YEAR_DAYS,
        ..now
    };
    p.live_course(species, &b.items, &world, later);
    // And comes back.
    let here = DVec3::new(4.0, GROUND, 4.0);
    let bi = p.bands.iter().position(|x| x.id == band).expect("the band");
    p.lift(bi, species, &mut world, &b.items, here, later);
    let mut stayed = 0;
    for (id, born) in &before {
        let q = p.get(*id).expect("the same record");
        assert_eq!(q.life.born, *born, "the same person");
        if q.alive() && q.social.band == band {
            stayed += 1;
            assert_eq!(q.tier, Tier::Full, "back in full");
            assert!((q.age(&later) - q.age(&now) - 5.0).abs() < 0.01);
            assert!((q.place.pos - here).length() < 12.0, "about the place");
            assert!(!q.social.ties.is_empty(), "its ties kept");
        } else if !q.alive() {
            assert!(
                q.life
                    .events
                    .iter()
                    .any(|e| matches!(e.event, Event::Died { .. })),
                "a death in its record"
            );
        }
    }
    let new: Vec<&hearth_people::Person> = p
        .members(band)
        .filter(|q| q.alive() && q.life.born > now.day)
        .collect();
    println!(
        "five years away: {stayed} of {} still there, {} born",
        before.len(),
        new.len()
    );
    assert!(stayed > before.len() / 2, "most still there");
    for c in &new {
        let mother = c.life.mother.and_then(|m| p.get(m)).expect("a mother");
        assert!(mother.life.female, "born to its mother");
        assert!(
            c.life
                .events
                .iter()
                .any(|e| matches!(e.event, Event::Born { .. }))
        );
    }
}

#[test]
fn a_band_met_for_the_first_time_has_coherent_families() {
    let b = base();
    let species = &b.species;
    let k = species.index_of("homo_sapiens").expect("our species");
    let sp = &species.list[k];
    let mut world = Savanna::new();
    let mut p = People::new(81);
    let now = world.now();
    let band = p.spawn_band(
        species,
        &b.graph,
        &b.items,
        &mut world,
        k,
        [8, 7, 10, 3],
        DVec3::new(2.0, GROUND, 2.0),
        now,
    );
    let maturity = sp.life.maturity_years as f64;
    let members: Vec<&hearth_people::Person> = p.members(band).filter(|q| q.alive()).collect();
    let mut siblings = 0;
    for q in &members {
        let age = q.age(&now);
        let Some(m) = q.life.mother.and_then(|m| p.get(m)) else {
            assert!(
                q.stage(sp, &now) == Stage::Adult,
                "only the grown may have no mother on record"
            );
            continue;
        };
        assert!(m.life.female, "a mother is a woman");
        let at_birth = m.age(&now) - age;
        assert!(
            at_birth >= maturity - 0.01 && at_birth <= 46.0,
            "a mother {at_birth:.1} at the birth"
        );
        // Her other children: a year and a half apart at least.
        for o in members
            .iter()
            .filter(|o| o.id != q.id && o.life.mother == q.life.mother)
        {
            siblings += 1;
            assert!(
                (o.age(&now) - age).abs() >= 1.49,
                "births spaced: {:.2} and {age:.2}",
                o.age(&now)
            );
        }
    }
    println!(
        "{} living, {siblings} sibling ties (counted both ways)",
        members.len()
    );
    assert!(siblings > 0, "brothers and sisters among them");
}

#[test]
fn the_people_lived_in_full_keep_within_budget_and_the_long_dead_are_pruned() {
    let b = base();
    let species = &b.species;
    let k = species.index_of("homo_sapiens").expect("our species");
    let mut world = Savanna::new();
    let mut p = People::new(82);
    let now = world.now();
    // Eight bands of forty about the player: more than the budget.
    for n in 0..8 {
        p.spawn_band(
            species,
            &b.graph,
            &b.items,
            &mut world,
            k,
            [14, 14, 10, 2],
            DVec3::new(40.0 * n as f64, GROUND, 0.0),
            now,
        );
    }
    let full = |p: &People| {
        p.persons
            .iter()
            .filter(|q| q.tier == Tier::Full && q.alive())
            .count()
    };
    assert!(full(&p) > hearth_people::sim::FULL_BUDGET);
    p.hold_full_budget(&[DVec3::new(0.0, GROUND, 0.0)], 0.0, now);
    println!("{} lived in full after the budget", full(&p));
    assert!(full(&p) <= hearth_people::sim::FULL_BUDGET, "within budget");
    // The nearest band is among those still in full; the farthest is not.
    let nearest = p.bands[0].tier;
    let farthest = p.bands[7].tier;
    assert_eq!(nearest, Tier::Full);
    assert_eq!(farthest, Tier::Household);
    // Fifty years on, those who died at the start are stubs.
    let gone: Vec<PersonId> = p.members(p.bands[0].id).take(2).map(|q| q.id).collect();
    for q in p.persons.iter_mut().filter(|q| gone.contains(&q.id)) {
        q.life.died = Some(hearth_people::Died {
            day: now.day,
            cause: Cause::Course,
        });
    }
    let later = hearth_people::Now {
        day: now.day + 50.0 * YEAR_DAYS,
        ..now
    };
    p.prune(&later);
    for id in &gone {
        let q = p.get(*id).expect("its record");
        assert!(q.life.stub && q.genome.is_none() && q.social.ties.is_empty());
        assert!(q.life.died.is_some(), "its dates kept");
    }
}

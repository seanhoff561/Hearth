//! H3 (V2.1 §7): the young play — chasing one another, romping about, never far from their
//! mothers — and go to crouch by the grown at their work, watching it and trying it after them,
//! and come to understand it.

mod common;

use common::*;
use glam::DVec3;
use hearth_items::Stack;
use hearth_people::{Doing, LifeStage, People, Person};

/// How far along it is toward knowing what it does not yet know.
fn insight(q: &Person) -> f32 {
    q.knowledge.insight.values().sum::<f32>() + q.knowledge.known.len() as f32
}

#[test]
fn the_young_play_and_learn_by_watching() {
    let b = base();
    let mut world = Savanna::new();
    // Marula stones to crack at the anvil.
    world.pile.add(
        DVec3::new(19.0, GROUND, 1.0),
        Stack::of(&item("marula_stone"), 30),
    );
    let k = b.species.index_of("australopithecus").expect("the hominin");
    let sp = &b.species.list[k];
    let mut p = People::new(5);
    let now = world.now();
    p.spawn_band(
        &b.species,
        &b.graph,
        &b.items,
        &mut world,
        k,
        [4, 3, 6, 0],
        DVec3::new(2.0, GROUND, 2.0),
        now,
    );
    let young: Vec<u64> = p
        .full()
        .filter(|q| {
            matches!(
                q.life_stage(sp, &now),
                LifeStage::Juvenile | LifeStage::Adolescent
            )
        })
        .map(|q| q.id)
        .collect();
    assert!(!young.is_empty(), "the band has young");
    // They have yet to learn the band's ways.
    for q in p.persons.iter_mut().filter(|q| young.contains(&q.id)) {
        q.knowledge = Default::default();
    }
    let before: Vec<f32> = young
        .iter()
        .map(|id| insight(p.get(*id).expect("young")))
        .collect();
    let (mut playing, mut imitating) = (0, 0);
    let mut works = 0;
    let (mut from_mother, mut counted) = (0.0, 0.0);
    let mut workers = std::collections::BTreeSet::new();
    let mothers: Vec<Option<u64>> = young
        .iter()
        .map(|id| p.get(*id).expect("young").life.mother)
        .collect();
    let mut nearest = f64::MAX;
    for _ in 0..(DAY_S / DT) as u64 {
        step(&mut p, &mut world, &[]);
        let at_work: Vec<DVec3> = p
            .full()
            .filter(|q| matches!(q.mind.doing, Doing::Working { .. }))
            .map(|q| q.place.pos)
            .collect();
        works += at_work.len();
        for q in p
            .full()
            .filter(|q| matches!(q.mind.doing, Doing::Working { .. }))
        {
            workers.insert(q.id);
        }
        for (k, id) in young.iter().enumerate() {
            let y = p.get(*id).expect("young").place.pos;
            if let Some(m) = mothers[k].and_then(|m| p.get(m)) {
                from_mother += (m.place.pos - y).length();
                counted += 1.0;
            }
            for w in &at_work {
                nearest = nearest.min((*w - y).length());
            }
        }
        for id in &young {
            match p.get(*id).expect("young").mind.doing {
                Doing::Playing { .. } => playing += 1,
                Doing::Imitating { .. } => imitating += 1,
                _ => {}
            }
        }
    }
    let after: Vec<f32> = young
        .iter()
        .map(|id| insight(p.get(*id).expect("young")))
        .collect();
    println!(
        "the young {:.1} m from their mothers on average",
        from_mother / counted
    );
    println!(
        "{works} person-steps of work by {workers:?}; the young's mothers {mothers:?}; the young came within {nearest:.1} m of it"
    );
    println!(
        "{} young: {playing} steps at play, {imitating} watching the work; insight {before:?} -> {after:?}",
        young.len()
    );
    assert!(playing > 100, "the young played");
    assert!(imitating > 0, "the young watched the work");
    assert!(
        after.iter().zip(&before).any(|(a, b)| a > b),
        "watching gave insight"
    );
}

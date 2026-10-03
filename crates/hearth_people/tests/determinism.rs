//! H0 (V2.1 §3): the people are deterministic. The same seed and the same inputs make the same
//! people; the number of threads deciding at once does not change them; another seed makes other
//! people.

mod common;

use common::*;
use glam::DVec3;
use hearth_items::Stack;
use hearth_people::{People, PlayerSeen};

/// A day of a band's life — marula stones to crack, a player coming near and standing calmly,
/// then running at them — as its save's JSON, on `threads` threads, the world's seed `seed`.
fn a_day(threads: usize, seed: u64) -> String {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("a pool of threads");
    pool.install(|| {
        let b = base();
        let mut world = Savanna::new();
        world.pile.add(
            DVec3::new(19.0, GROUND, 1.0),
            Stack::of(&item("marula_stone"), 30),
        );
        let mut p = People::new(seed);
        let k = b.species.index_of("australopithecus").expect("the hominin");
        let now = world.now();
        p.spawn_band(
            &b.species,
            &b.graph,
            &b.items,
            &mut world,
            k,
            [4, 3, 2, 1],
            DVec3::new(2.0, GROUND, 2.0),
            now,
        );
        let calm = PlayerSeen::standing(ONE, DVec3::new(-40.0, GROUND, -40.0));
        let running = PlayerSeen {
            running: true,
            ..PlayerSeen::standing(TWO, DVec3::new(-15.0, GROUND, -15.0))
        };
        live(&mut p, &mut world, DAY_S * 0.4, &[]);
        live(&mut p, &mut world, 300.0, &[calm]);
        live(&mut p, &mut world, 40.0, &[calm, running]);
        live(&mut p, &mut world, DAY_S * 0.6, &[]);
        let json = hearth_people::save::to_json(&p.to_save()).expect("saved");
        String::from_utf8(json).expect("JSON is text")
    })
}

#[test]
fn the_same_seed_and_inputs_make_the_same_people() {
    let a = a_day(2, 7);
    let b = a_day(2, 7);
    assert!(a.len() > 1000, "a day of people saved");
    assert_eq!(a, b);
}

#[test]
fn one_thread_or_many_make_the_same_people() {
    assert_eq!(a_day(1, 7), a_day(4, 7));
}

#[test]
fn another_seed_makes_other_people() {
    assert_ne!(a_day(2, 7), a_day(2, 8));
}

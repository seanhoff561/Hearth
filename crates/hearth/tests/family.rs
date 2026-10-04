//! H3 (V2.1 Addendum A, D180): a new world's player is born into a family of the place — their
//! mother and father and their brothers and sisters live beside them as people of the world.

mod common;

use common::*;

#[test]
fn the_player_is_born_into_a_family_beside_them() {
    let dir = temp("family");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::default(), 7);
    // The land about the player is made on workers; the family is set down once it is.
    let t0 = std::time::Instant::now();
    while w.people.is_empty() && t0.elapsed().as_secs() < 90 {
        w.run(40);
    }
    let me = w.mover.pos;
    let near: Vec<&hearth_people::PersonView> = w
        .people
        .iter()
        .filter(|v| (v.pos - me).length() < 40.0 && !v.dead)
        .collect();
    println!(
        "{} near the player: {:?}",
        near.len(),
        near.iter()
            .map(|v| (v.female, v.grown, v.plan))
            .collect::<Vec<_>>()
    );
    let grown = near.iter().filter(|v| v.grown >= 1.0).count();
    assert!(grown >= 2, "the player's mother and father are by them");
    assert!(
        near.iter().any(|v| v.female && v.grown >= 1.0)
            && near.iter().any(|v| !v.female && v.grown >= 1.0),
        "a mother and a father"
    );
    // What the player puts down is theirs: the family leaves it be (V2.1 §8.4).
    w.give("hearth:cobble/basalt", 2);
    w.put_down_all();
    w.run(20);
    let mine = |w: &World| {
        w.lying
            .iter()
            .filter(|t| t.stack.id == "hearth:cobble/basalt" && t.owner == Some(0))
            .count()
    };
    let put = mine(&w);
    assert!(put > 0, "put down as the player's");
    w.run(2400);
    assert_eq!(mine(&w), put, "still lying where they were put");
}

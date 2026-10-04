//! H3 (V2.1 Addendum A, D181): a scripted player is born a newborn into their family, is carried
//! by their mother, goes on through the moments of their childhood with the years passing
//! between, and comes of age grown, knowing their family's ways.

mod common;

use common::*;
use hearth_protocol::{Skip, ToServer};

#[test]
fn a_player_is_born_grows_up_and_comes_of_age() {
    let dir = temp("childhood");
    let mut w = World::start_with(&dir, hearth_save::KnowledgeMode::default(), 7, true);
    // Born: the first moment, carried.
    let t0 = std::time::Instant::now();
    while w.moments.is_empty() && t0.elapsed().as_secs() < 60 {
        w.run(20);
    }
    assert_eq!(w.moments.first().map(String::as_str), Some("Born"));
    let born = w.appearance.clone().expect("how the player looks");
    assert!(born.height_m < 0.7, "a newborn: {} m", born.height_m);
    // The family set down, the newborn is held at its mother's side.
    let t0 = std::time::Instant::now();
    while w.people.is_empty() && t0.elapsed().as_secs() < 90 {
        w.run(40);
    }
    w.run(4);
    let body = w.body.clone().expect("the body");
    assert!(body.held.is_some(), "carried");
    assert!(body.scale < 0.4, "a newborn's size: {}", body.scale);
    // On to the next moment, and the next: the years pass between.
    for _ in 0..2 {
        w.server.send(ToServer::Childhood(Skip::Next));
        w.run(10);
        w.server.send(ToServer::Childhood(Skip::Next));
        w.run(60);
    }
    println!("moments so far: {:?}", w.moments);
    assert!(w.moments.len() >= 3, "{:?}", w.moments);
    // Grown up now: the years to coming of age lived at the household's pace.
    w.server.send(ToServer::Childhood(Skip::GrownUp));
    let t0 = std::time::Instant::now();
    while !w.moments.iter().any(|m| m == "Coming of age") && t0.elapsed().as_secs() < 120 {
        w.run(40);
    }
    assert!(
        w.moments.iter().any(|m| m == "Coming of age"),
        "{:?}",
        w.moments
    );
    w.server.send(ToServer::Childhood(Skip::Next));
    let t0 = std::time::Instant::now();
    while w.childhood.is_some() && t0.elapsed().as_secs() < 60 {
        w.run(20);
    }
    assert!(w.childhood.is_none(), "grown");
    let grown = w.appearance.clone().expect("how the player looks");
    println!("a newborn {} m, grown {} m", born.height_m, grown.height_m);
    assert!(grown.height_m > born.height_m * 2.5, "grew up");
    // What the family knows, the grown child knows.
    let knows = |id: &str| w.knowledge.known.keys().any(|k| k.ends_with(id));
    assert!(
        knows("digging_stick") && knows("sharp_flake"),
        "{:?}",
        w.knowledge.known.keys().collect::<Vec<_>>()
    );
}

//! H2 (V2.1 §6.2–6.3): perception and memory. People act on what they see, hear and remember:
//! a leopard is seen by day and not across the dark; where it was is believed dangerous a while,
//! and forgotten in days; each starts from its band's places and comes to know the others by
//! sight; a player first met is remembered, and goes into the life history.

mod common;

use common::*;
use glam::DVec3;
use hearth_people::person::Event;
use hearth_people::{Feeling, Happened, PlaceKind, PlayerSeen, Who};

fn most_afraid(p: &hearth_people::People) -> f32 {
    p.full()
        .map(|q| q.psyche.feeling(Feeling::Fear))
        .fold(0.0, f32::max)
}

#[test]
fn a_hunter_is_seen_by_day_and_not_across_the_dark() {
    let mut world = Savanna::new();
    world.hour = 23.0;
    let mut p = band(&mut world);
    let leopard = DVec3::new(2.0, GROUND, 50.0);
    world.leopard = Some(leopard);
    live(&mut p, &mut world, 3.0, &[]);
    let night = most_afraid(&p);
    println!("fear by night: {night:.2}");
    assert!(
        night < 0.05,
        "a leopard fifty metres off in the dark goes unseen"
    );
    world.hour = 11.0;
    live(&mut p, &mut world, 3.0, &[]);
    let day = most_afraid(&p);
    println!("fear by day: {day:.2}");
    assert!(day > 0.3, "by day it is seen");
    let believing = p
        .full()
        .filter(|q| q.memory.danger_at(leopard) > 0.4)
        .count();
    assert!(believing >= 5, "{believing} believe the place dangerous");
    assert!(p.full().any(|q| {
        q.memory
            .episodes
            .iter()
            .any(|e| matches!(e.what, Happened::Threat))
    }));
}

#[test]
fn a_danger_believed_is_forgotten_in_days() {
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    let leopard = DVec3::new(2.0, GROUND, 45.0);
    world.leopard = Some(leopard);
    live(&mut p, &mut world, 2.0, &[]);
    world.leopard = None;
    assert!(p.full().any(|q| q.memory.danger_at(leopard) > 0.4));
    // Ten days of the world later, nothing more seen there: let go.
    world.day += 10.0;
    live(&mut p, &mut world, 1.0, &[]);
    let left = p
        .full()
        .map(|q| q.memory.danger_at(leopard))
        .fold(0.0, f32::max);
    assert!(left < 0.05, "{left:.2}");
}

#[test]
fn they_know_their_places_and_one_another_and_remember_a_player() {
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    // The band's places start each one's mental map.
    let first = p.full().next().map(|q| q.id).expect("someone");
    let q = p.get(first).expect("them");
    assert!(q.memory.nearest(PlaceKind::Water, q.place.pos).is_some());
    assert!(q.memory.nearest(PlaceKind::Sleep, q.place.pos).is_some());
    live(&mut p, &mut world, 120.0, &[]);
    let q = p.get(first).expect("them");
    let others: Vec<u64> = p
        .full()
        .filter(|o| o.id != first && o.social.band == q.social.band)
        .map(|o| o.id)
        .collect();
    for o in &others {
        assert!(
            q.memory.familiarity(Who::Person(*o)) > 0.0,
            "{o} known by sight"
        );
    }
    // A player first seen: a thing remembered, and an event of its life.
    let player = PlayerSeen::standing(3, q.place.pos + DVec3::new(30.0, 0.0, 0.0));
    live(&mut p, &mut world, 2.0, &[player]);
    let q = p.get(first).expect("them");
    assert!(
        q.memory
            .episodes
            .iter()
            .any(|e| matches!(e.what, Happened::Met { player: 3 }))
    );
    assert!(
        q.life
            .events
            .iter()
            .any(|e| matches!(e.event, Event::Met { player: 3 }))
    );
}

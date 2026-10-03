//! H0 (V2-11 (c)–(d) on the new framework): a band of *Australopithecus* lives its days in a small
//! savanna — feeding, drinking, gathering marula stones and cracking them at the anvil, up into
//! the trees to nest at dusk and down at dawn; a leopard sends them up the trees with the alarm,
//! or, enough of them together, they face it; a player running at them is fled, a calm one
//! watched, and in time let be; one they let be learns from watching their work.

mod common;

use common::*;
use glam::DVec3;
use hearth_fauna::live::Medium;
use hearth_items::Stack;
use hearth_people::{Doing, People, PlayerSeen};

#[test]
fn a_band_lives_its_days() {
    let b = base();
    let mut world = Savanna::new();
    // Marula stones under the tree.
    world.pile.add(
        DVec3::new(19.0, GROUND, 1.0),
        Stack::of(&item("marula_stone"), 30),
    );
    let mut p = band(&mut world);
    let mut cracked = 0;
    let mut slept_in_trees = 0;
    for _ in 0..3 {
        // The day's waking hours, then the night.
        let mut at_night = 0;
        for _ in 0..(DAY_S / DT) as u64 {
            step(&mut p, &mut world, &[]);
            cracked += p
                .done
                .iter()
                .filter(|d| {
                    d.triggers
                        .iter()
                        .any(|t| t.ends_with("crack_marula_stones"))
                })
                .count();
            if world.hour > 22.0 || world.hour < 4.0 {
                at_night =
                    at_night.max(p.full().filter(|a| a.place.medium == Medium::Tree).count());
            }
        }
        slept_in_trees = slept_in_trees.max(at_night);
    }
    let alive = p.full().count();
    assert_eq!(alive, 10, "all ten lived");
    for a in p.full() {
        let k = b.species.get(&a.species).expect("its species");
        let s = a.body.status(k.body(a.life.female));
        assert!(
            !matches!(
                s.thirst,
                hearth_body::Thirst::Parched | hearth_body::Thirst::Dying
            ),
            "#{} parched",
            a.id
        );
        assert!(
            !matches!(s.hunger, hearth_body::Hunger::Starving),
            "#{} starving",
            a.id
        );
    }
    assert!(world.nests > 0, "nests made");
    assert!(
        slept_in_trees >= alive / 2,
        "{slept_in_trees} of {alive} up the trees at night"
    );
    assert!(cracked > 0, "marula stones cracked at the anvil");
}

#[test]
fn a_leopard_sends_them_up_the_trees_with_the_alarm() {
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    live(&mut p, &mut world, 30.0, &[]);
    // A leopard comes up close.
    world.leopard = Some(DVec3::new(-8.0, GROUND, 25.0));
    live(&mut p, &mut world, 60.0, &[]);
    assert!(world.alarms > 0, "the alarm called");
    let up = p.full().filter(|a| a.place.medium == Medium::Tree).count();
    let mobbing = p
        .full()
        .filter(|a| matches!(a.mind.doing, Doing::Mobbing { .. }))
        .count();
    assert!(up + mobbing > 0, "they took to the trees or faced it");
    // It goes; in time they are down and about again.
    world.leopard = None;
    live(&mut p, &mut world, 400.0, &[]);
    let still_up = p.full().filter(|a| a.place.medium == Medium::Tree).count();
    assert!(still_up < p.full().count(), "they came down");
}

#[test]
fn a_running_player_is_fled_and_a_calm_one_watched() {
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    live(&mut p, &mut world, 20.0, &[]);
    // A player standing quietly some 70 m off: watched.
    let calm = PlayerSeen::standing(ONE, DVec3::new(-48.0, GROUND, -48.0));
    live(&mut p, &mut world, 10.0, &[calm]);
    let watching = p
        .full()
        .filter(|a| matches!(a.mind.doing, Doing::Watching { .. }))
        .count();
    assert!(watching > 0, "watched");
    // Running at them from 30 m: fled.
    let running = PlayerSeen {
        running: true,
        ..PlayerSeen::standing(ONE, DVec3::new(-15.0, GROUND, -20.0))
    };
    live(&mut p, &mut world, 10.0, &[running]);
    let fleeing = p
        .full()
        .filter(|a| {
            matches!(a.mind.doing, Doing::Fleeing { .. } | Doing::Mobbing { .. })
                || a.place.medium == Medium::Tree
        })
        .count();
    assert!(fleeing > 0, "fled");
}

#[test]
fn a_calm_player_comes_to_be_let_be() {
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    live(&mut p, &mut world, 20.0, &[]);
    // Standing calmly some 70 m off: watched at first.
    let calm = PlayerSeen::standing(ONE, DVec3::new(-48.0, GROUND, -48.0));
    let watching = |p: &People| {
        p.full()
            .filter(|a| matches!(a.mind.doing, Doing::Watching { .. }))
            .count()
    };
    live(&mut p, &mut world, 10.0, &[calm]);
    let at_first = watching(&p);
    assert!(at_first > 0, "watched at first");
    // Two play days of their company: they come to tolerate the player.
    live(&mut p, &mut world, 2.0 * DAY_S, &[calm]);
    let ease = p.bands[0].tolerance_of(ONE);
    println!("watched by {at_first} at first; at ease {ease:.2} after two days");
    assert!(ease > 0.4, "at ease {ease:.2}");
    // Running at them undoes it.
    let running = PlayerSeen {
        running: true,
        ..PlayerSeen::standing(ONE, DVec3::new(-20.0, GROUND, -20.0))
    };
    live(&mut p, &mut world, 20.0, &[running]);
    let after = p.bands[0].tolerance_of(ONE);
    assert!(after < ease - 0.2, "at ease {after:.2} after the run");
}

#[test]
fn a_watcher_learns_from_their_knapping() {
    let b = base();
    let mut world = Savanna::new();
    // Cobbles to knap by the anvil, and a hammer more.
    for k in 0..4 {
        world.pile.add(
            DVec3::new(18.4 + 0.3 * k as f64, GROUND, 2.6),
            Stack::of(&item("cobble/basalt"), 1),
        );
    }
    let mut p = band(&mut world);
    // A band at ease with the watcher, who stands some 30 m off, facing the anvil.
    for g in p.bands.iter_mut() {
        g.set_tolerance(ONE, 0.9);
    }
    let eye = DVec3::new(18.0, GROUND + 1.6, -30.0);
    let yaw = 0.0;
    let watcher_at = PlayerSeen::standing(ONE, DVec3::new(eye.x, GROUND, eye.z));
    let mut watcher = hearth_craft::KnowledgeState::default();
    let mut heard: std::collections::HashMap<String, u64> = Default::default();
    // What is seen heard an hour apart at most, as the game hears it.
    let hour = (DAY_S / 24.0 / DT) as u64;
    let mut seen_done = 0;
    for n in 0..(2.0 * DAY_S / DT) as u64 {
        step(&mut p, &mut world, &[watcher_at]);
        let seen = hearth_people::seen(&p.done, &b.graph, &b.crafts, eye, yaw);
        seen_done += seen.len();
        for t in seen {
            let fresh = heard.get(&t).is_none_or(|&last| n - last >= hour);
            if fresh {
                heard.insert(t.clone(), n);
                watcher.observe(&b.graph, &t, n, hearth_craft::Mode::Discovery);
            }
        }
    }
    let node = |id: &str| {
        b.graph
            .nodes
            .iter()
            .find(|k| k.id.ends_with(id))
            .map(|k| k.id.clone())
            .expect("the node")
    };
    let toward = |id: &str| {
        let id = node(id);
        if watcher.knows(&id) {
            1.0
        } else {
            watcher.insight.get(&id).copied().unwrap_or(0.0)
        }
    };
    println!(
        "{seen_done} things seen done; heard {:?}; stone as a hammer {:.2}, sharp flakes {:.2}",
        heard.keys().collect::<Vec<_>>(),
        toward("stone_as_hammer"),
        toward("sharp_flake")
    );
    assert!(
        toward("stone_as_hammer") > 0.0,
        "watching gave insight into the stone as a hammer"
    );
    assert!(
        toward("sharp_flake") > 0.0,
        "watching gave insight toward knapping flakes"
    );
}

#[test]
fn each_player_is_known_in_their_own_right() {
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    live(&mut p, &mut world, 20.0, &[]);
    // A play day of one player's calm company: that one is let be, the other not.
    let one = PlayerSeen::standing(ONE, DVec3::new(-48.0, GROUND, -48.0));
    live(&mut p, &mut world, DAY_S, &[one]);
    let b = &p.bands[0];
    assert!(
        b.tolerance_of(ONE) > 0.2,
        "at ease with one: {:.2}",
        b.tolerance_of(ONE)
    );
    assert_eq!(b.tolerance_of(TWO), 0.0, "never met the other");
    // Both near, the first calm, the second running at them: they flee the second.
    let two = PlayerSeen {
        running: true,
        ..PlayerSeen::standing(TWO, DVec3::new(-15.0, GROUND, -15.0))
    };
    live(&mut p, &mut world, 10.0, &[one, two]);
    let fleeing = p
        .full()
        .filter(|a| {
            matches!(a.mind.doing, Doing::Fleeing { .. } | Doing::Mobbing { .. })
                || a.place.medium == Medium::Tree
        })
        .count();
    assert!(fleeing > 0, "the running one is fled");
    // And what one learned of the second is the second's alone.
    assert!(p.bands[0].tolerance_of(ONE) > 0.2);
}

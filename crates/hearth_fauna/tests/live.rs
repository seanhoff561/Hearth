//! Animals in the world (V2-7 (c)): near the player the groups become animals on the ground,
//! they graze, wander and flee a person who comes too near, and they fold back into their
//! groups as the player leaves — the ones killed staying gone.

use std::sync::Arc;

use glam::DVec3;
use hearth_fauna::ecology::{Ecology, REGION_M};
use hearth_fauna::habitat::{Uniform, temperate_wood};
use hearth_fauna::live::{Act, Footing, Ground, Live, NEAR_M, Now};
use hearth_fauna::mind::Presence;
use hearth_fauna::species::Catalog;

/// Flat ground at y = 64 everywhere within reach.
struct Flat;

impl Ground for Flat {
    fn footing(&self, _x: f64, _z: f64, _y: f64) -> Option<Footing> {
        Some(Footing::dry(64.0))
    }

    fn top(&self, x: f64, z: f64) -> Option<Footing> {
        self.footing(x, z, 64.0)
    }
}

fn wood() -> Ecology {
    let cat = Arc::new(Catalog::new(&hearth_content::Content::load_base()));
    let land = Uniform {
        habitat: temperate_wood(&cat),
        cells_around: 4096,
    };
    let mut eco = Ecology::new(cat, 7, 0.0, &land);
    eco.ensure_region(&land, (3, 0), 0.0);
    eco
}

/// A group's members in the population.
fn members(eco: &Ecology, id: u64) -> u32 {
    eco.regions
        .values()
        .flat_map(|r| r.groups.iter())
        .find(|g| g.id == id)
        .map_or(0, |g| g.size())
}

#[test]
fn groups_come_into_the_world_and_fold_back() {
    let mut eco = wood();
    let mut live = Live::new(7);
    // The player where a herd of red deer is.
    let deer = eco.catalog.index("red_deer").expect("red deer") as u16;
    let herd = eco
        .regions
        .values()
        .flat_map(|r| r.groups.iter())
        .find(|g| g.species == deer && g.size() > 3)
        .expect("a herd")
        .pos;
    let player = DVec3::new(herd[0] + 20.0, 64.0, herd[1]);
    live.materialize(&mut eco, &Flat, player);
    let groups: Vec<u64> = eco
        .regions
        .values()
        .flat_map(|r| r.groups.iter())
        .filter(|g| g.live)
        .map(|g| g.id)
        .collect();
    assert!(!groups.is_empty(), "some groups are near the player");
    for &g in &groups {
        let animals = live.animals.iter().filter(|a| a.group == Some(g)).count() as u32;
        assert_eq!(
            animals,
            members(&eco, g),
            "every member stands in the world"
        );
    }
    let small = live.animals.iter().filter(|a| a.cell.is_some()).count();
    println!(
        "{} groups ({} animals), {small} small animals",
        groups.len(),
        live.animals.len() - small
    );
    for a in &live.animals {
        assert!((a.pos.x - player.x).hypot(a.pos.z - player.z) < NEAR_M + 60.0);
        assert_eq!(a.pos.y, 64.0);
    }
    // A minute of life: they move, and none flees from no one.
    for _ in 0..1200 {
        live.step(&eco, &Flat, None, &Now::day(0.4), 0.05);
    }
    assert!(live.animals.iter().any(|a| a.stride > 0.0), "some walked");
    assert!(live.animals.iter().all(|a| a.act != Act::Flee));
    // One group loses an animal to a hunter.
    let g = groups[0];
    let before = members(&eco, g);
    let victim = live
        .animals
        .iter()
        .find(|a| a.group == Some(g))
        .map(|a| a.id)
        .expect("a member");
    live.kill(victim);
    // The dead one is taken from the world to lie as a carcass, whole.
    let bodies = live.take_bodies(&eco.catalog);
    assert_eq!(bodies.len(), 1);
    assert_eq!(bodies[0].left, 1.0);
    assert_eq!(bodies[0].killed_by, None);
    // The player walks away: everything folds back, the dead one excepted.
    let away = player + DVec3::new(3000.0, 0.0, 0.0);
    live.fold(&mut eco, away);
    assert!(live.animals.is_empty(), "all folded");
    assert_eq!(members(&eco, g), before - 1);
    assert!(
        eco.regions
            .values()
            .flat_map(|r| r.groups.iter())
            .all(|g| !g.live)
    );
}

#[test]
fn a_person_coming_near_puts_them_to_flight() {
    let eco = wood();
    let mut live = Live::new(7);
    let deer = eco.catalog.index("red_deer").expect("red deer") as u16;
    let at = DVec3::new(3.5 * REGION_M, 64.0, 0.5 * REGION_M);
    let id = live.place(deer, hearth_fauna::live::Stage::Adult, true, at, 0.0);
    // Someone 40 m away (inside a red deer's flight distance).
    let person = at + DVec3::new(40.0, 0.0, 0.0);
    for _ in 0..100 {
        live.step(
            &eco,
            &Flat,
            Some(&Presence::walking(person)),
            &Now::day(0.4),
            0.05,
        );
    }
    let a = live.animals.iter().find(|a| a.id == id).expect("the deer");
    assert_eq!(a.act, Act::Flee);
    assert!(a.speed > 5.0, "running at {} m/s", a.speed);
    assert!(
        (a.pos.x - person.x).abs() > 40.0 + 10.0,
        "it ran away: {:.1} m off",
        (a.pos.x - person.x).abs()
    );
}

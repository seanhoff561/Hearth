//! Animals in the world (V2-7 (c), docs/design/fauna.md): about the player the populations'
//! groups and small animals come into the world, walking the ground; as the player leaves
//! they go, folded back into their numbers.

mod common;

use common::World;
use glam::DVec3;
use hearth_fauna::species::Catalog;

#[test]
fn animals_come_into_the_world_about_the_player_and_go_as_they_leave() {
    let dir = common::temp("fauna");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 7);
    let catalog = Catalog::new(&w.content);
    let home = w.mover.pos;
    // The populations about the spawn, once made: where their groups are.
    let mut groups = w.census_about();
    println!("{} groups about the spawn", groups.len());
    assert!(!groups.is_empty(), "the land about the spawn holds animals");
    groups.sort_by(|a, b| {
        let d = |p: glam::DVec2| (p.x - home.x).hypot(p.y - home.z);
        d(a.1).total_cmp(&d(b.1))
    });
    // Walking to the nearest groups until some are met.
    let mut met = Vec::new();
    'walk: for (_, at, _) in groups.iter().take(6) {
        w.go(at.x, at.y);
        for _ in 0..4 {
            w.run(40);
            if !w.animals.is_empty() {
                met = w.animals.clone();
                break 'walk;
            }
        }
    }
    assert!(!met.is_empty(), "no animals met about the spawn");
    let at = w.mover.pos;
    for v in &met {
        let sp = &catalog.species[v.species as usize];
        let d = ((v.pos.x - at.x).powi(2) + (v.pos.z - at.z).powi(2)).sqrt();
        println!(
            "{:<24} {:?} {} at {d:.0} m",
            sp.name,
            v.stage,
            if v.female { "f" } else { "m" }
        );
        assert!(d < 160.0, "{} is {d:.0} m off", sp.name);
        // On the ground: within a few metres of the surface.
        let ground = w.generator.terrain.sample(v.pos.x as i32, v.pos.z as i32);
        assert!(
            (v.pos.y - ground.height as f64).abs() < 12.0,
            "{} stands at {:.1}, the ground at {:.1}",
            sp.name,
            v.pos.y,
            ground.height
        );
    }
    // They move about over a minute of play (watched through it: one that runs off far is
    // folded away).
    let first: Vec<(u64, DVec3)> = met.iter().map(|v| (v.id, v.pos)).collect();
    let mut moved = false;
    for _ in 0..12 {
        w.run(100);
        moved |= first.iter().any(|(id, p)| {
            w.animals
                .iter()
                .find(|v| v.id == *id)
                .is_some_and(|v| (v.pos - *p).length() > 0.5)
        });
    }
    assert!(moved, "none of them moved in a minute");
    // Leaving, the player leaves them behind.
    w.go(at.x + 1500.0, at.z);
    w.run(80);
    assert!(
        w.animals
            .iter()
            .all(|v| first.iter().all(|(id, _)| v.id != *id)),
        "the animals left behind are folded away"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_dead_deer_lies_until_found_and_is_butchered_by_its_kind() {
    use hearth_protocol::{AimAt, ToServer};
    let dir = common::temp("carcass");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 7);
    w.census_about();
    let home = w.mover.pos;
    // A red deer hind dies 300 m to the east: out of sight, she is not yet in the world.
    let far = home + DVec3::new(300.0, 0.0, 0.0);
    w.server.send(ToServer::Die {
        species: "hearth:red_deer".into(),
        at: far,
    });
    w.run(81);
    assert!(w.lying.iter().all(|l| !l.stack.id.contains("_carcass")));
    // By day the ravens over her tell of her.
    if let Some((_, _, words)) = w.acted.iter().find(|(_, _, s)| s.contains("Ravens")) {
        assert!(words.contains("east"), "{words}");
    }
    // Come near, she is found, whole and fresh.
    w.go(far.x - 1.0, far.z);
    w.run(81);
    let c = w
        .lying
        .iter()
        .find(|l| l.stack.id == "hearth:red_deer_carcass")
        .expect("the carcass is found")
        .clone();
    assert!(c.stack.condition > 0.99, "{}", c.stack.condition);
    assert!(c.stack.decay < 0.2, "{}", c.stack.decay);
    // Butchered with a flint flake: the meat, hide and bone of a red deer hind.
    w.give("hearth:flake/flint", 1);
    w.put_down_all();
    let flake = w
        .lying
        .iter()
        .find(|l| l.stack.id == "hearth:flake/flint")
        .map(|l| l.id)
        .expect("the flake");
    assert!(w.hold(flake), "the flake in hand");
    let (done, words) = w.act("butcher_red_deer", AimAt::Thing(c.id));
    assert!(done, "{words}");
    let meat = w.at_hand(|id| id == "hearth:cut/meat", 6.0) as f32 * 0.254;
    let hide = w.at_hand(|id| id == "hearth:sheet/rawhide", 6.0);
    println!("{meat:.0} kg of meat, {hide} sheets of rawhide");
    assert!((40.0..60.0).contains(&meat), "{meat:.0} kg of meat");
    assert!(hide >= 7, "{hide} sheets of hide");
    assert!(
        w.lying.iter().all(|l| l.id != c.id),
        "the carcass is used up"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hunter_spears_an_animal_and_it_lies_where_it_fell() {
    use hearth_protocol::ToServer;
    let dir = common::temp("hunt");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 7);
    let catalog = Catalog::new(&w.content);
    let home = w.mover.pos;
    let mut groups = w.census_about();
    groups.sort_by(|a, b| {
        let d = |p: glam::DVec2| (p.x - home.x).hypot(p.y - home.z);
        d(a.1).total_cmp(&d(b.1))
    });
    // A spear in hand, and an animal of some size met about the spawn.
    let spear = "hearth:stone_tipped_spear/oak_wood";
    w.give(spear, 1);
    w.put_down_all();
    let id = w
        .lying
        .iter()
        .find(|l| l.stack.id == spear)
        .map(|l| l.id)
        .expect("the spear");
    assert!(w.hold(id), "the spear in hand");
    let mut quarry = None;
    'walk: for (s, at, _) in groups.iter().take(8) {
        if catalog.species[*s as usize].mass_kg < 10.0 {
            continue;
        }
        w.go(at.x, at.y);
        for _ in 0..4 {
            w.run(40);
            if let Some(v) = w.animals.iter().find(|v| {
                catalog.species[v.species as usize].mass_kg >= 10.0
                    && v.medium == hearth_fauna::live::Medium::Ground
            }) {
                quarry = Some(*v);
                break 'walk;
            }
        }
    }
    let v = quarry.expect("an animal to hunt");
    let sp = &catalog.species[v.species as usize];
    // Up beside it, and a thrust behind its shoulder.
    let ahead = DVec3::new(v.yaw.sin() as f64, 0.0, v.yaw.cos() as f64);
    let side = DVec3::new(ahead.z, 0.0, -ahead.x);
    let feet = v.pos + side * 1.6;
    w.go_exact(feet);
    // The heart and lungs: the front of the torso, a fifth of its length ahead of its feet.
    let chest =
        v.pos + ahead * (sp.length_m as f64 * 0.21) + DVec3::Y * (sp.shoulder_m as f64 * 0.65);
    let n = w.acted.len();
    w.server.send(ToServer::Thrust {
        dir: chest - (feet + DVec3::new(0.0, 1.5, 0.0)),
    });
    w.run(2);
    let said: Vec<String> = w.acted[n..].iter().map(|(_, _, s)| s.clone()).collect();
    println!("{}: {said:?}", sp.name);
    assert!(said.iter().any(|s| s.contains("strikes")), "{said:?}");
    // It runs and falls; followed to where it lies.
    let mut last = v.pos;
    for _ in 0..120 {
        w.run(20);
        match w.animals.iter().find(|x| x.id == v.id) {
            Some(x) => last = x.pos,
            None => break,
        }
    }
    w.go(last.x, last.z);
    w.run(81);
    let carcass = format!("{}_carcass", sp.id);
    assert!(
        w.lying.iter().any(|l| l.stack.id.starts_with(&carcass)),
        "no {carcass} where it fell"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

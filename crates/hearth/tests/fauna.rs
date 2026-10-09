//! Animals in the world (V2-7 (c), docs/design/fauna.md): about the player the populations'
//! groups and small animals come into the world, walking the ground; as the player leaves
//! they go, folded back into their numbers.

mod common;

use common::World;
use glam::DVec3;
use hearth_fauna::live::Medium;
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
    // Walking to the nearest groups until some are met (the large animals of the groups, not
    // only the small ones drawn about: a souslik stands and watches a person in the open as
    // long as they stand there), two at least (a red brocket alone freezes in its cover the
    // whole minute a person stands beside it).
    let mut met = Vec::new();
    'walk: for (_, at, _) in groups.iter().take(12) {
        w.go(at.x, at.y);
        for _ in 0..4 {
            w.run(40);
            let large: Vec<_> = w
                .animals
                .iter()
                .filter(|v| catalog.species[v.species as usize].grouped())
                .cloned()
                .collect();
            if large.len() >= 2 {
                met = large;
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
    // On into the morning, the birds of the day up and about (the world starts before sunrise).
    w.wait_hours(1.5);
    let home = w.mover.pos;
    // A red deer hind dies 300 m to the east: out of sight, she is not yet in the world.
    let far = home + DVec3::new(300.0, 0.0, 0.0);
    w.server.send(ToServer::Die {
        species: "hearth:red_deer".into(),
        at: far,
    });
    w.run(81);
    assert!(w.lying.iter().all(|l| !l.stack.id.contains("_carcass")));
    // By day the scavengers of the place circle over her, to be seen from afar; nothing tells of
    // them in words (Amendment T §2.1).
    let cat = Catalog::new(&w.content);
    let over = |w: &World| {
        w.flocks
            .iter()
            .find(|f| (f.at[0] - far.x).abs() < 1.0 && (f.at[2] - far.z).abs() < 1.0)
            .copied()
    };
    let flock = over(&w).expect("birds over her");
    let sp = &cat.species[flock.species as usize];
    println!("{} {} over her", flock.count, sp.name);
    assert!(hearth_fauna::flock::scavenges(sp), "{}", sp.name);
    assert!(!flock.risen, "no one near her yet");
    assert!(
        w.acted.iter().all(|(_, _, s)| !s.contains("circling")),
        "{:?}",
        w.acted
    );
    // Near her, the birds are of the world: circling over her, higher and wider with the player
    // standing by.
    w.go(far.x - 20.0, far.z);
    w.run(81);
    let birds: Vec<_> = w
        .animals
        .iter()
        .filter(|v| v.species == flock.species && v.medium == Medium::Air)
        .collect();
    assert!(!birds.is_empty(), "the birds over her are in the world");
    assert!(
        over(&w).is_none(),
        "drawn as birds of the world, not specks"
    );
    for b in &birds {
        let up = b.pos.y - w.mover.pos.y;
        assert!(up > 20.0, "{} at {up:.0} m up", sp.name);
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
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn a_carcass_found_hours_after_the_death_has_gone_off_by_those_hours() {
    use hearth_protocol::ToServer;
    let dir = common::temp("carcass-hours");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 7);
    w.census_about();
    let home = w.mover.pos;
    // A roe deer dies 300 m off; six hours later she is found. Meat keeps days, not hours: she
    // has begun to go off, but is good to butcher (the populations' years are reckoned in the
    // calendar's days, not a real year's).
    let far = home + DVec3::new(300.0, 0.0, 0.0);
    w.server.send(ToServer::Die {
        species: "hearth:roe_deer".into(),
        at: far,
    });
    w.run(81);
    w.wait_hours(6.0);
    w.go(far.x - 1.0, far.z);
    w.run(81);
    let c = w
        .lying
        .iter()
        .find(|l| l.stack.id == "hearth:roe_deer_carcass")
        .expect("the carcass is found");
    assert!(
        (0.02..0.5).contains(&c.stack.decay),
        "gone off {} in six hours",
        c.stack.decay
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
    // A spear in hand.
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
    // A grown hind a few steps off, grazing (one met about the spawn may be off at the hunter's
    // first step, as wild ones are).
    w.server.send(ToServer::Bring {
        species: "hearth:red_deer".into(),
        young: false,
        female: true,
        at: home + DVec3::new(0.0, 1.0, 4.0),
    });
    w.run(2);
    let deer = catalog.index("hearth:red_deer").expect("red deer") as u16;
    let mut v = *w
        .animals
        .iter()
        .find(|a| a.species == deer && (a.pos - home).length() < 8.0)
        .expect("the hind brought");
    let sp = &catalog.species[v.species as usize];
    // Up beside it, and a thrust behind its shoulder: it may walk on as the hunter comes up
    // (down into a hollow, along a bank), or start away as the spear is drawn back, and is
    // followed and struck again — until it falls (nothing tells of it: the hunter sees it go
    // down, or run on wounded, Amendment T §0.2).
    for attempt in 0..6 {
        let mut chest = v.pos;
        for _ in 0..4 {
            if let Some(x) = w.animals.iter().find(|x| x.id == v.id) {
                v = *x;
            }
            let ahead = DVec3::new(v.yaw.sin() as f64, 0.0, v.yaw.cos() as f64);
            let side = DVec3::new(ahead.z, 0.0, -ahead.x);
            // Beside where it will be by the time the thrust lands: a quarter of a second on, a
            // fleeing one gathering speed.
            let lead = |v: &hearth_fauna::live::AnimalView| {
                let gathering = if v.act == hearth_fauna::live::Act::Flee && v.speed < 8.0 {
                    0.5 * 6.0 * 0.25 * 0.25
                } else {
                    0.0
                };
                v.speed as f64 * 0.25 + gathering
            };
            w.go_exact(v.pos + ahead * lead(&v) + side * 1.2);
            if let Some(x) = w.animals.iter().find(|x| x.id == v.id) {
                v = *x;
            }
            let ahead = DVec3::new(v.yaw.sin() as f64, 0.0, v.yaw.cos() as f64);
            // The heart and lungs: the front of the torso, three tenths of its length ahead of
            // its feet, where it will be when the thrust lands (led, if it moves on).
            chest = v.pos
                + ahead * (sp.length_m as f64 * 0.3 + lead(&v))
                + DVec3::Y * (sp.shoulder_m as f64 * 0.65);
            if (chest - (w.mover.pos + DVec3::new(0.0, 1.5, 0.0))).length() < 2.0 {
                break;
            }
        }
        let n = w.struck.len();
        // From where the hunter stands (the bank or a reed bed may have stopped it short): the
        // thrust lands at the end of its wind-up, a fifth of a second on (E §3.2).
        w.server.send(ToServer::Blow {
            dir: chest - (w.mover.pos + DVec3::new(0.0, 1.5, 0.0)),
            kick: false,
            with: None,
        });
        w.run(1);
        if attempt == 0 {
            assert_eq!(w.struck.len(), n, "nothing before the wind-up is done");
        }
        w.run(12);
        let now = w.animals.iter().find(|x| x.id == v.id);
        println!(
            "{}: {:?}, {}",
            sp.name,
            &w.struck[n..],
            now.map_or("down".to_owned(), |x| format!(
                "{:?}, wounded {}",
                x.act, x.wounded
            ))
        );
        if now.is_none() {
            break;
        }
    }
    assert!(
        w.struck.iter().any(|(_, _, glancing)| !glancing),
        "no thrust went home: {:?}",
        w.struck
    );
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
    let found = |w: &World| w.lying.iter().any(|l| l.stack.id.starts_with(&carcass));
    if !found(&w) {
        // Struck in the belly, it lies up and dies within the hour.
        w.wait_hours(1.0);
        if let Some(x) = w.animals.iter().find(|x| x.id == v.id) {
            last = x.pos;
        }
        w.go(last.x, last.z);
        w.run(81);
    }
    assert!(found(&w), "no {carcass} where it fell");
    let _ = std::fs::remove_dir_all(&dir);
}

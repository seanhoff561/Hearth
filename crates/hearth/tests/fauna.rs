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
    w.run(80);
    w.census = None;
    w.server.send(hearth_protocol::ToServer::Census);
    assert!(w.until(30.0, |w| w.census.is_some()), "no census");
    let mut groups = w.census.clone().unwrap_or_default();
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
    // They move about over a minute of play.
    let first: Vec<(u64, DVec3)> = met.iter().map(|v| (v.id, v.pos)).collect();
    w.run(1200);
    let moved = first.iter().any(|(id, p)| {
        w.animals
            .iter()
            .find(|v| v.id == *id)
            .is_some_and(|v| (v.pos - *p).length() > 0.5)
    });
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

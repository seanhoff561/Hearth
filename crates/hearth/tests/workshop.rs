//! Making things in a running world (V2-5): processes done to blocks and things, stations laid
//! and lit, fire that cooks, digging that leaves a spoil pile, eating and drinking, and the
//! knowledge that doing things teaches; the changes outlast going away and the world being
//! saved and opened again. The world runs its ticks as fast as they go.

mod common;

use common::{World, temp};
use glam::DVec3;
use hearth_math::BlockPos;
use hearth_protocol::{AimAt, ToServer};

#[test]
fn knocking_stones_teaches_and_open_knowledge_builds_a_fire_that_cooks() {
    let dir = temp("workshop");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Discovery, 11);
    // Knocking a flint cobble with another stone, a few times, teaches what stone can do.
    w.give("hearth:cobble/flint", 1);
    w.give("hearth:cobble/granite", 1);
    for _ in 0..6 {
        w.act("knock_stones", AimAt::Nothing);
    }
    w.until(5.0, |w| w.knowledge.knows("hearth:sharp_flake"));
    assert!(
        w.knowledge.knows("hearth:stone_as_hammer"),
        "learned: {:?}",
        w.learned
    );
    assert!(w.knowledge.knows("hearth:sharp_flake"), "{:?}", w.learned);
    // Knowledge not yet had is refused.
    let (done, words) = w.act("make_hand_axe", AimAt::Nothing);
    assert!(!done, "{words}");
    drop(w);
    let _ = std::fs::remove_dir_all(&dir);

    // With all knowledge open: lay a fire, light it, roast meat on it.
    let dir = temp("workshop-open");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 11);
    let ground = w.ground();
    // A place to build: open ground next to the player.
    let site = [(2, 0), (-2, 0), (0, 2), (0, -2), (2, 2), (-2, -2)]
        .iter()
        .map(|(dx, dz)| {
            // The top solid block of that column near the feet.
            let mut p = BlockPos::new(ground.x + dx, ground.y + 2, ground.z + dz);
            for _ in 0..5 {
                let solid = w
                    .mirror
                    .block(p)
                    .is_some_and(|s| !w.reg.collision_shape(s).is_empty());
                if solid {
                    return Some(p);
                }
                p = p.down();
            }
            None
        })
        .find_map(|p| p)
        .expect("ground to build on");
    w.give("hearth:stick/oak_wood", 4);
    w.give("hearth:handful/dry_grass", 2);
    let (done, words) = w.act(
        "build_campfire",
        AimAt::Block {
            pos: site,
            top: true,
        },
    );
    assert!(done, "laid: {words}");
    let hearth = site.up();
    assert!(
        w.until(10.0, |w| w.block(hearth).as_deref() == Some("campfire")),
        "a campfire stands: {:?}",
        w.block(hearth)
    );
    w.give("hearth:ember", 1);
    let lit = loop {
        let (done, words) = w.act(
            "light_fire",
            AimAt::Block {
                pos: hearth,
                top: false,
            },
        );
        if done {
            break true;
        }
        assert!(words.contains("ember dies"), "{words}");
        w.give("hearth:ember", 1);
    };
    assert!(lit);
    let burning = w.until(30.0, |w| {
        w.mirror
            .block(hearth)
            .is_some_and(|s| matches!(w.reg.get(s, "fire"), Some("low") | Some("high")))
    });
    assert!(burning, "the fire burns");
    // A few minutes for the hearth to heat.
    let minutes = (w.ticks_per_day / 288.0) as u64;
    w.run(minutes);
    // Fed for the hour a novice takes to roast.
    for _ in 0..4 {
        w.give("hearth:stick/oak_wood", 1);
        let (done, words) = w.act(
            "feed_fire",
            AimAt::Block {
                pos: hearth,
                top: false,
            },
        );
        assert!(done, "fed: {words}");
    }
    w.give("hearth:cut/meat", 2);
    let (done, words) = w.act(
        "roast_meat",
        AimAt::Block {
            pos: hearth,
            top: false,
        },
    );
    assert!(done, "{words}");
    assert!(w.has("hearth:cut/cooked_meat") > 0, "roasted: {words}");
    // Eating it.
    let path = hearth_items::Path::at(hearth_items::Root::Hand(hearth_items::Hand::Right));
    if w.carry
        .right
        .as_ref()
        .is_some_and(|s| s.id.contains("cooked_meat"))
    {
        w.server.send(ToServer::Eat(path));
        assert!(w.until(10.0, |w| w.acted.iter().any(|(p, d, _)| p == "eat" && *d)));
    }
    // Digging by hand: a cubic metre of earth goes, the ground's surface falls where it was
    // dug (S §8.3), and a spoil pile rises beside the hole.
    let soil = w.ground();
    let before = w.block(soil);
    let above = DVec3::new(
        soil.x as f64 + 0.5,
        soil.y as f64 + 3.0,
        soil.z as f64 + 0.5,
    );
    let surface = |w: &World| {
        hearth_world::ground::raycast(&w.mirror, &w.reg, above, DVec3::NEG_Y, 8.0)
            .map_or(f64::NEG_INFINITY, |h| h.at.y)
    };
    let top = surface(&w);
    let (done, words) = w.act(
        "dig_by_hand",
        AimAt::Block {
            pos: soil,
            top: true,
        },
    );
    if done {
        assert!(
            w.until(10.0, |w| top - surface(w) > 0.7),
            "dug down: the surface from {top} to {}",
            surface(&w)
        );
        let spoil = w.until(10.0, |w| {
            (-2..=2).any(|dx: i32| {
                (-2..=2).any(|dz: i32| {
                    (-3..=3).any(|dy: i32| {
                        w.block(BlockPos::new(soil.x + dx, soil.y + dy, soil.z + dz))
                            .as_deref()
                            == Some("spoil")
                    })
                })
            })
        });
        assert!(spoil, "a spoil pile");
    } else {
        panic!("could not dig here: {words} ({before:?})");
    }

    // Going far away unloads the camp's terrain, and it is generated again on coming back: the
    // hearth, the hole and the spoil are still there.
    let dug = w.block(soil);
    let floor = surface(&w);
    let spoil_at: Vec<BlockPos> = (-2..=2)
        .flat_map(|dx: i32| (-2..=2).flat_map(move |dz: i32| (-3..=3).map(move |dy| (dx, dy, dz))))
        .map(|(dx, dy, dz)| BlockPos::new(soil.x + dx, soil.y + dy, soil.z + dz))
        .filter(|p| w.block(*p).as_deref() == Some("spoil"))
        .collect();
    let home = w.mover.pos;
    w.go(home.x + 300.0, home.z);
    w.run(20);
    assert!(
        w.until(20.0, |w| w.block(hearth).is_none()),
        "the camp's terrain was unloaded"
    );
    w.go_exact(home);
    let kept = |w: &World| {
        w.block(hearth).as_deref() == Some("campfire")
            && w.block(soil) == dug
            && (surface(w) - floor).abs() < 0.02
            && spoil_at
                .iter()
                .all(|p| w.block(*p).as_deref() == Some("spoil"))
    };
    assert!(w.until(30.0, kept), "the camp is as it was left");
    // Saved and opened again.
    drop(w);
    let w = World::start(&dir, hearth_save::KnowledgeMode::Open, 11);
    assert!(
        {
            let mut w = w;
            let ok = w.until(30.0, kept);
            drop(w);
            ok
        },
        "the camp is as it was left after saving"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Work stopped part way keeps what is done (E §7.2): digging a while, stopping and taking it up
/// again goes on from where it was left.
#[test]
fn work_left_part_done_is_taken_up_where_it_was_left() {
    let dir = temp("workshop-begun");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 11);
    let ground = w.ground();
    let aim = AimAt::Block {
        pos: ground,
        top: true,
    };
    w.server.send(ToServer::Act {
        process: "hearth:dig_by_hand".into(),
        aim,
        hand: None,
        with: None,
    });
    // Some minutes of digging (of four hours' work).
    w.run(20 * 60 * 10);
    w.pump();
    let before = w.work_done.expect("digging");
    assert!(before > 0.02, "{before}");
    // Let go: the work stops; what is done stays done.
    w.server.send(ToServer::StopWork);
    w.run(20);
    w.pump();
    assert!(!w.working);
    w.server.send(ToServer::Act {
        process: "hearth:dig_by_hand".into(),
        aim,
        hand: None,
        with: None,
    });
    w.run(2);
    w.pump();
    let again = w.work_done.expect("digging again");
    assert!(again >= before, "taken up at {again}, left at {before}");
    drop(w);
    let _ = std::fs::remove_dir_all(&dir);
}

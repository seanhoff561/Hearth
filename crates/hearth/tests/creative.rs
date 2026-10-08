//! The game modes and Creative (Amendment P §2–3, as amended by Amendment E §9.1): Realistic and
//! Easy keep watching, time, the weather and Creative's powers from the player; Creative cannot
//! be hurt, its inventory takes, places, plants, summons, builds and teaches every category, its
//! remove tool takes what is looked at away, and spectating far off streams the world about the
//! eye and resuming sets the body on the ground there.

mod common;

use common::World;
use glam::DVec3;
use hearth::creative::{Category, catalog};
use hearth_math::BlockPos;
use hearth_physics::Motion;
use hearth_protocol::{AimAt, CreativeAct, Moved, ToServer};

fn dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("hearth-creative-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// A hard landing and long under water, as the client would report them.
fn harm(w: &mut World) {
    w.server.send(ToServer::Moved(Moved {
        mover: w.mover,
        landed: Some(25.0),
        motion: Motion::Still,
        speed: 0.0,
        straining: false,
        immersion: 1.0,
        airless_s: 600.0,
        yaw: 0.0,
        grade: 0.0,
    }));
}

#[test]
fn realistic_and_easy_have_no_watching_time_or_creative_powers() {
    for mode in ["realistic", "easy"] {
        let d = dir(mode);
        let mut w = World::start_mode(&d, mode, 21);
        let at = w.mover.pos;
        let t0 = w.ticks;
        w.server.send(ToServer::SkipHours(10.0));
        w.server.send(ToServer::TimeWarp(5000.0));
        w.run(20);
        assert!(w.ticks - t0 < 100, "{mode}: time moved by {}", w.ticks - t0);
        // Neither watching from afar, nor being put somewhere else.
        let far = at + DVec3::new(300.0, 30.0, 300.0);
        w.server.send(ToServer::Observe(Some(far)));
        w.server.send(ToServer::Place(far));
        w.run(40);
        assert!((w.mover.pos - at).length() < 1.0, "{mode}: moved");
        let there = BlockPos::containing(far);
        assert!(
            (-40..40).all(|dy| w
                .mirror
                .block(BlockPos::new(there.x, there.y + dy, there.z))
                .is_none()),
            "{mode}: the world streamed about an eye it may not have"
        );
        // Creative's inventory refused.
        w.server.send(ToServer::Creative {
            act: CreativeAct::Take {
                item: "hearth:ember".into(),
                count: 4,
            },
            aim: AimAt::Nothing,
        });
        w.run(4);
        assert_eq!(w.has("hearth:ember"), 0, "{mode}: Creative's take");
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[test]
fn creative_cannot_be_hurt_and_its_inventory_holds_every_category() {
    let d = dir("powers");
    let mut w = World::start_mode(&d, "creative", 21);
    // Invulnerable: a fall that breaks bones and ten minutes under water leave it whole.
    harm(&mut w);
    w.run(60);
    let b = w.body.clone().expect("a body");
    assert!(
        b.dead.is_none() && b.injuries.is_empty(),
        "{:?} {:?}",
        b.dead,
        b.injuries
    );
    assert!(b.status.stamina > 0.9);

    let all = catalog(&w.content);
    let first = |c: Category, pred: &dyn Fn(&str) -> bool| {
        all.iter()
            .find(|e| e.category == c && pred(&e.id))
            .unwrap_or_else(|| panic!("no {c:?}"))
            .id
            .clone()
    };
    // Take: into the hands.
    let flint = first(Category::Items, &|id| id.contains("flint"));
    w.server.send(ToServer::Creative {
        act: CreativeAct::Take {
            item: flint.clone(),
            count: 4,
        },
        aim: AimAt::Nothing,
    });
    w.run(4);
    assert!(w.at_hand(|id| id == flint, 3.0) >= 4, "took no {flint}");

    // Each of Place, Plant and Build on its own patch of ground, east of the player.
    let spot = |w: &mut World, dx: f64| {
        let p = w.mover.pos + DVec3::new(dx, 3.0, 0.0);
        let g = w.ground_under(p).expect("ground");
        (g, AimAt::Block { pos: g, top: true })
    };
    let rock = first(Category::Terrain, &|id| !id.contains("cobbles"));
    let (g, aim) = spot(&mut w, 3.0);
    w.server.send(ToServer::Creative {
        act: CreativeAct::Place {
            block: rock.clone(),
        },
        aim,
    });
    w.run(6);
    let bare = |id: &str| id.rsplit(':').next().unwrap_or(id).to_owned();
    assert_eq!(
        w.block(g.up()).as_deref(),
        Some(bare(&rock).as_str()),
        "placed {rock}"
    );

    let tree = first(Category::Plants, &|id| id.ends_with("english_oak"));
    let (g, aim) = spot(&mut w, -4.0);
    w.server.send(ToServer::Creative {
        act: CreativeAct::Plant {
            species: tree.clone(),
            young: false,
        },
        aim,
    });
    w.run(6);
    let trunk = w.block(g.up()).unwrap_or_default();
    assert!(trunk.contains("oak"), "planted {tree}: {trunk}");

    let piece = first(Category::Building, &|id| id.contains('/'));
    let (g, aim) = spot(&mut w, 6.0);
    w.server.send(ToServer::Creative {
        act: CreativeAct::Build {
            piece: piece.clone(),
        },
        aim,
    });
    w.run(6);
    assert_eq!(
        w.block(g.up()).as_deref(),
        Some(bare(&piece).as_str()),
        "built {piece}"
    );

    // The remove tool takes the rock away again.
    let (g, _) = spot(&mut w, 3.0);
    w.server
        .send(ToServer::Remove(AimAt::Block { pos: g, top: true }));
    w.run(6);
    assert!(
        w.ground_under(w.mover.pos + DVec3::new(3.0, 3.0, 0.0)) != Some(g),
        "the placed rock stands"
    );

    // Summon: a herd of four beside the player.
    let deer = first(Category::Animals, &|id| id.ends_with("red_deer"));
    let near = |w: &World| {
        w.animals
            .iter()
            .filter(|a| (a.pos - w.mover.pos).length() < 15.0)
            .count()
    };
    let before = near(&w);
    w.server.send(ToServer::Creative {
        act: CreativeAct::Summon {
            species: deer,
            female: true,
            young: false,
            count: 4,
        },
        aim: AimAt::Nothing,
    });
    w.run(10);
    assert!(
        near(&w) >= before + 4,
        "summoned {} of 4",
        near(&w) - before
    );

    // Learn: a piece of knowledge known at once.
    let node = first(Category::Knowledge, &|_| true);
    w.server.send(ToServer::Creative {
        act: CreativeAct::Learn {
            node: node.clone(),
            known: true,
        },
        aim: AimAt::Nothing,
    });
    w.run(6);
    assert!(w.knowledge.known.contains_key(&node), "learned {node}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn spectating_far_off_streams_the_world_there_and_resuming_stands_the_body_on_it() {
    let d = dir("spectate");
    let mut w = World::start_mode(&d, "creative", 21);
    let home = w.mover.pos;
    let x = home.x + 400.0;
    let z = home.z + 400.0;
    let ground = w
        .generator
        .terrain
        .sample(x.floor() as i32, z.floor() as i32)
        .height as f64;
    let eye = DVec3::new(x, ground.max(0.0) + 20.0, z);
    w.server.send(ToServer::Observe(Some(eye)));
    // The full-detail world about the eye, as about a player.
    let column = |w: &World| {
        (-60..20).any(|dy| {
            w.mirror
                .block(BlockPos::new(x as i32, eye.y as i32 + dy, z as i32))
                .is_some()
        })
    };
    let mut streamed = false;
    for _ in 0..30 {
        w.run(20);
        if column(&w) {
            streamed = true;
            break;
        }
    }
    assert!(streamed, "no terrain streamed about the eye");
    // Resume here: off watching, the body set down on the ground below the eye.
    w.server.send(ToServer::Observe(None));
    w.server.send(ToServer::Place(eye));
    w.run(10);
    let at = w.mover.pos;
    assert!(
        (at.x - x).abs() < 40.0 && (at.z - z).abs() < 40.0,
        "set down at {at}, not about {eye}"
    );
    w.until(10.0, |w| {
        w.ground_under(w.mover.pos + DVec3::Y * 0.1).is_some()
    });
    // On the ground, once settled: on a block's top, or on the smooth ground's surface (S §8.1:
    // inside it just under the feet, out of it just over them).
    w.run(20);
    let at = w.mover.pos;
    let feet = BlockPos::containing(at + DVec3::Y * 0.1);
    let field = |p: DVec3| hearth_world::ground::field(&w.mirror, &w.reg, p);
    let on_block = w.free(feet) && w.solid(feet.down());
    let on_ground = field(at - DVec3::Y * 0.08) > 0.0 && field(at + DVec3::Y * 0.1) <= 0.0;
    assert!(
        on_block || on_ground,
        "not standing on the ground at {at}: field {} under, {} over; blocks {:?} {:?}",
        field(at - DVec3::Y * 0.08),
        field(at + DVec3::Y * 0.1),
        w.mirror.block(feet),
        w.mirror.block(feet.down())
    );
    let _ = std::fs::remove_dir_all(&d);
}

//! Building (V2-8, docs/design/building.md): construction pieces are put up from what they are
//! made of — a post from a pole, a beam lashed from it facing the way the builder faces, bark
//! laid on the slope beside the beam, brush piled against the wind — each a block of its
//! material, in stages (a frame before what hangs on it), and taken down again for most of what
//! it took.

mod common;

use common::World;
use glam::DVec3;
use hearth_math::{BlockPos, Direction};
use hearth_protocol::AimAt;

/// The block's name at a place (without its namespace) and its facing.
fn piece_at(w: &World, p: BlockPos) -> (String, Option<String>) {
    let s = w.mirror.block(p).expect("loaded");
    let name = w.reg.block_of(s).name.path().to_owned();
    (name, w.reg.get(s, "facing").map(str::to_owned))
}

/// Open ground beside the player and in reach, with three blocks of room over it.
fn open_ground(w: &World) -> BlockPos {
    let feet = w.mover.pos;
    let eye = feet + DVec3::new(0.0, 1.6, 0.0);
    let here = BlockPos::containing(feet);
    for (dx, dz) in [
        (2, 0),
        (0, 2),
        (-2, 0),
        (0, -2),
        (2, 1),
        (1, 2),
        (-2, 1),
        (1, -2),
    ] {
        let Some(g) = w.ground_under(DVec3::new(
            (here.x + dx) as f64 + 0.5,
            feet.y + 1.5,
            (here.z + dz) as f64 + 0.5,
        )) else {
            continue;
        };
        // Room above: nothing but what gives way to a piece (grass, a steppe's tussocks).
        let clear = (1..=3).all(|k| {
            let p = BlockPos::new(g.x, g.y + k, g.z);
            w.mirror.block(p).is_some() && w.free(p)
        });
        let c = DVec3::new(g.x as f64 + 0.5, g.y as f64 + 1.0, g.z as f64 + 0.5);
        if clear && (c - eye).length() < 3.5 {
            return g;
        }
    }
    panic!("no open ground about {feet}");
}

/// Level open ground near the player, gone to: a block of ground with `wide` blocks east of it
/// (itself the first) and the row south of those at the same height, three blocks of room over
/// each, and the player standing just west of it.
fn level_ground(w: &mut World, wide: i32) -> BlockPos {
    let feet = w.mover.pos;
    let here = BlockPos::containing(feet);
    let site = {
        let w = &*w;
        let ground = |x: i32, z: i32| {
            w.ground_under(DVec3::new(x as f64 + 0.5, feet.y + 6.0, z as f64 + 0.5))
        };
        let clear = |g: BlockPos, n: i32| {
            (1..=n).all(|k| {
                let p = BlockPos::new(g.x, g.y + k, g.z);
                w.mirror.block(p).is_some() && w.free(p)
            })
        };
        let mut found = None;
        'look: for r in 1..32 {
            for dz in -r..=r {
                for dx in -r..=r {
                    let Some(g) = ground(here.x + dx, here.z + dz) else {
                        continue;
                    };
                    let level = (0..wide).all(|ex| {
                        (0..2).all(|ez| {
                            ground(g.x + ex, g.z + ez).is_some_and(|n| n.y == g.y && clear(n, 3))
                        })
                    });
                    let stand = ground(g.x - 1, g.z).filter(|s| s.y == g.y && clear(*s, 2));
                    if let (true, Some(s)) = (level, stand) {
                        found = Some((g, s));
                        break 'look;
                    }
                }
            }
        }
        found
    };
    let Some((g, stand)) = site else {
        panic!("no level ground about {feet}");
    };
    w.go_exact(DVec3::new(
        stand.x as f64 + 0.5,
        stand.y as f64 + 1.0,
        stand.z as f64 + 0.5,
    ));
    g
}

#[test]
fn a_post_a_beam_and_a_bark_roof_go_up_and_come_down() {
    let dir = common::temp("building");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 7);
    w.run(40);
    let ground = level_ground(&mut w, 2);
    let top = |p: BlockPos| AimAt::Block { pos: p, top: true };
    let beside = |p: BlockPos, face: Direction| AimAt::Beside { pos: p, face };
    // A post from a hazel pole, on the ground beside the builder.
    w.give("hearth:pole/hazel_wood", 2);
    let (done, words) = w.act("place_post", top(ground));
    assert!(done, "{words}");
    let post = ground.up();
    assert_eq!(piece_at(&w, post).0, "post/hazel_wood");
    assert_eq!(w.has("hearth:pole/hazel_wood"), 1, "the pole went into it");
    // Two more for the roof's rafters.
    w.give("hearth:pole/hazel_wood", 2);
    // A beam rests from a post beside it, not on the post's top.
    let (done, words) = w.act("place_beam", top(post));
    assert!(!done, "a beam balanced on a post: {words}");
    let (done, words) = w.act("place_beam", beside(post, Direction::East));
    assert!(done, "{words}");
    let beam = post.offset(Direction::East);
    let (name, facing) = piece_at(&w, beam);
    assert_eq!(name, "beam/hazel_wood");
    assert!(facing.is_some(), "the beam faces a way");
    // Birch bark laid on the slope beside the beam (56 strips, overlapping).
    w.give("hearth:strip/birch_bark", 56);
    let (done, words) = w.act("place_bark_roof", beside(beam, Direction::South));
    assert!(done, "{words}");
    let roof = beam.offset(Direction::South);
    assert_eq!(piece_at(&w, roof).0, "bark_roof/birch_bark");
    let bark = |w: &World| w.at_hand(|id| id == "hearth:strip/birch_bark", 5.0);
    assert_eq!(bark(&w), 0, "all the bark went into it");
    // The roof covers the sky; the post does not.
    assert!(w.reg.light_opacity(w.mirror.block(roof).unwrap()) > 0);
    assert_eq!(w.reg.light_opacity(w.mirror.block(post).unwrap()), 0);
    // Taken down, most of the bark comes back.
    let (done, words) = w.act(
        "take_down_bark_roof",
        AimAt::Block {
            pos: roof,
            top: false,
        },
    );
    assert!(done, "{words}");
    assert!(w.mirror.block(roof).is_some_and(|s| s.is_air()));
    let back = bark(&w);
    assert!((44..=56).contains(&back), "{back} strips back");
    // Nothing is put up where there is no room.
    let (done, _) = w.act("place_post", top(ground));
    assert!(!done, "a post where a post is");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn brush_piled_against_the_wind_teaches_the_windbreak() {
    let dir = common::temp("windbreak");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Discovery, 7);
    w.run(40);
    assert!(!w.knowledge.knows("hearth:windbreak_shelter"));
    for _ in 0..4 {
        if w.knowledge.knows("hearth:windbreak_shelter") {
            break;
        }
        let at = open_ground(&w);
        w.give("hearth:stick/oak_wood", 24);
        let (done, words) = w.act("place_brush_wall", AimAt::Block { pos: at, top: true });
        assert!(done, "{words}");
        assert_eq!(w.block(at.up()).as_deref(), Some("brush_wall/oak_wood"));
        // On along the line of the wall.
        w.go(w.mover.pos.x + 1.0, w.mover.pos.z);
    }
    assert!(
        w.knowledge.knows("hearth:windbreak_shelter"),
        "{:?}",
        w.learned
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn what_reaches_too_far_or_loses_its_post_falls() {
    let dir = common::temp("collapse");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 7);
    w.run(40);
    let ground = level_ground(&mut w, 4);
    let beside = |p: BlockPos, face: Direction| AimAt::Beside { pos: p, face };
    // A post two high.
    for k in 0..2 {
        w.give("hearth:pole/hazel_wood", 1);
        let (done, words) = w.act(
            "place_post",
            AimAt::Block {
                pos: BlockPos::new(ground.x, ground.y + k, ground.z),
                top: true,
            },
        );
        assert!(done, "{words}");
    }
    let post = ground.up().up();
    // Beams lashed on east from its top, facing east so they lie along one line: a pole reaches
    // two metres from the post it is lashed to, no further.
    w.turn(std::f32::consts::FRAC_PI_2);
    let mut last = post;
    for k in 0..3 {
        w.give("hearth:pole/hazel_wood", 1);
        let (done, words) = w.act("place_beam", beside(last, Direction::East));
        assert!(done, "{words}");
        last = last.offset(Direction::East);
        w.run(4);
        let stands = w.block(last).is_some_and(|b| b.starts_with("beam/"));
        assert_eq!(stands, k < 2, "beam {k} at {last}: {:?}", w.block(last));
        // Along the row south of it, to reach the next.
        w.go_exact(DVec3::new(
            last.x as f64 + 0.5,
            ground.y as f64 + 1.0,
            ground.z as f64 + 1.5,
        ));
        w.turn(std::f32::consts::FRAC_PI_2);
    }
    // What fell lies where it fell.
    let lying = |w: &World| {
        w.lying
            .iter()
            .filter(|l| l.stack.id == "hearth:pole/hazel_wood")
            .map(|l| l.stack.count as usize)
            .sum::<usize>()
    };
    assert_eq!(lying(&w), 1, "the broken beam's pole");
    // Without the post under it, the post above and the beams fall too.
    w.go_exact(DVec3::new(
        ground.x as f64 - 0.5,
        ground.y as f64 + 1.0,
        ground.z as f64 + 0.5,
    ));
    let (done, words) = w.act(
        "take_down_post",
        AimAt::Block {
            pos: ground.up(),
            top: false,
        },
    );
    assert!(done, "{words}");
    w.run(10);
    for k in 0..=2 {
        let p = BlockPos::new(post.x + k, post.y, post.z);
        assert!(
            w.mirror.block(p).is_some_and(|s| s.is_air()),
            "{p} still {:?}",
            w.block(p)
        );
    }
    assert!(lying(&w) >= 4, "{} poles lying", lying(&w));
    let _ = std::fs::remove_dir_all(&dir);
}

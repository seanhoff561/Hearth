//! Felling a tree in a running world (V2-6): a trunk chopped through with a hand axe leaves a
//! stump, the tree comes to rest lying along the ground away from the cutter, its limbs are
//! lopped for poles and sticks, and its trunk cut into log sections.

mod common;

use common::{World, temp};
use hearth_math::BlockPos;
use hearth_protocol::AimAt;

/// The foot of a slim standing tree near the player, where the generator placed it (trees stand
/// turned and off their block's middle, so a stem's shape in blocks is no sign of its foot): a
/// limb-thick stem on the ground, felled in under an hour with a stone axe, with stem that thick
/// above the cut (what falls is a stem, not a whip), and all its wood standing in the world.
fn trunk(w: &World) -> Option<BlockPos> {
    let slim = |p: BlockPos| {
        w.mirror.block(p).is_some_and(|s| {
            w.reg.block_of(s).name.path().ends_with("_branch")
                && w.reg
                    .get(s, "thickness")
                    .is_some_and(|t| t == "8" || t == "12")
        })
    };
    // On the ground itself: not the slim stem over a flared foot of log.
    let ground = |p: BlockPos| {
        w.solid(p)
            && w.block(p)
                .is_some_and(|b| !b.ends_with("_log") && !b.ends_with("_branch"))
    };
    let wg = &w.generator;
    let veg = hearth_worldgen::vegetation::Vegetation::default();
    let (x, z) = (w.mover.pos.x.floor() as i32, w.mover.pos.z.floor() as i32);
    wg.features()
        .trees_in(wg, &veg, (x - 40, z - 40), (x + 40, z + 40))
        .into_iter()
        .filter(|t| t.remains == hearth_worldgen::cubegen::features::Remains::Living)
        .find_map(|t| {
            let foot = BlockPos::new(t.foot[0], t.foot[1], t.foot[2]);
            let wood: Vec<BlockPos> = t
                .blocks()
                .filter(|(_, part)| !matches!(part, hearth_flora::Part::Leaves))
                .map(|(p, _)| p)
                .collect();
            let standing = wood.iter().all(|p| {
                w.block(*p)
                    .is_some_and(|b| b.ends_with("_branch") || b.ends_with("_log"))
            });
            let stem = wood.iter().filter(|p| p.y > foot.y && slim(**p)).count() >= 2;
            (slim(foot) && ground(foot.down()) && standing && stem && w.stand_by(foot).is_some())
                .then_some(foot)
        })
}

/// Looks about for a slim tree to fell (see `trunk`), going a little way from the start.
fn find_trunk(w: &mut World) -> BlockPos {
    let start = w.mover.pos;
    let mut foot = trunk(w);
    for (dx, dz) in [
        (40.0, 0.0),
        (-40.0, 0.0),
        (0.0, 40.0),
        (0.0, -40.0),
        (80.0, 0.0),
        (-80.0, 0.0),
    ] {
        if foot.is_some() {
            break;
        }
        w.go(start.x + dx, start.z + dz);
        foot = trunk(w);
    }
    foot.expect("a tree to fell")
}

#[test]
fn a_tree_is_felled_limbed_and_bucked() {
    let dir = temp("felling");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 7);
    let foot = find_trunk(&mut w);
    let wood = w.block(foot).expect("log");
    println!("felling the {wood} at {foot:?}");
    w.go_to_block(foot);
    w.give("hearth:hand_axe/flint", 1);
    let (done, words) = w.act(
        "fell_tree",
        AimAt::Block {
            pos: foot,
            top: false,
        },
    );
    assert!(done, "felled: {words}");
    // The stump stands; above it, nothing.
    assert_eq!(w.block(foot).as_deref(), Some(wood.as_str()), "the stump");
    assert!(
        w.until(5.0, |w| !w.solid(foot.up())),
        "the trunk above the stump is gone: {:?}",
        w.block(foot.up())
    );
    // It comes to rest: the stem lying along the ground near the stump (joined to its neighbours
    // along the ground rather than up and down; the stem's end is joined on one side only).
    w.run(200);
    let lying = |w: &World| {
        w.find(30, |n, _| n == wood)
            .into_iter()
            .filter(|p| {
                w.mirror.block(*p).is_some_and(|s| {
                    w.reg
                        .get(s, "thickness")
                        .is_some_and(|t| t == "8" || t == "12")
                        && w.reg.get(s, "up") != Some("true")
                        && ["east", "west", "north", "south"]
                            .iter()
                            .any(|d| w.reg.get(s, d) == Some("true"))
                })
            })
            .collect::<Vec<_>>()
    };
    assert!(
        w.until(10.0, |w| lying(w).len() >= 2),
        "a stem lies on the ground: {:?}",
        lying(&w)
    );
    // Limbs lopped from it give poles, sticks and twigs.
    let branch = w
        .find(30, |n, _| n.ends_with("_branch"))
        .into_iter()
        .find(|p| w.stand_by(*p).is_some());
    if let Some(b) = branch {
        w.go_to_block(b);
        let wood_bits = |w: &World| {
            w.at_hand(
                |id| id.contains("stick/") || id.contains("twig/") || id.contains("pole/"),
                6.0,
            )
        };
        let before = wood_bits(&w);
        let (done, words) = w.act("lop_branch", AimAt::Block { pos: b, top: false });
        assert!(done, "lopped: {words}");
        assert!(!w.solid(b) || w.block(b).is_some_and(|n| !n.ends_with("_branch")));
        assert!(
            wood_bits(&w) > before,
            "poles, sticks and twigs from the limb"
        );
    }
    // A standing trunk is not cut into sections: it must be felled first.
    let standing = w
        .find(40, |n, _| n.ends_with("_log"))
        .into_iter()
        .find(|p| {
            w.mirror
                .block(*p)
                .is_some_and(|s| w.reg.get(s, "axis") == Some("y"))
                && w.stand_by(*p).is_some()
        });
    if let Some(p) = standing {
        w.go_to_block(p);
        let (done, words) = w.act("buck_log", AimAt::Block { pos: p, top: false });
        assert!(!done && words.contains("Fell it first"), "{words}");
    }
    drop(w);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_felled_tree_stays_felled_and_a_young_tree_takes_its_place() {
    let dir = temp("regrowth");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 7);
    let foot = find_trunk(&mut w);
    let wood = w.block(foot).expect("log");
    w.go_to_block(foot);
    w.give("hearth:hand_axe/flint", 1);
    let (done, words) = w.act(
        "fell_tree",
        AimAt::Block {
            pos: foot,
            top: false,
        },
    );
    assert!(done, "felled: {words}");
    w.run(200);
    let felled = |w: &World| {
        w.block(foot).as_deref() == Some(wood.as_str())
            && w.block(foot.up()).is_some_and(|b| b != wood)
    };
    assert!(w.until(10.0, felled), "a stump: {:?}", w.block(foot.up()));
    // Gone far away and back, the terrain generated anew: the tree stays felled.
    let home = w.mover.pos;
    w.go(home.x + 300.0, home.z);
    w.run(20);
    assert!(
        w.until(20.0, |w| w.block(foot).is_none()),
        "the stump's terrain was unloaded"
    );
    w.go_exact(home);
    assert!(w.until(30.0, felled), "still felled on coming back");
    // Saved and opened again: still felled.
    drop(w);
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 7);
    w.go_exact(home);
    assert!(w.until(30.0, felled), "still felled in the saved world");
    // Years later a young tree stands where it stood.
    let days_per_year = w.calendar.days_per_year();
    let hours = 12.0 * days_per_year * 24.0;
    let want = w.ticks + (hours / 24.0 * w.ticks_per_day) as u64;
    w.server.send(hearth_protocol::ToServer::SkipHours(hours));
    w.server.send(hearth_protocol::ToServer::Run(1));
    assert!(w.until(60.0, |w| w.ticks >= want), "the clock moved on");
    w.run(30);
    // Of the tree's own species, maybe: told from the stump by what grows over its foot.
    let tree = |b: Option<String>| {
        b.is_some_and(|b| b.ends_with("_branch") || b.ends_with("_log") || b.ends_with("_leaves"))
    };
    let young = |w: &World| {
        tree(w.block(foot))
            && (w.block(foot).as_deref() != Some(wood.as_str()) || tree(w.block(foot.up())))
    };
    assert!(
        w.until(10.0, young),
        "a young tree at the stump's place: {:?} under {:?}",
        w.block(foot),
        w.block(foot.up())
    );
    drop(w);
    let _ = std::fs::remove_dir_all(&dir);
}

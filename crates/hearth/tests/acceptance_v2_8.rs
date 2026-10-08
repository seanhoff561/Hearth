//! V2-8 acceptance (PLAN.md; docs/design/building.md): what is built stands or falls by its
//! materials and spans, the ground over an opening holds by itself or by timbers, roofs keep
//! the rain off by their pitch, and the reckoning keeps within its budget on great structures.
//!
//! - A slab spans a one-block doorway between dry-stone walls; two slabs over two blocks fall
//!   at their joint when the props under them are taken away (built by the player's own
//!   processes, on the generated land).
//! - A chamber three wide in clay falls in unshored, falls in on sapling poles, and stands on
//!   log posts and caps.
//! - Under reed thatch the rain does not reach the body; under a flat bark covering it drips
//!   (built by the player, in held rain).
//! - Eight thousand pieces are reckoned within a tick's budget.

mod common;

use std::sync::Arc;

use common::World;
use glam::DVec3;
use hearth::structure::Structures;
use hearth_content::Content;
use hearth_env::weather::WeatherHold;
use hearth_math::{BlockPos, CubePos, Direction, PlanetSize};
use hearth_protocol::{AimAt, ToServer};
use hearth_world::{BlockRegistry, BlockStateId, Cube, CubeMap};

/// Level open ground near the player, gone to: `wide` blocks east by two south level, with
/// four blocks of air over each, the player standing just west of it.
fn level_ground(w: &mut World, wide: i32) -> BlockPos {
    let feet = w.mover.pos;
    let here = BlockPos::containing(feet);
    let site = {
        let w = &*w;
        let ground = |x: i32, z: i32| {
            w.ground_under(DVec3::new(x as f64 + 0.5, feet.y + 6.0, z as f64 + 0.5))
        };
        // Room above: nothing but what gives way to a piece (grass, a steppe's tussocks).
        let clear = |g: BlockPos, n: i32| {
            (1..=n).all(|k| {
                let p = BlockPos::new(g.x, g.y + k, g.z);
                w.mirror.block(p).is_some() && w.free(p)
            })
        };
        let mut found = None;
        'look: for r in 1..40 {
            for dz in -r..=r {
                for dx in -r..=r {
                    let Some(g) = ground(here.x + dx, here.z + dz) else {
                        continue;
                    };
                    let level = (-1..wide).all(|ex| {
                        (0..2).all(|ez| {
                            ground(g.x + ex, g.z + ez).is_some_and(|n| n.y == g.y && clear(n, 4))
                        })
                    });
                    if level {
                        found = Some(g);
                        break 'look;
                    }
                }
            }
        }
        found
    };
    let Some(g) = site else {
        panic!("no level ground about {feet}");
    };
    w.go_exact(DVec3::new(
        g.x as f64 - 0.5,
        g.y as f64 + 1.0,
        g.z as f64 + 1.5,
    ));
    g
}

/// Stands south of the column `x` blocks east of `ground`, a block off, facing east.
fn stand_by(w: &mut World, ground: BlockPos, x: i32) {
    w.go_exact(DVec3::new(
        (ground.x + x) as f64 + 0.5,
        ground.y as f64 + 1.0,
        ground.z as f64 + 1.5,
    ));
    w.turn(std::f32::consts::FRAC_PI_2);
}

fn piece(w: &World, p: BlockPos) -> Option<String> {
    w.block(p).filter(|b| b.contains('/'))
}

/// Mild dry weather while the building goes on (the builder has no fire or clothes).
fn mild(w: &mut World) {
    w.server.send(ToServer::HoldWeather(Some(WeatherHold {
        temperature_c: Some(22.0),
        humidity: Some(0.5),
        precip_mm_h: Some(0.0),
        wind_speed_m_s: Some(1.0),
        ..WeatherHold::default()
    })));
    w.run(2);
}

/// Puts up a piece with what it takes given first.
fn put_up(w: &mut World, process: &str, gives: &[(&str, u16)], aim: AimAt) {
    for (id, n) in gives {
        w.give(id, *n);
        // What is too heavy to carry is dragged: let it lie to hand.
        w.let_go();
    }
    let (done, words) = w.act(process, aim);
    assert!(done, "{process}: {words}");
}

#[test]
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn a_slab_spans_a_doorway_and_two_slabs_end_to_end_do_not() {
    let dir = common::temp("acceptance-v2-8-stone");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 7);
    w.run(40);
    mild(&mut w);
    let g = level_ground(&mut w, 8);
    let at = |x: i32, up: i32| BlockPos::new(g.x + x, g.y + up, g.z);
    let top = |p: BlockPos| AimAt::Block { pos: p, top: true };
    // Dry-stone piers two high at 0 and 2 (a doorway between), and at 4 and 7 (two blocks).
    for x in [0, 2, 4, 7] {
        stand_by(&mut w, g, x);
        for up in 0..2 {
            put_up(
                &mut w,
                "place_dry_stone",
                &[("hearth:fieldstone/granite", 36)],
                top(at(x, up)),
            );
        }
        assert_eq!(piece(&w, at(x, 2)).as_deref(), Some("dry_stone/granite"));
    }
    // One slab across the doorway, from the pier: it stands.
    stand_by(&mut w, g, 1);
    put_up(
        &mut w,
        "place_stone_lintel",
        &[("hearth:slab/granite", 1)],
        AimAt::Beside {
            pos: at(0, 2),
            face: Direction::East,
        },
    );
    w.run(10);
    assert_eq!(piece(&w, at(1, 2)).as_deref(), Some("stone_lintel/granite"));
    // Two slabs over two blocks, laid on hazel props from the pier at 4 toward the pier at 7.
    for x in [5, 6] {
        stand_by(&mut w, g, x);
        put_up(
            &mut w,
            "place_post",
            &[("hearth:pole/hazel_wood", 1)],
            top(at(x, 0)),
        );
        put_up(
            &mut w,
            "place_stone_lintel",
            &[("hearth:slab/granite", 1)],
            AimAt::Beside {
                pos: at(x - 1, 2),
                face: Direction::East,
            },
        );
        w.run(10);
        assert_eq!(
            piece(&w, at(x, 2)).as_deref(),
            Some("stone_lintel/granite"),
            "slab {x} on its prop"
        );
    }
    // The first prop taken away, both slabs fall: nothing carries across their joint.
    stand_by(&mut w, g, 5);
    let (done, words) = w.act(
        "take_down_post",
        AimAt::Block {
            pos: at(5, 1),
            top: false,
        },
    );
    assert!(done, "{words}");
    w.run(20);
    for x in [5, 6] {
        assert_eq!(piece(&w, at(x, 2)), None, "slab {x} fell");
    }
    // The doorway's slab still stands, and some of the fallen slabs' stone lies about.
    assert_eq!(piece(&w, at(1, 2)).as_deref(), Some("stone_lintel/granite"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A cube of clay with air over it, the server's reckoning of what is built and dug in it.
struct Ground {
    reg: BlockRegistry,
    map: CubeMap,
    s: Structures,
}

fn clay() -> Ground {
    let reg = hearth_world::datapack::load_builtin_registry().expect("registry");
    let content = Content::load_base();
    let s = Structures::new(&reg, &content);
    let mut map = CubeMap::new(hearth_math::Planet::from_size(PlanetSize::Tiny).expect("planet"));
    let clay = reg.parse_state("hearth:earthenware_clay").expect("clay");
    let mut cube = Cube::filled(clay);
    for y in 12..16 {
        for z in 0..16 {
            for x in 0..16 {
                cube.set(hearth_math::LocalPos::new(x, y, z), BlockStateId::AIR);
            }
        }
    }
    map.insert_cube(CubePos::new(0, 0, 0), Arc::new(cube), &reg);
    Ground { reg, map, s }
}

impl Ground {
    fn set(&mut self, x: i32, y: i32, z: i32, state: &str) {
        let s = self.reg.parse_state(state).expect(state);
        let p = BlockPos::new(x, y, z);
        self.map.set_block(p, s, &self.reg);
        self.s.changed(&[p]);
    }

    /// A chamber along x from 3 to 12, three wide from z = 6, two high from y = 6; with sets of
    /// `post`s in its side rows every other block and `cap`s across between them.
    fn chamber(&mut self, sets: Option<(&str, &str)>) {
        for x in 3..13 {
            for z in 6..9 {
                for y in 6..8 {
                    self.set(x, y, z, "hearth:air");
                }
            }
            if let Some((post, cap)) = sets
                && x % 2 == 0
            {
                for y in 6..8 {
                    self.set(x, y, 6, post);
                    self.set(x, y, 8, post);
                }
                self.set(x, 7, 7, &format!("{cap}[facing=south]"));
            }
        }
    }

    /// Reckons until nothing more falls, laying fallen ground on the floor and taking fallen
    /// pieces away; what fell, ground and pieces.
    fn settle(&mut self) -> (usize, usize) {
        let (mut ground, mut pieces) = (0, 0);
        for _ in 0..64 {
            let fell = self.s.tick(&self.map, &self.reg);
            if fell.ground.is_empty() && fell.pieces.is_empty() && !self.s.busy() {
                break;
            }
            for p in fell.pieces.iter().chain(&fell.ground) {
                self.map.set_block(*p, BlockStateId::AIR, &self.reg);
                self.s.changed(&[*p]);
            }
            ground += fell.ground.len();
            pieces += fell.pieces.len();
        }
        (ground, pieces)
    }
}

#[test]
fn a_timbered_chamber_in_clay_stands() {
    // Unshored, the clay over three blocks' width falls in, and the fall works upward.
    let mut g = clay();
    g.chamber(None);
    let (ground, _) = g.settle();
    assert!(ground >= 10, "{ground} blocks fell in");
    // On hazel poles, the poles buckle under the clay and it comes in after them.
    let mut g = clay();
    g.chamber(Some(("hearth:post/hazel_wood", "hearth:beam/hazel_wood")));
    let (ground, pieces) = g.settle();
    assert!(
        pieces > 0 && ground > 0,
        "{pieces} poles and {ground} of ground fell"
    );
    // On log posts with log caps, it stands, and the timbers bear it with room to spare.
    let mut g = clay();
    g.chamber(Some((
        "hearth:log_post/oak_wood",
        "hearth:log_beam/oak_wood",
    )));
    let (ground, pieces) = g.settle();
    assert_eq!((ground, pieces), (0, 0));
    let worst = g.s.stress.values().copied().fold(0.0, f32::max);
    assert!(worst > 0.0 && worst < 0.7, "the timbers pressed {worst}");
}

#[test]
fn thatch_keeps_the_rain_off_and_a_flat_bark_covering_drips() {
    let dir = common::temp("acceptance-v2-8-rain");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 7);
    w.run(40);
    mild(&mut w);
    let g = level_ground(&mut w, 8);
    let at = |x: i32, up: i32| BlockPos::new(g.x + x, g.y + up, g.z);
    let top = |p: BlockPos| AimAt::Block { pos: p, top: true };
    // Two frames: posts two high at 0 and 2 with a beam between, and at 4 and 6 (thatch is
    // heavy: a pole lashed out from one post alone would break under it).
    for x in [0, 4] {
        for post in [x, x + 2] {
            stand_by(&mut w, g, post);
            for up in 0..2 {
                put_up(
                    &mut w,
                    "place_post",
                    &[("hearth:pole/hazel_wood", 1)],
                    top(at(post, up)),
                );
            }
        }
        stand_by(&mut w, g, x + 1);
        put_up(
            &mut w,
            "place_beam",
            &[("hearth:pole/hazel_wood", 1)],
            AimAt::Beside {
                pos: at(x, 2),
                face: Direction::East,
            },
        );
    }
    // Flat bark on the first beam, reed thatch on the second, each on two rafters.
    stand_by(&mut w, g, 1);
    put_up(
        &mut w,
        "place_bark_cover",
        &[
            ("hearth:strip/birch_bark", 40),
            ("hearth:pole/hazel_wood", 2),
        ],
        top(at(1, 2)),
    );
    stand_by(&mut w, g, 5);
    put_up(
        &mut w,
        "place_thatch",
        &[("hearth:sheaf/reed", 60), ("hearth:pole/hazel_wood", 2)],
        top(at(5, 2)),
    );
    assert_eq!(
        piece(&w, at(1, 3)).as_deref(),
        Some("bark_cover/birch_bark")
    );
    assert_eq!(piece(&w, at(5, 3)).as_deref(), Some("thatch/reed"));
    // Steady rain.
    w.server.send(ToServer::HoldWeather(Some(WeatherHold {
        precip_mm_h: Some(10.0),
        temperature_c: Some(12.0),
        ..WeatherHold::default()
    })));
    let rain_at = |w: &mut World, x: i32| {
        w.go_exact(DVec3::new(
            (g.x + x) as f64 + 0.5,
            g.y as f64 + 1.0,
            g.z as f64 + 0.5,
        ));
        w.run(40);
        w.body.as_ref().map_or(f32::NAN, |b| b.exposure.rain_mm_h)
    };
    let open = rain_at(&mut w, 3);
    let bark = rain_at(&mut w, 1);
    let thatch = rain_at(&mut w, 5);
    assert!(open > 9.0, "in the open {open} mm/h");
    assert!((3.0..5.0).contains(&bark), "under flat bark {bark} mm/h");
    assert_eq!(thatch, 0.0, "under thatch");
    w.server.send(ToServer::HoldWeather(None));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn eight_thousand_pieces_are_reckoned_within_a_tick() {
    // A dry-stone wall a hundred long, two thick and forty high on the clay.
    let mut g = clay();
    let clay = g.reg.parse_state("hearth:earthenware_clay").expect("clay");
    for cx in -3..=3 {
        for cy in 0..=3 {
            if (cx, cy) == (0, 0) {
                continue;
            }
            let mut cube = Cube::filled(if cy == 0 { clay } else { BlockStateId::AIR });
            if cy == 0 {
                for y in 12..16 {
                    for z in 0..16 {
                        for x in 0..16 {
                            cube.set(hearth_math::LocalPos::new(x, y, z), BlockStateId::AIR);
                        }
                    }
                }
            }
            g.map
                .insert_cube(CubePos::new(cx, cy, 0), Arc::new(cube), &g.reg);
        }
    }
    let wall = g
        .reg
        .parse_state("hearth:dry_stone/granite[facing=north]")
        .expect("wall");
    for x in -40..60 {
        for z in 4..6 {
            for y in 12..52 {
                g.map.set_block(BlockPos::new(x, y, z), wall, &g.reg);
            }
        }
    }
    g.s.changed(&[BlockPos::new(0, 12, 4)]);
    let t = std::time::Instant::now();
    let fell = g.s.tick(&g.map, &g.reg);
    let took = t.elapsed();
    println!("{} pieces reckoned in {took:?}", g.s.stress.len());
    assert_eq!(g.s.stress.len(), 8000);
    assert!(fell.pieces.is_empty());
    assert!(took.as_millis() < 100, "{took:?}");
}

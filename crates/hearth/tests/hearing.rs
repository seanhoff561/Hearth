//! What the player hears from the world, headless: the ground underfoot by its blocks, open
//! ground against a stone room, footsteps at the pace of the gait, and the splash of a jump
//! into water.

use std::sync::Arc;

use glam::{DVec2, DVec3};
use hearth::hearing::{Hearing, buried, enclosure, surface_under};
use hearth_audio::{Command, Sound, Surface};
use hearth_math::{BlockPos, CubePos, Planet, PlanetSize};
use hearth_physics::{Ability, BlockWorld, Gait, Intent, Mover};
use hearth_world::{BlockRegistry, Cube, CubeMap};

fn world() -> (CubeMap, BlockRegistry) {
    let reg = hearth_world::datapack::load_builtin_registry().expect("blocks");
    let mut map = CubeMap::new(Planet::from_size(PlanetSize::Tiny).expect("planet"));
    let air = reg.default_state("hearth:air");
    for cx in -2..2 {
        for cz in -2..2 {
            for cy in -1..2 {
                map.insert_cube(CubePos::new(cx, cy, cz), Arc::new(Cube::filled(air)), &reg);
            }
        }
    }
    (map, reg)
}

fn set(map: &mut CubeMap, reg: &BlockRegistry, lo: (i32, i32, i32), hi: (i32, i32, i32), b: &str) {
    let s = reg.default_state(b);
    for x in lo.0..=hi.0 {
        for y in lo.1..=hi.1 {
            for z in lo.2..=hi.2 {
                map.set_block(BlockPos::new(x, y, z), s, reg);
            }
        }
    }
}

#[test]
fn the_ground_underfoot_by_its_blocks() {
    let (mut map, reg) = world();
    let feet = DVec3::new(0.5, 1.0, 0.5);
    for (block, want) in [
        ("hearth:grass_block", Surface::Grass),
        ("hearth:podzol", Surface::Soil),
        ("hearth:quartz_sand", Surface::Sand),
        ("hearth:gravel", Surface::Gravel),
        ("hearth:mud", Surface::Mud),
        ("hearth:granite", Surface::Stone),
        ("hearth:oak_log", Surface::Wood),
        ("hearth:snow_block", Surface::Snow),
        ("hearth:ice", Surface::Ice),
        ("hearth:moss_block", Surface::Moss),
    ] {
        set(&mut map, &reg, (-1, 0, -1), (1, 0, 1), block);
        assert_eq!(surface_under(&map, &reg, feet), want, "{block}");
    }
    // Grass growing at the feet rustles over whatever is below.
    set(&mut map, &reg, (-1, 0, -1), (1, 0, 1), "hearth:podzol");
    set(&mut map, &reg, (0, 1, 0), (0, 1, 0), "hearth:short_grass");
    assert_eq!(surface_under(&map, &reg, feet), Surface::Grass);
    // At an edge, the block under a corner of the feet.
    set(&mut map, &reg, (0, 1, 0), (0, 1, 0), "hearth:air");
    set(&mut map, &reg, (0, 0, 0), (0, 0, 0), "hearth:air");
    assert_eq!(
        surface_under(&map, &reg, DVec3::new(0.25, 1.0, 0.5)),
        Surface::Soil
    );
}

#[test]
fn open_ground_and_a_stone_room() {
    let (mut map, reg) = world();
    set(&mut map, &reg, (-16, 0, -16), (15, 0, 15), "hearth:granite");
    let eye = DVec3::new(0.5, 2.6, 0.5);
    let open = enclosure(&map, &reg, eye);
    // Walls and a roof of stone around the player.
    set(&mut map, &reg, (-4, 1, -4), (4, 6, 4), "hearth:granite");
    set(&mut map, &reg, (-3, 1, -3), (3, 5, 3), "hearth:air");
    let room = enclosure(&map, &reg, eye);
    assert!(open < 0.15, "open ground {open}");
    assert!(room > 0.95, "a stone room {room}");
    // A one-block roof lets the rain be heard; eight blocks of rock do not.
    assert!(buried(&map, &reg, eye) < 0.05);
    set(&mut map, &reg, (-4, 7, -4), (4, 13, 4), "hearth:granite");
    assert!(buried(&map, &reg, eye) > 0.95);
}

/// Walks (or jogs, or sprints) east for `seconds`; the steps heard and the distance.
fn walk(gait: Gait, seconds: f64) -> (usize, f64) {
    let (mut map, reg) = world();
    set(
        &mut map,
        &reg,
        (-32, 0, -32),
        (31, 0, 31),
        "hearth:grass_block",
    );
    let terrain = BlockWorld {
        map: &map,
        reg: &reg,
    };
    let mut mover = Mover::new(DVec3::new(2.0, 1.0, 0.5));
    let mut hearing = Hearing::default();
    let intent = Intent {
        wish: DVec2::new(1.0, 0.0),
        gait,
        ..Intent::default()
    };
    let start = mover.pos;
    let dt = 1.0 / 60.0;
    let mut steps = 0;
    for _ in 0..(seconds / dt) as usize {
        let vy = mover.vel.y;
        let report = hearth_physics::step(&terrain, &mut mover, &intent, &Ability::human(), dt);
        hearing.moved(&map, &reg, &mover, &report, vy, dt);
        steps += hearing
            .out
            .drain(..)
            .filter(|c| {
                matches!(
                    c,
                    Command::Play {
                        sound: Sound::Step {
                            surface: Surface::Grass,
                            ..
                        },
                        ..
                    }
                )
            })
            .count();
    }
    (steps, (mover.pos - start).length())
}

#[test]
fn footsteps_keep_pace_with_the_gait() {
    let (walked, d_walk) = walk(Gait::Walk, 10.0);
    let (jogged, d_jog) = walk(Gait::Jog, 6.0);
    // About one step every three quarters of a metre walking, longer strides jogging.
    let per_m_walk = walked as f64 / d_walk;
    let per_m_jog = jogged as f64 / d_jog;
    assert!(
        (1.1..1.5).contains(&per_m_walk),
        "{walked} steps in {d_walk:.1} m"
    );
    assert!(
        (0.7..1.0).contains(&per_m_jog),
        "{jogged} steps in {d_jog:.1} m"
    );
    // Faster gaits step more often in time.
    assert!(jogged as f64 / 6.0 > walked as f64 / 10.0);
}

#[test]
fn a_jump_into_water_splashes() {
    let (mut map, reg) = world();
    set(&mut map, &reg, (-8, -8, -8), (7, -8, 7), "hearth:granite");
    set(&mut map, &reg, (-8, -7, -8), (7, 0, 7), "hearth:water");
    let terrain = BlockWorld {
        map: &map,
        reg: &reg,
    };
    // Dropped from 5 m above the surface.
    let mut mover = Mover::new(DVec3::new(0.5, 6.0, 0.5));
    let mut hearing = Hearing::default();
    let dt = 1.0 / 60.0;
    let mut splashes = Vec::new();
    for _ in 0..180 {
        let vy = mover.vel.y;
        let report = hearth_physics::step(
            &terrain,
            &mut mover,
            &Intent::default(),
            &Ability::human(),
            dt,
        );
        hearing.moved(&map, &reg, &mover, &report, vy, dt);
        for c in hearing.out.drain(..) {
            if let Command::Play {
                sound: Sound::Splash { speed },
                ..
            } = c
            {
                splashes.push(speed);
            }
        }
    }
    assert_eq!(splashes.len(), 1, "{splashes:?}");
    // About √(2 g h) at 5 m: 9.9 m/s, less the air's drag.
    assert!((8.0..10.5).contains(&splashes[0]), "{splashes:?}");
}

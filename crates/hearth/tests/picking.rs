//! Picking what is looked at (Amendment P §5.1): the first block along the look, by its shape,
//! and cheaply enough to run every frame (under 0.2 ms).

use std::sync::Arc;
use std::time::Instant;

use glam::DVec3;
use hearth::client::{Aim, pick_block};
use hearth_math::{BlockPos, CubePos, Planet, PlanetSize};
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
    // Ground at y 0, a stone lying on it, a raspberry bush beside.
    let grass = reg.default_state("hearth:grass_block");
    for x in -8..8 {
        for z in -8..8 {
            map.set_block(BlockPos::new(x, 0, z), grass, &reg);
        }
    }
    map.set_block(
        BlockPos::new(1, 1, 0),
        reg.default_state("hearth:granite_cobbles"),
        &reg,
    );
    map.set_block(
        BlockPos::new(0, 1, 2),
        reg.default_state("hearth:raspberry"),
        &reg,
    );
    (map, reg)
}

#[test]
fn the_first_thing_along_the_look_is_picked_quickly() {
    let (map, reg) = world();
    let eye = DVec3::new(0.5, 2.6, 0.5);
    // Down at the ground: the grass's top.
    let (_, hit) = pick_block(&map, &reg, eye, DVec3::new(0.0, -1.0, 0.0), 2.6).expect("ground");
    assert!(matches!(hit, Aim::Block { pos, top: true, .. } if pos == BlockPos::new(0, 0, 0)));
    // Toward the stone lying there: the stone, not the ground behind it.
    let to_stone = (DVec3::new(1.7, 1.1, 0.65) - eye).normalize();
    let (_, hit) = pick_block(&map, &reg, eye, to_stone, 2.6).expect("stone");
    assert!(
        matches!(hit, Aim::Block { pos, .. } if pos == BlockPos::new(1, 1, 0)),
        "{hit:?}"
    );
    // Level, at nothing within reach.
    assert!(pick_block(&map, &reg, eye, DVec3::X, 2.6).is_none());
    // Cost: the worst case marches the whole reach.
    let n = 20_000;
    let t0 = Instant::now();
    let mut hits = 0;
    for i in 0..n {
        let a = i as f64 * 0.37;
        let dir = DVec3::new(a.cos(), -0.05 - 0.4 * (i % 3) as f64, a.sin()).normalize();
        hits += pick_block(&map, &reg, eye, dir, 2.6).is_some() as usize;
    }
    let per = t0.elapsed().as_secs_f64() / n as f64 * 1e3;
    println!("{per:.4} ms a pick ({hits} hits)");
    assert!(per < 0.2, "{per} ms a pick");
}

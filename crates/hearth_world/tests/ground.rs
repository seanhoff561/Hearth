//! The ground as a field (Amendment S §8.3–8.4, §13): a look meets it to the centimetre, digging
//! and refilling conserve its volume and material, and loose ground piled or undercut slumps to
//! its angle of repose.

use std::sync::Arc;

use glam::DVec3;
use hearth_math::{BlockPos, CubePos, Planet, PlanetSize};
use hearth_world::fill::Fill;
use hearth_world::ground::{self, Taken};
use hearth_world::{BlockRegistry, BlockStateId, Cube, CubeMap};

/// A flat field of `ground` whose surface stands `top` m above y = 0 (its voxel at y = 0 part
/// full), on a small planet's patch of loaded cubes.
fn field(ground: &str, top: f32) -> (CubeMap, BlockRegistry, BlockStateId) {
    let reg = hearth_world::datapack::load_builtin_registry().expect("blocks");
    let s = reg.default_state(ground);
    assert!(!s.is_air(), "{ground}");
    let mut map = CubeMap::new(Planet::from_size(PlanetSize::Tiny).expect("planet"));
    for cx in -2..2 {
        for cz in -2..2 {
            map.insert_cube(
                CubePos::new(cx, 0, cz),
                Arc::new(Cube::filled(BlockStateId::AIR)),
                &reg,
            );
            map.insert_cube(CubePos::new(cx, -1, cz), Arc::new(Cube::filled(s)), &reg);
        }
    }
    for x in -32..32 {
        for z in -32..32 {
            let p = BlockPos::new(x, 0, z);
            map.set_block(p, s, &reg);
            map.set_fill(p, Fill::quantize(top - 0.5), &reg);
            map.set_fill(p.up(), Fill::quantize(top - 1.5), &reg);
            map.set_fill(p.down(), Fill::quantize(top + 0.5), &reg);
        }
    }
    (map, reg, s)
}

fn total(t: &Taken) -> f32 {
    t.iter().map(|(_, v)| v).sum()
}

fn region() -> (BlockPos, BlockPos) {
    (BlockPos::new(-20, -6, -20), BlockPos::new(19, 12, 19))
}

#[test]
fn a_look_meets_the_ground_to_the_centimetre() {
    let (map, reg, s) = field("hearth:loam", 0.3);
    let eye = DVec3::new(0.5, 2.0, 0.5);
    let hit = ground::raycast(&map, &reg, eye, DVec3::NEG_Y, 4.0).expect("the ground");
    assert!((hit.at.y - 0.3).abs() < 0.01, "met at {}", hit.at.y);
    assert!(hit.normal.y > 0.99, "{:?}", hit.normal);
    assert_eq!(hit.state, s);
    assert_eq!(hit.voxel, BlockPos::new(0, 0, 0));
    // Aslant: where the line crosses y = 0.3.
    let dir = DVec3::new(1.0, -1.0, 0.3).normalize();
    let hit = ground::raycast(&map, &reg, eye, dir, 4.0).expect("the ground aslant");
    assert!((hit.at.y - 0.3).abs() < 0.01, "{:?}", hit.at);
    assert!((hit.distance - (2.0 - 0.3) / -dir.y).abs() < 0.02);
    // Level, nothing.
    assert!(ground::raycast(&map, &reg, eye, DVec3::X, 4.0).is_none());
}

#[test]
fn digging_and_refilling_conserve_the_grounds_volume() {
    let (mut map, reg, s) = field("hearth:loam", 0.5);
    let (lo, hi) = region();
    let before = total(&ground::volume(&map, &reg, lo, hi));
    let at = DVec3::new(0.5, 0.5, 0.5);
    // Forty strokes of a digging stick, about 6 litres each.
    let mut dug: Taken = Vec::new();
    // Less than the fill's step at a stroke: what was not taken is carried to the next.
    let mut carry = 0.0;
    for _ in 0..40 {
        let hit = ground::raycast(&map, &reg, DVec3::new(0.3, 2.5, 0.7), DVec3::NEG_Y, 5.0)
            .expect("ground under");
        let got = ground::dig(
            &mut map,
            &reg,
            hit.at,
            hit.normal,
            0.5,
            0.006 + carry,
            &|_| true,
        );
        carry += 0.006 - total(&got);
        for (k, v) in got {
            match dug.iter_mut().find(|(d, _)| *d == k) {
                Some(e) => e.1 += v,
                None => dug.push((k, v)),
            }
        }
    }
    let after_dig = total(&ground::volume(&map, &reg, lo, hi));
    assert!((total(&dug) - 0.24).abs() < 0.01, "dug {} m³", total(&dug));
    assert!(
        (before - after_dig - total(&dug)).abs() < 0.02,
        "the ground lost {} m³ for {} dug",
        before - after_dig,
        total(&dug)
    );
    assert!(dug.iter().all(|(k, _)| *k == s), "only loam: {dug:?}");
    // A hole: the look meets the ground lower where it was dug.
    let hit = ground::raycast(&map, &reg, DVec3::new(0.3, 2.5, 0.7), DVec3::NEG_Y, 5.0)
        .expect("the hole's floor");
    assert!(hit.at.y < 0.45, "the floor at {}", hit.at.y);
    // Back into the hole: the same volume of the same material.
    let put = ground::pile(&mut map, &reg, at, 1.0, s, total(&dug));
    assert!((put - total(&dug)).abs() < 1e-3, "{put} put back");
    let after = ground::volume(&map, &reg, lo, hi);
    assert!(
        (total(&after) - before).abs() < 0.02,
        "{} m³ against {before}",
        total(&after)
    );
    assert!(after.iter().all(|(k, _)| *k == s));
}

/// The steepest step between neighbouring columns of ground, as a slope's angle (degrees).
fn steepest(map: &CubeMap, reg: &BlockRegistry, c: BlockPos, r: i32) -> f32 {
    let height = |x: i32, z: i32| {
        let (lo, hi) = (BlockPos::new(x, -6, z), BlockPos::new(x, 12, z));
        -6.0 + total(&ground::volume(map, reg, lo, hi))
    };
    let mut worst = 0.0f32;
    for z in c.z - r..=c.z + r {
        for x in c.x - r..=c.x + r {
            let h = height(x, z);
            for (dx, dz) in [(1, 0), (0, 1)] {
                worst = worst.max((h - height(x + dx, z + dz)).abs());
            }
        }
    }
    worst.atan().to_degrees()
}

#[test]
fn a_spoil_heap_and_a_pits_walls_slump_to_their_repose() {
    let (mut map, reg, sand) = field("hearth:quartz_sand", 0.5);
    let (lo, hi) = region();
    let before = total(&ground::volume(&map, &reg, lo, hi));
    let repose = |s: BlockStateId| (s == sand).then_some(34.0);
    // Four cubic metres tipped in one place: a heap, then it runs.
    let at = DVec3::new(0.5, 0.5, 0.5);
    let put = ground::pile(&mut map, &reg, at, 0.5, sand, 4.0);
    assert!((put - 4.0).abs() < 1e-3, "{put}");
    let heaped = steepest(&map, &reg, BlockPos::new(0, 0, 0), 4);
    assert!(
        heaped > 60.0,
        "a heap tipped stands steep at first: {heaped}°"
    );
    let moves = ground::settle(&mut map, &reg, BlockPos::new(0, 0, 0), 8, &repose, 20_000);
    let settled = steepest(&map, &reg, BlockPos::new(0, 0, 0), 6);
    eprintln!("a heap tipped at {heaped:.0}° settles to {settled:.1}° in {moves} moves");
    assert!(moves > 0);
    assert!(settled < 38.0, "settled to {settled}° (sand rests at 34°)");
    assert!(settled > 15.0, "still a heap: {settled}°");
    let after = total(&ground::volume(&map, &reg, lo, hi));
    assert!(
        (after - before - 4.0).abs() < 0.03,
        "{after} against {}",
        before + 4.0
    );
    // A pit dug straight down slumps in: its walls lean back to the repose, and nothing is lost.
    let mut pit = 0.0;
    for x in 6..8 {
        for z in 6..8 {
            for y in -3..=0 {
                let c = DVec3::new(x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5);
                pit += total(&ground::dig(&mut map, &reg, c, DVec3::Y, 0.3, 1.0, &|_| {
                    true
                }));
            }
        }
    }
    assert!(pit > 10.0, "a pit of {pit} m³");
    ground::settle(&mut map, &reg, BlockPos::new(7, 0, 7), 6, &repose, 20_000);
    let walls = steepest(&map, &reg, BlockPos::new(7, 0, 7), 3);
    eprintln!("a pit of {pit:.1} m³: its walls slump to {walls:.1}°");
    assert!(walls < 38.0, "the pit's walls stand at {walls}°");
    let left = total(&ground::volume(&map, &reg, lo, hi));
    assert!(
        (left - (after - pit)).abs() < 0.05,
        "{left} against {}",
        after - pit
    );
}

#[test]
fn rock_stands_where_sand_runs() {
    let (mut map, reg, _) = field("hearth:loam", 0.5);
    let granite = reg.default_state("hearth:granite");
    assert!(!granite.is_air());
    ground::pile(&mut map, &reg, DVec3::new(0.5, 0.5, 0.5), 0.5, granite, 3.0);
    let moves = ground::settle(&mut map, &reg, BlockPos::new(0, 0, 0), 4, &|_| None, 1000);
    assert_eq!(moves, 0, "rock does not flow");
}

#[test]
fn a_changed_cube_crosses_the_wire_exactly() {
    let (mut map, reg, s) = field("hearth:loam", 0.4);
    ground::dig(
        &mut map,
        &reg,
        DVec3::new(3.2, 0.4, 3.7),
        DVec3::Y,
        0.5,
        0.3,
        &|_| true,
    );
    ground::pile(&mut map, &reg, DVec3::new(6.5, 0.4, 3.5), 0.8, s, 0.3);
    let c = map.cube(CubePos::new(0, 0, 0)).expect("the cube");
    assert!(c.fill().is_some());
    let mut bytes = Vec::new();
    c.write_bytes(&mut bytes);
    let (back, n) = Cube::read_bytes(&bytes, &|v| BlockStateId(v)).expect("read");
    assert_eq!(n, bytes.len());
    assert_eq!(back.fill(), c.fill());
    assert!((0..hearth_math::CUBE_VOLUME).all(|i| back.get_index(i) == c.get_index(i)));
}

#[test]
fn levelling_cuts_the_high_ground_into_the_hollows() {
    let (mut map, reg, s) = field("hearth:loam", 0.3);
    // A hummock and a hollow side by side in a 3 m square: a mound of 0.6 m³ piled at one
    // corner, 0.4 m³ dug from the other.
    ground::pile(&mut map, &reg, DVec3::new(-0.5, 0.3, -0.5), 0.6, s, 0.6);
    let any = |_: BlockStateId| true;
    ground::dig(
        &mut map,
        &reg,
        DVec3::new(1.5, 0.3, 1.5),
        DVec3::Y,
        0.5,
        0.4,
        &any,
    );
    let (lo, hi) = region();
    let before = total(&ground::volume(&map, &reg, lo, hi));
    let left = ground::level(&mut map, &reg, (-1, -1), (1, 1), 0.33, &any);
    let after = total(&ground::volume(&map, &reg, lo, hi));
    // Conserved: what is in the ground and what was set aside make the volume before.
    assert!(
        (after + total(&left) - before).abs() < 0.02,
        "{before} → {after} + {}",
        total(&left)
    );
    // Level: the surface over the square within a few centimetres of the plane.
    for x in -1..=1 {
        for z in -1..=1 {
            let top = DVec3::new(x as f64 + 0.5, 3.0, z as f64 + 0.5);
            let hit = ground::raycast(&map, &reg, top, DVec3::NEG_Y, 6.0).expect("ground");
            assert!((hit.at.y - 0.33).abs() < 0.06, "({x}, {z}) at {}", hit.at.y);
        }
    }
}

#[test]
fn earth_piled_over_a_cave_lies_on_the_surface() {
    let (mut map, reg, loam) = field("hearth:loam", 0.5);
    // A cave three metres down under the place: open voxels on a floor of ground.
    for x in -3..4 {
        for z in -3..4 {
            for y in -4..=-3 {
                map.set_block(BlockPos::new(x, y, z), BlockStateId::AIR, &reg);
            }
        }
    }
    let put = ground::pile(&mut map, &reg, DVec3::new(0.5, 0.5, 0.5), 1.0, loam, 1.0);
    assert!((put - 1.0).abs() < 1e-3, "{put}");
    let cave_floor = (-3..4)
        .flat_map(|x| (-3..4).map(move |z| (x, z)))
        .flat_map(|(x, z)| (-4..=-3).map(move |y| BlockPos::new(x, y, z)))
        .filter(|p| map.block(*p).is_some_and(|b| !b.is_air()))
        .count();
    assert_eq!(cave_floor, 0, "earth piled into the cave");
}

/// A hole is dug where the tool strikes (T §2.3), not at the middle of the voxel struck: strokes
/// at a point 0.425 m off a voxel's middle take the earth from it and the voxel beyond in
/// proportion to their nearness, so the ground falls more on the point's side.
#[test]
fn a_hole_is_centred_where_the_tool_strikes() {
    let (mut map, reg, _) = field("hearth:loam", 0.5);
    let (lo, hi) = region();
    let before = total(&ground::volume(&map, &reg, lo, hi));
    let surface = |map: &CubeMap, x: f64| {
        ground::raycast(map, &reg, DVec3::new(x, 2.5, 0.5), DVec3::NEG_Y, 5.0)
            .map_or(f64::NAN, |h| h.at.y)
    };
    let (here, across) = (0.925, 0.075);
    let mut dug = 0.0;
    let mut carry = 0.0;
    for _ in 0..60 {
        let hit = ground::raycast(&map, &reg, DVec3::new(here, 2.5, 0.5), DVec3::NEG_Y, 5.0)
            .expect("ground under");
        let got = total(&ground::dig(
            &mut map,
            &reg,
            hit.at,
            hit.normal,
            ground::DIG_RADIUS_M,
            0.012 + carry,
            &|_| true,
        ));
        carry += 0.012 - got;
        dug += got;
    }
    let (at, off) = (surface(&map, here), surface(&map, across));
    eprintln!("{dug:.2} m³ dug: the ground at {at:.3} where struck, {off:.3} across");
    assert!(dug > 0.6, "{dug} m³");
    assert!(at < off - 0.05, "{at} where struck, {off} across the voxel");
    // Conserved all the same.
    let after = total(&ground::volume(&map, &reg, lo, hi));
    assert!(
        (before - after - dug).abs() < 0.02,
        "{before} → {after}, {dug} dug"
    );
}

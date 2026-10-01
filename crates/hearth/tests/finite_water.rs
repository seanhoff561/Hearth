//! Finite water in a generated world: a channel dug from a river's bank fills to the river's
//! level with the river's water while the river itself is left as it is; a well dug below the
//! water table takes in groundwater up to the table.

use glam::DVec3;
use hearth::scene::LocalWorld;
use hearth::water_env::WorldWater;
use hearth_math::{BlockPos, PlanetSize};
use hearth_world::water::{FULL, WaterSim};
use hearth_world::{BlockStateId, StateFlags};

/// A river column whose neighbour toward +X is a dry bank two to four blocks above the water:
/// (x, z) of the river's last column.
fn river_bank(lw: &LocalWorld) -> Option<(i32, i32)> {
    let terrain = lw.terrain();
    let c = lw.map.planet().circumference();
    for zi in -60..60 {
        for xi in 0..96 {
            let (x, z) = (c * xi / 96, zi * 41);
            let s = terrain.sample(x, z);
            if s.river.is_none() || !s.is_underwater() || s.ocean || s.lake {
                continue;
            }
            for dx in 1..24 {
                let b = terrain.sample(x + dx, z);
                if b.is_underwater() {
                    continue;
                }
                let w = terrain.sample(x + dx - 1, z);
                let rise = b.height_i() - w.water_i();
                if (2..=4).contains(&rise) && w.river.is_some() && !w.ocean && !w.lake {
                    return Some((x + dx - 1, z));
                }
                break;
            }
        }
    }
    None
}

/// The top water block of a column, scanning down from `from`.
fn top_water(lw: &LocalWorld, x: i32, z: i32, from: i32) -> Option<i32> {
    (from - 40..=from).rev().find(|&y| {
        lw.map
            .block(BlockPos::new(x, y, z))
            .is_some_and(|s| lw.reg.has(s, StateFlags::WATER))
    })
}

/// The top solid block of a column, scanning down from `from`.
fn ground_top(lw: &LocalWorld, x: i32, z: i32, from: i32) -> i32 {
    (from - 60..=from)
        .rev()
        .find(|&y| {
            lw.map
                .block(BlockPos::new(x, y, z))
                .is_some_and(|s| lw.reg.has(s, StateFlags::FULL_COLLISION))
        })
        .expect("ground")
}

fn dig(lw: &mut LocalWorld, sim: &mut WaterSim, p: BlockPos) {
    let reg = lw.reg.clone();
    lw.map.set_block(p, BlockStateId::AIR, &reg);
    sim.block_changed(&mut lw.map, &reg, p);
}

#[test]
fn channels_and_wells_in_a_generated_world() {
    let mut lw = LocalWorld::create(11, PlanetSize::Tiny, 256, None).expect("world");
    let (rx, rz) = river_bank(&lw).expect("a river with a low bank");
    let centre = DVec3::new(
        rx as f64 + 8.0,
        lw.surface_y(rx as f64, rz as f64),
        rz as f64,
    );
    lw.load_area(centre, 2, 1, Some(0.4));
    let reg = lw.reg.clone();
    let generator = lw.generator.clone();
    let env = WorldWater {
        generator: &generator,
        air_c: 15.0,
        humidity: 0.7,
        wind_m_s: 2.0,
    };
    let mut sim = WaterSim::new(&reg).expect("sim");
    let surface = lw.surface_y(rx as f64, rz as f64) as i32 + 8;
    let wy = top_water(&lw, rx, rz, surface).expect("river water");
    let river = lw.map.block(BlockPos::new(rx, wy, rz)).expect("loaded");

    // A channel six blocks long from the river at its water level, open to the sky.
    for k in 1..=6 {
        let top = ground_top(&lw, rx + k, rz, surface);
        for y in wy..=top {
            dig(&mut lw, &mut sim, BlockPos::new(rx + k, y, rz));
        }
    }
    for _ in 0..20_000 {
        sim.tick(&mut lw.map, &reg, &env);
        if sim.is_settled() {
            break;
        }
    }
    assert!(sim.is_settled());
    let natural = generator
        .hydro
        .quality(&generator, rx, wy, rz)
        .expect("river water");
    for k in 1..=6 {
        let p = sim
            .parcel(&lw.map, BlockPos::new(rx + k, wy, rz))
            .unwrap_or_else(|| panic!("channel water at {k}"));
        assert_eq!(p.litres, FULL, "full at {k}");
        assert!((p.quality.salinity_g_l - natural.salinity_g_l).abs() < 1e-3);
        assert!(
            sim.parcel(&lw.map, BlockPos::new(rx + k, wy + 1, rz))
                .is_none(),
            "no higher than the river at {k}"
        );
    }
    assert_eq!(sim.budget.sourced, 6 * FULL as u64);
    assert_eq!(lw.map.block(BlockPos::new(rx, wy, rz)), Some(river));

    // A well beside the channel, from the ground down to three blocks below the water table.
    let (wx, wz) = (rx + 3, rz + 6);
    let table = generator.hydro.water_table(&generator, wx, wz);
    let top = ground_top(&lw, wx, wz, surface);
    let bottom = table.floor() as i32 - 3;
    assert!(bottom < top - 2, "a dry bank over the table: {top} {table}");
    for y in bottom..=top {
        dig(&mut lw, &mut sim, BlockPos::new(wx, y, wz));
    }
    let seep = generator.hydro.seepage(&generator, wx, bottom, wz);
    let days = (3.5 * FULL as f32 / seep).ceil().max(2.0);
    for _ in 0..40 {
        for _ in 0..200 {
            sim.tick(&mut lw.map, &reg, &env);
            if sim.is_settled() {
                break;
            }
        }
        sim.weather(&mut lw.map, &reg, &env, days / 40.0);
    }
    for y in bottom..table.ceil() as i32 - 1 {
        let l = sim
            .parcel(&lw.map, BlockPos::new(wx, y, wz))
            .map_or(0, |p| p.litres);
        assert_eq!(l, FULL, "well full at y {y} (table {table})");
    }
    assert!(
        sim.parcel(&lw.map, BlockPos::new(wx, table.ceil() as i32 + 1, wz))
            .is_none(),
        "no higher than the table"
    );
}

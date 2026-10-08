//! Finite water in a generated world: a channel dug from a river's bank fills to the river's
//! level with the river's water while the river itself is left as it is; a well dug below the
//! water table takes in groundwater up to the table.

use glam::DVec3;
use hearth::scene::LocalWorld;
use hearth::water_env::WorldWater;
use hearth_math::{BlockPos, PlanetSize};
use hearth_world::water::{FULL, WaterSim};
use hearth_world::{BlockStateId, StateFlags};

/// River columns whose neighbour toward +X is a dry bank two to four blocks above the water, as
/// the terrain's samples have it: (x, z) of the river's last column, in the order found. (Rows
/// some 14 blocks apart and columns some 85: most banks found do not hold a channel as their
/// blocks lie, so enough are looked at for some that do.)
fn river_banks(lw: &LocalWorld) -> Vec<(i32, i32)> {
    let terrain = lw.terrain();
    let c = lw.map.planet().circumference();
    let mut out = Vec::new();
    for zi in -175..175 {
        for xi in 0..192 {
            let (x, z) = (c * xi / 192, zi * 14);
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
                    out.push((x + dx - 1, z));
                }
                break;
            }
        }
    }
    out
}

/// Whether a six-block channel dug toward +X from the river at (x, z), at its water's level
/// `wy`, holds its water as the loaded blocks stand: solid ground beside it and at its far end
/// at that level, and under it; and no water touching what is dug but the river at its mouth.
/// (The terrain's samples round a river's surface and its banks otherwise than its blocks: a
/// bank a block below the water beside the channel, or a stream stepping down beside it and
/// feeding it from above the river's level, lets the water run through it and down the land
/// for ever, and it never rests.)
fn holds(lw: &LocalWorld, x: i32, z: i32, wy: i32, surface: i32) -> bool {
    let has = |x: i32, y: i32, z: i32, flag: StateFlags| {
        lw.map
            .block(BlockPos::new(x, y, z))
            .is_some_and(|s| lw.reg.has(s, flag))
    };
    let solid = |x: i32, y: i32, z: i32| has(x, y, z, StateFlags::FULL_COLLISION);
    let walled = (1..=6)
        .all(|k| solid(x + k, wy, z - 1) && solid(x + k, wy, z + 1) && solid(x + k, wy - 1, z))
        && solid(x + 7, wy, z);
    let dry = (1..=6).all(|k| {
        let top = ground_top(lw, x + k, z, surface).max(wy);
        (wy..=top + 1).all(|y| {
            [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, 1, 0)]
                .iter()
                .all(|&(dx, dy, dz)| {
                    let (nx, ny, nz) = (x + k + dx, y + dy, z + dz);
                    (nx, ny, nz) == (x, wy, z) || !has(nx, ny, nz, StateFlags::WATER)
                })
        })
    });
    walled && dry
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
    // The first low bank, loaded, that holds a channel.
    let mut site = None;
    for (rx, rz) in river_banks(&lw) {
        let centre = DVec3::new(
            rx as f64 + 8.0,
            lw.surface_y(rx as f64, rz as f64),
            rz as f64,
        );
        lw.load_area(centre, 2, 1, Some(0.4));
        let surface = lw.surface_y(rx as f64, rz as f64) as i32 + 8;
        if let Some(wy) = top_water(&lw, rx, rz, surface)
            && holds(&lw, rx, rz, wy, surface)
        {
            site = Some((rx, rz, surface, wy));
            break;
        }
    }
    let (rx, rz, surface, wy) = site.expect("a river with a low bank that holds a channel");
    let reg = lw.reg.clone();
    let generator = lw.generator.clone();
    let env = WorldWater {
        generator: &generator,
        air_c: 15.0,
        humidity: 0.7,
        wind_m_s: 2.0,
    };
    let mut sim = WaterSim::new(&reg).expect("sim");
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

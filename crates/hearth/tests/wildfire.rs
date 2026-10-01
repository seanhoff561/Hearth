//! V2-6 acceptance: a wildfire in the dry season spreads and burns out plausibly. In the driest
//! month of a summer-dry place, in hot dry air with a steady wind, a fire set in the grass runs
//! downwind far faster than it backs into the wind, leaves burned ground and charred trees, and
//! burns out; the burned ground stays burned when its terrain is generated again. In damp air
//! the same fire goes nowhere.

mod common;

use common::{World, temp};
use hearth_env::weather::WeatherHold;
use hearth_math::BlockPos;
use hearth_protocol::ToServer;
use hearth_worldgen::region::Surface;

/// A grassy place with a summer-dry climate near the player, and the year fraction of its
/// driest month.
fn dry_grass(w: &World) -> ((i32, i32), f64) {
    let wg = &w.generator;
    let grid = wg.terrain.grid.clone();
    let (px, pz) = (w.mover.pos.x as i32, w.mover.pos.z as i32);
    let mut best: Option<(f64, (i32, i32))> = None;
    for j in -40..=40 {
        for i in -40..=40 {
            let (x, z) = (wg.planet().wrap_x(px + i * 64), pz + j * 64);
            let s = wg.terrain.sample(x, z);
            if s.is_underwater() || s.surface != Surface::Grass || s.slope > 0.3 {
                continue;
            }
            let n = hearth_env::climate::Normals::sample(&grid, x as f64, z as f64);
            if n.summer_dry < 0.3 || s.tree_density > 0.5 {
                continue;
            }
            let d = (i * i + j * j) as f64 - 400.0 * n.summer_dry;
            if best.is_none_or(|(b, _)| d < b) {
                best = Some((d, (x, z)));
            }
        }
    }
    let at = best.expect("a summer-dry grassland near the spawn").1;
    let n = hearth_env::climate::Normals::sample(&grid, at.0 as f64, at.1 as f64);
    // The month whose rain falls furthest short of twice its warmth.
    let driest = (0..50)
        .map(|k| k as f64 / 50.0)
        .max_by(|a, b| {
            let dry = |y: f64| 2.0 * n.temperature(y) - n.precip_mm_per_day(y) * 30.4;
            dry(*a).total_cmp(&dry(*b))
        })
        .unwrap_or(0.55);
    (at, driest)
}

/// Burned ground and charred wood, and flames, in a square about a place.
fn count(w: &World, (x, z): (i32, i32), r: i32) -> (Vec<BlockPos>, usize, usize) {
    let mut burnt = Vec::new();
    let (mut charred, mut flames) = (0, 0);
    for dz in -r..=r {
        for dx in -r..=r {
            let (bx, bz) = (x + dx, z + dz);
            let g = w.generator.terrain.sample(bx, bz).height_i();
            for y in g - 3..g + 30 {
                let p = BlockPos::new(bx, y, bz);
                match w.block(p).as_deref() {
                    Some("burnt_ground") => burnt.push(p),
                    Some("flames") => flames += 1,
                    Some(n) if n.starts_with("charred_") => charred += 1,
                    _ => {}
                }
            }
        }
    }
    (burnt, charred, flames)
}

/// Lets game minutes pass.
fn minutes(w: &mut World, m: f64) {
    let ticks = (m / 1440.0 * w.ticks_per_day) as u64;
    let mut left = ticks;
    while left > 0 {
        let step = left.min(400);
        w.run(step);
        left -= step;
    }
}

#[test]
fn a_dry_season_wildfire_runs_downwind_and_burns_out() {
    let dir = temp("wildfire");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 7);
    let (at, driest) = dry_grass(&w);
    // To the driest month (around noon).
    let now = w.calendar.at(w.ticks);
    let days = (driest - now.year_frac).rem_euclid(1.0) * w.calendar.days_per_year();
    w.server.send(ToServer::SkipHours(days * 24.0));
    // Hot dry air and a steady wind toward the east.
    w.server.send(ToServer::HoldWeather(Some(WeatherHold {
        humidity: Some(0.22),
        temperature_c: Some(31.0),
        precip_mm_h: Some(0.0),
        wind_speed_m_s: Some(5.0),
        wind_dir: Some(std::f64::consts::FRAC_PI_2),
    })));
    // The player stands upwind of where the fire is set.
    w.go(at.0 as f64 - 14.5, at.1 as f64 + 0.5);
    w.run(20);
    let ground = w.generator.terrain.sample(at.0, at.1).height_i() - 1;
    let start = BlockPos::new(at.0, ground, at.1);
    println!(
        "fire set at {start:?} ({:?}) in year fraction {driest:.2}",
        w.block(start)
    );
    w.server.send(ToServer::Ignite(start));
    minutes(&mut w, 12.0);
    let (burnt, charred, flames) = count(&w, at, 40);
    let east = burnt.iter().filter(|p| p.x > at.0 + 4).count();
    let west = burnt.iter().filter(|p| p.x < at.0 - 4).count();
    println!(
        "after 12 minutes: {} burned ({east} east, {west} west), {charred} charred, {flames} \
         flames",
        burnt.len()
    );
    assert!(burnt.len() > 60, "the fire spread: {} burned", burnt.len());
    assert!(east > 3 * west.max(1), "downwind {east}, upwind {west}");
    // It burns out over the next hours (the land beyond the loaded terrain is the far fire's).
    minutes(&mut w, 180.0);
    let (burnt, charred, flames) = count(&w, at, 40);
    println!(
        "after 3 hours: {} burned, {charred} charred, {flames} flames",
        burnt.len()
    );
    assert_eq!(flames, 0, "burned out");
    // The burned ground stays burned when its terrain is generated again.
    w.server.send(ToServer::HoldWeather(None));
    let home = w.mover.pos;
    w.go(home.x + 300.0, home.z);
    w.run(20);
    w.go_exact(home);
    w.run(20);
    let (again, _, _) = count(&w, at, 40);
    println!("generated again: {} burned", again.len());
    assert!(
        again.len() * 10 >= burnt.len() * 7,
        "{} burned before, {} after",
        burnt.len(),
        again.len()
    );
    drop(w);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn in_damp_air_a_fire_in_the_grass_goes_nowhere() {
    let dir = temp("damp_fire");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 7);
    let (at, _) = dry_grass(&w);
    w.server.send(ToServer::HoldWeather(Some(WeatherHold {
        humidity: Some(0.95),
        temperature_c: Some(12.0),
        precip_mm_h: Some(0.0),
        wind_speed_m_s: Some(3.0),
        wind_dir: Some(std::f64::consts::FRAC_PI_2),
    })));
    w.go(at.0 as f64 - 14.5, at.1 as f64 + 0.5);
    w.run(20);
    let ground = w.generator.terrain.sample(at.0, at.1).height_i() - 1;
    w.server
        .send(ToServer::Ignite(BlockPos::new(at.0, ground, at.1)));
    minutes(&mut w, 30.0);
    let (burnt, _, flames) = count(&w, at, 30);
    println!("damp: {} burned, {flames} flames", burnt.len());
    assert!(burnt.len() < 25, "{} burned in damp air", burnt.len());
    drop(w);
    let _ = std::fs::remove_dir_all(&dir);
}

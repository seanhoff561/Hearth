//! The globe (E4.1 §4.2): its map made from the planet grid in about a second at most, land
//! showing as land in the grid's own share (some 29 % on the Earth-like planet), not one
//! colour, kept and read back the same; and a click's details found on a thread of their own,
//! kept for the place.

use std::collections::HashSet;
use std::f64::consts::{FRAC_PI_2, PI};
use std::sync::Arc;
use std::time::{Duration, Instant};

use hearth::globe::{Details, MAP_WIDTH, RELIEF_WIDTH, cached_map};
use hearth::scene::LocalWorld;
use hearth_math::PlanetSize;

#[test]
fn the_globes_map_shows_land_and_sea_as_the_grid_has_them() {
    let lw = LocalWorld::create(7, PlanetSize::Earth, 256, None).expect("world");
    let terrain = lw.terrain();
    let kept = std::env::temp_dir().join(format!("hearth-test-globe-{}.zst", std::process::id()));
    let _ = std::fs::remove_file(&kept);
    let t0 = Instant::now();
    let map = cached_map(terrain, MAP_WIDTH, Some(&kept));
    let took = t0.elapsed().as_secs_f64();
    let (w, h) = (MAP_WIDTH, MAP_WIDTH / 2);
    assert_eq!(map.color.len(), w * h);
    assert_eq!(map.facts.len(), w * h);
    assert_eq!(map.elevation.len(), RELIEF_WIDTH * RELIEF_WIDTH / 2);
    let planet = terrain.planet();
    let edge = planet.latitude(-planet.pole_edge_z() + 1.0);
    // The map's land by area (a texel's area goes with the cosine of its latitude): any texel
    // not under water.
    let (mut land, mut all) = (0.0, 0.0);
    for y in 0..h {
        let lat = FRAC_PI_2 - (y as f64 + 0.5) / h as f64 * PI;
        if lat.abs() > edge {
            continue;
        }
        for i in y * w..(y + 1) * w {
            all += lat.cos();
            if !map.wet(i) {
                land += lat.cos();
            }
        }
    }
    let map_land = land / all;
    // The grid's, over the same latitudes (its rows are even in the world's Mercator z, so a
    // cell's area goes with the cosine's square).
    let g = &terrain.grid;
    let (mut land, mut all) = (0.0, 0.0);
    for j in 0..g.n() {
        let lat = g.geom.lat[j];
        if lat.abs() > edge {
            continue;
        }
        let area = lat.cos().powi(2);
        for i in 0..g.n() {
            all += area;
            if g.elevation.data[g.geom.idx(i, j)] > 0.0 {
                land += area;
            }
        }
    }
    let grid_land = land / all;
    println!(
        "map {:.1} % land, grid {:.1} %, made in {took:.2} s",
        100.0 * map_land,
        100.0 * grid_land
    );
    assert!((0.22..0.36).contains(&grid_land), "an Earth-like planet");
    assert!(
        (map_land - grid_land).abs() < 0.03,
        "the map's land {map_land:.3}, the grid's {grid_land:.3}"
    );
    let colours: HashSet<[u8; 4]> = map.color.iter().copied().collect();
    assert!(
        colours.len() > 20,
        "a map, not a wash: {} colours",
        colours.len()
    );
    // Its relief: the land's heights up to mountains, the sea's floor down to its deeps; rivers
    // on the land; ice at the poles.
    let heights: Vec<f32> = (0..map.elevation.len())
        .map(|i| map.elevation_m(i))
        .collect();
    let top = heights.iter().copied().fold(f32::MIN, f32::max);
    let deep = heights.iter().copied().fold(f32::MAX, f32::min);
    let rivers = map.facts.iter().filter(|f| f[0] > 0).count();
    let ice = map.facts.iter().filter(|f| f[1] == 255).count();
    println!("heights {deep:.0} to {top:.0} m, {rivers} river texels, {ice} of ice");
    assert!(top > 2000.0 && deep < -2000.0, "{deep} to {top}");
    assert!(rivers > 1000 && ice > 1000, "{rivers} rivers, {ice} ice");
    assert!(took < 3.0, "made in {took:.2} s");
    // Kept on a thread of its own, the file whole when it is there: read back, the same map.
    let whole = |at: Instant| loop {
        if let Ok(m) = std::fs::metadata(&kept) {
            break m.len();
        }
        assert!(at.elapsed() < Duration::from_secs(30), "the map was kept");
        std::thread::sleep(Duration::from_millis(10));
    };
    let size = whole(Instant::now());
    let t0 = Instant::now();
    let read = cached_map(terrain, MAP_WIDTH, Some(&kept));
    println!(
        "kept in {:.1} MB, read back in {:.3} s",
        size as f64 / 1e6,
        t0.elapsed().as_secs_f64()
    );
    assert!(read == map, "the map read back is the map made");
    // A file cut short is no map: made again, the same, and kept whole again.
    let bytes = std::fs::read(&kept).expect("the kept map");
    std::fs::write(&kept, &bytes[..bytes.len() / 2]).expect("cut short");
    assert!(cached_map(terrain, MAP_WIDTH, Some(&kept)) == map);
    let t0 = Instant::now();
    while std::fs::metadata(&kept).map_or(0, |m| m.len()) != size {
        assert!(t0.elapsed() < Duration::from_secs(30), "kept whole again");
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = std::fs::remove_file(&kept);
}

#[test]
fn a_clicks_details_come_from_a_thread_and_are_kept() {
    let lw = LocalWorld::create(7, PlanetSize::Standard, 256, None).expect("world");
    let finder = Arc::new(hearth::places::Finder::new(
        lw.generator.clone(),
        lw.content.clone(),
        lw.reg.clone(),
    ));
    let (x, z) = lw.terrain().find_spawn(false);
    let at = hearth::globe::lat_lon(lw.map.planet(), glam::DVec3::new(x as f64, 0.0, z as f64));
    let mut details = Details::default();
    let when = hearth::places::When::Spring;
    // Asked: nothing yet, the search under way.
    assert!(details.ask(&finder, at, when).is_none());
    assert!(details.looking());
    let t0 = Instant::now();
    let found = loop {
        if let Some((place, found)) = details.poll() {
            assert_eq!(place, at);
            break found;
        }
        assert!(t0.elapsed() < Duration::from_secs(60), "the details came");
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(!details.looking());
    // Asked again: answered at once, the same.
    assert_eq!(details.ask(&finder, at, when), Some(found));
    assert!(!details.looking());
    // A click elsewhere while one is under way: only the last one's answer comes.
    let other = (at.0 + 0.2, at.1 + 0.3);
    assert!(details.ask(&finder, other, when).is_none());
    let last = (at.0 - 0.2, at.1 - 0.3);
    assert!(details.ask(&finder, last, when).is_none());
    let t0 = Instant::now();
    loop {
        if let Some((place, _)) = details.poll() {
            assert_eq!(place, last, "the last click's answer");
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(60), "the details came");
        std::thread::sleep(Duration::from_millis(5));
    }
}

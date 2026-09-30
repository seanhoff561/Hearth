//! Seasonal cover on loaded terrain follows the calendar and is fully reversible: winter lays
//! snow on a summer landscape, and the next summer gives back exactly the blocks it started
//! with (buried plants included); rivers flood and fall with their regimes and come back to
//! the same blocks.

use glam::DVec3;
use hearth::scene::LocalWorld;
use hearth::season_cover::column_cover;
use hearth_math::{BlockPos, ColumnPos, CubePos, PlanetSize};

const SUMMER: f64 = 0.4;
const WINTER: f64 = 0.9;

/// A land column with deep winter snow and none in summer (northern hemisphere).
fn snowy_place(lw: &LocalWorld) -> Option<ColumnPos> {
    let planet = *lw.map.planet();
    let terrain = lw.terrain();
    let c = planet.circumference();
    for zi in 1..64 {
        let z = -(planet.pole_edge_z() * zi as f64 / 64.0) as i32;
        for xi in 0..32 {
            let x = c * xi / 32;
            let s = terrain.sample(x, z);
            if s.is_underwater() || s.ocean || s.lake {
                continue;
            }
            let col = ColumnPos::new(x >> 4, z >> 4);
            let winter = column_cover(&lw.generator, col, WINTER).snow_m;
            let summer = column_cover(&lw.generator, col, SUMMER).snow_m;
            if winter > 0.4 && summer == 0.0 {
                return Some(col);
            }
        }
    }
    None
}

fn snapshot(lw: &LocalWorld, cubes: &[CubePos]) -> Vec<(BlockPos, hearth_world::BlockStateId)> {
    let mut out = Vec::new();
    for &c in cubes {
        let min = c.min_block();
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let p = BlockPos::new(min.x + x, min.y + y, min.z + z);
                    if let Some(b) = lw.map.block(p) {
                        out.push((p, b));
                    }
                }
            }
        }
    }
    out
}

#[test]
fn winter_comes_and_goes() {
    let mut lw = LocalWorld::create(11, PlanetSize::Tiny, 256, None).expect("world");
    let col = snowy_place(&lw).expect("a place with snowy winters");
    let (x0, z0) = col.min_block_xz();
    let (x, z) = (x0 as f64 + 8.0, z0 as f64 + 8.0);
    let centre = DVec3::new(x, lw.surface_y(x, z), z);
    let cubes = lw.load_area(centre, 1, 1, Some(SUMMER));
    let cols: Vec<ColumnPos> = {
        let mut v: Vec<ColumnPos> = cubes.iter().map(|c| c.column()).collect();
        v.sort_unstable_by_key(|c| (c.x, c.z));
        v.dedup();
        v
    };
    let summer = snapshot(&lw, &cubes);
    let snow = |lw: &LocalWorld| {
        snapshot(lw, &cubes)
            .iter()
            .filter(|(_, b)| lw.cover.states.snow_layers.contains(b))
            .count()
    };
    assert_eq!(snow(&lw), 0, "no snow in summer");

    let changed = lw
        .cover
        .refresh(&mut lw.map, &lw.reg, &lw.generator, &cols, WINTER);
    let winter_snow = snow(&lw);
    assert!(winter_snow > 100, "winter snow on {winter_snow} blocks");
    assert!(changed.len() >= winter_snow);

    let changed_back = lw
        .cover
        .refresh(&mut lw.map, &lw.reg, &lw.generator, &cols, SUMMER);
    assert_eq!(
        changed_back.len(),
        changed.len(),
        "the thaw undoes every change"
    );
    assert_eq!(
        snapshot(&lw, &cubes),
        summer,
        "the summer landscape is back, block for block"
    );
}

/// A channel column of the most seasonal river reach (of some width) near the planet's rivers,
/// with the dates of its high and low water and of a flow near the mean.
fn seasonal_river(lw: &LocalWorld) -> Option<((i32, i32), f64, f64, f64)> {
    let grid = lw.grid();
    let rivers = &lw.cover.rivers;
    let terrain = lw.terrain();
    let dates: Vec<f64> = (0..73).map(|k| (k as f64 + 0.5) / 73.0).collect();
    let mut best: Option<(f64, (i32, i32), f64, f64, f64)> = None;
    for &cell in grid.rivers.keys() {
        let flows: Vec<f64> = dates.iter().map(|t| rivers.flow(cell, *t)).collect();
        let (hi, lo) = flows
            .iter()
            .fold((0.0f64, f64::MAX), |a, f| (a.0.max(*f), a.1.min(*f)));
        let swing = hi - lo;
        if best.as_ref().is_some_and(|b| b.0 >= swing) {
            continue;
        }
        // A column of this reach's channel near the cell centre.
        let (i, j) = grid.geom.ij(cell as usize);
        let (cx, cz) = grid.geom.world_xz(i, j);
        let found = (-24..=24).step_by(3).find_map(|dz| {
            (-24..=24).step_by(3).find_map(|dx| {
                let (x, z) = (cx as i32 + dx, cz as i32 + dz);
                let s = terrain.sample(x, z);
                let r = s.river?;
                (r.cell == cell && s.is_underwater() && r.width >= 5.0 && !s.ocean && !s.lake)
                    .then_some((x, z))
            })
        });
        let Some(at) = found else {
            continue;
        };
        let pick = |f: &dyn Fn(f64) -> f64| {
            dates
                .iter()
                .copied()
                .zip(&flows)
                .min_by(|a, b| f(*a.1).total_cmp(&f(*b.1)))
                .map(|(t, _)| t)
                .unwrap_or(0.0)
        };
        let t_hi = pick(&|f| -f);
        let t_lo = pick(&|f| f);
        let t_mean = pick(&|f| (f - 1.0).abs());
        best = Some((swing, at, t_hi, t_lo, t_mean));
    }
    best.map(|(_, at, hi, lo, mean)| (at, hi, lo, mean))
}

#[test]
fn rivers_rise_and_fall_with_the_seasons() {
    let mut lw = LocalWorld::create(11, PlanetSize::Tiny, 256, None).expect("world");
    let ((x, z), t_hi, t_lo, t_mean) = seasonal_river(&lw).expect("a seasonal river");
    let rivers = lw.cover.rivers.clone();
    let cell = lw.terrain().sample(x, z).river.expect("river").cell;
    assert!(
        rivers.flow(cell, t_hi) > 1.8 && rivers.flow(cell, t_lo) < 0.6,
        "a seasonal reach: {} / {}",
        rivers.flow(cell, t_hi),
        rivers.flow(cell, t_lo)
    );
    let centre = DVec3::new(x as f64, lw.surface_y(x as f64, z as f64), z as f64);
    let cubes = lw.load_area(centre, 1, 1, Some(t_mean));
    let cols: Vec<ColumnPos> = {
        let mut v: Vec<ColumnPos> = cubes.iter().map(|c| c.column()).collect();
        v.sort_unstable_by_key(|c| (c.x, c.z));
        v.dedup();
        v
    };
    let start = snapshot(&lw, &cubes);
    let water = |lw: &LocalWorld| {
        snapshot(lw, &cubes)
            .iter()
            .filter(|(_, b)| *b == lw.cover.states.water)
            .count()
    };
    let normal = water(&lw);
    lw.cover
        .refresh(&mut lw.map, &lw.reg, &lw.generator, &cols, t_hi);
    let high = water(&lw);
    lw.cover
        .refresh(&mut lw.map, &lw.reg, &lw.generator, &cols, t_lo);
    let low = water(&lw);
    assert!(
        high > normal && low < normal,
        "water blocks: high {high}, mean {normal}, low {low}"
    );
    lw.cover
        .refresh(&mut lw.map, &lw.reg, &lw.generator, &cols, t_mean);
    assert_eq!(
        snapshot(&lw, &cubes),
        start,
        "back at the same date, the same blocks"
    );
}

//! The refinement levels' tests.

use super::*;
use crate::WorldGenSettings;
use hearth_math::PlanetSize;

fn earth(seed: u64) -> Relief {
    let s = WorldGenSettings {
        seed,
        planet_size: PlanetSize::Earth,
        grid_resolution: 256,
    };
    Relief::new(Arc::new(PlanetGrid::build(&s.sanitized(), &|_, _| {})))
}

/// The walking level drains (G1b): over some of its land, closed hollows deeper than a
/// quarter of a metre hold under a hundredth of the ground outside its lakes, and its second
/// pass leaves fewer than its first pass's blend alone would. Each tile drains its own span;
/// the second pass cuts what their blend dams. So too as the blocks read it, between its
/// cells, its lakes' water on it: it drains along its rows and columns (its channels too),
/// as a cell draining only across a corner, between two higher, would hold a hollow there.
#[test]
fn the_walking_level_drains() {
    let r = earth(5);
    let top = r.levels().len();
    assert!(r.walking(top), "{:?}", r.levels());
    let scale = RATIO.pow(top as u32 - 1);
    let n = 160usize;
    // The ground in hollows deeper than a quarter of a metre (outside lakes and the sea),
    // filled from the edges, nothing cut.
    let hollows = |c: &[Cell]| {
        let h: Vec<f32> = c
            .iter()
            .map(|c| {
                if c.lake.is_finite() {
                    c.lake.max(c.h)
                } else {
                    c.h
                }
            })
            .collect();
        let mut filled = h.clone();
        let edge: Vec<bool> = (0..n * n)
            .map(|k| k % n == 0 || k / n == 0 || k % n == n - 1 || k / n == n - 1)
            .collect();
        fill(&mut filled, &edge, n, 0.0, &vec![-1.0; n * n], &D4);
        (0..n * n)
            .filter(|&k| c[k].lake.is_nan() && !c[k].sea)
            .fold((0usize, 0usize), |(d, all), k| {
                (d + (filled[k] - h[k] > 0.25 * r.v as f32) as usize, all + 1)
            })
    };
    // The same as the blocks read it: each block's column between the cells, a lake's
    // water over the ground below its surface, the sea's samples outlets.
    let blocks = |i0: i64, j0: i64| {
        let s = r.cell_size(top);
        let (x0, z0) = r.centre(top, i0, j0);
        let side = (n as f64 * s) as usize;
        let patch = r.patch(top, (x0, z0), (x0 + side as f64, z0 + side as f64), 2);
        let (mut h, mut sea) = (vec![0.0f32; side * side], vec![false; side * side]);
        for k in 0..side * side {
            let (x, z) = (x0 + (k % side) as f64 + 0.5, z0 + (k / side) as f64 + 0.5);
            let (wet_sea, lakes, level) = patch.water(x, z);
            h[k] = patch.height(x, z);
            if let Some(level) = level
                && lakes > 0.0
            {
                h[k] = h[k].max(level);
            }
            sea[k] = wet_sea > 0.0 || k % side == 0 || k / side == 0;
            sea[k] |= k % side == side - 1 || k / side == side - 1;
        }
        let mut filled = h.clone();
        fill(&mut filled, &sea, side, 0.0, &vec![-1.0; side * side], &D8);
        let deep = (0..side * side)
            .filter(|&k| !sea[k] && filled[k] - h[k] > 0.25 * r.v as f32)
            .count();
        (deep, side * side)
    };
    let (mut deep, mut first, mut cells) = (0, 0, 0);
    let (mut deep_blocks, mut all_blocks) = (0, 0);
    let places = (0..6).map(|nth| land(&r, nth));
    for (i1, j1) in places.chain((0..3).map(|nth| wet_hills(&r, nth))) {
        let (i0, j0) = (i1 * scale, j1 * scale);
        let (d, all) = blocks(i0, j0);
        deep_blocks += d;
        all_blocks += all;
        let at = |k: usize| (i0 + (k % n) as i64, j0 + (k / n) as i64);
        let drained: Vec<Cell> = (0..n * n)
            .map(|k| {
                let (i, j) = at(k);
                r.cell(top, i, j)
            })
            .collect();
        let mut run = Run::default();
        let blended: Vec<Cell> = (0..n * n)
            .map(|k| {
                let (i, j) = at(k);
                r.blended_in(top, i, j, &mut run)
            })
            .collect();
        let (d, all) = hollows(&drained);
        deep += d;
        cells += all;
        first += hollows(&blended).0;
    }
    assert!(
        (deep as f64) < 0.01 * cells as f64,
        "{deep} of {cells} cells in hollows"
    );
    assert!(
        deep <= first,
        "{deep} after the second pass, {first} before"
    );
    assert!(
        (deep_blocks as f64) < 0.01 * all_blocks as f64,
        "{deep_blocks} of {all_blocks} blocks in hollows"
    );
}

/// The walking level's channels step along its rows and columns (G1b): one stepping across
/// a corner between two banks would rise between its cells as the blocks read them.
#[test]
fn walking_channels_step_along_rows_and_columns() {
    let r = earth(5);
    let top = r.levels().len();
    let scale = RATIO.pow(top as u32 - 1);
    let (mut straight, mut channels) = (0, 0);
    for nth in 0..3 {
        let (i1, j1) = wet_hills(&r, nth);
        let (tx, tz) = ((i1 * scale).div_euclid(TILE), (j1 * scale).div_euclid(TILE));
        for (dx, dz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let tile = r.tile(top, tx + dx, tz + dz);
            let t = made(&tile);
            for k in 0..t.h.len() {
                if t.channel[k] && t.recv[k] != OUTLET {
                    channels += 1;
                    let (du, dw) = D8[t.recv[k] as usize];
                    straight += (du == 0 || dw == 0) as usize;
                }
            }
        }
    }
    assert!(channels > 100, "{channels} channel cells");
    assert_eq!(straight, channels, "of {channels} channel cells");
}

/// Read between the cells, a surface stays within the two values it lies between (no dip
/// below a valley's floor, no rise over a crest), and is Catmull-Rom's where the values rise
/// steadily.
#[test]
fn reads_between_cells_stay_between_them() {
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 11) as f64 / (1u64 << 53) as f64 * 20.0 - 10.0
    };
    for _ in 0..10_000 {
        let p = [next(), next(), next(), next()];
        let (lo, hi) = (p[1].min(p[2]), p[1].max(p[2]));
        for k in 0..=16 {
            let v = monotone(p, k as f64 / 16.0);
            assert!(v >= lo - 1e-9 && v <= hi + 1e-9, "{p:?} at {k}/16: {v}");
        }
    }
    let steady = [1.0, 2.0, 3.5, 4.0];
    for k in 0..=8 {
        let t = k as f64 / 8.0;
        assert!((monotone(steady, t) - catmull(steady, t)).abs() < 1e-12);
    }
}

/// A far read stands in for the walking level (G1b): the relief the walking level lays on
/// its parent's surface, and its stand-in without drainage, spread alike.
#[test]
fn a_far_read_stands_in_for_the_walking_level() {
    let r = earth(5);
    let top = r.levels().len();
    let scale = RATIO.pow(top as u32 - 1);
    let s = r.cell_size(top);
    let (i1, j1) = land(&r, 1);
    let (mut own, mut proxy) = (Vec::new(), Vec::new());
    for k in 0..80 * 80 {
        let (i, j) = (i1 * scale + (k % 80) as i64, j1 * scale + (k / 80) as i64);
        let (x, z) = r.centre(top, i, j);
        let base = r.height(top - 1, x, z) as f64;
        let slope = (
            (r.height(top - 1, x + s, z) as f64 - base) / s,
            (r.height(top - 1, x, z + s) as f64 - base) / s,
        );
        own.push(r.cell(top, i, j).h as f64 - base);
        proxy.push(r.walking_proxy(x, z, slope, 0.0));
    }
    let spread = |v: &[f64]| {
        let m = v.iter().sum::<f64>() / v.len() as f64;
        (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64).sqrt()
    };
    let (a, b) = (spread(&own), spread(&proxy));
    assert!(
        b > 0.0 && (0.5..2.0).contains(&(a / b)),
        "own {a}, stand-in {b}"
    );
}

#[test]
fn earth_has_levels_down_to_metres_and_a_test_planet_none() {
    let r = earth(3);
    let cells: Vec<f64> = r.levels().iter().map(|l| l.cell).collect();
    assert!(cells.len() >= 4, "{cells:?}");
    assert!(
        r.walking(cells.len()),
        "the finest is the walking scale's: {cells:?}"
    );
    assert!(*cells.last().unwrap() >= MIN_CELL && *cells.last().unwrap() < MIN_CELL * 8.0);
    let s = WorldGenSettings {
        seed: 3,
        planet_size: PlanetSize::Standard,
        grid_resolution: 2048,
    };
    let std = Relief::new(Arc::new(PlanetGrid::build(&s.sanitized(), &|_, _| {})));
    assert!(std.levels().is_empty(), "the grid is already fine");
}

/// A land cell of the grid away from the poles: (its centre's x, z) at the first level's
/// indices.
fn land(r: &Relief, nth: usize) -> (i64, i64) {
    let g = &r.grid;
    let n = g.n();
    let idx = (n * n / 4..n * n * 3 / 4)
        .filter(|&i| g.flags[i] & flags::OCEAN == 0 && g.elevation.data[i] > 50.0)
        .nth(nth * 97)
        .expect("land");
    let (x, z) = g.geom.world_xz(idx % n, idx / n);
    let cell = r.levels()[0].cell;
    ((x / cell) as i64, ((z + r.c * 0.5) / cell) as i64)
}

/// The first level's indices of the nth stretch of wet hills (relief of a hundred metres
/// and more, over a metre of rain), where streams run between steep banks.
fn wet_hills(r: &Relief, nth: usize) -> (i64, i64) {
    let g = &r.grid;
    let n = g.n();
    let idx = (n * n / 4..n * n * 3 / 4)
        .filter(|&i| {
            let (gx, gz) = ((i % n) as f64, (i / n) as f64);
            g.flags[i] & flags::OCEAN == 0
                && g.elevation.data[i] > 50.0
                && g.precipitation.bilinear(gx, gz) > 1000.0
                && r.roughness_at(gx, gz).amp > 50.0
        })
        .nth(nth * 5)
        .expect("wet hills");
    let (x, z) = g.geom.world_xz(idx % n, idx / n);
    let cell = r.levels()[0].cell;
    ((x / cell) as i64, ((z + r.c * 0.5) / cell) as i64)
}

#[test]
fn rivers_fall_all_the_way_down_through_every_level() {
    let r = earth(11);
    for nth in 0..3 {
        let (mut i0, mut j0) = land(&r, nth);
        for level in 1..=r.levels().len() {
            if level > 1 {
                (i0, j0) = (i0 * RATIO, j0 * RATIO);
            }
            for j in j0 - 40..j0 + 40 {
                for i in i0 - 40..i0 + 40 {
                    let c = r.cell(level, i, j);
                    let Some((ri, rj)) = c.receiver else {
                        continue;
                    };
                    if !c.channel || c.sea || c.lake.is_finite() {
                        continue;
                    }
                    let next = r.cell(level, ri, rj);
                    let surface = if next.lake.is_finite() {
                        next.lake
                    } else {
                        next.h
                    };
                    assert!(
                        next.sea || surface <= c.h + 0.5,
                        "level {level}: ({i}, {j}) at {} m runs up to ({ri}, {rj}) at {surface} m",
                        c.h
                    );
                }
            }
        }
    }
}

#[test]
fn each_level_keeps_its_parents_surface_on_the_whole() {
    let r = earth(13);
    let (mut i0, mut j0) = land(&r, 1);
    for level in 1..=r.levels().len() {
        if level > 1 {
            (i0, j0) = (i0 * RATIO, j0 * RATIO);
        }
        let cell = r.levels()[level - 1].cell;
        let (mut mean, mut parent) = (0.0, 0.0);
        for j in j0 - 30..j0 + 30 {
            for i in i0 - 30..i0 + 30 {
                mean += r.cell(level, i, j).h as f64;
                let x = (i as f64 + 0.5) * cell;
                let z = -r.c * 0.5 + (j as f64 + 0.5) * cell;
                parent += r.height(level - 1, x, z) as f64;
            }
        }
        let (mean, parent) = (mean / 3600.0, parent / 3600.0);
        // Within a tenth of the height, or a few metres on low ground.
        assert!(
            (mean - parent).abs() <= 0.1 * parent.abs() + 5.0,
            "level {level}: {mean:.1} m against its parent's {parent:.1} m"
        );
    }
}

#[test]
fn a_river_bends_through_its_nodes_instead_of_turning_at_them() {
    // Two reaches round a right angle (nodes 8 cells apart), without wander: where one
    // reach meets the next the course runs on (as chords it turned 90° there), and no piece
    // of a cell turns far from the one before (a curve through the corner's node swings a
    // little wide before it).
    let nodes = [
        (0.0, 0.0),
        (8.0, 0.0),
        (16.0, 0.0),
        (16.0, 8.0),
        (16.0, 16.0),
    ];
    let heading = |p: (f64, f64), q: (f64, f64)| (q.1 - p.1).atan2(q.0 - p.0).to_degrees();
    let turn = |a: f64, b: f64| ((b - a + 540.0).rem_euclid(360.0) - 180.0).abs();
    let mut course: Vec<(f64, f64)> = Vec::new();
    for k in 1..3 {
        let reach = (
            Some(nodes[k - 1]),
            nodes[k],
            nodes[k + 1],
            Some(nodes[k + 2]),
        );
        let path = meander(reach, (8.0, 1.0), 1, 1e9, 0.0);
        assert_eq!((path[0], *path.last().unwrap()), (nodes[k], nodes[k + 1]));
        course.extend(&path[usize::from(k > 1)..]);
    }
    let headings: Vec<f64> = course.windows(2).map(|w| heading(w[0], w[1])).collect();
    let sharpest = headings
        .windows(2)
        .map(|h| turn(h[0], h[1]))
        .fold(0.0, f64::max);
    assert!(sharpest < 25.0, "the course turns {sharpest:.0}° at once");
}

#[test]
fn a_cell_has_one_value_whichever_tile_is_made_first() {
    let a = earth(5);
    let b = earth(5);
    let lv = a.levels()[0];
    // Cells along a tile's edge, asked for from either side first.
    let j = 3 * TILE + 5;
    let i = 40 * TILE;
    let va: Vec<f32> = (i - 3..i + 3).map(|ii| a.cell(1, ii, j).h).collect();
    let vb: Vec<f32> = (i - 3..i + 3).rev().map(|ii| b.cell(1, ii, j).h).collect();
    let vb: Vec<f32> = vb.into_iter().rev().collect();
    assert_eq!(va, vb);
    assert!(lv.cell > 0.0);
}

//! `bench relief`: the refinement levels (E §5.2) about a point, as shaded relief maps — the
//! grid's surface and each level's, at widths that suit them — with the time each level's
//! tiles take.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use hearth_math::PlanetSize;
use hearth_worldgen::WorldGenSettings;
use hearth_worldgen::planet::PlanetGrid;
use hearth_worldgen::relief::Relief;
use rayon::prelude::*;

use crate::image::{Image, ramp, shade};

struct Args {
    /// `mountains` (the highest land), `coast` (a temperate coast) or `plain` (low flat land).
    site: Option<String>,
    seed: u64,
    res: usize,
    at: Option<(f64, f64)>,
    px: usize,
    out: PathBuf,
    /// A level and a pixel of its map whose cells to print.
    probe: Option<(usize, usize, usize)>,
    /// Time tiles made cold at each level instead of drawing maps.
    timing: bool,
}

fn parse(args: &[String]) -> anyhow::Result<Args> {
    let mut a = Args {
        site: None,
        seed: 7,
        res: 2048,
        at: None,
        px: 512,
        out: PathBuf::from("bench-out/relief"),
        probe: None,
        timing: false,
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut val = || {
            it.next()
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("{arg} needs a value"))
        };
        match arg.as_str() {
            "--seed" => a.seed = val()?.parse()?,
            "--res" => a.res = val()?.parse()?,
            "--px" => a.px = val()?.parse()?,
            "--site" => a.site = Some(val()?),
            "--out" => a.out = PathBuf::from(val()?),
            "--timing" => a.timing = true,
            "--probe" => {
                let v = val()?;
                let p: Vec<usize> = v.split(',').map(str::parse).collect::<Result<_, _>>()?;
                let [l, x, y] = p[..] else {
                    anyhow::bail!("--probe LEVEL,PX,PY");
                };
                a.probe = Some((l, x, y));
            }
            "--at" => {
                let v = val()?;
                let (x, z) = v
                    .split_once(',')
                    .ok_or_else(|| anyhow::anyhow!("--at X,Z"))?;
                a.at = Some((x.parse()?, z.parse()?));
            }
            other => anyhow::bail!("unknown option {other}"),
        }
    }
    Ok(a)
}

/// A planet's grid, built once per seed, size and resolution and kept under
/// `bench-out/planets`.
pub fn cached_grid(seed: u64, size: PlanetSize, res: usize) -> anyhow::Result<PlanetGrid> {
    let dir = PathBuf::from("bench-out/planets");
    let path = dir.join(format!("planet_{seed}_{}_{res}.bin.zst", size.name()));
    if let Ok(g) = PlanetGrid::load(&path) {
        return Ok(g);
    }
    let s = WorldGenSettings {
        seed,
        planet_size: size,
        grid_resolution: res,
    }
    .sanitized();
    let t0 = Instant::now();
    let g = PlanetGrid::build(&s, &|_, _| {});
    println!("grid built in {:.1}s", t0.elapsed().as_secs_f64());
    std::fs::create_dir_all(&dir)?;
    g.save(&path)?;
    Ok(g)
}

/// The highest land outside the polar caps: a mountain range to look at.
fn mountains(g: &PlanetGrid) -> (f64, f64) {
    let n = g.n();
    let mut best = (0usize, f32::MIN);
    for j in n / 6..n * 5 / 6 {
        for i in 0..n {
            let idx = j * n + i;
            let e = g.elevation.data[idx];
            if e > best.1 {
                best = (idx, e);
            }
        }
    }
    let (i, j) = (best.0 % n, best.0 / n);
    println!(
        "the highest land: {:.0} m, uplift {:.0} m",
        best.1,
        g.uplift.cell(i / 2, j / 2)
    );
    g.geom.world_xz(i, j)
}

/// A temperate coast: land at 40–50° beside the open sea.
fn coast(g: &PlanetGrid) -> (f64, f64) {
    let n = g.n();
    for j in 0..n {
        let lat = g.geom.lat[j].to_degrees();
        if !(40.0..50.0).contains(&lat.abs()) {
            continue;
        }
        for i in 0..n {
            let idx = j * n + i;
            let land = g.elevation.data[idx] > 0.0;
            let sea_next =
                i + 1 < n && g.flags[idx + 1] & hearth_worldgen::planet::flags::OCEAN != 0;
            if land && sea_next && g.elevation.data[idx] < 300.0 {
                return g.geom.world_xz(i, j);
            }
        }
    }
    mountains(g)
}

/// Low flat land: a lowland far from the coast with little relief about it.
fn plain(g: &PlanetGrid) -> (f64, f64) {
    let n = g.n();
    for j in n / 4..n * 3 / 4 {
        for i in 0..n {
            let idx = j * n + i;
            let e = g.elevation.data[idx];
            if !(20.0..200.0).contains(&e) || g.coast.cell(i / 2, j / 2) < 0.03 {
                continue;
            }
            let flat = (1..4).all(|d| {
                let k = j * n + (i + d) % n;
                (g.elevation.data[k] - e).abs() < 40.0
            });
            if flat {
                return g.geom.world_xz(i, j);
            }
        }
    }
    mountains(g)
}

/// What a map's pixel shows.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Land,
    Sea,
    Lake,
    River,
}

fn tint(h: f64, kind: Kind) -> [u8; 3] {
    match kind {
        Kind::Sea if h < -200.0 => [40, 70, 140],
        Kind::Sea => [70, 120, 190],
        Kind::Lake => [80, 150, 220],
        Kind::River => [20, 60, 230],
        // Land below the sea's level that the sea does not reach.
        Kind::Land if h < 0.0 => [150, 90, 150],
        Kind::Land => ramp(
            &[
                (0.0, [96, 140, 82]),
                (400.0, [150, 170, 100]),
                (1200.0, [170, 150, 105]),
                (2500.0, [140, 120, 100]),
                (4000.0, [200, 200, 205]),
                (6000.0, [250, 250, 255]),
            ],
            h,
        ),
    }
}

/// A shaded relief map `width` blocks across about `at`, from a sample of the surface and what
/// covers it.
fn render(
    px: usize,
    at: (f64, f64),
    width: f64,
    sample: &(dyn Fn(f64, f64) -> (f32, Kind) + Sync),
) -> Image {
    let step = width / px as f64;
    let rows: Vec<Vec<(f32, Kind)>> = (0..=px)
        .into_par_iter()
        .map(|y| {
            (0..=px)
                .map(|x| {
                    sample(
                        at.0 - width * 0.5 + x as f64 * step,
                        at.1 - width * 0.5 + y as f64 * step,
                    )
                })
                .collect()
        })
        .collect();
    let mut img = Image::new(px, px);
    for y in 0..px {
        for x in 0..px {
            let (h, kind) = rows[y][x];
            let dx = (rows[y][x + 1].0 - h) as f64 / step;
            let dz = (rows[y + 1][x].0 - h) as f64 / step;
            // Light from the north-west, a little exaggerated so plains still read.
            let nx = -dx * 2.0;
            let nz = -dz * 2.0;
            let len = (nx * nx + nz * nz + 1.0).sqrt();
            let light = ((-nx * -0.6 + -nz * -0.6 + 0.55) / len).clamp(0.0, 1.0);
            let c = tint(h as f64, kind);
            img.set(
                x,
                y,
                if kind == Kind::Land {
                    shade(c, 0.45 + 0.75 * light)
                } else {
                    c
                },
            );
        }
    }
    img
}

/// Discharge (m³/s) drawn as a river at each level's scale.
fn river_q(level: usize) -> f32 {
    match level {
        1 => 30.0,
        2 => 3.0,
        _ => 0.3,
    }
}

pub fn run(args: &[String]) -> anyhow::Result<()> {
    let a = parse(args)?;
    let g = Arc::new(cached_grid(a.seed, PlanetSize::Earth, a.res)?);
    let at = match (a.at, a.site.as_deref()) {
        (Some(at), _) => at,
        (None, Some("coast")) => coast(&g),
        (None, Some("plain")) => plain(&g),
        _ => mountains(&g),
    };
    println!("about {:.0}, {:.0}", at.0, at.1);
    let relief = Relief::new(g.clone());
    let levels = relief.levels().to_vec();
    println!(
        "levels: {}",
        levels
            .iter()
            .map(|l| format!("{:.0} m", l.cell))
            .collect::<Vec<_>>()
            .join(", ")
    );
    // Each level over a width of 200 of its cells (the grid's over 30 of its own).
    let widths: Vec<f64> = std::iter::once(g.geom.cell * 30.0)
        .chain(levels.iter().map(|l| l.cell * 200.0))
        .collect();
    if a.timing {
        // Tiles made cold along a line at each level, finest first (each pulls in the coarser
        // ones it needs): the time per tile and how many of each level it took.
        for level in (1..=levels.len()).rev() {
            let cell = levels[level - 1].cell;
            let i0 = (at.0 / cell).floor() as i64 + 64 * 40;
            let j0 = ((at.1 + g.geom.c * 0.5) / cell).floor() as i64;
            let before = relief.tiles_made();
            let t0 = Instant::now();
            for k in 0..6 {
                relief.cell(level, i0 + k * 64, j0);
            }
            let after = relief.tiles_made();
            let made: Vec<u64> = after.iter().zip(&before).map(|(a, b)| a - b).collect();
            println!(
                "level {level}: six tiles in a row in {:.0} ms ({:.1} ms each), tiles made by \
                 level {made:?}",
                t0.elapsed().as_secs_f64() * 1e3,
                t0.elapsed().as_secs_f64() * 1e3 / 6.0
            );
        }
        return Ok(());
    }
    if let Some((level, px, py)) = a.probe {
        // The cells about a pixel of a level's map: a row across it and a column down it.
        let cell = levels[level - 1].cell;
        let step = widths[level] / a.px as f64;
        let x = at.0 - widths[level] * 0.5 + px as f64 * step;
        let z = at.1 - widths[level] * 0.5 + py as f64 * step;
        let i0 = (x / cell).floor() as i64;
        let j0 = ((z + g.geom.c * 0.5) / cell).floor() as i64;
        let show = |i: i64, j: i64| {
            let c = relief.cell(level, i, j);
            println!(
                "  ({i}, {j}) tile ({}, {}) at ({}, {}): h {:.2} lake {:.2} sea {} channel {} q {:.3}",
                i.div_euclid(64),
                j.div_euclid(64),
                i.rem_euclid(64),
                j.rem_euclid(64),
                c.h,
                c.lake,
                c.sea,
                c.channel,
                c.q
            );
        };
        println!("level {level} about pixel ({px}, {py}), across:");
        for i in i0 - 4..=i0 + 4 {
            show(i, j0);
        }
        println!("down:");
        for j in j0 - 4..=j0 + 4 {
            show(i0, j);
        }
        println!("its course:");
        let (mut i, mut j) = (i0, j0);
        for _ in 0..12 {
            show(i, j);
            match relief.cell(level, i, j).receiver {
                Some(r) => (i, j) = r,
                None => break,
            }
        }
        return Ok(());
    }
    // What the levels hold about the point: the land's height, the sea, discharge, lakes.
    for level in 0..=levels.len() {
        let cell = if level == 0 {
            g.geom.cell
        } else {
            levels[level - 1].cell
        };
        let i0 = (at.0 / cell).floor() as i64;
        let j0 = ((at.1 + g.geom.c * 0.5) / cell).floor() as i64;
        let (mut qmax, mut lakes, mut wet, mut sea, mut sunk) = (0.0f32, 0, 0, 0, 0);
        // Rivers' cells whose next cell down their course stands higher (more than 0.5 m).
        let (mut rivers, mut uphill) = (0, 0);
        let mut land = Vec::new();
        // The level's mean surface and its parent's over the same ground.
        let (mut mean, mut parent_mean) = (0.0f64, 0.0f64);
        for j in j0 - 50..j0 + 50 {
            for i in i0 - 50..i0 + 50 {
                let c = relief.cell(level, i, j);
                qmax = qmax.max(c.q);
                lakes += c.lake.is_finite() as usize;
                wet += (level > 0 && c.q > river_q(level)) as usize;
                sea += c.sea as usize;
                if !c.sea {
                    land.push(c.h);
                    sunk += (c.h < 0.0) as usize;
                }
                mean += c.h as f64;
                if level > 0
                    && c.q > river_q(level)
                    && !c.sea
                    && c.lake.is_nan()
                    && let Some((ri, rj)) = c.receiver
                {
                    let r = relief.cell(level, ri, rj);
                    let surface = |c: &hearth_worldgen::relief::Cell| {
                        if c.lake.is_finite() { c.lake } else { c.h }
                    };
                    rivers += 1;
                    if !r.sea && surface(&r) > c.h + 0.5 {
                        uphill += 1;
                        if uphill <= 4 {
                            println!(
                                "  uphill at level {level} ({i}, {j}) tile ({}, {}) at ({}, {}): \
                                 {c:?} -> ({ri}, {rj}) {r:?}",
                                i.div_euclid(64),
                                j.div_euclid(64),
                                i.rem_euclid(64),
                                j.rem_euclid(64)
                            );
                        }
                    }
                }
                if level > 0 {
                    let x = (i as f64 + 0.5) * cell;
                    let z = -g.geom.c * 0.5 + (j as f64 + 0.5) * cell;
                    parent_mean += relief.height(level - 1, x, z) as f64;
                }
            }
        }
        if level > 0 {
            println!(
                "level {level}: mean surface {:.0} m, its parent's there {:.0} m; {uphill} of \
                 {rivers} river cells flow up to the next",
                mean / 10_000.0,
                parent_mean / 10_000.0
            );
        }
        land.sort_by(f32::total_cmp);
        let pct = |p: f64| {
            land.get(((land.len() as f64 - 1.0) * p) as usize)
                .copied()
                .unwrap_or(f32::NAN)
        };
        let all = 10_000.0;
        println!(
            "level {level}: {:.1}% sea; land {:.0} / {:.0} / {:.0} m (10/50/90%), {:.1}% below the \
             sea's level; max discharge {qmax:.1} m³/s; {:.1}% lakes, {:.1}% rivers",
            100.0 * sea as f64 / all,
            pct(0.1),
            pct(0.5),
            pct(0.9),
            100.0 * sunk as f64 / all,
            100.0 * lakes as f64 / all,
            100.0 * wet as f64 / all
        );
    }
    for (level, width) in widths.iter().enumerate() {
        let t0 = Instant::now();
        let cell = if level == 0 {
            g.geom.cell
        } else {
            levels[level - 1].cell
        };
        let img = render(a.px, at, *width, &|x, z| {
            let h = relief.height(level, x, z);
            let i = (x / cell).floor() as i64;
            let j = ((z + g.geom.c * 0.5) / cell).floor() as i64;
            let c = relief.cell(level, i, j);
            let kind = if c.sea {
                Kind::Sea
            } else if c.lake.is_finite() {
                Kind::Lake
            } else if level > 0 && c.q > river_q(level) {
                Kind::River
            } else {
                Kind::Land
            };
            (h, kind)
        });
        let name = format!("level{level}.png");
        img.save(&a.out.join(&name))?;
        println!(
            "level {level}: {:.0} km across in {:.1}s -> {}",
            width / 1000.0,
            t0.elapsed().as_secs_f64(),
            a.out.join(&name).display()
        );
    }
    Ok(())
}

/// `bench hypso`: an Earth grid's hypsometry against Earth's (E §5.1): the land's share, its mean
/// height and the ocean's mean depth, how much stands high, the highest and the deepest.
pub fn hypso(args: &[String]) -> anyhow::Result<()> {
    let a = parse(args)?;
    let g = cached_grid(a.seed, PlanetSize::Earth, a.res)?;
    let n = g.n();
    let (mut land, mut total, mut land_h, mut sea_d) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let (mut above2, mut above4, mut below6) = (0.0f64, 0.0f64, 0.0f64);
    let (mut top, mut deep) = (f32::MIN, f32::MAX);
    let mut lands: Vec<(f32, f64)> = Vec::new();
    for j in 0..n {
        // A cell's real area: its real side squared.
        let w = g.geom.phys_cell(j).powi(2);
        for i in 0..n {
            let e = g.elevation.data[j * n + i];
            total += w;
            if e > 0.0 {
                land += w;
                land_h += w * e as f64;
                lands.push((e, w));
            } else {
                sea_d -= w * e as f64;
            }
            if e > 2000.0 {
                above2 += w;
            }
            if e > 4000.0 {
                above4 += w;
            }
            if e < -6000.0 {
                below6 += w;
            }
            top = top.max(e);
            deep = deep.min(e);
        }
    }
    lands.sort_by(|x, y| x.0.total_cmp(&y.0));
    let pct = |p: f64| {
        let mut acc = 0.0;
        for (e, w) in &lands {
            acc += w;
            if acc >= p * land {
                return *e;
            }
        }
        f32::NAN
    };
    println!("seed {} at {}²", a.seed, a.res);
    println!("land {:.1} % (Earth 29.2)", 100.0 * land / total);
    println!(
        "mean land height {:.0} m (Earth 797-840); land 10/50/90/99 %: {:.0} / {:.0} / {:.0} / {:.0} m",
        land_h / land,
        pct(0.1),
        pct(0.5),
        pct(0.9),
        pct(0.99)
    );
    println!(
        "mean ocean depth {:.0} m (Earth 3,682)",
        sea_d / (total - land)
    );
    println!(
        "surface above 2 km {:.2} % (Earth ~3.5), above 4 km {:.2} % (Earth ~0.8), below 6 km {:.2} % (Earth ~1)",
        100.0 * above2 / total,
        100.0 * above4 / total,
        100.0 * below6 / total
    );
    println!(
        "highest cell {top:.0} m (peaks 8.8 km, 20 km cells ~6 km), deepest {deep:.0} m (trenches ~11 km)"
    );
    Ok(())
}

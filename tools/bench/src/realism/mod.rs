//! `bench realism`: the realism gap analysis's measures (Amendment T §3.3).
//!
//! `terrain` lays the generator's ground beside real ground of the same kind, at walking scale
//! (one-metre lidar, a kilometre square) and at the view from a hill (thirty-metre SRTM, some
//! thirty kilometres), with the same statistics and pictures for both. The real models are
//! fetched by `scripts/fetch-realism-refs.sh` into `bench-out/realism/refs` (not kept in the
//! repository; their sources and licences in `docs/review/realism/references.md`).

mod dem;
mod global;
mod images;
pub mod metrics;
mod photos;
mod rivers;
mod weather;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use hearth_math::PlanetSize;
use hearth_worldgen::Terrain;
use hearth_worldgen::region::biome::Biome;

use dem::{Hgt, Lidar, generated, hillshade};
use metrics::{Dem, Metrics, measure};
use rayon::prelude::*;

use crate::image::Image;

/// A kind of land, the biomes the generator makes it in and where real ground of the kind was
/// measured.
struct Kind {
    key: &'static str,
    name: &'static str,
    /// The generator's biomes for it, the likeliest first: a place is the first found of them
    /// at the heights and rain below.
    biomes: &'static [Biome],
    /// The generator's columns (at the grid's scale) that count: their height (m) and their
    /// rain (mm a year), the real place's within some hundreds of metres and millimetres.
    heights: (f32, f32),
    rain: (f32, f32),
    /// The lidar tile (in `refs/lidar`) and the SRTM tile (in `refs/srtm`) with the point at
    /// the centre of the thirty-metre window (latitude, longitude).
    lidar: &'static str,
    hgt: &'static str,
    at: (f64, f64),
}

const KINDS: &[Kind] = &[
    Kind {
        key: "hills",
        name: "forested hills (Oregon Coast Range)",
        biomes: &[Biome::TemperateRainforest],
        heights: (150.0, 900.0),
        rain: (1500.0, 3000.0),
        lidar: "USGS_1M_10_x41y481_OR_SouthCoast_2019_A19.tif",
        hgt: "N43W124.hgt",
        at: (43.45, -123.75),
    },
    Kind {
        key: "plains",
        name: "rolling plains (south-central Iowa)",
        // A tallgrass prairie's climate (Iowa's: some 850 mm, 10 °C) grows broadleaf or mixed
        // forest in the generator, which keeps prairie for drier land: the plains are found by
        // their height and rain among all three.
        biomes: &[
            Biome::TemperatePlains,
            Biome::BroadleafForest,
            Biome::MixedForest,
        ],
        heights: (100.0, 450.0),
        rain: (700.0, 1000.0),
        lidar: "USGS_1M_15_x45y454_IA_SouthCentral_2020_D20.tif",
        hgt: "N41W094.hgt",
        at: (41.3, -93.5),
    },
    Kind {
        key: "mountains",
        name: "glaciated mountains (the Teton Range; Glacier National Park from a hill)",
        // Found by height and relief instead (`mountain_site`): the generator's alpine meadows
        // lie mostly on cold low ground.
        biomes: &[Biome::AlpineMeadow],
        heights: (1500.0, 3000.0),
        rain: (400.0, 2500.0),
        // At walking scale the range's heart: the Grand Teton, its cirques and canyons (Glacier
        // National Park's tile, measured until G1b, was mostly a valley floor and a lake). From
        // a hill, Glacier National Park still (a Teton window is half Jackson Hole's floor).
        lidar: "USGS_1M_12_x51y485_WY_GrandTetonNP_D22.tif",
        hgt: "N48W114.hgt",
        at: (48.6, -113.75),
    },
    Kind {
        key: "desert",
        name: "basin and range desert (Mojave, Nevada)",
        biomes: &[Biome::HotDesert],
        heights: (300.0, 1500.0),
        rain: (50.0, 300.0),
        lidar: "USGS_1M_11_x74y407_NV_ClarkCounty_2018_C19.tif",
        hgt: "N36W115.hgt",
        at: (36.45, -114.55),
    },
    Kind {
        key: "boreal",
        name: "boreal shield with lakes (north-east Minnesota)",
        biomes: &[Biome::BorealForest],
        heights: (150.0, 700.0),
        rain: (550.0, 950.0),
        lidar: "USGS_1M_15_x61y530_MN_LakeCounty_2018_C20.tif",
        hgt: "N47W092.hgt",
        at: (47.75, -91.55),
    },
];

/// The window of walking scale (samples a side, a metre apart) and of the view from a hill
/// (thirty metres apart).
const N: usize = 1024;
const WIDE_STEP: i32 = 30;

struct Args {
    seed: u64,
    res: usize,
    planet: PlanetSize,
    refs: PathBuf,
    out: PathBuf,
    kinds: Vec<String>,
    /// Windows a side in the planet-wide comparison.
    count: usize,
    /// Only the thirty-metre windows (the view from a hill), not the walking scale's.
    wide: bool,
    /// How many windows apart the walking scale's candidates stand (`generated_windows`).
    spread: i32,
}

fn parse(args: &[String]) -> anyhow::Result<Args> {
    let mut a = Args {
        seed: 7,
        res: 2048,
        planet: PlanetSize::Earth,
        refs: PathBuf::from("bench-out/realism/refs"),
        out: PathBuf::from("bench-out/realism"),
        kinds: KINDS.iter().map(|k| k.key.to_owned()).collect(),
        count: 200,
        wide: false,
        spread: 3,
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
            "--planet" => {
                a.planet =
                    PlanetSize::from_name(&val()?).ok_or_else(|| anyhow::anyhow!("bad planet"))?
            }
            "--refs" => a.refs = PathBuf::from(val()?),
            "--out" => a.out = PathBuf::from(val()?),
            "--kinds" => a.kinds = val()?.split(',').map(str::to_owned).collect(),
            "--count" => a.count = val()?.parse()?,
            "--wide" => a.wide = true,
            "--spread" => a.spread = val()?.parse()?,
            other => anyhow::bail!("unknown option {other}"),
        }
    }
    Ok(a)
}

pub fn run(args: &[String]) -> anyhow::Result<()> {
    match args.first().map(String::as_str) {
        Some("terrain") => terrain(&parse(&args[1..])?),
        Some("global") => {
            let a = parse(&args[1..])?;
            let grid = Arc::new(crate::relief::cached_grid(a.seed, a.planet, a.res)?);
            let t = Terrain::new(grid);
            global::run(&t, a.planet.name(), &a.refs, &a.out, a.count)
        }
        Some("images") => {
            let dir = args
                .get(1)
                .ok_or_else(|| anyhow::anyhow!("bench realism images DIR"))?;
            let a = parse(&args[2..])?;
            images::run(Path::new(dir), &a.out)
        }
        Some("levels") => levels(&parse(&args[1..])?),
        Some("weather") => weather::run(),
        Some("rivers") => {
            let a = parse(&args[1..])?;
            let grid = Arc::new(crate::relief::cached_grid(a.seed, a.planet, a.res)?);
            std::fs::create_dir_all(a.out.join("terrain"))?;
            rivers::run(
                &Terrain::new(grid),
                &a.out.join("terrain").join("river.png"),
            )
        }
        // Photographs: the searches to run, then the pictures to fetch (for the fetch script),
        // and the contact sheet of the suite beside them.
        Some("photo-queries") => {
            print!("{}", photos::queries());
            Ok(())
        }
        Some("photo-pick") => {
            let a = parse(&args[1..])?;
            print!("{}", photos::pick(&a.refs)?);
            Ok(())
        }
        Some("sheet") => {
            let a = parse(&args[1..])?;
            photos::sheet(&a.out, &a.refs)
        }
        // The terrain tiles still to fetch, one URL a line (for the fetch script).
        Some("missing-tiles") => {
            let a = parse(&args[1..])?;
            for url in global::missing(&a.refs, a.count) {
                println!("{url}");
            }
            Ok(())
        }
        _ => anyhow::bail!(
            "bench realism terrain|global|levels|rivers|weather|images DIR|sheet|photo-queries|photo-pick|\
             missing-tiles \
             [--seed N] [--planet earth|standard] [--kinds a,b] [--count N] [--wide] \
             [--spread N] [--refs DIR] [--out DIR]"
        ),
    }
}

/// One measured window: where it is from, its scale, its statistics.
struct Row {
    kind: &'static str,
    source: String,
    scale: &'static str,
    m: Metrics,
}

/// Of the windows given as (grid, label), those nearly all known, sorted by relief: the median
/// and the upper quartile, so a window is typical of its land rather than its extreme.
fn typical<T>(mut windows: Vec<(f32, T)>) -> Vec<T> {
    windows.sort_by(|a, b| a.0.total_cmp(&b.0));
    let k = windows.len();
    if k == 0 {
        return Vec::new();
    }
    let (mid, upper) = (k / 2, (k * 3 / 4).min(k - 1));
    let mut out = Vec::new();
    for (i, (_, w)) in windows.into_iter().enumerate() {
        if i == mid || (i == upper && upper != mid) {
            out.push(w);
        }
    }
    out
}

/// The generator's windows of walking scale in a kind's land: of a lattice of nine by nine of
/// them `spread` windows apart about its place (`--spread`; 3, some 24 km across, as a lidar
/// tile's windows stand for its 10 km of land rather than one spot), the median and
/// upper-quartile reliefs (read first at 16 m). The reliefs found are printed.
fn generated_windows(t: &Terrain, x: i32, z: i32, spread: i32, key: &str) -> Vec<(i32, i32)> {
    let mut cands = Vec::new();
    let side = if spread <= 1 { 2 } else { 4 };
    for b in -side..=side {
        for a in -side..=side {
            let (cx, cz) = (x + a * spread * N as i32, z + b * spread * N as i32);
            let rough = generated(t, cx, cz, N / 16, 16);
            if rough.valid() > 0.99 {
                cands.push((rough.relief(), (cx, cz)));
            }
        }
    }
    let mut reliefs: Vec<f32> = cands.iter().map(|c| c.0).collect();
    reliefs.sort_by(f32::total_cmp);
    let q = |p: f64| reliefs.get(((reliefs.len() as f64 - 1.0) * p).round() as usize);
    println!(
        "{key}: {} windows of 1 km over {} km, relief at 16 m p10 · p50 · p75 · p90 {:.0?} · \
         {:.0?} · {:.0?} · {:.0?} m",
        reliefs.len(),
        (2 * side + 1) * spread,
        q(0.1),
        q(0.5),
        q(0.75),
        q(0.9)
    );
    typical(cands)
}

/// A lidar tile's windows: of those a kilometre apart and nearly all known, the median and
/// upper-quartile reliefs.
fn lidar_windows(l: &Lidar, key: &str) -> Vec<(usize, usize)> {
    let mut cands = Vec::new();
    for y in (0..l.h.saturating_sub(N)).step_by(N) {
        for x in (0..l.w.saturating_sub(N)).step_by(N) {
            let w = l.window(x, y, N).coarsen(16);
            if w.valid() > 0.99 {
                cands.push((w.relief(), (x, y)));
            }
        }
    }
    let mut reliefs: Vec<f32> = cands.iter().map(|c| c.0).collect();
    reliefs.sort_by(f32::total_cmp);
    let q = |p: f64| reliefs.get(((reliefs.len() as f64 - 1.0) * p).round() as usize);
    println!(
        "{key}: {} lidar windows of 1 km, relief at 16 m p10 · p50 · p75 · p90 {:.0?} · {:.0?} · \
         {:.0?} · {:.0?} m",
        reliefs.len(),
        q(0.1),
        q(0.5),
        q(0.75),
        q(0.9)
    );
    typical(cands)
}

/// A mountain range's flank: of the grid's cells between the heights given and 35° to 55° from
/// the equator, away from volcanoes (a cone is not a range), the one with the most relief among
/// its neighbours (the land within two cells).
fn mountain_site(t: &Terrain, lo: f32, hi: f32) -> Option<(i32, i32)> {
    let g = &*t.grid;
    let n = g.n();
    let mut best: Option<(f32, usize, usize)> = None;
    for j in 2..n - 2 {
        let lat = g.geom.lat[j].to_degrees().abs();
        if !(35.0..55.0).contains(&lat) {
            continue;
        }
        for i in 2..n - 2 {
            let e = g.elevation.data[g.geom.idx(i, j)];
            if !(lo..hi).contains(&e) {
                continue;
            }
            let (x, z) = g.geom.world_xz(i, j);
            let volcano = g.volcanoes.iter().any(|v| {
                let dx = (v.x - x).abs().min(g.geom.c - (v.x - x).abs());
                dx.hypot(v.z - z) < 60_000.0
            });
            if volcano {
                continue;
            }
            let (mut a, mut b) = (f32::MAX, f32::MIN);
            for dj in 0..5 {
                for di in 0..5 {
                    let v = g.elevation.data[g.geom.idx(i + di - 2, j + dj - 2)];
                    a = a.min(v);
                    b = b.max(v);
                }
            }
            if best.is_none_or(|(r, _, _)| b - a > r) {
                best = Some((b - a, i, j));
            }
        }
    }
    best.map(|(_, i, j)| {
        let (x, z) = g.geom.world_xz(i, j);
        (x as i32, z as i32)
    })
}

/// A grid shaded, its closed depressions deeper than 0.25 m tinted red.
fn shaded(d: &Dem) -> Image {
    let mut img = hillshade(d);
    if d.dx <= 1.0 {
        for (k, depth) in crate::realism::metrics::hollow_depths(d).iter().enumerate() {
            if *depth > 0.25 {
                let [r, g, b] = img.px[k];
                img.px[k] = [r.saturating_add(90), g / 2, b / 2];
            }
        }
    }
    img
}

fn save_pair(path: &Path, real: Option<&Dem>, generated: &Dem) -> anyhow::Result<()> {
    let g = shaded(generated);
    let Some(real) = real else {
        return g.save(path);
    };
    let r = shaded(real);
    let gap = 8;
    let mut img = Image::new(r.w + gap + g.w, r.h.max(g.h));
    for y in 0..img.h {
        for x in 0..img.w {
            img.set(x, y, [255, 255, 255]);
        }
    }
    for y in 0..r.h {
        for x in 0..r.w {
            img.set(x, y, r.px[y * r.w + x]);
        }
    }
    for y in 0..g.h {
        for x in 0..g.w {
            img.set(r.w + gap + x, y, g.px[y * g.w + x]);
        }
    }
    img.save(path)
}

fn terrain(a: &Args) -> anyhow::Result<()> {
    let t0 = Instant::now();
    let grid = Arc::new(crate::relief::cached_grid(a.seed, a.planet, a.res)?);
    let t = Terrain::new(grid);
    let planet = a.planet.name();
    println!(
        "{planet} planet, seed {}, ready in {:.1} s",
        a.seed,
        t0.elapsed().as_secs_f64()
    );
    let pics = a.out.join("terrain");
    std::fs::create_dir_all(&pics)?;
    let mut rows: Vec<Row> = Vec::new();
    for kind in KINDS.iter().filter(|k| a.kinds.iter().any(|s| s == k.key)) {
        let t1 = Instant::now();
        // The kinds' heights in blocks (a small planet's blocks stand for more than a metre).
        let found = site_of(&t, kind);
        let Some((x, z)) = found else {
            println!("{}: no {:?}", kind.key, kind.biomes);
            continue;
        };
        let s = t.sample(x, z);
        // The relief the generator gives the place (its height above base level, the
        // roughness's amplitude and valley spacing).
        let rough = t.relief().map_or(String::new(), |r| {
            let (gx, gz) = t.grid.geom.grid_coords(x as f64, z as f64);
            let k = r.roughness_at(gx, gz);
            let g = &*t.grid;
            format!(
                "; {:.0} m above base level, relief {:.0} m at {:.0} m; {:.0} mm, {:.1} °C, \
                 province {}, uplift {:.0} m{}",
                r.above_base(gx, gz),
                k.amp,
                k.spacing,
                g.precipitation.bilinear(gx, gz),
                g.temperature.bilinear(gx, gz),
                g.province[g.cell_at(x as f64, z as f64)],
                g.uplift.bilinear(gx, gz),
                if g.flags[g.cell_at(x as f64, z as f64)]
                    & hearth_worldgen::planet::flags::ENDORHEIC
                    != 0
                {
                    ", a closed basin"
                } else {
                    ""
                }
            )
        });
        println!(
            "{} — {}: {:?} at {x}, {z} (height {:.0} m{rough}) found in {:.1} s",
            kind.key,
            kind.name,
            s.biome,
            s.height,
            t1.elapsed().as_secs_f64()
        );

        // Walking scale.
        let lidar = (!a.wide)
            .then(|| Lidar::load(&a.refs.join("lidar").join(kind.lidar)).ok())
            .flatten();
        let real: Vec<Dem> = lidar.as_ref().map_or(Vec::new(), |l| {
            lidar_windows(l, kind.key)
                .into_iter()
                .map(|(x, y)| l.window(x, y, N))
                .collect()
        });
        let places = if a.wide {
            Vec::new()
        } else {
            generated_windows(&t, x, z, a.spread, kind.key)
        };
        let gens: Vec<Dem> = places
            .iter()
            .map(|&(x, z)| generated(&t, x, z, N, 1))
            .collect();
        for (i, g) in gens.iter().enumerate() {
            let r = real.get(i);
            save_pair(
                &pics.join(format!("{}_1m_{i}_{planet}.png", kind.key)),
                r,
                g,
            )?;
            if let Some(r) = r {
                rows.push(Row {
                    kind: kind.key,
                    source: format!("real {}", i + 1),
                    scale: "1 m",
                    m: measure(r),
                });
                rows.push(Row {
                    kind: kind.key,
                    source: format!("real {}", i + 1),
                    scale: "4 m",
                    m: measure(&r.coarsen(4)),
                });
            }
            rows.push(Row {
                kind: kind.key,
                source: format!("{planet} {}", i + 1),
                scale: "1 m",
                m: measure(g),
            });
            rows.push(Row {
                kind: kind.key,
                source: format!("{planet} {}", i + 1),
                scale: "4 m",
                m: measure(&g.coarsen(4)),
            });
        }

        // The view from a hill.
        let hgt = Hgt::load(&a.refs.join("srtm").join(kind.hgt)).ok();
        let real_wide = hgt.map(|h| h.window(kind.at.0, kind.at.1, N, WIDE_STEP as f64));
        let gen_wide = generated(&t, x, z, N, WIDE_STEP);
        save_pair(
            &pics.join(format!("{}_30m_{planet}.png", kind.key)),
            real_wide.as_ref(),
            &gen_wide,
        )?;
        if let Some(r) = &real_wide {
            rows.push(Row {
                kind: kind.key,
                source: "real".into(),
                scale: "30 m",
                m: measure(r),
            });
        }
        rows.push(Row {
            kind: kind.key,
            source: planet.into(),
            scale: "30 m",
            m: measure(&gen_wide),
        });
        println!(
            "{}: measured in {:.1} s",
            kind.key,
            t1.elapsed().as_secs_f64()
        );
    }
    let md = table(&rows);
    println!("{md}");
    std::fs::write(a.out.join(format!("terrain_{planet}.md")), md)?;
    std::fs::write(a.out.join(format!("terrain_{planet}.tsv")), tsv(&rows))?;
    Ok(())
}

/// Where a kind's land is on the generated planet (as `terrain` finds it).
fn site_of(t: &Terrain, kind: &Kind) -> Option<(i32, i32)> {
    let v = t.vertical_scale();
    let (lo, hi) = (kind.heights.0 * v, kind.heights.1 * v);
    if kind.key == "mountains"
        && let Some(p) = mountain_site(t, kind.heights.0, kind.heights.1)
    {
        return Some(p);
    }
    let rains = kind.rain.0..kind.rain.1;
    let matched = |s: &hearth_worldgen::ColumnSample| {
        (lo..hi).contains(&s.height) && rains.contains(&s.precipitation)
    };
    let first = |also: &(dyn Fn(&hearth_worldgen::ColumnSample) -> bool + Sync)| {
        kind.biomes
            .iter()
            .find_map(|&b| t.find_biome(b, 20_000.0, also))
    };
    first(&matched)
        .or_else(|| {
            println!(
                "{}: no {:?} between {lo} and {hi} m with {:?} mm; at those heights",
                kind.key, kind.biomes, kind.rain
            );
            first(&|s| (lo..hi).contains(&s.height))
        })
        .or_else(|| {
            println!(
                "{}: no {:?} between {lo} and {hi} m; its heart at any height",
                kind.key, kind.biomes
            );
            first(&|_| true)
        })
}

/// What each refinement level and the blocks' own noise give the ground (T §3.3, the
/// fine-detail regression): at each kind's place, a kilometre at 2 m and thirty kilometres at
/// 30 m read from each level's surface alone (bicubic between its cells) and from the whole.
fn levels(a: &Args) -> anyhow::Result<()> {
    let grid = Arc::new(crate::relief::cached_grid(a.seed, a.planet, a.res)?);
    let t = Terrain::new(grid);
    let finest = t.finest_level();
    let mut rows = Vec::new();
    for kind in KINDS.iter().filter(|k| a.kinds.iter().any(|s| s == k.key)) {
        let Some((x, z)) = site_of(&t, kind) else {
            continue;
        };
        for (n, step, scale) in [(512usize, 2i32, "2 m"), (1024, 30, "30 m")] {
            // A level far finer than the columns' spacing shows nothing between them (and
            // would make tiles over all thirty kilometres).
            for level in (0..=finest).filter(|&l| t.level_cell(l) * 4.0 >= step as f64) {
                let h: Vec<f32> = (0..n * n)
                    .into_par_iter()
                    .map(|k| {
                        let (i, j) = ((k % n) as i32, (k / n) as i32);
                        t.level_height(
                            level,
                            (x + (i - n as i32 / 2) * step) as f64,
                            (z + (j - n as i32 / 2) * step) as f64,
                        )
                    })
                    .collect();
                rows.push(Row {
                    kind: kind.key,
                    source: format!("level {level} ({:.0} m cells)", t.level_cell(level)),
                    scale,
                    m: measure(&Dem {
                        n,
                        dx: step as f64,
                        h,
                    }),
                });
            }
            rows.push(Row {
                kind: kind.key,
                source: "the blocks (all levels and noise)".into(),
                scale,
                m: measure(&generated(&t, x, z, n, step)),
            });
        }
        println!("{}: measured", kind.key);
    }
    let md = table(&rows);
    println!("{md}");
    std::fs::create_dir_all(&a.out)?;
    std::fs::write(a.out.join(format!("levels_{}.md", a.planet.name())), md)?;
    Ok(())
}

fn table(rows: &[Row]) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "| Land | Source | Grid | Slope p50 / p90 (°) | >30° | Curvature p5 / p95 (1/m), skew | \
         |Δh| at 1·4·16·64 cells (m) | Pits /km² | In hollows >0.25 m · >1 m | Drainage (km/km², \
         10⁴ · 10⁵ m²) | θ | Turn (m²) | Sinuosity | HI | Relief (m) |"
    );
    let _ = writeln!(
        s,
        "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"
    );
    for r in rows {
        let m = &r.m;
        let lag = |k: f64| m.rough_at(k * m.dx);
        let _ = writeln!(
            s,
            "| {} | {} | {} | {:.1} / {:.1} | {:.1}% | {:.4} / {:.4}, {:.2} | {:.2} · {:.2} · {:.2} · \
             {:.2} | {:.0} | {:.1}% · {:.1}% | {:.1} · {:.2} | {:.2} | {:.0} | {:.2} | {:.2} | \
             {:.0} |",
            r.kind,
            r.source,
            r.scale,
            m.slope[1],
            m.slope[2],
            100.0 * m.steep30,
            m.curv[0],
            m.curv[2],
            m.curv_skew,
            lag(1.0),
            lag(4.0),
            lag(16.0),
            lag(64.0),
            m.pits_per_km2,
            100.0 * m.depressed[0],
            100.0 * m.depressed[1],
            m.drainage[0],
            m.drainage[1],
            m.concavity,
            m.turn_area,
            m.sinuosity,
            m.hypsometric,
            m.relief
        );
    }
    s
}

fn tsv(rows: &[Row]) -> String {
    let mut s = String::from(
        "kind\tsource\tgrid\tdx\tn\tslope10\tslope50\tslope90\tslope99\tsteep30\tsteep45\tflat2\t\
         curv5\tcurv50\tcurv95\tcurv_skew\tpits_km2\tdepressed25\tdepressed100\tdd4\tdd5\t\
         concavity\tturn_area\tsinuosity\thypsometric\trelief\tstructure\thurst\tslope_area\n",
    );
    for r in rows {
        let m = &r.m;
        let _ = writeln!(
            s,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            r.kind,
            r.source,
            r.scale,
            m.dx,
            m.n,
            m.slope[0],
            m.slope[1],
            m.slope[2],
            m.slope[3],
            m.steep30,
            m.steep45,
            m.flat2,
            m.curv[0],
            m.curv[1],
            m.curv[2],
            m.curv_skew,
            m.pits_per_km2,
            m.depressed[0],
            m.depressed[1],
            m.drainage[0],
            m.drainage[1],
            m.concavity,
            m.turn_area,
            m.sinuosity,
            m.hypsometric,
            m.relief,
            m.structure
                .iter()
                .map(|(l, v)| format!("{l}:{v:.4}"))
                .collect::<Vec<_>>()
                .join(","),
            m.hurst()
                .iter()
                .map(|(l, h)| format!("{l}:{h:.3}"))
                .collect::<Vec<_>>()
                .join(","),
            m.slope_area
                .iter()
                .map(|(a, v, c)| format!("{a:.2}:{v:.4}:{c}"))
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    s
}

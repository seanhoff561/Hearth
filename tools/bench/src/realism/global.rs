//! `bench realism global`: the land's slopes and relief over the whole planet, the generator's
//! beside the Earth's — windows of some ten kilometres at random over the land between 60° S
//! and 60° N, as many of each, chosen by area as on a sphere.
//!
//! The Earth's windows are the AWS terrain tiles' "terrarium" PNGs at zoom 12 (256 pixels of
//! 38 m × cos(latitude), from SRTM and national models: public domain and openly licensed,
//! `docs/review/realism/references.md`); the land is told from the sea by the zoom-3 tiles.
//! Missing tiles are listed for `scripts/fetch-realism-refs.sh` to fetch, then the run is
//! repeated.

use std::path::{Path, PathBuf};

use hearth_math::hash::Rng;
use hearth_worldgen::Terrain;
use hearth_worldgen::planet::flags;
use rayon::prelude::*;

use super::dem::{generated, hillshade};
use super::metrics::{Dem, Metrics, measure};
use crate::image::Image;

/// Pixels a side of a terrarium tile.
const TILE: usize = 256;
/// The zoom of the windows and of the land mask.
const ZOOM: u32 = 12;
const MASK_ZOOM: u32 = 3;
/// Metres a pixel at the equator at zoom 0.
const EQUATOR_PX: f64 = 40_075_016.686 / 256.0;
const MAX_LAT: f64 = 60.0;

fn tile_path(refs: &Path, z: u32, x: u32, y: u32) -> PathBuf {
    refs.join("terrarium")
        .join(z.to_string())
        .join(x.to_string())
        .join(format!("{y}.png"))
}

fn tile_url(z: u32, x: u32, y: u32) -> String {
    format!("https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{z}/{x}/{y}.png")
}

/// A terrarium tile's heights (m): (R × 256 + G + B / 256) − 32768.
fn read_tile(path: &Path) -> anyhow::Result<Vec<f32>> {
    let dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
    let mut r = dec.read_info()?;
    let mut buf = vec![0; r.output_buffer_size().unwrap_or(TILE * TILE * 4)];
    let info = r.next_frame(&mut buf)?;
    let ch = info.color_type.samples();
    anyhow::ensure!(info.width as usize == TILE && ch >= 3, "{}", path.display());
    Ok(buf[..TILE * TILE * ch]
        .chunks_exact(ch)
        .map(|p| (p[0] as f32 * 256.0 + p[1] as f32 + p[2] as f32 / 256.0) - 32768.0)
        .collect())
}

/// The tile at a zoom holding a latitude and longitude, and the pixel within it.
fn tile_of(z: u32, lat: f64, lon: f64) -> (u32, u32, usize, usize) {
    let n = (1u64 << z) as f64;
    let x = (lon + 180.0) / 360.0 * n;
    let l = lat.to_radians();
    let y = (1.0 - (l.tan() + 1.0 / l.cos()).ln() / std::f64::consts::PI) / 2.0 * n;
    let (tx, ty) = (x.floor(), y.floor());
    (
        tx as u32,
        ty as u32,
        ((x - tx) * TILE as f64) as usize,
        ((y - ty) * TILE as f64) as usize,
    )
}

/// The Earth's land at the mask's zoom: true where a pixel and its eight neighbours stand above
/// the sea.
struct Mask {
    side: usize,
    land: Vec<bool>,
}

impl Mask {
    fn load(refs: &Path, missing: &mut Vec<String>) -> Option<Mask> {
        let tiles = 1u32 << MASK_ZOOM;
        let side = tiles as usize * TILE;
        let mut h = vec![0.0f32; side * side];
        let mut ok = true;
        for ty in 0..tiles {
            for tx in 0..tiles {
                let p = tile_path(refs, MASK_ZOOM, tx, ty);
                match read_tile(&p) {
                    Ok(t) => {
                        for j in 0..TILE {
                            for i in 0..TILE {
                                h[(ty as usize * TILE + j) * side + tx as usize * TILE + i] =
                                    t[j * TILE + i];
                            }
                        }
                    }
                    Err(_) => {
                        missing.push(tile_url(MASK_ZOOM, tx, ty));
                        ok = false;
                    }
                }
            }
        }
        if !ok {
            return None;
        }
        let mut land = vec![false; side * side];
        for j in 1..side - 1 {
            for i in 0..side {
                land[j * side + i] = (-1i64..=1).all(|b| {
                    (-1i64..=1).all(|a| {
                        let x = (i as i64 + a).rem_euclid(side as i64) as usize;
                        h[(j as i64 + b) as usize * side + x] > 0.0
                    })
                });
            }
        }
        Some(Mask { side, land })
    }

    fn is_land(&self, lat: f64, lon: f64) -> bool {
        let (tx, ty, px, py) = tile_of(MASK_ZOOM, lat, lon);
        self.land[(ty as usize * TILE + py) * self.side + tx as usize * TILE + px]
    }
}

/// A point at random on the sphere between the latitudes ±`MAX_LAT`.
fn on_sphere(rng: &mut Rng) -> (f64, f64) {
    let s = MAX_LAT.to_radians().sin();
    let lat = rng.range_f64(-s, s).asin().to_degrees();
    (lat, rng.range_f64(-180.0, 180.0))
}

/// The Earth's windows: the zoom-12 tiles at `count` points of land, or the tiles to fetch.
fn earth_windows(refs: &Path, count: usize, missing: &mut Vec<String>) -> Vec<(f64, Dem)> {
    let Some(mask) = Mask::load(refs, missing) else {
        return Vec::new();
    };
    let mut rng = Rng::new(0x0072_6561_6c69_736d);
    let mut points = Vec::new();
    while points.len() < count {
        let (lat, lon) = on_sphere(&mut rng);
        if mask.is_land(lat, lon) {
            points.push((lat, lon));
        }
    }
    let mut out = Vec::new();
    for (lat, lon) in points {
        let (tx, ty, _, _) = tile_of(ZOOM, lat, lon);
        let p = tile_path(refs, ZOOM, tx, ty);
        match read_tile(&p) {
            Ok(h) => {
                let h = h
                    .into_iter()
                    .map(|v| if v > 0.0 { v } else { f32::NAN })
                    .collect();
                let dx = EQUATOR_PX / (1u64 << ZOOM) as f64 * lat.to_radians().cos();
                out.push((lat, Dem { n: TILE, dx, h }));
            }
            Err(_) => missing.push(tile_url(ZOOM, tx, ty)),
        }
    }
    out
}

/// The generator's windows: as many points of its land, by area as on a sphere, each sampled
/// over the same ground as an Earth tile at its latitude.
fn generated_windows(t: &Terrain, count: usize) -> Vec<(f64, Dem)> {
    let g = &*t.grid;
    let n = g.n();
    // Rows by their share of the sphere's area.
    let weight: Vec<f64> = (0..n)
        .map(|j| {
            let lat = g.geom.lat[j];
            if lat.to_degrees().abs() > MAX_LAT {
                return 0.0;
            }
            let dlat = (g.geom.lat[(j + 1).min(n - 1)] - g.geom.lat[j.saturating_sub(1)]).abs();
            lat.cos() * dlat
        })
        .collect();
    let total: f64 = weight.iter().sum();
    let mut rng = Rng::new(0x6765_6e65_7261_7465);
    let land = |i: usize, j: usize| {
        (-1i64..=1).all(|b| {
            (-1i64..=1).all(|a| {
                let ii = (i as i64 + a).rem_euclid(n as i64) as usize;
                let jj = (j as i64 + b).clamp(0, n as i64 - 1) as usize;
                let k = g.geom.idx(ii, jj);
                g.elevation.data[k] > 0.0 && g.flags[k] & flags::OCEAN == 0
            })
        })
    };
    let mut points = Vec::new();
    while points.len() < count {
        let mut r = rng.next_f64() * total;
        let mut j = 0;
        while j + 1 < n && r > weight[j] {
            r -= weight[j];
            j += 1;
        }
        let i = rng.below(n as u32) as usize;
        if !land(i, j) {
            continue;
        }
        let x = (i as f64 + rng.next_f64()) * g.geom.cell;
        let z = -g.geom.c * 0.5 + (j as f64 + rng.next_f64()) * g.geom.cell;
        points.push((g.geom.lat[j].to_degrees(), x as i32, z as i32));
    }
    // Columns as far apart as an Earth tile's pixels at the same latitude.
    points
        .into_iter()
        .map(|(lat, x, z)| {
            let step = (EQUATOR_PX / (1u64 << ZOOM) as f64 * lat.to_radians().cos()).round();
            (lat, generated(t, x, z, TILE, step.max(8.0) as i32))
        })
        .collect()
}

/// Percentiles of a statistic over windows.
fn spread(v: &mut [f32]) -> [f32; 5] {
    v.sort_by(f32::total_cmp);
    [0.1, 0.25, 0.5, 0.75, 0.9].map(|p| {
        if v.is_empty() {
            f32::NAN
        } else {
            v[((v.len() - 1) as f64 * p).round() as usize]
        }
    })
}

/// The pooled share of the land steeper than each of a few slopes.
fn steeper(ms: &[Metrics]) -> [f32; 4] {
    let mut hist = vec![0u64; 90];
    for m in ms {
        for (h, c) in hist.iter_mut().zip(&m.slope_hist) {
            *h += *c as u64;
        }
    }
    let all: u64 = hist.iter().sum::<u64>().max(1);
    [5, 10, 20, 30].map(|d| hist[d..].iter().sum::<u64>() as f32 / all as f32)
}

/// A sheet of the first sixteen windows' relief, four by four.
fn sheet(windows: &[(f64, Dem)], path: &Path) -> anyhow::Result<()> {
    let k = 4;
    let gap = 4;
    let side = k * TILE + (k - 1) * gap;
    let mut img = Image::new(side, side);
    for p in img.px.iter_mut() {
        *p = [255, 255, 255];
    }
    for (w, (_, d)) in windows.iter().take(k * k).enumerate() {
        let h = hillshade(d);
        let (ox, oy) = ((w % k) * (TILE + gap), (w / k) * (TILE + gap));
        for y in 0..TILE {
            for x in 0..TILE {
                img.set(ox + x, oy + y, h.px[y * TILE + x]);
            }
        }
    }
    img.save(path)
}

/// The terrain tiles the comparison still needs, one URL a line (empty when it has them all).
pub fn missing(refs: &Path, count: usize) -> Vec<String> {
    let mut missing = Vec::new();
    earth_windows(refs, count, &mut missing);
    missing
}

pub fn run(t: &Terrain, planet: &str, refs: &Path, out: &Path, count: usize) -> anyhow::Result<()> {
    let mut missing = Vec::new();
    let earth = earth_windows(refs, count, &mut missing);
    if !missing.is_empty() {
        println!(
            "{} terrain tiles to fetch first: scripts/fetch-realism-refs.sh --dem",
            missing.len()
        );
        return Ok(());
    }
    let gens = generated_windows(t, count);
    let generated_name = format!("generated ({planet})");
    let sets: [(&str, &str, &[(f64, Dem)]); 2] = [
        ("real Earth", "real", &earth),
        (&generated_name, planet, &gens),
    ];
    let mut md = String::from(
        "| Land | Windows | Median slope p10 · p25 · p50 · p75 · p90 (°) | Relief p10 · p50 · p90 \
         (m) | In hollows >1 m, p50 · p90 | Land steeper than 5° · 10° · 20° · 30° |\n\
         |---|---|---|---|---|---|\n",
    );
    // Relief and slope by the windows' mean height: how relief grows with height above the sea
    // (the land's own measure of how far erosion has cut into it).
    let bands = [0.0, 200.0, 500.0, 1000.0, 2000.0, f64::MAX];
    let mut by_height = String::from(
        "\n| Land | 0–200 m | 200–500 m | 500–1000 m | 1–2 km | above 2 km |\n|---|---|---|---|---|---|\n",
    );
    for (name, file, set) in sets {
        let ms: Vec<Metrics> = set.par_iter().map(|(_, d)| measure(d)).collect();
        let means: Vec<f64> = set
            .iter()
            .map(|(_, d)| {
                let known: Vec<f64> =
                    d.h.iter()
                        .filter(|h| h.is_finite())
                        .map(|&h| h as f64)
                        .collect();
                known.iter().sum::<f64>() / known.len().max(1) as f64
            })
            .collect();
        by_height += &format!("| {name} |");
        for b in bands.windows(2) {
            let mut relief: Vec<f32> = Vec::new();
            let mut slope: Vec<f32> = Vec::new();
            for (m, &h) in ms.iter().zip(&means) {
                if (b[0]..b[1]).contains(&h) {
                    relief.push(m.relief);
                    slope.push(m.slope[1]);
                }
            }
            let n = relief.len();
            if n == 0 {
                by_height += " — |";
            } else {
                let (r, sl) = (spread(&mut relief)[2], spread(&mut slope)[2]);
                by_height += &format!(" {r:.0} m, {sl:.1}° ({n}) |");
            }
        }
        by_height += "\n";
        let mut med: Vec<f32> = ms.iter().map(|m| m.slope[1]).collect();
        let mut relief: Vec<f32> = ms.iter().map(|m| m.relief).collect();
        let mut hollows: Vec<f32> = ms.iter().map(|m| m.depressed[1]).collect();
        let s = spread(&mut med);
        let r = spread(&mut relief);
        let h = spread(&mut hollows);
        let st = steeper(&ms);
        md += &format!(
            "| {name} | {} | {:.1} · {:.1} · {:.1} · {:.1} · {:.1} | {:.0} · {:.0} · {:.0} | \
             {:.1}% · {:.1}% | {:.0}% · {:.0}% · {:.0}% · {:.1}% |\n",
            set.len(),
            s[0],
            s[1],
            s[2],
            s[3],
            s[4],
            r[0],
            r[2],
            r[4],
            100.0 * h[2],
            100.0 * h[4],
            100.0 * st[0],
            100.0 * st[1],
            100.0 * st[2],
            100.0 * st[3]
        );
        sheet(set, &out.join("terrain").join(format!("global_{file}.png")))?;
    }
    md += &by_height;
    md +=
        "\n(the median relief and median slope of the windows of each mean height, and how many)\n";
    println!("{md}");
    std::fs::write(out.join(format!("global_{planet}.md")), md)?;
    Ok(())
}

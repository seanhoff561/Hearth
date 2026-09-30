//! `bench worldmap`: renders whole-planet maps (Mercator and equirectangular) of the planet
//! model — relief, climate, biomes, geological provinces and the rock at the surface — plus
//! side-profile slices, block-scale geological cross-sections, and summary statistics.

use std::path::PathBuf;
use std::time::Instant;

use hearth_math::PlanetSize;
use hearth_worldgen::planet::climate::ClimateClass;
use hearth_worldgen::planet::{PlanetGrid, flags};
use hearth_worldgen::{FeatureRarity, WorldGenSettings};

use crate::image::{Image, ramp, shade};

pub struct Args {
    pub seed: u64,
    pub planet: PlanetSize,
    pub res: usize,
    pub out: PathBuf,
    pub width: usize,
    pub slices: Vec<Slice>,
    /// Block-scale geology of an area: (x, z, size in blocks) — a top-down map of the rock at
    /// the surface and an east–west cross-section through its middle.
    pub geo_areas: Vec<(i32, i32, i32)>,
    pub rarity: FeatureRarity,
}

#[derive(Debug, Clone, Copy)]
pub enum Slice {
    Latitude(f64),
    Longitude(f64),
}

pub fn parse(args: &[String]) -> anyhow::Result<Args> {
    let mut a = Args {
        seed: 1,
        planet: PlanetSize::Standard,
        res: 1024,
        out: PathBuf::from("bench-out/worldmap"),
        width: 2048,
        slices: Vec::new(),
        geo_areas: Vec::new(),
        rarity: FeatureRarity::Rare,
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
            "--planet" => {
                let v = val()?;
                a.planet = PlanetSize::from_name(&v)
                    .ok_or_else(|| anyhow::anyhow!("unknown planet size {v}"))?
            }
            "--res" => a.res = val()?.parse()?,
            "--out" => a.out = PathBuf::from(val()?),
            "--width" => a.width = val()?.parse()?,
            "--rarity" => {
                a.rarity = match val()?.as_str() {
                    "rare" => FeatureRarity::Rare,
                    "standard" => FeatureRarity::Standard,
                    "common" => FeatureRarity::Common,
                    other => anyhow::bail!("unknown rarity {other}"),
                }
            }
            "--slice" => {
                let v = val()?;
                let (k, num) = v
                    .split_once('=')
                    .ok_or_else(|| anyhow::anyhow!("--slice lat=45 or lon=-30"))?;
                let num: f64 = num.parse()?;
                a.slices.push(match k {
                    "lat" => Slice::Latitude(num),
                    "lon" => Slice::Longitude(num),
                    _ => anyhow::bail!("--slice lat=.. or lon=.."),
                });
            }
            "--geo-area" => {
                let v = val()?;
                let nums: Vec<i32> = v
                    .split(',')
                    .map(|p| p.trim().parse::<i32>())
                    .collect::<Result<_, _>>()?;
                match nums.as_slice() {
                    [x, z] => a.geo_areas.push((*x, *z, 1024)),
                    [x, z, size] => a.geo_areas.push((*x, *z, *size)),
                    _ => anyhow::bail!("--geo-area x,z[,size]"),
                }
            }
            other => anyhow::bail!("unknown argument {other}"),
        }
    }
    Ok(a)
}

fn hypsometric(e: f32) -> [u8; 3] {
    let e = e as f64;
    if e <= 0.0 {
        ramp(
            &[
                (-11000.0, [5, 5, 40]),
                (-6000.0, [10, 20, 70]),
                (-4000.0, [20, 45, 110]),
                (-2500.0, [35, 75, 150]),
                (-600.0, [50, 110, 180]),
                (-150.0, [80, 160, 210]),
                (0.0, [140, 205, 230]),
            ],
            e,
        )
    } else {
        ramp(
            &[
                (0.0, [70, 130, 60]),
                (300.0, [110, 160, 80]),
                (800.0, [190, 185, 110]),
                (1800.0, [170, 130, 80]),
                (3200.0, [140, 110, 90]),
                (4500.0, [200, 200, 200]),
                (6500.0, [255, 255, 255]),
            ],
            e,
        )
    }
}

fn hillshade(g: &PlanetGrid, idx: usize) -> f64 {
    let n = g.n();
    let (i, j) = g.geom.ij(idx);
    let v = g.vertical_scale as f32;
    let e = |di: isize, dj: isize| -> f32 {
        g.geom
            .neighbor(i, j, di, dj)
            .map_or(g.elevation.data[idx], |nb| g.elevation.data[nb])
            .max(0.0)
            * v
    };
    let cell = g.geom.phys_cell(j) as f32;
    let dx = (e(1, 0) - e(-1, 0)) / (2.0 * cell);
    let dz = (e(0, 1) - e(0, -1)) / (2.0 * cell);
    // Light from the north-west, 45° up.
    let nrm = glam_normal(-dx as f64 * 3.0, -dz as f64 * 3.0);
    let l = [-0.5f64, 0.707, -0.5];
    let d = nrm[0] * l[0] + nrm[1] * l[1] + nrm[2] * l[2];
    let _ = n;
    (0.55 + 0.6 * d).clamp(0.35, 1.25)
}

fn glam_normal(dx: f64, dz: f64) -> [f64; 3] {
    let len = (dx * dx + 1.0 + dz * dz).sqrt();
    [dx / len, 1.0 / len, dz / len]
}

fn relief_color(g: &PlanetGrid, idx: usize) -> [u8; 3] {
    let f = g.flags[idx];
    let e = g.elevation.data[idx];
    if f & flags::LAKE != 0 {
        return if f & flags::ENDORHEIC != 0 {
            [120, 190, 190]
        } else {
            [60, 120, 200]
        };
    }
    if f & flags::SALT_FLAT != 0 {
        return [235, 230, 215];
    }
    if f & flags::RIVER != 0 {
        let q = g.discharge_at(idx);
        let t = ((q / hearth_worldgen::planet::RIVER_MIN_DISCHARGE).ln() / 6.0).clamp(0.0, 1.0);
        return crate::image::lerp_color([110, 160, 220], [30, 80, 200], t as f64);
    }
    let base = hypsometric(e);
    if e > 0.0 {
        shade(base, hillshade(g, idx))
    } else {
        base
    }
}

fn bathymetry_color(g: &PlanetGrid, idx: usize) -> [u8; 3] {
    let e = g.elevation.data[idx] as f64;
    if e > 0.0 {
        return shade([120, 120, 110], hillshade(g, idx));
    }
    // Shelf / slope / abyss / trench bands.
    ramp(
        &[
            (-11000.0, [40, 0, 40]),
            (-7000.0, [120, 0, 60]),
            (-5200.0, [10, 10, 60]),
            (-3200.0, [20, 40, 120]),
            (-2000.0, [30, 90, 160]),
            (-400.0, [40, 150, 190]),
            (-200.0, [90, 210, 220]),
            (0.0, [170, 240, 240]),
        ],
        e,
    )
}

fn plate_color(g: &PlanetGrid, idx: usize) -> [u8; 3] {
    let p = g.plate[idx] as u64;
    let h = hearth_math::hash::mix64(p + 17);
    let base = [
        (h & 255) as u8,
        ((h >> 8) & 255) as u8,
        ((h >> 16) & 255) as u8,
    ];
    let cont = g.layout.plates[p as usize].continental;
    let c = if cont { base } else { shade(base, 0.45) };
    if g.elevation.data[idx] > 0.0 {
        crate::image::lerp_color(c, [255, 255, 255], 0.25)
    } else {
        c
    }
}

fn current_color(g: &PlanetGrid, idx: usize) -> [u8; 3] {
    if g.elevation.data[idx] > 0.0 {
        return [90, 90, 90];
    }
    let c = g.field_at(&g.current, idx) as f64;
    ramp(
        &[
            (-7.0, [0, 60, 255]),
            (0.0, [235, 235, 235]),
            (7.0, [255, 30, 0]),
        ],
        c,
    )
}

fn temperature_color(g: &PlanetGrid, idx: usize) -> [u8; 3] {
    let t = g.field_at(&g.temperature, idx) as f64;
    let c = ramp(
        &[
            (-40.0, [120, 0, 160]),
            (-20.0, [60, 60, 255]),
            (0.0, [200, 230, 255]),
            (10.0, [120, 220, 120]),
            (20.0, [250, 220, 60]),
            (30.0, [230, 40, 20]),
        ],
        t,
    );
    if g.elevation.data[idx] <= 0.0 {
        shade(c, 0.7)
    } else {
        c
    }
}

fn precip_color(g: &PlanetGrid, idx: usize) -> [u8; 3] {
    let p = g.field_at(&g.precipitation, idx) as f64;
    let c = ramp(
        &[
            (0.0, [150, 60, 20]),
            (250.0, [230, 190, 90]),
            (500.0, [240, 240, 140]),
            (1000.0, [110, 200, 90]),
            (2000.0, [30, 130, 200]),
            (3500.0, [30, 30, 150]),
        ],
        p,
    );
    if g.elevation.data[idx] <= 0.0 {
        shade(c, 0.55)
    } else {
        c
    }
}

fn climate_color(g: &PlanetGrid, idx: usize) -> [u8; 3] {
    let c = g.climate_at(idx);
    let col = c.color();
    if c == ClimateClass::Ocean {
        return col;
    }
    shade(col, 0.75 + 0.25 * hillshade(g, idx))
}

type Layer = (&'static str, fn(&PlanetGrid, usize) -> [u8; 3]);

const LAYERS: [Layer; 8] = [
    ("relief", relief_color),
    ("bathymetry", bathymetry_color),
    ("climate", climate_color),
    ("plates", plate_color),
    ("currents", current_color),
    ("temperature", temperature_color),
    ("precipitation", precip_color),
    ("rivers", rivers_color),
];

fn rivers_color(g: &PlanetGrid, idx: usize) -> [u8; 3] {
    let f = g.flags[idx];
    if g.elevation.data[idx] <= 0.0 && f & flags::LAKE == 0 {
        return [20, 20, 30];
    }
    if f & (flags::LAKE | flags::RIVER) != 0 {
        return relief_color(g, idx);
    }
    [200, 200, 190]
}

pub fn run(args: &[String]) -> anyhow::Result<()> {
    let a = parse(args)?;
    let settings = WorldGenSettings {
        seed: a.seed,
        planet_size: a.planet,
        grid_resolution: a.res,
        rarity: a.rarity,
        ..WorldGenSettings::default()
    };
    let t0 = Instant::now();
    let last = std::sync::Mutex::new(String::new());
    let g = PlanetGrid::build(&settings, &|f, stage| {
        let mut l = last.lock().expect("progress lock");
        if *l != stage {
            println!(
                "  [{:5.1}%] {stage}  ({:.2}s)",
                f * 100.0,
                t0.elapsed().as_secs_f64()
            );
            *l = stage.to_owned();
        }
    });
    println!(
        "planet built in {:.2}s (grid {}², {} plates, {} volcanoes)",
        t0.elapsed().as_secs_f64(),
        g.n(),
        g.layout.plates.len(),
        g.volcanoes.len()
    );
    std::fs::create_dir_all(&a.out)?;
    let n = g.n();
    let mut colors: Vec<(&str, Vec<[u8; 3]>)> = LAYERS
        .iter()
        .map(|(name, f)| (*name, (0..n * n).map(|idx| f(&g, idx)).collect()))
        .collect();
    // Biome layer: sample the block-level biome at every cell centre.
    let g = std::sync::Arc::new(g);
    let terrain = hearth_worldgen::Terrain::new(g.clone());
    let biome_px: Vec<[u8; 3]> = {
        use rayon::prelude::*;
        (0..n * n)
            .into_par_iter()
            .map(|idx| {
                let (i, j) = g.geom.ij(idx);
                let (x, z) = g.geom.world_xz(i, j);
                let s = terrain.sample(x as i32, z as i32);
                let c = s.biome.color();
                if s.is_underwater() {
                    c
                } else {
                    shade(c, 0.8 + 0.2 * hillshade(&g, idx))
                }
            })
            .collect()
    };
    colors.push(("biome", biome_px));
    // Geology: the province and the rock under the soil at every cell centre.
    let content = hearth_content::Content::load_base();
    let reg = hearth_world::datapack::load_builtin_registry().map_err(|e| anyhow::anyhow!(e))?;
    let geology = hearth_worldgen::geology::Geology::new(&g, &content, &reg)?;
    let (province_px, rock_px): (Vec<[u8; 3]>, Vec<[u8; 3]>) = {
        use rayon::prelude::*;
        (0..n * n)
            .into_par_iter()
            .map(|idx| {
                let (i, j) = g.geom.ij(idx);
                let (x, z) = g.geom.world_xz(i, j);
                let s = terrain.sample(x as i32, z as i32);
                let col = geology.column(x as i32, z as i32);
                let p = hearth_math::hash::hash2(0x9e0, col.province as u64);
                let pc = [
                    (p & 0xff) as u8 | 0x30,
                    ((p >> 8) & 0xff) as u8 | 0x30,
                    ((p >> 16) & 0xff) as u8 | 0x30,
                ];
                let rock = col.rock_at(s.height_i() - 1 - s.soil_depth as i32);
                let rc = reg.block_of(rock).map_color;
                let hs = 0.7 + 0.3 * hillshade(&g, idx);
                if s.is_underwater() {
                    (shade(pc, 0.6), shade(rc, 0.6))
                } else {
                    (shade(pc, hs), shade(rc, hs))
                }
            })
            .unzip()
    };
    colors.push(("province", province_px));
    colors.push(("rock", rock_px));
    for p in geology.provinces() {
        println!("  province {:<22} {:?}", p.id, p.setting);
    }
    let soils = hearth_worldgen::soil::Soils::new(
        &content,
        &reg,
        g.seed,
        g.planet().circumference() as i64,
    )?;
    for &(x, z, size) in &a.geo_areas {
        geo_area(&terrain, &geology, &soils, &reg, x, z, size, &a.out)?;
    }
    let g = &*g;
    let merc_size = a.width.min(n).max(64);
    for (name, px) in &colors {
        // Mercator: the grid itself (downsampled if needed).
        let mut img = Image::new(merc_size, merc_size);
        let _ = &g;
        for y in 0..merc_size {
            for x in 0..merc_size {
                let i = x * n / merc_size;
                let j = y * n / merc_size;
                img.set(x, y, px[j * n + i]);
            }
        }
        img.save(&a.out.join(format!("mercator_{name}.png")))?;
        // Equirectangular: uniform latitude rows.
        let w = a.width;
        let h = w / 2;
        let mut eq = Image::new(w, h);
        let planet = g.planet();
        for y in 0..h {
            let lat = 90.0 - (y as f64 + 0.5) / h as f64 * 180.0;
            let lat = lat.clamp(-85.0, 85.0).to_radians();
            let z = planet.z_for_latitude(lat);
            for x in 0..w {
                let wx = (x as f64 + 0.5) / w as f64 * planet.circumference_f64();
                eq.set(x, y, px[g.cell_at(wx, z)]);
            }
        }
        eq.save(&a.out.join(format!("equirect_{name}.png")))?;
    }
    for s in &a.slices {
        slice(g, *s, &a.out)?;
    }
    stats(g);
    println!("maps written to {}", a.out.display());
    Ok(())
}

/// Block-scale geology of a square area centred on (x, z): a top-down map of the rock under
/// the soil (hillshaded, water tinted) and an east–west cross-section through the middle, from
/// 300 blocks under the lowest surface to 60 above the highest.
#[allow(clippy::too_many_arguments)]
fn geo_area(
    terrain: &hearth_worldgen::Terrain,
    geology: &hearth_worldgen::geology::Geology,
    soils: &hearth_worldgen::soil::Soils,
    reg: &hearth_world::BlockRegistry,
    x: i32,
    z: i32,
    size: i32,
    out: &std::path::Path,
) -> anyhow::Result<()> {
    use rayon::prelude::*;
    let n = size.max(16) as usize;
    let (x0, z0) = (x - size / 2, z - size / 2);
    let rows: Vec<Vec<([u8; 3], i32)>> = (0..n)
        .into_par_iter()
        .map(|j| {
            (0..n)
                .map(|i| {
                    let (bx, bz) = (x0 + i as i32, z0 + j as i32);
                    let s = terrain.sample(bx, bz);
                    let rock = geology
                        .column(bx, bz)
                        .rock_at(s.height_i() - 1 - s.soil_depth as i32);
                    let mut c = reg.block_of(rock).map_color;
                    if s.is_underwater() {
                        c = [c[0] / 2, c[1] / 2, c[2] / 2 + 60];
                    }
                    (c, s.height_i())
                })
                .collect()
        })
        .collect();
    let mut map = Image::new(n, n);
    for j in 0..n {
        for i in 0..n {
            let (c, h) = rows[j][i];
            let (hl, hu) = (
                rows[j][i.saturating_sub(1)].1,
                rows[j.saturating_sub(1)][i].1,
            );
            let slope = ((h - hl) + (h - hu)) as f64;
            map.set(i, j, shade(c, (1.0 + slope * 0.08).clamp(0.6, 1.4)));
        }
    }
    let name = format!("geoarea_{x}_{z}");
    map.save(&out.join(format!("{name}_map.png")))?;
    // Cross-section along the middle row.
    let zc = z;
    let samples: Vec<_> = (0..n).map(|i| terrain.sample(x0 + i as i32, zc)).collect();
    let top = samples
        .iter()
        .map(|s| s.height_i().max(s.water_i()))
        .max()
        .unwrap_or(0)
        + 60;
    let bottom = samples.iter().map(|s| s.height_i()).min().unwrap_or(0) - 300;
    let h = (top - bottom).max(1) as usize;
    let mut img = Image::new(n, h);
    for (i, s) in samples.iter().enumerate() {
        let col = geology.column(x0 + i as i32, zc);
        let ground = s.height_i();
        let parent = col.rock_at(ground - 1 - s.soil_depth as i32);
        let profile = soils.profile(s, parent, x0 + i as i32, zc);
        for row in 0..h {
            let y = top - row as i32;
            let depth = ground - 1 - y;
            let c = if depth >= 0 && (depth as usize) < profile.len() {
                reg.block_of(profile[depth as usize]).map_color
            } else if y < ground {
                reg.block_of(col.rock_at(y)).map_color
            } else if y < s.water_i() {
                [40, 90, 200]
            } else {
                [200, 225, 250]
            };
            img.set(i, row, c);
        }
    }
    img.save(&out.join(format!("{name}_section.png")))?;
    let p = geology.province(geology.column(x, z).province);
    println!(
        "wrote {name} ({} blocks, province {}, y {bottom}..{top})",
        n, p.id
    );
    // Soils of the area: which types and how deep.
    let mut counts = vec![(0usize, 0usize, 0usize); soils.soil_count()];
    for j in (0..n).step_by(4) {
        for i in (0..n).step_by(4) {
            let (bx, bz) = (x0 + i as i32, z0 + j as i32);
            let s = terrain.sample(bx, bz);
            if s.is_underwater() {
                continue;
            }
            let parent = geology
                .column(bx, bz)
                .rock_at(s.height_i() - 1 - s.soil_depth as i32);
            let choice = soils.choose(&s, parent, bx, bz);
            let prof = soils.profile(&s, parent, bx, bz);
            let c = &mut counts[choice.soil];
            c.0 += 1;
            c.1 += prof.len();
            c.2 += s.soil_depth as usize;
        }
    }
    for (i, (count, prof, sd)) in counts.iter().enumerate() {
        if *count > 0 {
            println!(
                "    {:<24} {:>5} columns, profile {:.1} blocks, soil depth {:.1}",
                soils.soil_id(i),
                count,
                *prof as f64 / *count as f64,
                *sd as f64 / *count as f64
            );
        }
    }
    Ok(())
}

fn slice(g: &PlanetGrid, s: Slice, out: &std::path::Path) -> anyhow::Result<()> {
    let planet = g.planet();
    let w = 2048usize;
    let h = 600usize;
    let (e_min, e_max) = (-11000.0f64, 9000.0f64);
    let mut img = Image::new(w, h);
    let (name, points): (String, Vec<(f64, f64)>) = match s {
        Slice::Latitude(lat) => {
            let z = planet.z_for_latitude(lat.to_radians());
            (
                format!("slice_lat{lat}"),
                (0..w)
                    .map(|x| ((x as f64 + 0.5) / w as f64 * planet.circumference_f64(), z))
                    .collect(),
            )
        }
        Slice::Longitude(lon) => {
            let x = planet.x_for_longitude(lon.to_radians());
            (
                format!("slice_lon{lon}"),
                (0..w)
                    .map(|k| {
                        let lat = 85.0 - (k as f64 + 0.5) / w as f64 * 170.0;
                        (x, planet.z_for_latitude(lat.to_radians()))
                    })
                    .collect(),
            )
        }
    };
    let to_y = |e: f64| -> usize {
        (((e_max - e) / (e_max - e_min)) * h as f64).clamp(0.0, h as f64 - 1.0) as usize
    };
    for (x, (wx, wz)) in points.iter().enumerate() {
        let idx = g.cell_at(*wx, *wz);
        let e = g.elevation.data[idx] as f64;
        let water = g.water.data[idx];
        let ground_y = to_y(e);
        for y in 0..h {
            let c = if y >= ground_y {
                if e > 0.0 { [120, 95, 70] } else { [80, 70, 60] }
            } else if !water.is_nan() && y >= to_y(water as f64) {
                [40, 90, 200]
            } else {
                [200, 225, 250]
            };
            img.set(x, y, c);
        }
        img.set(x, to_y(0.0), [0, 0, 0]);
    }
    img.save(&out.join(format!("{name}.png")))?;
    Ok(())
}

fn stats(g: &PlanetGrid) {
    let n = g.n();
    let w = hearth_worldgen::planet::fields::row_area_weights(&g.geom);
    let v = g.vertical_scale;
    let mut land: Vec<(f32, f64)> = Vec::new();
    let mut sea: Vec<(f32, f64)> = Vec::new();
    for (j, wj) in w.iter().enumerate() {
        for i in 0..n {
            let e = g.elevation.data[j * n + i];
            if e > 0.0 {
                land.push((e, *wj));
            } else {
                sea.push((e, *wj));
            }
        }
    }
    let q = |v: &mut Vec<(f32, f64)>, qs: &[f64]| -> Vec<f32> {
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        let total: f64 = v.iter().map(|p| p.1).sum();
        qs.iter()
            .map(|q| {
                let target = total * q;
                let mut acc = 0.0;
                for (e, wt) in v.iter() {
                    acc += wt;
                    if acc >= target {
                        return *e;
                    }
                }
                v.last().map_or(0.0, |p| p.0)
            })
            .collect()
    };
    let lq = q(&mut land, &[0.25, 0.5, 0.75, 0.9, 0.99, 1.0]);
    let sq = q(&mut sea, &[0.0, 0.01, 0.1, 0.5, 0.9]);
    println!("land fraction: {:.1}%", g.land_fraction() * 100.0);
    println!(
        "land elevation (blocks) p25 {:.0} p50 {:.0} p75 {:.0} p90 {:.0} p99 {:.0} max {:.0}",
        lq[0] as f64 * v,
        lq[1] as f64 * v,
        lq[2] as f64 * v,
        lq[3] as f64 * v,
        lq[4] as f64 * v,
        lq[5] as f64 * v
    );
    println!(
        "sea floor (blocks) min {:.0} p1 {:.0} p10 {:.0} p50 {:.0} p90 {:.0}",
        sq[0] as f64 * v,
        sq[1] as f64 * v,
        sq[2] as f64 * v,
        sq[3] as f64 * v,
        sq[4] as f64 * v
    );
    // Climate distribution by latitude band.
    let mut table = vec![[0.0f64; 14]; 9];
    for (j, wj) in w.iter().enumerate() {
        let band = ((g.geom.lat[j].to_degrees().abs() / 10.0) as usize).min(8);
        for i in 0..n {
            let c = g.climate[j * n + i] as usize;
            if c != 0 {
                table[band][c] += wj;
            }
        }
    }
    println!("climate share of land by |latitude| band:");
    for (b, row) in table.iter().enumerate() {
        let total: f64 = row.iter().sum();
        if total <= 0.0 {
            continue;
        }
        let mut parts: Vec<(f64, &str)> = ClimateClass::ALL
            .iter()
            .skip(1)
            .map(|c| (row[*c as usize] / total, c.code()))
            .filter(|p| p.0 > 0.04)
            .collect();
        parts.sort_by(|a, b| b.0.total_cmp(&a.0));
        let s: Vec<String> = parts
            .iter()
            .map(|(f, c)| format!("{c} {:.0}%", f * 100.0))
            .collect();
        println!("  {:2}-{:2}°: {}", b * 10, b * 10 + 10, s.join(", "));
    }
    let rivers = g.flags.iter().filter(|f| **f & flags::RIVER != 0).count();
    let lakes = g.flags.iter().filter(|f| **f & flags::LAKE != 0).count();
    let salt = g
        .flags
        .iter()
        .filter(|f| **f & flags::SALT_FLAT != 0)
        .count();
    println!("river cells {rivers}, lake cells {lakes}, salt-flat cells {salt}");
    if std::env::var_os("HEARTH_DEBUG_LAKES").is_some() {
        let mut below = 0usize;
        let mut sum_e = 0.0f64;
        let mut sum_w = 0.0f64;
        let mut by_lat = [0usize; 9];
        for idx in 0..n * n {
            if g.flags[idx] & flags::LAKE != 0 {
                let e = g.elevation.data[idx];
                if e <= 0.0 {
                    below += 1;
                }
                sum_e += e as f64;
                sum_w += g.water.data[idx] as f64;
                let j = idx / n;
                by_lat[((g.geom.lat[j].to_degrees().abs() / 10.0) as usize).min(8)] += 1;
            }
        }
        println!(
            "lake cells below sea level {below}, mean elev {:.0} m, mean water {:.0} m, by lat {:?}",
            sum_e / lakes.max(1) as f64,
            sum_w / lakes.max(1) as f64,
            by_lat
        );
    }
}

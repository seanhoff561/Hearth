//! `bench region`: top-down block-resolution render of the regional sampler (surface colours,
//! water, hillshade) around a point — for inspecting terrain before cubes exist.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use hearth_math::PlanetSize;
use hearth_worldgen::region::biome::{foliage_color, grass_color, water_color};
use hearth_worldgen::{ColumnSample, PlanetGrid, Surface, Terrain, WorldGenSettings};
use rayon::prelude::*;

use crate::image::{Image, lerp_color, shade};

pub fn run(args: &[String]) -> anyhow::Result<()> {
    let mut seed = 1u64;
    let mut planet = PlanetSize::Standard;
    let mut res = 1024usize;
    let mut size = 1024usize;
    let mut scale = 1.0f64;
    let mut center: Option<(i32, i32)> = None;
    let mut out = PathBuf::from("bench-out/region");
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || {
            it.next()
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("{a} needs a value"))
        };
        match a.as_str() {
            "--seed" => seed = val()?.parse()?,
            "--planet" => {
                planet =
                    PlanetSize::from_name(&val()?).ok_or_else(|| anyhow::anyhow!("bad planet"))?
            }
            "--res" => res = val()?.parse()?,
            "--size" => size = val()?.parse()?,
            "--scale" => scale = val()?.parse()?,
            "--at" => {
                let v = val()?;
                let (x, z) = v
                    .split_once(',')
                    .ok_or_else(|| anyhow::anyhow!("--at X,Z"))?;
                center = Some((x.trim().parse()?, z.trim().parse()?));
            }
            "--out" => out = PathBuf::from(val()?),
            other => anyhow::bail!("unknown argument {other}"),
        }
    }
    let settings = WorldGenSettings {
        seed,
        planet_size: planet,
        grid_resolution: res,
        ..WorldGenSettings::default()
    };
    let t0 = Instant::now();
    let grid = Arc::new(PlanetGrid::build(&settings, &|_, _| {}));
    let terrain = Terrain::new(grid);
    println!(
        "planet + terrain ready in {:.2}s",
        t0.elapsed().as_secs_f64()
    );
    let (cx, cz) = center.unwrap_or_else(|| terrain.find_spawn(false));
    let s = terrain.sample(cx, cz);
    println!(
        "centre {cx},{cz}: lat {:.1} lon {:.1} height {:.1} water {:.1} biome {} climate {} T {:.1} P {:.0}",
        terrain.planet().latitude_deg(cz as f64),
        terrain.planet().longitude_deg(cx as f64),
        s.height,
        s.water,
        s.biome.name(),
        s.climate.code(),
        s.temperature,
        s.precipitation
    );
    let t1 = Instant::now();
    let half = size as f64 * scale * 0.5;
    let x0 = cx as f64 - half;
    let z0 = cz as f64 - half;
    // Sample in 16×16 tiles with shared river segments, like cube generation does.
    let rows: Vec<Vec<ColumnSample>> = (0..size)
        .into_par_iter()
        .map(|py| {
            let wz = (z0 + py as f64 * scale) as i32;
            let segs = terrain.river_segments(x0 as i32, wz, (x0 + size as f64 * scale) as i32, wz);
            (0..size)
                .map(|px| {
                    let wx = (x0 + px as f64 * scale) as i32;
                    terrain.sample_with(wx, wz, &segs)
                })
                .collect()
        })
        .collect();
    let dt = t1.elapsed().as_secs_f64();
    println!(
        "sampled {} columns in {:.2}s ({:.2} µs/column)",
        size * size,
        dt,
        dt * 1e6 / (size * size) as f64
    );
    let mut img = Image::new(size, size);
    for py in 0..size {
        for px in 0..size {
            let c = &rows[py][px];
            let hx =
                rows[py][(px + 1).min(size - 1)].height - rows[py][px.saturating_sub(1)].height;
            let hz =
                rows[(py + 1).min(size - 1)][px].height - rows[py.saturating_sub(1)][px].height;
            let light = (1.0 - (hx + hz) as f64 / (4.0 * scale) * 0.6).clamp(0.4, 1.4);
            img.set(px, py, column_color(c, light));
        }
    }
    img.save(&out.with_extension("png"))?;
    println!("wrote {}", out.with_extension("png").display());
    Ok(())
}

fn surface_color(c: &ColumnSample) -> [u8; 3] {
    let grass = grass_color(c.temperature, c.precipitation);
    match c.surface {
        Surface::Grass => grass,
        Surface::SnowGrass | Surface::Snow => [240, 245, 250],
        Surface::Podzol => [110, 80, 45],
        Surface::CoarseDirt => [120, 90, 60],
        Surface::Dirt => [130, 95, 65],
        Surface::Sand => [225, 210, 160],
        Surface::RedSand => [190, 105, 50],
        Surface::Gravel => [135, 130, 125],
        Surface::Stone => [125, 125, 125],
        Surface::Ice => [170, 200, 250],
        Surface::Mud => [70, 60, 55],
        Surface::Clay => [160, 165, 175],
        Surface::Calcite => [235, 235, 230],
        Surface::Moss => [85, 110, 45],
        Surface::Sandstone => [215, 200, 150],
        Surface::RedSandstone => [180, 95, 45],
        Surface::Tuff => [100, 100, 90],
        Surface::Coral => [214, 120, 130],
    }
}

fn column_color(c: &ColumnSample, light: f64) -> [u8; 3] {
    if c.is_underwater() {
        let depth = c.water - c.height;
        let base = water_color(c.sea_temperature, depth);
        let bed = surface_color(c);
        let t = (1.0 - depth as f64 / 30.0).clamp(0.0, 0.7);
        return lerp_color(shade(base, 0.9), bed, t * 0.5);
    }
    let mut col = shade(surface_color(c), light);
    if c.tree_density > 0.0 {
        let foliage = foliage_color(c.temperature, c.precipitation);
        col = lerp_color(
            col,
            shade(foliage, 0.6 * light),
            (c.tree_density * 0.9) as f64,
        );
    }
    col
}

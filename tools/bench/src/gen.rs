//! `bench gen`: cube generation throughput plus top-down and cross-section renders of real
//! generated blocks.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use hearth_math::{CubePos, LocalPos, PlanetSize};
use hearth_world::{BlockRegistry, BlockStateId, Cube};
use hearth_worldgen::{CubeClass, PlanetGrid, Terrain, WorldGenSettings, WorldGenerator};
use rayon::prelude::*;
use rustc_hash::FxHashMap;

use crate::image::{Image, lerp_color, shade};

pub fn run(args: &[String]) -> anyhow::Result<()> {
    let mut seed = 1u64;
    let mut planet = PlanetSize::Standard;
    let mut res = 1024usize;
    let mut radius = 8i32; // cubes
    let mut at: Option<(i32, i32)> = None;
    let mut out = PathBuf::from("bench-out/gen");
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
            "--radius" => radius = val()?.parse()?,
            "--at" => {
                let v = val()?;
                let (x, z) = v
                    .split_once(',')
                    .ok_or_else(|| anyhow::anyhow!("--at X,Z"))?;
                at = Some((x.trim().parse()?, z.trim().parse()?));
            }
            "--out" => out = PathBuf::from(val()?),
            other => anyhow::bail!("unknown argument {other}"),
        }
    }
    let reg = hearth_world::datapack::load_builtin_registry().map_err(|e| anyhow::anyhow!(e))?;
    let settings = WorldGenSettings {
        seed,
        planet_size: planet,
        grid_resolution: res,
        ..WorldGenSettings::default()
    };
    let t0 = Instant::now();
    let grid = Arc::new(PlanetGrid::build(&settings, &|_, _| {}));
    let terrain = Arc::new(Terrain::new(grid));
    let content = hearth_content::Content::load_base();
    let generator = WorldGenerator::new(terrain.clone(), &reg, &content)?;
    println!("world ready in {:.2}s", t0.elapsed().as_secs_f64());
    let (sx, sz) = at.unwrap_or_else(|| terrain.find_spawn(false));
    let surface = terrain.sample(sx, sz).height_i();
    println!("centre {sx},{sz} surface {surface}");
    let (ccx, ccy, ccz) = (sx >> 4, surface >> 4, sz >> 4);
    // Surface band: 2r+1 × 2r+1 columns, 10 cubes tall around the surface.
    let mut cubes = Vec::new();
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            for dy in -6..=4 {
                cubes.push(CubePos::new(ccx + dx, ccy + dy, ccz + dz));
            }
        }
    }
    let t1 = Instant::now();
    let generated: Vec<(CubePos, Cube)> = cubes
        .par_iter()
        .map(|p| (*p, generator.generate_cube(*p)))
        .collect();
    let dt = t1.elapsed().as_secs_f64();
    let s = &generator.stats;
    let (surf, deep, empty) = (
        s.surface.load(Ordering::Relaxed),
        s.deep.load(Ordering::Relaxed),
        s.empty.load(Ordering::Relaxed),
    );
    println!(
        "generated {} cubes in {:.3}s: {:.0} cubes/s ({} surface, {} deep, {} empty) on {} threads",
        generated.len(),
        dt,
        generated.len() as f64 / dt,
        surf,
        deep,
        empty,
        rayon::current_num_threads()
    );
    println!(
        "surface-band throughput: {:.0} surface cubes/s",
        surf as f64 / dt
    );
    // Deep-rock throughput.
    let deep_cubes: Vec<CubePos> = (0..2000)
        .map(|i| CubePos::new(ccx + (i % 40) - 20, -120 - i / 400, ccz + (i / 40) % 10))
        .collect();
    let t2 = Instant::now();
    let _deep: Vec<Cube> = deep_cubes
        .par_iter()
        .map(|p| generator.generate_cube(*p))
        .collect();
    println!(
        "deep cubes: {:.0} cubes/s",
        deep_cubes.len() as f64 / t2.elapsed().as_secs_f64()
    );
    let (hits, misses) = generator.cache_stats();
    println!("column cache: {hits} hits, {misses} misses");
    let map: FxHashMap<CubePos, Cube> = generated.into_iter().collect();
    let colors = block_colors(&reg);
    render_top(&map, &colors, ccx, ccy, ccz, radius, &out)?;
    render_slice(&map, &colors, sx, ccy, ccz, radius, &out)?;
    let _ = CubeClass::Empty;
    Ok(())
}

fn block_colors(reg: &BlockRegistry) -> Vec<[u8; 3]> {
    (0..reg.state_count())
        .map(|i| {
            let st = BlockStateId(i as u16);
            let b = reg.block_of(st);
            let name = b.name.path();
            // Readable overrides for the most common blocks.
            match name {
                "grass_block" => [95, 160, 60],
                "water" => [50, 90, 200],
                "stone" => [120, 120, 120],
                "deepslate" => [70, 70, 75],
                "dirt" => [130, 95, 65],
                "sand" => [225, 210, 160],
                n if n.ends_with("_leaves") => [40, 120, 30],
                n if n.ends_with("_log") => [110, 80, 45],
                _ => b.map_color,
            }
        })
        .collect()
}

fn render_top(
    map: &FxHashMap<CubePos, Cube>,
    colors: &[[u8; 3]],
    ccx: i32,
    ccy: i32,
    ccz: i32,
    radius: i32,
    out: &std::path::Path,
) -> anyhow::Result<()> {
    let size = ((2 * radius + 1) * 16) as usize;
    let mut img = Image::new(size, size);
    let x0 = (ccx - radius) * 16;
    let z0 = (ccz - radius) * 16;
    for pz in 0..size as i32 {
        for px in 0..size as i32 {
            let (x, z) = (x0 + px, z0 + pz);
            let mut found = None;
            'down: for cy in (ccy - 6..=ccy + 4).rev() {
                if let Some(c) = map.get(&CubePos::new(x >> 4, cy, z >> 4)) {
                    for ly in (0..16u8).rev() {
                        let st = c.get(LocalPos::new((x & 15) as u8, ly, (z & 15) as u8));
                        if !st.is_air() {
                            found = Some((st, cy * 16 + ly as i32));
                            break 'down;
                        }
                    }
                }
            }
            if let Some((st, y)) = found {
                let base = colors[st.0 as usize];
                let shade_f = 0.75 + ((y - ccy * 16) as f64 / 64.0).clamp(-0.3, 0.4);
                img.set(px as usize, pz as usize, shade(base, shade_f));
            }
        }
    }
    let p = out.with_file_name(format!(
        "{}_top.png",
        out.file_name().and_then(|n| n.to_str()).unwrap_or("gen")
    ));
    img.save(&p)?;
    println!("wrote {}", p.display());
    Ok(())
}

fn render_slice(
    map: &FxHashMap<CubePos, Cube>,
    colors: &[[u8; 3]],
    sx: i32,
    ccy: i32,
    ccz: i32,
    radius: i32,
    out: &std::path::Path,
) -> anyhow::Result<()> {
    // Vertical slice along X through the centre (z = centre), full generated height.
    let w = ((2 * radius + 1) * 16) as usize;
    let h = (11 * 16) as usize;
    let mut img = Image::new(w, h);
    let x0 = ((sx >> 4) - radius) * 16;
    let z = ccz * 16 + 8;
    let y_top = (ccy + 4) * 16 + 15;
    for py in 0..h as i32 {
        for px in 0..w as i32 {
            let (x, y) = (x0 + px, y_top - py);
            let c = map
                .get(&CubePos::new(x >> 4, y >> 4, z >> 4))
                .map(|c| {
                    c.get(LocalPos::new(
                        (x & 15) as u8,
                        (y & 15) as u8,
                        (z & 15) as u8,
                    ))
                })
                .unwrap_or(BlockStateId::AIR);
            let col = if c.is_air() {
                lerp_color([170, 200, 240], [120, 150, 200], py as f64 / h as f64)
            } else {
                colors[c.0 as usize]
            };
            img.set(px as usize, py as usize, col);
        }
    }
    let p = out.with_file_name(format!(
        "{}_slice.png",
        out.file_name().and_then(|n| n.to_str()).unwrap_or("gen")
    ));
    img.save(&p)?;
    println!("wrote {}", p.display());
    Ok(())
}

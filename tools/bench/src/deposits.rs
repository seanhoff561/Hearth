//! `bench deposits`: the deposit bodies of a world (the game's own settings for the planet
//! size), filtered by model and depth and sorted by distance from a point, and the resource
//! coverage of its continents. Used to find deposits to inspect and photograph.

use std::sync::Arc;
use std::time::Instant;

use hearth_math::PlanetSize;
use hearth_worldgen::{PlanetGrid, Terrain, WorldGenSettings, WorldGenerator};

pub fn run(args: &[String]) -> anyhow::Result<()> {
    let mut seed = 1u64;
    let mut planet = PlanetSize::Standard;
    let mut res = 0usize;
    let mut model: Option<String> = None;
    let mut near: Option<(i32, i32)> = None;
    let mut max_depth = i32::MAX;
    let mut limit = 10usize;
    let mut coverage = false;
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
            "--model" => model = Some(val()?),
            "--near" => {
                let v = val()?;
                let (x, z) = v
                    .split_once(',')
                    .ok_or_else(|| anyhow::anyhow!("--near X,Z"))?;
                near = Some((x.trim().parse()?, z.trim().parse()?));
            }
            "--max-depth" => max_depth = val()?.parse()?,
            "--limit" => limit = val()?.parse()?,
            "--coverage" => coverage = true,
            other => anyhow::bail!("unknown argument {other}"),
        }
    }
    let settings = WorldGenSettings {
        seed,
        planet_size: planet,
        grid_resolution: res,
        ..WorldGenSettings::default()
    }
    .sanitized();
    let t0 = Instant::now();
    let terrain = Arc::new(Terrain::new(Arc::new(PlanetGrid::build(
        &settings,
        &|_, _| {},
    ))));
    let content = hearth_content::Content::load_base();
    let reg = hearth_world::datapack::load_builtin_registry().map_err(|e| anyhow::anyhow!(e))?;
    let wg = WorldGenerator::new(terrain, &reg, &content)?;
    let bodies = wg.deposits.census(&wg);
    println!(
        "seed {seed}: {} bodies ({:.1}s, grid {}²)",
        bodies.len(),
        t0.elapsed().as_secs_f64(),
        settings.grid_resolution
    );
    let (nx, nz) = near.unwrap_or_else(|| wg.terrain.find_spawn(false));
    let planet = *wg.planet();
    let mut found: Vec<_> = bodies
        .iter()
        .filter(|b| {
            let m = &wg.deposits.models[b.model as usize];
            model
                .as_ref()
                .is_none_or(|q| m.id.contains(q.as_str()) || m.resource.contains(q.as_str()))
                && b.top_depth() <= max_depth
        })
        .map(|b| {
            let dx = planet.delta_block_x(nx, b.center[0]) as f64;
            let dz = (b.center[2] - nz) as f64;
            ((dx * dx + dz * dz).sqrt(), b)
        })
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    println!("{} matching; nearest to ({nx}, {nz}):", found.len());
    for (d, b) in found.iter().take(limit) {
        let m = &wg.deposits.models[b.model as usize];
        let s = wg.terrain.sample(b.center[0], b.center[2]);
        println!(
            "  {:<32} at ({}, {}, {}) surface {} top {} deep, {:.0} m across, grade {:.2e}, {:.0} m away, {} {:.0} mm {:.1} °C{}",
            m.id,
            b.center[0],
            b.center[1],
            b.center[2],
            b.surface,
            b.top_depth(),
            b.size(),
            b.grade,
            d,
            s.biome.name(),
            s.precipitation,
            s.temperature,
            if s.is_underwater() {
                ", under water"
            } else {
                ""
            }
        );
    }
    if coverage {
        let scale = planet.circumference() as f64 / 65_536.0;
        let cov =
            hearth_worldgen::coverage::coverage_of(&wg, &content, &bodies, 5, 50.0 * scale * scale);
        crate::worldmap::print_coverage(&cov, &wg.deposits);
    }
    Ok(())
}

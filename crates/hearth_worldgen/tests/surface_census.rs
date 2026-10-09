//! A census of the land as generated on the Earth-sized planet, for reviews that must show the
//! land looks the same after a change (E4.1 §4.8, `docs/review/earth-scale-performance.md`): 600
//! columns of land spread round the planet within about 60° of the equator, the two cubes about
//! each surface tallied by kind of block, each column's top block, and a hash of each column's
//! blocks, written one column a line to tell which came out the same in two builds.
//! `HEARTH_CENSUS_GRID=<planet file, else built> HEARTH_CENSUS_OUT=<file>
//! cargo test --profile dev-opt -p hearth_worldgen --test surface_census -- --ignored --nocapture`

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use hearth_content::Content;
use hearth_math::{BlockPos, CubePos, LocalPos, PlanetSize};
use hearth_worldgen::{PlanetGrid, Terrain, WorldGenSettings, WorldGenerator};
use rayon::prelude::*;

#[test]
#[ignore = "a review's tool: run by hand"]
fn the_land_about_six_hundred_columns() {
    let grid = match std::env::var("HEARTH_CENSUS_GRID") {
        Ok(path) => PlanetGrid::load(std::path::Path::new(&path)).expect("the planet file"),
        Err(_) => PlanetGrid::build(
            &WorldGenSettings {
                seed: 7,
                planet_size: PlanetSize::Earth,
                grid_resolution: 0,
            }
            .sanitized(),
            &|_, _| {},
        ),
    };
    let terrain = Arc::new(Terrain::new(Arc::new(grid)));
    let reg = hearth_world::datapack::load_builtin_registry().expect("base pack");
    let wg = WorldGenerator::new(terrain, &reg, &Content::load_base()).expect("generator");
    let c = wg.planet().circumference() as f64;
    // Spread evenly (two irrational strides) round the planet and over two thirds of its length;
    // the first 600 on land.
    let candidates: Vec<(i32, i32)> = (0..3000)
        .map(|k| {
            let u = (k as f64 * 0.618_033_988_75).fract();
            let v = (k as f64 * 0.754_877_666_25).fract();
            ((u * c) as i32, ((v - 0.5) * c * 0.66) as i32)
        })
        .collect();
    // The land among them, the first 600.
    let land: Vec<bool> = candidates
        .par_iter()
        .map(|&(x, z)| wg.terrain.sample(x, z).height_i() > 0)
        .collect();
    let cols: Vec<(i32, i32)> = candidates
        .iter()
        .zip(&land)
        .filter(|(_, l)| **l)
        .map(|(c, _)| *c)
        .take(600)
        .collect();
    assert_eq!(cols.len(), 600, "enough land");
    let t0 = std::time::Instant::now();
    let rows: Vec<(HashMap<String, u64>, String, i32, u64)> = cols
        .par_iter()
        .map(|&(x, z)| {
            let h = wg.terrain.sample(x, z).height_i();
            let base = BlockPos::new(x, h, z).cube();
            let (lx, lz) = (x.rem_euclid(16) as u8, z.rem_euclid(16) as u8);
            let mut tally: HashMap<String, u64> = HashMap::new();
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            let mut top = (i32::MIN, String::from("air"));
            for dy in 0..2 {
                let p = CubePos::new(base.x, base.y + dy, base.z);
                let cube = wg.generate_cube(p);
                for i in 0..hearth_math::CUBE_VOLUME {
                    let s = cube.get_index(i);
                    let name = reg.block_of(s).name.to_string();
                    name.hash(&mut hasher);
                    if s.is_air() {
                        continue;
                    }
                    let l = LocalPos::from_index(i);
                    let y = p.y * 16 + l.y as i32;
                    if l.x == lx && l.z == lz && y > top.0 {
                        top = (y, name.clone());
                    }
                    *tally.entry(name).or_default() += 1;
                }
            }
            (tally, top.1, h, hasher.finish())
        })
        .collect();
    let took = t0.elapsed().as_secs_f64();
    let mut all: HashMap<String, u64> = HashMap::new();
    let mut tops: HashMap<String, u64> = HashMap::new();
    let mut lines = String::new();
    for ((x, z), (tally, top, h, hash)) in cols.iter().zip(&rows) {
        for (k, v) in tally {
            *all.entry(k.clone()).or_default() += v;
        }
        *tops.entry(top.clone()).or_default() += 1;
        lines.push_str(&format!("{x} {z} {h} {hash:016x} {top}\n"));
    }
    if let Ok(out) = std::env::var("HEARTH_CENSUS_OUT") {
        std::fs::write(&out, lines).expect("written");
    }
    let total: u64 = all.values().sum();
    let mut v: Vec<_> = all.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    println!("{} columns, two cubes each, {took:.1} s", cols.len());
    println!("blocks by kind (share of those not air):");
    for (k, n) in v.iter().take(40) {
        println!("  {:>7.3} %  {k}", 100.0 * *n as f64 / total as f64);
    }
    let mut t: Vec<_> = tops.into_iter().collect();
    t.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    println!("the top block of each column:");
    for (k, n) in t.iter().take(25) {
        println!("  {n:>4}  {k}");
    }
}

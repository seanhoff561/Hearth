//! What lies about the spawn of a few worlds (development aid for the acceptance bot):
//! `cargo test -p hearth --test survey -- --ignored --nocapture`.

mod common;

use common::{World, temp};
use rustc_hash::FxHashMap;

#[test]
#[ignore]
fn what_lies_about_the_spawn() {
    for seed in [11u64, 3, 7, 21, 42] {
        let dir = temp(&format!("survey-{seed}"));
        let w = World::start(&dir, hearth_save::KnowledgeMode::Discovery, seed);
        let mut counts: FxHashMap<String, usize> = FxHashMap::default();
        for p in w.find(40, |name, _| {
            name.ends_with("_cobbles")
                || name.ends_with("_log")
                || matches!(
                    name,
                    "nettle"
                        | "hazel"
                        | "bramble"
                        | "tall_grass"
                        | "water"
                        | "vine"
                        | "sugar_cane"
                        | "dead_bush"
                        | "short_dry_grass"
                )
        }) {
            let name = w.block(p).unwrap_or_default();
            *counts.entry(name).or_default() += 1;
        }
        let mut v: Vec<_> = counts.into_iter().collect();
        v.sort();
        println!("seed {seed} at {:?}: {v:?}", w.mover.pos);
        drop(w);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Where the nearest fresh surface water lies from the spawn of seed 7 (rivers and fresh lakes).
#[test]
#[ignore]
fn fresh_water_about_the_spawn() {
    use std::sync::Arc;
    let seed = 7u64;
    let settings = hearth_worldgen::WorldGenSettings {
        seed,
        planet_size: hearth_math::PlanetSize::Tiny,
        ..hearth_worldgen::WorldGenSettings::default()
    }
    .sanitized();
    let grid = hearth_worldgen::PlanetGrid::build(&settings, &|_, _| {});
    let terrain = Arc::new(hearth_worldgen::Terrain::new(Arc::new(grid)));
    let packs = [hearth::scene::data_pack_dir()];
    let defs = hearth_world::datapack::load_block_defs(&packs).expect("blocks");
    let reg = hearth_world::BlockRegistry::build(defs).expect("registry");
    let content = hearth_content::Content::load_base();
    let wg = hearth_worldgen::WorldGenerator::new(terrain, &reg, &content).expect("generator");
    let (cx, cz) = (7533, 2011);
    let mut found: Vec<(i32, i32, i32, String)> = Vec::new();
    for dz in (-600..=600).step_by(4) {
        for dx in (-600..=600).step_by(4) {
            let (x, z) = (cx + dx, cz + dz);
            let col = wg.column(hearth_math::ColumnPos::new(x >> 4, z >> 4));
            let s = *col.at((x & 15) as usize, (z & 15) as usize);
            if !s.is_underwater() || s.ocean {
                continue;
            }
            let y = s.water_i() - 1;
            let q = wg.hydro.quality(&wg, x, y, z);
            let salty = q.is_some_and(|q| q.salinity_g_l > 1.0);
            if salty {
                continue;
            }
            let d = dx * dx + dz * dz;
            let kind = if s.river.is_some() {
                "river"
            } else if s.lake {
                "lake"
            } else {
                "other"
            };
            found.push((d, x, z, format!("{kind} y {y} height {}", s.height_i())));
        }
    }
    found.sort();
    for (d, x, z, what) in found.iter().take(12) {
        println!("{:.0} m: ({x}, {z}) {what}", (*d as f64).sqrt());
    }
    println!("{} fresh water columns within 600 m", found.len());
}

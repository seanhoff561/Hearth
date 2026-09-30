//! Generator tests: determinism, order independence, agreement with the surface sampler, fast
//! paths and the wrap seam.

use std::sync::{Arc, OnceLock};

use hearth_math::{CUBE_VOLUME, CubePos, LocalPos, PlanetSize};
use hearth_world::{BlockRegistry, BlockStateId, Cube, StateFlags};
use rayon::prelude::*;

use super::{CubeClass, WorldGenerator};
use crate::PlanetGrid;
use crate::region::Terrain;
use crate::settings::WorldGenSettings;

fn registry() -> &'static BlockRegistry {
    static R: OnceLock<BlockRegistry> = OnceLock::new();
    R.get_or_init(|| hearth_world::datapack::load_builtin_registry().expect("base pack"))
}

fn terrain() -> Arc<Terrain> {
    static T: OnceLock<Arc<Terrain>> = OnceLock::new();
    T.get_or_init(|| {
        let s = WorldGenSettings {
            seed: 5,
            planet_size: PlanetSize::Standard,
            grid_resolution: 256,
            ..WorldGenSettings::default()
        };
        Arc::new(Terrain::new(Arc::new(PlanetGrid::build(&s, &|_, _| {}))))
    })
    .clone()
}

fn generator() -> WorldGenerator {
    WorldGenerator::new(terrain(), registry()).expect("all generator blocks exist")
}

/// Cubes around the spawn surface.
fn sample_cubes() -> Vec<CubePos> {
    let t = terrain();
    let (sx, sz) = t.find_spawn(false);
    let h = t.sample(sx, sz).height_i();
    let (cx, cy, cz) = (sx >> 4, h >> 4, sz >> 4);
    let mut v = Vec::new();
    for dz in -2..=2 {
        for dy in -3..=2 {
            for dx in -2..=2 {
                v.push(CubePos::new(cx + dx, cy + dy, cz + dz));
            }
        }
    }
    v
}

fn same(a: &Cube, b: &Cube) -> bool {
    (0..CUBE_VOLUME).all(|i| a.get_index(i) == b.get_index(i))
}

#[test]
fn generation_is_deterministic_across_threads_and_orders() {
    let cubes = sample_cubes();
    let g1 = generator();
    let seq: Vec<Cube> = cubes.iter().map(|p| g1.generate_cube(*p)).collect();
    // A fresh generator (cold caches), reversed order, in parallel.
    let g2 = generator();
    let mut rev = cubes.clone();
    rev.reverse();
    let par: Vec<(CubePos, Cube)> = rev.par_iter().map(|p| (*p, g2.generate_cube(*p))).collect();
    for (p, c) in par {
        let i = cubes.iter().position(|q| *q == p).unwrap();
        assert!(same(&seq[i], &c), "cube {p:?} differs between runs");
    }
}

#[test]
fn features_cross_cube_borders_consistently() {
    // Generating a cube alone gives the same result as generating it after its neighbours.
    let cubes = sample_cubes();
    let g1 = generator();
    let g2 = generator();
    for p in cubes.iter().rev() {
        g2.generate_cube(*p);
    }
    for p in &cubes {
        assert!(same(&g1.generate_cube(*p), &g2.generate_cube(*p)), "{p:?}");
    }
}

#[test]
fn full_generation_agrees_with_surface_sampler() {
    let g = generator();
    let reg = registry();
    let b = &g.blocks;
    let t = terrain();
    let mut rng = hearth_math::hash::Rng::new(9);
    let (mut ok, mut total) = (0, 0);
    let mut columns = 0;
    while columns < 300 {
        let x = rng.range_i32(0, t.planet().circumference() - 1);
        let z = rng.range_i32(-20_000, 20_000);
        let s = t.sample(x, z);
        if s.is_underwater() || s.height < 2.0 {
            continue;
        }
        columns += 1;
        let top = s.height_i();
        // Generate the cubes spanning the surface and find the highest terrain block.
        let mut highest = i32::MIN;
        for cy in ((top - 20) >> 4)..=((top + 12) >> 4) {
            let cube = g.generate_cube(CubePos::new(x >> 4, cy, z >> 4));
            for ly in (0..16).rev() {
                let st = cube.get(LocalPos::new((x & 15) as u8, ly, (z & 15) as u8));
                let y = cy * 16 + ly as i32;
                let terrain_block =
                    !st.is_air() && reg.has(st, StateFlags::OPAQUE) && !is_feature(b, st);
                if terrain_block && y > highest {
                    highest = y;
                }
            }
        }
        total += 1;
        if (highest + 1 - top).abs() <= 1 {
            ok += 1;
        }
    }
    assert!(ok * 100 >= total * 99, "{ok}/{total} columns agree");
}

fn is_feature(b: &super::blocks::GenBlocks, s: BlockStateId) -> bool {
    [&b.oak, &b.birch, &b.spruce]
        .iter()
        .any(|w| s == w.log_y || s == w.log_x || s == w.log_z || w.leaves.contains(&s))
        || s == b.mossy_cobblestone
        || s == b.cobblestone
        || s == b.andesite
        || s == b.cactus
        || s == b.pumpkin
}

#[test]
fn fast_paths_cover_most_of_a_column() {
    let g = generator();
    let t = terrain();
    let (sx, sz) = t.find_spawn(false);
    let (mut fast, mut total) = (0, 0);
    for cy in -200..200 {
        total += 1;
        if g.classify(CubePos::new(sx >> 4, cy, sz >> 4)) != CubeClass::Surface {
            fast += 1;
        }
    }
    assert!(
        fast * 100 >= total * 95,
        "{fast}/{total} cubes on fast paths"
    );
    // Far above everything: uniform air; far below: rock without surface evaluation.
    let high = g.generate_cube(CubePos::new(sx >> 4, 5000, sz >> 4));
    assert!(high.blocks.is_uniform(BlockStateId::AIR));
    let deep = g.generate_cube(CubePos::new(sx >> 4, -3000, sz >> 4));
    assert!(deep.non_air_count() > 3000);
}

#[test]
fn seam_cubes_match_their_wrapped_twins() {
    let g = generator();
    let t = terrain();
    let n = t.planet().cubes_around();
    let z = 0;
    for cy in -3..3 {
        let a = g.generate_cube(CubePos::new(-1, cy, z));
        let b = g.generate_cube(CubePos::new(n - 1, cy, z));
        assert!(same(&a, &b), "cube -1 vs {} at y {cy}", n - 1);
    }
    // Terrain is continuous across the seam like anywhere else.
    let c = t.planet().circumference();
    for z in (-3000..3000).step_by(500) {
        let across = (t.sample(c - 1, z).height - t.sample(0, z).height).abs();
        let inside = (t.sample(100, z).height - t.sample(101, z).height).abs();
        assert!(across < inside + 12.0, "seam jump {across} at z {z}");
    }
}

#[test]
fn giant_caverns_are_rare_but_present() {
    let g = generator();
    let mut found = 0;
    let total = 20 * 20;
    for rz in -10..10 {
        for rx in 0..20 {
            if g.has_giant_cavern(rx, rz) {
                found += 1;
            }
        }
    }
    let frac = found as f64 / total as f64;
    // Rare: roughly one per several square kilometres (a region is ~1 km²).
    assert!((0.18..=0.38).contains(&frac), "cavern fraction {frac:.2}");
}

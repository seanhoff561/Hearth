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
        };
        Arc::new(Terrain::new(Arc::new(PlanetGrid::build(&s, &|_, _| {}))))
    })
    .clone()
}

fn content() -> &'static hearth_content::Content {
    static C: OnceLock<hearth_content::Content> = OnceLock::new();
    C.get_or_init(hearth_content::Content::load_base)
}

fn generator() -> WorldGenerator {
    WorldGenerator::new(terrain(), registry(), content()).expect("all generator blocks exist")
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
        || s == b.cactus
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

/// Fill (Amendment S §2.2): surface cubes carry the ground's depth from the continuous terrain,
/// regenerated the same each time; it agrees with every voxel's state; and the surface it
/// gives lies at the terrain's continuous height to within a few centimetres.
#[test]
fn surface_cubes_carry_the_grounds_fill() {
    use hearth_world::Fill;
    let reg = registry();
    let g = generator();
    let t = terrain();
    let mut with_fill = 0;
    let mut checked = 0;
    let mut worst = 0.0f32;
    for pos in sample_cubes() {
        let c = g.generate_cube(pos);
        assert_eq!(c.fill(), g.generate_cube(pos).fill(), "regenerated alike");
        let Some(_) = c.fill() else {
            continue;
        };
        with_fill += 1;
        for i in 0..CUBE_VOLUME {
            let natural = reg.has(c.get_index(i), StateFlags::NATURAL);
            assert_eq!(c.fill_at(reg, i) > 0, natural, "{pos:?} voxel {i}");
        }
        // Down each column: where the fill crosses zero against the terrain's height, on open
        // gentle ground with natural ground at the top.
        let o = pos.min_block();
        for lz in 0..16 {
            for lx in 0..16 {
                let s = t.sample(o.x + lx, o.z + lz);
                if s.slope > 0.3 || s.cliffiness > 0.0 {
                    continue;
                }
                for ly in 0..15usize {
                    let lo = LocalPos::new(lx as u8, ly as u8, lz as u8).index();
                    let hi = LocalPos::new(lx as u8, ly as u8 + 1, lz as u8).index();
                    let (a, b) = (c.fill_at(reg, lo), c.fill_at(reg, hi));
                    let natural = reg.has(c.get_index(lo), StateFlags::NATURAL);
                    if !(natural && a > 0 && b < 0 && a < 127 && b > -127) {
                        continue;
                    }
                    let (da, db) = (Fill::depth(a), Fill::depth(b));
                    let y = o.y as f32 + ly as f32 + 0.5 + da / (da - db);
                    worst = worst.max((y - s.height).abs());
                    checked += 1;
                }
            }
        }
    }
    assert!(with_fill > 10, "{with_fill} cubes with fill");
    assert!(checked > 100, "{checked} columns checked");
    assert!(
        worst < 0.06,
        "the surface is {worst} m from the terrain's height"
    );
}

/// A sink recording every block a tree puts, within generous bounds.
struct Record {
    lo: [i32; 3],
    hi: [i32; 3],
    put: Vec<(i32, i32, i32, BlockStateId)>,
}

impl super::features::TreeSink for Record {
    fn bounds(&self) -> ([i32; 3], [i32; 3]) {
        (self.lo, self.hi)
    }
    fn get(&self, _x: i32, _y: i32, _z: i32) -> Option<BlockStateId> {
        Some(BlockStateId::AIR)
    }
    fn put(&mut self, x: i32, y: i32, z: i32, s: BlockStateId) {
        self.put.push((x, y, z, s));
    }
}

#[test]
fn old_trees_grow_the_same_anywhere_on_the_planet() {
    // E4.1 §4.5: a tree is built about its foot, so one forty million blocks east (an Earth's
    // breadth, where f32 holds only every fourth block) is the one grown near the origin: its
    // trunk, branches and roots block for block.
    use super::features::{FeatureGen, TreeKind};
    let g = generator();
    let s = terrain().sample(0, 0);
    let earth = FeatureGen::new(5, PlanetSize::Earth.circumference());
    for kind in [
        TreeKind::BigOak,
        TreeKind::GiantOak,
        TreeKind::Mangrove,
        TreeKind::GiantSpruce,
        TreeKind::SavannaOak,
    ] {
        let wood = g.blocks.wood(kind.wood());
        let grown = |x0: i32| {
            let mut sink = Record {
                lo: [x0 - 48, -64, -48],
                hi: [x0 + 48, 256, 48],
                put: Vec::new(),
            };
            let mut rng = hearth_math::hash::Rng::new(11);
            earth.grow(&mut sink, &g.blocks, kind, x0, 64, 0, &mut rng, &s);
            let mut v: Vec<(i32, i32, i32, BlockStateId)> = sink
                .put
                .iter()
                .filter(|p| !wood.leaves.contains(&p.3) && !g.blocks.vines.contains(&p.3))
                .map(|p| (p.0 - x0, p.1, p.2, p.3))
                .collect();
            v.sort_by_key(|p| (p.0, p.1, p.2, p.3.0));
            v.dedup();
            v
        };
        let near = grown(1000);
        let far = grown(39_999_000);
        assert!(near.len() > 3, "{kind:?}: {} blocks of wood", near.len());
        assert_eq!(near, far, "{kind:?} grown far east is not the same tree");
    }
}

#[test]
fn trees_are_the_same_from_either_side_of_the_seam() {
    // E4.1 §4.5: the tree cells are the planet's own (the last one around wider by what is left
    // over), so a strip across the seam holds the same trees seen from the east (x about 0) and
    // from the west (x about C).
    let g = generator();
    let c = g.planet().circumference();
    let veg = crate::vegetation::Vegetation::default();
    let key = |t: &super::features::PlacedTree| {
        (
            t.foot[0].rem_euclid(c),
            t.foot[1],
            t.foot[2],
            t.species,
            format!("{:?} {:?}", t.stage, t.remains),
        )
    };
    let mut seen = 0;
    for z0 in (-c / 2..c / 2).step_by(1024) {
        let east = g.features().trees_in(&g, &veg, (-24, z0), (24, z0 + 300));
        let west = g
            .features()
            .trees_in(&g, &veg, (c - 24, z0), (c + 24, z0 + 300));
        let mut e: Vec<_> = east.iter().map(key).collect();
        let mut w: Vec<_> = west.iter().map(key).collect();
        e.sort();
        w.sort();
        assert_eq!(e, w, "the trees across the seam at z {z0}");
        seen += e.len();
    }
    assert!(seen > 0, "no trees along the seam to compare");
}

#[test]
fn the_features_noises_and_caverns_wrap_with_the_planet() {
    let g = generator();
    let c = g.planet().circumference();
    let f = g.features();
    for (x, y, z) in [
        (0, 40, 0),
        (17, 63, -5000),
        (c - 1, 12, 9000),
        (c / 3, -40, 1234),
    ] {
        let (a, b) = (f.cliff_noise(x, y, z), f.cliff_noise(x + c, y, z));
        assert!((a - b).abs() < 1e-4, "cliffs at x {x}: {a} and {b}");
        let (a, b) = (f.stand_age(x, z), f.stand_age(x + c, z));
        assert!((a - b).abs() < 1e-3, "stands at x {x}: {a} and {b}");
    }
    // A cavern region is the same from either side of the seam.
    let around = c / 1024;
    for rz in -12..12 {
        assert_eq!(
            g.caves.has_cavern(-1, rz, &g),
            g.caves.has_cavern(around - 1, rz, &g),
            "the cavern region west of the seam at row {rz}"
        );
    }
}

#[test]
fn rock_beds_run_on_across_the_seam() {
    // E4.1 §4.5: folds whole around the planet, domes from noise periodic in it: the top of the
    // beds steps across the seam no more than between neighbours anywhere.
    let g = generator();
    let c = g.planet().circumference();
    let mut compared = 0;
    for z in (-c / 2 + 64..c / 2 - 64).step_by(256) {
        let (west, east) = (g.geology.column(c - 1, z), g.geology.column(0, z));
        if west.province != east.province {
            continue;
        }
        compared += 1;
        let step = (west.top - east.top).abs();
        assert!(
            step < 4.0,
            "the beds step {step:.1} blocks across the seam at z {z}"
        );
    }
    assert!(compared > 50, "{compared} columns compared");
}

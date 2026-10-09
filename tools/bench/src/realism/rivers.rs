//! `bench realism rivers`: a river at the blocks from above (G1a, D299). Shaded relief two metres
//! a pixel, the water the sampler carves in blue, and in red the straight chords between the same
//! nodes: the course the blocks drew before rivers were curves.

use std::path::Path;

use hearth_worldgen::Terrain;
use hearth_worldgen::region::biome::Biome;
use rayon::prelude::*;

use super::dem::hillshade;
use super::metrics::Dem;

/// Pixels a side and blocks a pixel.
const SIDE: usize = 1024;
const STEP: f64 = 2.0;
/// Columns read together (one neighbourhood each).
const BLOCK: usize = 64;

pub fn run(t: &Terrain, out: &Path) -> anyhow::Result<()> {
    let relief = t
        .relief()
        .ok_or_else(|| anyhow::anyhow!("a planet without levels"))?;
    let (x, z) = t
        .find_biome(Biome::TemperatePlains, 20_000.0, |_| true)
        .ok_or_else(|| anyhow::anyhow!("no temperate plains"))?;
    // The largest river within some kilometres of the plains' heart, at the finest level.
    let finest = relief.levels().len();
    let (x, z) = (x as f64, z as f64);
    let patch = relief.patch(
        finest,
        (x - 3000.0, z - 3000.0),
        (x + 3000.0, z + 3000.0),
        1,
    );
    let reaches = relief.reaches(&patch, 0.3);
    let river = reaches
        .iter()
        .max_by(|a, b| a.q_a.total_cmp(&b.q_a))
        .ok_or_else(|| anyhow::anyhow!("no river about the plains"))?;
    let (cx, cz) = ((river.a.0 + river.b.0) * 0.5, (river.a.1 + river.b.1) * 0.5);
    println!("a river of {:.1} m³/s at {cx:.0}, {cz:.0}", river.q_a);
    let half = SIDE as f64 * STEP * 0.5;
    let (x0, z0) = (cx - half, cz - half);
    // The surface (water where there is water) and where the water is.
    let blocks: Vec<(usize, usize)> = (0..SIDE / BLOCK)
        .flat_map(|bj| (0..SIDE / BLOCK).map(move |bi| (bi, bj)))
        .collect();
    let done: Vec<(usize, usize, Vec<(f32, bool)>)> = blocks
        .par_iter()
        .map(|&(bi, bj)| {
            let at =
                |i: usize, j: usize| ((x0 + i as f64 * STEP) as i32, (z0 + j as f64 * STEP) as i32);
            let (xa, za) = at(bi * BLOCK, bj * BLOCK);
            let (xb, zb) = at(bi * BLOCK + BLOCK - 1, bj * BLOCK + BLOCK - 1);
            let near = t.nearby(xa, za, xb, zb);
            let mut v = Vec::with_capacity(BLOCK * BLOCK);
            for j in bj * BLOCK..(bj + 1) * BLOCK {
                for i in bi * BLOCK..(bi + 1) * BLOCK {
                    let (px, pz) = at(i, j);
                    let s = t.sample_with(px, pz, &near);
                    v.push((s.height.max(s.water), s.water > s.height));
                }
            }
            (bi, bj, v)
        })
        .collect();
    let mut h = vec![0.0f32; SIDE * SIDE];
    let mut wet = vec![false; SIDE * SIDE];
    for (bi, bj, v) in done {
        for (k, (hk, w)) in v.into_iter().enumerate() {
            let (i, j) = (bi * BLOCK + k % BLOCK, bj * BLOCK + k / BLOCK);
            h[j * SIDE + i] = hk;
            wet[j * SIDE + i] = w;
        }
    }
    let mut img = hillshade(&Dem {
        n: SIDE,
        dx: STEP,
        h,
    });
    for (k, &w) in wet.iter().enumerate() {
        if w {
            img.set(k % SIDE, k / SIDE, [40, 90, 200]);
        }
    }
    // The chords between the same reaches' nodes.
    let pixel = |p: (f64, f64)| ((p.0 - x0) / STEP, (p.1 - z0) / STEP);
    for r in &reaches {
        let (a, b) = (pixel(r.a), pixel(r.b));
        let steps = ((b.0 - a.0).hypot(b.1 - a.1)).ceil().max(1.0) as usize;
        for s in 0..=steps {
            let f = s as f64 / steps as f64;
            let (px, py) = (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f);
            if (0.0..SIDE as f64).contains(&px) && (0.0..SIDE as f64).contains(&py) {
                img.set(px as usize, py as usize, [220, 40, 40]);
            }
        }
    }
    img.save(out)?;
    println!("wrote {}", out.display());
    Ok(())
}

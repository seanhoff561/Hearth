//! Distance and feature transforms on the planet grid (jump flooding with great-circle
//! distances), plus simple grid filters.

use rayon::prelude::*;

use super::grid::GridGeom;

/// Sentinel for "no seed".
pub const NONE: u32 = u32::MAX;

/// For every cell, the index of the nearest seed cell by great-circle distance (jump flooding,
/// 1+JFA). Returns `NONE` everywhere if there are no seeds.
pub fn nearest_seed(geom: &GridGeom, is_seed: &[bool]) -> Vec<u32> {
    let n = geom.n;
    let mut cur: Vec<u32> = is_seed
        .iter()
        .enumerate()
        .map(|(i, s)| if *s { i as u32 } else { NONE })
        .collect();
    if cur.iter().all(|v| *v == NONE) {
        return cur;
    }
    let mut next = cur.clone();
    let mut steps = Vec::new();
    let mut k = n / 2;
    while k >= 1 {
        steps.push(k);
        k /= 2;
    }
    steps.push(1); // the extra "+1" pass cleans up JFA errors
    for &step in &steps {
        next.par_chunks_mut(n).enumerate().for_each(|(j, row)| {
            for (i, out) in row.iter_mut().enumerate() {
                let p = geom.sphere(i, j);
                let mut best = cur[j * n + i];
                let mut best_dot = if best == NONE {
                    -2.0
                } else {
                    let (si, sj) = geom.ij(best as usize);
                    p.dot(geom.sphere(si, sj))
                };
                for dj in [-(step as isize), 0, step as isize] {
                    let jj = j as isize + dj;
                    if jj < 0 || jj >= n as isize {
                        continue;
                    }
                    for di in [-(step as isize), 0, step as isize] {
                        if di == 0 && dj == 0 {
                            continue;
                        }
                        let ii = geom.wrap_i(i as isize + di);
                        let cand = cur[jj as usize * n + ii];
                        if cand == NONE || cand == best {
                            continue;
                        }
                        let (si, sj) = geom.ij(cand as usize);
                        let d = p.dot(geom.sphere(si, sj));
                        if d > best_dot {
                            best_dot = d;
                            best = cand;
                        }
                    }
                }
                *out = best;
            }
        });
        std::mem::swap(&mut cur, &mut next);
    }
    cur
}

/// Great-circle distance (radians) from every cell to its nearest seed (`INFINITY` if none).
pub fn distance_to(geom: &GridGeom, nearest: &[u32]) -> Vec<f32> {
    let n = geom.n;
    (0..geom.len())
        .into_par_iter()
        .map(|idx| {
            let s = nearest[idx];
            if s == NONE {
                return f32::INFINITY;
            }
            let (i, j) = (idx % n, idx / n);
            let (si, sj) = geom.ij(s as usize);
            geom.sphere(i, j)
                .dot(geom.sphere(si, sj))
                .clamp(-1.0, 1.0)
                .acos() as f32
        })
        .collect()
}

/// Cells whose 4-neighbourhood contains a different class (both sides of every edge).
pub fn class_edges<T: PartialEq + Copy + Sync>(geom: &GridGeom, class: &[T]) -> Vec<bool> {
    let n = geom.n;
    (0..geom.len())
        .into_par_iter()
        .map(|idx| {
            let (i, j) = (idx % n, idx / n);
            let c = class[idx];
            [(-1isize, 0isize), (1, 0), (0, -1), (0, 1)]
                .iter()
                .any(|&(di, dj)| geom.neighbor(i, j, di, dj).is_some_and(|nb| class[nb] != c))
        })
        .collect()
}

/// Separable box blur with X wrap (radius in cells), repeated `passes` times (≈ Gaussian).
pub fn blur(geom: &GridGeom, data: &mut [f32], radius: usize, passes: usize) {
    if radius == 0 {
        return;
    }
    let n = geom.n;
    let mut tmp = vec![0f32; data.len()];
    let norm = 1.0 / (2 * radius + 1) as f32;
    for _ in 0..passes {
        // Horizontal (wrapping).
        tmp.par_chunks_mut(n)
            .zip(data.par_chunks(n))
            .for_each(|(out, row)| {
                let mut sum: f32 = 0.0;
                for k in 0..=2 * radius {
                    sum += row[(k + n - radius) % n];
                }
                for i in 0..n {
                    out[i] = sum * norm;
                    sum += row[(i + radius + 1) % n] - row[(i + n - radius) % n];
                }
            });
        // Vertical (clamped).
        let src = &tmp;
        data.par_chunks_mut(n).enumerate().for_each(|(j, out)| {
            for (i, o) in out.iter_mut().enumerate() {
                let mut sum = 0.0;
                for dj in -(radius as isize)..=radius as isize {
                    let jj = (j as isize + dj).clamp(0, n as isize - 1) as usize;
                    sum += src[jj * n + i];
                }
                *o = sum * norm;
            }
        });
    }
}

/// Area weight of each row (cos² φ: Mercator cells shrink toward the poles).
pub fn row_area_weights(geom: &GridGeom) -> Vec<f64> {
    geom.cos_lat.iter().map(|c| c * c).collect()
}

/// Area-weighted quantile of `values` (q in 0..1). Rows are weighted by cos² φ.
pub fn weighted_quantile(geom: &GridGeom, values: &[f32], q: f64) -> f32 {
    let w = row_area_weights(geom);
    let n = geom.n;
    // Subsample for speed: every other cell is plenty for a quantile.
    let mut pairs: Vec<(f32, f64)> = Vec::with_capacity(values.len() / 4 + 1);
    for j in (0..n).step_by(2) {
        for i in (0..n).step_by(2) {
            pairs.push((values[j * n + i], w[j]));
        }
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = pairs.iter().map(|p| p.1).sum();
    let target = total * q;
    let mut acc = 0.0;
    for (v, wt) in &pairs {
        acc += wt;
        if acc >= target {
            return *v;
        }
    }
    pairs.last().map_or(0.0, |p| p.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hearth_math::{Planet, PlanetSize};

    #[test]
    fn jfa_matches_brute_force_mostly() {
        let g = GridGeom::new(Planet::from_size(PlanetSize::Tiny).unwrap(), 64);
        let mut seeds = vec![false; g.len()];
        for idx in [g.idx(5, 30), g.idx(40, 20), g.idx(63, 33), g.idx(20, 50)] {
            seeds[idx] = true;
        }
        let near = nearest_seed(&g, &seeds);
        let seed_list: Vec<usize> = (0..g.len()).filter(|i| seeds[*i]).collect();
        let mut wrong = 0;
        for j in 4..60 {
            for i in 0..64 {
                let p = g.sphere(i, j);
                let best = seed_list
                    .iter()
                    .max_by(|a, b| {
                        let (ai, aj) = g.ij(**a);
                        let (bi, bj) = g.ij(**b);
                        p.dot(g.sphere(ai, aj)).total_cmp(&p.dot(g.sphere(bi, bj)))
                    })
                    .unwrap();
                if near[g.idx(i, j)] as usize != *best {
                    wrong += 1;
                }
            }
        }
        assert!(wrong < 20, "{wrong} mismatches");
        // Wrap: cell (0, 33) is nearest to the seed at (63, 33).
        assert_eq!(near[g.idx(0, 33)] as usize, g.idx(63, 33));
    }

    #[test]
    fn blur_preserves_mean() {
        let g = GridGeom::new(Planet::from_size(PlanetSize::Tiny).unwrap(), 32);
        let mut d: Vec<f32> = (0..g.len()).map(|i| (i % 7) as f32).collect();
        let before: f32 = d.iter().sum::<f32>() / d.len() as f32;
        blur(&g, &mut d, 2, 2);
        let after: f32 = d.iter().sum::<f32>() / d.len() as f32;
        assert!((before - after).abs() < 0.2);
    }
}

//! Dissection (G1b): relief whose ridges and valleys run down the slope they lie on.
//!
//! Running water cuts a sloping surface into valleys that run down it, and the valleys' sides
//! into smaller ones that run down them into the valley: a drainage net's branching (Horton
//! 1945; Howard 1994). A level's relief of isotropic noise is bumps without that grain, which the
//! level's few rounds of erosion can only nibble at. Here an octave of a level's relief is laid
//! as stripes across the slope beneath it (the parent's surface with the longer octaves already
//! on it), so that its ridges and valleys run downhill, and a finer octave's valleys run down the
//! sides of a coarser one's and join it. Where the ground is too gentle for its slope to lead
//! the water at an octave's scale, the octave stays isotropic. The level's drainage, erosion and
//! creep then work on relief already shaped as water shapes it.
//!
//! An octave is a sum of jittered kernels with a direction, as Gabor noise is (Lagae et al.
//! 2009): about each point of a jittered lattice, a cosine across the slope under a bump that
//! falls to nothing a lattice cell away (so the nine nearest points are all that reach a place,
//! and the octave is smooth across the cells).

use std::f64::consts::TAU;

use hearth_math::hash::{hash_2d, hash2, unit_f64};

/// Scales an octave to the spread of the isotropic noise it stands in for (a Perlin octave's).
pub(crate) const GAIN: f64 = 0.56;

/// One octave at `p` (in its lattice's cells; `cells` of them round the planet, so it is whole
/// at the seam), its stripes running `along` (a unit vector: down the slope): its value (about
/// −1 … 1, before [`GAIN`]) and its gradient (per lattice cell).
pub(crate) fn octave(seed: u64, cells: i64, p: (f64, f64), along: (f64, f64)) -> (f64, (f64, f64)) {
    let across = (-along.1, along.0);
    let (iu, iv) = (p.0.floor() as i64, p.1.floor() as i64);
    let (mut h, mut gu, mut gv, mut wt) = (0.0, 0.0, 0.0, 1e-9);
    // How far `v` lies outside where a point of the cell from `c` may stand (its centre ±0.4).
    let off = |v: f64, c: i64| (c as f64 + 0.1 - v).max(v - (c as f64 + 0.9)).max(0.0);
    let first = (iu - 1).rem_euclid(cells);
    for cj in iv - 1..=iv + 1 {
        let oz = off(p.1, cj);
        for (k, ci) in (iu - 1..=iu + 1).enumerate() {
            // A point that cannot come within reach is not drawn (its bump would be nought).
            let ox = off(p.0, ci);
            if ox * ox + oz * oz >= 1.0 + 1e-9 {
                continue;
            }
            let wrapped = (first + k as i64) % cells;
            let hsh = hash_2d(seed, wrapped as i32, cj as i32);
            let jx = unit_f64(hsh) - 0.5;
            let jz = unit_f64(hash2(hsh, 1)) - 0.5;
            let du = p.0 - (ci as f64 + 0.5 + 0.8 * jx);
            let dv = p.1 - (cj as f64 + 0.5 + 0.8 * jz);
            let d2 = du * du + dv * dv;
            if d2 >= 1.0 {
                continue;
            }
            let w = (1.0 - d2) * (1.0 - d2);
            let (s, c) = (TAU * (du * across.0 + dv * across.1)).sin_cos();
            h += w * c;
            gu -= w * s * TAU * across.0;
            gv -= w * s * TAU * across.1;
            wt += w;
        }
    }
    (h / wt, (gu / wt, gv / wt))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ground's height along a line across and down the stripes.
    fn line(along: (f64, f64), dir: (f64, f64)) -> Vec<f64> {
        (0..4000)
            .map(|k| {
                let t = k as f64 * 0.01;
                octave(7, 1 << 20, (100.0 + dir.0 * t, 50.0 + dir.1 * t), along).0
            })
            .collect()
    }

    fn mean_step(v: &[f64]) -> f64 {
        v.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f64>() / (v.len() - 1) as f64
    }

    #[test]
    fn ridges_and_valleys_run_down_the_slope() {
        let along = (0.6, 0.8);
        let across = (-0.8, 0.6);
        let down = mean_step(&line(along, along));
        let side = mean_step(&line(along, across));
        assert!(side > 3.0 * down, "across {side}, along {down}");
    }

    #[test]
    fn an_octave_spreads_as_a_perlin_octave_does() {
        let v: Vec<f64> = (0..200_000)
            .map(|k| {
                let p = ((k % 400) as f64 * 0.37, (k / 400) as f64 * 0.41);
                octave(11, 1 << 20, p, (1.0, 0.0)).0 * GAIN
            })
            .collect();
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        let sd = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
        let noise = crate::noise::BlockFbm::new(3, 1 << 30, 1.0, 1, 0.5);
        let p: Vec<f64> = (0..200_000)
            .map(|k| noise.sample2((k % 400) as f64 * 0.37, (k / 400) as f64 * 0.41))
            .collect();
        let pm = p.iter().sum::<f64>() / p.len() as f64;
        let psd = (p.iter().map(|x| (x - pm).powi(2)).sum::<f64>() / p.len() as f64).sqrt();
        assert!(mean.abs() < 0.05, "mean {mean}");
        assert!(
            (sd / psd - 1.0).abs() < 0.15,
            "an octave's spread {sd}, a Perlin octave's {psd}"
        );
    }

    #[test]
    fn whole_at_the_seam() {
        let cells = 1000;
        for k in 0..50 {
            let z = k as f64 * 0.31;
            let a = octave(5, cells, (cells as f64 - 0.25, z), (0.0, 1.0)).0;
            let b = octave(5, cells, (-0.25, z), (0.0, 1.0)).0;
            assert!((a - b).abs() < 1e-9);
        }
    }
}

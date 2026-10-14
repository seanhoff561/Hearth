//! Where natural individuals stand (Amendment P §11.1): at real-valued places and turns, never one
//! to a block, spaced by competition, clustered by how their seeds travel and in clonal patches
//! where they spread from their roots. Every answer depends on the seed and the place alone, so
//! any part of the world, made in any order, finds the same individuals (P §11.4).
//!
//! - **Sites**: a candidate in each cell of a lattice, anywhere in it (a jittered lattice finer
//!   than any species stands, so that the thinning, not the lattice, shapes the pattern).
//! - **Spacing**: a candidate stands unless another outranking it stands within reach of it,
//!   their crowns' (Matérn 1960's second hard-core process: competition's self-thinning, the
//!   survivors spaced more evenly than chance would place them).
//! - **Clusters** (Thomas 1949): offspring scattered about parents in a normal kernel as wide as
//!   their seeds travel (birds' droppings, a fruit's fall), the parents a sparse lattice.
//! - **Patches**: a clone's extent about a centre, as wide as it spreads, its edge wandering.
//!
//! Positions are blocks (metres on Earth) and wrap round the planet in x (`round` cells).

use hearth_math::hash::{hash2, mix64, unit_f64};

/// A candidate site: where it is (unwrapped, about the cells asked for) and its rank (the
/// higher stands where two compete).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Site {
    pub x: f64,
    pub z: f64,
    pub rank: u64,
    /// A hash of its own for what else it draws (species, turn, size).
    pub hash: u64,
}

/// A jittered lattice of candidate sites, periodic round the planet in x.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lattice {
    seed: u64,
    cell: f64,
    /// Cells round the planet.
    round: i64,
}

impl Lattice {
    /// A lattice of cells of about `cell` blocks, a whole number of them round a planet of
    /// circumference `c` blocks.
    pub fn new(seed: u64, cell: f64, c: f64) -> Self {
        let round = ((c / cell).round() as i64).max(1);
        Self {
            seed,
            cell: c / round as f64,
            round,
        }
    }

    pub fn cell(&self) -> f64 {
        self.cell
    }

    /// The site in cell (i, j) (i unwrapped: the site lies in that cell's span of x).
    pub fn site(&self, i: i64, j: i64) -> Site {
        let h = hash2(hash2(self.seed, i.rem_euclid(self.round) as u64), j as u64);
        let (u, v) = (unit_f64(h), unit_f64(mix64(h ^ 0x5157)));
        Site {
            x: (i as f64 + u) * self.cell,
            z: (j as f64 + v) * self.cell,
            rank: mix64(h ^ 0xa11c),
            hash: mix64(h ^ 0x0b5e),
        }
    }

    /// The sites of the cells meeting the rectangle [x0, x1] × [z0, z1] (blocks).
    pub fn sites(
        &self,
        (x0, z0): (f64, f64),
        (x1, z1): (f64, f64),
    ) -> impl Iterator<Item = Site> + '_ {
        let (i0, i1) = (
            (x0 / self.cell).floor() as i64,
            (x1 / self.cell).floor() as i64,
        );
        let (j0, j1) = (
            (z0 / self.cell).floor() as i64,
            (z1 / self.cell).floor() as i64,
        );
        (j0..=j1).flat_map(move |j| (i0..=i1).map(move |i| self.site(i, j)))
    }
}

/// Which of `cands` stand: each unless another outranks it within `reach(it, other)` blocks of it
/// (Matérn's second process). `reach` is at most `max_reach`; the answer is right for every
/// candidate whose neighbours within `max_reach` are all in `cands`.
pub fn thin<T>(
    cands: &[T],
    at: impl Fn(&T) -> (f64, f64),
    rank: impl Fn(&T) -> u64,
    reach: impl Fn(&T, &T) -> f64,
    max_reach: f64,
) -> Vec<bool> {
    if cands.is_empty() {
        return Vec::new();
    }
    // A bucket grid of the reach's size: a candidate's rivals lie in the 3 × 3 about its own.
    let size = max_reach.max(1e-6);
    let key = |p: (f64, f64)| ((p.0 / size).floor() as i64, (p.1 / size).floor() as i64);
    let mut buckets: rustc_hash::FxHashMap<(i64, i64), Vec<usize>> = Default::default();
    for (k, c) in cands.iter().enumerate() {
        buckets.entry(key(at(c))).or_default().push(k);
    }
    cands
        .iter()
        .map(|c| {
            let p = at(c);
            let (bx, bz) = key(p);
            let r = rank(c);
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let Some(list) = buckets.get(&(bx + dx, bz + dz)) else {
                        continue;
                    };
                    for &o in list {
                        let other = &cands[o];
                        let ro = rank(other);
                        if ro <= r {
                            continue;
                        }
                        let q = at(other);
                        let d = reach(c, other);
                        if (p.0 - q.0).powi(2) + (p.1 - q.1).powi(2) < d * d {
                            return false;
                        }
                    }
                }
            }
            true
        })
        .collect()
}

/// How crowded with a cluster process's offspring a place is, as a share of the mean (1 on
/// average): parents one to each cell of `spacing` blocks (a jittered lattice of its own,
/// `seed`), each scattering its offspring in a normal kernel of `sigma` blocks (Thomas 1949).
/// Clusters are tight where `sigma` is small beside `spacing`, and the ground between them bare.
pub fn crowding(seed: u64, x: f64, z: f64, spacing: f64, sigma: f64, c: f64) -> f64 {
    let parents = Lattice::new(seed, spacing, c);
    let s = parents.cell();
    let reach = 3.0 * sigma;
    let mut sum = 0.0;
    for p in parents.sites((x - reach, z - reach), (x + reach, z + reach)) {
        let d2 = (p.x - x).powi(2) + (p.z - z).powi(2);
        sum += (-d2 / (2.0 * sigma * sigma)).exp();
    }
    // Each parent's kernel holds 2πσ² of area; one parent to each s² of ground.
    sum * s * s / (std::f64::consts::TAU * sigma * sigma)
}

/// How far within a clone's patch a place lies (0 outside, rising to 1 at its centre): patches
/// about centres on a lattice of `1.6 × width` blocks, each there with probability `share`, of
/// mean width `width`, their edges wandering by about a fifth.
pub fn patch(seed: u64, x: f64, z: f64, width: f64, share: f64, c: f64) -> f64 {
    if share <= 0.0 || width <= 0.0 {
        return 0.0;
    }
    let centres = Lattice::new(seed, 1.6 * width, c);
    let reach = 0.75 * width;
    let mut best: f64 = 0.0;
    for p in centres.sites((x - reach, z - reach), (x + reach, z + reach)) {
        if unit_f64(p.hash) >= share {
            continue;
        }
        let (dx, dz) = (x - p.x, z - p.z);
        let d = dx.hypot(dz);
        if d >= reach {
            continue;
        }
        // The edge: the mean radius wandering with the bearing (three lobes' worth of waves).
        let a = dz.atan2(dx);
        let mut wobble = 0.0;
        for k in 2..5u64 {
            let h = mix64(p.hash ^ (k * 0x9e37_79b9));
            let phase = unit_f64(h) * std::f64::consts::TAU;
            let amp = 0.08 + 0.12 * unit_f64(mix64(h));
            wobble += amp * (k as f64 * a + phase).cos();
        }
        let radius = 0.5 * width * (1.0 + wobble);
        best = best.max(1.0 - d / radius);
    }
    best.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const C: f64 = 4.0e7;

    /// Mean distance to the nearest other point, against what a random scatter of the same
    /// density gives (Clark and Evans 1954): 1 random, above it regular, below clustered.
    fn clark_evans(pts: &[(f64, f64)], area: f64) -> f64 {
        let n = pts.len() as f64;
        let mean: f64 = pts
            .iter()
            .enumerate()
            .map(|(k, a)| {
                pts.iter()
                    .enumerate()
                    .filter(|(o, _)| *o != k)
                    .map(|(_, b)| (a.0 - b.0).hypot(a.1 - b.1))
                    .fold(f64::MAX, f64::min)
            })
            .sum::<f64>()
            / n;
        mean / (0.5 * (area / n).sqrt())
    }

    /// Points in the middle of a window, away from its edges (whose neighbours lie outside).
    fn inner(pts: &[(f64, f64)], lo: f64, hi: f64, margin: f64) -> Vec<(f64, f64)> {
        pts.iter()
            .copied()
            .filter(|p| {
                p.0 > lo + margin && p.0 < hi - margin && p.1 > lo + margin && p.1 < hi - margin
            })
            .collect()
    }

    #[test]
    fn thinned_sites_are_spaced_by_their_reach_and_more_even_than_chance() {
        let lat = Lattice::new(11, 1.0, C);
        let sites: Vec<Site> = lat.sites((0.0, 0.0), (120.0, 120.0)).collect();
        let keep = thin(&sites, |s| (s.x, s.z), |s| s.rank, |_, _| 3.0, 3.0);
        let kept: Vec<(f64, f64)> = sites
            .iter()
            .zip(&keep)
            .filter(|(_, k)| **k)
            .map(|(s, _)| (s.x, s.z))
            .collect();
        for (k, a) in kept.iter().enumerate() {
            for b in &kept[k + 1..] {
                assert!((a.0 - b.0).hypot(a.1 - b.1) >= 3.0, "{a:?} {b:?}");
            }
        }
        let mid = inner(&kept, 0.0, 120.0, 6.0);
        let r = clark_evans(&mid, 108.0 * 108.0);
        assert!(r > 1.4, "Clark-Evans {r}");
    }

    #[test]
    fn clustered_offspring_gather_and_leave_the_ground_between_bare() {
        // Sites kept in proportion to the parents' crowding: tight clusters 20 m apart.
        let lat = Lattice::new(5, 1.0, C);
        let pts: Vec<(f64, f64)> = lat
            .sites((0.0, 0.0), (200.0, 200.0))
            .filter(|s| unit_f64(s.hash) < 0.05 * crowding(9, s.x, s.z, 20.0, 3.0, C))
            .map(|s| (s.x, s.z))
            .collect();
        let mid = inner(&pts, 0.0, 200.0, 10.0);
        let r = clark_evans(&mid, 180.0 * 180.0);
        assert!(r < 0.75, "Clark-Evans {r}");
        // Counts in 10 m squares: far more varied than a random scatter's (variance = mean).
        let mut counts = vec![0f64; 400];
        for p in &pts {
            let (i, j) = ((p.0 / 10.0) as usize, (p.1 / 10.0) as usize);
            counts[j.min(19) * 20 + i.min(19)] += 1.0;
        }
        let mean = counts.iter().sum::<f64>() / 400.0;
        let var = counts.iter().map(|c| (c - mean).powi(2)).sum::<f64>() / 400.0;
        assert!(var > 2.0 * mean, "variance {var}, mean {mean}");
        // And on the whole as dense as the crowding's mean says.
        let expected = 0.05 * 200.0 * 200.0;
        assert!(
            (pts.len() as f64 - expected).abs() < 0.3 * expected,
            "{} against {expected}",
            pts.len()
        );
    }

    #[test]
    fn places_and_turns_are_not_quantized_to_the_blocks() {
        let lat = Lattice::new(3, 2.0, C);
        let sites: Vec<Site> = lat.sites((0.0, 0.0), (400.0, 400.0)).collect();
        // The fractions of a block, and the turns, fall evenly into ten bins each.
        let mut bins = [[0usize; 10]; 3];
        for s in &sites {
            bins[0][(s.x.fract() * 10.0) as usize] += 1;
            bins[1][(s.z.fract() * 10.0) as usize] += 1;
            bins[2][(unit_f64(s.hash) * 10.0) as usize] += 1;
        }
        let n = sites.len() as f64 / 10.0;
        for b in bins {
            let chi2: f64 = b.iter().map(|&c| (c as f64 - n).powi(2) / n).sum();
            // 9 degrees of freedom: 27.9 at p = 0.001.
            assert!(chi2 < 27.9, "{b:?}");
        }
    }

    #[test]
    fn any_part_of_the_world_finds_the_same_individuals() {
        let lat = Lattice::new(17, 1.5, C);
        let pick = |lo: (f64, f64), hi: (f64, f64)| {
            let sites: Vec<Site> = lat
                .sites((lo.0 - 4.0, lo.1 - 4.0), (hi.0 + 4.0, hi.1 + 4.0))
                .collect();
            let keep = thin(&sites, |s| (s.x, s.z), |s| s.rank, |_, _| 4.0, 4.0);
            let mut out: Vec<(i64, i64)> = sites
                .iter()
                .zip(keep)
                .filter(|(s, k)| *k && s.x >= lo.0 && s.x < hi.0 && s.z >= lo.1 && s.z < hi.1)
                .map(|(s, _)| ((s.x * 1e6) as i64, (s.z * 1e6) as i64))
                .collect();
            out.sort();
            out
        };
        let whole = pick((0.0, 0.0), (64.0, 64.0));
        let mut halves = pick((0.0, 0.0), (32.0, 64.0));
        halves.extend(pick((32.0, 0.0), (64.0, 64.0)));
        halves.sort();
        assert_eq!(whole, halves);
        // Round the planet: the cells past the seam are the first ones again.
        let round = lat.round;
        assert_eq!(lat.site(round + 3, 7).hash, lat.site(3, 7).hash);
    }

    #[test]
    fn clonal_patches_cover_their_share_with_wandering_edges() {
        let (width, share) = (8.0, 0.5);
        let mut inside = 0usize;
        let n = 300usize;
        for j in 0..n {
            for i in 0..n {
                if patch(21, i as f64 * 0.5, j as f64 * 0.5, width, share, C) > 0.0 {
                    inside += 1;
                }
            }
        }
        // Each patch π (w/2)² of a cell (1.6 w)², half the cells holding one.
        let expected = share * std::f64::consts::PI * 0.25 / (1.6 * 1.6);
        let got = inside as f64 / (n * n) as f64;
        assert!(
            (got - expected).abs() < 0.35 * expected,
            "{got} against {expected}"
        );
        // An edge wanders: along one patch's rim the radius differs by bearing.
        let lat = Lattice::new(21, 1.6 * width, C);
        let p = (0..50)
            .map(|i| lat.site(i, 3))
            .find(|s| unit_f64(s.hash) < share)
            .expect("a patch");
        let radius = |a: f64| {
            (1..200)
                .map(|k| k as f64 * 0.05)
                .find(|&d| patch(21, p.x + d * a.cos(), p.z + d * a.sin(), width, share, C) <= 0.0)
                .unwrap_or(0.0)
        };
        let radii: Vec<f64> = (0..12).map(|k| radius(k as f64 * 0.52)).collect();
        let (lo, hi) = radii
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, h), &r| (l.min(r), h.max(r)));
        assert!(hi - lo > 0.3, "{radii:?}");
    }
}

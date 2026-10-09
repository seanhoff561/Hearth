//! Terrain statistics on a square height grid, the same for the generator's surface and a real
//! elevation model (T §3.3): slopes, curvature, roughness across scales, closed depressions,
//! drainage and the slope–area relation, hypsometry.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// A square grid of heights (metres), `n` × `n` samples `dx` metres apart, row by row; NaN where
/// nothing is known (no data, the sea).
#[derive(Clone)]
pub struct Dem {
    pub n: usize,
    pub dx: f64,
    pub h: Vec<f32>,
}

impl Dem {
    #[inline]
    pub fn get(&self, i: usize, j: usize) -> f32 {
        self.h[j * self.n + i]
    }

    /// The share of samples known.
    pub fn valid(&self) -> f64 {
        self.h.iter().filter(|h| h.is_finite()).count() as f64 / self.h.len() as f64
    }

    /// Means of `k` × `k` blocks: the same ground `k` times coarser (a block with any unknown
    /// sample is unknown).
    pub fn coarsen(&self, k: usize) -> Dem {
        let n = self.n / k;
        let mut h = vec![f32::NAN; n * n];
        for j in 0..n {
            for i in 0..n {
                let mut s = 0.0f64;
                let mut ok = true;
                'b: for b in 0..k {
                    for a in 0..k {
                        let v = self.get(i * k + a, j * k + b);
                        if !v.is_finite() {
                            ok = false;
                            break 'b;
                        }
                        s += v as f64;
                    }
                }
                if ok {
                    h[j * n + i] = (s / (k * k) as f64) as f32;
                }
            }
        }
        Dem {
            n,
            dx: self.dx * k as f64,
            h,
        }
    }

    /// The highest less the lowest known height.
    pub fn relief(&self) -> f32 {
        let (lo, hi) = self
            .h
            .iter()
            .filter(|h| h.is_finite())
            .fold((f32::MAX, f32::MIN), |(lo, hi), &h| (lo.min(h), hi.max(h)));
        hi - lo
    }
}

/// What the statistics found on one grid.
#[derive(Clone, Debug, Default)]
pub struct Metrics {
    /// Metres between samples, samples along a side.
    pub dx: f64,
    pub n: usize,
    /// Slope (degrees) at the 10th, 50th, 90th and 99th percentiles.
    pub slope: [f32; 4],
    /// Shares of the ground steeper than 30° and 45°.
    pub steep30: f32,
    pub steep45: f32,
    /// Share of the ground flatter than 2°.
    pub flat2: f32,
    /// Cells by whole degree of slope (0–89).
    pub slope_hist: Vec<u32>,
    /// Curvature (the Laplacian, 1/m) at the 5th, 50th and 95th percentiles, and its skewness.
    pub curv: [f32; 3],
    pub curv_skew: f32,
    /// Roughness across scales: (lag in metres, mean |Δh| in metres) along both axes.
    pub structure: Vec<(f64, f64)>,
    /// Local minima (lower than all eight neighbours) per square kilometre.
    pub pits_per_km2: f32,
    /// Shares of the ground in closed depressions deeper than 0.25 m and 1 m.
    pub depressed: [f32; 2],
    /// Drainage density (km of channel per km²) where channels drain at least 10⁴ m² and 10⁵ m².
    pub drainage: [f32; 2],
    /// The slope–area relation: (log10 area in m², median slope m/m, cells) by quarter decade.
    pub slope_area: Vec<(f32, f32, usize)>,
    /// The concavity θ of S ∝ A^-θ over areas of 10⁵ m² and more (NaN with too few bins).
    pub concavity: f32,
    /// The area (m²) where the median slope is greatest: hillslopes turn into valleys there.
    pub turn_area: f32,
    /// The largest channel's sinuosity: its length up from where it leaves the grid to where it
    /// drains less than 10⁶ m², over the straight distance between (NaN if it is shorter).
    pub sinuosity: f32,
    /// The hypsometric integral (mean − lowest) / (highest − lowest).
    pub hypsometric: f32,
    /// The highest less the lowest (m).
    pub relief: f32,
}

fn percentile(sorted: &[f32], p: f64) -> f32 {
    if sorted.is_empty() {
        return f32::NAN;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

const NEIGHBOURS: [(i64, i64); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];

/// A cell waiting in the flood, lowest first.
#[derive(PartialEq)]
struct Wet(f64, usize);

impl Eq for Wet {}

impl PartialOrd for Wet {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl Ord for Wet {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.total_cmp(&self.0).then(o.1.cmp(&self.1))
    }
}

/// The surface with its closed depressions filled, every cell a little above the one it drains
/// to (Priority-Flood with ε, Barnes et al. 2014): water leaves by the grid's edges and by
/// unknown ground.
fn fill(d: &Dem) -> Vec<f64> {
    let n = d.n;
    let mut out = vec![f64::NAN; n * n];
    let mut seen = vec![false; n * n];
    let mut heap = BinaryHeap::new();
    for j in 0..n {
        for i in 0..n {
            let k = j * n + i;
            let h = d.h[k];
            if !h.is_finite() {
                seen[k] = true;
                continue;
            }
            let edge = i == 0
                || j == 0
                || i == n - 1
                || j == n - 1
                || NEIGHBOURS.iter().any(|&(a, b)| {
                    !d.get((i as i64 + a) as usize, (j as i64 + b) as usize)
                        .is_finite()
                });
            if edge {
                seen[k] = true;
                out[k] = h as f64;
                heap.push(Wet(h as f64, k));
            }
        }
    }
    while let Some(Wet(z, k)) = heap.pop() {
        let (i, j) = ((k % n) as i64, (k / n) as i64);
        for &(a, b) in &NEIGHBOURS {
            let (x, y) = (i + a, j + b);
            if x < 0 || y < 0 || x >= n as i64 || y >= n as i64 {
                continue;
            }
            let m = y as usize * n + x as usize;
            if seen[m] {
                continue;
            }
            seen[m] = true;
            let h = (d.h[m] as f64).max(z + 1e-6);
            out[m] = h;
            heap.push(Wet(h, m));
        }
    }
    out
}

/// Every statistic of a grid.
pub fn measure(d: &Dem) -> Metrics {
    let n = d.n;
    let dx = d.dx;
    let mut m = Metrics {
        dx,
        n,
        ..Default::default()
    };

    // Slopes and curvature from central differences where all four neighbours are known.
    let mut slopes = Vec::with_capacity(n * n);
    let mut curvs = Vec::with_capacity(n * n);
    for j in 1..n - 1 {
        for i in 1..n - 1 {
            let c = d.get(i, j);
            let (e, w, s, no) = (
                d.get(i + 1, j),
                d.get(i - 1, j),
                d.get(i, j + 1),
                d.get(i, j - 1),
            );
            if !(c.is_finite() && e.is_finite() && w.is_finite() && s.is_finite() && no.is_finite())
            {
                continue;
            }
            let gx = (e - w) as f64 / (2.0 * dx);
            let gz = (s - no) as f64 / (2.0 * dx);
            slopes.push(gx.hypot(gz).atan().to_degrees() as f32);
            curvs.push(((e + w + s + no - 4.0 * c) as f64 / (dx * dx)) as f32);
        }
    }
    m.slope_hist = vec![0; 90];
    for &s in &slopes {
        m.slope_hist[(s as usize).min(89)] += 1;
    }
    let count = slopes.len().max(1) as f32;
    m.steep30 = slopes.iter().filter(|&&s| s > 30.0).count() as f32 / count;
    m.steep45 = slopes.iter().filter(|&&s| s > 45.0).count() as f32 / count;
    m.flat2 = slopes.iter().filter(|&&s| s < 2.0).count() as f32 / count;
    slopes.sort_by(f32::total_cmp);
    m.slope = [0.1, 0.5, 0.9, 0.99].map(|p| percentile(&slopes, p));
    let mean = curvs.iter().map(|&c| c as f64).sum::<f64>() / curvs.len().max(1) as f64;
    let var = curvs
        .iter()
        .map(|&c| (c as f64 - mean).powi(2))
        .sum::<f64>()
        / curvs.len().max(1) as f64;
    let skew = curvs
        .iter()
        .map(|&c| (c as f64 - mean).powi(3))
        .sum::<f64>()
        / curvs.len().max(1) as f64
        / var.powf(1.5).max(1e-30);
    m.curv_skew = skew as f32;
    curvs.sort_by(f32::total_cmp);
    m.curv = [0.05, 0.5, 0.95].map(|p| percentile(&curvs, p));

    // Roughness across scales: mean |Δh| at lags of 1, 2, 4… samples, along both axes.
    let mut lag = 1;
    while lag <= n / 4 {
        let (mut s, mut c) = (0.0f64, 0usize);
        for j in 0..n {
            for i in 0..n - lag {
                let (a, b) = (d.get(i, j), d.get(i + lag, j));
                if a.is_finite() && b.is_finite() {
                    s += (a - b).abs() as f64;
                    c += 1;
                }
                let (a, b) = (d.get(j, i), d.get(j, i + lag));
                if a.is_finite() && b.is_finite() {
                    s += (a - b).abs() as f64;
                    c += 1;
                }
            }
        }
        m.structure.push((lag as f64 * dx, s / c.max(1) as f64));
        lag *= 2;
    }

    // Local minima, below all eight neighbours.
    let mut pits = 0usize;
    let mut cells = 0usize;
    for j in 1..n - 1 {
        for i in 1..n - 1 {
            let c = d.get(i, j);
            if !c.is_finite() {
                continue;
            }
            cells += 1;
            let lowest = NEIGHBOURS.iter().all(|&(a, b)| {
                let v = d.get((i as i64 + a) as usize, (j as i64 + b) as usize);
                v.is_finite() && v > c
            });
            pits += lowest as usize;
        }
    }
    let km2 = cells as f64 * dx * dx / 1e6;
    m.pits_per_km2 = (pits as f64 / km2.max(1e-9)) as f32;

    // Closed depressions: how deep the flood stands over the ground.
    let filled = fill(d);
    let known = d.h.iter().filter(|h| h.is_finite()).count().max(1) as f32;
    for (t, out) in [0.25, 1.0].iter().zip(m.depressed.iter_mut()) {
        *out =
            d.h.iter()
                .zip(&filled)
                .filter(|(h, f)| h.is_finite() && **f - **h as f64 > *t)
                .count() as f32
                / known;
    }

    // Drainage: each cell to its steepest neighbour down the filled surface, the area above
    // each gathered from the highest down.
    let mut receiver = vec![usize::MAX; n * n];
    let mut dist = vec![0.0f64; n * n];
    for j in 0..n {
        for i in 0..n {
            let k = j * n + i;
            if !filled[k].is_finite() {
                continue;
            }
            let mut best = (0.0f64, usize::MAX, 0.0f64);
            for &(a, b) in &NEIGHBOURS {
                let (x, y) = (i as i64 + a, j as i64 + b);
                if x < 0 || y < 0 || x >= n as i64 || y >= n as i64 {
                    continue;
                }
                let r = y as usize * n + x as usize;
                if !filled[r].is_finite() {
                    continue;
                }
                let l = if a != 0 && b != 0 {
                    dx * 2f64.sqrt()
                } else {
                    dx
                };
                let g = (filled[k] - filled[r]) / l;
                if g > best.0 {
                    best = (g, r, l);
                }
            }
            receiver[k] = best.1;
            dist[k] = best.2;
        }
    }
    let mut order: Vec<usize> = (0..n * n).filter(|&k| filled[k].is_finite()).collect();
    order.sort_by(|&a, &b| filled[b].total_cmp(&filled[a]));
    let mut area = vec![dx * dx; n * n];
    for &k in &order {
        let r = receiver[k];
        if r != usize::MAX {
            area[r] += area[k];
        }
    }
    for (ac, out) in [1e4, 1e5].iter().zip(m.drainage.iter_mut()) {
        let length: f64 = order
            .iter()
            .filter(|&&k| area[k] >= *ac && receiver[k] != usize::MAX)
            .map(|&k| dist[k])
            .sum();
        *out = (length / 1000.0 / km2.max(1e-9)) as f32;
    }

    // The slope–area relation: the ground's own slope to the next cell down, by quarter decade
    // of the area above. Cells on the edges drain off the grid and their areas are cut short.
    let mut bins: Vec<Vec<f32>> = Vec::new();
    let a0 = (dx * dx).log10();
    for &k in &order {
        let r = receiver[k];
        let (i, j) = (k % n, k / n);
        if r == usize::MAX || i < 2 || j < 2 || i > n - 3 || j > n - 3 {
            continue;
        }
        let s = (d.h[k] - d.h[r]) as f64 / dist[k];
        if s <= 0.0 {
            continue;
        }
        let b = ((area[k].log10() - a0) * 4.0) as usize;
        if bins.len() <= b {
            bins.resize(b + 1, Vec::new());
        }
        bins[b].push(s as f32);
    }
    for (b, mut v) in bins.into_iter().enumerate() {
        if v.len() < 30 {
            continue;
        }
        v.sort_by(f32::total_cmp);
        m.slope_area.push((
            (a0 + (b as f64 + 0.5) / 4.0) as f32,
            percentile(&v, 0.5),
            v.len(),
        ));
    }
    let fluvial: Vec<(f64, f64)> = m
        .slope_area
        .iter()
        .filter(|(a, s, _)| *a >= 5.0 && *s > 0.0)
        .map(|(a, s, _)| (*a as f64, (*s as f64).log10()))
        .collect();
    m.concavity = if fluvial.len() >= 3 {
        let k = fluvial.len() as f64;
        let mx = fluvial.iter().map(|p| p.0).sum::<f64>() / k;
        let my = fluvial.iter().map(|p| p.1).sum::<f64>() / k;
        let sxy: f64 = fluvial.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
        let sxx: f64 = fluvial.iter().map(|p| (p.0 - mx).powi(2)).sum();
        (-sxy / sxx) as f32
    } else {
        f32::NAN
    };
    m.turn_area = m
        .slope_area
        .iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(f32::NAN, |p| 10f32.powf(p.0));

    // The largest channel, followed up from its outlet by its largest tributary each time.
    let mut donor = vec![usize::MAX; n * n];
    for &k in &order {
        let r = receiver[k];
        if r != usize::MAX && (donor[r] == usize::MAX || area[k] > area[donor[r]]) {
            donor[r] = k;
        }
    }
    m.sinuosity = f32::NAN;
    if let Some(&outlet) = order.iter().max_by(|&&a, &&b| area[a].total_cmp(&area[b])) {
        let (mut k, mut length) = (outlet, 0.0f64);
        while donor[k] != usize::MAX && area[donor[k]] >= 1e6 {
            length += dist[donor[k]];
            k = donor[k];
        }
        let (dx_, dz_) = (
            (k % n) as f64 - (outlet % n) as f64,
            (k / n) as f64 - (outlet / n) as f64,
        );
        let straight = dx_.hypot(dz_) * dx;
        if straight > 20.0 * dx {
            m.sinuosity = (length / straight) as f32;
        }
    }

    let known_h: Vec<f32> = d.h.iter().copied().filter(|h| h.is_finite()).collect();
    let (lo, hi) = known_h
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), &h| (lo.min(h), hi.max(h)));
    let mean = known_h.iter().map(|&h| h as f64).sum::<f64>() / known_h.len().max(1) as f64;
    m.relief = hi - lo;
    m.hypsometric = ((mean - lo as f64) / (hi - lo).max(1e-6) as f64) as f32;
    m
}

impl Metrics {
    /// The roughness exponent between two lags (log2 of the ratio of mean |Δh| per doubling):
    /// 1 where the ground is smooth (planar) at that scale, ½ for a random walk, 0 for noise.
    pub fn hurst(&self) -> Vec<(f64, f64)> {
        self.structure
            .windows(2)
            .map(|w| (w[0].0, (w[1].1 / w[0].1.max(1e-12)).log2()))
            .collect()
    }

    /// The mean |Δh| at a lag (m), if measured.
    pub fn rough_at(&self, lag: f64) -> f64 {
        self.structure
            .iter()
            .find(|(l, _)| (*l - lag).abs() < 1e-6 * lag)
            .map_or(f64::NAN, |p| p.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane(n: usize, dx: f64, grade: f32) -> Dem {
        let mut h = vec![0.0; n * n];
        for j in 0..n {
            for i in 0..n {
                h[j * n + i] = grade * i as f32 * dx as f32;
            }
        }
        Dem { n, dx, h }
    }

    #[test]
    fn a_plane_is_one_slope_without_pits_draining_off_its_low_edge() {
        let m = measure(&plane(64, 2.0, 0.5));
        assert!((m.slope[1] - 26.565).abs() < 0.01, "{:?}", m.slope);
        assert_eq!(m.pits_per_km2, 0.0);
        assert_eq!(m.depressed, [0.0, 0.0]);
        // Smooth at every scale: |Δh| doubles with the lag.
        assert!(m.hurst().iter().all(|(_, h)| (h - 1.0).abs() < 1e-6));
        assert!((m.hypsometric - 0.5).abs() < 0.01);
    }

    #[test]
    fn a_bowl_is_one_closed_depression() {
        let n = 33;
        let mut d = plane(n, 1.0, 0.0);
        for j in 0..n {
            for i in 0..n {
                let r = ((i as f32 - 16.0).powi(2) + (j as f32 - 16.0).powi(2)).sqrt();
                d.h[j * n + i] = (r - 12.0).min(0.0) * 0.5 + 10.0;
            }
        }
        let m = measure(&d);
        // The bowl, 6 m deep, holds water up to its rim: more than 1 m of it within 10 m of
        // the centre (some 317 of 1089 cells), more than 0.25 m within 11.5 m (some 415).
        assert!((0.27..0.31).contains(&m.depressed[1]), "{:?}", m.depressed);
        assert!((0.36..0.40).contains(&m.depressed[0]), "{:?}", m.depressed);
        assert!(m.pits_per_km2 > 0.0);
    }

    #[test]
    fn valleys_gather_area_and_flatten_downstream() {
        // A valley whose floor falls as 1/√A of the area gathered above: θ = ½. Flow runs
        // down the middle column toward j = 0; the sides fall steeply to it, so the area at a
        // floor cell grows with its distance u from the head and its slope as u^-½.
        let n = 512;
        let mut d = plane(n, 10.0, 0.0);
        for j in 0..n {
            for i in 0..n {
                let u = (n - j) as f64 * 10.0;
                let floor = 400.0 - 4.0 * u.sqrt();
                let side = (i as f64 - 256.0).abs() * 10.0 * 0.3;
                d.h[j * n + i] = (floor + side) as f32;
            }
        }
        let m = measure(&d);
        assert!(m.drainage[0] > 0.0);
        assert!(
            (m.concavity - 0.5).abs() < 0.1,
            "{} {:?}",
            m.concavity,
            m.slope_area
        );
    }
}

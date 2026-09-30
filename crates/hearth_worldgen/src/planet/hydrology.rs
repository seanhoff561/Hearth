//! Drainage on the planet grid: depression filling (Priority-Flood+ε), steepest-descent
//! receivers with sphere-correct distances, topological ordering, flow accumulation, and
//! implicit stream-power erosion (Braun & Willett 2013, n = 1).

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use rayon::prelude::*;

use super::grid::GridGeom;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Ord32(f32);
impl Eq for Ord32 {}
impl PartialOrd for Ord32 {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Ord32 {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&o.0)
    }
}

/// Fills depressions so every non-base cell drains to a base cell (Priority-Flood+ε, Barnes
/// et al. 2014). `is_base` marks base-level cells (the ocean). Cells in `is_sink` are also base
/// levels (endorheic basins that are kept).
pub fn fill_depressions(geom: &GridGeom, elev: &[f32], is_base: &[bool], eps: f32) -> Vec<f32> {
    let n = geom.n;
    let mut h = elev.to_vec();
    let mut closed = is_base.to_vec();
    let mut open: BinaryHeap<Reverse<(Ord32, u32)>> = BinaryHeap::new();
    let mut pit: VecDeque<u32> = VecDeque::new();
    // Seed with non-base cells adjacent to base cells (the coastline of the drainage domain).
    for idx in 0..geom.len() {
        if closed[idx] {
            continue;
        }
        let (i, j) = (idx % n, idx / n);
        let touches_base = GridGeom::OFFSETS8
            .iter()
            .any(|&(di, dj)| geom.neighbor(i, j, di, dj).is_some_and(|nb| is_base[nb]));
        if touches_base {
            closed[idx] = true;
            open.push(Reverse((Ord32(h[idx]), idx as u32)));
        }
    }
    while let Some(c) = pit
        .pop_front()
        .or_else(|| open.pop().map(|Reverse((_, c))| c))
    {
        let c = c as usize;
        let (i, j) = (c % n, c / n);
        let hc = h[c];
        for &(di, dj) in &GridGeom::OFFSETS8 {
            let Some(nb) = geom.neighbor(i, j, di, dj) else {
                continue;
            };
            if closed[nb] {
                continue;
            }
            closed[nb] = true;
            let floor = hc + eps;
            if h[nb] <= floor {
                h[nb] = floor;
                pit.push_back(nb as u32);
            } else {
                open.push(Reverse((Ord32(h[nb]), nb as u32)));
            }
        }
    }
    // Isolated cells never reached (no base anywhere): leave as is.
    h
}

/// Physical distance (blocks) between a cell in row `j` and its neighbour at (di, dj).
#[inline]
pub fn neighbor_distance(geom: &GridGeom, j: usize, di: isize, dj: isize) -> f32 {
    let d = geom.phys_cell(j) as f32;
    if di != 0 && dj != 0 {
        d * std::f32::consts::SQRT_2
    } else {
        d
    }
}

/// Steepest-descent receiver of every cell (itself for base cells and local minima).
pub fn receivers(geom: &GridGeom, h: &[f32], is_base: &[bool]) -> Vec<u32> {
    let n = geom.n;
    (0..geom.len())
        .into_par_iter()
        .map(|idx| {
            if is_base[idx] {
                return idx as u32;
            }
            let (i, j) = (idx % n, idx / n);
            let hc = h[idx];
            let mut best = idx;
            let mut best_slope = 0.0f32;
            for &(di, dj) in &GridGeom::OFFSETS8 {
                if let Some(nb) = geom.neighbor(i, j, di, dj) {
                    let drop = hc - h[nb];
                    if drop > 0.0 {
                        let s = drop / neighbor_distance(geom, j, di, dj);
                        if s > best_slope {
                            best_slope = s;
                            best = nb;
                        }
                    }
                }
            }
            best as u32
        })
        .collect()
}

/// Topological order: outlets (receiver == self) first, then every cell after its receiver.
pub fn stack_order(receiver: &[u32]) -> Vec<u32> {
    let len = receiver.len();
    let mut count = vec![0u32; len + 1];
    for (c, &r) in receiver.iter().enumerate() {
        if r as usize != c {
            count[r as usize + 1] += 1;
        }
    }
    for k in 1..=len {
        count[k] += count[k - 1];
    }
    let mut donors = vec![0u32; count[len] as usize];
    let mut fill = count.clone();
    for (c, &r) in receiver.iter().enumerate() {
        if r as usize != c {
            donors[fill[r as usize] as usize] = c as u32;
            fill[r as usize] += 1;
        }
    }
    let mut order = Vec::with_capacity(len);
    let mut queue: VecDeque<u32> = VecDeque::new();
    for (c, &r) in receiver.iter().enumerate() {
        if r as usize == c {
            queue.push_back(c as u32);
        }
    }
    while let Some(c) = queue.pop_front() {
        order.push(c);
        let c = c as usize;
        for &d in &donors[count[c] as usize..count[c + 1] as usize] {
            queue.push_back(d);
        }
    }
    order
}

/// Accumulates `weight` downstream. `loss[c]` (0..1) is the fraction of a cell's outflow lost
/// before reaching its receiver (evaporation and infiltration in dry climates).
pub fn accumulate(
    receiver: &[u32],
    order: &[u32],
    weight: &[f32],
    loss: Option<&[f32]>,
) -> Vec<f32> {
    let mut acc = weight.to_vec();
    for &c in order.iter().rev() {
        let c = c as usize;
        let r = receiver[c] as usize;
        if r != c {
            let keep = loss.map_or(1.0, |l| 1.0 - l[c]);
            acc[r] += acc[c] * keep;
        }
    }
    acc
}

/// Parameters of the stream-power erosion.
#[derive(Debug, Clone, Copy)]
pub struct ErosionParams {
    pub iterations: usize,
    /// Erodibility × time step, in units where drainage area is counted in equatorial cells.
    pub k: f32,
    /// Area exponent (≈ 0.5).
    pub m: f32,
    /// Maximum stable slope (blocks/blocks) for the talus pass.
    pub talus: f32,
}

impl Default for ErosionParams {
    fn default() -> Self {
        Self {
            iterations: 10,
            k: 0.012,
            m: 0.5,
            talus: 1.3,
        }
    }
}

/// Angular area of the equatorial cell of a 1024² grid, in square degrees: the reference unit
/// for drainage areas, so erosion and river sizes don't depend on grid resolution.
pub const REF_CELL_SQDEG: f64 = (360.0 / 1024.0) * (360.0 / 1024.0);

/// Physical area of a cell in row `j`, in square degrees of the sphere.
#[inline]
pub fn cell_sqdeg(geom: &GridGeom, j: usize) -> f64 {
    let deg = geom.phys_cell(j) / geom.radius() * (180.0 / std::f64::consts::PI);
    deg * deg
}

/// Erodes land cells (`h` in blocks) toward their receivers. Base cells are fixed. `rain` weights
/// the drainage area (wetter catchments cut deeper valleys). The input must drain (filled).
pub fn stream_power(
    geom: &GridGeom,
    h: &mut [f32],
    is_base: &[bool],
    rain: &[f32],
    params: &ErosionParams,
) {
    let n = geom.n;
    let weight: Vec<f32> = (0..geom.len())
        .map(|idx| {
            let j = idx / n;
            (cell_sqdeg(geom, j) / REF_CELL_SQDEG) as f32 * rain[idx]
        })
        .collect();
    // Distances in units of the reference (1024²) cell.
    let ref_cell = (geom.c / 1024.0) as f32;
    for _ in 0..params.iterations {
        let rec = receivers(geom, h, is_base);
        let order = stack_order(&rec);
        let area = accumulate(&rec, &order, &weight, None);
        for &c in &order {
            let c = c as usize;
            let r = rec[c] as usize;
            if r == c || is_base[c] {
                continue;
            }
            let (i, j) = (c % n, c / n);
            let (ri, rj) = (r % n, r / n);
            let di = geom_delta_i(n, i, ri);
            let dj = rj as isize - j as isize;
            let dist = neighbor_distance(geom, j, di, dj) / ref_cell;
            let f = params.k * area[c].powf(params.m) / dist;
            let hr = h[r];
            if h[c] > hr {
                h[c] = (h[c] + f * hr) / (1.0 + f);
            }
        }
        talus_pass(geom, h, is_base, params.talus);
    }
}

#[inline]
fn geom_delta_i(n: usize, from: usize, to: usize) -> isize {
    let d = to as isize - from as isize;
    if d > 1 {
        d - n as isize
    } else if d < -1 {
        d + n as isize
    } else {
        d
    }
}

/// Moves material down slopes steeper than `talus` (gather-style, parallel).
fn talus_pass(geom: &GridGeom, h: &mut [f32], is_base: &[bool], talus: f32) {
    let n = geom.n;
    let src = h.to_vec();
    // Outflow each cell sends to its steepest lower neighbour.
    let out: Vec<(u32, f32)> = (0..geom.len())
        .into_par_iter()
        .map(|idx| {
            if is_base[idx] {
                return (idx as u32, 0.0);
            }
            let (i, j) = (idx % n, idx / n);
            let mut best = (idx as u32, 0.0f32);
            let mut best_excess = 0.0f32;
            for &(di, dj) in &GridGeom::OFFSETS8 {
                if let Some(nb) = geom.neighbor(i, j, di, dj) {
                    let d = neighbor_distance(geom, j, di, dj);
                    let excess = (src[idx] - src[nb]) - talus * d;
                    if excess > best_excess {
                        best_excess = excess;
                        best = (nb as u32, excess * 0.25);
                    }
                }
            }
            best
        })
        .collect();
    for (idx, &(to, amount)) in out.iter().enumerate() {
        if amount > 0.0 && to as usize != idx {
            h[idx] -= amount;
            h[to as usize] += amount;
        }
    }
}

/// Connected components (8-neighbourhood) of cells where `mask` is set. Returns a label per
/// cell (`u32::MAX` outside the mask) and the number of components.
pub fn label_components(geom: &GridGeom, mask: &[bool]) -> (Vec<u32>, u32) {
    let n = geom.n;
    let mut label = vec![u32::MAX; mask.len()];
    let mut next = 0u32;
    let mut stack = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] || label[start] != u32::MAX {
            continue;
        }
        label[start] = next;
        stack.push(start);
        while let Some(c) = stack.pop() {
            let (i, j) = (c % n, c / n);
            for &(di, dj) in &GridGeom::OFFSETS8 {
                if let Some(nb) = geom.neighbor(i, j, di, dj)
                    && mask[nb]
                    && label[nb] == u32::MAX
                {
                    label[nb] = next;
                    stack.push(nb);
                }
            }
        }
        next += 1;
    }
    (label, next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hearth_math::{Planet, PlanetSize};

    fn geom(n: usize) -> GridGeom {
        GridGeom::new(Planet::from_size(PlanetSize::Tiny).unwrap(), n)
    }

    #[test]
    fn filled_surface_drains_everywhere() {
        let g = geom(32);
        let mut elev = vec![0f32; g.len()];
        let mut base = vec![false; g.len()];
        for idx in 0..g.len() {
            let (i, j) = g.ij(idx);
            if !(3..=28).contains(&j) {
                base[idx] = true;
                elev[idx] = -10.0;
            } else {
                // A bowl with a pit in the middle.
                let dx = i as f32 - 16.0;
                let dz = j as f32 - 16.0;
                elev[idx] = 50.0 - (dx * dx + dz * dz).sqrt() * 0.5
                    + if (i, j) == (16, 16) { -30.0 } else { 0.0 };
            }
        }
        let filled = fill_depressions(&g, &elev, &base, 0.01);
        let rec = receivers(&g, &filled, &base);
        let order = stack_order(&rec);
        assert_eq!(order.len(), g.len());
        // Every cell reaches a base cell by following receivers.
        for start in 0..g.len() {
            let mut c = start;
            for _ in 0..g.len() {
                if base[c] {
                    break;
                }
                let r = rec[c] as usize;
                assert_ne!(r, c, "non-base local minimum at {:?}", g.ij(c));
                assert!(filled[r] < filled[c] + 1e-6, "flows uphill");
                c = r;
            }
            assert!(base[c]);
        }
        assert!(
            filled[g.idx(16, 16)] > elev[g.idx(16, 16)],
            "pit was filled"
        );
    }

    #[test]
    fn accumulation_counts_upstream_cells() {
        // A 1D chain 0 <- 1 <- 2 <- 3.
        let rec = vec![0u32, 0, 1, 2];
        let order = stack_order(&rec);
        assert_eq!(order, vec![0, 1, 2, 3]);
        let acc = accumulate(&rec, &order, &[1.0; 4], None);
        assert_eq!(acc, vec![4.0, 3.0, 2.0, 1.0]);
        let lossy = accumulate(&rec, &order, &[1.0; 4], Some(&[0.0, 0.5, 0.5, 0.5]));
        assert!(lossy[0] < 4.0);
    }

    #[test]
    fn erosion_never_creates_pits() {
        let g = geom(64);
        let mut rng = hearth_math::hash::Rng::new(5);
        let mut base = vec![false; g.len()];
        let mut h = vec![0f32; g.len()];
        for idx in 0..g.len() {
            let (_, j) = g.ij(idx);
            if !(8..56).contains(&j) {
                base[idx] = true;
                h[idx] = 0.0;
            } else {
                h[idx] = 200.0 + rng.range_f32(0.0, 80.0);
            }
        }
        let mut filled = fill_depressions(&g, &h, &base, 0.01);
        let rain = vec![1.0f32; g.len()];
        stream_power(&g, &mut filled, &base, &rain, &ErosionParams::default());
        let rec = receivers(&g, &filled, &base);
        let pits = (0..g.len())
            .filter(|&c| !base[c] && rec[c] as usize == c)
            .count();
        // The talus pass can create a few flats; they must stay rare.
        assert!(pits < g.len() / 200, "{pits} pits");
    }

    #[test]
    fn components() {
        let g = geom(16);
        let mut mask = vec![false; g.len()];
        mask[g.idx(1, 1)] = true;
        mask[g.idx(2, 2)] = true;
        mask[g.idx(10, 10)] = true;
        let (_, count) = label_components(&g, &mask);
        assert_eq!(count, 2);
    }
}

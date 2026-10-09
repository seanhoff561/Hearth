//! The drainage of a refinement level's tile: hollows filled or cut through (Priority-Flood+ε),
//! receivers and their order, discharge, slumping, the sea's spread and the lakes.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use super::{D8, MARGIN, OUTLET};

/// The lakes over ground `z` whose hollows the water's surface `wet` fills, with the parent's
/// lakes `held` (their surfaces, NaN elsewhere): lake surfaces, NaN elsewhere. A hollow joined
/// to a parent's lake is that lake's; one of this level's own is a lake if it fits in a tile's
/// margin, so every tile that sees it sees it whole and agrees on it, and somewhere it is deeper
/// than `deep` or its bed lies below the sea's level (the water table standing at it). A larger
/// one of its own (the parent saw no lake there) is filled to its brim with what washed into
/// it, a basin's flat floor.
pub(super) fn lakes(
    wet: &[f32],
    z: &mut [f32],
    base: &[bool],
    held: &[f32],
    n: usize,
    deep: f32,
) -> Vec<f32> {
    const DAMP: f32 = 0.01;
    let len = wet.len();
    let water = |k: usize| held[k].is_finite() || (!base[k] && wet[k] - z[k] > DAMP);
    let mut lake = vec![f32::NAN; len];
    let mut seen = vec![false; len];
    let mut hollow = Vec::new();
    let mut basins = Vec::new();
    for start in 0..len {
        if seen[start] || !water(start) || wet[start] <= 0.0 {
            continue;
        }
        // One hollow: the water-covered cells joined to this one.
        hollow.clear();
        hollow.push(start);
        seen[start] = true;
        let mut i = 0;
        let (mut lo, mut hi) = ((i64::MAX, i64::MAX), (i64::MIN, i64::MIN));
        let (mut parents, mut own) = (false, false);
        while i < hollow.len() {
            let k = hollow[i];
            i += 1;
            parents |= held[k].is_finite();
            own |= wet[k] - z[k] > deep || z[k] < 0.0;
            let (u, w) = ((k % n) as i64, (k / n) as i64);
            lo = (lo.0.min(u), lo.1.min(w));
            hi = (hi.0.max(u), hi.1.max(w));
            for &(du, dw) in &D8 {
                let (nu, nw) = (u + du, w + dw);
                if nu < 0 || nw < 0 || nu >= n as i64 || nw >= n as i64 {
                    continue;
                }
                let nb = (nw * n as i64 + nu) as usize;
                if !seen[nb] && water(nb) {
                    seen[nb] = true;
                    hollow.push(nb);
                }
            }
        }
        let small = hi.0 - lo.0 < MARGIN && hi.1 - lo.1 < MARGIN;
        if parents || (own && small) {
            for &k in &hollow {
                lake[k] = if held[k].is_finite() { held[k] } else { wet[k] };
            }
        } else if !small {
            basins.extend_from_slice(&hollow);
        }
    }
    for k in basins {
        z[k] = wet[k];
    }
    lake
}

/// Ends any ring the channels' receivers close: following each channel's course, a cell met
/// twice on one walk gives up its receiver.
pub(super) fn break_rings(recv: &mut [u8], channel: &[bool], n: usize) {
    let len = recv.len();
    // 0: not yet walked; walk number + 1 while on a walk, or once its course is known to end.
    let mut mark = vec![0u32; len];
    for (walk, start) in (0..len).filter(|&k| channel[k]).enumerate() {
        let walk = walk as u32 + 1;
        let mut k = start;
        while channel[k] && mark[k] == 0 {
            mark[k] = walk;
            match target(k, recv[k], n) {
                Some(r) if mark[r] == walk => {
                    recv[k] = OUTLET;
                    break;
                }
                Some(r) => k = r,
                None => break,
            }
        }
    }
}

/// Spreads the sea from its cells over the ground below its level joined to them (side by side,
/// not corner to corner), and up channels whose beds lie at its level; a lake (`held`) keeps it
/// out.
pub(super) fn flood_sea(
    sea: &mut [bool],
    h: &[f32],
    channel: &[bool],
    outside: &[bool],
    held: &[f32],
    n: usize,
) {
    let mut reach: VecDeque<usize> = (0..sea.len()).filter(|&k| sea[k]).collect();
    while let Some(k) = reach.pop_front() {
        let (u, w) = ((k % n) as i64, (k / n) as i64);
        for (du, dw) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nu, nw) = (u + du, w + dw);
            if nu < 0 || nw < 0 || nu >= n as i64 || nw >= n as i64 {
                continue;
            }
            let nb = (nw * n as i64 + nu) as usize;
            let low = h[nb] < 0.0 || (channel[nb] && h[nb] <= 0.0);
            if !sea[nb] && !outside[nb] && held[nb].is_nan() && low {
                sea[nb] = true;
                reach.push_back(nb);
            }
        }
    }
}

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

/// Priority-Flood+ε over an n × n span from its base cells, so every other cell drains. A
/// hollow up to `breach` deep (at its bottom) is drained by cutting its way out (the path the
/// flood came by lowered below it, as a stream would have cut it: Lindsay 2016); a deeper one is
/// filled to its brim, a lake.
pub(super) fn fill(h: &mut [f32], base: &[bool], n: usize, eps: f32, breach: &[f32]) {
    let mut closed = base.to_vec();
    let mut from = vec![u32::MAX; h.len()];
    let mut open: BinaryHeap<Reverse<(Ord32, u32)>> = BinaryHeap::new();
    let mut pit: VecDeque<u32> = VecDeque::new();
    for k in 0..h.len() {
        if base[k] {
            open.push(Reverse((Ord32(h[k]), k as u32)));
        }
    }
    while let Some(c) = pit
        .pop_front()
        .or_else(|| open.pop().map(|Reverse((_, c))| c))
    {
        let c = c as usize;
        let (u, w) = ((c % n) as i64, (c / n) as i64);
        for &(du, dw) in &D8 {
            let (nu, nw) = (u + du, w + dw);
            if nu < 0 || nw < 0 || nu >= n as i64 || nw >= n as i64 {
                continue;
            }
            let nb = (nw * n as i64 + nu) as usize;
            if closed[nb] {
                continue;
            }
            closed[nb] = true;
            from[nb] = c as u32;
            let hc = h[c];
            if h[nb] <= hc + eps {
                // A hollow below the sea's level is not cut through, which would let the sea in.
                if h[nb] >= 0.0
                    && hc - h[nb] <= breach[nb]
                    && can_cut(h, base, &from, c, h[nb] - eps, eps)
                {
                    // Cut the way out: down the flood's path, each cell below the last.
                    let mut level = h[nb] - eps;
                    let mut p = c;
                    while !base[p] && h[p] > level {
                        h[p] = level;
                        level -= eps;
                        if from[p] == u32::MAX {
                            break;
                        }
                        p = from[p] as usize;
                    }
                    open.push(Reverse((Ord32(h[nb]), nb as u32)));
                } else {
                    h[nb] = hc + eps;
                    pit.push_back(nb as u32);
                }
            } else {
                open.push(Reverse((Ord32(h[nb]), nb as u32)));
            }
        }
    }
}

/// Whether the flood's path back from `c` can be cut to fall from `level`: it must reach a fixed
/// cell (the sea, a channel, the span's edge) that lies lower still, as fixed cells are not cut.
fn can_cut(h: &[f32], base: &[bool], from: &[u32], c: usize, mut level: f32, eps: f32) -> bool {
    let mut p = c;
    loop {
        if base[p] {
            return h[p] <= level;
        }
        if h[p] <= level {
            return true;
        }
        level -= eps;
        if from[p] == u32::MAX {
            return true;
        }
        p = from[p] as usize;
    }
}

/// How deep (m) a hollow may be and still be cut through at a level, rather than hold a lake, at
/// the least: as deep as the level's relief makes its hollows on low ground, so only the deepest
/// hold water (more where the relief is rougher, `build`). Rivers keep pace with the land's rise,
/// so a mountain range drains through its valleys; lakes are where the ground sank or the parent
/// held one.
pub(super) fn breach_depth(level: usize) -> f32 {
    match level {
        1 => 400.0,
        2 => 100.0,
        _ => 25.0,
    }
}

/// Steepest-descent receivers; channels keep theirs, base cells drain out.
pub(super) fn steepest(
    h: &[f32],
    base: &[bool],
    channel: &[bool],
    fixed: &[u8],
    n: usize,
    s: f32,
) -> Vec<u8> {
    let mut rec = vec![OUTLET; h.len()];
    for k in 0..h.len() {
        if channel[k] {
            rec[k] = fixed[k];
            continue;
        }
        if base[k] {
            continue;
        }
        let (u, w) = ((k % n) as i64, (k / n) as i64);
        let mut best = OUTLET;
        let mut best_slope = 0.0f32;
        for (code, &(du, dw)) in D8.iter().enumerate() {
            let (nu, nw) = (u + du, w + dw);
            if nu < 0 || nw < 0 || nu >= n as i64 || nw >= n as i64 {
                continue;
            }
            let nb = (nw * n as i64 + nu) as usize;
            let drop = h[k] - h[nb];
            if drop > 0.0 {
                let d = if du != 0 && dw != 0 { s * 1.414 } else { s };
                if drop / d > best_slope {
                    best_slope = drop / d;
                    best = code as u8;
                }
            }
        }
        rec[k] = best;
    }
    rec
}

/// The receiver's index of a cell, if any.
#[inline]
pub(super) fn target(k: usize, code: u8, n: usize) -> Option<usize> {
    if code == OUTLET {
        return None;
    }
    let (du, dw) = D8[code as usize];
    let (u, w) = ((k % n) as i64 + du, (k / n) as i64 + dw);
    (u >= 0 && w >= 0 && u < n as i64 && w < n as i64).then(|| (w * n as i64 + u) as usize)
}

/// Outlets first, then every cell after the cell it drains to.
pub(super) fn stack(rec: &[u8], n: usize) -> Vec<u32> {
    let len = rec.len();
    // Each cell's donors, packed: those of cell r at donors[first[r]..first[r + 1]].
    let recv: Vec<Option<usize>> = rec
        .iter()
        .enumerate()
        .map(|(k, &code)| target(k, code, n))
        .collect();
    let mut first = vec![0u32; len + 1];
    for r in recv.iter().flatten() {
        first[r + 1] += 1;
    }
    for k in 0..len {
        first[k + 1] += first[k];
    }
    let mut fill = first.clone();
    let mut donors = vec![0u32; first[len] as usize];
    for (k, r) in recv.iter().enumerate() {
        if let Some(r) = *r {
            donors[fill[r] as usize] = k as u32;
            fill[r] += 1;
        }
    }
    // Breadth first from the outlets: the order is the queue itself.
    let mut order: Vec<u32> = (0..len as u32)
        .filter(|&k| recv[k as usize].is_none())
        .collect();
    order.reserve(len - order.len());
    let mut head = 0;
    while head < order.len() {
        let c = order[head] as usize;
        head += 1;
        order.extend_from_slice(&donors[first[c] as usize..first[c + 1] as usize]);
    }
    order
}

/// Discharge: each cell's own runoff carried downstream; a channel carries at least its
/// parent river's.
pub(super) fn accumulate(
    rec: &[u8],
    order: &[u32],
    rain: &[f32],
    inflow: &[f32],
    channel: &[bool],
    n: usize,
) -> Vec<f32> {
    let mut q = rain.to_vec();
    for &c in order.iter().rev() {
        let c = c as usize;
        if channel[c] {
            q[c] = q[c].max(inflow[c]);
        }
        if let Some(r) = target(c, rec[c], n) {
            q[r] += q[c];
        }
    }
    q
}

/// Slopes steeper than `max_drop` per cell slump half their excess onto the lower neighbour.
pub(super) fn slump(h: &mut [f32], base: &[bool], n: usize, max_drop: f32) {
    let src = h.to_vec();
    for k in 0..h.len() {
        if base[k] {
            continue;
        }
        let (u, w) = ((k % n) as i64, (k / n) as i64);
        let mut best: Option<(usize, f32)> = None;
        for &(du, dw) in &D8 {
            let (nu, nw) = (u + du, w + dw);
            if nu < 0 || nw < 0 || nu >= n as i64 || nw >= n as i64 {
                continue;
            }
            let nb = (nw * n as i64 + nu) as usize;
            let d = if du != 0 && dw != 0 { 1.414 } else { 1.0 };
            let excess = src[k] - src[nb] - max_drop * d;
            if excess > best.map_or(0.0, |b| b.1) {
                best = Some((nb, excess));
            }
        }
        if let Some((nb, excess)) = best {
            h[k] -= excess * 0.25;
            if !base[nb] {
                h[nb] += excess * 0.25;
            }
        }
    }
}

/// Water routed by many flow directions (Quinn et al. 1991): each cell's water shared among all
/// its lower neighbours by slope and the width of the contour it crosses, so
/// that it spreads over open slopes and gathers in hollows instead of running down the grid's
/// eight directions; a channel keeps its own course.
pub(super) struct Routing {
    /// Each cell's share of its water to each neighbour, by [`D8`] code.
    share: Vec<[f32; 8]>,
    /// Every cell after all the cells draining into it (a ring, should one form, left out).
    order: Vec<u32>,
    /// How many of each cell's donors are not yet placed in the order.
    donors: Vec<u8>,
}

impl Routing {
    /// Buffers for a span of `len` cells.
    pub(super) fn new(len: usize) -> Self {
        Self {
            share: vec![[0.0; 8]; len],
            order: Vec::with_capacity(len),
            donors: vec![0; len],
        }
    }

    /// Routes the water over the surface `wet` (every cell but the base ones with a lower
    /// neighbour, as after [`fill`]; the base cells hold the span's edges, so the others lie
    /// inside it); channels drain along their receivers `fixed`.
    pub(super) fn route(
        &mut self,
        wet: &[f32],
        base: &[bool],
        channel: &[bool],
        fixed: &[u8],
        n: usize,
    ) {
        let len = wet.len();
        let (share, donors, order) = (&mut self.share, &mut self.donors, &mut self.order);
        donors.fill(0);
        // Each neighbour's index offset, its distance and the contour's width it takes: half a
        // cell straight, a third of one corner to corner.
        let step: [(isize, f32); 8] = std::array::from_fn(|c| {
            let (du, dw) = D8[c];
            let diagonal = du != 0 && dw != 0;
            (
                dw as isize * n as isize + du as isize,
                if diagonal {
                    0.354 / std::f32::consts::SQRT_2
                } else {
                    0.5
                },
            )
        });
        for k in 0..len {
            share[k] = [0.0; 8];
            if channel[k] {
                if let Some(r) = target(k, fixed[k], n) {
                    share[k][fixed[k] as usize] = 1.0;
                    donors[r] += 1;
                }
                continue;
            }
            if base[k] {
                continue;
            }
            let mut sum = 0.0;
            for (code, &(off, w)) in step.iter().enumerate() {
                let drop = wet[k] - wet[(k as isize + off) as usize];
                if drop > 0.0 {
                    share[k][code] = drop * w;
                    sum += drop * w;
                }
            }
            if sum > 0.0 {
                for (code, v) in share[k].iter_mut().enumerate() {
                    if *v > 0.0 {
                        *v /= sum;
                        donors[(k as isize + step[code].0) as usize] += 1;
                    }
                }
            }
        }
        // Kahn's order: a cell once all its donors are placed.
        order.clear();
        order.extend((0..len as u32).filter(|&k| donors[k as usize] == 0));
        let mut head = 0;
        while head < order.len() {
            let c = order[head] as usize;
            head += 1;
            for (code, &v) in share[c].iter().enumerate() {
                if v > 0.0 {
                    let r = target(c, code as u8, n).expect("a neighbour");
                    donors[r] -= 1;
                    if donors[r] == 0 {
                        order.push(r as u32);
                    }
                }
            }
        }
    }

    /// Discharge: each cell's own runoff carried downstream and shared; a channel carries at
    /// least its parent river's.
    pub(super) fn accumulate(
        &self,
        rain: &[f32],
        inflow: &[f32],
        channel: &[bool],
        n: usize,
    ) -> Vec<f32> {
        let mut q = rain.to_vec();
        for &c in &self.order {
            let c = c as usize;
            if channel[c] {
                q[c] = q[c].max(inflow[c]);
            }
            for (code, &v) in self.share[c].iter().enumerate() {
                if v > 0.0 {
                    q[target(c, code as u8, n).expect("a neighbour")] += q[c] * v;
                }
            }
        }
        q
    }

    /// Implicit stream-power erosion (n = 1) over the shared flow: a cell wears towards each
    /// lower receiver by its share, `f = K (√(q / q_cell) − 1) share / distance` (so ground that
    /// drains only itself is left to diffuse and slump), receivers worn first. Land wears down
    /// to the sea's level at most; water standing over a cell (`wet` above `z`) protects it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn erode(
        &self,
        z: &mut [f32],
        wet: &mut [f32],
        q: &[f32],
        k: &[f32],
        base: &[bool],
        q_cell: f32,
        n: usize,
    ) {
        for &c in self.order.iter().rev() {
            let c = c as usize;
            if base[c] || wet[c] > z[c] + 1e-2 {
                continue;
            }
            let power = k[c] * ((q[c] / q_cell).sqrt() - 1.0).max(0.0);
            if power <= 0.0 {
                continue;
            }
            let (mut fsum, mut fz) = (0.0f32, 0.0f32);
            for (code, &v) in self.share[c].iter().enumerate() {
                if v <= 0.0 {
                    continue;
                }
                let r = target(c, code as u8, n).expect("a neighbour");
                let hr = if z[c] >= 0.0 { wet[r].max(0.0) } else { wet[r] };
                if hr < z[c] {
                    let d = if code % 2 == 1 {
                        std::f32::consts::SQRT_2
                    } else {
                        1.0
                    };
                    let f = power * v / d;
                    fsum += f;
                    fz += f * hr;
                }
            }
            if fsum > 0.0 {
                z[c] = (z[c] + fz) / (1.0 + fsum);
                wet[c] = z[c];
            }
        }
    }
}

/// Hillslope diffusion (soil creep, rain splash): each cell moved towards its neighbours'
/// mean by `rate` of the way (the nine-point Laplacian; stable to 0.3), the base cells held.
/// Ridges round over and small hollows fill, as on soil-mantled hills (Culling 1960; Roering
/// et al. 1999 for the linear part).
pub(super) fn diffuse(z: &mut [f32], base: &[bool], n: usize, rate: f32) {
    let src = z.to_vec();
    for w in 1..n - 1 {
        for u in 1..n - 1 {
            let k = w * n + u;
            if base[k] {
                continue;
            }
            let straight = src[k - 1] + src[k + 1] + src[k - n] + src[k + n];
            let corner = src[k - n - 1] + src[k - n + 1] + src[k + n - 1] + src[k + n + 1];
            let lap = (4.0 * straight + corner - 20.0 * src[k]) / 6.0;
            z[k] = src[k] + rate * lap;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cone's flanks, `n` cells a side, its edge the base.
    fn cone(n: usize) -> (Vec<f32>, Vec<bool>) {
        let c = (n as f32 - 1.0) * 0.5;
        let h = (0..n * n)
            .map(|k| 100.0 - ((k % n) as f32 - c).hypot((k / n) as f32 - c))
            .collect();
        let base = (0..n * n)
            .map(|k| {
                let (u, w) = (k % n, k / n);
                u == 0 || w == 0 || u == n - 1 || w == n - 1
            })
            .collect();
        (h, base)
    }

    /// The discharge leaving a cone through a ring about its summit, by eighths of a turn about
    /// each of the grid's eight directions and the eight between: their ratio.
    fn spokes(q: &[f32], n: usize) -> f32 {
        let c = (n as f32 - 1.0) * 0.5;
        let (mut on, mut off) = (0.0f32, 0.0f32);
        for (k, &qk) in q.iter().enumerate() {
            let (x, z) = ((k % n) as f32 - c, (k / n) as f32 - c);
            let r = x.hypot(z);
            if !(0.4 * c..0.5 * c).contains(&r) {
                continue;
            }
            // Angle within an eighth of a turn from the nearest grid direction (0 … π/8).
            let a = z.atan2(x).rem_euclid(std::f32::consts::FRAC_PI_4);
            let from_grid = a.min(std::f32::consts::FRAC_PI_4 - a);
            if from_grid < std::f32::consts::PI / 32.0 {
                on += qk;
            } else if from_grid > std::f32::consts::PI / 16.0 {
                off += qk;
            }
        }
        on / off.max(1e-9)
    }

    #[test]
    fn water_spreads_over_open_slopes_instead_of_running_down_the_grid() {
        let n = 65;
        let (h, base) = cone(n);
        let none = vec![false; n * n];
        let rain = vec![1.0f32; n * n];
        let mut routing = Routing::new(n * n);
        routing.route(&h, &base, &none, &vec![OUTLET; n * n], n);
        let q = routing.accumulate(&rain, &vec![0.0; n * n], &none, n);
        // Steepest descent, for comparison: the water gathered on the grid's eight spokes.
        let rec = steepest(&h, &base, &none, &vec![OUTLET; n * n], n, 1.0);
        let q8 = accumulate(&rec, &stack(&rec, n), &rain, &vec![0.0; n * n], &none, n);
        let (shared, single) = (spokes(&q, n), spokes(&q8, n));
        assert!(shared < 1.6, "shared: {shared} as much on the spokes");
        assert!(
            single > 3.0 * shared,
            "steepest: {single}, shared: {shared}"
        );
        // No water lost: all the rain leaves through the edge.
        let out: f32 = (0..n * n)
            .filter(|&k| base[k])
            .map(|k| q[k] - rain[k])
            .sum();
        let fell = rain.iter().zip(&base).filter(|(_, b)| !**b).count() as f32;
        assert!((out - fell).abs() < 1e-2 * fell, "{out} of {fell}");
    }

    #[test]
    fn creep_rounds_a_ridge_and_keeps_its_ground() {
        // A ridge across the span, its flanks at 45°.
        let n = 33;
        let mut h: Vec<f32> = (0..n * n)
            .map(|k| 20.0 - ((k % n) as f32 - 16.0).abs())
            .collect();
        let base: Vec<bool> = (0..n * n)
            .map(|k| {
                let (u, w) = (k % n, k / n);
                u < 2 || w < 2 || u >= n - 2 || w >= n - 2
            })
            .collect();
        let before: f32 = h.iter().sum();
        for _ in 0..20 {
            diffuse(&mut h, &base, n, 0.12);
        }
        let crest = h[16 * n + 16];
        assert!(crest < 19.0, "the crest lowered: {crest}");
        // Rounded: the crest's neighbours fall away less than the flanks do.
        let near = crest - h[16 * n + 15];
        let far = h[16 * n + 10] - h[16 * n + 9];
        assert!(near < far, "{near} at the crest, {far} on the flank");
        // Ground moves, no more than what passed into the held edge is lost.
        let after: f32 = h.iter().sum();
        assert!(
            after <= before + 1e-2 && after > before * 0.97,
            "{before} → {after}"
        );
    }
}

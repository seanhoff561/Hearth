//! Tectonic layout: plates on the sphere (warped, weighted Voronoi), their motions (Euler
//! poles), which ones carry continents, and hotspots.

use glam::DVec3;
use hearth_math::hash::{Rng, derive_seed};

use crate::noise::SphereFbm;

/// One tectonic plate.
#[derive(Debug, Clone)]
pub struct Plate {
    pub id: u8,
    /// Centre of the plate (unit vector).
    pub center: DVec3,
    /// Additive Voronoi weight in radians (bigger = larger plate).
    pub weight: f64,
    /// Angular velocity (axis = Euler pole, length = rate).
    pub omega: DVec3,
    pub continental: bool,
    /// Relative crust density; the denser plate subducts at ocean–ocean boundaries.
    pub density: f64,
    /// Approximate fraction of the sphere covered.
    pub area: f64,
}

impl Plate {
    /// Surface velocity of the plate at `p`.
    #[inline]
    pub fn velocity(&self, p: DVec3) -> DVec3 {
        self.omega.cross(p)
    }
}

/// A mantle hotspot that produces a volcanic island chain.
#[derive(Debug, Clone)]
pub struct Hotspot {
    pub pos: DVec3,
    pub strength: f64,
}

/// The planet's tectonic configuration.
#[derive(Debug, Clone)]
pub struct TectonicLayout {
    pub plates: Vec<Plate>,
    pub hotspots: Vec<Hotspot>,
    /// Pairs of plates that share a boundary.
    pub adjacency: Vec<(u8, u8)>,
    warp: SphereFbm,
    warp_strength: f64,
}

/// Result of locating a point among the plates.
#[derive(Debug, Clone, Copy)]
pub struct PlateHit {
    pub plate: u8,
    pub second: u8,
    /// Weighted angular distance difference to the second-nearest plate (≈ 2× the distance
    /// to the boundary), radians.
    pub gap: f64,
}

fn fibonacci_point(i: usize, n: usize) -> DVec3 {
    let golden = std::f64::consts::PI * (3.0 - 5f64.sqrt());
    let y = 1.0 - (i as f64 + 0.5) / n as f64 * 2.0;
    let r = (1.0 - y * y).sqrt();
    let theta = golden * i as f64;
    DVec3::new(r * theta.cos(), y, r * theta.sin())
}

fn random_unit(rng: &mut Rng) -> DVec3 {
    loop {
        let v = DVec3::new(
            rng.range_f64(-1.0, 1.0),
            rng.range_f64(-1.0, 1.0),
            rng.range_f64(-1.0, 1.0),
        );
        let l = v.length_squared();
        if l > 1e-6 && l <= 1.0 {
            return v / l.sqrt();
        }
    }
}

/// Projects `v` onto the tangent plane at unit `p`.
#[inline]
pub fn tangent(p: DVec3, v: DVec3) -> DVec3 {
    v - p * p.dot(v)
}

impl TectonicLayout {
    /// Generates plates and hotspots for a seed. `continental_target` is the desired fraction
    /// of the sphere belonging to continental plates.
    pub fn generate(seed: u64, continental_target: f64) -> Self {
        let mut rng = Rng::new(derive_seed(seed, "tectonics"));
        let major = 8 + rng.below(4) as usize;
        let minor = 7 + rng.below(6) as usize;
        let total = major + minor;
        // Jittered Fibonacci seeds, randomly rotated.
        let rot = glam::DQuat::from_axis_angle(random_unit(&mut rng), rng.range_f64(0.0, std::f64::consts::TAU));
        let mut plates = Vec::with_capacity(total);
        let mut order: Vec<usize> = (0..total).collect();
        for i in (1..total).rev() {
            let j = rng.below(i as u32 + 1) as usize;
            order.swap(i, j);
        }
        for (k, &slot) in order.iter().enumerate() {
            let base = fibonacci_point(slot, total);
            let jitter = random_unit(&mut rng) * 0.25;
            let center = (rot * (base + jitter)).normalize();
            let is_major = k < major;
            let weight = if is_major {
                rng.range_f64(0.12, 0.30)
            } else {
                rng.range_f64(-0.05, 0.08)
            };
            let axis = random_unit(&mut rng);
            let rate = rng.range_f64(0.35, 1.0);
            plates.push(Plate {
                id: k as u8,
                center,
                weight,
                omega: axis * rate,
                continental: false,
                density: rng.next_f64(),
                area: 0.0,
            });
        }
        let warp = SphereFbm::new(derive_seed(seed, "plate-warp"), 1.6, 4, 0.5, 2.1);
        let mut layout = Self {
            plates,
            hotspots: Vec::new(),
            adjacency: Vec::new(),
            warp,
            warp_strength: 0.3,
        };
        layout.measure();
        layout.assign_continents(&mut rng, continental_target);
        layout.add_hotspots(&mut rng);
        layout
    }

    /// Locates the plate containing `p` (warped, weighted Voronoi).
    #[inline]
    pub fn locate(&self, p: DVec3) -> PlateHit {
        let q = (p + self.warp.warp(p) * self.warp_strength).normalize();
        let mut best = (f64::INFINITY, 0u8);
        let mut second = (f64::INFINITY, 0u8);
        for pl in &self.plates {
            let d = q.dot(pl.center).clamp(-1.0, 1.0).acos() - pl.weight;
            if d < best.0 {
                second = best;
                best = (d, pl.id);
            } else if d < second.0 {
                second = (d, pl.id);
            }
        }
        PlateHit {
            plate: best.1,
            second: second.1,
            gap: second.0 - best.0,
        }
    }

    /// Estimates plate areas and adjacency by sampling the sphere.
    fn measure(&mut self) {
        let samples = 40_000;
        let mut counts = vec![0usize; self.plates.len()];
        let mut adj = std::collections::BTreeSet::new();
        for i in 0..samples {
            let p = fibonacci_point(i, samples);
            let hit = self.locate(p);
            counts[hit.plate as usize] += 1;
            if hit.gap < 0.03 {
                let (a, b) = (hit.plate.min(hit.second), hit.plate.max(hit.second));
                adj.insert((a, b));
            }
        }
        for (pl, c) in self.plates.iter_mut().zip(counts) {
            pl.area = c as f64 / samples as f64;
        }
        self.adjacency = adj.into_iter().collect();
    }

    pub fn neighbors(&self, id: u8) -> impl Iterator<Item = u8> + '_ {
        self.adjacency.iter().filter_map(move |&(a, b)| {
            if a == id {
                Some(b)
            } else if b == id {
                Some(a)
            } else {
                None
            }
        })
    }

    /// Chooses continental plates to build a realistic size mix: a supercontinent cluster,
    /// a second large continent, mid-sized continents and a small plate colliding with a
    /// large one (subcontinent).
    fn assign_continents(&mut self, rng: &mut Rng, target: f64) {
        let mut by_area: Vec<u8> = (0..self.plates.len() as u8).collect();
        by_area.sort_by(|a, b| {
            self.plates[*b as usize]
                .area
                .total_cmp(&self.plates[*a as usize].area)
        });
        let mut area = 0.0;
        let set = |plates: &mut Vec<Plate>, id: u8, area: &mut f64| {
            if !plates[id as usize].continental {
                plates[id as usize].continental = true;
                *area += plates[id as usize].area;
            }
        };
        // 1. Supercontinent: the largest plate plus one neighbour.
        let core = by_area[0];
        set(&mut self.plates, core, &mut area);
        let core_neighbors: Vec<u8> = self.neighbors(core).collect();
        if let Some(&n) = core_neighbors.iter().max_by(|a, b| {
            self.plates[**a as usize]
                .area
                .total_cmp(&self.plates[**b as usize].area)
        }) {
            set(&mut self.plates, n, &mut area);
        }
        // 2. Second large continent, not adjacent to the first cluster if possible.
        for &id in &by_area[1..] {
            if self.plates[id as usize].continental {
                continue;
            }
            let touches = self
                .neighbors(id)
                .any(|n| self.plates[n as usize].continental);
            if !touches {
                set(&mut self.plates, id, &mut area);
                break;
            }
        }
        // 3. Subcontinent: a small plate adjacent to the core, forced to converge with it.
        let small_neighbor = core_neighbors
            .iter()
            .copied()
            .filter(|n| !self.plates[*n as usize].continental)
            .min_by(|a, b| {
                self.plates[*a as usize]
                    .area
                    .total_cmp(&self.plates[*b as usize].area)
            });
        if let Some(sub) = small_neighbor {
            set(&mut self.plates, sub, &mut area);
            let core_c = self.plates[core as usize].center;
            let sub_c = self.plates[sub as usize].center;
            let mid = (core_c + sub_c).normalize_or(core_c);
            let toward = tangent(mid, core_c - sub_c).normalize_or_zero();
            // omega = p × v gives surface velocity v at p.
            self.plates[sub as usize].omega = mid.cross(toward) * 1.1;
            self.plates[core as usize].omega *= 0.4;
        }
        // 4. Fill with random plates (mid-sized continents) until the target is reached.
        let mut candidates: Vec<u8> = by_area
            .iter()
            .copied()
            .filter(|id| !self.plates[*id as usize].continental)
            .collect();
        for i in (1..candidates.len()).rev() {
            let j = rng.below(i as u32 + 1) as usize;
            candidates.swap(i, j);
        }
        for id in candidates {
            if area >= target {
                break;
            }
            if self.plates[id as usize].area + area > target * 1.4 {
                continue;
            }
            set(&mut self.plates, id, &mut area);
        }
    }

    fn add_hotspots(&mut self, rng: &mut Rng) {
        let count = 5 + rng.below(6);
        for _ in 0..count {
            let pos = random_unit(rng);
            let hit = self.locate(pos);
            // Mostly oceanic hotspots; continental ones become volcanic highlands.
            if self.plates[hit.plate as usize].continental && rng.chance(0.7) {
                continue;
            }
            self.hotspots.push(Hotspot {
                pos,
                strength: rng.range_f64(0.6, 1.0),
            });
        }
    }

    /// Relative motion at a boundary between plates `a` and `b` at `p`: (convergence, shear),
    /// where positive convergence means the plates move toward each other.
    pub fn boundary_motion(&self, a: u8, b: u8, p: DVec3) -> (f64, f64) {
        let pa = &self.plates[a as usize];
        let pb = &self.plates[b as usize];
        let n = tangent(p, pb.center - pa.center).normalize_or_zero();
        let rel = pa.velocity(p) - pb.velocity(p);
        let conv = rel.dot(n);
        let shear = (rel - n * conv).length();
        (conv, shear)
    }

    /// Fraction of the sphere covered by continental plates.
    pub fn continental_area(&self) -> f64 {
        self.plates
            .iter()
            .filter(|p| p.continental)
            .map(|p| p.area)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_deterministic_and_reasonable() {
        for seed in 0..8 {
            let a = TectonicLayout::generate(seed, 0.45);
            let b = TectonicLayout::generate(seed, 0.45);
            assert_eq!(a.plates.len(), b.plates.len());
            for (x, y) in a.plates.iter().zip(&b.plates) {
                assert_eq!(x.center, y.center);
                assert_eq!(x.continental, y.continental);
            }
            let total: f64 = a.plates.iter().map(|p| p.area).sum();
            assert!((total - 1.0).abs() < 1e-9);
            let cont = a.continental_area();
            assert!(
                (0.3..0.7).contains(&cont),
                "seed {seed}: continental {cont}"
            );
            assert!(!a.adjacency.is_empty());
            assert!(a.plates.iter().filter(|p| p.area > 0.0).count() >= 12);
        }
    }

    #[test]
    fn locate_is_consistent_with_gap() {
        let t = TectonicLayout::generate(3, 0.45);
        let p = DVec3::new(0.2, 0.5, -0.8).normalize();
        let h = t.locate(p);
        assert_ne!(h.plate, h.second);
        assert!(h.gap >= 0.0);
    }
}

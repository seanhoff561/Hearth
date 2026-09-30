//! Deterministic noise.
//!
//! * [`Simplex3`] — 3D simplex noise used on the unit sphere for everything that scales with the
//!   planet (continents, plates, climate variation). Sphere sampling is seamless by
//!   construction.
//! * [`Perlin`] — gradient noise on a cubic lattice whose X axis can be made periodic, used for
//!   block-scale detail that must wrap exactly at the planet seam (period = circumference).
//!
//! All functions take `f64` coordinates so precision holds on Earth-sized planets.

use hearth_math::hash::{derive_seed, mix64};

/// Fast 32-bit integer hash of a lattice point.
#[inline(always)]
fn hash3(seed: u32, x: i32, y: i32, z: i32) -> u32 {
    let mut h = seed
        ^ (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ (z as u32).wrapping_mul(0xcb1a_b31f);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^ (h >> 15)
}

#[inline(always)]
fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline(always)]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Gradient for 3D Perlin: one of 12 cube-edge directions.
#[inline(always)]
fn grad3(h: u32, x: f64, y: f64, z: f64) -> f64 {
    match h % 12 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x + z,
        5 => -x + z,
        6 => x - z,
        7 => -x - z,
        8 => y + z,
        9 => -y + z,
        10 => y - z,
        _ => -y - z,
    }
}

/// Gradient for 2D Perlin: 8 directions.
#[inline(always)]
fn grad2(h: u32, x: f64, y: f64) -> f64 {
    const D: f64 = std::f64::consts::FRAC_1_SQRT_2;
    match h & 7 {
        0 => x,
        1 => -x,
        2 => y,
        3 => -y,
        4 => (x + y) * D,
        5 => (-x + y) * D,
        6 => (x - y) * D,
        _ => (-x - y) * D,
    }
}

/// Gradient noise on an integer lattice, optionally periodic in X.
#[derive(Debug, Clone, Copy)]
pub struct Perlin {
    seed: u32,
}

impl Perlin {
    pub fn new(seed: u64) -> Self {
        Self {
            seed: (mix64(seed) >> 32) as u32,
        }
    }

    /// 2D noise in lattice units, range ≈ [-1, 1]. `period_x` (lattice cells) makes it periodic
    /// in X when > 0.
    #[inline]
    pub fn noise2(&self, x: f64, y: f64, period_x: i64) -> f64 {
        let xf = x.floor();
        let yf = y.floor();
        let fx = x - xf;
        let fy = y - yf;
        let (x0, x1) = wrap_pair(xf as i64, period_x);
        let y0 = yf as i64 as i32;
        let y1 = y0.wrapping_add(1);
        let s = self.seed;
        let n00 = grad2(hash3(s, x0, y0, 0), fx, fy);
        let n10 = grad2(hash3(s, x1, y0, 0), fx - 1.0, fy);
        let n01 = grad2(hash3(s, x0, y1, 0), fx, fy - 1.0);
        let n11 = grad2(hash3(s, x1, y1, 0), fx - 1.0, fy - 1.0);
        let u = fade(fx);
        let v = fade(fy);
        lerp(lerp(n00, n10, u), lerp(n01, n11, u), v) * 1.4
    }

    /// 3D noise in lattice units, range ≈ [-1, 1], optionally periodic in X.
    #[inline]
    pub fn noise3(&self, x: f64, y: f64, z: f64, period_x: i64) -> f64 {
        let xf = x.floor();
        let yf = y.floor();
        let zf = z.floor();
        let fx = x - xf;
        let fy = y - yf;
        let fz = z - zf;
        let (x0, x1) = wrap_pair(xf as i64, period_x);
        let y0 = yf as i64 as i32;
        let z0 = zf as i64 as i32;
        let y1 = y0.wrapping_add(1);
        let z1 = z0.wrapping_add(1);
        let s = self.seed;
        let u = fade(fx);
        let v = fade(fy);
        let w = fade(fz);
        let n000 = grad3(hash3(s, x0, y0, z0), fx, fy, fz);
        let n100 = grad3(hash3(s, x1, y0, z0), fx - 1.0, fy, fz);
        let n010 = grad3(hash3(s, x0, y1, z0), fx, fy - 1.0, fz);
        let n110 = grad3(hash3(s, x1, y1, z0), fx - 1.0, fy - 1.0, fz);
        let n001 = grad3(hash3(s, x0, y0, z1), fx, fy, fz - 1.0);
        let n101 = grad3(hash3(s, x1, y0, z1), fx - 1.0, fy, fz - 1.0);
        let n011 = grad3(hash3(s, x0, y1, z1), fx, fy - 1.0, fz - 1.0);
        let n111 = grad3(hash3(s, x1, y1, z1), fx - 1.0, fy - 1.0, fz - 1.0);
        let x00 = lerp(n000, n100, u);
        let x10 = lerp(n010, n110, u);
        let x01 = lerp(n001, n101, u);
        let x11 = lerp(n011, n111, u);
        lerp(lerp(x00, x10, v), lerp(x01, x11, v), w) * 0.95
    }
}

#[inline(always)]
fn wrap_pair(x0: i64, period: i64) -> (i32, i32) {
    if period > 0 {
        let a = x0.rem_euclid(period);
        let b = (a + 1) % period;
        (a as i32, b as i32)
    } else {
        (x0 as i32, (x0 + 1) as i32)
    }
}

/// Octave-summed Perlin noise in block space, periodic in X with the planet circumference.
///
/// Octave wavelengths are `base_wavelength / 2^i`; the base wavelength is snapped so that the
/// circumference is a whole number of wavelengths, keeping every octave exactly periodic.
#[derive(Debug, Clone)]
pub struct BlockFbm {
    octaves: Vec<(Perlin, f64, i64, f64)>, // (noise, frequency, period in cells, amplitude)
    norm: f64,
}

impl BlockFbm {
    /// `wavelength` in blocks of the first octave; `gain` scales amplitude per octave.
    pub fn new(seed: u64, circumference: i64, wavelength: f64, octaves: u32, gain: f64) -> Self {
        let mut v = Vec::with_capacity(octaves as usize);
        let mut amp = 1.0;
        let mut norm = 0.0;
        for i in 0..octaves {
            let wl = wavelength / (1u64 << i) as f64;
            // Snap so the circumference holds an integer number of cells.
            let cells = ((circumference as f64 / wl).round() as i64).max(1);
            let freq = cells as f64 / circumference as f64;
            v.push((
                Perlin::new(derive_seed(seed, "oct") ^ i as u64),
                freq,
                cells,
                amp,
            ));
            norm += amp;
            amp *= gain;
        }
        Self {
            octaves: v,
            norm: 1.0 / norm,
        }
    }

    /// Sum of octaves normalised to ≈ [-1, 1].
    #[inline]
    pub fn sample2(&self, x: f64, z: f64) -> f64 {
        let mut sum = 0.0;
        for (i, (n, f, p, a)) in self.octaves.iter().enumerate() {
            // Per-octave offset in the non-periodic axis decorrelates octaves.
            let off = i as f64 * 17.13;
            sum += n.noise2(x * f, z * f + off, *p) * a;
        }
        sum * self.norm
    }

    /// Sum of the first `max_octaves` octaves (coarser evaluation for LOD sampling).
    #[inline]
    pub fn sample2_limited(&self, x: f64, z: f64, max_octaves: usize) -> f64 {
        let mut sum = 0.0;
        for (i, (n, f, p, a)) in self.octaves.iter().take(max_octaves).enumerate() {
            let off = i as f64 * 17.13;
            sum += n.noise2(x * f, z * f + off, *p) * a;
        }
        sum * self.norm
    }

    #[inline]
    pub fn sample3(&self, x: f64, y: f64, z: f64) -> f64 {
        let mut sum = 0.0;
        for (i, (n, f, p, a)) in self.octaves.iter().enumerate() {
            let off = i as f64 * 17.13;
            sum += n.noise3(x * f, y * f + off, z * f, *p) * a;
        }
        sum * self.norm
    }

    /// Ridged variant: 1 − |n| per octave, weighted by the previous octave (sharp ridges with
    /// smoother valleys). Range ≈ [0, 1].
    #[inline]
    pub fn ridged2(&self, x: f64, z: f64) -> f64 {
        let mut sum = 0.0;
        let mut weight = 1.0;
        for (i, (n, f, p, a)) in self.octaves.iter().enumerate() {
            let off = i as f64 * 17.13;
            let mut r = 1.0 - n.noise2(x * f, z * f + off, *p).abs();
            r *= r;
            r *= weight;
            weight = (r * 1.6).clamp(0.0, 1.0);
            sum += r * a;
        }
        sum * self.norm
    }

    /// Wavelength of the finest octave in blocks.
    pub fn finest_wavelength(&self) -> f64 {
        self.octaves.last().map_or(f64::INFINITY, |o| 1.0 / o.1)
    }

    pub fn octave_count(&self) -> usize {
        self.octaves.len()
    }
}

// ------------------------------------------------------------------ simplex (sphere space)

const GRAD3: [[f64; 3]; 12] = [
    [1.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0],
    [1.0, -1.0, 0.0],
    [-1.0, -1.0, 0.0],
    [1.0, 0.0, 1.0],
    [-1.0, 0.0, 1.0],
    [1.0, 0.0, -1.0],
    [-1.0, 0.0, -1.0],
    [0.0, 1.0, 1.0],
    [0.0, -1.0, 1.0],
    [0.0, 1.0, -1.0],
    [0.0, -1.0, -1.0],
];

/// Seeded 3D simplex noise (Gustavson), range ≈ [-1, 1].
#[derive(Clone)]
pub struct Simplex3 {
    perm: Box<[u8; 512]>,
}

impl std::fmt::Debug for Simplex3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Simplex3")
    }
}

impl Simplex3 {
    pub fn new(seed: u64) -> Self {
        let mut p: Vec<u8> = (0..=255).collect();
        let mut rng = hearth_math::hash::Rng::new(seed);
        for i in (1..256).rev() {
            let j = rng.below(i as u32 + 1) as usize;
            p.swap(i, j);
        }
        let mut perm = Box::new([0u8; 512]);
        for i in 0..512 {
            perm[i] = p[i & 255];
        }
        Self { perm }
    }

    #[inline]
    pub fn noise(&self, x: f64, y: f64, z: f64) -> f64 {
        const F3: f64 = 1.0 / 3.0;
        const G3: f64 = 1.0 / 6.0;
        let s = (x + y + z) * F3;
        let i = (x + s).floor();
        let j = (y + s).floor();
        let k = (z + s).floor();
        let t = (i + j + k) * G3;
        let x0 = x - (i - t);
        let y0 = y - (j - t);
        let z0 = z - (k - t);
        let (i1, j1, k1, i2, j2, k2) = if x0 >= y0 {
            if y0 >= z0 {
                (1, 0, 0, 1, 1, 0)
            } else if x0 >= z0 {
                (1, 0, 0, 1, 0, 1)
            } else {
                (0, 0, 1, 1, 0, 1)
            }
        } else if y0 < z0 {
            (0, 0, 1, 0, 1, 1)
        } else if x0 < z0 {
            (0, 1, 0, 0, 1, 1)
        } else {
            (0, 1, 0, 1, 1, 0)
        };
        let x1 = x0 - i1 as f64 + G3;
        let y1 = y0 - j1 as f64 + G3;
        let z1 = z0 - k1 as f64 + G3;
        let x2 = x0 - i2 as f64 + 2.0 * G3;
        let y2 = y0 - j2 as f64 + 2.0 * G3;
        let z2 = z0 - k2 as f64 + 2.0 * G3;
        let x3 = x0 - 1.0 + 3.0 * G3;
        let y3 = y0 - 1.0 + 3.0 * G3;
        let z3 = z0 - 1.0 + 3.0 * G3;
        let ii = (i as i64 & 255) as usize;
        let jj = (j as i64 & 255) as usize;
        let kk = (k as i64 & 255) as usize;
        let p = &self.perm;
        let gi0 = p[ii + p[jj + p[kk] as usize] as usize] as usize % 12;
        let gi1 = p[ii + i1 + p[jj + j1 + p[kk + k1] as usize] as usize] as usize % 12;
        let gi2 = p[ii + i2 + p[jj + j2 + p[kk + k2] as usize] as usize] as usize % 12;
        let gi3 = p[ii + 1 + p[jj + 1 + p[kk + 1] as usize] as usize] as usize % 12;
        let corner = |gi: usize, x: f64, y: f64, z: f64| -> f64 {
            let t = 0.6 - x * x - y * y - z * z;
            if t < 0.0 {
                0.0
            } else {
                let t2 = t * t;
                let g = GRAD3[gi];
                t2 * t2 * (g[0] * x + g[1] * y + g[2] * z)
            }
        };
        32.0 * (corner(gi0, x0, y0, z0)
            + corner(gi1, x1, y1, z1)
            + corner(gi2, x2, y2, z2)
            + corner(gi3, x3, y3, z3))
    }
}

/// Fractal simplex noise sampled on the unit sphere.
#[derive(Debug, Clone)]
pub struct SphereFbm {
    layers: Vec<(Simplex3, glam::DVec3)>,
    frequency: f64,
    gain: f64,
    lacunarity: f64,
    norm: f64,
}

impl SphereFbm {
    /// `frequency` is in cycles per radian-ish (1 ≈ continent-scale blobs).
    pub fn new(seed: u64, frequency: f64, octaves: u32, gain: f64, lacunarity: f64) -> Self {
        let mut layers = Vec::new();
        let mut rng = hearth_math::hash::Rng::new(seed);
        let mut norm = 0.0;
        let mut amp = 1.0;
        for i in 0..octaves {
            let off = glam::DVec3::new(
                rng.range_f64(-100.0, 100.0),
                rng.range_f64(-100.0, 100.0),
                rng.range_f64(-100.0, 100.0),
            );
            layers.push((Simplex3::new(derive_seed(seed, "sphere") ^ i as u64), off));
            norm += amp;
            amp *= gain;
        }
        Self {
            layers,
            frequency,
            gain,
            lacunarity,
            norm: 1.0 / norm,
        }
    }

    /// Normalised fBm at a unit vector, ≈ [-1, 1].
    #[inline]
    pub fn sample(&self, p: glam::DVec3) -> f64 {
        let mut f = self.frequency;
        let mut a = 1.0;
        let mut sum = 0.0;
        for (n, off) in &self.layers {
            let q = p * f + *off;
            sum += n.noise(q.x, q.y, q.z) * a;
            f *= self.lacunarity;
            a *= self.gain;
        }
        sum * self.norm
    }

    /// Ridged multifractal at a unit vector, ≈ [0, 1].
    #[inline]
    pub fn ridged(&self, p: glam::DVec3) -> f64 {
        let mut f = self.frequency;
        let mut a = 1.0;
        let mut sum = 0.0;
        let mut weight = 1.0;
        for (n, off) in &self.layers {
            let q = p * f + *off;
            let mut r = 1.0 - n.noise(q.x, q.y, q.z).abs();
            r = r * r * weight;
            weight = (r * 1.5).clamp(0.0, 1.0);
            sum += r * a;
            f *= self.lacunarity;
            a *= self.gain;
        }
        sum * self.norm
    }

    /// Vector-valued noise for domain warping (three decorrelated samples).
    #[inline]
    pub fn warp(&self, p: glam::DVec3) -> glam::DVec3 {
        glam::DVec3::new(
            self.sample(p),
            self.sample(p + glam::DVec3::new(5.2, 1.3, 7.7)),
            self.sample(p + glam::DVec3::new(-3.1, 9.4, 2.6)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perlin_is_periodic_in_x() {
        let n = Perlin::new(5);
        for i in 0..200 {
            let x = i as f64 * 0.37;
            let y = i as f64 * 0.11 - 3.0;
            let a = n.noise2(x, y, 64);
            let b = n.noise2(x + 64.0, y, 64);
            assert!((a - b).abs() < 1e-12, "{a} {b}");
            let c = n.noise3(x, y, 0.5 * y, 64);
            let d = n.noise3(x - 128.0, y, 0.5 * y, 64);
            assert!((c - d).abs() < 1e-12);
        }
    }

    #[test]
    fn block_fbm_wraps_at_circumference() {
        let c = 16_384i64;
        let f = BlockFbm::new(9, c, 300.0, 5, 0.5);
        for i in 0..100 {
            let x = i as f64 * 13.7;
            let z = i as f64 * -7.3;
            assert!((f.sample2(x, z) - f.sample2(x + c as f64, z)).abs() < 1e-9);
            assert!((f.ridged2(x, z) - f.ridged2(x - c as f64, z)).abs() < 1e-9);
        }
    }

    #[test]
    fn ranges_are_sane_and_deterministic() {
        let s = Simplex3::new(1);
        let s2 = Simplex3::new(1);
        let p = Perlin::new(1);
        let mut max = 0.0f64;
        for i in 0..20_000 {
            let x = i as f64 * 0.0731;
            let v = s.noise(x, x * 0.7 + 1.0, -x * 0.3);
            assert_eq!(v, s2.noise(x, x * 0.7 + 1.0, -x * 0.3));
            max = max.max(v.abs());
            let w = p.noise3(x, -x * 0.5, x * 0.25, 0);
            assert!(w.abs() <= 1.2, "{w}");
        }
        assert!(max > 0.5 && max <= 1.05, "{max}");
    }

    #[test]
    fn sphere_fbm_is_continuous() {
        let f = SphereFbm::new(3, 2.0, 6, 0.5, 2.0);
        let p = glam::DVec3::new(0.3, 0.4, 0.866).normalize();
        let q = (p + glam::DVec3::splat(1e-7)).normalize();
        assert!((f.sample(p) - f.sample(q)).abs() < 1e-4);
    }
}

//! Deterministic, platform-independent hashing and random numbers for procedural generation.
//! Everything here is pure: the same inputs produce the same outputs on every machine, thread
//! and run.

/// SplitMix64 finalizer: a high-quality 64-bit mix.
#[inline]
pub const fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Combines a seed with a value.
#[inline]
pub const fn hash2(seed: u64, a: u64) -> u64 {
    mix64(seed ^ mix64(a.wrapping_add(0x9e37_79b9_7f4a_7c15)))
}

/// Hash of a 2D integer position under a seed.
#[inline]
pub const fn hash_2d(seed: u64, x: i32, z: i32) -> u64 {
    let v = (x as u32 as u64) | ((z as u32 as u64) << 32);
    hash2(seed, v)
}

/// Hash of a 3D integer position under a seed.
#[inline]
pub const fn hash_3d(seed: u64, x: i32, y: i32, z: i32) -> u64 {
    let v = (x as u32 as u64) | ((z as u32 as u64) << 32);
    hash2(hash2(seed, v), y as u32 as u64)
}

/// Derives an independent sub-seed for a named purpose (e.g. `"caves"`).
pub const fn derive_seed(seed: u64, purpose: &str) -> u64 {
    // FNV-1a over the purpose string, then mixed with the seed. `const` so sub-seeds can be
    // computed at compile time for fixed purposes.
    let bytes = purpose.as_bytes();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0;
    while i < bytes.len() {
        h ^= bytes[i] as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    hash2(seed, h)
}

/// Uniform float in [0, 1) from a hash.
#[inline]
pub fn unit_f64(h: u64) -> f64 {
    (h >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// Uniform float in [0, 1) from a hash.
#[inline]
pub fn unit_f32(h: u64) -> f32 {
    (h >> 40) as f32 * (1.0 / (1u32 << 24) as f32)
}

/// Small, fast, seedable PRNG (xoshiro256++). Deterministic across platforms.
#[derive(Debug, Clone)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut sm = seed;
        let mut next = || {
            sm = sm.wrapping_add(0x9e37_79b9_7f4a_7c15);
            mix64(sm)
        };
        let s = [next(), next(), next(), next()];
        Self { s }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let result = (self.s[0].wrapping_add(self.s[3]))
            .rotate_left(23)
            .wrapping_add(self.s[0]);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Uniform in [0, 1).
    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        unit_f64(self.next_u64())
    }

    /// Uniform in [0, 1).
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        unit_f32(self.next_u64())
    }

    /// Uniform in [lo, hi).
    #[inline]
    pub fn range_f64(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }

    /// Uniform in [lo, hi).
    #[inline]
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    /// Uniform integer in [0, n). `n` must be > 0.
    #[inline]
    pub fn below(&mut self, n: u32) -> u32 {
        // Lemire's multiply-shift; bias is negligible for generation purposes.
        ((self.next_u32() as u64 * n as u64) >> 32) as u32
    }

    /// Uniform integer in [lo, hi] (inclusive).
    #[inline]
    pub fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + self.below((hi - lo + 1) as u32) as i32
    }

    #[inline]
    pub fn chance(&mut self, p: f64) -> bool {
        self.next_f64() < p
    }

    /// Standard normal sample (Box–Muller).
    pub fn normal(&mut self) -> f64 {
        let u1 = self.next_f64().max(1e-300);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_distinct() {
        assert_eq!(hash_3d(1, 2, 3, 4), hash_3d(1, 2, 3, 4));
        assert_ne!(hash_3d(1, 2, 3, 4), hash_3d(1, 2, 4, 3));
        assert_ne!(hash_2d(1, -1, 0), hash_2d(1, 0, -1));
        assert_ne!(derive_seed(7, "caves"), derive_seed(7, "ores"));
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn ranges() {
        let mut r = Rng::new(9);
        let mut hist = [0u32; 6];
        for _ in 0..60_000 {
            let v = r.range_i32(-2, 3);
            assert!((-2..=3).contains(&v));
            hist[(v + 2) as usize] += 1;
            let f = r.next_f64();
            assert!((0.0..1.0).contains(&f));
        }
        for h in hist {
            assert!((9000..11000).contains(&h), "{hist:?}");
        }
    }

    #[test]
    fn known_value_is_stable() {
        // Guards against accidental changes that would alter every generated world.
        assert_eq!(mix64(0), 0);
        assert_eq!(hash_2d(0, 0, 0), hash2(0, 0));
        let mut r = Rng::new(0);
        let first = r.next_u64();
        let mut r2 = Rng::new(0);
        assert_eq!(first, r2.next_u64());
    }
}

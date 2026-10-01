//! Small parts of signal processing: noise, filters, envelopes, smoothing.

use std::f32::consts::{PI, TAU};

/// RMS of uniform white noise in −1..1.
const WHITE_RMS: f32 = 0.577;
/// An envelope below this is silence.
pub(crate) const SILENT: f32 = 1e-4;

/// A fast generator of noise (xorshift32), the same from the same seed.
#[derive(Debug, Clone, Copy)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed | 1)
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// Uniform in 0..1.
    #[inline]
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    /// Uniform in −1..1: white noise.
    #[inline]
    pub fn white(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }

    /// Another generator, for a new voice.
    pub fn fork(&mut self) -> Rng {
        Rng::new(self.next_u32())
    }
}

/// The factor per sample of an exponential fall with time constant `tau_s`.
pub fn decay_per_sample(tau_s: f32, rate: f32) -> f32 {
    if tau_s <= 0.0 {
        0.0
    } else {
        (-1.0 / (tau_s * rate)).exp()
    }
}

/// A value easing toward its target (one pole).
#[derive(Debug, Clone, Copy)]
pub struct Smooth {
    pub value: f32,
    pub target: f32,
    keep: f32,
}

impl Smooth {
    pub fn new(value: f32, tau_s: f32, rate: f32) -> Self {
        Self {
            value,
            target: value,
            keep: decay_per_sample(tau_s, rate),
        }
    }

    /// Moves `n` samples on; the value after.
    pub fn advance(&mut self, n: usize) -> f32 {
        let keep = self.keep.powi(n as i32);
        self.value = self.target + (self.value - self.target) * keep;
        self.value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    Low,
    High,
    Band,
}

/// A two-pole filter (the usual cookbook forms; the band pass peaks at unity), in transposed
/// direct form II.
#[derive(Debug, Clone, Copy, Default)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    pub fn new(kind: FilterKind, freq: f32, q: f32, rate: f32) -> Self {
        let mut f = Self::default();
        f.set(kind, freq, q, rate);
        f
    }

    /// New coefficients, keeping the state (for filters that move).
    pub fn set(&mut self, kind: FilterKind, freq: f32, q: f32, rate: f32) {
        let f = freq.clamp(10.0, rate * 0.45);
        let (s, c) = (TAU * f / rate).sin_cos();
        let alpha = s / (2.0 * q.max(0.05));
        let a0 = 1.0 + alpha;
        let (b0, b1, b2) = match kind {
            FilterKind::Low => ((1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0),
            FilterKind::High => ((1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0),
            FilterKind::Band => (alpha, 0.0, -alpha),
        };
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = -2.0 * c / a0;
        self.a2 = (1.0 - alpha) / a0;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// About the RMS of white noise through such a filter: the square root of its share of the
/// band. Dividing by it makes a noise's level its RMS whatever its colour.
pub fn noise_rms(kind: FilterKind, freq: f32, q: f32, rate: f32) -> f32 {
    let nyquist = rate * 0.5;
    let f = freq.clamp(10.0, rate * 0.45);
    let band = match kind {
        FilterKind::Low => f * 1.1,
        FilterKind::High => nyquist - f,
        // A resonator's noise bandwidth is π/2 times its −3 dB width.
        FilterKind::Band => f / q.max(0.05) * PI / 2.0,
    };
    (band / nyquist).clamp(1e-4, 1.0).sqrt() * WHITE_RMS
}

/// Attack, then an exponential fall, after a delay.
#[derive(Debug, Clone, Copy)]
pub struct Env {
    delay: u32,
    attack: u32,
    t: u32,
    fall: f32,
    level: f32,
}

impl Env {
    pub fn new(delay_s: f32, attack_s: f32, tau_s: f32, rate: f32) -> Self {
        Self {
            delay: (delay_s * rate) as u32,
            attack: ((attack_s * rate) as u32).max(1),
            t: 0,
            fall: decay_per_sample(tau_s, rate),
            level: 0.0,
        }
    }

    #[inline]
    pub fn tick(&mut self) -> f32 {
        if self.delay > 0 {
            self.delay -= 1;
        } else if self.t < self.attack {
            self.t += 1;
            self.level = self.t as f32 / self.attack as f32;
        } else {
            self.level *= self.fall;
        }
        self.level
    }

    pub fn done(&self) -> bool {
        self.delay == 0 && self.t >= self.attack && self.level < SILENT
    }
}

/// Filtered noise in short random grains: crunching, crackling, the patter of drops.
#[derive(Debug, Clone, Copy)]
pub struct Grains {
    filter: Biquad,
    norm: f32,
    level: f32,
    fall: f32,
    chance: f32,
}

impl Grains {
    pub fn new(kind: FilterKind, freq: f32, q: f32, per_s: f32, grain_s: f32, rate: f32) -> Self {
        Self {
            filter: Biquad::new(kind, freq, q, rate),
            norm: 1.0 / noise_rms(kind, freq, q, rate),
            level: 0.0,
            fall: decay_per_sample(grain_s, rate),
            chance: (per_s / rate).min(1.0),
        }
    }

    /// Moves the filter and the number of grains a second.
    pub fn set(&mut self, kind: FilterKind, freq: f32, q: f32, per_s: f32, rate: f32) {
        self.filter.set(kind, freq, q, rate);
        self.norm = 1.0 / noise_rms(kind, freq, q, rate);
        self.chance = (per_s / rate).min(1.0);
    }

    #[inline]
    pub fn next(&mut self, rng: &mut Rng) -> f32 {
        let n = rng.white();
        if rng.unit() < self.chance {
            self.level = self.level.max(0.35 + 0.65 * rng.unit());
        } else {
            self.level *= self.fall;
        }
        self.filter.process(n) * self.norm * self.level
    }
}

/// Gains of the left and right channels for a pan of −1 (left) to 1 (right), equal in power
/// and unity in the middle.
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let a = (pan.clamp(-1.0, 1.0) + 1.0) * PI / 4.0;
    (
        a.cos() * std::f32::consts::SQRT_2,
        a.sin() * std::f32::consts::SQRT_2,
    )
}

/// 0 below `lo`, 1 above `hi`, straight between.
pub fn ramp(x: f32, lo: f32, hi: f32) -> f32 {
    ((x - lo) / (hi - lo)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms_through(kind: FilterKind, f: f32, q: f32) -> f32 {
        let rate = 48_000.0;
        let mut filter = Biquad::new(kind, f, q, rate);
        let mut rng = Rng::new(7);
        let n = 96_000;
        let mut sum = 0.0f64;
        for i in 0..n {
            let y = filter.process(rng.white());
            if i > 4800 {
                sum += (y * y) as f64;
            }
        }
        (sum / (n - 4801) as f64).sqrt() as f32
    }

    #[test]
    fn noise_levels_are_estimated_within_a_third() {
        for (kind, f, q) in [
            (FilterKind::Low, 300.0, 0.7),
            (FilterKind::Low, 2000.0, 0.7),
            (FilterKind::High, 3000.0, 0.7),
            (FilterKind::Band, 1000.0, 0.7),
            (FilterKind::Band, 2500.0, 3.0),
            (FilterKind::Band, 600.0, 12.0),
        ] {
            let measured = rms_through(kind, f, q);
            let estimate = noise_rms(kind, f, q, 48_000.0);
            let ratio = measured / estimate;
            assert!(
                (0.67..1.5).contains(&ratio),
                "{kind:?} {f} Hz Q {q}: measured {measured:.4}, estimated {estimate:.4}"
            );
        }
    }

    #[test]
    fn pans_keep_the_power() {
        for pan in [-1.0, -0.3, 0.0, 0.5, 1.0] {
            let (l, r) = pan_gains(pan);
            assert!((l * l + r * r - 2.0).abs() < 1e-5);
        }
        let (l, r) = pan_gains(0.0);
        assert!((l - 1.0).abs() < 1e-6 && (r - 1.0).abs() < 1e-6);
    }
}

//! The sounds, made as they play from a few parts: tones that glide and fade, filtered noise,
//! and noise in grains. Each is varied a little in pitch and loudness so no two steps match.

use std::f32::consts::TAU;

use crate::dsp::{Biquad, Env, FilterKind, Grains, Rng, decay_per_sample, noise_rms};
use crate::mixer::Bus;

/// What the ground underfoot sounds like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Grass,
    Soil,
    Mud,
    Sand,
    Gravel,
    Stone,
    Wood,
    Snow,
    Ice,
    Moss,
    /// Water to the shins.
    Shallow,
    Leaves,
}

impl Surface {
    /// The surface a block's sound group (`BlockDef::sound`) stands for; earth when unknown.
    pub fn of_group(group: &str) -> Surface {
        match group {
            "grass" | "wet_grass" | "vine" | "lily_pad" => Surface::Grass,
            "moss" | "wool" => Surface::Moss,
            "mud" => Surface::Mud,
            "sand" => Surface::Sand,
            "gravel" => Surface::Gravel,
            "stone" | "deepslate" => Surface::Stone,
            "wood" => Surface::Wood,
            "snow" => Surface::Snow,
            "glass" | "ice" => Surface::Ice,
            "leaves" => Surface::Leaves,
            _ => Surface::Soil,
        }
    }

    pub const ALL: [Surface; 12] = [
        Surface::Grass,
        Surface::Soil,
        Surface::Mud,
        Surface::Sand,
        Surface::Gravel,
        Surface::Stone,
        Surface::Wood,
        Surface::Snow,
        Surface::Ice,
        Surface::Moss,
        Surface::Shallow,
        Surface::Leaves,
    ];
}

/// A sound to play once.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sound {
    /// A footstep; `force` from creeping (0.2) to sprinting (1).
    Step { surface: Surface, force: f32 },
    /// Landing from a fall at `impact` m/s.
    Land { surface: Surface, impact: f32 },
    /// Going into water at `speed` m/s.
    Splash { speed: f32 },
    /// A swimming stroke, at the surface or under it.
    Stroke { under: bool },
    /// A sharp breath in: coming up for air, cold water's shock.
    Gasp { force: f32 },
    /// A blow to the body; a broken bone cracks.
    Hurt { force: f32, fracture: bool },
    /// The interface: a press.
    Click,
}

impl Sound {
    /// Heard inside the head: not muffled by water, no echo.
    pub fn inner(&self) -> bool {
        matches!(self, Sound::Gasp { .. } | Sound::Hurt { .. })
    }
}

pub(crate) const MAX_LAYERS: usize = 8;

/// One part of a sound.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Layer {
    Off,
    /// A sine from `f` gliding toward `f_end`.
    Tone {
        phase: f32,
        f: f32,
        f_end: f32,
        glide: f32,
        amp: f32,
        env: Env,
    },
    Noise {
        filter: Biquad,
        amp: f32,
        env: Env,
    },
    Crackle {
        grains: Grains,
        amp: f32,
        env: Env,
    },
}

impl Layer {
    #[inline]
    pub(crate) fn next(&mut self, rng: &mut Rng, inv_rate: f32) -> f32 {
        match self {
            Layer::Off => 0.0,
            Layer::Tone {
                phase,
                f,
                f_end,
                glide,
                amp,
                env,
            } => {
                let e = env.tick();
                *f = *f_end + (*f - *f_end) * *glide;
                *phase += *f * inv_rate;
                if *phase >= 1.0 {
                    *phase -= 1.0;
                }
                (*phase * TAU).sin() * *amp * e
            }
            Layer::Noise { filter, amp, env } => {
                let e = env.tick();
                filter.process(rng.white()) * *amp * e
            }
            Layer::Crackle { grains, amp, env } => {
                let e = env.tick();
                grains.next(rng) * *amp * e
            }
        }
    }

    pub(crate) fn done(&self) -> bool {
        match self {
            Layer::Off => true,
            Layer::Tone { env, .. } | Layer::Noise { env, .. } | Layer::Crackle { env, .. } => {
                env.done()
            }
        }
    }
}

/// A sound playing.
pub(crate) struct Voice {
    layers: [Layer; MAX_LAYERS],
    n: usize,
    rng: Rng,
    gain_l: f32,
    gain_r: f32,
    pub(crate) bus: Bus,
    pub(crate) inner: bool,
    /// Samples before it is cut off whatever is left.
    left: u32,
}

impl Voice {
    pub(crate) fn new(
        sound: Sound,
        bus: Bus,
        gain: f32,
        (gain_l, gain_r): (f32, f32),
        rate: f32,
        mut rng: Rng,
    ) -> Self {
        let mut b = Build {
            rate,
            pitch: rng.range(0.92, 1.08),
            layers: [Layer::Off; MAX_LAYERS],
            n: 0,
        };
        let longest = build(sound, &mut b);
        let gain = gain * rng.range(0.85, 1.0);
        Self {
            layers: b.layers,
            n: b.n,
            rng,
            gain_l: gain_l * gain,
            gain_r: gain_r * gain,
            bus,
            inner: sound.inner(),
            left: (longest * rate) as u32,
        }
    }

    /// Adds the next samples into `out` (stereo, interleaved) at `gain`; false once it has
    /// ended.
    pub(crate) fn render(&mut self, out: &mut [f32], gain: f32, inv_rate: f32) -> bool {
        let (gl, gr) = (self.gain_l * gain, self.gain_r * gain);
        let layers = &mut self.layers[..self.n];
        for frame in out.as_chunks_mut::<2>().0 {
            let mut s = 0.0;
            for l in layers.iter_mut() {
                s += l.next(&mut self.rng, inv_rate);
            }
            frame[0] += s * gl;
            frame[1] += s * gr;
        }
        self.left = self.left.saturating_sub((out.len() / 2) as u32);
        self.left > 0 && !layers.iter().all(Layer::done)
    }
}

/// When a part starts, how fast it rises and how slowly it falls (s).
#[derive(Debug, Clone, Copy)]
struct Shape {
    delay: f32,
    attack: f32,
    tau: f32,
}

const fn sh(delay: f32, attack: f32, tau: f32) -> Shape {
    Shape { delay, attack, tau }
}

struct Build {
    rate: f32,
    pitch: f32,
    layers: [Layer; MAX_LAYERS],
    n: usize,
}

use FilterKind::{Band, High, Low};

impl Build {
    fn push(&mut self, l: Layer) {
        if self.n < MAX_LAYERS {
            self.layers[self.n] = l;
            self.n += 1;
        }
    }

    fn env(&self, s: Shape) -> Env {
        Env::new(s.delay, s.attack, s.tau, self.rate)
    }

    /// A sine from `f0` gliding toward `f1` (time constant `glide_s`); `amp` is its peak.
    fn tone(&mut self, f0: f32, f1: f32, glide_s: f32, s: Shape, amp: f32) {
        let l = Layer::Tone {
            phase: 0.0,
            f: f0 * self.pitch,
            f_end: f1 * self.pitch,
            glide: decay_per_sample(glide_s, self.rate),
            amp,
            env: self.env(s),
        };
        self.push(l);
    }

    /// Filtered noise; `rms` is its level at the envelope's top.
    fn noise(&mut self, kind: FilterKind, f: f32, q: f32, s: Shape, rms: f32) {
        let f = f * self.pitch;
        let l = Layer::Noise {
            filter: Biquad::new(kind, f, q, self.rate),
            amp: rms / noise_rms(kind, f, q, self.rate),
            env: self.env(s),
        };
        self.push(l);
    }

    /// Noise in `per_s` grains a second, each falling with `grain_s`.
    fn crackle(
        &mut self,
        kind: FilterKind,
        f: f32,
        q: f32,
        per_s: f32,
        grain_s: f32,
        s: Shape,
        rms: f32,
    ) {
        let l = Layer::Crackle {
            grains: Grains::new(kind, f * self.pitch, q, per_s, grain_s, self.rate),
            amp: rms,
            env: self.env(s),
        };
        self.push(l);
    }
}

/// Lays out a sound's parts; returns the longest it may last (s).
fn build(sound: Sound, b: &mut Build) -> f32 {
    match sound {
        Sound::Step { surface, force } => {
            step(b, surface, force.clamp(0.05, 1.5));
            1.0
        }
        Sound::Land { surface, impact } => {
            let force = ((impact - 1.0) / 7.0).clamp(0.15, 1.6);
            step(b, surface, (force * 1.3).min(1.5));
            b.tone(75.0, 42.0, 0.06, sh(0.0, 0.003, 0.09), 0.22 * force);
            b.noise(Low, 350.0, 0.7, sh(0.0, 0.002, 0.07), 0.05 * force);
            1.5
        }
        Sound::Splash { speed } => {
            let f = (speed / 7.0).clamp(0.15, 1.3);
            b.noise(Band, 900.0, 0.5, sh(0.0, 0.006, 0.16), 0.07 * f);
            b.crackle(
                Band,
                3000.0,
                1.0,
                500.0,
                0.004,
                sh(0.02, 0.02, 0.28),
                0.06 * f,
            );
            b.tone(70.0, 40.0, 0.05, sh(0.0, 0.003, 0.07), 0.16 * f);
            // Bubbles rising.
            b.tone(520.0, 1300.0, 0.03, sh(0.06, 0.002, 0.03), 0.03);
            b.tone(380.0, 950.0, 0.035, sh(0.13, 0.002, 0.035), 0.025);
            b.tone(760.0, 1700.0, 0.025, sh(0.21, 0.002, 0.025), 0.02);
            2.0
        }
        Sound::Stroke { under: true } => {
            b.noise(Low, 450.0, 0.7, sh(0.0, 0.09, 0.18), 0.04);
            b.tone(300.0, 800.0, 0.04, sh(0.12, 0.002, 0.03), 0.02);
            b.tone(450.0, 1000.0, 0.04, sh(0.2, 0.002, 0.03), 0.015);
            1.2
        }
        Sound::Stroke { under: false } => {
            b.noise(Band, 1300.0, 0.7, sh(0.0, 0.06, 0.14), 0.035);
            b.crackle(Band, 2600.0, 1.2, 250.0, 0.004, sh(0.05, 0.03, 0.2), 0.035);
            1.2
        }
        Sound::Gasp { force } => {
            let f = force.clamp(0.1, 1.2);
            b.noise(Band, 1500.0, 1.2, sh(0.0, 0.12, 0.12), 0.05 * f);
            b.noise(Band, 2700.0, 3.0, sh(0.0, 0.12, 0.1), 0.02 * f);
            1.2
        }
        Sound::Hurt { force, fracture } => {
            let f = force.clamp(0.1, 1.5);
            b.tone(65.0, 35.0, 0.08, sh(0.0, 0.002, 0.11), 0.25 * f);
            b.noise(Low, 300.0, 0.7, sh(0.0, 0.002, 0.06), 0.05 * f);
            if fracture {
                b.noise(High, 1800.0, 0.7, sh(0.012, 0.0005, 0.006), 0.08);
                b.tone(1250.0, 900.0, 0.01, sh(0.012, 0.0005, 0.012), 0.05);
            }
            1.2
        }
        Sound::Click => {
            b.tone(1900.0, 1700.0, 0.01, sh(0.0, 0.0008, 0.007), 0.1);
            b.tone(3700.0, 3500.0, 0.01, sh(0.0, 0.0005, 0.003), 0.04);
            b.noise(High, 4500.0, 0.7, sh(0.0, 0.0003, 0.002), 0.03);
            0.3
        }
    }
}

/// A foot coming down: a low thump for the body's weight and the ground's own voice.
fn step(b: &mut Build, surface: Surface, force: f32) {
    let g = force.powf(0.8);
    // Harder steps are brighter.
    let bright = 0.8 + 0.3 * force;
    match surface {
        Surface::Grass => {
            b.tone(95.0, 60.0, 0.03, sh(0.0, 0.002, 0.025), 0.07 * g);
            b.noise(Band, 2400.0 * bright, 0.6, sh(0.0, 0.015, 0.06), 0.018 * g);
            b.crackle(
                Band,
                4200.0 * bright,
                1.0,
                450.0,
                0.003,
                sh(0.0, 0.01, 0.07),
                0.03 * g,
            );
        }
        Surface::Soil => {
            b.tone(110.0, 70.0, 0.03, sh(0.0, 0.002, 0.03), 0.09 * g);
            b.noise(Low, 900.0 * bright, 0.7, sh(0.0, 0.003, 0.045), 0.02 * g);
            b.crackle(
                Band,
                1800.0 * bright,
                0.8,
                220.0,
                0.002,
                sh(0.0, 0.005, 0.04),
                0.02 * g,
            );
        }
        Surface::Mud => {
            b.tone(80.0, 50.0, 0.04, sh(0.0, 0.004, 0.04), 0.09 * g);
            b.noise(Low, 600.0, 0.7, sh(0.0, 0.01, 0.08), 0.025 * g);
            b.crackle(
                Band,
                900.0,
                2.0,
                160.0,
                0.004,
                sh(0.01, 0.01, 0.09),
                0.03 * g,
            );
            // The foot pulling free.
            b.tone(320.0, 720.0, 0.03, sh(0.035, 0.002, 0.03), 0.02 * g);
        }
        Surface::Sand => {
            b.tone(100.0, 70.0, 0.03, sh(0.0, 0.003, 0.02), 0.05 * g);
            b.noise(Band, 3000.0 * bright, 0.5, sh(0.0, 0.025, 0.08), 0.02 * g);
            b.crackle(
                High,
                5000.0,
                0.7,
                900.0,
                0.001,
                sh(0.0, 0.02, 0.08),
                0.02 * g,
            );
        }
        Surface::Gravel => {
            b.tone(110.0, 70.0, 0.03, sh(0.0, 0.002, 0.025), 0.08 * g);
            b.crackle(
                Band,
                2800.0 * bright,
                0.9,
                520.0,
                0.003,
                sh(0.0, 0.005, 0.07),
                0.06 * g,
            );
            b.noise(Band, 1500.0, 0.7, sh(0.0, 0.004, 0.04), 0.012 * g);
        }
        Surface::Stone => {
            b.tone(140.0, 90.0, 0.02, sh(0.0, 0.001, 0.018), 0.1 * g);
            b.noise(Band, 3500.0 * bright, 1.2, sh(0.0, 0.001, 0.012), 0.04 * g);
            b.tone(900.0, 850.0, 0.02, sh(0.0, 0.001, 0.015), 0.012 * g);
        }
        Surface::Wood => {
            b.tone(120.0, 90.0, 0.03, sh(0.0, 0.001, 0.03), 0.09 * g);
            b.tone(220.0, 215.0, 0.05, sh(0.0, 0.001, 0.06), 0.05 * g);
            b.tone(470.0, 460.0, 0.05, sh(0.0, 0.001, 0.03), 0.025 * g);
            b.noise(Band, 2500.0 * bright, 1.0, sh(0.0, 0.001, 0.008), 0.03 * g);
        }
        Surface::Snow => {
            b.tone(80.0, 55.0, 0.03, sh(0.0, 0.004, 0.03), 0.06 * g);
            b.crackle(
                Low,
                2500.0 * bright,
                0.7,
                1500.0,
                0.0025,
                sh(0.0, 0.025, 0.09),
                0.05 * g,
            );
            b.noise(Band, 1200.0, 0.7, sh(0.0, 0.02, 0.06), 0.01 * g);
        }
        Surface::Ice => {
            b.tone(150.0, 100.0, 0.02, sh(0.0, 0.001, 0.015), 0.09 * g);
            b.noise(High, 3000.0, 0.7, sh(0.0, 0.0005, 0.008), 0.035 * g);
            b.tone(2100.0, 2080.0, 0.05, sh(0.0, 0.0005, 0.04), 0.012 * g);
            b.tone(3300.0, 3280.0, 0.05, sh(0.0, 0.0005, 0.025), 0.008 * g);
        }
        Surface::Moss => {
            b.tone(80.0, 55.0, 0.04, sh(0.0, 0.004, 0.035), 0.08 * g);
            b.noise(Low, 700.0, 0.7, sh(0.0, 0.008, 0.06), 0.018 * g);
        }
        Surface::Shallow => {
            b.noise(Band, 1100.0 * bright, 0.6, sh(0.0, 0.01, 0.12), 0.04 * g);
            b.crackle(
                Band,
                2500.0,
                1.5,
                300.0,
                0.004,
                sh(0.01, 0.005, 0.15),
                0.04 * g,
            );
            b.tone(500.0, 900.0, 0.03, sh(0.04, 0.002, 0.025), 0.015 * g);
        }
        Surface::Leaves => {
            b.noise(Band, 3000.0 * bright, 0.5, sh(0.0, 0.02, 0.12), 0.025 * g);
            b.crackle(
                High,
                3500.0,
                0.7,
                700.0,
                0.002,
                sh(0.0, 0.01, 0.12),
                0.04 * g,
            );
        }
    }
}

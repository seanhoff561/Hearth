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

/// The kind of an animal's call (as its species' data names it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cry {
    Roar,
    Bark,
    Grunt,
    Squeal,
    Howl,
    Growl,
    Hiss,
    Hoot,
    Song,
    Caw,
    Drum,
    Croak,
    Scream,
    Bellow,
    Gobble,
    Chatter,
    Rattle,
    Huff,
    Buzz,
}

impl Cry {
    pub const ALL: [Cry; 19] = [
        Cry::Roar,
        Cry::Bark,
        Cry::Grunt,
        Cry::Squeal,
        Cry::Howl,
        Cry::Growl,
        Cry::Hiss,
        Cry::Hoot,
        Cry::Song,
        Cry::Caw,
        Cry::Drum,
        Cry::Croak,
        Cry::Scream,
        Cry::Bellow,
        Cry::Gobble,
        Cry::Chatter,
        Cry::Rattle,
        Cry::Huff,
        Cry::Buzz,
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
    /// An empty stomach rumbling.
    Stomach { force: f32 },
    /// A dry swallow.
    Swallow,
    /// Pushing through foliage or brush; `force` by how dense and how fast.
    Rustle { force: f32 },
    /// The interface: a press.
    Click,
    /// Something built giving way (V2-8): wood cracks and splinters, stone grinds and
    /// thuds, brush and earth slump; `force` by its weight, 0.1 to 1.5.
    Break { surface: Surface, force: f32 },
    /// A blow or a thing flung landing on an animal: the dull smack of it driven into flesh, or
    /// the knock of it turned on hide and bone; `force` 0.2 to 1.5.
    Strike { force: f32, glancing: bool },
    /// An animal's call: its kind, its pitch's range (Hz), how long (s), and its own variety
    /// (a species' song keeps its notes).
    Call {
        cry: Cry,
        lo_hz: f32,
        hi_hz: f32,
        seconds: f32,
        variety: u32,
    },
}

impl Sound {
    /// Heard inside the head: not muffled by water, no echo.
    pub fn inner(&self) -> bool {
        matches!(
            self,
            Sound::Gasp { .. } | Sound::Hurt { .. } | Sound::Stomach { .. } | Sound::Swallow
        )
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
        Sound::Strike { force, glancing } => {
            let f = force.clamp(0.2, 1.5);
            if glancing {
                b.noise(Band, 1700.0, 1.2, sh(0.0, 0.0008, 0.03), 0.07 * f);
                b.tone(460.0, 320.0, 0.02, sh(0.0, 0.0008, 0.035), 0.05 * f);
                b.noise(High, 3200.0, 0.8, sh(0.01, 0.01, 0.06), 0.02 * f);
            } else {
                b.noise(Band, 650.0, 0.9, sh(0.0, 0.001, 0.05), 0.12 * f);
                b.tone(120.0, 60.0, 0.05, sh(0.0, 0.002, 0.09), 0.22 * f);
                b.noise(Low, 260.0, 0.7, sh(0.0, 0.002, 0.08), 0.06 * f);
            }
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
        Sound::Stomach { force } => {
            // A low churning with gurgles through it.
            let f = force.clamp(0.2, 1.2);
            b.noise(Low, 160.0, 0.9, sh(0.0, 0.2, 0.28), 0.02 * f);
            b.crackle(Band, 320.0, 3.0, 40.0, 0.02, sh(0.1, 0.15, 0.3), 0.03 * f);
            b.tone(110.0, 70.0, 0.4, sh(0.15, 0.1, 0.25), 0.035 * f);
            b.tone(180.0, 260.0, 0.1, sh(0.5, 0.02, 0.08), 0.012 * f);
            2.0
        }
        Sound::Swallow => {
            b.noise(Band, 1100.0, 2.0, sh(0.0, 0.01, 0.04), 0.03);
            b.tone(260.0, 150.0, 0.05, sh(0.05, 0.005, 0.07), 0.05);
            b.noise(Low, 300.0, 0.7, sh(0.06, 0.01, 0.06), 0.03);
            1.0
        }
        Sound::Rustle { force } => {
            // Leaves brushing past: a soft swish of high noise with crisp crackle in it.
            let g = force.clamp(0.1, 1.0);
            b.noise(Band, 2600.0, 0.6, sh(0.0, 0.05, 0.25), 0.03 * g);
            b.noise(High, 5200.0, 0.8, sh(0.02, 0.04, 0.2), 0.015 * g);
            b.crackle(
                High,
                4200.0,
                0.8,
                500.0,
                0.002,
                sh(0.0, 0.03, 0.22),
                0.012 * g,
            );
            1.0
        }
        Sound::Break { surface, force } => {
            let f = force.clamp(0.1, 1.5);
            match surface {
                Surface::Wood => {
                    // The crack, then the splintering, then the thump of the weight let go.
                    b.noise(High, 2600.0, 0.8, sh(0.0, 0.0005, 0.012), 0.16 * f);
                    b.tone(900.0, 420.0, 0.02, sh(0.0, 0.0005, 0.02), 0.06 * f);
                    b.crackle(
                        Band,
                        3200.0,
                        1.2,
                        600.0,
                        0.003,
                        sh(0.01, 0.01, 0.3),
                        0.07 * f,
                    );
                    b.tone(95.0, 50.0, 0.08, sh(0.05, 0.003, 0.14), 0.2 * f);
                }
                Surface::Stone | Surface::Gravel | Surface::Ice => {
                    // Grinding as the stones shift, and the deep knock of their fall.
                    b.noise(Low, 500.0, 0.8, sh(0.0, 0.03, 0.35), 0.07 * f);
                    b.crackle(
                        Band,
                        1400.0,
                        1.0,
                        300.0,
                        0.006,
                        sh(0.0, 0.02, 0.4),
                        0.06 * f,
                    );
                    b.tone(60.0, 32.0, 0.12, sh(0.08, 0.004, 0.2), 0.3 * f);
                }
                _ => {
                    b.noise(Low, 700.0, 0.7, sh(0.0, 0.04, 0.3), 0.06 * f);
                    b.crackle(
                        Band,
                        2400.0,
                        1.0,
                        350.0,
                        0.003,
                        sh(0.0, 0.03, 0.25),
                        0.04 * f,
                    );
                    b.tone(80.0, 45.0, 0.08, sh(0.04, 0.004, 0.12), 0.12 * f);
                }
            }
            1.6
        }
        Sound::Click => {
            b.tone(1900.0, 1700.0, 0.01, sh(0.0, 0.0008, 0.007), 0.1);
            b.tone(3700.0, 3500.0, 0.01, sh(0.0, 0.0005, 0.003), 0.04);
            b.noise(High, 4500.0, 0.7, sh(0.0, 0.0003, 0.002), 0.03);
            0.3
        }
        Sound::Call {
            cry,
            lo_hz,
            hi_hz,
            seconds,
            variety,
        } => {
            let lo = lo_hz.clamp(20.0, 12_000.0);
            let hi = hi_hz.clamp(lo, 16_000.0);
            let secs = seconds.clamp(0.05, 6.0);
            call(b, cry, lo, hi, secs, variety);
            secs + 1.5
        }
    }
}

/// A number 0–1 from a species' variety and a note's place in its call (its song keeps its
/// notes).
fn note(variety: u32, k: u32) -> f32 {
    let h = (variety ^ k.wrapping_mul(0x9e37_79b9)).wrapping_mul(0x85eb_ca6b);
    let h = h ^ (h >> 13);
    (h.wrapping_mul(0xc2b2_ae35) >> 8) as f32 / (1u32 << 24) as f32
}

/// An animal's call, from its kind, pitch and length: a red deer's roar a long rough bellow
/// from the chest, a fox's bark a sharp yelp, a wolf's howl a long rising and falling tone, an
/// owl's hoot a soft quavering note or three, a songbird's song quick notes up and down its
/// range, a crow's caw a harsh call or two, a woodpecker's drumming a roll of blows, a
/// rattlesnake's rattle a dry whirr.
fn call(b: &mut Build, cry: Cry, lo: f32, hi: f32, secs: f32, variety: u32) {
    let mid = (lo * hi).sqrt();
    match cry {
        Cry::Roar => {
            b.tone(
                lo * 1.3,
                lo,
                secs * 0.5,
                sh(0.0, secs * 0.15, secs * 0.45),
                0.14,
            );
            b.tone(
                lo * 2.6,
                lo * 2.0,
                secs * 0.5,
                sh(0.0, secs * 0.15, secs * 0.4),
                0.07,
            );
            b.tone(
                lo * 3.9,
                lo * 3.0,
                secs * 0.5,
                sh(0.0, secs * 0.15, secs * 0.35),
                0.035,
            );
            b.noise(Band, lo * 2.5, 0.8, sh(0.0, secs * 0.2, secs * 0.45), 0.05);
            b.crackle(
                Low,
                lo * 2.0,
                0.7,
                35.0,
                0.012,
                sh(0.0, secs * 0.15, secs * 0.45),
                0.04,
            );
        }
        Cry::Bellow => {
            b.tone(mid, lo, secs * 0.6, sh(0.0, secs * 0.2, secs * 0.5), 0.12);
            b.tone(
                mid * 2.0,
                lo * 2.0,
                secs * 0.6,
                sh(0.0, secs * 0.2, secs * 0.45),
                0.05,
            );
            b.noise(Band, mid * 1.5, 1.0, sh(0.0, secs * 0.2, secs * 0.4), 0.03);
        }
        Cry::Bark => {
            b.noise(Band, mid, 1.4, sh(0.0, 0.004, 0.07), 0.16);
            b.tone(hi, lo, 0.04, sh(0.0, 0.003, 0.06), 0.12);
            b.tone(hi * 2.0, lo * 2.0, 0.04, sh(0.0, 0.003, 0.05), 0.05);
        }
        Cry::Huff => {
            b.noise(High, 900.0, 0.7, sh(0.0, 0.01, 0.12), 0.1);
            b.noise(Band, 1800.0, 1.0, sh(0.0, 0.01, 0.1), 0.05);
        }
        Cry::Grunt => {
            for k in 0..(1 + (secs > 0.4) as u32) {
                let d = k as f32 * 0.22;
                b.tone(lo * 1.2, lo, 0.05, sh(d, 0.008, 0.08), 0.14);
                b.noise(Low, lo * 3.0, 0.8, sh(d, 0.005, 0.07), 0.08);
            }
        }
        Cry::Squeal => {
            b.tone(lo, hi, secs * 0.4, sh(0.0, 0.02, secs * 0.4), 0.08);
            b.tone(
                lo * 2.0,
                hi * 2.0,
                secs * 0.4,
                sh(0.0, 0.02, secs * 0.35),
                0.03,
            );
            b.noise(Band, hi, 2.0, sh(0.0, 0.02, secs * 0.3), 0.02);
        }
        Cry::Howl => {
            // Rising, held, falling away.
            b.tone(lo, hi, secs * 0.25, sh(0.0, secs * 0.15, secs * 0.35), 0.1);
            b.tone(
                hi,
                lo * 0.9,
                secs * 0.3,
                sh(secs * 0.5, 0.2, secs * 0.3),
                0.08,
            );
            b.tone(
                lo * 2.0,
                hi * 2.0,
                secs * 0.25,
                sh(0.0, secs * 0.15, secs * 0.3),
                0.02,
            );
            b.noise(Band, mid, 4.0, sh(0.0, secs * 0.2, secs * 0.4), 0.006);
        }
        Cry::Growl => {
            b.noise(Low, lo * 2.0, 0.9, sh(0.0, 0.05, secs * 0.5), 0.1);
            b.crackle(
                Low,
                lo * 3.0,
                0.8,
                28.0,
                0.015,
                sh(0.0, 0.05, secs * 0.5),
                0.06,
            );
            b.tone(lo, lo * 0.9, secs, sh(0.0, 0.05, secs * 0.5), 0.05);
        }
        Cry::Hiss => {
            b.noise(Band, mid.max(2000.0), 0.6, sh(0.0, 0.03, secs * 0.5), 0.07);
        }
        Cry::Hoot => {
            // A soft note and, after a pause, a quavering run.
            for (k, d) in [0.0, 0.5, 0.62, 0.74].into_iter().enumerate() {
                let f = mid * (1.02 - 0.04 * k as f32);
                b.tone(f, f * 0.95, 0.15, sh(d * secs, 0.04, 0.12), 0.1);
            }
        }
        Cry::Song => {
            // Quick notes up and down its range, the same for its kind.
            let n = 7;
            for k in 0..n {
                let f = lo + (hi - lo) * note(variety, k);
                let g = 0.85 + 0.35 * note(variety, k + 31);
                let dur = 0.03 + 0.07 * note(variety, k + 57);
                b.tone(
                    f,
                    f * g,
                    0.03,
                    sh(k as f32 * secs / n as f32, 0.004, dur),
                    0.045,
                );
            }
        }
        Cry::Caw => {
            for k in 0..2 {
                let d = k as f32 * 0.4;
                b.noise(Band, mid, 3.5, sh(d, 0.01, 0.16), 0.08);
                b.tone(lo * 1.1, lo, 0.1, sh(d, 0.01, 0.13), 0.06);
                b.tone(lo * 2.2, lo * 2.0, 0.1, sh(d, 0.01, 0.1), 0.04);
            }
        }
        Cry::Drum => {
            b.crackle(
                Band,
                1200.0,
                1.5,
                17.0,
                0.006,
                sh(0.0, 0.01, secs * 0.7),
                0.2,
            );
        }
        Cry::Croak => {
            b.crackle(
                Low,
                lo * 1.5,
                1.0,
                20.0,
                0.015,
                sh(0.0, 0.02, secs * 0.4),
                0.08,
            );
            b.tone(lo, lo * 0.95, secs, sh(0.0, 0.02, secs * 0.4), 0.05);
        }
        Cry::Scream => {
            b.tone(hi, mid, secs * 0.5, sh(0.0, 0.02, secs * 0.4), 0.1);
            b.tone(
                hi * 2.0,
                mid * 2.0,
                secs * 0.5,
                sh(0.0, 0.02, secs * 0.35),
                0.04,
            );
            b.noise(Band, hi, 1.2, sh(0.0, 0.02, secs * 0.35), 0.04);
        }
        Cry::Gobble => {
            b.crackle(Band, mid, 2.0, 14.0, 0.03, sh(0.0, 0.02, secs * 0.4), 0.12);
            b.tone(mid, mid * 0.9, secs, sh(0.0, 0.02, secs * 0.4), 0.03);
        }
        Cry::Chatter => {
            b.crackle(
                Band,
                hi * 0.8,
                1.5,
                13.0,
                0.02,
                sh(0.0, 0.01, secs * 0.45),
                0.08,
            );
        }
        Cry::Rattle => {
            b.crackle(
                High,
                5000.0,
                0.7,
                50.0,
                0.005,
                sh(0.0, 0.05, secs * 0.6),
                0.07,
            );
        }
        Cry::Buzz => {
            b.tone(mid, mid, 1.0, sh(0.0, 0.05, secs * 0.5), 0.03);
            b.tone(mid * 2.0, mid * 2.0, 1.0, sh(0.0, 0.05, secs * 0.5), 0.02);
            b.tone(mid * 3.0, mid * 3.0, 1.0, sh(0.0, 0.05, secs * 0.5), 0.01);
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

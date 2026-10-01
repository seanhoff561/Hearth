//! The mixer: sounds and beds into buses with their volumes, the world's sounds muffled under
//! water and echoing when shut in, faded while paused, and held under full scale.

use crate::beds::{Ambience, Beds};
use crate::dsp::{Biquad, FilterKind, Rng, Smooth, decay_per_sample, pan_gains};
use crate::sounds::{Sound, Voice};

/// Where a sound belongs, for its volume (the options' sound categories).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bus {
    Weather,
    Blocks,
    Players,
    Ambient,
    Hostile,
    Friendly,
    Ui,
    Music,
}

pub const BUSES: usize = 8;

impl Bus {
    pub const ALL: [Bus; BUSES] = [
        Bus::Weather,
        Bus::Blocks,
        Bus::Players,
        Bus::Ambient,
        Bus::Hostile,
        Bus::Friendly,
        Bus::Ui,
        Bus::Music,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    /// Sounds of the world: muffled, echoed and paused with it.
    fn in_world(self) -> bool {
        !matches!(self, Bus::Ui | Bus::Music)
    }
}

/// What the game tells the mixer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    /// A sound once; `pan` −1 (left) to 1 (right).
    Play {
        sound: Sound,
        bus: Bus,
        gain: f32,
        pan: f32,
    },
    Ambience(Ambience),
    /// Volumes 0–1: the master's and each bus's (by `Bus::index`).
    Volumes {
        master: f32,
        buses: [f32; BUSES],
    },
}

/// Sounds at once; a new one past this ends the oldest.
const MAX_VOICES: usize = 48;
/// Frames mixed at a time.
const CHUNK: usize = 256;

pub struct Mixer {
    rate: f32,
    inv_rate: f32,
    rng: Rng,
    voices: Vec<Voice>,
    beds: Beds,
    paused: bool,
    master: Smooth,
    buses: [Smooth; BUSES],
    world: Smooth,
    muffle: Smooth,
    muffle_f: [Biquad; 2],
    enclosed: Smooth,
    reverb: Reverb,
    limit: f32,
    release: f32,
    weather: Vec<f32>,
    ambient: Vec<f32>,
    body: Vec<f32>,
    world_mix: Vec<f32>,
    inner: Vec<f32>,
    front: Vec<f32>,
}

impl Mixer {
    pub fn new(rate: f32, seed: u32) -> Self {
        let buf = || vec![0.0; CHUNK * 2];
        Self {
            rate,
            inv_rate: 1.0 / rate,
            rng: Rng::new(seed ^ 0x9e37_79b9),
            voices: Vec::with_capacity(MAX_VOICES),
            beds: Beds::new(rate, seed),
            paused: false,
            master: Smooth::new(1.0, 0.03, rate),
            buses: [Smooth::new(1.0, 0.03, rate); BUSES],
            world: Smooth::new(1.0, 0.25, rate),
            muffle: Smooth::new(0.0, 0.08, rate),
            muffle_f: [Biquad::new(FilterKind::Low, 18_000.0, 0.7, rate); 2],
            enclosed: Smooth::new(0.0, 0.5, rate),
            reverb: Reverb::new(rate),
            limit: 0.0,
            release: decay_per_sample(0.2, rate),
            weather: buf(),
            ambient: buf(),
            body: buf(),
            world_mix: buf(),
            inner: buf(),
            front: buf(),
        }
    }

    pub fn rate(&self) -> f32 {
        self.rate
    }

    /// Sounds playing now.
    pub fn voices(&self) -> usize {
        self.voices.len()
    }

    pub fn apply(&mut self, c: Command) {
        match c {
            Command::Play {
                sound,
                bus,
                gain,
                pan,
            } => {
                if self.paused && bus.in_world() {
                    return;
                }
                if self.voices.len() >= MAX_VOICES {
                    self.voices.remove(0);
                }
                let rng = self.rng.fork();
                self.voices.push(Voice::new(
                    sound,
                    bus,
                    gain.max(0.0),
                    pan_gains(pan),
                    self.rate,
                    rng,
                ));
            }
            Command::Ambience(a) => {
                self.beds.set(&a);
                // Water over the ears muffles the world; so does weakness, partly.
                self.muffle.target = if a.underwater {
                    1.0
                } else {
                    0.65 * a.weak.clamp(0.0, 1.0)
                };
                self.enclosed.target = a.enclosed.clamp(0.0, 1.0);
                self.world.target = if a.paused { 0.0 } else { 1.0 };
                self.paused = a.paused;
            }
            Command::Volumes { master, buses } => {
                self.master.target = master.clamp(0.0, 1.0);
                for (s, v) in self.buses.iter_mut().zip(buses) {
                    s.target = v.clamp(0.0, 1.0);
                }
            }
        }
    }

    /// Fills `out` (stereo, interleaved) with the next samples.
    pub fn render(&mut self, out: &mut [f32]) {
        for chunk in out.chunks_mut(CHUNK * 2) {
            self.chunk(chunk);
        }
    }

    fn chunk(&mut self, out: &mut [f32]) {
        let len = out.len() & !1;
        let n = len / 2;
        for b in [
            &mut self.weather,
            &mut self.ambient,
            &mut self.body,
            &mut self.front,
        ] {
            b[..len].fill(0.0);
        }
        let master = self.master.advance(n);
        let mut vol = [0.0; BUSES];
        for (v, s) in vol.iter_mut().zip(&mut self.buses) {
            *v = s.advance(n);
        }
        let world_gain = self.world.advance(n);
        let muffle = self.muffle.advance(n);
        let enclosed = self.enclosed.advance(n);
        self.beds.render(
            &mut self.weather[..len],
            &mut self.ambient[..len],
            &mut self.body[..len],
        );
        let (vw, va, vp) = (
            vol[Bus::Weather.index()],
            vol[Bus::Ambient.index()],
            vol[Bus::Players.index()],
        );
        for i in 0..len {
            self.world_mix[i] = self.weather[i] * vw + self.ambient[i] * va;
            self.inner[i] = self.body[i] * vp;
        }
        let inv_rate = self.inv_rate;
        let (world_mix, inner, front) = (
            &mut self.world_mix[..len],
            &mut self.inner[..len],
            &mut self.front[..len],
        );
        self.voices.retain_mut(|v| {
            let target: &mut [f32] = if v.inner {
                inner
            } else if v.bus.in_world() {
                world_mix
            } else {
                front
            };
            v.render(target, vol[v.bus.index()], inv_rate)
        });
        // Under water: everything of the world dull and close.
        if muffle > 0.001 {
            let fc = 18_000.0 * (1.0 - muffle) + 450.0 * muffle;
            for f in &mut self.muffle_f {
                f.set(FilterKind::Low, fc, 0.7, self.rate);
            }
            for frame in world_mix.as_chunks_mut::<2>().0 {
                frame[0] = self.muffle_f[0].process(frame[0]);
                frame[1] = self.muffle_f[1].process(frame[1]);
            }
        }
        let wet = 0.6 * enclosed;
        let feedback = 0.72 + 0.14 * enclosed;
        for i in 0..n {
            let (l, r) = (world_mix[2 * i], world_mix[2 * i + 1]);
            let (el, er) = self.reverb.process(l, r, feedback);
            let mut ol = ((l + el * wet) + inner[2 * i]) * world_gain + front[2 * i];
            let mut or = ((r + er * wet) + inner[2 * i + 1]) * world_gain + front[2 * i + 1];
            ol *= master;
            or *= master;
            // A limiter: instant down, easing back up.
            let peak = ol.abs().max(or.abs());
            self.limit = if peak > self.limit {
                peak
            } else {
                self.limit * self.release + peak * (1.0 - self.release)
            };
            let g = if self.limit > 0.95 {
                0.95 / self.limit
            } else {
                1.0
            };
            out[2 * i] = (ol * g).clamp(-1.0, 1.0);
            out[2 * i + 1] = (or * g).clamp(-1.0, 1.0);
        }
    }
}

/// An echo: four damped feedback delays and two all-passes a side (after Schroeder and
/// Moorer).
struct Reverb {
    combs: [[Delay; 4]; 2],
    passes: [[Delay; 2]; 2],
    damp: [[f32; 4]; 2],
}

struct Delay {
    buf: Vec<f32>,
    i: usize,
}

impl Delay {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len.max(1)],
            i: 0,
        }
    }

    #[inline]
    fn read(&self) -> f32 {
        self.buf[self.i]
    }

    #[inline]
    fn write(&mut self, x: f32) {
        self.buf[self.i] = x;
        self.i += 1;
        if self.i == self.buf.len() {
            self.i = 0;
        }
    }
}

const DAMP: f32 = 0.3;

impl Reverb {
    fn new(rate: f32) -> Self {
        let scale = rate / 44_100.0;
        let len = |n: usize, side: usize| ((n + 23 * side) as f32 * scale) as usize;
        let combs = |side: usize| [1116, 1188, 1277, 1356].map(|n| Delay::new(len(n, side)));
        let passes = |side: usize| [556, 441].map(|n| Delay::new(len(n, side)));
        Self {
            combs: [combs(0), combs(1)],
            passes: [passes(0), passes(1)],
            damp: [[0.0; 4]; 2],
        }
    }

    #[inline]
    fn process(&mut self, l: f32, r: f32, feedback: f32) -> (f32, f32) {
        let input = (l + r) * 0.5;
        let mut out = [0.0; 2];
        for (side, o) in out.iter_mut().enumerate() {
            let mut sum = 0.0;
            for (c, d) in self.combs[side].iter_mut().zip(&mut self.damp[side]) {
                let y = c.read();
                *d = y * (1.0 - DAMP) + *d * DAMP;
                c.write(input + *d * feedback);
                sum += y;
            }
            let mut x = sum * 0.35;
            for a in &mut self.passes[side] {
                let b = a.read();
                a.write(x + b * 0.5);
                x = b - x;
            }
            *o = x;
        }
        (out[0], out[1])
    }
}

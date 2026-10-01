//! The sounds that go on: wind (gusting, whistling when strong), rain (hiss, patter and drops,
//! drumming on a roof when sheltered), the hush under water, and the body's own rhythms (the
//! heart and the breath), each easing toward what the world says.

use std::f32::consts::{PI, TAU};

use crate::dsp::{Biquad, Env, FilterKind, Grains, Rng, Smooth, decay_per_sample, noise_rms, ramp};
use crate::sounds::Layer;

/// What surrounds the listener; the mixer eases toward it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Ambience {
    /// Wind at the place (m/s), before any shelter.
    pub wind_m_s: f32,
    /// Rain falling (mm of water an hour; snow falls silently), before any shelter.
    pub rain_mm_h: f32,
    /// 0 under the open sky, 1 under a roof.
    pub sheltered: f32,
    /// 0 under a thin roof (rain drums on it), 1 deep under rock or earth (the weather is
    /// far away).
    pub buried: f32,
    /// 0 in the open, 1 shut in (a cave, a hut): the echo.
    pub enclosed: f32,
    /// The ears under water.
    pub underwater: bool,
    /// 0–1: so weak the world sounds far away (blood loss, fainting).
    pub weak: f32,
    /// The heart (beats a minute) and how loud it is heard (0 not at all, 1 pounding).
    pub heart_bpm: f32,
    pub heart: f32,
    /// Breathing (breaths a minute) and how loud (0 quiet, 1 gasping).
    pub breaths_per_min: f32,
    pub breath: f32,
    /// 0–1: shivering, trembling in the breath.
    pub shiver: f32,
    /// The world is paused: its sounds fade (the interface still sounds).
    pub paused: bool,
}

/// Samples between updates of the slow parameters.
const CONTROL: usize = 32;

pub(crate) struct Beds {
    rate: f32,
    inv_rate: f32,
    rng: Rng,
    wind: Smooth,
    rain: Smooth,
    shelter: Smooth,
    buried: Smooth,
    under: Smooth,
    heart: Smooth,
    breath: Smooth,
    shiver: Smooth,
    heart_bpm: f32,
    breath_rate: f32,
    control: usize,
    // Wind: low rushing noise and a whistle, both gusting.
    gust: Smooth,
    gust_left: usize,
    wind_f: [Biquad; 2],
    whistle_f: [Biquad; 2],
    wind_amp: [f32; 2],
    whistle_amp: [f32; 2],
    // Rain.
    hiss_hp: [Biquad; 2],
    hiss_lp: [Biquad; 2],
    patter: [Grains; 2],
    drops: [Grains; 2],
    hiss_amp: f32,
    patter_amp: f32,
    drop_amp: f32,
    // Under water.
    under_f: Biquad,
    under_amp: f32,
    // The heart.
    beat: f32,
    heart_parts: [Layer; 4],
    heart_f: Biquad,
    // The breath.
    cycle: f32,
    tremble: f32,
    breath_in: [Biquad; 2],
    breath_out: Biquad,
    breath_norm: [f32; 3],
}

impl Beds {
    pub(crate) fn new(rate: f32, seed: u32) -> Self {
        let lp = |f: f32| Biquad::new(FilterKind::Low, f, 0.7, rate);
        let hp = |f: f32| Biquad::new(FilterKind::High, f, 0.7, rate);
        let grains = |f: f32, q: f32, g: f32| Grains::new(FilterKind::Band, f, q, 0.0, g, rate);
        let breath_in = [
            Biquad::new(FilterKind::Band, 1600.0, 1.4, rate),
            Biquad::new(FilterKind::Band, 2800.0, 4.0, rate),
        ];
        Self {
            rate,
            inv_rate: 1.0 / rate,
            rng: Rng::new(seed),
            wind: Smooth::new(0.0, 1.0, rate),
            rain: Smooth::new(0.0, 2.0, rate),
            shelter: Smooth::new(0.0, 0.4, rate),
            buried: Smooth::new(0.0, 0.6, rate),
            under: Smooth::new(0.0, 0.08, rate),
            heart: Smooth::new(0.0, 0.8, rate),
            breath: Smooth::new(0.0, 0.8, rate),
            shiver: Smooth::new(0.0, 1.0, rate),
            heart_bpm: 65.0,
            breath_rate: 12.0,
            control: 0,
            gust: Smooth::new(0.0, 1.0, rate),
            gust_left: 0,
            wind_f: [lp(200.0), lp(200.0)],
            whistle_f: [
                Biquad::new(FilterKind::Band, 600.0, 14.0, rate),
                Biquad::new(FilterKind::Band, 620.0, 14.0, rate),
            ],
            wind_amp: [0.0; 2],
            whistle_amp: [0.0; 2],
            hiss_hp: [hp(900.0), hp(900.0)],
            hiss_lp: [lp(7000.0), lp(7000.0)],
            patter: [grains(3500.0, 0.9, 0.002), grains(3500.0, 0.9, 0.002)],
            drops: [grains(1400.0, 3.0, 0.005), grains(1400.0, 3.0, 0.005)],
            hiss_amp: 0.0,
            patter_amp: 0.0,
            drop_amp: 0.0,
            under_f: lp(250.0),
            under_amp: 0.0,
            beat: 0.0,
            heart_parts: [Layer::Off; 4],
            heart_f: lp(260.0),
            cycle: 0.0,
            tremble: 0.0,
            breath_in,
            breath_out: Biquad::new(FilterKind::Band, 900.0, 1.0, rate),
            breath_norm: [
                1.0 / noise_rms(FilterKind::Band, 1600.0, 1.4, rate),
                1.0 / noise_rms(FilterKind::Band, 2800.0, 4.0, rate),
                1.0 / noise_rms(FilterKind::Band, 900.0, 1.0, rate),
            ],
        }
    }

    pub(crate) fn set(&mut self, a: &Ambience) {
        self.wind.target = a.wind_m_s.clamp(0.0, 60.0);
        self.rain.target = a.rain_mm_h.clamp(0.0, 200.0);
        self.shelter.target = a.sheltered.clamp(0.0, 1.0);
        self.buried.target = a.buried.clamp(0.0, 1.0);
        self.under.target = if a.underwater { 1.0 } else { 0.0 };
        self.heart.target = a.heart.clamp(0.0, 1.0);
        self.breath.target = a.breath.clamp(0.0, 1.0);
        self.shiver.target = a.shiver.clamp(0.0, 1.0);
        self.heart_bpm = a.heart_bpm.clamp(20.0, 220.0);
        self.breath_rate = a.breaths_per_min.clamp(4.0, 60.0);
    }

    /// The slow parameters, every `CONTROL` samples.
    fn update(&mut self) {
        let rate = self.rate;
        let wind = self.wind.advance(CONTROL);
        let rain = self.rain.advance(CONTROL);
        let shelter = self.shelter.advance(CONTROL);
        let buried = self.buried.advance(CONTROL);
        let under = self.under.advance(CONTROL);
        self.heart.advance(CONTROL);
        self.breath.advance(CONTROL);
        self.shiver.advance(CONTROL);
        // Water over the ears or rock overhead keeps the weather out.
        let open = (1.0 - under) * (1.0 - 0.92 * buried);
        // Gusts: a new strength every one to four seconds, eased into.
        if self.gust_left <= CONTROL {
            let r = self.rng.white();
            self.gust.target = r * r.abs().sqrt();
            self.gust_left = (self.rng.range(0.8, 3.5) * rate) as usize;
        } else {
            self.gust_left -= CONTROL;
        }
        let g = self.gust.advance(CONTROL);
        let u = (wind * (1.0 + 0.5 * g * wind / (wind + 6.0))).max(0.0);
        let muffle = 1.0 - 0.6 * shelter;
        let fc = (120.0 + 45.0 * u) * muffle;
        let level = 0.045 * (u / 10.0).powf(1.5).min(5.0) * muffle * open;
        let whistle = 0.012 * ramp(u, 9.0, 25.0) * (1.0 - 0.8 * shelter) * open;
        let fw = (380.0 + 32.0 * u) * muffle;
        for c in 0..2 {
            // The two ears hear slightly different air.
            let detune = 1.0 + 0.03 * c as f32;
            self.wind_f[c].set(FilterKind::Low, fc * detune, 0.7, rate);
            self.wind_amp[c] = level / noise_rms(FilterKind::Low, fc * detune, 0.7, rate);
            self.whistle_f[c].set(FilterKind::Band, fw * detune, 14.0, rate);
            self.whistle_amp[c] = whistle / noise_rms(FilterKind::Band, fw * detune, 14.0, rate);
        }
        // Rain: louder as the square root of the rate; under a roof, lower and duller.
        let level = (rain / 6.0).sqrt().min(1.5) * open;
        let lp = 7000.0 * (1.0 - 0.8 * shelter);
        for c in 0..2 {
            self.hiss_lp[c].set(FilterKind::Low, lp, 0.7, rate);
            self.patter[c].set(
                FilterKind::Band,
                3500.0 * (1.0 - 0.72 * shelter),
                0.9,
                (60.0 + 260.0 * rain).min(4000.0),
                rate,
            );
            self.drops[c].set(
                FilterKind::Band,
                1400.0 * (1.0 - 0.6 * shelter),
                3.0 - shelter,
                (4.0 + 30.0 * rain).min(500.0),
                rate,
            );
        }
        let band = noise_rms(FilterKind::High, 900.0, 0.7, rate).min(noise_rms(
            FilterKind::Low,
            lp,
            0.7,
            rate,
        ));
        self.hiss_amp = 0.02 * level * (1.0 - 0.3 * shelter) / band;
        self.patter_amp = 0.035 * level;
        self.drop_amp = 0.04 * level;
        self.under_amp = 0.03 * under / noise_rms(FilterKind::Low, 250.0, 0.7, rate);
    }

    /// Adds the next samples (stereo, interleaved) into the weather's, the surroundings' and
    /// the body's buffers.
    pub(crate) fn render(&mut self, weather: &mut [f32], ambient: &mut [f32], body: &mut [f32]) {
        let frames = weather.len() / 2;
        for i in 0..frames {
            if self.control == 0 {
                self.update();
                self.control = CONTROL;
            }
            self.control -= 1;
            let rng = &mut self.rng;
            // Wind and rain.
            for c in 0..2 {
                let mut s = 0.0;
                if self.wind_amp[c] > 0.0 {
                    s += self.wind_f[c].process(rng.white()) * self.wind_amp[c];
                    s += self.whistle_f[c].process(rng.white()) * self.whistle_amp[c];
                }
                if self.hiss_amp > 0.0 {
                    let h = self.hiss_hp[c].process(rng.white());
                    s += self.hiss_lp[c].process(h) * self.hiss_amp;
                    s += self.patter[c].next(rng) * self.patter_amp;
                    s += self.drops[c].next(rng) * self.drop_amp;
                }
                weather[2 * i + c] += s;
            }
            if self.under_amp > 0.0 {
                let s = self.under_f.process(rng.white()) * self.under_amp;
                ambient[2 * i] += s;
                ambient[2 * i + 1] += s;
            }
            // The heart: "lub" as the valves close, "dub" a systole later.
            let heart = self.heart.value;
            self.beat += self.heart_bpm / 60.0 * self.inv_rate;
            if self.beat >= 1.0 {
                self.beat -= 1.0;
                if heart > 0.001 {
                    self.heart_parts = beat(self.rate, self.heart_bpm);
                }
            }
            let mut s = 0.0;
            for p in &mut self.heart_parts {
                s += p.next(rng, self.inv_rate);
            }
            let mut b = self.heart_f.process(s) * heart;
            // The breath: in through the nose and mouth, out, a pause.
            let breath = self.breath.value;
            self.cycle += self.breath_rate / 60.0 * self.inv_rate;
            if self.cycle >= 1.0 {
                self.cycle -= 1.0;
            }
            if breath > 0.001 {
                let p = self.cycle;
                let e_in = if p < 0.4 {
                    (PI * p / 0.4).sin().powi(2)
                } else {
                    0.0
                };
                let e_out = if (0.45..0.9).contains(&p) {
                    (PI * (p - 0.45) / 0.45).sin().powi(2)
                } else {
                    0.0
                };
                self.tremble += 7.0 * self.inv_rate;
                if self.tremble >= 1.0 {
                    self.tremble -= 1.0;
                }
                let tremble = 1.0 + 0.5 * self.shiver.value * (TAU * self.tremble).sin();
                let n = rng.white();
                let inhale = (self.breath_in[0].process(n) * self.breath_norm[0] * 0.8
                    + self.breath_in[1].process(n) * self.breath_norm[1] * 0.35)
                    * e_in;
                let exhale = self.breath_out.process(n) * self.breath_norm[2] * e_out;
                b += (inhale * 0.7 + exhale) * breath * 0.03 * tremble;
            }
            body[2 * i] += b;
            body[2 * i + 1] += b;
        }
    }
}

/// One beat's sounds.
fn beat(rate: f32, bpm: f32) -> [Layer; 4] {
    let tone = |f0: f32, f1: f32, delay: f32, tau: f32, amp: f32| Layer::Tone {
        phase: 0.0,
        f: f0,
        f_end: f1,
        glide: decay_per_sample(0.04, rate),
        amp,
        env: Env::new(delay, 0.004, tau, rate),
    };
    // The second sound follows by the systole: a third of a second at rest, shorter as the
    // heart quickens.
    let systole = (0.33 - 0.0012 * (bpm - 60.0)).clamp(0.18, 0.36);
    [
        tone(52.0, 38.0, 0.0, 0.05, 0.3),
        tone(104.0, 76.0, 0.0, 0.035, 0.1),
        tone(64.0, 48.0, systole, 0.04, 0.22),
        tone(128.0, 96.0, systole, 0.03, 0.07),
    ]
}

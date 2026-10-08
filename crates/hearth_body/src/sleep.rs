//! Sleep and fatigue (v2 §9.5): the pressure to sleep builds while awake (time constant about
//! 18 h, faster with hard work) and drains while asleep (about 4 h, slower when the sleep is
//! poor), after Borbély's two-process model; the body clock adds its own sleepiness in the small
//! hours and lifts it in the evening.

use serde::{Deserialize, Serialize};

const TAU_WAKE_S: f64 = 18.2 * 3600.0;
const TAU_SLEEP_S: f64 = 4.2 * 3600.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sleep {
    /// 0 rested – 1 unable to stay awake.
    pub pressure: f64,
    /// Real seconds asleep in the current sleep (0 when awake).
    pub asleep_s: f64,
}

impl Default for Sleep {
    fn default() -> Self {
        // Waking in the morning after a night's sleep.
        Self {
            pressure: 0.1,
            asleep_s: 0.0,
        }
    }
}

impl Sleep {
    /// Advances by `dt` real seconds awake or asleep; `quality` 0–1 of the sleep (warmth,
    /// comfort, quiet); `exertion` 0–1 of the work done awake; `rate` the balance multiplier.
    pub fn step(&mut self, asleep: bool, quality: f64, exertion: f64, rate: f64, dt: f64) {
        if asleep {
            let k = 1.0 - (-dt * quality.clamp(0.05, 1.0) / TAU_SLEEP_S).exp();
            self.pressure -= self.pressure * k;
            self.asleep_s += dt;
        } else {
            let k = 1.0 - (-dt * rate * (1.0 + 0.5 * exertion.clamp(0.0, 1.0)) / TAU_WAKE_S).exp();
            self.pressure += (1.0 - self.pressure) * k;
            self.asleep_s = 0.0;
        }
    }

    /// Sleepiness 0–1 at local solar time `hour`: the pressure plus the body clock's swing
    /// (sleepiest near 4 in the morning).
    pub fn sleepiness(&self, hour: f64) -> f64 {
        let clock = (std::f64::consts::TAU * (hour - 4.0) / 24.0).cos();
        (self.pressure + 0.12 * clock).clamp(0.0, 1.0)
    }

    /// Hours from `hour` (local solar time) until sleepiness reaches `sleepy`, staying awake
    /// at rest (P §7.1: "you'll probably fall asleep around dusk"); none within a day.
    pub fn hours_until(&self, hour: f64, sleepy: f64) -> Option<f64> {
        let mut s = *self;
        for q in 0..=96 {
            let h = q as f64 * 0.25;
            if s.sleepiness((hour + h).rem_euclid(24.0)) >= sleepy {
                return Some(h);
            }
            s.step(false, 1.0, 0.0, 1.0, 900.0);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_day_awake_and_a_night_asleep() {
        let mut s = Sleep::default();
        for _ in 0..16 * 60 {
            s.step(false, 1.0, 0.2, 1.0, 60.0);
        }
        assert!(s.pressure > 0.55 && s.pressure < 0.75, "{}", s.pressure);
        for _ in 0..8 * 60 {
            s.step(true, 1.0, 0.0, 1.0, 60.0);
        }
        assert!(s.pressure < 0.15, "{}", s.pressure);
    }

    #[test]
    fn the_evening_brings_sleep_a_noon_does_not() {
        // Up at six: at noon not sleepy for hours; by the late evening, sleepy.
        let mut s = Sleep::default();
        for _ in 0..6 * 60 {
            s.step(false, 1.0, 0.0, 1.0, 60.0);
        }
        let h = s.hours_until(12.0, 0.58).expect("sleepy within the day");
        assert!((7.0..12.0).contains(&h), "sleepy {h} h after noon");
        let mut tired = s;
        tired.pressure = 0.75;
        assert_eq!(tired.hours_until(12.0, 0.58), Some(0.0));
    }
}

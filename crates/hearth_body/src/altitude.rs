//! Altitude (E §5.2): the thin air of the heights, the body's acclimatisation to it over days,
//! acute mountain sickness when it climbs faster than that, and the death zone.

use serde::{Deserialize, Serialize};

pub use hearth_math::atmosphere::{SEA_LEVEL_KPA, boiling_c, pressure_kpa};

/// The highest the body acclimatises to (m): no one lives for long above some 5,000–5,500 m.
pub const MOST_ACCLIMATISED_M: f64 = 5400.0;
/// Up to this height (m) any body copes without acclimatising; acclimatisation climbs from it.
const AT_HOME_M: f64 = 2000.0;
/// How fast acclimatisation follows a climb (m a day): some 300–500 m.
const ACCLIMATISE_M_PER_DAY: f64 = 400.0;
/// How long it takes to lose it below (s): its time constant, some ten days.
const DEACCLIMATISE_S: f64 = 10.0 * 86_400.0;
/// Of the height it has acclimatised to, the share it no longer feels.
const ACCLIMATISED_SHARE: f64 = 0.4;
/// Mountain sickness comes on over some hours (its time constant), and goes over half a day.
const SICKEN_S: f64 = 8.0 * 3600.0;
const RECOVER_S: f64 = 12.0 * 3600.0;
/// Above what the body is used to by this much, and above 2,500 m, mountain sickness may come;
/// 2 km more is its worst.
const SICKNESS_AFTER_M: f64 = 500.0;
const SICKNESS_SPAN_M: f64 = 2000.0;
/// Severe sickness (cerebral or pulmonary oedema) kills in about a day untreated; the death
/// zone, where even the acclimatised body wastes, in some three days.
pub const SEVERE_FATAL_S: f64 = 24.0 * 3600.0;
pub const DEATH_ZONE_FATAL_S: f64 = 3.0 * 86_400.0;
/// Felt height (m) beyond which the body cannot hold its own: the death zone, some 8,000 m for
/// an acclimatised climber.
const DEATH_ZONE_FELT_M: f64 = 5800.0;

/// The body's state toward height.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Altitude {
    /// The height (m) it is at, as of its last step.
    pub height_m: f64,
    /// The height (m) it is acclimatised to.
    pub acclimatised_m: f64,
    /// Acute mountain sickness, 0–1 (above a half, nausea; near 1, severe).
    pub sickness: f64,
    /// Seconds spent severely sick, and in the death zone.
    pub severe_s: f64,
    pub zone_s: f64,
}

impl Altitude {
    /// The height the body feels at `height_m`: the acclimatised share of its acclimatisation
    /// taken off.
    pub fn felt_m(&self, height_m: f64) -> f64 {
        let gained = (self.acclimatised_m.min(height_m.max(0.0)) - 1500.0).max(0.0);
        height_m - ACCLIMATISED_SHARE * gained
    }

    /// Aerobic capacity at `height_m`, 0–1 of the sea level's: unchanged to some 1,500 m, about
    /// three quarters at 3,000 m and two fifths at 5,000 m unacclimatised, a fifth on the
    /// highest summit after weeks of acclimatisation (with the oxygen the air holds; VO2max at
    /// altitude, Fulco et al. 1998).
    pub fn capacity(&self, height_m: f64) -> f64 {
        let o2 = pressure_kpa(self.felt_m(height_m)) / SEA_LEVEL_KPA;
        ((o2 - 0.30) / (0.84 - 0.30)).clamp(0.02, 1.0)
    }

    /// Advances `dt` seconds at `height_m`.
    pub fn step(&mut self, height_m: f64, dt: f64) {
        self.height_m = height_m;
        let target = height_m.clamp(0.0, MOST_ACCLIMATISED_M);
        if target > self.acclimatised_m {
            self.acclimatised_m = self.acclimatised_m.max(AT_HOME_M.min(target));
            self.acclimatised_m =
                (self.acclimatised_m + ACCLIMATISE_M_PER_DAY * dt / 86_400.0).min(target);
        } else {
            self.acclimatised_m +=
                (target - self.acclimatised_m) * (1.0 - (-dt / DEACCLIMATISE_S).exp());
        }
        let excess = if height_m > 2500.0 {
            height_m - self.acclimatised_m - SICKNESS_AFTER_M
        } else {
            0.0
        };
        let toward = (excess / SICKNESS_SPAN_M).clamp(0.0, 1.0);
        let tau = if toward > self.sickness {
            SICKEN_S
        } else {
            RECOVER_S
        };
        self.sickness += (toward - self.sickness) * (1.0 - (-dt / tau).exp());
        if self.sickness > 0.9 {
            self.severe_s += dt;
        } else {
            self.severe_s = (self.severe_s - dt).max(0.0);
        }
        if self.felt_m(height_m) > DEATH_ZONE_FELT_M {
            self.zone_s += dt;
        } else {
            self.zone_s = (self.zone_s - 2.0 * dt).max(0.0);
        }
    }

    /// Whether the heights have killed: severe sickness a day long, or days in the death zone.
    pub fn fatal(&self) -> bool {
        self.severe_s >= SEVERE_FATAL_S || self.zone_s >= DEATH_ZONE_FATAL_S
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_acclimatises_over_days_and_sickens_climbing_too_fast() {
        let mut a = Altitude::default();
        assert_eq!(a.capacity(1000.0), 1.0);
        let fresh = a.capacity(3000.0);
        assert!((0.65..0.85).contains(&fresh), "{fresh}");
        // Straight up to 4,000 m: sick within the day.
        for _ in 0..(12 * 60) {
            a.step(4000.0, 60.0);
        }
        assert!(a.sickness > 0.25, "sick climbing fast: {}", a.sickness);
        // A week there: acclimatised, well again, breathing easier.
        for _ in 0..(7 * 24 * 60) {
            a.step(4000.0, 60.0);
        }
        assert!(a.sickness < 0.05 && a.acclimatised_m > 3900.0, "{a:?}");
        assert!(a.capacity(3000.0) > fresh + 0.1);
        assert!(!a.fatal());
        // A climber acclimatised to 5.4 km has a fifth of their capacity on the summit, and
        // dies there in days.
        let mut c = Altitude {
            acclimatised_m: MOST_ACCLIMATISED_M,
            ..Default::default()
        };
        assert!(
            (0.12..0.35).contains(&c.capacity(8848.0)),
            "{}",
            c.capacity(8848.0)
        );
        for _ in 0..(4 * 24 * 60) {
            c.step(8848.0, 60.0);
        }
        assert!(c.fatal());
    }
}

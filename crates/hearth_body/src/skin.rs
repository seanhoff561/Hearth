//! The skin's state (Amendment E §8, E7): what the sun has done to it, what is on it and what
//! marks it, over real days.
//!
//! - **Sun.** The erythemal dose taken from the UV index (1 UVI = 25 mW/m² erythemally
//!   weighted; a standard erythema dose, SED, is 100 J/m²). A dose beyond the skin's minimal
//!   erythema dose (MED: some 2 SED for the fairest skin to 15 and more for the darkest) burns:
//!   the redness rises over the next hours, peaks within a day and fades over days. Every dose
//!   tans a little, the melanin coming over some days and fading over weeks.
//! - **Dirt** on the legs from walking on soil (mud most) and on the hands from handling it;
//!   **blood** where wounds bleed. Water washes both off (fast when in it, slower in rain), and
//!   both wear off over a day or two.
//! - **Scars** where injuries of some severity healed; they stay.

use hearth_content::schema::body::BodyRegion;
use serde::{Deserialize, Serialize};

use crate::harm::{InjuryState, Side};

/// A mark on the skin of one region and side, 0–1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Mark {
    pub region: BodyRegion,
    pub side: Side,
    pub amount: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Skin {
    /// The natural tone (0 lightest – 1 darkest): how much sun it takes to burn.
    pub tone: f32,
    /// The erythemal dose taken recently (SED), fading with the day.
    pub dose_sed: f64,
    /// Redness from sunburn, 0–1, and the redness still to come.
    pub burn: f32,
    burn_coming: f32,
    /// Tan: melanin made by the sun, 0–1, and the melanin still to come.
    pub tan: f32,
    tan_coming: f32,
    /// Dirt on the legs (feet to knees) and the hands, 0–1.
    pub dirt_legs: f32,
    pub dirt_hands: f32,
    /// Blood on the skin.
    pub blood: Vec<Mark>,
    /// Healed injuries' scars.
    pub scars: Vec<Mark>,
}

impl Default for Skin {
    fn default() -> Self {
        Self {
            tone: 0.5,
            dose_sed: 0.0,
            burn: 0.0,
            burn_coming: 0.0,
            tan: 0.0,
            tan_coming: 0.0,
            dirt_legs: 0.0,
            dirt_hands: 0.0,
            blood: Vec::new(),
            scars: Vec::new(),
        }
    }
}

const HOUR: f64 = 3600.0;
const DAY: f64 = 86_400.0;

/// Exponential approach of `x` toward `to` over time constant `tau` (s) in `dt`.
fn toward(x: f32, to: f32, dt: f64, tau: f64) -> f32 {
    let k = 1.0 - (-dt / tau).exp();
    x + (to - x) * k as f32
}

/// What the skin is exposed to in a step.
#[derive(Debug, Clone, Copy, Default)]
pub struct SkinExposure {
    /// The UV index on the bare skin.
    pub uv_index: f32,
    /// Share of the skin covered by clothing (0–1): covered skin neither burns nor tans.
    pub covered: f32,
    /// How dirty the ground underfoot is (0 rock, snow or water – 1 mud) and the body's speed
    /// over it (m/s).
    pub ground_dirt: f32,
    pub speed_m_s: f32,
    /// How dirty what the hands handle is (0–1 a second, e.g. digging).
    pub hands_dirt: f32,
    /// Whether the body crawls (hands on the ground).
    pub crawling: bool,
    /// Share of the body in water, and the rain on it (mm/h).
    pub immersion: f32,
    pub rain_mm_h: f32,
}

impl Skin {
    /// The minimal erythema dose of this skin (SED).
    pub fn med_sed(&self) -> f64 {
        let t = self.tone.clamp(0.0, 1.0) as f64;
        2.0 + 18.0 * t * t
    }

    /// Advances the skin `dt` seconds.
    pub fn step(&mut self, dt: f64, e: &SkinExposure, injuries: &[InjuryState]) {
        if dt <= 0.0 {
            return;
        }
        // Sun: the day's dose, fading with a half-day's time constant.
        let bare = (1.0 - e.covered.clamp(0.0, 1.0)) as f64;
        let dose = e.uv_index.max(0.0) as f64 * 0.025 * dt / 100.0 * bare;
        let med = self.med_sed();
        let before = self.dose_sed;
        self.dose_sed = self.dose_sed * (-dt / (12.0 * HOUR)).exp() + dose;
        // Past one MED, each further MED burns more; the redness comes over hours.
        let over = (self.dose_sed - med).max(0.0) - (before - med).max(0.0);
        if over > 0.0 {
            self.burn_coming = (self.burn_coming + (over / med * 0.45) as f32).min(1.5);
        }
        let arriving = self.burn_coming * (1.0 - (-dt / (8.0 * HOUR)).exp()) as f32;
        self.burn_coming -= arriving;
        self.burn = (self.burn + arriving).min(1.0);
        self.burn = toward(self.burn, 0.0, dt, 2.5 * DAY);
        // Every dose tans a little: a few MEDs a day for a week or two gives a good tan.
        self.tan_coming += (dose / med * 0.03) as f32;
        let arriving = self.tan_coming * (1.0 - (-dt / (3.0 * DAY)).exp()) as f32;
        self.tan_coming -= arriving;
        self.tan = (self.tan + arriving).min(1.0);
        self.tan = toward(self.tan, 0.0, dt, 40.0 * DAY);
        // Dirt: picked up by walking on soil and handling it, washed off by water.
        let walked = e.ground_dirt.clamp(0.0, 1.0) * e.speed_m_s.max(0.0) * dt as f32;
        self.dirt_legs = (self.dirt_legs + walked * 0.004 * (1.0 - self.dirt_legs)).min(1.0);
        let handled = (e.hands_dirt.clamp(0.0, 1.0)
            + if e.crawling { e.ground_dirt * 0.5 } else { 0.0 })
            * dt as f32;
        self.dirt_hands = (self.dirt_hands + handled * 0.05 * (1.0 - self.dirt_hands)).min(1.0);
        let wash = if e.immersion > 0.3 {
            20.0
        } else if e.rain_mm_h > 0.5 {
            600.0 / (e.rain_mm_h as f64 / 2.0).max(1.0)
        } else {
            1.5 * DAY
        };
        self.dirt_legs = toward(self.dirt_legs, 0.0, dt, wash);
        self.dirt_hands = toward(self.dirt_hands, 0.0, dt, wash);
        // Blood where wounds bleed; it dries and stays till washed or worn off.
        for inj in injuries.iter().filter(|i| i.bleeding_ml_min > 0.0) {
            let want = (inj.bleeding_ml_min as f32 / 20.0).min(1.0);
            match self
                .blood
                .iter_mut()
                .find(|m| m.region == inj.region && m.side == inj.side)
            {
                Some(m) => m.amount = toward(m.amount, m.amount.max(want), dt, 30.0),
                None => self.blood.push(Mark {
                    region: inj.region,
                    side: inj.side,
                    amount: toward(0.0, want, dt, 30.0),
                }),
            }
        }
        for m in &mut self.blood {
            m.amount = toward(m.amount, 0.0, dt, wash.clamp(20.0, 2.0 * DAY));
        }
        self.blood.retain(|m| m.amount > 0.01);
        // Scars where injuries of some severity have healed.
        for inj in injuries
            .iter()
            .filter(|i| i.healed >= 0.95 && i.severity >= 0.3)
        {
            let amount = (inj.severity * 0.8).min(1.0);
            match self
                .scars
                .iter_mut()
                .find(|m| m.region == inj.region && m.side == inj.side)
            {
                Some(m) => m.amount = m.amount.max(amount),
                None => self.scars.push(Mark {
                    region: inj.region,
                    side: inj.side,
                    amount,
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sun(uv: f32) -> SkinExposure {
        SkinExposure {
            uv_index: uv,
            ..SkinExposure::default()
        }
    }

    /// Steps through `hours` of the given exposure in ten-minute steps.
    fn spend(s: &mut Skin, hours: f64, e: SkinExposure) {
        for _ in 0..(hours * 6.0) as usize {
            s.step(600.0, &e, &[]);
        }
    }

    #[test]
    fn fair_skin_burns_at_noon_and_the_redness_comes_later_and_fades() {
        let mut s = Skin {
            tone: 0.1,
            ..Skin::default()
        };
        // Two hours at UV index 8: some 14 SED against an MED of about 2.
        spend(&mut s, 2.0, sun(8.0));
        let soon = s.burn;
        spend(&mut s, 14.0, sun(0.0));
        let next_day = s.burn;
        assert!(
            next_day > soon,
            "redness grows after: {soon} then {next_day}"
        );
        assert!(next_day > 0.5, "a bad burn: {next_day}");
        spend(&mut s, 24.0 * 7.0, sun(0.0));
        assert!(s.burn < 0.1 * next_day, "fades in a week: {}", s.burn);
    }

    #[test]
    fn dark_skin_takes_the_same_sun_without_burning_and_tans_slowly() {
        let mut s = Skin {
            tone: 0.9,
            ..Skin::default()
        };
        spend(&mut s, 2.0, sun(8.0));
        spend(&mut s, 14.0, sun(0.0));
        assert!(s.burn < 0.05, "{}", s.burn);
        // A fortnight of two hours a day in the sun tans fair skin well.
        let mut fair = Skin {
            tone: 0.2,
            ..Skin::default()
        };
        for _ in 0..14 {
            spend(&mut fair, 2.0, sun(5.0));
            spend(&mut fair, 22.0, sun(0.0));
        }
        assert!(fair.tan > 0.25, "tan {}", fair.tan);
        // Clothing shades it.
        let mut covered = Skin {
            tone: 0.2,
            ..Skin::default()
        };
        spend(
            &mut covered,
            2.0,
            SkinExposure {
                covered: 1.0,
                ..sun(8.0)
            },
        );
        assert_eq!(covered.dose_sed, 0.0);
    }

    #[test]
    fn mud_on_the_legs_washes_off_in_water() {
        let mut s = Skin::default();
        spend(
            &mut s,
            0.5,
            SkinExposure {
                ground_dirt: 1.0,
                speed_m_s: 1.3,
                ..SkinExposure::default()
            },
        );
        assert!(s.dirt_legs > 0.8, "{}", s.dirt_legs);
        assert!(s.dirt_hands < 0.01);
        s.step(
            120.0,
            &SkinExposure {
                immersion: 0.6,
                ..SkinExposure::default()
            },
            &[],
        );
        assert!(s.dirt_legs < 0.05, "{}", s.dirt_legs);
    }
}

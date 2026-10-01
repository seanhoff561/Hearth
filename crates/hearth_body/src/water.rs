//! Body water (v2 §9.3). The deficit is the water lost and not replaced (litres; negative when
//! the body holds more than it needs). Water leaves as sweat, through the skin and in the breath
//! (from the heat balance), as urine — about 1.5 l a day when well watered, down to half a litre
//! as the kidneys hold back when it runs short, more when there is extra to shed — and in the
//! faeces; it comes from what is drunk, from food and from burning food (metabolic water).
//! Drinking salt water costs the water the kidneys need to excrete its salt.

use serde::{Deserialize, Serialize};

/// Urine when well watered and at most holding back (l per real day).
const URINE_L_DAY: f64 = 1.5;
const URINE_MIN_L_DAY: f64 = 0.5;
/// Deficit (share of body mass) at which the kidneys hold back fully.
const HOLD_BACK: f64 = 0.03;
/// Water in the faeces (l per real day).
const FAECES_L_DAY: f64 = 0.1;
/// Water made by burning food (l per kcal).
pub const METABOLIC_WATER_L_PER_KCAL: f64 = 1.3e-4;
/// Salt the kidneys can excrete per litre of urine (g): drinking water with this much salt per
/// litre gains nothing; seawater (35 g/l) loses half a litre per litre drunk.
pub const KIDNEY_SALT_G_L: f64 = 23.3;

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Water {
    /// Water lost and not replaced (l).
    pub deficit_l: f64,
}

impl Water {
    /// Urine output (l per real day) for the current deficit.
    pub fn urine_l_day(&self, mass_kg: f64) -> f64 {
        if self.deficit_l < 0.0 {
            // Extra water leaves within about an hour.
            URINE_L_DAY + (-self.deficit_l) * 24.0
        } else {
            let short = self.deficit_l / mass_kg / HOLD_BACK;
            (URINE_L_DAY - (URINE_L_DAY - URINE_MIN_L_DAY) * short).max(URINE_MIN_L_DAY)
        }
    }

    /// Advances by `dt` real seconds: `losses_kg_s` of sweat, evaporation and illness, `gained_l`
    /// absorbed from drink and food and made by metabolism; `rate` is the balance multiplier on
    /// losses.
    pub fn step(&mut self, mass_kg: f64, losses_kg_s: f64, gained_l: f64, rate: f64, dt: f64) {
        let kidneys = (self.urine_l_day(mass_kg) + FAECES_L_DAY) / 86_400.0 * dt;
        let lost = losses_kg_s * dt * rate + kidneys;
        self.deficit_l += lost - gained_l;
    }

    /// The water drinking `litres` with `salt_g_l` of salt costs to excrete the salt (l).
    pub fn salt_cost_l(litres: f64, salt_g_l: f64) -> f64 {
        litres * salt_g_l.max(0.0) / KIDNEY_SALT_G_L
    }

    /// Deficit as a share of body mass.
    pub fn deficit_share(&self, mass_kg: f64) -> f64 {
        self.deficit_l / mass_kg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kidneys_hold_back_when_short_and_shed_extra() {
        let mut w = Water::default();
        assert!((w.urine_l_day(70.0) - 1.5).abs() < 1e-9);
        w.deficit_l = 0.03 * 70.0;
        assert!((w.urine_l_day(70.0) - 0.5).abs() < 1e-9);
        w.deficit_l = -1.0;
        assert!(w.urine_l_day(70.0) > 20.0);
        assert!(
            (Water::salt_cost_l(1.0, 35.0) - 1.5).abs() < 0.01,
            "seawater: net −0.5 l"
        );
    }
}

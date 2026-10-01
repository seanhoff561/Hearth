//! Food energy (v2 §9.3). What is eaten waits in the stomach (its capacity limits a meal) and is
//! digested over hours; the energy tops up glycogen (about a day's worth in liver and muscle)
//! and the rest is stored as fat. The body burns glycogen and fat — fat alone once the
//! glycogen is gone, for weeks, until it runs out. Two slow measures watch the diet: protein's
//! share of the energy eaten over the last days (living on lean meat alone poisons: "rabbit
//! starvation") and a reserve of the vitamins of fresh food (scurvy after weeks without).

use serde::{Deserialize, Serialize};

/// Food energy stored in a kilogram of body fat (kcal).
pub const KCAL_PER_KG_FAT: f64 = 7700.0;
/// Glycogen per kilogram of body mass at full stores (kcal): ≈ 500 g for 70 kg.
const GLYCOGEN_KCAL_PER_KG: f64 = 28.0;
/// Body fat of the reference adult at the start (share of mass).
const START_FAT: f64 = 0.2;
/// Time constant of gastric emptying for food (s) and for drink (s).
const EMPTYING_S: f64 = 1.5 * 3600.0;
const DRINK_EMPTYING_S: f64 = 20.0 * 60.0;
/// Window of the diet measures (s).
const DIET_WINDOW_S: f64 = 3.0 * 86_400.0;
/// Fresh-food vitamins a well-fed body starts with and can store (days of need).
const FRESH_START_DAYS: f64 = 60.0;
const FRESH_MAX_DAYS: f64 = 120.0;
/// Share of surplus energy kept when stored as fat.
const FAT_STORAGE_EFFICIENCY: f64 = 0.85;

/// Something eaten or drunk.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Food {
    pub kcal: f64,
    pub protein_g: f64,
    pub fat_g: f64,
    pub carb_g: f64,
    /// Water it brings (l).
    pub water_l: f64,
    /// Bulk in the stomach (l).
    pub volume_l: f64,
    /// Days of fresh-food vitamins it supplies (fresh plants, berries, organ meat).
    pub fresh_days: f64,
}

impl Food {
    /// A drink of water.
    pub fn water(litres: f64) -> Food {
        Food {
            water_l: litres,
            volume_l: litres,
            ..Food::default()
        }
    }

    fn scaled(&self, k: f64) -> Food {
        Food {
            kcal: self.kcal * k,
            protein_g: self.protein_g * k,
            fat_g: self.fat_g * k,
            carb_g: self.carb_g * k,
            water_l: self.water_l * k,
            volume_l: self.volume_l * k,
            fresh_days: self.fresh_days * k,
        }
    }

    fn add(&mut self, o: &Food) {
        self.kcal += o.kcal;
        self.protein_g += o.protein_g;
        self.fat_g += o.fat_g;
        self.carb_g += o.carb_g;
        self.water_l += o.water_l;
        self.volume_l += o.volume_l;
        self.fresh_days += o.fresh_days;
    }

    fn sub(&mut self, o: &Food) {
        self.add(&o.scaled(-1.0));
    }
}

/// The body's energy stores and diet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Energy {
    /// What is still being digested (its water separately: drinks pass quickly).
    pub stomach: Food,
    pub glycogen_kcal: f64,
    pub fat_kcal: f64,
    /// Energy absorbed over the last days, and how much of it was protein (kcal).
    pub recent_kcal: f64,
    pub recent_protein_kcal: f64,
    /// Fresh-food vitamins in store (days of need).
    pub fresh_days: f64,
    /// Real seconds since the glycogen ran out (starvation).
    pub fasting_s: f64,
}

impl Energy {
    pub fn new(mass_kg: f64) -> Self {
        Self {
            stomach: Food::default(),
            glycogen_kcal: glycogen_max(mass_kg),
            fat_kcal: START_FAT * mass_kg * KCAL_PER_KG_FAT,
            // A mixed diet: about 15 % of energy from protein.
            recent_kcal: 6000.0,
            recent_protein_kcal: 900.0,
            fresh_days: FRESH_START_DAYS,
            fasting_s: 0.0,
        }
    }

    /// Room left in the stomach (l).
    pub fn room_l(&self, capacity_l: f64) -> f64 {
        (capacity_l - self.stomach.volume_l).max(0.0)
    }

    pub fn swallow(&mut self, f: &Food) {
        self.stomach.add(f);
    }

    /// Digests for `dt` real seconds: food energy into glycogen and fat, and the drink's water.
    /// Returns the water absorbed (l).
    pub fn digest(&mut self, mass_kg: f64, dt: f64) -> f64 {
        let k = 1.0 - (-dt / EMPTYING_S).exp();
        let kw = 1.0 - (-dt / DRINK_EMPTYING_S).exp();
        let mut out = self.stomach.scaled(k);
        // Drink leaves the stomach faster than food.
        let water = self.stomach.water_l * kw;
        out.water_l = water;
        out.volume_l = (self.stomach.volume_l - self.stomach.water_l).max(0.0) * k + water;
        self.stomach.sub(&out);
        self.stomach.volume_l = self.stomach.volume_l.max(0.0);
        self.stomach.water_l = self.stomach.water_l.max(0.0);
        // Energy: glycogen first, the rest to fat.
        let room = (glycogen_max(mass_kg) - self.glycogen_kcal).max(0.0);
        let to_glycogen = out.kcal.min(room);
        self.glycogen_kcal += to_glycogen;
        self.fat_kcal += (out.kcal - to_glycogen) * FAT_STORAGE_EFFICIENCY;
        // The diet over the last days.
        let decay = (-dt / DIET_WINDOW_S).exp();
        self.recent_kcal = self.recent_kcal * decay + out.kcal;
        self.recent_protein_kcal = self.recent_protein_kcal * decay + out.protein_g * 4.0;
        self.fresh_days = (self.fresh_days + out.fresh_days).min(FRESH_MAX_DAYS);
        water
    }

    /// Burns `kcal` over `dt` real seconds: glycogen in proportion to how full it is, the rest
    /// from fat.
    pub fn burn(&mut self, kcal: f64, mass_kg: f64, dt: f64) {
        let full = (self.glycogen_kcal / glycogen_max(mass_kg)).clamp(0.0, 1.0);
        let from_glycogen = (kcal * (0.2 + 0.6 * full)).min(self.glycogen_kcal);
        self.glycogen_kcal -= from_glycogen;
        self.fat_kcal -= kcal - from_glycogen;
        if self.glycogen_kcal < 0.02 * glycogen_max(mass_kg) {
            self.fasting_s += dt;
        } else {
            self.fasting_s = 0.0;
        }
        self.fresh_days = (self.fresh_days - dt / 86_400.0).max(0.0);
    }

    /// 0–1: how full the glycogen stores are.
    pub fn glycogen_frac(&self, mass_kg: f64) -> f64 {
        (self.glycogen_kcal / glycogen_max(mass_kg)).clamp(0.0, 1.0)
    }

    /// Protein's share of the energy absorbed over the last days.
    pub fn protein_share(&self) -> f64 {
        if self.recent_kcal < 300.0 {
            0.0
        } else {
            self.recent_protein_kcal / self.recent_kcal
        }
    }

    /// Body fat as a share of the body's mass.
    pub fn fat_share(&self, mass_kg: f64) -> f64 {
        self.fat_kcal.max(0.0) / KCAL_PER_KG_FAT / mass_kg
    }
}

/// Glycogen at full stores (kcal).
pub fn glycogen_max(mass_kg: f64) -> f64 {
    GLYCOGEN_KCAL_PER_KG * mass_kg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_meal_is_digested_over_hours_into_the_stores() {
        let mut e = Energy::new(70.0);
        e.glycogen_kcal = 500.0;
        let fat0 = e.fat_kcal;
        e.swallow(&Food {
            kcal: 2400.0,
            carb_g: 400.0,
            protein_g: 60.0,
            fat_g: 60.0,
            volume_l: 1.0,
            ..Food::default()
        });
        // After an hour, part of it; after eight hours, nearly all.
        for _ in 0..60 {
            e.digest(70.0, 60.0);
        }
        assert!(
            e.stomach.kcal > 900.0 && e.stomach.kcal < 1500.0,
            "{}",
            e.stomach.kcal
        );
        for _ in 0..420 {
            e.digest(70.0, 60.0);
        }
        assert!(e.stomach.kcal < 20.0);
        assert!(
            (e.glycogen_kcal - glycogen_max(70.0)).abs() < 1.0,
            "glycogen refilled first"
        );
        assert!(e.fat_kcal > fat0 + 600.0, "the surplus stored as fat");
    }

    #[test]
    fn lean_meat_alone_raises_protein_share() {
        let mut e = Energy::new(70.0);
        let rabbit = Food {
            kcal: 1300.0,
            protein_g: 280.0,
            fat_g: 20.0,
            volume_l: 1.5,
            ..Food::default()
        };
        for _ in 0..5 {
            e.swallow(&rabbit);
            for _ in 0..24 {
                e.digest(70.0, 3600.0);
            }
        }
        assert!(e.protein_share() > 0.6, "{}", e.protein_share());
    }
}

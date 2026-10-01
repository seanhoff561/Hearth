//! Fires (v2 §11.4): a simple thermal model of a fire's fuel, flames and coals, shared by
//! hearths and natural fires.
//!
//! * **Fuel** burns from its surface inwards at about 0.8 mm a minute, so a piece's life goes
//!   with its thickness, and a fire has only part of each piece in its flames: dry grass
//!   flares for a minute, a 2.5 cm stick burns for most of an hour, a 40 cm log section half a
//!   day. Wet fuel must dry first and burns slowly; the
//!   fire's own heat dries it.
//! * A seventh of the wood is left as **coals**, which glow on for an hour or two after the
//!   flames die and light fresh fuel laid on them; **banked** under ash they smoulder for half a
//!   day, without flame.
//! * The **heat** given off is the energy of what burns (about 18 MJ per kilogram of dry wood,
//!   29.5 for charcoal); the hearth's temperature rises with it towards what its build allows.
//!   A small campfire gives 5–10 kW and holds 600–800 °C; coals alone, a few hundred degrees.
//! * **Rain** soaks the fuel and quenches coals; a small fire in hard rain goes out.
//! * About a quarter of the heat leaves as radiation, which warms whoever sits by it.

use serde::{Deserialize, Serialize};

/// How fast flame eats into wood (m per hour of burning): 0.8 mm a minute.
pub const REGRESSION_M_H: f32 = 0.048;
/// Share of a piece in the flames at once (the rest lies outside the hearth or shielded).
pub const IN_FLAME: f32 = 0.4;
/// Share of wood's mass left as char when it burns.
pub const CHAR_SHARE: f32 = 0.15;
/// Heat of charcoal (MJ/kg).
pub const COAL_MJ_KG: f32 = 29.5;
/// Share of a fire's heat given off as radiation.
pub const RADIANT_SHARE: f32 = 0.25;

/// Fuel in a fire: one piece, or several alike.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Fuel {
    /// Mass left (kg).
    pub kg: f32,
    /// Heat of the dry fuel (MJ/kg).
    pub mj_kg: f32,
    /// Thickness (m).
    pub thick_m: f32,
    /// 0–1 how wet.
    pub wet: f32,
}

impl Fuel {
    /// Hours a piece of this thickness takes to burn through in a fire.
    pub fn burn_h(&self) -> f32 {
        (self.thick_m.max(0.001) * 0.5) / (REGRESSION_M_H * IN_FLAME)
    }
}

/// How a fire is, in a word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FireState {
    Out,
    /// Banked under ash.
    Banked,
    /// Coals glowing, no flame.
    Embers,
    /// Burning low.
    Low,
    /// Burning well.
    High,
}

impl FireState {
    /// Its name as a block state.
    pub fn name(self) -> &'static str {
        match self {
            FireState::Out => "out",
            FireState::Banked => "banked",
            FireState::Embers => "embers",
            FireState::Low => "low",
            FireState::High => "high",
        }
    }
}

/// A fire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fire {
    #[serde(default)]
    pub fuel: Vec<Fuel>,
    /// Glowing coals (kg).
    #[serde(default)]
    pub coals_kg: f32,
    /// The fuel is in flame.
    #[serde(default)]
    pub flaming: bool,
    /// Banked under ash.
    #[serde(default)]
    pub banked: bool,
    /// 0–1 how much of the fuel the flames have taken hold of (a fire grows over minutes).
    #[serde(default)]
    pub involved: f32,
    /// Heat given off now (kW).
    #[serde(default)]
    pub power_kw: f32,
    /// The hearth's temperature (°C).
    #[serde(default)]
    pub temp_c: f32,
    /// The hottest its build allows (°C).
    pub max_c: f32,
    /// A lamp: fat burns on a wick at a steady few grams an hour.
    #[serde(default)]
    pub wick: bool,
}

impl Fire {
    /// A laid, unlit fire of this build.
    pub fn laid(max_c: f32, fuel: Vec<Fuel>) -> Self {
        Self {
            fuel,
            coals_kg: 0.0,
            flaming: false,
            banked: false,
            involved: 0.0,
            power_kw: 0.0,
            temp_c: 15.0,
            max_c,
            wick: false,
        }
    }

    /// A lamp of fat with a wick.
    pub fn lamp(max_c: f32, fat_kg: f32) -> Self {
        Self {
            wick: true,
            ..Self::laid(
                max_c,
                vec![Fuel {
                    kg: fat_kg,
                    mj_kg: 39.0,
                    thick_m: 0.01,
                    wet: 0.0,
                }],
            )
        }
    }

    pub fn fuel_kg(&self) -> f32 {
        self.fuel.iter().map(|f| f.kg).sum()
    }

    /// Whether it gives fire to cook on or take from (flames or glowing coals).
    pub fn lit(&self) -> bool {
        self.flaming || (self.coals_kg > 0.01 && !self.banked)
    }

    pub fn state(&self) -> FireState {
        if self.wick {
            return if self.flaming {
                FireState::Low
            } else {
                FireState::Out
            };
        }
        if self.banked && self.coals_kg > 0.002 {
            FireState::Banked
        } else if self.flaming {
            if self.power_kw > 6.0 {
                FireState::High
            } else {
                FireState::Low
            }
        } else if self.coals_kg > 0.01 {
            FireState::Embers
        } else {
            FireState::Out
        }
    }

    /// Lights it from an ember or a drilled coal: it needs fuel that is dry enough to catch.
    pub fn ignite(&mut self) -> bool {
        if !self.fuel.iter().any(|f| f.kg > 0.01 && f.wet < 0.4) {
            return false;
        }
        self.flaming = true;
        self.banked = false;
        self.involved = self.involved.max(0.1);
        self.coals_kg = self.coals_kg.max(0.01);
        true
    }

    /// Lays fuel on: on coals or flames it catches.
    pub fn feed(&mut self, fuel: Fuel) {
        if fuel.kg <= 0.0 {
            return;
        }
        self.fuel.push(fuel);
        if self.banked {
            self.banked = false;
        }
        if !self.flaming && self.coals_kg >= 0.03 && fuel.wet < 0.5 {
            self.flaming = true;
            self.involved = self.involved.max(0.2);
        }
    }

    /// Covers the coals with ash: they smoulder for hours.
    pub fn bank(&mut self) -> bool {
        if self.coals_kg < 0.02 {
            return false;
        }
        self.banked = true;
        self.flaming = false;
        true
    }

    /// Radiant heat at `d_m` from the fire (W/m²).
    pub fn radiant_w_m2(&self, d_m: f32) -> f32 {
        let d = d_m.max(0.5);
        (1000.0 * RADIANT_SHARE * self.power_kw / (4.0 * std::f32::consts::PI * d * d)).min(800.0)
    }

    /// Lives `dt_h` hours in air of `air_c` with `rain_mm_h` falling on it and `wind_m_s`.
    pub fn step(&mut self, dt_h: f32, air_c: f32, rain_mm_h: f32, wind_m_s: f32) {
        if dt_h <= 0.0 {
            return;
        }
        if self.wick {
            // A lamp's flame: six grams of fat an hour, a few tens of watts.
            let mut burned = 0.0;
            if self.flaming {
                if let Some(f) = self.fuel.first_mut() {
                    burned = (0.006 * dt_h).min(f.kg);
                    f.kg -= burned;
                }
                self.fuel.retain(|f| f.kg > 0.0005);
                if self.fuel.is_empty() || rain_mm_h > 2.0 {
                    self.flaming = false;
                }
            }
            self.power_kw = burned * 39.0 / dt_h / 3.6;
            self.temp_c = if self.flaming { self.max_c } else { air_c };
            return;
        }
        let mut energy_mj = 0.0f32;
        // Rain soaks the fuel and quenches coals; the water it brings takes heat to boil off
        // (a hearth's half square metre: about a third of a kilowatt per mm an hour), and a
        // fire giving less than twice that drowns.
        let drowning = rain_mm_h > 0.0 && self.power_kw < 2.0 * 0.31 * rain_mm_h;
        if rain_mm_h > 0.0 {
            for f in &mut self.fuel {
                f.wet = (f.wet + rain_mm_h * 0.05 * dt_h).min(1.0);
            }
        }
        if drowning && !self.banked {
            self.flaming = false;
            self.coals_kg -= self.coals_kg * (rain_mm_h * 1.5 * dt_h).min(1.0);
        }
        // The fire's heat dries what lies in it.
        let drying = 0.5 * ((self.temp_c - 100.0) / 500.0).clamp(0.0, 2.0) * dt_h;
        if drying > 0.0 {
            for f in &mut self.fuel {
                f.wet = (f.wet - drying).max(0.0);
            }
        }
        if self.banked {
            self.flaming = false;
        }
        // Flames: each piece burns from its surface in, as far as the fire has taken hold.
        if self.flaming {
            self.involved += (1.0 - self.involved) * (1.0 - (-dt_h / 0.05).exp());
            let draught = 1.0 + 0.1 * wind_m_s.clamp(0.0, 5.0);
            for f in &mut self.fuel {
                // The piece thins from both sides; its mass goes with its section.
                let dry = (1.0 - f.wet).powi(2);
                let shrink = 2.0 * REGRESSION_M_H * IN_FLAME * dry * draught * self.involved * dt_h;
                let thick = (f.thick_m - shrink).max(0.0);
                let left = if f.thick_m > 0.0 {
                    (thick / f.thick_m).powi(2)
                } else {
                    0.0
                };
                let burned = f.kg * (1.0 - left);
                f.thick_m = thick;
                f.kg -= burned;
                energy_mj += burned * (1.0 - CHAR_SHARE) * f.mj_kg;
                self.coals_kg += burned * CHAR_SHARE;
            }
            self.fuel.retain(|f| f.kg > 0.002 && f.thick_m > 0.0005);
            if self.fuel.is_empty() {
                self.flaming = false;
            }
        } else {
            self.involved = 0.0;
            let catches = self.fuel.iter().any(|f| f.wet < 0.5);
            if !self.banked && !drowning && self.coals_kg >= 0.03 && self.temp_c > 250.0 && catches
            {
                // Coals light dry fuel laid on them.
                self.flaming = true;
                self.involved = 0.2;
            }
        }
        // Coals glow down: slowly under ash, faster in the open.
        let k = if self.banked {
            0.06
        } else if self.flaming {
            0.3
        } else {
            0.6
        };
        let glowed = self.coals_kg * (1.0 - (-k * dt_h).exp());
        self.coals_kg -= glowed;
        energy_mj += glowed * COAL_MJ_KG;
        if self.coals_kg < 0.002 {
            self.coals_kg = 0.0;
            self.banked = false;
        }
        self.power_kw = energy_mj / dt_h / 3.6;
        let reach = (self.max_c - air_c).max(0.0);
        let target = if self.banked {
            air_c + reach.min(150.0) * (1.0 - (-self.power_kw / 0.3).exp())
        } else {
            air_c + reach * (1.0 - (-self.power_kw / 3.0).exp())
        };
        // The hearth's stones and coals follow the fire within a minute or two.
        let follow = 1.0 - (-dt_h / 0.02).exp();
        self.temp_c += (target - self.temp_c) * follow;
    }
}

impl FireState {
    /// The state a block shows (`campfire[fire=low]`).
    pub fn from_name(name: &str) -> Self {
        match name {
            "banked" => FireState::Banked,
            "embers" => FireState::Embers,
            "low" => FireState::Low,
            "high" => FireState::High,
            _ => FireState::Out,
        }
    }

    /// Roughly how hot a hearth in this state is (°C), for one who sees only its look.
    pub fn seen_temp_c(self, air_c: f32) -> f32 {
        match self {
            FireState::Out => air_c,
            FireState::Banked => 120.0,
            FireState::Embers => 350.0,
            FireState::Low => 550.0,
            FireState::High => 750.0,
        }
    }

    pub fn lit(self) -> bool {
        matches!(self, FireState::Embers | FireState::Low | FireState::High)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stick() -> Fuel {
        Fuel {
            kg: 0.37,
            mj_kg: 18.0,
            thick_m: 0.025,
            wet: 0.0,
        }
    }

    fn run(fire: &mut Fire, hours: f32, rain: f32) {
        let dt = 1.0 / 360.0;
        for _ in 0..(hours / dt) as usize {
            fire.step(dt, 10.0, rain, 1.0);
        }
    }

    #[test]
    fn a_stick_burns_for_most_of_an_hour() {
        assert!((stick().burn_h() - 0.65).abs() < 0.03);
        let mut f = Fire::laid(800.0, vec![stick(); 4]);
        assert!(f.ignite());
        run(&mut f, 0.1, 0.0);
        assert!(f.flaming, "burning after six minutes");
        assert!(
            (5.0..30.0).contains(&f.power_kw),
            "a small campfire: {} kW",
            f.power_kw
        );
        assert!(f.temp_c > 500.0, "hot: {} °C", f.temp_c);
        run(&mut f, 0.8, 0.0);
        assert!(!f.flaming, "four sticks are gone within the hour");
        assert_eq!(f.state(), FireState::Embers, "coals glow on");
        run(&mut f, 5.0, 0.0);
        assert_eq!(f.state(), FireState::Out);
    }

    #[test]
    fn banked_coals_last_the_night_and_relight() {
        let mut f = Fire::laid(800.0, vec![stick(); 8]);
        f.ignite();
        run(&mut f, 0.6, 0.0);
        assert!(f.bank());
        run(&mut f, 8.0, 0.0);
        assert_eq!(
            f.state(),
            FireState::Banked,
            "smouldering after eight hours"
        );
        f.feed(stick());
        run(&mut f, 0.05, 0.0);
        assert!(f.flaming, "a stick on the coals catches");
    }

    #[test]
    fn hard_rain_puts_a_small_fire_out_and_wet_wood_will_not_light() {
        let mut f = Fire::laid(800.0, vec![stick(); 2]);
        f.ignite();
        run(&mut f, 0.05, 0.0);
        run(&mut f, 1.0, 8.0);
        assert!(!f.lit(), "out in the downpour: {:?}", f.state());
        let mut wet = Fire::laid(
            800.0,
            vec![Fuel {
                wet: 0.8,
                ..stick()
            }],
        );
        assert!(!wet.ignite());
    }

    #[test]
    fn a_lamp_burns_its_fat_for_hours() {
        let mut lamp = Fire::lamp(300.0, 0.1);
        assert!(lamp.ignite());
        run(&mut lamp, 10.0, 0.0);
        assert_eq!(lamp.state(), FireState::Low, "still lit after ten hours");
        run(&mut lamp, 8.0, 0.0);
        assert_eq!(lamp.state(), FireState::Out, "its fat used up");
    }

    #[test]
    fn a_fire_warms_those_beside_it() {
        let mut f = Fire::laid(800.0, vec![stick(); 6]);
        f.ignite();
        run(&mut f, 0.1, 0.0);
        let near = f.radiant_w_m2(1.0);
        let far = f.radiant_w_m2(4.0);
        assert!(
            near > 80.0 && far < near / 10.0,
            "{near} W/m² at 1 m, {far} at 4 m"
        );
    }
}

//! The human body (v2 §9). Needs, heat and short illnesses run on the day scale of v2 §4.2 — a
//! game day stands for a real day, so a body needs a day's food and water each game day and
//! cools in cold rain over the real hour or two — and each injury and illness heals or runs on
//! the scale its data declares. Short-term stamina is the one thing measured in seconds of play,
//! because movement is not compressed.
//!
//! The parts: the heat balance ([`thermal`]), food energy ([`energy`]), body water ([`water`]),
//! sleep ([`sleep`]), injuries and illness ([`harm`]) and what covers the body ([`clothing`]).
//! [`Body::step`] advances them together; [`Body::effects`] tells movement and actions what the
//! body can do, and [`Body::status`] tells the player how it feels.
//!
//! The physiology uses one reference adult whatever the character looks like: height and build
//! are cosmetic (v2 §9.1).

pub mod clothing;
pub mod energy;
pub mod harm;
pub mod sleep;
pub mod thermal;
pub mod water;

use hearth_content::Content;
use hearth_content::balance::Balance;
use hearth_content::schema::TimeScale;
use hearth_content::schema::body::{BodyParams, BodyRegion, Illness, Injury};
use hearth_content::time::TimeScales;
use hearth_math::hash::{hash2, unit_f64};
use serde::{Deserialize, Serialize};

pub use clothing::{REGIONS, RegionCover, Worn, region_area};
pub use energy::{Energy, Food};
pub use harm::{IllnessState, InjuryState, Side};
pub use sleep::Sleep;
pub use thermal::{Flows, Thermal};
pub use water::Water;

use harm::{CLEANING, INFECTION_WINDOW_S, STEMMING, is_arm, is_leg};
use thermal::Drive;

/// How the body is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Posture {
    #[default]
    Standing,
    Sitting,
    Lying,
}

/// The surroundings acting on the body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Exposure {
    pub air_c: f32,
    /// Relative humidity, 0–1.
    pub humidity: f32,
    pub wind_m_s: f32,
    /// Rain falling on the body (mm of water per hour; 0 under cover).
    pub rain_mm_h: f32,
    /// Share of the body under water (0–1) and the water's temperature.
    pub immersion: f32,
    pub water_c: f32,
    /// Sun and fire radiation absorbed, averaged over the body's surface (W/m²).
    pub radiant_w_m2: f32,
    /// Mean radiant temperature minus air temperature (°C): negative under a clear night sky.
    pub sky_c_offset: f32,
    /// Insulation between a lying body and the ground (clo): bedding, boughs, hides.
    pub ground_clo: f32,
    /// Local solar time (hours), for the body clock.
    pub local_hour: f32,
    /// 0–1: noise and threat nearby (they wake sleepers).
    pub disturbance: f32,
}

impl Exposure {
    /// Mild, still, half-humid air in the shade at midday.
    pub fn mild() -> Self {
        Self {
            air_c: 20.0,
            humidity: 0.5,
            wind_m_s: 0.5,
            rain_mm_h: 0.0,
            immersion: 0.0,
            water_c: 15.0,
            radiant_w_m2: 0.0,
            sky_c_offset: 0.0,
            ground_clo: 0.0,
            local_hour: 12.0,
            disturbance: 0.0,
        }
    }
}

/// What the body is doing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Activity {
    /// Metabolic rate in METs (1 = resting), from the content's activity table.
    pub met: f32,
    /// 0–1 short-term effort: 1 is all-out (a sprint) and drains stamina in seconds.
    pub exertion: f32,
    pub posture: Posture,
    pub asleep: bool,
    /// The body's own speed through the air (m/s).
    pub speed_m_s: f32,
}

/// Balance multipliers the body reads (v2 §3.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rates {
    pub hunger: f64,
    pub thirst: f64,
    pub fatigue: f64,
    pub cold_stress: f64,
    pub heat_stress: f64,
    pub injury_severity: f64,
    pub healing: f64,
    pub illness_chance: f64,
}

impl Rates {
    pub fn of(balance: &Balance) -> Self {
        let g = |k: &str| balance.get(k) as f64;
        Self {
            hunger: g("hunger_rate"),
            thirst: g("thirst_rate"),
            fatigue: g("fatigue_rate"),
            cold_stress: g("cold_stress"),
            heat_stress: g("heat_stress"),
            injury_severity: g("injury_severity"),
            healing: g("healing_rate"),
            illness_chance: g("illness_chance"),
        }
    }

    /// Everything as it is in the real world.
    pub fn authentic() -> Self {
        Self {
            hunger: 1.0,
            thirst: 1.0,
            fatigue: 1.0,
            cold_stress: 1.0,
            heat_stress: 1.0,
            injury_severity: 1.0,
            healing: 1.0,
            illness_chance: 1.0,
        }
    }
}

/// The body's parameters in one world: the content's physiology, the world's balance and its
/// time scales.
#[derive(Debug, Clone)]
pub struct BodyConfig {
    pub params: BodyParams,
    pub injuries: Vec<Injury>,
    pub illnesses: Vec<Illness>,
    pub scales: TimeScales,
    pub rates: Rates,
    /// Whether weeks without fresh food bring on scurvy (the Authentic preset's slow danger).
    pub fresh_food_deficiency: bool,
    pub mass_kg: f64,
    pub height_m: f64,
    /// Skin area (m², DuBois).
    pub area_m2: f64,
    /// Basal metabolic rate (W).
    pub bmr_w: f64,
}

impl BodyConfig {
    /// The body of a world with balance `preset` (and its resolved `balance`) and `scales`.
    pub fn new(content: &Content, balance: &Balance, preset: &str, scales: TimeScales) -> Self {
        let mut c = Self::with_rates(content, Rates::of(balance), scales);
        c.fresh_food_deficiency = preset.ends_with("authentic");
        c
    }

    pub fn with_rates(content: &Content, rates: Rates, scales: TimeScales) -> Self {
        let p = content.body.clone();
        let mass = (p.mass_kg.0 + p.mass_kg.1) as f64 / 2.0;
        let height = (p.height_m.0 + p.height_m.1) as f64 / 2.0;
        let area = 0.007184 * mass.powf(0.425) * (height * 100.0).powf(0.725);
        let bmr_w = p.bmr_kcal_per_kg_day as f64 * mass * 4184.0 / 86_400.0;
        Self {
            injuries: content.injuries.iter().cloned().collect(),
            illnesses: content.illnesses.iter().cloned().collect(),
            params: p,
            scales,
            rates,
            fresh_food_deficiency: true,
            mass_kg: mass,
            height_m: height,
            area_m2: area,
            bmr_w,
        }
    }

    /// The metabolic rate of an activity (METs; 1 for unknown ones).
    pub fn met(&self, activity: &str) -> f32 {
        self.params
            .activity_met
            .get(activity)
            .copied()
            .unwrap_or(1.0)
    }

    /// An activity by its name in the content's table, with the exertion and posture it means.
    pub fn activity(&self, name: &str) -> Activity {
        let (exertion, posture, asleep, speed) = match name {
            "sleeping" => (0.0, Posture::Lying, true, 0.0),
            "resting" => (0.0, Posture::Sitting, false, 0.0),
            "standing" => (0.05, Posture::Standing, false, 0.0),
            "walking" => (0.15, Posture::Standing, false, self.params.walk_m_s),
            "jogging" => (0.45, Posture::Standing, false, self.params.jog_m_s),
            "sprinting" => (1.0, Posture::Standing, false, self.params.sprint_m_s),
            "swimming" => (0.5, Posture::Lying, false, self.params.swim_m_s),
            "climbing" => (0.7, Posture::Standing, false, 0.3),
            "carrying_heavy" => (0.6, Posture::Standing, false, 1.0),
            _ => (0.3, Posture::Standing, false, 0.0),
        };
        Activity {
            met: self.met(name),
            exertion,
            posture,
            asleep,
            speed_m_s: speed,
        }
    }

    /// An injury type by id (`hearth:sprain` or `sprain`).
    pub fn injury(&self, id: &str) -> Option<&Injury> {
        self.injuries.iter().find(|i| same_id(&i.id, id))
    }

    pub fn illness(&self, id: &str) -> Option<&Illness> {
        self.illnesses.iter().find(|i| same_id(&i.id, id))
    }

    /// Seconds of play a real duration takes on a scale.
    pub fn play_seconds(&self, real_hours: f64, scale: TimeScale) -> f64 {
        self.scales.play_seconds(real_hours, scale)
    }
}

fn same_id(full: &str, id: &str) -> bool {
    full == id || full.rsplit_once(':').is_some_and(|(_, short)| short == id)
}

/// How a body died.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Death {
    Hypothermia,
    HeatStroke,
    Dehydration,
    Starvation,
    BloodLoss,
    Drowning,
    /// An illness (content id).
    Illness(String),
    /// A fatal injury, such as a long fall (what caused it).
    Injury(String),
}

/// Why food was not eaten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The stomach is full.
    Full,
    /// Nausea: nothing stays down.
    Nauseous,
    Dead,
}

/// What the body can do: for movement, actions and the view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Effects {
    /// Multiplier on walking and running speed.
    pub walk: f32,
    pub sprint: bool,
    pub jump: bool,
    /// Both hands usable (a broken arm leaves one).
    pub two_hands: bool,
    /// 0–1 grip and fine use of the hands (pain, numb or shivering hands lose it).
    pub grip: f32,
    /// 0–1 of full strength (illness, blood loss, starvation, thirst, heat).
    pub strength: f32,
    /// 0–1 clarity of sight (blood loss, exhaustion and deep cold dim and blur it).
    pub vision: f32,
    pub conscious: bool,
    /// 0–1, for animation and clumsiness.
    pub shivering: f32,
    pub sweating: f32,
    /// 0–1.
    pub pain: f32,
}

/// Hunger as the body feels it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Hunger {
    Stuffed,
    Full,
    Satisfied,
    Peckish,
    Hungry,
    VeryHungry,
    Starving,
}

/// Thirst as the body feels it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Thirst {
    Sated,
    Fine,
    Thirsty,
    VeryThirsty,
    Parched,
    Dying,
}

/// Warmth as the body feels it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Warmth {
    /// Severe hypothermia (core below 32 °C).
    Freezing,
    /// Hypothermic (core below 35 °C).
    Hypothermic,
    Cold,
    Chilly,
    Comfortable,
    Warm,
    Hot,
    /// Heat illness (core above 39.5 °C).
    Overheating,
}

/// Tiredness as the body feels it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tiredness {
    Rested,
    Awake,
    Tired,
    VeryTired,
    Exhausted,
}

/// What wakes a sleeper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wake {
    Rested,
    Cold,
    Heat,
    Wet,
    Pain,
    Hunger,
    Thirst,
    Disturbed,
}

macro_rules! keys {
    ($t:ty, $prefix:literal, $($v:ident => $k:literal),* $(,)?) => {
        impl $t {
            /// Localisation key of the state's words.
            pub fn key(self) -> &'static str {
                match self { $(Self::$v => concat!($prefix, $k)),* }
            }
        }
    };
}

keys!(Hunger, "body.hunger.", Stuffed => "stuffed", Full => "full", Satisfied => "satisfied",
    Peckish => "peckish", Hungry => "hungry", VeryHungry => "very_hungry", Starving => "starving");
keys!(Thirst, "body.thirst.", Sated => "sated", Fine => "fine", Thirsty => "thirsty",
    VeryThirsty => "very_thirsty", Parched => "parched", Dying => "dying");
keys!(Warmth, "body.warmth.", Freezing => "freezing", Hypothermic => "hypothermic", Cold => "cold",
    Chilly => "chilly", Comfortable => "comfortable", Warm => "warm", Hot => "hot",
    Overheating => "overheating");
keys!(Tiredness, "body.tiredness.", Rested => "rested", Awake => "awake", Tired => "tired",
    VeryTired => "very_tired", Exhausted => "exhausted");
keys!(Wake, "body.wake.", Rested => "rested", Cold => "cold", Heat => "heat", Wet => "wet",
    Pain => "pain", Hunger => "hunger", Thirst => "thirst", Disturbed => "disturbed");

/// How the body feels, for the Body panel and the diegetic cues.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Status {
    pub hunger: Hunger,
    pub thirst: Thirst,
    pub warmth: Warmth,
    pub tiredness: Tiredness,
    pub core_c: f32,
    pub skin_c: f32,
    /// 0–1 share of the blood lost.
    pub blood_lost: f32,
    pub bleeding: bool,
    pub sick: bool,
    pub stamina: f32,
    /// 0–1 wetness of skin and clothing.
    pub wet: f32,
    pub effects: Effects,
}

/// A human body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Body {
    pub thermal: Thermal,
    pub energy: Energy,
    pub water: Water,
    pub sleep: Sleep,
    /// Short-term stamina, 0–1.
    pub stamina: f64,
    pub blood_l: f64,
    pub injuries: Vec<InjuryState>,
    pub illnesses: Vec<IllnessState>,
    pub dead: Option<Death>,
    /// Real seconds lived (body time).
    pub age_s: f64,
    /// Real seconds the skin of the hands, the feet and the face has been freezing.
    #[serde(default)]
    pub freezing_s: [f64; 3],
    seed: u64,
    draws: u64,
    /// The last step's heat flows (not saved).
    #[serde(skip)]
    pub last: Flows,
}

/// Core temperatures of death by cold and by heat (°C).
const DEATH_COLD_C: f64 = 26.0;
const DEATH_HOT_C: f64 = 43.0;
/// Share of the blood whose loss kills.
const FATAL_BLOOD_LOSS: f64 = 0.4;
/// Body fat (share of mass) below which starvation kills.
const FATAL_FAT: f64 = 0.015;
/// Blood made back per real day when fed and watered (l).
const BLOOD_REGEN_L_DAY: f64 = 0.3;

impl Body {
    /// A healthy, rested, fed body; `seed` makes its chances (infection, illness) repeatable.
    pub fn new(cfg: &BodyConfig, seed: u64) -> Self {
        Self {
            thermal: Thermal::default(),
            energy: Energy::new(cfg.mass_kg),
            water: Water::default(),
            sleep: Sleep::default(),
            stamina: 1.0,
            blood_l: cfg.params.blood_l as f64,
            injuries: Vec::new(),
            illnesses: Vec::new(),
            dead: None,
            age_s: 0.0,
            freezing_s: [0.0; 3],
            seed,
            draws: 0,
            last: Flows::default(),
        }
    }

    fn roll(&mut self) -> f64 {
        self.draws += 1;
        unit_f64(hash2(self.seed, self.draws))
    }

    /// Advances the body by `play_dt` seconds of play in `exposure`, wearing `worn`, doing
    /// `activity`.
    pub fn step(
        &mut self,
        cfg: &BodyConfig,
        play_dt: f64,
        exposure: &Exposure,
        worn: &Worn,
        activity: &Activity,
    ) {
        if self.dead.is_some() || play_dt <= 0.0 {
            return;
        }
        let dt = play_dt / cfg.scales.factor(TimeScale::Day);
        let mass = cfg.mass_kg;
        self.age_s += dt;
        let fx = self.effects(cfg);
        let r = &cfg.rates;

        // Heat. Weakness caps the effort; starved bodies shiver less.
        let met = 1.0 + (activity.met as f64 - 1.0).max(0.0) * fx.strength as f64;
        let met = if activity.met < 1.0 {
            activity.met as f64
        } else {
            met
        };
        let fuel = 0.3 + 0.7 * self.energy.glycogen_frac(mass).max(0.3);
        let shiver_met = cfg.met("shivering_max") as f64;
        let drive = Drive {
            area_m2: cfg.area_m2,
            mass_kg: mass,
            metabolic_w: met * cfg.bmr_w,
            shiver_max_w: (shiver_met - 1.0).max(0.0) * cfg.bmr_w * fuel,
            fever_c: self.fever_c(cfg),
            sweat_capacity: (1.0 - self.water.deficit_share(mass) / 0.1).clamp(0.2, 1.0),
            cold_stress: r.cold_stress,
            heat_stress: r.heat_stress,
            air_speed: (exposure.wind_m_s + activity.speed_m_s) as f64,
            posture: activity.posture,
        };
        let flows = self.thermal.step(&drive, exposure, worn, dt);
        self.last = flows;

        // Food energy.
        let kcal = (drive.metabolic_w + flows.shivering_w) * dt / 4184.0 * r.hunger;
        let absorbed = self.energy.digest(mass, dt);
        self.energy.burn(kcal, mass, dt);

        // Water.
        let illness_l_h: f64 = self
            .active_illness_effects(cfg)
            .map(|e| match e {
                "diarrhoea" => 0.15,
                "dehydration" => 0.08,
                "nausea" => 0.03,
                _ => 0.0,
            })
            .sum();
        let losses = flows.sweat_kg_s + flows.insensible_kg_s + illness_l_h / 3600.0;
        let gained = absorbed + kcal * water::METABOLIC_WATER_L_PER_KCAL;
        self.water.step(mass, losses, gained, r.thirst, dt);

        // Sleep.
        let quality = self.sleep_quality(exposure, &fx);
        self.sleep.step(
            activity.asleep,
            quality,
            activity.exertion as f64,
            r.fatigue,
            dt,
        );

        // Stamina, in seconds of play.
        let sp = cfg.params.stamina;
        let effort = activity.exertion as f64;
        if effort > 0.35 {
            self.stamina -= effort / sp.all_out_s.max(1.0) as f64 * play_dt;
        } else {
            let tired = (self.sleep.pressure - 0.6).max(0.0) / 0.4;
            let condition = (1.0 - 0.5 * tired)
                * fx.strength as f64
                * if self.energy.glycogen_frac(mass) < 0.1 {
                    0.6
                } else {
                    1.0
                };
            self.stamina +=
                (1.0 - effort / 0.35) * condition / sp.recover_s.max(1.0) as f64 * play_dt;
        }
        self.stamina = self.stamina.clamp(0.0, 1.0);

        self.step_frostbite(cfg, dt);
        self.step_harm(cfg, play_dt, dt);
        self.check_death(cfg);
    }

    /// Skin that stays frozen (below −0.5 °C) for ten minutes is frostbitten, more severely the
    /// longer it stays so.
    fn step_frostbite(&mut self, cfg: &BodyConfig, dt: f64) {
        const PARTS: [BodyRegion; 3] = [BodyRegion::Hand, BodyRegion::Foot, BodyRegion::Head];
        for (k, &r) in PARTS.iter().enumerate() {
            let skin = self.thermal.regions_c[clothing::region_index(r)] as f64;
            if skin > -0.5 {
                self.freezing_s[k] = (self.freezing_s[k] - dt).max(0.0);
                continue;
            }
            self.freezing_s[k] += dt;
            let frozen_min = self.freezing_s[k] / 60.0;
            if frozen_min < 10.0 {
                continue;
            }
            // Deeper with time and cold.
            let severity =
                ((frozen_min - 10.0) / 120.0 + (-skin - 0.5) / 20.0 + 0.15).min(1.0) as f32;
            let side = if r == BodyRegion::Head {
                Side::Middle
            } else {
                Side::Left
            };
            match self
                .injuries
                .iter_mut()
                .find(|i| i.region == r && i.id.ends_with("frostbite"))
            {
                Some(i) => i.severity = i.severity.max(severity),
                None => {
                    self.injure(cfg, "frostbite", r, side, severity);
                }
            }
        }
    }

    /// Fever: a raised set point while an illness with fever runs.
    fn fever_c(&self, cfg: &BodyConfig) -> f64 {
        if self.active_illness_effects(cfg).any(|e| e == "fever") {
            1.8
        } else {
            0.0
        }
    }

    fn active_illness_effects<'a>(
        &'a self,
        cfg: &'a BodyConfig,
    ) -> impl Iterator<Item = &'a str> + 'a {
        self.illnesses
            .iter()
            .filter(|i| i.active())
            .filter_map(|i| cfg.illness(&i.id))
            .flat_map(|k| k.effects.iter().map(String::as_str))
    }

    /// How restful sleep is here and now, 0–1.
    fn sleep_quality(&self, e: &Exposure, fx: &Effects) -> f64 {
        let warm = ((self.thermal.skin_c - 26.0) / 5.0).clamp(0.2, 1.0);
        let dry = 1.0 - 0.5 * (self.thermal.wet_kg_m2 / 0.12).clamp(0.0, 1.0);
        let soft = 0.75 + 0.25 * (e.ground_clo as f64 / 0.5).clamp(0.0, 1.0);
        warm * dry * soft * (1.0 - 0.6 * fx.pain as f64) * (1.0 - 0.8 * e.disturbance as f64)
    }

    fn step_harm(&mut self, cfg: &BodyConfig, play_dt: f64, dt: f64) {
        let mass = cfg.mass_kg;
        // How well the body heals now.
        let mut condition = 1.0;
        if self.energy.fasting_s > 86_400.0 {
            condition *= 0.5;
        }
        condition *= (1.0 - self.water.deficit_share(mass) / 0.1).clamp(0.3, 1.0);
        if self.thermal.core_c < 36.0 {
            condition *= 0.7;
        }
        if cfg.fresh_food_deficiency && self.energy.fresh_days <= 0.0 {
            condition *= 0.4;
        }
        if self.sleep.asleep_s > 0.0 {
            condition *= 1.3;
        }

        let mut blood_lost = 0.0;
        let mut rolls = Vec::new();
        for (index, inj) in self.injuries.iter_mut().enumerate() {
            let Some(kind) = cfg.injury(&inj.id) else {
                inj.healed = 1.0;
                continue;
            };
            inj.age_s += dt;
            // Bleeding, slowed by pressure and bandages, clotting over time.
            let stemmed = inj.treated_with(&STEMMING);
            let rate = inj.bleeding_ml_min * if stemmed { 0.1 } else { 1.0 };
            blood_lost += rate / 60.0 * dt / 1000.0;
            let clot = inj.clotting_s(kind) / if stemmed { 3.0 } else { 1.0 };
            inj.bleeding_ml_min *= (-dt / clot).exp();
            // An uncleaned wound may become infected.
            if !inj.infection_rolled && kind.infection_risk > 0.0 && inj.age_s > INFECTION_WINDOW_S
            {
                inj.infection_rolled = true;
                if !inj.treated_with(&CLEANING) {
                    let chance = kind.infection_risk as f64
                        * cfg.rates.illness_chance
                        * (0.5 + inj.severity as f64);
                    rolls.push((index, chance));
                }
            }
            // Healing over the injury's time on its scale.
            let hours = kind.heal.hours as f64 * (0.5 + inj.severity as f64);
            let total = cfg.play_seconds(hours, kind.heal.scale).max(1.0);
            let mut speed = cfg.rates.healing * condition;
            if inj.infected {
                speed *= 0.4;
            }
            if kind.effects.iter().any(|e| e == "no_use_of_limb") && !inj.treated_with(&["splint"])
            {
                speed *= 0.5;
            }
            inj.healed += play_dt / total * speed;
        }
        // Roll the infections (after the loop: rolling needs the body).
        let mut caught = false;
        for (index, chance) in rolls {
            if self.roll() < chance {
                self.injuries[index].infected = true;
                caught = true;
            }
        }
        if caught {
            self.catch_illness(cfg, "wound_infection");
        }
        self.injuries.retain(|i| i.healed < 1.0);

        // Blood: lost from wounds, made back slowly when fed and watered.
        let full = cfg.params.blood_l as f64;
        let regen = if self.water.deficit_share(mass) < 0.03 && self.energy.fasting_s < 86_400.0 {
            BLOOD_REGEN_L_DAY / 86_400.0 * dt
        } else {
            0.0
        };
        self.blood_l = (self.blood_l - blood_lost + regen).min(full);

        // Illnesses: onset, then their course; a fatal course turned by any treatment.
        let mut died = None;
        for ill in &mut self.illnesses {
            if ill.onset_s > 0.0 {
                ill.onset_s -= play_dt;
                continue;
            }
            let Some(kind) = cfg.illness(&ill.id) else {
                ill.left_s = 0.0;
                continue;
            };
            let rest = if self.sleep.asleep_s > 0.0 { 1.3 } else { 1.0 };
            ill.left_s -= play_dt * cfg.rates.healing * rest;
            if ill.left_s <= 0.0 && ill.fatal && !ill.treated(kind) {
                died = Some(Death::Illness(kind.id.clone()));
            }
        }
        self.illnesses.retain(|i| i.left_s > 0.0);
        if died.is_some() {
            self.dead = died;
        }
    }

    fn check_death(&mut self, cfg: &BodyConfig) {
        if self.dead.is_some() {
            return;
        }
        let mass = cfg.mass_kg;
        let t = &self.thermal;
        self.dead = if t.core_c <= DEATH_COLD_C {
            Some(Death::Hypothermia)
        } else if t.core_c >= DEATH_HOT_C {
            Some(Death::HeatStroke)
        } else if self.water.deficit_share(mass) >= cfg.params.fatal_water_loss as f64 {
            Some(Death::Dehydration)
        } else if self.blood_l <= (1.0 - FATAL_BLOOD_LOSS) * cfg.params.blood_l as f64 {
            Some(Death::BloodLoss)
        } else if self.energy.fat_share(mass) <= FATAL_FAT {
            Some(Death::Starvation)
        } else {
            None
        };
    }

    /// Eats `food` if the stomach has room and nausea allows.
    pub fn eat(&mut self, cfg: &BodyConfig, food: &Food) -> Result<(), Refusal> {
        if self.dead.is_some() {
            return Err(Refusal::Dead);
        }
        if self.active_illness_effects(cfg).any(|e| e == "nausea") {
            return Err(Refusal::Nauseous);
        }
        if self.energy.room_l(cfg.params.stomach_capacity_l as f64) < food.volume_l {
            return Err(Refusal::Full);
        }
        self.energy.swallow(food);
        Ok(())
    }

    /// Drinks up to `litres` of water with `salt_g_l` of salt per litre (35 for seawater); a
    /// share `pathogen` (0–1) of such drinks makes people ill. Returns the litres drunk (the
    /// stomach holds what it holds).
    pub fn drink(&mut self, cfg: &BodyConfig, litres: f64, salt_g_l: f64, pathogen: f64) -> f64 {
        if self.dead.is_some() {
            return 0.0;
        }
        let l = litres.min(self.energy.room_l(cfg.params.stomach_capacity_l as f64));
        if l <= 0.0 {
            return 0.0;
        }
        self.energy.swallow(&Food::water(l));
        self.water.deficit_l += Water::salt_cost_l(l, salt_g_l);
        if pathogen > 0.0 {
            let chance = 1.0 - (1.0 - pathogen.clamp(0.0, 1.0)).powf(l / 0.25);
            self.expose(cfg, "bad_water", chance);
        }
        l
    }

    /// Exposes the body to a cause of illness (`raw_meat`, `bad_water`, `mosquito`...) with a
    /// `chance` of catching each illness it causes.
    pub fn expose(&mut self, cfg: &BodyConfig, cause: &str, chance: f64) {
        let ids: Vec<String> = cfg
            .illnesses
            .iter()
            .filter(|i| i.causes.iter().any(|c| c == cause))
            .map(|i| i.id.clone())
            .collect();
        for id in ids {
            if self.roll() < chance * cfg.rates.illness_chance {
                self.catch_illness(cfg, &id);
            }
        }
    }

    /// Catches an illness now (if not already ill with it).
    pub fn catch_illness(&mut self, cfg: &BodyConfig, id: &str) {
        let Some(kind) = cfg.illness(id) else {
            return;
        };
        if self.illnesses.iter().any(|i| i.id == kind.id) {
            return;
        }
        let fatal = self.roll() < kind.lethality as f64;
        self.illnesses.push(IllnessState {
            id: kind.id.clone(),
            onset_s: cfg.play_seconds(kind.onset.hours as f64, kind.onset.scale),
            left_s: cfg.play_seconds(kind.lasts.hours as f64, kind.lasts.scale),
            fatal,
            treatments: Vec::new(),
        });
    }

    /// Injures a region (`severity` 0–1 before the balance's injury severity). Returns the
    /// injury's index, or `None` for an unknown injury or a region it cannot affect.
    pub fn injure(
        &mut self,
        cfg: &BodyConfig,
        id: &str,
        region: BodyRegion,
        side: Side,
        severity: f32,
    ) -> Option<usize> {
        let kind = cfg.injury(id)?;
        if !kind.regions.contains(&region) || self.dead.is_some() {
            return None;
        }
        let s = (severity as f64 * cfg.rates.injury_severity).clamp(0.0, 1.0) as f32;
        self.injuries.push(InjuryState::new(kind, region, side, s));
        Some(self.injuries.len() - 1)
    }

    /// Applies a treatment to an injury.
    pub fn treat_injury(&mut self, index: usize, treatment: &str) {
        if let Some(i) = self.injuries.get_mut(index)
            && !i.treatments.iter().any(|t| t == treatment)
        {
            i.treatments.push(treatment.to_owned());
        }
    }

    /// Applies a treatment to an illness.
    pub fn treat_illness(&mut self, cfg: &BodyConfig, id: &str, treatment: &str) {
        let Some(kind) = cfg.illness(id) else {
            return;
        };
        if let Some(i) = self.illnesses.iter_mut().find(|i| i.id == kind.id)
            && !i.treatments.iter().any(|t| t == treatment)
        {
            i.treatments.push(treatment.to_owned());
        }
    }

    /// Ends the body's life (drowning, a fatal fall...).
    pub fn kill(&mut self, death: Death) {
        if self.dead.is_none() {
            self.dead = Some(death);
        }
    }

    /// What the body can do now.
    pub fn effects(&self, cfg: &BodyConfig) -> Effects {
        let mass = cfg.mass_kg;
        let mut fx = Effects {
            walk: 1.0,
            sprint: true,
            jump: true,
            two_hands: true,
            grip: 1.0,
            strength: 1.0,
            vision: 1.0,
            conscious: true,
            shivering: 0.0,
            sweating: 0.0,
            pain: 0.0,
        };
        let mut pain = 0.0;
        for inj in &self.injuries {
            let Some(kind) = cfg.injury(&inj.id) else {
                continue;
            };
            pain += inj.pain(kind);
            let left = (1.0 - inj.healed) as f32;
            let sev = inj.severity * left;
            for e in &kind.effects {
                match e.as_str() {
                    "slow_walk" if is_leg(inj.region) => fx.walk *= 1.0 - 0.5 * sev.max(0.3),
                    "no_sprint" if is_leg(inj.region) => fx.sprint = false,
                    "no_use_of_limb" if is_leg(inj.region) => {
                        fx.walk *= 0.4;
                        fx.sprint = false;
                        fx.jump = false;
                    }
                    "no_use_of_limb" | "no_two_hands" if is_arm(inj.region) => {
                        fx.two_hands = false;
                    }
                    "grip_pain" if is_arm(inj.region) => fx.grip *= 1.0 - 0.4 * sev,
                    "numbness" | "clumsiness" if is_arm(inj.region) => fx.grip *= 0.6,
                    "stiffness" => fx.walk *= 1.0 - 0.1 * sev,
                    "weakness" => fx.strength = fx.strength.min(1.0 - 0.3 * sev),
                    _ => {}
                }
            }
            if is_leg(inj.region) && inj.id.ends_with("sprain") {
                fx.jump = false;
            }
        }
        fx.pain = (pain as f32).min(1.0);
        for e in self.active_illness_effects(cfg) {
            match e {
                "weakness" => fx.strength = fx.strength.min(0.6),
                "fever" | "chills" => fx.strength = fx.strength.min(0.75),
                _ => {}
            }
        }
        // Blood loss: weak past 15 %, failing past 30 %.
        let lost = 1.0 - self.blood_l / cfg.params.blood_l as f64;
        if lost > 0.15 {
            fx.strength = fx.strength.min((1.0 - (lost - 0.15) / 0.25) as f32);
            fx.sprint &= lost < 0.2;
            fx.vision = fx.vision.min((1.0 - (lost - 0.15) / 0.25) as f32);
        }
        // Water.
        let dry = self.water.deficit_share(mass);
        if dry > 0.03 {
            fx.strength = fx.strength.min((1.0 - (dry - 0.03) / 0.12) as f32);
            fx.sprint &= dry < 0.06;
        }
        // Food: days of fasting, too much lean meat, no fresh food.
        if self.energy.fasting_s > 2.0 * 86_400.0 {
            fx.strength = fx.strength.min(0.75);
        }
        if self.energy.protein_share() > 0.45 {
            fx.strength = fx.strength.min(0.7);
        }
        if cfg.fresh_food_deficiency && self.energy.fresh_days <= 0.0 {
            fx.strength = fx.strength.min(0.7);
        }
        // Heat and cold.
        let core = self.thermal.core_c;
        let skin = self.thermal.skin_c;
        if core > 39.0 {
            fx.strength = fx.strength.min(0.6);
            fx.sprint = false;
        }
        if core < 35.0 {
            fx.strength = fx
                .strength
                .min(((core - 30.0) / 5.0).clamp(0.2, 1.0) as f32);
            fx.sprint = false;
            fx.vision = fx.vision.min(((core - 30.0) / 5.0).clamp(0.3, 1.0) as f32);
        }
        fx.grip *= ((skin - 12.0) / 14.0).clamp(0.35, 1.0) as f32;
        fx.shivering = (self.last.shivering_w / (2.0 * cfg.bmr_w)).clamp(0.0, 1.0) as f32;
        fx.sweating = (self.last.sweat_kg_s * 3600.0 / 1.0).clamp(0.0, 1.0) as f32;
        fx.grip *= 1.0 - 0.3 * fx.shivering;
        fx.grip *= 1.0 - 0.4 * fx.pain;
        // Exhaustion blurs sight.
        let sleepy = self.sleep.pressure;
        if sleepy > 0.85 {
            fx.vision = fx.vision.min((1.0 - (sleepy - 0.85) * 2.0) as f32);
        }
        fx.walk *= fx.strength.max(0.4);
        fx.conscious = core > 30.0 && lost < 0.35 && self.dead.is_none();
        if !fx.conscious {
            fx.walk = 0.0;
            fx.sprint = false;
            fx.jump = false;
        }
        fx
    }

    /// How the body feels.
    pub fn status(&self, cfg: &BodyConfig) -> Status {
        let mass = cfg.mass_kg;
        let cap = cfg.params.stomach_capacity_l as f64;
        let fill = self.energy.stomach.volume_l / cap.max(1e-3);
        let glyco = self.energy.glycogen_frac(mass);
        let hunger = if fill > 0.9 {
            Hunger::Stuffed
        } else if fill > 0.5 {
            Hunger::Full
        } else if self.energy.fasting_s > 2.0 * 86_400.0 {
            Hunger::Starving
        } else if fill > 0.2 || glyco > 0.7 {
            Hunger::Satisfied
        } else if glyco > 0.5 {
            Hunger::Peckish
        } else if glyco > 0.2 {
            Hunger::Hungry
        } else {
            Hunger::VeryHungry
        };
        let dry = self.water.deficit_share(mass);
        let thirst = match dry {
            d if d < 0.005 => Thirst::Sated,
            d if d < 0.015 => Thirst::Fine,
            d if d < 0.03 => Thirst::Thirsty,
            d if d < 0.06 => Thirst::VeryThirsty,
            d if d < 0.1 => Thirst::Parched,
            _ => Thirst::Dying,
        };
        let (core, skin) = (self.thermal.core_c, self.thermal.skin_c);
        let warmth = if core < 32.0 {
            Warmth::Freezing
        } else if core < 35.0 {
            Warmth::Hypothermic
        } else if core > 39.5 {
            Warmth::Overheating
        } else if core > 38.3 {
            Warmth::Hot
        } else if core < 36.3 || skin < 26.0 {
            Warmth::Cold
        } else if skin < 30.5 {
            Warmth::Chilly
        } else if skin > 35.0 || self.last.sweat_kg_s > 1e-4 {
            Warmth::Warm
        } else {
            Warmth::Comfortable
        };
        let tiredness = match self.sleep.pressure {
            p if p < 0.2 => Tiredness::Rested,
            p if p < 0.55 => Tiredness::Awake,
            p if p < 0.75 => Tiredness::Tired,
            p if p < 0.9 => Tiredness::VeryTired,
            _ => Tiredness::Exhausted,
        };
        Status {
            hunger,
            thirst,
            warmth,
            tiredness,
            core_c: core as f32,
            skin_c: skin as f32,
            blood_lost: (1.0 - self.blood_l / cfg.params.blood_l as f64).max(0.0) as f32,
            bleeding: self.injuries.iter().any(|i| i.bleeding_ml_min > 0.5),
            sick: self.illnesses.iter().any(IllnessState::active),
            stamina: self.stamina as f32,
            wet: (self.thermal.wet_kg_m2 / 0.12).clamp(0.0, 1.0) as f32,
            effects: self.effects(cfg),
        }
    }

    /// What would wake a sleeper now, if anything (the world decides to wake them).
    pub fn wakes(&self, cfg: &BodyConfig, e: &Exposure) -> Option<Wake> {
        let s = self.status(cfg);
        if e.disturbance > 0.5 {
            Some(Wake::Disturbed)
        } else if s.effects.pain > 0.6 {
            Some(Wake::Pain)
        } else if matches!(
            s.warmth,
            Warmth::Cold | Warmth::Hypothermic | Warmth::Freezing
        ) {
            Some(Wake::Cold)
        } else if matches!(s.warmth, Warmth::Hot | Warmth::Overheating) {
            Some(Wake::Heat)
        } else if e.rain_mm_h > 0.5 && s.wet > 0.6 {
            Some(Wake::Wet)
        } else if s.thirst >= Thirst::VeryThirsty {
            Some(Wake::Thirst)
        } else if s.hunger >= Hunger::VeryHungry {
            Some(Wake::Hunger)
        } else if self.sleep.pressure < 0.08 && self.sleep.asleep_s > 4.0 * 3600.0 {
            Some(Wake::Rested)
        } else {
            None
        }
    }
}

//! The player (v2 §9): a body ([`hearth_body`]) moving through the world ([`hearth_physics`]).
//! What the body allows becomes how it can move; what it does becomes what it burns; landings
//! injure it, the water it is in cools it, and the air it lacks drowns it.

use glam::DVec3;
use hearth_body::{Activity, Body, BodyConfig, Death, Exposure, Posture, Wake, Worn};
use hearth_physics::{Ability, Intent, Motion, Mover, Report, Stance, Terrain};
use serde::{Deserialize, Serialize};

/// Seconds without air after which a body drowns (after the held breath is spent).
pub const DROWN_S: f64 = 60.0;
/// Seconds without air after which a body loses consciousness.
pub const FAINT_S: f64 = 25.0;
/// Seconds lying sleepy and at ease before sleep comes (play time).
pub const DROP_OFF_S: f64 = 5.0;
/// How sleepy (0–1) a body must be to drop off.
pub const SLEEPY: f64 = 0.3;

/// A player's body and where it is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub body: Body,
    pub mover: Mover,
    /// Asleep (the world decides when to wake).
    pub asleep: bool,
    /// Lying down to rest or sleep.
    #[serde(default)]
    pub lying: bool,
    /// Seconds lying sleepy and at ease: sleep comes after a few.
    #[serde(skip)]
    pub drowsy_s: f64,
}

impl Player {
    pub fn new(cfg: &BodyConfig, feet: DVec3, seed: u64) -> Self {
        Self {
            body: Body::new(cfg, seed),
            mover: Mover::new(feet),
            asleep: false,
            lying: false,
            drowsy_s: 0.0,
        }
    }

    /// What the body allows the mover now.
    pub fn ability(&self, cfg: &BodyConfig) -> Ability {
        let fx = self.body.effects(cfg);
        let p = &cfg.params;
        let strength = fx.strength as f64;
        let walk = fx.walk as f64;
        let walk_m_s = p.walk_m_s as f64 * walk;
        Ability {
            walk_m_s,
            jog_m_s: if fx.jog {
                p.jog_m_s as f64 * walk
            } else {
                walk_m_s
            },
            sprint_m_s: p.sprint_m_s as f64 * walk,
            swim_m_s: p.swim_m_s as f64 * strength.max(0.3),
            sprint: fx.sprint && self.body.stamina > 0.05,
            jump_m: if fx.jump { 0.45 * strength } else { 0.0 },
            climb: fx.two_hands && strength > 0.5 && self.body.stamina > 0.1,
            breath_s: 45.0 * strength.max(0.4),
            stamina: self.body.stamina,
        }
    }

    /// Lying down: sleep comes to a sleepy body at ease after a few seconds and lasts until
    /// something wakes it (`hour` is the local hour, `dt` seconds of play). Returns why it
    /// woke, if it woke on this step.
    pub fn rest(&mut self, cfg: &BodyConfig, e: &Exposure, hour: f64, dt: f64) -> Option<Wake> {
        if self.body.dead.is_some() {
            self.asleep = false;
            self.lying = false;
            return None;
        }
        if self.asleep {
            let why = self.body.wakes(cfg, e)?;
            self.asleep = false;
            self.lying = false;
            return Some(why);
        }
        if self.lying
            && self.body.wakes(cfg, e).is_none()
            && self.body.sleep.sleepiness(hour) >= SLEEPY
        {
            self.drowsy_s += dt;
            if self.drowsy_s >= DROP_OFF_S {
                self.asleep = true;
            }
        } else {
            self.drowsy_s = 0.0;
        }
        None
    }

    /// Whether the player can act (awake and conscious).
    pub fn can_act(&self, cfg: &BodyConfig) -> bool {
        !self.asleep
            && !self.lying
            && self.body.dead.is_none()
            && self.body.effects(cfg).conscious
            && self.mover.airless_s < FAINT_S
    }

    /// Advances by `dt` seconds of play in `surroundings` (the weather and water where the
    /// player stands; the immersion comes from the movement), wearing `worn`.
    pub fn tick(
        &mut self,
        cfg: &BodyConfig,
        world: &impl Terrain,
        intent: &Intent,
        surroundings: &Exposure,
        worn: &Worn,
        dt: f64,
    ) -> Report {
        let acting = self.can_act(cfg);
        // A body that cannot act goes limp: in water it sinks, on a ladder it slides down.
        let idle = Intent {
            descend: true,
            ..Intent::default()
        };
        let ability = self.ability(cfg);
        let report = hearth_physics::step(
            world,
            &mut self.mover,
            if acting { intent } else { &idle },
            &ability,
            dt,
        );
        if let Some(v) = report.landed {
            self.body.land(cfg, v);
        }
        if report.airless_s >= DROWN_S {
            self.body.kill(Death::Drowning);
        }
        let exposure = Exposure {
            immersion: report.immersion as f32,
            // Under water the rain does not matter and the wind does not reach.
            rain_mm_h: if report.immersion > 0.8 {
                0.0
            } else {
                surroundings.rain_mm_h
            },
            wind_m_s: surroundings.wind_m_s * (1.0 - report.immersion as f32),
            ..*surroundings
        };
        let activity = self.activity(cfg, &report);
        self.body.step(cfg, dt, &exposure, worn, &activity);
        report
    }

    /// What the body is doing, from how it moved.
    pub fn activity(&self, cfg: &BodyConfig, r: &Report) -> Activity {
        if self.asleep {
            return cfg.activity("sleeping");
        }
        let name = match r.motion {
            Motion::Walking | Motion::Crouching => "walking",
            Motion::Wading => "foraging",
            Motion::Jogging => "jogging",
            Motion::Sprinting => "sprinting",
            Motion::Crawling | Motion::Climbing => "climbing",
            Motion::Swimming if r.speed > 0.2 => "swimming",
            Motion::Swimming => "treading_water",
            Motion::Falling | Motion::Still => match self.mover.stance {
                Stance::Crawling => "resting",
                _ => "standing",
            },
        };
        let mut a = cfg.activity(name);
        if r.straining {
            a.exertion = a.exertion.max(0.85);
            a.met = a.met.max(cfg.met("climbing"));
        }
        a.speed_m_s = r.speed as f32;
        a.posture = match self.mover.stance {
            Stance::Crawling | Stance::Swimming => Posture::Lying,
            Stance::Crouching => Posture::Sitting,
            _ => Posture::Standing,
        };
        a
    }
}

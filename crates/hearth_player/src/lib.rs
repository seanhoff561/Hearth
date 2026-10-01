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

/// What a life has been: when it began, the ground it covered and the farthest it went from
/// where it began (for the world's last words under permadeath).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Life {
    pub born_tick: u64,
    pub start: DVec3,
    pub walked_m: f64,
    pub farthest_m: f64,
}

impl Life {
    pub fn begin(at: DVec3, tick: u64) -> Self {
        Self {
            born_tick: tick,
            start: at,
            walked_m: 0.0,
            farthest_m: 0.0,
        }
    }

    /// The player moved from `from` to `to`; jumps of more than 10 m (being put somewhere)
    /// are not walking.
    pub fn moved(&mut self, from: DVec3, to: DVec3) {
        let step = (to - from).truncate_y();
        if step < 10.0 {
            self.walked_m += step;
        }
        self.farthest_m = self.farthest_m.max((to - self.start).truncate_y());
    }
}

/// Horizontal length.
trait Flat {
    fn truncate_y(self) -> f64;
}

impl Flat for DVec3 {
    fn truncate_y(self) -> f64 {
        (self.x * self.x + self.z * self.z).sqrt()
    }
}

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
    #[serde(default)]
    pub life: Life,
    /// What the player carries and wears.
    #[serde(default)]
    pub carry: hearth_items::Carry,
    /// What the player knows: discoveries, insight, skills, the journal.
    #[serde(default)]
    pub knowledge: hearth_craft::KnowledgeState,
}

impl Player {
    pub fn new(cfg: &BodyConfig, feet: DVec3, seed: u64) -> Self {
        Self {
            body: Body::new(cfg, seed),
            mover: Mover::new(feet),
            asleep: false,
            lying: false,
            drowsy_s: 0.0,
            life: Life::begin(feet, 0),
            carry: hearth_items::Carry::default(),
            knowledge: hearth_craft::KnowledgeState::default(),
        }
    }

    /// What the body allows the mover under a load (`load`; dragging over ground of friction
    /// `mu`): slower with weight, no running or sprinting under a heavy one, the pace of a
    /// drag, no climbing with the hands full.
    pub fn ability_with(&self, cfg: &BodyConfig, load: &hearth_items::Load, mu: f32) -> Ability {
        let mut a = self.ability(cfg);
        let f = load.walk() as f64;
        a.walk_m_s *= f;
        a.jog_m_s = if load.can_jog() {
            a.jog_m_s * f
        } else {
            a.walk_m_s
        };
        if !load.can_sprint() {
            a.sprint = false;
            a.sprint_m_s = a.jog_m_s;
        }
        if let Some(v) = load.drag_speed(mu) {
            let v = (v as f64).min(a.walk_m_s);
            a.walk_m_s = v;
            a.jog_m_s = v;
            a.sprint_m_s = v;
            a.jump_m = 0.0;
        }
        a.jump_m *= (1.0 - load.share as f64).clamp(0.2, 1.0);
        a.climb &= self.carry.hands_free();
        a
    }

    /// Its activity with the work of a load (Pandolf's equation; a drag over ground of
    /// friction `mu`).
    pub fn activity_with(
        &self,
        cfg: &BodyConfig,
        r: &Report,
        load: &hearth_items::Load,
        mu: f32,
    ) -> Activity {
        let mut a = self.activity(cfg, r);
        let body_kg = cfg.mass_kg as f32;
        let extra_w = load.work_w(body_kg, r.speed as f32, 0.0, mu);
        // One MET for this body (W): 1.163 W per kilogram.
        let met_w = 1.163 * body_kg;
        a.met += extra_w / met_w;
        a.exertion = a.exertion.max((extra_w / (4.0 * met_w)).min(1.0));
        a
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

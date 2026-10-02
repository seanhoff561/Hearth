//! Minds (V2-7 (f), v2 §7.3): what an animal senses of a person, and how its suspicion grows
//! and fades.
//!
//! It **sees** by its eyes' reach (its species' sight in daylight, its night vision in the
//! dark), within the field it watches (a prey animal's eyes on the sides of its head see all
//! about but behind it; a hunter's look ahead), as plain as the person stands — moving or
//! still, upright or crouched, in the open or among plants — past whatever stands between
//! (a trunk or the ground hides; foliage and tall plants half hide). It **hears** the noise of
//! the person's going (a sprint on dry leaves far off, a crouched step on moss hardly at all).
//! It **smells** them where the wind carries their scent to it: downwind far, upwind not at
//! all, all about but near in a calm. Very close, it is startled whatever it sensed. Each sense
//! raises its suspicion at a rate that grows as the person comes nearer within the sense's
//! reach; with nothing sensed the suspicion fades. A little suspicion makes it look up and
//! watch; full suspicion makes it aware of the person, and an aware animal runs when they come
//! within its flight distance (or, if it is one that hides, freezes until they come too close),
//! warning its herd, which runs with it.

use glam::{DVec2, DVec3, Vec3};

use crate::live::{Cell, Ground};
use crate::species::Species;

/// A person as the animals may sense them this moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Presence {
    /// Where their feet are.
    pub pos: DVec3,
    /// How loud their going is (0 silent … 1 a run over dry leaves).
    pub noise: f32,
    /// How plain to see (0 hidden … 1 upright in the open, moving).
    pub plain: f32,
    /// How high their body stands (m): upright, crouched, crawling.
    pub height: f32,
}

impl Presence {
    /// Someone walking upright in the open.
    pub fn walking(pos: DVec3) -> Self {
        Self {
            pos,
            noise: 0.55,
            plain: 1.0,
            height: 1.7,
        }
    }

    /// Someone stalking: crouched, slow and quiet.
    pub fn stalking(pos: DVec3) -> Self {
        Self {
            pos,
            noise: 0.2,
            plain: 0.4,
            height: 1.0,
        }
    }

    /// Someone running.
    pub fn running(pos: DVec3) -> Self {
        Self {
            pos,
            noise: 1.0,
            plain: 1.0,
            height: 1.7,
        }
    }
}

/// The air about the animals: the way the wind blows (a unit vector on the ground, x and z),
/// its speed, and how light it is to see by (0 a dark night … 1 day).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Air {
    pub wind: DVec2,
    pub wind_speed: f32,
    pub light: f32,
}

impl Air {
    /// A still day.
    pub fn calm_day() -> Self {
        Self {
            wind: DVec2::X,
            wind_speed: 0.0,
            light: 1.0,
        }
    }
}

/// How an animal came to know of a threat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sense {
    Sight,
    Hearing,
    Scent,
    /// Warned by its herd.
    Alarm,
    /// Startled close by.
    Startle,
}

/// An animal's wariness of a threat: its suspicion (0 none … 1 aware), where it believes the
/// threat is, how it came to know.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Wary {
    pub suspicion: f32,
    pub threat: Option<DVec3>,
    pub how: Option<Sense>,
}

/// Suspicion a little of which makes an animal watch; full, aware.
pub const WATCH: f32 = 0.3;
pub const AWARE: f32 = 1.0;
/// How fast suspicion fades with nothing sensed (a second's worth).
const FADE: f32 = 0.08;

/// Whether a species hunts (a hunter's eyes look ahead).
fn hunter(sp: &Species) -> bool {
    sp.hunts()
}

/// The rate (a second) at which an animal's suspicion of a person grows, and by which sense,
/// for an animal at `at` facing `yaw` (0 toward +z).
pub fn sense(
    sp: &Species,
    at: DVec3,
    yaw: f32,
    p: &Presence,
    air: &Air,
    ground: &dyn Ground,
) -> Option<(f32, Sense)> {
    let d = (p.pos - at).length().max(0.01);
    let flat = DVec2::new(p.pos.x - at.x, p.pos.z - at.z);
    let mut best: Option<(f32, Sense)> = None;
    let mut take = |rate: f32, s: Sense| {
        if rate > 0.0 && best.is_none_or(|b| rate > b.0) {
            best = Some((rate, s));
        }
    };
    // Startled close by, whatever it sensed.
    let startle = (sp.flight_m() * 0.15).max(2.0) as f64;
    if d < startle && p.noise + p.plain > 0.2 {
        take(10.0, Sense::Startle);
    }
    // Sight: within its field, its reach in this light, as plain as the person stands past what
    // stands between.
    let half_field = if hunter(sp) { 100f32 } else { 150f32 }.to_radians();
    let facing = DVec2::new(yaw.sin() as f64, yaw.cos() as f64);
    let off = flat.normalize_or_zero().dot(facing).clamp(-1.0, 1.0).acos() as f32;
    if off <= half_field && sp.senses.vision_m > 0.0 {
        let light = air.light + (1.0 - air.light) * sp.senses.night_vision * 0.6;
        let eye = at + DVec3::Y * (sp.shoulder_m as f64 * 1.1 + 0.1);
        let body = p.pos + DVec3::Y * (p.height as f64 * 0.7);
        let seen = p.plain * (1.0 - cover(ground, eye, body));
        let reach = (sp.senses.vision_m * light * seen) as f64;
        if d < reach {
            take(2.0 * (1.0 - d / reach) as f32 + 0.1, Sense::Sight);
        }
    }
    // Hearing: the noise of their going (twice as quiet is a quarter as far).
    let reach = (sp.senses.hearing_m * p.noise * p.noise) as f64;
    if d < reach {
        take(1.5 * (1.0 - d / reach) as f32, Sense::Hearing);
    }
    // Scent: carried downwind (far as the wind is brisk), all about but near in a calm.
    let reach = sp.senses.smell_m as f64;
    if reach > 0.0 {
        let toward = (-flat).normalize_or_zero();
        let downwind = toward.dot(air.wind);
        let range = if air.wind_speed < 0.5 {
            reach * 0.15
        } else if downwind > 0.8 {
            reach * (0.3 + (air.wind_speed as f64 / 3.0).min(1.0) * 0.7)
        } else {
            0.0
        };
        if d < range {
            take(3.0 * (1.0 - d / range) as f32, Sense::Scent);
        }
    }
    best
}

/// How much of a body is hidden from an eye by what stands between (0 none … 1 all): the ground
/// and trunks hide, foliage and tall plants half hide, every half metre along the way.
pub fn cover(ground: &dyn Ground, eye: DVec3, body: DVec3) -> f32 {
    let d = body - eye;
    let n = (d.length() / 0.5).ceil().max(1.0) as usize;
    let mut hidden = 0.0f32;
    // Not the cells at the eye or the body themselves (what the animal and the person stand
    // in).
    for i in 1..n {
        let t = i as f64 / n as f64;
        let q = eye + d * t;
        match ground.cell(q.x.floor() as i32, q.y.floor() as i32, q.z.floor() as i32) {
            Some(Cell::Solid | Cell::Trunk) => return 1.0,
            Some(Cell::Leaves) => hidden += 0.35,
            Some(Cell::Plant | Cell::Limb) => hidden += 0.18,
            _ => {}
        }
        if hidden >= 1.0 {
            return 1.0;
        }
    }
    hidden.min(1.0)
}

impl Wary {
    /// A moment's sensing: suspicion up by what was sensed (and the threat where the person
    /// is), down when nothing was.
    pub fn update(&mut self, sensed: Option<(f32, Sense)>, threat: DVec3, dt: f32) {
        match sensed {
            Some((rate, how)) => {
                self.suspicion = (self.suspicion + rate * dt).min(1.5);
                self.threat = Some(threat);
                if self.how.is_none() || self.suspicion < AWARE {
                    self.how = Some(how);
                }
            }
            None => {
                self.suspicion = (self.suspicion - FADE * dt).max(0.0);
                if self.suspicion <= 0.0 {
                    self.threat = None;
                    self.how = None;
                }
            }
        }
    }

    /// Warned by a herd mate: aware of the threat where the mate believed it.
    pub fn alarm(&mut self, threat: DVec3) {
        self.suspicion = self.suspicion.max(AWARE + 0.2);
        self.threat = Some(threat);
        self.how = Some(Sense::Alarm);
    }

    pub fn aware(&self) -> bool {
        self.suspicion >= AWARE
    }

    pub fn watching(&self) -> bool {
        self.suspicion >= WATCH
    }
}

/// The yaw (0 toward +z) toward a point from a place.
pub fn yaw_toward(from: DVec3, to: DVec3) -> f32 {
    let d = Vec3::new((to.x - from.x) as f32, 0.0, (to.z - from.z) as f32);
    d.x.atan2(d.z)
}

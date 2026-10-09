//! Birds over remains (Amendment T §2.1): the scavengers of a place (ravens and crows in the
//! temperate and boreal lands, vultures in the savannas, condors in the Andes) circle over fresh
//! remains by day, calling, come down to feed a few at a time, rise higher when a person or a
//! hunter comes near, and go when the remains are gone or the day ends. Each bird circles on a
//! ring of its own about the remains, where [`Flock::bird_at`] puts it at a time of the world's
//! clock, so the server and the client place a flock alike: the server's near the player as
//! animals of the world (`Animal::attend`), the client's farther off as the specks they are.

use glam::{DVec2, DVec3};
use hearth_math::hash::{mix64, unit_f64 as unit};
use serde::{Deserialize, Serialize};

use hearth_content::schema::fauna::BodyPlan;

use crate::live::{Act, Animal, Medium};
use crate::species::Species;

/// Seconds a bird circles between its turns at the remains, and feeds on the ground in a turn.
const CIRCLE_S: (f64, f64) = (15.0, 50.0);
const FEED_S: (f64, f64) = (20.0, 60.0);
/// Seconds a bird leaving takes to be gone.
pub const LEAVE_S: f64 = 40.0;

/// A flock over remains.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Flock {
    /// The remains it is over ([`Flock::key_of`]).
    pub key: u64,
    pub species: u16,
    /// The remains: x, the ground's height there, z.
    pub at: [f64; 3],
    pub count: u8,
    /// Someone is near the remains: the birds keep off them, higher and wider.
    pub risen: bool,
}

/// What a bird attending remains is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Doing {
    /// On its ring over the remains.
    Circling,
    /// Gliding down to them.
    Landing,
    /// On the ground at them, feeding.
    Feeding,
    /// Back up to its ring.
    Rising,
    /// Off and away.
    Leaving,
}

/// A bird attending remains: its flock, its place in it, what it is doing and for how long yet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Attend {
    pub flock: Flock,
    pub index: u8,
    pub doing: Doing,
    pub left: f64,
}

/// Whether a species is one of the birds that live off carrion and gather over it, circling: the
/// crows and ravens, the vultures and condors (the jays that come to remains come through the
/// trees, a few at a time, and are not among them).
pub fn scavenges(sp: &Species) -> bool {
    matches!(sp.plan, BodyPlan::BirdPerching | BodyPlan::Raptor)
        && sp.carrion >= 0.5
        && sp.mass_kg >= 0.25
        && sp.fly_m_s.is_some()
}

/// How many of a scavenger come to remains of `kg`: a few ravens or crows to a deer, a dozen and
/// more vultures to an antelope (as many as the place has, at most `there`).
pub fn flock_size(sp: &Species, kg: f32, key: u64, there: u32) -> u8 {
    let jitter = unit(mix64(key ^ 0x51ed)) as f32;
    let n = if sp.mass_kg > 3.0 {
        3.0 + kg / 18.0 + 3.0 * jitter
    } else {
        2.0 + kg / 70.0 + 2.0 * jitter
    };
    let most = if sp.mass_kg > 3.0 { 14.0 } else { 8.0 };
    (n.clamp(1.0, most) as u32).min(there.max(1)) as u8
}

/// How many of a flock feed at once: each in its turn, about half of them.
pub fn feeders(count: u8) -> usize {
    (count as usize).div_ceil(2).max(1)
}

impl Flock {
    /// The key of remains: where they lie and when the animal died.
    pub fn key_of(at: [f64; 2], time: f64) -> u64 {
        mix64(at[0].to_bits() ^ mix64(at[1].to_bits() ^ mix64(time.to_bits())))
    }

    /// Bird `i`'s ring: radius and height over the remains (m), turning (radians a second) and
    /// phase, for a bird that flies at `cruise` m/s and weighs `kg`. The big birds soar higher
    /// and wider; all of a flock turn one way, as in one thermal.
    fn ring(&self, i: u8, cruise: f64, kg: f64) -> (f64, f64, f64, f64) {
        let h = mix64(self.key ^ (i as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let big = (kg / 1.2).clamp(1.0, 6.0).sqrt();
        let mut radius = (14.0 + 30.0 * unit(h)) * big;
        let mut height = (18.0 + 36.0 * unit(mix64(h))) * big;
        if self.risen {
            radius *= 1.7;
            height += 30.0 * big;
        }
        let way = if self.key & 1 == 0 { 1.0 } else { -1.0 };
        // Soaring, slower than its flapping cruise.
        let turning = way * cruise * 0.7 / radius;
        let phase = unit(mix64(h ^ 0xa5a5_5a5a)) * std::f64::consts::TAU;
        (radius, height, turning, phase)
    }

    /// Where bird `i` is on its ring at `t` (the world's clock, seconds), and its heading
    /// (radians, 0 toward +z, turning toward +x): the ring drifting a little with the air and the
    /// bird rising and sinking in it.
    pub fn bird_at(&self, i: u8, t: f64, cruise: f64, kg: f64) -> (DVec3, f32) {
        let (r, h, w, ph) = self.ring(i, cruise, kg);
        let a = ph + w * t;
        let drift = DVec2::new((t * 0.013 + ph).sin(), (t * 0.011 + ph * 1.7).cos()) * r * 0.25;
        let pos = DVec3::new(
            self.at[0] + drift.x + r * a.sin(),
            self.at[1] + h + 3.0 * (t * 0.21 + ph).sin(),
            self.at[2] + drift.y + r * a.cos(),
        );
        let (vx, vz) = (r * w * a.cos(), -r * w * a.sin());
        (pos, vx.atan2(vz) as f32)
    }

    /// Where bird `i` comes down to feed: about the remains, each in a place of its own.
    fn feed_spot(&self, i: u8) -> DVec3 {
        let h = mix64(self.key ^ 0xfeed ^ i as u64);
        let a = unit(h) * std::f64::consts::TAU;
        let d = 0.6 + 1.6 * unit(mix64(h));
        DVec3::new(
            self.at[0] + d * a.sin(),
            self.at[1],
            self.at[2] + d * a.cos(),
        )
    }

    /// Where bird `i` leaves for: far off over the land, higher.
    fn away(&self, i: u8) -> DVec3 {
        let a = unit(mix64(self.key ^ 0x90ae ^ i as u64)) * std::f64::consts::TAU;
        DVec3::new(
            self.at[0] + 600.0 * a.sin(),
            self.at[1] + 90.0,
            self.at[2] + 600.0 * a.cos(),
        )
    }
}

/// Moves a bird toward a point at a speed, facing the way it goes; whether it got there.
fn toward(a: &mut Animal, to: DVec3, speed: f64, dt: f64) -> bool {
    let d = to - a.pos;
    let len = d.length();
    let step = speed * dt;
    if len <= step.max(0.05) {
        a.pos = to;
        return true;
    }
    a.pos += d / len * step;
    if d.x.abs() + d.z.abs() > 1e-6 {
        a.yaw = d.x.atan2(d.z) as f32;
    }
    false
}

/// A spread of seconds between `lo` and `hi`, its own for each bird and turn.
fn spread(a: &Animal, t: f64, (lo, hi): (f64, f64)) -> f64 {
    lo + (hi - lo) * unit(mix64(a.id ^ t.to_bits()))
}

/// One step of `dt` seconds of a bird attending remains, at `t` (the world's clock, seconds),
/// `down` of its flock on their way down to them or at them now (counting this bird in when it
/// starts down). Whether it has gone (left for long enough to be let go).
pub fn step(a: &mut Animal, sp: &Species, t: f64, down: &mut usize, dt: f64) -> bool {
    let Some(mut at) = a.attend else {
        return false;
    };
    let cruise = sp.fly_m_s.unwrap_or(10.0) as f64;
    let kg = sp.mass_kg as f64;
    let f = at.flock;
    at.left -= dt;
    // On the wing at a speed, its wings beating `beats` of the time.
    let flying = |a: &mut Animal, speed: f64, beats: f64| {
        a.medium = Medium::Air;
        a.act = Act::Fly;
        a.speed = speed as f32;
        a.stride += (dt * speed / 3.0 * beats) as f32;
    };
    let mut gone = false;
    match at.doing {
        Doing::Circling => {
            let (p, yaw) = f.bird_at(at.index, t, cruise, kg);
            a.pos = p;
            a.yaw = yaw;
            // Soaring: the wings beat seldom.
            flying(a, cruise * 0.7, 0.2);
            if at.left <= 0.0 {
                if !f.risen && *down < feeders(f.count) {
                    at.doing = Doing::Landing;
                    *down += 1;
                } else {
                    at.left = spread(a, t, CIRCLE_S) * 0.5;
                }
            }
        }
        Doing::Landing => {
            if f.risen {
                at.doing = Doing::Rising;
            } else if toward(a, f.feed_spot(at.index), cruise * 0.6, dt) {
                at.doing = Doing::Feeding;
                at.left = spread(a, t, FEED_S);
                a.medium = Medium::Ground;
                a.act = Act::Graze;
                a.speed = 0.0;
            } else {
                flying(a, cruise * 0.6, 0.5);
            }
        }
        Doing::Feeding => {
            a.medium = Medium::Ground;
            a.speed = 0.0;
            // Head down to tear at it, up now and then to look about.
            a.act = if (t * 0.4 + a.id as f64 * 0.37).sin() > 0.75 {
                Act::Alert
            } else {
                Act::Graze
            };
            if f.risen || at.left <= 0.0 {
                at.doing = Doing::Rising;
            }
        }
        Doing::Rising => {
            let (to, _) = f.bird_at(at.index, t, cruise, kg);
            if toward(a, to, cruise, dt) {
                at.doing = Doing::Circling;
                at.left = spread(a, t, CIRCLE_S);
            } else {
                flying(a, cruise, 1.0);
            }
        }
        Doing::Leaving => {
            toward(a, f.away(at.index), cruise, dt);
            flying(a, cruise, 1.0);
            gone = at.left <= 0.0;
        }
    }
    a.attend = Some(at);
    gone
}

/// A bird of a flock coming to attend remains: on its ring, circling for a while first.
pub fn attending(flock: Flock, index: u8, sp: &Species, t: f64) -> (Attend, DVec3, f32) {
    let cruise = sp.fly_m_s.unwrap_or(10.0) as f64;
    let (pos, yaw) = flock.bird_at(index, t, cruise, sp.mass_kg as f64);
    let first = CIRCLE_S.0 + (CIRCLE_S.1 - CIRCLE_S.0) * unit(mix64(flock.key ^ index as u64));
    (
        Attend {
            flock,
            index,
            doing: Doing::Circling,
            left: first,
        },
        pos,
        yaw,
    )
}

/// Sends a bird off: the remains are gone, or the day is.
pub fn leave(a: &mut Animal) {
    if let Some(at) = a.attend.as_mut()
        && at.doing != Doing::Leaving
    {
        at.doing = Doing::Leaving;
        at.left = LEAVE_S;
    }
}

//! Wounds (V2-7 (h), docs/design/fauna.md "Hunting and wounds"): where a thrown or thrust weapon
//! strikes a body, and what it does there.
//!
//! A body is its torso (a box of the rig's width, depth and length about the height of its
//! middle), its neck and head ahead of and above the chest, and its legs below. A blow carries
//! the weapon's energy and its sharpness (`piercing`: a stone-tipped spear 0.85, a wooden one
//! 0.4, a thrown stone none). A sharp one goes in as deep as its energy drives it (a
//! stone-tipped spear thrown hard about a third of a metre) and reaches the vitals if it goes
//! deeper than the hide and muscle over them (three tenths of the body's width: a hare's a few
//! centimetres, a red deer's a dozen, an aurochs' more than twenty). By where it strikes:
//! - the heart and lungs (the front of the torso), reached: it bleeds out within a minute,
//!   running a hundred metres or so first; the neck reached: sooner;
//! - the belly reached: it bleeds slowly, runs, lies up, and dies within the hour;
//! - the haunch: now and then a great vessel cut (bleeding out); otherwise lame and bleeding;
//! - a leg: lame;
//! - the head: pierced, killed; a heavy blow stuns.
//!
//! A blunt blow (a stone, a club) kills a small animal struck hard on the head or body and only
//! bruises a large one. Flesh wounds bleed less as they clot. An animal that has lost two
//! fifths of its blood falls dead; before that it goes the slower the more it has lost.

use glam::{DVec3, Vec3};

use crate::rig::{Frame, Rig};

/// The blood lost (a share of all of it) at which an animal falls dead.
pub const LETHAL: f32 = 0.4;

/// Where a blow strikes a body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Head,
    Neck,
    /// The front of the torso: the heart and lungs.
    Chest,
    /// The middle: the liver and the gut.
    Belly,
    /// The hind quarters.
    Haunch,
    Leg,
}

impl Part {
    pub fn words(self) -> &'static str {
        match self {
            Part::Head => "in the head",
            Part::Neck => "in the neck",
            Part::Chest => "behind the shoulder",
            Part::Belly => "in the belly",
            Part::Haunch => "in the haunch",
            Part::Leg => "in the leg",
        }
    }
}

/// A blow: the energy it lands with (J) and how sharp it is (0 blunt … 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blow {
    pub energy_j: f32,
    pub piercing: f32,
}

/// What a body suffers of its wounds: how fast it bleeds (a share of its blood a second) where
/// the bleeding goes on and where it slows as it clots, how much blood it has lost, how lame it
/// is (0 sound … 1 unable to go), how long it lies stunned (s), and how long since a person
/// hurt it (s).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Hurt {
    pub bleeding: f32,
    pub clotting: f32,
    pub lost: f32,
    pub lame: f32,
    pub stunned: f32,
    pub since: Option<f32>,
}

impl Hurt {
    /// A moment's bleeding: whether it is dead of it.
    pub fn bleed(&mut self, dt: f32) -> bool {
        self.lost = (self.lost + (self.bleeding + self.clotting) * dt).min(1.0);
        // Flesh wounds clot over a minute or two.
        self.clotting *= (-dt / 90.0).exp();
        self.stunned = (self.stunned - dt).max(0.0);
        if let Some(s) = self.since.as_mut() {
            *s += dt;
        }
        self.lost >= LETHAL
    }

    /// How much of its strength and speed it has left.
    pub fn vigour(&self) -> f32 {
        let weak = (self.lost / LETHAL).clamp(0.0, 1.0);
        ((1.0 - 0.9 * weak.powf(1.5)) * (1.0 - self.lame.clamp(0.0, 0.9))).max(0.1)
    }

    /// Hurt by a person within `secs`.
    pub fn lately(&self, secs: f32) -> bool {
        self.since.is_some_and(|s| s < secs)
    }

    pub fn wounded(&self) -> bool {
        self.bleeding > 0.0 || self.lost > 0.02 || self.lame > 0.0
    }
}

/// What one blow did: where, whether it went deep, what it adds to the body's hurt, and
/// whether it killed outright.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wound {
    pub part: Part,
    pub deep: bool,
    pub bleeding: f32,
    pub clotting: f32,
    pub lame: f32,
    pub stunned: f32,
    pub killed: bool,
}

impl Wound {
    pub fn add_to(&self, h: &mut Hurt) {
        h.bleeding += self.bleeding;
        h.clotting += self.clotting;
        h.lame = (h.lame + self.lame).min(0.9);
        h.stunned = h.stunned.max(self.stunned);
        h.since = Some(0.0);
        if self.killed {
            h.lost = LETHAL;
        }
    }
}

/// The torso's half sizes and middle, the head's middle and half size, in the body's frame at
/// its size.
struct Shape {
    torso_mid: Vec3,
    torso_half: Vec3,
    head_mid: Vec3,
    head_half: Vec3,
    neck_from: Vec3,
    neck_r: f32,
}

fn shape(rig: &Rig, scale: f32) -> Shape {
    let t = rig.torso * scale;
    let pitch = rig.neck_pitch;
    let dir = Vec3::new(0.0, pitch.sin(), pitch.cos());
    let neck_from = rig.neck_base * scale;
    let head_mid = neck_from + dir * rig.neck_len * scale + Vec3::Z * rig.head_len * scale * 0.4;
    let hw = rig.head_w.max(0.01) * scale;
    Shape {
        torso_mid: Vec3::new(0.0, rig.torso_y * scale, 0.0),
        torso_half: (t * 0.5).max(Vec3::splat(0.01)),
        head_mid,
        head_half: Vec3::new(hw * 0.5, hw * 0.6, rig.head_len.max(0.02) * scale * 0.5),
        neck_from,
        neck_r: hw * 0.45,
    }
}

/// Where a segment from `a` to `b` (the body's frame) first enters a box: the fraction along it.
fn enters(a: Vec3, b: Vec3, mid: Vec3, half: Vec3) -> Option<f32> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for k in 0..3 {
        let lo = mid[k] - half[k];
        let hi = mid[k] + half[k];
        if d[k].abs() < 1e-9 {
            if a[k] < lo || a[k] > hi {
                return None;
            }
            continue;
        }
        let (mut u, mut v) = ((lo - a[k]) / d[k], (hi - a[k]) / d[k]);
        if u > v {
            std::mem::swap(&mut u, &mut v);
        }
        t0 = t0.max(u);
        t1 = t1.min(v);
        if t0 > t1 {
            return None;
        }
    }
    Some(t0)
}

/// Where a segment of a path (world, `a` to `b`) first strikes a body standing at `pos` facing
/// `yaw` at `scale`: the fraction along the segment and the part.
pub fn strikes(
    rig: &Rig,
    scale: f32,
    pos: DVec3,
    yaw: f32,
    a: DVec3,
    b: DVec3,
) -> Option<(f32, Part)> {
    let s = shape(rig, scale.max(0.05));
    // Into the body's frame: about its feet, turned back by its facing.
    let rot = glam::Quat::from_rotation_y(-yaw);
    let la = rot * (a - pos).as_vec3();
    let lb = rot * (b - pos).as_vec3();
    // Quickly past what is nowhere near.
    let reach = (s.torso_half.length() + rig.len * scale) * 1.5;
    let mid = (la + lb) * 0.5;
    if (mid - s.torso_mid).length() > reach + (lb - la).length() * 0.5 {
        return None;
    }
    let mut best: Option<(f32, Part)> = None;
    let mut take = |t: Option<f32>, part: Part| {
        if let Some(t) = t
            && best.is_none_or(|(b, _)| t < b)
        {
            best = Some((t, part));
        }
    };
    let bird_or_small = matches!(
        rig.frame,
        Frame::Bird | Frame::Fish | Frame::Snake | Frame::Frog | Frame::Insect
    );
    // The torso, its part by where along it.
    if let Some(t) = enters(la, lb, s.torso_mid, s.torso_half) {
        let p = la + (lb - la) * t;
        let along = (p.z - s.torso_mid.z) / s.torso_half.z;
        let part = if bird_or_small || along > 0.25 {
            Part::Chest
        } else if along > -0.35 {
            Part::Belly
        } else {
            Part::Haunch
        };
        take(Some(t), part);
    }
    // The head, and the neck between it and the chest.
    take(enters(la, lb, s.head_mid, s.head_half), Part::Head);
    let neck_mid = (s.neck_from + s.head_mid) * 0.5;
    let neck_half = Vec3::new(
        s.neck_r,
        ((s.head_mid.y - s.neck_from.y).abs() * 0.5).max(s.neck_r),
        ((s.head_mid.z - s.neck_from.z).abs() * 0.5).max(s.neck_r),
    );
    if !bird_or_small {
        take(enters(la, lb, neck_mid, neck_half), Part::Neck);
    }
    // The legs under the torso.
    let under = s.torso_mid.y - s.torso_half.y;
    if under > 0.05 && rig.frame == Frame::Quadruped {
        let legs_mid = Vec3::new(0.0, under * 0.5, s.torso_mid.z);
        let legs_half = Vec3::new(s.torso_half.x * 0.8, under * 0.5, s.torso_half.z * 0.9);
        take(enters(la, lb, legs_mid, legs_half), Part::Leg);
    }
    best
}

/// The wound a blow makes in a body of `mass` kg (at `scale` of its rig), struck at `part`;
/// `roll` (0–1) for what is chance in it (a great vessel cut).
pub fn wound(rig: &Rig, scale: f32, mass: f32, part: Part, blow: &Blow, roll: f32) -> Wound {
    let mut w = Wound {
        part,
        deep: false,
        bleeding: 0.0,
        clotting: 0.0,
        lame: 0.0,
        stunned: 0.0,
        killed: false,
    };
    let e = blow.energy_j.max(0.0);
    let small = mass < 8.0;
    if blow.piercing < 0.15 {
        // Blunt: a small animal struck hard is killed or stunned; a large one bruised, a leg
        // perhaps lamed.
        let hard = e / (8.0 * mass.max(0.05).sqrt());
        match part {
            Part::Head | Part::Neck if hard > 1.0 => {
                if small || hard > 4.0 {
                    w.killed = true;
                } else {
                    w.stunned = 4.0 + 6.0 * hard.min(3.0);
                }
            }
            Part::Chest | Part::Belly | Part::Haunch if small && hard > 1.5 => w.killed = true,
            Part::Leg if hard > 1.0 => w.lame = 0.4,
            _ => w.clotting = 0.0005,
        }
        return w;
    }
    // Sharp: how deep it goes, against what lies over the vitals.
    let depth = 0.25 * blow.piercing * (e / 100.0).sqrt();
    let over = (rig.torso.x * scale * 0.3).max(0.01);
    w.deep = depth >= over;
    match (part, w.deep) {
        // The heart and lungs: it falls within half a minute; a great vessel of the neck, sooner.
        (Part::Chest, true) => w.bleeding = 0.02,
        (Part::Neck, true) => w.bleeding = 0.03,
        (Part::Head, true) => w.killed = true,
        // The gut and liver: slowly, within the quarter hour or so.
        (Part::Belly, true) => {
            w.bleeding = 0.0004;
            w.clotting = 0.0015;
        }
        (Part::Haunch, true) if roll < 0.3 => w.bleeding = 0.01,
        (Part::Haunch, true) => {
            w.clotting = 0.0025;
            w.lame = 0.35;
        }
        (Part::Leg, _) => {
            w.clotting = 0.0015;
            w.lame = if w.deep { 0.6 } else { 0.3 };
        }
        (Part::Head, false) => w.stunned = if small { 6.0 } else { 0.0 },
        // A flesh wound: it bleeds a while (never the two fifths) and smarts.
        _ => w.clotting = 0.001 + 0.0015 * (depth / over).min(1.0),
    }
    // A small body has little to stop a point at all.
    if small && w.deep && matches!(part, Part::Chest | Part::Neck | Part::Belly) {
        w.killed = true;
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hurt_animal_bleeds_and_weakens() {
        let mut h = Hurt {
            bleeding: 0.02,
            ..Hurt::default()
        };
        let mut t = 0.0;
        while !h.bleed(0.1) {
            t += 0.1;
            assert!(t < 120.0);
        }
        assert!(
            (15.0..30.0).contains(&t),
            "{t} s to bleed out from the heart and lungs"
        );
        let mut f = Hurt {
            clotting: 0.0025,
            ..Hurt::default()
        };
        for _ in 0..6000 {
            assert!(!f.bleed(0.1), "a flesh wound clots");
        }
        assert!(f.vigour() < 1.0 && f.vigour() > 0.5);
    }
}

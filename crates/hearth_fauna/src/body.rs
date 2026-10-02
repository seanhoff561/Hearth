//! Bodies (V2-7): each species as boxes at its real dimensions — a torso sized from its mass
//! and length, legs to its shoulder height, a neck and head, ears, a tail, antlers on a stag in
//! their season and tusks on a boar — in its coat's colours (the back, the paler belly, the
//! points, the rump patch, the winter coat, the male's colour), posed by what it is doing (the
//! legs swinging with the gait, the head down to graze, lying at rest).
//!
//! The frame is the animal's: x to its left, y up, z forward; the origin under the middle of
//! its body on the ground.

use glam::{Affine3A, Quat, Vec3};
use hearth_content::schema::fauna::{BodyPlan, Coat, CoatPattern};

use crate::live::{Act, Stage};
use crate::species::Species;

/// A box of a posed body: placed in the animal's frame, and its colour (sRGB).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyBox {
    pub place: Affine3A,
    pub color: [u8; 3],
}

/// The colours of a coat at a time (sRGB).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Colors {
    pub back: [u8; 3],
    pub belly: [u8; 3],
    pub points: [u8; 3],
    pub rump: [u8; 3],
    pub marking: [u8; 3],
    pub pattern: CoatPattern,
}

impl Colors {
    /// A coat's colours for an animal: the male's back where it differs, the winter coat in
    /// winter, the young's pattern on the young.
    pub fn of(coat: Option<&Coat>, female: bool, stage: Stage, winter: bool) -> Self {
        let Some(c) = coat else {
            let grey = [120, 110, 100];
            return Self {
                back: grey,
                belly: [170, 160, 150],
                points: [60, 55, 50],
                rump: grey,
                marking: [60, 55, 50],
                pattern: CoatPattern::Plain,
            };
        };
        let mut back = c.base.0;
        if winter && let Some(w) = c.winter {
            back = w.0;
        }
        if !female
            && stage == Stage::Adult
            && let Some(m) = c.male
        {
            back = m.0;
        }
        let pattern = match (stage, c.young) {
            (Stage::Young, Some(p)) => p,
            _ => c.pattern,
        };
        Self {
            back,
            belly: c.belly.0,
            points: c.points.map_or(back, |p| p.0),
            rump: c.rump.map_or(back, |p| p.0),
            marking: c.marking.map_or([240, 236, 226], |p| p.0),
            pattern,
        }
    }
}

/// How an animal looks now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub stage: Stage,
    pub female: bool,
    pub act: Act,
    /// Strides taken (the gait's phase).
    pub stride: f32,
    /// Ground speed, m/s.
    pub speed: f32,
    /// The year fraction (0 at the March equinox; the north's seasons).
    pub year_frac: f32,
    /// South of the equator.
    pub southern: bool,
}

/// The size of an animal against a grown one.
pub fn scale_of(sp: &Species, stage: Stage, year_frac: f32) -> f32 {
    match stage {
        Stage::Adult => 1.0,
        Stage::Juvenile => 0.8,
        Stage::Young => {
            // Born at the birth date, half grown by its first autumn.
            let age = (year_frac - sp.life.birth_frac).rem_euclid(1.0);
            let birth = (sp.life.birth_mass_kg / sp.mass_kg).cbrt().clamp(0.15, 0.6);
            birth + (0.75 - birth) * (age / 0.6).min(1.0)
        }
    }
}

struct Builder {
    out: Vec<BodyBox>,
}

impl Builder {
    /// A box of `size` with its centre at `c`, turned by `q` about `pivot`.
    fn put(&mut self, c: Vec3, size: Vec3, color: [u8; 3], pivot: Vec3, q: Quat) {
        let local = Affine3A::from_scale_rotation_translation(size, Quat::IDENTITY, c - pivot);
        let place = Affine3A::from_rotation_translation(q, pivot) * local;
        self.out.push(BodyBox { place, color });
    }

    fn plain(&mut self, c: Vec3, size: Vec3, color: [u8; 3]) {
        self.put(c, size, color, Vec3::ZERO, Quat::IDENTITY);
    }
}

/// The boxes of an animal of a species, posed.
pub fn boxes(sp: &Species, look: &Look) -> Vec<BodyBox> {
    let s = scale_of(sp, look.stage, look.year_frac);
    let winter = !(0.05..0.6).contains(&frac_north(look));
    let col = Colors::of(sp.coat.as_ref(), look.female, look.stage, winter);
    let mut b = Builder { out: Vec::new() };
    match sp.plan {
        BodyPlan::BirdGround | BodyPlan::BirdPerching | BodyPlan::Raptor | BodyPlan::Waterfowl => {
            bird(&mut b, sp, look, s, &col)
        }
        BodyPlan::Snake | BodyPlan::Eel => snake(&mut b, sp, s, &col),
        BodyPlan::FishFusiform | BodyPlan::FishFlat => fish(&mut b, sp, s, &col),
        _ => quadruped(&mut b, sp, look, s, &col),
    }
    b.out
}

/// The swing of a leg (radians) at a gait phase: walking legs in a four-beat sequence, running
/// ones the fore and hind pairs together.
fn leg_swing(look: &Look, sp: &Species, leg: usize) -> f32 {
    if look.speed < 0.05 {
        return 0.0;
    }
    let running = look.speed > sp.walk_m_s * 1.6;
    let phase = if running {
        [0.0, 0.1, 0.5, 0.6][leg]
    } else {
        [0.0, 0.5, 0.75, 0.25][leg]
    };
    let amp = if running { 0.65 } else { 0.38 };
    amp * (std::f32::consts::TAU * (look.stride + phase)).sin()
}

fn quadruped(b: &mut Builder, sp: &Species, look: &Look, s: f32, col: &Colors) {
    let plan = sp.plan;
    let len = sp.length_m * s;
    let shoulder = sp.shoulder_m * s;
    let mass = sp.mass_kg * s.powi(3);
    // Proportions by body plan: the torso's share of the length and the height, the legs' and
    // neck's build, the head's length.
    let (torso_l, torso_h, neck_l, head_l, leg_w) = match plan {
        BodyPlan::Ungulate => (0.55, 0.42, 0.28, 0.24, 0.09),
        BodyPlan::Bear => (0.6, 0.55, 0.1, 0.22, 0.17),
        BodyPlan::Lagomorph => (0.62, 0.62, 0.06, 0.24, 0.12),
        BodyPlan::Rodent => (0.6, 0.6, 0.05, 0.25, 0.12),
        _ => (0.58, 0.48, 0.16, 0.24, 0.11),
    };
    let tl = len * torso_l;
    let th = shoulder * torso_h;
    // Width from the volume of the body (about the density of water, three quarters of the box).
    let volume = mass / 1000.0 / 0.75;
    let tw = (volume / (tl * th)).clamp(0.35 * th, 1.15 * th);
    // Lying, the legs fold under and the body rests on the ground.
    let lying = look.act == Act::Rest;
    let leg_len = (shoulder - th * 0.85).max(shoulder * 0.15);
    let lift = if lying { th * 0.5 } else { shoulder - th * 0.5 };
    let torso_c = Vec3::new(0.0, lift, 0.0);
    // The back and the paler belly beneath.
    b.plain(torso_c, Vec3::new(tw, th, tl), col.back);
    b.plain(
        torso_c - Vec3::Y * th * 0.42,
        Vec3::new(tw * 0.86, th * 0.2, tl * 0.84),
        col.belly,
    );
    // The rump patch (deer).
    if col.rump != col.back {
        b.plain(
            torso_c + Vec3::new(0.0, th * 0.05, -tl * 0.5),
            Vec3::new(tw * 0.7, th * 0.6, tl * 0.04),
            col.rump,
        );
    }
    pattern(b, torso_c, Vec3::new(tw, th, tl), col);
    // Legs: fore and hind pairs, swinging from the hip and shoulder; folded when lying.
    let lw = (tw * leg_w * 2.2).clamp(0.02, 0.4) * if plan == BodyPlan::Bear { 1.3 } else { 1.0 };
    let hips = [
        Vec3::new(tw * 0.32, lift - th * 0.3, tl * 0.38),
        Vec3::new(-tw * 0.32, lift - th * 0.3, tl * 0.38),
        Vec3::new(tw * 0.32, lift - th * 0.3, -tl * 0.38),
        Vec3::new(-tw * 0.32, lift - th * 0.3, -tl * 0.38),
    ];
    for (k, hip) in hips.into_iter().enumerate() {
        if lying {
            let c = Vec3::new(
                hip.x,
                lw * 0.5,
                hip.z + if k < 2 { tl * 0.1 } else { -tl * 0.05 },
            );
            b.plain(c, Vec3::new(lw, lw, leg_len * 0.45), col.points);
            continue;
        }
        let swing = leg_swing(look, sp, k);
        let q = Quat::from_rotation_x(swing);
        let upper = Vec3::new(hip.x, hip.y - leg_len * 0.3, hip.z);
        b.put(
            upper,
            Vec3::new(lw * 1.25, leg_len * 0.62, lw * 1.35),
            col.back,
            hip,
            q,
        );
        let lower = Vec3::new(hip.x, hip.y - leg_len * 0.78, hip.z);
        b.put(lower, Vec3::new(lw, leg_len * 0.46, lw), col.points, hip, q);
    }
    // The neck reaches up and forward from the front of the torso; down to graze.
    let grazing = look.act == Act::Graze && look.speed < 0.2;
    let alert = matches!(look.act, Act::Alert | Act::Flee);
    let neck_base = torso_c + Vec3::new(0.0, th * 0.2, tl * 0.45);
    let neck_angle: f32 = if grazing {
        1.25
    } else if alert {
        -0.95
    } else {
        -0.6
    };
    let neck_angle = if lying { -0.7 } else { neck_angle };
    let nl = len * neck_l;
    let neck_dir = Quat::from_rotation_x(neck_angle) * Vec3::Z;
    let neck_c = neck_base + neck_dir * nl * 0.5;
    let nw = tw * 0.45;
    b.put(
        neck_c,
        Vec3::new(nw, nw * 1.1, nl.max(nw)),
        col.back,
        neck_c,
        Quat::from_rotation_x(neck_angle),
    );
    // The head at the end of the neck: skull and muzzle.
    let head_at = neck_base + neck_dir * nl;
    let hl = len * head_l;
    let hq = Quat::from_rotation_x(if grazing { 1.0 } else { 0.25 });
    let skull = head_at + hq * Vec3::new(0.0, 0.0, hl * 0.25);
    let hw = (tw * 0.42).max(hl * 0.35);
    b.put(
        skull,
        Vec3::new(hw, hw * 0.95, hl * 0.55),
        col.back,
        head_at,
        hq,
    );
    let muzzle = head_at + hq * Vec3::new(0.0, -hw * 0.12, hl * 0.72);
    b.put(
        muzzle,
        Vec3::new(hw * 0.62, hw * 0.6, hl * 0.42),
        col.points,
        head_at,
        hq,
    );
    // Ears: long on a hare, tufted on a lynx, small on a bear.
    let ear = match plan {
        BodyPlan::Lagomorph => Vec3::new(hw * 0.25, hl * 0.9, hw * 0.12),
        BodyPlan::Bear => Vec3::new(hw * 0.22, hw * 0.25, hw * 0.1),
        _ => Vec3::new(hw * 0.22, hw * 0.45, hw * 0.1),
    };
    for side in [-1.0f32, 1.0] {
        let e = head_at + hq * Vec3::new(side * hw * 0.38, hw * 0.55 + ear.y * 0.4, hl * 0.12);
        b.put(e, ear, col.points, head_at, hq);
    }
    // Antlers on a grown male deer (cast in early spring and grown again by the rut); tusks
    // on a boar.
    let deer = sp.id.ends_with("_deer");
    let antlers = !(0.0..0.1).contains(&frac_north(look));
    if deer && !look.female && look.stage == Stage::Adult && antlers {
        let ah = shoulder * 0.5;
        for side in [-1.0f32, 1.0] {
            let base = head_at + hq * Vec3::new(side * hw * 0.3, hw * 0.5, hl * 0.05);
            let tilt = Quat::from_rotation_z(-side * 0.45) * Quat::from_rotation_x(-0.35);
            let beam = base + tilt * Vec3::new(0.0, ah * 0.5, 0.0);
            let color = [214, 196, 160];
            b.put(beam, Vec3::new(ah * 0.07, ah, ah * 0.07), color, base, tilt);
            for tine in 1..4 {
                let t = tine as f32 / 4.0;
                let at = base + tilt * Vec3::new(0.0, ah * t, 0.0);
                let tq = tilt * Quat::from_rotation_x(0.9);
                let c = at + tq * Vec3::new(0.0, ah * 0.15, 0.0);
                b.put(c, Vec3::new(ah * 0.05, ah * 0.3, ah * 0.05), color, at, tq);
            }
        }
    }
    if sp.id.ends_with("wild_boar") && !look.female && look.stage == Stage::Adult {
        for side in [-1.0f32, 1.0] {
            let t = head_at + hq * Vec3::new(side * hw * 0.3, -hw * 0.1, hl * 0.85);
            b.put(
                t,
                Vec3::new(hw * 0.08, hw * 0.25, hw * 0.08),
                [236, 228, 210],
                head_at,
                hq,
            );
        }
    }
    // The tail: long and bushy on a fox or wolf, a stub on a lynx or deer.
    let (tail_l, tail_w) = match sp.id.as_str() {
        id if id.ends_with("red_fox") => (len * 0.5, tw * 0.35),
        id if id.ends_with("gray_wolf") => (len * 0.35, tw * 0.3),
        id if id.ends_with("squirrel") => (len * 0.8, tw * 0.6),
        _ if plan == BodyPlan::Ungulate => (len * 0.1, tw * 0.25),
        _ if plan == BodyPlan::Bear => (len * 0.04, tw * 0.15),
        _ => (len * 0.12, tw * 0.25),
    };
    let tail_base = torso_c + Vec3::new(0.0, th * 0.3, -tl * 0.5);
    let sway = if look.speed > 0.2 {
        0.2 * (std::f32::consts::TAU * look.stride * 0.5).sin()
    } else {
        0.0
    };
    let tq = Quat::from_rotation_y(sway)
        * Quat::from_rotation_x(if plan == BodyPlan::Rodent { -1.2 } else { 0.6 });
    let tc = tail_base + tq * Vec3::new(0.0, 0.0, -tail_l * 0.5);
    let tail_col = if sp.id.ends_with("red_fox") {
        col.back
    } else {
        col.points
    };
    b.put(
        tc,
        Vec3::new(tail_w, tail_w, tail_l),
        tail_col,
        tail_base,
        tq,
    );
}

/// The year fraction as the animal's hemisphere has its seasons (0 at the spring equinox).
fn frac_north(look: &Look) -> f32 {
    if look.southern {
        (look.year_frac + 0.5).rem_euclid(1.0)
    } else {
        look.year_frac
    }
}

/// A coat's pattern over the torso: spots, stripes, the badger's mask on the head is left to
/// the head's colours.
fn pattern(b: &mut Builder, c: Vec3, size: Vec3, col: &Colors) {
    match col.pattern {
        CoatPattern::Spotted | CoatPattern::Speckled => {
            for k in 0..8 {
                let u = ((k as f32 * 0.618).fract() - 0.5) * 0.8;
                let v = ((k as f32 * 0.414).fract() - 0.5) * 0.5;
                for side in [-1.0f32, 1.0] {
                    let at = c + Vec3::new(side * size.x * 0.5, size.y * (0.1 + v), size.z * u);
                    b.plain(
                        at,
                        Vec3::new(0.012, size.y * 0.08, size.z * 0.06),
                        col.marking,
                    );
                }
            }
        }
        CoatPattern::Striped => {
            for k in 0..4 {
                let y = size.y * (0.1 + 0.1 * k as f32);
                for side in [-1.0f32, 1.0] {
                    let at = c + Vec3::new(side * size.x * 0.5, y, 0.0);
                    b.plain(
                        at,
                        Vec3::new(0.012, size.y * 0.04, size.z * 0.9),
                        col.marking,
                    );
                }
            }
        }
        CoatPattern::Banded | CoatPattern::ZigZag => {
            for k in 0..5 {
                let z = size.z * (k as f32 / 4.0 - 0.5) * 0.8;
                b.plain(
                    c + Vec3::new(0.0, size.y * 0.5, z),
                    Vec3::new(size.x * 0.6, 0.012, size.z * 0.08),
                    col.marking,
                );
            }
        }
        _ => {}
    }
}

fn bird(b: &mut Builder, sp: &Species, look: &Look, s: f32, col: &Colors) {
    let len = sp.length_m * s;
    let h = sp.shoulder_m * s;
    let bw = (len * 0.32).max(0.03);
    let leg = h * 0.45;
    let body_c = Vec3::new(0.0, leg + bw * 0.5, 0.0);
    b.plain(body_c, Vec3::new(bw, bw * 0.9, len * 0.5), col.back);
    b.plain(
        body_c - Vec3::Y * bw * 0.3,
        Vec3::new(bw * 0.85, bw * 0.35, len * 0.42),
        col.belly,
    );
    let grazing = look.act == Act::Graze && look.speed < 0.2;
    let head_c = body_c + Vec3::new(0.0, if grazing { -bw * 0.3 } else { bw * 0.7 }, len * 0.3);
    b.plain(head_c, Vec3::new(bw * 0.55, bw * 0.55, bw * 0.6), col.back);
    b.plain(
        head_c + Vec3::new(0.0, -bw * 0.05, bw * 0.4),
        Vec3::new(bw * 0.18, bw * 0.15, bw * 0.3),
        col.points,
    );
    b.plain(
        body_c + Vec3::new(0.0, bw * 0.15, -len * 0.35),
        Vec3::new(bw * 0.8, bw * 0.12, len * 0.3),
        col.back,
    );
    for (k, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let hip = Vec3::new(side * bw * 0.22, leg, 0.0);
        let q = Quat::from_rotation_x(leg_swing(look, sp, k * 2));
        b.put(
            hip - Vec3::Y * leg * 0.5,
            Vec3::new(bw * 0.08, leg, bw * 0.08),
            [120, 110, 90],
            hip,
            q,
        );
    }
}

fn snake(b: &mut Builder, sp: &Species, s: f32, col: &Colors) {
    let len = sp.length_m * s;
    let w = (len * 0.035).max(0.01);
    for k in 0..8 {
        let t = k as f32 / 7.0;
        let z = (t - 0.5) * len * 0.7;
        let x = (t * 9.0).sin() * len * 0.06;
        b.plain(
            Vec3::new(x, w * 0.5, z),
            Vec3::new(w, w, len * 0.11),
            col.back,
        );
    }
    let _ = col.marking;
}

fn fish(b: &mut Builder, sp: &Species, s: f32, col: &Colors) {
    let len = sp.length_m * s;
    let h = len * 0.25;
    b.plain(
        Vec3::new(0.0, h, 0.0),
        Vec3::new(h * 0.45, h, len * 0.8),
        col.back,
    );
    b.plain(
        Vec3::new(0.0, h, -len * 0.45),
        Vec3::new(h * 0.1, h * 0.8, len * 0.15),
        col.points,
    );
}

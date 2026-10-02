//! The bodies moving (V2-7, v2 §7.2): poses of a [`Rig`] from gait data and what the animal is
//! doing. Four-legged gaits by the Froude number of the speed against the leg (walking, trotting
//! and galloping; hares and squirrels bound, bears do not trot), the stride's length after
//! Alexander (λ = 2.3 h Fr^0.3), each foot down for its share of the stride and lifted through
//! the rest, set on the ground under it (the body pitched to the slope between the fore and
//! hind feet) and the leg bent to reach it (two segments, the foreleg's middle joint bending
//! forward and the hind's hock back). The head nods with the walk, looks at what it watches,
//! goes down to graze or drink; the jaw chews; the ears turn forward when alert, back in
//! flight, and flick; the tail sways with the gait and rises in alarm; the chest breathes.
//! And the poses of doing things: lying with the legs folded, asleep with the head turned back,
//! grooming the flank, rearing, attacking. Birds stand, hop or walk, peck, crouch and fly;
//! snakes wind along their path or lie coiled; fish swim with a wave down the body; frogs sit
//! and hop.
//!
//! The state a body keeps between frames is a [`Motion`]: the gait's phase and its weights
//! eased toward what the animal does, so that a deer lies down rather than snapping flat.

use glam::{Affine3A, Quat, Vec2, Vec3};
use hearth_content::schema::fauna::BodyPlan;

use crate::live::{Act, Medium, Stage};
use crate::rig::{Frame, Gear, MAX_SEGS, Rig, SLOTS, Slot};

const G: f32 = 9.81;
const TAU: f32 = std::f32::consts::TAU;
const PI: f32 = std::f32::consts::PI;

/// What a body is doing, as its pose needs it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drive {
    pub act: Act,
    /// Ground speed, m/s.
    pub speed: f32,
    /// A point it looks at, in its frame.
    pub look: Option<Vec3>,
    pub stage: Stage,
    pub female: bool,
    /// The year fraction (0 at the March equinox, the north's seasons) and the hemisphere.
    pub year_frac: f32,
    pub southern: bool,
    /// Its size against a grown one.
    pub scale: f32,
    /// On the ground, swimming, on the wing, up a tree.
    pub medium: Medium,
}

impl Drive {
    pub fn standing() -> Self {
        Self {
            act: Act::Walk,
            speed: 0.0,
            look: None,
            stage: Stage::Adult,
            female: false,
            year_frac: 0.4,
            southern: false,
            scale: 1.0,
            medium: Medium::Ground,
        }
    }
}

/// The ground as a smaller body's frame has it.
struct Scaled<'a> {
    inner: &'a dyn Footing,
    s: f32,
}

impl Footing for Scaled<'_> {
    fn ground(&self, at: Vec3) -> Option<f32> {
        self.inner.ground(at * self.s).map(|y| y / self.s)
    }
}

/// The ground under a body: its height (in the body's frame) at a point of the body's frame,
/// where it is known.
pub trait Footing {
    fn ground(&self, at: Vec3) -> Option<f32>;
}

/// Level ground at the body's feet.
pub struct Flat;

impl Footing for Flat {
    fn ground(&self, _at: Vec3) -> Option<f32> {
        Some(0.0)
    }
}

/// What a body keeps between frames.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion {
    /// Strides taken (the gait's phase).
    pub phase: f32,
    /// The gait, eased: 0 walking, 1 trotting, 2 galloping or bounding.
    pub gait: f32,
    /// Ground speed, eased.
    pub speed: f32,
    /// How far into each way of being, 0–1, eased.
    pub graze: f32,
    pub lie: f32,
    pub sleep: f32,
    pub alert: f32,
    pub flee: f32,
    pub groom: f32,
    pub drink: f32,
    pub rear: f32,
    pub attack: f32,
    pub fly: f32,
    pub swim: f32,
    pub climb: f32,
    /// Wingbeats.
    pub flap: f32,
    /// Where the head looks (yaw, pitch; radians from straight ahead), eased.
    pub look: Vec2,
    /// Seconds lived (breathing, chewing, flicks).
    pub time: f32,
    /// Its own variety: which side it grooms, when it flicks.
    pub seed: u32,
}

/// The targets of a motion's weights for an act.
struct Weights {
    graze: f32,
    lie: f32,
    sleep: f32,
    alert: f32,
    flee: f32,
    groom: f32,
    drink: f32,
    rear: f32,
    attack: f32,
    fly: f32,
    swim: f32,
    climb: f32,
}

fn weights(d: &Drive) -> Weights {
    let mut w = Weights {
        graze: 0.0,
        lie: 0.0,
        sleep: 0.0,
        alert: 0.0,
        flee: 0.0,
        groom: 0.0,
        drink: 0.0,
        rear: 0.0,
        attack: 0.0,
        fly: 0.0,
        swim: 0.0,
        climb: 0.0,
    };
    match d.medium {
        Medium::Water => w.swim = 1.0,
        Medium::Tree => w.climb = 1.0,
        Medium::Air => w.fly = 1.0,
        Medium::Ground => {}
    }
    let still = d.speed < 0.3;
    match d.act {
        Act::Graze if still => w.graze = 1.0,
        Act::Rest => w.lie = 1.0,
        Act::Sleep => {
            w.lie = 1.0;
            w.sleep = 1.0;
        }
        Act::Alert => w.alert = 1.0,
        Act::Flee => w.flee = 1.0,
        Act::Groom if still => w.groom = 1.0,
        Act::Drink if still => w.drink = 1.0,
        Act::Rear => w.rear = 1.0,
        Act::Attack => w.attack = 1.0,
        Act::Fly => w.fly = 1.0,
        _ => {}
    }
    w
}

impl Motion {
    pub fn new(id: u64) -> Self {
        let seed = (id ^ (id >> 29)).wrapping_mul(0x9e37_79b9_7f4a_7c15) as u32;
        Self {
            phase: (seed % 1000) as f32 / 1000.0,
            gait: 0.0,
            speed: 0.0,
            graze: 0.0,
            lie: 0.0,
            sleep: 0.0,
            alert: 0.0,
            flee: 0.0,
            groom: 0.0,
            drink: 0.0,
            rear: 0.0,
            attack: 0.0,
            fly: 0.0,
            swim: 0.0,
            climb: 0.0,
            flap: 0.0,
            look: Vec2::ZERO,
            time: (seed % 7919) as f32 * 0.01,
            seed,
        }
    }

    /// At once in the pose of `d` (a screenshot; an animal just come into view).
    pub fn settle(&mut self, rig: &Rig, d: &Drive) {
        let w = weights(d);
        self.speed = d.speed;
        self.gait = gait_of(rig, d.speed);
        self.graze = w.graze;
        self.lie = w.lie;
        self.sleep = w.sleep;
        self.alert = w.alert;
        self.flee = w.flee;
        self.groom = w.groom;
        self.drink = w.drink;
        self.rear = w.rear;
        self.attack = w.attack;
        self.fly = w.fly;
        self.swim = w.swim;
        self.climb = w.climb;
        self.look = look_angles(rig, d.look);
    }

    /// Eases toward `d` over `dt` seconds, the gait's phase advancing with the speed.
    pub fn update(&mut self, rig: &Rig, d: &Drive, dt: f32) {
        let dt = dt.clamp(0.0, 0.25);
        self.time += dt;
        let k = |rate: f32| 1.0 - (-rate * dt).exp();
        self.speed += (d.speed - self.speed) * k(6.0);
        self.gait += (gait_of(rig, self.speed) - self.gait) * k(5.0);
        let w = weights(d);
        let ease = |v: &mut f32, to: f32, rate: f32| *v += (to - *v) * k(rate);
        ease(&mut self.graze, w.graze, 3.0);
        ease(&mut self.lie, w.lie, 1.6);
        ease(&mut self.sleep, w.sleep, 1.0);
        ease(&mut self.alert, w.alert, 5.0);
        ease(&mut self.flee, w.flee, 6.0);
        ease(&mut self.groom, w.groom, 2.5);
        ease(&mut self.drink, w.drink, 2.5);
        ease(&mut self.rear, w.rear, 3.0);
        ease(&mut self.attack, w.attack, 8.0);
        ease(&mut self.fly, w.fly, 4.0);
        ease(&mut self.swim, w.swim, 4.0);
        ease(&mut self.climb, w.climb, 4.0);
        let look = look_angles(rig, d.look);
        self.look += (look - self.look) * k(4.0);
        self.phase = (self.phase + self.speed / stride_of(rig, self.speed) * dt).rem_euclid(1.0);
        self.flap = (self.flap + flap_rate(rig) * dt * self.fly.max(0.05)).rem_euclid(1.0);
    }
}

/// The yaw and pitch toward a point, within what a neck turns.
fn look_angles(rig: &Rig, at: Option<Vec3>) -> Vec2 {
    let Some(p) = at else {
        return Vec2::ZERO;
    };
    let from = rig.neck_base + Vec3::Y * rig.head_w;
    let d = p - from;
    let yaw = d.x.atan2(d.z).clamp(-1.6, 1.6);
    let pitch = d.y.atan2(Vec2::new(d.x, d.z).length()).clamp(-0.8, 0.8);
    Vec2::new(yaw, pitch)
}

/// The height of the hips over the ground at rest (the leg's reach).
fn hip_height(rig: &Rig) -> f32 {
    rig.legs
        .iter()
        .filter(|l| !l.fore)
        .map(|l| l.joint.y)
        .fold(0.0f32, f32::max)
        .max(rig.torso_y * 0.5)
        .max(0.005)
}

/// The gait for a speed: 0 walking, 1 trotting, 2 galloping (bounding for hares and small
/// rodents, which do not trot; a bear walks, then gallops).
fn gait_of(rig: &Rig, speed: f32) -> f32 {
    let fr = speed * speed / (G * hip_height(rig));
    match rig.plan {
        BodyPlan::Lagomorph | BodyPlan::Rodent => {
            if fr < 0.25 {
                0.0
            } else {
                2.0
            }
        }
        BodyPlan::Bear => {
            if fr < 0.6 {
                0.0
            } else {
                2.0
            }
        }
        _ => {
            if fr < 0.45 {
                0.0
            } else if fr < 2.2 {
                1.0
            } else {
                2.0
            }
        }
    }
}

/// A stride's length at a speed (Alexander: 2.3 h Fr^0.3), at least a third of the leg.
fn stride_of(rig: &Rig, speed: f32) -> f32 {
    let h = hip_height(rig);
    let fr = speed * speed / (G * h);
    (2.3 * h * fr.powf(0.3)).max(h * 0.35)
}

/// Wingbeats a second: fewer as birds are bigger.
fn flap_rate(rig: &Rig) -> f32 {
    match rig.frame {
        Frame::Insect => 37.0,
        _ => (3.2 * (rig.mass / 0.1).max(0.01).powf(-0.25)).clamp(2.0, 9.0),
    }
}

/// A gait: when each leg (left fore, right fore, left hind, right hind) sets down in the
/// stride, the share of the stride a foot is down, and how high it lifts (of the leg).
#[derive(Debug, Clone, Copy)]
struct Gait {
    offsets: [f32; 4],
    duty: f32,
    lift: f32,
}

const WALK: Gait = Gait {
    offsets: [0.25, 0.75, 0.0, 0.5],
    duty: 0.68,
    lift: 0.1,
};
const TROT: Gait = Gait {
    offsets: [0.0, 0.5, 0.5, 0.0],
    duty: 0.45,
    lift: 0.16,
};
const GALLOP: Gait = Gait {
    offsets: [0.55, 0.65, 0.0, 0.12],
    duty: 0.3,
    lift: 0.22,
};
const BOUND: Gait = Gait {
    offsets: [0.5, 0.56, 0.0, 0.05],
    duty: 0.32,
    lift: 0.25,
};

fn gait_table(rig: &Rig, g: f32) -> (Gait, Gait, f32) {
    let fast = match rig.plan {
        BodyPlan::Lagomorph | BodyPlan::Rodent => BOUND,
        _ => GALLOP,
    };
    let trot = match rig.plan {
        BodyPlan::Lagomorph | BodyPlan::Rodent | BodyPlan::Bear => WALK,
        _ => TROT,
    };
    if g <= 1.0 {
        (WALK, trot, g.clamp(0.0, 1.0))
    } else {
        (trot, fast, (g - 1.0).clamp(0.0, 1.0))
    }
}

/// Where a foot is in its stride: forward of its joint (m) and its lift (m).
fn foot_in_stride(gait: &Gait, phase: f32, leg: usize, travel: f32, lift: f32) -> (f32, f32) {
    let p = (phase + gait.offsets[leg]).rem_euclid(1.0);
    let half = travel * 0.5;
    if p < gait.duty {
        let s = p / gait.duty;
        (half * (1.0 - 2.0 * s), 0.0)
    } else {
        let s = (p - gait.duty) / (1.0 - gait.duty);
        let sm = s * s * (3.0 - 2.0 * s);
        (-half + 2.0 * half * sm, lift * (PI * s).sin())
    }
}

/// The middle joint of a two-segment limb from `a` reaching for `t`, bending toward `pole`;
/// the point reached (`t`, or as near as the limb goes).
pub fn two_bone(a: Vec3, t: Vec3, upper: f32, lower: f32, pole: Vec3) -> (Vec3, Vec3) {
    let to = t - a;
    let dist = to
        .length()
        .clamp((upper - lower).abs() + 1e-4, (upper + lower) * 0.999);
    let dir = if to.length_squared() > 1e-10 {
        to.normalize()
    } else {
        Vec3::NEG_Y
    };
    let reach = a + dir * dist;
    let cos_a =
        ((upper * upper + dist * dist - lower * lower) / (2.0 * upper * dist)).clamp(-1.0, 1.0);
    let sin_a = (1.0 - cos_a * cos_a).sqrt();
    let side = (pole - dir * pole.dot(dir)).normalize_or(Vec3::Z);
    let knee = a + (dir * cos_a + side * sin_a) * upper;
    (knee, reach)
}

/// A slot's frame at `at` with its −y along `dir` (a hanging segment), turned about it as
/// little as it can be.
fn hang(at: Vec3, dir: Vec3) -> Affine3A {
    let d = dir.normalize_or(Vec3::NEG_Y);
    Affine3A::from_rotation_translation(Quat::from_rotation_arc(Vec3::NEG_Y, d), at)
}

fn rx(a: f32) -> Affine3A {
    Affine3A::from_rotation_x(a)
}

fn ry(a: f32) -> Affine3A {
    Affine3A::from_rotation_y(a)
}

fn rz(a: f32) -> Affine3A {
    Affine3A::from_rotation_z(a)
}

fn tr(v: Vec3) -> Affine3A {
    Affine3A::from_translation(v)
}

/// A body posed: each slot's frame in the body's, the size it is drawn at (a young one is
/// smaller), what grows on its head and how far it has grown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub slots: [Affine3A; SLOTS],
    pub scale: f32,
    /// How grown a male's antlers are (0 cast, 1 hard); whether horns and tusks show.
    pub antlers: f32,
    pub horns: bool,
    pub tusks: bool,
    /// The chest's breath (a fraction of its size).
    pub breath: f32,
}

/// The size of an animal against a grown one, from its stage (and a young one's age).
pub fn scale_of(rig: &Rig, stage: Stage, year_frac: f32, birth_frac: f32, birth_mass: f32) -> f32 {
    match stage {
        Stage::Adult => 1.0,
        Stage::Juvenile => 0.82,
        Stage::Young => {
            let age = (year_frac - birth_frac).rem_euclid(1.0);
            let birth = (birth_mass / rig.mass.max(1e-6)).cbrt().clamp(0.15, 0.6);
            birth + (0.75 - birth) * (age / 0.6).min(1.0)
        }
    }
}

impl Pose {
    /// Every box of a body as placed: its index in the rig and its placement (the unit cube
    /// to the body's frame).
    pub fn boxes(&self, rig: &Rig) -> Vec<(usize, Affine3A)> {
        let s = Affine3A::from_scale(Vec3::splat(self.scale));
        let mut out = Vec::with_capacity(rig.boxes.len());
        for (i, b) in rig.boxes.iter().enumerate() {
            let grown = match b.gear {
                Gear::None => 1.0,
                Gear::Antler => self.antlers,
                Gear::Horn => {
                    if self.horns {
                        1.0
                    } else {
                        0.0
                    }
                }
                Gear::Tusk => {
                    if self.tusks {
                        1.0
                    } else {
                        0.0
                    }
                }
            };
            if grown <= 0.01 {
                continue;
            }
            let mut size = b.size;
            if matches!(b.slot, Slot::Chest | Slot::Hips)
                && b.part != hearth_texgen::coats::SkinPart::Hump
            {
                size *= Vec3::new(1.0 + self.breath * 0.6, 1.0 + self.breath, 1.0);
            }
            let local = Affine3A::from_scale_rotation_translation(size, b.rot, b.center);
            let grow = Affine3A::from_scale(Vec3::splat(grown));
            out.push((i, s * self.slots[b.slot.index()] * grow * local));
        }
        out
    }
}

/// Poses a body.
pub fn pose(rig: &Rig, m: &Motion, d: &Drive, ground: &dyn Footing) -> Pose {
    if d.act == Act::Dead {
        return dead(rig, m, d);
    }
    let mut p = Pose {
        slots: [Affine3A::IDENTITY; SLOTS],
        scale: 1.0,
        antlers: 0.0,
        horns: false,
        tusks: false,
        breath: 0.0,
    };
    let male_adult = !d.female && d.stage == Stage::Adult;
    // What grows on the head: antlers in their seasons (half grown, in velvet, in the season
    // before), horns on both sexes or the males, tusks on grown males.
    if male_adult {
        p.antlers = antler_growth(rig.antler_seasons, d.year_frac, d.southern);
    }
    p.horns = match rig.gear {
        Some(hearth_content::schema::fauna::HeadGear::Horns { both_sexes, .. }) => {
            d.stage != Stage::Young && (both_sexes || !d.female)
        }
        _ => false,
    };
    p.tusks = male_adult;
    // Breathing: slower as bodies are bigger, faster after running.
    let rate = 0.35 * (rig.mass / 100.0).max(1e-4).powf(-0.25) * (1.0 + m.flee * 2.0);
    p.breath = 0.015 * (TAU * rate.min(3.0) * m.time).sin() * (1.0 + m.flee);
    p.scale = d.scale.max(0.05);
    let scaled = Scaled {
        inner: ground,
        s: p.scale,
    };
    let ground: &dyn Footing = &scaled;
    match rig.frame {
        Frame::Quadruped => quadruped(rig, m, ground, &mut p),
        Frame::Bird => bird(rig, m, ground, &mut p),
        Frame::Fish => fish(rig, m, &mut p),
        Frame::Snake => snake(rig, m, &mut p),
        Frame::Frog => frog(rig, m, ground, &mut p),
        Frame::Insect => insect(rig, m, &mut p),
    }
    p
}

/// Lying dead on a flank: the body as it stood, its legs straight, rolled over onto the side its
/// seed gives; a snake or an insect as it lies.
fn dead(rig: &Rig, m: &Motion, d: &Drive) -> Pose {
    let standing = Drive {
        act: Act::Walk,
        speed: 0.0,
        look: None,
        medium: Medium::Ground,
        ..*d
    };
    let mut still = *m;
    still.settle(rig, &standing);
    let mut p = pose(rig, &still, &standing, &Flat);
    p.breath = 0.0;
    if matches!(rig.frame, Frame::Snake | Frame::Insect) {
        return p;
    }
    // About the long axis, the middle of the body coming down to half its width.
    let s = if m.seed & 1 == 0 { 1.0 } else { -1.0 };
    let roll = tr(Vec3::new(s * rig.torso_y, rig.torso.x * 0.5, 0.0)) * rz(s * PI * 0.5);
    for slot in p.slots.iter_mut() {
        *slot = roll * *slot;
    }
    p
}

/// Antler growth in a season: hard antlers in the seasons the species carries them, growing
/// (half size, in velvet) in the season before the first of them.
pub fn antler_growth(seasons: [bool; 4], year_frac: f32, southern: bool) -> f32 {
    let f = if southern {
        (year_frac + 0.5).rem_euclid(1.0)
    } else {
        year_frac
    };
    let s = ((f * 4.0).floor() as usize).min(3);
    if seasons[s] {
        1.0
    } else if seasons[(s + 1) % 4] {
        // Growing through the season before, from the casting.
        (f * 4.0).fract()
    } else {
        0.0
    }
}

/// A pseudo-random 0–1 from the body's seed and a whole number (flicks, which way).
fn chance(seed: u32, n: u32) -> f32 {
    let h = (seed ^ n.wrapping_mul(0x85eb_ca6b)).wrapping_mul(0xc2b2_ae35);
    let h = h ^ (h >> 15);
    (h % 10_000) as f32 / 10_000.0
}

/// A short twitch now and then: 0 most of the time, up to 1 for a moment every few seconds.
fn flick(m: &Motion, salt: u32, every: f32) -> f32 {
    let n = (m.time / every).floor() as u32;
    let t = m.time / every - n as f32;
    if chance(m.seed ^ salt, n) < 0.45 && t < 0.12 {
        (t / 0.12 * PI).sin()
    } else {
        0.0
    }
}

fn quadruped(rig: &Rig, m: &Motion, ground: &dyn Footing, p: &mut Pose) {
    let tl = rig.torso.z;
    let th = rig.torso.y;
    let h = hip_height(rig);
    let speed = m.speed;
    let moving = (speed / (0.15 * (G * h).sqrt()).max(0.05)).clamp(0.0, 1.0) * (1.0 - m.lie);
    // The ground under the fore and hind feet; the body pitched to the slope between.
    let at = |x: f32, z: f32| ground.ground(Vec3::new(x, 0.0, z));
    let fz = tl * 0.36;
    // Swimming or up a tree, the ground under it is nothing to it.
    let grounded = 1.0 - m.swim.max(m.climb);
    let gf = at(0.0, fz).unwrap_or(0.0) * grounded;
    let gh = at(0.0, -fz).unwrap_or(0.0) * grounded;
    let slope = -((gf - gh) / (2.0 * fz).max(1e-3)).atan().clamp(-0.45, 0.45);
    let base = (gf + gh) * 0.5;
    // The gait: two neighbouring gaits blended, the stride, the body's bob and rock.
    let (ga, gb, gt) = gait_table(rig, m.gait);
    let stride = stride_of(rig, speed);
    let duty = ga.duty + (gb.duty - ga.duty) * gt;
    let travel = (stride * duty).min(h * 0.9) * moving;
    let lift_f = ga.lift + (gb.lift - ga.lift) * gt;
    let galloping = (m.gait - 1.0).clamp(0.0, 1.0) * moving;
    let bob = -h
        * (0.018 + 0.02 * m.gait.min(1.0))
        * (2.0 * TAU * m.phase).cos()
        * moving
        * (1.0 - galloping)
        + h * 0.05 * (TAU * m.phase).sin() * galloping;
    let rock = 0.07 * (TAU * m.phase + 0.6).sin() * galloping;
    // Lying lowers the body onto the ground; rearing lifts the front about the hind legs.
    let lie_y = (th * 0.5 + 0.0) - rig.torso_y;
    // Hares and rodents sit up to look about.
    let sits = matches!(rig.plan, BodyPlan::Lagomorph | BodyPlan::Rodent);
    let sit = if sits {
        0.6 * m.alert * (1.0 - moving)
    } else {
        0.0
    };
    let rear = m.rear * 0.85 + sit + m.climb * 1.35;
    let mut body_y = base + bob + lie_y * m.lie;
    let pitch = slope + rock + m.attack * 0.12;
    let middle = Vec3::Y * rig.torso_y;
    let pivot = Vec3::new(0.0, rig.torso_y, -tl * 0.36);
    let place = |y: f32| {
        tr(Vec3::Y * y)
            * tr(middle)
            * rx(pitch)
            * tr(-middle)
            * tr(pivot)
            * rx(-rear)
            * tr(-pivot)
            * tr(Vec3::Y * rig.torso_y)
    };
    // Where a leg cannot reach the ground under it, the body comes down to it.
    if m.lie < 0.5 && m.rear < 0.5 && grounded > 0.5 {
        let trial = place(body_y);
        let mut short = 0.0f32;
        for leg in &rig.legs {
            let joint = trial.transform_point3(leg.joint - Vec3::Y * rig.torso_y);
            let g = at(leg.joint.x, leg.joint.z).unwrap_or(base) + leg.foot.y;
            short = short.max(joint.y - g - (leg.upper + leg.lower) * 0.985);
        }
        body_y -= short.clamp(0.0, rig.torso_y * 0.5);
    }
    let hips = place(body_y);
    // The back flexes in the gallop (most in the cats and dogs).
    let supple = match rig.plan {
        BodyPlan::Ungulate | BodyPlan::Bear => 0.35,
        _ => 1.0,
    };
    let flex = 0.14 * supple * (TAU * m.phase).sin() * galloping;
    let chest = hips * rx(flex);
    p.slots[Slot::Hips.index()] = hips;
    p.slots[Slot::Chest.index()] = chest;
    let spine = |v: Vec3| v - Vec3::Y * rig.torso_y;
    // Legs.
    let pole_of = |fore: bool| if fore { Vec3::Z } else { Vec3::NEG_Z };
    for (li, leg) in rig.legs.iter().enumerate() {
        let frame = if leg.fore { chest } else { hips };
        let joint = frame.transform_point3(spine(leg.joint));
        let hoof = leg.foot.y;
        // Where the foot goes.
        let (dz, lift) = {
            let (za, la) = foot_in_stride(&ga, m.phase, li, travel, h * lift_f * moving);
            let (zb, lb) = foot_in_stride(&gb, m.phase, li, travel, h * lift_f * moving);
            (za + (zb - za) * gt, la + (lb - la) * gt)
        };
        let mut x = leg.joint.x;
        let mut z = leg.joint.z + dz;
        if leg.fore {
            // Drinking, the forelegs splay; grazing, a short-necked beast leans a little.
            x *= 1.0 + 0.6 * m.drink;
            z += leg.upper * 0.3 * m.drink;
        }
        let g = at(x, z).unwrap_or(base);
        let mut target = Vec3::new(x, g + hoof + lift, z);
        // Lying: the legs folded under the body.
        if m.lie > 0.0 {
            let folded = if leg.fore {
                Vec3::new(
                    leg.joint.x * 0.9,
                    base + hoof,
                    leg.joint.z + leg.upper * 0.75,
                )
            } else {
                Vec3::new(
                    leg.joint.x * 1.5,
                    base + hoof,
                    leg.joint.z + leg.upper * 0.55,
                )
            };
            target = target.lerp(folded, m.lie);
        }
        // Swimming: the legs paddle under the body.
        if m.swim > 0.0 {
            let reach = leg.upper + leg.lower;
            let p = (TAU * (m.phase + [0.25, 0.75, 0.0, 0.5][li])).sin();
            let paddle = joint + Vec3::new(0.0, -reach * 0.75, reach * 0.3 * p);
            target = target.lerp(paddle, m.swim);
        }
        // Climbing: the feet gripping the trunk in front.
        if m.climb > 0.0 {
            let grip = Vec3::new(
                leg.joint.x * 1.6,
                joint.y - leg.upper * 0.2,
                rig.shoulder * 0.3,
            );
            target = target.lerp(grip, m.climb);
        }
        // Rearing: the forelegs lifted and bent.
        if leg.fore && m.rear > 0.0 {
            let up = joint + Vec3::new(0.0, -leg.upper * 0.7, leg.upper * 0.5);
            target = target.lerp(up, m.rear);
        }
        // Attacking (a cat or dog): the forelegs reach.
        if leg.fore && m.attack > 0.0 && rig.plan != BodyPlan::Ungulate {
            let reach = joint + Vec3::new(0.0, -leg.upper * 0.9, leg.upper * 1.2);
            target = target.lerp(reach, m.attack * 0.7);
        }
        let (knee, reached) = two_bone(joint, target, leg.upper, leg.lower, pole_of(leg.fore));
        p.slots[Slot::Upper(li as u8).index()] = hang(joint, knee - joint);
        p.slots[Slot::Lower(li as u8).index()] = hang(knee, reached - knee);
        // The foot flat on the ground when down, tipping with the swing when up.
        let tip = if lift > 1e-4 {
            -0.5 * (lift / (h * 0.25)).min(1.0)
        } else {
            0.0
        };
        let tip = if leg.fore { tip } else { tip * 0.6 };
        p.slots[Slot::Foot(li as u8).index()] = tr(reached - Vec3::Y * hoof) * rx(-tip);
    }
    // The neck and head: up for alarm, down to graze or drink, nodding with the walk; turned to
    // what it looks at; turned back to groom or to sleep.
    let groom_side = if chance(m.seed, 7) < 0.5 { 1.0 } else { -1.0 };
    let nod = 0.06 * (2.0 * TAU * m.phase).sin() * moving * (1.0 - galloping);
    // How far down the neck must go for the mouth to reach the ground.
    let base_h = rig.neck_base.y;
    let reach = (rig.neck_len + rig.head_len * 0.9).max(1e-3);
    let down = ((base_h - 0.04) / reach).clamp(0.0, 1.0).asin() * 0.9;
    let feeding = m.graze.max(m.drink);
    let mut neck_up = rig.neck_pitch * (1.0 - feeding) - down * feeding + 0.35 * m.alert
        - 0.2 * m.flee
        + nod
        + m.look.y * 0.5;
    neck_up -= 0.35 * m.groom + 0.2 * m.sleep;
    neck_up += 0.35 * m.swim;
    neck_up = neck_up * (1.0 - m.lie * 0.4) + m.lie * 0.15 * (1.0 - m.sleep);
    let neck_yaw = m.look.x * 0.55 + groom_side * (1.6 * m.groom + 1.9 * m.sleep);
    let neck = chest * tr(spine(rig.neck_base)) * ry(neck_yaw) * rx(-neck_up);
    p.slots[Slot::Neck.index()] = neck;
    // The head's angle below the level: a little down at rest, level when alert, down to the
    // ground to feed, laid down asleep.
    let chew = (TAU * 1.7 * m.time).sin() * 0.5 + 0.5;
    let head_down = 0.32 - 0.25 * m.alert + 0.95 * feeding + 0.3 * m.groom + 0.4 * m.sleep
        - m.look.y * 0.5
        + 0.15 * m.flee
        + 0.5
            * m.attack
            * if rig.plan == BodyPlan::Ungulate {
                1.0
            } else {
                0.0
            }
        + nod * 0.5;
    let head = neck
        * tr(Vec3::Z * rig.neck_len)
        * ry(m.look.x * 0.45 + groom_side * 0.5 * m.groom)
        * rx(head_down + neck_up);
    p.slots[Slot::Head.index()] = head;
    let hh = rig.head_w * 0.95;
    let jaw_open = 0.12 * chew * m.graze
        + 0.55
            * m.attack
            * if rig.plan == BodyPlan::Ungulate {
                0.2
            } else {
                1.0
            };
    p.slots[Slot::Jaw.index()] = head * tr(Vec3::new(0.0, -hh * 0.3, rig.skull_len)) * rx(jaw_open);
    // Ears: up and splayed, forward when alert, laid back in flight or lying asleep, flicking.
    for s in 0..2u8 {
        let side = if s == 0 { 1.0 } else { -1.0 };
        let twitch = flick(m, 11 + s as u32, 2.7);
        let long = matches!(rig.plan, BodyPlan::Lagomorph);
        let back = 0.25 + 0.9 * m.flee + if long { 0.9 * m.lie } else { 0.3 * m.sleep };
        let pitch = 0.35 * m.alert - back + 0.5 * twitch;
        let splay = 0.35 + 0.25 * m.flee;
        p.slots[Slot::Ear(s).index()] =
            head * tr(Vec3::new(
                side * rig.head_w * 0.32,
                hh * 0.42,
                rig.skull_len * 0.25,
            )) * rz(-side * splay)
                * rx(pitch);
    }
    // The tail: its carriage, swaying with the gait and flicking; up in alarm.
    if rig.tail_segs > 0 {
        let sway = 0.22 * (TAU * m.phase).sin() * moving + 0.35 * flick(m, 31, 3.3);
        let raise = 0.5 * m.alert + 1.4 * m.flee;
        let mut t = hips * tr(spine(rig.tail_base)) * ry(sway) * rx(-rig.tail_pitch + raise);
        if rig.tail_curl {
            // A squirrel's tail rises from the rump and curls over the back.
            t = hips * tr(spine(rig.tail_base)) * ry(sway) * rx(1.1 + raise * 0.3);
        }
        for k in 0..rig.tail_segs {
            p.slots[Slot::Tail(k).index()] = t;
            let bend = if rig.tail_curl { 0.75 } else { 0.12 };
            t = t * tr(Vec3::NEG_Z * rig.tail_seg) * rx(bend) * ry(sway * 0.5);
        }
    }
}

fn bird(rig: &Rig, m: &Motion, ground: &dyn Footing, p: &mut Pose) {
    let bd = rig.torso.y;
    let bw = rig.torso.x;
    let bl = rig.torso.z;
    let base = ground.ground(Vec3::ZERO).unwrap_or(0.0);
    let aloft = m.fly;
    // Owls sit upright; small birds tilt their heads up; ground birds stand level.
    let upright = match rig.plan {
        BodyPlan::Raptor => 0.9,
        BodyPlan::BirdGround => 0.15,
        _ => 0.35,
    };
    // Hopping (small birds) or walking (ground birds) when moving on the ground.
    let moving = (m.speed / 0.05).clamp(0.0, 1.0) * (1.0 - aloft) * (1.0 - m.lie);
    let hopper = rig.plan == BodyPlan::BirdPerching;
    let hop = if hopper {
        let s = m.phase;
        (PI * s).sin().max(0.0) * rig.torso_y * 0.5 * moving
    } else {
        0.0
    };
    let peck = (TAU * 2.2 * m.time).sin().max(0.0) * m.graze;
    let crouch = rig.torso_y * 0.55 * m.lie;
    let pitch = -upright * (1.0 - aloft) * (1.0 - m.graze * 0.7) + 0.55 * m.graze + 0.2 * peck;
    let lift = aloft * rig.torso_y * 3.0;
    let body = tr(Vec3::Y * (base + rig.torso_y + hop - crouch + lift)) * rx(pitch);
    p.slots[Slot::Hips.index()] = body;
    p.slots[Slot::Chest.index()] = body;
    // Legs: down to the ground, alternating when walking; tucked up in flight.
    for (k, leg) in rig.legs.iter().enumerate() {
        let li = (k + 2) as u8;
        let joint = body.transform_point3(leg.joint - Vec3::Y * rig.torso_y);
        let step = if hopper {
            0.0
        } else {
            let (z, l) = foot_in_stride(
                &WALK_BIPED,
                m.phase,
                k,
                rig.torso.z * 0.6 * moving,
                bd * 0.4 * moving,
            );
            let _ = l;
            z
        };
        let mut target = Vec3::new(leg.joint.x, base + leg.foot.y, leg.joint.z + step);
        if aloft > 0.0 {
            let tucked = joint + Vec3::new(0.0, -leg.upper * 0.4, -leg.upper * 0.8);
            target = target.lerp(tucked, aloft);
        }
        let (knee, reached) = two_bone(joint, target, leg.upper, leg.lower, Vec3::NEG_Z);
        p.slots[Slot::Upper(li).index()] = hang(joint, knee - joint);
        p.slots[Slot::Lower(li).index()] = hang(knee, reached - knee);
        p.slots[Slot::Foot(li).index()] = tr(reached - Vec3::Y * leg.foot.y);
    }
    // The head on its short neck: up, down to peck, turned to look or tucked asleep.
    let neck_up = rig.neck_pitch * (1.0 - m.graze) - 0.8 * m.graze + 0.3 * m.alert - 0.6 * m.sleep;
    let neck = body
        * tr(rig.neck_base - Vec3::Y * rig.torso_y)
        * ry(m.look.x * 0.5 + 2.4 * m.sleep)
        * rx(-neck_up);
    p.slots[Slot::Neck.index()] = neck;
    let head_down = -pitch * 0.8 + 0.3 * m.graze + 0.8 * peck - m.look.y;
    p.slots[Slot::Head.index()] =
        neck * tr(Vec3::Z * rig.neck_len) * ry(m.look.x * 0.5) * rx(head_down + neck_up);
    // The tail: cocked a little, fanned down in flight.
    let tail = body
        * tr(rig.tail_base - Vec3::Y * rig.torso_y)
        * rx(
            -rig.tail_pitch + upright * (1.0 - aloft) * 0.95 * (1.0 - m.graze) - 0.5 * m.graze
                + 0.25 * flick(m, 41, 1.7)
                - 0.15 * aloft,
        );
    p.slots[Slot::Tail(0).index()] = tail;
    // The wings: folded along the flanks, or spread and beating.
    let beat = (TAU * m.flap).sin();
    let glide = if rig.plan == BodyPlan::Raptor {
        0.4
    } else {
        1.0
    };
    for s in 0..2u8 {
        let side = if s == 0 { 1.0 } else { -1.0 };
        let shoulder = Vec3::new(side * bw * 0.5, bd * 0.22, bl * 0.18);
        let spread = -side * (PI * 0.5) * aloft;
        let flap = side * 0.9 * beat * aloft * glide;
        let arm = body * tr(shoulder) * rz(flap) * ry(spread) * rx(-0.1 * (1.0 - aloft));
        p.slots[Slot::Wing(s, 0).index()] = arm;
        // The hand: folded back over the arm along the flank, or out from the wrist.
        let wrist = arm * tr(Vec3::NEG_Z * rig.wing.x);
        let folded = body * tr(shoulder + Vec3::new(side * 0.004, bd * 0.06, 0.0)) * rx(-0.08);
        let hand = if aloft > 0.5 {
            wrist * rz(flap * 0.5)
        } else {
            folded
        };
        p.slots[Slot::Wing(s, 1).index()] = hand;
    }
}

/// A bird's walk: the legs alternating.
const WALK_BIPED: Gait = Gait {
    offsets: [0.0, 0.5, 0.0, 0.5],
    duty: 0.6,
    lift: 0.2,
};

fn fish(rig: &Rig, m: &Motion, p: &mut Pose) {
    // A wave down the body, growing toward the tail; the fins fluttering.
    let swim = (m.speed / 0.1).clamp(0.15, 1.0);
    let front = Vec3::new(0.0, rig.torso_y, rig.torso.z * 0.5);
    let mut t = tr(front);
    for k in 0..rig.segs.min(MAX_SEGS as u8) {
        let along = k as f32 / rig.segs.max(1) as f32;
        let wave = 0.35 * swim * along * (TAU * (m.phase * 2.0 - along * 0.8)).sin();
        t *= ry(if k == 0 { -wave * 0.3 } else { wave * 0.5 });
        p.slots[Slot::Seg(k).index()] = t;
        t *= tr(Vec3::NEG_Z * rig.seg_len);
    }
    p.slots[Slot::Hips.index()] = p.slots[Slot::Seg(0).index()];
}

fn snake(rig: &Rig, m: &Motion, p: &mut Pose) {
    let segs = rig.segs.min(MAX_SEGS as u8) as usize;
    let len = rig.seg_len * segs as f32 + rig.head_len;
    let y = rig.torso.y * 0.5;
    // Winding along a wave that travels back down the body as it goes.
    let amp = len * 0.09;
    let wave = len * 0.42;
    let head_z = len * 0.45;
    let mut wind = Vec::with_capacity(segs);
    let mut at = Vec3::new(0.0, y, head_z);
    for k in 0..segs {
        let s = k as f32 * rig.seg_len;
        let slope = (TAU * amp / wave) * (TAU * (s / wave - m.phase)).cos();
        // The segment's −z runs back along the path.
        let back = Vec3::new(slope, 0.0, -1.0).normalize();
        let yaw = (-back.x).atan2(-back.z);
        wind.push((at, yaw));
        at += back * rig.seg_len;
    }
    // Coiled: a flat spiral, the head at its middle, raised.
    let mut coil = Vec::with_capacity(segs);
    let girth = rig.torso.x;
    let mut theta: f32 = 1.2;
    for k in 0..segs {
        let r = girth * 1.3 + girth * 1.15 * theta / TAU;
        let pos = Vec3::new(r * theta.cos(), y, r * theta.sin());
        // The tangent of the spiral, walking outward.
        let tangent = Vec3::new(-theta.sin(), 0.0, theta.cos());
        let yaw = (-tangent.x).atan2(-tangent.z);
        coil.push((pos + Vec3::Y * if k == 0 { girth } else { 0.0 }, yaw));
        theta += rig.seg_len / r;
    }
    let curl = m.lie.max(m.alert * 0.3);
    for k in 0..segs {
        let (pa, ya) = wind[k];
        let (pb, yb) = coil[k];
        let at = pa.lerp(pb, curl);
        let mut yaw = ya + (((yb - ya + PI).rem_euclid(TAU)) - PI) * curl;
        // Alarmed, the front of the body rises in an S, the head toward the threat.
        let rise = if k < 3 {
            m.alert * (3 - k) as f32 * 0.25 * rig.torso.y * 4.0
        } else {
            0.0
        };
        if k == 0 {
            yaw += m.look.x;
        }
        p.slots[Slot::Seg(k as u8).index()] =
            tr(at + Vec3::Y * rise) * ry(yaw) * rx(if k < 2 { -0.3 * m.alert } else { 0.0 });
    }
    // The head on the first segment looks forward from it (its box reaches along +z).
    p.slots[Slot::Hips.index()] = p.slots[Slot::Seg(0).index()];
}

fn frog(rig: &Rig, m: &Motion, ground: &dyn Footing, p: &mut Pose) {
    let base = ground.ground(Vec3::ZERO).unwrap_or(0.0);
    let bl = rig.torso.z;
    // Hopping: a leap every stride, the legs thrown back and folded again.
    let hopping = (m.speed / 0.05).clamp(0.0, 1.0);
    let s = m.phase;
    let air = if s < 0.6 { (PI * s / 0.6).sin() } else { 0.0 } * hopping;
    let lift = air * rig.len * 1.2;
    let body = tr(Vec3::Y * (base + rig.torso_y + lift)) * rx(-0.35 * (1.0 - air) - 0.1 * air);
    p.slots[Slot::Hips.index()] = body;
    p.slots[Slot::Chest.index()] = body;
    p.slots[Slot::Head.index()] = body
        * tr(rig.neck_base - Vec3::Y * rig.torso_y)
        * rx(0.25 + m.look.y * -0.5)
        * ry(m.look.x * 0.3);
    for (li, leg) in rig.legs.iter().enumerate() {
        let joint = body.transform_point3(leg.joint - Vec3::Y * rig.torso_y);
        let sitting = if leg.fore {
            Vec3::new(
                leg.joint.x * 1.2,
                base + leg.foot.y,
                leg.joint.z + bl * 0.15,
            )
        } else {
            Vec3::new(
                leg.joint.x * 1.35,
                base + leg.foot.y,
                leg.joint.z + bl * 0.45,
            )
        };
        let thrown = joint + Vec3::new(0.0, -leg.upper * 0.3, -(leg.upper + leg.lower) * 0.95);
        let target = if leg.fore {
            sitting
        } else {
            sitting.lerp(thrown, air)
        };
        let pole = if leg.fore {
            Vec3::NEG_Z
        } else {
            Vec3::new(leg.side, 0.4, 0.6)
        };
        let (knee, reached) = two_bone(joint, target, leg.upper, leg.lower, pole);
        p.slots[Slot::Upper(li as u8).index()] = hang(joint, knee - joint);
        p.slots[Slot::Lower(li as u8).index()] = hang(knee, reached - knee);
        p.slots[Slot::Foot(li as u8).index()] = tr(reached - Vec3::Y * leg.foot.y);
    }
}

fn insect(rig: &Rig, m: &Motion, p: &mut Pose) {
    let body = tr(Vec3::Y * (rig.torso_y + m.fly * rig.len * 4.0));
    p.slots[Slot::Hips.index()] = body;
    p.slots[Slot::Chest.index()] = body;
    p.slots[Slot::Head.index()] = body * tr(rig.neck_base - Vec3::Y * rig.torso_y);
    // Wings: folded over the back, or a blur of beats.
    let beat = (TAU * m.flap).sin();
    for s in 0..2u8 {
        let side = if s == 0 { 1.0 } else { -1.0 };
        let spread = -side * (0.35 + 0.9 * m.fly);
        let flap = side * 0.8 * beat * m.fly;
        p.slots[Slot::Wing(s, 0).index()] =
            body * tr(Vec3::new(
                side * rig.torso.x * 0.3,
                rig.torso.x * 0.45,
                rig.len * 0.1,
            )) * rz(flap)
                * ry(spread);
    }
}

/// The pose a body's coat is painted on: standing square, head up, wings folded, a snake or
/// fish straight.
pub fn rest_pose(rig: &Rig) -> Pose {
    let mut m = Motion::new(0);
    m.time = 0.0;
    m.phase = 0.0;
    let mut d = Drive::standing();
    d.speed = 0.0;
    m.settle(rig, &d);
    let mut p = pose(rig, &m, &d, &Flat);
    // Every box shown, at its full size.
    p.antlers = 1.0;
    p.horns = true;
    p.tusks = true;
    p.breath = 0.0;
    if matches!(rig.frame, Frame::Snake) {
        // Straight, head forward.
        let segs = rig.segs.min(MAX_SEGS as u8);
        let len = rig.seg_len * segs as f32 + rig.head_len;
        for k in 0..segs {
            let z = len * 0.45 - k as f32 * rig.seg_len;
            p.slots[Slot::Seg(k).index()] = tr(Vec3::new(0.0, rig.torso.y * 0.5, z));
        }
    }
    if matches!(rig.frame, Frame::Fish) {
        let mut t = tr(Vec3::new(0.0, rig.torso_y, rig.torso.z * 0.5));
        for k in 0..rig.segs.min(MAX_SEGS as u8) {
            p.slots[Slot::Seg(k).index()] = t;
            t *= tr(Vec3::NEG_Z * rig.seg_len);
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_limb_reaches_what_it_can_and_bends_toward_its_pole() {
        let a = Vec3::new(0.0, 1.0, 0.0);
        let t = Vec3::new(0.0, 0.2, 0.1);
        let (knee, reached) = two_bone(a, t, 0.5, 0.5, Vec3::Z);
        assert!((reached - t).length() < 1e-4);
        assert!(((knee - a).length() - 0.5).abs() < 1e-4);
        assert!(((reached - knee).length() - 0.5).abs() < 1e-4);
        // Bent forward.
        assert!(knee.z > 0.05);
        // Out of reach: as far as it goes, straight.
        let (_, far) = two_bone(a, Vec3::new(0.0, -2.0, 0.0), 0.5, 0.5, Vec3::Z);
        assert!(((far - a).length() - 1.0).abs() < 0.01);
    }

    #[test]
    fn a_stance_foot_moves_back_and_a_swinging_one_forward_and_up() {
        let travel = 1.0;
        let (z0, l0) = foot_in_stride(&WALK, 0.0, 2, travel, 0.2);
        let (z1, l1) = foot_in_stride(&WALK, 0.3, 2, travel, 0.2);
        assert_eq!(l0, 0.0);
        assert_eq!(l1, 0.0);
        assert!(z1 < z0, "down, the foot goes back under the body");
        let (_, lift) = foot_in_stride(&WALK, 0.84, 2, travel, 0.2);
        assert!(lift > 0.15, "swinging, it lifts");
    }
}

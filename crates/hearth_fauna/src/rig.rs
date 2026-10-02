//! Bodies (V2-7, v2 §7.2): each species as a skeleton of slots with boxes on them at its real
//! dimensions — a torso of two halves sized from its mass, length and height, legs to its
//! shoulder height in two segments and a foot, neck, head, snout and jaw, ears, a tail of up to
//! three segments, antlers, horns or tusks, a hump; a bird's body, wings and tail; the segments
//! of a snake or a fish; a frog's folded legs — built from its body plan and the proportions of
//! its `shape`. [`crate::anim`] poses them; their coats are painted onto the boxes as they lie
//! at rest (`hearth_texgen::coats`).
//!
//! The frame is the animal's: x to its left, y up, z forward; the origin on the ground under
//! the middle of its body. Each slot has its own frame, placed by the pose: a leg segment
//! hangs down its slot's −y from the joint, a neck, head or jaw reaches along +z, a tail or a
//! body segment runs back along −z, an ear stands up its +y.

use glam::{Mat3, Quat, Vec3};
use hearth_content::schema::fauna::{BodyPlan, EarShape, HeadGear};
use hearth_texgen::coats::{SkinKind, SkinPart};

use crate::species::Species;

/// The slots a pose places.
pub const SLOTS: usize = 38;

/// What a box hangs from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slot {
    /// The hindquarters (the body as a whole, for a bird, fish or snake's head end).
    Hips,
    /// The forequarters, bending from the middle of the back.
    Chest,
    Neck,
    Head,
    Jaw,
    /// Left (0) and right (1).
    Ear(u8),
    /// From the base.
    Tail(u8),
    /// Legs: left fore, right fore, left hind, right hind (a bird's or frog's: 2 and 3 are its
    /// legs, 0 and 1 a frog's arms).
    Upper(u8),
    Lower(u8),
    Foot(u8),
    /// Side (0 left, 1 right) and segment (0 the arm, 1 the hand).
    Wing(u8, u8),
    /// A snake's or fish's body, from the head.
    Seg(u8),
}

/// How many body segments a snake or fish has at most.
pub const MAX_SEGS: usize = 12;

impl Slot {
    pub fn index(self) -> usize {
        match self {
            Slot::Hips => 0,
            Slot::Chest => 1,
            Slot::Neck => 2,
            Slot::Head => 3,
            Slot::Jaw => 4,
            Slot::Ear(s) => 5 + s as usize,
            Slot::Tail(k) => 7 + k as usize,
            Slot::Upper(l) => 10 + l as usize,
            Slot::Lower(l) => 14 + l as usize,
            Slot::Foot(l) => 18 + l as usize,
            Slot::Wing(s, k) => 22 + 2 * s as usize + k as usize,
            Slot::Seg(k) => 26 + k as usize,
        }
    }
}

/// The skeleton families.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    Quadruped,
    Bird,
    Fish,
    Snake,
    Frog,
    Insect,
}

/// What a box is beyond its coat: a thing that grows on the head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gear {
    None,
    /// A grown male's in its season.
    Antler,
    Horn,
    /// A grown male's.
    Tusk,
}

/// A box of the body: its slot, its middle, turn and size in the slot's frame, what it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigBox {
    pub slot: Slot,
    pub center: Vec3,
    pub rot: Quat,
    pub size: Vec3,
    pub part: SkinPart,
    pub gear: Gear,
}

/// A leg: where its top joint is at rest (the animal's frame), its segments' lengths, the
/// foot's size, whether it is a foreleg and its side (+1 left).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Leg {
    pub joint: Vec3,
    pub upper: f32,
    pub lower: f32,
    pub foot: Vec3,
    pub fore: bool,
    pub side: f32,
}

/// A species' body.
#[derive(Debug, Clone, PartialEq)]
pub struct Rig {
    pub frame: Frame,
    pub plan: BodyPlan,
    pub kind: SkinKind,
    /// Its coat's pixels a metre.
    pub ppm: f32,
    pub boxes: Vec<RigBox>,
    /// Length nose to rump, height at the shoulder (a bird's back), mass.
    pub len: f32,
    pub shoulder: f32,
    pub mass: f32,
    /// The torso: width, depth, length; the height of its middle at rest.
    pub torso: Vec3,
    pub torso_y: f32,
    pub legs: Vec<Leg>,
    /// Where the neck leaves the chest (the animal's frame), its length and its rest angle
    /// above the level (radians).
    pub neck_base: Vec3,
    pub neck_len: f32,
    pub neck_pitch: f32,
    /// The head's length (skull and snout) and width; the skull's share of the length.
    pub head_len: f32,
    pub head_w: f32,
    pub skull_len: f32,
    /// Where the tail leaves the body, its segments' length and count, and its carriage below
    /// the level at rest (radians; negative held up).
    pub tail_base: Vec3,
    pub tail_seg: f32,
    pub tail_segs: u8,
    pub tail_pitch: f32,
    /// Squirrel-like: the tail curls up over the back.
    pub tail_curl: bool,
    /// A bird's wing: the arm's and the hand's length and the chord; a snake's or fish's
    /// segments' length and count.
    pub wing: Vec3,
    pub seg_len: f32,
    pub segs: u8,
    /// What grows on the head, and the seasons a male carries antlers.
    pub gear: Option<HeadGear>,
    pub antler_seasons: [bool; 4],
}

/// The usual proportions of a body plan.
struct PlanShape {
    neck: f32,
    head: f32,
    snout: f32,
    tail: f32,
    tail_width: f32,
    ears: f32,
    ear_shape: EarShape,
    legs: f32,
    /// The torso's depth, of the shoulder height.
    depth: f32,
    neck_pitch: f32,
    tail_pitch: f32,
}

fn plan_shape(plan: BodyPlan) -> PlanShape {
    let deg = f32::to_radians;
    match plan {
        BodyPlan::Ungulate | BodyPlan::Giraffe | BodyPlan::Elephant | BodyPlan::Hippo => {
            PlanShape {
                neck: 0.3,
                head: 0.22,
                snout: 0.45,
                tail: 0.1,
                tail_width: 0.25,
                ears: 0.4,
                ear_shape: EarShape::Pointed,
                legs: 1.0,
                depth: 0.42,
                neck_pitch: deg(48.0),
                tail_pitch: deg(70.0),
            }
        }
        BodyPlan::Bear => PlanShape {
            neck: 0.12,
            head: 0.2,
            snout: 0.4,
            tail: 0.05,
            tail_width: 0.2,
            ears: 0.2,
            ear_shape: EarShape::Round,
            legs: 1.3,
            depth: 0.55,
            neck_pitch: deg(12.0),
            tail_pitch: deg(45.0),
        },
        BodyPlan::Rodent => PlanShape {
            neck: 0.04,
            head: 0.26,
            snout: 0.35,
            tail: 0.5,
            tail_width: 0.15,
            ears: 0.25,
            ear_shape: EarShape::Round,
            legs: 0.8,
            depth: 0.75,
            neck_pitch: deg(15.0),
            tail_pitch: deg(5.0),
        },
        BodyPlan::Lagomorph => PlanShape {
            neck: 0.06,
            head: 0.24,
            snout: 0.3,
            tail: 0.1,
            tail_width: 0.4,
            ears: 0.8,
            ear_shape: EarShape::Long,
            legs: 1.0,
            depth: 0.75,
            neck_pitch: deg(25.0),
            tail_pitch: deg(-35.0),
        },
        // Carnivores, primates and the rest of the four-legged.
        _ => PlanShape {
            neck: 0.18,
            head: 0.2,
            snout: 0.45,
            tail: 0.3,
            tail_width: 0.3,
            ears: 0.35,
            ear_shape: EarShape::Pointed,
            legs: 1.0,
            depth: 0.45,
            neck_pitch: deg(25.0),
            tail_pitch: deg(35.0),
        },
    }
}

/// The skeleton family of a body plan.
pub fn frame_of(plan: BodyPlan) -> Frame {
    match plan {
        BodyPlan::BirdPerching
        | BodyPlan::BirdGround
        | BodyPlan::Waterfowl
        | BodyPlan::Raptor
        | BodyPlan::Seabird
        | BodyPlan::Penguin => Frame::Bird,
        BodyPlan::FishFusiform | BodyPlan::FishFlat | BodyPlan::Cetacean => Frame::Fish,
        BodyPlan::Snake | BodyPlan::Eel => Frame::Snake,
        BodyPlan::Amphibian => Frame::Frog,
        BodyPlan::Insect | BodyPlan::Crab => Frame::Insect,
        _ => Frame::Quadruped,
    }
}

/// The mass of a grown animal of a sex (heavier for a male by the species' dimorphism).
pub fn mass_of(sp: &Species, male: bool) -> f32 {
    hearth_content::butchery::grown_mass(sp.mass_range, sp.dimorphism, male)
}

struct Builder {
    boxes: Vec<RigBox>,
}

impl Builder {
    fn put(&mut self, slot: Slot, center: Vec3, size: Vec3, part: SkinPart) {
        self.gear(slot, center, size, part, Gear::None);
    }

    fn gear(&mut self, slot: Slot, center: Vec3, size: Vec3, part: SkinPart, gear: Gear) {
        self.boxes.push(RigBox {
            slot,
            center,
            rot: Quat::IDENTITY,
            size: size.max(Vec3::splat(0.002)),
            part,
            gear,
        });
    }
}

impl Rig {
    /// The body of a grown animal of a species and sex (a young one is this, smaller; a male's
    /// head gear is in every rig of its species, shown as the pose says).
    pub fn of(sp: &Species, male: bool) -> Rig {
        let mut r = match frame_of(sp.plan) {
            Frame::Quadruped => quadruped(sp, male),
            Frame::Bird => bird(sp, male),
            Frame::Fish => fish(sp, male),
            Frame::Snake => snake(sp, male),
            Frame::Frog => frog(sp, male),
            Frame::Insect => insect(sp, male),
        };
        // Pixels a metre: a deer's coat at the world's sixteen, a vole's finer so that it has
        // a face at all; no box wider than an unwrap can hold.
        let biggest = r
            .boxes
            .iter()
            .map(|b| b.size.max_element())
            .fold(0.0f32, f32::max)
            .max(1e-3);
        r.ppm = (24.0 / r.len.max(0.01))
            .clamp(16.0, 160.0)
            .min(120.0 / biggest);
        r
    }

    /// The pixel size of a box's coat (along x, y, z).
    pub fn pixels(&self, b: &RigBox) -> [u32; 3] {
        let p = (b.size * self.ppm)
            .round()
            .max(Vec3::ONE)
            .min(Vec3::splat(255.0));
        [p.x as u32, p.y as u32, p.z as u32]
    }

    fn empty(sp: &Species, frame: Frame, kind: SkinKind, mass: f32) -> Rig {
        Rig {
            frame,
            plan: sp.plan,
            kind,
            ppm: 16.0,
            boxes: Vec::new(),
            len: sp.length_m,
            shoulder: sp.shoulder_m,
            mass,
            torso: Vec3::ZERO,
            torso_y: 0.0,
            legs: Vec::new(),
            neck_base: Vec3::ZERO,
            neck_len: 0.0,
            neck_pitch: 0.0,
            head_len: 0.0,
            head_w: 0.0,
            skull_len: 0.0,
            tail_base: Vec3::ZERO,
            tail_seg: 0.0,
            tail_segs: 0,
            tail_pitch: 0.0,
            tail_curl: false,
            wing: Vec3::ZERO,
            seg_len: 0.0,
            segs: 0,
            gear: sp.shape.head_gear,
            antler_seasons: sp.antler_seasons,
        }
    }
}

/// The head's boxes: skull, snout, jaw, ears and what grows on it, on the head's slot (its
/// origin where it meets the neck, +z forward).
#[allow(clippy::too_many_arguments)]
fn head(
    b: &mut Builder,
    r: &Rig,
    hw: f32,
    hh: f32,
    skull_l: f32,
    snout_l: f32,
    ears: f32,
    ear_shape: EarShape,
) {
    b.put(
        Slot::Head,
        Vec3::new(0.0, 0.0, skull_l * 0.5),
        Vec3::new(hw, hh, skull_l),
        SkinPart::Head,
    );
    if snout_l > 0.0 {
        b.put(
            Slot::Head,
            Vec3::new(0.0, -hh * 0.12, skull_l + snout_l * 0.5),
            Vec3::new(hw * 0.62, hh * 0.58, snout_l),
            SkinPart::Snout,
        );
        b.put(
            Slot::Jaw,
            Vec3::new(0.0, -hh * 0.09, snout_l * 0.45),
            Vec3::new(hw * 0.5, hh * 0.18, snout_l * 0.9),
            SkinPart::Jaw,
        );
    }
    // Ears, standing up their slots' +y.
    let el = ears * (skull_l + snout_l);
    let size = match ear_shape {
        EarShape::Pointed | EarShape::Tufted => Vec3::new(hw * 0.2, el, hw * 0.07),
        EarShape::Round => Vec3::new(hw * 0.28, el.max(hw * 0.2), hw * 0.07),
        EarShape::Long => Vec3::new(hw * 0.26, el, hw * 0.09),
    };
    if el > 0.0 {
        for s in 0..2u8 {
            b.put(
                Slot::Ear(s),
                Vec3::new(0.0, size.y * 0.5, 0.0),
                size,
                SkinPart::Ear,
            );
            if ear_shape == EarShape::Tufted {
                b.put(
                    Slot::Ear(s),
                    Vec3::new(0.0, size.y + hw * 0.08, 0.0),
                    Vec3::new(hw * 0.05, hw * 0.16, hw * 0.05),
                    SkinPart::Ear,
                );
            }
        }
    }
    // What grows on the head.
    match r.gear {
        Some(HeadGear::Antlers {
            length_m,
            tines,
            palmate,
            ..
        }) => {
            // Each beam rises from the skull's top back, outward and back, then forward; the
            // tines point forward and up off it (a palmate pair, out to the sides and broad).
            // Placed on the head's slot.
            let beam = length_m;
            let w = (beam * 0.07).max(0.012);
            for side in [1.0f32, -1.0] {
                let base = Vec3::new(side * hw * 0.3, hh * 0.5, skull_l * 0.25);
                if palmate {
                    // A short beam out to the side, then a broad palm cupped up and out, its
                    // rim set with tines (a moose's).
                    let d1 = Vec3::new(side * 0.85, 0.3, -0.3).normalize();
                    let mid = base + d1 * beam * 0.28;
                    antler_segment(b, base, mid, w);
                    let d2 = Vec3::new(side * 0.8, 0.5, 0.05).normalize();
                    let l2 = beam * 0.72;
                    let across = Vec3::new(0.0, 0.35, 1.0).normalize();
                    let width = beam * 0.42;
                    antler_plate(b, mid, mid + d2 * l2, across, width, w * 0.55);
                    // The tines: along the palm's far end and its front and back edges.
                    for k in 0..tines {
                        let t = (k as f32 + 0.5) / tines as f32;
                        let edge = if k % 2 == 0 { 1.0 } else { -1.0 };
                        let along = 0.45 + 0.55 * t;
                        let at = mid + d2 * (l2 * along) + across * (edge * width * 0.45);
                        let dir = (d2 * 0.6 + Vec3::Y * 0.7 + across * edge * 0.4).normalize();
                        antler_segment(b, at, at + dir * beam * 0.14, w * 0.45);
                    }
                    continue;
                }
                let d1 = Vec3::new(side * 0.45, 0.85, -0.35).normalize();
                let d2 = Vec3::new(side * 0.3, 0.8, 0.45).normalize();
                let l1 = beam * 0.55;
                let l2 = beam * 0.45;
                let mid = base + d1 * l1;
                let tip = mid + d2 * l2;
                antler_segment(b, base, mid, w);
                antler_segment(b, mid, tip, w * 0.8);
                // The tines, spread along the beam.
                for k in 0..tines {
                    let t = (k as f32 + 0.6) / (tines as f32 + 0.4);
                    let at = if t < 0.55 {
                        base + d1 * (l1 * t / 0.55)
                    } else {
                        mid + d2 * (l2 * (t - 0.55) / 0.45)
                    };
                    let tine = beam * if k == 0 { 0.28 } else { 0.22 };
                    let dir = Vec3::new(side * 0.1, 0.45, 0.9).normalize();
                    antler_segment(b, at, at + dir * tine, w * 0.6);
                }
            }
        }
        Some(HeadGear::Horns {
            length_m,
            curve,
            droop: true,
            ..
        }) => {
            // A boss across the brow; from it each horn sweeps down beside the face, then out,
            // forward and up at the tip.
            let w = (length_m * 0.16).max(0.02);
            horn_segment(
                b,
                Vec3::new(-hw * 0.5, hh * 0.45, skull_l * 0.15),
                Vec3::new(hw * 0.5, hh * 0.45, skull_l * 0.15),
                w * 1.3,
                SkinPart::Horn,
                Gear::Horn,
            );
            for side in [1.0f32, -1.0] {
                let mut at = Vec3::new(side * hw * 0.5, hh * 0.4, skull_l * 0.15);
                let mut dir = Vec3::new(side * 0.55, -0.8, 0.2).normalize();
                let segs = 4;
                for k in 0..segs {
                    let l = length_m / segs as f32;
                    let next = at + dir * l;
                    let thick = w * (1.0 - 0.22 * k as f32);
                    horn_segment(b, at, next, thick, SkinPart::Horn, Gear::Horn);
                    at = next;
                    let turn = curve * 0.9;
                    dir = (dir + Vec3::new(side * 0.2, 0.8, 0.45) * turn).normalize();
                }
            }
        }
        Some(HeadGear::Horns {
            length_m, curve, ..
        }) => {
            // Out from the skull's top sides, curving forward and up.
            let w = (length_m * 0.12).max(0.02);
            for side in [1.0f32, -1.0] {
                let mut at = Vec3::new(side * hw * 0.42, hh * 0.38, skull_l * 0.2);
                let mut dir = Vec3::new(side, 0.15, 0.0).normalize();
                let segs = 3;
                for k in 0..segs {
                    let l = length_m / segs as f32;
                    let next = at + dir * l;
                    let thick = w * (1.0 - 0.28 * k as f32);
                    horn_segment(b, at, next, thick, SkinPart::Horn, Gear::Horn);
                    at = next;
                    // Bending forward and up by the curve.
                    let turn = curve * 1.2;
                    dir = (dir + Vec3::new(-side * 0.35, 0.5, 0.75) * turn).normalize();
                }
            }
        }
        Some(HeadGear::Tusks { length_m }) => {
            let w = (length_m * 0.18).max(0.008);
            for side in [1.0f32, -1.0] {
                let at = Vec3::new(side * hw * 0.3, -hh * 0.2, skull_l + snout_l * 0.75);
                let tip = at + Vec3::new(side * 0.35, 0.9, -0.2).normalize() * length_m;
                horn_segment(b, at, tip, w, SkinPart::Tusk, Gear::Tusk);
            }
        }
        None => {}
    }
}

/// A box from `from` to `to` on the head's slot, `w` thick (a beam, a tine, a horn's length).
fn horn_segment(b: &mut Builder, from: Vec3, to: Vec3, w: f32, part: SkinPart, gear: Gear) {
    let d = to - from;
    let len = d.length().max(1e-4);
    b.boxes.push(RigBox {
        slot: Slot::Head,
        center: (from + to) * 0.5,
        rot: Quat::from_rotation_arc(Vec3::Z, d / len),
        size: Vec3::new(w, w, len + w * 0.5).max(Vec3::splat(0.002)),
        part,
        gear,
    });
}

fn antler_segment(b: &mut Builder, from: Vec3, to: Vec3, w: f32) {
    horn_segment(b, from, to, w, SkinPart::Antler, Gear::Antler);
}

/// A flat plate of antler (a moose's palm) from `from` to `to`, `width` across along `across`
/// and `thick` through.
fn antler_plate(b: &mut Builder, from: Vec3, to: Vec3, across: Vec3, width: f32, thick: f32) {
    let d = to - from;
    let len = d.length().max(1e-4);
    let z = d / len;
    let x = (across - z * across.dot(z)).normalize_or_zero();
    let x = if x == Vec3::ZERO {
        z.any_orthonormal_vector()
    } else {
        x
    };
    let y = z.cross(x);
    b.boxes.push(RigBox {
        slot: Slot::Head,
        center: (from + to) * 0.5,
        rot: Quat::from_mat3(&Mat3::from_cols(x, y, z)),
        size: Vec3::new(width, thick, len).max(Vec3::splat(0.002)),
        part: SkinPart::Antler,
        gear: Gear::Antler,
    });
}

fn quadruped(sp: &Species, male: bool) -> Rig {
    let plan = sp.plan;
    let d = plan_shape(plan);
    let s = sp.shape;
    let mass = mass_of(sp, male);
    let mut r = Rig::empty(sp, Frame::Quadruped, SkinKind::Mammal, mass);
    let len = sp.length_m;
    let shoulder = sp.shoulder_m;
    let neck_f = s.neck.unwrap_or(d.neck);
    let head_f = s.head.unwrap_or(d.head);
    let snout_f = s.snout.unwrap_or(d.snout);
    let tail_f = s.tail.unwrap_or(d.tail);
    let tail_w = s.tail_width.unwrap_or(d.tail_width);
    let ears = s.ears.unwrap_or(d.ears);
    let ear_shape = s.ear_shape.unwrap_or(d.ear_shape);
    let legs_f = s.legs.unwrap_or(d.legs);
    let hump = s.hump.unwrap_or(0.0);
    let neck_l = neck_f * len;
    let head_l = head_f * len;
    // The torso: what the length leaves after the head and the neck reaching forward.
    let reach = head_l * 0.85 + neck_l * d.neck_pitch.cos() * 0.75;
    let mut tl = (len - reach).clamp(len * 0.42, len * 0.8);
    // Hares and rodents are round, their backs arched: shorter than the length leaves.
    if matches!(plan, BodyPlan::Lagomorph | BodyPlan::Rodent) {
        tl *= 0.8;
    }
    let th = shoulder * d.depth;
    // Wide enough to hold its mass (the torso about four fifths of it, filling half the box).
    let volume = mass / 1000.0;
    let tw = (0.8 * volume / (0.55 * tl * th)).clamp(0.45 * th, 1.25 * th);
    let torso_y = shoulder - th * 0.5;
    r.torso = Vec3::new(tw, th, tl);
    r.torso_y = torso_y;
    let mut b = Builder { boxes: Vec::new() };
    // The two halves of the torso about the middle of the back.
    b.put(
        Slot::Hips,
        Vec3::new(0.0, 0.0, -tl * 0.26),
        Vec3::new(tw * 0.96, th * 0.96, tl * 0.52),
        SkinPart::Body,
    );
    b.put(
        Slot::Chest,
        Vec3::new(0.0, 0.0, tl * 0.25),
        Vec3::new(tw, th, tl * 0.52),
        SkinPart::Body,
    );
    if hump > 0.0 {
        let hh = hump * shoulder;
        b.put(
            Slot::Chest,
            Vec3::new(0.0, th * 0.5 + hh * 0.4, tl * 0.2),
            Vec3::new(tw * 0.7, hh, tl * 0.34),
            SkinPart::Hump,
        );
    }
    // Legs: the joints low in the torso, the feet under them; standing a little flexed.
    let lw = (tw * 0.24 * legs_f).clamp(0.006, th * 0.6);
    let lagomorph = plan == BodyPlan::Lagomorph;
    let plantigrade = plan == BodyPlan::Bear;
    for l in 0..4u8 {
        let fore = l < 2;
        let side = if l % 2 == 0 { 1.0 } else { -1.0 };
        let joint = Vec3::new(
            side * tw * 0.3,
            torso_y - th * if fore { 0.2 } else { 0.12 },
            if fore { tl * 0.36 } else { -tl * 0.36 },
        );
        let reach = joint.y / 0.93;
        // A hind leg's upper segment the longer (the thigh and shank to the hock), but for a
        // hare's, whose long foot is the lower.
        let (upper, lower) = if fore || lagomorph {
            (reach * 0.5, reach * 0.5)
        } else {
            (reach * 0.55, reach * 0.45)
        };
        let foot_l = if plantigrade || (lagomorph && !fore) {
            lw * 2.6
        } else {
            lw * 1.5
        };
        let foot = Vec3::new(lw * 1.1, (lw * 0.6).min(lower * 0.25), foot_l);
        r.legs.push(Leg {
            joint,
            upper,
            lower,
            foot,
            fore,
            side,
        });
        b.put(
            Slot::Upper(l),
            Vec3::new(0.0, -upper * 0.5, 0.0),
            Vec3::new(lw * 1.25, upper + lw * 0.3, lw * 1.45),
            SkinPart::UpperLeg,
        );
        b.put(
            Slot::Lower(l),
            Vec3::new(0.0, -lower * 0.5, 0.0),
            Vec3::new(lw * 0.9, lower, lw * 0.9),
            SkinPart::LowerLeg,
        );
        b.put(
            Slot::Foot(l),
            Vec3::new(0.0, foot.y * 0.5, foot.z * 0.3),
            foot,
            SkinPart::Foot,
        );
    }
    // The neck from the top front of the chest, along its slot's +z.
    let nw = (tw * 0.5).max(lw * 1.5);
    let nh = th * 0.55;
    r.neck_base = Vec3::new(0.0, torso_y + th * 0.2, tl * 0.5 - nw * 0.2);
    r.neck_len = neck_l.max(nh * 0.4);
    r.neck_pitch = d.neck_pitch;
    b.put(
        Slot::Neck,
        Vec3::new(0.0, 0.0, r.neck_len * 0.45),
        Vec3::new(nw, nh, r.neck_len + nh * 0.3),
        SkinPart::Neck,
    );
    // The head.
    let snout_l = head_l * snout_f;
    let skull_l = head_l - snout_l;
    let hw = (tw * 0.5).clamp(head_l * 0.3, head_l * 0.7);
    let hh = hw * 0.95;
    r.head_len = head_l;
    r.head_w = hw;
    r.skull_len = skull_l;
    head(&mut b, &r, hw, hh, skull_l, snout_l, ears, ear_shape);
    // The tail from the top of the rump.
    let tail_l = tail_f * len;
    if tail_l > 0.0 {
        let segs: u8 = if tail_l < len * 0.2 {
            1
        } else if tail_l < len * 0.6 {
            2
        } else {
            3
        };
        let seg = tail_l / segs as f32;
        let w = (tw * tail_w).max(0.004);
        r.tail_base = Vec3::new(0.0, torso_y + th * 0.3, -tl * 0.5);
        r.tail_seg = seg;
        r.tail_segs = segs;
        r.tail_pitch = d.tail_pitch;
        r.tail_curl = plan == BodyPlan::Rodent && tail_f > 0.6;
        for k in 0..segs {
            // Bushy tails thicken toward the middle.
            let bush = if tail_w > 0.35 && k > 0 { 1.15 } else { 1.0 };
            b.put(
                Slot::Tail(k),
                Vec3::new(0.0, 0.0, -seg * 0.5),
                Vec3::new(w * bush, w * bush, seg + w * 0.2),
                SkinPart::Tail,
            );
        }
    }
    r.boxes = b.boxes;
    r
}

fn bird(sp: &Species, male: bool) -> Rig {
    let s = sp.shape;
    let mass = mass_of(sp, male);
    let mut r = Rig::empty(sp, Frame::Bird, SkinKind::Bird, mass);
    let len = sp.length_m;
    let height = sp.shoulder_m.max(len * 0.25);
    let head_f = s.head.unwrap_or(0.2);
    let beak_f = s.snout.unwrap_or(0.35);
    let tail_f = s.tail.unwrap_or(0.3);
    let neck_f = s.neck.unwrap_or(0.06);
    let legs_f = s.legs.unwrap_or(1.0);
    let tail_w = s.tail_width.unwrap_or(0.8);
    let bl = (len * (1.0 - head_f - tail_f * 0.75 - neck_f)).clamp(len * 0.3, len * 0.6);
    let bd = bl * 0.62;
    let bw = bl * 0.55;
    let torso_y = height - bd * 0.5;
    r.torso = Vec3::new(bw, bd, bl);
    r.torso_y = torso_y;
    let mut b = Builder { boxes: Vec::new() };
    b.put(
        Slot::Hips,
        Vec3::new(0.0, 0.0, -bl * 0.2),
        Vec3::new(bw * 0.92, bd * 0.9, bl * 0.6),
        SkinPart::Body,
    );
    b.put(
        Slot::Chest,
        Vec3::new(0.0, -bd * 0.04, bl * 0.22),
        Vec3::new(bw, bd, bl * 0.56),
        SkinPart::Hump,
    );
    // Legs: the thigh hidden in the body, the shank and the scaly tarsus below, the toes.
    let leg_top = torso_y - bd * 0.25;
    let lw = (bw * 0.12 * legs_f).max(0.003);
    for l in 2..4u8 {
        let side = if l % 2 == 0 { 1.0 } else { -1.0 };
        let joint = Vec3::new(side * bw * 0.25, leg_top, bl * 0.02);
        let reach = joint.y / 0.97;
        let (upper, lower) = (reach * 0.45, reach * 0.55);
        let foot = Vec3::new(lw * 2.5, lw * 0.7, bl * 0.32);
        r.legs.push(Leg {
            joint,
            upper,
            lower,
            foot,
            fore: false,
            side,
        });
        b.put(
            Slot::Upper(l),
            Vec3::new(0.0, -upper * 0.5, 0.0),
            Vec3::new(lw * 2.2, upper, lw * 2.4),
            SkinPart::UpperLeg,
        );
        b.put(
            Slot::Lower(l),
            Vec3::new(0.0, -lower * 0.5, 0.0),
            Vec3::new(lw, lower, lw),
            SkinPart::LowerLeg,
        );
        b.put(
            Slot::Foot(l),
            Vec3::new(0.0, foot.y * 0.5, foot.z * 0.25),
            foot,
            SkinPart::Foot,
        );
    }
    // A short neck, and the head with its beak.
    let head_l = head_f * len;
    let beak = head_l * beak_f;
    let skull = head_l - beak;
    let hw = (bw * 0.62).max(skull * 0.6);
    let hh = hw * 0.95;
    r.neck_base = Vec3::new(0.0, torso_y + bd * 0.25, bl * 0.42);
    r.neck_len = (neck_f * len).max(hh * 0.3);
    r.neck_pitch = 60f32.to_radians();
    b.put(
        Slot::Neck,
        Vec3::new(0.0, 0.0, r.neck_len * 0.5),
        Vec3::new(hw * 0.75, hh * 0.75, r.neck_len + hh * 0.2),
        SkinPart::Neck,
    );
    r.head_len = head_l;
    r.head_w = hw;
    r.skull_len = skull;
    b.put(
        Slot::Head,
        Vec3::new(0.0, 0.0, skull * 0.5),
        Vec3::new(hw, hh, skull),
        SkinPart::Head,
    );
    b.put(
        Slot::Head,
        Vec3::new(0.0, -hh * 0.08, skull + beak * 0.5),
        Vec3::new(hw * 0.38, hh * 0.34, beak),
        SkinPart::Beak,
    );
    // The tail: a flat fan from the rump.
    let tl = tail_f * len;
    r.tail_base = Vec3::new(0.0, torso_y + bd * 0.1, -bl * 0.48);
    r.tail_seg = tl;
    r.tail_segs = 1;
    r.tail_pitch = 10f32.to_radians();
    b.put(
        Slot::Tail(0),
        Vec3::new(0.0, 0.0, -tl * 0.5),
        Vec3::new(bw * tail_w * 1.1, bd * 0.1, tl),
        SkinPart::Tail,
    );
    // The wings, folded along the flanks: the arm and the hand, each along its slot's −z.
    let span_half = len
        * match sp.plan {
            BodyPlan::Raptor => 1.15,
            BodyPlan::BirdGround => 0.75,
            _ => 0.8,
        };
    let chord = bl * 0.5;
    r.wing = Vec3::new(span_half * 0.42, span_half * 0.58, chord);
    for side in 0..2u8 {
        b.put(
            Slot::Wing(side, 0),
            Vec3::new(0.0, 0.0, -r.wing.x * 0.5),
            Vec3::new(bd * 0.1, chord, r.wing.x),
            SkinPart::Wing,
        );
        b.put(
            Slot::Wing(side, 1),
            Vec3::new(0.0, 0.0, -r.wing.y * 0.5),
            Vec3::new(bd * 0.08, chord * 0.8, r.wing.y),
            SkinPart::Wing,
        );
    }
    r.boxes = b.boxes;
    r
}

fn fish(sp: &Species, male: bool) -> Rig {
    let s = sp.shape;
    let mass = mass_of(sp, male);
    let mut r = Rig::empty(sp, Frame::Fish, SkinKind::Fish, mass);
    let len = sp.length_m;
    let tail_f = s.tail.unwrap_or(0.18);
    let head_f = s.head.unwrap_or(0.22);
    let body = len * (1.0 - tail_f * 0.8);
    // Deep and narrow, from its mass (a fish a little denser than water).
    let depth = (mass / 1000.0 / (0.55 * 0.45 * body))
        .sqrt()
        .clamp(len * 0.14, len * 0.32);
    let width = depth * 0.45;
    r.torso = Vec3::new(width, depth, body);
    r.torso_y = depth * 0.7;
    r.head_len = head_f * len;
    r.head_w = width;
    let segs = 4u8;
    r.segs = segs;
    r.seg_len = body / segs as f32;
    let mut b = Builder { boxes: Vec::new() };
    for k in 0..segs {
        // Deepest a third back, tapering to the tail.
        let t = k as f32 / (segs - 1) as f32;
        let taper = 1.0 - 0.55 * ((t - 0.3).max(0.0) / 0.7).powf(1.3);
        let part = if k == 0 {
            SkinPart::Head
        } else {
            SkinPart::Body
        };
        b.put(
            Slot::Seg(k),
            Vec3::new(0.0, 0.0, -r.seg_len * 0.5),
            Vec3::new(width * taper, depth * taper, r.seg_len * 1.04),
            part,
        );
    }
    // The tail fin on the last segment, the back fin on the second, the breast fins on the
    // first.
    let last = segs - 1;
    b.put(
        Slot::Seg(last),
        Vec3::new(0.0, 0.0, -r.seg_len - tail_f * len * 0.45),
        Vec3::new(width * 0.15, depth * 0.95, tail_f * len),
        SkinPart::Fin,
    );
    b.put(
        Slot::Seg(1),
        Vec3::new(0.0, depth * 0.6, -r.seg_len * 0.4),
        Vec3::new(width * 0.12, depth * 0.35, r.seg_len * 0.9),
        SkinPart::Fin,
    );
    for side in [1.0f32, -1.0] {
        b.put(
            Slot::Seg(0),
            Vec3::new(side * width * 0.55, -depth * 0.25, -r.seg_len * 0.8),
            Vec3::new(width * 0.4, depth * 0.08, r.seg_len * 0.45),
            SkinPart::Fin,
        );
    }
    r.boxes = b.boxes;
    r
}

fn snake(sp: &Species, male: bool) -> Rig {
    let s = sp.shape;
    let mass = mass_of(sp, male);
    let mut r = Rig::empty(sp, Frame::Snake, SkinKind::Snake, mass);
    let len = sp.length_m;
    let head_f = s.head.unwrap_or(0.06);
    let tail_f = s.tail.unwrap_or(0.15);
    let girth = (4.0 * mass / 1000.0 / (std::f32::consts::PI * len))
        .sqrt()
        .max(len * 0.025);
    r.torso = Vec3::new(girth, girth * 0.8, len);
    r.torso_y = girth * 0.4;
    let segs = 10u8;
    r.segs = segs;
    let head_l = head_f * len;
    r.seg_len = (len - head_l) / segs as f32;
    r.head_len = head_l;
    r.head_w = girth * 1.25;
    let mut b = Builder { boxes: Vec::new() };
    // The head on the first slot, the body behind it, thinning to the tail's end.
    b.put(
        Slot::Seg(0),
        Vec3::new(0.0, 0.0, head_l * 0.5),
        Vec3::new(girth * 1.3, girth * 0.75, head_l),
        SkinPart::Head,
    );
    for k in 0..segs {
        let t = (k as f32 + 0.5) / segs as f32;
        let taper = if t > 1.0 - tail_f {
            1.0 - 0.75 * (t - (1.0 - tail_f)) / tail_f
        } else {
            1.0
        };
        b.put(
            Slot::Seg(k),
            Vec3::new(0.0, 0.0, -r.seg_len * 0.5),
            Vec3::new(girth * taper, girth * 0.8 * taper, r.seg_len * 1.08),
            SkinPart::Body,
        );
    }
    r.boxes = b.boxes;
    r
}

fn frog(sp: &Species, male: bool) -> Rig {
    let s = sp.shape;
    let mass = mass_of(sp, male);
    let mut r = Rig::empty(sp, Frame::Frog, SkinKind::Frog, mass);
    let len = sp.length_m;
    let head_f = s.head.unwrap_or(0.35);
    let legs_f = s.legs.unwrap_or(1.0);
    let bl = len * (1.0 - head_f * 0.8);
    let bw = len * 0.48;
    let bh = len * 0.3;
    r.torso = Vec3::new(bw, bh, bl);
    r.torso_y = bh * 0.75;
    let mut b = Builder { boxes: Vec::new() };
    b.put(
        Slot::Hips,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(bw, bh, bl),
        SkinPart::Body,
    );
    // The head straight off the body's front (no neck to speak of).
    let hl = head_f * len;
    r.neck_base = Vec3::new(0.0, r.torso_y + bh * 0.1, bl * 0.45);
    r.neck_len = 0.0;
    r.head_len = hl;
    r.head_w = bw * 0.95;
    r.skull_len = hl;
    b.put(
        Slot::Head,
        Vec3::new(0.0, 0.0, hl * 0.5),
        Vec3::new(bw * 0.95, bh * 0.7, hl),
        SkinPart::Head,
    );
    // Short arms in front, long legs folded behind.
    let lw = len * 0.09 * legs_f;
    for l in 0..4u8 {
        let fore = l < 2;
        let side = if l % 2 == 0 { 1.0 } else { -1.0 };
        let joint = Vec3::new(
            side * bw * 0.42,
            r.torso_y - bh * 0.2,
            if fore { bl * 0.3 } else { -bl * 0.4 },
        );
        let (upper, lower) = if fore {
            (len * 0.18, len * 0.18)
        } else {
            (len * 0.42 * legs_f, len * 0.42 * legs_f)
        };
        let foot = if fore {
            Vec3::new(lw * 1.4, lw * 0.4, lw * 1.4)
        } else {
            Vec3::new(lw * 1.6, lw * 0.4, len * 0.38)
        };
        r.legs.push(Leg {
            joint,
            upper,
            lower,
            foot,
            fore,
            side,
        });
        let w = if fore { lw * 0.8 } else { lw * 1.3 };
        b.put(
            Slot::Upper(l),
            Vec3::new(0.0, -upper * 0.5, 0.0),
            Vec3::new(w, upper, w),
            SkinPart::UpperLeg,
        );
        b.put(
            Slot::Lower(l),
            Vec3::new(0.0, -lower * 0.5, 0.0),
            Vec3::new(w * 0.8, lower, w * 0.8),
            SkinPart::LowerLeg,
        );
        b.put(
            Slot::Foot(l),
            Vec3::new(0.0, foot.y * 0.5, foot.z * 0.3),
            foot,
            SkinPart::Foot,
        );
    }
    r.boxes = b.boxes;
    r
}

fn insect(sp: &Species, male: bool) -> Rig {
    let s = sp.shape;
    let mass = mass_of(sp, male);
    let mut r = Rig::empty(sp, Frame::Insect, SkinKind::Insect, mass);
    let len = sp.length_m;
    let head_f = s.head.unwrap_or(0.22);
    let w = len * 0.32;
    r.torso = Vec3::new(w, w, len);
    r.torso_y = w * 0.8;
    let mut b = Builder { boxes: Vec::new() };
    // The abdomen behind, the thorax, the head in front; the wings off the thorax.
    b.put(
        Slot::Hips,
        Vec3::new(0.0, 0.0, -len * 0.22),
        Vec3::new(w, w * 0.95, len * 0.48),
        SkinPart::Body,
    );
    b.put(
        Slot::Chest,
        Vec3::new(0.0, 0.0, len * 0.12),
        Vec3::new(w * 0.9, w * 0.9, len * 0.3),
        SkinPart::Hump,
    );
    let hl = head_f * len;
    r.neck_base = Vec3::new(0.0, r.torso_y, len * 0.27);
    r.head_len = hl;
    r.head_w = w * 0.8;
    r.skull_len = hl;
    b.put(
        Slot::Head,
        Vec3::new(0.0, 0.0, hl * 0.5),
        Vec3::new(w * 0.8, w * 0.75, hl),
        SkinPart::Head,
    );
    r.wing = Vec3::new(len * 0.62, 0.0, len * 0.28);
    for side in 0..2u8 {
        b.put(
            Slot::Wing(side, 0),
            Vec3::new(0.0, 0.0, -r.wing.x * 0.5),
            Vec3::new(len * 0.02, r.wing.z, r.wing.x),
            SkinPart::Wing,
        );
    }
    r.boxes = b.boxes;
    r
}

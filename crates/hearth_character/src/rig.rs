//! The body's frame: proportions from anthropometric segment lengths (fractions of stature,
//! after Drillis and Contini), seventeen joints, and the boxes of flesh, hair and cloth on them.
//!
//! The figure's own frame: +Y up, +Z forward (where the face looks), the left side at +X. The
//! feet stand on y = 0 in the rest pose; the root joint is the middle of the hip joints.

use glam::Vec3;

use hearth_content::schema::body::{BodyRegion, ClothingLayer};

use crate::appearance::{Appearance, BodyType, FacialHair, HairStyle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Joint {
    Root,
    Waist,
    Chest,
    Neck,
    Head,
    ShoulderL,
    ElbowL,
    WristL,
    ShoulderR,
    ElbowR,
    WristR,
    HipL,
    KneeL,
    AnkleL,
    HipR,
    KneeR,
    AnkleR,
}

pub const JOINTS: usize = 17;

impl Joint {
    pub const ALL: [Joint; JOINTS] = [
        Joint::Root,
        Joint::Waist,
        Joint::Chest,
        Joint::Neck,
        Joint::Head,
        Joint::ShoulderL,
        Joint::ElbowL,
        Joint::WristL,
        Joint::ShoulderR,
        Joint::ElbowR,
        Joint::WristR,
        Joint::HipL,
        Joint::KneeL,
        Joint::AnkleL,
        Joint::HipR,
        Joint::KneeR,
        Joint::AnkleR,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn parent(self) -> Option<Joint> {
        use Joint::*;
        match self {
            Root => None,
            Waist => Some(Root),
            Chest => Some(Waist),
            Neck | ShoulderL | ShoulderR => Some(Chest),
            Head => Some(Neck),
            ElbowL => Some(ShoulderL),
            WristL => Some(ElbowL),
            ElbowR => Some(ShoulderR),
            WristR => Some(ElbowR),
            HipL | HipR => Some(Root),
            KneeL => Some(HipL),
            AnkleL => Some(KneeL),
            KneeR => Some(HipR),
            AnkleR => Some(KneeR),
        }
    }
}

/// Heights of landmarks as fractions of stature.
pub const EYE: f32 = 0.936;
const TOP: f32 = 1.0;
const CHIN: f32 = 0.870;
const HEAD_JOINT: f32 = 0.880;
const SHOULDER: f32 = 0.818;
const CHEST: f32 = 0.720;
const WAIST: f32 = 0.600;
const HIP: f32 = 0.530;
const CROTCH: f32 = 0.470;
const KNEE: f32 = 0.285;
const ANKLE: f32 = 0.039;

/// Sizes of the body for one appearance (metres).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Proportions {
    pub stature: f32,
    pub upper_arm: f32,
    pub forearm: f32,
    pub hand: f32,
    pub thigh: f32,
    pub shank: f32,
    pub ankle: f32,
    pub hip: f32,
    pub foot_len: f32,
    pub foot_w: f32,
    pub shoulder_w: f32,
    pub chest_w: f32,
    pub chest_d: f32,
    pub waist_w: f32,
    pub waist_d: f32,
    pub hip_w: f32,
    pub pelvis_d: f32,
    pub hip_spacing: f32,
    pub head_w: f32,
    pub head_d: f32,
    pub head_h: f32,
    pub neck_w: f32,
    pub neck_d: f32,
    pub arm_t: f32,
    pub forearm_t: f32,
    pub hand_t: f32,
    pub hand_w: f32,
    pub thigh_t: f32,
    pub shank_t: f32,
    /// Depth of the bust (0 for a male body).
    pub bust_d: f32,
}

impl Proportions {
    pub fn of(a: &Appearance) -> Self {
        let h = a.height_m;
        let female = a.body == BodyType::Female;
        let pick = |m: f32, f: f32| if female { f } else { m };
        // Build thickens flesh, little of the bones; the waist most of all.
        let g = 0.82 + 0.36 * a.build;
        let gw = 0.72 + 0.56 * a.build;
        let bones = 0.96 + 0.08 * a.build;
        Self {
            stature: h,
            upper_arm: 0.186 * h,
            forearm: 0.146 * h,
            hand: 0.108 * h,
            thigh: (HIP - KNEE) * h,
            shank: (KNEE - ANKLE) * h,
            ankle: ANKLE * h,
            hip: HIP * h,
            foot_len: 0.152 * h,
            foot_w: 0.055 * h,
            // Bony shoulder (biacromial) and hip breadths of adult surveys.
            shoulder_w: pick(0.237, 0.224) * h * bones,
            chest_w: pick(0.180, 0.168) * h * g,
            chest_d: pick(0.130, 0.118) * h * g,
            waist_w: pick(0.165, 0.150) * h * gw,
            waist_d: pick(0.118, 0.108) * h * gw,
            hip_w: pick(0.196, 0.220) * h * (0.9 + 0.2 * a.build),
            pelvis_d: pick(0.120, 0.124) * h * g,
            hip_spacing: pick(0.100, 0.110) * h,
            head_w: pick(0.088, 0.086) * h,
            head_d: pick(0.112, 0.109) * h,
            head_h: (TOP - CHIN) * h,
            neck_w: pick(0.068, 0.060) * h * (0.9 + 0.2 * a.build),
            neck_d: pick(0.070, 0.062) * h * (0.9 + 0.2 * a.build),
            arm_t: pick(0.056, 0.050) * h * g,
            forearm_t: pick(0.046, 0.040) * h * g,
            hand_t: pick(0.018, 0.016) * h,
            hand_w: pick(0.050, 0.046) * h,
            // At the top of the thigh and the calf; the limbs taper below.
            thigh_t: pick(0.100, 0.112) * h * g,
            shank_t: pick(0.062, 0.060) * h * g,
            bust_d: if female {
                0.022 * h * (0.8 + 0.4 * a.build)
            } else {
                0.0
            },
        }
    }

    /// Each joint's offset from its parent in the rest pose (the root's from the ground).
    pub fn rest_offsets(&self) -> [Vec3; JOINTS] {
        let h = self.stature;
        let shoulder_x = self.shoulder_w / 2.0 - 0.012 * h;
        let mut o = [Vec3::ZERO; JOINTS];
        let mut set = |j: Joint, v: Vec3| o[j.index()] = v;
        // The hips stand on the legs (a person's at HIP of the stature).
        set(
            Joint::Root,
            Vec3::new(0.0, self.thigh + self.shank + self.ankle, 0.0),
        );
        set(Joint::Waist, Vec3::new(0.0, (WAIST - HIP) * h, 0.0));
        set(Joint::Chest, Vec3::new(0.0, (CHEST - WAIST) * h, 0.0));
        set(
            Joint::Neck,
            Vec3::new(0.0, (SHOULDER - CHEST) * h, -0.012 * h),
        );
        set(
            Joint::Head,
            Vec3::new(0.0, (HEAD_JOINT - SHOULDER) * h, 0.006 * h),
        );
        for (s, sh, el, wr) in [
            (1.0, Joint::ShoulderL, Joint::ElbowL, Joint::WristL),
            (-1.0, Joint::ShoulderR, Joint::ElbowR, Joint::WristR),
        ] {
            set(
                sh,
                Vec3::new(s * shoulder_x, (SHOULDER - CHEST - 0.015) * h, -0.005 * h),
            );
            set(el, Vec3::new(0.0, -self.upper_arm, 0.0));
            set(wr, Vec3::new(0.0, -self.forearm, 0.0));
        }
        for (s, hip, knee, ankle) in [
            (1.0, Joint::HipL, Joint::KneeL, Joint::AnkleL),
            (-1.0, Joint::HipR, Joint::KneeR, Joint::AnkleR),
        ] {
            set(hip, Vec3::new(s * self.hip_spacing / 2.0, 0.0, 0.0));
            set(knee, Vec3::new(0.0, -self.thigh, 0.0));
            set(ankle, Vec3::new(0.0, -self.shank, 0.0));
        }
        o
    }
}

/// What a box is made of, for its colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stuff {
    Skin,
    Lips,
    Hair,
    Brow,
    /// Short stubble or a buzz cut: hair over skin.
    Shadow,
    Sclera,
    Iris,
    Cloth,
    /// A garment's own colour (sRGB).
    Dyed([u8; 3]),
}

/// What a box belongs to, so parts can be hidden (the head in first person).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    Body,
    Head,
}

/// A box on a joint: its centre and size in the joint's frame, and whether its edges are
/// rounded (three overlapping boxes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Part {
    pub joint: Joint,
    pub center: Vec3,
    pub size: Vec3,
    pub stuff: Stuff,
    pub region: Region,
    /// Rounding of the edges (m); 0 for a plain box.
    pub round: f32,
}

/// A person's frame and flesh, built once from their appearance.
#[derive(Debug, Clone, PartialEq)]
pub struct Rig {
    pub dims: Proportions,
    pub rest: [Vec3; JOINTS],
    pub parts: Vec<Part>,
    /// The garments' boxes, over the body's.
    pub clothes: Vec<Part>,
    /// A hood covers the hair.
    pub hooded: bool,
}

impl Rig {
    /// The point between the eyes in the head joint's frame.
    pub fn eye_in_head(&self) -> Vec3 {
        let (c, half) = head_box(&self.dims);
        Vec3::new(0.0, (EYE - HEAD_JOINT) * self.dims.stature, c.z + half.z)
    }

    pub fn new(a: &Appearance) -> Self {
        let a = a.clone().sanitized();
        let dims = Proportions::of(&a);
        let mut parts = Vec::new();
        body(&dims, &mut parts);
        face(&a, &dims, &mut parts);
        hair(&a, &dims, &mut parts);
        Self {
            dims,
            rest: dims.rest_offsets(),
            parts,
            clothes: Vec::new(),
            hooded: false,
        }
    }

    /// Dresses the body in these garments (replacing what it wore).
    pub fn dress(&mut self, garbs: &[Garb]) {
        let mut clothes = Vec::new();
        self.hooded = garments(&self.parts, &self.dims, garbs, &mut clothes);
        self.clothes = clothes;
    }
}

fn add(
    out: &mut Vec<Part>,
    joint: Joint,
    center: Vec3,
    size: Vec3,
    stuff: Stuff,
    region: Region,
    round: f32,
) {
    out.push(Part {
        joint,
        center,
        size,
        stuff,
        region,
        round,
    });
}

fn body(d: &Proportions, out: &mut Vec<Part>) {
    let h = d.stature;
    let v = Vec3::new;
    let b = Region::Body;
    let skin = Stuff::Skin;
    // The trunk: pelvis from the crotch to the waist, abdomen, chest to the top of the
    // shoulders, and the shoulders' slope joining the arms.
    add(
        out,
        Joint::Root,
        v(0.0, ((CROTCH + WAIST) / 2.0 - HIP) * h, 0.0),
        v(d.hip_w, (WAIST - CROTCH + 0.01) * h, d.pelvis_d),
        skin,
        b,
        0.012 * h,
    );
    add(
        out,
        Joint::Waist,
        v(0.0, 0.06 * h, 0.004 * h),
        v(d.waist_w, 0.13 * h, d.waist_d),
        skin,
        b,
        0.014 * h,
    );
    add(
        out,
        Joint::Chest,
        v(0.0, 0.055 * h, -0.004 * h),
        v(d.chest_w, 0.12 * h, d.chest_d),
        skin,
        b,
        0.014 * h,
    );
    add(
        out,
        Joint::Chest,
        v(0.0, 0.088 * h, -0.008 * h),
        v(d.shoulder_w - 0.01 * h, 0.03 * h, d.chest_d * 0.8),
        skin,
        b,
        0.01 * h,
    );
    if d.bust_d > 0.0 {
        add(
            out,
            Joint::Chest,
            v(0.0, 0.032 * h, d.chest_d / 2.0 - 0.004 * h + d.bust_d / 2.0),
            v(d.chest_w * 0.78, 0.05 * h, d.bust_d),
            skin,
            b,
            0.008 * h,
        );
    }
    // The neck goes with the head: looking down in first person shows the chest, not it.
    add(
        out,
        Joint::Neck,
        v(0.0, 0.03 * h, 0.004 * h),
        v(d.neck_w, 0.075 * h, d.neck_d),
        skin,
        Region::Head,
        0.008 * h,
    );
    // The head is the head's region, so the first-person view leaves it out.
    add(
        out,
        Joint::Head,
        v(0.0, 0.055 * h, 0.012 * h),
        v(d.head_w, d.head_h, d.head_d),
        skin,
        Region::Head,
        0.016 * h,
    );
    for (s, sh, el, wr) in [
        (1.0, Joint::ShoulderL, Joint::ElbowL, Joint::WristL),
        (-1.0, Joint::ShoulderR, Joint::ElbowR, Joint::WristR),
    ] {
        limb(
            out,
            sh,
            d.upper_arm,
            0.012 * h,
            d.arm_t * 1.08,
            d.arm_t * 0.9,
            1.04,
            0.0,
        );
        limb(
            out,
            el,
            d.forearm,
            0.005 * h,
            d.forearm_t * 1.05,
            d.forearm_t * 0.78,
            0.9,
            0.002 * h,
        );
        add(
            out,
            wr,
            v(0.0, -0.046 * h, 0.002 * h),
            v(d.hand_t, 0.092 * h, d.hand_w),
            skin,
            b,
            0.004 * h,
        );
        // The thumb, forward of the palm.
        add(
            out,
            wr,
            v(-s * 0.002 * h, -0.03 * h, d.hand_w / 2.0 + 0.004 * h),
            v(0.012 * h, 0.035 * h, 0.01 * h),
            skin,
            b,
            0.0,
        );
    }
    for (hip, knee, ankle) in [
        (Joint::HipL, Joint::KneeL, Joint::AnkleL),
        (Joint::HipR, Joint::KneeR, Joint::AnkleR),
    ] {
        limb(
            out,
            hip,
            d.thigh,
            0.01 * h,
            d.thigh_t,
            d.thigh_t * 0.74,
            1.04,
            0.0,
        );
        limb(
            out,
            knee,
            d.shank,
            0.004 * h,
            d.shank_t,
            d.shank_t * 0.66,
            1.05,
            -0.004 * h,
        );
        // The foot: from the heel, a quarter of its length behind the ankle, to the toes.
        add(
            out,
            ankle,
            v(0.0, -d.ankle / 2.0, d.foot_len * 0.25),
            v(d.foot_w, d.ankle, d.foot_len),
            skin,
            b,
            0.006 * h,
        );
    }
}

/// A limb hanging from `joint` for `len` (plus `over` above the joint), in two boxes: `top`
/// thick at the upper half, `bottom` at the lower, `depth` front to back as a share of the
/// width, set forward by `dz`.
fn limb(
    out: &mut Vec<Part>,
    joint: Joint,
    len: f32,
    over: f32,
    top: f32,
    bottom: f32,
    depth: f32,
    dz: f32,
) {
    let upper = len / 2.0 + over;
    let lower = len / 2.0 + 0.01 * len;
    add(
        out,
        joint,
        Vec3::new(0.0, over - upper / 2.0, dz),
        Vec3::new(top, upper, top * depth),
        Stuff::Skin,
        Region::Body,
        0.12 * top,
    );
    add(
        out,
        joint,
        Vec3::new(0.0, -len + lower / 2.0, dz),
        Vec3::new(bottom, lower, bottom * depth),
        Stuff::Skin,
        Region::Body,
        0.12 * bottom,
    );
}

/// The head's box in the head joint's frame: centre and half sizes.
fn head_box(d: &Proportions) -> (Vec3, Vec3) {
    let h = d.stature;
    (
        Vec3::new(0.0, 0.055 * h, 0.012 * h),
        Vec3::new(d.head_w, d.head_h, d.head_d) / 2.0,
    )
}

fn face(a: &Appearance, d: &Proportions, out: &mut Vec<Part>) {
    let h = d.stature;
    let v = Vec3::new;
    let r = Region::Head;
    let (c, half) = head_box(d);
    let front = c.z + half.z;
    let eye_y = (EYE - HEAD_JOINT) * h;
    for s in [1.0, -1.0] {
        let x = s * 0.018 * h;
        add(
            out,
            Joint::Head,
            v(x, eye_y, front + 0.0006 * h),
            v(0.017 * h, 0.0075 * h, 0.002 * h),
            Stuff::Sclera,
            r,
            0.0,
        );
        add(
            out,
            Joint::Head,
            v(x, eye_y, front + 0.0016 * h),
            v(0.0075 * h, 0.0075 * h, 0.002 * h),
            Stuff::Iris,
            r,
            0.0,
        );
        add(
            out,
            Joint::Head,
            v(s * 0.019 * h, eye_y + 0.011 * h, front + 0.0012 * h),
            v(0.021 * h, 0.0035 * h, 0.003 * h),
            Stuff::Brow,
            r,
            0.0,
        );
        // Ears.
        add(
            out,
            Joint::Head,
            v(s * (half.x + 0.002 * h), eye_y - 0.011 * h, c.z - 0.006 * h),
            v(0.006 * h, 0.034 * h, 0.02 * h),
            Stuff::Skin,
            r,
            0.0,
        );
    }
    // Nose and mouth.
    add(
        out,
        Joint::Head,
        v(0.0, eye_y - 0.016 * h, front + 0.005 * h),
        v(0.013 * h, 0.032 * h, 0.012 * h),
        Stuff::Skin,
        r,
        0.003 * h,
    );
    let mouth_y = 0.014 * h;
    add(
        out,
        Joint::Head,
        v(0.0, mouth_y, front + 0.0008 * h),
        v(0.028 * h, 0.0045 * h, 0.002 * h),
        Stuff::Lips,
        r,
        0.0,
    );
    // Facial hair, over the lower face; the mouth stays in front of it.
    let jaw = |out: &mut Vec<Part>, below: f32, thick: f32, stuff: Stuff| {
        let top = mouth_y + 0.004 * h;
        let bottom = c.y - half.y - below;
        add(
            out,
            Joint::Head,
            v(0.0, (top + bottom) / 2.0, c.z + 0.1 * half.z + thick / 2.0),
            v(d.head_w + 2.0 * thick, top - bottom, half.z * 1.8 + thick),
            stuff,
            r,
            0.006 * h,
        );
    };
    let moustache = |out: &mut Vec<Part>| {
        add(
            out,
            Joint::Head,
            v(0.0, mouth_y + 0.0075 * h, front + 0.003 * h),
            v(0.036 * h, 0.007 * h, 0.004 * h),
            Stuff::Hair,
            r,
            0.0,
        );
    };
    match a.facial_hair {
        FacialHair::None => {}
        FacialHair::Stubble => jaw(out, 0.0, 0.0015 * h, Stuff::Shadow),
        FacialHair::Moustache => moustache(out),
        FacialHair::Goatee => {
            moustache(out);
            add(
                out,
                Joint::Head,
                v(0.0, c.y - half.y + 0.008 * h, front - 0.002 * h),
                v(0.026 * h, 0.03 * h, 0.012 * h),
                Stuff::Hair,
                r,
                0.004 * h,
            );
        }
        FacialHair::ShortBeard => {
            jaw(out, 0.008 * h, 0.005 * h, Stuff::Hair);
            moustache(out);
        }
        FacialHair::FullBeard => {
            jaw(out, 0.045 * h, 0.012 * h, Stuff::Hair);
            moustache(out);
        }
    }
    // Keep the mouth visible in front of a beard.
    if matches!(
        a.facial_hair,
        FacialHair::ShortBeard | FacialHair::FullBeard
    ) && let Some(m) = out.iter_mut().find(|p| p.stuff == Stuff::Lips)
    {
        let thick = if a.facial_hair == FacialHair::FullBeard {
            0.012
        } else {
            0.005
        };
        m.center.z = c.z + 0.1 * half.z + thick * h + (half.z * 1.8 + thick * h) / 2.0 + 0.0005 * h;
    }
}

fn hair(a: &Appearance, d: &Proportions, out: &mut Vec<Part>) {
    let h = d.stature;
    let v = Vec3::new;
    let r = Region::Head;
    let (c, half) = head_box(d);
    let (top, back, front) = (c.y + half.y, c.z - half.z, c.z + half.z);
    let hairline = (EYE - HEAD_JOINT) * h + 0.044 * h;
    let nape = 0.0;
    // A cap of hair `t` thick over the top, the back down to `low`, and the temples.
    let cap = |out: &mut Vec<Part>, t: f32, low: f32, stuff: Stuff, over_ears: bool| {
        let w = d.head_w + 2.0 * t;
        add(
            out,
            Joint::Head,
            v(0.0, top + t / 2.0 - 0.002 * h, c.z - 0.001 * h),
            v(w, t + 0.004 * h, d.head_d + 2.0 * t),
            stuff,
            r,
            (t * 0.8).min(0.012 * h),
        );
        add(
            out,
            Joint::Head,
            v(0.0, (hairline + top) / 2.0, front + t * 0.25),
            v(w, top - hairline, t * 0.5 + 0.001 * h),
            stuff,
            r,
            0.0,
        );
        let depth = d.head_d * 0.42;
        add(
            out,
            Joint::Head,
            v(0.0, (low + top) / 2.0, back - t + depth / 2.0),
            v(w, top - low, depth),
            stuff,
            r,
            (t * 0.8).min(0.012 * h),
        );
        let side_low = if over_ears { low } else { hairline - 0.02 * h };
        for s in [1.0, -1.0] {
            add(
                out,
                Joint::Head,
                v(
                    s * (half.x + t / 2.0),
                    (side_low + top) / 2.0,
                    c.z - half.z * 0.25,
                ),
                v(t, top - side_low, d.head_d * 0.75),
                stuff,
                r,
                0.0,
            );
        }
    };
    // Hair hanging behind, down to `low`, split into `strands` slabs (waves alternate).
    let fall = |out: &mut Vec<Part>, t: f32, low: f32, strands: usize, wave: f32| {
        let w = (d.head_w + 2.0 * t) / strands as f32;
        for i in 0..strands {
            let x = -(d.head_w + 2.0 * t) / 2.0 + w * (i as f32 + 0.5);
            let dz = if i % 2 == 0 { wave } else { -wave };
            add(
                out,
                Joint::Head,
                v(x, (low + nape + 0.02 * h) / 2.0, back - t + 0.006 * h + dz),
                v(w + 0.0005 * h, nape + 0.02 * h - low, 0.012 * h),
                Stuff::Hair,
                r,
                0.0,
            );
        }
    };
    let curtains = |out: &mut Vec<Part>, t: f32, low: f32| {
        for s in [1.0, -1.0] {
            add(
                out,
                Joint::Head,
                v(
                    s * (half.x + t / 2.0),
                    (low + hairline) / 2.0,
                    back + d.head_d * 0.3,
                ),
                v(t, hairline - low, d.head_d * 0.6),
                Stuff::Hair,
                r,
                0.0,
            );
        }
    };
    // A rounded mass of hair `t` thick standing off the head (curls, coils).
    let volume = |out: &mut Vec<Part>, t: f32| {
        let low = nape - 0.015 * h;
        let face_open = front - d.head_d * 0.3;
        let z0 = back - t;
        add(
            out,
            Joint::Head,
            v(0.0, (low + top + t) / 2.0, (z0 + face_open) / 2.0),
            v(d.head_w + 2.0 * t, top + t - low, face_open - z0),
            Stuff::Hair,
            r,
            t * 0.6,
        );
        add(
            out,
            Joint::Head,
            v(
                0.0,
                (hairline + top + t) / 2.0,
                (face_open + front + t * 0.4) / 2.0,
            ),
            v(
                d.head_w + 2.0 * t,
                top + t - hairline,
                front + t * 0.4 - face_open,
            ),
            Stuff::Hair,
            r,
            t * 0.5,
        );
    };
    match a.hair {
        HairStyle::Bald => {}
        HairStyle::Buzzed => cap(out, 0.003 * h, nape + 0.01 * h, Stuff::Shadow, false),
        HairStyle::ShortCrop => cap(out, 0.012 * h, nape + 0.008 * h, Stuff::Hair, false),
        HairStyle::ShoulderLength => {
            cap(out, 0.012 * h, nape, Stuff::Hair, true);
            fall(out, 0.012 * h, -0.065 * h, 1, 0.0);
            curtains(out, 0.012 * h, -0.03 * h);
        }
        HairStyle::LongStraight => {
            cap(out, 0.01 * h, nape, Stuff::Hair, true);
            fall(out, 0.01 * h, -0.21 * h, 1, 0.0);
            curtains(out, 0.01 * h, -0.06 * h);
        }
        HairStyle::LongWavy => {
            cap(out, 0.013 * h, nape, Stuff::Hair, true);
            fall(out, 0.013 * h, -0.2 * h, 5, 0.004 * h);
            curtains(out, 0.013 * h, -0.06 * h);
        }
        HairStyle::Curly => volume(out, 0.025 * h),
        HairStyle::Coily => volume(out, 0.055 * h),
        HairStyle::Braids => {
            cap(out, 0.008 * h, nape, Stuff::Hair, false);
            for i in 0..6 {
                let x = (i as f32 - 2.5) / 2.5 * half.x * 0.75;
                add(
                    out,
                    Joint::Head,
                    v(x, (nape - 0.19 * h) / 2.0, back - 0.012 * h),
                    v(0.011 * h, 0.19 * h + 0.01 * h, 0.011 * h),
                    Stuff::Hair,
                    r,
                    0.0,
                );
            }
        }
        HairStyle::Locs => {
            cap(out, 0.012 * h, nape, Stuff::Hair, true);
            for i in 0..9 {
                let angle = (i as f32 / 8.0 - 0.5) * 2.4;
                let (sx, cz) = angle.sin_cos();
                add(
                    out,
                    Joint::Head,
                    v(
                        sx * (half.x + 0.008 * h),
                        (nape - 0.09 * h) / 2.0 + 0.01 * h,
                        c.z - cz * (half.z + 0.008 * h),
                    ),
                    v(0.015 * h, 0.1 * h + 0.02 * h, 0.015 * h),
                    Stuff::Hair,
                    r,
                    0.0,
                );
            }
        }
        HairStyle::TiedBack => {
            cap(out, 0.008 * h, nape + 0.004 * h, Stuff::Hair, false);
            add(
                out,
                Joint::Head,
                v(0.0, 0.075 * h, back - 0.02 * h),
                v(0.045 * h, 0.045 * h, 0.045 * h),
                Stuff::Hair,
                r,
                0.015 * h,
            );
            add(
                out,
                Joint::Head,
                v(0.0, 0.03 * h, back - 0.03 * h),
                v(0.02 * h, 0.06 * h, 0.02 * h),
                Stuff::Hair,
                r,
                0.005 * h,
            );
        }
    }
}

/// A garment worn, as it looks: which garment, its colour (sRGB), its layer and the regions it
/// covers.
#[derive(Debug, Clone, PartialEq)]
pub struct Garb {
    /// The garment's content id (`hearth:loincloth`).
    pub garment: String,
    pub color: [u8; 3],
    pub layer: ClothingLayer,
    pub regions: Vec<BodyRegion>,
}

/// What each joint's flesh is, as the clothing's regions name it.
fn region_of(j: Joint) -> BodyRegion {
    use Joint::*;
    match j {
        Root => BodyRegion::Pelvis,
        Waist => BodyRegion::Abdomen,
        Chest => BodyRegion::Chest,
        Neck => BodyRegion::Neck,
        Head => BodyRegion::Head,
        ShoulderL | ShoulderR => BodyRegion::UpperArm,
        ElbowL | ElbowR => BodyRegion::LowerArm,
        WristL | WristR => BodyRegion::Hand,
        HipL | HipR => BodyRegion::UpperLeg,
        KneeL | KneeR => BodyRegion::LowerLeg,
        AnkleL | AnkleR => BodyRegion::Foot,
    }
}

/// How thick a layer lies over the skin (a share of stature).
fn layer_thickness(l: ClothingLayer) -> f32 {
    match l {
        ClothingLayer::Under => 0.003,
        ClothingLayer::Main => 0.007,
        ClothingLayer::Outer => 0.012,
        ClothingLayer::Feet | ClothingLayer::Hands => 0.006,
        ClothingLayer::Head => 0.009,
        ClothingLayer::Belt | ClothingLayer::Back => 0.004,
    }
}

/// The boxes of the garments worn, over the body's.
fn garments(body: &[Part], d: &Proportions, garbs: &[Garb], out: &mut Vec<Part>) -> bool {
    let h = d.stature;
    let v = Vec3::new;
    let mut hooded = false;
    for g in garbs {
        let cloth = Stuff::Dyed(g.color);
        let t = layer_thickness(g.layer) * h;
        let name = g.garment.rsplit(':').next().unwrap_or(&g.garment);
        match name {
            "loincloth" => {
                // A tie at the hips, flaps before and behind, the cloth between the legs.
                add(
                    out,
                    Joint::Root,
                    v(0.0, (0.585 - HIP) * h, 0.0),
                    v(d.hip_w + 0.008 * h, 0.02 * h, d.pelvis_d + 0.008 * h),
                    cloth,
                    Region::Body,
                    0.004 * h,
                );
                for (s, w) in [(1.0, 0.075), (-1.0, 0.085)] {
                    add(
                        out,
                        Joint::Root,
                        v(0.0, (0.505 - HIP) * h, s * (d.pelvis_d / 2.0 + 0.003 * h)),
                        v(w * h, 0.13 * h, 0.004 * h),
                        cloth,
                        Region::Body,
                        0.0,
                    );
                }
                add(
                    out,
                    Joint::Root,
                    v(0.0, (CROTCH + 0.008 - HIP) * h, 0.0),
                    v(d.hip_w * 0.55, 0.025 * h, d.pelvis_d * 0.9),
                    cloth,
                    Region::Body,
                    0.004 * h,
                );
            }
            "chest_band" => {
                let front = d.chest_d / 2.0 - 0.004 * h + d.bust_d + 0.003 * h;
                let back = -d.chest_d / 2.0 - 0.007 * h;
                add(
                    out,
                    Joint::Chest,
                    v(0.0, 0.032 * h, (front + back) / 2.0),
                    v(d.chest_w + 0.006 * h, 0.062 * h, front - back),
                    cloth,
                    Region::Body,
                    0.008 * h,
                );
            }
            "belt" => {
                add(
                    out,
                    Joint::Waist,
                    v(0.0, 0.004 * h, 0.004 * h),
                    v(d.waist_w + 0.01 * h, 0.018 * h, d.waist_d + 0.01 * h),
                    cloth,
                    Region::Body,
                    0.005 * h,
                );
            }
            _ => {
                for r in &g.regions {
                    if *r == BodyRegion::Head {
                        // A hood: over the top, the back and the sides, the face left open.
                        hooded = true;
                        let (c, half) = head_box(d);
                        let w = d.head_w + 2.0 * t;
                        add(
                            out,
                            Joint::Head,
                            v(0.0, c.y + half.y + t / 2.0, c.z - 0.002 * h),
                            v(w, t + 0.004 * h, d.head_d + 2.0 * t),
                            cloth,
                            Region::Head,
                            0.01 * h,
                        );
                        add(
                            out,
                            Joint::Head,
                            v(0.0, c.y, c.z - half.z - t / 2.0),
                            v(w, d.head_h + 2.0 * t, t),
                            cloth,
                            Region::Head,
                            0.0,
                        );
                        for side in [1.0, -1.0] {
                            add(
                                out,
                                Joint::Head,
                                v(side * (half.x + t / 2.0), c.y, c.z - half.z * 0.2),
                                v(t, d.head_h + 2.0 * t, d.head_d * 0.8),
                                cloth,
                                Region::Head,
                                0.0,
                            );
                        }
                        continue;
                    }
                    for p in body.iter().filter(|p| {
                        p.stuff == Stuff::Skin
                            && region_of(p.joint) == *r
                            && p.joint != Joint::Head
                            && p.size.min_element() > 0.008 * h
                    }) {
                        // A little longer than the flesh, so neighbouring pieces meet over the
                        // joints, and less rounded.
                        add(
                            out,
                            p.joint,
                            p.center,
                            p.size + Vec3::new(2.0 * t, 2.0 * t + 0.012 * h, 2.0 * t),
                            cloth,
                            p.region,
                            p.round * 0.5,
                        );
                    }
                }
            }
        }
    }
    hooded
}

/// What a new person starts in (as the server dresses them): a loincloth of their chosen
/// material, and a chest band for a female body (D70).
pub fn starting_garbs(a: &Appearance) -> Vec<Garb> {
    let color = a.loincloth.srgb();
    let mut out = vec![Garb {
        garment: "hearth:loincloth".into(),
        color,
        layer: ClothingLayer::Under,
        regions: vec![BodyRegion::Pelvis],
    }];
    if a.body == BodyType::Female {
        out.push(Garb {
            garment: "hearth:chest_band".into(),
            color,
            layer: ClothingLayer::Under,
            regions: vec![BodyRegion::Chest],
        });
    }
    out
}

//! The body's surface (Amendment E §8, E7): a person sculpted from anatomical forms (the
//! ribcage, pelvis and belly, the shoulder girdle and the great muscles, the knees and calves,
//! hands with fingers and a thumb, feet with heels and toes, a face of brow, cheekbones, nose,
//! lips, jaw and ears) as a signed distance field, each form tied to the joint that carries it,
//! smoothly blended, and meshed through its zero crossing (`hearth_smooth`, Surface Nets) into
//! a smooth skin. Each vertex follows the joints of the forms nearest it, weighted by their
//! nearness, so the skin bends where the forms meet.
//!
//! Sizes come from the rig's proportions (`Proportions`, adult surveys) and the build slider:
//! flesh and muscle swell with build, the bones barely. The figure's frame is the rig's: +Y up,
//! +Z forward, the left side at +X, feet on y = 0.

use glam::{Affine3A, IVec3, Quat, Vec3};
use hearth_smooth::{Field, Method, Region};

use crate::appearance::{Appearance, BodyType, Face, FacialHair, HairStyle};
use crate::rig::{JOINTS, Joint, Proportions};

/// What a part of the skin is, for its colour and sheen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tissue {
    Skin,
    Lips,
    /// Finger- and toenails.
    Nail,
}

/// A form's shape in the figure's frame.
#[derive(Debug, Clone, Copy)]
enum Shape {
    /// A cone between two points, rounded at both ends (radii there).
    Cone {
        a: Vec3,
        b: Vec3,
        ra: f32,
        rb: f32,
    },
    Ellipsoid {
        c: Vec3,
        r: Vec3,
    },
}

/// An anatomical form: its shape, the joint that carries it, how softly it blends into what is
/// already there (m), what it is made of, and whether it is cut away (an eye socket).
#[derive(Debug, Clone, Copy)]
struct Form {
    shape: Shape,
    joint: Joint,
    blend: f32,
    tissue: Tissue,
    cut: bool,
    /// From the figure's frame to the shape's: the bind pose's turn of the limb the form is on.
    inv: Affine3A,
}

impl Form {
    fn distance(&self, p: Vec3) -> f32 {
        self.shape.distance(self.inv.transform_point3(p))
    }

    /// A box holding the form, in the figure's frame.
    fn bounds(&self) -> (Vec3, Vec3) {
        let (l, h) = self.shape.bounds();
        let place = self.inv.inverse();
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for k in 0..8 {
            let c = Vec3::new(
                if k & 1 == 0 { l.x } else { h.x },
                if k & 2 == 0 { l.y } else { h.y },
                if k & 4 == 0 { l.z } else { h.z },
            );
            let q = place.transform_point3(c);
            lo = lo.min(q);
            hi = hi.max(q);
        }
        (lo, hi)
    }
}

impl Shape {
    fn distance(&self, p: Vec3) -> f32 {
        match *self {
            Shape::Cone { a, b, ra, rb } => round_cone(p, a, b, ra, rb),
            Shape::Ellipsoid { c, r } => {
                let q = p - c;
                let k0 = (q / r).length();
                let k1 = (q / (r * r)).length();
                if k1 < 1e-9 {
                    -r.min_element()
                } else {
                    k0 * (k0 - 1.0) / k1
                }
            }
        }
    }

    /// A box holding the shape.
    fn bounds(&self) -> (Vec3, Vec3) {
        match *self {
            Shape::Cone { a, b, ra, rb } => {
                let r = ra.max(rb);
                (a.min(b) - Vec3::splat(r), a.max(b) + Vec3::splat(r))
            }
            Shape::Ellipsoid { c, r } => (c - r, c + r),
        }
    }
}

/// The distance to a round cone (Quílez's exact form).
fn round_cone(p: Vec3, a: Vec3, b: Vec3, r1: f32, r2: f32) -> f32 {
    let ba = b - a;
    let l2 = ba.dot(ba);
    if l2 < 1e-12 {
        return (p - a).length() - r1.max(r2);
    }
    let rr = r1 - r2;
    let a2 = l2 - rr * rr;
    let il2 = 1.0 / l2;
    let pa = p - a;
    let y = pa.dot(ba);
    let z = y - l2;
    let x2 = (pa * l2 - ba * y).length_squared();
    let y2 = y * y * l2;
    let z2 = z * z * l2;
    let k = rr.signum() * rr * rr * x2;
    if z.signum() * a2 * z2 > k {
        return (x2 + z2).sqrt() * il2 - r2;
    }
    if y.signum() * a2 * y2 < k {
        return (x2 + y2).sqrt() * il2 - r1;
    }
    ((x2 * a2 * il2).sqrt() + y * rr) * il2 - r1
}

/// The smooth minimum of two distances, blending over `k`.
fn smin(a: f32, b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return a.min(b);
    }
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k * 0.25
}

/// The body's skin, meshed in its rest pose (the bind pose), in the figure's frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Anatomy {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    /// The four joints a vertex follows most, and their weights (summing to one).
    pub joints: Vec<[u8; 4]>,
    pub weights: Vec<[f32; 4]>,
    /// How much the skin there is lips and nail (0–1 each), blended over a cell at their edges,
    /// how thick the short hair over it is (the scalp under the hair, a buzz cut, stubble), and
    /// whether it is not skin but a garment's (1: `garment`).
    pub tissue: Vec<[f32; 4]>,
    /// Counter-clockwise triangles seen from outside.
    pub indices: Vec<u32>,
    /// The joints in the pose the skin was sculpted in (`bind_pose`).
    pub bind: [Affine3A; JOINTS],
}

impl Anatomy {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Adds another mesh in the same bind pose (a garment) to this one.
    pub fn merge(&mut self, other: &Anatomy) {
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&other.positions);
        self.normals.extend_from_slice(&other.normals);
        self.joints.extend_from_slice(&other.joints);
        self.weights.extend_from_slice(&other.weights);
        self.tissue.extend_from_slice(&other.tissue);
        self.indices.extend(other.indices.iter().map(|i| i + base));
    }
}

/// The joints' places in the rest pose, in the figure's frame.
pub fn rest_positions(dims: &Proportions) -> [Vec3; JOINTS] {
    let off = dims.rest_offsets();
    let mut at = [Vec3::ZERO; JOINTS];
    for j in Joint::ALL {
        at[j.index()] = match j.parent() {
            Some(p) => at[p.index()] + off[j.index()],
            None => off[j.index()],
        };
    }
    at
}

/// How far the arms hang out from the sides and the legs stand apart in the pose the skin is
/// sculpted in (the bind pose, radians): enough that hands and thighs, arms and flanks, the two
/// thighs do not blend into one another.
const ARM_OUT: f32 = 0.52;
const LEG_OUT: f32 = 0.1;

/// The joints' places and turns in the bind pose, in the figure's frame: the rest pose with the
/// arms out and the legs apart.
pub fn bind_pose(dims: &Proportions) -> [Affine3A; JOINTS] {
    let off = dims.rest_offsets();
    let turn = |j: Joint| match j {
        Joint::ShoulderL => Quat::from_rotation_z(ARM_OUT),
        Joint::ShoulderR => Quat::from_rotation_z(-ARM_OUT),
        Joint::HipL => Quat::from_rotation_z(LEG_OUT),
        Joint::HipR => Quat::from_rotation_z(-LEG_OUT),
        // The feet stay flat on the ground.
        Joint::AnkleL => Quat::from_rotation_z(-LEG_OUT),
        Joint::AnkleR => Quat::from_rotation_z(LEG_OUT),
        _ => Quat::IDENTITY,
    };
    let mut at = [Affine3A::IDENTITY; JOINTS];
    for j in Joint::ALL {
        let local = Affine3A::from_rotation_translation(turn(j), off[j.index()]);
        at[j.index()] = match j.parent() {
            Some(p) => at[p.index()] * local,
            None => local,
        };
    }
    at
}

/// The forms of a body with this appearance, in the bind pose: each built on the rest pose's
/// straight limbs, then carried with its joint into the bind pose.
fn forms(a: &Appearance, dims: &Proportions) -> Vec<Form> {
    let rest = rest_positions(dims);
    let bind = bind_pose(dims);
    let mut f = rest_forms(a, dims);
    for form in &mut f {
        let i = form.joint.index();
        let place = bind[i] * Affine3A::from_translation(-rest[i]);
        form.inv = place.inverse();
    }
    f
}

/// The forms of a body with this appearance, in the rest pose.
fn rest_forms(a: &Appearance, dims: &Proportions) -> Vec<Form> {
    let h = dims.stature;
    let female = a.body == BodyType::Female;
    let build = a.build.clamp(0.0, 1.0);
    // Muscle and fat: lean bodies show their bones and muscles, heavy ones are rounder.
    let flesh = 0.9 + 0.25 * build;
    let at = rest_positions(dims);
    let j = |x: Joint| at[x.index()];
    let mut f = Vec::new();
    let mut add = |shape: Shape, joint: Joint, blend: f32, tissue: Tissue| {
        f.push(Form {
            shape,
            joint,
            blend,
            tissue,
            cut: false,
            inv: Affine3A::IDENTITY,
        })
    };
    let ell = |c: Vec3, r: Vec3| Shape::Ellipsoid { c, r };
    let cone = |a: Vec3, b: Vec3, ra: f32, rb: f32| Shape::Cone { a, b, ra, rb };
    let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z) * h;
    let skin = Tissue::Skin;
    let soft = 0.022 * h;
    // --- The trunk.
    let root = j(Joint::Root);
    let chest = j(Joint::Chest);
    // Pelvis, belly and ribcage.
    add(
        ell(
            root + v(0.0, 0.012, -0.004),
            Vec3::new(dims.hip_w * 0.47, 0.075 * h, dims.pelvis_d * 0.5),
        ),
        Joint::Root,
        soft,
        skin,
    );
    add(
        ell(
            j(Joint::Waist) + v(0.0, 0.0, 0.004),
            Vec3::new(dims.waist_w * 0.5, 0.088 * h, dims.waist_d * 0.5),
        ),
        Joint::Waist,
        soft,
        skin,
    );
    add(
        ell(
            chest + v(0.0, -0.012, 0.0),
            Vec3::new(dims.chest_w * 0.5, 0.108 * h, dims.chest_d * 0.5),
        ),
        Joint::Chest,
        soft,
        skin,
    );
    // The belly's softness with build, low and in front.
    add(
        ell(
            j(Joint::Waist) + v(0.0, -0.035, dims.waist_d / h * 0.16 + 0.008 * build),
            Vec3::new(dims.waist_w * 0.4, 0.055 * h, (0.02 + 0.03 * build) * h),
        ),
        Joint::Waist,
        soft,
        skin,
    );
    // The shoulder girdle: the clavicles' line and the trapezius over them.
    let (sl, sr) = (j(Joint::ShoulderL), j(Joint::ShoulderR));
    add(
        cone(
            sl + v(-0.014, -0.002, 0.0),
            sr + v(0.014, -0.002, 0.0),
            0.007 * h,
            0.007 * h,
        ),
        Joint::Chest,
        0.025 * h,
        skin,
    );
    add(
        ell(
            j(Joint::Neck) + v(0.0, -0.016, -0.018),
            Vec3::new(dims.shoulder_w * 0.42, 0.03 * h, 0.036 * h),
        ),
        Joint::Chest,
        soft,
        skin,
    );
    // Shoulder blades.
    for s in [1.0, -1.0] {
        add(
            ell(
                chest + v(s * 0.05, 0.03, -dims.chest_d / h * 0.42),
                Vec3::new(0.04 * h, 0.05 * h, 0.018 * h),
            ),
            Joint::Chest,
            soft,
            skin,
        );
    }
    // The chest: pectorals, and breasts on a female body.
    for s in [1.0, -1.0] {
        add(
            ell(
                chest + v(s * 0.046, 0.032, dims.chest_d / h * 0.33),
                Vec3::new(0.044 * h, 0.03 * h, 0.014 * h * flesh),
            ),
            Joint::Chest,
            0.03 * h,
            skin,
        );
        if female {
            add(
                ell(
                    chest + v(s * 0.052, 0.008, dims.chest_d / h * 0.38),
                    Vec3::new(0.036 * h, 0.034 * h, dims.bust_d * 0.8 + 0.012 * h),
                ),
                Joint::Chest,
                0.02 * h,
                skin,
            );
        }
        // Buttocks.
        add(
            ell(
                root + v(s * 0.045, -0.02, -dims.pelvis_d / h * 0.32),
                Vec3::new(0.062 * h, 0.068 * h, 0.048 * h * flesh),
            ),
            Joint::Root,
            0.02 * h,
            skin,
        );
    }
    // --- The neck and head.
    let neck = j(Joint::Neck);
    let head = j(Joint::Head);
    add(
        cone(
            neck + v(0.0, -0.01, -0.004),
            head + v(0.0, -0.004, 0.0),
            dims.neck_w * 0.5,
            dims.neck_w * 0.45,
        ),
        Joint::Neck,
        0.018 * h,
        skin,
    );
    face(&mut f, dims, head, female, build, a.face);
    // --- The arms.
    for (s, sh, el, wr) in [
        (1.0, Joint::ShoulderL, Joint::ElbowL, Joint::WristL),
        (-1.0, Joint::ShoulderR, Joint::ElbowR, Joint::WristR),
    ] {
        let (a0, e0, w0) = (j(sh), j(el), j(wr));
        let mut add = |shape: Shape, joint: Joint, blend: f32, tissue: Tissue| {
            f.push(Form {
                shape,
                joint,
                blend,
                tissue,
                cut: false,
                inv: Affine3A::IDENTITY,
            })
        };
        // The deltoid caps the shoulder.
        add(
            ell(
                a0 + v(s * 0.006, -0.022, 0.0),
                Vec3::new(0.026 * h * flesh, 0.042 * h, 0.03 * h * flesh),
            ),
            sh,
            0.028 * h,
            skin,
        );
        add(
            cone(a0, e0, dims.arm_t * 0.5, dims.forearm_t * 0.47),
            sh,
            0.012 * h,
            skin,
        );
        // Biceps in front, triceps behind.
        add(
            ell(
                a0.lerp(e0, 0.5) + v(0.0, 0.0, 0.008),
                Vec3::new(dims.arm_t * 0.4, 0.06 * h, dims.arm_t * 0.42 * flesh),
            ),
            sh,
            0.012 * h,
            skin,
        );
        add(
            ell(
                a0.lerp(e0, 0.4) + v(0.0, 0.0, -0.01),
                Vec3::new(dims.arm_t * 0.4, 0.065 * h, dims.arm_t * 0.38 * flesh),
            ),
            sh,
            0.012 * h,
            skin,
        );
        // The elbow's point.
        add(
            ell(e0 + v(0.0, 0.0, -0.012), Vec3::splat(0.012 * h)),
            el,
            0.01 * h,
            skin,
        );
        // The forearm, full below the elbow and tapering to the wrist.
        add(
            cone(e0, w0 + v(0.0, 0.006, 0.0), dims.forearm_t * 0.5, 0.017 * h),
            el,
            0.01 * h,
            skin,
        );
        add(
            ell(
                e0.lerp(w0, 0.28) + v(0.0, 0.0, 0.004),
                Vec3::new(
                    dims.forearm_t * 0.47,
                    0.055 * h,
                    dims.forearm_t * 0.5 * flesh,
                ),
            ),
            el,
            0.01 * h,
            skin,
        );
        hand(&mut f, dims, s, w0, wr);
    }
    // --- The legs.
    for (s, hp, kn, an) in [
        (1.0, Joint::HipL, Joint::KneeL, Joint::AnkleL),
        (-1.0, Joint::HipR, Joint::KneeR, Joint::AnkleR),
    ] {
        let (h0, k0, a0) = (j(hp), j(kn), j(an));
        let mut add = |shape: Shape, joint: Joint, blend: f32| {
            f.push(Form {
                shape,
                joint,
                blend,
                tissue: Tissue::Skin,
                cut: false,
                inv: Affine3A::IDENTITY,
            })
        };
        add(
            cone(
                h0 + v(s * 0.008, 0.012, 0.0),
                k0,
                dims.thigh_t * 0.5,
                dims.shank_t * 0.5,
            ),
            hp,
            0.02 * h,
        );
        // Quadriceps in front, the hamstrings behind, the inner thigh.
        add(
            ell(
                h0.lerp(k0, 0.45) + v(0.0, 0.0, 0.016),
                Vec3::new(dims.thigh_t * 0.42, 0.11 * h, dims.thigh_t * 0.36 * flesh),
            ),
            hp,
            0.016 * h,
        );
        add(
            ell(
                h0.lerp(k0, 0.35) + v(0.0, 0.0, -0.016),
                Vec3::new(dims.thigh_t * 0.4, 0.1 * h, dims.thigh_t * 0.33 * flesh),
            ),
            hp,
            0.016 * h,
        );
        // The knee: the kneecap and the joint's breadth.
        add(
            ell(
                k0 + v(0.0, 0.004, 0.022),
                Vec3::new(0.02 * h, 0.022 * h, 0.012 * h),
            ),
            kn,
            0.01 * h,
        );
        add(
            ell(k0, Vec3::new(0.027 * h, 0.026 * h, 0.025 * h)),
            kn,
            0.02 * h,
        );
        // The shank and the calf behind it.
        add(
            cone(k0, a0 + v(0.0, 0.01, 0.0), dims.shank_t * 0.42, 0.021 * h),
            kn,
            0.012 * h,
        );
        add(
            ell(
                k0.lerp(a0, 0.3) + v(0.0, 0.0, -0.016),
                Vec3::new(dims.shank_t * 0.42, 0.075 * h, dims.shank_t * 0.42 * flesh),
            ),
            kn,
            0.014 * h,
        );
        foot(&mut f, dims, s, a0, an);
    }
    f
}

/// The face on the head joint (`head` is its place): the cranium, the face's mass, brow,
/// cheekbones, nose, lips, chin and jaw, ears, and the eye sockets cut in.
fn face(f: &mut Vec<Form>, dims: &Proportions, head: Vec3, female: bool, build: f32, fc: Face) {
    let h = dims.stature;
    // The creator's sliders (−1 to 1) as scales of the features.
    let fc = Face {
        jaw: 1.0 + 0.12 * fc.jaw.clamp(-1.0, 1.0),
        cheekbones: 1.0 + 0.25 * fc.cheekbones.clamp(-1.0, 1.0),
        brow: 1.0 + 0.35 * fc.brow.clamp(-1.0, 1.0),
        nose: 1.0 + 0.18 * fc.nose.clamp(-1.0, 1.0),
        eyes: 1.0 + 0.1 * fc.eyes.clamp(-1.0, 1.0),
        lips: 1.0 + 0.3 * fc.lips.clamp(-1.0, 1.0),
        ears: 1.0 + 0.2 * fc.ears.clamp(-1.0, 1.0),
    };
    let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z) * h;
    let mut add = |shape: Shape, blend: f32, tissue: Tissue, cut: bool| {
        f.push(Form {
            shape,
            joint: Joint::Head,
            blend,
            tissue,
            cut,
            inv: Affine3A::IDENTITY,
        })
    };
    let ell = |c: Vec3, r: Vec3| Shape::Ellipsoid { c, r };
    let skin = Tissue::Skin;
    let jaw = if female { 0.9 } else { 1.0 } * fc.jaw;
    // The cranium.
    add(
        ell(
            head + v(0.0, 0.058, -0.008),
            Vec3::new(dims.head_w * 0.5, 0.062 * h, dims.head_d * 0.5),
        ),
        0.0,
        skin,
        false,
    );
    // The face's mass, from the cheekbones to the jaw.
    add(
        ell(
            head + v(0.0, 0.02, 0.016),
            Vec3::new(dims.head_w * 0.43 * jaw, 0.042 * h, dims.head_d * 0.4),
        ),
        0.012 * h,
        skin,
        false,
    );
    // The jaw's angles and the chin.
    for s in [1.0, -1.0] {
        add(
            Shape::Cone {
                a: head + v(s * 0.034 * jaw, 0.012, -0.004),
                b: head + v(s * 0.008, -0.004, 0.042),
                ra: 0.012 * h,
                rb: 0.01 * h,
            },
            0.01 * h,
            skin,
            false,
        );
        // Cheekbones.
        add(
            ell(
                head + v(s * 0.026, 0.045, 0.028 + 0.004 * (fc.cheekbones - 1.0)),
                Vec3::new(0.015, 0.008, 0.014) * h * fc.cheekbones,
            ),
            0.02 * h,
            skin,
            false,
        );
        // Ears.
        add(
            ell(
                head + v(s * dims.head_w / h * 0.49, 0.05, -0.004),
                Vec3::new(0.006, 0.018, 0.011) * h * fc.ears,
            ),
            0.004 * h,
            skin,
            false,
        );
    }
    add(
        ell(head + v(0.0, -0.006, 0.042), Vec3::splat(0.012 * h * jaw)),
        0.008 * h,
        skin,
        false,
    );
    // The brow ridge, heavier on a male face.
    let brow = if female { 0.005 } else { 0.0075 } * fc.brow;
    add(
        ell(
            head + v(0.0, 0.066, 0.043),
            Vec3::new(0.034 * h, brow * h, 0.007 * h * fc.brow),
        ),
        0.02 * h,
        skin,
        false,
    );
    // The eye sockets, cut in under the brow (the eyes sit in them).
    for s in [1.0, -1.0] {
        add(
            ell(
                head + v(s * 0.017 * fc.eyes, 0.056, 0.057),
                Vec3::new(0.01, 0.007, 0.005) * h * fc.eyes,
            ),
            0.01 * h,
            skin,
            true,
        );
    }
    // The nose: its bridge to the tip, and the nostrils' wings.
    add(
        Shape::Cone {
            a: head + v(0.0, 0.06, 0.05),
            b: head + v(0.0, 0.06 - 0.026 * fc.nose, 0.05 + 0.014 * fc.nose),
            ra: 0.0055 * h,
            rb: 0.0075 * h * fc.nose,
        },
        0.006 * h,
        skin,
        false,
    );
    for s in [1.0, -1.0] {
        add(
            ell(
                head + v(s * 0.0075 * fc.nose, 0.06 - 0.028 * fc.nose, 0.056),
                Vec3::new(0.0062 * h, 0.0052 * h, 0.0062 * h),
            ),
            0.004 * h,
            skin,
            false,
        );
    }
    // The mouth's line between the lips, cut in a little.
    add(
        ell(
            head + v(0.0, 0.0138, 0.0645),
            Vec3::new(0.012 * h, 0.0005 * h, 0.0035 * h),
        ),
        0.002 * h,
        skin,
        true,
    );
    // The lips, fuller with build a little.
    let full = (1.0 + 0.15 * build) * fc.lips;
    add(
        ell(
            head + v(0.0, 0.0172, 0.0565),
            Vec3::new(0.0145 * h, 0.0034 * h * full, 0.0058 * h),
        ),
        0.003 * h,
        Tissue::Lips,
        false,
    );
    add(
        ell(
            head + v(0.0, 0.0102, 0.0545),
            Vec3::new(0.0125 * h, 0.0042 * h * full, 0.006 * h),
        ),
        0.003 * h,
        Tissue::Lips,
        false,
    );
}

/// A hand hanging from the wrist at `w` (palm toward the thigh): the palm, four fingers of
/// three segments each, gently curled, and the thumb before them.
fn hand(f: &mut Vec<Form>, dims: &Proportions, side: f32, w: Vec3, wrist: Joint) {
    let h = dims.stature;
    let len = dims.hand;
    let width = dims.hand_w;
    let mut add = |shape: Shape, blend: f32, tissue: Tissue| {
        f.push(Form {
            shape,
            joint: wrist,
            blend,
            tissue,
            cut: false,
            inv: Affine3A::IDENTITY,
        })
    };
    // The palm: thin across (x), long down (y), wide front to back (z); fuller at the heel of
    // the hand, flatter at the knuckles, and the thumb's mound before it on the palm side.
    for (y, r) in [
        (0.17, Vec3::new(dims.hand_t * 0.5, len * 0.17, width * 0.42)),
        (0.36, Vec3::new(dims.hand_t * 0.42, len * 0.15, width * 0.5)),
    ] {
        add(
            Shape::Ellipsoid {
                c: w + Vec3::new(0.0, -len * y, 0.0),
                r,
            },
            0.008 * h,
            Tissue::Skin,
        );
    }
    add(
        Shape::Ellipsoid {
            c: w + Vec3::new(-side * dims.hand_t * 0.2, -len * 0.2, width * 0.26),
            r: Vec3::new(dims.hand_t * 0.45, len * 0.14, width * 0.2),
        },
        0.008 * h,
        Tissue::Skin,
    );
    // Fingers: index (front) to little (back), the middle longest.
    let lengths = [0.40, 0.44, 0.41, 0.33];
    for (k, l) in lengths.iter().enumerate() {
        let z = width * (0.36 - 0.24 * k as f32);
        let base = w + Vec3::new(0.0, -len * 0.47, z);
        let seg = len * l / 3.0;
        let r = 0.0045 * h * (1.0 - 0.07 * k as f32);
        // Each joint bends a little toward the palm (toward the body, −side in x).
        let mut p = base;
        let mut dir = Vec3::new(0.0, -1.0, 0.0);
        for s in 0..3 {
            dir = (dir + Vec3::new(-side * 0.18, 0.0, 0.0)).normalize();
            let q = p + dir * seg;
            let tip = s == 2;
            add(
                Shape::Cone {
                    a: p,
                    b: q,
                    ra: r * (1.0 - 0.1 * s as f32),
                    rb: r * (0.95 - 0.1 * s as f32),
                },
                0.002 * h,
                Tissue::Skin,
            );
            if tip {
                // The nail on the finger's back (away from the palm).
                add(
                    Shape::Ellipsoid {
                        c: q - dir * seg * 0.3 + Vec3::new(side * r * 0.62, 0.0, 0.0),
                        r: Vec3::new(r * 0.32, seg * 0.36, r * 0.72),
                    },
                    0.0,
                    Tissue::Nail,
                );
            }
            p = q;
        }
    }
    // The thumb, from the palm's front edge, down and forward, turned toward the palm.
    let t0 = w + Vec3::new(-side * 0.004 * h, -len * 0.16, width * 0.42);
    let mut p = t0;
    let mut dir = Vec3::new(-side * 0.25, -0.75, 0.6).normalize();
    for s in 0..2 {
        let q = p + dir * len * 0.2;
        add(
            Shape::Cone {
                a: p,
                b: q,
                ra: 0.0065 * h * (1.0 - 0.12 * s as f32),
                rb: 0.0058 * h * (1.0 - 0.12 * s as f32),
            },
            0.004 * h,
            Tissue::Skin,
        );
        dir = (dir + Vec3::new(-side * 0.2, -0.3, 0.0)).normalize();
        p = q;
    }
}

/// A foot under the ankle at `a`: the heel, the arch to the ball, and five toes.
fn foot(f: &mut Vec<Form>, dims: &Proportions, side: f32, a: Vec3, ankle: Joint) {
    let h = dims.stature;
    let len = dims.foot_len;
    let w = dims.foot_w;
    let mut add = |shape: Shape, blend: f32, tissue: Tissue| {
        f.push(Form {
            shape,
            joint: ankle,
            blend,
            tissue,
            cut: false,
            inv: Affine3A::IDENTITY,
        })
    };
    let ground = |y: f32| y.max(0.0);
    // The ankle bones either side.
    add(
        Shape::Ellipsoid {
            c: a,
            r: Vec3::new(w * 0.42, 0.022 * h, 0.022 * h),
        },
        0.008 * h,
        Tissue::Skin,
    );
    // The heel, a little behind the ankle.
    let heel = Vec3::new(a.x, ground(0.022 * h), a.z - len * 0.18);
    add(
        Shape::Ellipsoid {
            c: heel,
            r: Vec3::new(w * 0.36, 0.022 * h, 0.03 * h),
        },
        0.01 * h,
        Tissue::Skin,
    );
    // The arch to the ball of the foot, splayed a little outward.
    let ball = Vec3::new(a.x + side * w * 0.08, 0.012 * h, a.z + len * 0.55);
    add(
        Shape::Cone {
            a: Vec3::new(a.x, 0.03 * h, a.z),
            b: ball,
            ra: 0.024 * h,
            rb: 0.013 * h,
        },
        0.012 * h,
        Tissue::Skin,
    );
    add(
        Shape::Ellipsoid {
            c: ball,
            r: Vec3::new(w * 0.5, 0.012 * h, 0.022 * h),
        },
        0.008 * h,
        Tissue::Skin,
    );
    // The toes, the big toe on the inner side (toward the other foot).
    let lengths = [0.19, 0.16, 0.14, 0.12, 0.10];
    for (k, l) in lengths.iter().enumerate() {
        let x = ball.x - side * w * (0.36 - 0.18 * k as f32);
        let r = if k == 0 { 0.0085 * h } else { 0.0055 * h };
        let base = Vec3::new(x, 0.009 * h, ball.z + 0.008 * h);
        let tip = base + Vec3::new(0.0, -0.002 * h, len * l);
        add(
            Shape::Cone {
                a: base,
                b: tip,
                ra: r,
                rb: r * 0.9,
            },
            0.003 * h,
            Tissue::Skin,
        );
        add(
            Shape::Ellipsoid {
                c: tip + Vec3::new(0.0, r * 0.55, -r * 0.5),
                r: Vec3::new(r * 0.75, r * 0.3, r * 0.9),
            },
            0.0,
            Tissue::Nail,
        );
    }
}

/// The body's field at a point: the blended forms' distance, and the nearest form.
fn field_at(forms: &[Form], p: Vec3) -> (f32, usize) {
    let mut d = f32::MAX;
    let mut near = (f32::MAX, 0);
    for (i, form) in forms.iter().enumerate() {
        if form.cut {
            continue;
        }
        let di = form.distance(p);
        if di < near.0 {
            near = (di, i);
        }
        d = if d == f32::MAX {
            di
        } else {
            smin(d, di, form.blend)
        };
    }
    // The eye sockets, cut in (smoothly).
    for form in forms.iter().filter(|f| f.cut) {
        let di = form.distance(p);
        d = -smin(-d, di, form.blend);
    }
    (d, near.1)
}

/// The body's field, for what is placed on the skin (hair roots, garments): its distance and
/// the nearest point of the skin.
pub(crate) struct Skin {
    forms: Vec<Form>,
}

impl Skin {
    pub(crate) fn new(a: &Appearance) -> Self {
        let a = a.clone().sanitized();
        let dims = Proportions::of(&a);
        Self {
            forms: forms(&a, &dims),
        }
    }

    /// Only the trunk's skin (no limbs or head): what a garment round the trunk wraps.
    pub(crate) fn trunk(a: &Appearance) -> Self {
        let mut s = Self::new(a);
        s.forms
            .retain(|f| matches!(f.joint, Joint::Root | Joint::Waist | Joint::Chest));
        s
    }

    /// The distance from the skin (negative inside).
    pub(crate) fn distance(&self, p: Vec3) -> f32 {
        field_at(&self.forms, p).0
    }

    /// The skin's outward normal near `p`.
    pub(crate) fn normal(&self, p: Vec3) -> Vec3 {
        let e = 0.001;
        let g = Vec3::new(
            self.distance(p + Vec3::X * e) - self.distance(p - Vec3::X * e),
            self.distance(p + Vec3::Y * e) - self.distance(p - Vec3::Y * e),
            self.distance(p + Vec3::Z * e) - self.distance(p - Vec3::Z * e),
        );
        g.normalize_or(Vec3::Y)
    }

    /// The point of the skin nearest `p`, and its normal there.
    pub(crate) fn project(&self, p: Vec3) -> (Vec3, Vec3) {
        let mut q = p;
        let mut n = Vec3::Y;
        for _ in 0..6 {
            n = self.normal(q);
            let d = self.distance(q);
            q -= n * d;
            if d.abs() < 1e-4 {
                break;
            }
        }
        (q, n)
    }
}

/// How thick the short hair at a point of the skin is (0–1): the scalp within the hairline under
/// any style but bald (a buzz cut thinner), and stubble or the skin under a beard.
fn short_hair(a: &Appearance, dims: &Proportions, p: Vec3) -> f32 {
    let scalp = match a.hair {
        HairStyle::Bald => 0.0,
        HairStyle::Buzzed => 0.75,
        _ => 0.9,
    };
    let (center, radii) = crate::hair::cranium(dims);
    let above = crate::hair::hairline((p - center) / radii);
    let mut d = scalp * (above / 0.08).clamp(0.0, 1.0);
    let beard = match a.facial_hair {
        FacialHair::None => 0.0,
        FacialHair::Stubble => 0.5,
        _ => 0.65,
    };
    let rel = (p - rest_positions(dims)[Joint::Head.index()]) / dims.stature;
    if beard > 0.0 && rel.z > -0.012 && crate::hair::beard_at(a.facial_hair, rel.x, rel.y) {
        d = d.max(beard);
    }
    d
}

/// How much coarser the first look at the field is than the mesh's cells.
const COARSE: usize = 4;

/// The skin of a body of this appearance, meshed with cells of `cell` metres (some 1.2 cm for
/// the close view, coarser for the distant ones).
pub fn anatomy(a: &Appearance, cell: f32) -> Anatomy {
    let a = a.clone().sanitized();
    let dims = Proportions::of(&a);
    let forms = forms(&a, &dims);
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for f in forms.iter().filter(|f| !f.cut) {
        let (l, h) = f.bounds();
        lo = lo.min(l);
        hi = hi.max(h);
    }
    let margin = Vec3::splat(cell * (hearth_smooth::APRON as f32 + 1.0));
    lo -= margin;
    hi += margin;
    let size = ((hi - lo) / cell).ceil().as_ivec3() + IVec3::ONE;
    let size = [size.x as usize, size.y as usize, size.z as usize];
    // The forms each sample can be touched by: the field is evaluated only for forms whose box,
    // grown by their blending and a cell or two, holds the sample.
    let reach: Vec<(Vec3, Vec3)> = forms
        .iter()
        .map(|f| {
            let (l, h) = f.bounds();
            let g = Vec3::splat(f.blend + cell * (2.0 * hearth_smooth::FILL_RANGE + COARSE as f32));
            (l - g, h + g)
        })
        .collect();
    let sample = |p: Vec3, local: &mut Vec<Form>| -> Option<f32> {
        local.clear();
        local.extend(
            forms
                .iter()
                .zip(&reach)
                .filter(|(_, (l, h))| p.cmpge(*l).all() && p.cmple(*h).all())
                .map(|(f, _)| *f),
        );
        if local.iter().all(|f| f.cut) {
            return None;
        }
        Some(field_at(local, p).0)
    };
    let mut local: Vec<Form> = Vec::with_capacity(forms.len());
    // The field first on a grid COARSE times sparser; a fine sample is evaluated only where the
    // nearest coarse one is near enough the skin that the fine one could lie within the fill's
    // range of it (the distances change no faster than distance does, with a margin for the
    // ellipsoids' approximate ones).
    let csize = size.map(|s| s.div_ceil(COARSE) + 1);
    let mut coarse = vec![f32::MAX; csize[0] * csize[1] * csize[2]];
    for y in 0..csize[1] {
        for z in 0..csize[2] {
            for x in 0..csize[0] {
                let p = lo + Vec3::new(x as f32, y as f32, z as f32) * (cell * COARSE as f32);
                if let Some(d) = sample(p, &mut local) {
                    coarse[(y * csize[2] + z) * csize[0] + x] = d;
                }
            }
        }
    }
    let near = cell * (COARSE as f32 * 0.87 + hearth_smooth::FILL_RANGE + 1.0) * 1.5;
    let mut fill = vec![-127i8; size[0] * size[1] * size[2]];
    for y in 0..size[1] {
        for z in 0..size[2] {
            for x in 0..size[0] {
                let c = |i: usize| (i + COARSE / 2) / COARSE;
                let dc = coarse[(c(y) * csize[2] + c(z)) * csize[0] + c(x)];
                let i = (y * size[2] + z) * size[0] + x;
                if dc == f32::MAX || dc > near {
                    continue;
                }
                if dc < -near {
                    fill[i] = 127;
                    continue;
                }
                let p = lo + Vec3::new(x as f32, y as f32, z as f32) * cell;
                if let Some(d) = sample(p, &mut local) {
                    // Positive inside, in cells.
                    fill[i] = hearth_smooth::quantize(-d / cell);
                }
            }
        }
    }
    let material = vec![0u16; fill.len()];
    let field = Field::from_parts(IVec3::ZERO, size, fill, material).expect("the field's arrays");
    let mesh = hearth_smooth::mesh(
        &field,
        Region::interior(&field),
        Method::SurfaceNets,
        &|_: u16| 0.0,
    );
    let mut out = Anatomy {
        indices: mesh.indices.clone(),
        bind: bind_pose(&dims),
        ..Anatomy::default()
    };
    // Each vertex follows the joints of the forms nearest it, by nearness.
    let sigma = 0.012 * dims.stature;
    for p in &mesh.positions {
        // The mesh's place and slope come from the quantized field, which flattens where two
        // surfaces near each other (the creases between forms); the field itself sets both
        // right: two Newton steps onto its zero, and its gradient for the normal.
        let mut world = lo + *p * cell;
        let mut normal = Vec3::Y;
        for _ in 0..3 {
            let Some(d) = sample(world, &mut local) else {
                break;
            };
            let e = cell * 0.25;
            let mut g = Vec3::ZERO;
            for axis in 0..3 {
                let mut o = Vec3::ZERO;
                o[axis] = e;
                let hi = sample(world + o, &mut local).unwrap_or(d + e);
                let lo = sample(world - o, &mut local).unwrap_or(d - e);
                g[axis] = (hi - lo) / (2.0 * e);
            }
            let len = g.length();
            if len < 1e-6 {
                break;
            }
            normal = g / len;
            if (d / len).abs() > cell {
                break;
            }
            world -= normal * (d / len);
        }
        let mut per = [0.0f32; JOINTS];
        // Lips and nails, thin as they are, are what the skin is wherever it lies within a
        // part of a cell of them.
        let mut tissue = [0.0f32; 4];
        tissue[2] = short_hair(&a, &dims, world);
        for form in forms.iter().filter(|f| !f.cut) {
            let d = form.distance(world);
            let k = match form.tissue {
                Tissue::Skin => None,
                Tissue::Lips => Some(0),
                Tissue::Nail => Some(1),
            };
            if let Some(k) = k {
                tissue[k] = tissue[k].max((1.0 - d / cell).clamp(0.0, 1.0));
            }
            per[form.joint.index()] +=
                (-(d.max(0.0)) / sigma).exp() * (1.0 / (1.0 + d.max(0.0) * 50.0));
        }
        let mut ranked: Vec<(usize, f32)> = per.iter().copied().enumerate().collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
        let top: Vec<(usize, f32)> = ranked.into_iter().take(4).filter(|r| r.1 > 0.0).collect();
        let sum: f32 = top.iter().map(|t| t.1).sum::<f32>().max(1e-9);
        let mut joints = [top.first().map_or(0, |t| t.0) as u8; 4];
        let mut weights = [0.0f32; 4];
        for (i, (jn, w)) in top.iter().enumerate() {
            joints[i] = *jn as u8;
            weights[i] = w / sum;
        }
        out.positions.push(world);
        out.normals.push(normal);
        out.joints.push(joints);
        out.weights.push(weights);
        out.tissue.push(tissue);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_is_whole_and_its_size() {
        let a = Appearance::default();
        let t0 = std::time::Instant::now();
        let body = anatomy(&a, 0.006);
        eprintln!(
            "{} vertices, {} triangles in {:.0} ms",
            body.positions.len(),
            body.triangle_count(),
            t0.elapsed().as_secs_f64() * 1e3
        );
        let n = body.positions.len();
        assert!(n > 5_000, "{n} vertices");
        let (lo, hi) = body.positions.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(l, h), p| (l.min(*p), h.max(*p)),
        );
        // Stands on the ground, as tall as the person (the hair adds a little later).
        assert!(lo.y.abs() < 0.03, "feet at {}", lo.y);
        assert!(
            (hi.y - a.height_m).abs() < 0.04,
            "top at {} for {}",
            hi.y,
            a.height_m
        );
        // Weights sum to one; left and right alike.
        for w in &body.weights {
            assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-3);
        }
        assert!((lo.x + hi.x).abs() < 0.02, "symmetric: {} {}", lo.x, hi.x);
        // Lips and nails are there.
        assert!(body.tissue.iter().any(|t| t[0] > 0.9));
        assert!(body.tissue.iter().any(|t| t[1] > 0.9));
        // The scalp is covered; the face is not.
        assert!(body.tissue.iter().any(|t| t[2] > 0.8));
        let face = body
            .positions
            .iter()
            .zip(&body.tissue)
            .filter(|(p, _)| p.z > 0.09 && (p.y - 1.62).abs() < 0.02)
            .all(|(_, t)| t[2] < 0.1);
        assert!(face);
    }
}

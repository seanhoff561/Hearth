//! Coats (V2-7, v2 §7.2): an animal's skin painted from its species' coat recipe — the back,
//! the paler belly and throat (countershading), the points (muzzle, ear rims, tail tip), the
//! legs, the rump patch, the face, and the pattern (spots, stripes, grizzled hair, a mask,
//! speckled or pied feathers, a snake's zig-zag or bands) — onto the unwrapped boxes of its
//! body.
//!
//! Each box is unwrapped as six faces in a cross: the top and bottom in the first row, the
//! four sides in the second (see [`face_rect`]). Every pixel is painted from where it lies on
//! the body at rest, so a pattern runs across the boxes as it would across the animal. The
//! body's frame: x to its left, y up, z forward.

use glam::{Affine3A, Vec3};
use hearth_math::hash::hash_2d;

use crate::paint::{Rgb, Tex, lerp, scale};

/// What a box of the body is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkinPart {
    Body,
    /// A hump over the shoulders, a bird's breast, an insect's thorax.
    Hump,
    Neck,
    Head,
    Snout,
    Jaw,
    Ear,
    UpperLeg,
    LowerLeg,
    Foot,
    Tail,
    Wing,
    Beak,
    Fin,
    Antler,
    Horn,
    Tusk,
}

/// The kind of animal, for where its colours go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinKind {
    Mammal,
    Bird,
    Fish,
    Snake,
    Frog,
    Insect,
}

/// A coat's pattern (as the content's `CoatPattern`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    Plain,
    Spotted,
    Striped,
    Grizzled,
    Masked,
    Speckled,
    Pied,
    ZigZag,
    Banded,
}

/// The colours and pattern of one coat: a species' at a season, of a sex and an age.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoatRecipe {
    pub kind: SkinKind,
    pub back: Rgb,
    pub belly: Rgb,
    pub points: Rgb,
    pub rump: Option<Rgb>,
    pub marking: Option<Rgb>,
    pub face: Option<Rgb>,
    pub legs: Option<Rgb>,
    pub tail_tip: Option<Rgb>,
    pub pattern: Pattern,
    /// Hooves rather than paws.
    pub hooves: bool,
    pub seed: u64,
}

/// A box to paint: where its unwrap lies in the texture, its size in pixels (along x, y, z),
/// where it lies on the body at rest (the unit cube −½..½ to the body's frame) and what it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkinBox {
    pub origin: [u32; 2],
    pub dims: [u32; 3],
    pub rest: Affine3A,
    pub part: SkinPart,
}

/// The size of a box's unwrap in pixels: two sides and two ends across, an end and a side
/// down.
pub fn unwrap_size(d: [u32; 3]) -> [u32; 2] {
    [2 * (d[0] + d[2]), d[2] + d[1]]
}

/// The rectangle of a face in a box's unwrap, from its origin: (x, y, width, height). The
/// faces as the figure shader numbers them: +x, −x, +y, −y, +z, −z.
pub fn face_rect(face: usize, d: [u32; 3]) -> [u32; 4] {
    let [w, h, z] = d;
    match face {
        0 => [z + w, z, z, h],
        1 => [0, z, z, h],
        2 => [z, 0, w, z],
        3 => [z + w, 0, w, z],
        4 => [z, z, w, h],
        _ => [2 * z + w, z, w, h],
    }
}

/// Each face's normal and the edges its pixels run along (u across, v up the image), as the
/// figure shader has them.
pub const FACE_AXES: [[Vec3; 3]; 6] = [
    [Vec3::X, Vec3::NEG_Z, Vec3::Y],
    [Vec3::NEG_X, Vec3::Z, Vec3::Y],
    [Vec3::Y, Vec3::X, Vec3::NEG_Z],
    [Vec3::NEG_Y, Vec3::X, Vec3::Z],
    [Vec3::Z, Vec3::X, Vec3::Y],
    [Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y],
];

/// Where a pixel of a face lies on the unit cube.
pub fn face_point(face: usize, d: [u32; 3], px: u32, py: u32) -> Vec3 {
    let [_, _, fw, fh] = face_rect(face, d);
    let s = (px as f32 + 0.5) / fw.max(1) as f32;
    let t = 1.0 - (py as f32 + 0.5) / fh.max(1) as f32;
    let [n, u, v] = FACE_AXES[face];
    0.5 * (n + (2.0 * s - 1.0) * u + (2.0 * t - 1.0) * v)
}

/// The extent of a set of boxes at rest.
#[derive(Debug, Clone, Copy)]
struct Extent {
    min: Vec3,
    max: Vec3,
}

impl Extent {
    fn of<'a>(boxes: impl Iterator<Item = &'a SkinBox>) -> Option<Self> {
        let mut e: Option<Extent> = None;
        for b in boxes {
            for k in 0..8 {
                let c = Vec3::new(
                    if k & 1 == 0 { -0.5 } else { 0.5 },
                    if k & 2 == 0 { -0.5 } else { 0.5 },
                    if k & 4 == 0 { -0.5 } else { 0.5 },
                );
                let p = b.rest.transform_point3(c);
                e = Some(match e {
                    None => Extent { min: p, max: p },
                    Some(e) => Extent {
                        min: e.min.min(p),
                        max: e.max.max(p),
                    },
                });
            }
        }
        e
    }

    fn size(&self) -> Vec3 {
        self.max - self.min
    }

    /// A point's place within it, 0–1 on each axis.
    fn frac(&self, p: Vec3) -> Vec3 {
        (p - self.min) / self.size().max(Vec3::splat(1e-4))
    }
}

/// What the painter knows of the body about a pixel.
struct Body {
    torso: Extent,
    head: Option<Extent>,
    tail: Option<Extent>,
    /// The size of a pixel on the body (m).
    pixel: f32,
}

/// A deterministic random 0–1 for a point on the body, at a grain of `cell` metres.
fn noise3(seed: u64, p: Vec3, cell: f32) -> f32 {
    let q = (p / cell.max(1e-4)).floor();
    let h = hash_2d(
        seed ^ (q.z as i64 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15),
        q.x as i32,
        q.y as i32,
    );
    (h >> 40) as f32 / (1u64 << 24) as f32
}

const HOOF: Rgb = [42, 36, 32];
const EYE: Rgb = [16, 14, 12];
const NOSE: Rgb = [28, 24, 22];
const ANTLER_BASE: Rgb = [112, 88, 62];
const ANTLER_TIP: Rgb = [232, 222, 196];
const HORN: Rgb = [216, 206, 186];
const HORN_TIP: Rgb = [40, 36, 32];
const TUSK: Rgb = [236, 228, 206];
const WINGS: Rgb = [214, 218, 222];

/// Paints the boxes of a body into `tex` with a coat.
pub fn paint_coat(tex: &mut Tex, boxes: &[SkinBox], coat: &CoatRecipe) {
    let torso = Extent::of(
        boxes
            .iter()
            .filter(|b| matches!(b.part, SkinPart::Body | SkinPart::Hump)),
    )
    .or_else(|| Extent::of(boxes.iter()));
    let Some(torso) = torso else {
        return;
    };
    let pixel = boxes
        .iter()
        .find(|b| b.part == SkinPart::Body)
        .or(boxes.first())
        .map(|b| {
            let len = b.rest.matrix3.z_axis.length();
            len / b.dims[2].max(1) as f32
        })
        .unwrap_or(0.0625);
    let body = Body {
        torso,
        head: Extent::of(
            boxes
                .iter()
                .filter(|b| matches!(b.part, SkinPart::Head | SkinPart::Snout)),
        ),
        tail: Extent::of(boxes.iter().filter(|b| b.part == SkinPart::Tail)),
        pixel,
    };
    for b in boxes {
        let normal_of = |n: Vec3| b.rest.matrix3.mul_vec3(n).normalize_or_zero();
        for (face, axes) in FACE_AXES.iter().enumerate() {
            let [fx, fy, fw, fh] = face_rect(face, b.dims);
            let n = normal_of(axes[0]);
            for py in 0..fh {
                for px in 0..fw {
                    let local = face_point(face, b.dims, px, py);
                    let p = b.rest.transform_point3(local);
                    let c = color_at(coat, &body, b, local, p, n);
                    // A little grain in every hair, feather and scale.
                    let g = hash_2d(
                        coat.seed ^ 0x6a09,
                        (b.origin[0] + fx + px) as i32,
                        (b.origin[1] + fy + py) as i32,
                    );
                    let grain = 0.94 + 0.12 * ((g >> 40) as f32 / (1u64 << 24) as f32);
                    tex.set(
                        (b.origin[0] + fx + px) as i32,
                        (b.origin[1] + fy + py) as i32,
                        scale(c, grain),
                    );
                }
            }
        }
    }
}

/// The colour of the body at a point of a box (`local` on its unit cube, `p` on the body, `n`
/// the face's normal on the body).
fn color_at(coat: &CoatRecipe, body: &Body, b: &SkinBox, local: Vec3, p: Vec3, n: Vec3) -> Rgb {
    match coat.kind {
        SkinKind::Mammal => mammal(coat, body, b, local, p, n),
        SkinKind::Bird => bird(coat, body, b, local, p, n),
        SkinKind::Fish => fish(coat, body, b, p, n),
        SkinKind::Snake => snake(coat, body, b, p, n),
        SkinKind::Frog => frog(coat, body, b, local, p, n),
        SkinKind::Insect => insect(coat, body, b, p),
    }
}

/// The eyes: a dark pixel on each side of the head, toward its front and top.
fn eye(b: &SkinBox, local: Vec3, n: Vec3, height: f32, ahead: f32) -> bool {
    b.part == SkinPart::Head
        && n.x.abs() > 0.7
        && (local.y - height).abs() < 0.5 / b.dims[1].max(1) as f32 + 0.02
        && (local.z - ahead).abs() < 0.5 / b.dims[2].max(1) as f32 + 0.02
}

/// The pattern over the back and flanks (`h` the height on the torso, 0 the belly line).
fn pattern(coat: &CoatRecipe, body: &Body, p: Vec3, h: f32, base: Rgb) -> Rgb {
    let mark = coat.marking.unwrap_or_else(|| scale(coat.back, 0.55));
    let px = body.pixel;
    match coat.pattern {
        CoatPattern::Spotted if h > 0.3 => {
            // Spots on the back and flanks, a pixel or two across.
            let r = noise3(coat.seed ^ 0x5b07, p, px * 1.0);
            let near_spine = (p.x.abs() / (body.torso.size().x * 0.5).max(1e-3)) < 0.75;
            let density = if near_spine { 0.24 } else { 0.16 };
            if r < density { mark } else { base }
        }
        CoatPattern::Striped if h > 0.3 => {
            // Pale stripes along the body.
            let row = ((p.y - body.torso.min.y) / px).floor() as i32;
            if row.rem_euclid(3) == 0 { mark } else { base }
        }
        CoatPattern::Grizzled => {
            // Dark and pale tips mixed through the hair.
            let r = noise3(coat.seed ^ 0x9e11, p, px);
            if r < 0.12 {
                coat.marking.unwrap_or_else(|| scale(base, 0.62))
            } else if r > 0.88 {
                lerp(base, coat.belly, 0.45)
            } else {
                scale(base, 0.88 + 0.24 * noise3(coat.seed ^ 0x77, p, px))
            }
        }
        CoatPattern::Speckled => {
            let r = noise3(coat.seed ^ 0x3c6e, p, px);
            if r < 0.2 {
                mark
            } else if r > 0.9 {
                lerp(base, coat.belly, 0.5)
            } else {
                base
            }
        }
        CoatPattern::Banded => {
            // Dark bars across the body.
            let along = (p.z - body.torso.min.z) / (px * 3.0);
            if along.rem_euclid(1.0) < 0.34 {
                mark
            } else {
                base
            }
        }
        _ => base,
    }
}

use Pattern as CoatPattern;

fn mammal(coat: &CoatRecipe, body: &Body, b: &SkinBox, local: Vec3, p: Vec3, n: Vec3) -> Rgb {
    let t = body.torso;
    let h = t.frac(p).y;
    let back = coat.back;
    let rough = noise3(coat.seed ^ 0x1d, p, body.pixel) * 0.08;
    match b.part {
        SkinPart::Body | SkinPart::Hump => {
            // Countershading: the belly beneath and up the lower flanks.
            if n.y < -0.5 || (n.y.abs() < 0.5 && h < 0.26 + rough) {
                return coat.belly;
            }
            // The rump patch about the tail.
            if let Some(rump) = coat.rump {
                let behind = (p.z - t.min.z) / t.size().z.max(1e-3);
                if (n.z < -0.5 && h > 0.25) || (behind < 0.08 && h > 0.45) {
                    return rump;
                }
            }
            pattern(coat, body, p, h, back)
        }
        SkinPart::Neck => {
            if n.y < -0.5 || local.y < -0.25 {
                coat.belly
            } else {
                pattern(coat, body, p, h.max(0.5), back)
            }
        }
        SkinPart::Head | SkinPart::Snout | SkinPart::Jaw => head(coat, body, b, local, p, n),
        SkinPart::Ear => {
            if n.z > 0.5 {
                // The inside of the ear.
                lerp(coat.belly, back, 0.25)
            } else if local.y > 0.3 {
                coat.points
            } else {
                back
            }
        }
        SkinPart::UpperLeg => {
            // Paler inside, and darkening toward the lower leg.
            let inside = n.x * p.x.signum() < -0.5;
            let base = if inside {
                lerp(back, coat.belly, 0.6)
            } else {
                back
            };
            match coat.legs {
                Some(l) if local.y < -0.1 => lerp(base, l, 0.6),
                _ => base,
            }
        }
        SkinPart::LowerLeg => coat.legs.unwrap_or_else(|| scale(back, 0.9)),
        SkinPart::Foot => {
            if coat.hooves {
                HOOF
            } else {
                coat.legs.unwrap_or_else(|| scale(back, 0.85))
            }
        }
        SkinPart::Tail => {
            let tip = coat.tail_tip.unwrap_or(coat.points);
            let Some(tail) = body.tail else {
                return back;
            };
            // How far along the tail (it runs back from the rump), and whether it is short
            // enough to be all tip.
            let along = 1.0 - tail.frac(p).z;
            let short = tail.size().z < t.size().z * 0.12;
            if short && coat.tail_tip.is_some() {
                return tip;
            }
            if along > 0.75 {
                tip
            } else if n.y < -0.5 {
                coat.rump.unwrap_or(coat.belly)
            } else {
                pattern(coat, body, p, 0.8, back)
            }
        }
        SkinPart::Antler => {
            let up = body.head.map_or(0.5, |hd| {
                ((p.y - hd.max.y) / (hd.size().y * 3.0).max(1e-3)).clamp(0.0, 1.0)
            });
            lerp(ANTLER_BASE, ANTLER_TIP, up)
        }
        SkinPart::Horn => {
            let from = body.head.map_or(0.5, |hd| {
                ((p - (hd.min + hd.max) * 0.5).length() / (hd.size().length() * 1.5).max(1e-3))
                    .clamp(0.0, 1.0)
            });
            if from > 0.8 { HORN_TIP } else { HORN }
        }
        SkinPart::Tusk => TUSK,
        SkinPart::Wing | SkinPart::Beak | SkinPart::Fin => back,
    }
}

/// A mammal's head: the face's colour, the muzzle, the mask, the eyes and the nose.
fn head(coat: &CoatRecipe, body: &Body, b: &SkinBox, local: Vec3, p: Vec3, n: Vec3) -> Rgb {
    let face = coat.face.unwrap_or(coat.back);
    if b.part == SkinPart::Head && eye(b, local, n, 0.18, 0.2) {
        return EYE;
    }
    if b.part == SkinPart::Snout && n.z > 0.5 && local.y > 0.05 && local.x.abs() < 0.3 {
        return NOSE;
    }
    if n.y < -0.5 || b.part == SkinPart::Jaw {
        return lerp(coat.belly, coat.points, 0.3);
    }
    if coat.pattern == CoatPattern::Masked {
        // Dark stripes from the nose over the eyes to the ears, on a pale face.
        let x = local.x.abs();
        let stripe = (0.1..0.36).contains(&x) && (n.y > 0.5 || local.y > -0.05);
        return if stripe {
            coat.marking.unwrap_or(EYE)
        } else {
            face
        };
    }
    if b.part == SkinPart::Snout {
        // The muzzle paling or darkening toward its end.
        let tip = local.z > 0.0;
        return if tip {
            coat.points
        } else {
            lerp(face, coat.points, 0.4)
        };
    }
    let _ = (body, p);
    face
}

fn bird(coat: &CoatRecipe, body: &Body, b: &SkinBox, local: Vec3, p: Vec3, n: Vec3) -> Rgb {
    let t = body.torso;
    let f = t.frac(p);
    let back = coat.back;
    match b.part {
        SkinPart::Body | SkinPart::Hump | SkinPart::Neck => {
            let under = n.y < -0.5 || (n.y.abs() < 0.5 && f.y < 0.4);
            // The breast: the front of the underside.
            let breast = f.z > 0.55 && f.y < 0.75 && (n.y < 0.5) && n.z > -0.5;
            match coat.pattern {
                CoatPattern::Pied => {
                    // White patches on the shoulders, red under the tail.
                    if f.z < 0.2 && f.y < 0.35 {
                        return coat.marking.unwrap_or(back);
                    }
                    if !under && f.z > 0.45 && f.y > 0.45 && n.x.abs() > 0.5 {
                        return coat.belly;
                    }
                }
                CoatPattern::Masked if breast && f.z > 0.8 => {
                    return coat.marking.unwrap_or(EYE);
                }
                CoatPattern::Plain if breast => {
                    if let Some(m) = coat.marking {
                        return m;
                    }
                }
                _ => {}
            }
            if under {
                if coat.pattern == CoatPattern::Banded {
                    // Bars across the breast and belly.
                    let row = ((p.y - t.min.y) / body.pixel).floor() as i32;
                    return if row.rem_euclid(2) == 0 {
                        coat.marking.unwrap_or(back)
                    } else {
                        coat.belly
                    };
                }
                return coat.belly;
            }
            pattern(coat, body, p, 0.8, back)
        }
        SkinPart::Head => {
            if eye(b, local, n, 0.12, 0.18) {
                return EYE;
            }
            let face = coat.face.unwrap_or(back);
            match coat.pattern {
                // A black cap and bib on white cheeks.
                CoatPattern::Masked => {
                    if local.y > 0.1 || n.y > 0.5 || local.y < -0.3 {
                        coat.marking.unwrap_or(EYE)
                    } else {
                        coat.belly
                    }
                }
                // White cheeks, a red nape.
                CoatPattern::Pied => {
                    if local.z < -0.3 && local.y > 0.1 {
                        coat.marking.unwrap_or(back)
                    } else if n.x.abs() > 0.5 && local.y < 0.15 {
                        coat.belly
                    } else {
                        back
                    }
                }
                _ => {
                    if n.y < -0.5 {
                        lerp(face, coat.belly, 0.6)
                    } else {
                        face
                    }
                }
            }
        }
        SkinPart::Beak | SkinPart::Snout | SkinPart::Jaw => coat.points,
        SkinPart::Wing => {
            // Flight feathers darker toward the wing's trailing edge and tip.
            let dark = local.z < -0.2 || local.x.abs() > 0.3;
            let base = pattern(coat, body, p, 0.8, back);
            if dark { scale(base, 0.72) } else { base }
        }
        SkinPart::Tail => {
            if n.y < -0.5 {
                if coat.pattern == CoatPattern::Pied {
                    return coat.marking.unwrap_or(back);
                }
                return lerp(coat.belly, back, 0.5);
            }
            scale(pattern(coat, body, p, 0.8, back), 0.85)
        }
        SkinPart::UpperLeg => lerp(coat.belly, back, 0.3),
        SkinPart::LowerLeg | SkinPart::Foot => scale(coat.points, 0.8),
        _ => back,
    }
}

fn fish(coat: &CoatRecipe, body: &Body, b: &SkinBox, p: Vec3, n: Vec3) -> Rgb {
    let t = body.torso;
    let h = t.frac(p).y;
    match b.part {
        SkinPart::Fin | SkinPart::Tail => coat.points,
        _ => {
            if b.part == SkinPart::Head && n.x.abs() > 0.7 && h > 0.55 && t.frac(p).z > 0.85 {
                return EYE;
            }
            let side = lerp(coat.belly, coat.back, ((h - 0.25) / 0.45).clamp(0.0, 1.0));
            let base = if n.y > 0.5 {
                coat.back
            } else if n.y < -0.5 {
                coat.belly
            } else {
                side
            };
            match coat.pattern {
                CoatPattern::Spotted if h > 0.3 => {
                    let r = noise3(coat.seed ^ 0x51, p, body.pixel);
                    if r < 0.1 {
                        coat.marking.unwrap_or(EYE)
                    } else if r < 0.2 {
                        scale(coat.back, 0.45)
                    } else {
                        base
                    }
                }
                CoatPattern::Banded if h > 0.3 => {
                    // Dark bars down the flanks.
                    let along = (p.z - t.min.z) / (t.size().z / 7.0).max(1e-4);
                    if along.rem_euclid(1.0) < 0.35 {
                        coat.marking.unwrap_or(EYE)
                    } else {
                        base
                    }
                }
                _ => base,
            }
        }
    }
}

fn snake(coat: &CoatRecipe, body: &Body, b: &SkinBox, p: Vec3, n: Vec3) -> Rgb {
    let t = body.torso;
    let along = t.frac(p).z;
    if n.y < -0.5 {
        return coat.belly;
    }
    if b.part == SkinPart::Head && n.x.abs() > 0.7 && along > 0.97 {
        return EYE;
    }
    // The tail's end.
    if along < 0.08 {
        return coat.points;
    }
    let mark = coat.marking.unwrap_or(EYE);
    let px = body.pixel;
    match coat.pattern {
        CoatPattern::ZigZag => {
            // A dark zig-zag down the back.
            let wave = ((p.z / (px * 4.0)).rem_euclid(1.0) - 0.5).abs() * 4.0 - 1.0;
            let x = wave * (t.size().x * 0.3);
            if n.y > 0.5 && (p.x - x).abs() < px * 0.5 {
                mark
            } else {
                coat.back
            }
        }
        CoatPattern::Banded => {
            if (p.z / (px * 4.0)).rem_euclid(1.0) < 0.4 {
                mark
            } else {
                coat.back
            }
        }
        _ => coat.back,
    }
}

fn frog(coat: &CoatRecipe, body: &Body, b: &SkinBox, local: Vec3, p: Vec3, n: Vec3) -> Rgb {
    if b.part == SkinPart::Head && eye(b, local, n, 0.3, 0.1) {
        return EYE;
    }
    if n.y < -0.5 {
        return coat.belly;
    }
    let mark = coat.marking.unwrap_or(EYE);
    match coat.pattern {
        // A dark mask from the snout through the eye.
        CoatPattern::Masked if b.part == SkinPart::Head && n.x.abs() > 0.5 && local.y < 0.3 => mark,
        CoatPattern::Spotted => {
            if noise3(coat.seed ^ 0x2f, p, body.pixel) < 0.18 {
                mark
            } else {
                coat.back
            }
        }
        _ => {
            // Bars across the legs.
            if matches!(b.part, SkinPart::UpperLeg | SkinPart::LowerLeg)
                && (local.z * 4.0).rem_euclid(1.0) < 0.3
            {
                mark
            } else {
                coat.back
            }
        }
    }
}

fn insect(coat: &CoatRecipe, body: &Body, b: &SkinBox, p: Vec3) -> Rgb {
    match b.part {
        SkinPart::Wing => WINGS,
        SkinPart::Head => coat.points,
        SkinPart::Hump => lerp(coat.back, coat.points, 0.5),
        _ => {
            let t = body.torso;
            let along = (p.z - t.min.z) / (t.size().z / 5.0).max(1e-5);
            if coat.pattern == CoatPattern::Banded && along.rem_euclid(1.0) < 0.4 {
                coat.marking.unwrap_or(coat.points)
            } else {
                coat.back
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deer_coat() -> CoatRecipe {
        CoatRecipe {
            kind: SkinKind::Mammal,
            back: [139, 74, 43],
            belly: [196, 171, 136],
            points: [58, 42, 32],
            rump: Some([214, 192, 149]),
            marking: None,
            face: None,
            legs: None,
            tail_tip: None,
            pattern: Pattern::Plain,
            hooves: true,
            seed: 7,
        }
    }

    #[test]
    fn faces_tile_the_unwrap_and_points_lie_on_the_cube() {
        let d = [5, 3, 7];
        let [w, h] = unwrap_size(d);
        let mut seen = vec![0u8; (w * h) as usize];
        for (f, axes) in FACE_AXES.iter().enumerate() {
            let [x, y, fw, fh] = face_rect(f, d);
            for py in 0..fh {
                for px in 0..fw {
                    seen[((y + py) * w + x + px) as usize] += 1;
                    let p = face_point(f, d, px, py);
                    let n = axes[0];
                    assert!(
                        (p.dot(n) - 0.5).abs() < 1e-5,
                        "face {f} pixel off its plane"
                    );
                    assert!(p.abs().max_element() <= 0.5 + 1e-5);
                }
            }
        }
        // Six faces, no overlaps; the two corners of the top row stay empty.
        assert!(seen.iter().all(|&s| s <= 1));
        let painted = seen.iter().filter(|&&s| s == 1).count() as u32;
        assert_eq!(painted, 2 * (5 * 3 + 3 * 7 + 5 * 7));
    }

    #[test]
    fn a_torso_is_dark_above_and_pale_below() {
        // A torso 1.3 m long, 0.5 m deep, 0.4 m wide at 16 pixels a metre.
        let rest = Affine3A::from_scale_rotation_translation(
            Vec3::new(0.4, 0.5, 1.3),
            glam::Quat::IDENTITY,
            Vec3::new(0.0, 1.0, 0.0),
        );
        let b = SkinBox {
            origin: [0, 0],
            dims: [6, 8, 21],
            rest,
            part: SkinPart::Body,
        };
        let [w, h] = unwrap_size(b.dims);
        let mut tex = Tex::new(w, h);
        let coat = deer_coat();
        paint_coat(&mut tex, &[b], &coat);
        let near = |c: [u8; 4], to: Rgb| {
            (0..3).all(|k| (c[k] as i32 - to[k] as i32).abs() <= (to[k] as i32 / 10 + 3))
        };
        // The top face is the back, the bottom the belly.
        let [tx, ty, tw, th] = face_rect(2, b.dims);
        assert!(near(
            tex.get((tx + tw / 2) as i32, (ty + th / 2) as i32),
            coat.back
        ));
        let [bx, by, bw, bh] = face_rect(3, b.dims);
        assert!(near(
            tex.get((bx + bw / 2) as i32, (by + bh / 2) as i32),
            coat.belly
        ));
        // The rear end is the rump patch.
        let [rx, ry, rw, rh] = face_rect(5, b.dims);
        assert!(near(
            tex.get((rx + rw / 2) as i32, (ry + rh / 3) as i32),
            coat.rump.unwrap()
        ));
        // The same every time.
        let mut again = Tex::new(w, h);
        paint_coat(&mut again, &[b], &coat);
        assert_eq!(tex, again);
    }
}

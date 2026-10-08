//! Hair as cards (Amendment E §8, E7): many narrow strips, each painted with strands by the
//! shader, laid over the scalp in layers and grown along a path from its root: off the skin,
//! along the style's flow, bending under its weight and kept off the head and body by the body's
//! own field (`anatomy::Skin`). Every creator style is built this way: flat cards for short and
//! flowing hair, shells of outward cards for coily hair, crossed cards for braids, locs and a
//! tail; and the brows and facial hair the same, short and lying close.
//!
//! A few guide strands (`HairSim`) carry the cards: each card follows its nearest guide, which
//! swings with the head's motion, gravity and wind, and settles back to the style.

use glam::{Affine3A, Vec3};

use crate::anatomy::{Skin, rest_positions};
use crate::appearance::{Appearance, FacialHair, HairStyle};
use crate::rig::{Joint, Proportions};

/// How the shader paints a card's strands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Strands {
    Straight = 0,
    Wavy = 1,
    /// Tightly coiled: a fuzz of short kinks.
    Kinky = 2,
    Braid = 3,
    Loc = 4,
    Brow = 5,
    Beard = 6,
}

/// The guide index of cards no guide swings.
pub const NO_GUIDE: u8 = 255;

/// Guide strands a head of hair at most, and the points along each.
pub const GUIDES: usize = 32;
pub const GUIDE_POINTS: usize = 5;

/// A head of hair's cards, in the body's bind frame (they ride the head joint).
#[derive(Debug, Clone, Default)]
pub struct HairMesh {
    pub positions: Vec<Vec3>,
    /// Along the strands, root to tip.
    pub tangents: Vec<Vec3>,
    /// The card's face, away from the body.
    pub normals: Vec<Vec3>,
    /// Across the card (0–1) and along it, root to tip (0–1).
    pub uv: Vec<[f32; 2]>,
    /// The guide a vertex follows, how the strands look (`Strands`), how deep in the hair it lies
    /// (0 the innermost layer, 255 the outermost) and how far along its guide it is (0–255).
    pub info: Vec<[u8; 4]>,
    pub indices: Vec<u32>,
    /// The guides' rest paths, root first.
    pub guides: Vec<[Vec3; GUIDE_POINTS]>,
    /// How firmly each guide holds the style (0 loose – 1 stiff).
    pub stiffness: Vec<f32>,
}

impl HairMesh {
    pub fn card_count(&self) -> usize {
        self.indices.len() / 6 / CARD_SEGMENTS
    }
}

/// Segments along a card.
const CARD_SEGMENTS: usize = 6;

/// A card before it is cut into triangles.
struct Card {
    points: Vec<Vec3>,
    normals: Vec<Vec3>,
    width: f32,
    strands: Strands,
    depth: f32,
    /// Crossed with a second card at right angles (ropes: braids, locs, a tail).
    rope: bool,
}

impl Card {
    /// Whether the guides swing it (the head's hair; brows and beards stay put).
    fn swings(&self) -> bool {
        !matches!(self.strands, Strands::Brow | Strands::Beard)
    }
}

/// A small, fixed random sequence (SplitMix64), so a person's hair is the same every time.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 40) as f32 / (1u64 << 24) as f32
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }
}

/// The hair's makings: the skin to keep off, the scalp's ellipsoid, the head's size.
struct Builder<'a> {
    skin: &'a Skin,
    /// The head joint's place.
    head: Vec3,
    /// The cranium's centre and radii.
    center: Vec3,
    radii: Vec3,
    h: f32,
    rng: Rng,
    cards: Vec<Card>,
    /// Whether the hair is kept out of the face's front (the head's hair, not the brows and
    /// beard that grow there).
    clear_face: bool,
}

impl Builder<'_> {
    /// A point in the cranium's own measure (the unit sphere is its surface).
    fn local(&self, p: Vec3) -> Vec3 {
        (p - self.center) / self.radii
    }

    /// Whether hair grows here on the scalp (`l` in the cranium's measure): above a hairline
    /// high at the forehead, receding at the temples, over the ears and low at the nape.
    fn on_scalp(l: Vec3) -> bool {
        hairline(l) > 0.0
    }

    /// Roots over the scalp: `n` points spread evenly (a Fibonacci spiral over the cranium),
    /// each set on the skin, with its normal.
    fn scalp_roots(&mut self, n: usize) -> Vec<(Vec3, Vec3)> {
        let mut out = Vec::new();
        // Over-sample the whole sphere; the hairline keeps some 45%.
        let total = (n as f32 / 0.45) as usize;
        let golden = std::f32::consts::PI * (3.0 - 5f32.sqrt());
        for i in 0..total {
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / total as f32;
            let r = (1.0 - y * y).sqrt();
            let t = golden * i as f32 + self.rng.range(-0.2, 0.2);
            let u = Vec3::new(r * t.cos(), y, r * t.sin());
            if !Self::on_scalp(u) {
                continue;
            }
            let guess = self.center + u * self.radii * 1.05;
            let (p, normal) = self.skin.project(guess);
            out.push((p, normal));
        }
        out
    }

    /// A strand's path from `root`: lifted `gap` off the skin (its layer), set off along `dir`
    /// (and up off the skin by `lift`), bending toward the ground by `gravity` a segment, kept at
    /// least `gap` off the skin and clear of the face.
    #[allow(clippy::too_many_arguments)]
    fn grow(
        &self,
        root: Vec3,
        normal: Vec3,
        dir: Vec3,
        length: f32,
        gravity: f32,
        lift: f32,
        gap: f32,
        wave: (f32, f32),
        hug: f32,
    ) -> (Vec<Vec3>, Vec<Vec3>) {
        let seg = length / CARD_SEGMENTS as f32;
        let mut p = root + normal * gap;
        let mut d = (dir + normal * lift).normalize_or(dir);
        let mut points = vec![p];
        let mut normals = vec![normal];
        let side = d.cross(normal).normalize_or(Vec3::X);
        for i in 0..CARD_SEGMENTS {
            d = (d + Vec3::NEG_Y * gravity).normalize_or(Vec3::NEG_Y);
            let mut q = p + d * seg;
            // Waves and curls: a sideways swing along the strand.
            let (amp, period) = wave;
            if amp > 0.0 {
                let s = (i + 1) as f32 * seg;
                let s0 = i as f32 * seg;
                q += side
                    * amp
                    * ((s / period * std::f32::consts::TAU).sin()
                        - (s0 / period * std::f32::consts::TAU).sin());
            }
            q = self.keep_clear(q, gap);
            // Lying on the head: held down to its layer while above `hug` (in the cranium's
            // measure), so it follows the head's curve; below, it falls free.
            if self.local(q).y > hug {
                let off = self.skin.distance(q);
                if off > gap {
                    q -= self.skin.normal(q) * (off - gap);
                }
            }
            d = (q - p).normalize_or(d);
            normals.push(self.skin.normal(q));
            points.push(q);
            p = q;
        }
        (points, normals)
    }

    /// `q` moved off the skin to at least `gap`, and out of the face's front.
    fn keep_clear(&self, mut q: Vec3, gap: f32) -> Vec3 {
        for _ in 0..3 {
            let d = self.skin.distance(q);
            if d >= gap {
                break;
            }
            q += self.skin.normal(q) * (gap - d);
        }
        if !self.clear_face {
            return q;
        }
        let l = self.local(q);
        let below_brow = q.y < self.head.y + 0.07 * self.h;
        if l.z > 0.45 && l.x.abs() < 0.9 && below_brow && q.y > self.head.y - 0.05 * self.h {
            let x = l.x.signum() * 0.9 * self.radii.x;
            q.x = self.center.x + x.signum() * x.abs().max((q.x - self.center.x).abs());
        }
        q
    }

    fn card(
        &mut self,
        points: Vec<Vec3>,
        normals: Vec<Vec3>,
        width: f32,
        strands: Strands,
        depth: f32,
    ) {
        self.cards.push(Card {
            points,
            normals,
            width,
            strands,
            depth,
            rope: false,
        });
    }

    fn rope(&mut self, points: Vec<Vec3>, normals: Vec<Vec3>, width: f32, strands: Strands) {
        self.cards.push(Card {
            points,
            normals,
            width,
            strands,
            depth: 1.0,
            rope: true,
        });
    }

    /// The way hair falls from a root with this place on the cranium (`l`) and normal: down
    /// and away from a parting along the top, the front swept to the sides and back, clear of
    /// the face.
    fn flow(l: Vec3, normal: Vec3) -> Vec3 {
        let mut base = Vec3::new(l.x * 1.3, -0.9, -0.45);
        if l.z > 0.4 {
            base.x += l.x.signum() * 0.6;
            base.z = -0.6;
        }
        (base - normal * base.dot(normal)).normalize_or(Vec3::NEG_Y)
    }
}

/// How far above the hairline a point of the cranium (`l`, in its measure: the unit sphere is
/// its surface) lies: the hairline high at the forehead, receding at the temples, over the ears
/// and low at the nape. Hair grows where this is positive.
pub(crate) fn hairline(l: Vec3) -> f32 {
    let mut lim = if l.z > 0.0 {
        0.12 + 0.36 * l.z.min(1.0) - 0.12 * l.x.abs()
    } else {
        0.12 - 0.72 * (-l.z).min(1.0)
    };
    if l.x.abs() > 0.72 && l.z > -0.35 && l.z < 0.35 {
        lim = lim.max(0.24);
    }
    l.y - lim
}

/// The cranium's centre and radii (the scalp's ellipsoid) for a body of these proportions.
pub(crate) fn cranium(dims: &Proportions) -> (Vec3, Vec3) {
    let h = dims.stature;
    let head = rest_positions(dims)[Joint::Head.index()];
    (
        head + Vec3::new(0.0, 0.058, -0.008) * h,
        Vec3::new(dims.head_w * 0.5, 0.062 * h, dims.head_d * 0.5),
    )
}

/// Where facial hair of this style grows: `x`, `y` across and up the face from the head joint,
/// in statures. (Moustache, chin, jaw and cheeks.)
pub(crate) fn beard_at(style: FacialHair, x: f32, y: f32) -> bool {
    let (lip, chin, jaw) = match style {
        FacialHair::None => return false,
        FacialHair::Moustache => (true, false, false),
        FacialHair::Goatee => (true, true, false),
        FacialHair::Stubble | FacialHair::ShortBeard | FacialHair::FullBeard => (true, true, true),
    };
    let ax = x.abs();
    let in_lip = (0.021..0.029).contains(&y) && ax < 0.02;
    let in_chin = (-0.03..0.007).contains(&y) && ax < 0.02;
    let in_jaw =
        (-0.03..0.034).contains(&y) && (ax >= 0.026 || ax >= 0.016 && !(0.004..0.03).contains(&y));
    let on_lips = (0.006..0.0195).contains(&y) && ax < 0.016;
    let wanted = (lip && in_lip) || (chin && in_chin) || (jaw && (in_jaw || in_chin));
    wanted && !on_lips
}

/// The hair of a person of this appearance (head, brows, beard).
pub fn hair(a: &Appearance) -> HairMesh {
    let a = a.clone().sanitized();
    let dims = Proportions::of(&a);
    let skin = Skin::new(&a);
    let h = dims.stature;
    let head = rest_positions(&dims)[Joint::Head.index()];
    let seed = a.name.bytes().fold(0x5EED_u64 ^ a.hair as u64, |s, b| {
        s.wrapping_mul(31).wrapping_add(b as u64)
    });
    let (center, radii) = cranium(&dims);
    let mut b = Builder {
        skin: &skin,
        head,
        center,
        radii,
        h,
        rng: Rng(seed),
        cards: Vec::new(),
        clear_face: true,
    };
    // The style's own length at 0.5 of the slider; half to one and a half of it across.
    let grow = 0.5 + a.hair_length.clamp(0.0, 1.0);
    scalp(&mut b, a.hair, grow);
    b.clear_face = false;
    brows(&mut b, a.eyebrows.weight());
    beard(&mut b, a.facial_hair);
    finish(b)
}

fn scalp(b: &mut Builder, style: HairStyle, grow: f32) {
    let cm = 0.01;
    match style {
        HairStyle::Bald | HairStyle::Buzzed => {}
        HairStyle::ShortCrop
        | HairStyle::ShoulderLength
        | HairStyle::LongStraight
        | HairStyle::LongWavy
        | HairStyle::Curly => {
            // (length, gravity, card width, layers' gaps, count, strands, wave)
            let (len, gravity, width, gaps, count, strands, wave): (
                f32,
                f32,
                f32,
                &[f32],
                usize,
                Strands,
                (f32, f32),
            ) = match style {
                HairStyle::ShortCrop => (
                    5.0,
                    0.12,
                    1.6,
                    &[0.25, 0.6],
                    220,
                    Strands::Straight,
                    (0.0, 1.0),
                ),
                HairStyle::ShoulderLength => (
                    28.0,
                    0.3,
                    2.4,
                    &[0.3, 0.7, 1.1],
                    260,
                    Strands::Straight,
                    (0.0, 1.0),
                ),
                HairStyle::LongStraight => (
                    45.0,
                    0.32,
                    2.4,
                    &[0.3, 0.7, 1.1],
                    280,
                    Strands::Straight,
                    (0.0, 1.0),
                ),
                HairStyle::LongWavy => (
                    42.0,
                    0.3,
                    2.4,
                    &[0.35, 0.9, 1.4],
                    280,
                    Strands::Wavy,
                    (1.0, 7.0),
                ),
                _ => (
                    18.0,
                    0.22,
                    2.0,
                    &[0.5, 1.3, 2.1],
                    300,
                    Strands::Wavy,
                    (1.5, 4.0),
                ),
            };
            let roots = b.scalp_roots(count);
            for (k, &gap) in gaps.iter().enumerate() {
                let depth = (k + 1) as f32 / gaps.len() as f32;
                for (i, &(root, normal)) in roots.iter().enumerate() {
                    // Each layer a little sparser and shorter inside.
                    if (i + k) % (gaps.len() - k).max(1) != 0 && k + 1 < gaps.len() {
                        continue;
                    }
                    let l = b.local(root);
                    let dir = Builder::flow(l, normal);
                    let length = len * cm * grow * b.rng.range(0.85, 1.1) * (0.8 + 0.2 * depth);
                    // Hair over the crown lies flatter than the hair at the sides.
                    let (points, normals) = b.grow(
                        root,
                        normal,
                        dir,
                        length,
                        gravity,
                        0.1,
                        gap * cm,
                        (wave.0 * cm, wave.1 * cm),
                        -0.25,
                    );
                    let w = width * cm * b.rng.range(0.85, 1.15);
                    b.card(points, normals, w, strands, depth);
                }
            }
        }
        HairStyle::Coily => {
            // Shells of short cards standing out from the scalp.
            let len = 9.0 * cm * grow;
            let roots = b.scalp_roots(340);
            for (k, frac) in [0.35f32, 0.65, 1.0].into_iter().enumerate() {
                for (i, &(root, normal)) in roots.iter().enumerate() {
                    if k < 2 && i % 2 == 1 {
                        continue;
                    }
                    let dir = (normal + Vec3::new(0.0, -0.15, 0.0)).normalize();
                    let base = root + normal * len * frac * 0.8;
                    let (points, normals) = b.grow(
                        base,
                        normal,
                        dir,
                        len * 0.55,
                        0.04,
                        0.0,
                        0.3 * cm,
                        (0.0, 1.0),
                        10.0,
                    );
                    let tangent_dir = Builder::flow(b.local(root), normal);
                    // Lay the card across the shell, not out of it.
                    let lie: Vec<Vec3> = points
                        .iter()
                        .enumerate()
                        .map(|(j, p)| {
                            let t = j as f32 / CARD_SEGMENTS as f32;
                            *p - dir * len * 0.55 * t + tangent_dir * len * 0.55 * t
                        })
                        .collect();
                    b.card(lie, normals, 3.0 * cm, Strands::Kinky, frac);
                }
            }
        }
        HairStyle::Braids | HairStyle::Locs => {
            let braid = style == HairStyle::Braids;
            let roots = b.scalp_roots(if braid { 60 } else { 46 });
            for &(root, normal) in &roots {
                let l = b.local(root);
                if braid {
                    // Cornrows: flat to the scalp, running back.
                    let dir = (Vec3::new(0.0, -0.2, -1.0)
                        - normal * normal.dot(Vec3::new(0.0, -0.2, -1.0)))
                    .normalize_or(Vec3::NEG_Z);
                    let (points, normals) = b.grow(
                        root,
                        normal,
                        dir,
                        8.0 * cm,
                        0.0,
                        0.0,
                        0.4 * cm,
                        (0.0, 1.0),
                        -10.0,
                    );
                    b.card(points, normals, 1.1 * cm, Strands::Braid, 0.5);
                    // The ends hang from the back.
                    if l.z > -0.2 {
                        continue;
                    }
                }
                let dir = Builder::flow(l, normal);
                let length = 34.0 * cm * grow * b.rng.range(0.9, 1.1);
                let (points, normals) = b.grow(
                    root,
                    normal,
                    dir,
                    length,
                    0.45,
                    0.25,
                    0.8 * cm,
                    (0.0, 1.0),
                    -0.25,
                );
                let strands = if braid { Strands::Braid } else { Strands::Loc };
                b.rope(
                    points,
                    normals,
                    if braid { 1.0 * cm } else { 1.3 * cm },
                    strands,
                );
            }
        }
        HairStyle::TiedBack => {
            // Combed back to a tie at the back of the head, and a tail from it.
            let tie = b.center + Vec3::new(0.0, 0.05, -1.02) * b.radii;
            let roots = b.scalp_roots(220);
            for &(root, normal) in &roots {
                let mut points = Vec::new();
                let mut normals = Vec::new();
                for j in 0..=CARD_SEGMENTS {
                    let t = j as f32 / CARD_SEGMENTS as f32;
                    let p = b.keep_clear(root.lerp(tie, t), 0.3 * cm);
                    normals.push(skin_normal(b, p, normal));
                    points.push(p);
                }
                b.card(points, normals, 1.6 * cm, Strands::Straight, 1.0);
            }
            let length = 26.0 * cm * grow;
            for k in 0..6 {
                let a = k as f32 / 6.0 * std::f32::consts::TAU;
                let off = Vec3::new(a.cos(), 0.0, a.sin()) * 0.6 * cm;
                let (points, normals) = b.grow(
                    tie + off,
                    Vec3::NEG_Z,
                    Vec3::new(0.0, -0.6, -0.8).normalize(),
                    length,
                    0.35,
                    0.0,
                    0.6 * cm,
                    (0.0, 1.0),
                    10.0,
                );
                b.rope(points, normals, 1.8 * cm, Strands::Straight);
            }
        }
    }
}

fn skin_normal(b: &Builder, p: Vec3, fallback: Vec3) -> Vec3 {
    let n = b.skin.normal(p);
    if n.is_finite() { n } else { fallback }
}

/// The brows: short cards along each brow's arch, the inner hairs rising, the outer ones lying
/// outward.
fn brows(b: &mut Builder, weight: f32) {
    let h = b.h;
    let cm = 0.01;
    let count = (9.0 * weight.sqrt()).round().max(4.0) as usize;
    for (s, row) in [(1.0f32, 0.0f32), (-1.0, 0.0), (1.0, 1.0), (-1.0, 1.0)] {
        for k in 0..count {
            let t = k as f32 / (count - 1) as f32;
            let x = 0.007 + 0.034 * t;
            let arch = 0.004 * (std::f32::consts::PI * (t * 0.8 + 0.1)).sin()
                + 0.0022 * row * (1.0 - 0.6 * t);
            let guess = b.head + Vec3::new(s * x, 0.0665 + arch, 0.08) * h;
            let (root, normal) = b.skin.project(guess);
            let along = Vec3::new(s, -0.15 + 0.6 * (1.0 - t), 0.0).normalize();
            let dir = (along - normal * normal.dot(along)).normalize_or(Vec3::X * s);
            let (points, normals) = b.grow(
                root - dir * 0.3 * cm,
                normal,
                dir,
                (0.9 + 0.5 * (1.0 - t)) * cm,
                0.0,
                0.0,
                0.04 * cm,
                (0.0, 1.0),
                -10.0,
            );
            b.card(
                points,
                normals,
                0.45 * cm * weight.sqrt(),
                Strands::Brow,
                1.0,
            );
        }
    }
}

/// Facial hair over the lip, chin, jaw and cheeks below the cheekbones.
fn beard(b: &mut Builder, style: FacialHair) {
    let h = b.h;
    let cm = 0.01;
    // (length (cm), gravity)
    // (length (cm), gravity, held to the face down to (the cranium's measure))
    let (len, gravity, hug) = match style {
        FacialHair::None | FacialHair::Stubble => return,
        FacialHair::Moustache => (1.3, 0.1, -10.0),
        FacialHair::Goatee => (2.2, 0.25, -10.0),
        FacialHair::ShortBeard => (1.6, 0.15, -10.0),
        FacialHair::FullBeard => (5.5, 0.45, -1.35),
    };
    let step = 0.0036;
    let mut y = -0.032;
    while y < 0.038 {
        let mut x = -0.05;
        while x < 0.05 {
            let jx = x + b.rng.range(-0.3, 0.3) * step;
            let jy = y + b.rng.range(-0.3, 0.3) * step;
            let in_lip = (0.019..0.029).contains(&jy) && jx.abs() < 0.021;
            if beard_at(style, jx, jy) {
                let guess = b.head + Vec3::new(jx, jy, 0.065 - 0.4 * jx.abs()) * h;
                let (root, normal) = b.skin.project(guess);
                // Only the face's front and sides, and under the jaw; and where the root landed
                // near where it was meant to (not slid onto the lips or nose).
                let l = root - b.head;
                let slid = (l.y / h - jy).abs() > 0.007;
                if l.z > -0.012 * h && normal.z > -0.3 && !slid {
                    // Down the face; under the jaw, back toward the throat.
                    let down = Vec3::new(jx.signum() * 0.15, -1.0, 0.25);
                    let mut dir = down - normal * normal.dot(down);
                    if dir.length() < 0.4 {
                        dir = Vec3::new(0.0, -0.3, -1.0)
                            - normal * normal.dot(Vec3::new(0.0, -0.3, -1.0));
                    }
                    let dir = dir.normalize_or(Vec3::NEG_Y);
                    let depth = if in_lip { 1.0 } else { 0.8 };
                    let length = len * cm * b.rng.range(0.8, 1.1);
                    let (points, normals) = b.grow(
                        root,
                        normal,
                        dir,
                        length,
                        gravity,
                        0.0,
                        0.12 * cm,
                        (0.0, 1.0),
                        hug,
                    );
                    b.card(points, normals, 0.7 * cm, Strands::Beard, depth);
                }
            }
            x += step;
        }
        y += step;
    }
}

/// Cuts the cards into triangles and picks the guides.
fn finish(b: Builder) -> HairMesh {
    let mut out = HairMesh::default();
    // Guides: spread over the longer cards' roots (farthest-point picking); short hair hardly
    // moves and follows the nearest guide stiffly.
    let swinging: Vec<usize> = (0..b.cards.len())
        .filter(|&i| b.cards[i].swings())
        .collect();
    let long: Vec<usize> = swinging
        .iter()
        .copied()
        .filter(|&i| path_length(&b.cards[i].points) > 0.06)
        .collect();
    let pool: Vec<usize> = if long.is_empty() { swinging } else { long };
    let mut picked: Vec<usize> = Vec::new();
    if let Some(&first) = pool.first() {
        picked.push(first);
        while picked.len() < GUIDES.min(pool.len()) {
            let next = pool
                .iter()
                .copied()
                .max_by(|&x, &y| {
                    let dx = nearest(&b.cards, &picked, b.cards[x].points[0]).1;
                    let dy = nearest(&b.cards, &picked, b.cards[y].points[0]).1;
                    dx.total_cmp(&dy)
                })
                .expect("a pool");
            picked.push(next);
        }
    }
    for &i in &picked {
        let points = &b.cards[i].points;
        out.guides.push(std::array::from_fn(|k| {
            resample(points, k as f32 / (GUIDE_POINTS - 1) as f32)
        }));
        let len = path_length(points);
        // Short hair stands stiff; long hair swings.
        out.stiffness.push((1.0 - len / 0.3).clamp(0.15, 0.95));
    }
    for card in &b.cards {
        let guide = if picked.is_empty() || !card.swings() {
            NO_GUIDE as usize
        } else {
            nearest(&b.cards, &picked, card.points[0]).0
        };
        let guide_len = picked
            .get(guide)
            .map_or(1.0, |&g| path_length(&b.cards[g].points).max(1e-3));
        let len = path_length(&card.points);
        let sheets: &[bool] = if card.rope { &[false, true] } else { &[false] };
        for &turned in sheets {
            let base = out.positions.len() as u32;
            for (j, (&p, &n)) in card.points.iter().zip(&card.normals).enumerate() {
                let t = j as f32 / CARD_SEGMENTS as f32;
                let tangent = if j + 1 < card.points.len() {
                    card.points[j + 1] - p
                } else {
                    p - card.points[j - 1]
                }
                .normalize_or(Vec3::NEG_Y);
                let n = (n - tangent * n.dot(tangent)).normalize_or(Vec3::Z);
                let side = tangent.cross(n).normalize_or(Vec3::X);
                let (face, across) = if turned { (side, -n) } else { (n, side) };
                // Tapering to the tip.
                let w = card.width * (1.0 - 0.45 * t);
                let along = ((t * len / guide_len).min(1.0) * 255.0).round() as u8;
                for (u, sign) in [(0.0f32, -0.5f32), (1.0, 0.5)] {
                    out.positions.push(p + across * w * sign);
                    out.tangents.push(tangent);
                    out.normals.push(face);
                    out.uv.push([u, t]);
                    out.info.push([
                        guide as u8,
                        card.strands as u8,
                        (card.depth.clamp(0.0, 1.0) * 255.0) as u8,
                        along,
                    ]);
                }
            }
            for j in 0..CARD_SEGMENTS as u32 {
                let a = base + j * 2;
                out.indices
                    .extend_from_slice(&[a, a + 1, a + 3, a, a + 3, a + 2]);
            }
        }
    }
    out
}

fn path_length(points: &[Vec3]) -> f32 {
    points.windows(2).map(|w| (w[1] - w[0]).length()).sum()
}

/// The point a fraction `t` of the way along a path.
fn resample(points: &[Vec3], t: f32) -> Vec3 {
    let total = path_length(points);
    let mut want = t * total;
    for w in points.windows(2) {
        let l = (w[1] - w[0]).length();
        if want <= l && l > 0.0 {
            return w[0].lerp(w[1], want / l);
        }
        want -= l;
    }
    *points.last().expect("a path")
}

/// The picked guide whose root is nearest `p`, and how far.
fn nearest(cards: &[Card], picked: &[usize], p: Vec3) -> (usize, f32) {
    picked
        .iter()
        .enumerate()
        .map(|(k, &i)| (k, cards[i].points[0].distance(p)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap_or((0, f32::MAX))
}

/// The guides in motion: each a chain of points pinned at its root to the head, falling under
/// gravity and pushed by the wind, held to its length and toward its style by its stiffness,
/// and kept out of the head (Verlet integration).
#[derive(Debug, Clone)]
pub struct HairSim {
    rest: Vec<[Vec3; GUIDE_POINTS]>,
    stiffness: Vec<f32>,
    pos: Vec<[Vec3; GUIDE_POINTS]>,
    prev: Vec<[Vec3; GUIDE_POINTS]>,
    seg: Vec<f32>,
    /// The cranium's centre (bind frame) and a radius holding it.
    head_center: Vec3,
    head_radius: f32,
    started: bool,
}

impl HairSim {
    pub fn new(mesh: &HairMesh, a: &Appearance) -> Self {
        let dims = Proportions::of(a);
        Self {
            rest: mesh.guides.clone(),
            stiffness: mesh.stiffness.clone(),
            pos: mesh.guides.clone(),
            prev: mesh.guides.clone(),
            seg: mesh
                .guides
                .iter()
                .map(|g| path_length(g) / (GUIDE_POINTS - 1) as f32)
                .collect(),
            head_center: cranium(&dims).0,
            head_radius: dims.head_d * 0.5,
            started: false,
        }
    }

    /// Steps the guides `dt` seconds with the head carried by `head` (bind frame to the frame
    /// the palette draws in), in `wind` (m/s); wet hair hangs heavier.
    pub fn step(&mut self, head: Affine3A, dt: f32, wind: Vec3, wet: f32) {
        let dt = dt.clamp(0.0, 0.05);
        let center = head.transform_point3(self.head_center);
        if !self.started {
            for (g, rest) in self.rest.iter().enumerate() {
                self.pos[g] = rest.map(|p| head.transform_point3(p));
                self.prev[g] = self.pos[g];
            }
            self.started = true;
        }
        let gravity = Vec3::new(0.0, -9.81, 0.0);
        // Damping and the pull to the style are set per 60th of a second and scaled to the
        // step, so the hair moves alike at any frame rate.
        let frames = dt * 60.0;
        let keep = 0.96f32.powf(frames);
        for g in 0..self.rest.len() {
            let stiff = self.stiffness[g] * (1.0 - 0.3 * wet.clamp(0.0, 1.0));
            // Air drag toward the wind's speed: strong on light dry hair.
            let drag = 3.0 * (1.0 - 0.5 * wet.clamp(0.0, 1.0));
            let root = head.transform_point3(self.rest[g][0]);
            self.pos[g][0] = root;
            self.prev[g][0] = root;
            for k in 1..GUIDE_POINTS {
                let x = self.pos[g][k];
                let v = (x - self.prev[g][k]) / dt.max(1e-4);
                let accel = gravity + (wind - v) * drag;
                let next = x + (x - self.prev[g][k]) * keep + accel * dt * dt;
                self.prev[g][k] = x;
                self.pos[g][k] = next;
            }
            for _ in 0..3 {
                for k in 1..GUIDE_POINTS {
                    // Toward the style's shape, as stiff as the hair is.
                    let styled = head.transform_point3(self.rest[g][k]);
                    let pull = 1.0 - (1.0 - stiff * stiff * 0.5).powf(frames);
                    self.pos[g][k] = self.pos[g][k].lerp(styled, pull);
                    // Its length from the point before.
                    let a = self.pos[g][k - 1];
                    let d = self.pos[g][k] - a;
                    self.pos[g][k] = a + d.normalize_or(Vec3::NEG_Y) * self.seg[g];
                    // Out of the head.
                    let c = self.pos[g][k] - center;
                    if c.length() < self.head_radius {
                        self.pos[g][k] = center + c.normalize_or(Vec3::Y) * self.head_radius;
                    }
                }
            }
        }
    }

    /// Each guide point's offset from where the style holds it, in the frame `head` draws to,
    /// for the shader (`GUIDES × GUIDE_POINTS`, unused guides zero).
    pub fn offsets(&self, head: Affine3A) -> [[f32; 4]; GUIDES * GUIDE_POINTS] {
        let mut out = [[0.0; 4]; GUIDES * GUIDE_POINTS];
        if !self.started {
            return out;
        }
        for (g, rest) in self.rest.iter().enumerate().take(GUIDES) {
            for k in 0..GUIDE_POINTS {
                let d = self.pos[g][k] - head.transform_point3(rest[k]);
                out[g * GUIDE_POINTS + k] = [d.x, d.y, d.z, 0.0];
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::Eyebrows;

    #[test]
    fn every_style_grows_on_the_scalp_and_off_the_skin() {
        for style in HairStyle::ALL {
            let a = Appearance {
                hair: style,
                facial_hair: FacialHair::FullBeard,
                eyebrows: Eyebrows::Thick,
                ..Appearance::default()
            };
            let t0 = std::time::Instant::now();
            let mesh = hair(&a);
            let ms = t0.elapsed().as_secs_f64() * 1e3;
            let skin = Skin::new(&a);
            let inside = mesh
                .positions
                .iter()
                .filter(|p| skin.distance(**p) < -0.004)
                .count();
            eprintln!(
                "{style:?}: {} cards, {} vertices, {} guides, {inside} inside, {ms:.0} ms",
                mesh.card_count(),
                mesh.positions.len(),
                mesh.guides.len()
            );
            assert!(mesh.positions.len() == mesh.info.len());
            assert!(
                inside * 50 < mesh.positions.len().max(1),
                "{style:?}: {inside} under the skin"
            );
            assert!(mesh.guides.len() <= GUIDES);
            if !matches!(style, HairStyle::Bald | HairStyle::Buzzed) {
                assert!(mesh.card_count() > 100, "{style:?}");
            }
        }
    }

    #[test]
    fn long_hair_swings_and_settles() {
        let a = Appearance {
            hair: HairStyle::LongStraight,
            ..Appearance::default()
        };
        let mesh = hair(&a);
        let mut sim = HairSim::new(&mesh, &a);
        sim.step(Affine3A::IDENTITY, 1.0 / 60.0, Vec3::ZERO, 0.0);
        // A turn of the head: the tips lag, then settle.
        let turned = Affine3A::from_translation(Vec3::new(0.0, 1.6, 0.0))
            * Affine3A::from_rotation_y(0.6)
            * Affine3A::from_translation(Vec3::new(0.0, -1.6, 0.0));
        sim.step(turned, 1.0 / 60.0, Vec3::ZERO, 0.0);
        let lag = sim
            .offsets(turned)
            .iter()
            .map(|o| Vec3::from_slice(&o[..3]).length())
            .fold(0.0, f32::max);
        for _ in 0..240 {
            sim.step(turned, 1.0 / 60.0, Vec3::ZERO, 0.0);
        }
        let settled = sim
            .offsets(turned)
            .iter()
            .map(|o| Vec3::from_slice(&o[..3]).length())
            .fold(0.0, f32::max);
        eprintln!("lag {lag:.3} m, settled {settled:.3} m");
        assert!(lag > 0.02);
        assert!(settled < lag);
    }
}

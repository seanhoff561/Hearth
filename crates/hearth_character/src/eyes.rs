//! Eyes (Amendment E §8, E7): two eyeballs set in the sockets the face cuts, each turning on its
//! centre to where the person looks (with the small quick jumps of saccades), the upper lids
//! shells over them that follow the gaze and close to blink, the lower lids, and lashes along the
//! upper lids' edges.
//!
//! The mesh is in the body's bind frame; each part turns about its eye's centre
//! (`transforms`), carried by the head.

use glam::{Affine3A, Quat, Vec2, Vec3};

use crate::anatomy::rest_positions;
use crate::appearance::Appearance;
use crate::rig::{Joint, Proportions};

/// The parts that move on their own: the eyeballs, the upper lids, the lower lids (left, right).
pub const EYE_PARTS: usize = 6;

/// What a vertex of the eyes is, for its shading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EyeSurface {
    Ball = 0,
    Lid = 1,
    /// A lid's wet inner edge.
    Margin = 2,
    Lash = 3,
}

/// The eyes' mesh.
#[derive(Debug, Clone, Default)]
pub struct EyeMesh {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    /// The position from its eye's centre (the eye's own frame at rest: +z out of the face).
    pub local: Vec<Vec3>,
    /// Across and along a lash card (0–1); zero elsewhere.
    pub uv: Vec<[f32; 2]>,
    /// The part (0–5: ball L, ball R, upper lid L, R, lower lid L, R) and the surface.
    pub part: Vec<[u8; 2]>,
    pub indices: Vec<u32>,
    /// The eyes' centres (left, right), bind frame.
    pub centers: [Vec3; 2],
    pub radius: f32,
}

/// The eyes of a person of this appearance.
pub fn eyes(a: &Appearance) -> EyeMesh {
    let a = a.clone().sanitized();
    let dims = Proportions::of(&a);
    let h = dims.stature;
    let head = rest_positions(&dims)[Joint::Head.index()];
    // The sockets as the face cuts them (`anatomy::face`), the ball a little behind.
    let spread = 1.0 + 0.1 * a.face.eyes.clamp(-1.0, 1.0);
    let radius = 0.0068 * h;
    let centers = [1.0f32, -1.0].map(|s| head + Vec3::new(s * 0.017 * spread, 0.056, 0.0508) * h);
    let mut m = EyeMesh {
        centers,
        radius,
        ..EyeMesh::default()
    };
    for (side, &c) in centers.iter().enumerate() {
        ball(&mut m, side as u8, c, radius);
        lid(&mut m, 2 + side as u8, c, radius, true, side == 0);
        lid(&mut m, 4 + side as u8, c, radius, false, side == 0);
    }
    m
}

fn push(
    m: &mut EyeMesh,
    c: Vec3,
    local: Vec3,
    normal: Vec3,
    uv: [f32; 2],
    part: u8,
    s: EyeSurface,
) {
    m.positions.push(c + local);
    m.normals.push(normal);
    m.local.push(local);
    m.uv.push(uv);
    m.part.push([part, s as u8]);
}

/// A grid of `rows × cols` quads (vertices already pushed from `base`, row-major), wound to face
/// outward.
fn grid(m: &mut EyeMesh, base: u32, rows: u32, cols: u32, flip: bool) {
    for r in 0..rows {
        for k in 0..cols {
            let a = base + r * (cols + 1) + k;
            let b = a + cols + 1;
            if flip {
                m.indices.extend_from_slice(&[a, a + 1, b + 1, a, b + 1, b]);
            } else {
                m.indices.extend_from_slice(&[a, b + 1, a + 1, a, b, b + 1]);
            }
        }
    }
}

/// An eyeball: a sphere, its cornea bulging a little in front.
fn ball(m: &mut EyeMesh, part: u8, c: Vec3, r: f32) {
    let (rows, cols) = (24u32, 32u32);
    let base = m.positions.len() as u32;
    for i in 0..=rows {
        // From the front (+z) to the back.
        let polar = std::f32::consts::PI * i as f32 / rows as f32;
        for k in 0..=cols {
            let az = std::f32::consts::TAU * k as f32 / cols as f32;
            let d = Vec3::new(polar.sin() * az.cos(), polar.sin() * az.sin(), polar.cos());
            // The cornea: a cap some 12° wider than the iris, standing 0.6 mm proud.
            let bulge = 1.0 + 0.05 * (1.0 - (polar / 0.62).min(1.0)).powi(2);
            push(m, c, d * r * bulge, d, [0.0; 2], part, EyeSurface::Ball);
        }
    }
    grid(m, base, rows, cols, true);
}

/// A lid: a shell over the ball from far above (or below) to its edge, which folds in to the ball
/// at the margin; lashes on an upper lid's edge.
fn lid(m: &mut EyeMesh, part: u8, c: Vec3, r: f32, upper: bool, left: bool) {
    let (rows, cols) = (8u32, 24u32);
    let shell = r * if upper { 1.1 } else { 1.04 };
    let half = 1.3f32; // radians either side of forward
    // The edge's height at each angle across: arched, highest a little toward the nose.
    let edge = |t: f32| -> f32 {
        let toward_nose = if left { -t } else { t };
        if upper {
            0.42 * (1.0 - t * t) + 0.04 * toward_nose
        } else {
            -0.34 * (1.0 - 0.6 * t * t)
        }
    };
    let far = if upper { 1.25f32 } else { -0.95 };
    let base = m.positions.len() as u32;
    let dir = |el: f32, az: f32| Vec3::new(el.cos() * az.sin(), el.sin(), el.cos() * az.cos());
    for i in 0..=rows {
        let f = i as f32 / rows as f32;
        for k in 0..=cols {
            let t = k as f32 / cols as f32 * 2.0 - 1.0;
            let az = t * half;
            let el = far + (edge(t) - far) * f;
            let d = dir(el, az);
            push(m, c, d * shell, d, [0.0; 2], part, EyeSurface::Lid);
        }
    }
    // The margin: the edge rolled in to touch the ball.
    for k in 0..=cols {
        let t = k as f32 / cols as f32 * 2.0 - 1.0;
        let az = t * half;
        let el = edge(t) + if upper { -0.06 } else { 0.06 };
        let d = dir(el, az);
        let normal = dir(el + if upper { -1.2 } else { 1.2 }, az);
        push(
            m,
            c,
            d * r * 1.02,
            normal,
            [0.0; 2],
            part,
            EyeSurface::Margin,
        );
    }
    grid(m, base, rows + 1, cols, !upper);
    if upper {
        // Lashes: a card along the edge, curving out and up.
        let base = m.positions.len() as u32;
        let length = r * 0.6;
        for j in 0..=2u32 {
            let along = j as f32 / 2.0;
            for k in 0..=cols {
                let t = k as f32 / cols as f32 * 2.0 - 1.0;
                let az = t * half * 0.85;
                let el = edge(t * 0.85);
                let root = dir(el, az) * shell;
                let out = dir(el, az);
                let curl = (out + Vec3::Y * (0.15 + 0.5 * along)).normalize();
                let p = root + curl * length * along * (1.0 - 0.4 * t * t);
                let normal = dir(el + 0.8, az);
                push(
                    m,
                    c,
                    p,
                    normal,
                    [(t + 1.0) * 0.5, along],
                    part,
                    EyeSurface::Lash,
                );
            }
        }
        grid(m, base, 2, cols, false);
    }
}

/// Where the eyes look and how closed the lids are, moving as eyes do: gaze held on a target with
/// small jumps (saccades) every half second to few seconds, blinks every two to ten seconds,
/// each about a fifth of a second.
#[derive(Debug, Clone)]
pub struct EyeMotion {
    /// Yaw (left positive) and pitch (down positive), radians.
    gaze: Vec2,
    jitter: Vec2,
    until_saccade: f32,
    until_blink: f32,
    /// Seconds into a blink (negative: none).
    blinking: f32,
    rng: u64,
}

/// A blink's closing and opening (s).
const CLOSE_S: f32 = 0.07;
const OPEN_S: f32 = 0.15;

impl EyeMotion {
    pub fn new(seed: u64) -> Self {
        Self {
            gaze: Vec2::ZERO,
            jitter: Vec2::ZERO,
            until_saccade: 0.6,
            until_blink: 2.5,
            blinking: -1.0,
            rng: seed | 1,
        }
    }

    fn next(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Steps `dt` seconds, looking toward `look` (yaw, pitch down; radians, from the head).
    pub fn step(&mut self, dt: f32, look: Vec2) {
        self.until_saccade -= dt;
        if self.until_saccade <= 0.0 {
            self.jitter = Vec2::new(self.next() - 0.5, self.next() - 0.5) * 0.16;
            self.until_saccade = 0.4 + 2.2 * self.next();
        }
        // A saccade lands in some 30 ms: as good as at once at a frame's pace.
        let target = (look + self.jitter).clamp(Vec2::new(-0.6, -0.45), Vec2::new(0.6, 0.5));
        self.gaze += (target - self.gaze) * (dt / 0.03).min(1.0);
        if self.blinking >= 0.0 {
            self.blinking += dt;
            if self.blinking > CLOSE_S + OPEN_S {
                self.blinking = -1.0;
            }
        } else {
            self.until_blink -= dt;
            if self.until_blink <= 0.0 {
                self.blinking = 0.0;
                self.until_blink = 2.0 + 8.0 * self.next();
            }
        }
    }

    /// Yaw and pitch (down positive), radians.
    pub fn gaze(&self) -> Vec2 {
        self.gaze
    }

    /// How closed the lids are (0 open – 1 shut).
    pub fn blink(&self) -> f32 {
        if self.blinking < 0.0 {
            0.0
        } else if self.blinking < CLOSE_S {
            self.blinking / CLOSE_S
        } else {
            1.0 - (self.blinking - CLOSE_S) / OPEN_S
        }
    }
}

/// Each part's transform from the bind frame for a gaze and a blink, carried by `head` (the
/// head joint's skinning transform).
pub fn transforms(mesh: &EyeMesh, head: Affine3A, gaze: Vec2, blink: f32) -> [Affine3A; EYE_PARTS] {
    let about = |c: Vec3, q: Quat| {
        head * Affine3A::from_translation(c)
            * Affine3A::from_quat(q)
            * Affine3A::from_translation(-c)
    };
    let ball = Quat::from_rotation_y(gaze.x) * Quat::from_rotation_x(gaze.y);
    // The upper lid follows the gaze down (and a little up), and comes down over the ball to
    // blink; the lower lid rises a little.
    let blink = blink.clamp(0.0, 1.0);
    let upper = Quat::from_rotation_x(0.6 * gaze.y.max(-0.2) + 0.62 * blink);
    let lower = Quat::from_rotation_x(0.25 * gaze.y.max(0.0) - 0.12 * blink);
    let [l, r] = mesh.centers;
    [
        about(l, ball),
        about(r, ball),
        about(l, upper),
        about(r, upper),
        about(l, lower),
        about(r, lower),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eyes_sit_in_the_sockets_and_lids_close() {
        let a = Appearance::default();
        let m = eyes(&a);
        assert!(m.positions.len() > 1000);
        assert_eq!(m.positions.len(), m.part.len());
        // Symmetric, a pupil's spacing apart (some 6 cm).
        let gap = (m.centers[0] - m.centers[1]).length();
        assert!((0.055..0.07).contains(&gap), "{gap}");
        // Shut, the upper lid's edge comes below the ball's middle.
        let shut = transforms(&m, Affine3A::IDENTITY, Vec2::ZERO, 1.0);
        let edge = m
            .positions
            .iter()
            .zip(&m.part)
            .filter(|(_, p)| p[0] == 2 && p[1] == EyeSurface::Margin as u8)
            .map(|(q, _)| shut[2].transform_point3(*q).y)
            .fold(f32::MAX, f32::min);
        assert!(edge < m.centers[0].y, "{edge} vs {}", m.centers[0].y);
    }

    #[test]
    fn blinks_and_saccades_come_and_go() {
        let mut e = EyeMotion::new(7);
        let (mut blinks, mut shut, mut jumps) = (0, 0, 0);
        let mut last = e.gaze();
        let mut was = 0.0;
        for _ in 0..(60 * 60) {
            e.step(1.0 / 60.0, Vec2::ZERO);
            let b = e.blink();
            if b > 0.0 && was == 0.0 {
                blinks += 1;
            }
            if b > 0.9 {
                shut += 1;
            }
            was = b;
            if (e.gaze() - last).length() > 0.02 {
                jumps += 1;
            }
            last = e.gaze();
        }
        // A minute: some 6 to 30 blinks, each shut only a moment; some dozens of saccades.
        assert!((6..=30).contains(&blinks), "{blinks} blinks");
        assert!(shut < blinks * 4, "{shut} frames shut");
        assert!(jumps > 15, "{jumps} saccades");
    }
}

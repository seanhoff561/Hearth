//! Garments fitted to the sculpted body (Amendment E §8, E7). The loincloth: a cord tied round
//! the hips just under the waist, following the body's surface, and a front and a back flap
//! hung from it and draped over the body (kept off the skin by its field). The cord rides the
//! hips; the flaps' lower parts are carried partly by the thighs, so they swing as the legs do.
//!
//! The mesh is an `Anatomy` in the same bind pose as the skin (its tissue marked a garment's),
//! to be merged into it and drawn with it.

use glam::{Vec3, Vec4};

use crate::anatomy::{Anatomy, Skin, bind_pose, rest_positions};
use crate::appearance::Appearance;
use crate::rig::{Joint, Proportions};

/// Mesh vertices: position, outward normal, (joint, weight) pairs.
struct Builder {
    out: Anatomy,
}

impl Builder {
    fn vertex(&mut self, p: Vec3, n: Vec3, joints: &[(Joint, f32)]) -> u32 {
        let mut j = [joints[0].0.index() as u8; 4];
        let mut w = [0.0f32; 4];
        let sum: f32 = joints.iter().map(|x| x.1).sum::<f32>().max(1e-6);
        for (k, (jn, wt)) in joints.iter().take(4).enumerate() {
            j[k] = jn.index() as u8;
            w[k] = wt / sum;
        }
        self.out.positions.push(p);
        self.out.normals.push(n.normalize_or(Vec3::Z));
        self.out.joints.push(j);
        self.out.weights.push(w);
        self.out.tissue.push([0.0, 0.0, 0.0, 1.0]);
        self.out.surface.push([0.0; 4]);
        self.out.positions.len() as u32 - 1
    }

    /// Quads over a `rows × cols` grid of vertices from `base`, row-major.
    fn grid(&mut self, base: u32, rows: u32, cols: u32, flip: bool) {
        for r in 0..rows {
            for k in 0..cols {
                let a = base + r * (cols + 1) + k;
                let b = a + cols + 1;
                if flip {
                    self.out
                        .indices
                        .extend_from_slice(&[a, b + 1, a + 1, a, b, b + 1]);
                } else {
                    self.out
                        .indices
                        .extend_from_slice(&[a, a + 1, b + 1, a, b + 1, b]);
                }
            }
        }
    }
}

/// The loincloth of a person of this appearance, fitted to their body.
pub fn loincloth(a: &Appearance) -> Anatomy {
    let a = a.clone().sanitized();
    let dims = Proportions::of(&a);
    let skin = Skin::new(&a);
    let h = dims.stature;
    let rest = rest_positions(&dims);
    let root = rest[Joint::Root.index()];
    let mut b = Builder {
        out: Anatomy {
            bind: bind_pose(&dims),
            ..Anatomy::default()
        },
    };
    // The cord: round the hips a little below the waist, on the skin; a tube of 6 mm.
    let cord_y = root.y + 0.022 * h;
    let ring = 64usize;
    let r = 0.004 * h;
    let around: Vec<(Vec3, Vec3)> = (0..ring)
        .map(|i| {
            let t = i as f32 / ring as f32 * std::f32::consts::TAU;
            // Lower at the front, as a cord settles under the belly.
            let dip = 0.012 * h * (0.5 + 0.5 * t.cos());
            let out = Vec3::new(t.sin(), 0.0, t.cos());
            let (p, n) = skin.project(Vec3::new(root.x, cord_y - dip, root.z) + out * 0.25);
            (p + n * r, n)
        })
        .collect();
    let base = b.out.positions.len() as u32;
    let sides = 6usize;
    for i in 0..=ring {
        let (p, n) = around[i % ring];
        let next = around[(i + 1) % ring].0;
        let tangent = (next - p).normalize_or(Vec3::X);
        let up = tangent.cross(n).normalize_or(Vec3::Y);
        for k in 0..=sides {
            let t = k as f32 / sides as f32 * std::f32::consts::TAU;
            let d = n * t.cos() + up * t.sin();
            b.vertex(p + d * r, d, &[(Joint::Root, 1.0)]);
        }
    }
    b.grid(base, ring as u32, sides as u32, false);
    // The flaps: a front one (z > 0) and a longer back one, a hide's width hanging from the
    // cord and draped over the body below.
    let width = 0.15 * h;
    for (front, length) in [(1.0f32, 0.13 * h), (-1.0, 0.16 * h)] {
        let (cols, rows) = (12u32, 14u32);
        let mut sheet: Vec<Vec<(Vec3, Vec3)>> = Vec::new();
        for i in 0..=rows {
            let v = i as f32 / rows as f32;
            let mut row = Vec::new();
            for k in 0..=cols {
                let u = k as f32 / cols as f32 - 0.5;
                // Narrowing a little as it falls; hung from the cord's front or back.
                let x = u * width * (1.0 - 0.15 * v);
                let top = Vec3::new(
                    root.x + x,
                    cord_y - 0.012 * h * (0.5 + 0.5 * front),
                    root.z + front * 0.3,
                );
                let (on_cord, n) = skin.project(top);
                let mut p = on_cord + n * (r * 1.8) + Vec3::NEG_Y * length * v;
                // Draped: off the skin by a few millimetres, the hide's stiffness bridging the
                // gap between the thighs (pushed out further low down).
                let gap = 0.004 + 0.01 * v;
                for _ in 0..4 {
                    let d = skin.distance(p);
                    if d >= gap {
                        break;
                    }
                    let nn = skin.normal(p);
                    // Out toward its own side (front or back), not between the legs.
                    let out = (nn + Vec3::Z * front * 0.8).normalize_or(nn);
                    p += out * (gap - d);
                }
                let n = skin.normal(p);
                let n = if n.z * front < 0.0 {
                    Vec3::Z * front
                } else {
                    n
                };
                row.push((p, n));
            }
            sheet.push(row);
        }
        // Weights: the cord's root at the top; lower down, the thighs share it (each more on
        // its own side).
        let weights = |u: f32, v: f32| -> [(Joint, f32); 3] {
            let legs = (v * 0.8).min(0.7);
            let left = 0.5 + u;
            [
                (Joint::Root, 1.0 - legs),
                (Joint::HipL, legs * left),
                (Joint::HipR, legs * (1.0 - left)),
            ]
        };
        // Both faces, a hide's thickness (3 mm) apart.
        for (side, flip) in [(1.0f32, front < 0.0), (-1.0, front > 0.0)] {
            let base = b.out.positions.len() as u32;
            for (i, row) in sheet.iter().enumerate() {
                let v = i as f32 / rows as f32;
                for (k, &(p, n)) in row.iter().enumerate() {
                    let u = k as f32 / cols as f32 - 0.5;
                    let face = n * side;
                    b.vertex(p + face * 0.0015, face, &weights(u, v));
                }
            }
            b.grid(base, rows, cols, flip);
        }
    }
    b.out
}

/// A chest band: a strip of hide wrapped round the chest over the breasts, on the skin.
pub fn chest_band(a: &Appearance) -> Anatomy {
    let a = a.clone().sanitized();
    let dims = Proportions::of(&a);
    let skin = Skin::trunk(&a);
    let h = dims.stature;
    let chest = rest_positions(&dims)[Joint::Chest.index()];
    let mut b = Builder {
        out: Anatomy {
            bind: bind_pose(&dims),
            ..Anatomy::default()
        },
    };
    let (ring, rows) = (72u32, 6u32);
    // From under the breasts to over them, wrapped close (its tension bridges the hollow
    // between them: the band is pushed out to the line joining the two fronts).
    let (lo, hi) = (chest.y - 0.045 * h, chest.y + 0.035 * h);
    let mut grid: Vec<(Vec3, Vec3)> = Vec::new();
    for i in 0..=rows {
        let y = lo + (hi - lo) * i as f32 / rows as f32;
        for k in 0..=ring {
            let t = k as f32 / ring as f32 * std::f32::consts::TAU;
            let out = Vec3::new(t.sin(), 0.0, t.cos());
            let (p, n) = skin.project(Vec3::new(chest.x, y, chest.z) + out * 0.3);
            let mut q = p + n * 0.003;
            // Spanning the cleavage: no deeper than the fronts either side.
            if out.z > 0.6 && t.sin().abs() < 0.35 {
                let side = |s: f32| {
                    let o = Vec3::new(0.35 * s, 0.0, 0.94).normalize();
                    skin.project(Vec3::new(chest.x, y, chest.z) + o * 0.3).0.z
                };
                q.z = q.z.max(side(1.0).min(side(-1.0)) + 0.003);
            }
            grid.push((q, n));
        }
    }
    let base = b.out.positions.len() as u32;
    for &(p, n) in &grid {
        b.vertex(p, n, &[(Joint::Chest, 1.0)]);
    }
    b.grid(base, rows, ring, false);
    b.out
}

/// The garments fitted as meshes (the rest are drawn as the rig's boxes).
pub const FITTED: [&str; 2] = ["hearth:loincloth", "hearth:chest_band"];

/// The fitted garments of these worn ones, as one mesh.
pub fn fitted(a: &Appearance, worn: &[&str]) -> Anatomy {
    let mut out = Anatomy::default();
    for g in worn {
        match *g {
            "hearth:loincloth" => out.merge(&loincloth(a)),
            "hearth:chest_band" => out.merge(&chest_band(a)),
            _ => {}
        }
    }
    out
}

/// The loincloth's look: its colour (linear) and what it is (0 hide, 1 plant fibre).
pub fn loincloth_look(a: &Appearance) -> Vec4 {
    let c = crate::appearance::srgb_to_linear(a.loincloth.srgb());
    let kind = match a.loincloth {
        crate::appearance::Loincloth::Hide => 0.0,
        crate::appearance::Loincloth::PlantFibre => 1.0,
    };
    Vec4::new(c[0], c[1], c[2], kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_loincloth_fits_over_the_hips_and_off_the_skin() {
        let a = Appearance::default();
        let m = loincloth(&a);
        let skin = Skin::new(&a);
        let dims = Proportions::of(&a);
        let root = rest_positions(&dims)[Joint::Root.index()];
        assert!(m.positions.len() > 500);
        assert_eq!(m.positions.len(), m.tissue.len());
        let inside = m
            .positions
            .iter()
            .filter(|p| skin.distance(**p) < -0.002)
            .count();
        assert!(inside * 20 < m.positions.len(), "{inside} under the skin");
        // Between the waist and mid-thigh.
        for p in &m.positions {
            assert!(p.y < root.y + 0.08 * dims.stature && p.y > root.y - 0.2 * dims.stature);
        }
        for w in &m.weights {
            assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-3);
        }
    }

    #[test]
    fn a_chest_band_wraps_the_chest() {
        let a = Appearance::female();
        let m = chest_band(&a);
        let skin = Skin::new(&a);
        // Round the chest, not out on the arms.
        let dims = Proportions::of(&a);
        for p in &m.positions {
            assert!(p.x.abs() < dims.chest_w * 0.75, "{p}");
        }
        let inside = m
            .positions
            .iter()
            .filter(|p| skin.distance(**p) < -0.002)
            .count();
        assert!(inside * 20 < m.positions.len(), "{inside} under the skin");
        let worn = fitted(
            &a,
            &["hearth:loincloth", "hearth:chest_band", "hearth:fur_cape"],
        );
        assert_eq!(
            worn.positions.len(),
            m.positions.len() + loincloth(&a).positions.len()
        );
    }
}

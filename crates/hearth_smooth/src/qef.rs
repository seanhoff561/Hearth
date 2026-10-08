//! The feature solve: the point nearest a set of planes (edge crossings with their surface
//! normals), as Dual Contouring places its vertices (Ju, Losasso, Schaefer and Warren 2002).

use glam::Vec3;

/// A quadratic error function: the sum of squared distances from a point to planes, each a
/// point on the surface and the surface's normal there.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Qef {
    /// AᵀA, symmetric: xx, xy, xz, yy, yz, zz.
    ata: [f32; 6],
    /// Aᵀb.
    atb: Vec3,
    /// bᵀb.
    btb: f32,
    sum: Vec3,
    count: u32,
}

impl Qef {
    /// Adds the plane through `point` with unit `normal`.
    pub fn add(&mut self, point: Vec3, normal: Vec3) {
        let n = normal;
        self.ata[0] += n.x * n.x;
        self.ata[1] += n.x * n.y;
        self.ata[2] += n.x * n.z;
        self.ata[3] += n.y * n.y;
        self.ata[4] += n.y * n.z;
        self.ata[5] += n.z * n.z;
        let d = n.dot(point);
        self.atb += n * d;
        self.btb += d * d;
        self.sum += point;
        self.count += 1;
    }

    /// Planes added.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// The mean of the planes' points.
    pub fn mass_point(&self) -> Vec3 {
        if self.count == 0 {
            Vec3::ZERO
        } else {
            self.sum / self.count as f32
        }
    }

    /// The sum of squared distances from `x` to the planes.
    pub fn error(&self, x: Vec3) -> f32 {
        let ax = self.mul(x);
        (x.dot(ax) - 2.0 * x.dot(self.atb) + self.btb).max(0.0)
    }

    /// The point minimizing the squared distances to the planes plus `lambda` times the squared
    /// distance to `toward`. The pull is what makes the solve stable: where the planes pin a
    /// point down (a corner, a crease) it barely moves it; along directions they leave free (the
    /// run of a crease, the face of a flat or gently curved surface) it holds the point at
    /// `toward`. `lambda` must be positive.
    pub fn solve(&self, toward: Vec3, lambda: f32) -> Vec3 {
        // Solve (AᵀA + λI) y = Aᵀb − AᵀA·toward for the offset y from `toward`.
        let r = self.atb - self.mul(toward);
        let [xx, xy, xz, yy, yz, zz] = self.ata;
        let (a00, a01, a02) = (xx + lambda, xy, xz);
        let (a11, a12) = (yy + lambda, yz);
        let a22 = zz + lambda;
        // Cholesky: the matrix is symmetric positive definite (eigenvalues ≥ λ).
        let l00 = a00.sqrt();
        let l10 = a01 / l00;
        let l20 = a02 / l00;
        let l11 = (a11 - l10 * l10).max(f32::MIN_POSITIVE).sqrt();
        let l21 = (a12 - l20 * l10) / l11;
        let l22 = (a22 - l20 * l20 - l21 * l21).max(f32::MIN_POSITIVE).sqrt();
        let z0 = r.x / l00;
        let z1 = (r.y - l10 * z0) / l11;
        let z2 = (r.z - l20 * z0 - l21 * z1) / l22;
        let y2 = z2 / l22;
        let y1 = (z1 - l21 * y2) / l11;
        let y0 = (z0 - l10 * y1 - l20 * y2) / l00;
        let y = Vec3::new(y0, y1, y2);
        if y.is_finite() { toward + y } else { toward }
    }

    fn mul(&self, v: Vec3) -> Vec3 {
        let [xx, xy, xz, yy, yz, zz] = self.ata;
        Vec3::new(
            xx * v.x + xy * v.y + xz * v.z,
            xy * v.x + yy * v.y + yz * v.z,
            xz * v.x + yz * v.y + zz * v.z,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_planes_meet_at_their_corner() {
        let mut q = Qef::default();
        let corner = Vec3::new(0.3, 0.6, 0.45);
        q.add(Vec3::new(0.3, 0.1, 0.9), Vec3::X);
        q.add(Vec3::new(0.0, 0.6, 0.2), Vec3::Y);
        q.add(Vec3::new(0.7, 0.9, 0.45), Vec3::Z);
        let x = q.solve(q.mass_point(), 1e-4);
        assert!(x.distance(corner) < 1e-3, "{x}");
        assert!(q.error(x) < 1e-5);
    }

    #[test]
    fn a_crease_keeps_its_line_and_the_mean_along_it() {
        // Two planes meeting along the line x = 0.5, y = 0.5 (a roof ridge running along z).
        let mut q = Qef::default();
        let n1 = Vec3::new(1.0, 1.0, 0.0).normalize();
        let n2 = Vec3::new(-1.0, 1.0, 0.0).normalize();
        q.add(Vec3::new(0.7, 0.3, 0.1), n1);
        q.add(Vec3::new(0.9, 0.1, 0.8), n1);
        q.add(Vec3::new(0.3, 0.3, 0.2), n2);
        q.add(Vec3::new(0.1, 0.1, 0.9), n2);
        let m = q.mass_point();
        let x = q.solve(m, 0.05);
        assert!((x.x - 0.5).abs() < 0.02 && (x.y - 0.5).abs() < 0.02, "{x}");
        assert!((x.z - m.z).abs() < 1e-4, "{x} vs {m}");
    }

    #[test]
    fn a_flat_surface_keeps_the_point_where_it_is_held() {
        let mut q = Qef::default();
        for (x, z) in [(0.0, 0.2), (1.0, 0.7), (0.4, 0.0), (0.6, 1.0)] {
            q.add(Vec3::new(x, 0.4, z), Vec3::Y);
        }
        let m = q.mass_point();
        let x = q.solve(m, 0.05);
        assert!(x.distance(m) < 1e-5, "{x} vs {m}");
    }
}

//! Camera: position in world space (f64), orientation, reverse-Z infinite perspective, and
//! camera-relative view matrices (the camera always sits at the origin of render space).

use glam::{DVec3, Mat4, Vec3, Vec4};

/// A perspective camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Eye position in world coordinates.
    pub pos: DVec3,
    /// Yaw in degrees (0 = looking south/+Z, 90 = west/−X, like the reference game).
    pub yaw: f32,
    /// Pitch in degrees (positive = looking down).
    pub pitch: f32,
    /// Vertical field of view in degrees.
    pub fov_y: f32,
    pub near: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            pos: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            fov_y: 70.0,
            near: 0.05,
        }
    }
}

impl Camera {
    /// Unit view direction.
    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        Vec3::new(-sy * cp, -sp, cy * cp)
    }

    /// View matrix with the camera at the origin (camera-relative rendering).
    pub fn view(&self) -> Mat4 {
        let f = self.forward();
        let up = if f.y.abs() > 0.999 { Vec3::Z } else { Vec3::Y };
        glam::camera::rh::view::look_to_mat4(Vec3::ZERO, f, up)
    }

    /// Reverse-Z infinite perspective projection (depth 1 at the near plane, 0 at infinity).
    pub fn projection(&self, aspect: f32) -> Mat4 {
        glam::camera::rh::proj::directx::perspective_infinite_reverse(
            self.fov_y.to_radians(),
            aspect.max(1e-3),
            self.near,
        )
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        self.projection(aspect) * self.view()
    }
}

/// Frustum planes for culling (camera-relative space). With an infinite far plane there are
/// five planes: left, right, bottom, top, near.
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    planes: [Vec4; 5],
}

impl Frustum {
    pub fn from_view_proj(m: Mat4) -> Self {
        let r0 = m.row(0);
        let r1 = m.row(1);
        let r2 = m.row(2);
        let r3 = m.row(3);
        let norm = |p: Vec4| p / p.truncate().length().max(1e-6);
        // Reverse-Z: the near plane is z_clip <= w (depth <= 1).
        Self {
            planes: [
                norm(r3 + r0),
                norm(r3 - r0),
                norm(r3 + r1),
                norm(r3 - r1),
                norm(r3 - r2),
            ],
        }
    }

    /// True if the axis-aligned box (camera-relative) may be visible.
    #[inline]
    pub fn intersects_aabb(&self, min: Vec3, max: Vec3) -> bool {
        for p in &self.planes {
            let n = p.truncate();
            // Farthest corner along the plane normal.
            let v = Vec3::new(
                if n.x >= 0.0 { max.x } else { min.x },
                if n.y >= 0.0 { max.y } else { min.y },
                if n.z >= 0.0 { max.z } else { min.z },
            );
            if n.dot(v) + p.w < 0.0 {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_follows_yaw_and_pitch() {
        let mut c = Camera::default();
        assert!((c.forward() - Vec3::Z).length() < 1e-5, "yaw 0 looks south");
        c.yaw = 90.0;
        assert!(
            (c.forward() - Vec3::NEG_X).length() < 1e-5,
            "yaw 90 looks west"
        );
        c.pitch = 90.0;
        assert!(
            (c.forward() - Vec3::NEG_Y).length() < 1e-5,
            "pitch 90 looks down"
        );
    }

    #[test]
    fn reverse_z_depth_and_frustum() {
        let c = Camera::default();
        let vp = c.view_proj(16.0 / 9.0);
        let near = vp * Vec4::new(0.0, 0.0, 0.05, 1.0);
        let far = vp * Vec4::new(0.0, 0.0, 10_000.0, 1.0);
        assert!(
            (near.z / near.w - 1.0).abs() < 1e-3,
            "near plane at depth 1"
        );
        assert!(
            far.z / far.w < 1e-4 && far.z / far.w >= 0.0,
            "far away tends to 0"
        );
        let f = Frustum::from_view_proj(vp);
        assert!(f.intersects_aabb(Vec3::new(-1.0, -1.0, 10.0), Vec3::new(1.0, 1.0, 12.0)));
        assert!(
            !f.intersects_aabb(Vec3::new(-1.0, -1.0, -12.0), Vec3::new(1.0, 1.0, -10.0)),
            "behind"
        );
        assert!(
            !f.intersects_aabb(Vec3::new(500.0, -1.0, 10.0), Vec3::new(510.0, 1.0, 12.0)),
            "far right"
        );
    }
}

//! Axis-aligned bounding boxes with the swept-collision clipping used by entity physics.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::coords::BlockPos;
use crate::direction::{Axis, Direction};

/// Tolerance used when clipping movement against boxes.
pub const COLLISION_EPSILON: f64 = 1.0e-7;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Aabb {
    pub min: DVec3,
    pub max: DVec3,
}

impl Aabb {
    #[inline]
    pub fn new(min: DVec3, max: DVec3) -> Self {
        Self {
            min: min.min(max),
            max: min.max(max),
        }
    }

    /// Box from explicit coordinates.
    #[inline]
    pub fn from_coords(x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64) -> Self {
        Self::new(DVec3::new(x0, y0, z0), DVec3::new(x1, y1, z1))
    }

    /// Entity-style box: centred on `feet` horizontally, extending `height` upward.
    #[inline]
    pub fn from_feet(feet: DVec3, width: f64, height: f64) -> Self {
        let h = width * 0.5;
        Self {
            min: DVec3::new(feet.x - h, feet.y, feet.z - h),
            max: DVec3::new(feet.x + h, feet.y + height, feet.z + h),
        }
    }

    /// The unit box of a block.
    #[inline]
    pub fn block(p: BlockPos) -> Self {
        let min = p.as_dvec3();
        Self {
            min,
            max: min + DVec3::ONE,
        }
    }

    #[inline]
    pub fn size(&self) -> DVec3 {
        self.max - self.min
    }

    #[inline]
    pub fn center(&self) -> DVec3 {
        (self.min + self.max) * 0.5
    }

    #[inline]
    pub fn offset(&self, d: DVec3) -> Self {
        Self {
            min: self.min + d,
            max: self.max + d,
        }
    }

    /// Grows the box by `amount` on every side (negative shrinks).
    #[inline]
    pub fn inflate(&self, amount: DVec3) -> Self {
        Self::new(self.min - amount, self.max + amount)
    }

    /// Extends the box in the direction of `d` (the swept volume of a move by `d`).
    #[inline]
    pub fn expand_towards(&self, d: DVec3) -> Self {
        let mut min = self.min;
        let mut max = self.max;
        if d.x < 0.0 {
            min.x += d.x
        } else {
            max.x += d.x
        }
        if d.y < 0.0 {
            min.y += d.y
        } else {
            max.y += d.y
        }
        if d.z < 0.0 {
            min.z += d.z
        } else {
            max.z += d.z
        }
        Self { min, max }
    }

    #[inline]
    pub fn union(&self, o: &Aabb) -> Self {
        Self {
            min: self.min.min(o.min),
            max: self.max.max(o.max),
        }
    }

    /// Strict overlap (touching faces do not count).
    #[inline]
    pub fn intersects(&self, o: &Aabb) -> bool {
        self.min.x < o.max.x
            && self.max.x > o.min.x
            && self.min.y < o.max.y
            && self.max.y > o.min.y
            && self.min.z < o.max.z
            && self.max.z > o.min.z
    }

    #[inline]
    pub fn contains_point(&self, p: DVec3) -> bool {
        p.x >= self.min.x
            && p.x < self.max.x
            && p.y >= self.min.y
            && p.y < self.max.y
            && p.z >= self.min.z
            && p.z < self.max.z
    }

    /// Range of block positions this box touches (inclusive min, inclusive max).
    pub fn block_range(&self) -> (BlockPos, BlockPos) {
        (
            BlockPos::containing(self.min),
            BlockPos::containing(self.max - DVec3::splat(COLLISION_EPSILON)),
        )
    }

    /// Clips a movement `offset` along `axis` so that `self` (moving) does not enter `other`
    /// (static). Returns the possibly shortened offset.
    #[inline]
    pub fn clip_axis(&self, other: &Aabb, axis: Axis, offset: f64) -> f64 {
        let (a, b) = match axis {
            Axis::X => (1, 2),
            Axis::Y => (0, 2),
            Axis::Z => (0, 1),
        };
        let i = axis.index();
        // Overlap on the two other axes is required for any collision on this one.
        if !(self.max[a] > other.min[a] + COLLISION_EPSILON
            && self.min[a] < other.max[a] - COLLISION_EPSILON
            && self.max[b] > other.min[b] + COLLISION_EPSILON
            && self.min[b] < other.max[b] - COLLISION_EPSILON)
        {
            return offset;
        }
        if offset > 0.0 && self.max[i] <= other.min[i] + COLLISION_EPSILON {
            let max_move = other.min[i] - self.max[i];
            if max_move < offset {
                return max_move.max(0.0);
            }
        } else if offset < 0.0 && self.min[i] >= other.max[i] - COLLISION_EPSILON {
            let max_move = other.max[i] - self.min[i];
            if max_move > offset {
                return max_move.min(0.0);
            }
        }
        offset
    }

    /// Ray–box intersection (slab test). Returns the entry distance along `dir` and the face that
    /// was hit, or `None`. A ray starting inside the box reports t = 0 and the face it would
    /// exit through is not reported (returns the nearest face instead).
    pub fn ray_intersect(&self, origin: DVec3, dir: DVec3, max_t: f64) -> Option<(f64, Direction)> {
        let mut t_min = 0.0f64;
        let mut t_max = max_t;
        let mut face = Direction::Up;
        for axis in Axis::ALL {
            let i = axis.index();
            let o = origin[i];
            let d = dir[i];
            if d.abs() < 1e-12 {
                if o < self.min[i] || o > self.max[i] {
                    return None;
                }
                continue;
            }
            let inv = 1.0 / d;
            let mut t0 = (self.min[i] - o) * inv;
            let mut t1 = (self.max[i] - o) * inv;
            // Face we enter through on this axis.
            let mut enter_face = match axis {
                Axis::X => Direction::West,
                Axis::Y => Direction::Down,
                Axis::Z => Direction::North,
            };
            if t0 > t1 {
                std::mem::swap(&mut t0, &mut t1);
                enter_face = enter_face.opposite();
            }
            if t0 > t_min {
                t_min = t0;
                face = enter_face;
            }
            t_max = t_max.min(t1);
            if t_min > t_max {
                return None;
            }
        }
        Some((t_min, face))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_falling_onto_block() {
        let player = Aabb::from_feet(DVec3::new(0.5, 1.2, 0.5), 0.6, 1.8);
        let ground = Aabb::block(BlockPos::new(0, 0, 0));
        let dy = player.clip_axis(&ground, Axis::Y, -0.5);
        assert!((dy - -0.2).abs() < 1e-9);
        // No horizontal overlap -> no clipping.
        let far = Aabb::block(BlockPos::new(5, 0, 0));
        assert_eq!(player.clip_axis(&far, Axis::Y, -0.5), -0.5);
    }

    #[test]
    fn clip_walking_into_wall() {
        let player = Aabb::from_feet(DVec3::new(0.5, 1.0, 0.5), 0.6, 1.8);
        let wall = Aabb::block(BlockPos::new(1, 1, 0));
        let dx = player.clip_axis(&wall, Axis::X, 0.3);
        assert!((dx - 0.2).abs() < 1e-9);
        let dx_away = player.clip_axis(&wall, Axis::X, -0.3);
        assert_eq!(dx_away, -0.3);
    }

    #[test]
    fn ray_hits_expected_face() {
        let b = Aabb::block(BlockPos::new(2, 0, 0));
        let (t, face) = b
            .ray_intersect(DVec3::new(0.0, 0.5, 0.5), DVec3::X, 10.0)
            .unwrap();
        assert!((t - 2.0).abs() < 1e-9);
        assert_eq!(face, Direction::West);
        let (t, face) = b
            .ray_intersect(DVec3::new(2.5, 5.0, 0.5), DVec3::NEG_Y, 10.0)
            .unwrap();
        assert!((t - 4.0).abs() < 1e-9);
        assert_eq!(face, Direction::Up);
        assert!(
            b.ray_intersect(DVec3::new(0.0, 2.0, 0.5), DVec3::X, 10.0)
                .is_none()
        );
    }

    #[test]
    fn block_range_excludes_touching() {
        let a = Aabb::from_coords(0.0, 0.0, 0.0, 1.0, 2.0, 1.0);
        let (lo, hi) = a.block_range();
        assert_eq!(lo, BlockPos::new(0, 0, 0));
        assert_eq!(hi, BlockPos::new(0, 1, 0));
    }
}

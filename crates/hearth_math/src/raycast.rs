//! Voxel traversal (Amanatides & Woo DDA). Works in continuous, *unwrapped* coordinates; the
//! caller wraps block positions through [`crate::Planet`] when looking them up.

use glam::{DVec3, IVec3};

use crate::coords::BlockPos;
use crate::direction::Direction;

/// One voxel visited by a ray.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayStep {
    pub block: BlockPos,
    /// Face through which the ray entered this voxel (`None` for the starting voxel).
    pub face: Option<Direction>,
    /// Distance along the (normalised) ray at which the voxel was entered.
    pub t: f64,
}

/// Iterator over the voxels a ray passes through, in order, up to `max_t`.
#[derive(Debug, Clone)]
pub struct VoxelRay {
    pos: IVec3,
    step: IVec3,
    t_max: DVec3,
    t_delta: DVec3,
    limit: f64,
    t: f64,
    face: Option<Direction>,
    done: bool,
}

impl VoxelRay {
    /// `dir` need not be normalised; distances are measured along the normalised direction.
    pub fn new(origin: DVec3, dir: DVec3, max_t: f64) -> Self {
        let dir = dir.normalize_or_zero();
        let pos = BlockPos::containing(origin).as_ivec3();
        let mut step = IVec3::ZERO;
        let mut t_max = DVec3::splat(f64::INFINITY);
        let mut t_delta = DVec3::splat(f64::INFINITY);
        for i in 0..3 {
            let d = dir[i];
            if d > 0.0 {
                step[i] = 1;
                t_delta[i] = 1.0 / d;
                t_max[i] = ((pos[i] as f64 + 1.0) - origin[i]) / d;
            } else if d < 0.0 {
                step[i] = -1;
                t_delta[i] = -1.0 / d;
                t_max[i] = (pos[i] as f64 - origin[i]) / d;
            }
        }
        Self {
            pos,
            step,
            t_max,
            t_delta,
            limit: max_t,
            t: 0.0,
            face: None,
            done: dir == DVec3::ZERO,
        }
    }
}

impl Iterator for VoxelRay {
    type Item = RayStep;

    fn next(&mut self) -> Option<RayStep> {
        if self.done || self.t > self.limit {
            return None;
        }
        let out = RayStep {
            block: BlockPos::from(self.pos),
            face: self.face,
            t: self.t,
        };
        // Advance along the axis whose boundary is nearest.
        let axis = if self.t_max.x < self.t_max.y {
            if self.t_max.x < self.t_max.z { 0 } else { 2 }
        } else if self.t_max.y < self.t_max.z {
            1
        } else {
            2
        };
        if !self.t_max[axis].is_finite() {
            self.done = true;
            return Some(out);
        }
        self.t = self.t_max[axis];
        self.pos[axis] += self.step[axis];
        self.t_max[axis] += self.t_delta[axis];
        // Entered through the face opposite to the step direction.
        self.face = Some(match (axis, self.step[axis] > 0) {
            (0, true) => Direction::West,
            (0, false) => Direction::East,
            (1, true) => Direction::Down,
            (1, false) => Direction::Up,
            (2, true) => Direction::North,
            _ => Direction::South,
        });
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_down_across_cube_boundaries() {
        // From y = 40.5 down 50 blocks: crosses cubes 2 → 1 → 0 → −1.
        let steps: Vec<_> = VoxelRay::new(DVec3::new(0.5, 40.5, 0.5), DVec3::NEG_Y, 50.0).collect();
        assert_eq!(steps.first().unwrap().block, BlockPos::new(0, 40, 0));
        assert_eq!(steps.len(), 51);
        for (i, s) in steps.iter().enumerate() {
            assert_eq!(s.block.y, 40 - i as i32);
            if i > 0 {
                assert_eq!(s.face, Some(Direction::Up));
                assert!((s.t - (i as f64 - 0.5)).abs() < 1e-9);
            }
        }
        let cubes: Vec<i32> = steps.iter().map(|s| s.block.cube().y).collect();
        assert!(cubes.contains(&2) && cubes.contains(&-1));
    }

    #[test]
    fn straight_up_at_extreme_height() {
        let y0 = 16_000_000.5;
        let steps: Vec<_> = VoxelRay::new(DVec3::new(-3.5, y0, 7.5), DVec3::Y, 20.0).collect();
        assert_eq!(steps[0].block, BlockPos::new(-4, 16_000_000, 7));
        assert_eq!(steps.last().unwrap().block.y, 16_000_020);
        assert!(
            steps
                .iter()
                .skip(1)
                .all(|s| s.face == Some(Direction::Down))
        );
    }

    #[test]
    fn diagonal_visits_connected_voxels() {
        let steps: Vec<_> =
            VoxelRay::new(DVec3::new(0.2, 0.3, 0.1), DVec3::new(1.0, 0.7, -0.4), 30.0).collect();
        for w in steps.windows(2) {
            let d = w[1].block - w[0].block;
            assert_eq!(d.abs().element_sum(), 1, "face-connected steps only");
            assert!(w[1].t >= w[0].t);
        }
    }

    #[test]
    fn zero_direction_yields_nothing() {
        assert_eq!(VoxelRay::new(DVec3::ZERO, DVec3::ZERO, 5.0).count(), 0);
    }
}

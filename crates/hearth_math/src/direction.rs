//! The six axis-aligned directions and the three axes.
//!
//! Conventions (same as the reference game): +Y is up, North is −Z, South is +Z, West is −X and
//! East is +X. On the planet, north (−Z) points toward the north pole.

use glam::{DVec3, IVec3, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub const ALL: [Axis; 3] = [Axis::X, Axis::Y, Axis::Z];

    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum Direction {
    Down = 0,
    Up = 1,
    North = 2,
    South = 3,
    West = 4,
    East = 5,
}

impl Direction {
    pub const ALL: [Direction; 6] = [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ];

    /// The four horizontal directions in clockwise order starting at North.
    pub const HORIZONTAL: [Direction; 4] = [
        Direction::North,
        Direction::East,
        Direction::South,
        Direction::West,
    ];

    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub fn from_index(i: usize) -> Direction {
        Self::ALL[i % 6]
    }

    #[inline]
    pub fn offset(self) -> IVec3 {
        match self {
            Direction::Down => IVec3::new(0, -1, 0),
            Direction::Up => IVec3::new(0, 1, 0),
            Direction::North => IVec3::new(0, 0, -1),
            Direction::South => IVec3::new(0, 0, 1),
            Direction::West => IVec3::new(-1, 0, 0),
            Direction::East => IVec3::new(1, 0, 0),
        }
    }

    #[inline]
    pub fn normal(self) -> Vec3 {
        self.offset().as_vec3()
    }

    #[inline]
    pub fn normal_f64(self) -> DVec3 {
        self.offset().as_dvec3()
    }

    #[inline]
    pub fn opposite(self) -> Direction {
        match self {
            Direction::Down => Direction::Up,
            Direction::Up => Direction::Down,
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::West => Direction::East,
            Direction::East => Direction::West,
        }
    }

    #[inline]
    pub fn axis(self) -> Axis {
        match self {
            Direction::Down | Direction::Up => Axis::Y,
            Direction::North | Direction::South => Axis::Z,
            Direction::West | Direction::East => Axis::X,
        }
    }

    /// True for Up, South, East (the positive direction along the axis).
    #[inline]
    pub fn is_positive(self) -> bool {
        matches!(self, Direction::Up | Direction::South | Direction::East)
    }

    pub fn is_horizontal(self) -> bool {
        self.axis() != Axis::Y
    }

    /// Rotates a horizontal direction 90° clockwise (seen from above). Vertical directions are
    /// returned unchanged.
    pub fn rotate_cw(self) -> Direction {
        match self {
            Direction::North => Direction::East,
            Direction::East => Direction::South,
            Direction::South => Direction::West,
            Direction::West => Direction::North,
            d => d,
        }
    }

    pub fn rotate_ccw(self) -> Direction {
        match self {
            Direction::North => Direction::West,
            Direction::West => Direction::South,
            Direction::South => Direction::East,
            Direction::East => Direction::North,
            d => d,
        }
    }

    /// The horizontal direction closest to a yaw angle in degrees (0 = South, 90 = West,
    /// 180 = North, 270 = East — the reference game's convention).
    pub fn from_yaw(yaw_degrees: f32) -> Direction {
        let i = ((yaw_degrees / 90.0).round() as i32).rem_euclid(4);
        [
            Direction::South,
            Direction::West,
            Direction::North,
            Direction::East,
        ][i as usize]
    }

    /// Yaw angle in degrees that faces this horizontal direction.
    pub fn to_yaw(self) -> f32 {
        match self {
            Direction::South => 0.0,
            Direction::West => 90.0,
            Direction::North => 180.0,
            Direction::East => 270.0,
            _ => 0.0,
        }
    }

    /// Direction with the largest component along `v`.
    pub fn nearest(v: DVec3) -> Direction {
        let a = v.abs();
        if a.x >= a.y && a.x >= a.z {
            if v.x >= 0.0 {
                Direction::East
            } else {
                Direction::West
            }
        } else if a.y >= a.z {
            if v.y >= 0.0 {
                Direction::Up
            } else {
                Direction::Down
            }
        } else if v.z >= 0.0 {
            Direction::South
        } else {
            Direction::North
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Direction::Down => "down",
            Direction::Up => "up",
            Direction::North => "north",
            Direction::South => "south",
            Direction::West => "west",
            Direction::East => "east",
        }
    }

    pub fn from_name(s: &str) -> Option<Direction> {
        Self::ALL.into_iter().find(|d| d.name() == s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opposites_and_offsets() {
        for d in Direction::ALL {
            assert_eq!(d.opposite().opposite(), d);
            assert_eq!(d.offset() + d.opposite().offset(), IVec3::ZERO);
            assert_eq!(Direction::from_index(d.index()), d);
            assert_eq!(Direction::from_name(d.name()), Some(d));
        }
    }

    #[test]
    fn rotation_cycles() {
        for d in Direction::HORIZONTAL {
            assert_eq!(d.rotate_cw().rotate_cw().rotate_cw().rotate_cw(), d);
            assert_eq!(d.rotate_cw().rotate_ccw(), d);
            assert_eq!(Direction::from_yaw(d.to_yaw()), d);
        }
    }

    #[test]
    fn nearest_direction() {
        assert_eq!(
            Direction::nearest(DVec3::new(0.1, -2.0, 0.5)),
            Direction::Down
        );
        assert_eq!(
            Direction::nearest(DVec3::new(0.0, 0.0, -1.0)),
            Direction::North
        );
    }
}

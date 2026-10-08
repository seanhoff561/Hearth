//! Integer world coordinates: blocks, cubes (16³ sections), columns of cubes, and positions
//! inside a cube. All axes are `i32`; Y has no fixed range.
//!
//! These types are *not* wrap-aware on their own: canonicalisation across the planet's X seam
//! goes through [`crate::Planet`], which is the only place wrapping is implemented.

use std::fmt;
use std::ops::{Add, Sub};

use glam::{DVec3, IVec3};
use serde::{Deserialize, Serialize};

use crate::direction::Direction;

/// Edge length of a cube in blocks.
pub const CUBE_SIZE: i32 = 16;
/// log2 of [`CUBE_SIZE`].
pub const CUBE_SHIFT: u32 = 4;
/// Mask for the local part of a block coordinate.
pub const CUBE_MASK: i32 = CUBE_SIZE - 1;
/// Blocks in one cube.
pub const CUBE_VOLUME: usize = (CUBE_SIZE * CUBE_SIZE * CUBE_SIZE) as usize;
/// Blocks in one horizontal layer of a cube.
pub const CUBE_AREA: usize = (CUBE_SIZE * CUBE_SIZE) as usize;

/// Soft safety limit on |Y| (the world has no build limit; this only guards against overflow
/// in arithmetic on extreme values).
pub const Y_SOFT_LIMIT: i32 = 1 << 24;

/// Position of a block.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    pub const ORIGIN: BlockPos = BlockPos { x: 0, y: 0, z: 0 };

    #[inline]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// The block containing a continuous world position.
    #[inline]
    pub fn containing(p: DVec3) -> Self {
        Self::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)
    }

    /// The cube this block belongs to.
    #[inline]
    pub fn cube(self) -> CubePos {
        CubePos::new(
            self.x >> CUBE_SHIFT,
            self.y >> CUBE_SHIFT,
            self.z >> CUBE_SHIFT,
        )
    }

    /// Position inside the cube.
    #[inline]
    pub fn local(self) -> LocalPos {
        LocalPos::new(
            (self.x & CUBE_MASK) as u8,
            (self.y & CUBE_MASK) as u8,
            (self.z & CUBE_MASK) as u8,
        )
    }

    /// The cube column containing this block.
    #[inline]
    pub fn column(self) -> ColumnPos {
        ColumnPos::new(self.x >> CUBE_SHIFT, self.z >> CUBE_SHIFT)
    }

    #[inline]
    pub fn offset(self, d: Direction) -> Self {
        self + d.offset()
    }

    #[inline]
    pub fn up(self) -> Self {
        Self::new(self.x, self.y + 1, self.z)
    }

    #[inline]
    pub fn down(self) -> Self {
        Self::new(self.x, self.y - 1, self.z)
    }

    #[inline]
    pub fn as_ivec3(self) -> IVec3 {
        IVec3::new(self.x, self.y, self.z)
    }

    /// Minimum corner in world space.
    #[inline]
    pub fn as_dvec3(self) -> DVec3 {
        DVec3::new(self.x as f64, self.y as f64, self.z as f64)
    }

    /// Center of the block in world space.
    #[inline]
    pub fn center(self) -> DVec3 {
        self.as_dvec3() + DVec3::splat(0.5)
    }
}

impl Add<IVec3> for BlockPos {
    type Output = BlockPos;
    #[inline]
    fn add(self, o: IVec3) -> BlockPos {
        BlockPos::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub<BlockPos> for BlockPos {
    type Output = IVec3;
    #[inline]
    fn sub(self, o: BlockPos) -> IVec3 {
        IVec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl From<IVec3> for BlockPos {
    fn from(v: IVec3) -> Self {
        Self::new(v.x, v.y, v.z)
    }
}

impl fmt::Display for BlockPos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, {}, {}", self.x, self.y, self.z)
    }
}

/// Position of a 16³ cube, in cube units.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
pub struct CubePos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl CubePos {
    #[inline]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Cube containing a world position.
    #[inline]
    pub fn containing(p: DVec3) -> Self {
        BlockPos::containing(p).cube()
    }

    /// Lowest-coordinate block of the cube.
    #[inline]
    pub fn min_block(self) -> BlockPos {
        BlockPos::new(
            self.x << CUBE_SHIFT,
            self.y << CUBE_SHIFT,
            self.z << CUBE_SHIFT,
        )
    }

    /// Highest-coordinate block of the cube.
    #[inline]
    pub fn max_block(self) -> BlockPos {
        self.min_block() + IVec3::splat(CUBE_MASK)
    }

    /// World position of a local position inside this cube.
    #[inline]
    pub fn block(self, local: LocalPos) -> BlockPos {
        self.min_block() + local.as_ivec3()
    }

    #[inline]
    pub fn column(self) -> ColumnPos {
        ColumnPos::new(self.x, self.z)
    }

    /// Neighbour cube (not wrap-aware; use [`crate::Planet::cube_neighbor`] in world code).
    #[inline]
    pub fn offset(self, d: Direction) -> Self {
        let o = d.offset();
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }

    #[inline]
    pub fn as_ivec3(self) -> IVec3 {
        IVec3::new(self.x, self.y, self.z)
    }

    /// Center of the cube in world space.
    pub fn center(self) -> DVec3 {
        self.min_block().as_dvec3() + DVec3::splat(CUBE_SIZE as f64 * 0.5)
    }

    /// True if the block lies inside this cube.
    #[inline]
    pub fn contains(self, b: BlockPos) -> bool {
        b.cube() == self
    }
}

impl fmt::Display for CubePos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, {}, {}", self.x, self.y, self.z)
    }
}

/// Position of a vertical column of cubes, in cube units.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
pub struct ColumnPos {
    pub x: i32,
    pub z: i32,
}

impl ColumnPos {
    #[inline]
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    #[inline]
    pub fn cube(self, y: i32) -> CubePos {
        CubePos::new(self.x, y, self.z)
    }

    /// Lowest-coordinate block column (x, z) of this cube column.
    #[inline]
    pub fn min_block_xz(self) -> (i32, i32) {
        (self.x << CUBE_SHIFT, self.z << CUBE_SHIFT)
    }
}

impl fmt::Display for ColumnPos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, {}", self.x, self.z)
    }
}

/// Position inside a cube (each component 0..16), with a dense index in Y-Z-X order
/// (`index = y * 256 + z * 16 + x`), so horizontal layers are contiguous.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct LocalPos {
    pub x: u8,
    pub y: u8,
    pub z: u8,
}

impl LocalPos {
    #[inline]
    pub const fn new(x: u8, y: u8, z: u8) -> Self {
        debug_assert!(x < 16 && y < 16 && z < 16);
        Self { x, y, z }
    }

    #[inline]
    pub const fn index(self) -> usize {
        ((self.y as usize) << 8) | ((self.z as usize) << 4) | self.x as usize
    }

    #[inline]
    pub const fn from_index(i: usize) -> Self {
        Self {
            x: (i & 15) as u8,
            y: ((i >> 8) & 15) as u8,
            z: ((i >> 4) & 15) as u8,
        }
    }

    #[inline]
    pub fn as_ivec3(self) -> IVec3 {
        IVec3::new(self.x as i32, self.y as i32, self.z as i32)
    }

    /// Iterates all 4096 positions in index order.
    pub fn all() -> impl Iterator<Item = LocalPos> {
        (0..CUBE_VOLUME).map(LocalPos::from_index)
    }
}

/// Index of a local (x, y, z) inside a cube; components must be in 0..16.
#[inline]
pub const fn local_index(x: usize, y: usize, z: usize) -> usize {
    (y << 8) | (z << 4) | x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_to_cube_and_local_negative() {
        let b = BlockPos::new(-1, -17, 15);
        assert_eq!(b.cube(), CubePos::new(-1, -2, 0));
        assert_eq!(b.local(), LocalPos::new(15, 15, 15));
        assert_eq!(b.cube().block(b.local()), b);
    }

    #[test]
    fn huge_y_round_trips() {
        for y in [
            Y_SOFT_LIMIT,
            -Y_SOFT_LIMIT,
            i32::MAX - 20,
            i32::MIN + 20,
            3_000_000,
            -2_999_999,
        ] {
            let b = BlockPos::new(7, y, -9);
            let c = b.cube();
            assert_eq!(c.block(b.local()), b, "y={y}");
            assert!(c.contains(b));
            assert!(c.min_block().y <= y && y <= c.max_block().y);
        }
    }

    #[test]
    fn local_index_round_trip() {
        for i in 0..CUBE_VOLUME {
            let l = LocalPos::from_index(i);
            assert_eq!(l.index(), i);
            assert_eq!(local_index(l.x as usize, l.y as usize, l.z as usize), i);
        }
    }

    #[test]
    fn containing_floors() {
        assert_eq!(
            BlockPos::containing(DVec3::new(-0.5, 64.99, 0.0)),
            BlockPos::new(-1, 64, 0)
        );
        assert_eq!(
            CubePos::containing(DVec3::new(-0.01, -0.01, 16.0)),
            CubePos::new(-1, -1, 1)
        );
    }
}

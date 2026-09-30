//! Math and coordinate primitives: block/cube/column/local positions (all `i32`), the
//! wrap-aware [`Planet`] geometry, directions, AABBs with collision clipping, voxel raycasting,
//! and small deterministic hashing helpers.

pub mod aabb;
pub mod coords;
pub mod direction;
pub mod hash;
pub mod planet;
pub mod raycast;

pub use aabb::Aabb;
pub use coords::{
    BlockPos, CUBE_AREA, CUBE_MASK, CUBE_SHIFT, CUBE_SIZE, CUBE_VOLUME, ColumnPos, CubePos,
    LocalPos, Y_SOFT_LIMIT, local_index,
};
pub use direction::{Axis, Direction};
pub use planet::{Planet, PlanetError, PlanetSize, PoleCrossing};
pub use raycast::{RayStep, VoxelRay};

//! Cubic-chunk world storage.
//!
//! * [`block`] — data-driven blocks, properties and states with precomputed per-state tables.
//! * [`shape`] — collision/outline shapes derived from shape families and state properties.
//! * [`palette`] — palette-compressed block storage for one cube.
//! * [`light`] — lazily allocated 4-bit light channels.
//! * [`cube`] — a 16³ cube.
//! * [`fill`] — how far each voxel is inside the ground's surface (the smooth world's field).
//! * [`ground`] — the ground as that field: looking at it, digging and piling, slumping.
//! * [`storage`] — the loaded world: wrap-aware cube map and column heightmaps.
//! * [`query`] — raycasts against real block shapes and collision box gathering.
//! * [`water`] — finite, conserved water the player moves, and its quality.

pub mod block;
pub mod cube;
pub mod datapack;
pub mod fill;
pub mod ground;
pub mod light;
pub mod lighting;
pub mod palette;
pub mod query;
pub mod shape;
pub mod storage;
pub mod structure;
pub mod water;

pub use block::{
    Block, BlockDef, BlockError, BlockId, BlockRegistry, BlockStateId, RenderKind, RenderLayer,
    StateFlags, TintKind, VoxelKind,
};
pub use cube::{Cube, LightStatus};
pub use fill::Fill;
pub use light::{LightData, MAX_LIGHT};
pub use lighting::{Channel, LightEngine};
pub use palette::{PaletteError, PalettedBlocks};
pub use query::{BlockHit, FluidMode, collides, collision_boxes, raycast_blocks};
pub use shape::{Shape, ShapeId, ShapeKind};
pub use storage::{Column, CubeMap, NO_HEIGHT};
pub use water::{Ground, WaterBudget, WaterEnv, WaterSim};

//! Cubic-chunk world storage.
//!
//! * [`block`] — data-driven blocks, properties and states with precomputed per-state tables.
//! * [`shape`] — collision/outline shapes derived from shape families and state properties.
//! * [`palette`] — palette-compressed block storage for one cube.
//! * [`light`] — lazily allocated 4-bit light channels.
//! * [`cube`] — a 16³ cube.
//! * [`storage`] — the loaded world: wrap-aware cube map and column heightmaps.
//! * [`query`] — raycasts against real block shapes and collision box gathering.

pub mod block;
pub mod cube;
pub mod light;
pub mod palette;
pub mod query;
pub mod shape;
pub mod storage;

pub use block::{
    Block, BlockDef, BlockError, BlockId, BlockRegistry, BlockStateId, RenderKind, RenderLayer,
    StateFlags, TintKind, ToolKind, ToolTier,
};
pub use cube::{Cube, LightStatus};
pub use light::{LightData, MAX_LIGHT};
pub use palette::{PaletteError, PalettedBlocks};
pub use query::{BlockHit, FluidMode, collides, collision_boxes, raycast_blocks};
pub use shape::{Shape, ShapeId, ShapeKind};
pub use storage::{Column, CubeMap, NO_HEIGHT};

//! Things (v2 §10): what kinds of items there are (from the content: forms × materials, explicit
//! items and garments, each with a real mass, volume, footprint, size and colour), stacks of
//! them, containers with grids that hold them without overlap and within their load, nested,
//! and what a person carries.

pub mod carry;
pub mod container;
pub mod registry;
pub mod stack;

pub use carry::{Carry, Hand, Load, Path, Refusal, Root, Target, Worn};
pub use container::{Container, Misfit, Placed};
pub use hearth_content::schema::item::{ContainerSpec, Stacking};
pub use registry::{ItemKind, Items, Wear};
pub use stack::Stack;

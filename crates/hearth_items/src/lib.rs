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

/// A thing lying in the world: where it rests (the middle of its base), which way it lies, and
/// what it is.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorldItem {
    pub id: u64,
    pub stack: Stack,
    pub pos: [f64; 3],
    #[serde(default)]
    pub yaw: f32,
    /// Work left to itself here (meat drying on a rack, acorns soaking).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work: Option<Batch>,
}

/// Unattended work: a process (content id) under way, the hours it has had of the conditions
/// it needs, and the hours it spent wet.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Batch {
    pub process: String,
    #[serde(default)]
    pub hours: f32,
    #[serde(default)]
    pub wet_hours: f32,
}

/// The things lying in a world.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct WorldItems {
    pub next_id: u64,
    pub items: Vec<WorldItem>,
}

impl WorldItems {
    /// Lays a thing down; its new id.
    pub fn add(&mut self, stack: Stack, pos: [f64; 3], yaw: f32) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.items.push(WorldItem {
            id,
            stack,
            pos,
            yaw,
            work: None,
        });
        id
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut WorldItem> {
        self.items.iter_mut().find(|w| w.id == id)
    }

    pub fn take(&mut self, id: u64) -> Option<WorldItem> {
        let i = self.items.iter().position(|w| w.id == id)?;
        Some(self.items.swap_remove(i))
    }

    pub fn get(&self, id: u64) -> Option<&WorldItem> {
        self.items.iter().find(|w| w.id == id)
    }
}

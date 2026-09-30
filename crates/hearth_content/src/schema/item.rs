//! Items (`items/`). Most items are generated as form × material ("knife" × "flint"), with
//! mass and properties derived from the material; `items/` can also list explicit items.

use serde::{Deserialize, Serialize};

use super::entry;
use super::material::MaterialFilter;
use crate::IdRef;

/// How an item stacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Stacking {
    /// Each item is separate.
    #[default]
    Single,
    /// Up to this many in one grid cell (arrows in a quiver).
    Count(u16),
    /// Measured by volume in a container (berries, grain, sand).
    Bulk,
}

/// A numeric property of generated items, constant or read from the material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PropertyValue {
    Const(f32),
    /// Material field (`knapping`, `hardness_mohs`, `density_kg_m3`, `fuel_mj_kg`,
    /// `elastic_modulus_gpa`, `bending_mpa`, `workability`, `durability`) times a scale.
    FromMaterial {
        field: String,
        scale: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Property {
    /// e.g. `sharp_edge`, `hard_hammer`, `fuel`, `bow_stave`.
    pub name: String,
    pub value: PropertyValue,
}

entry! {
    /// A shape that can be made of many materials.
    pub struct ItemForm in "items/forms", schema 1, name name {
        /// Display name pattern; `{material}` is replaced by the material name.
        pub name: String,
        pub materials: MaterialFilter,
        /// Bounding box in metres.
        pub size_m: [f32; 3],
        /// Fraction of the box filled with material (mass = box × fill × density).
        #[serde(default = "full")]
        pub fill: f32,
        /// Inventory grid cells (width, height).
        pub footprint: (u8, u8),
        #[serde(default)]
        pub stacking: Stacking,
        #[serde(default)]
        pub properties: Vec<Property>,
        #[serde(default)]
        pub tags: Vec<String>,
    }
}

fn full() -> f32 {
    1.0
}

entry! {
    /// An item that isn't a simple form × material (composites, containers, special things).
    pub struct Item in "items/items", schema 1, name name {
        pub name: String,
        #[serde(default)]
        pub material: Option<IdRef>,
        pub mass_kg: f32,
        pub volume_l: f32,
        pub footprint: (u8, u8),
        #[serde(default)]
        pub stacking: Stacking,
        /// Constant properties.
        #[serde(default)]
        pub properties: Vec<(String, f32)>,
        #[serde(default)]
        pub tags: Vec<String>,
    }
}

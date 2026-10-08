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

/// What a container holds and how it is carried (v2 §10.2).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ContainerSpec {
    /// Its grid (cells wide and high).
    pub grid: (u8, u8),
    /// The most it holds (kg).
    pub max_kg: f32,
    /// The liquid it holds (litres), if it holds liquid.
    #[serde(default)]
    pub liquid_l: f32,
    /// Carried on the back (a back basket, a bundle tied on).
    #[serde(default)]
    pub back: bool,
}

/// What a thing in hand does when it is used with nothing aimed at (E §3.2). Food is eaten, a
/// water skin with water in it drunk from, and a thing a wound is treated with (a process's
/// `treats`) laid on the wound, whatever this says; a thing with none is struck with in the
/// fist that holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Use {
    /// Held up high to see by and to keep animals off (a burning brand).
    HoldUp,
    /// Driven point first along the look (a spear, a pole, a digging stick).
    Thrust,
    /// Swung in an arc across the body (a stick, a hafted axe, an adze).
    Swing,
    /// Drawn across edge first in a short arc (a flake, a blade, a sickle).
    Slash,
    /// Stabbed point first, a short way (an awl, a point, an arrow in the hand).
    Stab,
    /// Brought down hard in the fist (a cobble, a hand axe).
    Strike,
    /// Drawn while held and loosed when let go (a bow, with an arrow carried).
    Draw,
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
        /// It holds things.
        #[serde(default)]
        pub container: Option<ContainerSpec>,
        /// Attachment points it can hang from (`tie`, `belt_loop`, `strap`).
        #[serde(default)]
        pub hangs_on: Vec<String>,
        /// What it does in the hand with nothing aimed at.
        #[serde(default)]
        pub primary: Option<Use>,
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
        /// Bounding box in metres (a cube of its volume when not given).
        #[serde(default)]
        pub size_m: Option<[f32; 3]>,
        #[serde(default)]
        pub container: Option<ContainerSpec>,
        #[serde(default)]
        pub hangs_on: Vec<String>,
        /// What it does in the hand with nothing aimed at.
        #[serde(default)]
        pub primary: Option<Use>,
    }
}

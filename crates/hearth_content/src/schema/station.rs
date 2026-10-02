//! Workstations (`workstations/`) and construction pieces (`construction/`): physical builds
//! that enable processes and make up structures.

use serde::{Deserialize, Serialize};

use super::material::MaterialFilter;
use super::process::{Input, ToolReq};
use super::{Duration, entry};
use crate::IdRef;

/// What a workstation makes possible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Capability {
    /// Can reach this temperature (with good fuel and draught).
    MaxTempC(f32),
    /// Forced air (bellows).
    Airflow,
    /// Holds work in a closed chamber (kiln, oven).
    Enclosure,
    Smoke,
    Drying,
    Grinding,
    Spinning,
    Weaving,
    Anvil,
    /// Holds water (pits, troughs).
    Watertight,
}

entry! {
    /// A workstation built in the world.
    pub struct Workstation in "workstations", schema 1, name name {
        pub name: String,
        /// Materials and items used to build it.
        pub parts: Vec<Input>,
        pub provides: Vec<Capability>,
        #[serde(default)]
        pub knowledge: Option<IdRef>,
        pub build: Duration,
        /// The block that stands for it in the world.
        #[serde(default)]
        pub block: Option<IdRef>,
        /// What laying it out is called ("lay a fire").
        #[serde(default)]
        pub action: Option<String>,
    }
}

/// How pieces are joined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Joint {
    Stacked,
    Lashed,
    Pegged,
    MortiseTenon,
    Nailed,
    Mortared,
    Woven,
}

/// How a piece sits in the block it is put up in (V2-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PieceShape {
    /// Upright in the middle (a post, a pole).
    Post,
    /// Lying across the top of the block along the way it faces (a beam, a joist, a lintel).
    Beam,
    /// A thin wall at the side it faces (wattle, bark, a hide, brush).
    Panel,
    /// A layer over the block's floor (bark laid flat, floorboards).
    Layer,
    /// A pitched covering rising toward the way it faces (thatch, bark or hides on rafters).
    Roof,
    /// A wall the block's height and half its width, at the side it faces (dry stone, log
    /// courses, mudbrick).
    Wall,
    /// The whole block (snow blocks, rammed earth, stone fill).
    #[default]
    Block,
}

entry! {
    /// A building piece (post, beam, wattle panel, thatch, stone course...), put up in a block of
    /// the world from what it is made of.
    pub struct ConstructionPiece in "construction", schema 1, name name {
        /// Display name pattern with `{material}`.
        pub name: String,
        pub materials: MaterialFilter,
        /// The piece's own size (m): a post's thickness and height, a wall's length, height and
        /// thickness. Its mass and strength come from it and the material.
        pub size_m: [f32; 3],
        #[serde(default = "full")]
        pub fill: f32,
        pub joints: Vec<Joint>,
        #[serde(default)]
        pub knowledge: Option<IdRef>,
        pub build: Duration,
        /// Sheds rain when laid at this pitch or steeper (roofing pieces).
        #[serde(default)]
        pub sheds_rain_min_pitch_deg: Option<f32>,
        /// Thermal resistance per piece thickness (m²·K/W).
        #[serde(default)]
        pub insulation_r: Option<f32>,
        /// How it sits in its block.
        #[serde(default)]
        pub shape: PieceShape,
        /// What putting one up uses (its material is the first's).
        #[serde(default)]
        pub inputs: Vec<Input>,
        #[serde(default)]
        pub tools: Vec<ToolReq>,
        /// A roof's pitch (degrees).
        #[serde(default)]
        pub pitch_deg: Option<f32>,
        /// What putting it up is called ("set a post").
        #[serde(default)]
        pub action: Option<String>,
        /// How hard the work is (METs).
        #[serde(default = "building_work")]
        pub mets: f32,
    }
}

fn full() -> f32 {
    1.0
}

fn building_work() -> f32 {
    4.0
}

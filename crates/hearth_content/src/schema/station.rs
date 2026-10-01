//! Workstations (`workstations/`) and construction pieces (`construction/`): physical builds
//! that enable processes and make up structures.

use serde::{Deserialize, Serialize};

use super::material::MaterialFilter;
use super::process::Input;
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

entry! {
    /// A building piece (post, beam, wattle panel, thatch bundle, stone course...).
    pub struct ConstructionPiece in "construction", schema 1, name name {
        /// Display name pattern with `{material}`.
        pub name: String,
        pub materials: MaterialFilter,
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
    }
}

fn full() -> f32 {
    1.0
}

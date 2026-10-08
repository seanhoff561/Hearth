//! A person's meshes, made together (Amendment E §8, E7): the sculpted skin with the garments
//! fitted to it, the hair and the eyes. Meshing the skin takes a fraction of a second, so this
//! is meant for a worker thread; the meshes depend only on the appearance, the worn fitted
//! garments and the level of detail.

use crate::anatomy::{Anatomy, anatomy};
use crate::appearance::Appearance;
use crate::eyes::{EyeMesh, eyes};
use crate::garment::fitted;
use crate::hair::{HairMesh, hair};

/// The levels of detail: the skin's cell (m) and whether hair and eyes are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Detail {
    /// Close: 6 mm cells, hair and eyes.
    Close,
    /// Within some tens of metres: 12 mm, hair and eyes.
    Near,
    /// Far: 24 mm, hair; no eyes.
    Far,
}

impl Detail {
    pub fn cell(self) -> f32 {
        match self {
            Detail::Close => 0.006,
            Detail::Near => 0.012,
            Detail::Far => 0.024,
        }
    }

    /// The detail for a person this far from the eye (m).
    pub fn at(distance: f32) -> Self {
        if distance < 4.0 {
            Detail::Close
        } else if distance < 14.0 {
            Detail::Near
        } else {
            Detail::Far
        }
    }
}

#[derive(Debug, Clone)]
pub struct Meshes {
    pub body: Anatomy,
    pub hair: HairMesh,
    pub eyes: Option<EyeMesh>,
}

/// The meshes of a person of this appearance wearing these garments, at this detail.
pub fn meshes(a: &Appearance, worn: &[&str], detail: Detail) -> Meshes {
    let mut body = anatomy(a, detail.cell());
    body.merge(&fitted(a, worn));
    Meshes {
        body,
        hair: hair(a),
        eyes: (detail != Detail::Far).then(|| eyes(a)),
    }
}

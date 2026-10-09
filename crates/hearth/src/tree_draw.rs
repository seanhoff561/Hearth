//! Trees drawn as meshes (Amendment S §7.1–7.2, S5): each species' look, its bark and leaves
//! coloured as the distant terrain colours its blocks (`hearth_lod::BlockColors`, so the near
//! trees and the far crowns agree), and its meshes by stage, variant and detail, each kept on the
//! GPU under one key (`hearth_render::trees`).

use glam::Vec3;
use hearth_content::schema::flora::Crown;
use hearth_flora::mesh::{Detail, Options, TreeMesh};
use hearth_flora::{Stage, VARIANTS};
use hearth_lod::BlockColors;
use hearth_render::trees::TreeInstance;
use hearth_worldgen::trees::Forest;

/// How a species' trees look as instances.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeLook {
    /// Bark colour (sRGB, packed) | its pattern << 24 (`BarkPattern`, as `tree.wgsl` has them).
    pub bark: u32,
    /// Leaf colour (sRGB, packed) | tint kind << 24 (the distant terrain's: the kind in the low
    /// two bits, deciduous, evergreen or plain, its variant above).
    pub leaf: u32,
}

/// Every species' look, in the forest's order: its trunk's bark and its leaves as the distant
/// terrain colours those blocks.
pub fn looks(forest: &Forest, colors: &BlockColors) -> Vec<TreeLook> {
    forest
        .templates
        .species
        .iter()
        .zip(&forest.blocks)
        .map(|(sp, b)| {
            let (leaf, kind) = colors.get(b.leaves);
            TreeLook {
                bark: colors.side(b.log[1]) & 0xff_ffff | (sp.form.bark as u32) << 24,
                leaf: leaf & 0xff_ffff | (kind as u32) << 24,
            }
        })
        .collect()
}

/// The key a mesh is kept under in the tree pass: its species, stage, variant and detail.
pub fn mesh_key(species: usize, stage: Stage, variant: u8, detail: Detail) -> u64 {
    (species as u64) << 24
        | (stage.index() as u64) << 16
        | ((variant % VARIANTS) as u64) << 8
        | detail as u64
}

/// A species' tree at a stage and variant, meshed at a detail from the skeleton its blocks are
/// made of (`template::variant_skeleton`), so that the two agree.
pub fn mesh(
    forest: &Forest,
    species: usize,
    stage: Stage,
    variant: u8,
    detail: Detail,
) -> TreeMesh {
    let sp = &forest.templates.species[species];
    let variant = variant % VARIANTS;
    let sk = hearth_flora::template::variant_skeleton(sp, stage, variant);
    let opts = Options::new(
        detail,
        sp.form.leaf,
        sp.form.crown == Crown::Palm,
        sp.form.foliage_density,
    );
    let seed = hearth_math::hash::derive_seed(variant as u64 * 31 + stage.index() as u64, &sp.id);
    hearth_flora::mesh::mesh(&sk, opts, seed)
}

/// A tree drawn: its look, where its skeleton's origin is (camera-relative), the climate where
/// it stands (`hearth_env::tint::encode`) with the snow on it (0..255), the sky's and the block
/// light's levels there (0..15) and its sway's phase.
pub fn instance(
    look: TreeLook,
    origin: Vec3,
    climate: u32,
    snow: u8,
    light: (u8, u8),
    phase: u8,
) -> TreeInstance {
    TreeInstance {
        origin: origin.to_array(),
        bark: look.bark,
        turn: [1.0, 0.0, 0.0, 1.0],
        leaf: look.leaf,
        climate: climate & 0xff_ffff | (snow as u32) << 24,
        light: (light.0.min(15) as u32) | (light.1.min(15) as u32) << 4 | (phase as u32) << 8,
        pad: 0,
    }
}

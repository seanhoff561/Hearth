//! Trees drawn as meshes (Amendment S §7.1–7.2, S5): each species' look, its bark and leaves
//! coloured as the distant terrain colours its blocks (`hearth_lod::BlockColors`, so the near
//! trees and the far crowns agree), and its meshes by stage, variant and detail, each kept on the
//! GPU under one key (`hearth_render::trees`).

use glam::Vec3;
use hearth_content::schema::flora::Crown;
use hearth_flora::mesh::{Detail, Options, TreeMesh};
use hearth_flora::{Stage, VARIANTS};
use hearth_lod::BlockColors;
use hearth_render::trees::{BARK_CHARRED, TreeInstance};
use hearth_worldgen::cubegen::features::Remains;
use hearth_worldgen::trees::Forest;

/// Nearer than this (m) a tree is drawn in full, nearer than `REDUCED_M` without its twigs,
/// farther with few-sided stems and crossed cards (`hearth_flora::mesh::Detail`).
pub const FULL_M: f32 = 24.0;
pub const REDUCED_M: f32 = 80.0;

/// The detail a tree is drawn at from a distance (m).
pub fn detail_at(distance: f32) -> Detail {
    if distance < FULL_M {
        Detail::Full
    } else if distance < REDUCED_M {
        Detail::Reduced
    } else {
        Detail::Low
    }
}

/// A tree is felled this high (m): its stump is its foot's block (`Remains::Stump`).
pub const STUMP_M: f32 = 1.0;

/// What of a tree is left, as a number for its mesh's key.
fn remains_index(r: Remains) -> u64 {
    match r {
        Remains::Living => 0,
        Remains::Snag => 1,
        Remains::Charred => 2,
        Remains::Stump => 3,
    }
}

/// Which tree a mesh draws, and how much of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshOf {
    pub species: usize,
    pub stage: Stage,
    pub variant: u8,
    pub remains: Remains,
    /// Only what falls of it, felled: all of it above its stump.
    pub felled: bool,
}

impl MeshOf {
    /// A tree as it stands, or what is left of it.
    pub fn standing(species: usize, stage: Stage, variant: u8, remains: Remains) -> Self {
        Self {
            species,
            stage,
            variant,
            remains,
            felled: false,
        }
    }
}

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

/// The key a mesh is kept under in the tree pass: its species, stage, variant, what is left of
/// the tree (and whether only what falls of it, felled) and detail.
pub fn mesh_key(of: MeshOf, detail: Detail) -> u64 {
    (of.species as u64) << 24
        | (of.stage.index() as u64) << 16
        | ((of.variant % VARIANTS) as u64) << 8
        | (remains_index(of.remains) | if of.felled { 4 } else { 0 }) << 4
        | detail as u64
}

/// A species' tree at a stage and variant, what is left of it, meshed at a detail from the
/// skeleton its blocks are made of (`template::variant_skeleton`), so that the two agree: a
/// snag without its leaves, a burned tree without them and its twigs (the limbs its blocks
/// keep, `PlacedTree::shows`), a stump its foot's block high; felled, all of it but its stump.
pub fn mesh(forest: &Forest, of: MeshOf, detail: Detail) -> TreeMesh {
    let sp = &forest.templates.species[of.species];
    let variant = of.variant % VARIANTS;
    let sk = hearth_flora::template::variant_skeleton(sp, of.stage, variant);
    let mut opts = Options::new(
        detail,
        sp.form.leaf,
        sp.form.crown == Crown::Palm,
        sp.form.foliage_density,
    );
    match of.remains {
        Remains::Living => {}
        Remains::Snag => opts.leaves = false,
        Remains::Charred => {
            opts.leaves = false;
            opts.min_diameter = 0.15;
        }
        Remains::Stump => {
            opts.leaves = false;
            opts.cut = Some(STUMP_M);
        }
    }
    if of.felled {
        opts.above = Some(STUMP_M);
    }
    let seed =
        hearth_math::hash::derive_seed(variant as u64 * 31 + of.stage.index() as u64, &sp.id);
    hearth_flora::mesh::mesh(&sk, opts, seed)
}

/// A tree turned down about a level axis through its origin (a tree falling): by `angle`
/// (radians, 0..π) about `axis` (level), as `TreeInstance::tilt` holds it.
pub fn tilt(angle: f32, axis: Vec3) -> u32 {
    use std::f32::consts::{PI, TAU};
    let a = (angle.clamp(0.0, PI) / PI * 65535.0).round() as u32;
    let heading = axis.z.atan2(axis.x).rem_euclid(TAU);
    let h = ((heading / TAU * 65536.0).round() as u32) & 0xffff;
    a | h << 16
}

/// The light where a tree stands: the sky's level at its foot (its stem and roots) and above
/// its crown (its limbs and leaves, lit from outside), and the block light's (0..15 each).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeLight {
    pub foot: u8,
    pub crown: u8,
    pub block: u8,
}

impl TreeLight {
    /// In the open.
    pub const OPEN: Self = Self {
        foot: 15,
        crown: 15,
        block: 0,
    };
}

/// A tree drawn: its look, where its skeleton's origin is (camera-relative) and its turn in
/// the ground's plane (`PlacedTree::frame`), the climate where it stands
/// (`hearth_env::tint::encode`) with the snow on it (0..255), the light there, its sway's phase
/// and whether fire charred it.
#[allow(clippy::too_many_arguments)]
pub fn instance(
    look: TreeLook,
    origin: Vec3,
    turn: [f32; 4],
    climate: u32,
    snow: u8,
    light: TreeLight,
    phase: u8,
    charred: bool,
) -> TreeInstance {
    let flags = if charred { BARK_CHARRED << 24 } else { 0 };
    TreeInstance {
        origin: origin.to_array(),
        bark: look.bark | flags,
        turn,
        leaf: look.leaf,
        climate: climate & 0xff_ffff | (snow as u32) << 24,
        light: (light.foot.min(15) as u32)
            | (light.block.min(15) as u32) << 4
            | (phase as u32) << 8
            | (light.crown.min(15) as u32) << 16,
        tilt: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A falling tree's turn packs into its instance: a quarter turn about the z axis, half a
    /// turn about the x axis, and none.
    #[test]
    fn a_tilt_packs_its_angle_and_level_axis() {
        let t = tilt(std::f32::consts::FRAC_PI_2, Vec3::Z);
        assert_eq!(t & 0xffff, 32768);
        assert_eq!(t >> 16, 16384, "a quarter of a turn from +x");
        let t = tilt(std::f32::consts::PI, Vec3::NEG_X);
        assert_eq!((t & 0xffff, t >> 16), (65535, 32768));
        assert_eq!(tilt(0.0, Vec3::X), 0, "standing");
        // Felled and standing meshes are kept apart.
        let of = MeshOf::standing(3, Stage::Mature, 1, Remains::Living);
        let felled = MeshOf { felled: true, ..of };
        assert_ne!(mesh_key(of, Detail::Full), mesh_key(felled, Detail::Full));
        assert_ne!(
            mesh_key(felled, Detail::Full),
            mesh_key(
                MeshOf::standing(3, Stage::Mature, 1, Remains::Stump),
                Detail::Full
            )
        );
    }
}

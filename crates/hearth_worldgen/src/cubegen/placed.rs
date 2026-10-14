//! A tree where the generator grows one, as the world, the drawing and the felling see it: its
//! species, stage and variant, where its skeleton stands and turns, what is left of it, and its
//! blocks.

use hearth_world::BlockStateId;

use super::succession::Remains;
use crate::vegetation::Vegetation;

/// A tree of a real species where the generator grows one.
#[derive(Debug, Clone)]
pub struct PlacedTree {
    /// Index into the generator's forest.
    pub species: usize,
    pub stage: hearth_flora::Stage,
    pub template: std::sync::Arc<hearth_flora::TreeTemplate>,
    /// The template's variant (its skeleton's, so its mesh is the same tree).
    pub variant: u8,
    pub turn: hearth_flora::Turn,
    /// The block over the ground the trunk stands in.
    pub foot: [i32; 3],
    /// What of it is left: all of it, a snag, a charred trunk or a stump.
    pub remains: Remains,
    /// A young tree of the understory, where no canopy tree stands.
    pub understory: bool,
}

impl PlacedTree {
    /// Where its skeleton stands (`hearth_flora::template::variant_skeleton`'s metres): the
    /// foot block's minimum corner and offset, and the turn in the ground's plane (x' = t0 x +
    /// t1 z, z' = t2 x + t3 z), as its blocks are turned (`Turn::apply_f`): a world point is
    /// `origin + (t0 x + t1 z, y, t2 x + t3 z)`.
    pub fn frame(&self) -> ([f64; 3], [f32; 4]) {
        let corner = self.template.corner;
        let f = |x: f32, z: f32| self.turn.apply_f(x, z, corner);
        let (ox, oz) = f(0.0, 0.0);
        let (ax, az) = f(1.0, 0.0);
        let (bx, bz) = f(0.0, 1.0);
        (
            [
                self.foot[0] as f64 + ox as f64,
                self.foot[1] as f64,
                self.foot[2] as f64 + oz as f64,
            ],
            [ax - ox, bx - ox, az - oz, bz - oz],
        )
    }

    /// What the tree is, as a number: its species, stage and what is left of it. A tree cut
    /// into is kept as cut while it stays what it was (`Vegetation::is_cut`).
    pub fn what(&self) -> u32 {
        let remains = match self.remains {
            Remains::Living => 0,
            Remains::Snag => 1,
            Remains::Charred => 2,
            Remains::Stump => 3,
        };
        (self.species as u32 & 0xffff) << 16 | (self.stage.index() as u32) << 8 | remains
    }

    /// Whether it has been cut into (drawn as its blocks, not as its mesh).
    pub fn is_cut(&self, veg: &Vegetation) -> bool {
        veg.is_cut(self.foot[0], self.foot[2], self.what())
    }

    /// Whether a part of the template is drawn, for what is left of the tree.
    pub fn shows(&self, c: [i16; 3], part: hearth_flora::Part) -> bool {
        use hearth_flora::Part;
        match self.remains {
            Remains::Living => true,
            Remains::Snag => !matches!(part, Part::Leaves),
            Remains::Charred => match part {
                Part::Leaves => false,
                Part::Branch { thickness, .. } => thickness > 2,
                Part::Log { .. } => true,
            },
            Remains::Stump => c[1] <= 0 && !matches!(part, Part::Leaves),
        }
    }

    /// The block a part of it is drawn with (charred where fire killed it).
    pub fn state(&self, forest: &crate::trees::Forest, part: hearth_flora::Part) -> BlockStateId {
        match (&forest.charred, self.remains) {
            (Some(c), Remains::Charred) => c.state(part),
            _ => forest.blocks[self.species].state(part),
        }
    }

    /// Its blocks where they stand (what is left of it), as template parts turned with it.
    pub fn blocks(&self) -> impl Iterator<Item = (hearth_math::BlockPos, hearth_flora::Part)> + '_ {
        let shown = self
            .template
            .blocks
            .iter()
            .filter(|(c, part)| self.shows(*c, *part));
        shown.map(|(c, part)| {
            let (dx, dz) = self
                .turn
                .apply(c[0] as i32, c[2] as i32, self.template.corner);
            (
                hearth_math::BlockPos::new(
                    self.foot[0] + dx,
                    self.foot[1] + c[1] as i32,
                    self.foot[2] + dz,
                ),
                self.turn.part(*part),
            )
        })
    }
}

//! What movement asks of the world: collision boxes, water, the ground underfoot.

use hearth_math::{Aabb, BlockPos};
use hearth_world::{BlockRegistry, CubeMap, StateFlags};

/// The surface under the feet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ground {
    /// Slipperiness: 0.6 normal, 0.98 ice.
    pub friction: f64,
    /// Multiplier on walking speed (mud, soul-sucking bog).
    pub speed: f64,
    /// Multiplier on jumps.
    pub jump: f64,
    /// 0–1: how much a fall onto it is softened.
    pub cushion: f64,
}

impl Default for Ground {
    fn default() -> Self {
        Self {
            friction: 0.6,
            speed: 1.0,
            jump: 1.0,
            cushion: 0.0,
        }
    }
}

/// Plants or foliage in a block, as a body moving through them feels them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plant {
    /// 0–1: how dense and stiff the stems and leaves are; a body they cover is slowed by this
    /// share of its pace.
    pub drag: f64,
    /// How high above the block's floor they reach (m, 0..=1).
    pub top: f64,
}

/// The world as movement sees it.
pub trait Terrain {
    /// Pushes the collision boxes of the blocks overlapping `area` (world coordinates, in the
    /// frame of `area`: across the date line boxes keep the area's side).
    fn boxes(&self, area: &Aabb, out: &mut Vec<Aabb>);
    /// How high the water in a block reaches (0 none – 1 full).
    fn water(&self, pos: BlockPos) -> f64;
    /// The surface of a block stood on.
    fn ground(&self, pos: BlockPos) -> Ground;
    /// Whether a block can be climbed like a ladder (vines, rope ladders).
    fn climbable(&self, pos: BlockPos) -> bool;
    /// The plant or foliage in a block that a body pushes through, if any.
    fn plant(&self, _pos: BlockPos) -> Option<Plant> {
        None
    }
    /// The canonical X of a position (the world wraps east–west).
    fn wrap_x(&self, x: f64) -> f64 {
        x
    }
}

/// A loaded block world: its cubes and its block types. Unloaded cubes are solid (nothing falls
/// through the world before it arrives).
pub struct BlockWorld<'a> {
    pub map: &'a CubeMap,
    pub reg: &'a BlockRegistry,
}

impl Terrain for BlockWorld<'_> {
    fn boxes(&self, area: &Aabb, out: &mut Vec<Aabb>) {
        let (lo, hi) = area.block_range();
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                for x in lo.x..=hi.x {
                    let p = BlockPos::new(x, y, z);
                    let Some(s) = self.map.block(p) else {
                        out.push(Aabb::block(p));
                        continue;
                    };
                    if !self.reg.has(s, StateFlags::HAS_COLLISION) {
                        continue;
                    }
                    if self.reg.has(s, StateFlags::FULL_COLLISION) {
                        out.push(Aabb::block(p));
                        continue;
                    }
                    let base = p.as_dvec3();
                    for b in &self.reg.collision_shape(s).boxes {
                        out.push(b.offset(base));
                    }
                }
            }
        }
    }

    fn water(&self, pos: BlockPos) -> f64 {
        let Some(s) = self.map.block(pos) else {
            return 0.0;
        };
        if !self.reg.has(s, StateFlags::WATER) {
            return 0.0;
        }
        let amount = self.reg.fluid_amount(s);
        // Water under water fills its block; a surface block shows its eighths.
        let above = self
            .map
            .block(BlockPos::new(pos.x, pos.y + 1, pos.z))
            .is_some_and(|a| self.reg.has(a, StateFlags::WATER));
        if above || amount == 0 {
            1.0
        } else {
            amount as f64 / 9.0
        }
    }

    fn ground(&self, pos: BlockPos) -> Ground {
        let Some(s) = self.map.block(pos) else {
            return Ground::default();
        };
        let d = &self.reg.block_of(s).def;
        Ground {
            friction: d.friction as f64,
            speed: d.speed_factor as f64,
            jump: d.jump_factor as f64,
            cushion: d.cushion as f64,
        }
    }

    fn climbable(&self, pos: BlockPos) -> bool {
        self.map
            .block(pos)
            .is_some_and(|s| self.reg.has(s, StateFlags::CLIMBABLE))
    }

    fn plant(&self, pos: BlockPos) -> Option<Plant> {
        let s = self.map.block(pos)?;
        let drag = self.reg.block_of(s).def.drag as f64;
        (drag > 0.0).then(|| Plant {
            drag,
            top: self.reg.plant_top(s) as f64,
        })
    }

    fn wrap_x(&self, x: f64) -> f64 {
        self.map.planet().wrap_xf(x)
    }
}

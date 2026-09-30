//! Spatial queries against loaded blocks: raycasts with real outline shapes, and collision box
//! gathering for physics. Positions are continuous and unwrapped; lookups wrap internally.

use glam::DVec3;
use hearth_math::{Aabb, BlockPos, Direction, VoxelRay};
use smallvec::SmallVec;

use crate::block::{BlockRegistry, BlockStateId, StateFlags};
use crate::storage::CubeMap;

/// Result of a block raycast.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockHit {
    /// Block that was hit (in the ray's unwrapped coordinate frame).
    pub pos: BlockPos,
    pub state: BlockStateId,
    /// Face of the block that was hit.
    pub face: Direction,
    /// Exact hit point.
    pub point: DVec3,
    /// Distance from the ray origin.
    pub distance: f64,
}

/// What a raycast stops at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FluidMode {
    /// Rays pass through fluids.
    Ignore,
    /// Fluid source blocks are hit as full cubes (bucket filling).
    SourcesOnly,
    /// Any fluid block is hit as a full cube.
    Any,
}

/// Casts a ray and returns the first block whose outline shape it hits.
pub fn raycast_blocks(
    map: &CubeMap,
    reg: &BlockRegistry,
    origin: DVec3,
    dir: DVec3,
    max_distance: f64,
    fluids: FluidMode,
) -> Option<BlockHit> {
    let dir_n = dir.normalize_or_zero();
    if dir_n == DVec3::ZERO {
        return None;
    }
    for step in VoxelRay::new(origin, dir_n, max_distance) {
        let Some(state) = map.block(step.block) else {
            continue;
        };
        if state.is_air() {
            continue;
        }
        let base = step.block.as_dvec3();
        let mut best: Option<(f64, Direction)> = None;
        let is_fluid = reg.has(state, StateFlags::FLUID);
        let fluid_hit = match fluids {
            FluidMode::Ignore => false,
            FluidMode::SourcesOnly => is_fluid && reg.fluid_amount(state) == 8,
            FluidMode::Any => reg.has(state, StateFlags::WATER),
        };
        if fluid_hit {
            let b = Aabb::block(step.block);
            best = b.ray_intersect(origin, dir_n, max_distance);
        }
        if best.is_none() && !is_fluid {
            for b in &reg.outline_shape(state).boxes {
                if let Some((t, face)) = b.offset(base).ray_intersect(origin, dir_n, max_distance)
                    && best.is_none_or(|(bt, _)| t < bt)
                {
                    best = Some((t, face));
                }
            }
        }
        if let Some((t, face)) = best {
            return Some(BlockHit {
                pos: step.block,
                state,
                face,
                point: origin + dir_n * t,
                distance: t,
            });
        }
    }
    None
}

/// Collects the world-space collision boxes of all blocks overlapping `region`. Unloaded cubes
/// count as solid when `unloaded_solid` is set, so nothing falls into unloaded territory.
pub fn collision_boxes(
    map: &CubeMap,
    reg: &BlockRegistry,
    region: &Aabb,
    unloaded_solid: bool,
    out: &mut SmallVec<[Aabb; 32]>,
) {
    out.clear();
    let (lo, hi) = region.block_range();
    // Include one block below for tall shapes (fences) reaching up into the region.
    for y in (lo.y - 1)..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let p = BlockPos::new(x, y, z);
                let base = p.as_dvec3();
                match map.block(p) {
                    Some(state) => {
                        if !reg.has(state, StateFlags::HAS_COLLISION) {
                            continue;
                        }
                        if reg.has(state, StateFlags::FULL_COLLISION) {
                            if y >= lo.y {
                                out.push(Aabb::block(p));
                            }
                            continue;
                        }
                        for b in &reg.collision_shape(state).boxes {
                            let wb = b.offset(base);
                            if wb.intersects(region) {
                                out.push(wb);
                            }
                        }
                    }
                    None if unloaded_solid && y >= lo.y => out.push(Aabb::block(p)),
                    None => {}
                }
            }
        }
    }
}

/// True if any block collision box intersects `region`.
pub fn collides(map: &CubeMap, reg: &BlockRegistry, region: &Aabb, unloaded_solid: bool) -> bool {
    let mut boxes = SmallVec::new();
    collision_boxes(map, reg, region, unloaded_solid, &mut boxes);
    boxes.iter().any(|b| b.intersects(region))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::tests::test_registry;
    use crate::cube::Cube;
    use hearth_math::{CubePos, Planet, PlanetSize};
    use std::sync::Arc;

    fn world() -> (CubeMap, BlockRegistry) {
        let reg = test_registry();
        let mut m = CubeMap::new(Planet::from_size(PlanetSize::Tiny).unwrap());
        for cy in -2..=2 {
            for cx in -1..=1 {
                for cz in -1..=1 {
                    m.insert_cube(
                        CubePos::new(cx, cy, cz),
                        Arc::new(Cube::filled(BlockStateId::AIR)),
                        &reg,
                    );
                }
            }
        }
        (m, reg)
    }

    #[test]
    fn raycast_down_through_cubes_hits_top_face() {
        let (mut m, reg) = world();
        let stone = reg.default_state("stone");
        m.set_block(BlockPos::new(0, -20, 0), stone, &reg);
        let hit = raycast_blocks(
            &m,
            &reg,
            DVec3::new(0.5, 20.5, 0.5),
            DVec3::NEG_Y,
            100.0,
            FluidMode::Ignore,
        )
        .unwrap();
        assert_eq!(hit.pos, BlockPos::new(0, -20, 0));
        assert_eq!(hit.face, Direction::Up);
        assert!((hit.distance - 39.5).abs() < 1e-9);
    }

    #[test]
    fn raycast_respects_shapes_and_fluids() {
        let (mut m, reg) = world();
        let torch = reg.default_state("torch");
        let water = reg.water_source();
        m.set_block(BlockPos::new(3, 0, 0), torch, &reg);
        // A ray passing above the small torch box misses it.
        assert!(
            raycast_blocks(
                &m,
                &reg,
                DVec3::new(0.5, 0.9, 0.5),
                DVec3::X,
                10.0,
                FluidMode::Ignore
            )
            .is_none()
        );
        let hit = raycast_blocks(
            &m,
            &reg,
            DVec3::new(0.5, 0.3, 0.5),
            DVec3::X,
            10.0,
            FluidMode::Ignore,
        )
        .unwrap();
        assert_eq!(hit.state, torch);
        m.set_block(BlockPos::new(1, 0, 0), water, &reg);
        let through = raycast_blocks(
            &m,
            &reg,
            DVec3::new(0.5, 0.3, 0.5),
            DVec3::X,
            10.0,
            FluidMode::Ignore,
        )
        .unwrap();
        assert_eq!(through.state, torch);
        let bucket = raycast_blocks(
            &m,
            &reg,
            DVec3::new(0.5, 0.3, 0.5),
            DVec3::X,
            10.0,
            FluidMode::SourcesOnly,
        )
        .unwrap();
        assert_eq!(bucket.state, water);
    }

    #[test]
    fn collision_gathering() {
        let (mut m, reg) = world();
        let stone = reg.default_state("stone");
        let stairs = reg.default_state("oak_stairs");
        m.set_block(BlockPos::new(0, 0, 0), stone, &reg);
        m.set_block(BlockPos::new(1, 0, 0), stairs, &reg);
        m.set_block(BlockPos::new(2, 0, 0), reg.default_state("torch"), &reg);
        let region = Aabb::from_coords(0.0, 0.0, 0.0, 3.0, 1.0, 1.0);
        let mut out = SmallVec::new();
        collision_boxes(&m, &reg, &region, false, &mut out);
        assert_eq!(out.len(), 3, "stone + two stair boxes, torch has none");
        assert!(collides(&m, &reg, &region, false));
        let far = Aabb::from_coords(100.0, 100.0, 100.0, 101.0, 101.0, 101.0);
        assert!(!collides(&m, &reg, &far, false));
        assert!(collides(&m, &reg, &far, true), "unloaded counts as solid");
    }
}

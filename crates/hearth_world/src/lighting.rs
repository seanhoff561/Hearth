//! The light engine for cubic chunks.
//!
//! * **Sky light**: above a column's sky heightmap (highest sky-blocking block) light is 15.
//!   Below it, light comes from propagation: level 15 travels straight down through blocks with
//!   zero opacity without dimming; every other step costs `max(1, opacity)`.
//! * **Block light**: emitted by light sources, spreading with the same cost rule.
//!
//! New cubes are lit in batches ([`LightEngine::light_new_cubes`]); edits are handled
//! incrementally with the classic remove-then-refill breadth-first passes
//! ([`LightEngine::block_changed`]). All positions go through the wrap-aware [`CubeMap`].

use std::collections::VecDeque;
use std::sync::Arc;

use hearth_math::{BlockPos, CUBE_SIZE, CubePos, Direction, LocalPos};

use crate::block::BlockRegistry;
use crate::cube::{Cube, LightStatus};
use crate::light::LightData;
use crate::storage::CubeMap;

/// Which light channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Sky,
    Block,
}

#[derive(Debug, Clone, Copy)]
struct Node {
    pos: BlockPos,
    level: u8,
}

/// Reusable BFS state (keeps its queues' capacity between calls).
#[derive(Debug, Default)]
pub struct LightEngine {
    add: VecDeque<Node>,
    remove: VecDeque<Node>,
    /// Statistics: nodes processed by the last operation.
    pub last_steps: usize,
}

#[inline]
fn light_of(cube: &Cube, ch: Channel, i: usize) -> u8 {
    match ch {
        Channel::Sky => cube.sky_light.get(i),
        Channel::Block => cube.block_light.get(i),
    }
}

#[inline]
fn data_mut(cube: &mut Cube, ch: Channel) -> &mut LightData {
    match ch {
        Channel::Sky => &mut cube.sky_light,
        Channel::Block => &mut cube.block_light,
    }
}

impl LightEngine {
    pub fn new() -> Self {
        Self::default()
    }

    fn get(map: &CubeMap, ch: Channel, pos: BlockPos) -> Option<u8> {
        let c = map.cube(pos.cube())?;
        Some(light_of(c, ch, pos.local().index()))
    }

    fn set(map: &mut CubeMap, ch: Channel, pos: BlockPos, v: u8) {
        if let Some(c) = map.cube_mut(pos.cube()) {
            data_mut(c, ch).set(pos.local().index(), v);
        }
    }

    /// Cost of light entering `state` (opaque blocks absorb everything).
    #[inline]
    fn cost(reg: &BlockRegistry, map: &CubeMap, pos: BlockPos) -> Option<u8> {
        let s = map.block(pos)?;
        Some(reg.light_opacity(s).max(1))
    }

    /// Lights a batch of newly inserted cubes (and lets light flow between them and their
    /// already-lit neighbours).
    pub fn light_new_cubes(&mut self, map: &mut CubeMap, reg: &BlockRegistry, cubes: &[CubePos]) {
        self.add.clear();
        let planet = *map.planet();
        let mut canon: Vec<CubePos> = cubes.iter().map(|c| planet.wrap_cube(*c)).collect();
        // Top-down so each cube's top boundary can read the (already initialised) cube above.
        canon.sort_by(|a, b| b.y.cmp(&a.y).then(a.x.cmp(&b.x)).then(a.z.cmp(&b.z)));
        canon.dedup();
        for &cp in &canon {
            self.init_cube(map, reg, cp);
        }
        // Seed propagation: new cubes' lit blocks plus neighbours' boundary layers facing them.
        for &cp in &canon {
            self.seed_cube(map, cp, Channel::Sky);
            for d in Direction::ALL {
                let n = planet.cube_neighbor(cp, d);
                if canon.binary_search_by(|p| cmp_desc(p, &n)).is_ok() {
                    continue;
                }
                self.seed_face(map, n, d.opposite(), Channel::Sky);
            }
        }
        self.propagate_add(map, reg, Channel::Sky);
        for &cp in &canon {
            self.seed_emitters(map, reg, cp);
            for d in Direction::ALL {
                let n = planet.cube_neighbor(cp, d);
                self.seed_face(map, n, d.opposite(), Channel::Block);
            }
        }
        self.propagate_add(map, reg, Channel::Block);
        for &cp in &canon {
            if let Some(c) = map.cube_mut(cp) {
                c.sky_light.compact();
                c.block_light.compact();
                c.light_status = LightStatus::Lit;
            }
        }
    }

    /// Straight-down sky light inside one cube from its top boundary.
    fn init_cube(&mut self, map: &mut CubeMap, reg: &BlockRegistry, cp: CubePos) {
        let planet = *map.planet();
        let Some(cube) = map.cube(cp).cloned() else {
            return;
        };
        let min = cp.min_block();
        let max_y = min.y + CUBE_SIZE - 1;
        let col = map.column(cp.column());
        let above = map.cube(planet.cube_neighbor(cp, Direction::Up)).cloned();
        // Fast path: the whole cube is above the heightmap of every column → uniform 15.
        if let Some(col) = col
            && col.sky_top.iter().all(|t| *t < min.y)
        {
            let c = map.cube_mut(cp).expect("cube exists");
            c.sky_light.fill(15);
            c.block_light.fill(0);
            return;
        }
        let mut sky = LightData::Uniform(0);
        let opaque_fill = cube
            .blocks
            .as_single()
            .is_some_and(|s| reg.light_opacity(s) >= 15);
        if !opaque_fill {
            for lz in 0..16u8 {
                for lx in 0..16u8 {
                    let xz = lz as usize * 16 + lx as usize;
                    let top = col.map_or(i32::MIN, |c| c.sky_top[xz]);
                    let mut level: u8 = if top < max_y + 1 {
                        15
                    } else if let Some(a) = &above {
                        if a.light_status == LightStatus::Lit {
                            a.sky_light.get(LocalPos::new(lx, 0, lz).index())
                        } else {
                            0
                        }
                    } else {
                        0
                    };
                    for ly in (0..16u8).rev() {
                        let y = min.y + ly as i32;
                        let p = LocalPos::new(lx, ly, lz);
                        let s = cube.get(p);
                        let op = reg.light_opacity(s);
                        if y > top {
                            level = 15;
                        } else if level == 15 && op == 0 && y >= top {
                            // Still in the open column.
                        } else {
                            level = level.saturating_sub(op.max(1));
                        }
                        if op >= 15 {
                            level = 0;
                        }
                        if level > 0 {
                            sky.set(p.index(), level);
                        }
                    }
                }
            }
        }
        let c = map.cube_mut(cp).expect("cube exists");
        c.sky_light = sky;
        c.block_light.fill(0);
    }

    /// Enqueues every block of a cube whose light could spread to a darker neighbour.
    fn seed_cube(&mut self, map: &CubeMap, cp: CubePos, ch: Channel) {
        let Some(c) = map.cube(cp) else { return };
        let data = match ch {
            Channel::Sky => &c.sky_light,
            Channel::Block => &c.block_light,
        };
        let min = cp.min_block();
        if let Some(u) = data.uniform_value() {
            if u <= 1 {
                return;
            }
            // Uniform: only the boundary can spread anywhere.
            for i in 0..hearth_math::CUBE_VOLUME {
                let p = LocalPos::from_index(i);
                let edge = p.x == 0 || p.x == 15 || p.y == 0 || p.y == 15 || p.z == 0 || p.z == 15;
                if edge {
                    self.add.push_back(Node {
                        pos: cp.block(p),
                        level: u,
                    });
                }
            }
            return;
        }
        for i in 0..hearth_math::CUBE_VOLUME {
            let l = data.get(i);
            if l > 1 {
                let p = LocalPos::from_index(i);
                // Skip interior blocks whose six neighbours are all at least l − 1.
                let interior = p.x > 0 && p.x < 15 && p.y > 0 && p.y < 15 && p.z > 0 && p.z < 15;
                if interior {
                    let idx = |dx: i32, dy: i32, dz: i32| {
                        LocalPos::new(
                            (p.x as i32 + dx) as u8,
                            (p.y as i32 + dy) as u8,
                            (p.z as i32 + dz) as u8,
                        )
                        .index()
                    };
                    let all_bright = [
                        idx(1, 0, 0),
                        idx(-1, 0, 0),
                        idx(0, 1, 0),
                        idx(0, -1, 0),
                        idx(0, 0, 1),
                        idx(0, 0, -1),
                    ]
                    .iter()
                    .all(|j| data.get(*j) + 1 >= l);
                    if all_bright {
                        continue;
                    }
                }
                self.add.push_back(Node {
                    pos: BlockPos::new(min.x + p.x as i32, min.y + p.y as i32, min.z + p.z as i32),
                    level: l,
                });
            }
        }
    }

    /// Enqueues one face layer of an existing (lit) cube.
    fn seed_face(&mut self, map: &CubeMap, cp: CubePos, face: Direction, ch: Channel) {
        let Some(c) = map.cube(cp) else { return };
        if c.light_status != LightStatus::Lit {
            return;
        }
        let min = cp.min_block();
        for a in 0..16u8 {
            for b in 0..16u8 {
                let p = match face {
                    Direction::Down => LocalPos::new(a, 0, b),
                    Direction::Up => LocalPos::new(a, 15, b),
                    Direction::North => LocalPos::new(a, b, 0),
                    Direction::South => LocalPos::new(a, b, 15),
                    Direction::West => LocalPos::new(0, a, b),
                    Direction::East => LocalPos::new(15, a, b),
                };
                let l = light_of(c, ch, p.index());
                if l > 1 {
                    self.add.push_back(Node {
                        pos: BlockPos::new(
                            min.x + p.x as i32,
                            min.y + p.y as i32,
                            min.z + p.z as i32,
                        ),
                        level: l,
                    });
                }
            }
        }
    }

    fn seed_emitters(&mut self, map: &mut CubeMap, reg: &BlockRegistry, cp: CubePos) {
        let Some(c) = map.cube(cp).cloned() else {
            return;
        };
        if c.is_empty() {
            return;
        }
        if let Some(s) = c.blocks.as_single()
            && reg.light_emission(s) == 0
        {
            return;
        }
        let min = cp.min_block();
        for i in 0..hearth_math::CUBE_VOLUME {
            let s = c.get_index(i);
            let e = reg.light_emission(s);
            if e > 0 {
                let p = LocalPos::from_index(i);
                let pos = BlockPos::new(min.x + p.x as i32, min.y + p.y as i32, min.z + p.z as i32);
                Self::set(map, Channel::Block, pos, e);
                self.add.push_back(Node { pos, level: e });
            }
        }
    }

    /// Breadth-first spreading of queued light.
    fn propagate_add(&mut self, map: &mut CubeMap, reg: &BlockRegistry, ch: Channel) {
        let mut steps = 0;
        while let Some(Node { pos, level }) = self.add.pop_front() {
            steps += 1;
            // Stale entries (light was raised further since) still spread correctly.
            for d in Direction::ALL {
                let n = pos.offset(d);
                let Some(cost) = Self::cost(reg, map, n) else {
                    continue;
                };
                if cost >= 15 {
                    continue;
                }
                let target =
                    if ch == Channel::Sky && d == Direction::Down && level == 15 && cost <= 1 {
                        // Straight-down sky light through clear blocks keeps full strength.
                        let s = map.block(n).expect("checked above");
                        if reg.light_opacity(s) == 0 {
                            15
                        } else {
                            15 - cost
                        }
                    } else {
                        level.saturating_sub(cost)
                    };
                if target == 0 {
                    continue;
                }
                let cur = Self::get(map, ch, n).unwrap_or(0);
                if target > cur {
                    Self::set(map, ch, n, target);
                    self.add.push_back(Node {
                        pos: n,
                        level: target,
                    });
                }
            }
        }
        self.last_steps = steps;
    }

    /// Updates light after the block at `pos` changed. Call after the change is applied to the
    /// map (and its heightmap).
    pub fn block_changed(&mut self, map: &mut CubeMap, reg: &BlockRegistry, pos: BlockPos) {
        let pos = map.planet().wrap_block(pos);
        let Some(state) = map.block(pos) else { return };
        for ch in [Channel::Block, Channel::Sky] {
            self.remove.clear();
            self.add.clear();
            let old = Self::get(map, ch, pos).unwrap_or(0);
            // New intrinsic level of the changed block.
            let intrinsic = match ch {
                Channel::Block => reg.light_emission(state),
                Channel::Sky => {
                    let top = map.sky_top(pos.x, pos.z).unwrap_or(i32::MIN);
                    if pos.y > top { 15 } else { 0 }
                }
            };
            if old > 0 {
                Self::set(map, ch, pos, 0);
                self.remove.push_back(Node { pos, level: old });
            }
            // A newly opaque block may also cut straight-down sky light below it.
            if ch == Channel::Sky {
                self.cut_column(map, reg, pos);
            }
            self.propagate_remove(map, reg, ch);
            if intrinsic > 0 {
                let cur = Self::get(map, ch, pos).unwrap_or(0);
                if intrinsic > cur {
                    Self::set(map, ch, pos, intrinsic);
                }
                self.add.push_back(Node {
                    pos,
                    level: intrinsic.max(cur),
                });
            }
            // Neighbours may now shine into the changed block (e.g. it became transparent).
            for d in Direction::ALL {
                let n = pos.offset(d);
                if let Some(l) = Self::get(map, ch, n)
                    && l > 1
                {
                    self.add.push_back(Node { pos: n, level: l });
                }
            }
            if ch == Channel::Sky {
                self.open_column(map, reg, pos);
            }
            self.propagate_add(map, reg, ch);
        }
    }

    /// When the column's heightmap rose to `pos`, blocks below lose their direct sky light.
    fn cut_column(&mut self, map: &mut CubeMap, reg: &BlockRegistry, pos: BlockPos) {
        let _ = reg;
        let top = map.sky_top(pos.x, pos.z).unwrap_or(i32::MIN);
        if top != pos.y {
            return;
        }
        let mut y = pos.y - 1;
        for _ in 0..CUBE_SIZE * 8 {
            let p = BlockPos::new(pos.x, y, pos.z);
            match Self::get(map, Channel::Sky, p) {
                Some(15) => {
                    Self::set(map, Channel::Sky, p, 0);
                    self.remove.push_back(Node { pos: p, level: 15 });
                }
                _ => break,
            }
            y -= 1;
        }
    }

    /// When the heightmap dropped (the top block was removed), the newly exposed column gets
    /// full sky light down to the new top.
    fn open_column(&mut self, map: &mut CubeMap, reg: &BlockRegistry, pos: BlockPos) {
        let top = map.sky_top(pos.x, pos.z).unwrap_or(i32::MIN);
        let mut y = pos.y;
        for _ in 0..CUBE_SIZE * 8 {
            if y <= top {
                break;
            }
            let p = BlockPos::new(pos.x, y, pos.z);
            let Some(s) = map.block(p) else { break };
            if reg.light_opacity(s) > 0 {
                break;
            }
            if Self::get(map, Channel::Sky, p).unwrap_or(0) < 15 {
                Self::set(map, Channel::Sky, p, 15);
                self.add.push_back(Node { pos: p, level: 15 });
            }
            y -= 1;
        }
    }

    fn propagate_remove(&mut self, map: &mut CubeMap, reg: &BlockRegistry, ch: Channel) {
        while let Some(Node { pos, level }) = self.remove.pop_front() {
            for d in Direction::ALL {
                let n = pos.offset(d);
                let Some(nl) = Self::get(map, ch, n) else {
                    continue;
                };
                if nl == 0 {
                    continue;
                }
                let emitted = if ch == Channel::Block {
                    map.block(n).map_or(0, |s| reg.light_emission(s))
                } else {
                    0
                };
                let from_this = nl < level
                    || (ch == Channel::Sky && d == Direction::Down && level == 15 && nl == 15);
                if from_this && emitted < nl {
                    Self::set(map, ch, n, emitted);
                    self.remove.push_back(Node { pos: n, level: nl });
                    if emitted > 0 {
                        self.add.push_back(Node {
                            pos: n,
                            level: emitted,
                        });
                    }
                } else {
                    // Lit by another source: it will refill the hole.
                    self.add.push_back(Node { pos: n, level: nl });
                }
            }
        }
    }
}

/// Order used to binary-search the sorted batch (y descending, then x, z).
fn cmp_desc(a: &CubePos, b: &CubePos) -> std::cmp::Ordering {
    b.y.cmp(&a.y).then(a.x.cmp(&b.x)).then(a.z.cmp(&b.z))
}

/// Convenience: sky light of a block (15 above unloaded columns' tops).
pub fn sky_light_at(map: &CubeMap, pos: BlockPos) -> u8 {
    map.sky_light(pos)
}

/// Replaces a cube in the map and relights it (used by tools and tests).
pub fn insert_and_light(
    engine: &mut LightEngine,
    map: &mut CubeMap,
    reg: &BlockRegistry,
    pos: CubePos,
    cube: Cube,
) {
    map.insert_cube(pos, Arc::new(cube), reg);
    engine.light_new_cubes(map, reg, &[pos]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::BlockStateId;
    use crate::block::tests::test_registry;
    use hearth_math::{ColumnPos, Planet, PlanetSize};

    fn world(reg: &BlockRegistry) -> CubeMap {
        CubeMap::new(Planet::from_size(PlanetSize::Tiny).unwrap()).tap(|m| {
            let _ = reg;
            m.ensure_column(ColumnPos::new(0, 0), || [crate::storage::NO_HEIGHT; 256]);
        })
    }

    trait Tap: Sized {
        fn tap(mut self, f: impl FnOnce(&mut Self)) -> Self {
            f(&mut self);
            self
        }
    }
    impl Tap for CubeMap {}

    #[test]
    fn open_sky_is_fully_lit_and_ground_is_dark() {
        let reg = test_registry();
        let stone = reg.default_state("stone");
        let mut map = world(&reg);
        let mut eng = LightEngine::new();
        let cubes = [
            CubePos::new(0, 0, 0),
            CubePos::new(0, 1, 0),
            CubePos::new(0, -1, 0),
        ];
        map.insert_cube(cubes[0], Arc::new(Cube::filled(BlockStateId::AIR)), &reg);
        map.insert_cube(cubes[1], Arc::new(Cube::filled(BlockStateId::AIR)), &reg);
        map.insert_cube(cubes[2], Arc::new(Cube::filled(stone)), &reg);
        eng.light_new_cubes(&mut map, &reg, &cubes);
        assert_eq!(map.sky_light(BlockPos::new(3, 5, 3)), 15);
        assert_eq!(map.sky_light(BlockPos::new(3, -5, 3)), 0);
    }

    #[test]
    fn cave_opened_to_the_sky_from_below() {
        // A roofed room, then the roof is removed: the room fills with sky light.
        let reg = test_registry();
        let stone = reg.default_state("stone");
        let mut map = world(&reg);
        let mut eng = LightEngine::new();
        let mut room = Cube::filled(stone);
        for y in 2..14u8 {
            for z in 2..14u8 {
                for x in 2..14u8 {
                    room.set(LocalPos::new(x, y, z), BlockStateId::AIR);
                }
            }
        }
        map.insert_cube(CubePos::new(0, 0, 0), Arc::new(room), &reg);
        map.insert_cube(
            CubePos::new(0, 1, 0),
            Arc::new(Cube::filled(BlockStateId::AIR)),
            &reg,
        );
        eng.light_new_cubes(
            &mut map,
            &reg,
            &[CubePos::new(0, 0, 0), CubePos::new(0, 1, 0)],
        );
        assert_eq!(
            map.sky_light(BlockPos::new(8, 8, 8)),
            0,
            "closed room is dark"
        );
        // Dig a shaft through the roof (y = 14, 15).
        for y in 14..16 {
            map.set_block(BlockPos::new(8, y, 8), BlockStateId::AIR, &reg);
            eng.block_changed(&mut map, &reg, BlockPos::new(8, y, 8));
        }
        assert_eq!(
            map.sky_light(BlockPos::new(8, 3, 8)),
            15,
            "direct sky down the shaft"
        );
        assert_eq!(
            map.sky_light(BlockPos::new(10, 8, 8)),
            13,
            "spreads sideways"
        );
        // Close it again: darkness returns.
        map.set_block(BlockPos::new(8, 15, 8), stone, &reg);
        eng.block_changed(&mut map, &reg, BlockPos::new(8, 15, 8));
        assert_eq!(map.sky_light(BlockPos::new(8, 8, 8)), 0);
    }

    #[test]
    fn cube_loaded_before_the_cubes_above_it() {
        let reg = test_registry();
        let stone = reg.default_state("stone");
        let mut map = CubeMap::new(Planet::from_size(PlanetSize::Tiny).unwrap());
        let mut eng = LightEngine::new();
        // Heightmap says there's a roof above (estimate at y = 40); the lower cube loads first.
        map.ensure_column(ColumnPos::new(0, 0), || [40; 256]);
        map.insert_cube(
            CubePos::new(0, 0, 0),
            Arc::new(Cube::filled(BlockStateId::AIR)),
            &reg,
        );
        eng.light_new_cubes(&mut map, &reg, &[CubePos::new(0, 0, 0)]);
        assert_eq!(
            map.sky_light(BlockPos::new(4, 4, 4)),
            0,
            "under the estimated roof"
        );
        // The cubes above arrive: open air, and the roof turns out to be at y = 20 only in part.
        let mut roof = Cube::filled(BlockStateId::AIR);
        for z in 0..16u8 {
            for x in 0..8u8 {
                roof.set(LocalPos::new(x, 4, z), stone);
            }
        }
        map.insert_cube(CubePos::new(0, 1, 0), Arc::new(roof), &reg);
        map.insert_cube(
            CubePos::new(0, 2, 0),
            Arc::new(Cube::filled(BlockStateId::AIR)),
            &reg,
        );
        map.insert_cube(
            CubePos::new(0, 3, 0),
            Arc::new(Cube::filled(BlockStateId::AIR)),
            &reg,
        );
        eng.light_new_cubes(
            &mut map,
            &reg,
            &[
                CubePos::new(0, 1, 0),
                CubePos::new(0, 2, 0),
                CubePos::new(0, 3, 0),
            ],
        );
        // Columns without roof get direct light; covered columns get side light.
        assert_eq!(map.sky_light(BlockPos::new(12, 25, 3)), 15);
        assert_eq!(
            map.sky_light(BlockPos::new(12, 10, 3)),
            15,
            "light reaches the lower cube"
        );
        assert!(
            map.sky_light(BlockPos::new(6, 10, 3)) >= 12,
            "spreads under the roof edge"
        );
        assert!(map.sky_light(BlockPos::new(0, 10, 3)) < 15);
    }

    #[test]
    fn torch_light_spreads_across_cube_borders_and_is_removed() {
        let reg = test_registry();
        let torch = reg.default_state("torch");
        let stone = reg.default_state("stone");
        let mut map = world(&reg);
        let mut eng = LightEngine::new();
        let cubes = [CubePos::new(0, 0, 0), CubePos::new(1, 0, 0)];
        let mut dark = Cube::filled(BlockStateId::AIR);
        for x in 0..16u8 {
            for z in 0..16u8 {
                dark.set(LocalPos::new(x, 15, z), stone); // roof
            }
        }
        for c in cubes {
            map.insert_cube(c, Arc::new(dark.clone()), &reg);
        }
        eng.light_new_cubes(&mut map, &reg, &cubes);
        let at = BlockPos::new(14, 5, 5);
        map.set_block(at, torch, &reg);
        eng.block_changed(&mut map, &reg, at);
        assert_eq!(map.block_light(at), 14);
        assert_eq!(
            map.block_light(BlockPos::new(17, 5, 5)),
            11,
            "crosses into the next cube"
        );
        map.set_block(at, BlockStateId::AIR, &reg);
        eng.block_changed(&mut map, &reg, at);
        assert_eq!(map.block_light(BlockPos::new(17, 5, 5)), 0);
        assert_eq!(map.block_light(at), 0);
    }

    #[test]
    fn water_attenuates_sky_light_with_depth() {
        let reg = test_registry();
        let water = reg.water_source();
        let mut map = world(&reg);
        let mut eng = LightEngine::new();
        map.insert_cube(
            CubePos::new(0, 1, 0),
            Arc::new(Cube::filled(BlockStateId::AIR)),
            &reg,
        );
        map.insert_cube(CubePos::new(0, 0, 0), Arc::new(Cube::filled(water)), &reg);
        eng.light_new_cubes(
            &mut map,
            &reg,
            &[CubePos::new(0, 1, 0), CubePos::new(0, 0, 0)],
        );
        let top = map.sky_light(BlockPos::new(5, 15, 5));
        let deep = map.sky_light(BlockPos::new(5, 8, 5));
        assert!(top > deep && deep < 15, "top {top} deep {deep}");
    }
}

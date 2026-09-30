//! The loaded world: cubes keyed by canonical `(cx, cy, cz)` plus per-column records holding
//! the sky-light heightmap. All lookups go through [`Planet`] so the X seam is invisible.

use std::sync::Arc;

use hearth_math::{BlockPos, CUBE_AREA, CUBE_SIZE, ColumnPos, CubePos, LocalPos, Planet};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::block::{BlockRegistry, BlockStateId, StateFlags};
use crate::cube::Cube;

/// Marker for "no sky-blocking block known in this column".
pub const NO_HEIGHT: i32 = i32::MIN / 2;

/// Maximum number of cubes scanned downward when the top block of a column is removed.
const MAX_SCAN_CUBES: usize = 128;

/// Per-column data.
#[derive(Debug, Clone)]
pub struct Column {
    /// Y of the highest sky-blocking block per (x, z) (index `z * 16 + x`). Above it, sky light
    /// is always 15.
    pub sky_top: [i32; CUBE_AREA],
    /// World-generation estimate of the surface, used for parts of the column that are not
    /// loaded.
    pub estimate: [i32; CUBE_AREA],
    /// Y coordinates (in cubes) of loaded cubes, sorted.
    loaded: SmallVec<[i32; 16]>,
    /// Bumped whenever `sky_top` changes.
    pub heightmap_version: u32,
}

impl Column {
    /// A column whose heightmap starts at the given world-generation estimate.
    pub fn with_estimate(estimate: [i32; CUBE_AREA]) -> Self {
        Self {
            sky_top: estimate,
            estimate,
            loaded: SmallVec::new(),
            heightmap_version: 0,
        }
    }

    /// A column with no information about its contents.
    pub fn unknown() -> Self {
        Self::with_estimate([NO_HEIGHT; CUBE_AREA])
    }

    pub fn loaded_cubes(&self) -> &[i32] {
        &self.loaded
    }

    pub fn is_loaded(&self, cy: i32) -> bool {
        self.loaded.binary_search(&cy).is_ok()
    }

    #[inline]
    pub fn sky_top_at(&self, lx: usize, lz: usize) -> i32 {
        self.sky_top[lz * 16 + lx]
    }

    fn mark_loaded(&mut self, cy: i32) {
        if let Err(i) = self.loaded.binary_search(&cy) {
            self.loaded.insert(i, cy);
        }
    }

    fn mark_unloaded(&mut self, cy: i32) {
        if let Ok(i) = self.loaded.binary_search(&cy) {
            self.loaded.remove(i);
        }
    }
}

/// The set of loaded cubes and columns.
#[derive(Debug, Clone)]
pub struct CubeMap {
    planet: Planet,
    cubes: FxHashMap<CubePos, Arc<Cube>>,
    columns: FxHashMap<ColumnPos, Column>,
}

impl CubeMap {
    pub fn new(planet: Planet) -> Self {
        Self {
            planet,
            cubes: FxHashMap::default(),
            columns: FxHashMap::default(),
        }
    }

    #[inline]
    pub fn planet(&self) -> &Planet {
        &self.planet
    }

    pub fn cube_count(&self) -> usize {
        self.cubes.len()
    }

    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// Creates the column record if missing, seeding its heightmap with `estimate`.
    pub fn ensure_column(
        &mut self,
        pos: ColumnPos,
        estimate: impl FnOnce() -> [i32; CUBE_AREA],
    ) -> &mut Column {
        let pos = self.planet.wrap_column(pos);
        self.columns
            .entry(pos)
            .or_insert_with(|| Column::with_estimate(estimate()))
    }

    pub fn column(&self, pos: ColumnPos) -> Option<&Column> {
        self.columns.get(&self.planet.wrap_column(pos))
    }

    pub fn columns(&self) -> impl Iterator<Item = (&ColumnPos, &Column)> {
        self.columns.iter()
    }

    /// Removes a column record (only when none of its cubes are loaded).
    pub fn remove_column_if_empty(&mut self, pos: ColumnPos) -> Option<Column> {
        let pos = self.planet.wrap_column(pos);
        if self.columns.get(&pos).is_some_and(|c| c.loaded.is_empty()) {
            self.columns.remove(&pos)
        } else {
            None
        }
    }

    #[inline]
    pub fn cube(&self, pos: CubePos) -> Option<&Arc<Cube>> {
        self.cubes.get(&self.planet.wrap_cube(pos))
    }

    /// Mutable access (copy-on-write if a worker still holds a snapshot).
    #[inline]
    pub fn cube_mut(&mut self, pos: CubePos) -> Option<&mut Cube> {
        self.cubes
            .get_mut(&self.planet.wrap_cube(pos))
            .map(Arc::make_mut)
    }

    pub fn contains_cube(&self, pos: CubePos) -> bool {
        self.cubes.contains_key(&self.planet.wrap_cube(pos))
    }

    pub fn cubes(&self) -> impl Iterator<Item = (&CubePos, &Arc<Cube>)> {
        self.cubes.iter()
    }

    pub fn cube_positions(&self) -> impl Iterator<Item = CubePos> + '_ {
        self.cubes.keys().copied()
    }

    /// Inserts (or replaces) a cube and updates its column's heightmap. Creates an unknown
    /// column if none exists.
    pub fn insert_cube(&mut self, pos: CubePos, cube: Arc<Cube>, reg: &BlockRegistry) {
        let pos = self.planet.wrap_cube(pos);
        let highest = cube.highest_sky_blocking(reg);
        self.cubes.insert(pos, cube);
        let col_pos = pos.column();
        let column = self.columns.entry(col_pos).or_insert_with(Column::unknown);
        column.mark_loaded(pos.y);
        let min_y = pos.y * CUBE_SIZE;
        let max_y = min_y + CUBE_SIZE - 1;
        let mut changed = false;
        for (xz, &h) in highest.iter().enumerate() {
            let top = column.sky_top[xz];
            let new_top = if h >= 0 && min_y + h as i32 > top {
                min_y + h as i32
            } else if (min_y..=max_y).contains(&top) {
                // The known top lies in this cube but the block there doesn't block the sky
                // (e.g. the estimate was wrong, or the cube was edited while unloaded).
                if h >= 0 {
                    min_y + h as i32
                } else {
                    scan_down(
                        &self.cubes,
                        column,
                        &self.planet,
                        col_pos,
                        xz,
                        min_y - 1,
                        reg,
                    )
                }
            } else {
                top
            };
            if new_top != top {
                column.sky_top[xz] = new_top;
                changed = true;
            }
        }
        if changed {
            column.heightmap_version = column.heightmap_version.wrapping_add(1);
        }
    }

    /// Removes a cube. The column record stays (its heightmap remains valid knowledge).
    pub fn remove_cube(&mut self, pos: CubePos) -> Option<Arc<Cube>> {
        let pos = self.planet.wrap_cube(pos);
        let cube = self.cubes.remove(&pos)?;
        if let Some(col) = self.columns.get_mut(&pos.column()) {
            col.mark_unloaded(pos.y);
        }
        Some(cube)
    }

    /// Block state at a position, or `None` if its cube isn't loaded.
    #[inline]
    pub fn block(&self, pos: BlockPos) -> Option<BlockStateId> {
        let pos = self.planet.wrap_block(pos);
        self.cubes.get(&pos.cube()).map(|c| c.get(pos.local()))
    }

    /// Block state, treating unloaded cubes as air.
    #[inline]
    pub fn block_or_air(&self, pos: BlockPos) -> BlockStateId {
        self.block(pos).unwrap_or(BlockStateId::AIR)
    }

    /// Sets a block. Returns the previous state, or `None` if the cube isn't loaded. Keeps the
    /// column heightmap exact.
    pub fn set_block(
        &mut self,
        pos: BlockPos,
        state: BlockStateId,
        reg: &BlockRegistry,
    ) -> Option<BlockStateId> {
        let pos = self.planet.wrap_block(pos);
        let cube_pos = pos.cube();
        let cube = Arc::make_mut(self.cubes.get_mut(&cube_pos)?);
        let old = cube.set(pos.local(), state);
        if old == state {
            return Some(old);
        }
        let col_pos = cube_pos.column();
        let column = self.columns.entry(col_pos).or_insert_with(Column::unknown);
        let xz = ((pos.z & 15) * 16 + (pos.x & 15)) as usize;
        let top = column.sky_top[xz];
        let blocks = reg.has(state, StateFlags::BLOCKS_SKY);
        let new_top = if blocks && pos.y > top {
            pos.y
        } else if !blocks && pos.y == top {
            scan_down(
                &self.cubes,
                column,
                &self.planet,
                col_pos,
                xz,
                pos.y - 1,
                reg,
            )
        } else {
            top
        };
        if new_top != top {
            column.sky_top[xz] = new_top;
            column.heightmap_version = column.heightmap_version.wrapping_add(1);
        }
        Some(old)
    }

    /// Highest sky-blocking Y at a world (x, z), if the column is known.
    pub fn sky_top(&self, x: i32, z: i32) -> Option<i32> {
        let x = self.planet.wrap_x(x);
        let col = self.columns.get(&ColumnPos::new(x >> 4, z >> 4))?;
        Some(col.sky_top[((z & 15) * 16 + (x & 15)) as usize])
    }

    /// Sky light at a block (15 above the heightmap for unloaded or unlit cubes).
    pub fn sky_light(&self, pos: BlockPos) -> u8 {
        let pos = self.planet.wrap_block(pos);
        match self.cubes.get(&pos.cube()) {
            Some(c) => c.sky_light(pos.local()),
            None => match self.sky_top(pos.x, pos.z) {
                Some(top) if pos.y > top => 15,
                Some(_) => 0,
                None => 15,
            },
        }
    }

    /// Block light at a block (0 when unloaded).
    pub fn block_light(&self, pos: BlockPos) -> u8 {
        let pos = self.planet.wrap_block(pos);
        self.cubes
            .get(&pos.cube())
            .map_or(0, |c| c.block_light(pos.local()))
    }

    /// Total approximate heap bytes of loaded cubes.
    pub fn heap_bytes(&self) -> usize {
        self.cubes.values().map(|c| c.heap_bytes()).sum()
    }
}

/// Finds the highest sky-blocking block at or below `from_y` in a column, walking loaded cubes.
/// Falls back to the world-generation estimate when an unloaded cube is reached.
fn scan_down(
    cubes: &FxHashMap<CubePos, Arc<Cube>>,
    column: &Column,
    planet: &Planet,
    col_pos: ColumnPos,
    xz: usize,
    from_y: i32,
    reg: &BlockRegistry,
) -> i32 {
    let lx = (xz & 15) as u8;
    let lz = (xz >> 4) as u8;
    let mut y = from_y;
    for _ in 0..MAX_SCAN_CUBES {
        let cy = y >> 4;
        let cube_pos = planet.wrap_cube(col_pos.cube(cy));
        let Some(cube) = cubes.get(&cube_pos) else {
            // Unknown territory: trust the estimate, but never above what we just scanned.
            return column.estimate[xz].min(y);
        };
        if let Some(s) = cube.blocks.as_single() {
            if reg.has(s, StateFlags::BLOCKS_SKY) {
                return y;
            }
        } else {
            let min_y = cy * CUBE_SIZE;
            while y >= min_y {
                let state = cube.get(LocalPos::new(lx, (y - min_y) as u8, lz));
                if reg.has(state, StateFlags::BLOCKS_SKY) {
                    return y;
                }
                y -= 1;
            }
            continue;
        }
        y = cy * CUBE_SIZE - 1;
    }
    column.estimate[xz].min(y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::tests::test_registry;
    use hearth_math::PlanetSize;

    fn map() -> CubeMap {
        CubeMap::new(Planet::from_size(PlanetSize::Tiny).unwrap())
    }

    #[test]
    fn seam_is_transparent() {
        let reg = test_registry();
        let stone = reg.default_state("stone");
        let mut m = map();
        let c = m.planet().circumference();
        let last = CubePos::new(m.planet().cubes_around() - 1, 0, 0);
        m.insert_cube(last, Arc::new(Cube::filled(BlockStateId::AIR)), &reg);
        m.insert_cube(
            CubePos::new(0, 0, 0),
            Arc::new(Cube::filled(BlockStateId::AIR)),
            &reg,
        );
        // Writing at x = -1 lands in the last cube; x = C lands in cube 0.
        m.set_block(BlockPos::new(-1, 3, 3), stone, &reg).unwrap();
        m.set_block(BlockPos::new(c, 4, 4), stone, &reg).unwrap();
        assert_eq!(m.block(BlockPos::new(c - 1, 3, 3)), Some(stone));
        assert_eq!(m.block(BlockPos::new(0, 4, 4)), Some(stone));
        assert_eq!(m.block(BlockPos::new(2 * c, 4, 4)), Some(stone));
        assert!(m.cube(CubePos::new(-1, 0, 0)).is_some());
        assert_eq!(m.cube_count(), 2);
    }

    #[test]
    fn heightmap_follows_edits_and_huge_y() {
        let reg = test_registry();
        let stone = reg.default_state("stone");
        let mut m = map();
        let base = 1_000_000 >> 4; // cube Y around one million blocks up
        for cy in base - 2..=base {
            m.insert_cube(
                CubePos::new(0, cy, 0),
                Arc::new(Cube::filled(BlockStateId::AIR)),
                &reg,
            );
        }
        let floor = BlockPos::new(1, (base - 2) * 16 + 2, 1);
        m.set_block(floor, stone, &reg);
        assert_eq!(m.sky_top(1, 1), Some(floor.y));
        let roof = BlockPos::new(1, base * 16 + 7, 1);
        m.set_block(roof, stone, &reg);
        assert_eq!(m.sky_top(1, 1), Some(roof.y));
        // Removing the roof scans down across a cube boundary to the floor.
        m.set_block(roof, BlockStateId::AIR, &reg);
        assert_eq!(m.sky_top(1, 1), Some(floor.y));
        // Removing the floor reaches an unloaded cube → falls back to the (unknown) estimate.
        m.set_block(floor, BlockStateId::AIR, &reg);
        assert!(m.sky_top(1, 1).unwrap() < floor.y);
        // Torches don't block the sky.
        m.set_block(
            BlockPos::new(2, base * 16, 2),
            reg.default_state("torch"),
            &reg,
        );
        assert!(m.sky_top(2, 2).unwrap() < base * 16);
    }

    #[test]
    fn loading_a_cube_corrects_a_wrong_estimate() {
        let reg = test_registry();
        let stone = reg.default_state("stone");
        let mut m = map();
        // Estimate says the surface is at y = 20 everywhere.
        m.ensure_column(ColumnPos::new(0, 0), || [20; CUBE_AREA]);
        assert_eq!(m.sky_top(0, 0), Some(20));
        // The real cube at y 16..31 is empty (a cave opening), the cube below is solid.
        m.insert_cube(CubePos::new(0, 0, 0), Arc::new(Cube::filled(stone)), &reg);
        assert_eq!(
            m.sky_top(0, 0),
            Some(20),
            "cube below the estimate changes nothing"
        );
        m.insert_cube(
            CubePos::new(0, 1, 0),
            Arc::new(Cube::filled(BlockStateId::AIR)),
            &reg,
        );
        assert_eq!(
            m.sky_top(0, 0),
            Some(15),
            "corrected to the loaded solid cube below"
        );
        // A loaded cube above with a block raises it.
        let mut high = Cube::filled(BlockStateId::AIR);
        high.set(LocalPos::new(0, 5, 0), stone);
        m.insert_cube(CubePos::new(0, 10, 0), Arc::new(high), &reg);
        assert_eq!(m.sky_top(0, 0), Some(165));
        assert_eq!(m.sky_top(1, 0), Some(15));
    }

    #[test]
    fn unloaded_reads_and_light_defaults() {
        let reg = test_registry();
        let mut m = map();
        assert_eq!(m.block(BlockPos::new(0, 0, 0)), None);
        assert_eq!(m.block_or_air(BlockPos::new(0, 0, 0)), BlockStateId::AIR);
        assert_eq!(
            m.set_block(BlockPos::new(0, 0, 0), BlockStateId(1), &reg),
            None
        );
        m.ensure_column(ColumnPos::new(0, 0), || [10; CUBE_AREA]);
        assert_eq!(m.sky_light(BlockPos::new(0, 11, 0)), 15);
        assert_eq!(m.sky_light(BlockPos::new(0, 9, 0)), 0);
        assert_eq!(m.block_light(BlockPos::new(0, 9, 0)), 0);
    }

    #[test]
    fn remove_cube_keeps_column_until_empty() {
        let reg = test_registry();
        let mut m = map();
        m.insert_cube(
            CubePos::new(3, 1, 3),
            Arc::new(Cube::filled(BlockStateId::AIR)),
            &reg,
        );
        assert!(m.remove_column_if_empty(ColumnPos::new(3, 3)).is_none());
        assert!(m.remove_cube(CubePos::new(3, 1, 3)).is_some());
        assert!(m.remove_column_if_empty(ColumnPos::new(3, 3)).is_some());
    }
}

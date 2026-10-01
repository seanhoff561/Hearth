//! The blocks the player has changed (v2 §5.1): kept apart from the generated terrain, laid back
//! over each cube as it loads again, and saved with the world (`blocks.json`). Terrain is
//! generated afresh whenever a cube loads; the player's changes, the finite water
//! (`hearth_world::water`) and the seasonal cover (`season_cover`) are laid over it in turn.

use hearth_math::{BlockPos, CubePos};
use hearth_world::{BlockRegistry, BlockStateId, CubeMap};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

/// What `blocks.json` holds: each changed block and its state, by name (so the save outlives
/// changes to the block registry's numbering).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EditsSave {
    #[serde(default)]
    pub blocks: Vec<(BlockPos, String)>,
}

/// The player's changes to the terrain, by cube.
#[derive(Debug, Clone, Default)]
pub struct Edits {
    by_cube: FxHashMap<CubePos, FxHashMap<BlockPos, BlockStateId>>,
    /// Changes to blocks this content no longer has, kept as saved (they come back with them).
    unknown: Vec<(BlockPos, String)>,
    /// Counts the changes made (to tell when the distant terrain should hear of them).
    version: u64,
}

impl Edits {
    pub fn load(save: EditsSave, reg: &BlockRegistry) -> Self {
        let mut edits = Self::default();
        for (p, name) in save.blocks {
            match reg.parse_state(&name) {
                Ok(state) => edits.set(p, state),
                Err(_) => edits.unknown.push((p, name)),
            }
        }
        if !edits.unknown.is_empty() {
            log::warn!(
                "{} changed blocks are of kinds this content does not have; they are kept but \
                 not shown",
                edits.unknown.len()
            );
        }
        edits
    }

    pub fn save(&self, reg: &BlockRegistry) -> EditsSave {
        let mut blocks: Vec<(BlockPos, String)> = self
            .by_cube
            .values()
            .flat_map(|m| m.iter().map(|(p, s)| (*p, reg.state_string(*s))))
            .chain(self.unknown.iter().cloned())
            .collect();
        blocks.sort_by_key(|(p, _)| (p.x, p.y, p.z));
        EditsSave { blocks }
    }

    /// Records that the block at `p` is now `state`.
    pub fn set(&mut self, p: BlockPos, state: BlockStateId) {
        self.by_cube.entry(p.cube()).or_default().insert(p, state);
        self.version += 1;
    }

    /// Counts the changes made so far.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// The changes as the distant terrain shows them: per column (x wrapped by `wrap`), the
    /// highest solid block placed.
    pub fn tops(&self, reg: &BlockRegistry, wrap: impl Fn(i32) -> i32) -> hearth_lod::EditTops {
        let mut tops = hearth_lod::EditTops::default();
        for m in self.by_cube.values() {
            for (p, s) in m {
                if s.is_air() || reg.collision_shape(*s).is_empty() {
                    continue;
                }
                let e = tops.entry((wrap(p.x), p.z)).or_insert((p.y, *s));
                if p.y > e.0 {
                    *e = (p.y, *s);
                }
            }
        }
        tops
    }

    /// Forgets any change at `p` (the block is the generator's again).
    pub fn forget(&mut self, p: BlockPos) {
        if let Some(m) = self.by_cube.get_mut(&p.cube()) {
            if m.remove(&p).is_some() {
                self.version += 1;
            }
            if m.is_empty() {
                self.by_cube.remove(&p.cube());
            }
        }
    }

    /// The changed state at `p`, if the player changed it.
    pub fn get(&self, p: BlockPos) -> Option<BlockStateId> {
        self.by_cube.get(&p.cube())?.get(&p).copied()
    }

    /// How many blocks the player has changed.
    pub fn len(&self) -> usize {
        self.by_cube.values().map(|m| m.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Lays the changes inside `cube` (just generated and inserted) over it.
    pub fn restore(&self, map: &mut CubeMap, reg: &BlockRegistry, cube: CubePos) {
        if let Some(m) = self.by_cube.get(&cube) {
            for (p, s) in m {
                map.set_block(*p, *s, reg);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg() -> BlockRegistry {
        let defs = hearth_world::datapack::load_block_defs(&[crate::scene::data_pack_dir()])
            .expect("blocks");
        BlockRegistry::build(defs).expect("registry")
    }

    #[test]
    fn changes_round_trip_by_name_and_keep_unknown_kinds() {
        let reg = reg();
        let fire = reg
            .parse_state("hearth:campfire[fire=high]")
            .expect("a campfire state");
        let mut e = Edits::default();
        e.set(BlockPos::new(3, 4, 5), fire);
        e.set(BlockPos::new(-20, 1, 7), BlockStateId::AIR);
        let mut saved = e.save(&reg);
        assert_eq!(saved.blocks.len(), 2);
        saved
            .blocks
            .push((BlockPos::new(0, 0, 0), "hearth:no_such_block".into()));
        let back = Edits::load(saved.clone(), &reg);
        assert_eq!(back.get(BlockPos::new(3, 4, 5)), Some(fire));
        assert_eq!(back.get(BlockPos::new(-20, 1, 7)), Some(BlockStateId::AIR));
        assert_eq!(back.len(), 2);
        // The unknown kind is not shown but is saved again as it was.
        assert_eq!(back.save(&reg).blocks.len(), 3);
    }
}

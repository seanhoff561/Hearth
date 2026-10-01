//! Block states used by the generator, resolved once from the registry by name.

use hearth_world::{BlockRegistry, BlockStateId};

/// A wood species.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Wood {
    Oak,
    Birch,
    Spruce,
    Mangrove,
}

/// States for one wood species.
#[derive(Debug, Clone, Copy)]
pub struct WoodStates {
    pub log_y: BlockStateId,
    pub log_x: BlockStateId,
    pub log_z: BlockStateId,
    /// Leaves by distance 1..=7 (index 0 = distance 1).
    pub leaves: [BlockStateId; 7],
}

/// All states the generator places. Rock comes from the geology (`crate::geology`).
#[derive(Debug, Clone)]
pub struct GenBlocks {
    pub air: BlockStateId,
    /// Per state: 1 a tree's wood (log, limb, bark), 2 its foliage, 0 anything else.
    pub tree_part: Vec<u8>,
    /// Per state: rock made of a material (bedrock and outcrops), anything caves may cut, and
    /// ground plants can grow in.
    rock: Vec<bool>,
    carvable: Vec<bool>,
    plantable: Vec<bool>,
    /// Loose-stone block of each rock (by the rock's state), and whether a state is one.
    cobbles: rustc_hash::FxHashMap<BlockStateId, BlockStateId>,
    loose_stone: Vec<bool>,
    pub cobblestone: BlockStateId,
    pub mossy_cobblestone: BlockStateId,
    pub grass: BlockStateId,
    pub grass_snowy: BlockStateId,
    pub podzol: BlockStateId,
    pub podzol_snowy: BlockStateId,
    pub moss_block: BlockStateId,
    pub snow_layers: [BlockStateId; 8],
    pub snow_block: BlockStateId,
    pub ice: BlockStateId,
    pub packed_ice: BlockStateId,
    pub water: BlockStateId,
    pub oak: WoodStates,
    pub birch: WoodStates,
    pub spruce: WoodStates,
    pub mangrove: WoodStates,
    /// Mangrove prop roots in air and in water.
    pub mangrove_roots: [BlockStateId; 2],
    pub coral: BlockStateId,
    pub coral_block: BlockStateId,
    pub seaweed: BlockStateId,
    pub cordgrass: [BlockStateId; 2],
    pub short_grass: BlockStateId,
    pub tall_grass: [BlockStateId; 2],
    pub fern: BlockStateId,
    pub large_fern: [BlockStateId; 2],
    pub short_dry_grass: BlockStateId,
    pub tall_dry_grass: [BlockStateId; 2],
    pub dead_bush: BlockStateId,
    /// Useful plants of the temperate woods (stand-ins until the flora framework): nettle
    /// patches, hazel bushes (two blocks tall), brambles.
    pub nettle: BlockStateId,
    pub hazel: [BlockStateId; 2],
    pub bramble: BlockStateId,
    pub flowers_meadow: [BlockStateId; 5],
    pub flowers_forest: [BlockStateId; 2],
    pub flowers_alpine: [BlockStateId; 2],
    pub brown_mushroom: BlockStateId,
    pub red_mushroom: BlockStateId,
    pub moss_carpet: BlockStateId,
    pub lily_pad: BlockStateId,
    pub sugar_cane: BlockStateId,
    pub cactus: BlockStateId,
    pub seagrass: BlockStateId,
    pub tall_seagrass: [BlockStateId; 2],
    pub kelp: BlockStateId,
    pub kelp_plant: BlockStateId,
    /// Vine facing each horizontal side (north, east, south, west): the side of the block the
    /// vine is attached to.
    pub vines: [BlockStateId; 4],
}

/// Error when a required block is missing from the registry.
#[derive(Debug, Clone, thiserror::Error)]
#[error("world generation needs block state {0}, which is not registered")]
pub struct MissingBlock(pub String);

impl GenBlocks {
    pub fn resolve(reg: &BlockRegistry) -> Result<Self, MissingBlock> {
        let s = |name: &str| -> Result<BlockStateId, MissingBlock> {
            reg.parse_state(name)
                .map_err(|_| MissingBlock(name.to_owned()))
        };
        let wood = |w: &str| -> Result<WoodStates, MissingBlock> {
            // Leaves by distance where the block keeps one (the old trees), else the one state.
            let mut leaves = [BlockStateId::AIR; 7];
            let plain = reg.parse_state(&format!("{w}_leaves"));
            for (i, l) in leaves.iter_mut().enumerate() {
                *l = match s(&format!(
                    "{w}_leaves[distance={},persistent=false,waterlogged=false]",
                    i + 1
                )) {
                    Ok(st) => st,
                    Err(e) => plain.clone().map_err(|_| e)?,
                };
            }
            Ok(WoodStates {
                log_y: s(&format!("{w}_log[axis=y]"))?,
                log_x: s(&format!("{w}_log[axis=x]"))?,
                log_z: s(&format!("{w}_log[axis=z]"))?,
                leaves,
            })
        };
        let pair = |name: &str| -> Result<[BlockStateId; 2], MissingBlock> {
            Ok([
                s(&format!("{name}[half=lower]"))?,
                s(&format!("{name}[half=upper]"))?,
            ])
        };
        let mut snow_layers = [BlockStateId::AIR; 8];
        for (i, l) in snow_layers.iter_mut().enumerate() {
            *l = s(&format!("snow[layers={}]", i + 1))?;
        }
        // Terrain: opaque blocks of a material (wood is a material too, but it burns).
        let rock: Vec<bool> = (0..reg.state_count())
            .map(|i| {
                let st = BlockStateId(i as u16);
                let def = &reg.block_of(st).def;
                def.material.is_some() && !def.flammable && reg.is_opaque(st)
            })
            .collect();
        let carvable = rock.clone();
        let plantable = vec![false; reg.state_count()];
        let mut cobbles = rustc_hash::FxHashMap::default();
        let mut loose_stone = vec![false; reg.state_count()];
        for block in reg.blocks() {
            if let Some(rock) = block.name.path().strip_suffix("_cobbles")
                && let Ok(r) = reg.parse_state(&format!("{}:{rock}", block.name.namespace()))
            {
                cobbles.insert(r, block.default_state);
                loose_stone[block.default_state.0 as usize] = true;
            }
        }
        let mut tree_part = vec![0u8; reg.state_count()];
        for block in reg.blocks() {
            let path = block.name.path();
            let part =
                if path.ends_with("_log") || path.ends_with("_branch") || path.ends_with("_wood") {
                    1
                } else if path.ends_with("_leaves") {
                    2
                } else {
                    0
                };
            let first = block.first_state.0 as usize;
            tree_part[first..first + block.state_count as usize].fill(part);
        }
        Ok(Self {
            air: BlockStateId::AIR,
            tree_part,
            rock,
            carvable,
            plantable,
            cobbles,
            loose_stone,
            cobblestone: s("cobblestone")?,
            mossy_cobblestone: s("mossy_cobblestone")?,
            grass: s("grass_block[snowy=false]")?,
            grass_snowy: s("grass_block[snowy=true]")?,
            podzol: s("podzol[snowy=false]")?,
            podzol_snowy: s("podzol[snowy=true]")?,
            moss_block: s("moss_block")?,
            snow_layers,
            snow_block: s("snow_block")?,
            ice: s("ice")?,
            packed_ice: s("packed_ice")?,
            water: s("water[level=0]")?,
            oak: wood("oak")?,
            birch: wood("birch")?,
            spruce: wood("spruce")?,
            mangrove: wood("mangrove")?,
            mangrove_roots: [
                s("mangrove_roots[waterlogged=false]")?,
                s("mangrove_roots[waterlogged=true]")?,
            ],
            coral: s("coral")?,
            coral_block: s("coral_block")?,
            seaweed: s("seaweed")?,
            cordgrass: pair("cordgrass")?,
            short_grass: s("short_grass")?,
            tall_grass: pair("tall_grass")?,
            fern: s("fern")?,
            large_fern: pair("large_fern")?,
            short_dry_grass: s("short_dry_grass")?,
            tall_dry_grass: pair("tall_dry_grass")?,
            dead_bush: s("dead_bush")?,
            nettle: s("nettle")?,
            hazel: pair("hazel")?,
            bramble: s("bramble")?,
            flowers_meadow: [
                s("dandelion")?,
                s("poppy")?,
                s("cornflower")?,
                s("oxeye_daisy")?,
                s("azure_bluet")?,
            ],
            flowers_forest: [s("lily_of_the_valley")?, s("poppy")?],
            flowers_alpine: [s("gentian")?, s("alpine_aster")?],
            brown_mushroom: s("brown_mushroom")?,
            red_mushroom: s("red_mushroom")?,
            moss_carpet: s("moss_carpet")?,
            lily_pad: s("lily_pad")?,
            sugar_cane: s("sugar_cane[age=0]")?,
            cactus: s("cactus[age=0]")?,
            seagrass: s("seagrass")?,
            tall_seagrass: pair("tall_seagrass")?,
            kelp: s("kelp[age=0]")?,
            kelp_plant: s("kelp_plant")?,
            vines: [
                s("vine[north=true]")?,
                s("vine[east=true]")?,
                s("vine[south=true]")?,
                s("vine[west=true]")?,
            ],
        })
    }

    pub fn wood(&self, w: Wood) -> &WoodStates {
        match w {
            Wood::Oak => &self.oak,
            Wood::Birch => &self.birch,
            Wood::Spruce => &self.spruce,
            Wood::Mangrove => &self.mangrove,
        }
    }

    /// Marks the soils' and sediments' blocks as ground plants grow in and caves may cut.
    pub fn add_ground(&mut self, ground: impl Iterator<Item = BlockStateId>) {
        for g in ground {
            if let Some(p) = self.plantable.get_mut(g.0 as usize) {
                *p = true;
            }
            if let Some(c) = self.carvable.get_mut(g.0 as usize) {
                *c = true;
            }
        }
    }

    /// Loose stones of a rock, if it has them.
    pub fn cobbles_for(&self, rock: BlockStateId) -> Option<BlockStateId> {
        self.cobbles.get(&rock).copied()
    }

    /// True for loose stones lying on the ground.
    #[inline]
    pub fn is_loose_stone(&self, s: BlockStateId) -> bool {
        self.loose_stone.get(s.0 as usize).copied().unwrap_or(false)
    }

    /// True for ground plants can grow in (turf, soils, sands, muds).
    #[inline]
    pub fn is_plantable(&self, s: BlockStateId) -> bool {
        self.plantable.get(s.0 as usize).copied().unwrap_or(false)
    }

    /// True for rock that deposits may replace.
    #[inline]
    pub fn is_base_rock(&self, s: BlockStateId) -> bool {
        self.rock.get(s.0 as usize).copied().unwrap_or(false)
    }

    /// True for terrain that caves may carve.
    #[inline]
    pub fn is_carvable(&self, s: BlockStateId) -> bool {
        self.carvable.get(s.0 as usize).copied().unwrap_or(false)
    }
}

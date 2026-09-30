//! Block states used by the generator, resolved once from the registry by name.

use hearth_world::{BlockRegistry, BlockStateId};

use crate::region::Surface;

/// A wood species.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Wood {
    Oak,
    Birch,
    Spruce,
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

/// All states the generator places.
#[derive(Debug, Clone)]
pub struct GenBlocks {
    pub air: BlockStateId,
    pub stone: BlockStateId,
    pub deepslate: BlockStateId,
    pub cobblestone: BlockStateId,
    pub mossy_cobblestone: BlockStateId,
    pub granite: BlockStateId,
    pub diorite: BlockStateId,
    pub andesite: BlockStateId,
    pub tuff: BlockStateId,
    pub calcite: BlockStateId,
    pub sandstone: BlockStateId,
    pub red_sandstone: BlockStateId,
    pub grass: BlockStateId,
    pub grass_snowy: BlockStateId,
    pub dirt: BlockStateId,
    pub coarse_dirt: BlockStateId,
    pub rooted_dirt: BlockStateId,
    pub podzol: BlockStateId,
    pub podzol_snowy: BlockStateId,
    pub mud: BlockStateId,
    pub clay: BlockStateId,
    pub sand: BlockStateId,
    pub red_sand: BlockStateId,
    pub gravel: BlockStateId,
    pub moss_block: BlockStateId,
    pub snow_layers: [BlockStateId; 8],
    pub snow_block: BlockStateId,
    pub ice: BlockStateId,
    pub packed_ice: BlockStateId,
    pub water: BlockStateId,
    pub oak: WoodStates,
    pub birch: WoodStates,
    pub spruce: WoodStates,
    pub short_grass: BlockStateId,
    pub tall_grass: [BlockStateId; 2],
    pub fern: BlockStateId,
    pub large_fern: [BlockStateId; 2],
    pub short_dry_grass: BlockStateId,
    pub tall_dry_grass: [BlockStateId; 2],
    pub dead_bush: BlockStateId,
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
            let mut leaves = [BlockStateId::AIR; 7];
            for (i, l) in leaves.iter_mut().enumerate() {
                *l = s(&format!(
                    "{w}_leaves[distance={},persistent=false,waterlogged=false]",
                    i + 1
                ))?;
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
        Ok(Self {
            air: BlockStateId::AIR,
            stone: s("stone")?,
            deepslate: s("deepslate")?,
            cobblestone: s("cobblestone")?,
            mossy_cobblestone: s("mossy_cobblestone")?,
            granite: s("granite")?,
            diorite: s("diorite")?,
            andesite: s("andesite")?,
            tuff: s("tuff")?,
            calcite: s("calcite")?,
            sandstone: s("sandstone")?,
            red_sandstone: s("red_sandstone")?,
            grass: s("grass_block[snowy=false]")?,
            grass_snowy: s("grass_block[snowy=true]")?,
            dirt: s("dirt")?,
            coarse_dirt: s("coarse_dirt")?,
            rooted_dirt: s("rooted_dirt")?,
            podzol: s("podzol[snowy=false]")?,
            podzol_snowy: s("podzol[snowy=true]")?,
            mud: s("mud")?,
            clay: s("clay")?,
            sand: s("sand")?,
            red_sand: s("red_sand")?,
            gravel: s("gravel")?,
            moss_block: s("moss_block")?,
            snow_layers,
            snow_block: s("snow_block")?,
            ice: s("ice")?,
            packed_ice: s("packed_ice")?,
            water: s("water[level=0]")?,
            oak: wood("oak")?,
            birch: wood("birch")?,
            spruce: wood("spruce")?,
            short_grass: s("short_grass")?,
            tall_grass: pair("tall_grass")?,
            fern: s("fern")?,
            large_fern: pair("large_fern")?,
            short_dry_grass: s("short_dry_grass")?,
            tall_dry_grass: pair("tall_dry_grass")?,
            dead_bush: s("dead_bush")?,
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
        }
    }

    /// Top-block state for a surface kind.
    pub fn surface(&self, s: Surface, snowy: bool) -> BlockStateId {
        match s {
            Surface::Grass => {
                if snowy {
                    self.grass_snowy
                } else {
                    self.grass
                }
            }
            Surface::SnowGrass => self.grass_snowy,
            Surface::Podzol => {
                if snowy {
                    self.podzol_snowy
                } else {
                    self.podzol
                }
            }
            Surface::CoarseDirt => self.coarse_dirt,
            Surface::Dirt => self.dirt,
            Surface::Sand => self.sand,
            Surface::RedSand => self.red_sand,
            Surface::Gravel => self.gravel,
            Surface::Stone => self.stone,
            Surface::Snow => self.snow_block,
            Surface::Ice => self.packed_ice,
            Surface::Mud => self.mud,
            Surface::Clay => self.clay,
            Surface::Calcite => self.calcite,
            Surface::Moss => self.moss_block,
            Surface::Sandstone => self.sandstone,
            Surface::RedSandstone => self.red_sandstone,
            Surface::Tuff => self.tuff,
        }
    }

    /// Filler state under the top block.
    pub fn filler(&self, s: Surface) -> BlockStateId {
        match s {
            Surface::Grass | Surface::SnowGrass | Surface::Podzol | Surface::Moss => self.dirt,
            other => self.surface(other, false),
        }
    }

    /// True for stone-like base rock that ores and caves may replace.
    #[inline]
    pub fn is_base_rock(&self, s: BlockStateId) -> bool {
        s == self.stone
            || s == self.deepslate
            || s == self.granite
            || s == self.diorite
            || s == self.andesite
            || s == self.tuff
    }

    /// True for terrain that caves may carve.
    #[inline]
    pub fn is_carvable(&self, s: BlockStateId) -> bool {
        self.is_base_rock(s)
            || s == self.dirt
            || s == self.gravel
            || s == self.sandstone
            || s == self.red_sandstone
            || s == self.coarse_dirt
            || s == self.clay
            || s == self.calcite
            || s == self.grass
            || s == self.grass_snowy
            || s == self.podzol
            || s == self.sand
            || s == self.red_sand
    }
}

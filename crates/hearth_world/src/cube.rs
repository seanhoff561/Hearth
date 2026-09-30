//! A 16³ cube: blocks, light and bookkeeping.

use hearth_math::{CUBE_AREA, CUBE_VOLUME, LocalPos};

use crate::block::{BlockRegistry, BlockStateId, StateFlags};
use crate::light::LightData;
use crate::palette::{PaletteError, PalettedBlocks};

/// Lighting status of a cube.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LightStatus {
    /// Light has never been computed.
    #[default]
    Unlit,
    /// Light is valid.
    Lit,
}

/// One cube of the world.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cube {
    pub blocks: PalettedBlocks,
    pub sky_light: LightData,
    pub block_light: LightData,
    pub light_status: LightStatus,
    /// Incremented on every block change (used to invalidate meshes and saves).
    pub version: u32,
    non_air: u16,
}

impl Cube {
    /// A cube filled with one state (air cubes are sky-lit by default; solid fills dark).
    pub fn filled(state: BlockStateId) -> Self {
        Self {
            blocks: PalettedBlocks::filled(state),
            sky_light: LightData::Uniform(if state.is_air() { 15 } else { 0 }),
            block_light: LightData::Uniform(0),
            light_status: LightStatus::Unlit,
            version: 0,
            non_air: if state.is_air() {
                0
            } else {
                CUBE_VOLUME as u16
            },
        }
    }

    /// Builds from a full array of states.
    pub fn from_states(states: &[BlockStateId]) -> Self {
        let blocks = PalettedBlocks::from_states(states);
        let non_air = states.iter().filter(|s| !s.is_air()).count() as u16;
        Self {
            blocks,
            sky_light: LightData::Uniform(0),
            block_light: LightData::Uniform(0),
            light_status: LightStatus::Unlit,
            version: 0,
            non_air,
        }
    }

    /// Builds from already-compressed blocks.
    pub fn from_blocks(blocks: PalettedBlocks) -> Self {
        let non_air = blocks.count(|s| !s.is_air()) as u16;
        Self {
            blocks,
            sky_light: LightData::Uniform(0),
            block_light: LightData::Uniform(0),
            light_status: LightStatus::Unlit,
            version: 0,
            non_air,
        }
    }

    #[inline]
    pub fn get(&self, p: LocalPos) -> BlockStateId {
        self.blocks.get(p.index())
    }

    #[inline]
    pub fn get_index(&self, i: usize) -> BlockStateId {
        self.blocks.get(i)
    }

    /// Sets a block; returns the previous state.
    pub fn set(&mut self, p: LocalPos, state: BlockStateId) -> BlockStateId {
        let old = self.blocks.set(p.index(), state);
        if old != state {
            self.version = self.version.wrapping_add(1);
            match (old.is_air(), state.is_air()) {
                (true, false) => self.non_air += 1,
                (false, true) => self.non_air -= 1,
                _ => {}
            }
        }
        old
    }

    /// True if every block is air.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.non_air == 0
    }

    pub fn non_air_count(&self) -> usize {
        self.non_air as usize
    }

    #[inline]
    pub fn sky_light(&self, p: LocalPos) -> u8 {
        self.sky_light.get(p.index())
    }

    #[inline]
    pub fn block_light(&self, p: LocalPos) -> u8 {
        self.block_light.get(p.index())
    }

    /// For each (x, z) column of this cube (index `z * 16 + x`), the local Y of the highest
    /// sky-blocking block, or -1.
    pub fn highest_sky_blocking(&self, reg: &BlockRegistry) -> [i8; CUBE_AREA] {
        let mut out = [-1i8; CUBE_AREA];
        if let Some(s) = self.blocks.as_single() {
            if reg.has(s, StateFlags::BLOCKS_SKY) {
                out.fill(15);
            }
            return out;
        }
        let mut all = [BlockStateId::AIR; CUBE_VOLUME];
        self.blocks.decode_into(&mut all);
        for (xz, slot) in out.iter_mut().enumerate() {
            for y in (0..16).rev() {
                if reg.has(all[(y << 8) | xz], StateFlags::BLOCKS_SKY) {
                    *slot = y as i8;
                    break;
                }
            }
        }
        out
    }

    /// Approximate heap memory in bytes.
    pub fn heap_bytes(&self) -> usize {
        self.blocks.heap_bytes() + self.sky_light.heap_bytes() + self.block_light.heap_bytes()
    }

    /// Serializes blocks and light.
    pub fn write_bytes(&self, out: &mut Vec<u8>) {
        out.push(match self.light_status {
            LightStatus::Unlit => 0,
            LightStatus::Lit => 1,
        });
        self.blocks.write_bytes(out);
        self.sky_light.write_bytes(out);
        self.block_light.write_bytes(out);
    }

    /// Parses the form written by [`Self::write_bytes`].
    pub fn read_bytes(
        bytes: &[u8],
        remap: &dyn Fn(u16) -> BlockStateId,
    ) -> Result<(Self, usize), PaletteError> {
        let status = match bytes.first() {
            Some(0) => LightStatus::Unlit,
            Some(1) => LightStatus::Lit,
            Some(_) => return Err(PaletteError::Corrupt("bad light status")),
            None => return Err(PaletteError::Truncated),
        };
        let mut pos = 1;
        let (blocks, n) = PalettedBlocks::read_bytes(&bytes[pos..], remap)?;
        pos += n;
        let (sky, n) =
            LightData::read_bytes(&bytes[pos..]).ok_or(PaletteError::Corrupt("bad sky light"))?;
        pos += n;
        let (block, n) =
            LightData::read_bytes(&bytes[pos..]).ok_or(PaletteError::Corrupt("bad block light"))?;
        pos += n;
        let mut cube = Cube::from_blocks(blocks);
        cube.sky_light = sky;
        cube.block_light = block;
        cube.light_status = status;
        Ok((cube, pos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::tests::test_registry;

    #[test]
    fn non_air_tracking() {
        let mut c = Cube::filled(BlockStateId::AIR);
        assert!(c.is_empty());
        let stone = BlockStateId(1);
        c.set(LocalPos::new(1, 2, 3), stone);
        assert_eq!(c.non_air_count(), 1);
        c.set(LocalPos::new(1, 2, 3), stone);
        assert_eq!(c.non_air_count(), 1);
        c.set(LocalPos::new(1, 2, 3), BlockStateId::AIR);
        assert!(c.is_empty());
        assert_eq!(c.version, 2);
        assert_eq!(Cube::filled(stone).non_air_count(), CUBE_VOLUME);
    }

    #[test]
    fn highest_blocking() {
        let reg = test_registry();
        let stone = reg.default_state("stone");
        let torch = reg.default_state("torch");
        let mut c = Cube::filled(BlockStateId::AIR);
        c.set(LocalPos::new(0, 3, 0), stone);
        c.set(LocalPos::new(0, 9, 0), torch); // torches don't block sky light
        c.set(LocalPos::new(5, 15, 7), stone);
        let h = c.highest_sky_blocking(&reg);
        assert_eq!(h[0], 3);
        assert_eq!(h[7 * 16 + 5], 15);
        assert_eq!(h[1], -1);
        assert_eq!(Cube::filled(stone).highest_sky_blocking(&reg)[200], 15);
    }

    #[test]
    fn bytes_round_trip() {
        let mut c = Cube::filled(BlockStateId(1));
        c.set(LocalPos::new(4, 4, 4), BlockStateId(2));
        c.sky_light.set(10, 9);
        c.light_status = LightStatus::Lit;
        let mut b = Vec::new();
        c.write_bytes(&mut b);
        let (back, n) = Cube::read_bytes(&b, &|v| BlockStateId(v)).unwrap();
        assert_eq!(n, b.len());
        assert_eq!(back.blocks, c.blocks);
        assert_eq!(back.sky_light, c.sky_light);
        assert_eq!(back.light_status, LightStatus::Lit);
        assert_eq!(back.non_air_count(), c.non_air_count());
    }
}

//! Palette-compressed storage of the 4096 block states of a cube.
//!
//! * `Single` — the whole cube is one state (air, stone fill): no allocation.
//! * `Indexed` — a local palette plus bit-packed indices with 1, 2, 4 or 8 bits per entry.
//!   Widths are powers of two so an entry never straddles a `u64` word.
//! * `Direct` — raw 16-bit state ids once a cube holds more than 256 distinct states.

use hearth_math::CUBE_VOLUME;

use crate::block::BlockStateId;

const WORDS_PER_BIT: usize = CUBE_VOLUME / 64; // words needed at 1 bit per entry

#[derive(Debug, Clone, PartialEq, Eq)]
enum Storage {
    Single(BlockStateId),
    Indexed {
        /// log2 of bits per entry: 0..=3 → 1, 2, 4, 8 bits.
        shift: u8,
        palette: Vec<BlockStateId>,
        data: Box<[u64]>,
    },
    Direct(Box<[BlockStateId]>),
}

/// Block states of one cube.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PalettedBlocks {
    storage: Storage,
}

impl Default for PalettedBlocks {
    fn default() -> Self {
        Self::filled(BlockStateId::AIR)
    }
}

#[inline]
fn bits_of(shift: u8) -> usize {
    1usize << shift
}

impl PalettedBlocks {
    /// A cube filled with one state.
    pub fn filled(state: BlockStateId) -> Self {
        Self {
            storage: Storage::Single(state),
        }
    }

    /// Builds from a full array of 4096 states, choosing the most compact representation.
    pub fn from_states(states: &[BlockStateId]) -> Self {
        assert_eq!(states.len(), CUBE_VOLUME);
        let first = states[0];
        if states.iter().all(|s| *s == first) {
            return Self::filled(first);
        }
        let mut palette: Vec<BlockStateId> = Vec::with_capacity(16);
        // Small open-addressed lookup keyed by state id for building the palette quickly.
        let mut lookup: rustc_hash::FxHashMap<u16, u16> = rustc_hash::FxHashMap::default();
        for s in states {
            if let std::collections::hash_map::Entry::Vacant(e) = lookup.entry(s.0) {
                e.insert(palette.len() as u16);
                palette.push(*s);
                if palette.len() > 256 {
                    return Self {
                        storage: Storage::Direct(states.to_vec().into_boxed_slice()),
                    };
                }
            }
        }
        let shift = shift_for(palette.len());
        let bits = bits_of(shift);
        let mut data = vec![0u64; WORDS_PER_BIT * bits].into_boxed_slice();
        let per_word = 64 / bits;
        for (i, s) in states.iter().enumerate() {
            let idx = lookup[&s.0] as u64;
            data[i / per_word] |= idx << ((i % per_word) * bits);
        }
        Self {
            storage: Storage::Indexed {
                shift,
                palette,
                data,
            },
        }
    }

    /// The single state if the cube is uniform.
    #[inline]
    pub fn as_single(&self) -> Option<BlockStateId> {
        match self.storage {
            Storage::Single(s) => Some(s),
            _ => None,
        }
    }

    /// True if every block is `state`.
    pub fn is_uniform(&self, state: BlockStateId) -> bool {
        self.as_single() == Some(state)
    }

    #[inline]
    pub fn get(&self, index: usize) -> BlockStateId {
        match &self.storage {
            Storage::Single(s) => *s,
            Storage::Indexed {
                shift,
                palette,
                data,
            } => {
                let bits = bits_of(*shift);
                let per_word = 64 / bits;
                let word = data[index / per_word];
                let v = (word >> ((index % per_word) * bits)) & ((1u64 << bits) - 1);
                palette[v as usize]
            }
            Storage::Direct(d) => d[index],
        }
    }

    /// Sets a block, growing the palette as needed. Returns the previous state.
    pub fn set(&mut self, index: usize, state: BlockStateId) -> BlockStateId {
        let old = self.get(index);
        if old == state {
            return old;
        }
        // A full 8-bit palette that doesn't contain the state: switch to direct storage.
        if let Storage::Indexed {
            shift: 3, palette, ..
        } = &self.storage
            && palette.len() == 256
            && !palette.contains(&state)
        {
            let mut all = [BlockStateId::AIR; CUBE_VOLUME];
            self.decode_into(&mut all);
            all[index] = state;
            self.storage = Storage::Direct(all.to_vec().into_boxed_slice());
            return old;
        }
        match &mut self.storage {
            Storage::Single(s) => {
                let fill = *s;
                let mut data = vec![0u64; WORDS_PER_BIT].into_boxed_slice();
                data[index / 64] |= 1u64 << (index % 64);
                self.storage = Storage::Indexed {
                    shift: 0,
                    palette: vec![fill, state],
                    data,
                };
            }
            Storage::Indexed {
                shift,
                palette,
                data,
            } => {
                let idx = match palette.iter().position(|p| *p == state) {
                    Some(i) => i,
                    None => {
                        if palette.len() >= (1 << bits_of(*shift)) {
                            let (new_shift, new_data) = repack(*shift, data, *shift + 1);
                            *shift = new_shift;
                            *data = new_data;
                        }
                        palette.push(state);
                        palette.len() - 1
                    }
                };
                let bits = bits_of(*shift);
                let per_word = 64 / bits;
                let w = index / per_word;
                let off = (index % per_word) * bits;
                let mask = ((1u64 << bits) - 1) << off;
                data[w] = (data[w] & !mask) | ((idx as u64) << off);
            }
            Storage::Direct(d) => d[index] = state,
        }
        old
    }

    /// Fills the whole cube with one state.
    pub fn fill(&mut self, state: BlockStateId) {
        self.storage = Storage::Single(state);
    }

    /// Decodes all 4096 states into `out` (fast bulk path for meshing and lighting).
    pub fn decode_into(&self, out: &mut [BlockStateId; CUBE_VOLUME]) {
        match &self.storage {
            Storage::Single(s) => out.fill(*s),
            Storage::Indexed {
                shift,
                palette,
                data,
            } => {
                let bits = bits_of(*shift);
                let per_word = 64 / bits;
                let mask = (1u64 << bits) - 1;
                for (w, word) in data.iter().enumerate() {
                    let mut word = *word;
                    let base = w * per_word;
                    for slot in out[base..base + per_word].iter_mut() {
                        *slot = palette[(word & mask) as usize];
                        word >>= bits;
                    }
                }
            }
            Storage::Direct(d) => out.copy_from_slice(d),
        }
    }

    /// Distinct states currently referenced by the palette (may include unused entries until
    /// [`Self::compact`] runs).
    pub fn palette(&self) -> Vec<BlockStateId> {
        match &self.storage {
            Storage::Single(s) => vec![*s],
            Storage::Indexed { palette, .. } => palette.clone(),
            Storage::Direct(d) => {
                let mut v: Vec<_> = d.to_vec();
                v.sort_unstable();
                v.dedup();
                v
            }
        }
    }

    /// Rebuilds the storage in its most compact form (drops unused palette entries, collapses
    /// to `Single` when uniform).
    pub fn compact(&mut self) {
        if matches!(self.storage, Storage::Single(_)) {
            return;
        }
        let mut all = [BlockStateId::AIR; CUBE_VOLUME];
        self.decode_into(&mut all);
        *self = Self::from_states(&all);
    }

    /// Bits per entry of the current representation (0 for uniform, 16 for direct).
    pub fn bits_per_entry(&self) -> u32 {
        match &self.storage {
            Storage::Single(_) => 0,
            Storage::Indexed { shift, .. } => bits_of(*shift) as u32,
            Storage::Direct(_) => 16,
        }
    }

    /// Approximate heap bytes used.
    pub fn heap_bytes(&self) -> usize {
        match &self.storage {
            Storage::Single(_) => 0,
            Storage::Indexed { palette, data, .. } => palette.len() * 2 + data.len() * 8,
            Storage::Direct(d) => d.len() * 2,
        }
    }

    /// Counts blocks satisfying `pred`.
    pub fn count(&self, mut pred: impl FnMut(BlockStateId) -> bool) -> usize {
        match &self.storage {
            Storage::Single(s) => {
                if pred(*s) {
                    CUBE_VOLUME
                } else {
                    0
                }
            }
            _ => {
                let mut all = [BlockStateId::AIR; CUBE_VOLUME];
                self.decode_into(&mut all);
                all.iter().filter(|s| pred(**s)).count()
            }
        }
    }

    /// Serializes to a compact byte form: `[kind][...]`. Used by saves and the protocol.
    pub fn write_bytes(&self, out: &mut Vec<u8>) {
        match &self.storage {
            Storage::Single(s) => {
                out.push(0);
                out.extend_from_slice(&s.0.to_le_bytes());
            }
            Storage::Indexed {
                shift,
                palette,
                data,
            } => {
                out.push(1);
                out.push(*shift);
                out.extend_from_slice(&(palette.len() as u16).to_le_bytes());
                for p in palette {
                    out.extend_from_slice(&p.0.to_le_bytes());
                }
                for w in data.iter() {
                    out.extend_from_slice(&w.to_le_bytes());
                }
            }
            Storage::Direct(d) => {
                out.push(2);
                for s in d.iter() {
                    out.extend_from_slice(&s.0.to_le_bytes());
                }
            }
        }
    }

    /// Parses the form written by [`Self::write_bytes`], remapping every state id through
    /// `remap` (identity when ids didn't change). Returns the value and bytes consumed.
    pub fn read_bytes(
        bytes: &[u8],
        remap: &dyn Fn(u16) -> BlockStateId,
    ) -> Result<(Self, usize), PaletteError> {
        let mut r = Reader { bytes, pos: 0 };
        let kind = r.u8()?;
        let v = match kind {
            0 => Self::filled(remap(r.u16()?)),
            1 => {
                let shift = r.u8()?;
                if shift > 3 {
                    return Err(PaletteError::Corrupt("bad bit width"));
                }
                let len = r.u16()? as usize;
                if len == 0 || len > (1 << bits_of(shift)) {
                    return Err(PaletteError::Corrupt("bad palette length"));
                }
                let mut palette = Vec::with_capacity(len);
                for _ in 0..len {
                    palette.push(remap(r.u16()?));
                }
                let words = WORDS_PER_BIT * bits_of(shift);
                let mut data = Vec::with_capacity(words);
                for _ in 0..words {
                    data.push(r.u64()?);
                }
                // Validate indices so corrupt data can't cause out-of-bounds lookups.
                let bits = bits_of(shift);
                let mask = (1u64 << bits) - 1;
                for word in &data {
                    let mut w = *word;
                    for _ in 0..64 / bits {
                        if (w & mask) as usize >= len {
                            return Err(PaletteError::Corrupt("palette index out of range"));
                        }
                        w >>= bits;
                    }
                }
                Self {
                    storage: Storage::Indexed {
                        shift,
                        palette,
                        data: data.into_boxed_slice(),
                    },
                }
            }
            2 => {
                let mut d = Vec::with_capacity(CUBE_VOLUME);
                for _ in 0..CUBE_VOLUME {
                    d.push(remap(r.u16()?));
                }
                Self {
                    storage: Storage::Direct(d.into_boxed_slice()),
                }
            }
            _ => return Err(PaletteError::Corrupt("unknown storage kind")),
        };
        Ok((v, r.pos))
    }
}

fn shift_for(palette_len: usize) -> u8 {
    match palette_len {
        0..=2 => 0,
        3..=4 => 1,
        5..=16 => 2,
        _ => 3,
    }
}

fn repack(shift: u8, data: &[u64], new_shift: u8) -> (u8, Box<[u64]>) {
    let bits = bits_of(shift);
    let new_bits = bits_of(new_shift);
    let per_word = 64 / bits;
    let new_per_word = 64 / new_bits;
    let mask = (1u64 << bits) - 1;
    let mut out = vec![0u64; WORDS_PER_BIT * new_bits].into_boxed_slice();
    for i in 0..CUBE_VOLUME {
        let v = (data[i / per_word] >> ((i % per_word) * bits)) & mask;
        out[i / new_per_word] |= v << ((i % new_per_word) * new_bits);
    }
    (new_shift, out)
}

/// Errors decoding serialized palettes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PaletteError {
    #[error("unexpected end of cube data")]
    Truncated,
    #[error("corrupt cube data: {0}")]
    Corrupt(&'static str),
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], PaletteError> {
        if self.pos + n > self.bytes.len() {
            return Err(PaletteError::Truncated);
        }
        let s = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, PaletteError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, PaletteError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u64(&mut self) -> Result<u64, PaletteError> {
        let b = self.take(8)?;
        Ok(u64::from_le_bytes(b.try_into().expect("8 bytes")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hearth_math::hash::Rng;

    fn s(v: u16) -> BlockStateId {
        BlockStateId(v)
    }

    #[test]
    fn single_fast_path() {
        let p = PalettedBlocks::filled(s(5));
        assert_eq!(p.get(1234), s(5));
        assert_eq!(p.heap_bytes(), 0);
        assert_eq!(p.bits_per_entry(), 0);
    }

    #[test]
    fn grows_through_all_widths_and_back() {
        let mut p = PalettedBlocks::default();
        let mut expect = vec![s(0); CUBE_VOLUME];
        let mut rng = Rng::new(1);
        for n in 1..=300u16 {
            let i = rng.below(CUBE_VOLUME as u32) as usize;
            p.set(i, s(n));
            expect[i] = s(n);
            let width = p.bits_per_entry();
            assert!(matches!(width, 1 | 2 | 4 | 8 | 16), "{width}");
        }
        assert_eq!(p.bits_per_entry(), 16);
        for (i, e) in expect.iter().enumerate() {
            assert_eq!(p.get(i), *e);
        }
        // Overwrite everything with two states → compacts back to 1 bit.
        for i in 0..CUBE_VOLUME {
            p.set(i, s((i % 2) as u16 + 7));
        }
        p.compact();
        assert_eq!(p.bits_per_entry(), 1);
        for i in 0..CUBE_VOLUME {
            assert_eq!(p.get(i), s((i % 2) as u16 + 7));
        }
        p.fill(s(0));
        p.compact();
        assert_eq!(p.as_single(), Some(s(0)));
    }

    #[test]
    fn from_states_and_decode_agree() {
        let mut rng = Rng::new(7);
        for distinct in [1u16, 2, 3, 5, 16, 17, 200, 600] {
            let states: Vec<_> = (0..CUBE_VOLUME)
                .map(|_| s(rng.below(distinct as u32) as u16))
                .collect();
            let p = PalettedBlocks::from_states(&states);
            let mut out = [s(0); CUBE_VOLUME];
            p.decode_into(&mut out);
            assert_eq!(&out[..], &states[..], "distinct={distinct}");
            for (i, st) in states.iter().enumerate().step_by(97) {
                assert_eq!(p.get(i), *st);
            }
        }
    }

    #[test]
    fn serialization_round_trip_with_remap() {
        let mut rng = Rng::new(3);
        for distinct in [1u16, 4, 9, 100, 400] {
            let states: Vec<_> = (0..CUBE_VOLUME)
                .map(|_| s(rng.below(distinct as u32) as u16))
                .collect();
            let p = PalettedBlocks::from_states(&states);
            let mut bytes = Vec::new();
            p.write_bytes(&mut bytes);
            let (back, used) = PalettedBlocks::read_bytes(&bytes, &|v| s(v)).unwrap();
            assert_eq!(used, bytes.len());
            assert_eq!(back, p);
            let (shifted, _) = PalettedBlocks::read_bytes(&bytes, &|v| s(v + 1000)).unwrap();
            for i in (0..CUBE_VOLUME).step_by(131) {
                assert_eq!(shifted.get(i).0, states[i].0 + 1000);
            }
        }
    }

    #[test]
    fn corrupt_input_is_rejected() {
        assert_eq!(
            PalettedBlocks::read_bytes(&[], &|v| s(v)).unwrap_err(),
            PaletteError::Truncated
        );
        assert!(PalettedBlocks::read_bytes(&[9], &|v| s(v)).is_err());
        // Indexed, 1 bit, palette of 1 entry but data referencing index 1.
        let mut bytes = vec![1u8, 0, 1, 0, 0, 0];
        bytes.extend(std::iter::repeat_n(0xffu8, 8 * WORDS_PER_BIT));
        assert!(PalettedBlocks::read_bytes(&bytes, &|v| s(v)).is_err());
    }
}

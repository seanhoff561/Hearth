//! Light storage: 4-bit values per block, allocated lazily (uniform cubes store one value).

use hearth_math::CUBE_VOLUME;

/// Maximum light level.
pub const MAX_LIGHT: u8 = 15;

const NIBBLE_BYTES: usize = CUBE_VOLUME / 2;

/// One light channel (sky or block) of a cube.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LightData {
    Uniform(u8),
    Nibbles(Box<[u8; NIBBLE_BYTES]>),
}

impl Default for LightData {
    fn default() -> Self {
        LightData::Uniform(0)
    }
}

impl LightData {
    #[inline]
    pub fn get(&self, index: usize) -> u8 {
        match self {
            LightData::Uniform(v) => *v,
            LightData::Nibbles(n) => (n[index >> 1] >> ((index & 1) * 4)) & 15,
        }
    }

    #[inline]
    pub fn set(&mut self, index: usize, value: u8) {
        let value = value.min(MAX_LIGHT);
        match self {
            LightData::Uniform(v) => {
                if *v == value {
                    return;
                }
                let fill = *v | (*v << 4);
                let mut n = Box::new([fill; NIBBLE_BYTES]);
                write_nibble(&mut n, index, value);
                *self = LightData::Nibbles(n);
            }
            LightData::Nibbles(n) => write_nibble(n, index, value),
        }
    }

    pub fn fill(&mut self, value: u8) {
        *self = LightData::Uniform(value.min(MAX_LIGHT));
    }

    pub fn uniform_value(&self) -> Option<u8> {
        match self {
            LightData::Uniform(v) => Some(*v),
            LightData::Nibbles(_) => None,
        }
    }

    /// Collapses to `Uniform` when every value is equal.
    pub fn compact(&mut self) {
        if let LightData::Nibbles(n) = self {
            let first = n[0];
            if first >> 4 == first & 15 && n.iter().all(|b| *b == first) {
                *self = LightData::Uniform(first & 15);
            }
        }
    }

    pub fn heap_bytes(&self) -> usize {
        match self {
            LightData::Uniform(_) => 0,
            LightData::Nibbles(_) => NIBBLE_BYTES,
        }
    }

    pub fn write_bytes(&self, out: &mut Vec<u8>) {
        match self {
            LightData::Uniform(v) => {
                out.push(0);
                out.push(*v);
            }
            LightData::Nibbles(n) => {
                out.push(1);
                out.extend_from_slice(&n[..]);
            }
        }
    }

    pub fn read_bytes(bytes: &[u8]) -> Option<(Self, usize)> {
        match bytes.first()? {
            0 => Some((LightData::Uniform(*bytes.get(1)? & 15), 2)),
            1 => {
                let data = bytes.get(1..1 + NIBBLE_BYTES)?;
                let mut n = Box::new([0u8; NIBBLE_BYTES]);
                n.copy_from_slice(data);
                Some((LightData::Nibbles(n), 1 + NIBBLE_BYTES))
            }
            _ => None,
        }
    }
}

#[inline]
fn write_nibble(n: &mut [u8; NIBBLE_BYTES], index: usize, value: u8) {
    let b = &mut n[index >> 1];
    if index & 1 == 0 {
        *b = (*b & 0xf0) | value;
    } else {
        *b = (*b & 0x0f) | (value << 4);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lazy_allocation_and_compaction() {
        let mut l = LightData::Uniform(15);
        assert_eq!(l.get(100), 15);
        l.set(100, 15);
        assert_eq!(l.heap_bytes(), 0);
        l.set(101, 3);
        assert_eq!(l.get(101), 3);
        assert_eq!(l.get(100), 15);
        assert_eq!(l.get(102), 15);
        l.set(101, 15);
        l.compact();
        assert_eq!(l.uniform_value(), Some(15));
    }

    #[test]
    fn every_index_independent() {
        let mut l = LightData::default();
        for i in 0..CUBE_VOLUME {
            l.set(i, (i % 16) as u8);
        }
        for i in 0..CUBE_VOLUME {
            assert_eq!(l.get(i), (i % 16) as u8);
        }
        let mut bytes = Vec::new();
        l.write_bytes(&mut bytes);
        let (back, n) = LightData::read_bytes(&bytes).unwrap();
        assert_eq!(n, bytes.len());
        assert_eq!(back, l);
    }
}

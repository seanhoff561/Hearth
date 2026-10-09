//! The refinement levels' tiles kept on disk (E4.1 §4.4): a tile made once, about a place the
//! player has been or looked at closely, is read back the next time instead of made again (the
//! same tile: making it is deterministic), so a saved world opens faster. Kept per planet (its
//! grid's fingerprint) and per format, at most so much in all, the oldest let go first.

use std::path::{Path, PathBuf};

use hearth_core::disk_cache::Folder;

use crate::relief::Tile;

/// The files' format: bumped when a tile is made otherwise.
pub const FORMAT: u32 = 2;
/// A file's first bytes.
const MAGIC: &[u8; 4] = b"HRLT";
/// A file's head: magic, format, level, tile x and z, cells.
const HEAD: usize = 32;

/// Tiles kept in a folder of their own.
pub struct TileStore {
    folder: Folder,
}

impl TileStore {
    /// The store in `dir`, holding at most `cap` bytes.
    pub fn open(dir: &Path, cap: u64) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        Ok(Self {
            folder: Folder::new(dir, cap),
        })
    }

    fn path(&self, level: usize, tx: i64, tz: i64) -> PathBuf {
        self.folder.dir().join(format!("L{level}_{tx}_{tz}.tile"))
    }

    /// The tile kept for this place, if one is and is whole.
    pub fn load(&self, level: usize, tx: i64, tz: i64, span: usize) -> Option<Tile> {
        let file = std::fs::File::open(self.path(level, tx, tz)).ok()?;
        let bytes = zstd::decode_all(file).ok()?;
        let (head, mut rest) = bytes.split_at_checked(HEAD)?;
        let word = |i: usize| u32::from_le_bytes([head[i], head[i + 1], head[i + 2], head[i + 3]]);
        let long = |i: usize| {
            let mut b = [0u8; 8];
            b.copy_from_slice(&head[i..i + 8]);
            i64::from_le_bytes(b)
        };
        let n = span * span;
        let fits = &head[..4] == MAGIC
            && word(4) == FORMAT
            && word(8) as usize == level
            && long(12) == tx
            && long(20) == tz
            && word(28) as usize == n
            && rest.len() == n * 15;
        if !fits {
            return None;
        }
        let mut floats = || -> Vec<f32> {
            let (a, b) = rest.split_at(n * 4);
            rest = b;
            a.chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect()
        };
        let h = floats();
        let lake = floats();
        let q = floats();
        let (recv, rest) = rest.split_at(n);
        let (channel, sea) = rest.split_at(n);
        Some(Tile {
            h,
            lake,
            q,
            recv: recv.to_vec(),
            channel: channel.iter().map(|b| *b != 0).collect(),
            sea: sea.iter().map(|b| *b != 0).collect(),
        })
    }

    /// Keeps a tile made, letting the oldest go when the store is full (quietly gives up if the
    /// disk will not have it).
    pub fn store(&self, level: usize, tx: i64, tz: i64, tile: &Tile) {
        let n = tile.h.len();
        let mut bytes = Vec::with_capacity(HEAD + n * 15);
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&FORMAT.to_le_bytes());
        bytes.extend_from_slice(&(level as u32).to_le_bytes());
        bytes.extend_from_slice(&tx.to_le_bytes());
        bytes.extend_from_slice(&tz.to_le_bytes());
        bytes.extend_from_slice(&(n as u32).to_le_bytes());
        for v in [&tile.h, &tile.lake, &tile.q] {
            for f in v.iter() {
                bytes.extend_from_slice(&f.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&tile.recv);
        bytes.extend(tile.channel.iter().map(|b| *b as u8));
        bytes.extend(tile.sea.iter().map(|b| *b as u8));
        let Ok(packed) = zstd::encode_all(&bytes[..], 1) else {
            return;
        };
        if let Err(e) = self.folder.write(&self.path(level, tx, tz), &packed) {
            log::debug!("terrain tile L{level} {tx},{tz} not kept: {e}");
        }
    }

    /// What the store holds (bytes).
    pub fn used(&self) -> u64 {
        self.folder.used()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile(n: usize, k: usize) -> Tile {
        Tile {
            h: (0..n).map(|i| (i * k) as f32 * 0.5).collect(),
            lake: (0..n)
                .map(|i| if i % 7 == 0 { 3.0 } else { f32::NAN })
                .collect(),
            q: (0..n).map(|i| ((i + k) % 13) as f32).collect(),
            recv: (0..n).map(|i| (i % 9) as u8).collect(),
            channel: (0..n).map(|i| i % 3 == 0).collect(),
            sea: (0..n).map(|i| i % 5 == 0).collect(),
        }
    }

    #[test]
    fn a_tile_kept_comes_back_the_same_and_the_store_keeps_to_its_size() {
        let dir = std::env::temp_dir().join(format!("hearth-tiles-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = TileStore::open(&dir, 1 << 20).expect("store");
        let t = tile(96 * 96, 1);
        store.store(2, -3, 41, &t);
        let back = store.load(2, -3, 41, 96).expect("kept");
        assert_eq!(back.h, t.h);
        assert_eq!(back.q, t.q);
        assert_eq!(back.recv, t.recv);
        assert_eq!(back.channel, t.channel);
        assert_eq!(back.sea, t.sea);
        assert!(
            back.lake
                .iter()
                .zip(&t.lake)
                .all(|(a, b)| a == b || a.is_nan() && b.is_nan())
        );
        // Another place, level or span: none.
        assert!(store.load(2, -3, 42, 96).is_none());
        assert!(store.load(3, -3, 41, 96).is_none());
        assert!(store.load(2, -3, 41, 64).is_none());
        // Filled to twice its most: it keeps to it, the newest kept.
        let count = (2 << 20) / store.used().max(1) + 2;
        for k in 0..count as i64 {
            store.store(1, k, 0, &tile(96 * 96, k as usize + 2));
        }
        assert!(store.used() <= 1 << 20, "{} bytes", store.used());
        assert!(store.load(1, count as i64 - 1, 0, 96).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

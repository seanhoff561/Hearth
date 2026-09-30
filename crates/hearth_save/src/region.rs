//! Region files: 8×8×8 cubes per file (`r.<x>.<y>.<z>.hrg`), each cube zstd-compressed.
//!
//! Layout: an 8-byte header (`HRGN`, format u16, reserved u16), then 512 index entries of
//! (sector offset u32, byte length u32), then 4 KiB sectors of data from sector 2 on. A record
//! is `[compression u8][raw length u32][payload]` where the payload is `[cube format u16]` plus
//! `Cube::write_bytes`. Rewritten cubes reuse their sectors when they fit, otherwise go to the
//! first free run (or the end of the file).

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use hearth_math::CubePos;
use hearth_world::{BlockStateId, Cube};
use rustc_hash::FxHashMap;

use crate::SaveError;

pub const REGION_SIZE: i32 = 8;
const CUBES: usize = (REGION_SIZE * REGION_SIZE * REGION_SIZE) as usize;
const SECTOR: u64 = 4096;
const HEADER_SECTORS: u64 = 2;
const MAGIC: &[u8; 4] = b"HRGN";
/// Region file layout version.
pub const REGION_FORMAT: u16 = 1;
/// Cube record payload version.
pub const CUBE_FORMAT: u16 = 1;

/// Region coordinates of a cube.
pub fn region_of(p: CubePos) -> (i32, i32, i32) {
    (
        p.x.div_euclid(REGION_SIZE),
        p.y.div_euclid(REGION_SIZE),
        p.z.div_euclid(REGION_SIZE),
    )
}

fn index_of(p: CubePos) -> usize {
    let x = p.x.rem_euclid(REGION_SIZE) as usize;
    let y = p.y.rem_euclid(REGION_SIZE) as usize;
    let z = p.z.rem_euclid(REGION_SIZE) as usize;
    (y * REGION_SIZE as usize + z) * REGION_SIZE as usize + x
}

struct RegionFile {
    file: File,
    index: Vec<(u32, u32)>,
    /// Sector usage (true = used), including the header.
    used: Vec<bool>,
}

impl RegionFile {
    fn open(path: &Path, create: bool) -> Result<Option<RegionFile>, SaveError> {
        if !create && !path.exists() {
            return Ok(None);
        }
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(create)
            .truncate(false)
            .open(path)?;
        let len = file.metadata()?.len();
        let mut index = vec![(0u32, 0u32); CUBES];
        if len == 0 {
            let mut header = vec![0u8; (HEADER_SECTORS * SECTOR) as usize];
            header[..4].copy_from_slice(MAGIC);
            header[4..6].copy_from_slice(&REGION_FORMAT.to_le_bytes());
            file.write_all(&header)?;
        } else {
            let mut header = vec![0u8; 8 + CUBES * 8];
            file.seek(SeekFrom::Start(0))?;
            file.read_exact(&mut header).map_err(|_| {
                SaveError::Corrupt(format!("{}: truncated region header", path.display()))
            })?;
            if &header[..4] != MAGIC {
                return Err(SaveError::Corrupt(format!(
                    "{}: not a region file",
                    path.display()
                )));
            }
            let version = u16::from_le_bytes([header[4], header[5]]);
            if version != REGION_FORMAT {
                return Err(SaveError::Incompatible(format!(
                    "{}: region format {version}, expected {REGION_FORMAT}",
                    path.display()
                )));
            }
            for (i, e) in index.iter_mut().enumerate() {
                let o = 8 + i * 8;
                let off = u32::from_le_bytes(header[o..o + 4].try_into().expect("4 bytes"));
                let n = u32::from_le_bytes(header[o + 4..o + 8].try_into().expect("4 bytes"));
                *e = (off, n);
            }
        }
        let total = (file.metadata()?.len().div_ceil(SECTOR)).max(HEADER_SECTORS) as usize;
        let mut used = vec![false; total];
        for u in used.iter_mut().take(HEADER_SECTORS as usize) {
            *u = true;
        }
        for &(off, n) in &index {
            if n == 0 {
                continue;
            }
            let start = off as usize;
            let count = (n as u64).div_ceil(SECTOR) as usize;
            if start < HEADER_SECTORS as usize || start + count > used.len() {
                return Err(SaveError::Corrupt(format!(
                    "{}: index points outside the file",
                    path.display()
                )));
            }
            for u in &mut used[start..start + count] {
                *u = true;
            }
        }
        Ok(Some(RegionFile { file, index, used }))
    }

    fn read(&mut self, i: usize) -> Result<Option<Vec<u8>>, SaveError> {
        let (off, n) = self.index[i];
        if n == 0 {
            return Ok(None);
        }
        let mut buf = vec![0u8; n as usize];
        self.file.seek(SeekFrom::Start(off as u64 * SECTOR))?;
        self.file.read_exact(&mut buf)?;
        Ok(Some(buf))
    }

    fn write(&mut self, i: usize, data: &[u8]) -> Result<(), SaveError> {
        let need = (data.len() as u64).div_ceil(SECTOR) as usize;
        let (old_off, old_n) = self.index[i];
        let old_count = (old_n as u64).div_ceil(SECTOR) as usize;
        // Free the old run, then find the first run that fits (possibly the same one).
        for u in &mut self.used[old_off as usize..old_off as usize + old_count] {
            *u = false;
        }
        let mut start = None;
        let mut run = 0;
        for (s, &u) in self.used.iter().enumerate() {
            if u {
                run = 0;
                continue;
            }
            run += 1;
            if run == need {
                start = Some(s + 1 - need);
                break;
            }
        }
        let start = start.unwrap_or(self.used.len());
        if start + need > self.used.len() {
            self.used.resize(start + need, false);
        }
        for u in &mut self.used[start..start + need] {
            *u = true;
        }
        self.file.seek(SeekFrom::Start(start as u64 * SECTOR))?;
        self.file.write_all(data)?;
        // Pad to a whole sector so the file length stays sector-aligned.
        let pad = (need as u64 * SECTOR) as usize - data.len();
        if pad > 0 {
            self.file.write_all(&vec![0u8; pad])?;
        }
        self.index[i] = (start as u32, data.len() as u32);
        let mut entry = [0u8; 8];
        entry[..4].copy_from_slice(&(start as u32).to_le_bytes());
        entry[4..].copy_from_slice(&(data.len() as u32).to_le_bytes());
        self.file.seek(SeekFrom::Start(8 + i as u64 * 8))?;
        self.file.write_all(&entry)?;
        Ok(())
    }
}

/// Reads and writes cubes of one world.
pub struct RegionStore {
    dir: PathBuf,
    open: FxHashMap<(i32, i32, i32), RegionFile>,
    /// zstd level for new records.
    pub level: i32,
}

impl RegionStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            open: FxHashMap::default(),
            level: 3,
        }
    }

    fn path(&self, r: (i32, i32, i32)) -> PathBuf {
        self.dir.join(format!("r.{}.{}.{}.hrg", r.0, r.1, r.2))
    }

    fn region(
        &mut self,
        r: (i32, i32, i32),
        create: bool,
    ) -> Result<Option<&mut RegionFile>, SaveError> {
        if !self.open.contains_key(&r) {
            if create {
                std::fs::create_dir_all(&self.dir)?;
            }
            match RegionFile::open(&self.path(r), create)? {
                Some(f) => {
                    // Keep the number of open handles bounded.
                    if self.open.len() >= 64 {
                        self.open.clear();
                    }
                    self.open.insert(r, f);
                }
                None => return Ok(None),
            }
        }
        Ok(self.open.get_mut(&r))
    }

    /// Stores a cube (block state ids as in the world's state palette).
    pub fn write_cube(&mut self, pos: CubePos, cube: &Cube) -> Result<(), SaveError> {
        let mut raw = Vec::with_capacity(512);
        raw.extend_from_slice(&CUBE_FORMAT.to_le_bytes());
        cube.write_bytes(&mut raw);
        let compressed = zstd::bulk::compress(&raw, self.level)?;
        let mut record = Vec::with_capacity(compressed.len() + 5);
        record.push(1);
        record.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        record.extend_from_slice(&compressed);
        let file = self.region(region_of(pos), true)?.expect("created");
        file.write(index_of(pos), &record)
    }

    /// Loads a cube, remapping saved state ids through `remap`. `Ok(None)` if never saved.
    pub fn read_cube(
        &mut self,
        pos: CubePos,
        remap: &dyn Fn(u16) -> BlockStateId,
    ) -> Result<Option<Cube>, SaveError> {
        let Some(file) = self.region(region_of(pos), false)? else {
            return Ok(None);
        };
        let Some(record) = file.read(index_of(pos))? else {
            return Ok(None);
        };
        let corrupt = |what: &str| SaveError::Corrupt(format!("cube {pos:?}: {what}"));
        if record.len() < 5 {
            return Err(corrupt("truncated record"));
        }
        let raw_len = u32::from_le_bytes(record[1..5].try_into().expect("4 bytes")) as usize;
        let raw = match record[0] {
            0 => record[5..].to_vec(),
            1 => zstd::bulk::decompress(&record[5..], raw_len)?,
            _ => return Err(corrupt("unknown compression")),
        };
        if raw.len() < 2 {
            return Err(corrupt("empty payload"));
        }
        let format = u16::from_le_bytes([raw[0], raw[1]]);
        if format != CUBE_FORMAT {
            return Err(SaveError::Incompatible(format!(
                "cube {pos:?}: cube format {format}, expected {CUBE_FORMAT}"
            )));
        }
        let (cube, _) = Cube::read_bytes(&raw[2..], remap).map_err(|e| corrupt(&e.to_string()))?;
        Ok(Some(cube))
    }

    /// Flushes and closes all open region files.
    pub fn flush(&mut self) -> Result<(), SaveError> {
        for f in self.open.values_mut() {
            f.file.flush()?;
            f.file.sync_data()?;
        }
        self.open.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hearth_math::LocalPos;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("hearth_region_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn cubes_round_trip_and_rewrite_in_place() {
        let dir = tmp("rt");
        let mut store = RegionStore::new(&dir);
        let id = |i: u16| BlockStateId(i);
        let mut a = Cube::filled(id(1));
        a.set(LocalPos::new(3, 4, 5), id(7));
        let b = Cube::filled(id(2));
        let pa = CubePos::new(-1, 3, 9);
        let pb = CubePos::new(-8, 3, 9);
        store.write_cube(pa, &a).unwrap();
        store.write_cube(pb, &b).unwrap();
        // A noisy cube needs more sectors; rewriting must not corrupt the neighbour.
        let mut noisy = Cube::filled(id(1));
        for i in 0..4096usize {
            noisy.set(LocalPos::from_index(i), id((i * 7919 % 251) as u16));
        }
        store.write_cube(pa, &noisy).unwrap();
        store.flush().unwrap();
        let mut store = RegionStore::new(&dir);
        let ident = |i: u16| BlockStateId(i);
        let ra = store.read_cube(pa, &ident).unwrap().unwrap();
        let rb = store.read_cube(pb, &ident).unwrap().unwrap();
        for i in (0..4096usize).step_by(97) {
            assert_eq!(ra.get_index(i), noisy.get_index(i));
        }
        assert_eq!(rb.get(LocalPos::new(0, 0, 0)), id(2));
        assert!(
            store
                .read_cube(CubePos::new(100, 0, 0), &ident)
                .unwrap()
                .is_none()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn regions_split_at_negative_coordinates() {
        assert_eq!(region_of(CubePos::new(-1, 0, 7)), (-1, 0, 0));
        assert_eq!(region_of(CubePos::new(-8, -9, 8)), (-1, -2, 1));
        assert_ne!(
            index_of(CubePos::new(-1, 0, 0)),
            index_of(CubePos::new(0, 0, 0))
        );
    }
}

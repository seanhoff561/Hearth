//! Saving and loading the planet model (`planet.bin.zst` in the world folder).
//!
//! The format is a zstd-compressed little-endian stream. The tectonic layout is not stored: it
//! is regenerated from the seed, which is deterministic.

use std::io::{Read, Write};
use std::path::Path;

use hearth_math::Planet;

use super::grid::{Field, GridGeom};
use super::tectonics::TectonicLayout;
use super::{PlanetGrid, RiverCell, Volcano};
use crate::settings::{LAND_FRACTION, WorldGenSettings};

const MAGIC: &[u8; 4] = b"HPLN";
const VERSION: u32 = 3;

/// Errors reading or writing a planet file.
#[derive(Debug, thiserror::Error)]
pub enum PlanetIoError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a planet file")]
    BadMagic,
    #[error("unsupported planet file version {0}")]
    Version(u32),
    #[error("corrupt planet file: {0}")]
    Corrupt(String),
}

struct W(Vec<u8>);

impl W {
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn bytes(&mut self, b: &[u8]) {
        self.u32(b.len() as u32);
        self.0.extend_from_slice(b);
    }
    fn f32s(&mut self, v: &[f32]) {
        self.u32(v.len() as u32);
        // Byte planes compress much better than interleaved floats.
        for plane in 0..4 {
            self.0.extend(v.iter().map(|f| f.to_le_bytes()[plane]));
        }
    }
    fn u8s(&mut self, v: &[u8]) {
        self.bytes(v);
    }
    fn field(&mut self, f: &Field<f32>) {
        self.u32(f.n as u32);
        self.u32(f.scale as u32);
        self.f32s(&f.data);
    }
}

struct R<'a> {
    b: &'a [u8],
    pos: usize,
}

impl R<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], PlanetIoError> {
        if self.pos + n > self.b.len() {
            return Err(PlanetIoError::Corrupt("truncated".into()));
        }
        let s = &self.b[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32, PlanetIoError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn bytes(&mut self) -> Result<Vec<u8>, PlanetIoError> {
        let n = self.u32()? as usize;
        Ok(self.take(n)?.to_vec())
    }
    fn f32s(&mut self) -> Result<Vec<f32>, PlanetIoError> {
        let n = self.u32()? as usize;
        let raw = self.take(n * 4)?;
        let mut out = vec![0f32; n];
        for (i, o) in out.iter_mut().enumerate() {
            *o = f32::from_le_bytes([raw[i], raw[n + i], raw[2 * n + i], raw[3 * n + i]]);
        }
        Ok(out)
    }
    fn field(&mut self) -> Result<Field<f32>, PlanetIoError> {
        let n = self.u32()? as usize;
        let scale = self.u32()? as usize;
        let data = self.f32s()?;
        if data.len() != n * n || scale == 0 {
            return Err(PlanetIoError::Corrupt("field size".into()));
        }
        Ok(Field { n, data, scale })
    }
}

impl PlanetGrid {
    /// Serializes the planet (uncompressed bytes).
    fn to_bytes(&self) -> Vec<u8> {
        let mut w = W(Vec::with_capacity(self.geom.len() * 16));
        w.0.extend_from_slice(MAGIC);
        w.u32(VERSION);
        let settings = serde_json::to_vec(&self.settings).expect("settings serialize");
        w.bytes(&settings);
        w.u32(self.geom.n as u32);
        w.field(&self.elevation);
        w.field(&self.water);
        w.u8s(&self.flags);
        w.u8s(&self.plate);
        w.u8s(&self.province);
        w.u8s(&self.dry_season);
        w.u8s(&self.climate);
        w.u8s(&self.flow);
        w.f32s(&self.discharge);
        for f in [
            &self.uplift,
            &self.coast,
            &self.temperature,
            &self.sea_level_temperature,
            &self.temp_range,
            &self.precipitation,
            &self.winter_dry,
            &self.summer_dry,
            &self.current,
        ] {
            w.field(f);
        }
        let mut rivers: Vec<(&u32, &RiverCell)> = self.rivers.iter().collect();
        rivers.sort_by_key(|r| *r.0);
        w.u32(rivers.len() as u32);
        for (idx, r) in rivers {
            w.u32(*idx);
            w.u32(r.receiver);
            w.0.extend_from_slice(&r.discharge.to_le_bytes());
        }
        w.u32(self.volcanoes.len() as u32);
        for v in &self.volcanoes {
            w.0.extend_from_slice(&v.x.to_le_bytes());
            w.0.extend_from_slice(&v.z.to_le_bytes());
            for f in [v.summit, v.crater_radius, v.crater_depth] {
                w.0.extend_from_slice(&f.to_le_bytes());
            }
            w.0.push(u8::from(v.crater_lake));
        }
        w.u32(self.seed as u32);
        w.u32((self.seed >> 32) as u32);
        w.0
    }

    fn from_bytes(b: &[u8]) -> Result<Self, PlanetIoError> {
        let mut r = R { b, pos: 0 };
        if r.take(4)? != MAGIC {
            return Err(PlanetIoError::BadMagic);
        }
        let version = r.u32()?;
        if version != VERSION {
            return Err(PlanetIoError::Version(version));
        }
        let settings: WorldGenSettings = serde_json::from_slice(&r.bytes()?)
            .map_err(|e| PlanetIoError::Corrupt(format!("settings: {e}")))?;
        let n = r.u32()? as usize;
        let planet = Planet::from_size(settings.planet_size)
            .map_err(|e| PlanetIoError::Corrupt(e.to_string()))?;
        let geom = GridGeom::new(planet, n);
        let elevation = r.field()?;
        let water = r.field()?;
        let flags = r.bytes()?;
        let plate = r.bytes()?;
        let province = r.bytes()?;
        let dry_season = r.bytes()?;
        let climate = r.bytes()?;
        let flow = r.bytes()?;
        let discharge = r.f32s()?;
        for v in [&flags, &plate, &province, &dry_season, &climate, &flow] {
            if v.len() != n * n {
                return Err(PlanetIoError::Corrupt("cell array size".into()));
            }
        }
        if discharge.len() != n * n {
            return Err(PlanetIoError::Corrupt("discharge size".into()));
        }
        let mut halves = Vec::with_capacity(9);
        for _ in 0..9 {
            halves.push(r.field()?);
        }
        let count = r.u32()? as usize;
        let mut rivers = rustc_hash::FxHashMap::default();
        for _ in 0..count {
            let idx = r.u32()?;
            let receiver = r.u32()?;
            let d = r.take(4)?;
            rivers.insert(
                idx,
                RiverCell {
                    receiver,
                    discharge: f32::from_le_bytes([d[0], d[1], d[2], d[3]]),
                },
            );
        }
        let nv = r.u32()? as usize;
        let mut volcanoes = Vec::with_capacity(nv.min(100_000));
        for _ in 0..nv {
            let f64_of = |b: &[u8]| f64::from_le_bytes(b.try_into().expect("8 bytes"));
            let f32_of = |b: &[u8]| f32::from_le_bytes(b.try_into().expect("4 bytes"));
            let x = f64_of(r.take(8)?);
            let z = f64_of(r.take(8)?);
            let summit = f32_of(r.take(4)?);
            let crater_radius = f32_of(r.take(4)?);
            let crater_depth = f32_of(r.take(4)?);
            let crater_lake = r.take(1)?[0] != 0;
            volcanoes.push(Volcano {
                x,
                z,
                summit,
                crater_radius,
                crater_depth,
                crater_lake,
            });
        }
        let seed = r.u32()? as u64 | ((r.u32()? as u64) << 32);
        let layout = TectonicLayout::generate(seed, (LAND_FRACTION * 1.6).min(0.8));
        let mut h = halves.into_iter();
        let mut next = || h.next().expect("nine fields read above");
        Ok(PlanetGrid {
            geom,
            seed,
            vertical_scale: settings.vertical_scale(),
            settings,
            elevation,
            water,
            rivers,
            flow,
            discharge,
            flags,
            plate,
            province,
            uplift: next(),
            coast: next(),
            temperature: next(),
            sea_level_temperature: next(),
            temp_range: next(),
            precipitation: next(),
            dry_season,
            winter_dry: next(),
            summer_dry: next(),
            climate,
            current: next(),
            volcanoes,
            layout,
        })
    }

    /// Writes the planet to `path` (zstd-compressed, atomic via a temp file).
    pub fn save(&self, path: &Path) -> Result<(), PlanetIoError> {
        let raw = self.to_bytes();
        let tmp = path.with_extension("tmp");
        {
            let file = std::fs::File::create(&tmp)?;
            let mut enc = zstd::Encoder::new(std::io::BufWriter::new(file), 3)?;
            enc.write_all(&raw)?;
            enc.finish()?.flush()?;
        }
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Reads a planet written by [`Self::save`].
    pub fn load(path: &Path) -> Result<Self, PlanetIoError> {
        let file = std::fs::File::open(path)?;
        let mut dec = zstd::Decoder::new(std::io::BufReader::new(file))?;
        let mut raw = Vec::new();
        dec.read_to_end(&mut raw)?;
        Self::from_bytes(&raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hearth_math::PlanetSize;

    #[test]
    fn save_load_round_trip_is_lossless() {
        let s = WorldGenSettings {
            seed: 77,
            planet_size: PlanetSize::Small,
            grid_resolution: 128,
        };
        let g = PlanetGrid::build(&s, &|_, _| {});
        let dir = std::env::temp_dir().join(format!("hearth-planet-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("planet.bin.zst");
        g.save(&path).unwrap();
        let l = PlanetGrid::load(&path).unwrap();
        assert_eq!(l.seed, g.seed);
        assert_eq!(l.elevation.data, g.elevation.data);
        assert!(
            l.water
                .data
                .iter()
                .zip(&g.water.data)
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
        assert_eq!(l.precipitation.data, g.precipitation.data);
        assert_eq!(l.rivers.len(), g.rivers.len());
        assert_eq!(l.climate, g.climate);
        assert_eq!(l.volcanoes, g.volcanoes);
        assert_eq!(l.layout.plates.len(), g.layout.plates.len());
        std::fs::remove_dir_all(&dir).ok();
        assert!(matches!(
            PlanetGrid::from_bytes(b"nope"),
            Err(PlanetIoError::BadMagic)
        ));
    }
}

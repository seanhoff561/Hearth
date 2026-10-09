//! The distant terrain's tiles kept on disk (V2-6, docs/design/flora.md): each tile built is
//! written beside the world's planet cache, stamped with what it was built from, and read back
//! instead of built again while the stamp holds. The stamp is the build's fingerprint (two probe
//! tiles near the spawn built and hashed, so a change to generation, meshing or colours makes
//! every tile stale) with the vegetation and the player's changes that reach the tile. The
//! folder holds at most so much, the oldest tiles let go first (E4.1 §4.4).

use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use hearth_core::disk_cache::Folder;

use hearth_worldgen::WorldGenerator;
use rustc_hash::FxHasher;

use crate::{GroundVertex, LodGen, LodQuad, LodWorld, TileKey, TileMesh};

const MAGIC: &[u8; 4] = b"HLT4";

/// Tiles on disk.
pub struct TileCache {
    folder: Folder,
    fingerprint: OnceLock<u64>,
}

fn hash_mesh(h: &mut FxHasher, m: &TileMesh) {
    m.origin.hash(h);
    m.groups.hash(h);
    (m.min_y, m.max_y, m.error.to_bits()).hash(h);
    for q in &m.quads {
        (q.a, q.b, q.c, q.d).hash(h);
    }
    m.skirt.to_bits().hash(h);
    for g in m.ground.iter().chain(&m.canopy) {
        (g.y, g.n, g.c, g.m, g.s).hash(h);
    }
}

impl TileCache {
    /// The tiles kept in `dir`, at most `cap` bytes of them.
    pub fn new(dir: impl Into<PathBuf>, cap: u64) -> Self {
        Self {
            folder: Folder::new(dir, cap),
            fingerprint: OnceLock::new(),
        }
    }

    pub fn dir(&self) -> &Path {
        self.folder.dir()
    }

    /// What the folder holds (bytes).
    pub fn used(&self) -> u64 {
        self.folder.used()
    }

    /// What the build makes of two probe tiles near the spawn, hashed (once).
    pub fn fingerprint(&self, lod: &LodGen, wg: &WorldGenerator) -> u64 {
        *self.fingerprint.get_or_init(|| {
            let (sx, sz) = wg.terrain.find_spawn(false);
            let mut h = FxHasher::default();
            for level in [0u8, 4] {
                let size = crate::TILE << level;
                let key = TileKey {
                    level,
                    x: sx.div_euclid(size),
                    z: sz.div_euclid(size),
                };
                hash_mesh(&mut h, &lod.build(wg, key));
            }
            h.finish()
        })
    }

    /// The stamp of a tile: the fingerprint, and the vegetation and changes that reach it.
    pub fn stamp(&self, lod: &LodGen, wg: &WorldGenerator, world: &LodWorld, key: TileKey) -> u64 {
        let mut h = FxHasher::default();
        self.fingerprint(lod, wg).hash(&mut h);
        let (x0, z0) = key.min_block();
        let size = key.size();
        let half = size as f32 * 0.5;
        let (cx, cz) = (x0 + size / 2, z0 + size / 2);
        (world.veg.year.floor() as i64).hash(&mut h);
        for d in world.veg.disturbances() {
            let reach = d.radius * hearth_worldgen::vegetation::EDGE + half * 1.5 + 24.0;
            if world.veg.distance(d, cx, cz) <= reach {
                (d.kind as u8, d.year.to_bits(), d.x, d.z, d.radius.to_bits()).hash(&mut h);
                d.severity.to_bits().hash(&mut h);
                d.patches.len().hash(&mut h);
            }
        }
        let mut edits: Vec<(&(i32, i32), &(i32, hearth_world::BlockStateId))> = world
            .edits
            .iter()
            .filter(|((x, z), _)| (x0..x0 + size).contains(x) && (z0..z0 + size).contains(z))
            .collect();
        edits.sort_unstable_by_key(|(p, _)| **p);
        for (p, (y, s)) in edits {
            (p, y, s.0).hash(&mut h);
        }
        h.finish()
    }

    fn path(&self, key: TileKey) -> PathBuf {
        self.folder
            .dir()
            .join(format!("{}", key.level))
            .join(format!("{}_{}.lod", key.x, key.z))
    }

    /// The tile as kept, if it was kept with this stamp.
    pub fn load(&self, key: TileKey, stamp: u64) -> Option<TileMesh> {
        let raw = std::fs::read(self.path(key)).ok()?;
        let mut data = Vec::new();
        zstd::stream::read::Decoder::new(raw.as_slice())
            .ok()?
            .read_to_end(&mut data)
            .ok()?;
        let mut r = Reader { data: &data, at: 0 };
        if r.take(4)? != MAGIC || r.u64()? != stamp || r.u64()? != key.id() {
            return None;
        }
        let origin = [r.i32()?, r.i32()?];
        let mut groups = [0u32; 6];
        for g in &mut groups {
            *g = r.u32()?;
        }
        let (min_y, max_y) = (r.i32()?, r.i32()?);
        let error = f32::from_bits(r.u32()?);
        let n = r.u32()? as usize;
        let mut quads = Vec::with_capacity(n);
        for _ in 0..n {
            quads.push(LodQuad {
                a: r.u32()?,
                b: r.u32()?,
                c: r.u32()?,
                d: r.u32()?,
            });
        }
        let skirt = f32::from_bits(r.u32()?);
        let ground = r.field(crate::GROUND_SIDE * crate::GROUND_SIDE)?;
        let n = r.u32()? as usize;
        let canopy = r.field(n)?;
        Some(TileMesh {
            key,
            origin,
            ground,
            skirt,
            canopy,
            quads,
            groups,
            min_y,
            max_y,
            error,
        })
    }

    /// Keeps a tile with its stamp (quietly gives up if the disk will not have it).
    pub fn store(&self, mesh: &TileMesh, stamp: u64) {
        let path = self.path(mesh.key);
        let mut data = Vec::with_capacity(64 + (mesh.quads.len() + mesh.ground.len()) * 16);
        data.extend_from_slice(MAGIC);
        data.extend_from_slice(&stamp.to_le_bytes());
        data.extend_from_slice(&mesh.key.id().to_le_bytes());
        for v in mesh.origin {
            data.extend_from_slice(&v.to_le_bytes());
        }
        for g in mesh.groups {
            data.extend_from_slice(&g.to_le_bytes());
        }
        data.extend_from_slice(&mesh.min_y.to_le_bytes());
        data.extend_from_slice(&mesh.max_y.to_le_bytes());
        data.extend_from_slice(&mesh.error.to_bits().to_le_bytes());
        data.extend_from_slice(&(mesh.quads.len() as u32).to_le_bytes());
        for q in &mesh.quads {
            for v in [q.a, q.b, q.c, q.d] {
                data.extend_from_slice(&v.to_le_bytes());
            }
        }
        data.extend_from_slice(&mesh.skirt.to_bits().to_le_bytes());
        let field = |data: &mut Vec<u8>, vs: &[GroundVertex]| {
            for g in vs {
                data.extend_from_slice(&g.y.to_le_bytes());
                for v in [g.n, g.c, g.m, g.s] {
                    data.extend_from_slice(&v.to_le_bytes());
                }
            }
        };
        field(&mut data, &mesh.ground);
        data.extend_from_slice(&(mesh.canopy.len() as u32).to_le_bytes());
        field(&mut data, &mesh.canopy);
        let written = zstd::encode_all(data.as_slice(), 1)
            .and_then(|packed| self.folder.write(&path, &packed));
        if let Err(e) = written {
            log::debug!("LOD tile {:?} not cached: {e}", mesh.key);
        }
    }
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.data.get(self.at..self.at + n)?;
        self.at += n;
        Some(s)
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn field(&mut self, n: usize) -> Option<Vec<GroundVertex>> {
        (0..n)
            .map(|_| {
                Some(GroundVertex {
                    y: self.i32()?,
                    n: self.u32()?,
                    c: self.u32()?,
                    m: self.u32()?,
                    s: self.u32()?,
                })
            })
            .collect()
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hearth_worldgen::vegetation::{Disturbance, DisturbanceKind, Vegetation, VegetationSave};

    #[test]
    fn a_tile_is_kept_and_read_back_while_its_stamp_holds() {
        let (wg, reg, tex) = crate::tests::tiny_world();
        let lod = LodGen::new(&reg, &tex);
        let (x, z) = crate::tests::forest(&wg);
        // A level whose crowns are a canopy surface.
        let key = TileKey {
            level: 3,
            x: x.div_euclid(crate::TILE << 3),
            z: z.div_euclid(crate::TILE << 3),
        };
        let dir = std::env::temp_dir().join(format!("hearth-lodcache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cache = TileCache::new(&dir, 1 << 30);
        let world = LodWorld::default();
        let stamp = cache.stamp(&lod, &wg, &world, key);
        assert!(cache.load(key, stamp).is_none(), "nothing kept yet");
        let built = lod.build_in(&wg, &world, key);
        cache.store(&built, stamp);
        let back = cache.load(key, stamp).expect("kept");
        assert_eq!(back.quads.len(), built.quads.len());
        assert!(
            back.quads
                .iter()
                .zip(&built.quads)
                .all(|(a, b)| { (a.a, a.b, a.c, a.d) == (b.a, b.b, b.c, b.d) })
        );
        assert_eq!((back.origin, back.groups), (built.origin, built.groups));
        assert_eq!((back.ground, back.skirt), (built.ground, built.skirt));
        assert!(!built.canopy.is_empty(), "a forest's canopy");
        assert_eq!(back.canopy, built.canopy);
        assert!(cache.load(key, stamp ^ 1).is_none(), "another stamp");
        // A fire that burned the tile changes its stamp; one far off does not.
        let burn = |x: i32, z: i32| Disturbance {
            kind: DisturbanceKind::Burned,
            year: 0.5,
            x,
            z,
            radius: 30.0,
            severity: 1.0,
            patches: Vec::new(),
        };
        let c = wg.planet().circumference();
        let near = LodWorld {
            veg: Vegetation::new(
                &VegetationSave {
                    disturbances: vec![burn(x, z)],
                },
                c,
                0.9,
            ),
            edits: Default::default(),
        };
        let far = LodWorld {
            veg: Vegetation::new(
                &VegetationSave {
                    disturbances: vec![burn(x + 3000, z)],
                },
                c,
                0.9,
            ),
            edits: Default::default(),
        };
        assert_ne!(cache.stamp(&lod, &wg, &near, key), stamp);
        assert_eq!(cache.stamp(&lod, &wg, &far, key), stamp);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

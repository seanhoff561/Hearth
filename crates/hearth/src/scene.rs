//! A locally generated world around a point: planet → generator → cubes → light → meshes.
//! Used by the headless screenshot mode and the free-flying preview camera until the
//! client/server split takes over world streaming.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use glam::DVec3;
use hearth_math::{CUBE_AREA, ColumnPos, CubePos, PlanetSize};
use hearth_render::mesh::{ColumnTints, CubeMesh, MeshInput, MeshOptions, Mesher};
use hearth_render::models::BlockModels;
use hearth_world::{BlockRegistry, Cube, CubeMap, LightEngine};
use hearth_worldgen::cubegen::MAX_FEATURE_HEIGHT;
use hearth_worldgen::region::biome::{foliage_color, grass_color, water_color};
use hearth_worldgen::{PlanetGrid, Terrain, WorldGenSettings, WorldGenerator};
use rayon::prelude::*;

/// A world generated directly in-process.
pub struct LocalWorld {
    pub reg: Arc<BlockRegistry>,
    pub generator: Arc<WorldGenerator>,
    pub map: CubeMap,
    pub light: LightEngine,
}

impl LocalWorld {
    /// Builds (or loads from `cache_dir`) the planet for `seed` and prepares a generator.
    pub fn create(
        seed: u64,
        size: PlanetSize,
        resolution: usize,
        cache_dir: Option<&Path>,
    ) -> anyhow::Result<Self> {
        let defs = hearth_world::datapack::load_block_defs(&[data_pack_dir()])?;
        let reg = Arc::new(BlockRegistry::build(defs)?);
        let settings = WorldGenSettings {
            seed,
            planet_size: size,
            grid_resolution: resolution,
            ..WorldGenSettings::default()
        }
        .sanitized();
        let t0 = Instant::now();
        let cache = cache_dir.map(|d| {
            d.join(format!(
                "planet_{seed}_{}_{}.bin.zst",
                size.name(),
                settings.grid_resolution
            ))
        });
        let grid = match cache.as_ref().filter(|p| p.exists()) {
            Some(p) => match PlanetGrid::load(p) {
                Ok(g) => g,
                Err(e) => {
                    log::warn!("planet cache unreadable ({e}); rebuilding");
                    PlanetGrid::build(&settings, &|_, _| {})
                }
            },
            None => {
                let g = PlanetGrid::build(&settings, &|f, stage| {
                    log::debug!("planet {:.0}% {stage}", f * 100.0);
                });
                if let Some(p) = &cache {
                    if let Some(d) = p.parent() {
                        std::fs::create_dir_all(d).ok();
                    }
                    if let Err(e) = g.save(p) {
                        log::warn!("could not cache planet: {e}");
                    }
                }
                g
            }
        };
        log::info!("planet ready in {:.2}s", t0.elapsed().as_secs_f64());
        let terrain = Arc::new(Terrain::new(Arc::new(grid)));
        let generator = Arc::new(WorldGenerator::new(terrain.clone(), &reg)?);
        Ok(Self {
            map: CubeMap::new(*terrain.planet()),
            reg,
            generator,
            light: LightEngine::new(),
        })
    }

    pub fn terrain(&self) -> &Terrain {
        &self.generator.terrain
    }

    /// Generates, inserts and lights every cube of the columns within `radius` cubes of
    /// `center`, over one vertical range spanning the lowest surface to above the tallest trees
    /// (and the camera). Returns the cubes whose six neighbours are all loaded, i.e. the ones
    /// that can be meshed without guessing at missing neighbours.
    pub fn load_area(&mut self, center: DVec3, radius: i32, extra_above: i32) -> Vec<CubePos> {
        let t0 = Instant::now();
        let c = CubePos::containing(center);
        let mut columns = Vec::new();
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                columns.push(ColumnPos::new(c.x + dx, c.z + dz));
            }
        }
        let generator = self.generator.clone();
        let data: Vec<_> = columns
            .par_iter()
            .map(|col| generator.column(*col))
            .collect();
        let lo = data
            .iter()
            .map(|d| d.h_min >> 4)
            .min()
            .unwrap_or(0)
            .min(c.y)
            - 2;
        let hi = data
            .iter()
            .map(|d| ((d.h_max + MAX_FEATURE_HEIGHT) >> 4).max(d.water_max >> 4))
            .max()
            .unwrap_or(0)
            .max(c.y)
            + 1
            + extra_above;
        let cubes: Vec<(CubePos, Cube)> = columns
            .par_iter()
            .flat_map_iter(|col| {
                let generator = &generator;
                (lo..=hi).map(move |cy| {
                    let p = col.cube(cy);
                    (p, generator.generate_cube(p))
                })
            })
            .collect();
        let gen_time = t0.elapsed().as_secs_f64();
        let mut positions = Vec::with_capacity(cubes.len());
        for (d, col) in data.iter().zip(&columns) {
            self.map.ensure_column(*col, || {
                let mut est = [0i32; CUBE_AREA];
                for (e, s) in est.iter_mut().zip(&d.samples) {
                    *e = s.height_i().max(s.water_i()) - 1;
                }
                est
            });
        }
        for (p, cube) in cubes {
            self.map.insert_cube(p, Arc::new(cube), &self.reg);
            positions.push(p);
        }
        let t1 = Instant::now();
        self.light
            .light_new_cubes(&mut self.map, &self.reg, &positions);
        log::info!(
            "generated {} cubes (y {}..={}) in {:.2}s, lit in {:.2}s",
            positions.len(),
            lo,
            hi,
            gen_time,
            t1.elapsed().as_secs_f64()
        );
        positions.retain(|p| {
            (p.x - c.x).abs() < radius && (p.z - c.z).abs() < radius && p.y > lo && p.y < hi
        });
        positions
    }

    /// Climate tints of a column.
    pub fn tints(&self, col: ColumnPos) -> ColumnTints {
        let data = self.generator.column(col);
        let mut t = ColumnTints::default();
        for (i, s) in data.samples.iter().enumerate() {
            t.grass[i] = grass_color(s.temperature, s.precipitation);
            t.foliage[i] = foliage_color(s.temperature, s.precipitation);
            let depth = s.water_i().saturating_sub(s.height_i()).max(0) as f32;
            t.water[i] = water_color(s.sea_temperature, depth);
            let g = t.grass[i];
            t.dry_grass[i] = [
                ((g[0] as u16 + 200) / 2) as u8,
                ((g[1] as u16 + 180) / 2) as u8,
                ((g[2] as u16 + 90) / 2) as u8,
            ];
        }
        t
    }

    /// Meshes cubes in parallel.
    pub fn mesh(
        &self,
        models: &BlockModels,
        positions: &[CubePos],
        opts: MeshOptions,
    ) -> Vec<CubeMesh> {
        let t0 = Instant::now();
        let mesher = Mesher {
            reg: &self.reg,
            models,
            opts,
        };
        let meshes: Vec<CubeMesh> = positions
            .par_iter()
            .map(|p| mesher.mesh(&MeshInput::gather(&self.map, *p, self.tints(p.column()))))
            .collect();
        let quads: usize = meshes
            .iter()
            .map(|m| m.quads.len() + m.models.len() + m.translucent.len())
            .sum();
        log::debug!(
            "meshed {} cubes ({quads} quads) in {:.2}s",
            meshes.len(),
            t0.elapsed().as_secs_f64()
        );
        meshes
    }

    /// Height of the terrain surface at (x, z).
    pub fn surface_y(&self, x: f64, z: f64) -> f64 {
        let s = self.terrain().sample(x.floor() as i32, z.floor() as i32);
        s.height.max(s.water) as f64
    }
}

/// The repository's (or installed) base data pack directory.
pub fn data_pack_dir() -> std::path::PathBuf {
    // Installed layout: `data/` next to the executable; development: the repository root.
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        for dir in [exe_dir.join("data"), exe_dir.join("../../data")] {
            if dir.join("hearth").is_dir() {
                return dir;
            }
        }
    }
    hearth_world::datapack::builtin_pack_dir()
}

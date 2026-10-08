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
use hearth_worldgen::region::biome::water_color;
use hearth_worldgen::{PlanetGrid, Terrain, WorldGenSettings, WorldGenerator};
use rayon::prelude::*;

/// A world generated directly in-process.
pub struct LocalWorld {
    pub reg: Arc<BlockRegistry>,
    pub generator: Arc<WorldGenerator>,
    pub map: CubeMap,
    pub light: LightEngine,
    /// Seasonal snow, ice and river levels on the loaded terrain.
    pub cover: crate::season_cover::SeasonCover,
    /// The game data the world was built from.
    pub content: Arc<hearth_content::Content>,
    /// The blocks the player has changed, laid over the terrain as it loads.
    pub edits: crate::edits::Edits,
    /// The vegetation the terrain is grown with: the year the trees have grown to, and what
    /// has been felled, cleared and burned.
    pub vegetation: hearth_worldgen::vegetation::Vegetation,
    /// The smooth ground's materials (Amendment S): natural ground is meshed through its fill.
    pub ground: Arc<hearth_render::smooth::GroundMaterials>,
}

impl LocalWorld {
    /// Builds (or loads from `cache_dir`) the planet for `seed` and prepares a generator.
    pub fn create(
        seed: u64,
        size: PlanetSize,
        resolution: usize,
        cache_dir: Option<&Path>,
    ) -> anyhow::Result<Self> {
        let settings = WorldGenSettings {
            seed,
            planet_size: size,
            grid_resolution: resolution,
        }
        .sanitized();
        Self::create_with(&settings, cache_dir, &|f, stage| {
            log::debug!("planet {:.0}% {stage}", f * 100.0);
        })
    }

    /// Builds (or loads from `cache_dir`) the planet of `settings`, telling `progress` how far it
    /// has come (a share and the stage's words), and prepares a generator.
    pub fn create_with(
        settings: &WorldGenSettings,
        cache_dir: Option<&Path>,
        progress: hearth_worldgen::planet::Progress<'_>,
    ) -> anyhow::Result<Self> {
        let (content, report) = hearth_content::Content::load(&[data_pack_dir()]);
        let content = Arc::new(content.ok_or_else(|| {
            let msgs: Vec<String> = report.sorted().iter().map(|d| d.to_string()).collect();
            anyhow::anyhow!(
                "game data failed to load:
{}",
                msgs.join(
                    "
"
                )
            )
        })?);
        let defs = hearth_world::datapack::load_block_defs(&[data_pack_dir()])?;
        let reg = Arc::new(BlockRegistry::build(defs)?);
        let t0 = Instant::now();
        let cache = cache_dir.map(|d| d.join(planet_cache_name(settings)));
        let cached =
            cache
                .as_ref()
                .filter(|p| p.exists())
                .and_then(|p| match PlanetGrid::load(p) {
                    Ok(g) => Some(g),
                    Err(e) => {
                        log::warn!("planet cache unreadable ({e}); rebuilding");
                        None
                    }
                });
        let grid = match cached {
            Some(g) => g,
            // Built afresh, and cached (over an unreadable file, an older format's).
            None => {
                let g = PlanetGrid::build(settings, progress);
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
        let cover = crate::season_cover::SeasonCover::new(&reg, &grid)?;
        let terrain = Arc::new(Terrain::new(Arc::new(grid)));
        let generator = Arc::new(WorldGenerator::new(terrain.clone(), &reg, &content)?);
        let vegetation = hearth_worldgen::vegetation::Vegetation::new(
            &Default::default(),
            terrain.planet().circumference(),
            0.0,
        );
        let ground = Arc::new(ground_materials(&reg, &content).0);
        Ok(Self {
            map: CubeMap::new(*terrain.planet()),
            cover,
            content,
            reg,
            generator,
            light: LightEngine::new(),
            edits: crate::edits::Edits::default(),
            vegetation,
            ground,
        })
    }

    /// The planet model (for the environment sampler).
    pub fn grid(&self) -> Arc<hearth_worldgen::PlanetGrid> {
        self.generator.terrain.grid.clone()
    }

    pub fn terrain(&self) -> &Terrain {
        &self.generator.terrain
    }

    /// Generates, inserts and lights every cube of the columns within `radius` cubes of
    /// `center`, over one vertical range spanning the lowest surface to above the tallest trees
    /// (and the camera). Returns the cubes whose six neighbours are all loaded, i.e. the ones
    /// that can be meshed without guessing at missing neighbours.
    /// With `year_frac`, the seasonal snow and ice of that date are laid on before lighting.
    pub fn load_area(
        &mut self,
        center: DVec3,
        radius: i32,
        extra_above: i32,
        year_frac: Option<f64>,
    ) -> Vec<CubePos> {
        let t0 = Instant::now();
        let c = CubePos::containing(center);
        let mut columns = Vec::new();
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                columns.push(ColumnPos::new(c.x + dx, c.z + dz));
            }
        }
        let generator = self.generator.clone();
        let veg = self.vegetation.clone();
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
                let (generator, veg) = (&generator, &veg);
                (lo..=hi).map(move |cy| {
                    let p = col.cube(cy);
                    (p, generator.generate_cube_in(p, veg).0)
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
            // The player's changes, as the server lays them over the terrain.
            self.edits.restore(&mut self.map, &self.reg, p);
            positions.push(p);
        }
        if let Some(yf) = year_frac {
            let changed = self
                .cover
                .apply(&mut self.map, &self.reg, &self.generator, &columns, yf);
            log::debug!("seasonal cover changed {changed} blocks");
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

    /// Tint inputs of a column: climate codes for seasonal vegetation colours, baked water.
    pub fn tints(&self, col: ColumnPos) -> ColumnTints {
        use hearth_env::tint::{DryType, encode};
        use hearth_worldgen::planet::climate::ClimateClass as C;
        let data = self.generator.column(col);
        let (x0, z0) = col.min_block_xz();
        let southern = self.map.planet().latitude(z0 as f64 + 8.0) < 0.0;
        // Dry-season strengths vary smoothly: one sample per column.
        let normals = hearth_env::climate::Normals::sample(
            &self.generator.terrain.grid,
            x0 as f64 + 8.0,
            z0 as f64 + 8.0,
        );
        let mut t = ColumnTints::default();
        for (i, s) in data.samples.iter().enumerate() {
            let arid = matches!(
                s.climate,
                C::HotDesert | C::HotSteppe | C::ColdDesert | C::ColdSteppe
            );
            let dry = DryType::of(&normals, arid);
            let range = (2.0 * (s.t_warm - s.temperature)).max(0.0) as f64;
            t.climate[i] = encode(
                s.temperature as f64,
                range,
                s.precipitation as f64,
                dry,
                southern,
            );
            let depth = s.water_i().saturating_sub(s.height_i()).max(0) as f32;
            t.water[i] = water_color(s.sea_temperature, depth);
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
            .map(|p| {
                mesher.mesh(
                    &MeshInput::gather(&self.map, *p, self.tints(p.column())).with_ground(
                        &self.map,
                        &self.reg,
                        &self.ground,
                    ),
                )
            })
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

/// The smooth ground's materials (Amendment S §4): the registry's natural blocks as slots, their
/// sharpness from their ground families, and how the shader colours each: its material's two
/// colours and grain (the block's map colour where it names no material), grass tinted by
/// place and season.
pub fn ground_materials(
    reg: &BlockRegistry,
    content: &hearth_content::Content,
) -> (
    hearth_render::smooth::GroundMaterials,
    Vec<hearth_render::terrain::GroundMaterial>,
) {
    use hearth_content::schema::material::Pattern;
    let soil = content.reference.ground.iter().find(|g| g.id == "soil");
    let sharpness = |m: Option<&str>| {
        m.and_then(|m| content.ground_of(m))
            .or(soil)
            .map_or(0.2, |g| g.sharpness)
    };
    let slots = hearth_render::smooth::GroundMaterials::new(reg, &sharpness);
    let lin = |c: [u8; 3]| {
        let f = |v: u8| {
            let c = v as f32 / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        [f(c[0]), f(c[1]), f(c[2])]
    };
    let hex = |s: &str| -> [u8; 3] {
        let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0x707070);
        [(v >> 16) as u8, (v >> 8) as u8, v as u8]
    };
    let params = slots
        .slots
        .iter()
        .map(|slot| {
            let block = reg
                .blocks()
                .find(|b| b.name.to_string() == slot.block)
                .map(|b| b.def.clone())
                .unwrap_or_default();
            let mat = slot
                .material
                .as_deref()
                .and_then(|m| content.materials.get(m));
            let (c1, c2, pattern, rough) = match mat {
                Some(m) => (
                    m.appearance.color.0,
                    m.appearance.color2.map(|c| c.0),
                    m.appearance.pattern,
                    m.appearance.roughness.unwrap_or(0.9),
                ),
                None => (hex(&block.map_color), None, Pattern::Grainy, 0.9),
            };
            let a = lin(c1);
            let b = c2.map(lin).unwrap_or([a[0] * 0.7, a[1] * 0.7, a[2] * 0.7]);
            // The grain (m) and how far it stands up when materials meet.
            let (grain, relief) = match pattern {
                Pattern::Powder => (0.03, 0.1),
                Pattern::Grainy | Pattern::Speckled => (0.06, 0.25),
                Pattern::Clumpy => (0.25, 0.5),
                Pattern::Layered | Pattern::Banded => (0.4, 0.4),
                Pattern::Crystalline | Pattern::Veined => (0.15, 0.6),
                Pattern::Porous => (0.12, 0.45),
                Pattern::Fibrous => (0.08, 0.3),
                _ => (0.3, 0.35),
            };
            let grass = slot.block.ends_with("grass_block");
            hearth_render::terrain::GroundMaterial {
                color: [a[0], a[1], a[2], rough],
                color2: [b[0], b[1], b[2], grain],
                tint: grass as u32,
                relief,
                _pad: [0.0; 2],
            }
        })
        .collect();
    (slots, params)
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

/// The planet cache's file for world-generation settings: its seed, size and resolution.
pub fn planet_cache_name(settings: &WorldGenSettings) -> String {
    format!(
        "planet_{}_{}_{}.bin.zst",
        settings.seed,
        settings.planet_size.name(),
        settings.grid_resolution
    )
}

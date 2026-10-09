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
        Self::create_with(&settings, cache_dir, None, &|f, stage| {
            log::debug!("planet {:.0}% {stage}", f * 100.0);
        })
    }

    /// Builds (or loads, [`planet_for`]: a saved world's `own`, else from `cache_dir`) the planet
    /// of `settings`, telling `progress` how far it has come (a share and the stage's words), and
    /// prepares a generator.
    pub fn create_with(
        settings: &WorldGenSettings,
        cache_dir: Option<&Path>,
        own: Option<&Path>,
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
        let grid = planet_for(settings, cache_dir, own, progress);
        log::info!("planet ready in {:.2}s", t0.elapsed().as_secs_f64());
        let terrain = Arc::new(Terrain::new(Arc::new(grid)));
        if let Some(d) = cache_dir {
            keep_relief(&terrain, d);
        }
        let generator = Arc::new(WorldGenerator::new(terrain, &reg, &content)?);
        Self::from_generator(generator, content, reg)
    }

    /// A world from a generator made already (the menus', E4.1 §4.4), its game data and blocks.
    pub fn from_generator(
        generator: Arc<WorldGenerator>,
        content: Arc<hearth_content::Content>,
        reg: Arc<BlockRegistry>,
    ) -> anyhow::Result<Self> {
        let terrain = &generator.terrain;
        let cover = crate::season_cover::SeasonCover::new(&reg, &terrain.grid)?;
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
            // Under the grass, the soil that shows where it is thin.
            let b = match (grass, content.materials.get("hearth:loam")) {
                (true, Some(loam)) => lin(loam.appearance.color.0),
                _ => b,
            };
            // Bedded rock shows its beds (S §4.2): sandstone and shale thin, limestone thicker.
            let strata = match pattern {
                Pattern::Layered => 0.25,
                Pattern::Banded => 0.12,
                _ => 0.0,
            };
            hearth_render::terrain::GroundMaterial {
                color: [a[0], a[1], a[2], rough],
                color2: [b[0], b[1], b[2], grain],
                tint: grass as u32,
                relief,
                strata,
                _pad: 0.0,
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

/// The most the refinement levels' tiles kept on disk take, every planet's together (bytes).
pub const RELIEF_CACHE_CAP: u64 = 512 << 20;

/// Keeps a planet's refinement tiles in the cache folder, to be read back rather than made
/// again (E4.1 §4.4).
pub fn keep_relief(terrain: &Terrain, cache_dir: &Path) {
    terrain.keep_tiles_on_disk(&cache_dir.join("relief"), RELIEF_CACHE_CAP);
}

/// The file the globe's map of a planet is kept in, beside the planet's cache (E4.1 §4.2):
/// named for the planet's fingerprint, so that no other planet of the seed is shown with it.
pub fn globe_cache_name(grid: &PlanetGrid) -> String {
    format!("globe_{:016x}.bin.zst", grid.fingerprint())
}

/// The planet cache's file for world-generation settings: its seed, size and resolution, and
/// the generator's version, so that no new world is made on a planet an older build cached.
pub fn planet_cache_name(settings: &WorldGenSettings) -> String {
    format!(
        "{}_g{}.bin.zst",
        planet_cache_stem(settings),
        hearth_worldgen::planet::GENERATOR
    )
}

/// The name planets were cached under before the generator's version was part of it: the
/// planet a world saved then has been opened on.
fn legacy_planet_cache_name(settings: &WorldGenSettings) -> String {
    format!("{}.bin.zst", planet_cache_stem(settings))
}

fn planet_cache_stem(settings: &WorldGenSettings) -> String {
    format!(
        "planet_{}_{}_{}",
        settings.seed,
        settings.planet_size.name(),
        settings.grid_resolution
    )
}

/// The planet of `settings`: a saved world's own (`own`, in its folder) where it has one; else,
/// for a world saved before worlds kept theirs, the planet it has been opened on (the cache's
/// older name); else this build's, from the cache or made afresh (`progress` told) and cached.
/// A saved world without its own keeps in its folder the planet it is given.
pub fn planet_for(
    settings: &WorldGenSettings,
    cache_dir: Option<&Path>,
    own: Option<&Path>,
    progress: hearth_worldgen::planet::Progress<'_>,
) -> PlanetGrid {
    let settings = settings.clone().sanitized();
    let read = |p: &Path| -> Option<PlanetGrid> {
        if !p.exists() {
            return None;
        }
        match PlanetGrid::load(p) {
            Ok(g) if g.settings == settings => Some(g),
            Ok(_) => {
                log::warn!("{} holds another planet; not used", p.display());
                None
            }
            Err(e) => {
                log::warn!("planet file {} unreadable ({e}); not used", p.display());
                None
            }
        }
    };
    if let Some(g) = own.and_then(read) {
        return g;
    }
    let cache = cache_dir.map(|d| d.join(planet_cache_name(&settings)));
    let legacy = own
        .and(cache_dir)
        .map(|d| d.join(legacy_planet_cache_name(&settings)));
    let grid = legacy
        .as_deref()
        .and_then(read)
        .or_else(|| cache.as_deref().and_then(read))
        .unwrap_or_else(|| {
            let g = PlanetGrid::build(&settings, progress);
            if let Some(p) = &cache {
                cache_planet(&g, p);
            }
            g
        });
    if let Some(p) = own
        && let Err(e) = grid.save(p)
    {
        log::warn!("could not keep the world's planet: {e}");
    }
    grid
}

/// Caches a planet made afresh at `path`, letting go of the planets of its settings that other
/// generators made (the worlds made on them keep their own).
fn cache_planet(grid: &PlanetGrid, path: &Path) {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).ok();
    }
    if let Err(e) = grid.save(path) {
        log::warn!("could not cache planet: {e}");
        return;
    }
    let others = format!("{}_g", planet_cache_stem(&grid.settings));
    let Some(dir) = path.parent().and_then(|d| std::fs::read_dir(d).ok()) else {
        return;
    };
    for e in dir.flatten() {
        let name = e.file_name();
        if Some(name.as_os_str()) != path.file_name() && name.to_string_lossy().starts_with(&others)
        {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny(seed: u64) -> WorldGenSettings {
        WorldGenSettings {
            seed,
            planet_size: PlanetSize::Tiny,
            grid_resolution: 64,
        }
        .sanitized()
    }

    fn made(s: &WorldGenSettings, cache: Option<&Path>, own: Option<&Path>) -> u64 {
        planet_for(s, cache, own, &|_, _| {}).content_hash()
    }

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("hearth-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_world_keeps_its_planet_and_a_new_world_gets_this_builds() {
        let dir = temp("planet-for");
        let cache = dir.join("cache");
        let s = tiny(5);
        let fresh = PlanetGrid::build(&s, &|_, _| {}).content_hash();
        // A new world: this build's planet, cached under the generator's name.
        assert_eq!(made(&s, Some(&cache), None), fresh);
        assert!(cache.join(planet_cache_name(&s)).exists());
        // A planet of the seed an older build cached is not used for a new world...
        let mut older = PlanetGrid::build(&s, &|_, _| {});
        older.elevation.data[0] += 100.0;
        let old = older.content_hash();
        older
            .save(&cache.join(legacy_planet_cache_name(&s)))
            .unwrap();
        assert_eq!(made(&s, Some(&cache), None), fresh);
        // ...but a world saved then, with no planet of its own, keeps the one it was opened on.
        let world = dir.join("world");
        std::fs::create_dir_all(&world).unwrap();
        let own = world.join(hearth_save::PLANET_FILE);
        assert_eq!(made(&s, Some(&cache), Some(&own)), old);
        assert!(own.exists(), "kept in the world's folder");
        // The world's own planet, whatever the cache holds or after it is emptied.
        std::fs::remove_dir_all(&cache).unwrap();
        assert_eq!(made(&s, Some(&cache), Some(&own)), old);
        // A world saved before, without a planet and no older one cached: this build's.
        let other = dir.join("other");
        std::fs::create_dir_all(&other).unwrap();
        assert_eq!(
            made(
                &s,
                Some(&cache),
                Some(&other.join(hearth_save::PLANET_FILE))
            ),
            fresh
        );
        // A file of another planet is not taken for the world's.
        PlanetGrid::build(&tiny(6), &|_, _| {}).save(&own).unwrap();
        assert_eq!(made(&s, Some(&cache), Some(&own)), fresh);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn caching_a_planet_lets_go_of_what_other_generators_made_of_it() {
        let cache = temp("planet-cache");
        std::fs::create_dir_all(&cache).unwrap();
        let s = tiny(5);
        let stem = planet_cache_stem(&s);
        let other_generator = cache.join(format!("{stem}_g0.bin.zst"));
        let other_seed = cache.join(format!("{}_g0.bin.zst", planet_cache_stem(&tiny(55))));
        let legacy = cache.join(legacy_planet_cache_name(&s));
        for p in [&other_generator, &other_seed, &legacy] {
            std::fs::write(p, b"kept by an older build").unwrap();
        }
        made(&s, Some(&cache), None);
        assert!(cache.join(planet_cache_name(&s)).exists());
        assert!(
            !other_generator.exists(),
            "an older generator's planet of it let go"
        );
        assert!(other_seed.exists(), "another seed's kept");
        assert!(
            legacy.exists(),
            "the older name's kept, for the worlds saved on it"
        );
        let _ = std::fs::remove_dir_all(&cache);
    }
}

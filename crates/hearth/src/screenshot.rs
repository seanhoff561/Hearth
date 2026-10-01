//! Headless screenshot mode: `hearth --screenshot "seed=1,season=autumn,hour=16,out=shot.png"`
//! renders fixed camera shots to PNG without a window (a software adapter is used if no GPU
//! exists). Shots can pick a latitude, date and time, so seasonal suites are one list file.
//! Every shot waits for its distant (LOD) terrain to be built before rendering, and fails if
//! that takes longer than `lod_timeout` seconds.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use glam::DVec3;
use hearth_content::schema::Season;
use hearth_content::schema::config::TimeConfig;
use hearth_env::{Calendar, Precip};
use hearth_math::PlanetSize;
use hearth_render::GpuContext;
use hearth_render::atlas::TextureArray;
use hearth_render::camera::Camera;
use hearth_render::mesh::MeshOptions;
use hearth_render::models::BlockModels;
use hearth_render::offscreen::{OFFSCREEN_FORMAT, OffscreenTarget, write_png};
use hearth_render::precip::SkyHeights;
use hearth_render::scene::SceneRenderer;

use crate::environment::{EnvOverrides, EnvSampler};
use crate::scene::LocalWorld;
use rayon::prelude::*;

/// One screenshot request.
#[derive(Debug, Clone, PartialEq)]
pub struct ShotSpec {
    pub seed: u64,
    pub planet: PlanetSize,
    pub resolution: usize,
    /// Camera position; `None` → spawn (x, z) or a land spot at `lat`, above the surface (y).
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub z: Option<f64>,
    /// Pick a land spot near this latitude (degrees) when x/z aren't given.
    pub lat: Option<f64>,
    /// Height above the surface when `y` is not given.
    pub above: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
    pub width: u32,
    pub height: u32,
    /// Horizontal load radius in cubes.
    pub distance: i32,
    /// Season (mid-season in the place's hemisphere) or an explicit year fraction.
    pub season: Option<Season>,
    pub year_frac: Option<f64>,
    /// Local solar hour (0–24).
    pub hour: f64,
    /// Cloud cover override (0–1); `None` = the weather's.
    pub clouds: Option<f64>,
    /// Lay the date's snow and ice.
    pub snow: bool,
    /// Precipitation override: type and rate (mm/h of water); `None` = the weather's.
    pub precipitation: Option<(Precip, f64)>,
    /// Output file; `None` → a numbered file in the game's screenshot folder.
    pub out: Option<PathBuf>,
    pub software: bool,
    /// Also render with CPU culling and fail if GPU occlusion culling changes any pixel.
    pub verify_cull: bool,
    /// Distant (LOD) terrain radius in chunks; 0 = none.
    pub lod: u32,
    /// Aerial perspective (haze and the blue of distance); off only for comparisons.
    pub fog: bool,
    /// Seconds the LOD terrain may take to build before the shot fails.
    pub lod_timeout: f64,
    /// Draw the planet as the globe (at this zoom) centred on the camera's place instead.
    pub globe: Option<f32>,
    /// A person standing on the ground this far (m) in front of the camera, facing it.
    pub person: Option<f64>,
    /// See through the eyes of a person standing on the ground below the camera (their body
    /// drawn as in first person).
    pub body: bool,
    /// The body's senses on the image: `hurt`, `cold`, `hot`, `exhausted` or `faint`.
    pub senses: Option<String>,
}

impl Default for ShotSpec {
    fn default() -> Self {
        Self {
            seed: 1,
            planet: PlanetSize::Standard,
            resolution: 0,
            x: None,
            y: None,
            z: None,
            lat: None,
            above: 24.0,
            yaw: 30.0,
            pitch: 15.0,
            fov: 70.0,
            width: 1280,
            height: 720,
            distance: 10,
            season: None,
            year_frac: None,
            hour: 11.0,
            clouds: None,
            snow: true,
            precipitation: None,
            out: None,
            software: false,
            verify_cull: false,
            lod: 256,
            fog: true,
            lod_timeout: 180.0,
            globe: None,
            person: None,
            body: false,
            senses: None,
        }
    }
}

fn parse_season(v: &str) -> anyhow::Result<Season> {
    Ok(match v {
        "spring" => Season::Spring,
        "summer" => Season::Summer,
        "autumn" | "fall" => Season::Autumn,
        "winter" => Season::Winter,
        other => anyhow::bail!("unknown season {other:?}"),
    })
}

impl ShotSpec {
    /// Parses `key=value` pairs separated by commas.
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let mut spec = Self::default();
        for kv in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (k, v) = kv
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("expected key=value, got {kv:?}"))?;
            let v = v.trim();
            match k.trim() {
                "seed" => spec.seed = v.parse()?,
                "planet" => {
                    spec.planet = PlanetSize::from_name(v)
                        .ok_or_else(|| anyhow::anyhow!("unknown planet {v}"))?
                }
                "res" => spec.resolution = v.parse()?,
                "x" => spec.x = Some(v.parse()?),
                "y" => spec.y = Some(v.parse()?),
                "z" => spec.z = Some(v.parse()?),
                "lat" => spec.lat = Some(v.parse()?),
                "above" => spec.above = v.parse()?,
                "yaw" => spec.yaw = v.parse()?,
                "pitch" => spec.pitch = v.parse()?,
                "fov" => spec.fov = v.parse()?,
                "w" | "width" => spec.width = v.parse()?,
                "h" | "height" => spec.height = v.parse()?,
                "dist" | "distance" => spec.distance = v.parse()?,
                "season" => spec.season = Some(parse_season(v)?),
                "yf" => spec.year_frac = Some(v.parse()?),
                "hour" => spec.hour = v.parse()?,
                "clouds" => spec.clouds = Some(v.parse()?),
                "snow" => spec.snow = v.parse()?,
                "rain" => spec.precipitation = Some((Precip::Rain, v.parse()?)),
                "sleet" => spec.precipitation = Some((Precip::Sleet, v.parse()?)),
                "snowfall" => spec.precipitation = Some((Precip::Snow, v.parse()?)),
                "dry" if v.parse::<bool>()? => spec.precipitation = Some((Precip::None, 0.0)),
                "dry" => {}
                "out" => spec.out = Some(PathBuf::from(v)),
                "software" => spec.software = v.parse()?,
                "verify_cull" => spec.verify_cull = v.parse()?,
                "lod" => spec.lod = v.parse()?,
                "fog" => spec.fog = v.parse()?,
                "lod_timeout" => spec.lod_timeout = v.parse()?,
                "globe" => spec.globe = Some(v.parse()?),
                "person" => spec.person = Some(v.parse()?),
                "body" => spec.body = v.parse()?,
                "senses" => spec.senses = Some(v.to_owned()),
                other => anyhow::bail!("unknown screenshot key {other:?}"),
            }
        }
        Ok(spec)
    }

    /// Parses a shot list: one spec per line, `#` comments, blank lines ignored. A line
    /// starting with `defaults:` sets keys for all following lines.
    pub fn parse_list(text: &str) -> anyhow::Result<Vec<Self>> {
        let mut defaults = String::new();
        let mut out = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if let Some(d) = line.strip_prefix("defaults:") {
                defaults = d.trim().to_owned();
                continue;
            }
            let spec = Self::parse(&format!("{defaults},{line}"))
                .map_err(|e| anyhow::anyhow!("line {}: {e}", i + 1))?;
            out.push(spec);
        }
        Ok(out)
    }
}

/// Renders every shot. Shots with the same world settings share one generated world. Shots
/// without an output path are numbered into `default_dir`.
pub fn run(specs: &[ShotSpec], cache_dir: Option<&Path>, default_dir: &Path) -> anyhow::Result<()> {
    let t0 = Instant::now();
    let first = specs
        .first()
        .ok_or_else(|| anyhow::anyhow!("no screenshots requested"))?;
    let ctx = GpuContext::headless(first.software).or_else(|e| {
        log::warn!("no hardware adapter ({e}); trying the software adapter");
        GpuContext::headless(true)
    })?;
    log::info!("rendering on {} ({:?})", ctx.info.name, ctx.info.backend);
    let mut world: Option<(u64, PlanetSize, usize, LocalWorld)> = None;
    let mut atlas: Option<(TextureArray, hearth_lod::LodGen)> = None;
    for (i, spec) in specs.iter().enumerate() {
        let out = spec
            .out
            .clone()
            .unwrap_or_else(|| default_dir.join(format!("shot_{:03}.png", i + 1)));
        let key = (spec.seed, spec.planet, spec.resolution);
        if world.as_ref().is_none_or(|w| (w.0, w.1, w.2) != key) {
            let lw = LocalWorld::create(spec.seed, spec.planet, spec.resolution, cache_dir)?;
            world = Some((spec.seed, spec.planet, spec.resolution, lw));
        }
        let lw = &mut world.as_mut().expect("created above").3;
        let (atlas, lod) = atlas.get_or_insert_with(|| {
            let entries = hearth_texgen::textures_for(Some(&lw.content));
            (
                TextureArray::from_entries(&entries),
                hearth_lod::LodGen::new(&lw.reg, &entries),
            )
        });
        let time = lw.content.time.clone();
        // Each shot starts from a clean map so dates don't mix (snow from an earlier shot).
        lw.map = hearth_world::CubeMap::new(*lw.map.planet());
        shoot(&ctx, atlas, lod, lw, spec, &out, Some(&time))?;
    }
    log::info!(
        "{} screenshot(s) in {:.2}s",
        specs.len(),
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}

/// A gentle land spot near a latitude (scans along the parallel).
fn land_at_latitude(lw: &LocalWorld, lat: f64) -> Option<(f64, f64)> {
    let planet = *lw.map.planet();
    let z = planet.z_for_latitude(lat.to_radians());
    let c = planet.circumference_f64();
    let steps = 4096;
    let mut best: Option<(f64, (f64, f64))> = None;
    for i in 0..steps {
        let x = c * i as f64 / steps as f64;
        let s = lw.terrain().sample(x as i32, z as i32);
        if s.is_underwater() || s.ocean || s.lake {
            continue;
        }
        // Prefer gentle, vegetated, moderately high ground.
        let score =
            -s.slope as f64 * 3.0 + s.tree_density as f64 + (s.height as f64 / 200.0).min(1.0);
        if best.is_none_or(|(b, _)| score > b) {
            best = Some((score, (x + 0.5, z + 0.5)));
        }
    }
    best.map(|(_, p)| p)
}

/// A rendered shot: colour (RGBA8 rows), depth (reverse-Z) and the view it was taken from.
pub struct Shot {
    pub pixels: Vec<u8>,
    pub depth: Vec<f32>,
    pub camera: Camera,
    /// Half-width of the full-detail area around the camera (blocks).
    pub near_radius: f64,
    /// LOD tiles drawn.
    pub lod_tiles: usize,
    pub terrain: hearth_render::terrain::TerrainStats,
    pub lod: hearth_render::lod::LodStats,
}

/// Loads the full-detail terrain around the shot's camera, builds its LOD terrain, and renders
/// it (three frames, so GPU culling settles; with `verify_cull`, checked against CPU culling).
pub fn render_shot(
    ctx: &GpuContext,
    atlas: &TextureArray,
    lod: &hearth_lod::LodGen,
    lw: &mut LocalWorld,
    spec: &ShotSpec,
    out: &Path,
    time: Option<&TimeConfig>,
) -> anyhow::Result<Shot> {
    let (sx, sz) = match (spec.x, spec.z, spec.lat) {
        (Some(x), Some(z), _) => (x, z),
        (_, _, Some(lat)) => land_at_latitude(lw, lat)
            .ok_or_else(|| anyhow::anyhow!("no land near latitude {lat}"))?,
        _ => {
            let (x, z) = lw.terrain().find_spawn(false);
            (x as f64 + 0.5, z as f64 + 0.5)
        }
    };
    let sy = spec.y.unwrap_or_else(|| lw.surface_y(sx, sz) + spec.above);
    let mut camera = Camera {
        pos: DVec3::new(sx, sy, sz),
        yaw: spec.yaw,
        pitch: spec.pitch,
        fov_y: spec.fov,
        near: 0.05,
    };
    // Date and time: season (mid-season in this hemisphere) or year fraction; local hour.
    let planet = *lw.map.planet();
    let southern = planet.latitude(sz) < 0.0;
    let year_frac = spec.year_frac.unwrap_or_else(|| {
        spec.season
            .map_or(0.3, |s| Calendar::season_start(s, southern) + 0.125)
    });
    let mut calendar = time.map_or_else(|| Calendar::new(48, 8, 23.44), Calendar::from_config);
    calendar.year_offset = year_frac;
    calendar.day_offset = (spec.hour / 24.0 - planet.solar_time_offset(sx)).rem_euclid(1.0);
    let moment = calendar.at(0);
    log::info!(
        "shot {} at {:.1}, {:.1}, {:.1} (lat {:.1}°, year {:.3}, {:.1} h)",
        out.display(),
        sx,
        sy,
        sz,
        planet.latitude_deg(sz),
        year_frac,
        spec.hour
    );
    let positions = lw.load_area(camera.pos, spec.distance, 2, spec.snow.then_some(year_frac));
    let models = BlockModels::build(&lw.reg, atlas);
    let meshes = lw.mesh(&models, &positions, MeshOptions::default());
    let mut scene = SceneRenderer::new(ctx, atlas, OFFSCREEN_FORMAT, planet, 4, 4);
    scene.terrain.render_distance = spec.distance;
    scene.terrain.vertical_distance = 64;
    for m in &meshes {
        scene.terrain.upload(ctx, m);
    }
    let (bx, bz) = (camera.pos.x.floor() as i32, camera.pos.z.floor() as i32);
    scene.set_sky_heights(ctx, &SkyHeights::build(bx, bz, |x, z| lw.map.sky_top(x, z)));
    scene.terrain.water.set_heights(
        ctx,
        crate::water_env::water_heights(&lw.map, &lw.reg, bx, bz),
    );
    let sampler = EnvSampler::new(lw.grid(), calendar);
    let (mut env, weather) = sampler.sample(
        &moment,
        camera.pos,
        0.0,
        EnvOverrides {
            cloud_cover: spec.clouds,
            precipitation: spec.precipitation,
        },
    );
    env.aerial_perspective = spec.fog;
    log::info!(
        "  sun elevation {:.1}°, {:.0} lux, clouds {:.0}%, {:.1} °C, {:?}",
        env.sun_dir.y.asin().to_degrees(),
        env.horizontal_lux(),
        weather.cloud_cover * 100.0,
        weather.temperature_c,
        weather.precip
    );
    let n = crate::season_cover::column_normals(
        &lw.generator,
        hearth_math::BlockPos::containing(camera.pos).column(),
    );
    let cover = hearth_env::climate::SeasonalCover::compute(&n);
    log::info!(
        "  climate: mean {:.1} °C (months {:.1}..{:.1}), {:.0} mm/yr, dry season {:?}, snow {:.2} m now, {:.2} m at the peak",
        n.t_mean,
        n.t_mean - n.t_range / 2.0,
        n.t_mean + n.t_range / 2.0,
        n.precip,
        hearth_env::tint::DryType::of(&n, false),
        cover.snow_depth_m(year_frac),
        cover.snow_depth_m(cover.peak_snow()),
    );
    // Distant terrain: every LOD tile out to the LOD distance, built before the shot (the queue
    // must be empty), beyond the full-detail cubes meshed above.
    let c = hearth_math::CubePos::containing(camera.pos);
    let r = spec.distance;
    let near = [
        ((c.x - r + 1) * 16) as f64,
        ((c.z - r + 1) * 16) as f64,
        ((c.x + r) * 16) as f64,
        ((c.z + r) * 16) as f64,
    ];
    let t_lod = Instant::now();
    let v = lw.terrain().vertical_scale() as f64;
    let reach = hearth_lod::draw_distance(spec.lod, camera.pos.y, v);
    // Refined by screen-space error as the game does (`LodStream`).
    let ppr = hearth_lod::px_per_rad(spec.height, spec.fov);
    let unsplit = rustc_hash::FxHashSet::default();
    let select = |errors: &crate::lod_stream::Errors| {
        if spec.lod == 0 {
            return Vec::new();
        }
        let built = |k: hearth_lod::TileKey| errors.get(&k).copied();
        let refine = hearth_lod::Refine {
            px_per_rad: ppr,
            max_error_px: hearth_core::options::VideoOptions::default().lod_error_px(),
            camera_y: camera.pos.y,
            built: &built,
            split_before: &unsplit,
        };
        hearth_lod::select_refined(
            &planet,
            camera.pos.x,
            camera.pos.z,
            reach,
            Some(near),
            Some(&refine),
        )
    };
    let late = AtomicBool::new(false);
    let mut quads = 0u64;
    let errors = crate::lod_stream::build_refined(
        1 + hearth_lod::MAX_EXTRA_LEVELS,
        select,
        |keys| {
            let tiles: Vec<hearth_lod::TileMesh> = keys
                .par_iter()
                .filter_map(|k| {
                    if t_lod.elapsed().as_secs_f64() > spec.lod_timeout {
                        late.store(true, Ordering::Relaxed);
                        return None;
                    }
                    Some(lod.build(&lw.generator, *k))
                })
                .collect();
            if late.load(Ordering::Relaxed) {
                anyhow::bail!(
                    "LOD terrain was not ready after {:.0} s ({} of {} tiles built)",
                    spec.lod_timeout,
                    tiles.len(),
                    keys.len()
                );
            }
            Ok(tiles)
        },
        |t| {
            crate::lod_stream::upload(ctx, &mut scene.lod, t);
            quads += t.quads() as u64;
        },
    )?;
    let keys = select(&errors);
    scene.lod_show = hearth_lod::cover(&keys, |k| errors.contains_key(&k))
        .iter()
        .map(|k| k.id())
        .collect();
    scene.near_area = (spec.lod > 0).then_some(near);
    scene.vertical_scale = lw.terrain().vertical_scale();
    log::info!(
        "  LOD: {} tiles ({} built), {} quads to {:.0} blocks in {:.2}s",
        keys.len(),
        errors.len(),
        quads,
        reach,
        t_lod.elapsed().as_secs_f64()
    );
    if spec.body {
        // The camera in the eyes of someone standing here, looking as the shot looks.
        let feet = DVec3::new(sx, lw.surface_y(sx, sz) + 1.0, sz);
        let figure = hearth_character::Figure::starting(hearth_character::Appearance::default());
        let pose = figure.animator.pose(
            &figure.rig,
            hearth_character::Activity::Stand,
            &hearth_character::Drive {
                look_pitch: spec.pitch,
                ..Default::default()
            },
        );
        let turn = glam::Quat::from_rotation_y(-spec.yaw.to_radians());
        camera.pos = feet + (turn * figure.eye(&pose)).as_dvec3();
        let chest = hearth_math::BlockPos::containing(feet + DVec3::Y * 1.2);
        let mut boxes = Vec::new();
        hearth_character::instances(
            &figure.rig,
            &figure.palette,
            &pose,
            glam::Affine3A::from_rotation_translation(turn, (feet - camera.pos).as_vec3()),
            hearth_character::Show {
                hide_head: true,
                sky_light: lw.map.sky_light(chest),
                block_light: lw.map.block_light(chest),
            },
            &mut boxes,
        );
        scene.figures.set(ctx, &boxes);
    }
    if let Some(d) = spec.person {
        // Someone standing on the ground in front of the camera, facing it.
        let f = camera.forward().as_dvec3();
        let flat = DVec3::new(f.x, 0.0, f.z).normalize_or(DVec3::Z);
        let (px, pz) = (camera.pos.x + flat.x * d, camera.pos.z + flat.z * d);
        let feet = DVec3::new(px, lw.surface_y(px, pz), pz);
        let figure = hearth_character::Figure::starting(hearth_character::Appearance::default());
        let pose = figure.animator.pose(
            &figure.rig,
            hearth_character::Activity::Stand,
            &hearth_character::Drive::default(),
        );
        let chest = hearth_math::BlockPos::containing(feet + DVec3::Y * 1.2);
        let place = glam::Affine3A::from_rotation_translation(
            glam::Quat::from_rotation_y(-(spec.yaw + 180.0).to_radians()),
            (feet - camera.pos).as_vec3(),
        );
        let mut boxes = Vec::new();
        hearth_character::instances(
            &figure.rig,
            &figure.palette,
            &pose,
            place,
            hearth_character::Show {
                hide_head: false,
                sky_light: lw.map.sky_light(chest),
                block_light: lw.map.block_light(chest),
            },
            &mut boxes,
        );
        scene.figures.set(ctx, &boxes);
    }
    if let Some(name) = &spec.senses {
        use hearth_render::post::Senses;
        scene.senses = match name.as_str() {
            "hurt" => Senses {
                vignette: 0.75,
                red: 0.6,
                desaturate: 0.3,
                ..Senses::default()
            },
            "cold" => Senses {
                cold: 0.6,
                desaturate: 0.1,
                ..Senses::default()
            },
            "hot" => Senses {
                heat: 0.8,
                time: 1.3,
                ..Senses::default()
            },
            "exhausted" => Senses {
                desaturate: 0.6,
                ..Senses::default()
            },
            "faint" => Senses {
                desaturate: 0.7,
                dim: 0.5,
                vignette: 0.5,
                ..Senses::default()
            },
            other => anyhow::bail!("unknown senses {other:?}"),
        };
    }
    let target = OffscreenTarget::new(ctx, spec.width, spec.height);
    let size = (spec.width, spec.height);
    let frame = |scene: &mut SceneRenderer| {
        scene.prepare(ctx, &camera, size, &env, f32::INFINITY);
        let mut enc = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("shot"),
            });
        scene.render(ctx, &mut enc, &target.color_view, &target.depth.view, size);
        ctx.queue.submit(Some(enc.finish()));
        target.read_rgba(ctx)
    };
    // Three frames: with GPU culling the first draws everything in phase 1, the second learns
    // which cubes are occluded, and the third draws only the survivors in phase 0.
    frame(&mut scene);
    frame(&mut scene);
    let pixels = frame(&mut scene);
    if spec.verify_cull && scene.terrain.uses_gpu_culling() {
        let counts = scene.terrain.read_gpu_draw_counts(ctx).unwrap_or_default();
        scene.terrain.gpu_culling = false;
        let reference = frame(&mut scene);
        let cpu_draws = scene.terrain.stats.draws;
        scene.terrain.gpu_culling = true;
        let differing = pixels
            .as_chunks::<4>()
            .0
            .iter()
            .zip(reference.as_chunks::<4>().0)
            .filter(|(a, b)| a != b)
            .count();
        log::info!(
            "cull check: GPU draws phase 0 {:?}, phase 1 {:?}; CPU draws {}; {} of {} pixels differ",
            &counts[0..4],
            &counts[4..8],
            cpu_draws,
            differing,
            pixels.len() / 4
        );
        if differing > 0 {
            let cpu_path = out.with_extension("cpu.png");
            let gpu_path = out.with_extension("gpu.png");
            write_png(&cpu_path, spec.width, spec.height, &reference)?;
            write_png(&gpu_path, spec.width, spec.height, &pixels)?;
            anyhow::bail!(
                "GPU culling changed {differing} pixels (see {} and {})",
                cpu_path.display(),
                gpu_path.display()
            );
        }
    }
    let depth = target.read_depth(ctx);
    Ok(Shot {
        pixels,
        depth,
        camera,
        near_radius: (spec.distance * 16) as f64,
        lod_tiles: scene.lod.stats.drawn,
        terrain: scene.terrain.stats,
        lod: scene.lod.stats,
    })
}

fn shoot(
    ctx: &GpuContext,
    atlas: &TextureArray,
    lod: &hearth_lod::LodGen,
    lw: &mut LocalWorld,
    spec: &ShotSpec,
    out: &Path,
    time: Option<&TimeConfig>,
) -> anyhow::Result<()> {
    if let Some(zoom) = spec.globe {
        return shoot_globe(ctx, lw, spec, out, zoom);
    }
    let shot = render_shot(ctx, atlas, lod, lw, spec, out, time)?;
    write_png(out, spec.width, spec.height, &shot.pixels)?;
    let s = shot.terrain;
    log::info!(
        "wrote {} ({} visible cubes, {} draws, {} quads, {:.1} MiB mesh memory; {} LOD tiles drawn, {} quads, {:.1} MiB)",
        out.display(),
        s.visible_cubes,
        s.draws,
        s.quads_drawn,
        (s.packed_bytes + s.general_bytes) as f64 / (1 << 20) as f64,
        shot.lod.drawn,
        shot.lod.quads,
        shot.lod.bytes as f64 / (1 << 20) as f64
    );
    Ok(())
}

/// The planet as the globe, centred on the shot's place (marked).
fn shoot_globe(
    ctx: &GpuContext,
    lw: &LocalWorld,
    spec: &ShotSpec,
    out: &Path,
    zoom: f32,
) -> anyhow::Result<()> {
    let t0 = Instant::now();
    let (x, z) = match (spec.x, spec.z, spec.lat) {
        (Some(x), Some(z), _) => (x, z),
        (_, _, Some(lat)) => land_at_latitude(lw, lat)
            .ok_or_else(|| anyhow::anyhow!("no land near latitude {lat}"))?,
        _ => {
            let (x, z) = lw.terrain().find_spawn(false);
            (x as f64, z as f64)
        }
    };
    let map = crate::globe::planet_map(lw.terrain(), crate::globe::MAP_WIDTH);
    log::info!("globe map in {:.2}s", t0.elapsed().as_secs_f64());
    let mut globe = hearth_render::globe::GlobeRenderer::new(ctx, OFFSCREEN_FORMAT);
    let w = crate::globe::MAP_WIDTH as u32;
    globe.set_map(ctx, w, w / 2, &map);
    let (lat, lon) = crate::globe::lat_lon(lw.map.planet(), DVec3::new(x, 0.0, z));
    let view = hearth_render::globe::GlobeView { lat, lon, zoom };
    let target = OffscreenTarget::new(ctx, spec.width, spec.height);
    let size = (spec.width, spec.height);
    let mut enc = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("globe shot"),
        });
    globe.render(
        ctx,
        &mut enc,
        &target.color_view,
        size,
        &view,
        Some((lat, lon)),
        None,
    );
    ctx.queue.submit(Some(enc.finish()));
    write_png(out, spec.width, spec.height, &target.read_rgba(ctx))?;
    log::info!(
        "wrote {} (globe at {})",
        out.display(),
        crate::globe::describe(lw.terrain(), lat, lon)
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_specs_and_lists() {
        let s = ShotSpec::parse(
            "seed=7, x=10.5, z=-3, yaw=90, w=640, h=360, out=a.png, season=autumn, hour=16.5",
        )
        .unwrap();
        assert_eq!(s.seed, 7);
        assert_eq!(s.x, Some(10.5));
        assert_eq!(s.z, Some(-3.0));
        assert_eq!((s.width, s.height), (640, 360));
        assert_eq!(s.out, Some(PathBuf::from("a.png")));
        assert_eq!(s.season, Some(Season::Autumn));
        assert_eq!(s.hour, 16.5);
        assert!(ShotSpec::parse("seed=x").is_err());
        assert!(ShotSpec::parse("colour=red").is_err());
        assert!(ShotSpec::parse("season=monsoon").is_err());
        let list =
            ShotSpec::parse_list("# suite\ndefaults: seed=3, w=320\n\nyaw=0\nyaw=180 # back\n")
                .unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|s| s.seed == 3 && s.width == 320));
        assert_eq!(list[1].yaw, 180.0);
    }
}

//! `hearth bench`: renders fixed camera paths through representative scenes offscreen, with the
//! default graphics preset, and reports frame times (average FPS, 1 % lows, 99th percentile),
//! GPU time per pass (timestamp queries), CPU time per system, draw calls, triangles, video
//! memory, bytes uploaded and heap allocations per frame. Results are appended to
//! `BENCHMARKS.md` and written as JSON; `--golden DIR` saves a reference image of every scene and
//! `--compare DIR` checks new images against them (SSIM). With `--baseline FILE`, a scene whose
//! average FPS or 1 % lows fall more than `--gate` percent below the baseline fails the run.
//!
//! Frames run as a real frame loop without presentation: at most two frames in flight, the
//! camera advancing a fixed step along its path each frame (so every run renders the same
//! frames), after warm-up frames that let GPU occlusion culling settle.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use glam::DVec3;
use hearth_content::schema::config::TimeConfig;
use hearth_core::options::VideoOptions;
use hearth_env::{Calendar, Precip};
use hearth_math::{BlockPos, PlanetSize};
use hearth_render::GpuContext;
use hearth_render::atlas::TextureArray;
use hearth_render::camera::Camera;
use hearth_render::mesh::MeshOptions;
use hearth_render::models::BlockModels;
use hearth_render::offscreen::{OFFSCREEN_FORMAT, OffscreenTarget, write_png};
use hearth_render::precip::SkyHeights;
use hearth_render::profiler::GpuTimer;
use hearth_render::scene::SceneRenderer;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use crate::environment::{EnvOverrides, EnvSampler};
use crate::lod_stream::Errors;
use crate::scene::LocalWorld;
use hearth_lod::TileKey;

/// Frames that may be queued on the GPU while the CPU records the next (as the swapchain's
/// frame latency).
const FRAMES_IN_FLIGHT: usize = 2;
/// The LOD selection is renewed when the camera has moved this far (blocks), as in the game.
const LOD_RESELECT: f64 = 16.0;
/// The rain-cover map is recentred when the camera has moved this far (blocks).
const SKY_HEIGHTS_RECENTRE: f64 = 32.0;

/// Where the camera is at a keyframe.
#[derive(Debug, Clone, Copy)]
enum Height {
    /// World Y.
    At(f64),
    /// Blocks above the ground or water surface.
    Above(f64),
}

#[derive(Debug, Clone, Copy)]
struct Key {
    x: f64,
    z: f64,
    y: Height,
    yaw: f32,
    pitch: f32,
}

const fn key(x: f64, z: f64, y: Height, yaw: f32, pitch: f32) -> Key {
    Key {
        x,
        z,
        y,
        yaw,
        pitch,
    }
}

/// How a scene's camera moves.
#[derive(Debug, Clone, Copy)]
enum CameraPath {
    /// Straight lines between keyframes, evenly timed.
    Keys(&'static [Key]),
    /// Along the longest dry cave near the driest high ground around (x, z) (where the water
    /// table lies deepest), lit by torches placed on its floor.
    Cave { x: f64, z: f64 },
}

/// A benchmark scene.
#[derive(Debug, Clone, Copy)]
struct SceneDef {
    name: &'static str,
    about: &'static str,
    seed: u64,
    path: CameraPath,
    /// LOD distance in chunks.
    lod: u32,
    year_frac: f64,
    /// Local solar hour.
    hour: f64,
    clouds: Option<f64>,
    precipitation: Option<(Precip, f64)>,
}

const FOREST: &[Key] = &[
    key(11660.0, -9460.0, Height::Above(18.0), 90.0, 14.0),
    key(11460.0, -9460.0, Height::Above(18.0), 90.0, 14.0),
];
const PEAK: &[Key] = &[
    key(10506.0, 8200.0, Height::At(420.0), 0.0, 6.0),
    key(10506.0, 8200.0, Height::At(420.0), 120.0, 6.0),
    key(10506.0, 8200.0, Height::At(420.0), 240.0, 6.0),
    key(10506.0, 8200.0, Height::At(420.0), 360.0, 6.0),
];
const COAST: &[Key] = &[
    key(176.0, -4580.0, Height::Above(10.0), 90.0, 6.0),
    key(176.0, -4440.0, Height::Above(10.0), 90.0, 6.0),
];
const UNDERWATER: &[Key] = &[
    key(110.0, -4496.0, Height::At(-5.0), 90.0, 10.0),
    key(-10.0, -4496.0, Height::At(-5.0), 90.0, 10.0),
];

/// Every scene, in report order.
const SCENES: &[SceneDef] = &[
    SceneDef {
        name: "lowland_forest",
        about: "flight over broadleaf forest and a river at 46° N, summer noon",
        seed: 7,
        path: CameraPath::Keys(FOREST),
        lod: 256,
        year_frac: 0.4,
        hour: 12.0,
        clouds: Some(0.3),
        precipitation: Some((Precip::None, 0.0)),
    },
    SceneDef {
        name: "peak_lod512",
        about: "a full turn on a volcano's summit (420), LOD distance 512",
        seed: 7,
        path: CameraPath::Keys(PEAK),
        lod: 512,
        year_frac: 0.4,
        hour: 11.0,
        clouds: Some(0.2),
        precipitation: Some((Precip::None, 0.0)),
    },
    SceneDef {
        name: "peak_lod1024",
        about: "the same turn, LOD distance 1024",
        seed: 7,
        path: CameraPath::Keys(PEAK),
        lod: 1024,
        year_frac: 0.4,
        hour: 11.0,
        clouds: Some(0.2),
        precipitation: Some((Precip::None, 0.0)),
    },
    SceneDef {
        name: "coast_sunset",
        about: "along a rocky shore at 24° N looking out to sea at sunset",
        seed: 7,
        path: CameraPath::Keys(COAST),
        lod: 256,
        year_frac: 0.4,
        hour: 18.9,
        clouds: Some(0.25),
        precipitation: Some((Precip::None, 0.0)),
    },
    SceneDef {
        name: "underwater",
        about: "six blocks under the sea off the same shore, afternoon",
        seed: 7,
        path: CameraPath::Keys(UNDERWATER),
        lod: 256,
        year_frac: 0.4,
        hour: 14.0,
        clouds: Some(0.2),
        precipitation: Some((Precip::None, 0.0)),
    },
    SceneDef {
        name: "cave_torches",
        about: "through the longest cave under a hill, lit by torches, at night",
        seed: 7,
        path: CameraPath::Cave {
            x: 3155.0,
            z: -6737.0,
        },
        lod: 256,
        year_frac: 0.4,
        hour: 23.0,
        clouds: Some(0.0),
        precipitation: Some((Precip::None, 0.0)),
    },
    SceneDef {
        name: "thunderstorm",
        about: "the forest flight under a heavy thunderstorm (16 mm/h, overcast)",
        seed: 7,
        path: CameraPath::Keys(FOREST),
        lod: 256,
        year_frac: 0.4,
        hour: 16.0,
        clouds: Some(1.0),
        precipitation: Some((Precip::Rain, 16.0)),
    },
];

/// The scenes of the short set the regression gate runs.
const QUICK: &[&str] = &["lowland_forest", "peak_lod512", "cave_torches"];

/// Command-line settings.
#[derive(Debug, Clone)]
pub struct BenchOptions {
    pub scenes: Vec<String>,
    pub frames: usize,
    pub warmup: usize,
    pub width: u32,
    pub height: u32,
    pub report: Option<PathBuf>,
    pub json: Option<PathBuf>,
    pub label: String,
    pub golden: Option<PathBuf>,
    pub compare: Option<PathBuf>,
    pub baseline: Option<PathBuf>,
    /// Allowed drop in average FPS or 1 % lows against the baseline (percent).
    pub gate: f64,
    pub software: bool,
    /// Vertical LOD error allowed on screen (pixels; 0: the distance rule alone).
    pub lod_error: f64,
}

impl Default for BenchOptions {
    fn default() -> Self {
        Self {
            scenes: SCENES.iter().map(|s| s.name.to_string()).collect(),
            frames: 600,
            warmup: 90,
            width: 1920,
            height: 1080,
            report: Some(PathBuf::from("BENCHMARKS.md")),
            json: Some(PathBuf::from("bench-out/bench.json")),
            label: String::new(),
            golden: None,
            compare: None,
            baseline: None,
            gate: 5.0,
            software: false,
            lod_error: VideoOptions::default().lod_error_px(),
        }
    }
}

pub const HELP: &str = "\
USAGE:
    hearth bench [OPTIONS]

Renders fixed camera paths offscreen with the default graphics preset and reports frame
times, GPU time per pass, CPU time per system, draws, triangles, video memory, uploads and
allocations per frame.

OPTIONS:
    --scenes all|quick|NAME,...  Scenes to run (default all): lowland_forest, peak_lod512,
                                 peak_lod1024, coast_sunset, underwater, cave_torches,
                                 thunderstorm; quick = the regression-gate set
    --frames N                   Measured frames per scene (default 600)
    --warmup N                   Frames before measuring (default 90)
    --size WxH                   Resolution (default 1920x1080)
    --report FILE|none           Markdown report to append to (default BENCHMARKS.md)
    --json FILE|none             Machine-readable results (default bench-out/bench.json)
    --label TEXT                 Name of this run in the report
    --golden DIR                 Save each scene's reference image into DIR
    --compare DIR                Compare each scene's image with DIR's (SSIM, diff images)
    --baseline FILE              Fail if average FPS or 1 % lows drop more than --gate
                                 percent below FILE's (a --json output)
    --gate PCT                   Allowed drop (default 5)
    --lod-error PX               Vertical LOD error allowed on screen (default: the
                                 preset's, 2; 0 = the distance rule alone)
    --software                   Use the software adapter";

impl BenchOptions {
    pub fn parse(args: &[String]) -> anyhow::Result<Self> {
        let mut o = Self::default();
        let mut it = args.iter();
        while let Some(a) = it.next() {
            let mut val = || {
                it.next()
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("{a} needs a value"))
            };
            let path = |v: String| (v != "none").then(|| PathBuf::from(v));
            match a.as_str() {
                "--scenes" => {
                    let v = val()?;
                    o.scenes = match v.as_str() {
                        "all" => SCENES.iter().map(|s| s.name.to_string()).collect(),
                        "quick" => QUICK.iter().map(|s| s.to_string()).collect(),
                        list => list.split(',').map(|s| s.trim().to_string()).collect(),
                    };
                    for s in &o.scenes {
                        if !SCENES.iter().any(|d| d.name == s) {
                            anyhow::bail!("unknown scene {s:?}");
                        }
                    }
                }
                "--frames" => o.frames = val()?.parse::<usize>()?.max(10),
                "--warmup" => o.warmup = val()?.parse()?,
                "--size" => {
                    let v = val()?;
                    let (w, h) = v
                        .split_once('x')
                        .ok_or_else(|| anyhow::anyhow!("--size WxH"))?;
                    o.width = w.parse()?;
                    o.height = h.parse()?;
                }
                "--report" => o.report = path(val()?),
                "--json" => o.json = path(val()?),
                "--label" => o.label = val()?,
                "--golden" => o.golden = Some(PathBuf::from(val()?)),
                "--compare" => o.compare = Some(PathBuf::from(val()?)),
                "--baseline" => o.baseline = Some(PathBuf::from(val()?)),
                "--gate" => o.gate = val()?.parse()?,
                "--software" => o.software = true,
                "--lod-error" => o.lod_error = val()?.parse::<f64>()?.max(0.0),
                other => anyhow::bail!("unknown argument {other:?}"),
            }
        }
        Ok(o)
    }
}

/// Averages of one scene's measured frames.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SceneResult {
    pub name: String,
    pub about: String,
    pub frames: usize,
    pub avg_fps: f64,
    /// Average frame rate of the slowest 1 % of frames.
    pub low1_fps: f64,
    pub p99_ms: f64,
    pub frame_ms: f64,
    /// GPU time per pass (ms), in frame order, and in total.
    pub gpu_passes: Vec<(String, f64)>,
    pub gpu_ms: f64,
    /// CPU time per system (ms), in frame order.
    pub cpu: Vec<(String, f64)>,
    /// Draw calls: GPU-culled terrain draws plus CPU-issued ones.
    pub draws: f64,
    pub triangles: f64,
    pub visible_cubes: f64,
    pub lod_tiles: f64,
    /// Video memory allocated at the end of the scene (MiB), where the backend reports it.
    pub vram_mib: Option<f64>,
    pub upload_kib: f64,
    pub upload_kib_max: f64,
    /// Heap allocations of the render thread per frame.
    pub allocs: f64,
    pub allocs_max: u64,
    /// Seconds spent generating, meshing and building LOD before the frames.
    pub setup_s: f64,
    /// Cubes meshed per second and LOD tiles built per second during the setup (all threads).
    pub mesh_cubes_per_s: f64,
    pub lod_tiles_per_s: f64,
    /// Heap allocations per frame by system.
    pub allocs_by_system: Vec<(String, f64)>,
    /// The slowest frames: frame time (ms) and what the render thread spent it on.
    pub slowest: Vec<(f64, Vec<(String, f64)>)>,
    /// Structural similarity with the golden image (1 = identical), when compared.
    pub ssim: Option<f64>,
}

/// A benchmark run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BenchRun {
    pub label: String,
    pub commit: String,
    pub adapter: String,
    pub backend: String,
    pub resolution: (u32, u32),
    pub preset: String,
    pub scenes: Vec<SceneResult>,
}

/// Runs the benchmark; returns the process exit code.
pub fn run(args: &[String], cache_dir: Option<&Path>) -> i32 {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{HELP}");
        return 0;
    }
    let opts = match BenchOptions::parse(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}\n\n{HELP}");
            return 2;
        }
    };
    match run_with(&opts, cache_dir) {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(e) => {
            eprintln!("benchmark failed: {e:#}");
            1
        }
    }
}

/// Runs the scenes; `Ok(false)` when the regression gate failed.
pub fn run_with(opts: &BenchOptions, cache_dir: Option<&Path>) -> anyhow::Result<bool> {
    let ctx = GpuContext::headless(opts.software)?;
    let video = VideoOptions::default();
    log::info!(
        "benchmark on {} ({:?}), {}x{}, preset {:?}: render distance {}, LOD {}",
        ctx.info.name,
        ctx.info.backend,
        opts.width,
        opts.height,
        video.graphics,
        video.render_distance,
        video.lod_distance
    );
    if !ctx.caps.timestamps_inside_encoders {
        log::warn!("this adapter cannot time passes: GPU times will be missing");
    }
    let mut worlds: FxHashMap<u64, LocalWorld> = FxHashMap::default();
    let mut assets: Option<(TextureArray, hearth_lod::LodGen, BlockModels)> = None;
    let mut run = BenchRun {
        label: opts.label.clone(),
        commit: git_commit(),
        adapter: ctx.info.name.clone(),
        backend: format!("{:?}", ctx.info.backend),
        resolution: (opts.width, opts.height),
        preset: format!("{:?}", video.graphics),
        scenes: Vec::new(),
    };
    for name in &opts.scenes {
        let def = SCENES
            .iter()
            .find(|d| d.name == name)
            .ok_or_else(|| anyhow::anyhow!("unknown scene {name}"))?;
        let lw = match worlds.entry(def.seed) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => e.insert(LocalWorld::create(
                def.seed,
                PlanetSize::Standard,
                0,
                cache_dir,
            )?),
        };
        let (atlas, lod, models) = assets.get_or_insert_with(|| {
            let entries = hearth_texgen::textures_for(Some(&lw.content));
            let atlas = TextureArray::from_entries(&entries);
            let models = BlockModels::build(&lw.reg, &atlas);
            (atlas, hearth_lod::LodGen::new(&lw.reg, &entries), models)
        });
        let time = lw.content.time.clone();
        lw.map = hearth_world::CubeMap::new(*lw.map.planet());
        let result = run_scene(&ctx, atlas, lod, models, lw, def, opts, &video, &time)?;
        log::info!(
            "{}: {:.1} FPS avg, {:.1} FPS 1% low, p99 {:.2} ms, GPU {:.2} ms",
            result.name,
            result.avg_fps,
            result.low1_fps,
            result.p99_ms,
            result.gpu_ms
        );
        run.scenes.push(result);
    }
    let text = report(&run);
    println!("{text}");
    if let Some(path) = &opts.report {
        append_report(path, &text)?;
    }
    if let Some(path) = &opts.json {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(&run)?)?;
    }
    match &opts.baseline {
        Some(b) => gate(&run, b, opts.gate),
        None => Ok(true),
    }
}

fn git_commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// A camera path resolved to world positions.
struct Path3 {
    keys: Vec<(DVec3, f32, f32)>,
}

impl Path3 {
    fn at(&self, t: f64) -> Camera {
        let n = self.keys.len();
        let (pos, yaw, pitch) = if n == 1 {
            self.keys[0]
        } else {
            let f = t.clamp(0.0, 1.0) * (n - 1) as f64;
            let i = (f.floor() as usize).min(n - 2);
            let u = f - i as f64;
            let (a, b) = (self.keys[i], self.keys[i + 1]);
            let mut dyaw = b.1 - a.1;
            if dyaw.abs() > 180.0 {
                dyaw -= 360.0 * dyaw.signum();
            }
            (
                a.0.lerp(b.0, u),
                a.1 + dyaw * u as f32,
                a.2 + (b.2 - a.2) * u as f32,
            )
        };
        Camera {
            pos,
            yaw: yaw.rem_euclid(360.0),
            pitch,
            fov_y: VideoOptions::default().fov,
            near: 0.05,
        }
    }

    fn samples(&self, n: usize) -> Vec<DVec3> {
        (0..n)
            .map(|i| self.at(i as f64 / (n - 1).max(1) as f64).pos)
            .collect()
    }
}

#[allow(clippy::too_many_arguments)]
fn run_scene(
    ctx: &GpuContext,
    atlas: &TextureArray,
    lodgen: &hearth_lod::LodGen,
    models: &BlockModels,
    lw: &mut LocalWorld,
    def: &SceneDef,
    opts: &BenchOptions,
    video: &VideoOptions,
    time: &TimeConfig,
) -> anyhow::Result<SceneResult> {
    let t_setup = Instant::now();
    let planet = *lw.map.planet();
    let rd = video.render_distance as i32;
    // Date and time at the path's start.
    let (sx, _) = match def.path {
        CameraPath::Keys(k) => (k[0].x, k[0].z),
        CameraPath::Cave { x, z } => (x, z),
    };
    let mut calendar = Calendar::from_config(time);
    calendar.year_offset = def.year_frac;
    calendar.day_offset = (def.hour / 24.0 - planet.solar_time_offset(sx)).rem_euclid(1.0);
    // Terrain along the path, then the path itself.
    let (path, positions) = match def.path {
        CameraPath::Keys(keys) => {
            let path = resolve_keys(lw, keys);
            let positions = load_along(lw, &path, rd, def.year_frac);
            (path, positions)
        }
        CameraPath::Cave { x, z } => {
            let (cx, cz) = dry_ground(lw, (x.floor() as i32, z.floor() as i32));
            let top = lw.surface_y(cx as f64, cz as f64);
            // Deep enough for caves well under the surface.
            let centre = DVec3::new(cx as f64, top - CAVE_DEPTH as f64, cz as f64);
            let positions = lw.load_area(centre, rd + 4, 2, Some(def.year_frac));
            let path = cave_path(lw, (cx, cz), CAVE_RADIUS)
                .ok_or_else(|| anyhow::anyhow!("no dry cave near ({cx}, {cz})"))?;
            (path, positions)
        }
    };
    let t_mesh = Instant::now();
    let meshes = lw.mesh(models, &positions, MeshOptions::default());
    let mesh_cubes_per_s = meshes.len() as f64 / t_mesh.elapsed().as_secs_f64().max(1e-9);
    let mut scene = SceneRenderer::new(
        ctx,
        atlas,
        OFFSCREEN_FORMAT,
        planet,
        video.mipmap_levels,
        video.anisotropic_filtering as u16,
    );
    scene.terrain.render_distance = rd;
    scene.terrain.vertical_distance = video.vertical_render_distance as i32;
    for m in &meshes {
        scene.terrain.upload(ctx, m);
    }
    drop(meshes);
    scene.vertical_scale = lw.terrain().vertical_scale();
    let v = scene.vertical_scale as f64;
    // Every LOD tile the path needs, built up front (the game streams them).
    let near_of = |c: DVec3| {
        let cube = hearth_math::CubePos::containing(c);
        let r = (rd - 1).max(1);
        [
            ((cube.x - r + 1) * 16) as f64,
            ((cube.z - r + 1) * 16) as f64,
            ((cube.x + r) * 16) as f64,
            ((cube.z + r) * 16) as f64,
        ]
    };
    // Refined by screen-space error as the game does (`LodStream`).
    let ppr = hearth_lod::px_per_rad(opts.height, VideoOptions::default().fov);
    let select = |c: DVec3, errors: &Errors, split_before: &FxHashSet<TileKey>| {
        if def.lod == 0 {
            return Vec::new();
        }
        let reach = hearth_lod::draw_distance(def.lod, c.y, v);
        let built = |k: TileKey| errors.get(&k).copied();
        let refine = (opts.lod_error > 0.0).then_some(hearth_lod::Refine {
            px_per_rad: ppr,
            max_error_px: opts.lod_error,
            camera_y: c.y,
            built: &built,
            split_before,
        });
        hearth_lod::select_refined(&planet, c.x, c.z, reach, Some(near_of(c)), refine.as_ref())
    };
    let unsplit = FxHashSet::default();
    let samples = path.samples(24);
    let t_lod = Instant::now();
    let rounds = if opts.lod_error > 0.0 {
        1 + hearth_lod::MAX_EXTRA_LEVELS
    } else {
        1
    };
    let errors = crate::lod_stream::build_refined(
        rounds,
        |errors| {
            let mut wanted = Vec::new();
            for p in &samples {
                wanted.extend(select(*p, errors, &unsplit));
            }
            wanted
        },
        |keys| {
            Ok(keys
                .par_iter()
                .map(|k| lodgen.build(&lw.generator, *k))
                .collect())
        },
        |t| crate::lod_stream::upload(ctx, &mut scene.lod, t),
    )?;
    let lod_tiles_per_s = errors.len() as f64 / t_lod.elapsed().as_secs_f64().max(1e-9);
    // What to draw: the selection, or the built tiles standing in for it.
    let show = |wanted: &[TileKey]| -> Vec<u64> {
        hearth_lod::cover(wanted, |k| errors.contains_key(&k))
            .iter()
            .map(|k| k.id())
            .collect()
    };
    let sampler = EnvSampler::new(lw.grid(), calendar);
    let overrides = EnvOverrides {
        cloud_cover: def.clouds,
        precipitation: def.precipitation,
    };
    let target = OffscreenTarget::new(ctx, opts.width, opts.height);
    let size = (opts.width, opts.height);
    scene.timer = GpuTimer::new(ctx);
    let setup_s = t_setup.elapsed().as_secs_f64();
    log::info!(
        "{}: {} cubes meshed ({:.0}/s), {} LOD tiles ({:.0}/s), set up in {:.1}s",
        def.name,
        positions.len(),
        mesh_cubes_per_s,
        errors.len(),
        lod_tiles_per_s,
        setup_s
    );

    let map = &lw.map;
    let sky_heights = |c: DVec3| {
        SkyHeights::build(c.x.floor() as i32, c.z.floor() as i32, |x, z| {
            map.sky_top(x, z)
        })
    };
    let total = opts.warmup + opts.frames;
    let mut state = FrameState::default();
    let mut in_flight: VecDeque<wgpu::SubmissionIndex> = VecDeque::new();
    let mut frame_ms = Vec::with_capacity(opts.frames);
    let mut sums = Sums::default();
    // What each measured frame spent its time on, to explain the slowest frames.
    let mut sections: Vec<[f64; SECTIONS.len()]> = Vec::with_capacity(opts.frames);
    let mut last_start: Option<Instant> = None;
    for f in 0..total {
        let measured = f >= opts.warmup;
        let t = if measured {
            (f - opts.warmup) as f64 / (opts.frames - 1) as f64
        } else {
            0.0
        };
        let camera = path.at(t);
        let start = Instant::now();
        if let Some(prev) = last_start
            && measured
            && f > opts.warmup
        {
            frame_ms.push((start - prev).as_secs_f64() * 1e3);
        }
        last_start = Some(start);
        let allocs0 = crate::alloc_count::thread_allocations();
        let uploads0 = ctx.uploaded_bytes();
        // The world clock runs at 20 ticks a second of a 60 Hz frame loop.
        let t0 = Instant::now();
        let a0 = crate::alloc_count::thread_allocations();
        let moment = calendar.at(f as u64 / 3);
        // Firelight at the camera sets the eye's adaptation (in caves by torchlight).
        let fire = map.block_light(BlockPos::containing(camera.pos)) as f32 / 15.0;
        let (mut env, _) = sampler.sample(&moment, camera.pos, fire, overrides);
        env.seconds = f as f32 / 60.0;
        let t1 = Instant::now();
        let a1 = crate::alloc_count::thread_allocations();
        if state
            .lod_at
            .is_none_or(|p| p.distance(camera.pos) >= LOD_RESELECT)
        {
            state.lod_at = Some(camera.pos);
            let wanted = select(camera.pos, &errors, &state.lod_split);
            state.lod_split = hearth_lod::split_nodes(&wanted);
            scene.lod_show = show(&wanted);
            scene.near_area = (def.lod > 0).then(|| near_of(camera.pos));
        }
        let t2 = Instant::now();
        let a2 = crate::alloc_count::thread_allocations();
        if state
            .sky_at
            .is_none_or(|p| p.distance(camera.pos) >= SKY_HEIGHTS_RECENTRE)
        {
            state.sky_at = Some(camera.pos);
            scene.set_sky_heights(ctx, &sky_heights(camera.pos));
        }
        let t3 = Instant::now();
        let a3 = crate::alloc_count::thread_allocations();
        scene.prepare(ctx, &camera, size, &env, 1.0 / 60.0);
        let t4 = Instant::now();
        let a4 = crate::alloc_count::thread_allocations();
        let mut enc = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("bench frame"),
            });
        scene.render(ctx, &mut enc, &target.color_view, &target.depth.view, size);
        let t5 = Instant::now();
        let a5 = crate::alloc_count::thread_allocations();
        let idx = ctx.queue.submit(Some(enc.finish()));
        scene.submitted();
        in_flight.push_back(idx);
        let t6 = Instant::now();
        let a6 = crate::alloc_count::thread_allocations();
        while in_flight.len() > FRAMES_IN_FLIGHT {
            let i = in_flight.pop_front().expect("non-empty");
            let _ = ctx.device.poll(wgpu::PollType::Wait {
                submission_index: Some(i),
                timeout: None,
            });
        }
        let t7 = Instant::now();
        let a7 = crate::alloc_count::thread_allocations();
        let gpu = scene
            .timer
            .as_mut()
            .map(|t| t.collect(ctx))
            .unwrap_or_default();
        let allocs = crate::alloc_count::thread_allocations() - allocs0;
        let uploaded = ctx.uploaded_bytes() - uploads0;
        if !measured {
            continue;
        }
        let ms = |a: Instant, b: Instant| (b - a).as_secs_f64() * 1e3;
        let times = [
            ms(t0, t1),
            ms(t1, t2),
            ms(t2, t3),
            scene.cpu.terrain_ms,
            scene.cpu.lod_ms,
            scene.cpu.sky_ms,
            ms(t4, t5),
            ms(t5, t6),
            ms(t6, t7),
        ];
        let counts = [
            a1 - a0,
            a2 - a1,
            a3 - a2,
            a4 - a3,
            0,
            0,
            a5 - a4,
            a6 - a5,
            a7 - a6,
        ];
        for ((label, t), n) in SECTIONS.iter().zip(times).zip(counts) {
            sums.add_cpu(label, t);
            sums.add_allocs(label, n);
        }
        sections.push(times);
        sums.frames += 1;
        sums.uploads += uploaded as f64;
        sums.uploads_max = sums.uploads_max.max(uploaded);
        sums.allocs += allocs as f64;
        sums.allocs_max = sums.allocs_max.max(allocs);
        let st = scene.terrain.stats;
        sums.visible += st.visible_cubes as f64;
        sums.lod_tiles += scene.lod.stats.drawn as f64;
        // CPU-issued draws: terrain draw lists (translucent, or all on the CPU path), one per
        // LOD tile, the sky, rain and tonemap.
        sums.cpu_draws += (st.draws + scene.lod.stats.drawn + 3) as f64;
        sums.cpu_quads += (st.translucent_quads + scene.lod.stats.quads) as f64;
        if !st.gpu_culling {
            sums.cpu_quads += (st.quads_drawn - st.translucent_quads) as f64;
        }
        for g in gpu {
            if g.seq as usize <= opts.warmup {
                continue;
            }
            sums.gpu_frames += 1;
            sums.gpu_total += g.total_ms;
            for (label, ms) in &g.passes {
                sums.add_gpu(label, *ms);
            }
            if let Some(s) = g.stats {
                sums.gpu_draws += s[..8].iter().map(|&x| x as f64).sum::<f64>();
                sums.gpu_quads += s[8..].iter().map(|&x| x as f64).sum::<f64>();
                sums.gpu_stat_frames += 1;
            }
        }
    }
    if let Some(t) = scene.timer.as_mut() {
        for g in t.drain(ctx) {
            if g.seq as usize <= opts.warmup {
                continue;
            }
            sums.gpu_frames += 1;
            sums.gpu_total += g.total_ms;
            for (label, ms) in &g.passes {
                sums.add_gpu(label, *ms);
            }
            if let Some(s) = g.stats {
                sums.gpu_draws += s[..8].iter().map(|&x| x as f64).sum::<f64>();
                sums.gpu_quads += s[8..].iter().map(|&x| x as f64).sum::<f64>();
                sums.gpu_stat_frames += 1;
            }
        }
    }
    let vram_mib = ctx
        .device
        .generate_allocator_report()
        .map(|r| r.total_allocated_bytes as f64 / (1u64 << 20) as f64);
    // The golden image: the middle of the path, after the culling has settled on it.
    let ssim = if opts.golden.is_some() || opts.compare.is_some() {
        scene.timer = None;
        let camera = path.at(0.5);
        let moment = calendar.at(0);
        let fire = map.block_light(BlockPos::containing(camera.pos)) as f32 / 15.0;
        let (env, _) = sampler.sample(&moment, camera.pos, fire, overrides);
        scene.lod_show = show(&select(camera.pos, &errors, &unsplit));
        scene.near_area = (def.lod > 0).then(|| near_of(camera.pos));
        scene.set_sky_heights(ctx, &sky_heights(camera.pos));
        let mut pixels = Vec::new();
        for _ in 0..3 {
            scene.prepare(ctx, &camera, size, &env, f32::INFINITY);
            let mut enc = ctx
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("bench golden"),
                });
            scene.render(ctx, &mut enc, &target.color_view, &target.depth.view, size);
            ctx.queue.submit(Some(enc.finish()));
            pixels = target.read_rgba(ctx);
        }
        let file = format!("{}.png", def.name);
        if let Some(dir) = &opts.golden {
            std::fs::create_dir_all(dir)?;
            write_png(&dir.join(&file), opts.width, opts.height, &pixels)?;
        }
        match &opts.compare {
            Some(dir) => {
                let golden = read_png(&dir.join(&file))?;
                anyhow::ensure!(
                    golden.0 == opts.width && golden.1 == opts.height,
                    "golden {} is {}x{}, not {}x{}",
                    file,
                    golden.0,
                    golden.1,
                    opts.width,
                    opts.height
                );
                let s = ssim(&golden.2, &pixels, opts.width, opts.height);
                let diff = diff_image(&golden.2, &pixels);
                let out = dir.join(format!("{}.diff.png", def.name));
                write_png(&out, opts.width, opts.height, &diff)?;
                write_png(
                    &dir.join(format!("{}.new.png", def.name)),
                    opts.width,
                    opts.height,
                    &pixels,
                )?;
                log::info!("{}: SSIM {s:.5} against the golden image", def.name);
                Some(s)
            }
            None => None,
        }
    } else {
        None
    };
    // The slowest frames: a frame's time runs from its start to the next frame's start, so it
    // is spent on its own sections.
    let mut order: Vec<usize> = (0..frame_ms.len()).collect();
    order.sort_by(|a, b| frame_ms[*b].total_cmp(&frame_ms[*a]));
    let slowest: Vec<(f64, Vec<(String, f64)>)> = order
        .iter()
        .take(5)
        .filter_map(|&k| {
            let s = sections.get(k)?;
            Some((
                frame_ms[k],
                SECTIONS
                    .iter()
                    .zip(s)
                    .filter(|(_, t)| **t >= 0.05)
                    .map(|(l, t)| (l.to_string(), *t))
                    .collect(),
            ))
        })
        .collect();
    for (ms, parts) in &slowest {
        let parts: Vec<String> = parts.iter().map(|(l, t)| format!("{l} {t:.2}")).collect();
        log::info!("{}: slow frame {ms:.2} ms: {}", def.name, parts.join(", "));
    }
    let mut r = sums.result(def, frame_ms, vram_mib, setup_s, ssim);
    r.mesh_cubes_per_s = mesh_cubes_per_s;
    r.lod_tiles_per_s = lod_tiles_per_s;
    r.slowest = slowest;
    Ok(r)
}

/// The render thread's work in a frame, in order.
const SECTIONS: [&str; 9] = [
    "environment",
    "lod selection",
    "rain cover map",
    "prepare: terrain",
    "prepare: lod",
    "prepare: sky, rain",
    "encode",
    "submit",
    "wait for gpu",
];

/// Keyframes to a dense path (a point every few blocks) whose heights above the ground follow
/// the terrain, smoothed so the camera clears every rise near the line.
fn resolve_keys(lw: &LocalWorld, keys: &[Key]) -> Path3 {
    const STEP: f64 = 4.0;
    let mut pts: Vec<(DVec3, f32, f32, Option<f64>)> = Vec::new();
    for (i, pair) in keys.windows(2).enumerate() {
        let (a, b) = (pair[0], pair[1]);
        let len = ((b.x - a.x).powi(2) + (b.z - a.z).powi(2)).sqrt();
        let n = ((len / STEP).ceil() as usize).max(1);
        let mut dyaw = b.yaw - a.yaw;
        if dyaw.abs() > 180.0 {
            dyaw -= 360.0 * dyaw.signum();
        }
        for k in 0..=n {
            if k == 0 && i > 0 {
                continue;
            }
            let u = k as f64 / n as f64;
            let (x, z) = (a.x + (b.x - a.x) * u, a.z + (b.z - a.z) * u);
            let (y, above) = match (a.y, b.y) {
                (Height::At(ya), Height::At(yb)) => (ya + (yb - ya) * u, None),
                (Height::Above(ha), Height::Above(hb)) => (0.0, Some(ha + (hb - ha) * u)),
                (Height::At(y), _) | (_, Height::At(y)) => (y, None),
            };
            pts.push((
                DVec3::new(x, y, z),
                a.yaw + dyaw * u as f32,
                a.pitch + (b.pitch - a.pitch) * u as f32,
                above,
            ));
        }
    }
    if keys.len() == 1 {
        let k = keys[0];
        let (y, above) = match k.y {
            Height::At(y) => (y, None),
            Height::Above(h) => (0.0, Some(h)),
        };
        pts.push((DVec3::new(k.x, y, k.z), k.yaw, k.pitch, above));
    }
    // Heights above the ground: the highest ground within a few points, so the camera never
    // dips into a rise between samples.
    let ground: Vec<f64> = pts.iter().map(|p| lw.surface_y(p.0.x, p.0.z)).collect();
    let keys = pts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let y = match p.3 {
                Some(h) => {
                    let lo = i.saturating_sub(3);
                    let hi = (i + 3).min(ground.len() - 1);
                    ground[lo..=hi].iter().copied().fold(f64::MIN, f64::max) + h
                }
                None => p.0.y,
            };
            (DVec3::new(p.0.x, y, p.0.z), p.1.rem_euclid(360.0), p.2)
        })
        .collect();
    Path3 { keys }
}

/// Loads the full-detail terrain every point of the path needs; returns the cubes to mesh.
fn load_along(
    lw: &mut LocalWorld,
    path: &Path3,
    rd: i32,
    year_frac: f64,
) -> Vec<hearth_math::CubePos> {
    let pts = path.samples(32);
    let (mut lo, mut hi) = (pts[0], pts[0]);
    for p in &pts {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    let centre = (lo + hi) * 0.5;
    let extent = ((hi.x - lo.x).max(hi.z - lo.z) / 2.0 / 16.0).ceil() as i32;
    let extra = ((hi.y - centre.y) / 16.0).ceil().max(0.0) as i32 + 2;
    lw.load_area(
        DVec3::new(centre.x, lo.y, centre.z),
        rd + extent + 1,
        extra,
        Some(year_frac),
    )
}

#[derive(Default)]
struct FrameState {
    lod_at: Option<DVec3>,
    /// The tiles the last LOD selection split (hysteresis).
    lod_split: FxHashSet<TileKey>,
    sky_at: Option<DVec3>,
}

#[derive(Default)]
struct Sums {
    frames: usize,
    cpu: Vec<(&'static str, f64)>,
    gpu: Vec<(&'static str, f64)>,
    gpu_frames: usize,
    gpu_total: f64,
    gpu_draws: f64,
    gpu_quads: f64,
    gpu_stat_frames: usize,
    cpu_draws: f64,
    cpu_quads: f64,
    visible: f64,
    lod_tiles: f64,
    uploads: f64,
    uploads_max: u64,
    allocs: f64,
    allocs_max: u64,
    allocs_by: Vec<(&'static str, f64)>,
}

impl Sums {
    fn add_cpu(&mut self, label: &'static str, ms: f64) {
        match self.cpu.iter_mut().find(|(l, _)| *l == label) {
            Some(e) => e.1 += ms,
            None => self.cpu.push((label, ms)),
        }
    }

    fn add_allocs(&mut self, label: &'static str, n: u64) {
        match self.allocs_by.iter_mut().find(|(l, _)| *l == label) {
            Some(e) => e.1 += n as f64,
            None => self.allocs_by.push((label, n as f64)),
        }
    }

    fn add_gpu(&mut self, label: &'static str, ms: f64) {
        match self.gpu.iter_mut().find(|(l, _)| *l == label) {
            Some(e) => e.1 += ms,
            None => self.gpu.push((label, ms)),
        }
    }

    fn result(
        self,
        def: &SceneDef,
        mut frame_ms: Vec<f64>,
        vram_mib: Option<f64>,
        setup_s: f64,
        ssim: Option<f64>,
    ) -> SceneResult {
        let n = self.frames.max(1) as f64;
        let gn = self.gpu_frames.max(1) as f64;
        let total: f64 = frame_ms.iter().sum();
        frame_ms.sort_by(|a, b| a.total_cmp(b));
        let count = frame_ms.len().max(1);
        let p99 = frame_ms
            .get(((count as f64 * 0.99).ceil() as usize).saturating_sub(1))
            .copied()
            .unwrap_or(0.0);
        let worst = (count as f64 * 0.01).ceil().max(1.0) as usize;
        let worst_mean = frame_ms.iter().rev().take(worst).sum::<f64>() / worst as f64;
        let stat_n = self.gpu_stat_frames.max(1) as f64;
        let gpu_draws = if self.gpu_stat_frames > 0 {
            self.gpu_draws / stat_n
        } else {
            0.0
        };
        let gpu_quads = if self.gpu_stat_frames > 0 {
            self.gpu_quads / stat_n
        } else {
            0.0
        };
        SceneResult {
            name: def.name.into(),
            about: def.about.into(),
            frames: self.frames,
            avg_fps: if total > 0.0 {
                frame_ms.len() as f64 * 1e3 / total
            } else {
                0.0
            },
            low1_fps: if worst_mean > 0.0 {
                1e3 / worst_mean
            } else {
                0.0
            },
            p99_ms: p99,
            frame_ms: total / count as f64,
            gpu_passes: self
                .gpu
                .iter()
                .map(|(l, ms)| (l.to_string(), ms / gn))
                .collect(),
            gpu_ms: self.gpu_total / gn,
            cpu: self
                .cpu
                .iter()
                .map(|(l, ms)| (l.to_string(), ms / n))
                .collect(),
            draws: self.cpu_draws / n + gpu_draws,
            triangles: 2.0 * (self.cpu_quads / n + gpu_quads),
            visible_cubes: self.visible / n,
            lod_tiles: self.lod_tiles / n,
            vram_mib,
            upload_kib: self.uploads / n / 1024.0,
            upload_kib_max: self.uploads_max as f64 / 1024.0,
            allocs: self.allocs / n,
            allocs_max: self.allocs_max,
            setup_s,
            mesh_cubes_per_s: 0.0,
            lod_tiles_per_s: 0.0,
            allocs_by_system: self
                .allocs_by
                .iter()
                .map(|(l, a)| (l.to_string(), a / n))
                .collect(),
            slowest: Vec::new(),
            ssim,
        }
    }
}

// ------------------------------------------------------------------------------ caves

/// Caves are searched this far from their centre (blocks) and down to this depth under the
/// surface.
const CAVE_RADIUS: i32 = 224;
const CAVE_DEPTH: i32 = 96;

/// The place near `around` (within 2 km) where the water table lies deepest under dry ground:
/// caves above the table are dry.
fn dry_ground(lw: &LocalWorld, around: (i32, i32)) -> (i32, i32) {
    let wg = &lw.generator;
    let mut best = (f32::MIN, around);
    for dz in (-2048..=2048).step_by(128) {
        for dx in (-2048..=2048).step_by(128) {
            let (x, z) = (around.0 + dx, around.1 + dz);
            let s = wg.terrain.sample(x, z);
            if s.is_underwater() || s.ocean || s.lake || s.slope > 0.6 {
                continue;
            }
            let dry = s.height - wg.hydro.water_table(wg, x, z);
            if dry > best.0 {
                best = (dry, (x, z));
            }
        }
    }
    log::info!(
        "cave search around ({}, {}): water table {:.0} blocks down",
        best.1.0,
        best.1.1,
        best.0
    );
    best.1
}

/// A path through the longest cave near `centre`: underground air (at least six blocks under
/// the surface) is split into connected caves, the largest is kept, and the path runs between
/// its two most distant points. Torches are set on the floor along the path and around the
/// cave, and the light is updated.
fn cave_path(lw: &mut LocalWorld, centre: (i32, i32), radius: i32) -> Option<Path3> {
    let air = |lw: &LocalWorld, p: BlockPos| lw.map.block(p).is_some_and(|b| b.is_air());
    let mut cells: FxHashSet<BlockPos> = FxHashSet::default();
    for z in centre.1 - radius..=centre.1 + radius {
        for x in centre.0 - radius..=centre.0 + radius {
            let top = lw.terrain().sample(x, z).height_i();
            for y in top - CAVE_DEPTH..top - 6 {
                let p = BlockPos::new(x, y, z);
                if air(lw, p) {
                    cells.insert(p);
                }
            }
        }
    }
    // Connected caves, largest first.
    let mut seen: FxHashSet<BlockPos> = FxHashSet::default();
    let mut best: Vec<BlockPos> = Vec::new();
    let neighbours = |p: BlockPos| {
        [
            BlockPos::new(p.x + 1, p.y, p.z),
            BlockPos::new(p.x - 1, p.y, p.z),
            BlockPos::new(p.x, p.y + 1, p.z),
            BlockPos::new(p.x, p.y - 1, p.z),
            BlockPos::new(p.x, p.y, p.z + 1),
            BlockPos::new(p.x, p.y, p.z - 1),
        ]
    };
    let mut starts: Vec<BlockPos> = cells.iter().copied().collect();
    starts.sort_unstable_by_key(|p| (p.x, p.y, p.z));
    for &s in &starts {
        if !seen.insert(s) {
            continue;
        }
        let mut comp = vec![s];
        let mut i = 0;
        while i < comp.len() {
            for n in neighbours(comp[i]) {
                if cells.contains(&n) && seen.insert(n) {
                    comp.push(n);
                }
            }
            i += 1;
        }
        if comp.len() > best.len() {
            best = comp;
        }
    }
    if best.len() < 200 {
        return None;
    }
    let cave: FxHashSet<BlockPos> = best.iter().copied().collect();
    // Camera cells: air with air above (room for the eye).
    let roomy = |p: &BlockPos| cave.contains(&BlockPos::new(p.x, p.y + 1, p.z));
    let bfs = |from: BlockPos| {
        let mut prev: FxHashMap<BlockPos, BlockPos> = FxHashMap::default();
        let mut order = vec![from];
        prev.insert(from, from);
        let mut i = 0;
        while i < order.len() {
            let p = order[i];
            for n in neighbours(p) {
                if cave.contains(&n) && roomy(&n) && !prev.contains_key(&n) {
                    prev.insert(n, p);
                    order.push(n);
                }
            }
            i += 1;
        }
        (order, prev)
    };
    let start = *best
        .iter()
        .filter(|p| roomy(p))
        .min_by_key(|p| (p.x, p.y, p.z))?;
    let (order, _) = bfs(start);
    let a = *order.last()?;
    let (order, prev) = bfs(a);
    let b = *order.last()?;
    let mut cells_path = vec![b];
    while let Some(&p) = prev.get(cells_path.last()?) {
        if p == *cells_path.last()? {
            break;
        }
        cells_path.push(p);
    }
    if cells_path.len() < 24 {
        return None;
    }
    // Torches: on the floor every 7 blocks along the path, and scattered through the cave.
    let torch = lw.reg.parse_state("torch").ok()?;
    let mut lights = Vec::new();
    let floor_of = |lw: &LocalWorld, p: BlockPos| {
        let mut q = p;
        for _ in 0..8 {
            let below = BlockPos::new(q.x, q.y - 1, q.z);
            if !air(lw, below) {
                return Some(q);
            }
            q = below;
        }
        None
    };
    for p in cells_path.iter().step_by(7) {
        if let Some(q) = floor_of(lw, *p) {
            lights.push(q);
        }
    }
    let mut rest: Vec<BlockPos> = best.clone();
    rest.sort_unstable_by_key(|p| (p.x, p.y, p.z));
    for p in rest.iter().step_by(97) {
        if let Some(q) = floor_of(lw, *p) {
            lights.push(q);
        }
    }
    for q in &lights {
        lw.map.set_block(*q, torch, &lw.reg);
        lw.light.block_changed(&mut lw.map, &lw.reg, *q);
    }
    log::info!(
        "cave of {} blocks, path {} blocks, {} torches",
        best.len(),
        cells_path.len(),
        lights.len()
    );
    // Keyframes every four cells, eye half a block above the cell's floor, looking ahead.
    let pts: Vec<DVec3> = cells_path
        .iter()
        .step_by(4)
        .map(|p| DVec3::new(p.x as f64 + 0.5, p.y as f64 + 0.6, p.z as f64 + 0.5))
        .collect();
    let keys = pts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let next = pts[(i + 1).min(pts.len() - 1)];
            let prev = pts[i.saturating_sub(1)];
            let d = if i + 1 < pts.len() {
                next - *p
            } else {
                *p - prev
            };
            // yaw: forward = (−sin, 0, cos).
            let yaw = (-d.x).atan2(d.z).to_degrees() as f32;
            (*p, yaw.rem_euclid(360.0), 0.0)
        })
        .collect();
    Some(Path3 { keys })
}

// ------------------------------------------------------------------------------ images

fn read_png(path: &Path) -> anyhow::Result<(u32, u32, Vec<u8>)> {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let info = reader.next_frame(&mut buf)?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        other => anyhow::bail!("{}: unsupported PNG colour type {other:?}", path.display()),
    };
    Ok((info.width, info.height, rgba))
}

/// Mean structural similarity of two RGBA8 images (luma, 8×8 windows every 4 pixels).
pub fn ssim(a: &[u8], b: &[u8], w: u32, h: u32) -> f64 {
    let luma = |p: &[u8]| -> Vec<f32> {
        p.as_chunks::<4>()
            .0
            .iter()
            .map(|c| 0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32)
            .collect()
    };
    let (la, lb) = (luma(a), luma(b));
    let (w, h) = (w as usize, h as usize);
    let (c1, c2) = ((0.01f64 * 255.0).powi(2), (0.03f64 * 255.0).powi(2));
    let mut sum = 0.0;
    let mut n = 0usize;
    for y in (0..h.saturating_sub(8)).step_by(4) {
        for x in (0..w.saturating_sub(8)).step_by(4) {
            let (mut ma, mut mb, mut va, mut vb, mut cov) = (0f64, 0f64, 0f64, 0f64, 0f64);
            for yy in y..y + 8 {
                for xx in x..x + 8 {
                    let (p, q) = (la[yy * w + xx] as f64, lb[yy * w + xx] as f64);
                    ma += p;
                    mb += q;
                    va += p * p;
                    vb += q * q;
                    cov += p * q;
                }
            }
            let k = 64.0;
            let (ma, mb) = (ma / k, mb / k);
            let (va, vb, cov) = (va / k - ma * ma, vb / k - mb * mb, cov / k - ma * mb);
            sum += ((2.0 * ma * mb + c1) * (2.0 * cov + c2))
                / ((ma * ma + mb * mb + c1) * (va + vb + c2));
            n += 1;
        }
    }
    if n == 0 { 1.0 } else { sum / n as f64 }
}

/// Absolute difference, amplified four times, for review.
fn diff_image(a: &[u8], b: &[u8]) -> Vec<u8> {
    a.as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .flat_map(|(p, q)| {
            let d = |i: usize| ((p[i] as i32 - q[i] as i32).unsigned_abs() * 4).min(255) as u8;
            [d(0), d(1), d(2), 255]
        })
        .collect()
}

// ------------------------------------------------------------------------------ reports

fn report(run: &BenchRun) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "## {} — commit {}, {} ({}), {}x{}, preset {}",
        if run.label.is_empty() {
            "Run"
        } else {
            &run.label
        },
        run.commit,
        run.adapter,
        run.backend,
        run.resolution.0,
        run.resolution.1,
        run.preset
    );
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "| Scene | Avg FPS | 1% low FPS | p99 ms | GPU ms | CPU ms | Draws | Triangles | VRAM MiB | Upload KiB/frame (max) | Allocs/frame (max) |"
    );
    let _ = writeln!(s, "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    for r in &run.scenes {
        let cpu: f64 = r
            .cpu
            .iter()
            .filter(|(l, _)| l != "wait for gpu")
            .map(|(_, ms)| ms)
            .sum();
        let _ = writeln!(
            s,
            "| {} | {:.1} | {:.1} | {:.2} | {:.2} | {:.2} | {:.0} | {:.2} M | {} | {:.1} ({:.0}) | {:.1} ({}) |",
            r.name,
            r.avg_fps,
            r.low1_fps,
            r.p99_ms,
            r.gpu_ms,
            cpu,
            r.draws,
            r.triangles / 1e6,
            r.vram_mib.map_or("–".into(), |v| format!("{v:.0}")),
            r.upload_kib,
            r.upload_kib_max,
            r.allocs,
            r.allocs_max
        );
    }
    let _ = writeln!(s);
    // GPU passes and CPU systems, one column per scene.
    let names: Vec<&str> = run.scenes.iter().map(|r| r.name.as_str()).collect();
    let mut table = |title: &str, rows: Vec<String>, get: &dyn Fn(&SceneResult, &str) -> f64| {
        let _ = writeln!(s, "| {title} | {} |", names.join(" | "));
        let _ = writeln!(s, "|---|{}", "---:|".repeat(names.len()));
        for row in rows {
            let cells: Vec<String> = run
                .scenes
                .iter()
                .map(|r| format!("{:.3}", get(r, &row)))
                .collect();
            let _ = writeln!(s, "| {row} | {} |", cells.join(" | "));
        }
        let _ = writeln!(s);
    };
    let mut gpu_rows: Vec<String> = Vec::new();
    let mut cpu_rows: Vec<String> = Vec::new();
    for r in &run.scenes {
        for (l, _) in &r.gpu_passes {
            if !gpu_rows.contains(l) {
                gpu_rows.push(l.clone());
            }
        }
        for (l, _) in &r.cpu {
            if !cpu_rows.contains(l) {
                cpu_rows.push(l.clone());
            }
        }
    }
    let find =
        |v: &[(String, f64)], l: &str| v.iter().find(|(k, _)| k == l).map_or(0.0, |(_, ms)| *ms);
    table("GPU ms per pass", gpu_rows, &|r, l| find(&r.gpu_passes, l));
    table("CPU ms per system", cpu_rows.clone(), &|r, l| {
        find(&r.cpu, l)
    });
    table("Allocations per frame", cpu_rows, &|r, l| {
        find(&r.allocs_by_system, l)
    });
    let _ = writeln!(
        s,
        "| Scene | Visible cubes | LOD tiles drawn | Meshing cubes/s | LOD tiles/s | Setup s | SSIM vs golden | Slowest frame | What |"
    );
    let _ = writeln!(s, "|---|---:|---:|---:|---:|---:|---:|---|---|");
    for r in &run.scenes {
        let slow = r.slowest.first().map_or("–".into(), |(ms, parts)| {
            let top = parts
                .iter()
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map_or(String::new(), |(l, t)| format!(", {l} {t:.2}"));
            format!("{ms:.2} ms{top}")
        });
        let _ = writeln!(
            s,
            "| {} | {:.0} | {:.0} | {:.0} | {:.0} | {:.1} | {} | {} | {} |",
            r.name,
            r.visible_cubes,
            r.lod_tiles,
            r.mesh_cubes_per_s,
            r.lod_tiles_per_s,
            r.setup_s,
            r.ssim.map_or("–".into(), |v| format!("{v:.4}")),
            slow,
            r.about
        );
    }
    s
}

fn append_report(path: &Path, text: &str) -> anyhow::Result<()> {
    use std::io::Write;
    let fresh = !path.exists();
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    if fresh {
        writeln!(
            f,
            "# Benchmarks\n\nResults of `hearth bench` (newest last). Frame times come from an \
             offscreen frame loop with two frames in flight and no presentation; GPU times from \
             timestamps written between passes. See `docs/perf-audit.md`.\n"
        )?;
    }
    writeln!(f, "{text}")?;
    Ok(())
}

/// Checks a run against a baseline: `Ok(false)` if any scene's average FPS or 1 % lows fell by
/// more than `pct` percent.
fn gate(run: &BenchRun, baseline: &Path, pct: f64) -> anyhow::Result<bool> {
    let base: BenchRun = serde_json::from_str(&std::fs::read_to_string(baseline)?)?;
    let mut ok = true;
    for r in &run.scenes {
        let Some(b) = base.scenes.iter().find(|b| b.name == r.name) else {
            log::warn!("{}: not in the baseline", r.name);
            continue;
        };
        for (what, new, old) in [
            ("average FPS", r.avg_fps, b.avg_fps),
            ("1% low FPS", r.low1_fps, b.low1_fps),
        ] {
            let change = (new / old.max(1e-9) - 1.0) * 100.0;
            if change < -pct {
                ok = false;
                eprintln!(
                    "REGRESSION {}: {what} {new:.1} vs {old:.1} ({change:+.1} %, limit -{pct} %)",
                    r.name
                );
            } else {
                println!("{}: {what} {new:.1} vs {old:.1} ({change:+.1} %)", r.name);
            }
        }
    }
    Ok(ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssim_of_identical_and_different_images() {
        let (w, h) = (32u32, 32u32);
        let a: Vec<u8> = (0..w * h)
            .flat_map(|i| {
                let v = ((i * 37) % 251) as u8;
                [v, v, v, 255]
            })
            .collect();
        assert!((ssim(&a, &a, w, h) - 1.0).abs() < 1e-9);
        let b: Vec<u8> = a.iter().map(|v| v.saturating_add(40)).collect();
        let c: Vec<u8> = a.iter().rev().copied().collect();
        let (sb, sc) = (ssim(&a, &b, w, h), ssim(&a, &c, w, h));
        assert!(sb < 1.0 && sc < sb, "{sb} {sc}");
    }

    #[test]
    fn options_parse() {
        let o = BenchOptions::parse(&[
            "--scenes".into(),
            "quick".into(),
            "--size".into(),
            "640x360".into(),
            "--report".into(),
            "none".into(),
        ])
        .unwrap();
        assert_eq!(o.scenes.len(), QUICK.len());
        assert_eq!((o.width, o.height), (640, 360));
        assert!(o.report.is_none());
        assert!(BenchOptions::parse(&["--scenes".into(), "moon".into()]).is_err());
    }

    #[test]
    fn paths_interpolate_the_short_way_round() {
        let p = Path3 {
            keys: vec![(DVec3::ZERO, 350.0, 0.0), (DVec3::X, 10.0, 10.0)],
        };
        let c = p.at(0.5);
        assert!(
            (c.yaw - 0.0).abs() < 1e-3 || (c.yaw - 360.0).abs() < 1e-3,
            "{}",
            c.yaw
        );
        assert!((c.pos.x - 0.5).abs() < 1e-9 && (c.pitch - 5.0).abs() < 1e-6);
        // A full turn in thirds keeps turning the same way.
        let turn = Path3 {
            keys: vec![
                (DVec3::ZERO, 0.0, 0.0),
                (DVec3::ZERO, 120.0, 0.0),
                (DVec3::ZERO, 240.0, 0.0),
                (DVec3::ZERO, 360.0, 0.0),
            ],
        };
        assert!((turn.at(0.9).yaw - 324.0).abs() < 1e-3);
    }
}

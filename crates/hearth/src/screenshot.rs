//! Headless screenshot mode: `hearth --screenshot "seed=1,yaw=30,pitch=12,out=shot.png"` renders
//! fixed camera shots to PNG without a window (a software adapter is used if no GPU exists).

use std::path::{Path, PathBuf};
use std::time::Instant;

use glam::{DVec3, Vec3};
use hearth_math::PlanetSize;
use hearth_render::atlas::TextureArray;
use hearth_render::camera::Camera;
use hearth_render::mesh::MeshOptions;
use hearth_render::models::BlockModels;
use hearth_render::offscreen::{OFFSCREEN_FORMAT, OffscreenTarget, write_png};
use hearth_render::terrain::{FrameParams, TerrainRenderer};
use hearth_render::{GpuContext, LinearColor, render_terrain};

use crate::scene::LocalWorld;

/// One screenshot request.
#[derive(Debug, Clone, PartialEq)]
pub struct ShotSpec {
    pub seed: u64,
    pub planet: PlanetSize,
    pub resolution: usize,
    /// Camera position; `None` → spawn (x, z) and above the surface (y).
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub z: Option<f64>,
    /// Height above the surface when `y` is not given.
    pub above: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
    pub width: u32,
    pub height: u32,
    /// Horizontal load radius in cubes.
    pub distance: i32,
    /// Output file; `None` → a numbered file in the game's screenshot folder.
    pub out: Option<PathBuf>,
    pub software: bool,
    /// Also render with CPU culling and fail if GPU occlusion culling changes any pixel.
    pub verify_cull: bool,
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
            above: 24.0,
            yaw: 30.0,
            pitch: 15.0,
            fov: 70.0,
            width: 1280,
            height: 720,
            distance: 10,
            out: None,
            software: false,
            verify_cull: false,
        }
    }
}

impl ShotSpec {
    /// Parses `key=value` pairs separated by commas.
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let mut spec = Self::default();
        for kv in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (k, v) = kv
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("expected key=value, got {kv:?}"))?;
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
                "above" => spec.above = v.parse()?,
                "yaw" => spec.yaw = v.parse()?,
                "pitch" => spec.pitch = v.parse()?,
                "fov" => spec.fov = v.parse()?,
                "w" | "width" => spec.width = v.parse()?,
                "h" | "height" => spec.height = v.parse()?,
                "dist" | "distance" => spec.distance = v.parse()?,
                "out" => spec.out = Some(PathBuf::from(v)),
                "software" => spec.software = v.parse()?,
                "verify_cull" => spec.verify_cull = v.parse()?,
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
    let atlas = TextureArray::from_entries(&hearth_texgen::default_textures());
    let mut world: Option<(u64, PlanetSize, usize, LocalWorld)> = None;
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
        shoot(&ctx, &atlas, lw, spec, &out)?;
    }
    log::info!(
        "{} screenshot(s) in {:.2}s",
        specs.len(),
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}

fn shoot(
    ctx: &GpuContext,
    atlas: &TextureArray,
    lw: &mut LocalWorld,
    spec: &ShotSpec,
    out: &Path,
) -> anyhow::Result<()> {
    let (sx, sz) = match (spec.x, spec.z) {
        (Some(x), Some(z)) => (x, z),
        _ => {
            let (x, z) = lw.terrain().find_spawn(false);
            (x as f64 + 0.5, z as f64 + 0.5)
        }
    };
    let sy = spec.y.unwrap_or_else(|| lw.surface_y(sx, sz) + spec.above);
    let camera = Camera {
        pos: DVec3::new(sx, sy, sz),
        yaw: spec.yaw,
        pitch: spec.pitch,
        fov_y: spec.fov,
        near: 0.05,
    };
    log::info!("shot {} at {:.1}, {:.1}, {:.1}", out.display(), sx, sy, sz);
    let positions = lw.load_area(camera.pos, spec.distance, 2);
    let models = BlockModels::build(&lw.reg, atlas);
    let meshes = lw.mesh(&models, &positions, MeshOptions::default());
    let mut terrain = TerrainRenderer::new(ctx, atlas, OFFSCREEN_FORMAT, *lw.map.planet(), 4, 4);
    terrain.render_distance = spec.distance;
    terrain.vertical_distance = 64;
    for m in &meshes {
        terrain.upload(ctx, m);
    }
    let target = OffscreenTarget::new(ctx, spec.width, spec.height);
    let fog = Vec3::new(0.62, 0.76, 0.95);
    let radius = (spec.distance * 16) as f32;
    let params = FrameParams {
        fog_color: fog,
        fog_start: radius * 0.55,
        fog_end: radius * 0.95,
        ..FrameParams::default()
    };
    let clear = LinearColor {
        r: fog.x as f64,
        g: fog.y as f64,
        b: fog.z as f64,
        a: 1.0,
    };
    let size = (spec.width, spec.height);
    let frame = |terrain: &mut TerrainRenderer| {
        terrain.prepare(ctx, &camera, size, &params);
        render_terrain(ctx, terrain, &target.color_view, &target.depth.view, clear);
        target.read_rgba(ctx)
    };
    // Three frames: with GPU culling the first draws everything in phase 1, the second learns
    // which cubes are occluded, and the third draws only the survivors in phase 0.
    frame(&mut terrain);
    frame(&mut terrain);
    let pixels = frame(&mut terrain);
    if spec.verify_cull && terrain.uses_gpu_culling() {
        let counts = terrain.read_gpu_draw_counts(ctx).unwrap_or_default();
        terrain.gpu_culling = false;
        let reference = frame(&mut terrain);
        let cpu_draws = terrain.stats.draws;
        terrain.gpu_culling = true;
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
    write_png(out, spec.width, spec.height, &pixels)?;
    let s = terrain.stats;
    log::info!(
        "wrote {} ({} visible cubes, {} draws, {} quads, {:.1} MiB mesh memory)",
        out.display(),
        s.visible_cubes,
        s.draws,
        s.quads_drawn,
        (s.packed_bytes + s.general_bytes) as f64 / (1 << 20) as f64
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_specs_and_lists() {
        let s = ShotSpec::parse("seed=7, x=10.5, z=-3, yaw=90, w=640, h=360, out=a.png").unwrap();
        assert_eq!(s.seed, 7);
        assert_eq!(s.x, Some(10.5));
        assert_eq!(s.z, Some(-3.0));
        assert_eq!((s.width, s.height), (640, 360));
        assert_eq!(s.out, Some(PathBuf::from("a.png")));
        assert!(ShotSpec::parse("seed=x").is_err());
        assert!(ShotSpec::parse("colour=red").is_err());
        let list = ShotSpec::parse_list(
            "# suite
defaults: seed=3, w=320

yaw=0
yaw=180 # back
",
        )
        .unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|s| s.seed == 3 && s.width == 320));
        assert_eq!(list[1].yaw, 180.0);
    }
}

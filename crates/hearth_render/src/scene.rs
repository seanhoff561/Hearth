//! The frame: atmosphere lookup tables, opaque terrain (GPU-culled), distant LOD terrain, the
//! sky, translucent terrain, then tonemapping to the display. Lighting comes in physical units (lux) from the
//! caller (`hearth_env::sky`) and is pre-exposed here with an adapting eye.

use glam::{Mat3, Vec2, Vec3};
use hearth_math::Planet;

use crate::atlas::TextureArray;
use crate::camera::Camera;
use crate::gpu::GpuContext;
use crate::lod::LodRenderer;
use crate::post::PostProcess;
use crate::precip::{PrecipRenderer, Precipitation, SkyHeights};
use crate::profiler::GpuTimer;
use crate::sky::{SkyParams, SkyRenderer};
use crate::terrain::{FrameParams, TerrainRenderer, begin_pass};

/// Firelight illuminance at block-light level 15 (lux): a torch or campfire at arm's length.
pub const FIRE_LUX: f32 = 60.0;
/// Colour of firelight (about 1,900 K).
pub const FIRE_COLOR: Vec3 = Vec3::new(1.0, 0.62, 0.30);
/// The eye can't adapt below about full-moon light: darker scenes stay dark.
pub const ADAPTATION_FLOOR_LUX: f32 = 0.3;
/// Starlight and airglow that reach everything open to the sky (lux).
pub const NIGHT_FLOOR_LUX: f32 = 0.0005;
/// Full-moon luminance (cd/m²) for the moon disc.
pub const MOON_LUMINANCE: f32 = 2500.0;

/// The lighting environment of a frame, in physical units.
#[derive(Debug, Clone, Copy)]
pub struct Environment {
    pub sun_dir: Vec3,
    pub moon_dir: Vec3,
    pub moon_phase: f32,
    /// Direct sun and moon illuminance on a facing surface (lux, RGB).
    pub sun_lux: Vec3,
    pub moon_lux: Vec3,
    /// Sky irradiance on a horizontal surface (lux, RGB).
    pub sky_lux: Vec3,
    pub year_frac: f32,
    pub seconds: f32,
    pub wind: f32,
    pub star_rotation: Mat3,
    pub cloud_cover: f32,
    /// Cloud base above sea level (blocks).
    pub cloud_base: f32,
    pub cloud_offset: Vec2,
    /// Aerosol density multiplier (1 = clear).
    pub haze: f32,
    /// Block-light level at the camera (0..1), for adaptation to firelight.
    pub block_light_at_camera: f32,
    /// Aerial perspective on (off only for tests and comparisons).
    pub aerial_perspective: bool,
    /// Exposure compensation (1 = none).
    pub exposure_bias: f32,
    /// Rain or snow falling at the camera.
    pub precipitation: Precipitation,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            sun_dir: Vec3::new(0.3, 0.8, 0.5).normalize(),
            moon_dir: Vec3::new(-0.3, -0.8, -0.5).normalize(),
            moon_phase: 0.0,
            sun_lux: Vec3::new(95_000.0, 90_000.0, 80_000.0),
            moon_lux: Vec3::ZERO,
            sky_lux: Vec3::new(12_000.0, 16_000.0, 24_000.0),
            year_frac: 0.3,
            seconds: 0.0,
            wind: 1.0,
            star_rotation: Mat3::IDENTITY,
            cloud_cover: 0.0,
            cloud_base: 1500.0,
            cloud_offset: Vec2::ZERO,
            haze: 1.0,
            block_light_at_camera: 0.0,
            aerial_perspective: true,
            exposure_bias: 1.0,
            precipitation: Precipitation::default(),
        }
    }
}

impl Environment {
    /// Illuminance on a horizontal surface (lux, green channel).
    pub fn horizontal_lux(&self) -> f32 {
        self.sun_lux.y * self.sun_dir.y.max(0.0)
            + self.moon_lux.y * self.moon_dir.y.max(0.0)
            + self.sky_lux.y
    }
}

/// CPU time of the last `prepare` by part (milliseconds).
#[derive(Debug, Clone, Copy, Default)]
pub struct PrepareTimes {
    /// Visible cubes (cave culling, frustum), draw lists, translucent sorting, uploads.
    pub terrain_ms: f64,
    /// LOD tile frustum tests and origins.
    pub lod_ms: f64,
    /// Sky parameters and precipitation particles.
    pub sky_ms: f64,
}

pub struct SceneRenderer {
    pub sky: SkyRenderer,
    pub terrain: TerrainRenderer,
    /// Distant terrain beyond the full-detail area.
    pub lod: LodRenderer,
    /// LOD tiles to draw this frame (ids of the current selection).
    pub lod_show: Vec<u64>,
    /// Full-detail terrain area in world blocks (min x, min z, max x, max z): the LOD draws
    /// beyond it. `None` without LOD.
    pub near_area: Option<[f64; 4]>,
    /// Blocks per real metre of height (the world's vertical scale): the atmosphere's density
    /// by altitude and the planet's curvature follow it.
    pub vertical_scale: f32,
    pub precip: PrecipRenderer,
    pub post: PostProcess,
    /// Adapted illuminance (natural log of lux).
    adapted: Option<f32>,
    pub exposure: f32,
    night: f32,
    sky_params: Option<(SkyParams, glam::Mat4)>,
    /// Seconds since the last frame for the highlight metering (not finite = adapt instantly).
    meter_dt: f32,
    /// GPU pass timing (benchmark only).
    pub timer: Option<GpuTimer>,
    /// CPU time of the last `prepare`.
    pub cpu: PrepareTimes,
}

impl SceneRenderer {
    pub fn new(
        ctx: &GpuContext,
        atlas: &TextureArray,
        output_format: wgpu::TextureFormat,
        planet: Planet,
        mip_levels: u32,
        anisotropy: u16,
    ) -> Self {
        let sky = SkyRenderer::new(ctx);
        let terrain = TerrainRenderer::new(ctx, atlas, &sky, planet, mip_levels, anisotropy);
        let lod = LodRenderer::new(ctx, &terrain, planet, crate::post::HDR_FORMAT);
        Self {
            post: PostProcess::new(ctx, output_format),
            precip: PrecipRenderer::new(ctx),
            sky,
            terrain,
            lod,
            lod_show: Vec::new(),
            near_area: None,
            vertical_scale: 1.0,
            adapted: None,
            exposure: 1.0,
            night: 0.0,
            sky_params: None,
            meter_dt: f32::INFINITY,
            timer: None,
            cpu: PrepareTimes::default(),
        }
    }

    /// Forgets the eye's adaptation (the next frame adapts instantly).
    pub fn reset_adaptation(&mut self) {
        self.adapted = None;
    }

    /// Replaces the map of what covers the sky around the camera (hides rain and snow).
    pub fn set_sky_heights(&mut self, ctx: &GpuContext, h: &SkyHeights) {
        self.precip.set_heights(ctx, h);
    }

    /// Prepares a frame: adapts exposure over `dt` seconds (∞ = instantly), uploads terrain
    /// draws and lighting.
    pub fn prepare(
        &mut self,
        ctx: &GpuContext,
        camera: &Camera,
        size: (u32, u32),
        env: &Environment,
        dt: f32,
    ) {
        let fire = FIRE_LUX * env.block_light_at_camera * env.block_light_at_camera;
        let target = env
            .horizontal_lux()
            .max(fire)
            .max(ADAPTATION_FLOOR_LUX)
            .ln();
        let adapted = match self.adapted {
            Some(a) if dt.is_finite() => {
                // Brightening adapts in about a second, darkening more slowly.
                let rate = if target > a { 2.5 } else { 0.8 };
                a + (target - a) * (1.0 - (-dt * rate).exp())
            }
            _ => target,
        };
        self.meter_dt = if self.adapted.is_some() {
            dt
        } else {
            f32::INFINITY
        };
        self.adapted = Some(adapted);
        let lux_adapted = adapted.exp();
        self.exposure =
            std::f32::consts::PI / lux_adapted * adaptation_key(adapted) * env.exposure_bias;
        self.night = 1.0 - smoothstep(0.5f32.ln(), 20f32.ln(), adapted);
        let e = self.exposure;
        // The dominant direct light: the sun by day, the moon by night.
        let sun_h = env.sun_lux.y * env.sun_dir.y.max(0.0);
        let moon_h = env.moon_lux.y * env.moon_dir.y.max(0.0);
        let (direct, dir) = if sun_h >= moon_h || env.sun_dir.y > -0.02 {
            (env.sun_lux + env.moon_lux * 0.0, env.sun_dir)
        } else {
            (env.moon_lux, env.moon_dir)
        };
        // A thick cloud deck and falling rain or snow turn the whole sky, haze and distance
        // grey: the light of the cloud base, about the sky's irradiance over pi.
        let precip = env.precipitation.intensity.clamp(0.0, 1.0);
        let overcast_w = smoothstep(0.55, 1.0, env.cloud_cover).max(precip * 0.85);
        let overcast = (env.sky_lux * e / std::f32::consts::PI).extend(overcast_w);
        let params = FrameParams {
            direct_light: direct * e,
            light_dir: dir,
            sky_light: env.sky_lux * e,
            ambient_floor: NIGHT_FLOOR_LUX * e,
            block_light: FIRE_COLOR * FIRE_LUX * e,
            // Aerosol at sea level as in `atmosphere.wgsl` (with Rayleigh, about 70 km of
            // visibility in clear air), thickened by haze and humidity; rain and snow on top.
            haze_extinction: 4.44e-5 * env.haze.max(0.1),
            precip_extinction: precipitation_extinction(&env.precipitation),
            vertical_scale: self.vertical_scale,
            aerial_perspective: env.aerial_perspective,
            near_area: self.near_area,
            seconds: env.seconds,
            anim_ticks: env.seconds * 20.0,
            wind: env.wind,
            year_frac: env.year_frac,
            overcast,
        };
        let t0 = std::time::Instant::now();
        self.terrain.prepare(ctx, camera, size, &params);
        let t1 = std::time::Instant::now();
        let aspect = size.0.max(1) as f32 / size.1.max(1) as f32;
        let hzb = self.terrain.hzb().map(|(_, size, mips)| (size, mips));
        self.lod.prepare(
            ctx,
            camera,
            aspect,
            self.vertical_scale,
            &self.lod_show,
            hzb,
        );
        let t2 = std::time::Instant::now();
        let star_visibility = 1.0 - smoothstep(0.05, 3.0, env.sky_lux.y);
        let moon_trans = (env.moon_dir.y * 6.0).clamp(0.0, 1.0);
        let sky = SkyParams {
            sun_dir: env.sun_dir,
            sun_illuminance: hearth_sun_toa() * e,
            moon_dir: env.moon_dir,
            moon_illuminance: moon_toa(env.moon_phase) * e,
            moon_phase: env.moon_phase,
            altitude: camera.pos.y.max(0.0) as f32,
            haze: env.haze,
            seconds: env.seconds,
            star_rotation: env.star_rotation,
            star_visibility,
            cloud_cover: env.cloud_cover,
            cloud_height: (env.cloud_base - camera.pos.y as f32).max(0.0),
            cloud_offset: env.cloud_offset,
            exposure: e,
            night: self.night,
            moon_disc: MOON_LUMINANCE * e * moon_trans,
            direct: (env.sun_lux * env.sun_dir.y.max(0.0) + env.moon_lux * env.moon_dir.y.max(0.0))
                * e,
            ambient: env.sky_lux * e,
            overcast,
        };
        self.sky_params = Some((sky, camera.view_proj(aspect)));
        self.precip.prepare(
            ctx,
            camera,
            aspect,
            &env.precipitation,
            env.seconds,
            env.sky_lux * e,
            env.sun_lux * env.sun_dir.y.max(0.0) * e + env.moon_lux * env.moon_dir.y.max(0.0) * e,
        );
        let ms = |a: std::time::Instant, b: std::time::Instant| (b - a).as_secs_f64() * 1e3;
        self.cpu = PrepareTimes {
            terrain_ms: ms(t0, t1),
            lod_ms: ms(t1, t2),
            sky_ms: ms(t2, std::time::Instant::now()),
        };
    }

    /// Records the frame into `enc` and tonemaps into `output`.
    pub fn render(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        let mut timer = self.timer.take();
        let mark = |t: &mut Option<GpuTimer>, enc: &mut wgpu::CommandEncoder, l: &'static str| {
            if let Some(t) = t {
                t.mark(enc, l);
            }
        };
        if let Some(t) = &mut timer {
            t.begin_frame();
        }
        mark(&mut timer, enc, "start");
        if let Some((p, vp)) = self.sky_params {
            self.sky.update(ctx, enc, &p, vp);
        }
        mark(&mut timer, enc, "sky tables");
        let hdr = self.post.hdr_view(ctx, size).clone();
        self.terrain.render_opaque(
            ctx,
            enc,
            &hdr,
            depth,
            Some(wgpu::Color::BLACK),
            timer.as_mut(),
        );
        self.lod
            .cull(ctx, enc, self.terrain.hzb().map(|(view, _, _)| view));
        mark(&mut timer, enc, "lod cull");
        {
            let (_, bind0) = self.terrain.globals_bind();
            let mut pass = begin_pass(enc, &hdr, depth, None);
            self.lod.draw(&mut pass, bind0);
        }
        mark(&mut timer, enc, "lod terrain");
        {
            let mut pass = begin_pass(enc, &hdr, depth, None);
            self.sky.draw(&mut pass);
            self.terrain.draw_translucent(&mut pass);
            self.precip.draw(&mut pass);
        }
        mark(&mut timer, enc, "sky, translucent, rain");
        self.post.meter(ctx, enc, self.meter_dt);
        mark(&mut timer, enc, "metering");
        self.post.render(ctx, enc, output, 1.0, self.night);
        mark(&mut timer, enc, "tonemap");
        if let Some(t) = &mut timer {
            t.end_frame(enc, self.terrain.cull_counters());
        }
        self.timer = timer;
    }

    /// Call after the frame recorded by `render` was submitted (starts the timing readback).
    pub fn submitted(&mut self) {
        if let Some(t) = &mut self.timer {
            t.submitted();
        }
    }
}

/// How completely the eye compensates for the light it is adapted to (natural log of lux):
/// fully in daylight, only partly at dusk and by moonlight, so dim scenes look dim.
pub fn adaptation_key(ln_lux: f32) -> f32 {
    0.3 + 0.7 * smoothstep(ADAPTATION_FLOOR_LUX.ln(), 1000f32.ln(), ln_lux)
}

/// Extinction (per metre) by falling rain or snow, from the visibility they leave: about 4 km
/// in a heavy shower, well under a kilometre in heavy snow (Koschmieder: 3.9 / visibility).
fn precipitation_extinction(p: &Precipitation) -> f32 {
    let i = p.intensity.clamp(0.0, 1.0);
    let rain = 3.9 / 4_000.0;
    let snow = 3.9 / 600.0;
    i * (rain * p.rain + snow * (1.0 - p.rain))
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Solar illuminance at the top of the atmosphere (lux); matches `hearth_env::sky`.
fn hearth_sun_toa() -> f32 {
    128_000.0
}

/// Moon illuminance at the top of the atmosphere for a phase (0 new, 0.5 full), with the
/// opposition surge; matches `hearth_env::sky::moon_illuminance`.
fn moon_toa(phase: f32) -> f32 {
    let angle = ((phase - 0.5).abs() * 360.0).min(180.0);
    if angle >= 170.0 {
        return 0.0;
    }
    0.3 * 10f32.powf(-0.4 * (0.026 * angle + 4e-9 * angle.powi(4)))
}

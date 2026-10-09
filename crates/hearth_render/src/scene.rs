//! The frame: atmosphere lookup tables, the sun's shadow maps, opaque terrain (GPU-culled),
//! distant LOD terrain, the sky, translucent terrain, then tonemapping to the display. Lighting
//! comes in physical units (lux) from the caller (`hearth_env::sky`) and is pre-exposed here with
//! an adapting eye.

use glam::{Mat3, Vec2, Vec3};
use hearth_math::Planet;

use crate::atlas::TextureArray;
use crate::camera::Camera;
use crate::figure::FigureRenderer;
use crate::gpu::GpuContext;
use crate::lod::LodRenderer;
use crate::post::PostProcess;
use crate::precip::{PrecipRenderer, Precipitation, SkyHeights};
use crate::profiler::GpuTimer;
use crate::sky::{SkyParams, SkyRenderer};
use crate::terrain::{FrameParams, TerrainRenderer, begin_pass, begin_pass_read_depth};

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
    /// Real seconds since the world began at the normal pace of time, exact (the waves travel
    /// by it, Amendment P §8).
    pub real_seconds: f64,
    pub wind: f32,
    /// The direction the wind blows toward (world x, z) and its speed (m/s), for the waves.
    pub wind_dir: Vec2,
    pub wind_speed_m_s: f32,
    pub star_rotation: Mat3,
    pub cloud_cover: f32,
    /// Cloud base above sea level (blocks).
    pub cloud_base: f32,
    pub cloud_offset: Vec2,
    /// How far the clouds' smaller shapes have slid against their larger (m): the clouds change
    /// shape as they go (`CLOUD_CHURN_M_S`).
    pub cloud_churn: f32,
    /// How wet the ground's surface is from rain (0 dry … 1 wet through), drying by the
    /// weather: darker ground after a shower (S §4.2).
    pub wetness: f32,
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
            real_seconds: 0.0,
            wind: 1.0,
            wind_dir: Vec2::X,
            wind_speed_m_s: 4.0,
            star_rotation: Mat3::IDENTITY,
            cloud_cover: 0.0,
            cloud_base: 1500.0,
            cloud_offset: Vec2::ZERO,
            cloud_churn: 0.0,
            wetness: 0.0,
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
    /// Smoke over fires in the vegetation.
    pub smoke: crate::smoke::SmokeRenderer,
    /// Temporal anti-aliasing, when on.
    pub taa: Option<crate::taa::TaaRenderer>,
    /// This frame's jittered and unjittered view-projections and camera (for TAA).
    taa_frame: Option<(glam::Mat4, glam::Mat4, glam::DVec3)>,
    /// Bodies (the player's own, later people and animals), drawn with the opaque terrain.
    pub figures: FigureRenderer,
    /// People as sculpted bodies (E7).
    pub people: crate::body::PeopleRenderer,
    /// Trees and woody shrubs as meshes (S5).
    pub trees: crate::trees::TreeRenderer,
    /// The player's body's senses on the image.
    pub senses: crate::post::Senses,
    pub post: PostProcess,
    /// The thing looked at, outlined by its own shape (T §2.3), and what it is this frame.
    pub outline: crate::outline::OutlineRenderer,
    pub highlight: Option<crate::outline::Highlight>,
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
    /// Size of the rendered frame relative to the output (0.5–2): smaller frames are upscaled
    /// (FSR 1), larger ones filtered down.
    pub render_scale: f32,
    /// The depth target when the frame is rendered at another size than the output's.
    depth: Option<crate::offscreen::DepthTarget>,
    /// The camera under water this frame (from the map of water surfaces), and clip space back
    /// to camera-relative space, for the tonemap's view through the water.
    underwater: Option<crate::post::Underwater>,
    inv_view_proj: glam::Mat4,
}

/// The terrain textures' mip levels and anisotropic filtering, the game's and its tools'.
pub const MIP_LEVELS: u32 = 4;
pub const ANISOTROPY: u16 = 4;

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
        let figures = FigureRenderer::new(ctx, terrain.globals_bind().0);
        let people = crate::body::PeopleRenderer::new(ctx, terrain.globals_bind().0);
        let trees =
            crate::trees::TreeRenderer::new(ctx, terrain.globals_bind().0, crate::post::HDR_FORMAT);
        let outline =
            crate::outline::OutlineRenderer::new(ctx, terrain.globals_bind().0, output_format);
        Self {
            figures,
            people,
            trees,
            outline,
            highlight: None,
            senses: crate::post::Senses::default(),
            post: PostProcess::new(ctx, output_format),
            precip: PrecipRenderer::new(ctx),
            smoke: crate::smoke::SmokeRenderer::new(ctx),
            taa: None,
            taa_frame: None,
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
            render_scale: 1.0,
            depth: None,
            underwater: None,
            inv_view_proj: glam::Mat4::IDENTITY,
        }
    }

    /// The size the scene is rendered at for an output of `size` (the render scale applied).
    pub fn render_size(&self, size: (u32, u32)) -> (u32, u32) {
        let s = self.render_scale.clamp(0.5, 2.0);
        if (s - 1.0).abs() < 1e-3 {
            return size;
        }
        let scaled = |v: u32| ((v as f32 * s).round() as u32).max(1);
        (scaled(size.0), scaled(size.1))
    }

    /// Turns temporal anti-aliasing on or off.
    pub fn set_taa(&mut self, ctx: &GpuContext, on: bool) {
        match (on, self.taa.is_some()) {
            (true, false) => self.taa = Some(crate::taa::TaaRenderer::new(ctx)),
            (false, true) => self.taa = None,
            _ => {}
        }
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
        let output = size;
        let size = self.render_size(size);
        // With TAA, every pass sees the camera jittered by this frame's sub-pixel offset.
        let unjittered = *camera;
        let jittered;
        let camera = match &self.taa {
            Some(taa) => {
                let px = taa.jitter_px();
                jittered = Camera {
                    jitter: glam::Vec2::new(
                        px.x * 2.0 / size.0.max(1) as f32,
                        -px.y * 2.0 / size.1.max(1) as f32,
                    ),
                    ..unjittered
                };
                &jittered
            }
            None => camera,
        };
        // Under water: how deep the camera is (real metres) below the surface over its column.
        let below = self
            .terrain
            .water
            .surface_at(camera.pos.x.floor() as i32, camera.pos.z.floor() as i32)
            .map(|s| s as f64 - camera.pos.y)
            .filter(|&d| d > 0.0);
        let depth_m = below.map_or(0.0, |d| d as f32 / self.vertical_scale.max(1e-3));
        // Light falls off with depth (red first, blue deepest), and the eye adapts to it.
        let reach = Vec3::new(-0.35, -0.07, -0.045) * depth_m;
        let reach = Vec3::new(reach.x.exp(), reach.y.exp(), reach.z.exp());
        let fire = FIRE_LUX * env.block_light_at_camera * env.block_light_at_camera;
        let target = (env.horizontal_lux() * reach.y)
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
        // The water around an underwater camera scatters the light reaching its depth toward the
        // eye: blue-green, from the sky and the sun overhead.
        self.underwater = below.map(|d| {
            let down = env.sky_lux + env.sun_lux * env.sun_dir.y.max(0.0);
            crate::post::Underwater {
                surface_above: d as f32,
                inscatter: down * reach * e * Vec3::new(0.03, 0.12, 0.16) / std::f32::consts::PI,
            }
        });
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
            wetness: env.wetness,
        };
        let t0 = std::time::Instant::now();
        self.terrain.prepare(ctx, camera, size, &params);
        self.terrain.water.prepare(
            ctx,
            size,
            env.wind_dir,
            env.wind_speed_m_s,
            camera.near,
            env.real_seconds,
        );
        let t1 = std::time::Instant::now();
        let aspect = size.0.max(1) as f32 / size.1.max(1) as f32;
        self.inv_view_proj = camera.view_proj(aspect).inverse();
        self.taa_frame = self.taa.as_ref().map(|_| {
            (
                camera.view_proj(aspect),
                unjittered.view_proj(aspect),
                camera.pos,
            )
        });
        self.outline.prepare(
            ctx,
            self.highlight.as_ref(),
            unjittered.view_proj(aspect),
            self.inv_view_proj,
            size,
            output,
        );
        let hzb = self.terrain.hzb().map(|(_, size, mips)| (size, mips));
        self.lod.prepare(
            ctx,
            camera,
            aspect,
            self.vertical_scale,
            &self.lod_show,
            hzb,
        );
        self.lod.prepare_shadow(
            ctx,
            camera.pos,
            self.vertical_scale,
            &self.terrain.shadows,
            &self.lod_show,
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
            seconds: env.real_seconds.rem_euclid(600.0) as f32,
            turbulence: (env.wind_speed_m_s / 12.0).clamp(0.0, 1.0),
            star_rotation: env.star_rotation,
            star_visibility,
            cloud_cover: env.cloud_cover,
            cloud_height: (env.cloud_base - camera.pos.y as f32).max(0.0),
            cloud_offset: env.cloud_offset,
            cloud_churn: env.cloud_churn,
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
            env.real_seconds,
            env.sky_lux * e,
            env.sun_lux * env.sun_dir.y.max(0.0) * e + env.moon_lux * env.moon_dir.y.max(0.0) * e,
        );
        self.smoke.prepare(
            ctx,
            camera,
            aspect,
            Vec3::new(env.wind_dir.x, 0.0, env.wind_dir.y) * env.wind_speed_m_s,
            env.seconds,
            &crate::smoke::SmokeLight {
                ambient: env.sky_lux * e,
                direct: env.sun_lux * env.sun_dir.y.max(0.0) * e
                    + env.moon_lux * env.moon_dir.y.max(0.0) * e,
                fire: FIRE_COLOR * FIRE_LUX * e,
                haze: env.sky_lux * e / std::f32::consts::PI * Vec3::new(0.85, 0.92, 1.0),
                haze_extinction: 4.44e-5 * env.haze.max(0.1),
            },
        );
        let ms = |a: std::time::Instant, b: std::time::Instant| (b - a).as_secs_f64() * 1e3;
        self.cpu = PrepareTimes {
            terrain_ms: ms(t0, t1),
            lod_ms: ms(t1, t2),
            sky_ms: ms(t2, std::time::Instant::now()),
        };
    }

    /// Records the frame into `enc` and tonemaps into `output` (`size` pixels). `depth` is used
    /// at a render scale of 1; at others the scene renders at its own size and depth.
    pub fn render(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        let render = self.render_size(size);
        if render != size
            && self
                .depth
                .as_ref()
                .is_none_or(|d| (d.width, d.height) != render)
        {
            self.depth = Some(crate::offscreen::DepthTarget::new(ctx, render.0, render.1));
        }
        let depth = match &self.depth {
            Some(d) if render != size => &d.view,
            _ => depth,
        };
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
        // The sun's shadow maps (R1a), before anything that receives them: the full-detail
        // world and the bodies in it into the near cascades, the distant terrain into the far.
        for i in self.terrain.shadows.due() {
            let mut pass = self.terrain.begin_shadow_pass(enc, i);
            let bind0 = self.terrain.shadow_bind(i);
            if i < crate::shadow::NEAR_CASCADES {
                self.terrain.draw_shadow(&mut pass, i);
                self.trees.draw_shadow(&mut pass, bind0);
                self.figures.draw_shadow(&mut pass, bind0);
                self.people.draw_shadow(&mut pass, bind0);
            } else {
                self.lod.draw_shadow(&mut pass, bind0);
            }
        }
        mark(&mut timer, enc, "shadows");
        let hdr = self.post.hdr_view(ctx, render).clone();
        self.terrain.render_opaque(
            ctx,
            enc,
            &hdr,
            depth,
            Some(wgpu::Color::BLACK),
            timer.as_mut(),
        );
        if self.figures.count() > 0 || self.people.count() > 0 || self.trees.stats.instances > 0 {
            let (_, bind0) = self.terrain.globals_bind();
            let mut pass = begin_pass(enc, &hdr, depth, None);
            self.trees.draw(&mut pass, bind0);
            self.figures.draw(&mut pass, bind0);
            self.people.draw(&mut pass, bind0);
        }
        self.lod
            .cull(ctx, enc, self.terrain.hzb().map(|(view, _, _)| view));
        mark(&mut timer, enc, "lod cull");
        {
            let (_, bind0) = self.terrain.globals_bind();
            let mut pass = begin_pass(enc, &hdr, depth, None);
            self.lod
                .draw(&mut pass, bind0, self.terrain.water.waves_bind());
        }
        mark(&mut timer, enc, "lod terrain");
        {
            let mut pass = begin_pass(enc, &hdr, depth, None);
            self.sky.draw(&mut pass);
        }
        mark(&mut timer, enc, "sky");
        // The water sees the scene drawn so far through itself (copied, unless nothing
        // translucent is drawn), and its depth (read-only in the pass).
        let reads = self.terrain.water.reads_scene();
        if reads
            && let Some(mut rect) = self.terrain.stats.translucent_rect
            && let Some(color) = self.post.hdr_texture()
        {
            // Screen-space reflections come from above the water on screen: copy up to the top.
            if self.terrain.water.quality == crate::water::WaterQuality::High {
                rect[1] = 0.0;
            }
            self.terrain.water.copy_scene(enc, color, rect);
        }
        mark(&mut timer, enc, "water's scene copy");
        let water = self.terrain.water.bind(ctx, depth).clone();
        {
            let mut pass = if reads {
                begin_pass_read_depth(enc, &hdr, depth)
            } else {
                begin_pass(enc, &hdr, depth, None)
            };
            self.terrain.draw_translucent(&mut pass, &water);
            self.precip.draw(&mut pass);
            self.smoke.draw(&mut pass);
        }
        mark(&mut timer, enc, "translucent, rain");
        if let (Some(taa), Some((vp, unjittered, cam)), Some(tex)) =
            (&mut self.taa, self.taa_frame, self.post.hdr_texture())
        {
            taa.resolve(ctx, enc, tex, &hdr, depth, render, vp, unjittered, cam);
            mark(&mut timer, enc, "taa");
        }
        self.post.meter(ctx, enc, self.meter_dt);
        mark(&mut timer, enc, "metering");
        self.post.render(
            ctx,
            enc,
            output,
            size,
            1.0,
            self.night,
            depth,
            self.underwater,
            self.inv_view_proj,
            &self.senses,
            timer.as_mut(),
        );
        if self.outline.active() {
            self.outline.render(
                ctx,
                enc,
                self.terrain.globals_bind().1,
                depth,
                output,
                render,
            );
            mark(&mut timer, enc, "outline");
        }
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

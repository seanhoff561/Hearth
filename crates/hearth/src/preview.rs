//! Free-flying world preview: streams terrain around a camera and renders it with the sky,
//! seasons and weather of a running world clock. This is the development view until the
//! player and the client/server split exist.

use std::sync::Arc;

use glam::DVec3;
use hearth_content::schema::Season;
use hearth_core::options::Options;
use hearth_env::Calendar;
use hearth_input::{InputState, builtin};
use hearth_math::{Planet, PlanetSize};
use hearth_render::atlas::TextureArray;
use hearth_render::camera::Camera;
use hearth_render::scene::SceneRenderer;
use hearth_render::{FrameTargets, GpuContext};

use crate::environment::{EnvOverrides, EnvSampler};
use crate::lod_stream::LodStream;
use crate::streamer::{StreamEvent, StreamTarget, StreamWorld, Streamer};

/// Meshes uploaded per frame at most (keeps frame times smooth while streaming).
const UPLOADS_PER_FRAME: usize = 256;
/// LOD tiles uploaded per frame at most.
const LOD_UPLOADS_PER_FRAME: usize = 24;

pub struct Preview {
    streamer: Streamer,
    scene: Option<SceneRenderer>,
    env: Option<EnvSampler>,
    atlas: Arc<TextureArray>,
    color_format: wgpu::TextureFormat,
    planet: Option<Planet>,
    pub camera: Camera,
    /// Base flying speed in blocks per second.
    speed: f64,
    radius: i32,
    vertical: i32,
    /// Distant terrain, once the world is ready; LOD distance in chunks (0 = off).
    lod: Option<LodStream>,
    lod_distance: u32,
    /// Vertical error of distant terrain allowed on screen (pixels).
    lod_error_px: f64,
    /// Size of the rendered frame relative to the window (upscaled or filtered down).
    render_scale: f32,
    water_quality: hearth_render::water::WaterQuality,
    vertical_scale: f32,
    /// World clock (20 ticks per second of play) and calendar.
    pub ticks: u64,
    pub calendar: Calendar,
    /// Extra ticks per second of play (time warp for looking at days and seasons).
    pub time_warp: f64,
    tick_remainder: f64,
    starting_season: Season,
    pub status: String,
}

impl Preview {
    /// `content` provides the calendar (`time.ron`) and the generated blocks' textures; the
    /// built-in defaults when `None`.
    pub fn new(
        world: StreamWorld,
        options: &Options,
        color_format: wgpu::TextureFormat,
        content: Option<&hearth_content::Content>,
    ) -> Self {
        let atlas = Arc::new(TextureArray::from_entries(&hearth_texgen::textures_for(
            content,
        )));
        let time = content.map(|c| &c.time);
        let radius = options.video.render_distance as i32;
        let vertical = options.video.vertical_render_distance as i32;
        let (calendar, starting_season) = match time {
            Some(cfg) => (Calendar::from_config(cfg), cfg.starting_season),
            None => (Calendar::new(48, 8, 23.44), Season::Spring),
        };
        let streamer = Streamer::start(
            world,
            atlas.clone(),
            StreamTarget {
                center: DVec3::ZERO,
                radius,
                vertical,
                year_frac: calendar.at(0).year_frac,
            },
        );
        Self {
            streamer,
            scene: None,
            env: None,
            atlas,
            color_format,
            planet: None,
            camera: Camera {
                fov_y: options.video.fov,
                ..Camera::default()
            },
            speed: 12.0,
            radius,
            vertical,
            lod: None,
            lod_distance: options.video.lod_distance,
            lod_error_px: options.video.lod_error_px(),
            render_scale: options.video.render_scale,
            water_quality: options.video.shader.water.into(),
            vertical_scale: 1.0,
            ticks: 0,
            calendar,
            time_warp: 0.0,
            tick_remainder: 0.0,
            starting_season,
            status: "generating planet".into(),
        }
    }

    pub fn default_world(seed: u64, cache_dir: Option<std::path::PathBuf>) -> StreamWorld {
        StreamWorld {
            seed,
            planet: PlanetSize::Standard,
            cache_dir,
        }
    }

    /// Jumps the clock forward (or back) by a number of game hours.
    pub fn skip_hours(&mut self, hours: f64) {
        let dt = hours / 24.0 * self.calendar.ticks_per_day();
        self.ticks = (self.ticks as f64 + dt).max(0.0) as u64;
    }

    /// Moves the camera from input; `look` is the accumulated mouse motion when captured.
    pub fn update(
        &mut self,
        dt: f64,
        input: &mut InputState,
        look: Option<(f64, f64)>,
        sensitivity: f32,
    ) {
        // The clock runs at 20 ticks per second, plus any time warp.
        let advance = dt * (20.0 + self.time_warp) + self.tick_remainder;
        self.ticks += advance.floor() as u64;
        self.tick_remainder = advance.fract();
        if let Some((dx, dy)) = look {
            let f = sensitivity as f64 * 0.6 + 0.2;
            let deg_per_count = f * f * f * 8.0 * 0.15;
            self.camera.yaw = (self.camera.yaw + (dx * deg_per_count) as f32).rem_euclid(360.0);
            self.camera.pitch =
                (self.camera.pitch + (dy * deg_per_count) as f32).clamp(-90.0, 90.0);
        }
        let steps = input.take_scroll_steps(1.0, true);
        if steps != 0 {
            self.speed = (self.speed * 1.25f64.powi(steps)).clamp(1.0, 2000.0);
        }
        let (sy, cy) = (self.camera.yaw as f64).to_radians().sin_cos();
        let forward = DVec3::new(-sy, 0.0, cy);
        let right = DVec3::new(-cy, 0.0, -sy);
        let mut dir = DVec3::ZERO;
        if input.is_active(builtin::FORWARD) {
            dir += forward;
        }
        if input.is_active(builtin::BACK) {
            dir -= forward;
        }
        if input.is_active(builtin::RIGHT) {
            dir += right;
        }
        if input.is_active(builtin::LEFT) {
            dir -= right;
        }
        if input.is_active(builtin::JUMP) {
            dir.y += 1.0;
        }
        if input.is_active(builtin::SNEAK) {
            dir.y -= 1.0;
        }
        if dir != DVec3::ZERO {
            let boost = if input.is_active(builtin::SPRINT) {
                4.0
            } else {
                1.0
            };
            self.camera.pos += dir.normalize() * self.speed * boost * dt;
            if let Some(p) = &self.planet {
                self.camera.pos.x = p.wrap_xf(self.camera.pos.x);
            }
        }
    }

    /// Applies finished streaming work; call once per frame before rendering.
    pub fn pump(&mut self, ctx: &GpuContext) {
        let mut uploaded = 0;
        while uploaded < UPLOADS_PER_FRAME {
            let Some(ev) = self.streamer.poll() else {
                break;
            };
            match ev {
                StreamEvent::Ready {
                    planet,
                    spawn,
                    grid,
                    generator,
                    lod,
                    vertical_scale,
                } => {
                    self.lod = Some(LodStream::new(
                        generator,
                        lod,
                        self.lod_distance,
                        vertical_scale as f64,
                        self.lod_error_px,
                    ));
                    self.vertical_scale = vertical_scale;
                    self.planet = Some(planet);
                    self.camera.pos = spawn;
                    // Start the world in the morning, local time, at the spawn.
                    self.calendar = self.calendar.start_at(
                        self.starting_season,
                        planet.latitude(spawn.z) < 0.0,
                        0.33,
                        planet.solar_time_offset(spawn.x),
                    );
                    let mut scene =
                        SceneRenderer::new(ctx, &self.atlas, self.color_format, planet, 4, 4);
                    scene.terrain.render_distance = self.radius;
                    scene.terrain.vertical_distance = self.vertical;
                    scene.render_scale = self.render_scale;
                    scene.terrain.water.quality = self.water_quality;
                    self.scene = Some(scene);
                    self.env = Some(EnvSampler::new(grid, self.calendar));
                    self.status = "streaming".into();
                }
                StreamEvent::Mesh(m) => {
                    if let Some(s) = &mut self.scene {
                        s.terrain.upload(ctx, &m);
                    }
                    uploaded += 1;
                }
                StreamEvent::Unload(p) => {
                    if let Some(s) = &mut self.scene {
                        s.terrain.remove(p);
                    }
                }
                StreamEvent::SkyHeights(h) => {
                    if let Some(s) = &mut self.scene {
                        s.set_sky_heights(ctx, &h);
                    }
                }
                StreamEvent::Failed(e) => {
                    log::error!("terrain streaming failed: {e}");
                    self.status = format!("streaming failed: {e}");
                }
            }
        }
        self.streamer.set_target(StreamTarget {
            center: self.camera.pos,
            radius: self.radius,
            vertical: self.vertical,
            year_frac: self.calendar.at(self.ticks).year_frac,
        });
        let near = self.near_area();
        if let (Some(lod), Some(scene), Some(planet)) =
            (&mut self.lod, &mut self.scene, &self.planet)
        {
            let forward = self.camera.forward().as_dvec3();
            lod.update(planet, self.camera.pos, forward, near, &mut scene.lod);
            lod.pump(ctx, &mut scene.lod, LOD_UPLOADS_PER_FRAME);
        }
    }

    /// The full-detail area around the camera (world blocks), one cube inside the streamed
    /// radius so the LOD covers cubes still on their way.
    fn near_area(&self) -> [f64; 4] {
        let c = hearth_math::CubePos::containing(self.camera.pos);
        let r = (self.radius - 1).max(1);
        [
            ((c.x - r + 1) * 16) as f64,
            ((c.z - r + 1) * 16) as f64,
            ((c.x + r) * 16) as f64,
            ((c.z + r) * 16) as f64,
        ]
    }

    /// Records the frame.
    pub fn render(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        targets: FrameTargets<'_>,
        dt: f32,
    ) {
        let near = self.near_area();
        if let (Some(lod), Some(scene)) = (&mut self.lod, &self.scene) {
            // Distant terrain is detailed for the pixels rendered.
            lod.set_view(scene.render_size(targets.size).1, self.camera.fov_y);
        }
        let (Some(scene), Some(env)) = (&mut self.scene, &mut self.env) else {
            // Nothing to draw yet: just clear.
            let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: targets.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.06,
                            b: 0.08,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            return;
        };
        env.calendar = self.calendar;
        let moment = self.calendar.at(self.ticks);
        let (e, _) = env.sample(&moment, self.camera.pos, 0.0, EnvOverrides::default());
        scene.vertical_scale = self.vertical_scale;
        match &self.lod {
            Some(lod) if self.lod_distance > 0 => {
                scene.near_area = Some(near);
                scene.lod_show.clear();
                scene.lod_show.extend_from_slice(lod.show());
            }
            _ => scene.near_area = None,
        }
        scene.prepare(ctx, &self.camera, targets.size, &e, dt);
        scene.render(ctx, enc, targets.color, targets.depth, targets.size);
    }

    /// One-line status for the window title.
    pub fn title_status(&self, fps: f64) -> String {
        let p = self.camera.pos;
        match (&self.scene, &self.planet) {
            (Some(s), Some(planet)) => {
                let st = s.terrain.stats;
                let m = self.calendar.at(self.ticks);
                let local = m.local_time(planet.solar_time_offset(p.x)) * 24.0;
                let southern = planet.latitude(p.z) < 0.0;
                let day_of_season =
                    (m.season_progress() * self.calendar.days_per_season as f64) as u32 + 1;
                let (wanted, pending) = self.lod.as_ref().map_or((0, 0), |l| l.progress(&s.lod));
                format!(
                    "{fps:.0} fps | {:.0} {:.0} {:.0} (lat {:.1}°) | {:?} day {} {:02}:{:02} | {} cubes, {} visible{} | LOD {}/{} drawn, {} queued | {:.0} b/s",
                    p.x,
                    p.y,
                    p.z,
                    planet.latitude_deg(p.z),
                    m.season(southern),
                    day_of_season,
                    local as u32,
                    (local.fract() * 60.0) as u32,
                    st.meshes,
                    st.visible_cubes,
                    if st.gpu_culling { ", GPU culled" } else { "" },
                    s.lod.stats.drawn,
                    wanted,
                    pending,
                    self.speed
                )
            }
            _ => self.status.clone(),
        }
    }
}

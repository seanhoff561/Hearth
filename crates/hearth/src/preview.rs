//! Free-flying world preview: streams terrain around a camera and renders it in the window.
//! This is the development view until the player and client/server split exist.

use std::sync::Arc;
use std::time::Instant;

use glam::{DVec3, Vec3};
use hearth_core::options::Options;
use hearth_input::{InputState, builtin};
use hearth_math::{Planet, PlanetSize};
use hearth_render::atlas::TextureArray;
use hearth_render::camera::Camera;
use hearth_render::terrain::{FrameParams, TerrainRenderer};
use hearth_render::{FrameTargets, GpuContext};

use crate::streamer::{StreamEvent, StreamTarget, StreamWorld, Streamer};

/// Meshes uploaded per frame at most (keeps frame times smooth while streaming).
const UPLOADS_PER_FRAME: usize = 256;

pub struct Preview {
    streamer: Streamer,
    terrain: Option<TerrainRenderer>,
    atlas: Arc<TextureArray>,
    color_format: wgpu::TextureFormat,
    planet: Option<Planet>,
    pub camera: Camera,
    /// Base flying speed in blocks per second.
    speed: f64,
    radius: i32,
    vertical: i32,
    start: Instant,
    pub status: String,
}

impl Preview {
    pub fn new(world: StreamWorld, options: &Options, color_format: wgpu::TextureFormat) -> Self {
        let atlas = Arc::new(TextureArray::from_entries(
            &hearth_texgen::default_textures(),
        ));
        let radius = options.video.render_distance as i32;
        let vertical = options.video.vertical_render_distance as i32;
        let streamer = Streamer::start(
            world,
            atlas.clone(),
            StreamTarget {
                center: DVec3::ZERO,
                radius,
                vertical,
            },
        );
        Self {
            streamer,
            terrain: None,
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
            start: Instant::now(),
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

    /// Moves the camera from input; `look` is the accumulated mouse motion when captured.
    pub fn update(
        &mut self,
        dt: f64,
        input: &mut InputState,
        look: Option<(f64, f64)>,
        sensitivity: f32,
    ) {
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
                StreamEvent::Ready { planet, spawn } => {
                    self.planet = Some(planet);
                    self.camera.pos = spawn;
                    let mut t =
                        TerrainRenderer::new(ctx, &self.atlas, self.color_format, planet, 4, 4);
                    t.render_distance = self.radius;
                    t.vertical_distance = self.vertical;
                    self.terrain = Some(t);
                    self.status = "streaming".into();
                }
                StreamEvent::Mesh(m) => {
                    if let Some(t) = &mut self.terrain {
                        t.upload(ctx, &m);
                    }
                    uploaded += 1;
                }
                StreamEvent::Unload(p) => {
                    if let Some(t) = &mut self.terrain {
                        t.remove(p);
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
        });
    }

    /// Records the frame.
    pub fn render(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        targets: FrameTargets<'_>,
    ) {
        let fog = Vec3::new(0.62, 0.76, 0.95);
        let clear = wgpu::Color {
            r: fog.x as f64,
            g: fog.y as f64,
            b: fog.z as f64,
            a: 1.0,
        };
        let Some(terrain) = &mut self.terrain else {
            // Nothing to draw yet: just clear.
            let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: targets.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
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
        let radius = (self.radius * 16) as f32;
        let params = FrameParams {
            fog_color: fog,
            fog_start: radius * 0.6,
            fog_end: radius * 0.95,
            seconds: self.start.elapsed().as_secs_f32(),
            anim_ticks: self.start.elapsed().as_secs_f32() * 20.0,
            ..FrameParams::default()
        };
        terrain.prepare(ctx, &self.camera, targets.size, &params);
        terrain.render(ctx, enc, targets.color, targets.depth, Some(clear));
    }

    /// One-line status for the window title.
    pub fn title_status(&self, fps: f64) -> String {
        let p = self.camera.pos;
        match &self.terrain {
            Some(t) => {
                let s = t.stats;
                format!(
                    "{fps:.0} fps | {:.0} {:.0} {:.0} | {} cubes, {} visible{} | {:.0} b/s",
                    p.x,
                    p.y,
                    p.z,
                    s.meshes,
                    s.visible_cubes,
                    if s.gpu_culling { ", GPU culled" } else { "" },
                    self.speed
                )
            }
            None => self.status.clone(),
        }
    }
}

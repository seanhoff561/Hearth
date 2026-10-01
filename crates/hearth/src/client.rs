//! The game's client (V2-3): it starts the world's server, renders what it sends, and plays the
//! player — input becomes movement, predicted every frame against the client's own copy of the
//! nearby blocks and reported to the server twenty times a second; the camera looks from the
//! player's eyes. A free camera (F3+N) flies anywhere for development.

use std::sync::Arc;

use glam::{DVec2, DVec3};
use hearth_core::options::Options;
use hearth_env::Calendar;
use hearth_input::{InputState, builtin};
use hearth_math::Planet;
use hearth_physics::{Ability, BlockWorld, Gait, Intent, Mover};
use hearth_protocol::{BodyView, Moved, ToClient, ToServer};
use hearth_render::atlas::TextureArray;
use hearth_render::camera::Camera;
use hearth_render::scene::SceneRenderer;
use hearth_render::ui::UiRenderer;
use hearth_render::{FrameTargets, GpuContext};
use hearth_ui::{DrawList, Font, Lang, Rgba};
use hearth_world::{BlockRegistry, CubeMap};

use crate::environment::{EnvOverrides, EnvSampler};
use crate::globe::GlobePicker;
use crate::lod_stream::LodStream;
use crate::server::{Server, View, WorldSpec};

/// Meshes uploaded per frame at most (keeps frame times smooth while streaming).
const UPLOADS_PER_FRAME: usize = 256;
/// LOD tiles uploaded per frame at most.
const LOD_UPLOADS_PER_FRAME: usize = 24;
/// Seconds between movement reports to the server.
const REPORT_S: f64 = 0.05;
/// A second press of the sprint key within this long breaks into a sprint.
const DOUBLE_TAP_S: f64 = 0.35;

/// Where the camera is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    /// From the player's eyes.
    Body,
    /// Flying freely (development).
    Free,
}

/// What the client knows once the world is ready.
struct World {
    planet: Planet,
    terrain: Arc<hearth_worldgen::Terrain>,
    reg: Arc<BlockRegistry>,
    /// The blocks near the player, for movement.
    mirror: CubeMap,
}

/// Movement since the last report.
#[derive(Default)]
struct Pending {
    landed: Option<f64>,
    straining: bool,
}

pub struct Client {
    server: Server,
    scene: Option<SceneRenderer>,
    env: Option<EnvSampler>,
    atlas: Arc<TextureArray>,
    color_format: wgpu::TextureFormat,
    world: Option<World>,
    /// The planet as a globe to pick a place on (the world-map key).
    pub globe: GlobePicker,
    pub camera: Camera,
    pub mode: CameraMode,
    /// The player's body as the server last told it, and the player's movement here.
    body: Option<BodyView>,
    pub mover: Mover,
    pending: Pending,
    since_report: f64,
    last_report: Option<hearth_physics::Report>,
    /// The camera's height (smoothed over steps up).
    eye_y: f64,
    /// Sprint key state: whether it was down last frame, when it was last released.
    sprint_was: bool,
    sprint_released: Option<f64>,
    sprinting: bool,
    clock_s: f64,
    /// Free camera speed (blocks per second).
    speed: f64,
    radius: i32,
    vertical: i32,
    lod: Option<LodStream>,
    lod_distance: u32,
    lod_error_px: f64,
    render_scale: f32,
    water_quality: hearth_render::water::WaterQuality,
    vertical_scale: f32,
    /// World clock (from the server, advanced locally between its messages) and calendar.
    pub ticks: u64,
    tick_frac: f64,
    pub calendar: Calendar,
    /// Extra ticks per second of play (asked of the server).
    pub time_warp: f64,
    pub status: String,
    /// The interface: its renderer, font, words and this frame's drawing.
    ui: Option<UiRenderer>,
    font: Font,
    pub lang: Lang,
    draw: DrawList,
    gui_scale: u32,
    /// The debug screen (F3), and the frame rate it shows.
    pub debug_overlay: bool,
    pub fps: f64,
}

impl Client {
    /// Starts the world's server; `content` provides the generated blocks' textures.
    pub fn new(
        world: WorldSpec,
        options: &Options,
        color_format: wgpu::TextureFormat,
        content: Option<&hearth_content::Content>,
    ) -> Self {
        let atlas = Arc::new(TextureArray::from_entries(&hearth_texgen::textures_for(
            content,
        )));
        let radius = options.video.render_distance as i32;
        let vertical = options.video.vertical_render_distance as i32;
        let server = Server::start(world, atlas.clone(), View { radius, vertical });
        let calendar = match content.map(|c| &c.time) {
            Some(cfg) => Calendar::from_config(cfg),
            None => Calendar::new(48, 8, 23.44),
        };
        Self {
            server,
            scene: None,
            env: None,
            atlas,
            color_format,
            world: None,
            globe: GlobePicker::default(),
            camera: Camera {
                fov_y: options.video.fov,
                ..Camera::default()
            },
            mode: CameraMode::Body,
            body: None,
            mover: Mover::new(DVec3::ZERO),
            pending: Pending::default(),
            since_report: 0.0,
            last_report: None,
            eye_y: 0.0,
            sprint_was: false,
            sprint_released: None,
            sprinting: false,
            clock_s: 0.0,
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
            tick_frac: 0.0,
            calendar,
            time_warp: 0.0,
            status: "generating planet".into(),
            ui: None,
            font: Font::new(),
            lang: Lang::load(
                &[crate::scene::data_pack_dir().join("hearth")],
                &options.language,
            ),
            draw: DrawList::new(1),
            gui_scale: options.video.gui_scale,
            debug_overlay: false,
            fps: 0.0,
        }
    }

    /// Shows or hides the debug screen.
    pub fn toggle_debug(&mut self) {
        self.debug_overlay = !self.debug_overlay;
    }

    /// The default world: its name, seed and where it is saved.
    pub fn default_world(
        name: &str,
        seed: u64,
        cache_dir: Option<std::path::PathBuf>,
        saves_dir: Option<std::path::PathBuf>,
    ) -> WorldSpec {
        WorldSpec {
            name: name.to_owned(),
            seed,
            planet: hearth_math::PlanetSize::Standard,
            cache_dir,
            saves_dir,
        }
    }

    /// Jumps the clock forward (or back) by a number of game hours.
    pub fn skip_hours(&mut self, hours: f64) {
        self.server.send(ToServer::SkipHours(hours));
    }

    /// Sets the time warp (extra ticks per second of play).
    pub fn set_time_warp(&mut self, warp: f64) {
        self.time_warp = warp;
        self.server.send(ToServer::TimeWarp(warp));
    }

    /// Switches between the player's eyes and the free camera.
    pub fn toggle_free_camera(&mut self) {
        self.mode = match self.mode {
            CameraMode::Body => CameraMode::Free,
            CameraMode::Free => {
                self.camera.pos = self.mover.eye();
                CameraMode::Body
            }
        };
    }

    /// Asks to live on as a new person (after death).
    pub fn respawn(&mut self) {
        if self.body.as_ref().is_some_and(|b| b.dead.is_some()) {
            self.server.send(ToServer::Respawn);
        }
    }

    pub fn dead(&self) -> bool {
        self.body.as_ref().is_some_and(|b| b.dead.is_some())
    }

    /// Opens or closes the globe; whether it is open. It opens once the world is ready,
    /// centred on the player.
    pub fn toggle_globe(&mut self) -> bool {
        if self.globe.open {
            self.globe.close();
        } else if let Some(w) = &self.world {
            let (lat, lon) = crate::globe::lat_lon(&w.planet, self.camera.pos);
            self.globe.open(&w.terrain, lat, lon);
        }
        self.globe.open
    }

    /// The mouse button over the globe went down or up: a click puts the player at the place
    /// under it (closing the globe).
    pub fn globe_button(&mut self, pressed: bool) {
        if let Some((lat, lon)) = self.globe.button(pressed)
            && let Some(w) = &self.world
        {
            let (x, z) = crate::globe::world_xz(&w.planet, lat, lon);
            log::info!("going to {}", crate::globe::describe(&w.terrain, lat, lon));
            self.server
                .send(ToServer::Place(DVec3::new(x as f64, 0.0, z as f64)));
            self.globe.close();
        }
    }

    /// What the body allows the player's movement now.
    fn ability(&self) -> Ability {
        self.body
            .as_ref()
            .map_or_else(Ability::human, |b| b.ability)
    }

    /// Moves the player (or the free camera) from input; `look` is the accumulated mouse motion
    /// when captured.
    pub fn update(
        &mut self,
        dt: f64,
        input: &mut InputState,
        look: Option<(f64, f64)>,
        sensitivity: f32,
    ) {
        self.clock_s += dt;
        // The clock runs at 20 ticks per second (plus warp) between the server's messages.
        self.tick_frac += dt * (20.0 + self.time_warp);
        if let Some((dx, dy)) = look {
            let f = sensitivity as f64 * 0.6 + 0.2;
            let deg_per_count = f * f * f * 8.0 * 0.15;
            self.camera.yaw = (self.camera.yaw + (dx * deg_per_count) as f32).rem_euclid(360.0);
            self.camera.pitch =
                (self.camera.pitch + (dy * deg_per_count) as f32).clamp(-90.0, 90.0);
        }
        if self.globe.open {
            // The wheel zooms the globe; nothing moves.
            let steps = input.take_scroll_steps(1.0, true);
            self.globe.view.zoom_by(steps);
            return;
        }
        let (sy, cy) = (self.camera.yaw as f64).to_radians().sin_cos();
        let forward = DVec2::new(-sy, cy);
        let right = DVec2::new(-cy, -sy);
        let mut wish = DVec2::ZERO;
        if input.is_active(builtin::FORWARD) {
            wish += forward;
        }
        if input.is_active(builtin::BACK) {
            wish -= forward;
        }
        if input.is_active(builtin::RIGHT) {
            wish += right;
        }
        if input.is_active(builtin::LEFT) {
            wish -= right;
        }
        match self.mode {
            CameraMode::Free => self.fly(dt, input, wish),
            CameraMode::Body => self.walk(dt, input, wish),
        }
    }

    fn fly(&mut self, dt: f64, input: &mut InputState, wish: DVec2) {
        let steps = input.take_scroll_steps(1.0, true);
        if steps != 0 {
            self.speed = (self.speed * 1.25f64.powi(steps)).clamp(1.0, 2000.0);
        }
        let mut dir = DVec3::new(wish.x, 0.0, wish.y);
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
            if let Some(w) = &self.world {
                self.camera.pos.x = w.planet.wrap_xf(self.camera.pos.x);
            }
        }
    }

    fn walk(&mut self, dt: f64, input: &mut InputState, wish: DVec2) {
        // The sprint key jogs; pressed twice quickly, it sprints until let go.
        let sprint_key = input.is_active(builtin::SPRINT);
        if sprint_key && !self.sprint_was {
            self.sprinting = self
                .sprint_released
                .is_some_and(|t| self.clock_s - t < DOUBLE_TAP_S);
        }
        if !sprint_key && self.sprint_was {
            self.sprint_released = Some(self.clock_s);
            self.sprinting = false;
        }
        self.sprint_was = sprint_key;
        let gait = if !sprint_key {
            Gait::Walk
        } else if self.sprinting {
            Gait::Sprint
        } else {
            Gait::Jog
        };
        let Some(w) = &self.world else {
            return;
        };
        let alive = !self.dead()
            && self
                .body
                .as_ref()
                .is_none_or(|b| !b.asleep && b.status.effects.conscious);
        let intent = if alive {
            let crouch = input.is_active(builtin::SNEAK);
            Intent {
                wish: wish.try_normalize().unwrap_or(DVec2::ZERO),
                gait,
                jump: input.is_active(builtin::JUMP),
                crouch,
                crawl: input.is_active(builtin::CRAWL),
                descend: crouch,
            }
        } else {
            // Limp: in water the body sinks.
            Intent {
                descend: true,
                ..Intent::default()
            }
        };
        let ability = self.ability();
        let terrain = BlockWorld {
            map: &w.mirror,
            reg: &w.reg,
        };
        let report = hearth_physics::step(&terrain, &mut self.mover, &intent, &ability, dt);
        self.pending.landed = match (self.pending.landed, report.landed) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        self.pending.straining |= report.straining;
        self.last_report = Some(report);
        self.since_report += dt;
        if self.since_report >= REPORT_S {
            self.since_report = 0.0;
            self.server.send(ToServer::Moved(Moved {
                mover: self.mover,
                landed: self.pending.landed.take(),
                motion: report.motion,
                speed: report.speed,
                straining: std::mem::take(&mut self.pending.straining),
                immersion: report.immersion,
                airless_s: report.airless_s,
            }));
        }
        // The eyes, smoothed over steps up; falls and crouches follow at once.
        let eye = self.mover.eye();
        if eye.y > self.eye_y && eye.y - self.eye_y < 1.2 {
            self.eye_y += (eye.y - self.eye_y) * (1.0 - (-dt * 14.0).exp());
        } else {
            self.eye_y = eye.y;
        }
        self.camera.pos = DVec3::new(eye.x, self.eye_y, eye.z);
    }

    /// Applies the server's messages; call once per frame before rendering.
    pub fn pump(&mut self, ctx: &GpuContext) {
        let mut uploaded = 0;
        while uploaded < UPLOADS_PER_FRAME {
            let Some(ev) = self.server.poll() else {
                break;
            };
            match ev {
                ToClient::Ready(r) => {
                    let r = *r;
                    let planet = r.planet;
                    self.lod = Some(LodStream::new(
                        r.generator.clone(),
                        r.lod,
                        self.lod_distance,
                        r.vertical_scale as f64,
                        self.lod_error_px,
                    ));
                    self.vertical_scale = r.vertical_scale;
                    self.calendar = r.calendar;
                    self.ticks = r.ticks;
                    self.tick_frac = 0.0;
                    self.mover = r.player;
                    self.eye_y = self.mover.eye().y;
                    self.camera.pos = self.mover.eye();
                    self.world = Some(World {
                        planet,
                        terrain: r.generator.terrain.clone(),
                        reg: r.reg,
                        mirror: CubeMap::new(planet),
                    });
                    let mut scene =
                        SceneRenderer::new(ctx, &self.atlas, self.color_format, planet, 4, 4);
                    scene.terrain.render_distance = self.radius;
                    scene.terrain.vertical_distance = self.vertical;
                    scene.render_scale = self.render_scale;
                    scene.terrain.water.quality = self.water_quality;
                    self.scene = Some(scene);
                    self.env = Some(EnvSampler::new(r.grid, self.calendar));
                    self.status = "streaming".into();
                }
                ToClient::Cube(p, cube) => {
                    if let Some(w) = &mut self.world {
                        w.mirror.insert_cube(p, cube, &w.reg);
                    }
                }
                ToClient::Mesh(m) => {
                    if let Some(s) = &mut self.scene {
                        s.terrain.upload(ctx, &m);
                    }
                    uploaded += 1;
                }
                ToClient::Unload(p) => {
                    if let Some(s) = &mut self.scene {
                        s.terrain.remove(p);
                    }
                    if let Some(w) = &mut self.world {
                        w.mirror.remove_cube(p);
                    }
                }
                ToClient::Heights(h, water) => {
                    if let Some(s) = &mut self.scene {
                        s.set_sky_heights(ctx, &h);
                        s.terrain.water.set_heights(ctx, *water);
                    }
                }
                ToClient::Clock(t) => {
                    self.ticks = t;
                    self.tick_frac = 0.0;
                }
                ToClient::Body(b) => self.body = Some(*b),
                ToClient::Placed(m) => {
                    self.mover = m;
                    self.eye_y = m.eye().y;
                    self.mode = CameraMode::Body;
                }
                ToClient::Saved => {}
                ToClient::Failed(e) => {
                    log::error!("the world failed: {e}");
                    self.status = format!("the world failed: {e}");
                }
            }
        }
        let near = self.near_area();
        if let (Some(lod), Some(scene), Some(w)) = (&mut self.lod, &mut self.scene, &self.world) {
            let forward = self.camera.forward().as_dvec3();
            lod.update(&w.planet, self.camera.pos, forward, near, &mut scene.lod);
            lod.pump(ctx, &mut scene.lod, LOD_UPLOADS_PER_FRAME);
        }
    }

    /// The full-detail area around the camera (world blocks), one cube inside the streamed
    /// radius so the LOD covers cubes still on their way.
    fn near_area(&self) -> [f64; 4] {
        let c = hearth_math::CubePos::containing(self.mover.pos);
        let r = (self.radius - 1).max(1);
        [
            ((c.x - r + 1) * 16) as f64,
            ((c.z - r + 1) * 16) as f64,
            ((c.x + r) * 16) as f64,
            ((c.z + r) * 16) as f64,
        ]
    }

    /// The world clock now, between the server's ticks.
    fn now_ticks(&self) -> u64 {
        self.ticks + self.tick_frac.min(40.0) as u64
    }

    /// Records the frame.
    pub fn render(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        targets: FrameTargets<'_>,
        dt: f32,
    ) {
        if self.globe.open
            && let Some(w) = &self.world
        {
            let camera = crate::globe::lat_lon(&w.planet, self.mover.pos);
            self.globe.render(
                ctx,
                enc,
                targets.color,
                targets.size,
                self.color_format,
                camera,
            );
            self.draw_interface(ctx, enc, &targets);
            return;
        }
        let near = self.near_area();
        if let (Some(lod), Some(scene)) = (&mut self.lod, &self.scene) {
            // Distant terrain is detailed for the pixels rendered.
            lod.set_view(scene.render_size(targets.size).1, self.camera.fov_y);
        }
        let ticks = self.now_ticks();
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
        let moment = self.calendar.at(ticks);
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
        self.draw_interface(ctx, enc, &targets);
    }

    /// The interface over the frame: what death says, and the debug screen.
    fn draw_interface(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        targets: &FrameTargets<'_>,
    ) {
        let scale = hearth_ui::gui_scale(self.gui_scale, targets.size.1);
        let mut draw = std::mem::take(&mut self.draw);
        draw.clear(scale);
        let (w, h) = (
            targets.size.0 as f32 / scale as f32,
            targets.size.1 as f32 / scale as f32,
        );
        if let Some(b) = &self.body
            && let Some(death) = &b.dead
        {
            let what = death_words(&self.lang, death);
            let lines = [what, self.lang.get("body.death.live_on").to_owned()];
            draw.rect(0.0, h * 0.4 - 6.0, w, 34.0, Rgba([0, 0, 0, 140]));
            for (k, line) in lines.iter().enumerate() {
                let lw = self.font.width(line) as f32;
                draw.text_shadowed(
                    &self.font,
                    line,
                    ((w - lw) / 2.0).round(),
                    (h * 0.4 + 12.0 * k as f32).round(),
                    Rgba::rgb(240, 220, 200),
                );
            }
        }
        if self.debug_overlay {
            let lines = self.debug_lines();
            let width = lines.iter().map(|l| self.font.width(l)).max().unwrap_or(0) as f32;
            let lh = hearth_ui::font::LINE as f32;
            draw.rect(
                1.0,
                1.0,
                width + 4.0,
                lines.len() as f32 * lh + 3.0,
                Rgba([0, 0, 0, 110]),
            );
            for (k, line) in lines.iter().enumerate() {
                draw.text_shadowed(&self.font, line, 3.0, 3.0 + k as f32 * lh, Rgba::WHITE);
            }
        }
        if !draw.vertices.is_empty() {
            let font = &self.font;
            let ui = self
                .ui
                .get_or_insert_with(|| UiRenderer::new(ctx, self.color_format, &font.pixels));
            ui.draw(ctx, enc, targets.color, targets.size, &draw);
        }
        self.draw = draw;
    }

    /// The debug screen's lines.
    fn debug_lines(&self) -> Vec<String> {
        let l = &self.lang;
        let mut out = vec![format!(
            "{} {} · {:.0} fps",
            l.get("game.title"),
            hearth_core::GAME_VERSION,
            self.fps
        )];
        let Some(w) = &self.world else {
            out.push(self.status.clone());
            return out;
        };
        let p = self.camera.pos;
        let m = self.calendar.at(self.now_ticks());
        let local = m.local_time(w.planet.solar_time_offset(p.x)) * 24.0;
        let southern = w.planet.latitude(p.z) < 0.0;
        let day = (m.season_progress() * self.calendar.days_per_season as f64) as u32 + 1;
        let season = format!("{:?}", m.season(southern)).to_lowercase();
        out.push(l.format(
            "debug.position",
            &[
                ("x", &format!("{:.1}", p.x)),
                ("y", &format!("{:.1}", p.y)),
                ("z", &format!("{:.1}", p.z)),
                ("lat", &format!("{:.2}", w.planet.latitude_deg(p.z))),
                ("lon", &format!("{:.2}", w.planet.longitude_deg(p.x))),
            ],
        ));
        out.push(l.format(
            "debug.time",
            &[
                ("season", l.get(&format!("season.{season}"))),
                ("day", &day.to_string()),
                (
                    "time",
                    &format!("{:02}:{:02}", local as u32, (local.fract() * 60.0) as u32),
                ),
            ],
        ));
        if let Some(b) = &self.body {
            let e = &b.exposure;
            let rain = if e.rain_mm_h > 0.05 {
                l.format(
                    "debug.weather.rain",
                    &[("rain", &format!("{:.1}", e.rain_mm_h))],
                )
            } else {
                String::new()
            };
            let shelter = if e.radiant_w_m2 == 0.0 && e.sky_c_offset == 0.0 {
                l.get("debug.weather.sheltered").to_owned()
            } else {
                String::new()
            };
            out.push(l.format(
                "debug.weather",
                &[
                    ("air", &format!("{:.1}", e.air_c)),
                    ("humidity", &format!("{:.0}", e.humidity * 100.0)),
                    ("wind", &format!("{:.1}", e.wind_m_s)),
                    ("rain", &rain),
                    ("shelter", &shelter),
                ],
            ));
            let s = &b.status;
            out.push(l.format(
                "debug.body",
                &[
                    ("hunger", l.get(s.hunger.key())),
                    ("thirst", l.get(s.thirst.key())),
                    ("warmth", l.get(s.warmth.key())),
                    ("tiredness", l.get(s.tiredness.key())),
                    ("core", &format!("{:.2}", s.core_c)),
                    ("skin", &format!("{:.1}", s.skin_c)),
                    ("stamina", &format!("{:.0}", s.stamina * 100.0)),
                ],
            ));
            if !b.injuries.is_empty() {
                let list: Vec<String> = b
                    .injuries
                    .iter()
                    .map(|i| {
                        format!(
                            "{} ({:?}, {:.0}% healed)",
                            i.id.rsplit(':').next().unwrap_or(&i.id),
                            i.region,
                            i.healed * 100.0
                        )
                    })
                    .collect();
                out.push(l.format("debug.injuries", &[("list", &list.join(", "))]));
            }
            if !b.illnesses.is_empty() {
                out.push(l.format("debug.illnesses", &[("list", &b.illnesses.join(", "))]));
            }
        }
        if self.mode == CameraMode::Free {
            out.push(l.format(
                "debug.free_camera",
                &[("speed", &format!("{:.0}", self.speed))],
            ));
        } else {
            let r = self.last_report.unwrap_or_default();
            out.push(l.format(
                "debug.movement",
                &[
                    ("mode", l.get("debug.mode.body")),
                    ("stance", &format!("{:?}", self.mover.stance).to_lowercase()),
                    (
                        "ground",
                        if self.mover.on_ground {
                            " · on the ground"
                        } else {
                            ""
                        },
                    ),
                    (
                        "water",
                        &if r.immersion > 0.0 {
                            format!(" · {:.0}% in water", r.immersion * 100.0)
                        } else {
                            String::new()
                        },
                    ),
                ],
            ));
        }
        if let Some(s) = &self.scene {
            let st = s.terrain.stats;
            let (wanted, pending) = self.lod.as_ref().map_or((0, 0), |l| l.progress(&s.lod));
            let _ = wanted;
            out.push(l.format(
                "debug.terrain",
                &[
                    ("cubes", &st.meshes.to_string()),
                    ("visible", &st.visible_cubes.to_string()),
                    ("lod_drawn", &s.lod.stats.drawn.to_string()),
                    ("lod_queued", &pending.to_string()),
                ],
            ));
        }
        out
    }

    /// One-line status for the window title.
    pub fn title_status(&self, fps: f64) -> String {
        if self.globe.open
            && let Some(w) = &self.world
        {
            let place = self.globe.hovered().map_or_else(
                || "point at a place".to_owned(),
                |(lat, lon)| crate::globe::describe(&w.terrain, lat, lon),
            );
            return format!(
                "{place} | click to go there, drag to turn, wheel to zoom, M or Esc to close"
            );
        }
        let Some(w) = &self.world else {
            return self.status.clone();
        };
        let p = self.camera.pos;
        let m = self.calendar.at(self.now_ticks());
        let local = m.local_time(w.planet.solar_time_offset(p.x)) * 24.0;
        let southern = w.planet.latitude(p.z) < 0.0;
        let day_of_season = (m.season_progress() * self.calendar.days_per_season as f64) as u32 + 1;
        let place = format!(
            "{fps:.0} fps | lat {:.1}° | {:?} day {} {:02}:{:02}",
            w.planet.latitude_deg(p.z),
            m.season(southern),
            day_of_season,
            local as u32,
            (local.fract() * 60.0) as u32,
        );
        match (&self.body, self.mode) {
            (_, CameraMode::Free) => format!(
                "{place} | free camera {:.0} {:.0} {:.0}, {:.0} b/s (F3+N back to the body)",
                p.x, p.y, p.z, self.speed
            ),
            (Some(b), _) if b.dead.is_some() => format!(
                "{place} | dead ({:?}) — press Space to live on as a new person",
                b.dead.as_ref().expect("checked")
            ),
            (Some(b), _) => {
                let s = &b.status;
                let words = |k: &str| k.rsplit('.').next().unwrap_or(k).replace('_', " ");
                let hurt: Vec<String> = b
                    .injuries
                    .iter()
                    .map(|i| {
                        format!(
                            "{} ({:?})",
                            i.id.rsplit(':').next().unwrap_or(&i.id),
                            i.region
                        )
                    })
                    .collect();
                let e = &b.exposure;
                format!(
                    "{place} | {:.0} °C, wind {:.0} m/s{} | {}, {}, {}, {} | core {:.1} °C | stamina {:.0}%{}{}{}",
                    e.air_c,
                    e.wind_m_s,
                    if e.rain_mm_h > 0.1 {
                        format!(", rain {:.1} mm/h", e.rain_mm_h)
                    } else {
                        String::new()
                    },
                    words(s.hunger.key()),
                    words(s.thirst.key()),
                    words(s.warmth.key()),
                    words(s.tiredness.key()),
                    s.core_c,
                    s.stamina * 100.0,
                    if hurt.is_empty() {
                        String::new()
                    } else {
                        format!(" | {}", hurt.join(", "))
                    },
                    if b.illnesses.is_empty() {
                        String::new()
                    } else {
                        format!(" | ill: {}", b.illnesses.join(", "))
                    },
                    if b.asleep { " | asleep" } else { "" },
                )
            }
            (None, _) => place,
        }
    }
}

/// How a body died, in the player's words.
fn death_words(l: &Lang, d: &hearth_body::Death) -> String {
    use hearth_body::Death;
    let key = match d {
        Death::Hypothermia => "body.death.hypothermia",
        Death::HeatStroke => "body.death.heat_stroke",
        Death::Dehydration => "body.death.dehydration",
        Death::Starvation => "body.death.starvation",
        Death::BloodLoss => "body.death.blood_loss",
        Death::Drowning => "body.death.drowning",
        Death::Illness(id) => {
            let name = id.rsplit(':').next().unwrap_or(id).replace('_', " ");
            return l.format("body.death.illness", &[("illness", &name)]);
        }
        Death::Injury(cause) => {
            return l.format("body.death.injury", &[("cause", cause)]);
        }
    };
    let words = l.get(key);
    let mut c = words.chars();
    match c.next() {
        Some(first) => format!("You {}{}.", first.to_lowercase(), c.as_str()),
        None => String::new(),
    }
}

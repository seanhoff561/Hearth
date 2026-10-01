//! The game's client (V2-3): it starts the world's server, renders what it sends, and plays the
//! player — input becomes movement, predicted every frame against the client's own copy of the
//! nearby blocks and reported to the server twenty times a second; the camera looks from the
//! player's eyes. A free camera (F3+N) flies anywhere for development.

use std::sync::Arc;

use glam::{Affine3A, DVec2, DVec3, Quat};
use hearth_character::{Activity, Drive, Figure, FigureInstance, Pose, Show};
use hearth_core::options::Options;
use hearth_env::Calendar;
use hearth_input::{InputState, builtin};
use hearth_math::Planet;
use hearth_physics::{Ability, BlockWorld, Gait, Intent, Motion, Mover, Stance};
use hearth_protocol::{AimAt, BodyView, Moved, ToClient, ToServer};
use hearth_render::atlas::TextureArray;
use hearth_render::camera::Camera;
use hearth_render::scene::SceneRenderer;
use hearth_render::{FrameTargets, GpuContext};
use hearth_ui::{Lang, Rgba, Ui};
use hearth_world::{BlockRegistry, CubeMap};

use crate::crafting_ui::{Crafting, Do, News, Seen};
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

/// Where the camera looks from in the body: the eyes, behind the shoulders, or facing them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Perspective {
    #[default]
    First,
    Behind,
    Front,
}

/// What the eyes rest on within reach.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Aim {
    /// A thing lying in the world.
    Item(u64),
    /// A block (a plant, water, a hearth, the ground): where, whether its top face is looked
    /// at, and the point the eyes rest on.
    Block {
        pos: hearth_math::BlockPos,
        top: bool,
        at: DVec3,
    },
}

/// How far the hands reach from the eyes (m).
const REACH_M: f64 = 2.6;
/// Seconds to draw a thing from where it hangs into the hand.
const DRAW_S: f64 = 0.5;
/// Quick slots (keys 1–6): the attachment points, in order.
const QUICK_SLOTS: usize = 6;

/// Where a ray from `from` along `dir` enters the box `lo`–`hi` (distance), if it does.
fn ray_box(from: DVec3, dir: DVec3, lo: DVec3, hi: DVec3) -> Option<f64> {
    let mut near = 0.0f64;
    let mut far = f64::INFINITY;
    for a in 0..3 {
        let (o, d, l, h) = (from[a], dir[a], lo[a], hi[a]);
        if d.abs() < 1e-9 {
            if o < l || o > h {
                return None;
            }
            continue;
        }
        let (t0, t1) = ((l - o) / d, (h - o) / d);
        near = near.max(t0.min(t1));
        far = far.min(t0.max(t1));
        if near > far {
            return None;
        }
    }
    Some(near)
}

/// How far a third-person camera stands from the eyes (m).
const THIRD_PERSON_M: f64 = 3.5;

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
    /// The debug screen (F3), and the frame rate it shows.
    pub debug_overlay: bool,
    pub fps: f64,
    /// What the player hears, the wind and rain where they stand (m/s, mm/h of water), whether
    /// the world is paused, and whether to caption the sounds.
    pub hearing: crate::hearing::Hearing,
    weather: (f32, f32),
    paused: bool,
    captions: bool,
    /// The player's person, their pose this frame, which way the body faces (degrees, as the
    /// camera's yaw), and the boxes drawn.
    figure: Option<Figure>,
    pose: Option<Pose>,
    body_yaw: f32,
    pub perspective: Perspective,
    view_bobbing: bool,
    figure_boxes: Vec<FigureInstance>,
    /// The eyelids (0 open, 1 shut): shut asleep or unconscious, slow to open on waking.
    eyes_shut: f32,
    /// Why the player last woke, and how long ago (s).
    woke: Option<(hearth_body::Wake, f64)>,
    /// Where the heart and the breath are in their cycles (for the pulse at the edges of
    /// sight and the breath's fog).
    heart_phase: f64,
    breath_phase: f64,
    reduce_motion: bool,
    guided_hud: bool,
    /// The Body panel (B) open, and the body's definitions it names injuries from.
    pub body_panel: bool,
    body_cfg: Option<Arc<hearth_body::BodyConfig>>,
    /// What death means in this world, and the life's tale if it has ended.
    pub death_rules: hearth_save::DeathRules,
    pub ended: Option<hearth_protocol::LifeSummary>,
    /// The kinds of things, and what the player carries (as the server last said).
    pub items: Option<Arc<hearth_items::Items>>,
    pub carry: hearth_items::Carry,
    /// The things lying nearby, what the eyes rest on, and where a dragged thing is.
    pub world_items: Vec<hearth_items::WorldItem>,
    pub aim: Option<Aim>,
    dragged_at: Option<DVec3>,
    drag_was: bool,
    /// A thing being drawn into the right hand from where it hangs, and how long the reach
    /// has taken; where the right hand's thing came from (to put it back).
    drawing: Option<(hearth_items::Root, f64)>,
    drawn_from: Option<hearth_items::Root>,
    /// The quick-choice wheel (hold Q): open, and where the pointer leans.
    pub radial: Option<DVec2>,
    /// Making and knowing: what can be done, the work under way, what is known.
    pub crafting: Option<Crafting>,
    /// Knapping by hand asked for (the app opens its screen).
    pub knap_request: Option<crate::knapping_ui::KnapScreen>,
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
            debug_overlay: false,
            fps: 0.0,
            hearing: crate::hearing::Hearing::default(),
            weather: (0.0, 0.0),
            paused: false,
            captions: options.sound.subtitles,
            figure: None,
            pose: None,
            body_yaw: 0.0,
            perspective: Perspective::First,
            view_bobbing: options.video.view_bobbing,
            figure_boxes: Vec::new(),
            eyes_shut: 0.0,
            woke: None,
            heart_phase: 0.0,
            breath_phase: 0.0,
            reduce_motion: options.accessibility.reduce_motion,
            guided_hud: options.accessibility.guided_hud,
            body_panel: false,
            body_cfg: None,
            death_rules: hearth_save::DeathRules::default(),
            ended: None,
            items: None,
            carry: hearth_items::Carry::default(),
            world_items: Vec::new(),
            aim: None,
            dragged_at: None,
            drag_was: false,
            drawing: None,
            drawn_from: None,
            radial: None,
            crafting: None,
            knap_request: None,
        }
    }

    /// The places the quick slots draw from: each worn garment's attachment points in order.
    pub fn quick_places(&self) -> Vec<hearth_items::Root> {
        let mut out = Vec::new();
        for (i, w) in self.carry.worn.iter().enumerate() {
            for p in 0..w.hung.len() {
                out.push(hearth_items::Root::Hung(i, p));
            }
        }
        out.truncate(QUICK_SLOTS);
        out
    }

    /// Quick slot `n` (0-based): draws what hangs there into the right hand, after a moment,
    /// or puts the drawn thing back where it came from.
    fn quick(&mut self, n: usize) {
        use hearth_items::{Hand, Path, Root, Target};
        let Some(&place) = self.quick_places().get(n) else {
            return;
        };
        let right = Path::at(Root::Hand(Hand::Right));
        if self.carry.right.is_some() && self.drawn_from == Some(place) {
            // Back where it hangs.
            self.shift(right, None, Target::Root(place));
            self.drawn_from = None;
            return;
        }
        if self.carry.get(&Path::at(place)).is_some() {
            self.drawing = Some((place, 0.0));
        }
    }

    /// The reach for a thing being drawn: when done, what the hand held is stowed and the
    /// thing comes into it.
    fn reach(&mut self, dt: f64) {
        use hearth_items::{Hand, Path, Root, Target};
        let Some((place, t)) = &mut self.drawing else {
            return;
        };
        *t += dt;
        if *t < DRAW_S {
            return;
        }
        let place = *place;
        self.drawing = None;
        let right = Path::at(Root::Hand(Hand::Right));
        if self.carry.right.is_some() {
            match self.drawn_from {
                Some(from) => self.shift(right.clone(), None, Target::Root(from)),
                None => self.shift(right.clone(), None, Target::Stow),
            }
        }
        self.shift(Path::at(place), None, Target::Root(Root::Hand(Hand::Right)));
        self.drawn_from = Some(place);
    }

    /// The quick-choice wheel: the places and the one the pointer leans toward.
    pub fn radial_choice(&self) -> Option<usize> {
        let v = self.radial?;
        let n = self.quick_places().len();
        if n == 0 || v.length() < 20.0 {
            return None;
        }
        // Slots around the circle clockwise from the top.
        let angle = v.x.atan2(-v.y).rem_euclid(std::f64::consts::TAU);
        Some(((angle / std::f64::consts::TAU * n as f64 + 0.5) as usize) % n)
    }

    /// What the eyes rest on within reach: a thing lying there, or a block (plants and water
    /// included).
    fn find_aim(&self) -> Option<Aim> {
        let w = self.world.as_ref()?;
        let items = self.items.as_ref()?;
        let eye = self.camera.pos;
        let dir = self.camera.forward().as_dvec3();
        let mut best: Option<(f64, Aim)> = None;
        for wi in &self.world_items {
            let Some(k) = wi.stack.kind(items) else {
                continue;
            };
            let p = DVec3::from_array(wi.pos);
            if (p - eye).length() > REACH_M + 2.0 {
                continue;
            }
            let r = (0.5 * k.size_m[0].max(k.size_m[2]) as f64).max(0.08);
            let h = (k.size_m[1] as f64).max(0.06);
            let lo = p - DVec3::new(r, 0.0, r);
            let hi = p + DVec3::new(r, h, r);
            if let Some(t) = ray_box(eye, dir, lo, hi)
                && t <= REACH_M
                && best.as_ref().is_none_or(|b| t < b.0)
            {
                best = Some((t, Aim::Item(wi.id)));
            }
        }
        let mut t = 0.0;
        let mut prev = eye;
        while t <= REACH_M {
            let p = eye + dir * t;
            let bp = hearth_math::BlockPos::containing(p);
            if let Some(s) = w.mirror.block(bp)
                && !s.is_air()
            {
                let local = p - DVec3::new(bp.x as f64, bp.y as f64, bp.z as f64);
                let def = &w.reg.block_of(s).def;
                let shape = w.reg.outline_shape(s);
                let hit_box = shape.boxes.iter().find(|b| {
                    local.x >= b.min.x
                        && local.x <= b.max.x
                        && local.y >= b.min.y
                        && local.y <= b.max.y
                        && local.z >= b.min.z
                        && local.z <= b.max.z
                });
                let top_y = match (def.fluid.is_some(), hit_box) {
                    (true, _) => Some(0.85),
                    (false, Some(b)) => Some(b.max.y),
                    _ => None,
                };
                if let Some(top_y) = top_y {
                    if best.as_ref().is_none_or(|b| t < b.0) {
                        let top = prev.y >= bp.y as f64 + top_y - 1e-3;
                        best = Some((
                            t,
                            Aim::Block {
                                pos: bp,
                                top,
                                at: p,
                            },
                        ));
                    }
                    break;
                }
            }
            prev = p;
            t += 0.03;
        }
        best.map(|(_, a)| a)
    }

    /// What the server is told the player looks at.
    pub fn aim_at(&self) -> AimAt {
        match self.aim {
            Some(Aim::Item(id)) => AimAt::Thing(id),
            Some(Aim::Block { pos, top, .. }) => AimAt::Block { pos, top },
            None => AimAt::Nothing,
        }
    }

    /// The weather and place where the player is, as the client sees them.
    fn surroundings(&self) -> hearth_craft::Surroundings {
        let e = self.body.as_ref().map(|b| &b.exposure);
        let moment = self.calendar.at(self.ticks);
        let feet = self.mover.pos;
        let mut water_near = false;
        let mut sheltered = false;
        if let Some(w) = &self.world {
            let base = hearth_math::BlockPos::containing(feet);
            'scan: for dy in -2..=1 {
                for dz in -2..=2 {
                    for dx in -2..=2 {
                        let p = hearth_math::BlockPos::new(base.x + dx, base.y + dy, base.z + dz);
                        if w.mirror
                            .block(p)
                            .is_some_and(|s| w.reg.block_of(s).def.fluid.is_some())
                        {
                            water_near = true;
                            break 'scan;
                        }
                    }
                }
            }
            sheltered = w
                .mirror
                .sky_top(base.x, base.z)
                .is_some_and(|top| top as f64 > feet.y + 2.0);
        }
        let southern = self
            .world
            .as_ref()
            .is_some_and(|w| w.planet.latitude(feet.z) < 0.0);
        hearth_craft::Surroundings {
            raining: e.is_some_and(|e| e.rain_mm_h > 0.1),
            humidity: e.map_or(0.5, |e| e.humidity),
            daylight: self
                .env
                .as_ref()
                .is_some_and(|env| env.sun_up(&moment, feet)),
            air_c: e.map_or(15.0, |e| e.air_c),
            sheltered,
            water_near,
            near: Vec::new(),
            season: Some(moment.season(southern)),
        }
    }

    /// Draws up the list of what can be done, a few times a second.
    fn refresh_offers(&mut self) {
        if !self.crafting.as_mut().is_some_and(|c| c.due()) {
            return;
        }
        let around = self.surroundings();
        let aim = self.aim_at();
        let day_s = self.calendar.ticks_per_day() / 20.0;
        let year_s = day_s * 4.0 * self.calendar.days_per_season as f64;
        let (Some(w), Some(items), Some(c)) = (&self.world, &self.items, &mut self.crafting) else {
            return;
        };
        let seen = Seen {
            reg: &w.reg,
            mirror: &w.mirror,
            items,
            carry: &self.carry,
            world_items: &self.world_items,
            aim,
            feet: self.mover.pos,
            around,
            day_s,
            year_s,
        };
        c.refresh(&seen);
    }

    /// Does what was chosen.
    fn act(&mut self, d: Do) {
        let aim = self.aim_at();
        let m = match d {
            Do::Process(process) => ToServer::Act {
                process,
                aim,
                hand: None,
            },
            Do::Eat(p) => ToServer::Eat(p),
            Do::Drink(f) => ToServer::Drink(f),
            Do::Fill(skin) => ToServer::Fill { skin, aim },
        };
        self.server.send(m);
    }

    /// Knapping done by hand (or left to habit) does its process.
    pub fn act_by_hand(&mut self, process: String, aim: AimAt, hand: Option<f32>) {
        self.server.send(ToServer::Act { process, aim, hand });
    }

    /// The chosen offer, done: knapping a shape opens the stone to knap by hand.
    fn act_chosen(&mut self) {
        let Some(c) = &self.crafting else {
            return;
        };
        let Some(offer) = c.chosen().cloned() else {
            return;
        };
        let Some(Do::Process(id)) = offer.act.clone() else {
            if let Some(d) = offer.act {
                self.act(d);
            }
            return;
        };
        if let Some(shape) = hearth_craft::knap::Knap::shape_of(&id) {
            let content = c.content.clone();
            let mat = offer
                .material
                .as_deref()
                .and_then(|m| content.materials.get(m));
            let knapping = mat.and_then(|m| m.knapping).unwrap_or(0.3);
            let skill = c.knowledge.skill("knapping");
            let color = mat.map_or([150, 150, 150], |m| m.appearance.color.0);
            let seed = self.ticks ^ (self.mover.pos.x.to_bits() >> 7);
            let title = c
                .crafts
                .get(&id)
                .map_or_else(|| id.clone(), |r| r.def.name.clone());
            self.knap_request = Some(crate::knapping_ui::KnapScreen {
                knap: hearth_craft::knap::Knap::new(shape, knapping * (0.7 + 0.3 * skill), seed),
                process: id,
                aim: self.aim_at(),
                title,
                color,
                press: None,
            });
            return;
        }
        self.act(Do::Process(id));
    }

    /// What the journal shows.
    pub fn journal_view(&self) -> Option<crate::journal_ui::JournalView<'_>> {
        let c = self.crafting.as_ref()?;
        Some(crate::journal_ui::JournalView {
            knowledge: &c.knowledge,
            graph: &c.graph,
            mode: c.mode,
            ticks_per_day: self.calendar.ticks_per_day(),
        })
    }

    /// The hand that puts down or uses things first: the right, else the left.
    fn busy_hand(&self) -> Option<hearth_items::Hand> {
        if self.carry.right.is_some() {
            Some(hearth_items::Hand::Right)
        } else if self.carry.left.is_some() {
            Some(hearth_items::Hand::Left)
        } else {
            None
        }
    }

    /// The things the hands do this frame: pick up, gather, put down, drag and let go.
    fn handle_things(&mut self, input: &mut InputState, dt: f64) {
        self.aim = self.find_aim();
        for (n, key) in builtin::HOTBAR.iter().take(QUICK_SLOTS).enumerate() {
            if input.was_pressed(*key) {
                self.quick(n);
            }
        }
        self.reach(dt);
        self.refresh_offers();
        let aim = self.aim_at();
        let working = self.crafting.as_ref().is_some_and(|c| c.work.is_some());
        if let Some(c) = &mut self.crafting {
            if c.age(dt, aim) {
                c.told_look = aim;
                self.server.send(ToServer::Look(aim));
            }
            let steps = input.take_scroll_steps(1.0, true);
            if steps != 0 && !working {
                c.choose(steps);
            }
        }
        // The primary action does what is chosen, or stops the work under way.
        if input.was_pressed(builtin::ATTACK) && self.radial.is_none() {
            if working {
                self.server.send(ToServer::StopWork);
            } else {
                self.act_chosen();
            }
        }
        if input.was_pressed(builtin::INTERACT) {
            match self.aim {
                Some(Aim::Item(id)) => self.server.send(ToServer::PickUp(id)),
                Some(Aim::Block { .. }) => {
                    if let Some(d) = self.crafting.as_ref().and_then(|c| c.first_act()) {
                        self.act(d);
                    }
                }
                None => {}
            }
        }
        // A throw: wound up while the key is held, let fly when it is let go.
        if let Some(c) = &mut self.crafting {
            if input.is_active(builtin::THROW) && self.carry.right.is_some() {
                *c.charge.get_or_insert(0.0) += dt;
            } else if let Some(charge) = c.charge.take() {
                let dir = (self.camera.forward().as_dvec3() + DVec3::new(0.0, 0.12, 0.0))
                    .normalize_or_zero();
                let speed = 6.0 + 16.0 * charge.min(1.0);
                self.server.send(ToServer::Throw { dir, speed });
            }
        }
        let all = input.was_pressed(builtin::DROP_STACK);
        if (all || input.was_pressed(builtin::DROP))
            && let Some(hand) = self.busy_hand()
        {
            let (sy, cy) = (self.camera.yaw as f64).to_radians().sin_cos();
            let at = match self.aim {
                Some(Aim::Block { top: true, at, .. }) => at,
                _ => self.mover.pos + DVec3::new(-sy * 0.6, 0.5, cy * 0.6),
            };
            let count = self
                .carry
                .right
                .as_ref()
                .or(self.carry.left.as_ref())
                .and_then(|s| (!all && s.count > 1).then_some(1));
            self.server.send(ToServer::PutDown {
                from: hearth_items::Path::at(hearth_items::Root::Hand(hand)),
                count,
                at,
            });
        }
        let drag = input.is_active(builtin::DRAG);
        if drag
            && !self.drag_was
            && let Some(Aim::Item(id)) = self.aim
        {
            self.server.send(ToServer::Drag(id));
        }
        if !drag && self.drag_was && self.carry.dragging.is_some() {
            let at = self.dragged_at.unwrap_or(self.mover.pos);
            self.server.send(ToServer::LetGo(at));
        }
        self.drag_was = drag;
        // A dragged thing trails behind on the ground.
        if self.carry.dragging.is_some() {
            let (sy, cy) = (self.body_yaw as f64).to_radians().sin_cos();
            let behind = self.mover.pos - DVec3::new(-sy, 0.0, cy) * 1.6;
            let at = self.dragged_at.get_or_insert(behind);
            *at += (behind - *at) * (1.0 - (-dt * 4.0).exp());
        } else {
            self.dragged_at = None;
        }
    }

    /// Boxes for the things lying around, held and dragged.
    fn thing_boxes(&mut self, view: DVec3) {
        let (Some(items), Some(w)) = (&self.items, &self.world) else {
            return;
        };
        let light = |p: DVec3| {
            let b = hearth_math::BlockPos::containing(p + DVec3::Y * 0.3);
            (w.mirror.sky_light(b), w.mirror.block_light(b))
        };
        for wi in &self.world_items {
            let Some(k) = wi.stack.kind(items) else {
                continue;
            };
            let p = DVec3::from_array(wi.pos);
            if (p - view).length() > 96.0 {
                continue;
            }
            let size = glam::Vec3::from(k.size_m);
            let center = p + DVec3::new(0.0, size.y as f64 / 2.0, 0.0);
            let place = Affine3A::from_scale_rotation_translation(
                size,
                Quat::from_rotation_y(wi.yaw),
                (center - view).as_vec3(),
            );
            self.figure_boxes
                .push(hearth_character::solid(place, k.color, light(p)));
        }
        let (Some(f), Some(pose)) = (&self.figure, &self.pose) else {
            return;
        };
        let body = Affine3A::from_rotation_translation(
            Quat::from_rotation_y(-self.body_yaw.to_radians()),
            (self.mover.pos - view).as_vec3(),
        );
        let lit = light(self.mover.pos + DVec3::Y);
        // In the hands: the long side along the forearm; a load in both arms before the chest.
        let held = |s: &hearth_items::Stack| {
            let k = s.kind(items)?;
            let mut d = k.size_m;
            d.sort_by(|a, b| b.total_cmp(a));
            Some((k.color, glam::Vec3::new(d[1], d[0], d[2])))
        };
        for (stack, left) in [(&self.carry.left, true), (&self.carry.right, false)] {
            let Some((color, size)) = stack.as_ref().and_then(held) else {
                continue;
            };
            let place = if self.carry.both {
                let a = f.hand(pose, true).translation;
                let b = f.hand(pose, false).translation;
                body * Affine3A::from_scale_rotation_translation(
                    glam::Vec3::new(size.y, size.x, size.z),
                    Quat::IDENTITY,
                    ((a + b) / 2.0).into(),
                )
            } else {
                body * f.hand(pose, left)
                    * Affine3A::from_scale_rotation_translation(
                        size,
                        Quat::IDENTITY,
                        glam::Vec3::new(0.0, -size.y * 0.25, 0.0),
                    )
            };
            self.figure_boxes
                .push(hearth_character::solid(place, color, lit));
        }
        if let (Some(s), Some(at)) = (&self.carry.dragging, self.dragged_at)
            && let Some(k) = s.kind(items)
        {
            let size = glam::Vec3::from(k.size_m);
            let along = self.mover.pos - at;
            let yaw = along.x.atan2(along.z) as f32 + std::f32::consts::FRAC_PI_2;
            let center = at + DVec3::new(0.0, size.y as f64 / 2.0, 0.0);
            let place = Affine3A::from_scale_rotation_translation(
                size,
                Quat::from_rotation_y(yaw),
                (center - view).as_vec3(),
            );
            self.figure_boxes
                .push(hearth_character::solid(place, k.color, light(at)));
        }
    }

    /// Dresses the figure in what is worn.
    fn redress(&mut self) {
        let (Some(items), Some(f)) = (&self.items, &mut self.figure) else {
            return;
        };
        let garbs: Vec<hearth_character::Garb> = self
            .carry
            .worn
            .iter()
            .filter_map(|w| {
                let k = w.stack.kind(items)?;
                let wear = k.wear.as_ref()?;
                Some(hearth_character::Garb {
                    garment: wear.garment.clone(),
                    color: k.color,
                    layer: wear.layer,
                    regions: wear.regions.clone(),
                })
            })
            .collect();
        f.dress(&garbs);
    }

    /// What the inventory screen shows.
    pub fn inventory_view(&self) -> Option<crate::inventory_ui::InventoryView<'_>> {
        Some(crate::inventory_ui::InventoryView {
            carry: &self.carry,
            items: self.items.as_deref()?,
            body_kg: self.body_cfg.as_ref().map_or(70.0, |c| c.mass_kg as f32),
        })
    }

    /// Puts a carried thing down just in front of the feet.
    pub fn put_down_from(&mut self, from: hearth_items::Path, count: Option<u16>) {
        let (sy, cy) = (self.camera.yaw as f64).to_radians().sin_cos();
        let at = self.mover.pos + DVec3::new(-sy * 0.6, 0.5, cy * 0.6);
        self.server.send(ToServer::PutDown { from, count, at });
    }

    /// Whether the player can look through what they carry now.
    pub fn can_handle(&self) -> bool {
        self.items.is_some() && !self.dead() && !self.lying() && self.mode == CameraMode::Body
    }

    /// What the crosshair says: what is aimed at and what can be done with it.
    pub fn aim_words(&self, l: &Lang) -> Option<String> {
        let items = self.items.as_ref()?;
        if self.carry.dragging.is_some() {
            return Some(l.get("aim.dragging").to_owned());
        }
        match self.aim? {
            Aim::Item(id) => {
                let wi = self.world_items.iter().find(|w| w.id == id)?;
                let k = wi.stack.kind(items)?;
                let body_kg = self.body_cfg.as_ref().map_or(70.0, |c| c.mass_kg as f32);
                let name = if wi.stack.count > 1 {
                    format!("{} ×{}", k.name, wi.stack.count)
                } else {
                    k.name.clone()
                };
                let key = if wi.stack.mass(items) > hearth_items::carry::BOTH_HANDS_SHARE * body_kg
                {
                    "aim.drag"
                } else {
                    "aim.pick_up"
                };
                Some(l.format(key, &[("name", &name)]))
            }
            Aim::Block { pos, .. } => {
                let w = self.world.as_ref()?;
                let s = w.mirror.block(pos)?;
                let b = w.reg.block_of(s);
                if b.def.fluid.is_some() {
                    return Some(l.get("aim.water").to_owned());
                }
                // Plain ground says nothing; what can be worked says what it is.
                let offers = self
                    .crafting
                    .as_ref()
                    .is_some_and(|c| c.offers.iter().any(|o| o.act.is_some()));
                offers.then(|| b.name.path().replace('_', " "))
            }
        }
    }

    /// Moves a carried thing (or `count` of a stack): asks the server, which keeps it.
    pub fn shift(
        &mut self,
        from: hearth_items::Path,
        count: Option<u16>,
        to: hearth_items::Target,
    ) {
        // Shown at once; the server's word follows.
        let body_kg = self.body_cfg.as_ref().map_or(70.0, |c| c.mass_kg as f32);
        if let Some(items) = &self.items {
            self.carry.shift(items, &from, count, &to, body_kg);
        }
        self.server.send(ToServer::Shift { from, count, to });
    }

    /// The death screen's words: how the player died, the world's rules, and the life's tale
    /// if it ended the world.
    pub fn death_info(&self, l: &Lang) -> Option<crate::menus::DeathInfo> {
        let death = self.body.as_ref()?.dead.as_ref()?;
        let summary = self.ended.as_ref().map(|s| {
            let mut lines = Vec::new();
            if !s.name.trim().is_empty() {
                lines.push(s.name.clone());
            }
            lines.push(l.format("life.days", &[("n", &format!("{:.0}", s.days.floor()))]));
            lines.push(l.format("life.walked", &[("km", &format!("{:.1}", s.walked_km))]));
            lines.push(l.format("life.farthest", &[("km", &format!("{:.1}", s.farthest_km))]));
            lines
        });
        Some(crate::menus::DeathInfo {
            words: death_words(l, death),
            rules: self.death_rules,
            summary,
        })
    }

    /// Opens or closes the Body panel.
    pub fn toggle_body_panel(&mut self) {
        self.body_panel = !self.body_panel;
    }

    /// The body's senses on the image (v2 §9.9): exhaustion, thirst and weakness drain colour,
    /// fainting dims, pain and blood loss close in the edges with the pulse, cold greys the
    /// world blue, heat makes it waver.
    fn senses(&self) -> hearth_render::post::Senses {
        use hearth_body::{Hunger, Thirst, Tiredness, Warmth};
        let time = self.clock_s as f32;
        let Some(b) = &self.body else {
            return hearth_render::post::Senses::default();
        };
        if b.dead.is_some() {
            return hearth_render::post::Senses {
                desaturate: 0.85,
                vignette: 0.6,
                red: 0.2,
                dim: 0.35,
                time,
                ..Default::default()
            };
        }
        let s = &b.status;
        let fx = &s.effects;
        let ramp = |x: f32, lo: f32, hi: f32| ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
        let sight = (1.0 - fx.vision).clamp(0.0, 1.0);
        let tired = match s.tiredness {
            Tiredness::Exhausted => 0.55,
            Tiredness::VeryTired => 0.25,
            _ => 0.0,
        };
        let dry = match s.thirst {
            Thirst::Dying => 0.5,
            Thirst::Parched => 0.25,
            _ => 0.0,
        };
        let starving = if s.hunger == Hunger::Starving {
            0.2
        } else {
            0.0
        };
        let pulse = if self.reduce_motion {
            0.0
        } else {
            (-(self.heart_phase.fract() as f32) * 8.0).exp()
        };
        let hurt = (ramp(s.blood_lost, 0.08, 0.35) + 0.5 * fx.pain).min(1.0);
        let cold = match s.warmth {
            Warmth::Freezing => 0.8,
            Warmth::Hypothermic => 0.55,
            Warmth::Cold => 0.25,
            Warmth::Chilly => 0.08,
            _ => 0.0,
        };
        let heat = if self.reduce_motion {
            0.0
        } else {
            match s.warmth {
                Warmth::Overheating => 0.8,
                Warmth::Hot => 0.3,
                _ => 0.0,
            }
        };
        hearth_render::post::Senses {
            desaturate: (tired
                + dry
                + starving
                + 0.8 * sight
                + 0.5 * ramp(s.blood_lost, 0.1, 0.35))
            .min(0.9),
            vignette: hurt * (0.55 + 0.25 * pulse),
            red: ramp(s.blood_lost, 0.05, 0.3),
            dim: (0.6 * sight).min(0.8),
            cold,
            heat,
            time,
        }
    }

    /// Lies down to rest (sleep comes when the body is sleepy), or gets up.
    pub fn toggle_rest(&mut self) {
        let lying = self.body.as_ref().is_some_and(|b| b.lying);
        if !self.dead() {
            self.server.send(ToServer::Sleep(!lying));
        }
    }

    /// Lying down, awake or asleep.
    pub fn lying(&self) -> bool {
        self.body.as_ref().is_some_and(|b| b.lying || b.asleep)
    }

    /// First person, behind, in front, and round again.
    pub fn toggle_perspective(&mut self) {
        self.perspective = match self.perspective {
            Perspective::First => Perspective::Behind,
            Perspective::Behind => Perspective::Front,
            Perspective::Front => Perspective::First,
        };
    }

    /// The camera the frame is seen through: the eyes', or one standing back from them
    /// (stopped short of anything solid).
    pub fn view_camera(&self) -> Camera {
        let mut cam = self.camera;
        // A shivering body shakes the view a little.
        let shiver = self.hearing.rhythms.shiver;
        if self.mode == CameraMode::Body && shiver > 0.25 && !self.reduce_motion {
            let t = self.clock_s;
            let a = (shiver - 0.25) / 0.75;
            cam.yaw += ((t * 41.0).sin() * 0.3 * a as f64) as f32;
            cam.pitch += ((t * 53.0).sin() * 0.25 * a as f64) as f32;
        }
        if self.mode != CameraMode::Body || self.perspective == Perspective::First {
            return cam;
        }
        let f = cam.forward().as_dvec3();
        let dir = if self.perspective == Perspective::Behind {
            -f
        } else {
            f
        };
        let dist = self.clear_distance(cam.pos, dir, THIRD_PERSON_M);
        cam.pos += dir * dist;
        if self.perspective == Perspective::Front {
            cam.yaw = (cam.yaw + 180.0).rem_euclid(360.0);
            cam.pitch = -cam.pitch;
        }
        cam
    }

    /// How far along `dir` from `from` the way is clear (m, at most `max`).
    fn clear_distance(&self, from: DVec3, dir: DVec3, max: f64) -> f64 {
        let Some(w) = &self.world else {
            return max;
        };
        let mut t = 0.2;
        while t < max {
            let p = hearth_math::BlockPos::containing(from + dir * t);
            if let Some(s) = w.mirror.block(p)
                && !w.reg.collision_shape(s).is_empty()
            {
                return (t - 0.3).max(0.2);
            }
            t += 0.1;
        }
        max
    }

    /// Moves the body: which way it faces, what it is doing, its pose; then the eyes.
    fn animate(&mut self, dt: f64) {
        let Some(previous) = self.figure.as_ref().map(|f| f.animator.activity()) else {
            return;
        };
        let limp = self.dead()
            || self
                .body
                .as_ref()
                .is_some_and(|b| b.asleep || b.lying || !b.status.effects.conscious);
        let report = self.last_report.unwrap_or_default();
        let m = &self.mover;
        // The body turns to where the eyes look as it moves, or when the head would turn
        // too far.
        let diff = wrap180(self.camera.yaw - self.body_yaw);
        let moving = report.speed > 0.2
            || matches!(report.motion, Motion::Swimming | Motion::Climbing)
            || m.climb.is_some();
        if moving {
            self.body_yaw += diff * (1.0 - (-dt * 10.0).exp()) as f32;
        } else if diff.abs() > 55.0 {
            self.body_yaw += diff - 55.0 * diff.signum();
        }
        self.body_yaw = self.body_yaw.rem_euclid(360.0);
        let (sy, cy) = (self.body_yaw as f64).to_radians().sin_cos();
        let along = m.vel.x * -sy + m.vel.z * cy;
        let body = self.body.as_ref();
        let activity = if limp {
            Activity::Lie
        } else if m.climb.is_some() {
            Activity::Climb
        } else {
            match (report.motion, m.stance) {
                (Motion::Climbing, _) => Activity::Ladder,
                (_, Stance::Swimming) if report.eyes_under => Activity::Dive,
                (_, Stance::Swimming) if report.speed < 0.3 => Activity::Tread,
                (_, Stance::Swimming) => Activity::Swim,
                (_, Stance::Crawling) => Activity::Crawl,
                (_, Stance::Crouching) => Activity::Crouch,
                (Motion::Walking, _) => Activity::Walk,
                (Motion::Jogging, _) => Activity::Jog,
                (Motion::Sprinting, _) => Activity::Sprint,
                (Motion::Wading, _) => Activity::Wade,
                (Motion::Falling, _) if m.vel.y < -4.0 => Activity::Fall,
                // A hop keeps the gait it left the ground with.
                (Motion::Falling, _) => previous,
                _ => Activity::Stand,
            }
        };
        let climb = m.climb.map_or(0.0, |c| {
            (1.0 - c.left_s / c.total_s.max(1e-3)).clamp(0.0, 1.0) as f32
        });
        let drive = Drive {
            activity,
            speed: if along < -0.2 {
                -report.speed as f32
            } else {
                report.speed as f32
            },
            vertical: m.vel.y as f32,
            look_pitch: self.camera.pitch,
            look_yaw: diff.clamp(-75.0, 75.0),
            climb,
            shiver: body.map_or(0.0, |b| b.status.effects.shivering),
            breaths_per_min: self.hearing.rhythms.breaths_per_min,
            holding: hearth_character::Holding {
                left: self.carry.left.is_some(),
                right: self.carry.right.is_some(),
                both: self.carry.both,
                dragging: self.carry.dragging.is_some(),
            },
        };
        if let Some(fig) = &mut self.figure {
            self.pose = Some(fig.animator.update(&fig.rig, &drive, dt as f32));
        }
    }

    /// The eyes, from the body's pose (or steady at the person's height without view bobbing),
    /// smoothed over steps up.
    fn place_eyes(&mut self, dt: f64) {
        let feet = self.mover.pos;
        let physics_eye = self.mover.eye() - feet;
        let local = match (&self.figure, &self.pose) {
            (Some(f), Some(p))
                if self.view_bobbing
                    || matches!(self.mover.stance, Stance::Swimming | Stance::Crawling)
                    || self.mover.climb.is_some() =>
            {
                f.eye(p).as_dvec3()
            }
            (Some(f), _) => DVec3::new(0.0, physics_eye.y * f.rig.dims.stature as f64 / 1.75, 0.0),
            _ => physics_eye,
        };
        let turn = glam::DQuat::from_rotation_y(-(self.body_yaw as f64).to_radians());
        let eye = feet + turn * local;
        if eye.y > self.eye_y && eye.y - self.eye_y < 1.2 {
            self.eye_y += (eye.y - self.eye_y) * (1.0 - (-dt * 14.0).exp());
        } else {
            self.eye_y = eye.y;
        }
        self.camera.pos = DVec3::new(eye.x, self.eye_y, eye.z);
    }

    /// Shows or hides the debug screen.
    pub fn toggle_debug(&mut self) {
        self.debug_overlay = !self.debug_overlay;
    }

    /// Pauses the world (a single-player menu) or lets it run.
    pub fn pause(&mut self, paused: bool) {
        self.paused = paused;
        self.server.send(ToServer::Pause(paused));
    }

    /// The sounds to play since the last call.
    pub fn take_sounds(&mut self) -> std::vec::Drain<'_, hearth_audio::Command> {
        self.hearing.out.drain(..)
    }

    /// The surroundings to sound now; `dt` ages the captions.
    pub fn ambience(&mut self, dt: f64) -> hearth_audio::Ambience {
        let eye = self.camera.pos;
        let sheltered = self.world.as_ref().is_some_and(|w| {
            w.mirror
                .sky_top(eye.x.floor() as i32, eye.z.floor() as i32)
                .is_some_and(|top| top as f64 >= eye.y)
        });
        let underwater =
            self.mode == CameraMode::Body && self.last_report.is_some_and(|r| r.eyes_under);
        let paused = self.paused || self.world.is_none();
        self.hearing
            .ambience(self.weather, sheltered, underwater, paused, dt)
    }

    /// Takes changed options: the view, distances and detail.
    pub fn apply_options(&mut self, options: &Options) {
        self.captions = options.sound.subtitles;
        self.reduce_motion = options.accessibility.reduce_motion;
        self.guided_hud = options.accessibility.guided_hud;
        let v = &options.video;
        self.view_bobbing = v.view_bobbing;
        self.camera.fov_y = v.fov;
        self.render_scale = v.render_scale;
        self.water_quality = v.shader.water.into();
        self.lod_distance = v.lod_distance;
        self.lod_error_px = v.lod_error_px();
        let (radius, vertical) = (v.render_distance as i32, v.vertical_render_distance as i32);
        if (radius, vertical) != (self.radius, self.vertical) {
            self.radius = radius;
            self.vertical = vertical;
            self.server.send(ToServer::View { radius, vertical });
        }
        if let Some(scene) = &mut self.scene {
            scene.render_scale = self.render_scale;
            scene.terrain.water.quality = self.water_quality;
            scene.terrain.render_distance = self.radius;
            scene.terrain.vertical_distance = self.vertical;
        }
        if let Some(lod) = &mut self.lod {
            lod.set_settings(self.lod_distance, self.lod_error_px);
        }
    }

    /// The default world: its name, seed and where it is saved.
    pub fn default_world(
        name: &str,
        seed: u64,
        cache_dir: Option<std::path::PathBuf>,
        saves_dir: Option<std::path::PathBuf>,
        appearance: hearth_character::Appearance,
        death_rules: hearth_save::DeathRules,
        knowledge: hearth_save::KnowledgeMode,
    ) -> WorldSpec {
        WorldSpec {
            name: name.to_owned(),
            seed,
            planet: hearth_math::PlanetSize::Standard,
            cache_dir,
            saves_dir,
            appearance,
            death_rules,
            knowledge,
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
    /// After death: live on as the world's rules allow (as `who`, under Legacy).
    pub fn respawn(&mut self, who: Option<hearth_character::Appearance>) {
        if self.dead() {
            self.server.send(ToServer::Respawn(who));
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
        pad: &crate::gamepad::Pad,
        pad_sensitivity: f32,
    ) {
        self.clock_s += dt;
        // The controller's right stick turns at up to 120–360 degrees a second.
        if pad.look != DVec2::ZERO {
            let rate = 120.0 + 240.0 * pad_sensitivity as f64;
            self.camera.yaw = (self.camera.yaw + (pad.look.x * rate * dt) as f32).rem_euclid(360.0);
            self.camera.pitch =
                (self.camera.pitch + (pad.look.y * rate * dt) as f32).clamp(-90.0, 90.0);
        }
        // The clock runs at 20 ticks per second (faster warped or asleep) between the
        // server's messages.
        let rate = self.body.as_ref().map_or(20.0 + self.time_warp, |b| b.rate);
        self.tick_frac += dt * rate;
        // Eyes close for sleep and fainting, and open slowly, a little lost, on waking.
        let shut = self
            .body
            .as_ref()
            .is_some_and(|b| b.asleep || (b.dead.is_none() && !b.status.effects.conscious));
        let rate = if shut { 0.7 } else { 0.4 };
        let target = if shut { 1.0 } else { 0.0 };
        self.eyes_shut += (target - self.eyes_shut) * (1.0 - (-dt * rate).exp()) as f32;
        if let Some((_, t)) = &mut self.woke {
            *t += dt;
        }
        let r = self.hearing.rhythms;
        self.heart_phase += dt * r.heart_bpm as f64 / 60.0;
        self.breath_phase += dt * r.breaths_per_min as f64 / 60.0;
        // Holding the quick-choice key, the mouse leans the wheel instead of turning the head.
        if self.mode == CameraMode::Body && input.is_active(builtin::RADIAL) && !self.dead() {
            let v = self.radial.get_or_insert(DVec2::ZERO);
            if let Some((dx, dy)) = look {
                *v += DVec2::new(dx, dy);
                if v.length() > 120.0 {
                    *v = v.normalize() * 120.0;
                }
            }
        } else if self.radial.is_some() {
            if let Some(n) = self.radial_choice() {
                self.quick(n);
            }
            self.radial = None;
        } else if let Some((dx, dy)) = look {
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
        wish += forward * pad.stick.y + right * pad.stick.x;
        match self.mode {
            CameraMode::Free => self.fly(dt, input, wish),
            CameraMode::Body => self.walk(dt, input, wish, pad),
        }
        if self.mode == CameraMode::Body && !self.dead() && !self.lying() {
            self.handle_things(input, dt);
        } else {
            self.aim = None;
        }
        self.hearing
            .body(self.body.as_ref(), self.last_report.as_ref(), dt);
        if let Some(w) = &self.world {
            self.hearing
                .look_around(&w.mirror, &w.reg, self.camera.pos, dt);
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

    fn walk(&mut self, dt: f64, input: &mut InputState, wish: DVec2, pad: &crate::gamepad::Pad) {
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
        let gait = if pad.sprint || (sprint_key && self.sprinting) {
            Gait::Sprint
        } else if sprint_key || pad.stick.length() > 0.92 {
            Gait::Jog
        } else {
            Gait::Walk
        };
        let Some(w) = &self.world else {
            return;
        };
        let alive = !self.dead()
            && self
                .body
                .as_ref()
                .is_none_or(|b| !b.asleep && !b.lying && b.status.effects.conscious);
        let intent = if alive {
            let crouch = input.is_active(builtin::SNEAK) || pad.crouch;
            // A part-way stick walks slower; keys are full.
            let wish = if wish.length() > 1.0 {
                wish.normalize()
            } else {
                wish
            };
            Intent {
                wish,
                gait,
                jump: input.is_active(builtin::JUMP) || pad.jump,
                crouch,
                crawl: input.is_active(builtin::CRAWL) || pad.crawl,
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
        let vy_before = self.mover.vel.y;
        let report = hearth_physics::step(&terrain, &mut self.mover, &intent, &ability, dt);
        let foot = self
            .hearing
            .moved(&w.mirror, &w.reg, &self.mover, &report, vy_before, dt);
        // The footsteps heard and the feet seen come down together.
        if let (Some(left), Some(f)) = (foot, &mut self.figure) {
            f.animator.foot_down(left);
        }
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
        self.animate(dt);
        self.place_eyes(dt);
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
                    self.figure = Some(Figure::new(r.appearance));
                    self.body_cfg = Some(r.body);
                    self.death_rules = r.death_rules;
                    self.ended = r.ended;
                    self.items = Some(r.items);
                    self.crafting = Some(Crafting::new(
                        r.content,
                        r.crafts,
                        r.graph,
                        r.knowledge_mode,
                    ));
                    self.pose = None;
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
                ToClient::Woke(why) => self.woke = Some((why, 0.0)),
                ToClient::Person(a) => {
                    self.figure = Some(Figure::new(a));
                    self.redress();
                    self.pose = None;
                    self.hearing = crate::hearing::Hearing::default();
                }
                ToClient::Ended(s) => self.ended = Some(s),
                ToClient::Carried(c) => {
                    self.carry = c;
                    self.redress();
                }
                ToClient::Items(v) => self.world_items = v,
                ToClient::Knowledge(k) => {
                    if let Some(c) = &mut self.crafting {
                        c.knowledge = *k;
                    }
                }
                ToClient::Work(w) => {
                    if let Some(c) = &mut self.crafting {
                        c.work = w;
                    }
                }
                ToClient::Acted(a) => {
                    if let Some(c) = &mut self.crafting {
                        let kind = if a.done { News::Done } else { News::Failed };
                        c.tell(a.words, kind);
                    }
                }
                ToClient::Learned {
                    name,
                    discovered,
                    text,
                } => {
                    if let Some(c) = &mut self.crafting {
                        if discovered {
                            c.tell(format!("Learned: {name}"), News::Learned);
                        } else if !text.is_empty() {
                            c.tell(text, News::Hunch);
                        }
                    }
                }
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
            return;
        }
        let near = self.near_area();
        if let (Some(lod), Some(scene)) = (&mut self.lod, &self.scene) {
            // Distant terrain is detailed for the pixels rendered.
            lod.set_view(scene.render_size(targets.size).1, self.camera.fov_y);
        }
        let ticks = self.now_ticks();
        let view = self.view_camera();
        let senses = self.senses();
        self.figure_boxes.clear();
        self.thing_boxes(view.pos);
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
        let (e, weather) = env.sample(&moment, view.pos, 0.0, EnvOverrides::default());
        // Rain is heard; snow falls silently.
        let rain = match weather.precip {
            hearth_env::weather::Precip::Rain => weather.precip_mm_h,
            hearth_env::weather::Precip::Sleet => 0.6 * weather.precip_mm_h,
            _ => 0.0,
        };
        self.weather = (weather.wind_speed_m_s as f32, rain as f32);
        scene.vertical_scale = self.vertical_scale;
        match &self.lod {
            Some(lod) if self.lod_distance > 0 => {
                scene.near_area = Some(near);
                scene.lod_show.clear();
                scene.lod_show.extend_from_slice(lod.show());
            }
            _ => scene.near_area = None,
        }
        // The player's body: in first person without the head (the eyes are in it).
        if let (Some(f), Some(pose), Some(w)) = (&self.figure, &self.pose, &self.world) {
            let rel = (self.mover.pos - view.pos).as_vec3();
            let place = Affine3A::from_rotation_translation(
                Quat::from_rotation_y(-self.body_yaw.to_radians()),
                rel,
            );
            let chest = hearth_math::BlockPos::containing(self.mover.pos + DVec3::Y * 1.2);
            let show = Show {
                hide_head: self.mode == CameraMode::Body && self.perspective == Perspective::First,
                sky_light: w.mirror.sky_light(chest),
                block_light: w.mirror.block_light(chest),
            };
            hearth_character::instances(
                &f.rig,
                &f.palette,
                pose,
                place,
                show,
                &mut self.figure_boxes,
            );
        }
        scene.figures.set(ctx, &self.figure_boxes);
        scene.senses = senses;
        scene.prepare(ctx, &view, targets.size, &e, dt);
        scene.render(ctx, enc, targets.color, targets.depth, targets.size);
    }

    /// The client's part of the interface: what death says, and the debug screen;
    /// `backdrop` is the opacity of the panels behind text (0–1).
    pub fn hud(&self, ui: &mut Ui<'_>, backdrop: f32) {
        let (w, h) = ui.size;
        let veil = (backdrop.clamp(0.0, 1.0) * 255.0) as u8;
        if let Some(b) = &self.body
            && b.dead.is_none()
            && !b.asleep
            && self.mode == CameraMode::Body
            && self.perspective == Perspective::First
        {
            self.draw_breath(ui, b);
        }
        if self.guided_hud {
            self.draw_guided(ui, veil);
        }
        if self.radial.is_some() {
            self.draw_radial(ui);
        }
        if self.mode == CameraMode::Body
            && self.perspective == Perspective::First
            && !self.dead()
            && !self.body_panel
        {
            ui.draw.rect(
                (w / 2.0).round() - 1.0,
                (h / 2.0).round() - 1.0,
                2.0,
                2.0,
                Rgba([235, 235, 235, 180]),
            );
            if let Some(words) = self.aim_words(ui.lang) {
                let lw = ui.font.width(&words) as f32;
                ui.label(
                    ((w - lw) / 2.0).round(),
                    (h / 2.0 + 8.0).round(),
                    &words,
                    Rgba([235, 235, 230, 220]),
                );
            }
        }
        if self.mode == CameraMode::Body
            && !self.dead()
            && !self.body_panel
            && let Some(c) = &self.crafting
        {
            c.draw(ui, veil);
        }
        // Eyelids: the world goes dark asleep or fainting.
        if self.eyes_shut > 0.01 {
            let a = (self.eyes_shut.clamp(0.0, 1.0) * 255.0) as u8;
            ui.draw.rect(0.0, 0.0, w, h, Rgba([0, 0, 0, a]));
        }
        let lying = self.body.as_ref().is_some_and(|b| b.lying && !b.asleep);
        if let Some(b) = &self.body
            && b.dead.is_none()
        {
            let line = if b.asleep {
                Some(ui.t("body.asleep"))
            } else if lying {
                Some(ui.t("body.lying"))
            } else {
                None
            };
            if let Some(line) = line {
                let lw = ui.font.width(&line) as f32;
                ui.label((w - lw) / 2.0, h * 0.82, &line, Rgba([220, 220, 230, 200]));
            }
        }
        if let Some((why, t)) = self.woke
            && t < 5.0
        {
            let line = ui.t(why.key());
            let lw = ui.font.width(&line) as f32;
            let a = ((1.0 - ((t - 3.5).max(0.0) / 1.5)) * 230.0) as u8;
            ui.label((w - lw) / 2.0, h * 0.4, &line, Rgba([235, 230, 220, a]));
        }
        if self.captions {
            self.draw_captions(ui, veil);
        }
        if self.body_panel
            && let Some(b) = &self.body
        {
            crate::body_panel::draw(ui, self.body_cfg.as_deref(), b, veil);
        }
        if self.debug_overlay {
            let lines = self.debug_lines(ui.lang);
            let width = lines.iter().map(|l| ui.font.width(l)).max().unwrap_or(0) as f32;
            let lh = hearth_ui::font::LINE as f32;
            ui.draw.rect(
                1.0,
                1.0,
                width + 4.0,
                lines.len() as f32 * lh + 3.0,
                Rgba([0, 0, 0, veil]),
            );
            for (k, line) in lines.iter().enumerate() {
                ui.label(3.0, 3.0 + k as f32 * lh, line, Rgba::WHITE);
            }
        }
    }

    /// Breath fog in cold air: puffs rising in front of the eyes as the breath goes out, thicker
    /// the colder and damper the air.
    fn draw_breath(&self, ui: &mut Ui<'_>, b: &BodyView) {
        let e = &b.exposure;
        if e.immersion > 0.8 {
            return;
        }
        let cold = ((7.0 - e.air_c) / 14.0).clamp(0.0, 1.0) * (0.4 + 0.6 * e.humidity);
        let p = self.breath_phase.fract() as f32;
        if cold < 0.02 || !(0.45..0.95).contains(&p) {
            return;
        }
        let (w, h) = ui.size;
        let u = (p - 0.45) / 0.5;
        let env = (std::f32::consts::PI * u).sin();
        for k in 0..4 {
            let kf = k as f32;
            let size = 10.0 + u * 26.0 + kf * 4.0;
            let x = w / 2.0 + (kf - 1.5) * (6.0 + u * 18.0) - size / 2.0;
            let y = h * (0.98 - 0.18 * u) - kf * 3.0 - size * 0.3;
            let a = (cold * env * 0.35 * (1.0 - kf * 0.15) * 255.0) as u8;
            ui.draw
                .rect(x, y, size, size * 0.6, Rgba([232, 236, 240, a]));
        }
    }

    /// The quick-choice wheel: what hangs at each attachment point, around the middle of the
    /// view, the one leaned toward lit.
    fn draw_radial(&self, ui: &mut Ui<'_>) {
        let Some(items) = &self.items else {
            return;
        };
        let places = self.quick_places();
        if places.is_empty() {
            return;
        }
        let (w, h) = ui.size;
        let (cx, cy) = (w / 2.0, h / 2.0);
        let chosen = self.radial_choice();
        let r = 52.0;
        for (k, place) in places.iter().enumerate() {
            let a = k as f32 / places.len() as f32 * std::f32::consts::TAU;
            let (x, y) = (cx + r * a.sin(), cy - r * a.cos());
            let name = self
                .carry
                .get(&hearth_items::Path::at(*place))
                .and_then(|s| s.kind(items))
                .map_or_else(|| ui.t("inv.empty"), |k| k.name.clone());
            let label = format!("{} {name}", k + 1);
            let lw = ui.font.width(&label) as f32;
            let bg = if chosen == Some(k) {
                Rgba([70, 80, 96, 230])
            } else {
                Rgba([10, 12, 16, 200])
            };
            ui.draw
                .rect(x - lw / 2.0 - 3.0, y - 6.0, lw + 6.0, 12.0, bg);
            ui.label(x - lw / 2.0, y - 4.0, &label, Rgba([235, 235, 230, 240]));
        }
    }

    /// The Guided HUD: compact bars for what the sensations say.
    fn draw_guided(&self, ui: &mut Ui<'_>, veil: u8) {
        use hearth_body::{Hunger, Thirst, Tiredness, Warmth};
        let Some(b) = &self.body else {
            return;
        };
        if b.dead.is_some() {
            return;
        }
        let s = &b.status;
        let food = match s.hunger {
            Hunger::Stuffed => 1.0,
            Hunger::Full => 0.9,
            Hunger::Satisfied => 0.75,
            Hunger::Peckish => 0.6,
            Hunger::Hungry => 0.4,
            Hunger::VeryHungry => 0.2,
            Hunger::Starving => 0.05,
        };
        let water = match s.thirst {
            Thirst::Sated => 1.0,
            Thirst::Fine => 0.8,
            Thirst::Thirsty => 0.5,
            Thirst::VeryThirsty => 0.3,
            Thirst::Parched => 0.15,
            Thirst::Dying => 0.03,
        };
        let rest = match s.tiredness {
            Tiredness::Rested => 1.0,
            Tiredness::Awake => 0.75,
            Tiredness::Tired => 0.5,
            Tiredness::VeryTired => 0.3,
            Tiredness::Exhausted => 0.1,
        };
        // Warmth: full when comfortable, falling either way, blue for cold and red for heat.
        let steps = [
            Warmth::Freezing,
            Warmth::Hypothermic,
            Warmth::Cold,
            Warmth::Chilly,
            Warmth::Comfortable,
            Warmth::Warm,
            Warmth::Hot,
            Warmth::Overheating,
        ];
        let i = steps.iter().position(|w| *w == s.warmth).unwrap_or(4) as f32;
        let warmth = 1.0 - (i - 4.0).abs() / 4.0;
        let warmth_color = if i < 4.0 {
            Rgba::rgb(110, 170, 240)
        } else if i > 4.0 {
            Rgba::rgb(235, 110, 70)
        } else {
            Rgba::rgb(120, 200, 110)
        };
        let bars = [
            ("hud.food", food, Rgba::rgb(222, 150, 60)),
            ("hud.water", water, Rgba::rgb(80, 150, 230)),
            ("hud.warmth", warmth, warmth_color),
            ("hud.rest", rest, Rgba::rgb(160, 120, 210)),
            ("hud.stamina", s.stamina, Rgba::rgb(230, 210, 80)),
            (
                "hud.blood",
                (1.0 - s.blood_lost / 0.4).clamp(0.0, 1.0),
                Rgba::rgb(200, 50, 50),
            ),
        ];
        let (_, h) = ui.size;
        let row = 9.0;
        let top = h - 4.0 - bars.len() as f32 * row;
        ui.draw.rect(
            2.0,
            top - 2.0,
            104.0,
            bars.len() as f32 * row + 3.0,
            Rgba([0, 0, 0, veil]),
        );
        for (k, (key, v, color)) in bars.iter().enumerate() {
            let y = top + k as f32 * row;
            let label = ui.t(key);
            ui.label(4.0, y, &label, Rgba([220, 220, 225, 230]));
            ui.draw
                .rect(50.0, y + 2.0, 52.0, 4.0, Rgba([40, 40, 46, 220]));
            ui.draw
                .rect(50.0, y + 2.0, 52.0 * v.clamp(0.0, 1.0), 4.0, *color);
        }
    }

    /// The sounds in words, lowest right, fading as they pass.
    fn draw_captions(&self, ui: &mut Ui<'_>, veil: u8) {
        let lines: Vec<(String, f32)> = self
            .hearing
            .captions()
            .map(|(key, alpha)| (ui.t(key), alpha))
            .collect();
        if lines.is_empty() {
            return;
        }
        let (w, h) = ui.size;
        let lh = hearth_ui::font::LINE as f32;
        let width = lines
            .iter()
            .map(|(t, _)| ui.font.width(t))
            .max()
            .unwrap_or(0) as f32;
        let top = h - 4.0 - lines.len() as f32 * lh;
        let left = w - width - 8.0;
        ui.draw.rect(
            left - 2.0,
            top - 2.0,
            width + 6.0,
            lines.len() as f32 * lh + 3.0,
            Rgba([0, 0, 0, veil]),
        );
        for (k, (text, alpha)) in lines.iter().enumerate() {
            let a = (alpha * 255.0) as u8;
            ui.label(left, top + k as f32 * lh, text, Rgba([255, 255, 255, a]));
        }
    }

    /// The debug screen's lines.
    fn debug_lines(&self, l: &Lang) -> Vec<String> {
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

/// An angle in degrees brought into −180..180.
fn wrap180(a: f32) -> f32 {
    (a + 180.0).rem_euclid(360.0) - 180.0
}

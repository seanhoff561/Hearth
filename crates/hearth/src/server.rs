//! The integrated server (v1 M4, V2-3): a thread that owns the world. Twenty times a second it
//! takes the client's reports of the player's movement, advances the clock and lives the
//! player's body in the weather, water and shelter where they are (`hearth_body`); between ticks
//! it generates, lights and meshes the terrain around the player, nearest first, sending each
//! cube's blocks (for the client's collision) and its mesh, and unloads what is left behind. It
//! saves the world's clock and the player on request, every five minutes and when it stops.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use glam::{DVec2, DVec3};
use hearth_body::{BodyConfig, Exposure, Posture, Worn};
use hearth_content::balance::Balance;
use hearth_env::{Calendar, Moment};
use hearth_math::{BlockPos, CubePos, Planet, PlanetSize};
use hearth_physics::{Mover, Report};
use hearth_player::{DROWN_S, Player};
use hearth_protocol::{BodyView, Moved, Ready, ToClient, ToServer};
use hearth_render::atlas::TextureArray;
use hearth_render::mesh::MeshOptions;
use hearth_render::models::BlockModels;
use hearth_render::precip::SkyHeights;
use hearth_save::{WorldDir, WorldMeta, WorldSettings};
use hearth_world::water::{WaterEnv, WaterSim};
use hearth_world::{BlockStateId, Cube};
use hearth_worldgen::vegetation::Vegetation;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::environment::{EnvOverrides, EnvSampler};
use crate::scene::LocalWorld;
use crate::workshop::{Here, Workshop, WorkshopSave};

/// Cubes generated per batch (bounded so nearby terrain appears quickly while moving).
const BATCH: usize = 192;
/// The body's longest step (s): the world going faster than lived is lived by it in steps no
/// longer.
const BODY_STEP_S: f64 = 30.0;
/// Seconds of play per tick.
pub const TICK_S: f64 = 1.0 / hearth_core::TICKS_PER_SECOND as f64;
/// Ticks between autosaves (five minutes).
const AUTOSAVE_TICKS: u64 = 6000;
/// Seasonal cover steps per year (as the year-scale snow model).
const COVER_STEPS: f64 = 73.0;
/// Cubes grown again at most per step of streaming as the vegetation changes.
const REGROW_BATCH: usize = 48;
/// Format of `player.json`.
const PLAYER_FORMAT: u32 = 1;

/// The world to run.
#[derive(Debug, Clone)]
pub struct WorldSpec {
    pub name: String,
    /// Used when the world is created; a saved world keeps its own.
    pub seed: u64,
    pub planet: PlanetSize,
    pub cache_dir: Option<PathBuf>,
    /// Where worlds are saved; `None` runs without saving.
    pub saves_dir: Option<PathBuf>,
    /// Who a new world's player is (a saved world keeps its own person): an adult (Amendment E
    /// §6.1).
    pub appearance: hearth_character::Appearance,
    /// How knowledge is gained in a new world (its mode's, when it has one).
    pub knowledge: hearth_save::KnowledgeMode,
    /// A new world's era (`eras/`; a saved world keeps its own).
    pub era: String,
    /// A new world's shape beyond its planet's size (Create World's More options).
    pub shape: WorldShape,
    /// Where a new world's first life begins (world x, z), chosen on the globe; `None`: where
    /// the world finds a place.
    pub birthplace: Option<DVec2>,
    /// A new world's game mode (`balance/modes.ron`, Amendment P §2): it sets the realism, the
    /// knowledge, what a new life keeps and the predators' ways over `knowledge`; `None` (tests
    /// and tools): `knowledge` as given, and every power of watching and of time open.
    pub mode: Option<String>,
}

/// A new world's shape, as Create World's More options set it (Amendment P §4.3); each `None`
/// the game's default.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WorldShape {
    /// When the world's clock begins (E §4.1).
    pub start: Option<hearth_save::Start>,
}

impl WorldShape {
    /// The world-generation settings of a new world of `seed` and `size` (Earth, or a test
    /// planet: E §5.1).
    pub fn planet(&self, seed: u64, size: PlanetSize) -> hearth_worldgen::WorldGenSettings {
        hearth_worldgen::WorldGenSettings {
            seed,
            planet_size: size,
            ..Default::default()
        }
        .sanitized()
    }

    /// Sets a new world's life and time settings to this shape.
    pub fn apply(&self, life: &mut hearth_save::LifeSettings) {
        if let Some(s) = self.start {
            life.start = s;
        }
    }
}

/// How much terrain to keep around the player (cubes).
#[derive(Debug, Clone, Copy)]
pub struct View {
    pub radius: i32,
    pub vertical: i32,
}

/// What `player.json` holds.
#[derive(serde::Serialize, serde::Deserialize)]
struct PlayerSave {
    format: u32,
    player: Player,
    /// How the player looks (saves before it take the default person).
    #[serde(default)]
    appearance: hearth_character::Appearance,
    /// What the player's lives before this one knew: kept per player through any number of
    /// deaths (a new life knows it in Easy, Amendment E §6.6).
    #[serde(default)]
    past_lives: std::collections::BTreeSet<String>,
}

/// Handle to the server thread; it saves and stops when dropped.
pub struct Server {
    to: Sender<ToServer>,
    from: Receiver<ToClient>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    pub fn start(spec: WorldSpec, atlas: Arc<TextureArray>, view: View) -> Self {
        let (to, inbox) = channel();
        let (tx, from) = channel();
        let thread = std::thread::Builder::new()
            .name("server".into())
            .spawn(move || {
                if let Err(e) = run(spec, atlas, view, inbox, &tx) {
                    log::error!("server: {e:#}");
                    let _ = tx.send(ToClient::Failed(format!("{e:#}")));
                }
            })
            .expect("spawn server thread");
        Self {
            to,
            from,
            thread: Some(thread),
        }
    }

    pub fn send(&self, m: ToServer) {
        let _ = self.to.send(m);
    }

    /// The next message, if any.
    pub fn poll(&self) -> Option<ToClient> {
        self.from.try_recv().ok()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.to.send(ToServer::Quit);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// The world's save, opened or created.
struct Save {
    dir: WorldDir,
    meta: WorldMeta,
}

/// A new world's life settings: its shape's, and its mode's rules (or, with none, the knowledge
/// given).
fn new_life(spec: &WorldSpec, content: &hearth_content::Content) -> hearth_save::LifeSettings {
    let mut life = hearth_save::LifeSettings {
        knowledge_mode: spec.knowledge,
        ..Default::default()
    };
    if let Some(r) = mode_rules(content, spec.mode.as_deref()) {
        crate::modes::apply(&r, &mut life);
    }
    spec.shape.apply(&mut life);
    life
}

/// Creative's body (Amendment P §3.1): health, food, water, warmth, rest and stamina always full;
/// no injury, illness or harm from a fall stays, and nothing kills it.
fn creative_body(player: &mut Player, cfg: &BodyConfig, seed: u64, ticks: u64) {
    let b = &mut player.body;
    let worn = b.dead.is_some()
        || !b.injuries.is_empty()
        || !b.illnesses.is_empty()
        || b.stamina < 0.95
        || b.blood_l < b.blood_full_l * 0.99;
    b.injuries.clear();
    b.illnesses.clear();
    b.stamina = 1.0;
    if worn || ticks.is_multiple_of(40) {
        let age = b.age_s;
        let sized = (b.sized_kg, b.blood_full_l);
        *b = hearth_body::Body::new(cfg, seed);
        b.age_s = age;
        (b.sized_kg, b.blood_full_l) = sized;
        b.blood_l = b.blood_full_l;
        player.asleep = false;
    }
}

/// A mode's rules by its id (none for a world of no mode, or one no longer known).
pub fn mode_rules(
    content: &hearth_content::Content,
    id: Option<&str>,
) -> Option<crate::modes::Rules> {
    id.and_then(|id| crate::modes::find(content, id))
        .map(crate::modes::rules)
}

fn open_save(spec: &WorldSpec, lw: &LocalWorld) -> anyhow::Result<Option<Save>> {
    let Some(saves) = &spec.saves_dir else {
        return Ok(None);
    };
    let root = saves.join(&spec.name);
    if root.join("level.json").exists() {
        let (dir, meta, _) = WorldDir::open(&root)?;
        return Ok(Some(Save { dir, meta }));
    }
    std::fs::create_dir_all(saves)?;
    let planet = spec.shape.planet(spec.seed, spec.planet);
    let mut settings = WorldSettings::new(planet);
    settings.era = spec.era.clone();
    settings.life = new_life(spec, &lw.content);
    settings.mode = spec.mode.clone();
    settings.played_in_creative =
        mode_rules(&lw.content, spec.mode.as_deref()).is_some_and(|r| r.creative);
    settings.birthplace = spec.birthplace.map(|p| [p.x, p.y]);
    let meta = WorldMeta::new(&spec.name, settings, lw.reg.state_names());
    let dir = WorldDir::create(saves, &meta)?;
    log::info!("created world {:?} in {}", spec.name, dir.root.display());
    Ok(Some(Save { dir, meta }))
}

/// Whether a point is within the player's reach (3 m of the eyes).
/// The year the vegetation has grown to: years of the calendar since the world began.
fn vegetation_year(calendar: &Calendar, ticks: u64) -> f64 {
    ticks as f64 / calendar.ticks_per_day() / calendar.days_per_year()
}

fn within_reach(player: &Player, at: DVec3) -> bool {
    let eye = player.mover.pos + DVec3::new(0.0, 1.6, 0.0);
    (at - eye).length() <= 3.0 && at.is_finite()
}

/// Where a thing put down at `at` comes to rest: on the first solid surface below it.
pub(crate) fn rest_on(lw: &LocalWorld, at: DVec3) -> DVec3 {
    let x = at.x.floor() as i32;
    let z = at.z.floor() as i32;
    let top = (at.y + 1.5).floor() as i32;
    for y in (top - 8..=top).rev() {
        let p = BlockPos::new(x, y, z);
        if let Some(s) = lw.map.block(p) {
            let shape = lw.reg.collision_shape(s);
            if !shape.is_empty() {
                let h = shape.boxes.iter().map(|b| b.max.y).fold(0.0f64, f64::max);
                let surface = y as f64 + h;
                if surface <= at.y + 1.0 {
                    return DVec3::new(at.x, surface, at.z);
                }
            }
        }
    }
    at
}

/// How hard the ground underfoot drags at a load pulled over it: sliding friction (snow and ice
/// let a load slide, sand and mud hold it), less on a sledge's runners, and rolling resistance
/// on wheels (V2-12: little on hard ground, much in sand, mud and snow).
fn drag_friction(
    lw: &LocalWorld,
    feet: DVec3,
    dragged: Option<&hearth_items::Stack>,
    items: &hearth_items::Items,
) -> f32 {
    let below = BlockPos::containing(feet - DVec3::new(0.0, 0.05, 0.0));
    let at = BlockPos::containing(feet + DVec3::new(0.0, 0.05, 0.0));
    let group = |p: BlockPos| {
        lw.map
            .block(p)
            .filter(|s| !s.is_air())
            .map(|s| lw.reg.block_of(s).def.sound.clone())
    };
    let ground = group(at).or_else(|| group(below));
    let has = |name: &str| {
        dragged
            .and_then(|s| s.property(items, name))
            .is_some_and(|v| v > 0.0)
    };
    if has("wheels") {
        return match ground.as_deref() {
            Some("snow") => 0.25,
            Some("glass") => 0.03,
            Some("sand") => 0.2,
            Some("mud") => 0.3,
            Some("stone") | Some("deepslate") => 0.03,
            Some("none") => 0.03,
            _ => 0.07,
        };
    }
    if has("runners") {
        return match ground.as_deref() {
            Some("snow") => 0.05,
            Some("glass") => 0.03,
            Some("sand") => 0.45,
            Some("mud") => 0.4,
            Some("stone") | Some("deepslate") => 0.25,
            Some("none") => 0.05,
            _ => 0.3,
        };
    }
    match ground.as_deref() {
        Some("snow") => 0.12,
        Some("glass") => 0.05,
        Some("sand") => 0.6,
        Some("mud") => 0.7,
        Some("stone") | Some("deepslate") => 0.35,
        Some("none") => 0.1,
        _ => 0.45,
    }
}

/// A new world's calendar and first spawn, the world made in no year (tools: as a world made
/// with `start` would be).
pub fn calendar_for(lw: &LocalWorld, start: hearth_save::Start) -> (Calendar, DVec3) {
    let spawn = first_spawn(lw, None);
    (calendar_of(lw.map.planet(), start, None, spawn), spawn)
}

/// A world's calendar (E §4.1): Earth's, set when the world was made (`created_unix`): a
/// spring morning where its first life begins (`first_spawn`), in the year it was made, or the
/// real moment it was made. A world made in no year (not saved) takes an undated year's spring
/// (`Start::Now`: the moment it starts).
pub fn calendar_of(
    planet: &Planet,
    start: hearth_save::Start,
    created_unix: Option<u64>,
    first_spawn: DVec3,
) -> Calendar {
    let made = created_unix.map(|t| Calendar::from_unix(t as f64));
    match start {
        hearth_save::Start::Now => made.unwrap_or_else(|| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0.0, |d| d.as_secs_f64());
            Calendar::from_unix(now)
        }),
        hearth_save::Start::SpringMorning => Calendar::spring_dawn(
            made.map_or(hearth_env::calendar::UNDATED_YEAR, |c| c.at(0).year),
            planet.latitude_deg(first_spawn.z),
            planet.solar_time_offset(first_spawn.x),
        ),
    }
}

/// The world's first spawn: where the world finds a place, moved by `place` (from the place
/// found): the birthplace chosen on the globe.
pub fn first_spawn(lw: &LocalWorld, place: Option<&dyn Fn(DVec2) -> DVec2>) -> DVec3 {
    let (mut sx, mut sz) = lw.terrain().find_spawn(false);
    if let Some(place) = place {
        let at = place(DVec2::new(sx as f64, sz as f64));
        (sx, sz) = lw
            .terrain()
            .spawn_near(at.x.floor() as i32, at.y.floor() as i32);
    }
    ground_at(lw, sx, sz)
}

/// Feet on the ground at a column: on the surface or the water, a little above.
fn ground_at(lw: &LocalWorld, x: i32, z: i32) -> DVec3 {
    DVec3::new(
        x as f64 + 0.5,
        lw.surface_y(x as f64 + 0.5, z as f64 + 0.5) + 0.5,
        z as f64 + 0.5,
    )
}

/// How much of the weather's wind (measured high and in the open) reaches a body standing at
/// `feet`: about three quarters at head height over open ground (the wind's log profile),
/// less among trees and brush close by (a wood's floor sees a fifth to a half of the open
/// wind) and behind solid ground or walls upwind.
fn lee(lw: &LocalWorld, feet: DVec3) -> f64 {
    let base = BlockPos::containing(feet);
    let mut cover = 0.0f64;
    let mut cells = 0.0f64;
    for dz in -4..=4 {
        for dx in -4..=4 {
            if dx * dx + dz * dz > 16 {
                continue;
            }
            for dy in 0..4 {
                cells += 1.0;
                let p = BlockPos::new(base.x + dx, base.y + dy, base.z + dz);
                if let Some(s) = lw.map.block(p)
                    && !s.is_air()
                {
                    let def = &lw.reg.block_of(s).def;
                    // Solid ground and trunks stop the wind; leaves, brush and tall plants
                    // slow it.
                    cover += if def.opaque {
                        1.0
                    } else if def.fluid.is_some() {
                        0.0
                    } else if dy == 0 {
                        // Grass and flowers about the feet hardly matter.
                        0.1
                    } else {
                        0.5
                    };
                }
            }
        }
    }
    let shelter = (cover / cells.max(1.0) * 2.5).min(0.75);
    0.75 * (1.0 - shelter)
}

/// The weather, water and shelter where the player is, as the body feels them.
fn exposure(
    env: &EnvSampler,
    lw: &LocalWorld,
    moment: &Moment,
    mover: &Mover,
    immersion: f64,
) -> Exposure {
    let pos = mover.pos;
    let (light, w) = env.sample(moment, pos, 0.0, EnvOverrides::default());
    let head = pos.y + mover.stance.height();
    let (x, z) = (pos.x.floor() as i32, pos.z.floor() as i32);
    // Under a roof, and how much of the rain comes through it (V2-8 (d)).
    let (covered, rain_through) = crate::building::cover(&lw.map, &lw.reg, &lw.content, x, z, head);
    // The sun's beam (about 105 lm per W) on the share of the body facing it (27 %), 70 %
    // absorbed; under cover, shade.
    let sun_lux = (light.sun_lux.x + light.sun_lux.y + light.sun_lux.z) as f64 / 3.0;
    let radiant = if covered {
        0.0
    } else {
        sun_lux / 105.0 * 0.27 * 0.7
    };
    let night = light.sun_dir.y < 0.0;
    let clear = 1.0 - w.cloud_cover;
    let water = crate::water_env::WorldWater {
        generator: &lw.generator,
        air_c: w.temperature_c as f32,
        humidity: w.humidity as f32,
        wind_m_s: w.wind_speed_m_s as f32,
    }
    .natural(BlockPos::containing(pos));
    let falling = match w.precip {
        hearth_env::weather::Precip::None => 0.0,
        hearth_env::weather::Precip::Rain => w.precip_mm_h,
        hearth_env::weather::Precip::Sleet => 0.7 * w.precip_mm_h,
        // Snow wets far less than rain.
        hearth_env::weather::Precip::Snow => 0.2 * w.precip_mm_h,
    };
    Exposure {
        air_c: w.temperature_c as f32,
        humidity: w.humidity as f32,
        wind_m_s: (w.wind_speed_m_s * if covered { 0.3 } else { 1.0 } * lee(lw, pos)) as f32,
        rain_mm_h: falling as f32 * rain_through,
        immersion: immersion as f32,
        water_c: water.temperature_c,
        radiant_w_m2: radiant as f32,
        sky_c_offset: if covered {
            0.0
        } else if night {
            (-8.0 * clear) as f32
        } else {
            (-2.0 * clear) as f32
        },
        ground_clo: 0.0,
        local_hour: (env.local_time(moment, pos.x) * 24.0) as f32,
        disturbance: 0.0,
        // Blocks are metres up and down on Earth; a test planet's are taller.
        altitude_m: (pos.y / lw.generator.terrain.vertical_scale() as f64) as f32,
    }
}

/// The physics report the body reads, from the client's movement report.
fn report_of(m: &Moved) -> Report {
    Report {
        motion: m.motion,
        speed: m.speed,
        landed: m.landed,
        immersion: m.immersion,
        eyes_under: m.airless_s > 0.0,
        airless_s: m.airless_s,
        straining: m.straining,
    }
}

fn body_view(
    cfg: &BodyConfig,
    p: &Player,
    exposure: Exposure,
    rate: f64,
    items: &hearth_items::Items,
    mu: f32,
) -> BodyView {
    let load = p.carry.load(items, cfg.mass_kg as f32);
    let mut ability = p.ability_with(cfg, &load, mu);
    // A boat dragged into the water is sat in and paddled (V2-12): as fast as the paddler is
    // strong.
    if p.carry
        .dragging
        .as_ref()
        .and_then(|s| s.property(items, "boat"))
        .is_some_and(|b| b > 0.0)
    {
        let params = &cfg.params;
        ability.boat_m_s =
            1.8 * (ability.swim_m_s / params.swim_m_s.max(0.1) as f64).clamp(0.3, 1.2);
    }
    BodyView {
        status: p.body.status(cfg),
        ability,
        asleep: p.asleep,
        lying: p.lying,
        rate,
        dead: p.body.dead.clone(),
        injuries: p.body.injuries.clone(),
        illnesses: p
            .body
            .illnesses
            .iter()
            .filter(|i| i.active())
            .map(|i| i.id.clone())
            .collect(),
        sleepy_in_h: p
            .body
            .sleep
            .hours_until(exposure.local_hour as f64, hearth_player::SLEEPY)
            .map(|h| h as f32),
        exposure,
    }
}

/// Saves the player, the clock, the things lying about and what is built.
fn save(
    save: &mut Option<Save>,
    player: &Player,
    appearance: &hearth_character::Appearance,
    past_lives: &std::collections::BTreeSet<String>,
    world_items: &hearth_items::WorldItems,
    workshop: &Workshop,
    lw: &LocalWorld,
    fauna: &crate::fauna::Fauna,
    ticks: u64,
) {
    let Some(s) = save else {
        return;
    };
    fauna.save(&s.dir.root);
    s.meta.clock.ticks = ticks;
    s.meta.last_played_unix = hearth_save::meta::unix_now();
    let player = PlayerSave {
        format: PLAYER_FORMAT,
        player: player.clone(),
        appearance: appearance.clone(),
        past_lives: past_lives.clone(),
    };
    if let Err(e) = s
        .dir
        .save_meta(&s.meta)
        .and_then(|()| s.dir.write_json("player.json", &player))
        .and_then(|()| s.dir.write_json("items.json", world_items))
        .and_then(|()| s.dir.write_json("crafts.json", &workshop.save()))
        .and_then(|()| s.dir.write_json("blocks.json", &lw.edits.save(&lw.reg)))
        .and_then(|()| s.dir.write_json("vegetation.json", &lw.vegetation.save()))
    {
        log::error!("could not save the world: {e}");
    } else {
        log::info!("saved world {:?} at tick {ticks}", s.meta.name);
    }
}

fn run(
    spec: WorldSpec,
    atlas: Arc<TextureArray>,
    mut view: View,
    inbox: Receiver<ToServer>,
    tx: &Sender<ToClient>,
) -> anyhow::Result<()> {
    // A saved world keeps its own planet.
    let mut gen_settings = spec.shape.planet(spec.seed, spec.planet);
    if let Some(saves) = &spec.saves_dir {
        let root = saves.join(&spec.name);
        if root.join("level.json").exists() {
            let (_, meta, _) = WorldDir::open(&root)?;
            gen_settings = meta.settings.planet.clone();
        }
    }
    let seed = gen_settings.seed;
    let progress = |share: f32, stage: &str| {
        let _ = tx.send(ToClient::Progress {
            share,
            stage: stage.to_owned(),
        });
    };
    progress(0.0, "menu.making.opening");
    let mut lw = LocalWorld::create_with(&gen_settings, spec.cache_dir.as_deref(), &|f, stage| {
        log::debug!("planet {:.0}% {stage}", f * 100.0);
        progress(f, stage);
    })?;
    let planet = *lw.map.planet();
    let mut save_state = open_save(&spec, &lw)?;

    // The body's world: the world's realism.
    let content = lw.content.clone();
    let (life, mut ticks) = match &save_state {
        Some(s) => (s.meta.settings.life.clone(), s.meta.clock.ticks),
        None => (new_life(&spec, &content), 0),
    };
    // The world's mode: what may be done beyond living (watching, time, Creative's powers).
    let rules = mode_rules(
        &content,
        match &save_state {
            Some(s) => s.meta.settings.mode.as_deref(),
            None => spec.mode.as_deref(),
        },
    );
    // Watching the world, and moving its time and weather: Creative's, or a world of no mode.
    let free = rules.as_ref().is_none_or(|r| r.observer);
    let creative = rules.as_ref().is_some_and(|r| r.creative);
    // Creative's instant actions (on by default, Amendment P §3.1).
    let mut instant = true;
    let balance = Balance::resolve(&content, &life.realism.preset, &life.realism.overrides);
    let cfg = Arc::new(BodyConfig::new(&content, &balance, &life.realism.preset));

    // The calendar starts when the world was made, set to a spring morning at the world's first
    // spawn (where the player chose on the globe, else where the world finds a place) or to the
    // moment it was made.
    let birthplace = save_state.as_ref().map_or(spec.birthplace, |s| {
        s.meta.settings.birthplace.map(|[x, z]| DVec2::new(x, z))
    });
    let place = |at: DVec2| birthplace.unwrap_or(at);
    let first_spawn = first_spawn(&lw, Some(&place));
    let created = save_state.as_ref().map(|s| s.meta.created_unix);
    let calendar = calendar_of(&planet, life.start, created, first_spawn);
    let mut env = EnvSampler::new(lw.grid(), calendar);

    let saved =
        save_state
            .as_ref()
            .and_then(|s| match s.dir.read_json::<PlayerSave>("player.json") {
                Ok(p) => p,
                Err(e) => {
                    log::error!("player.json unreadable ({e}); starting anew");
                    None
                }
            });
    // Who the player is: the adult they chose to be (Amendment E §6.1), or as saved.
    // A life begun now wakes lying in the grass (Amendment E §6.5) and lies until it gets up.
    let mut waking = saved.is_none();
    let (mut player, mut appearance, mut past_lives) = match saved {
        Some(p) => (p.player, p.appearance.sanitized(), p.past_lives),
        None => (
            Player::new(&cfg, first_spawn, seed ^ 0x5eed),
            spec.appearance.clone().sanitized(),
            Default::default(),
        ),
    };
    let items = Arc::new(hearth_items::Items::from_content(&content));
    // What a new person starts in: the loincloth (with a chest band for a female body, D70) of
    // their chosen material, as things they wear.
    let outfit = |a: &hearth_character::Appearance| {
        let material = match a.loincloth {
            hearth_character::Loincloth::Hide => "hearth:rawhide",
            hearth_character::Loincloth::PlantFibre => "hearth:nettle_fibre",
        };
        let mut garments = vec!["hearth:loincloth"];
        if a.body == hearth_character::BodyType::Female {
            garments.push("hearth:chest_band");
        }
        let stacks: Vec<hearth_items::Stack> = garments
            .iter()
            .filter_map(|g| items.garment(g, material))
            .map(|k| hearth_items::Stack::one(&k.id))
            .collect();
        hearth_items::Carry::dressed(&items, stacks)
    };
    player.lying |= waking;
    // Saves from before carrying wore nothing: dress them.
    if player.carry.worn.is_empty() {
        player.carry = outfit(&appearance);
    }
    let dress_carry = |c: &hearth_items::Carry| {
        Worn::of(
            c.garments(&items)
                .iter()
                .filter_map(|g| content.garments.get(g)),
        )
    };
    let mut worn = dress_carry(&player.carry);
    let mut carry_sent: Option<hearth_items::Carry> = None;
    // The things lying in the world; where the player was when they were last told of them.
    let mut world_items: hearth_items::WorldItems = save_state
        .as_ref()
        .and_then(|s| s.dir.read_json("items.json").ok().flatten())
        .unwrap_or_default();
    let mut items_told: Option<DVec3> = None;
    let mut items_changed = true;
    // Blocks changed by the player since the last tick.
    let mut gathered: Vec<BlockPos> = Vec::new();
    // What a new life keeps of what earlier lives knew (Amendment E §6.6).
    let kept = match life.after_death {
        hearth_save::AfterDeath::TheirsOnly => hearth_craft::Kept::TheirsOnly,
        hearth_save::AfterDeath::HeadStart => hearth_craft::Kept::HeadStart,
        hearth_save::AfterDeath::KeepEverything => hearth_craft::Kept::Everything,
    };
    let mut death_told = player.body.dead.is_some();
    // Where the player last died: a new life may begin near it.
    let mut died_at: Option<DVec3> = player.body.dead.is_some().then_some(player.mover.pos);
    // Making and knowing: how knowledge is gained here, the stations and fires standing.
    let mode = match life.knowledge_mode {
        hearth_save::KnowledgeMode::Discovery => hearth_craft::Mode::Discovery,
        hearth_save::KnowledgeMode::Guided => hearth_craft::Mode::Guided,
        hearth_save::KnowledgeMode::Open => hearth_craft::Mode::Open,
    };
    let workshop_save: Option<WorkshopSave> = save_state
        .as_ref()
        .and_then(|s| s.dir.read_json("crafts.json").ok().flatten());
    // The blocks the player changed, laid over the terrain as it streams in.
    if let Some(e) = save_state.as_ref().and_then(|s| {
        s.dir
            .read_json::<crate::edits::EditsSave>("blocks.json")
            .ok()
            .flatten()
    }) {
        lw.edits = crate::edits::Edits::load(e, &lw.reg);
    }
    // The vegetation: what has been felled, cleared and burned, grown to the calendar's year.
    let vegetation: hearth_worldgen::vegetation::VegetationSave = save_state
        .as_ref()
        .and_then(|s| s.dir.read_json("vegetation.json").ok().flatten())
        .unwrap_or_default();
    lw.vegetation = hearth_worldgen::vegetation::Vegetation::new(
        &vegetation,
        planet.circumference(),
        vegetation_year(&calendar, ticks),
    );
    let mut workshop = Workshop::new(&content, &items, mode, workshop_save, seed, ticks);
    // The animals: the populations about the player, saved with the world.
    let years_at = |t: u64| calendar.years(t);
    let mut fauna = crate::fauna::Fauna::new(
        &lw,
        seed,
        calendar.year_offset(),
        years_at(ticks),
        save_state.as_ref().map(|s| s.dir.root.as_path()),
    );
    let mut animals_shown = false;
    // Watching the world (Creative's spectating): where its eye is.
    let mut observing: Option<DVec3> = None;
    // How readily the animals turn on people: the world's Predator Behavior setting.
    fauna.live.aggression = match life.predator_behavior {
        hearth_save::PredatorBehavior::Authentic => 1.0,
        hearth_save::PredatorBehavior::Wild => 2.5,
        hearth_save::PredatorBehavior::Tranquil => 0.15,
    };
    // The tick of the player's last shout.
    let mut shouted = 0u64;
    // Signs were last sent (an empty list is sent once when they go).
    let mut signs_shown = false;
    if mode == hearth_craft::Mode::Open {
        player.knowledge.known = hearth_craft::KnowledgeState::open(&workshop.graph, ticks).known;
    }
    let authentic = life.realism.preset == "authentic";
    // Messages the workshop has for the client.
    let mut outbox: Vec<ToClient> = Vec::new();
    // Whether the client was last told of smoke; the vegetation it was last told of.
    let mut smoke_shown = false;
    let mut veg_told: Option<Vegetation> = None;
    let mut edits_told: Option<u64> = None;
    // Game ticks the last server tick moved the clock by.
    let mut advanced = 1.0f64;
    // A rest under way (E §4.3), and how much faster it has the world go.
    let mut resting: Option<crate::rest::Resting> = None;
    let mut rest_speed = 0.0f64;
    let sleep = content.time.sleep;

    let lod = Arc::new(hearth_lod::LodGen::new(
        &lw.reg,
        &hearth_texgen::textures_for(Some(&lw.content)),
    ));
    if tx
        .send(ToClient::Ready(Box::new(Ready {
            planet,
            grid: lw.grid(),
            generator: lw.generator.clone(),
            lod,
            reg: lw.reg.clone(),
            body: cfg.clone(),
            vertical_scale: lw.terrain().vertical_scale(),
            calendar,
            ticks,
            player: player.mover,
            appearance: appearance.clone(),
            after_death: life.after_death,
            items: items.clone(),
            content: content.clone(),
            crafts: workshop.crafts.clone(),
            graph: workshop.graph.clone(),
            knowledge_mode: mode,
            mode: rules.as_ref().map(|r| r.id.clone()),
            lod_cache: spec.cache_dir.as_ref().map(|d| {
                d.join("lod")
                    .join(format!("{seed}_{}", planet.circumference()))
            }),
        })))
        .is_err()
    {
        return Ok(());
    }
    let models = BlockModels::build(&lw.reg, &atlas);
    let opts = MeshOptions::default();
    let mut stream = Stream::default();
    let mut water = WaterSim::new(&lw.reg)?;
    // What is built, standing or falling (V2-8 (b)), and the day its weather was last told.
    let mut structures = crate::structure::Structures::new(&lw.reg, &lw.content);
    let mut weathered_day = (ticks as f64 / calendar.ticks_per_day()) as u64;
    let mut stress_told = false;
    let mut last_moved: Option<Moved> = None;
    let mut warp = 0.0f64;
    let mut warp_carry = 0.0f64;
    let mut paused = false;
    // Tests and bots: ticks run only when asked for (lockstep), as fast as they go.
    let mut lockstep = false;
    let mut owed: u64 = 0;
    let mut next_tick = Instant::now();
    let mut since_save = 0u64;
    macro_rules! here {
        () => {
            Here {
                lw: &mut lw,
                items: &items,
                cfg: &cfg,
                env: &env,
                moment: calendar.at(ticks),
                ticks,
                ticks_per_day: calendar.ticks_per_day(),
                days_per_year: calendar.days_per_year(),
                player: &mut player,
                world_items: &mut world_items,
                changed: &mut gathered,
                out: &mut outbox,
                items_changed: &mut items_changed,
                facing: last_moved.as_ref().map_or(0.0, |m| m.yaw),
                fauna: &mut fauna,
            }
        };
    }
    loop {
        // Messages from the client.
        loop {
            let msg = inbox.try_recv();
            // Waking, the player gets up to do anything, or to go anywhere.
            if waking && let Ok(m) = &msg {
                let up = match m {
                    ToServer::Moved(m) => (m.mover.pos - player.mover.pos).length() > 0.25,
                    ToServer::Rest(_) | ToServer::NewLife { .. } => false,
                    _ => true,
                };
                if up {
                    waking = false;
                    player.lying = false;
                }
            }
            match msg {
                Ok(ToServer::Moved(m)) => {
                    if player.body.dead.is_none() {
                        player.life.moved(player.mover.pos, m.mover.pos);
                    }
                    let scale = player.mover.scale;
                    player.mover = m.mover;
                    player.mover.scale = scale;
                    if !player.asleep
                        && !creative
                        && let Some(v) = m.landed
                    {
                        player.body.land(&cfg, v);
                    }
                    if m.airless_s >= DROWN_S && !creative {
                        player.body.kill(hearth_body::Death::Drowning);
                    }
                    last_moved = Some(m);
                }
                Ok(ToServer::Rest(rest)) => {
                    // Lying down to sleep or rest until something (sleep comes if the body is
                    // sleepy); getting up ends it.
                    player.drowsy_s = 0.0;
                    let seen = crate::rest::Seen {
                        ticks,
                        ticks_per_hour: calendar.ticks_per_day() / 24.0,
                        ..Default::default()
                    };
                    let mut ended = None;
                    match rest {
                        Some(r) if player.body.dead.is_none() && observing.is_none() => {
                            let sun_up = env.sun_up(&calendar.at(ticks), player.mover.pos);
                            match crate::rest::Resting::begin(
                                r,
                                ticks,
                                &player,
                                sun_up,
                                &world_items,
                            ) {
                                Ok(begun) => {
                                    player.lying = true;
                                    resting = Some(begun);
                                }
                                Err(end) => {
                                    ended = Some(hearth_protocol::Rested {
                                        hours: 0.0,
                                        slept_h: 0.0,
                                        rest: r,
                                        end,
                                    })
                                }
                            }
                        }
                        _ => {
                            ended = resting
                                .take()
                                .map(|r| r.rested(&seen, hearth_protocol::RestEnd::GotUp));
                            player.lying = false;
                            player.asleep = false;
                            waking = false;
                        }
                    }
                    if let Some(ended) = ended {
                        let _ = tx.send(ToClient::Rested(ended));
                    }
                }
                Ok(ToServer::Place(_)) if !free => {}
                Ok(ToServer::Place(p)) => {
                    let (x, z) = lw
                        .terrain()
                        .spawn_near(p.x.floor() as i32, p.z.floor() as i32);
                    player.mover = Mover::new(ground_at(&lw, x, z));
                    let _ = tx.send(ToClient::Placed(player.mover));
                }
                Ok(ToServer::PickUp(id)) => {
                    let near = world_items
                        .get(id)
                        .is_some_and(|w| within_reach(&player, DVec3::from_array(w.pos)));
                    if player.can_act(&cfg)
                        && near
                        && let Some(w) = world_items.take(id)
                    {
                        let body_kg = cfg.mass_kg as f32;
                        // Into what is carried; too heavy for that, taken hold of to drag.
                        let back = match player.carry.stow(&items, w.stack, body_kg) {
                            Ok(()) => None,
                            Err(stack) => player.carry.drag(&items, stack, body_kg).err(),
                        };
                        let full = back.as_ref().is_some_and(|(s, _)| {
                            s.mass(&items) < hearth_items::carry::ONE_HAND_KG
                        });
                        if let Some((stack, _)) = back {
                            world_items
                                .items
                                .push(hearth_items::WorldItem { stack, ..w });
                        }
                        items_changed = true;
                        if full {
                            workshop.hands_full(&mut here!());
                        }
                    }
                }
                Ok(ToServer::Drag(id)) => {
                    let near = world_items
                        .get(id)
                        .is_some_and(|w| within_reach(&player, DVec3::from_array(w.pos)));
                    if player.can_act(&cfg)
                        && near
                        && let Some(w) = world_items.take(id)
                    {
                        if let Err((stack, _)) =
                            player.carry.drag(&items, w.stack, cfg.mass_kg as f32)
                        {
                            world_items
                                .items
                                .push(hearth_items::WorldItem { stack, ..w });
                        }
                        items_changed = true;
                    }
                }
                Ok(ToServer::PutDown { from, count, at }) => {
                    if player.can_act(&cfg)
                        && within_reach(&player, at)
                        && let Some(stack) = player.carry.take(&items, &from, count)
                    {
                        let rest = rest_on(&lw, at);
                        world_items.add(stack, rest.to_array(), 0.0);
                        worn = dress_carry(&player.carry);
                        items_changed = true;
                    }
                }
                Ok(ToServer::LetGo(at)) => {
                    let at = if within_reach(&player, at) {
                        at
                    } else {
                        player.mover.pos
                    };
                    if let Some(stack) = player.carry.dragging.take() {
                        world_items.add(stack, rest_on(&lw, at).to_array(), 0.0);
                        items_changed = true;
                    }
                }
                Ok(ToServer::Act {
                    process,
                    aim,
                    hand,
                    with,
                }) => {
                    workshop.act(&mut here!(), &process, aim, hand, with);
                    // Creative's instant actions: the work is done as soon as it is begun.
                    if creative
                        && instant
                        && let Some(w) = &mut workshop.work
                    {
                        w.ticks = w.ticks.max(w.needed);
                    }
                }
                Ok(ToServer::Creative { .. } | ToServer::Remove(_)) if !creative => {}
                Ok(ToServer::Creative { act, aim }) => workshop.creative(&mut here!(), &act, aim),
                Ok(ToServer::Remove(aim)) => {
                    workshop.creative_remove(&mut here!(), aim);
                }
                Ok(ToServer::Instant(on)) => instant = on,
                Ok(ToServer::StopWork) => workshop.stop(&mut here!()),
                Ok(ToServer::Inspect { id, numbers }) => {
                    workshop.inspect(&mut here!(), id, numbers);
                }
                Ok(ToServer::Look(aim)) => workshop.look(&mut here!(), aim),
                Ok(ToServer::Eat(path)) => workshop.eat(&mut here!(), &path),
                Ok(ToServer::Drink(from)) => workshop.drink(&mut here!(), &from),
                Ok(ToServer::Fill { skin, aim }) => workshop.fill(&mut here!(), &skin, aim),
                Ok(ToServer::Throw { dir, speed }) => {
                    if let Some(f) = workshop.throw(&mut here!(), dir, speed) {
                        // What it strikes on its way, and where it falls.
                        let year_frac = calendar.at(ticks).year_frac as f32;
                        let cat = fauna.eco.catalog.clone();
                        let (words, end) = crate::strikes::fly(
                            &f,
                            &items,
                            &mut fauna.live,
                            &cat,
                            player.mover.pos,
                            year_frac,
                        );
                        if let Some(words) = words {
                            let _ = tx.send(ToClient::Acted(hearth_protocol::Acted {
                                process: String::new(),
                                done: false,
                                words,
                            }));
                        }
                        world_items.add(f.stack, rest_on(&lw, end).to_array(), 0.0);
                        items_changed = true;
                    }
                }
                Ok(ToServer::Loose { dir, drawn_s }) => {
                    if let Some(f) = workshop.shoot(&mut here!(), dir, drawn_s) {
                        let year_frac = calendar.at(ticks).year_frac as f32;
                        let cat = fauna.eco.catalog.clone();
                        let (words, end) = crate::strikes::fly(
                            &f,
                            &items,
                            &mut fauna.live,
                            &cat,
                            player.mover.pos,
                            year_frac,
                        );
                        if let Some(words) = words {
                            let _ = tx.send(ToClient::Acted(hearth_protocol::Acted {
                                process: String::new(),
                                done: false,
                                words,
                            }));
                        }
                        world_items.add(f.stack, rest_on(&lw, end).to_array(), 0.0);
                        items_changed = true;
                    }
                }
                Ok(ToServer::Blow { dir, kick, with }) => {
                    crate::strikes::begin(&mut player, &items, &cfg, dir, kick, with);
                }
                Ok(ToServer::HoldUp(up)) => player.held_up = up,
                Ok(ToServer::Give(stack)) => {
                    let body_kg = cfg.mass_kg as f32;
                    if items.get(&stack.id).is_some() {
                        let left = match player.carry.stow(&items, stack, body_kg) {
                            Ok(()) => None,
                            Err(s) => player.carry.drag(&items, s, body_kg).err(),
                        };
                        if let Some((s, _)) = left {
                            world_items.add(s, player.mover.pos.to_array(), 0.0);
                            items_changed = true;
                        }
                    }
                }
                Ok(ToServer::Shift { from, count, to }) => {
                    if player.can_act(&cfg) {
                        let body_kg = cfg.mass_kg as f32;
                        if player.carry.shift(&items, &from, count, &to, body_kg) {
                            worn = dress_carry(&player.carry);
                        }
                    }
                }
                Ok(ToServer::NewLife {
                    at,
                    appearance: looks,
                }) => {
                    // A new life (Amendment E §6.6): a new adult about the place chosen (or near
                    // where the last one died); what the one who died carried lies where they
                    // fell. What it keeps of what earlier lives knew is the mode's.
                    if player.body.dead.is_some() {
                        let here = at.unwrap_or_else(|| died_at.unwrap_or(player.mover.pos));
                        let (x, z) = lw
                            .terrain()
                            .spawn_near(here.x.floor() as i32, here.z.floor() as i32);
                        let place = ground_at(&lw, x, z);
                        let fell = player.mover.pos;
                        let left = std::mem::take(&mut player.carry);
                        for (k, stack) in left.into_stacks().into_iter().enumerate() {
                            let a = k as f64 * 2.4;
                            let spot = fell + DVec3::new(a.cos() * 0.6, 0.5, a.sin() * 0.6);
                            world_items.add(stack, rest_on(&lw, spot).to_array(), a as f32);
                        }
                        items_changed = true;
                        let knew = std::mem::take(&mut player.knowledge);
                        workshop.stop(&mut here!());
                        player = Player::new(&cfg, place, seed ^ ticks);
                        player.knowledge = hearth_craft::KnowledgeState::default().lived_on(
                            &knew.journal,
                            &past_lives,
                            kept,
                            &workshop.graph,
                            ticks,
                        );
                        if mode == hearth_craft::Mode::Open {
                            player.knowledge.known =
                                hearth_craft::KnowledgeState::open(&workshop.graph, ticks).known;
                        }
                        workshop.knowledge_changed = true;
                        player.life = hearth_player::Life::begin(place, ticks);
                        waking = true;
                        player.lying = true;
                        if let Some(a) = looks {
                            appearance = a.sanitized();
                            let _ = tx.send(ToClient::Looks(appearance.clone()));
                        }
                        player.carry = outfit(&appearance);
                        worn = dress_carry(&player.carry);
                        death_told = false;
                        died_at = None;
                        let _ = tx.send(ToClient::Placed(player.mover));
                    }
                }
                Ok(ToServer::SkipHours(_) | ToServer::HoldWeather(_)) if !free => {}
                Ok(ToServer::TimeWarp(w)) if !free && w > 0.0 => {}
                Ok(ToServer::Observe(Some(_))) if !free => {}
                Ok(ToServer::SkipHours(h)) => {
                    let dt = h / 24.0 * calendar.ticks_per_day();
                    ticks = (ticks as f64 + dt).max(0.0) as u64;
                }
                Ok(ToServer::TimeWarp(w)) => warp = w.max(0.0),
                Ok(ToServer::Pause(p)) => paused = p,
                Ok(ToServer::Shout) => shouted = ticks,
                Ok(ToServer::Die { species, at }) => fauna.die(&species, at),
                Ok(ToServer::Bring {
                    species,
                    young,
                    female,
                    at,
                }) => {
                    let at = crate::server::rest_on(&lw, at);
                    if fauna.bring(&species, young, female, at).is_none() {
                        log::warn!("no animal {species} to bring");
                    }
                }
                Ok(ToServer::Observe(eye)) => observing = eye,
                Ok(ToServer::Census) => {
                    let _ = tx.send(ToClient::Census(fauna.census()));
                }
                Ok(ToServer::Run(n)) => {
                    lockstep = true;
                    owed += n;
                }
                Ok(ToServer::Strike { x, z }) => {
                    workshop.strike(&mut here!(), x, z);
                }
                Ok(ToServer::HoldWeather(hold)) => env.hold = hold,
                Ok(ToServer::Ignite(p)) => {
                    workshop.ignite_at(&mut here!(), p);
                }
                Ok(ToServer::Disturb { kind, x, z, radius }) => {
                    let year = vegetation_year(&calendar, ticks);
                    lw.vegetation = lw.vegetation.at_year(year).with(
                        hearth_worldgen::vegetation::Disturbance {
                            kind,
                            year,
                            x,
                            z,
                            radius,
                            severity: 1.0,
                            patches: Vec::new(),
                        },
                    );
                }
                Ok(ToServer::View { radius, vertical }) => {
                    view = View {
                        radius: radius.clamp(1, 64),
                        vertical: vertical.clamp(1, 32),
                    };
                }
                Ok(ToServer::Save) => {
                    save(
                        &mut save_state,
                        &player,
                        &appearance,
                        &past_lives,
                        &world_items,
                        &workshop,
                        &lw,
                        &fauna,
                        ticks,
                    );
                    let _ = tx.send(ToClient::Saved);
                }
                Ok(ToServer::Quit) | Err(TryRecvError::Disconnected) => {
                    save(
                        &mut save_state,
                        &player,
                        &appearance,
                        &past_lives,
                        &world_items,
                        &workshop,
                        &lw,
                        &fauna,
                        ticks,
                    );
                    let _ = tx.send(ToClient::Saved);
                    return Ok(());
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        for m in outbox.drain(..) {
            let _ = tx.send(m);
        }

        // The tick (none while paused: the world stands still, the terrain still streams).
        if paused {
            next_tick = Instant::now() + Duration::from_secs_f64(TICK_S);
        } else if (lockstep && owed > 0) || (!lockstep && Instant::now() >= next_tick) {
            workshop.tick(&mut here!(), advanced);
            if creative {
                creative_body(&mut player, &cfg, seed ^ ticks, ticks);
            }
            {
                let moment = calendar.at(ticks);
                // The world about the player — or about the Observer's eye, unseen there.
                let at = observing.unwrap_or(player.mover.pos);
                let now = hearth_fauna::live::Now {
                    hour: env.local_time(&moment, at.x) as f32,
                    air: env.air_at(&moment, at),
                    year_frac: moment.year_frac as f32,
                    southern: lw.map.planet().latitude(at.z) < 0.0,
                };
                let hurt = player
                    .body
                    .injuries
                    .iter()
                    .filter(|i| i.healed < 1.0)
                    .count();
                let mut presence = fauna.presence_of(
                    &player.mover,
                    last_moved.as_ref().map_or(0.0, |m| m.yaw),
                    ticks.saturating_sub(shouted) < 20,
                    hurt,
                    now.air.light,
                    &lw.map,
                    &lw.reg,
                );
                // A burning brand held up keeps hungry animals off as a fire does.
                presence.by_fire |= crate::strikes::brand_up(&player, &items);
                if observing.is_some() {
                    // An Observer is perceived by nothing.
                    presence.pos = at;
                    presence.noise = 0.0;
                    presence.plain = 0.0;
                    presence.shouting = false;
                    presence.running = false;
                    presence.vulnerable = 0.0;
                }
                // The animals live the world's seconds of this tick (more of them resting).
                let lived_s = (TICK_S * advanced) as f32;
                fauna.tick(&lw, &presence, &now, years_at(ticks), lived_s, ticks);
                // A blow under way strikes at the end of its wind-up.
                if let Some(striking) = player.striking.as_mut() {
                    let now_strikes = striking.advance(TICK_S);
                    let done = striking.done();
                    if now_strikes {
                        let cat = fauna.eco.catalog.clone();
                        if let Some(words) = crate::strikes::land(
                            &player,
                            &items,
                            &cfg,
                            &mut fauna.live,
                            &cat,
                            now.year_frac,
                        ) {
                            let _ = tx.send(ToClient::Acted(hearth_protocol::Acted {
                                process: String::new(),
                                done: false,
                                words,
                            }));
                        }
                    }
                    if done {
                        player.striking = None;
                    }
                }
                // The kept animals' doings near the player, told.
                for t in std::mem::take(&mut fauna.tidings) {
                    use hearth_fauna::herd::Tiding;
                    let cat = &fauna.eco.catalog;
                    let (at, words) = match t {
                        Tiding::Born {
                            species, young, at, ..
                        } => (
                            at,
                            format!(
                                "One of your {}s has given birth: {}.",
                                cat.species[species as usize].name.to_lowercase(),
                                match young {
                                    1 => "a single young one".to_owned(),
                                    2 => "twins".to_owned(),
                                    n => format!("{n} young"),
                                }
                            ),
                        ),
                        Tiding::Died { species, at } => (
                            at,
                            format!(
                                "One of your {}s has died of old age.",
                                cat.species[species as usize].name.to_lowercase()
                            ),
                        ),
                    };
                    if (at - player.mover.pos).length() < 120.0 {
                        outbox.push(ToClient::Acted(hearth_protocol::Acted {
                            process: "herd".into(),
                            done: true,
                            words,
                        }));
                    }
                }
                // What the animals called, in the world and about it.
                let mut calls = std::mem::take(&mut fauna.live.calls);
                calls.extend(fauna.chorus(at, &now, TICK_S as f32));
                if !calls.is_empty() {
                    let _ = tx.send(ToClient::Calls(calls));
                }
                // What the animals did to the player: hurt them, or made them stop and think;
                // and why.
                for at in fauna.live.attacks.clone() {
                    if !at.injury.is_empty() && player.body.dead.is_none() {
                        let side = if at.animal % 2 == 0 {
                            hearth_body::Side::Left
                        } else {
                            hearth_body::Side::Right
                        };
                        player
                            .body
                            .injure(&cfg, at.injury, at.region, side, at.severity);
                        if at.venom {
                            player.body.catch_illness(&cfg, "envenomation");
                        }
                    }
                    let _ = tx.send(ToClient::Acted(hearth_protocol::Acted {
                        process: String::new(),
                        done: false,
                        words: at.words.clone(),
                    }));
                }
                // The dead lie where they fell, as carcasses; the remains of the populations'
                // dead come into the world as the player comes near, and ravens circling by day
                // tell of the fresh ones within sight.
                let (mut lying, fallen) = fauna.carcasses(&items, at);
                for words in fallen {
                    let _ = tx.send(ToClient::Acted(hearth_protocol::Acted {
                        process: String::new(),
                        done: false,
                        words,
                    }));
                }
                if ticks.is_multiple_of(40) {
                    let air_c = env.weather_at(&moment, at).temperature_c as f32;
                    let content = lw.content.clone();
                    let days = calendar.days_per_year();
                    lying.extend(fauna.found(&items, &content, &lw, at, 90.0, air_c, days));
                    if now.air.light > 0.3
                        && let Some(way) = fauna.ravens(at)
                    {
                        let _ = tx.send(ToClient::Acted(hearth_protocol::Acted {
                            process: String::new(),
                            done: false,
                            words: format!(
                                "Ravens are circling to the {}.",
                                crate::workshop::compass(way.x, way.y)
                            ),
                        }));
                    }
                }
                for (stack, pos, yaw) in lying {
                    world_items.add(stack, rest_on(&lw, pos).to_array(), yaw);
                    items_changed = true;
                }
                // The signs the animals left about the player; prints underfoot teach tracking.
                if ticks.is_multiple_of(20) {
                    let feet = player.mover.pos;
                    let near: Vec<hearth_fauna::live::Sign> = fauna
                        .live
                        .signs
                        .iter()
                        .filter(|s| (s.pos.x - feet.x).hypot(s.pos.z - feet.z) < 64.0)
                        .copied()
                        .collect();
                    let clock = fauna.live.clock;
                    let fresh_print = near.iter().any(|s| {
                        s.kind == hearth_fauna::live::SignKind::Print
                            && (s.pos - feet).length() < 2.5
                            && clock - s.t < hearth_content::time::DAY_S
                    });
                    if !near.is_empty() || signs_shown {
                        signs_shown = !near.is_empty();
                        let _ = tx.send(ToClient::Signs {
                            now: clock,
                            signs: near,
                        });
                    }
                    if fresh_print {
                        let hour = (calendar.ticks_per_day() / 24.0) as u64;
                        workshop.hear_now_and_then(
                            &mut here!(),
                            hearth_content::triggers::SEE_TRACKS.to_owned(),
                            hour,
                        );
                    }
                }
                if ticks.is_multiple_of(2) {
                    let views = fauna.views();
                    if !views.is_empty() || animals_shown {
                        animals_shown = !views.is_empty();
                        let _ = tx.send(ToClient::Animals(views));
                    }
                }
            }
            // The vegetation for the distant terrain: when it changes, and as the years turn.
            if ticks.is_multiple_of(20)
                && veg_told.as_ref().is_none_or(|v| {
                    !v.same_disturbances(&lw.vegetation)
                        || v.year.floor() != lw.vegetation.year.floor()
                })
            {
                veg_told = Some(lw.vegetation.clone());
                let _ = tx.send(ToClient::Vegetation(lw.vegetation.clone()));
            }
            // The player's changes for the distant terrain, when they change.
            if ticks.is_multiple_of(20) && edits_told != Some(lw.edits.version()) {
                edits_told = Some(lw.edits.version());
                let tops = lw.edits.tops(&lw.reg, |x| planet.wrap_x(x));
                let _ = tx.send(ToClient::EditTops(Arc::new(tops)));
            }
            // The smoke over fires in the vegetation, for the client to draw.
            if ticks.is_multiple_of(20) && (workshop.fire_burning() || smoke_shown) {
                let plumes: Vec<hearth_protocol::Plume> = workshop
                    .plumes(&here!())
                    .into_iter()
                    .map(|p| hearth_protocol::Plume {
                        at: p.at,
                        strength: p.strength,
                        far: p.far,
                    })
                    .collect();
                smoke_shown = !plumes.is_empty();
                let _ = tx.send(ToClient::Smoke(plumes));
            }
            let moment = calendar.at(ticks);
            let immersion = last_moved.map_or(0.0, |m| m.immersion);
            let mut e = exposure(&env, &lw, &moment, &player.mover, immersion);
            // Fires warm those beside them; bedding keeps the ground's cold off a sleeper.
            e.radiant_w_m2 += workshop.radiant_w_m2(player.mover.pos + DVec3::new(0.0, 0.9, 0.0));
            // Within walls the wind is broken and a fire warms the air (V2-8 (e)).
            let eye = player.mover.pos + DVec3::new(0.0, player.mover.stance.height() - 0.2, 0.0);
            let shelter = crate::building::shelter(&lw.map, &lw.reg, &lw.content, eye);
            e.wind_m_s *= shelter.wind_share();
            e.air_c += shelter.warming_c(workshop.fire_kw_near(eye, 3.0));
            if player.lying || player.asleep {
                e.ground_clo = e.ground_clo.max(workshop.bedding_clo(player.mover.pos));
            }
            let report = last_moved.as_ref().map(report_of).unwrap_or_default();
            // Watching the world, a living player is put aside: its body still, unharmed.
            if player.body.dead.is_none() && observing.is_none() {
                let load = player.carry.load(&items, cfg.mass_kg as f32);
                let mu = drag_friction(
                    &lw,
                    player.mover.pos,
                    player.carry.dragging.as_ref(),
                    &items,
                );
                let mut activity = player.activity_with(&cfg, &report, &load, mu);
                if player.asleep || player.lying {
                    activity.posture = Posture::Lying;
                }
                // Work has its own cost.
                if let Some(mets) = workshop.work_mets() {
                    activity.met = activity.met.max(mets);
                }
                // Time going faster passes for the body too.
                let step_s = TICK_S * (1.0 + (warp + rest_speed) / 20.0);
                let steps = (step_s / BODY_STEP_S).ceil().max(1.0);
                for _ in 0..steps as usize {
                    player.body.step(&cfg, step_s / steps, &e, &worn, &activity);
                }
                let hour = env.local_time(&moment, player.mover.pos.x) * 24.0;
                let woke = player.rest(&cfg, &e, hour, step_s);
                if let Some(r) = &mut resting {
                    let seen = crate::rest::Seen {
                        ticks,
                        ticks_per_hour: calendar.ticks_per_day() / 24.0,
                        sun_up: env.sun_up(&moment, player.mover.pos),
                        woke,
                        needs: if player.asleep {
                            None
                        } else {
                            player.body.wakes(&cfg, &e)
                        },
                        animal: crate::rest::animal_near(
                            &fauna.live,
                            &fauna.eco.catalog,
                            player.mover.pos,
                        ),
                    };
                    let step_ticks = (step_s / TICK_S).round() as u64;
                    match r.tick(&player, &world_items, step_ticks, &seen) {
                        Some(end) => {
                            let _ = tx.send(ToClient::Rested(r.rested(&seen, end)));
                            resting = None;
                            player.lying = false;
                            player.asleep = false;
                        }
                        // Up rested in the night, a sleep until morning lies on.
                        None => player.lying = true,
                    }
                }
            } else {
                player.asleep = false;
                player.lying = waking;
                resting = None;
            }
            // Resting, the world speeds up smoothly; up, it slows back.
            rest_speed = crate::rest::ease(
                rest_speed,
                resting.is_some(),
                sleep.max_factor,
                sleep.ramp_s,
                TICK_S,
            );
            // A death, once: what this life knew joins what the player's lives have known (a new
            // life keeps it in Easy, Amendment E §6.6), and where it ended.
            if !death_told && player.body.dead.is_some() {
                death_told = true;
                past_lives.extend(player.knowledge.known.keys().cloned());
                died_at = Some(player.mover.pos);
            }
            warp_carry += (warp + rest_speed) * TICK_S;
            let extra = warp_carry.floor();
            warp_carry -= extra;
            ticks += 1 + extra as u64;
            advanced = 1.0 + extra;
            // In lockstep, the game ticks asked for (warped ticks count as they pass).
            owed = owed.saturating_sub(1 + extra as u64);
            // Skills unused for long slip (Authentic), once a game day.
            let day = calendar.ticks_per_day().max(1.0) as u64;
            if authentic && (ticks / day) != ((ticks - 1 - extra as u64) / day) {
                player
                    .knowledge
                    .slip_skills(ticks, calendar.ticks_per_day());
            }
            since_save += 1;
            // The finite water moves ten times a second; its slow changes every five game
            // minutes.
            let world_water = crate::water_env::WorldWater {
                generator: &lw.generator,
                air_c: e.air_c,
                humidity: e.humidity,
                wind_m_s: e.wind_m_s,
            };
            let mut changed: Vec<BlockPos> = std::mem::take(&mut gathered);
            // What the player changed is what structures and the ground answer to (the water's
            // own flow opens nothing).
            structures.changed(&changed);
            if ticks.is_multiple_of(2) {
                changed.extend_from_slice(water.tick(&mut lw.map, &lw.reg, &world_water));
            }
            // (Crossed, not landed on: the clock moves many ticks at a time resting.)
            let weather_every = (calendar.ticks_per_day() / 288.0).max(1.0) as u64;
            if ticks / weather_every != (ticks - 1 - extra as u64) / weather_every {
                let days = weather_every as f64 / calendar.ticks_per_day();
                water.weather(&mut lw.map, &lw.reg, &world_water, days as f32);
                changed.extend_from_slice(water.changed());
            }
            // A day's weather on what was built: rot, wash and thaw, a stage at a time.
            let day = (ticks as f64 / calendar.ticks_per_day()) as u64;
            if day != weathered_day {
                weathered_day = day;
                let worn = structures.weather(
                    &lw.map,
                    &lw.reg,
                    lw.edits.places().into_iter(),
                    e.air_c,
                    calendar.days_per_year() as f32,
                    day,
                );
                let reg = lw.reg.clone();
                for (p, s) in worn {
                    lw.map.set_block(p, s, &reg);
                    lw.edits.set(p, s);
                    changed.push(p);
                    structures.changed(&[p]);
                }
            }
            // What was built stands or falls: what fails breaks, half of it lying where it
            // fell; ground over too wide an opening falls in, loose, to the floor under it.
            // What either held is reckoned on the next tick.
            let fell = structures.tick(&lw.map, &lw.reg);
            if !fell.pieces.is_empty() || !fell.ground.is_empty() {
                let mut told = Vec::new();
                let mut moved = Vec::new();
                let reg = lw.reg.clone();
                for p in &fell.pieces {
                    let Some(s) = lw.map.block(*p) else {
                        continue;
                    };
                    let b = reg.block_of(s);
                    let (name, material) = (b.name.to_string(), b.def.material.clone());
                    lw.map.set_block(*p, BlockStateId::AIR, &reg);
                    lw.edits.set(*p, BlockStateId::AIR);
                    moved.push(*p);
                    told.push((*p, s));
                    for (id, n) in crate::structure::debris(&lw.content, &name, material.as_deref())
                    {
                        if items.get(&id).is_some() {
                            let at = rest_on(&lw, p.center());
                            world_items.add(hearth_items::Stack::of(&id, n), at.to_array(), 0.0);
                            items_changed = true;
                        }
                    }
                }
                for p in &fell.ground {
                    let Some(s) = lw.map.block(*p) else {
                        continue;
                    };
                    lw.map.set_block(*p, BlockStateId::AIR, &reg);
                    lw.edits.set(*p, BlockStateId::AIR);
                    moved.push(*p);
                    told.push((*p, s));
                    // It lands on the floor of the opening, broken and loose.
                    let mut floor = p.down();
                    for _ in 0..64 {
                        let open = lw.map.block(floor).is_some_and(|b| {
                            b.is_air() || {
                                let d = &reg.block_of(b).def;
                                d.replaceable || d.fluid.is_some()
                            }
                        });
                        if !open {
                            break;
                        }
                        floor = floor.down();
                    }
                    let at = floor.up();
                    if at != *p && lw.map.block(at).is_some() {
                        let loose = crate::structure::fallen(&reg, s);
                        lw.map.set_block(at, loose, &reg);
                        lw.edits.set(at, loose);
                        moved.push(at);
                    }
                }
                changed.extend_from_slice(&moved);
                structures.changed(&moved);
                let _ = tx.send(ToClient::Collapse(told));
            }
            // Each second: the pieces about the player not yet reckoned since their land loaded
            // are reckoned, and how hard those reckoned are pressed is told (V2-8 (f)).
            if ticks.is_multiple_of(20) {
                let here = BlockPos::containing(player.mover.pos);
                let near = |p: &BlockPos| {
                    (p.x - here.x).abs() <= 48
                        && (p.y - here.y).abs() <= 48
                        && (p.z - here.z).abs() <= 48
                };
                let unreckoned: Vec<BlockPos> = lw
                    .edits
                    .places()
                    .into_iter()
                    .filter(|p| near(p) && !structures.stress.contains_key(p))
                    .filter(|p| lw.map.block(*p).is_some_and(|s| structures.is_piece(s)))
                    .collect();
                structures.changed(&unreckoned);
                let stress = structures.stress_near(here, 48);
                if !stress.is_empty() || stress_told {
                    stress_told = !stress.is_empty();
                    let _ = tx.send(ToClient::Stress(stress));
                }
            }
            if !changed.is_empty()
                && stream
                    .blocks_changed(&mut lw, &models, opts, &changed, tx)
                    .is_err()
            {
                save(
                    &mut save_state,
                    &player,
                    &appearance,
                    &past_lives,
                    &world_items,
                    &workshop,
                    &lw,
                    &fauna,
                    ticks,
                );
                return Ok(());
            }
            // The things lying near the player: told when they change or the player goes far.
            let moved_far = items_told.is_none_or(|p| (p - player.mover.pos).length() > 16.0);
            if items_changed || moved_far {
                items_changed = false;
                items_told = Some(player.mover.pos);
                let near: Vec<hearth_items::WorldItem> = world_items
                    .items
                    .iter()
                    .filter(|w| {
                        let d = DVec3::from_array(w.pos) - player.mover.pos;
                        d.x * d.x + d.z * d.z < 96.0 * 96.0
                    })
                    .cloned()
                    .collect();
                let _ = tx.send(ToClient::Items(near));
            }
            if workshop.knowledge_changed {
                workshop.knowledge_changed = false;
                let _ = tx.send(ToClient::Knowledge(Box::new(player.knowledge.clone())));
            }
            for m in outbox.drain(..) {
                let _ = tx.send(m);
            }
            if carry_sent.as_ref() != Some(&player.carry) {
                carry_sent = Some(player.carry.clone());
                let _ = tx.send(ToClient::Carried(player.carry.clone()));
            }
            if tx.send(ToClient::Clock(ticks)).is_err()
                || tx
                    .send(ToClient::Body(Box::new(body_view(
                        &cfg,
                        &player,
                        e,
                        20.0 + warp + rest_speed,
                        &items,
                        drag_friction(
                            &lw,
                            player.mover.pos,
                            player.carry.dragging.as_ref(),
                            &items,
                        ),
                    ))))
                    .is_err()
            {
                save(
                    &mut save_state,
                    &player,
                    &appearance,
                    &past_lives,
                    &world_items,
                    &workshop,
                    &lw,
                    &fauna,
                    ticks,
                );
                return Ok(());
            }
            if since_save >= AUTOSAVE_TICKS {
                since_save = 0;
                save(
                    &mut save_state,
                    &player,
                    &appearance,
                    &past_lives,
                    &world_items,
                    &workshop,
                    &lw,
                    &fauna,
                    ticks,
                );
                let _ = tx.send(ToClient::Saved);
            }
            next_tick += Duration::from_secs_f64(TICK_S);
            if Instant::now() > next_tick + Duration::from_secs(1) {
                // Far behind (a long batch, a breakpoint): don't run the missed ticks at once.
                next_tick = Instant::now();
            }
            // In lockstep (tests), the terrain streams between every tick.
            if !lockstep {
                continue;
            }
        }

        // Terrain around the player between ticks, grown to the calendar's year.
        let year_frac = calendar.at(ticks).year_frac;
        lw.vegetation.year = vegetation_year(&calendar, ticks);
        // Watching at a month a second or faster: a day of the world or more a tick.
        stream.hold_growth = observing.is_some() && warp * TICK_S >= calendar.ticks_per_day();
        let worked = stream
            .work(
                &mut lw,
                &mut water,
                &models,
                opts,
                planet,
                observing.unwrap_or(player.mover.pos),
                view,
                year_frac,
                tx,
            )
            .map_err(|_| anyhow::anyhow!("client gone"));
        match worked {
            Ok((true, covered)) => {
                // The cover's changes next to finite water wake it.
                for p in covered {
                    water.block_changed(&mut lw.map, &lw.reg, p);
                }
            }
            Ok((false, _)) => {
                if !lockstep {
                    let wait = next_tick.saturating_duration_since(Instant::now());
                    std::thread::sleep(wait.min(Duration::from_millis(5)));
                }
            }
            Err(_) => {
                save(
                    &mut save_state,
                    &player,
                    &appearance,
                    &past_lives,
                    &world_items,
                    &workshop,
                    &lw,
                    &fauna,
                    ticks,
                );
                return Ok(());
            }
        }
    }
}

/// Terrain streaming around a point.
#[derive(Default)]
struct Stream {
    /// The land's growth held back while the Observer passes the years fast (D210): the globe
    /// is the view; the cubes about the eye grow on when time slows.
    hold_growth: bool,
    loaded: FxHashSet<CubePos>,
    meshed: FxHashSet<CubePos>,
    wanted: Vec<(i64, CubePos)>,
    last_center: Option<CubePos>,
    last_view: (i32, i32),
    cover_step: Option<i64>,
    heights_at: Option<(i32, i32)>,
    heights_dirty: bool,
    heights_sent: Option<Instant>,
    /// Each loaded cube's vegetation: the snapshot it was grown with and the year it next
    /// changes.
    grown: FxHashMap<CubePos, (Vegetation, f64)>,
    /// The vegetation last looked at (new disturbances since it regrow what they reach).
    veg_seen: Option<Vegetation>,
}

impl Stream {
    /// One step of streaming work: whether there was work, and the blocks the seasonal cover
    /// changed. `Err` when the client is gone.
    fn work(
        &mut self,
        lw: &mut LocalWorld,
        water: &mut WaterSim,
        models: &BlockModels,
        opts: MeshOptions,
        planet: Planet,
        center: DVec3,
        view: View,
        year_frac: f64,
        tx: &Sender<ToClient>,
    ) -> Result<(bool, Vec<BlockPos>), ()> {
        let column = (center.x.floor() as i32, center.z.floor() as i32);
        let moved = self
            .heights_at
            .is_none_or(|(x, z)| (x - column.0).abs().max((z - column.1).abs()) >= 8);
        if (moved || self.heights_dirty)
            && self
                .heights_sent
                .is_none_or(|t| t.elapsed() > Duration::from_millis(250))
        {
            let map = &lw.map;
            let heights = SkyHeights::build(column.0, column.1, |x, z| map.sky_top(x, z));
            let water = crate::water_env::water_heights(map, &lw.reg, column.0, column.1);
            tx.send(ToClient::Heights(Box::new(heights), Box::new(water)))
                .map_err(|_| ())?;
            self.heights_at = Some(column);
            self.heights_dirty = false;
            self.heights_sent = Some(Instant::now());
        }
        let c = planet.wrap_cube(CubePos::containing(center));
        if self.last_center != Some(c) || self.last_view != (view.radius, view.vertical) {
            self.last_center = Some(c);
            self.last_view = (view.radius, view.vertical);
            // Unload what fell out of range (with a margin so small moves don't thrash).
            let far: Vec<CubePos> = self
                .loaded
                .iter()
                .copied()
                .filter(|p| {
                    let d = planet.cube_delta(c, *p);
                    d.x.abs() > view.radius + 2
                        || d.z.abs() > view.radius + 2
                        || d.y.abs() > view.vertical + 2
                })
                .collect();
            for p in far {
                self.loaded.remove(&p);
                self.grown.remove(&p);
                lw.map.remove_cube(p);
                if self.meshed.remove(&p) {
                    tx.send(ToClient::Unload(p)).map_err(|_| ())?;
                }
            }
            // Everything in range that is missing, nearest first.
            self.wanted.clear();
            for dy in -view.vertical..=view.vertical {
                for dz in -view.radius..=view.radius {
                    for dx in -view.radius..=view.radius {
                        let p = planet.wrap_cube(CubePos::new(c.x + dx, c.y + dy, c.z + dz));
                        if !self.loaded.contains(&p) {
                            let d = (dx * dx + dz * dz) as i64 + (dy * dy) as i64 * 2;
                            self.wanted.push((d, p));
                        }
                    }
                }
            }
            // Farthest first so the nearest can be popped off the end.
            self.wanted
                .sort_unstable_by_key(|(d, _)| std::cmp::Reverse(*d));
        }
        // As the calendar moves on (every five days of the year), loaded terrain gets the
        // date's snow and ice.
        let step = (year_frac * COVER_STEPS).floor() as i64;
        let mut covered = Vec::new();
        if self.cover_step != Some(step) {
            if self.cover_step.is_some() {
                covered = self.refresh_cover(lw, models, opts, year_frac, tx)?;
            }
            self.cover_step = Some(step);
        }
        // Trees grow and the land disturbed grows back.
        covered.extend(self.regrow(lw, models, opts, year_frac, tx)?);
        if self.wanted.is_empty() {
            return Ok((!covered.is_empty(), covered));
        }
        let take = self.wanted.len().min(BATCH);
        let batch: Vec<CubePos> = self
            .wanted
            .drain(self.wanted.len() - take..)
            .map(|(_, p)| p)
            .collect();
        let generator = lw.generator.clone();
        let veg = lw.vegetation.clone();
        let cubes: Vec<_> = batch
            .par_iter()
            .map(|p| (*p, generator.generate_cube_in(*p, &veg)))
            .collect();
        for (p, (cube, next)) in cubes {
            self.grown.insert(p, (veg.clone(), next));
            let data = lw.generator.column(p.column());
            lw.map.ensure_column(p.column(), || {
                let mut est = [0i32; hearth_math::CUBE_AREA];
                for (e, s) in est.iter_mut().zip(&data.samples) {
                    *e = s.height_i().max(s.water_i()) - 1;
                }
                est
            });
            lw.map.insert_cube(p, Arc::new(cube), &lw.reg);
            self.loaded.insert(p);
            // The player's changes, and finite water that was here when the cube was unloaded,
            // come back.
            lw.edits.restore(&mut lw.map, &lw.reg, p);
            water.restore(&mut lw.map, &lw.reg, p);
        }
        // Seasonal snow and ice on the new terrain, then light.
        let mut cols: Vec<hearth_math::ColumnPos> = batch.iter().map(|p| p.column()).collect();
        cols.sort_unstable_by_key(|c| (c.x, c.z));
        cols.dedup();
        lw.cover
            .apply(&mut lw.map, &lw.reg, &lw.generator, &cols, year_frac);
        lw.light.light_new_cubes(&mut lw.map, &lw.reg, &batch);
        self.heights_dirty = true;
        // Mesh every cube around the batch whose 26 neighbours are now all present; cubes that
        // were already meshed are redone because their lighting may have changed. Their blocks
        // go to the client with them.
        let mut ready: FxHashSet<CubePos> = FxHashSet::default();
        for p in &batch {
            for dz in -1..=1 {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let q = planet.wrap_cube(CubePos::new(p.x + dx, p.y + dy, p.z + dz));
                        if self.loaded.contains(&q)
                            && !ready.contains(&q)
                            && neighbours_loaded(&planet, &self.loaded, q)
                        {
                            ready.insert(q);
                        }
                    }
                }
            }
        }
        let ready: Vec<CubePos> = ready.into_iter().collect();
        for &p in &ready {
            if let Some(cube) = lw.map.cube(p) {
                tx.send(ToClient::Cube(p, cube.clone())).map_err(|_| ())?;
            }
        }
        let meshes = lw.mesh(models, &ready, opts);
        for m in meshes {
            self.meshed.insert(m.pos);
            tx.send(ToClient::Mesh(Box::new(m))).map_err(|_| ())?;
        }
        Ok((true, covered))
    }

    /// Grows the loaded terrain on with the vegetation: the cubes whose trees pass into a new
    /// stage or whose ground moves on, and those a new disturbance reaches, are generated again
    /// and what changed laid in, under the player's changes and the water. Relights and
    /// remeshes; returns the blocks changed.
    fn regrow(
        &mut self,
        lw: &mut LocalWorld,
        models: &BlockModels,
        opts: MeshOptions,
        year_frac: f64,
        tx: &Sender<ToClient>,
    ) -> Result<Vec<BlockPos>, ()> {
        if self.hold_growth {
            return Ok(Vec::new());
        }
        let now = lw.vegetation.clone();
        if let Some(seen) = &self.veg_seen
            && !now.same_disturbances(seen)
        {
            for d in now.added_since(seen) {
                let reach = d.radius * hearth_worldgen::vegetation::EDGE + 32.0;
                for (p, (_, next)) in self.grown.iter_mut() {
                    if now.distance(d, p.x * 16 + 8, p.z * 16 + 8) <= reach {
                        *next = f64::NEG_INFINITY;
                    }
                }
            }
        }
        self.veg_seen = Some(now.clone());
        let mut due: Vec<CubePos> = self
            .grown
            .iter()
            .filter(|(_, (_, next))| *next <= now.year)
            .map(|(p, _)| *p)
            .collect();
        if due.is_empty() {
            return Ok(Vec::new());
        }
        let t0 = Instant::now();
        due.sort_unstable_by_key(|p| (p.x, p.z, p.y));
        due.truncate(REGROW_BATCH);
        let jobs: Vec<(CubePos, Vegetation)> =
            due.iter().map(|p| (*p, self.grown[p].0.clone())).collect();
        let generator = lw.generator.clone();
        let grown: Vec<(CubePos, Cube, Cube, f64)> = jobs
            .par_iter()
            .map(|(p, old)| {
                let (before, _) = generator.generate_cube_in(*p, old);
                let (after, next) = generator.generate_cube_in(*p, &now);
                (*p, before, after, next)
            })
            .collect();
        let mut diffs: Vec<(BlockPos, BlockStateId)> = Vec::new();
        for (p, before, after, next) in &grown {
            // Never due again before the year moves on.
            self.grown
                .insert(*p, (now.clone(), next.max(now.year + 1e-4)));
            let o = p.min_block();
            for i in 0..hearth_math::CUBE_VOLUME {
                let (b, a) = (before.get_index(i), after.get_index(i));
                if a != b {
                    let l = hearth_math::LocalPos::from_index(i);
                    diffs.push((
                        BlockPos::new(o.x + l.x as i32, o.y + l.y as i32, o.z + l.z as i32),
                        a,
                    ));
                }
            }
        }
        if diffs.is_empty() {
            return Ok(Vec::new());
        }
        // The seasonal cover comes off the columns that change, and is laid again after.
        let mut cols: Vec<hearth_math::ColumnPos> =
            diffs.iter().map(|(p, _)| p.cube().column()).collect();
        cols.sort_unstable_by_key(|c| (c.x, c.z));
        cols.dedup();
        let mut changed = lw
            .cover
            .strip(&mut lw.map, &lw.reg, &lw.generator, &cols, year_frac);
        let reg = lw.reg.clone();
        let mut grew = 0;
        for (pos, state) in diffs {
            if lw.edits.get(pos).is_some() {
                continue;
            }
            let Some(cur) = lw.map.block(pos) else {
                continue;
            };
            if cur == state || reg.block_of(cur).def.fluid.is_some() {
                continue;
            }
            lw.map.set_block(pos, state, &reg);
            changed.push(pos);
            grew += 1;
        }
        changed.extend(
            lw.cover
                .lay(&mut lw.map, &lw.reg, &lw.generator, &cols, year_frac),
        );
        changed.sort_unstable_by_key(|p| (p.x, p.y, p.z));
        changed.dedup();
        let remeshed = self.blocks_changed(lw, models, opts, &changed, tx)?;
        log::info!(
            "vegetation of year {:.2}: {} cubes grown again, {grew} blocks changed, {remeshed} \
             remeshed in {:.0} ms",
            now.year,
            grown.len(),
            t0.elapsed().as_secs_f64() * 1e3
        );
        Ok(changed)
    }

    /// Re-lays the date's snow and ice on all loaded terrain; relights and remeshes what
    /// changed. Returns the blocks changed.
    fn refresh_cover(
        &mut self,
        lw: &mut LocalWorld,
        models: &BlockModels,
        opts: MeshOptions,
        year_frac: f64,
        tx: &Sender<ToClient>,
    ) -> Result<Vec<BlockPos>, ()> {
        let t0 = Instant::now();
        let mut cols: Vec<hearth_math::ColumnPos> =
            self.loaded.iter().map(|p| p.column()).collect();
        cols.sort_unstable_by_key(|c| (c.x, c.z));
        cols.dedup();
        lw.cover.buried.retain_columns(|c| {
            cols.binary_search_by_key(&(c.x, c.z), |k| (k.x, k.z))
                .is_ok()
        });
        let changed = lw
            .cover
            .refresh(&mut lw.map, &lw.reg, &lw.generator, &cols, year_frac);
        let remeshed = self.blocks_changed(lw, models, opts, &changed, tx)?;
        log::info!(
            "seasonal cover for year {:.3}: {} blocks changed, {} cubes remeshed in {:.0} ms",
            year_frac,
            changed.len(),
            remeshed,
            t0.elapsed().as_secs_f64() * 1e3
        );
        Ok(changed)
    }

    /// Relights the blocks that changed and sends the cubes they touch (and the neighbours that
    /// show them on a face) again, with new meshes. Returns the cubes remeshed.
    fn blocks_changed(
        &mut self,
        lw: &mut LocalWorld,
        models: &BlockModels,
        opts: MeshOptions,
        changed: &[BlockPos],
        tx: &Sender<ToClient>,
    ) -> Result<usize, ()> {
        let planet = *lw.map.planet();
        let mut dirty: FxHashSet<CubePos> = FxHashSet::default();
        for &p in changed {
            lw.light.block_changed(&mut lw.map, &lw.reg, p);
            let p = planet.wrap_block(p);
            let local = p.local();
            dirty.insert(p.cube());
            for (d, edge) in [
                ((-1, 0, 0), local.x == 0),
                ((1, 0, 0), local.x == 15),
                ((0, -1, 0), local.y == 0),
                ((0, 1, 0), local.y == 15),
                ((0, 0, -1), local.z == 0),
                ((0, 0, 1), local.z == 15),
            ] {
                if edge {
                    let c = p.cube();
                    dirty.insert(planet.wrap_cube(CubePos::new(c.x + d.0, c.y + d.1, c.z + d.2)));
                }
            }
        }
        let dirty: Vec<CubePos> = dirty
            .into_iter()
            .filter(|c| self.meshed.contains(c))
            .collect();
        for &p in &dirty {
            if let Some(cube) = lw.map.cube(p) {
                tx.send(ToClient::Cube(p, cube.clone())).map_err(|_| ())?;
            }
        }
        let meshes = lw.mesh(models, &dirty, opts);
        let n = meshes.len();
        for m in meshes {
            tx.send(ToClient::Mesh(Box::new(m))).map_err(|_| ())?;
        }
        self.heights_dirty |= !changed.is_empty();
        Ok(n)
    }
}

fn neighbours_loaded(planet: &Planet, loaded: &FxHashSet<CubePos>, p: CubePos) -> bool {
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                if (dx, dy, dz) != (0, 0, 0)
                    && !loaded.contains(&planet.wrap_cube(CubePos::new(
                        p.x + dx,
                        p.y + dy,
                        p.z + dz,
                    )))
                {
                    return false;
                }
            }
        }
    }
    true
}

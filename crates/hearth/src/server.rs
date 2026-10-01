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

use glam::DVec3;
use hearth_body::{BodyConfig, Exposure, Posture, Worn};
use hearth_content::balance::Balance;
use hearth_content::time::TimeScales;
use hearth_env::{Calendar, Moment};
use hearth_math::{BlockPos, CubePos, Planet, PlanetSize};
use hearth_physics::{Mover, Report};
use hearth_player::{DROWN_S, Player};
use hearth_protocol::{BodyView, LifeSummary, Moved, Ready, ToClient, ToServer};
use hearth_render::atlas::TextureArray;
use hearth_render::mesh::MeshOptions;
use hearth_render::models::BlockModels;
use hearth_render::precip::SkyHeights;
use hearth_save::{WorldDir, WorldMeta, WorldSettings};
use hearth_world::water::{WaterEnv, WaterSim};
use rayon::prelude::*;
use rustc_hash::FxHashSet;

use crate::environment::{EnvOverrides, EnvSampler};
use crate::scene::LocalWorld;
use crate::workshop::{Here, Workshop, WorkshopSave};

/// Cubes generated per batch (bounded so nearby terrain appears quickly while moving).
const BATCH: usize = 192;
/// Seconds of play per tick.
pub const TICK_S: f64 = 0.05;
/// Ticks between autosaves (five minutes).
const AUTOSAVE_TICKS: u64 = 6000;
/// Seasonal cover steps per year (as the year-scale snow model).
const COVER_STEPS: f64 = 73.0;
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
    /// Who the player is in a new world (a saved world keeps its own person).
    pub appearance: hearth_character::Appearance,
    /// What death means in a new world (a saved world keeps its own rules).
    pub death_rules: hearth_save::DeathRules,
    /// How knowledge is gained in a new world.
    pub knowledge: hearth_save::KnowledgeMode,
}

/// How much terrain to keep around the player (cubes).
#[derive(Debug, Clone, Copy)]
pub struct View {
    pub radius: i32,
    pub vertical: i32,
}

/// How much faster the world goes while the player sleeps (v2 §9.5: smoothly, up to 60–120×).
const SLEEP_SPEED: f64 = 90.0;

/// What `player.json` holds.
#[derive(serde::Serialize, serde::Deserialize)]
struct PlayerSave {
    format: u32,
    player: Player,
    /// How the player looks (saves before it take the default person).
    #[serde(default)]
    appearance: hearth_character::Appearance,
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
    let planet = hearth_worldgen::WorldGenSettings {
        seed: spec.seed,
        planet_size: spec.planet,
        ..Default::default()
    }
    .sanitized();
    let mut settings = WorldSettings::new(planet);
    settings.life = hearth_save::LifeSettings::from_content(&lw.content.time);
    settings.life.death_rules = spec.death_rules;
    settings.life.knowledge_mode = spec.knowledge;
    let meta = WorldMeta::new(&spec.name, settings, lw.reg.state_names());
    let dir = WorldDir::create(saves, &meta)?;
    log::info!("created world {:?} in {}", spec.name, dir.root.display());
    Ok(Some(Save { dir, meta }))
}

/// Whether a point is within the player's reach (3 m of the eyes).
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

/// How hard the ground underfoot drags at a load pulled over it (sliding friction): snow and
/// ice let a load slide, sand and mud hold it.
fn drag_friction(lw: &LocalWorld, feet: DVec3) -> f32 {
    let below = BlockPos::containing(feet - DVec3::new(0.0, 0.05, 0.0));
    let at = BlockPos::containing(feet + DVec3::new(0.0, 0.05, 0.0));
    let group = |p: BlockPos| {
        lw.map
            .block(p)
            .filter(|s| !s.is_air())
            .map(|s| lw.reg.block_of(s).def.sound.clone())
    };
    match group(at).or_else(|| group(below)).as_deref() {
        Some("snow") => 0.12,
        Some("glass") => 0.05,
        Some("sand") => 0.6,
        Some("mud") => 0.7,
        Some("stone") | Some("deepslate") => 0.35,
        Some("none") => 0.1,
        _ => 0.45,
    }
}

/// Feet on the ground at a column: on the surface or the water, a little above.
fn ground_at(lw: &LocalWorld, x: i32, z: i32) -> DVec3 {
    DVec3::new(
        x as f64 + 0.5,
        lw.surface_y(x as f64 + 0.5, z as f64 + 0.5) + 0.5,
        z as f64 + 0.5,
    )
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
    let covered = lw.map.sky_top(x, z).is_some_and(|top| top as f64 >= head);
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
        wind_m_s: (w.wind_speed_m_s * if covered { 0.3 } else { 1.0 }) as f32,
        rain_mm_h: if covered { 0.0 } else { falling as f32 },
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
    BodyView {
        status: p.body.status(cfg),
        ability: p.ability_with(cfg, &load, mu),
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
        exposure,
    }
}

/// Saves the player, the clock, the things lying about and what is built.
fn save(
    save: &mut Option<Save>,
    player: &Player,
    appearance: &hearth_character::Appearance,
    world_items: &hearth_items::WorldItems,
    workshop: &Workshop,
    ticks: u64,
) {
    let Some(s) = save else {
        return;
    };
    s.meta.clock.ticks = ticks;
    s.meta.last_played_unix = hearth_save::meta::unix_now();
    let player = PlayerSave {
        format: PLAYER_FORMAT,
        player: player.clone(),
        appearance: appearance.clone(),
    };
    if let Err(e) = s
        .dir
        .save_meta(&s.meta)
        .and_then(|()| s.dir.write_json("player.json", &player))
        .and_then(|()| s.dir.write_json("items.json", world_items))
        .and_then(|()| s.dir.write_json("crafts.json", &workshop.save()))
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
    // A saved world keeps its own seed and planet.
    let mut seed = spec.seed;
    let mut size = spec.planet;
    if let Some(saves) = &spec.saves_dir {
        let root = saves.join(&spec.name);
        if root.join("level.json").exists() {
            let (_, meta, _) = WorldDir::open(&root)?;
            seed = meta.settings.planet.seed;
            size = meta.settings.planet.planet_size;
        }
    }
    let mut lw = LocalWorld::create(seed, size, 0, spec.cache_dir.as_deref())?;
    let planet = *lw.map.planet();
    let mut save_state = open_save(&spec, &lw)?;

    // The body's world: the world's time scales and realism.
    let content = lw.content.clone();
    let (life, mut ticks) = match &save_state {
        Some(s) => (s.meta.settings.life.clone(), s.meta.clock.ticks),
        None => {
            let mut life = hearth_save::LifeSettings::from_content(&content.time);
            life.death_rules = spec.death_rules;
            life.knowledge_mode = spec.knowledge;
            (life, 0)
        }
    };
    let scales = TimeScales::new(life.day_length_min, life.days_per_season, &content.time);
    let balance = Balance::resolve(&content, &life.realism.preset, &life.realism.overrides);
    let cfg = Arc::new(BodyConfig::new(
        &content,
        &balance,
        &life.realism.preset,
        scales,
    ));

    // The calendar starts in the morning of the starting season at the world's first spawn.
    let (sx, sz) = lw.terrain().find_spawn(false);
    let first_spawn = ground_at(&lw, sx, sz);
    let calendar = Calendar::from_config(&content.time).start_at(
        life.starting_season,
        planet.latitude(first_spawn.z) < 0.0,
        0.33,
        planet.solar_time_offset(first_spawn.x),
    );
    let env = EnvSampler::new(lw.grid(), calendar);

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
    let (mut player, mut appearance) = match saved {
        Some(p) => (p.player, p.appearance.sanitized()),
        None => (
            Player::new(&cfg, first_spawn, seed ^ 0x5eed),
            spec.appearance.clone().sanitized(),
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
    let death_rules = save_state
        .as_ref()
        .map_or(spec.death_rules, |s| s.meta.settings.life.death_rules);
    let ended = save_state.as_ref().and_then(|s| {
        s.meta
            .ended
            .then(|| s.dir.read_json::<LifeSummary>("life.json").ok().flatten())
            .flatten()
    });
    let mut death_told = player.body.dead.is_some();
    // Making and knowing: how knowledge is gained here, the stations and fires standing.
    let mode = match life.knowledge_mode {
        hearth_save::KnowledgeMode::Discovery => hearth_craft::Mode::Discovery,
        hearth_save::KnowledgeMode::Guided => hearth_craft::Mode::Guided,
        hearth_save::KnowledgeMode::Open => hearth_craft::Mode::Open,
    };
    let workshop_save: Option<WorkshopSave> = save_state
        .as_ref()
        .and_then(|s| s.dir.read_json("crafts.json").ok().flatten());
    let mut workshop = Workshop::new(&content, &items, mode, workshop_save, seed, ticks);
    if mode == hearth_craft::Mode::Open {
        player.knowledge.known = hearth_craft::KnowledgeState::open(&workshop.graph, ticks).known;
    }
    let authentic = life.realism.preset == "authentic";
    // Messages the workshop has for the client.
    let mut outbox: Vec<ToClient> = Vec::new();
    // Game ticks the last server tick moved the clock by.
    let mut advanced = 1.0f64;
    let mut work_warp = 0.0f64;

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
            death_rules,
            items: items.clone(),
            content: content.clone(),
            crafts: workshop.crafts.clone(),
            graph: workshop.graph.clone(),
            knowledge_mode: mode,
            ended,
        })))
        .is_err()
    {
        return Ok(());
    }

    let models = BlockModels::build(&lw.reg, &atlas);
    let opts = MeshOptions::default();
    let mut stream = Stream::default();
    let mut water = WaterSim::new(&lw.reg)?;
    let mut last_moved: Option<Moved> = None;
    let mut warp = 0.0f64;
    let mut sleep_warp = 0.0f64;
    let mut warp_carry = 0.0f64;
    let mut paused = false;
    // Ticks as fast as they go rather than twenty a second (tests and bots).
    let mut fast = false;
    let mut next_tick = Instant::now();
    let mut since_save = 0u64;
    macro_rules! here {
        () => {
            Here {
                lw: &mut lw,
                items: &items,
                cfg: &cfg,
                env: &env,
                scales: &scales,
                moment: calendar.at(ticks),
                ticks,
                ticks_per_day: calendar.ticks_per_day(),
                player: &mut player,
                world_items: &mut world_items,
                changed: &mut gathered,
                out: &mut outbox,
                items_changed: &mut items_changed,
            }
        };
    }
    loop {
        // Messages from the client.
        loop {
            match inbox.try_recv() {
                Ok(ToServer::Moved(m)) => {
                    if player.body.dead.is_none() {
                        player.life.moved(player.mover.pos, m.mover.pos);
                    }
                    player.mover = m.mover;
                    if !player.asleep
                        && let Some(v) = m.landed
                    {
                        player.body.land(&cfg, v);
                    }
                    if m.airless_s >= DROWN_S {
                        player.body.kill(hearth_body::Death::Drowning);
                    }
                    last_moved = Some(m);
                }
                Ok(ToServer::Sleep(lie)) => {
                    // Lying down to rest (sleep comes if the body is sleepy); getting up wakes.
                    player.lying = lie && player.body.dead.is_none();
                    if !player.lying {
                        player.asleep = false;
                    }
                    player.drowsy_s = 0.0;
                }
                Ok(ToServer::Place(p)) => {
                    let (x, z) = lw
                        .terrain()
                        .spawn_near(p.x.floor() as i32, p.z.floor() as i32);
                    player.mover = Mover::new(ground_at(&lw, x, z));
                    let _ = tx.send(ToClient::Placed(player.mover));
                }
                Ok(ToServer::Respawn(who)) => {
                    if player.body.dead.is_some() {
                        // v2 §9.8. Legacy: someone new arrives in the same region; Hardy: the
                        // same person again where the world began; permadeath: the end.
                        let at = match death_rules {
                            hearth_save::DeathRules::Legacy => {
                                let at = player.mover.pos;
                                let (x, z) = lw
                                    .terrain()
                                    .spawn_near(at.x.floor() as i32, at.z.floor() as i32);
                                if let Some(a) = who {
                                    appearance = a.sanitized();
                                }
                                Some(ground_at(&lw, x, z))
                            }
                            hearth_save::DeathRules::Hardy => Some(first_spawn),
                            hearth_save::DeathRules::Permadeath => None,
                        };
                        if let Some(at) = at {
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
                            player = Player::new(&cfg, at, seed ^ ticks);
                            player.knowledge = match death_rules {
                                hearth_save::DeathRules::Hardy => knew,
                                _ => knew.passed_on(&workshop.graph, ticks),
                            };
                            workshop.knowledge_changed = true;
                            player.life = hearth_player::Life::begin(at, ticks);
                            player.carry = outfit(&appearance);
                            worn = dress_carry(&player.carry);
                            death_told = false;
                            let _ = tx.send(ToClient::Person(appearance.clone()));
                            let _ = tx.send(ToClient::Placed(player.mover));
                        }
                    }
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
                Ok(ToServer::Act { process, aim, hand }) => {
                    workshop.act(&mut here!(), &process, aim, hand);
                }
                Ok(ToServer::StopWork) => workshop.stop(&mut here!()),
                Ok(ToServer::Look(aim)) => workshop.look(&mut here!(), aim),
                Ok(ToServer::Eat(path)) => workshop.eat(&mut here!(), &path),
                Ok(ToServer::Drink(from)) => workshop.drink(&mut here!(), &from),
                Ok(ToServer::Fill { skin, aim }) => workshop.fill(&mut here!(), &skin, aim),
                Ok(ToServer::Throw { dir, speed }) => {
                    workshop.throw(&mut here!(), dir, speed);
                }
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
                Ok(ToServer::SkipHours(h)) => {
                    let dt = h / 24.0 * calendar.ticks_per_day();
                    ticks = (ticks as f64 + dt).max(0.0) as u64;
                }
                Ok(ToServer::TimeWarp(w)) => warp = w.max(0.0),
                Ok(ToServer::Pause(p)) => paused = p,
                Ok(ToServer::Fast(f)) => fast = f,
                Ok(ToServer::View { radius, vertical }) => {
                    view = View {
                        radius: radius.clamp(1, 64),
                        vertical: vertical.clamp(1, 32),
                    };
                }
                Ok(ToServer::Quit) | Err(TryRecvError::Disconnected) => {
                    save(
                        &mut save_state,
                        &player,
                        &appearance,
                        &world_items,
                        &workshop,
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
        } else if fast || Instant::now() >= next_tick {
            workshop.tick(&mut here!(), advanced);
            let moment = calendar.at(ticks);
            let immersion = last_moved.map_or(0.0, |m| m.immersion);
            let mut e = exposure(&env, &lw, &moment, &player.mover, immersion);
            // Fires warm those beside them; bedding keeps the ground's cold off a sleeper.
            e.radiant_w_m2 += workshop.radiant_w_m2(player.mover.pos + DVec3::new(0.0, 0.9, 0.0));
            if player.lying || player.asleep {
                e.ground_clo = e.ground_clo.max(workshop.bedding_clo(player.mover.pos));
            }
            let report = last_moved.as_ref().map(report_of).unwrap_or_default();
            if player.body.dead.is_none() {
                let load = player.carry.load(&items, cfg.mass_kg as f32);
                let mu = drag_friction(&lw, player.mover.pos);
                let mut activity = player.activity_with(&cfg, &report, &load, mu);
                if player.asleep || player.lying {
                    activity.posture = Posture::Lying;
                }
                // Work has its own cost.
                if let Some(mets) = workshop.work_mets() {
                    activity.met = activity.met.max(mets);
                }
                // Warped time passes for the body too.
                player.body.step(
                    &cfg,
                    TICK_S * (1.0 + (warp + sleep_warp + work_warp) / 20.0),
                    &e,
                    &worn,
                    &activity,
                );
                let hour = env.local_time(&moment, player.mover.pos.x) * 24.0;
                if let Some(why) = player.rest(&cfg, &e, hour, TICK_S) {
                    let _ = tx.send(ToClient::Woke(why));
                }
            } else {
                player.asleep = false;
                player.lying = false;
            }
            // Asleep, the world speeds up smoothly; awake, it slows back.
            let target = if player.asleep {
                20.0 * (SLEEP_SPEED - 1.0)
            } else {
                0.0
            };
            sleep_warp += (target - sleep_warp) * (1.0 - (-TICK_S / 2.5).exp());
            if sleep_warp < 1.0 && target == 0.0 {
                sleep_warp = 0.0;
            }
            // A death is told once; under permadeath it ends the world, with its life's tale.
            if !death_told && let Some(cause) = player.body.dead.clone() {
                death_told = true;
                if death_rules == hearth_save::DeathRules::Permadeath {
                    let summary = LifeSummary {
                        name: appearance.name.clone(),
                        days: ticks.saturating_sub(player.life.born_tick) as f64
                            / calendar.ticks_per_day(),
                        walked_km: player.life.walked_m / 1000.0,
                        farthest_km: player.life.farthest_m / 1000.0,
                        cause,
                        discovered: player
                            .knowledge
                            .known
                            .keys()
                            .filter_map(|k| workshop.graph.node(k).map(|n| n.name.clone()))
                            .collect(),
                    };
                    if let Some(s) = &mut save_state {
                        s.meta.ended = true;
                        if let Err(e) = s.dir.write_json("life.json", &summary) {
                            log::error!("could not write the life's tale: {e}");
                        }
                    }
                    save(
                        &mut save_state,
                        &player,
                        &appearance,
                        &world_items,
                        &workshop,
                        ticks,
                    );
                    let _ = tx.send(ToClient::Ended(summary));
                }
            }
            // Long work speeds the world up as sleep does (no more than a minute or so of
            // waiting for any task).
            let work_target = 20.0 * workshop.work_warp();
            work_warp += (work_target - work_warp) * (1.0 - (-TICK_S / 1.0).exp());
            if work_warp < 1.0 && work_target == 0.0 {
                work_warp = 0.0;
            }
            warp_carry += (warp + sleep_warp + work_warp) * TICK_S;
            let extra = warp_carry.floor();
            warp_carry -= extra;
            ticks += 1 + extra as u64;
            advanced = 1.0 + extra;
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
            if ticks.is_multiple_of(2) {
                changed.extend_from_slice(water.tick(&mut lw.map, &lw.reg, &world_water));
            }
            let weather_every = (calendar.ticks_per_day() / 288.0).max(1.0) as u64;
            if ticks.is_multiple_of(weather_every) {
                let days = weather_every as f64 / calendar.ticks_per_day();
                water.weather(&mut lw.map, &lw.reg, &world_water, days as f32);
                changed.extend_from_slice(water.changed());
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
                    &world_items,
                    &workshop,
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
                        20.0 + warp + sleep_warp + work_warp,
                        &items,
                        drag_friction(&lw, player.mover.pos),
                    ))))
                    .is_err()
            {
                save(
                    &mut save_state,
                    &player,
                    &appearance,
                    &world_items,
                    &workshop,
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
                    &world_items,
                    &workshop,
                    ticks,
                );
                let _ = tx.send(ToClient::Saved);
            }
            next_tick += Duration::from_secs_f64(TICK_S);
            if Instant::now() > next_tick + Duration::from_secs(1) {
                // Far behind (a long batch, a breakpoint): don't run the missed ticks at once.
                next_tick = Instant::now();
            }
            // Running fast (tests), the terrain streams between every tick.
            if !fast {
                continue;
            }
        }

        // Terrain around the player between ticks.
        let year_frac = calendar.at(ticks).year_frac;
        let worked = stream
            .work(
                &mut lw,
                &mut water,
                &models,
                opts,
                planet,
                player.mover.pos,
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
                if !fast {
                    let wait = next_tick.saturating_duration_since(Instant::now());
                    std::thread::sleep(wait.min(Duration::from_millis(5)));
                }
            }
            Err(_) => {
                save(
                    &mut save_state,
                    &player,
                    &appearance,
                    &world_items,
                    &workshop,
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
    loaded: FxHashSet<CubePos>,
    meshed: FxHashSet<CubePos>,
    wanted: Vec<(i64, CubePos)>,
    last_center: Option<CubePos>,
    last_view: (i32, i32),
    cover_step: Option<i64>,
    heights_at: Option<(i32, i32)>,
    heights_dirty: bool,
    heights_sent: Option<Instant>,
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
        let cubes: Vec<_> = batch
            .par_iter()
            .map(|p| (*p, generator.generate_cube(*p)))
            .collect();
        for (p, cube) in cubes {
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
            // Finite water that was here when the cube was unloaded comes back.
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

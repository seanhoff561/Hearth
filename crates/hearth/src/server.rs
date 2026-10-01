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
use hearth_protocol::{BodyView, Moved, Ready, ToClient, ToServer};
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
    let meta = WorldMeta::new(&spec.name, settings, lw.reg.state_names());
    let dir = WorldDir::create(saves, &meta)?;
    log::info!("created world {:?} in {}", spec.name, dir.root.display());
    Ok(Some(Save { dir, meta }))
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

fn body_view(cfg: &BodyConfig, p: &Player, exposure: Exposure) -> BodyView {
    BodyView {
        status: p.body.status(cfg),
        ability: p.ability(cfg),
        asleep: p.asleep,
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

/// Saves the player and the clock.
fn save(save: &mut Option<Save>, player: &Player, ticks: u64) {
    let Some(s) = save else {
        return;
    };
    s.meta.clock.ticks = ticks;
    s.meta.last_played_unix = hearth_save::meta::unix_now();
    let player = PlayerSave {
        format: PLAYER_FORMAT,
        player: player.clone(),
    };
    if let Err(e) = s
        .dir
        .save_meta(&s.meta)
        .and_then(|()| s.dir.write_json("player.json", &player))
    {
        log::error!("could not save the world: {e}");
    } else {
        log::info!("saved world {:?} at tick {ticks}", s.meta.name);
    }
}

fn run(
    spec: WorldSpec,
    atlas: Arc<TextureArray>,
    view: View,
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
        None => (hearth_save::LifeSettings::from_content(&content.time), 0),
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

    let mut player = save_state
        .as_ref()
        .and_then(|s| match s.dir.read_json::<PlayerSave>("player.json") {
            Ok(p) => p.map(|p| p.player),
            Err(e) => {
                log::error!("player.json unreadable ({e}); starting anew");
                None
            }
        })
        .unwrap_or_else(|| Player::new(&cfg, first_spawn, seed ^ 0x5eed));
    let worn = Worn::of(content.garments.get("hearth:loincloth"));

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
    let mut warp_carry = 0.0f64;
    let mut next_tick = Instant::now();
    let mut since_save = 0u64;
    loop {
        // Messages from the client.
        loop {
            match inbox.try_recv() {
                Ok(ToServer::Moved(m)) => {
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
                Ok(ToServer::Sleep(asleep)) => player.asleep = asleep && player.body.dead.is_none(),
                Ok(ToServer::Place(p)) => {
                    let (x, z) = lw
                        .terrain()
                        .spawn_near(p.x.floor() as i32, p.z.floor() as i32);
                    player.mover = Mover::new(ground_at(&lw, x, z));
                    let _ = tx.send(ToClient::Placed(player.mover));
                }
                Ok(ToServer::Respawn) => {
                    if player.body.dead.is_some() {
                        // A new person arrives in the same region (Legacy rules, v2 §9.8).
                        let at = player.mover.pos;
                        let (x, z) = lw
                            .terrain()
                            .spawn_near(at.x.floor() as i32, at.z.floor() as i32);
                        player = Player::new(&cfg, ground_at(&lw, x, z), seed ^ ticks);
                        let _ = tx.send(ToClient::Placed(player.mover));
                    }
                }
                Ok(ToServer::SkipHours(h)) => {
                    let dt = h / 24.0 * calendar.ticks_per_day();
                    ticks = (ticks as f64 + dt).max(0.0) as u64;
                }
                Ok(ToServer::TimeWarp(w)) => warp = w.max(0.0),
                Ok(ToServer::Quit) | Err(TryRecvError::Disconnected) => {
                    save(&mut save_state, &player, ticks);
                    let _ = tx.send(ToClient::Saved);
                    return Ok(());
                }
                Err(TryRecvError::Empty) => break,
            }
        }

        // The tick.
        if Instant::now() >= next_tick {
            let moment = calendar.at(ticks);
            let immersion = last_moved.map_or(0.0, |m| m.immersion);
            let e = exposure(&env, &lw, &moment, &player.mover, immersion);
            let report = last_moved.as_ref().map(report_of).unwrap_or_default();
            if player.body.dead.is_none() {
                let mut activity = player.activity(&cfg, &report);
                if player.asleep {
                    activity.posture = Posture::Lying;
                }
                // Warped time passes for the body too.
                player
                    .body
                    .step(&cfg, TICK_S * (1.0 + warp / 20.0), &e, &worn, &activity);
                if player.asleep && player.body.wakes(&cfg, &e).is_some() {
                    player.asleep = false;
                }
            }
            warp_carry += warp * TICK_S;
            let extra = warp_carry.floor();
            warp_carry -= extra;
            ticks += 1 + extra as u64;
            since_save += 1;
            // The finite water moves ten times a second; its slow changes every five game
            // minutes.
            let world_water = crate::water_env::WorldWater {
                generator: &lw.generator,
                air_c: e.air_c,
                humidity: e.humidity,
                wind_m_s: e.wind_m_s,
            };
            let mut changed: Vec<BlockPos> = Vec::new();
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
                save(&mut save_state, &player, ticks);
                return Ok(());
            }
            if tx.send(ToClient::Clock(ticks)).is_err()
                || tx
                    .send(ToClient::Body(Box::new(body_view(&cfg, &player, e))))
                    .is_err()
            {
                save(&mut save_state, &player, ticks);
                return Ok(());
            }
            if since_save >= AUTOSAVE_TICKS {
                since_save = 0;
                save(&mut save_state, &player, ticks);
                let _ = tx.send(ToClient::Saved);
            }
            next_tick += Duration::from_secs_f64(TICK_S);
            if Instant::now() > next_tick + Duration::from_secs(1) {
                // Far behind (a long batch, a breakpoint): don't run the missed ticks at once.
                next_tick = Instant::now();
            }
            continue;
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
                let wait = next_tick.saturating_duration_since(Instant::now());
                std::thread::sleep(wait.min(Duration::from_millis(5)));
            }
            Err(_) => {
                save(&mut save_state, &player, ticks);
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

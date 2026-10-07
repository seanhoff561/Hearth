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
use hearth_world::{BlockStateId, Cube};
use hearth_worldgen::vegetation::Vegetation;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};

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
    /// What the player asks of their birth in a new world (a saved world keeps its own person):
    /// their looks come from the parents the place gives them (V2.1 Addendum A).
    pub wish: hearth_protocol::Wish,
    /// What death means in a new world: whom a dead player may live on as, what is kept of what
    /// was known, whether one may be born again (Addendum B §2.3–2.5; a saved world keeps its
    /// own).
    pub death: hearth_save::Death,
    /// How knowledge is gained in a new world.
    pub knowledge: hearth_save::KnowledgeMode,
    /// Whether a new world's player lives their childhood (V2.1 Addendum A); tests and bots
    /// begin grown, at their people's coming of age.
    pub childhood: bool,
    /// A new world's era (`eras/`; a saved world keeps its own).
    pub era: String,
    /// Tests and bots: which of the births offered in an era's world the player takes (none:
    /// the player chooses).
    pub birth: Option<usize>,
}

/// How much terrain to keep around the player (cubes).
#[derive(Debug, Clone, Copy)]
pub struct View {
    pub radius: i32,
    pub vertical: i32,
}

/// The years of the recent past lived about a new life's birthplace (V2.1 §15.2: three or four
/// generations).
const RECENT_YEARS: f64 = 100.0;
/// How much faster the world goes while the player sleeps (v2 §9.5: smoothly, up to 60–120×).
const SLEEP_SPEED: f64 = 90.0;
/// How near a person must be for the player to hand them a thing (m).
const GIVE_M: f64 = 3.5;
/// How near a person must be for the player to speak with them (m).
const SPEAK_M: f64 = 8.0;

/// What `player.json` holds.
#[derive(serde::Serialize, serde::Deserialize)]
struct PlayerSave {
    format: u32,
    player: Player,
    /// How the player looks (saves before it take the default person).
    #[serde(default)]
    appearance: hearth_character::Appearance,
    /// The player's birth: their parents' genomes and theirs (a save from before genes draws
    /// one when loaded).
    #[serde(default)]
    birth: Option<hearth_people::Birth>,
    /// The household they were born into (H3): their parents' ages, theirs, their brothers and
    /// sisters (a save from before families has none, and its player no family).
    #[serde(default)]
    household: Option<hearth_people::Household>,
    /// Their childhood, while it lasts (and after: grown).
    #[serde(default)]
    childhood: Option<crate::childhood::Childhood>,
    /// What the player's lives before this one knew (Addendum B §2.4): kept per player, not per
    /// person, through any number of deaths.
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
    settings.era = spec.era.clone();
    settings.life = hearth_save::LifeSettings::from_content(&lw.content.time);
    settings.life.set_death(spec.death);
    settings.life.knowledge_mode = spec.knowledge;
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

/// How much of a line the player must make out for it to be phrased (H10): a phrasing is a
/// translation, and the player is given none of what it does not understand.
const PHRASED_UNDERSTOOD: f32 = 0.9;
/// The most characters of typed words read.
const SAID_CHARS: usize = 400;

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

/// The world's calendar: it starts in the morning of the starting season at the world's first
/// spawn (where it is spring or autumn by the hemisphere); and that spawn.
pub fn calendar_for(
    lw: &LocalWorld,
    starting: hearth_content::schema::Season,
) -> (Calendar, DVec3) {
    calendar_at(lw, starting, None)
}

/// As [`calendar_for`], the first spawn moved by `place` (from the place found): an era's, among
/// its people (H8).
pub fn calendar_at(
    lw: &LocalWorld,
    starting: hearth_content::schema::Season,
    place: Option<&dyn Fn(DVec2) -> DVec2>,
) -> (Calendar, DVec3) {
    let planet = *lw.map.planet();
    let (mut sx, mut sz) = lw.terrain().find_spawn(false);
    if let Some(place) = place {
        let at = place(DVec2::new(sx as f64, sz as f64));
        (sx, sz) = lw
            .terrain()
            .spawn_near(at.x.floor() as i32, at.y.floor() as i32);
    }
    let first_spawn = ground_at(lw, sx, sz);
    let calendar = Calendar::from_config(&lw.content.time).start_at(
        starting,
        planet.latitude(first_spawn.z) < 0.0,
        0.33,
        planet.solar_time_offset(first_spawn.x),
    );
    (calendar, first_spawn)
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
/// A life born in a country of a mean year colder than this (°C) begins dressed as its people
/// dress against the cold.
const DRESSED_BELOW_C: f32 = 12.0;

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
    held: Option<DVec3>,
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
        held,
        scale: p.mover.scale,
    }
}

/// Saves the player, the clock, the things lying about and what is built.
fn save(
    save: &mut Option<Save>,
    player: &Player,
    appearance: &hearth_character::Appearance,
    birth: &Option<hearth_people::Birth>,
    household: &Option<hearth_people::Household>,
    childhood: &Option<crate::childhood::Childhood>,
    past_lives: &std::collections::BTreeSet<String>,
    world_items: &hearth_items::WorldItems,
    workshop: &Workshop,
    lw: &LocalWorld,
    fauna: &crate::fauna::Fauna,
    people: &crate::people::PeopleNear,
    ticks: u64,
) {
    let Some(s) = save else {
        return;
    };
    people.save(fauna, &s.dir.root);
    s.meta.clock.ticks = ticks;
    s.meta.last_played_unix = hearth_save::meta::unix_now();
    let player = PlayerSave {
        format: PLAYER_FORMAT,
        player: player.clone(),
        appearance: appearance.clone(),
        birth: birth.clone(),
        household: household.clone(),
        childhood: childhood.clone(),
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
            life.set_death(spec.death);
            life.knowledge_mode = spec.knowledge;
            (life, 0)
        }
    };
    let scales = TimeScales::new(life.day_length_min, life.days_per_season, &content.time);
    let balance = Balance::resolve(&content, &life.realism.preset, &life.realism.overrides);
    let mut cfg = Arc::new(BodyConfig::new(
        &content,
        &balance,
        &life.realism.preset,
        scales,
    ));

    // The world's era (V2.1 §15.3) and its deep past (H8): run when the world is made and kept
    // with it; the first spawn among the era's people.
    let era_id = save_state
        .as_ref()
        .map_or_else(|| spec.era.clone(), |s| s.meta.settings.era.clone());
    let era = crate::eras::era_of(&content, &era_id).cloned();
    let history = era.as_ref().and_then(|e| {
        let graph = hearth_craft::Graph::from_content(&content);
        let file = save_state
            .as_ref()
            .map(|s| s.dir.root.join(crate::eras::FILE))
            .or_else(|| {
                spec.cache_dir.as_ref().map(|d| {
                    d.join(format!(
                        "history_{seed}_{}_{}.json.zst",
                        e.id.rsplit(':').next().unwrap_or(&e.id),
                        planet.circumference()
                    ))
                })
            });
        crate::eras::history(&lw, &content, &graph, e, seed, file.as_deref(), &|f| {
            log::debug!("deep time {:.0}%", f * 100.0)
        })
    });
    // The calendar starts in the morning of the starting season at the world's first spawn.
    let among = |at: DVec2| match (&history, &era) {
        (Some(h), Some(e)) => crate::eras::spawn_among(h, e, at),
        _ => at,
    };
    let (calendar, first_spawn) = calendar_at(&lw, life.starting_season, Some(&among));
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
    // Who the player is: the child of two parents of the place they begin (V2.1 Addendum A,
    // D173), looking as their genes make them; a save from before genes draws its birth now.
    let genetics = hearth_people::Genetics::from_content(&content);
    let (
        mut player,
        mut appearance,
        mut birth,
        mut household,
        mut childhood,
        mut past_lives,
        new_life,
    ) = match saved {
        Some(p) => (
            p.player,
            p.appearance.sanitized(),
            p.birth,
            p.household,
            p.childhood,
            p.past_lives,
            false,
        ),
        None => (
            Player::new(&cfg, first_spawn, seed ^ 0x5eed),
            crate::born::unborn(&spec.wish),
            None,
            None,
            None,
            Default::default(),
            true,
        ),
    };
    // A new life begins at birth, in a family of the place, and its childhood is lived (V2.1
    // Addendum A) — or, for tests and bots, at its people's coming of age.
    let life_begins = if spec.childhood {
        0.0
    } else {
        crate::born::coming_of_age(&content)
    };
    // In an era's world a new life is born into one of its households, once the recent past
    // about the place is lived (below); Wild Earth's is one of its wandering families (H3).
    let era_birth = history.is_some() && new_life;
    if birth.is_none()
        && !era_birth
        && let Some(g) = &genetics
    {
        let female = if new_life {
            spec.wish.female
        } else {
            Some(appearance.body == hearth_character::BodyType::Female)
        };
        let latitude = planet.latitude_deg(player.mover.pos.z);
        birth = crate::born::draw(g, latitude, female, seed);
        if let Some(b) = &birth {
            let age = if new_life {
                life_begins
            } else {
                crate::born::GROWN_YEARS
            };
            appearance =
                crate::born::player(&content, b, &appearance.name, appearance.loincloth, age);
            if new_life {
                household = Some(crate::born::household(g, b, age, seed));
                if spec.childhood {
                    childhood = Some(crate::childhood::Childhood::new(calendar.days(ticks)));
                }
            }
        }
    }
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
    // What is kept after death, whether one may be born again, and whether a death ends the
    // world (the Permadeath preset: no one to live on as, no being born again).
    let kept = match life.after_death {
        hearth_save::AfterDeath::TheirsOnly => hearth_craft::Kept::TheirsOnly,
        hearth_save::AfterDeath::HeadStart => hearth_craft::Kept::HeadStart,
        hearth_save::AfterDeath::KeepEverything => hearth_craft::Kept::Everything,
    };
    let born_again = life.born_again;
    let permadeath = life.death().ends_the_world();
    let ended = save_state.as_ref().and_then(|s| {
        s.meta
            .ended
            .then(|| s.dir.read_json::<LifeSummary>("life.json").ok().flatten())
            .flatten()
    });
    let mut death_told = player.body.dead.is_some();
    // Where the player last died, and who the world lets them live on as (Addendum B §2.3).
    let mut died_at: Option<DVec3> = player.body.dead.is_some().then_some(player.mover.pos);
    let inhabit_scope = life.inhabit;
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
    let years_at = |t: u64| calendar.days(t) / calendar.days_per_year();
    let peoples = crate::eras::peoples(&content, era.as_ref());
    let mut fauna = crate::fauna::Fauna::new(
        &lw,
        seed,
        calendar.year_offset,
        years_at(ticks),
        save_state.as_ref().map(|s| s.dir.root.as_path()),
        life.hominin_range == hearth_save::HomininRange::SingleCradleRegion,
        &peoples,
        history
            .clone()
            .map(|h| h as Arc<dyn hearth_fauna::ecology::Peopling>),
        life.players,
    );
    let mut animals_shown = false;
    // The people about the player: persons drawn out of the populations' bands, and everyone
    // met before, as saved; and the person the developer's inspector looks at.
    let mut people = crate::people::PeopleNear::new(
        &content,
        &workshop.graph,
        &cfg,
        seed,
        save_state.as_ref().map(|s| s.dir.root.as_path()),
    );
    people.live.history = history.clone();
    // How the era's peoples move through the year: their rounds' camps, their gatherings.
    people.live.era = crate::eras::ways(era.as_ref(), &calendar, lw.generator.terrain.clone());
    // The player's species: their person's (an era's people may be another than ours, H8).
    let mut player_species: String = people.live.player_person(0).map_or_else(
        || crate::born::PLAYER_SPECIES.to_owned(),
        |p| p.species.clone(),
    );
    // The optional conversation backend (V2.1 §10.4; H10): off until the client sets it up.
    let mut conversation = crate::conversation::Conversation::new(&content, &workshop.graph);
    let mut inspecting: Option<u64> = None;
    // The person the player looks at (H9).
    let mut regarding: Option<u64> = None;
    // Watching the world (the Observer, H9): where its eye is, and whom it follows.
    let mut observing: Option<DVec3> = None;
    let mut following: Option<u64> = None;
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
            death: life.death(),
            items: items.clone(),
            content: content.clone(),
            crafts: workshop.crafts.clone(),
            graph: workshop.graph.clone(),
            knowledge_mode: mode,
            ended,
            lod_cache: spec.cache_dir.as_ref().map(|d| {
                d.join("lod")
                    .join(format!("{seed}_{}", planet.circumference()))
            }),
        })))
        .is_err()
    {
        return Ok(());
    }
    // A new life begins with the birth shown.
    if new_life && let Some(b) = &birth {
        let latitude = planet.latitude_deg(player.mover.pos.z);
        let shown = crate::born::shown(&content, b, &appearance, latitude, household.as_ref());
        let _ = tx.send(ToClient::Born(Box::new(shown)));
    }
    // A life begun in one of an era's households (H8): the player's person a child of the
    // mother and the father, the body their genes give, the place their band keeps, the
    // childhood lived from `$age` (or none, starting grown).
    macro_rules! era_life {
        ($option:expr, $female:expr, $age:expr) => {{
            let age: f64 = $age;
            let now = hearth_people::Now {
                tick: ticks,
                hour: 12.0,
                day: calendar.days(ticks),
                year_days: calendar.days_per_year(),
            };
            let graph = workshop.graph.clone();
            match genetics.as_ref().and_then(|g| {
                people
                    .live
                    .born_into($option, 0, $female, age, g, &people.species, &graph, now)
            }) {
                Some((b, h, _)) => {
                    let band = people.live.bands.iter().find(|x| x.id == $option.band);
                    if let Some(band) = band {
                        player_species = band.species.clone();
                    }
                    let home = band.map_or(player.mover.pos, |x| {
                        x.camp
                            .unwrap_or_else(|| DVec3::new(x.home.x, 0.0, x.home.y))
                    });
                    let at = ground_at(&lw, home.x.floor() as i32, home.z.floor() as i32);
                    let mut looks = crate::born::appearance_of(
                        &content,
                        &player_species,
                        &b.phenotype,
                        b.genome.female,
                        age as f32,
                    );
                    looks.name = appearance.name.clone();
                    looks.loincloth = appearance.loincloth;
                    appearance = looks.sanitized();
                    player.mover = Mover::new(at);
                    player.life = hearth_player::Life::begin(at, ticks);
                    player.carry = outfit(&appearance);
                    // And, where the country is cold, what the band's people wear against it,
                    // of hide (D202).
                    let cold = lw
                        .terrain()
                        .sample(at.x.floor() as i32, at.z.floor() as i32)
                        .temperature
                        < DRESSED_BELOW_C;
                    for g in people
                        .live
                        .dress($option.band, &content)
                        .into_iter()
                        .filter(|_| cold)
                    {
                        if let Some(k) = items.garment(&g.id, "hearth:rawhide") {
                            let _ = player.carry.wear(&items, hearth_items::Stack::one(&k.id));
                        }
                    }
                    worn = dress_carry(&player.carry);
                    childhood = (age <= 0.0)
                        .then(|| crate::childhood::Childhood::new(calendar.days(ticks)));
                    let latitude = planet.latitude_deg(at.z);
                    let shown = crate::born::shown(&content, &b, &appearance, latitude, Some(&h));
                    birth = Some(b);
                    household = Some(h);
                    let _ = tx.send(ToClient::Born(Box::new(shown)));
                    let _ = tx.send(ToClient::Person(appearance.clone()));
                    let _ = tx.send(ToClient::Placed(player.mover));
                    true
                }
                None => false,
            }
        }};
    }
    // An era's new life (H8): the recent past about the place lived — a century of its
    // households — then a birth into one of them: the one a test or bot asks for, or the
    // player's choice of those offered.
    let mut offered: Vec<hearth_people::BirthOption> = Vec::new();
    let mut awaiting_birth = false;
    if era_birth {
        fauna.ensure_about(&lw, player.mover.pos);
        let now = hearth_people::Now {
            tick: ticks,
            hour: 12.0,
            day: calendar.days(ticks),
            year_days: calendar.days_per_year(),
        };
        let at = DVec2::new(player.mover.pos.x, player.mover.pos.z);
        let graph = workshop.graph.clone();
        let t0 = Instant::now();
        let bands = people.live.recent_history(
            &mut fauna.eco,
            &people.species,
            &graph,
            &items,
            &|p: DVec3| planet.latitude_deg(p.z),
            at,
            hearth_people::sim::HOUSEHOLD_M,
            RECENT_YEARS,
            now,
            &|f| log::debug!("the recent past {:.0}%", f * 100.0),
        );
        log::info!(
            "the recent past lived: {bands} bands, a century, in {:.1} s",
            t0.elapsed().as_secs_f64()
        );
        offered = people.live.birth_options(
            &people.species,
            at,
            hearth_people::sim::HOUSEHOLD_M,
            life_begins,
            &now,
            4,
        );
        // None about the place (a small world's few people): the households of the world's
        // others, wherever they live; the life begins where its band keeps camp.
        if offered.is_empty() {
            offered =
                people
                    .live
                    .birth_options(&people.species, at, f64::INFINITY, life_begins, &now, 4);
        }
        match (spec.birth, offered.is_empty()) {
            (_, true) => log::warn!("no household to be born into about the place"),
            (Some(k), false) => {
                let o = offered[k.min(offered.len() - 1)];
                era_life!(o, spec.wish.female, life_begins);
            }
            (None, false) => {
                let choices = offered
                    .iter()
                    .map(|o| people.birth_choice(o, &lw, &graph, &now))
                    .collect();
                let _ = tx.send(ToClient::Births(choices));
                awaiting_birth = true;
            }
        }
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
    let mut sleep_warp = 0.0f64;
    let mut warp_carry = 0.0f64;
    // The childhood (V2.1 Addendum A): its moments, the grown body it is sized from, the years
    // passing (extra ticks a second), where it holds the player, what the client was told.
    let mut moments = crate::childhood::curriculum(&content, &player_species);
    let grown_cfg = cfg.clone();
    let mut childhood_warp: f64;
    let mut held: Option<DVec3> = None;
    let mut childhood_told: Option<hearth_protocol::ChildhoodView> = None;
    let mut childhood_sent = u64::MAX;
    let mut sized_at = f64::NEG_INFINITY;
    // The world waits while the player chooses their birth.
    let mut paused = awaiting_birth;
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
                scales: &scales,
                moment: calendar.at(ticks),
                ticks,
                ticks_per_day: calendar.ticks_per_day(),
                player: &mut player,
                world_items: &mut world_items,
                changed: &mut gathered,
                out: &mut outbox,
                items_changed: &mut items_changed,
                facing: last_moved.as_ref().map_or(0.0, |m| m.yaw),
            }
        };
    }
    loop {
        // Messages from the client.
        loop {
            match inbox.try_recv() {
                Ok(ToServer::Moved(m)) => {
                    if held.is_some() {
                        continue;
                    }
                    if player.body.dead.is_none() {
                        player.life.moved(player.mover.pos, m.mover.pos);
                    }
                    let scale = player.mover.scale;
                    player.mover = m.mover;
                    player.mover.scale = scale;
                    if !player.asleep
                        && let Some(v) = m.landed
                    {
                        player.body.land(&cfg, v);
                    }
                    if m.airless_s >= DROWN_S && childhood.as_ref().is_none_or(|c| c.grown()) {
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
                        world_items.add_owned(stack, rest.to_array(), 0.0, 0);
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
                        world_items.add_owned(stack, rest_on(&lw, at).to_array(), 0.0, 0);
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
                    if let Some(f) = workshop.throw(&mut here!(), dir, speed) {
                        // What it strikes on its way, and where it falls.
                        let year_frac = calendar.at(ticks).year_frac as f32;
                        let mut end = f.path.last().copied().unwrap_or(player.mover.pos);
                        let kind = items.get(&f.stack.id);
                        if let Some(hit) =
                            fauna
                                .live
                                .hit_along(&fauna.eco.catalog.clone(), &f.path, year_frac)
                        {
                            let v = f.speed_at(hit.segment);
                            let blow = hearth_fauna::wound::Blow {
                                energy_j: (0.5 * f.mass * v * v) as f32,
                                piercing: kind.and_then(|k| k.property("piercing")).unwrap_or(0.0),
                            };
                            let what = kind.map_or("thing".to_owned(), |k| k.name.clone());
                            if let Some(s) = fauna.live.strike(
                                &fauna.eco.catalog.clone(),
                                &hit,
                                &blow,
                                &what,
                                player.mover.pos,
                                year_frac,
                            ) {
                                let _ = tx.send(ToClient::Acted(hearth_protocol::Acted {
                                    process: String::new(),
                                    done: false,
                                    words: s.words,
                                }));
                            }
                            end = hit.at;
                        }
                        world_items.add_owned(f.stack, rest_on(&lw, end).to_array(), 0.0, 0);
                        items_changed = true;
                    }
                }
                Ok(ToServer::Thrust { dir }) => {
                    // A thrust or a blow with what is in the right hand, at what is in reach.
                    if player.can_act(&cfg)
                        && let Some(held) = player.carry.right.as_ref()
                        && let Some(kind) = items.get(&held.id)
                    {
                        let reach = kind.property("reach_m").unwrap_or(0.5) as f64;
                        let eye = player.mover.pos + DVec3::new(0.0, 1.5, 0.0);
                        let path = [eye, eye + dir.normalize_or_zero() * (0.7 + reach)];
                        let year_frac = calendar.at(ticks).year_frac as f32;
                        let cat = fauna.eco.catalog.clone();
                        if let Some(hit) = fauna.live.hit_along(&cat, &path, year_frac) {
                            // A spear thrust with the body behind it, against a blow of the arm.
                            let blow = hearth_fauna::wound::Blow {
                                energy_j: if reach >= 1.5 {
                                    150.0
                                } else {
                                    40.0 + 60.0 * kind.mass_kg.min(1.5)
                                },
                                piercing: kind.property("piercing").unwrap_or(0.0),
                            };
                            let what = kind.name.clone();
                            if let Some(s) = fauna.live.strike(
                                &cat,
                                &hit,
                                &blow,
                                &what,
                                player.mover.pos,
                                year_frac,
                            ) {
                                let _ = tx.send(ToClient::Acted(hearth_protocol::Acted {
                                    process: String::new(),
                                    done: false,
                                    words: s.words,
                                }));
                            }
                        }
                    }
                }
                Ok(ToServer::Give(stack)) => {
                    let body_kg = cfg.mass_kg as f32;
                    if items.get(&stack.id).is_some() {
                        let left = match player.carry.stow(&items, stack, body_kg) {
                            Ok(()) => None,
                            Err(s) => player.carry.drag(&items, s, body_kg).err(),
                        };
                        if let Some((s, _)) = left {
                            world_items.add_owned(s, player.mover.pos.to_array(), 0.0, 0);
                            items_changed = true;
                        }
                    }
                }
                Ok(ToServer::Speak { person, ask }) => {
                    speak(&mut people, &player, &workshop.graph, person, ask, tx);
                }
                Ok(ToServer::SayText { person, text }) => {
                    // Typed words (V2.1 §10.4; H10): read by the backend as an act, to one within
                    // speaking distance; with none, not heard.
                    let near = people.live.get(person).is_some_and(|q| {
                        q.alive() && (q.place.pos - player.mover.pos).length() < SPEAK_M
                    });
                    let text: String = text.trim().chars().take(SAID_CHARS).collect();
                    if !conversation.free_text() {
                        let _ = tx.send(ToClient::Acted(hearth_protocol::Acted {
                            process: String::new(),
                            done: false,
                            words: "No one takes your meaning: speak with the talk wheel."
                                .to_owned(),
                        }));
                    } else if near && player.body.dead.is_none() && !text.is_empty() {
                        // The techniques the words may mean: every one the game has.
                        let offered: Vec<(String, String)> = workshop
                            .graph
                            .nodes
                            .iter()
                            .filter(|n| n.implemented)
                            .map(|n| (n.id.clone(), n.name.clone()))
                            .collect();
                        let whom = people.as_known(0, person);
                        conversation.read(person, &text, &whom, offered);
                    }
                }
                Ok(ToServer::Conversation(o)) => {
                    conversation.configure(&o);
                    let _ = tx.send(ToClient::Conversing {
                        on: conversation.on(),
                        free_text: conversation.free_text(),
                        trouble: conversation.trouble.clone(),
                    });
                }
                Ok(ToServer::GiveTo { person }) => {
                    // A gift (V2.1 §8.7): what the hands hold, the right first, to one within
                    // reach; kept when they cannot take it.
                    let near = people.live.get(person).is_some_and(|q| {
                        q.alive()
                            && q.player.is_none()
                            && (q.place.pos - player.mover.pos).length() < GIVE_M
                    });
                    if near && player.can_act(&cfg) {
                        let right = player.carry.right.is_some();
                        let held = if right {
                            player.carry.right.take()
                        } else {
                            player.carry.left.take()
                        };
                        if let Some(stack) = held {
                            match people.give_to(0, person, stack, &items, &lw.content) {
                                Ok(words) => {
                                    let _ = tx.send(ToClient::Acted(hearth_protocol::Acted {
                                        process: String::new(),
                                        done: true,
                                        words,
                                    }));
                                }
                                Err(stack) => {
                                    if right {
                                        player.carry.right = Some(stack);
                                    } else {
                                        player.carry.left = Some(stack);
                                    }
                                }
                            }
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
                Ok(ToServer::Inhabit(id)) => {
                    // Living on as another of the world's people (Addendum B §2): the person's
                    // body, what it knows and carries, where it stands, taken up whole — one the
                    // world's scope allows, not fighting, fleeing, dying or giving birth; a
                    // child's childhood lived on from its age, safe throughout.
                    let died = died_at.unwrap_or(player.mover.pos);
                    if player.body.dead.is_some()
                        && people.may_live_as(0, id, died, inhabit_scope)
                        && let Some(q) = people.live.inhabit(0, id)
                    {
                        let maturity = people
                            .species
                            .get(&q.species)
                            .map_or(18.0, |sp| sp.life.maturity_years as f64);
                        let day = calendar.days(ticks);
                        let age = (day - q.life.born) / calendar.days_per_year().max(1.0);
                        player_species = q.species.clone();
                        moments = crate::childhood::curriculum(&content, &player_species);
                        cfg = grown_cfg.clone();
                        player.body = q.body.clone();
                        let knew = std::mem::take(&mut player.knowledge);
                        player.knowledge = q.knowledge.clone().lived_on(
                            &knew.journal,
                            &past_lives,
                            kept,
                            &workshop.graph,
                            ticks,
                        );
                        workshop.knowledge_changed = true;
                        player.carry = q.possessions.carry.clone();
                        if player.carry.worn.is_empty() {
                            player.carry = outfit(&appearance);
                        }
                        worn = dress_carry(&player.carry);
                        // One lived as a household has no footing yet: the ground where it is.
                        let at = if q.tier == hearth_people::person::Tier::Full {
                            q.place.pos
                        } else {
                            ground_at(
                                &lw,
                                q.place.pos.x.floor() as i32,
                                q.place.pos.z.floor() as i32,
                            )
                        };
                        player.mover = Mover::new(at);
                        player.asleep = false;
                        player.lying = false;
                        player.life = hearth_player::Life::begin(at, ticks);
                        if let (Some(genome), Some(phenotype)) = (&q.genome, &q.phenotype) {
                            let parent = |id: Option<u64>| {
                                id.and_then(|id| people.live.get(id))
                                    .and_then(|m| m.genome.clone())
                                    .unwrap_or_else(|| genome.clone())
                            };
                            let parent_ph = |id: Option<u64>| {
                                id.and_then(|id| people.live.get(id))
                                    .and_then(|m| m.phenotype.clone())
                                    .unwrap_or_else(|| phenotype.clone())
                            };
                            birth = Some(hearth_people::Birth {
                                mother: parent(q.life.mother),
                                father: parent(q.life.father),
                                mother_phenotype: parent_ph(q.life.mother),
                                father_phenotype: parent_ph(q.life.father),
                                genome: genome.clone(),
                                phenotype: phenotype.clone(),
                            });
                            let mut looks = crate::born::appearance_of(
                                &content,
                                &player_species,
                                phenotype,
                                q.life.female,
                                age.min(maturity.max(age)) as f32,
                            );
                            looks.name = appearance.name.clone();
                            looks.loincloth = appearance.loincloth;
                            appearance = looks.sanitized();
                        }
                        household = None;
                        childhood = (age < maturity).then(|| {
                            crate::childhood::Childhood::taken_up(q.life.born, age, &moments)
                        });
                        sized_at = f64::NEG_INFINITY;
                        death_told = false;
                        died_at = None;
                        let known: Vec<String> = player
                            .knowledge
                            .known
                            .keys()
                            .filter_map(|k| workshop.graph.node(k).map(|n| n.name.clone()))
                            .collect();
                        let _ = tx.send(ToClient::Person(appearance.clone()));
                        let _ = tx.send(ToClient::Placed(player.mover));
                        let _ = tx.send(ToClient::WhoYouAre(people.who_you_are(id, &known)));
                    }
                }
                Ok(ToServer::BornAgain { at, female }) => {
                    // Born again (Addendum B §2.2): about the place chosen (or where they died),
                    // into one of its households — offered to choose from in an era's world, one
                    // of Wild Earth's families there — the childhood lived from birth. Never
                    // under permadeath.
                    if player.body.dead.is_some() && born_again {
                        let here = at.unwrap_or_else(|| died_at.unwrap_or(player.mover.pos));
                        let (x, z) = lw
                            .terrain()
                            .spawn_near(here.x.floor() as i32, here.z.floor() as i32);
                        let place = ground_at(&lw, x, z);
                        // The one who died lies where they fell, theirs no more.
                        people.live.release_player(0);
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
                        cfg = grown_cfg.clone();
                        player = Player::new(&cfg, place, seed ^ ticks);
                        player.knowledge = hearth_craft::KnowledgeState::default().lived_on(
                            &knew.journal,
                            &past_lives,
                            kept,
                            &workshop.graph,
                            ticks,
                        );
                        workshop.knowledge_changed = true;
                        player.life = hearth_player::Life::begin(place, ticks);
                        household = None;
                        childhood = None;
                        sized_at = f64::NEG_INFINITY;
                        death_told = false;
                        died_at = None;
                        let now = hearth_people::Now {
                            tick: ticks,
                            hour: 12.0,
                            day: calendar.days(ticks),
                            year_days: calendar.days_per_year(),
                        };
                        let at2 = DVec2::new(place.x, place.z);
                        if history.is_some() {
                            // The recent past about the place, where no band of the era's
                            // peoples is lived yet; then the households offered.
                            let lived = people.live.bands.iter().any(|b| {
                                matches!(
                                    b.tier,
                                    hearth_people::person::Tier::Full
                                        | hearth_people::person::Tier::Household
                                ) && (b.home - at2).length() <= hearth_people::sim::HOUSEHOLD_M
                            });
                            if !lived {
                                fauna.ensure_about(&lw, place);
                                let graph = workshop.graph.clone();
                                people.live.recent_history(
                                    &mut fauna.eco,
                                    &people.species,
                                    &graph,
                                    &items,
                                    &|p: DVec3| planet.latitude_deg(p.z),
                                    at2,
                                    hearth_people::sim::HOUSEHOLD_M,
                                    RECENT_YEARS,
                                    now,
                                    &|_| {},
                                );
                            }
                            offered = people.live.birth_options(
                                &people.species,
                                at2,
                                hearth_people::sim::HOUSEHOLD_M,
                                life_begins,
                                &now,
                                4,
                            );
                            if offered.is_empty() {
                                offered = people.live.birth_options(
                                    &people.species,
                                    at2,
                                    f64::INFINITY,
                                    life_begins,
                                    &now,
                                    4,
                                );
                            }
                            let graph = workshop.graph.clone();
                            let choices: Vec<hearth_protocol::BirthChoice> = offered
                                .iter()
                                .map(|o| people.birth_choice(o, &lw, &graph, &now))
                                .collect();
                            if choices.is_empty() {
                                let _ = tx.send(ToClient::Acted(hearth_protocol::Acted {
                                    process: String::new(),
                                    done: false,
                                    words:
                                        "No household of the era's people lives about that place."
                                            .into(),
                                }));
                            }
                            let _ = tx.send(ToClient::Births(choices));
                            awaiting_birth = !offered.is_empty();
                            paused = awaiting_birth;
                        } else if let Some(g) = &genetics {
                            // Wild Earth: one of its families about the place (H3's birth).
                            let latitude = planet.latitude_deg(place.z);
                            birth = crate::born::draw(g, latitude, female, seed ^ ticks);
                            if let Some(b) = &birth {
                                appearance = crate::born::player(
                                    &content,
                                    b,
                                    &appearance.name,
                                    appearance.loincloth,
                                    life_begins,
                                );
                                household =
                                    Some(crate::born::household(g, b, life_begins, seed ^ ticks));
                                if spec.childhood {
                                    childhood = Some(crate::childhood::Childhood::new(
                                        calendar.days(ticks),
                                    ));
                                }
                                let shown = crate::born::shown(
                                    &content,
                                    b,
                                    &appearance,
                                    latitude,
                                    household.as_ref(),
                                );
                                let _ = tx.send(ToClient::Born(Box::new(shown)));
                            }
                            player_species = crate::born::PLAYER_SPECIES.to_owned();
                            moments = crate::childhood::curriculum(&content, &player_species);
                        }
                        player.carry = outfit(&appearance);
                        worn = dress_carry(&player.carry);
                        let _ = tx.send(ToClient::Person(appearance.clone()));
                        let _ = tx.send(ToClient::Placed(player.mover));
                    }
                }
                Ok(ToServer::Childhood(skip)) => {
                    if let Some(ch) = &mut childhood
                        && !ch.grown()
                    {
                        let year_ticks = calendar.ticks_per_day() * calendar.days_per_year();
                        let born_tick = ch.born * calendar.ticks_per_day();
                        let to = match (skip, &ch.phase) {
                            (
                                hearth_protocol::Skip::Next,
                                crate::childhood::Phase::Moment { .. },
                            ) => {
                                ch.phase = crate::childhood::Phase::Passing;
                                None
                            }
                            (hearth_protocol::Skip::Next, _) => ch.next_age(&moments),
                            (hearth_protocol::Skip::GrownUp, _) => {
                                ch.next = moments.len().saturating_sub(1);
                                ch.phase = crate::childhood::Phase::Passing;
                                ch.next_age(&moments)
                            }
                        };
                        // The years skipped are lived at the household's pace.
                        if let Some(age) = to {
                            ticks = ticks.max((born_tick + age * year_ticks).ceil() as u64);
                        }
                    }
                }
                Ok(ToServer::SkipHours(h)) => {
                    let dt = h / 24.0 * calendar.ticks_per_day();
                    ticks = (ticks as f64 + dt).max(0.0) as u64;
                }
                Ok(ToServer::TimeWarp(w)) => warp = w.max(0.0),
                Ok(ToServer::Pause(p)) => paused = p || awaiting_birth,
                Ok(ToServer::BeBorn { choice, female }) => {
                    if awaiting_birth && let Some(&o) = offered.get(choice) {
                        let age = life_begins;
                        if era_life!(o, female, age) {
                            awaiting_birth = false;
                            paused = false;
                            moments = crate::childhood::curriculum(&content, &player_species);
                        }
                    }
                }
                Ok(ToServer::Shout) => shouted = ticks,
                Ok(ToServer::Die { species, at }) => fauna.die(&species, at),
                Ok(ToServer::Inspect(id)) => inspecting = id,
                Ok(ToServer::Observe(eye)) => observing = eye,
                Ok(ToServer::Follow(id)) => {
                    following = id;
                    if id.is_none() {
                        let _ = tx.send(ToClient::LifeOf(None));
                    }
                }
                Ok(ToServer::Chronicle) => {
                    let lines = crate::observer::chronicle(
                        &people.live.notable,
                        history.as_deref(),
                        lw.terrain(),
                        calendar.days(ticks),
                        calendar.days_per_year(),
                    );
                    let _ = tx.send(ToClient::Chronicle(lines));
                }
                Ok(ToServer::Overlay(kind)) => {
                    let map = kind.and_then(|k| {
                        let bands: Vec<(DVec3, usize)> = people
                            .live
                            .bands
                            .iter()
                            .filter(|b| b.tier != hearth_people::person::Tier::Dormant)
                            .map(|b| {
                                let at = b.camp.unwrap_or(DVec3::new(b.home.x, 0.0, b.home.y));
                                (at, b.members.len())
                            })
                            .collect();
                        crate::observer::overlay(k, history.as_deref(), &bands, &planet)
                    });
                    let _ = tx.send(ToClient::Overlay(map));
                }
                Ok(ToServer::Regard(id)) => {
                    regarding = id;
                    if id.is_none() {
                        let _ = tx.send(ToClient::Regarded(None));
                    }
                }
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
                        &birth,
                        &household,
                        &childhood,
                        &past_lives,
                        &world_items,
                        &workshop,
                        &lw,
                        &fauna,
                        &people,
                        ticks,
                    );
                    let _ = tx.send(ToClient::Saved);
                }
                Ok(ToServer::Quit) | Err(TryRecvError::Disconnected) => {
                    save(
                        &mut save_state,
                        &player,
                        &appearance,
                        &birth,
                        &household,
                        &childhood,
                        &past_lives,
                        &world_items,
                        &workshop,
                        &lw,
                        &fauna,
                        &people,
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
            // The childhood (V2.1 Addendum A): its moments at the world's pace and the years
            // between passing quickly; the child sized to its age, safe, where it should be.
            childhood_warp = 0.0;
            held = None;
            if let Some(ch) = &mut childhood
                && let Some(b) = &birth
            {
                // Grown already: only the body grows on, to its full size.
                let was_grown = ch.grown();
                let year_days = calendar.days_per_year();
                let day = calendar.days(ticks);
                let age = ch.age(day, year_days);
                let people_now = hearth_people::Now {
                    tick: ticks,
                    hour: 12.0,
                    day,
                    year_days,
                };
                let kin = people.live.kin_of_player(0);
                let keeper = people
                    .live
                    .keeper_of_player(0, &people.species, &people_now);
                let mut began: Option<usize> = None;
                match ch.phase.clone() {
                    crate::childhood::Phase::Passing => match moments.get(ch.next) {
                        Some(m) if age + 1e-9 >= m.age as f64 => {
                            let until = ticks + (m.minutes as f64 * 60.0 / TICK_S) as u64;
                            ch.phase = crate::childhood::Phase::Moment {
                                index: ch.next,
                                until,
                            };
                            began = Some(ch.next);
                            ch.next += 1;
                        }
                        Some(_) => {
                            let year_ticks = calendar.ticks_per_day() * year_days;
                            childhood_warp = year_ticks / crate::childhood::YEAR_S;
                        }
                        None => ch.phase = crate::childhood::Phase::Grown,
                    },
                    crate::childhood::Phase::Moment { until, .. } if ticks >= until => {
                        ch.phase = if ch.next < moments.len() {
                            crate::childhood::Phase::Passing
                        } else {
                            crate::childhood::Phase::Grown
                        };
                    }
                    _ => {}
                }
                let just_grown = ch.grown() && !was_grown;
                // What the years taught: the family's ways.
                if began.is_some() || just_grown {
                    let ways = kin
                        .and_then(|k| people.live.bands.iter().find(|band| band.id == k.band))
                        .map(|band| band.culture.knowledge.clone())
                        .unwrap_or_default();
                    if crate::childhood::learn(
                        &mut player.knowledge,
                        &workshop.graph,
                        &ways,
                        age - ch.learned_to,
                        ticks,
                    ) {
                        workshop.knowledge_changed = true;
                    }
                    ch.learned_to = age;
                }
                // The body its age's, to full size: its physiology, its height, its looks.
                let maturity = people
                    .species
                    .get(&player_species)
                    .map_or(18.0, |sp| sp.life.maturity_years as f64);
                if age < maturity + 0.1
                    && ((age - sized_at).abs() > 0.02 || began.is_some() || just_grown)
                {
                    sized_at = age;
                    let female = b.genome.female;
                    cfg = match people.species.get(&player_species) {
                        Some(sp) if age < maturity => Arc::new(
                            sp.body_at(female, age, b.phenotype.z("stature"))
                                .into_owned(),
                        ),
                        _ => grown_cfg.clone(),
                    };
                    let mut looks = crate::born::appearance_of(
                        &content,
                        &player_species,
                        &b.phenotype,
                        female,
                        age as f32,
                    );
                    let grown = crate::born::appearance_of(
                        &content,
                        &player_species,
                        &b.phenotype,
                        female,
                        maturity as f32,
                    );
                    player.mover.scale = (looks.height_m / grown.height_m.max(0.1)).min(1.0) as f64;
                    looks.name = appearance.name.clone();
                    looks.loincloth = appearance.loincloth;
                    if (looks.height_m - appearance.height_m).abs() >= 0.01
                        || (looks.grown - appearance.grown).abs() >= 0.02
                    {
                        appearance = looks.sanitized();
                        let _ = tx.send(ToClient::Person(appearance.clone()));
                    }
                }
                // Where the child is: carried while an infant, kept by the family while the
                // years pass, set by its kin as a moment begins, fetched back if it strays.
                if !was_grown {
                    let infant = people
                        .species
                        .get(&player_species)
                        .is_some_and(|sp| sp.life_stage(age) == hearth_people::LifeStage::Infant);
                    let passing = ch.phase == crate::childhood::Phase::Passing;
                    if let Some((_, at, yaw, height)) = keeper {
                        let right = DVec3::new(yaw.cos() as f64, 0.0, -yaw.sin() as f64);
                        if infant && !ch.grown() {
                            held = Some(at + right * 0.16 + DVec3::Y * (0.42 * height as f64));
                        } else if passing {
                            let p = at + right * 1.5;
                            held = Some(ground_at(&lw, p.x.floor() as i32, p.z.floor() as i32));
                        } else if began.is_some() {
                            // By whoever the moment is with.
                            let with = began.and_then(|i| moments.get(i)).and_then(|m| {
                                use hearth_content::schema::life::Kin;
                                let k = kin?;
                                match m.with {
                                    Kin::Mother | Kin::Family => k.mother,
                                    Kin::Father => k.father,
                                    Kin::Elder => k.eldest,
                                    Kin::Youngest => k.youngest,
                                }
                            });
                            let by = with
                                .and_then(|id| people.live.get(id))
                                .map_or(at, |q| q.place.pos);
                            let p = by + right * 1.5;
                            player.mover = Mover {
                                scale: player.mover.scale,
                                ..Mover::new(ground_at(&lw, p.x.floor() as i32, p.z.floor() as i32))
                            };
                            let _ = tx.send(ToClient::Placed(player.mover));
                        } else if (player.mover.pos - at).length() > crate::childhood::STRAY_M {
                            let p = at + right * 1.5;
                            player.mover = Mover {
                                scale: player.mover.scale,
                                ..Mover::new(ground_at(&lw, p.x.floor() as i32, p.z.floor() as i32))
                            };
                            let _ = tx.send(ToClient::Placed(player.mover));
                            let _ = tx.send(ToClient::Acted(hearth_protocol::Acted {
                                process: String::new(),
                                done: false,
                                words: "You are fetched back to your family.".into(),
                            }));
                        }
                    }
                    // No family yet to keep it (the land about still being made): it waits where it is.
                    if keeper.is_none() && (infant || passing) {
                        held = Some(player.mover.pos);
                    }
                    if let Some(p) = held {
                        player.mover.pos = p;
                        player.mover.vel = DVec3::ZERO;
                    }
                    // What a moment sets going.
                    if let Some(i) = began
                        && let (Some(m), Some(k)) = (moments.get(i), kin)
                    {
                        use hearth_content::schema::life::Setup;
                        match m.setup {
                            Setup::Water => {
                                if let Some(e) = k.eldest {
                                    people.live.lead_to_water(e);
                                }
                            }
                            Setup::Knapping => {
                                // Stones by the father, and some for the child.
                                let cobble = items
                                    .iter()
                                    .find(|kind| kind.id.ends_with("cobble/basalt"))
                                    .map(|kind| kind.id.clone());
                                let father = k
                                    .father
                                    .and_then(|f| people.live.get(f))
                                    .map(|q| q.place.pos);
                                if let (Some(c), Some(f)) = (cobble, father) {
                                    for (n, at) in [f, f, f, player.mover.pos, player.mover.pos]
                                        .into_iter()
                                        .enumerate()
                                    {
                                        let a = n as f64 * 1.3;
                                        let spot =
                                            at + DVec3::new(a.cos() * 0.7, 0.5, a.sin() * 0.7);
                                        world_items.add(
                                            hearth_items::Stack::of(&c, 1),
                                            rest_on(&lw, spot).to_array(),
                                            a as f32,
                                        );
                                    }
                                    items_changed = true;
                                }
                            }
                            Setup::ComingOfAge => {
                                people.live.gather_to(k.band, player.mover.pos);
                            }
                            _ => {}
                        }
                    }
                    // Safe: no hurt stays, and a body worn down is made whole.
                    player.body.injuries.clear();
                    if player.body.dead.is_some() || passing || ticks.is_multiple_of(600) {
                        player.body = hearth_body::Body::new(&cfg, seed ^ ticks);
                        player.asleep = false;
                        player.lying = false;
                    }
                    // What the player is told of it: as it changes, and the age as the years pass.
                    let view = ch.view(&moments, age);
                    let changed = view.as_ref().map(|v| (&v.name, v.passing))
                        != childhood_told.as_ref().map(|v| (&v.name, v.passing));
                    if changed || (passing && ticks.saturating_sub(childhood_sent) >= 20) {
                        childhood_sent = ticks;
                        childhood_told = view.clone();
                        let _ = tx.send(ToClient::Childhood(view));
                    }
                }
            }
            {
                let moment = calendar.at(ticks);
                // The world about the player — or about the Observer's eye, unseen there.
                let at = observing.unwrap_or(player.mover.pos);
                let now = hearth_fauna::live::Now {
                    hour: env.local_time(&moment, at.x) as f32,
                    day_s: (calendar.ticks_per_day() * TICK_S) as f32,
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
                if observing.is_some() {
                    // An Observer is perceived by nothing.
                    presence.pos = at;
                    presence.noise = 0.0;
                    presence.plain = 0.0;
                    presence.shouting = false;
                    presence.running = false;
                    presence.vulnerable = 0.0;
                }
                fauna.tick(&lw, &presence, &now, years_at(ticks), TICK_S as f32, ticks);
                // The people about the player live their tick, their calls heard with the
                // animals'.
                {
                    let e = exposure(&env, &lw, &moment, &player.mover, 0.0);
                    let around = workshop.around(&here!(), at);
                    let seen = hearth_people::PlayerSeen {
                        // The one player of a single-player world (R1 gives each player their own).
                        id: 0,
                        pos: at,
                        running: presence.running,
                        hunting: false,
                        plain: presence.plain,
                    };
                    let people_now = hearth_people::Now {
                        tick: ticks,
                        hour: now.hour * 24.0,
                        day: calendar.days(ticks),
                        year_days: calendar.days_per_year(),
                    };
                    let crafts = workshop.crafts.clone();
                    let graph = workshop.graph.clone();
                    // The player's person where the player is.
                    let yaw = last_moved.as_ref().map_or(0.0, |m| m.yaw);
                    people.live.place_player(0, player.mover.pos, yaw);
                    let seen: &[hearth_people::PlayerSeen] = if observing.is_some() {
                        &[]
                    } else {
                        std::slice::from_ref(&seen)
                    };
                    // Watching the years pass fast (a day of the world or more a tick: a month a
                    // second and faster), the people live at the demographic tier: a
                    // band lived as households only while it holds someone the player knows
                    // (D210).
                    let fast = observing.is_some() && warp * TICK_S >= calendar.ticks_per_day();
                    people.live.household_m = if fast {
                        0.0
                    } else {
                        hearth_people::sim::HOUSEHOLD_M
                    };
                    // Nor is any lived in full about the eye: the view is the globe's.
                    let eyes: &[DVec3] = if fast { &[] } else { std::slice::from_ref(&at) };
                    let t_people = std::time::Instant::now();
                    let ticked = people.tick(
                        &mut lw,
                        &mut fauna,
                        &mut world_items,
                        &items,
                        &crafts,
                        &graph,
                        &mut gathered,
                        e,
                        &|p| workshop.radiant_w_m2(p),
                        around,
                        now.year_frac,
                        eyes,
                        seen,
                        birth.as_ref().zip(household.as_ref()),
                        people_now,
                        TICK_S as f32,
                    );
                    if ticked {
                        items_changed = true;
                    }
                    if t_people.elapsed().as_secs_f64() > 0.05 {
                        log::debug!(
                            "the people lived their tick in {:.2} s",
                            t_people.elapsed().as_secs_f64()
                        );
                    }
                    // Their camps kept: the fire laid and fed, the beds about it (H8).
                    if ticks.is_multiple_of(40) {
                        for c in people.camps() {
                            workshop.keep_camp(&mut here!(), &c);
                        }
                    }
                    // What the people near say, as the player makes it out (V2.1 §10.3) — and,
                    // with a conversation backend, what is said to the player (or overheard while
                    // it is idle) and wholly made out, phrased from the speaker's state (H10).
                    let heard = people.heard_by(0);
                    if !heard.is_empty() && !player.asleep {
                        let mut lines = Vec::with_capacity(heard.len());
                        let mut phrased = Vec::new();
                        for (line, said) in heard {
                            if conversation.on()
                                && line.understood >= PHRASED_UNDERSTOOD
                                && (line.to_you || conversation.idle())
                                && let Some((context, guard)) =
                                    people.context_for(&said, &workshop.graph)
                                && let Some(o) = conversation.phrase(line.id, &context, guard)
                            {
                                phrased.push(o);
                            }
                            lines.push(line);
                        }
                        let _ = tx.send(ToClient::Heard(lines));
                        for o in phrased {
                            if let crate::conversation::Outcome::Phrased { line, text } = o {
                                let _ = tx.send(ToClient::Phrased { line, text });
                            }
                        }
                    }
                    // What the backend has come back with.
                    for o in conversation.poll() {
                        match o {
                            crate::conversation::Outcome::Phrased { line, text } => {
                                let _ = tx.send(ToClient::Phrased { line, text });
                            }
                            crate::conversation::Outcome::Act { person, ask } => {
                                speak(&mut people, &player, &workshop.graph, person, ask, tx);
                            }
                            crate::conversation::Outcome::Unclear {
                                person,
                                text,
                                options,
                            } => {
                                let options = options
                                    .into_iter()
                                    .map(|a| {
                                        let w = ask_words(&a, &workshop.graph);
                                        (a, w)
                                    })
                                    .collect();
                                let _ = tx.send(ToClient::Clarify {
                                    person,
                                    text,
                                    options,
                                });
                            }
                        }
                    }
                    // How the people have met the player (V2.1 §8.7), told.
                    for (words, done) in people.news_for(0) {
                        let _ = tx.send(ToClient::Acted(hearth_protocol::Acted {
                            process: String::new(),
                            done,
                            words,
                        }));
                    }
                    // What the player, awake, sees them do (and the scatters they leave), heard
                    // an hour apart at most.
                    if !player.asleep && player.body.dead.is_none() {
                        let yaw = last_moved.as_ref().map_or(0.0, |m| m.yaw);
                        let eye = at + DVec3::Y * player.mover.stance.height();
                        let hour = (calendar.ticks_per_day() / 24.0) as u64;
                        for t in people.watched(&crafts, &graph, &world_items, eye, yaw) {
                            workshop.hear_now_and_then(&mut here!(), t, hour);
                        }
                        // Shown how by those at their work who would teach the player.
                        for (node, insight) in people.lessons(0, &crafts, eye, yaw, TICK_S as f32) {
                            workshop.taught(&mut here!(), &node, insight);
                        }
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
                let a_child = childhood.as_ref().is_some_and(|c| !c.grown());
                for at in fauna.live.attacks.clone() {
                    if !at.injury.is_empty() && player.body.dead.is_none() && !a_child {
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
                            && clock - s.t < now.day_s as f64
                    });
                    if !near.is_empty() || signs_shown {
                        signs_shown = !near.is_empty();
                        let _ = tx.send(ToClient::Signs {
                            now: clock,
                            day_s: now.day_s,
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
                if ticks.is_multiple_of(20)
                    && let Some(id) = inspecting
                {
                    let report = people.inspect(id, &workshop.graph).map(Box::new);
                    let _ = tx.send(ToClient::Inspected(report));
                }
                if ticks.is_multiple_of(20)
                    && let Some(id) = following
                {
                    let now = hearth_people::Now {
                        tick: ticks,
                        hour: 12.0,
                        day: calendar.days(ticks),
                        year_days: calendar.days_per_year(),
                    };
                    let lines = people.live.life_of(id, &now);
                    let _ = tx.send(ToClient::LifeOf(Some((id, lines))));
                }
                if ticks.is_multiple_of(10)
                    && let Some(id) = regarding
                {
                    let lines = people.regard(0, id);
                    let _ = tx.send(ToClient::Regarded(Some((id, lines))));
                }
                if ticks.is_multiple_of(2) {
                    let views = fauna.views();
                    if !views.is_empty() || animals_shown {
                        animals_shown = !views.is_empty();
                        let _ = tx.send(ToClient::Animals(views));
                    }
                    let views = people.views(player.mover.pos);
                    if !views.is_empty() || people.shown {
                        people.shown = !views.is_empty();
                        let _ = tx.send(ToClient::People(views));
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
                // An event of the world: the player's person dies with them, its people mourn,
                // and their life is told, with who of their people they could live on as.
                people
                    .live
                    .player_died(0, format!("{cause:?}"), calendar.days(ticks));
                // What this life knew joins what the player's lives have known (Addendum B §2.4).
                past_lives.extend(player.knowledge.known.keys().cloned());
                let known: Vec<String> = player
                    .knowledge
                    .known
                    .keys()
                    .filter_map(|k| workshop.graph.node(k).map(|n| n.name.clone()))
                    .collect();
                died_at = Some(player.mover.pos);
                let story = people.life_story(&crate::people::LifeFacts {
                    name: &appearance.name,
                    walked_km: player.life.walked_m / 1000.0,
                    farthest_km: player.life.farthest_m / 1000.0,
                    known,
                    at: player.mover.pos,
                    scope: inhabit_scope,
                });
                let _ = tx.send(ToClient::Story(Box::new(story)));
                if permadeath {
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
                        &birth,
                        &household,
                        &childhood,
                        &past_lives,
                        &world_items,
                        &workshop,
                        &lw,
                        &fauna,
                        &people,
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
            warp_carry += (warp + sleep_warp + work_warp + childhood_warp) * TICK_S;
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
            let weather_every = (calendar.ticks_per_day() / 288.0).max(1.0) as u64;
            if ticks.is_multiple_of(weather_every) {
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
                    calendar.days_per_season as f32 * 4.0,
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
                    &birth,
                    &household,
                    &childhood,
                    &past_lives,
                    &world_items,
                    &workshop,
                    &lw,
                    &fauna,
                    &people,
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
                        20.0 + warp + sleep_warp + work_warp + childhood_warp,
                        &items,
                        drag_friction(&lw, player.mover.pos),
                        held,
                    ))))
                    .is_err()
            {
                save(
                    &mut save_state,
                    &player,
                    &appearance,
                    &birth,
                    &household,
                    &childhood,
                    &past_lives,
                    &world_items,
                    &workshop,
                    &lw,
                    &fauna,
                    &people,
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
                    &birth,
                    &household,
                    &childhood,
                    &past_lives,
                    &world_items,
                    &workshop,
                    &lw,
                    &fauna,
                    &people,
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
                    &birth,
                    &household,
                    &childhood,
                    &past_lives,
                    &world_items,
                    &workshop,
                    &lw,
                    &fauna,
                    &people,
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

/// Said or done to a person within speaking distance (V2.1 §16; H9) — chosen on the wheel, or
/// read from typed words (H10): their answer, and the technique a lesson agreed on is of.
fn speak(
    people: &mut crate::people::PeopleNear,
    player: &Player,
    graph: &hearth_craft::Graph,
    person: u64,
    ask: hearth_people::player::Ask,
    tx: &Sender<ToClient>,
) {
    let near = people
        .live
        .get(person)
        .is_some_and(|q| q.alive() && (q.place.pos - player.mover.pos).length() < SPEAK_M);
    if !near || player.body.dead.is_some() {
        return;
    }
    let knew = &player.knowledge;
    // Offering to teach, unnamed: the first thing the player knows that they do not.
    let ask = match ask {
        hearth_people::player::Ask::Teach(None) => {
            let theirs = people.live.get(person).map(|q| &q.knowledge);
            hearth_people::player::Ask::Teach(
                knew.known
                    .keys()
                    .find(|n| theirs.is_some_and(|t| !t.knows(n)))
                    .cloned(),
            )
        }
        other => other,
    };
    let a = people.ask(0, person, ask, &|n| knew.knows(n));
    let about = a
        .about
        .as_deref()
        .and_then(|n| graph.node(n))
        .map_or(String::new(), |n| format!(" ({})", n.name));
    let _ = tx.send(ToClient::Acted(hearth_protocol::Acted {
        process: String::new(),
        done: a.yes,
        words: format!("{}{about}", a.words),
    }));
}

/// An act offered to choose from, in words.
fn ask_words(a: &hearth_people::player::Ask, graph: &hearth_craft::Graph) -> String {
    use hearth_people::player::Ask;
    let of = |n: &Option<String>| {
        n.as_deref()
            .and_then(|n| graph.node(n))
            .map_or(String::new(), |n| format!(": {}", n.name))
    };
    match a {
        Ask::Greet => "Greet them".to_owned(),
        Ask::Introduce => "Tell them your name".to_owned(),
        Ask::Thank => "Thank them".to_owned(),
        Ask::Apologise => "Say sorry".to_owned(),
        Ask::Praise => "Praise them".to_owned(),
        Ask::Joke => "Joke with them".to_owned(),
        Ask::Insult => "Insult them".to_owned(),
        Ask::BeTaught(n) => format!("Ask to be shown how{}", of(n)),
        Ask::Teach(n) => format!("Offer to show them how{}", of(n)),
        Ask::Join => "Ask to stay with their band".to_owned(),
        Ask::Pair => "Ask them to be your partner".to_owned(),
        Ask::Gesture(g) => format!(
            "A gesture: {}",
            hearth_people::converse::gesture_words(*g)
                .replace("your", "the")
                .replace("yourself", "oneself")
        ),
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

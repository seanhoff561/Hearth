//! Headless screenshot mode: `hearth --screenshot "seed=1,season=autumn,hour=16,out=shot.png"`
//! renders fixed camera shots to PNG without a window (a software adapter is used if no GPU
//! exists). Shots can pick a latitude, date and time, so seasonal suites are one list file.
//! Every shot waits for its distant (LOD) terrain to be built before rendering, and fails if
//! that takes longer than `lod_timeout` seconds.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use glam::DVec3;
use hearth_content::schema::Season;
use hearth_content::schema::config::TimeConfig;
use hearth_env::{Calendar, Precip};
use hearth_math::PlanetSize;
use hearth_render::GpuContext;
use hearth_render::atlas::TextureArray;
use hearth_render::camera::Camera;
use hearth_render::mesh::MeshOptions;
use hearth_render::models::BlockModels;
use hearth_render::offscreen::{OFFSCREEN_FORMAT, OffscreenTarget, write_png};
use hearth_render::precip::SkyHeights;
use hearth_render::scene::SceneRenderer;

use crate::environment::{EnvOverrides, EnvSampler};
use crate::scene::LocalWorld;
use rayon::prelude::*;

/// Sets the grass `ahead` metres in front of the camera alight and lets the fire burn on for
/// `minutes` game minutes in hot dry air, the wind blowing to the camera's right; the blocks it
/// changes are relit. Returns its smoke.
fn burn_ahead(
    lw: &mut LocalWorld,
    camera: &Camera,
    minutes: f64,
    ahead: f64,
    seed: u64,
) -> Vec<hearth_render::smoke::SmokePlume> {
    use crate::wildfire::{Danger, FireWorld, FuelTable, STEP_TICKS, Wildfire};
    struct ShotFire<'a> {
        lw: &'a mut LocalWorld,
        changed: Vec<hearth_math::BlockPos>,
    }
    impl FireWorld for ShotFire<'_> {
        fn block(&self, p: hearth_math::BlockPos) -> Option<hearth_world::BlockStateId> {
            self.lw.map.block(p)
        }
        fn set(&mut self, p: hearth_math::BlockPos, s: hearth_world::BlockStateId) {
            let reg = self.lw.reg.clone();
            self.lw.map.set_block(p, s, &reg);
            self.changed.push(p);
        }
        fn spared(&self, _: hearth_math::BlockPos) -> bool {
            false
        }
    }
    let f = camera.forward().as_dvec3();
    let flat = DVec3::new(f.x, 0.0, f.z).normalize_or(DVec3::Z);
    let side = DVec3::new(-flat.z, 0.0, flat.x);
    let (px, pz) = (camera.pos.x + flat.x * ahead, camera.pos.z + flat.z * ahead);
    let start = hearth_math::BlockPos::containing(DVec3::new(px, lw.surface_y(px, pz) - 0.5, pz));
    let table = FuelTable::new(&lw.reg);
    let mut fire = Wildfire::new(seed);
    let danger = Danger {
        level: 0.9,
        wind: glam::Vec2::new(side.x as f32, side.z as f32) * 4.0,
        rain_mm_h: 0.0,
    };
    let tick_s = 1.5;
    let mut world = ShotFire {
        lw,
        changed: Vec::new(),
    };
    if !fire.ignite(&mut world, &table, start, 0, tick_s) {
        fire.ignite(&mut world, &table, start.up(), 0, tick_s);
    }
    let steps = (minutes * 60.0 / (STEP_TICKS as f64 * tick_s as f64)) as u64;
    let mut beyond = Vec::new();
    for k in 0..steps {
        fire.step(
            &mut world,
            &table,
            danger,
            k * STEP_TICKS,
            tick_s,
            &mut beyond,
        );
    }
    let ShotFire { lw, mut changed } = world;
    changed.sort_unstable();
    changed.dedup();
    let reg = lw.reg.clone();
    for p in &changed {
        lw.light.block_changed(&mut lw.map, &reg, *p);
    }
    log::info!(
        "fire set at {start:?}: {} blocks burning after {minutes} minutes, {} changed",
        fire.burning_count(),
        changed.len()
    );
    fire.plumes()
        .into_iter()
        .map(|p| hearth_render::smoke::SmokePlume {
            at: p.at,
            strength: p.strength,
            far: p.far,
        })
        .collect()
}

/// One screenshot request.
#[derive(Debug, Clone, PartialEq)]
pub struct ShotSpec {
    pub seed: u64,
    pub planet: PlanetSize,
    pub resolution: usize,
    /// Camera position; `None` → spawn (x, z) or a land spot at `lat`, above the surface (y).
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub z: Option<f64>,
    /// Pick a land spot near this latitude (degrees) when x/z aren't given.
    pub lat: Option<f64>,
    /// Or the heart of a biome (`tundra`, `boreal_forest`; `tundra:-12` where the mean
    /// temperature is below −12 °C).
    pub biome: Option<String>,
    /// Height above the surface when `y` is not given.
    pub above: f64,
    /// Height above the ground under the water instead (`floor=2`: on the sea floor).
    pub floor: Option<f64>,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
    pub width: u32,
    pub height: u32,
    /// Horizontal load radius in cubes.
    pub distance: i32,
    /// Season (mid-season in the place's hemisphere) or an explicit year fraction.
    pub season: Option<Season>,
    /// How far into the season (0 its start, 1 its end; `season=autumn@0.2`).
    pub season_at: f64,
    pub year_frac: Option<f64>,
    /// Local solar hour (0–24).
    pub hour: f64,
    /// Cloud cover override (0–1); `None` = the weather's.
    pub clouds: Option<f64>,
    /// Lay the date's snow and ice.
    pub snow: bool,
    /// Precipitation override: type and rate (mm/h of water); `None` = the weather's.
    pub precipitation: Option<(Precip, f64)>,
    /// Output file; `None` → a numbered file in the game's screenshot folder.
    pub out: Option<PathBuf>,
    pub software: bool,
    /// Also render with CPU culling and fail if GPU occlusion culling changes any pixel.
    pub verify_cull: bool,
    /// Distant (LOD) terrain radius in chunks; 0 = none.
    pub lod: u32,
    /// Aerial perspective (haze and the blue of distance); off only for comparisons.
    pub fog: bool,
    /// Seconds the LOD terrain may take to build before the shot fails.
    pub lod_timeout: f64,
    /// Draw the planet as the globe (at this zoom) centred on the camera's place instead.
    pub globe: Option<f32>,
    /// A person standing on the ground this far (m) in front of the camera, facing it.
    pub person: Option<f64>,
    /// Blocks set on the ground in front of the camera: (block state, metres ahead, metres to
    /// the right, blocks up).
    pub place: Vec<(String, f64, f64, i32)>,
    /// Blocks set on the ground about the camera by the compass, for things built square
    /// whichever way the camera looks: (block state, metres east, metres south, blocks up).
    pub put: Vec<(String, f64, f64, i32)>,
    /// The plants cut down about a point in front of the camera (radius m, metres ahead), so
    /// that what stands there is seen.
    pub mow: Option<(f64, f64)>,
    /// Trees grown on the ground in front of the camera: (species, stage, variant, metres
    /// ahead, metres to the right).
    pub trees: Vec<(String, String, u8, f64, f64)>,
    /// Ground cleared or burned in front of the camera, seen years later: (what, radius m,
    /// years since, metres ahead). The rest of the land has grown those years too.
    pub disturb: Vec<(hearth_worldgen::vegetation::DisturbanceKind, f64, f64, f64)>,
    /// A fire set in the grass this far (m) ahead, burned on for this many game minutes in
    /// hot dry air with the wind blowing to the right: (minutes, metres ahead).
    pub fire: Option<(f64, f64)>,
    /// The smoke of a far fire this many metres ahead (as a burning ecological cell sends up).
    pub far_smoke: Vec<f64>,
    /// Temporal anti-aliasing (the shot is the last of a run of jittered frames).
    pub taa: bool,
    /// See through the eyes of a person standing on the ground below the camera (their body
    /// drawn as in first person).
    pub body: bool,
    /// The body's senses on the image: `hurt`, `cold`, `hot`, `exhausted` or `faint`.
    pub senses: Option<String>,
    /// Animals on the ground in front of the camera: (species, stage, female, what it does,
    /// metres ahead, metres to the right, its facing in degrees from the camera's).
    pub animals: Vec<ShotAnimal>,
    /// Hominins on the ground in front of the camera.
    pub hominins: Vec<ShotHominin>,
    /// Signs laid on the ground: whose, how many, what (prints, blood, droppings), from where
    /// (metres ahead, to the right) and going which way (degrees from the camera's).
    pub trails: Vec<(String, usize, hearth_fauna::live::SignKind, f64, f64, f32)>,
    /// The animals the populations put about the camera.
    pub fauna: bool,
    /// The builder's view: the pieces about the camera outlined by how hard they are pressed.
    pub stress: bool,
    /// A saved world to show as its player left it (its seed, planet, date, changes and things
    /// lying about), the camera at the player.
    pub save: Option<PathBuf>,
    /// Metres the camera stands back from where it would be, along the way it looks.
    pub back: f64,
    /// Things lying about, drawn as boxes: middle, size, colour, turn (from a save).
    pub lying: Vec<(DVec3, glam::Vec3, [u8; 3], f32)>,
    /// The camera to the nearest group of this species, looking at it from 30 m along `yaw`.
    pub seek: Option<String>,
    /// Seconds the animals live on before the shot with the camera as a person among them
    /// (they see it, and flee it).
    pub run: Option<f64>,
    /// The camera to the bank of the nearest water at least a metre and a half deep, looking
    /// out over it.
    pub near_water: bool,
    /// The camera's pitch aimed at the group sought (a `seek` without a `pitch`).
    pub aim: bool,
    /// Whether `yaw` was given (a `near` place faces its sight otherwise).
    pub yaw_given: bool,
}

/// A hominin placed in a screenshot: its sex and age, what it does (`stand`, `walk`, `run`,
/// `feed`, `work`, `groom`, `climb`, `sleep`), where (metres ahead, to the right, above the
/// ground) and its facing in degrees from the camera's.
#[derive(Debug, Clone, PartialEq)]
pub struct ShotHominin {
    pub female: bool,
    pub stage: hearth_fauna::live::Stage,
    pub act: String,
    pub ahead: f64,
    pub right: f64,
    pub yaw: f32,
    pub up: f64,
}

/// An animal placed in a screenshot.
#[derive(Debug, Clone, PartialEq)]
pub struct ShotAnimal {
    pub species: String,
    pub stage: hearth_fauna::live::Stage,
    pub female: bool,
    pub act: hearth_fauna::live::Act,
    pub pace: Pace,
    pub ahead: f64,
    pub right: f64,
    pub yaw: f32,
    /// Above the ground (a bird on the wing, a squirrel up a trunk).
    pub up: f64,
    /// On the ground, swimming, on the wing, up a tree (from what it does).
    pub medium: hearth_fauna::live::Medium,
}

/// How fast an animal placed in a shot goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pace {
    Still,
    Walk,
    Trot,
    Run,
}

impl Default for ShotSpec {
    fn default() -> Self {
        Self {
            seed: 1,
            planet: PlanetSize::Standard,
            resolution: 0,
            x: None,
            y: None,
            z: None,
            lat: None,
            biome: None,
            above: 24.0,
            floor: None,
            yaw: 30.0,
            pitch: 15.0,
            fov: 70.0,
            width: 1280,
            height: 720,
            distance: 10,
            season: None,
            season_at: 0.5,
            year_frac: None,
            hour: 11.0,
            clouds: None,
            snow: true,
            precipitation: None,
            out: None,
            software: false,
            verify_cull: false,
            lod: 256,
            fog: true,
            lod_timeout: 180.0,
            globe: None,
            person: None,
            place: Vec::new(),
            put: Vec::new(),
            mow: None,
            trees: Vec::new(),
            disturb: Vec::new(),
            fire: None,
            far_smoke: Vec::new(),
            taa: false,
            body: false,
            senses: None,
            animals: Vec::new(),
            hominins: Vec::new(),
            trails: Vec::new(),
            fauna: false,
            stress: false,
            save: None,
            back: 0.0,
            lying: Vec::new(),
            seek: None,
            run: None,
            near_water: false,
            yaw_given: false,
            aim: false,
        }
    }
}

fn parse_season(v: &str) -> anyhow::Result<Season> {
    Ok(match v {
        "spring" => Season::Spring,
        "summer" => Season::Summer,
        "autumn" | "fall" => Season::Autumn,
        "winter" => Season::Winter,
        other => anyhow::bail!("unknown season {other:?}"),
    })
}

impl ShotSpec {
    /// Parses `key=value` pairs separated by commas.
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let mut spec = Self::default();
        let mut given = Vec::new();
        for kv in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (k, v) = kv
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("expected key=value, got {kv:?}"))?;
            let v = v.trim();
            given.push(k.trim().to_owned());
            match k.trim() {
                "seed" => spec.seed = v.parse()?,
                "planet" => {
                    spec.planet = PlanetSize::from_name(v)
                        .ok_or_else(|| anyhow::anyhow!("unknown planet {v}"))?
                }
                "res" => spec.resolution = v.parse()?,
                "x" => spec.x = Some(v.parse()?),
                "y" => spec.y = Some(v.parse()?),
                "z" => spec.z = Some(v.parse()?),
                "lat" => spec.lat = Some(v.parse()?),
                "biome" => spec.biome = Some(v.to_owned()),
                "above" => spec.above = v.parse()?,
                "floor" => spec.floor = Some(v.parse()?),
                "yaw" => spec.yaw = v.parse()?,
                "pitch" => spec.pitch = v.parse()?,
                "fov" => spec.fov = v.parse()?,
                "w" | "width" => spec.width = v.parse()?,
                "h" | "height" => spec.height = v.parse()?,
                "dist" | "distance" => spec.distance = v.parse()?,
                "season" => {
                    let (name, at) = v.split_once('@').unwrap_or((v, "0.5"));
                    spec.season = Some(parse_season(name)?);
                    spec.season_at = at.parse::<f64>()?.clamp(0.0, 1.0);
                }
                "yf" => spec.year_frac = Some(v.parse()?),
                "hour" => spec.hour = v.parse()?,
                "clouds" => spec.clouds = Some(v.parse()?),
                "snow" => spec.snow = v.parse()?,
                "rain" => spec.precipitation = Some((Precip::Rain, v.parse()?)),
                "sleet" => spec.precipitation = Some((Precip::Sleet, v.parse()?)),
                "snowfall" => spec.precipitation = Some((Precip::Snow, v.parse()?)),
                "dry" if v.parse::<bool>()? => spec.precipitation = Some((Precip::None, 0.0)),
                "dry" => {}
                "out" => spec.out = Some(PathBuf::from(v)),
                "software" => spec.software = v.parse()?,
                "verify_cull" => spec.verify_cull = v.parse()?,
                "lod" => spec.lod = v.parse()?,
                "fog" => spec.fog = v.parse()?,
                "lod_timeout" => spec.lod_timeout = v.parse()?,
                "globe" => spec.globe = Some(v.parse()?),
                "person" => spec.person = Some(v.parse()?),
                // `place=campfire[fire=high]@3` or `@3:1` or `@3:1:1` (ahead:right:up),
                // repeatable.
                // `mow=8@10`: the plants cut within 8 m of the ground 10 m ahead.
                "mow" => {
                    let (radius, ahead) = v.split_once('@').unwrap_or((v, "10"));
                    spec.mow = Some((radius.parse()?, ahead.parse()?));
                }
                "place" => {
                    let (state, at) = v.split_once('@').unwrap_or((v, "3"));
                    let mut n = at.split(':');
                    let ahead = n.next().unwrap_or("3").parse()?;
                    let right = n.next().unwrap_or("0").parse()?;
                    let up = n.next().unwrap_or("0").parse()?;
                    spec.place.push((state.to_owned(), ahead, right, up));
                }
                // `put=hearth:post/hazel_wood@-2:4:1` (east:south:up from the camera),
                // repeatable.
                "put" => {
                    let (state, at) = v.split_once('@').unwrap_or((v, "0:3"));
                    let mut n = at.split(':');
                    let east = n.next().unwrap_or("0").parse()?;
                    let south = n.next().unwrap_or("3").parse()?;
                    let up = n.next().unwrap_or("0").parse()?;
                    spec.put.push((state.to_owned(), east, south, up));
                }
                // `tree=english_oak:mature:2@12:-6` (species, stage, variant @ ahead:right),
                // repeatable.
                "tree" => {
                    let (what, at) = v.split_once('@').unwrap_or((v, "8"));
                    let mut w = what.split(':');
                    let species = w.next().unwrap_or("english_oak").to_owned();
                    let stage = w.next().unwrap_or("mature").to_owned();
                    let variant = w.next().unwrap_or("0").parse()?;
                    let mut n = at.split(':');
                    let ahead = n.next().unwrap_or("8").parse()?;
                    let right = n.next().unwrap_or("0").parse()?;
                    spec.trees.push((species, stage, variant, ahead, right));
                }
                // `clear=30@5` or `burn=40@0.1:20` (radius @ years since : metres ahead).
                "clear" | "burn" => {
                    use hearth_worldgen::vegetation::DisturbanceKind;
                    let kind = if k.trim() == "burn" {
                        DisturbanceKind::Burned
                    } else {
                        DisturbanceKind::Cleared
                    };
                    let (radius, at) = v.split_once('@').unwrap_or((v, "1"));
                    let mut n = at.split(':');
                    let years = n.next().unwrap_or("1").parse()?;
                    let ahead = n.next().unwrap_or("0").parse()?;
                    spec.disturb.push((kind, radius.parse()?, years, ahead));
                }
                // `fire=6@40`: burned on 6 game minutes, set 40 m ahead.
                "fire" => {
                    let (minutes, ahead) = v.split_once('@').unwrap_or((v, "30"));
                    spec.fire = Some((minutes.parse()?, ahead.parse()?));
                }
                // `farsmoke=4000`: a far fire's smoke 4 km ahead, repeatable.
                "farsmoke" => spec.far_smoke.push(v.parse()?),
                "taa" => spec.taa = v.parse()?,
                "body" => spec.body = v.parse()?,
                "senses" => spec.senses = Some(v.to_owned()),
                // `fauna=true`: the animals the populations put about the camera;
                // `seek=red_deer`: the camera to the nearest group of a species.
                "fauna" => spec.fauna = v.parse()?,
                // `stress=true`: the builder's view of what is built about the camera.
                "stress" => spec.stress = v.parse()?,
                // `save=path/to/world`: that world as its player left it.
                "save" => spec.save = Some(PathBuf::from(v)),
                "back" => spec.back = v.parse()?,
                // `run=3`: the animals live on three seconds with the camera among them.
                "run" => spec.run = Some(v.parse()?),
                // `near=water`: on the bank of the nearest deep water.
                "near" if v == "water" => spec.near_water = true,
                "seek" => {
                    spec.fauna = true;
                    spec.seek = Some(v.to_owned());
                }
                // `animal=red_deer:adult:m:graze@20:-3:90` (species, stage, sex, what it does @
                // metres ahead : to the right : facing in degrees from the camera's : above the
                // ground), repeatable; `herd=red_deer:9@25:0` a herd about a point. What it
                // does: graze, walk, trot, run (or flee, gallop), rest, sleep, alert, groom,
                // drink, rear, attack, fly.
                "animal" => {
                    use hearth_fauna::live::Stage;
                    let (what, at) = v.split_once('@').unwrap_or((v, "15"));
                    let mut w = what.split(':');
                    let species = w.next().unwrap_or("red_deer").to_owned();
                    let stage = match w.next().unwrap_or("adult") {
                        "young" => Stage::Young,
                        "juvenile" => Stage::Juvenile,
                        _ => Stage::Adult,
                    };
                    let female = w.next().unwrap_or("f") != "m";
                    let (act, pace, medium) = act_of(w.next().unwrap_or("graze"))?;
                    let mut n = at.split(':');
                    let ahead = n.next().unwrap_or("15").parse()?;
                    let right = n.next().unwrap_or("0").parse()?;
                    let yaw = n.next().unwrap_or("90").parse()?;
                    let up = n.next().unwrap_or("0").parse()?;
                    spec.animals.push(ShotAnimal {
                        species,
                        stage,
                        female,
                        act,
                        pace,
                        ahead,
                        right,
                        yaw,
                        up,
                        medium,
                    });
                }
                // `hominin=f:adult:feed@6:-1:150` (sex, age, what it does @ metres ahead : to the
                // right : facing in degrees from the camera's : above the ground), repeatable.
                "hominin" => {
                    use hearth_fauna::live::Stage;
                    let (what, at) = v.split_once('@').unwrap_or((v, "6"));
                    let mut w = what.split(':');
                    let female = w.next().unwrap_or("f") != "m";
                    let stage = match w.next().unwrap_or("adult") {
                        "young" => Stage::Young,
                        "juvenile" => Stage::Juvenile,
                        _ => Stage::Adult,
                    };
                    let act = w.next().unwrap_or("stand").to_owned();
                    let mut n = at.split(':');
                    spec.hominins.push(ShotHominin {
                        female,
                        stage,
                        act,
                        ahead: n.next().unwrap_or("6").parse()?,
                        right: n.next().unwrap_or("0").parse()?,
                        yaw: n.next().unwrap_or("180").parse()?,
                        up: n.next().unwrap_or("0").parse()?,
                    });
                }
                // `trail=red_deer:12:prints@4:-1:30` a trail of signs (prints, blood or droppings)
                // from a point going a way, repeatable.
                "trail" => {
                    use hearth_fauna::live::SignKind;
                    let (what, at) = v.split_once('@').unwrap_or((v, "4"));
                    let mut w = what.split(':');
                    let species = w.next().unwrap_or("red_deer").to_owned();
                    let n: usize = w.next().unwrap_or("10").parse()?;
                    let kind = match w.next().unwrap_or("prints") {
                        "blood" => SignKind::Blood,
                        "droppings" => SignKind::Droppings,
                        _ => SignKind::Print,
                    };
                    let mut p = at.split(':');
                    let ahead = p.next().unwrap_or("4").parse()?;
                    let right = p.next().unwrap_or("0").parse()?;
                    let yaw = p.next().unwrap_or("0").parse()?;
                    spec.trails.push((species, n, kind, ahead, right, yaw));
                }
                "herd" => {
                    use hearth_fauna::live::{Act, Stage};
                    let (what, at) = v.split_once('@').unwrap_or((v, "25"));
                    let (species, count) = what.split_once(':').unwrap_or((what, "8"));
                    let count: usize = count.parse()?;
                    let mut n = at.split(':');
                    let ahead: f64 = n.next().unwrap_or("25").parse()?;
                    let right: f64 = n.next().unwrap_or("0").parse()?;
                    for k in 0..count {
                        // A herd's mothers, a stag or two, the year's young; grazing, some
                        // walking, one looking up.
                        let u = (k as f64 * 0.618_034).fract();
                        let v2 = (k as f64 * 0.414_214).fract();
                        let r = (count as f64).sqrt() * 2.2 * u.sqrt();
                        let a = v2 * std::f64::consts::TAU;
                        let (stage, female) = match k % 5 {
                            0 if k > 0 => (Stage::Adult, false),
                            3 => (Stage::Young, k % 2 == 0),
                            _ => (Stage::Adult, true),
                        };
                        let (act, pace) = match k % 4 {
                            1 => (Act::Walk, Pace::Walk),
                            3 if k == 3 => (Act::Alert, Pace::Still),
                            _ => (Act::Graze, Pace::Still),
                        };
                        spec.animals.push(ShotAnimal {
                            species: species.to_owned(),
                            stage,
                            female,
                            act,
                            pace,
                            ahead: ahead + r * a.cos(),
                            right: right + r * a.sin(),
                            yaw: (v2 * 360.0) as f32,
                            up: 0.0,
                            medium: hearth_fauna::live::Medium::Ground,
                        });
                    }
                }
                other => anyhow::bail!("unknown screenshot key {other:?}"),
            }
        }
        // Seeking a group, the camera stands a little above the ground, looking at it through
        // a longer lens.
        if spec.seek.is_some() {
            if !given.iter().any(|k| k == "above") {
                spec.above = 4.0;
            }
            if !given.iter().any(|k| k == "fov") {
                spec.fov = 40.0;
            }
            spec.aim = !given.iter().any(|k| k == "pitch");
        }
        spec.yaw_given = given.iter().any(|k| k == "yaw");
        Ok(spec)
    }

    /// Parses a shot list: one spec per line, `#` comments, blank lines ignored. A line
    /// starting with `defaults:` sets keys for all following lines.
    pub fn parse_list(text: &str) -> anyhow::Result<Vec<Self>> {
        let mut defaults = String::new();
        let mut out = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if let Some(d) = line.strip_prefix("defaults:") {
                defaults = d.trim().to_owned();
                continue;
            }
            let spec = Self::parse(&format!("{defaults},{line}"))
                .map_err(|e| anyhow::anyhow!("line {}: {e}", i + 1))?;
            out.push(spec);
        }
        Ok(out)
    }
}

/// Renders every shot. Shots with the same world settings share one generated world. Shots
/// without an output path are numbered into `default_dir`.
pub fn run(specs: &[ShotSpec], cache_dir: Option<&Path>, default_dir: &Path) -> anyhow::Result<()> {
    let t0 = Instant::now();
    let first = specs
        .first()
        .ok_or_else(|| anyhow::anyhow!("no screenshots requested"))?;
    let ctx = GpuContext::headless(first.software).or_else(|e| {
        log::warn!("no hardware adapter ({e}); trying the software adapter");
        GpuContext::headless(true)
    })?;
    log::info!("rendering on {} ({:?})", ctx.info.name, ctx.info.backend);
    let mut world: Option<(u64, PlanetSize, usize, LocalWorld)> = None;
    let mut atlas: Option<(TextureArray, hearth_lod::LodGen)> = None;
    for (i, spec) in specs.iter().enumerate() {
        let out = spec
            .out
            .clone()
            .unwrap_or_else(|| default_dir.join(format!("shot_{:03}.png", i + 1)));
        // A saved world keeps its own seed and planet.
        let mut spec = spec.clone();
        let saved = match &spec.save {
            Some(dir) => {
                let (d, meta, _) = hearth_save::WorldDir::open(dir)?;
                spec.seed = meta.settings.planet.seed;
                spec.planet = meta.settings.planet.planet_size;
                Some((d, meta))
            }
            None => None,
        };
        let key = (spec.seed, spec.planet, spec.resolution);
        if world.as_ref().is_none_or(|w| (w.0, w.1, w.2) != key) {
            let lw = LocalWorld::create(spec.seed, spec.planet, spec.resolution, cache_dir)?;
            world = Some((spec.seed, spec.planet, spec.resolution, lw));
        }
        let lw = &mut world.as_mut().expect("created above").3;
        lw.edits = crate::edits::Edits::default();
        if let Some((d, meta)) = &saved {
            load_saved(lw, &mut spec, d, meta)?;
        }
        let spec = &spec;
        let (atlas, lod) = atlas.get_or_insert_with(|| {
            let entries = hearth_texgen::textures_for(Some(&lw.content));
            (
                TextureArray::from_entries(&entries),
                hearth_lod::LodGen::new(&lw.reg, &entries),
            )
        });
        let time = lw.content.time.clone();
        // Each shot starts from a clean map so dates don't mix (snow from an earlier shot).
        lw.map = hearth_world::CubeMap::new(*lw.map.planet());
        shoot(&ctx, atlas, lod, lw, spec, &out, Some(&time))?;
    }
    log::info!(
        "{} screenshot(s) in {:.2}s",
        specs.len(),
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}

/// Lays a saved world over `lw` and points `spec` at its player: the changes they made, the
/// vegetation as they left it, the things lying about, the date and hour of its clock.
fn load_saved(
    lw: &mut LocalWorld,
    spec: &mut ShotSpec,
    d: &hearth_save::WorldDir,
    meta: &hearth_save::WorldMeta,
) -> anyhow::Result<()> {
    if let Ok(Some(e)) = d.read_json::<crate::edits::EditsSave>("blocks.json") {
        lw.edits = crate::edits::Edits::load(e, &lw.reg);
    }
    if let Ok(Some(v)) =
        d.read_json::<hearth_worldgen::vegetation::VegetationSave>("vegetation.json")
    {
        let planet = *lw.map.planet();
        lw.vegetation =
            hearth_worldgen::vegetation::Vegetation::new(&v, planet.circumference(), 0.0);
    }
    #[derive(serde::Deserialize)]
    struct Saved {
        player: hearth_player::Player,
    }
    let feet = match d.read_json::<Saved>("player.json") {
        Ok(Some(p)) => p.player.mover.pos,
        _ => anyhow::bail!("save={}: no player", d.root.display()),
    };
    let (calendar, _) = crate::server::calendar_for(lw, meta.settings.life.starting_season);
    let m = calendar.at(meta.clock.ticks);
    let planet = *lw.map.planet();
    spec.year_frac = Some(m.year_frac);
    spec.season = None;
    spec.hour = m.local_time(planet.solar_time_offset(feet.x)) * 24.0;
    let (sy, cy) = (spec.yaw as f64).to_radians().sin_cos();
    spec.x = Some(feet.x + sy * spec.back);
    spec.z = Some(feet.z - cy * spec.back);
    spec.y = Some(feet.y + spec.above);
    let items = hearth_items::Items::from_content(&lw.content);
    if let Ok(Some(lying)) = d.read_json::<hearth_items::WorldItems>("items.json") {
        spec.lying = lying
            .items
            .iter()
            .filter_map(|w| {
                let k = w.stack.kind(&items)?;
                let p = DVec3::from_array(w.pos);
                ((p - feet).length() < 64.0).then(|| {
                    let size = glam::Vec3::from(k.resting_m());
                    (p + DVec3::Y * (size.y as f64 / 2.0), size, k.color, w.yaw)
                })
            })
            .collect();
    }
    log::info!(
        "save {}: {} changes, {} things lying about, the player at {:.1}, {:.1}, {:.1}",
        d.root.display(),
        lw.edits.len(),
        spec.lying.len(),
        feet.x,
        feet.y,
        feet.z
    );
    Ok(())
}

/// Where a shot is taken: its x and z if given, else land near its latitude, else the heart of
/// its biome, else the spawn (columns offset by `centre`).
fn place_of(lw: &LocalWorld, spec: &ShotSpec, centre: f64) -> anyhow::Result<(f64, f64)> {
    if let (Some(x), Some(z)) = (spec.x, spec.z) {
        return Ok((x, z));
    }
    if let Some(lat) = spec.lat {
        return land_at_latitude(lw, lat)
            .ok_or_else(|| anyhow::anyhow!("no land near latitude {lat}"));
    }
    if let Some(name) = &spec.biome {
        let (name, below) = match name.split_once(':') {
            Some((n, t)) => (n, Some(t.parse::<f32>()?)),
            None => (name.as_str(), None),
        };
        let biome = hearth_worldgen::region::biome::Biome::from_name(name)
            .ok_or_else(|| anyhow::anyhow!("biome={name}: no such biome"))?;
        let (x, z) = lw
            .terrain()
            .find_biome(biome, 12_000.0, |s| below.is_none_or(|t| s.temperature < t))
            .ok_or_else(|| anyhow::anyhow!("biome={name}: none on this planet"))?;
        log::info!("biome {name}: at {x}, {z}");
        return Ok((x as f64 + centre, z as f64 + centre));
    }
    let (x, z) = lw.terrain().find_spawn(false);
    Ok((x as f64 + centre, z as f64 + centre))
}

/// A gentle land spot near a latitude (scans along the parallel).
fn land_at_latitude(lw: &LocalWorld, lat: f64) -> Option<(f64, f64)> {
    let planet = *lw.map.planet();
    let z = planet.z_for_latitude(lat.to_radians());
    let c = planet.circumference_f64();
    let steps = 4096;
    let mut best: Option<(f64, (f64, f64))> = None;
    for i in 0..steps {
        let x = c * i as f64 / steps as f64;
        let s = lw.terrain().sample(x as i32, z as i32);
        if s.is_underwater() || s.ocean || s.lake {
            continue;
        }
        // Prefer gentle, vegetated, moderately high ground.
        let score =
            -s.slope as f64 * 3.0 + s.tree_density as f64 + (s.height as f64 / 200.0).min(1.0);
        if best.is_none_or(|(b, _)| score > b) {
            best = Some((score, (x + 0.5, z + 0.5)));
        }
    }
    best.map(|(_, p)| p)
}

/// A rendered shot: colour (RGBA8 rows), depth (reverse-Z) and the view it was taken from.
pub struct Shot {
    pub pixels: Vec<u8>,
    pub depth: Vec<f32>,
    pub camera: Camera,
    /// Half-width of the full-detail area around the camera (blocks).
    pub near_radius: f64,
    /// LOD tiles drawn.
    pub lod_tiles: usize,
    pub terrain: hearth_render::terrain::TerrainStats,
    pub lod: hearth_render::lod::LodStats,
}

/// Loads the full-detail terrain around the shot's camera, builds its LOD terrain, and renders
/// it (three frames, so GPU culling settles; with `verify_cull`, checked against CPU culling).
pub fn render_shot(
    ctx: &GpuContext,
    atlas: &TextureArray,
    lod: &hearth_lod::LodGen,
    lw: &mut LocalWorld,
    spec: &ShotSpec,
    out: &Path,
    time: Option<&TimeConfig>,
) -> anyhow::Result<Shot> {
    let (mut sx, mut sz) = place_of(lw, spec, 0.5)?;
    let mut yaw_override = None;
    if spec.near_water {
        let (x, z, yaw) = bank_near(lw, sx, sz)
            .ok_or_else(|| anyhow::anyhow!("near=water: no deep water within 3 km"))?;
        (sx, sz) = (x, z);
        if !spec.yaw_given {
            yaw_override = Some(yaw);
        }
    }
    // Date and time: season (mid-season in this hemisphere) or year fraction; local hour.
    let planet = *lw.map.planet();
    let southern = planet.latitude(sz) < 0.0;
    let year_frac = spec.year_frac.unwrap_or_else(|| {
        spec.season.map_or(0.3, |s| {
            (Calendar::season_start(s, southern) + 0.25 * spec.season_at).rem_euclid(1.0)
        })
    });
    // The populations about the place; the camera to a group sought.
    let mut seen = None;
    let mut herd = None;
    let mut sought = None;
    let mut fauna = None;
    // A hominin group sought: where it is.
    let mut hominins_at: Option<DVec3> = None;
    'seek: {
        if spec.fauna {
            let made = std::time::Instant::now();
            let mut f = crate::fauna::Fauna::new(lw, spec.seed, 0.0, year_frac, None, false);
            f.ensure_about(lw, DVec3::new(sx, 0.0, sz));
            log::info!(
                "{} regions of populations made in {:.2}s",
                f.eco.regions.len(),
                made.elapsed().as_secs_f64()
            );
            if let Some(seek) = &spec.seek {
                // `red_deer` or `red_deer:3` (the third nearest group).
                let (name, nth) = match seek.split_once(':') {
                    Some((name, n)) => (name, n.parse::<usize>()?.max(1)),
                    None => (seek.as_str(), 1),
                };
                let s = f
                    .eco
                    .catalog
                    .index(name)
                    .ok_or_else(|| anyhow::anyhow!("seek={name}: no such species"))?;
                sought = Some(s);
                let hominin = f.eco.catalog.species[s].hominin;
                let mut groups: Vec<_> = f
                    .census()
                    .into_iter()
                    .filter(|g| g.0 as usize == s)
                    .collect();
                let d = |p: glam::DVec2| (p.x - sx).hypot(p.y - sz);
                groups.sort_by(|a, b| d(a.1).total_cmp(&d(b.1)));
                let Some(&(_, at, n)) = groups.get(nth - 1) else {
                    anyhow::bail!("seek={seek}: {} groups about", groups.len());
                };
                log::info!(
                    "seeking {name}: group {nth} of {} ({n} head) {:.0} m off",
                    groups.len(),
                    d(at)
                );
                if hominin {
                    // Hominins are agents, drawn out once the blocks are in: the camera on the
                    // group's place for now.
                    let centre = DVec3::new(at.x, lw.surface_y(at.x, at.y) + 0.8, at.y);
                    let h = Herd::new(vec![centre]);
                    let (eye, yaw) = h.first_view(lw, spec.yaw, spec.above);
                    (sx, sz) = (eye.x, eye.z);
                    seen = Some((eye, yaw, h.mid));
                    hominins_at = Some(centre);
                    fauna = Some(f);
                    break 'seek;
                }
                // The group brought into the world (on the generated ground: the blocks are not
                // loaded yet) a few seconds into their day; the camera where it sees most of them.
                let ground = crate::fauna::MapGround {
                    map: &lw.map,
                    reg: &lw.reg,
                    lw,
                    cells: &f.cells,
                };
                let centre = DVec3::new(at.x, lw.surface_y(at.x, at.y), at.y);
                f.live.materialize(&mut f.eco, &ground, centre);
                for _ in 0..200 {
                    f.live.step(
                        &f.eco,
                        &ground,
                        None,
                        &hearth_fauna::live::Now::day((spec.hour / 24.0) as f32),
                        0.05,
                    );
                }
                let back = f.eco.catalog.species[s].shoulder_m as f64 * 0.8;
                let backs: Vec<DVec3> = f
                    .views()
                    .iter()
                    .filter(|v| {
                        v.species as usize == s && (v.pos.x - at.x).hypot(v.pos.z - at.y) < 80.0
                    })
                    .map(|v| v.pos + DVec3::Y * back)
                    .collect();
                if backs.is_empty() {
                    anyhow::bail!("seek={seek}: the group did not come into the world");
                }
                // A first view to load the blocks about; the best once they are.
                let h = Herd::new(backs);
                let (eye, yaw) = h.first_view(lw, spec.yaw, spec.above);
                (sx, sz) = (eye.x, eye.z);
                seen = Some((eye, yaw, h.mid));
                herd = Some(h);
            }
            fauna = Some(f);
        }
    }
    let (pos, yaw, pitch) = match seen {
        Some((eye, yaw, mid)) => {
            let pitch = if spec.aim {
                let off = (mid.x - eye.x).hypot(mid.z - eye.z);
                ((eye.y - mid.y) as f32).atan2(off as f32).to_degrees()
            } else {
                spec.pitch
            };
            (eye, yaw, pitch)
        }
        None => {
            let floor = spec.floor.map(|f| {
                lw.terrain()
                    .sample(sx.floor() as i32, sz.floor() as i32)
                    .height as f64
                    + f
            });
            let sy = spec
                .y
                .or(floor)
                .unwrap_or_else(|| lw.surface_y(sx, sz) + spec.above);
            (
                DVec3::new(sx, sy, sz),
                yaw_override.unwrap_or(spec.yaw),
                spec.pitch,
            )
        }
    };
    let mut camera = Camera {
        pos,
        yaw,
        pitch,
        fov_y: spec.fov,
        near: 0.05,
        jitter: glam::Vec2::ZERO,
    };
    let mut calendar = time.map_or_else(|| Calendar::new(48, 8, 23.44), Calendar::from_config);
    calendar.year_offset = year_frac;
    calendar.day_offset = (spec.hour / 24.0 - planet.solar_time_offset(sx)).rem_euclid(1.0);
    let moment = calendar.at(0);
    log::info!(
        "shot {} at {:.1}, {:.1}, {:.1} (lat {:.1}°, year {:.3}, {:.1} h)",
        out.display(),
        camera.pos.x,
        camera.pos.y,
        camera.pos.z,
        planet.latitude_deg(sz),
        year_frac,
        spec.hour
    );
    // Cleared and burned ground in front of the camera, as it is years later.
    if !spec.disturb.is_empty() {
        let years = spec.disturb.iter().map(|d| d.2).fold(0.0, f64::max);
        let f = camera.forward().as_dvec3();
        let flat = DVec3::new(f.x, 0.0, f.z).normalize_or(DVec3::Z);
        let disturbances = spec
            .disturb
            .iter()
            .map(|&(kind, radius, since, ahead)| {
                let at = camera.pos + flat * ahead;
                hearth_worldgen::vegetation::Disturbance {
                    kind,
                    year: years - since,
                    x: at.x.floor() as i32,
                    z: at.z.floor() as i32,
                    radius: radius as f32,
                    severity: 1.0,
                    patches: Vec::new(),
                }
            })
            .collect();
        lw.vegetation = hearth_worldgen::vegetation::Vegetation::new(
            &hearth_worldgen::vegetation::VegetationSave { disturbances },
            planet.circumference(),
            years,
        );
    }
    let positions = lw.load_area(camera.pos, spec.distance, 2, spec.snow.then_some(year_frac));
    if let Some(h) = &herd {
        let (eye, yaw, n) = h.best_view(lw, spec.yaw, spec.above);
        log::info!(
            "  seen from {:.1}, {:.1}, {:.1} looking along {yaw:.0}°: {n} of {} in sight",
            eye.x,
            eye.y,
            eye.z,
            h.backs.len()
        );
        camera.pos = eye;
        camera.yaw = yaw;
        if spec.aim {
            let off = (h.mid.x - eye.x).hypot(h.mid.z - eye.z);
            camera.pitch = ((eye.y - h.mid.y) as f32).atan2(off as f32).to_degrees();
        }
    }
    // The plants cut about a point ahead.
    if let Some((radius, ahead)) = spec.mow {
        let f = camera.forward().as_dvec3();
        let flat = DVec3::new(f.x, 0.0, f.z).normalize_or(DVec3::Z);
        let at = camera.pos + flat * ahead;
        let reg = lw.reg.clone();
        let air = hearth_world::BlockStateId::AIR;
        let r = radius.ceil() as i32;
        let mut cut = 0;
        for dz in -r..=r {
            for dx in -r..=r {
                if (dx * dx + dz * dz) as f64 > radius * radius {
                    continue;
                }
                let (x, z) = (at.x.floor() as i32 + dx, at.z.floor() as i32 + dz);
                let top = lw.surface_y(x as f64 + 0.5, z as f64 + 0.5).floor() as i32;
                for y in top - 2..top + 4 {
                    let p = hearth_math::BlockPos::new(x, y, z);
                    if let Some(s) = lw.map.block(p)
                        && !s.is_air()
                        && reg.block_of(s).def.replaceable
                        && reg.collision_shape(s).is_empty()
                        && reg.fluid_amount(s) == 0
                    {
                        lw.map.set_block(p, air, &reg);
                        lw.light.block_changed(&mut lw.map, &reg, p);
                        cut += 1;
                    }
                }
            }
        }
        log::info!("  mowed {cut} plants");
    }
    // Things set on the ground in front of the camera (or about it by the compass), lit as they
    // would be.
    let f = camera.forward().as_dvec3();
    let flat = DVec3::new(f.x, 0.0, f.z).normalize_or(DVec3::Z);
    let side = DVec3::new(-flat.z, 0.0, flat.x);
    let placed = spec.place.iter().map(|(state, ahead, right, up)| {
        (
            state,
            camera.pos.x + flat.x * ahead + side.x * right,
            camera.pos.z + flat.z * ahead + side.z * right,
            up,
        )
    });
    let put = spec
        .put
        .iter()
        .map(|(state, east, south, up)| (state, camera.pos.x + east, camera.pos.z + south, up));
    for (state, px, pz, up) in placed.chain(put) {
        let s = lw
            .reg
            .parse_state(state)
            .map_err(|e| anyhow::anyhow!("place={state}: {e}"))?;
        let pos = hearth_math::BlockPos::containing(DVec3::new(
            px,
            lw.surface_y(px, pz) + 0.5 + *up as f64,
            pz,
        ));
        let reg = lw.reg.clone();
        lw.map.set_block(pos, s, &reg);
        lw.light.block_changed(&mut lw.map, &reg, pos);
    }
    // Trees grown in front of the camera, their feet on the ground.
    if !spec.trees.is_empty() {
        let forest = hearth_worldgen::trees::Forest::new(&lw.reg, &lw.content);
        for (species, stage, variant, ahead, right) in &spec.trees {
            let i = forest
                .templates
                .index_of(species)
                .ok_or_else(|| anyhow::anyhow!("tree={species}: no such tree species"))?;
            let stage = hearth_flora::Stage::from_name(stage)
                .ok_or_else(|| anyhow::anyhow!("tree: unknown stage {stage}"))?;
            let t = forest.templates.get(i, stage, *variant);
            let f = camera.forward().as_dvec3();
            let flat = DVec3::new(f.x, 0.0, f.z).normalize_or(DVec3::Z);
            let side = DVec3::new(-flat.z, 0.0, flat.x);
            let (px, pz) = (
                camera.pos.x + flat.x * ahead + side.x * right,
                camera.pos.z + flat.z * ahead + side.z * right,
            );
            let foot =
                hearth_math::BlockPos::containing(DVec3::new(px, lw.surface_y(px, pz) + 0.5, pz));
            let reg = lw.reg.clone();
            for (c, part) in &t.blocks {
                let p = hearth_math::BlockPos::new(
                    foot.x + c[0] as i32,
                    foot.y + c[1] as i32,
                    foot.z + c[2] as i32,
                );
                // Over air and plants only (roots go into the ground under the foot).
                let open = lw
                    .map
                    .block(p)
                    .is_none_or(|s| s.is_air() || reg.block_of(s).def.replaceable || c[1] < 0);
                if open {
                    lw.map.set_block(p, forest.blocks[i].state(*part), &reg);
                    lw.light.block_changed(&mut lw.map, &reg, p);
                }
            }
            log::info!(
                "  tree {species} {stage:?} {variant}: {:.1} m tall, {:.2} m through, {} blocks",
                t.height_m,
                t.diameter_m,
                t.blocks.len()
            );
        }
    }
    // A fire set in the grass ahead and burned on.
    let mut plumes = Vec::new();
    if let Some((minutes, ahead)) = spec.fire {
        plumes = burn_ahead(lw, &camera, minutes, ahead, spec.seed);
    }
    for ahead in &spec.far_smoke {
        let f = camera.forward().as_dvec3();
        let flat = DVec3::new(f.x, 0.0, f.z).normalize_or(DVec3::Z);
        let at = camera.pos + flat * *ahead;
        let ground = lw
            .terrain()
            .sample(planet.wrap_x(at.x.floor() as i32), at.z.floor() as i32)
            .height as f64;
        plumes.push(hearth_render::smoke::SmokePlume {
            at: DVec3::new(at.x, ground, at.z),
            strength: 0.9,
            far: true,
        });
    }
    let models = BlockModels::build(&lw.reg, atlas);
    let meshes = lw.mesh(&models, &positions, MeshOptions::default());
    let mut scene = SceneRenderer::new(ctx, atlas, OFFSCREEN_FORMAT, planet, 4, 4);
    scene.smoke.set_plumes(plumes);
    scene.terrain.render_distance = spec.distance;
    scene.terrain.vertical_distance = 64;
    for m in &meshes {
        scene.terrain.upload(ctx, m);
    }
    let (bx, bz) = (camera.pos.x.floor() as i32, camera.pos.z.floor() as i32);
    scene.set_sky_heights(ctx, &SkyHeights::build(bx, bz, |x, z| lw.map.sky_top(x, z)));
    scene.terrain.water.set_heights(
        ctx,
        crate::water_env::water_heights(&lw.map, &lw.reg, bx, bz),
    );
    let sampler = EnvSampler::new(lw.grid(), calendar);
    // Firelight where the camera is: the eye adapts to it as to daylight.
    let glow = lw
        .map
        .block_light(hearth_math::BlockPos::containing(camera.pos)) as f32
        / 15.0;
    let (mut env, weather) = sampler.sample(
        &moment,
        camera.pos,
        glow,
        EnvOverrides {
            cloud_cover: spec.clouds,
            precipitation: spec.precipitation,
        },
    );
    env.aerial_perspective = spec.fog;
    log::info!(
        "  sun elevation {:.1}°, {:.0} lux, clouds {:.0}%, {:.1} °C, {:?}",
        env.sun_dir.y.asin().to_degrees(),
        env.horizontal_lux(),
        weather.cloud_cover * 100.0,
        weather.temperature_c,
        weather.precip
    );
    let n = crate::season_cover::column_normals(
        &lw.generator,
        hearth_math::BlockPos::containing(camera.pos).column(),
    );
    let cover = hearth_env::climate::SeasonalCover::compute(&n);
    log::info!(
        "  climate: mean {:.1} °C (months {:.1}..{:.1}), {:.0} mm/yr, dry season {:?}, snow {:.2} m now, {:.2} m at the peak",
        n.t_mean,
        n.t_mean - n.t_range / 2.0,
        n.t_mean + n.t_range / 2.0,
        n.precip,
        hearth_env::tint::DryType::of(&n, false),
        cover.snow_depth_m(year_frac),
        cover.snow_depth_m(cover.peak_snow()),
    );
    // Distant terrain: every LOD tile out to the LOD distance, built before the shot (the queue
    // must be empty), beyond the full-detail cubes meshed above.
    let c = hearth_math::CubePos::containing(camera.pos);
    let r = spec.distance;
    let near = [
        ((c.x - r + 1) * 16) as f64,
        ((c.z - r + 1) * 16) as f64,
        ((c.x + r) * 16) as f64,
        ((c.z + r) * 16) as f64,
    ];
    let t_lod = Instant::now();
    let v = lw.terrain().vertical_scale() as f64;
    let reach = hearth_lod::draw_distance(spec.lod, camera.pos.y, v);
    // Refined by screen-space error as the game does (`LodStream`).
    let ppr = hearth_lod::px_per_rad(spec.height, spec.fov);
    let unsplit = rustc_hash::FxHashSet::default();
    let select = |errors: &crate::lod_stream::Errors| {
        if spec.lod == 0 {
            return Vec::new();
        }
        let built = |k: hearth_lod::TileKey| errors.get(&k).copied();
        let refine = hearth_lod::Refine {
            px_per_rad: ppr,
            max_error_px: hearth_core::options::VideoOptions::default().lod_error_px(),
            camera_y: camera.pos.y,
            built: &built,
            split_before: &unsplit,
        };
        hearth_lod::select_refined(
            &planet,
            camera.pos.x,
            camera.pos.z,
            reach,
            Some(near),
            Some(&refine),
        )
    };
    let late = AtomicBool::new(false);
    let mut quads = 0u64;
    let world = hearth_lod::LodWorld {
        veg: lw.vegetation.clone(),
        edits: Default::default(),
    };
    let errors = crate::lod_stream::build_refined(
        1 + hearth_lod::MAX_EXTRA_LEVELS,
        select,
        |keys| {
            let tiles: Vec<hearth_lod::TileMesh> = keys
                .par_iter()
                .filter_map(|k| {
                    if t_lod.elapsed().as_secs_f64() > spec.lod_timeout {
                        late.store(true, Ordering::Relaxed);
                        return None;
                    }
                    Some(lod.build_in(&lw.generator, &world, *k))
                })
                .collect();
            if late.load(Ordering::Relaxed) {
                anyhow::bail!(
                    "LOD terrain was not ready after {:.0} s ({} of {} tiles built)",
                    spec.lod_timeout,
                    tiles.len(),
                    keys.len()
                );
            }
            Ok(tiles)
        },
        |t| {
            crate::lod_stream::upload(ctx, &mut scene.lod, t);
            quads += t.quads() as u64;
        },
    )?;
    let keys = select(&errors);
    scene.lod_show = hearth_lod::cover(&keys, |k| errors.contains_key(&k))
        .iter()
        .map(|k| k.id())
        .collect();
    scene.near_area = (spec.lod > 0).then_some(near);
    scene.vertical_scale = lw.terrain().vertical_scale();
    log::info!(
        "  LOD: {} tiles ({} built), {} quads to {:.0} blocks in {:.2}s",
        keys.len(),
        errors.len(),
        quads,
        reach,
        t_lod.elapsed().as_secs_f64()
    );
    if spec.body {
        // The camera in the eyes of someone standing here, looking as the shot looks.
        let feet = DVec3::new(sx, lw.surface_y(sx, sz) + 1.0, sz);
        let figure = hearth_character::Figure::starting(hearth_character::Appearance::default());
        let pose = figure.animator.pose(
            &figure.rig,
            hearth_character::Activity::Stand,
            &hearth_character::Drive {
                look_pitch: spec.pitch,
                ..Default::default()
            },
        );
        let turn = glam::Quat::from_rotation_y(-spec.yaw.to_radians());
        camera.pos = feet + (turn * figure.eye(&pose)).as_dvec3();
        let chest = hearth_math::BlockPos::containing(feet + DVec3::Y * 1.2);
        let mut boxes = Vec::new();
        hearth_character::instances(
            &figure.rig,
            &figure.palette,
            &pose,
            glam::Affine3A::from_rotation_translation(turn, (feet - camera.pos).as_vec3()),
            hearth_character::Show {
                hide_head: true,
                sky_light: lw.map.sky_light(chest),
                block_light: lw.map.block_light(chest),
            },
            &mut boxes,
        );
        scene.figures.set(ctx, &boxes);
    }
    if let Some(d) = spec.person {
        // Someone standing on the ground in front of the camera, facing it.
        let f = camera.forward().as_dvec3();
        let flat = DVec3::new(f.x, 0.0, f.z).normalize_or(DVec3::Z);
        let (px, pz) = (camera.pos.x + flat.x * d, camera.pos.z + flat.z * d);
        let feet = DVec3::new(px, lw.surface_y(px, pz), pz);
        let figure = hearth_character::Figure::starting(hearth_character::Appearance::default());
        let pose = figure.animator.pose(
            &figure.rig,
            hearth_character::Activity::Stand,
            &hearth_character::Drive::default(),
        );
        let chest = hearth_math::BlockPos::containing(feet + DVec3::Y * 1.2);
        let place = glam::Affine3A::from_rotation_translation(
            glam::Quat::from_rotation_y(-(spec.yaw + 180.0).to_radians()),
            (feet - camera.pos).as_vec3(),
        );
        let mut boxes = Vec::new();
        hearth_character::instances(
            &figure.rig,
            &figure.palette,
            &pose,
            place,
            hearth_character::Show {
                hide_head: false,
                sky_light: lw.map.sky_light(chest),
                block_light: lw.map.block_light(chest),
            },
            &mut boxes,
        );
        scene.figures.set(ctx, &boxes);
    }
    // The hominins: those placed and those of a group sought.
    let mut hominin_views = placed_hominins(spec, lw, &camera);
    // The animals: their bodies and coats, those placed and those of the populations.
    let mut drawn = Vec::new();
    let mut bodies = None;
    if !spec.animals.is_empty() || fauna.is_some() {
        let catalog = hearth_fauna::species::Catalog::new(&lw.content);
        let made = std::time::Instant::now();
        let b = hearth_fauna::skin::Bodies::new(&catalog);
        log::info!(
            "bodies and coats ({}x{}) in {:.0} ms",
            b.atlas.w,
            b.atlas.h,
            made.elapsed().as_secs_f64() * 1000.0
        );
        scene
            .figures
            .set_coats(ctx, b.atlas.w, b.atlas.h, &b.atlas.px);
        drawn = placed_animals(spec, lw, &catalog, &b, &camera, year_frac as f32)?;
        bodies = Some((catalog, b));
    }
    if let Some(f) = &mut fauna {
        let ground = crate::fauna::MapGround {
            map: &lw.map,
            reg: &lw.reg,
            lw,
            cells: &f.cells,
        };
        // Those already in the world (a group sought) onto the blocks now loaded; the rest
        // about the camera, a few seconds into their day.
        f.live.settle(&ground);
        f.live.materialize(&mut f.eco, &ground, camera.pos);
        if spec.seek.is_none() {
            for _ in 0..200 {
                f.live.step(
                    &f.eco,
                    &ground,
                    None,
                    &hearth_fauna::live::Now::day((spec.hour / 24.0) as f32),
                    0.05,
                );
            }
        }
        if let Some(seconds) = spec.run {
            // On with the camera among them, timed (the ways they look for are most of it).
            let steps = (seconds / 0.05).round().max(1.0) as usize;
            let started = std::time::Instant::now();
            for _ in 0..steps {
                f.live.step(
                    &f.eco,
                    &ground,
                    Some(&hearth_fauna::mind::Presence::walking(
                        camera.pos - DVec3::Y * spec.above.min(1.6),
                    )),
                    &hearth_fauna::live::Now::day((spec.hour / 24.0) as f32),
                    0.05,
                );
            }
            let ms = started.elapsed().as_secs_f64() * 1000.0;
            log::info!(
                "  {} animals lived {seconds} s with the camera among them: {:.3} ms a step",
                f.live.animals.len(),
                ms / steps as f64,
            );
            // The camera after the group sought, where it has gone.
            if let Some(s) = sought {
                let near: Vec<DVec3> = f
                    .live
                    .animals
                    .iter()
                    .filter(|a| a.species as usize == s && !a.dead)
                    .map(|a| a.pos)
                    .filter(|p| (p.x - camera.pos.x).hypot(p.z - camera.pos.z) < 200.0)
                    .collect();
                if !near.is_empty() {
                    let mid = near.iter().copied().sum::<DVec3>() / near.len() as f64;
                    let d = mid - camera.pos;
                    camera.yaw = (-d.x as f32).atan2(d.z as f32).to_degrees();
                    if spec.aim {
                        let flat = d.x.hypot(d.z);
                        camera.pitch = ((-d.y + 0.8) as f32).atan2(flat as f32).to_degrees();
                    }
                }
            }
        }
        if let Some(centre) = hominins_at {
            // The group's agents drawn out as the game draws them, living their day a while.
            let (views, eye) = hominins_living(spec, lw, f, centre, &camera, year_frac as f32);
            if let Some((eye, yaw, pitch)) = eye {
                camera.pos = eye;
                camera.yaw = yaw;
                if spec.aim {
                    camera.pitch = pitch;
                }
            }
            hominin_views = views;
        }
        let views = f.views();
        log::info!("{} animals about the camera", views.len());
        for v in &views {
            let d = v.pos - camera.pos;
            log::debug!(
                "  {} {:?} {:?}: {:.0} m east, {:.0} m south, {:+.1} m up",
                f.eco.catalog.species[v.species as usize].name,
                v.stage,
                v.act,
                d.x,
                d.z,
                d.y
            );
        }
        drawn.extend(drawn_views(&views));
    }
    let mut boxes = Vec::new();
    for v in &hominin_views {
        let figure = crate::people::figure(v);
        let drive = crate::people::drive(v);
        let pose = figure.animator.pose(&figure.rig, drive.activity, &drive);
        let chest = hearth_math::BlockPos::containing(v.pos + DVec3::Y * 0.8);
        hearth_character::instances(
            &figure.rig,
            &figure.palette,
            &pose,
            glam::Affine3A::from_rotation_translation(
                glam::Quat::from_rotation_y(v.yaw),
                (v.pos - camera.pos).as_vec3(),
            ),
            hearth_character::Show {
                hide_head: false,
                sky_light: lw.map.sky_light(chest),
                block_light: lw.map.block_light(chest),
            },
            &mut boxes,
        );
    }
    if let Some((catalog, b)) = &bodies
        && (!drawn.is_empty() || !spec.trails.is_empty())
    {
        boxes.extend(animal_instances(
            &drawn,
            catalog,
            b,
            lw,
            &camera,
            year_frac as f32,
        ));
        let signs = trail_signs(spec, lw, catalog, &camera)?;
        let light = |p: DVec3| {
            let b = hearth_math::BlockPos::containing(p + DVec3::Y * 0.3);
            (lw.map.sky_light(b), lw.map.block_light(b))
        };
        boxes.extend(crate::signs::instances(
            &signs, 0.0, 1200.0, catalog, camera.pos, &light,
        ));
    }
    if spec.stress {
        // The pieces within 24 m of the camera, reckoned and outlined by their stress.
        let mut s = crate::structure::Structures::new(&lw.reg, &lw.content);
        let c = hearth_math::BlockPos::containing(camera.pos);
        let mut pieces = Vec::new();
        for dy in -24..=24 {
            for dz in -24..=24 {
                for dx in -24..=24 {
                    let p = hearth_math::BlockPos::new(c.x + dx, c.y + dy, c.z + dz);
                    if lw.map.block(p).is_some_and(|b| s.is_piece(b)) {
                        pieces.push(p);
                    }
                }
            }
        }
        s.changed(&pieces);
        let fell = s.tick(&lw.map, &lw.reg);
        log::info!(
            "  builder's view: {} pieces, {} would fall",
            s.stress.len(),
            fell.pieces.len()
        );
        for (p, stress) in &s.stress {
            if let Some(state) = lw.map.block(*p) {
                boxes.extend(crate::building::outline(
                    &lw.reg,
                    state,
                    *p,
                    crate::building::stress_color(*stress),
                    camera.pos,
                ));
            }
        }
    }
    // Things lying about (from a save).
    for (c, size, color, yaw) in &spec.lying {
        let b = hearth_math::BlockPos::containing(*c + DVec3::Y * 0.3);
        let place = glam::Affine3A::from_scale_rotation_translation(
            *size,
            glam::Quat::from_rotation_y(*yaw),
            (*c - camera.pos).as_vec3(),
        );
        boxes.push(hearth_character::solid(
            place,
            *color,
            (lw.map.sky_light(b), lw.map.block_light(b)),
        ));
    }
    if !boxes.is_empty() {
        scene.figures.set(ctx, &boxes);
    }
    if let Some(name) = &spec.senses {
        use hearth_render::post::Senses;
        scene.senses = match name.as_str() {
            "hurt" => Senses {
                vignette: 0.75,
                red: 0.6,
                desaturate: 0.3,
                ..Senses::default()
            },
            "cold" => Senses {
                cold: 0.6,
                desaturate: 0.1,
                ..Senses::default()
            },
            "hot" => Senses {
                heat: 0.8,
                time: 1.3,
                ..Senses::default()
            },
            "exhausted" => Senses {
                desaturate: 0.6,
                ..Senses::default()
            },
            "faint" => Senses {
                desaturate: 0.7,
                dim: 0.5,
                vignette: 0.5,
                ..Senses::default()
            },
            other => anyhow::bail!("unknown senses {other:?}"),
        };
    }
    let target = OffscreenTarget::new(ctx, spec.width, spec.height);
    let size = (spec.width, spec.height);
    let frame = |scene: &mut SceneRenderer| {
        scene.prepare(ctx, &camera, size, &env, f32::INFINITY);
        let mut enc = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("shot"),
            });
        scene.render(ctx, &mut enc, &target.color_view, &target.depth.view, size);
        ctx.queue.submit(Some(enc.finish()));
        target.read_rgba(ctx)
    };
    // Three frames: with GPU culling the first draws everything in phase 1, the second learns
    // which cubes are occluded, and the third draws only the survivors in phase 0. With TAA,
    // sixteen more for the jittered frames to settle into one.
    scene.set_taa(ctx, spec.taa);
    for _ in 0..if spec.taa { 18 } else { 2 } {
        frame(&mut scene);
    }
    let pixels = frame(&mut scene);
    if spec.verify_cull && scene.terrain.uses_gpu_culling() {
        let counts = scene.terrain.read_gpu_draw_counts(ctx).unwrap_or_default();
        scene.terrain.gpu_culling = false;
        let reference = frame(&mut scene);
        let cpu_draws = scene.terrain.stats.draws;
        scene.terrain.gpu_culling = true;
        let differing = pixels
            .as_chunks::<4>()
            .0
            .iter()
            .zip(reference.as_chunks::<4>().0)
            .filter(|(a, b)| a != b)
            .count();
        log::info!(
            "cull check: GPU draws phase 0 {:?}, phase 1 {:?}; CPU draws {}; {} of {} pixels differ",
            &counts[0..4],
            &counts[4..8],
            cpu_draws,
            differing,
            pixels.len() / 4
        );
        if differing > 0 {
            let cpu_path = out.with_extension("cpu.png");
            let gpu_path = out.with_extension("gpu.png");
            write_png(&cpu_path, spec.width, spec.height, &reference)?;
            write_png(&gpu_path, spec.width, spec.height, &pixels)?;
            anyhow::bail!(
                "GPU culling changed {differing} pixels (see {} and {})",
                cpu_path.display(),
                gpu_path.display()
            );
        }
    }
    let depth = target.read_depth(ctx);
    Ok(Shot {
        pixels,
        depth,
        camera,
        near_radius: (spec.distance * 16) as f64,
        lod_tiles: scene.lod.stats.drawn,
        terrain: scene.terrain.stats,
        lod: scene.lod.stats,
    })
}

/// The least distance from a group sought the camera stands (m).
const SEEK_M: f64 = 20.0;

/// A group sought (the backs of its animals), and how far off to stand to have most of it in a
/// 40° lens (at least SEEK_M).
struct Herd {
    backs: Vec<DVec3>,
    mid: DVec3,
    off: f64,
}

impl Herd {
    fn new(backs: Vec<DVec3>) -> Self {
        let mid = backs.iter().copied().sum::<DVec3>() / backs.len().max(1) as f64;
        let mut far: Vec<f64> = backs
            .iter()
            .map(|p| (p.x - mid.x).hypot(p.z - mid.z))
            .collect();
        far.sort_by(f64::total_cmp);
        let wide = far.get(far.len() * 4 / 5).copied().unwrap_or(0.0);
        Self {
            backs,
            mid,
            off: SEEK_M.max(wide * 1.4 + 4.0),
        }
    }

    /// The sixteen ways to look at it, the first along `yaw`.
    fn yaws(yaw: f32) -> impl Iterator<Item = f32> {
        (0..16).map(move |k| yaw + k as f32 * 22.5)
    }

    /// Where to stand looking at it along `yaw`: `off` from its middle, as high above the
    /// ground as it takes to look down on it and to see three in four of its animals over the
    /// generated ground between (1.5 m over it for what grows there, narrowing to nothing at
    /// each animal).
    fn stand(&self, lw: &LocalWorld, yaw: f32, above: f64) -> DVec3 {
        // The generated ground as the blocks have it (their tops at the rounded height).
        let ground_at = |x: f64, z: f64| lw.surface_y(x, z).round();
        // The camera's forward on the ground is (-sin yaw, cos yaw).
        let (s, c) = (yaw as f64).to_radians().sin_cos();
        let (cx, cz) = (self.mid.x + s * self.off, self.mid.z - c * self.off);
        let ground = ground_at(cx, cz);
        let mut needs: Vec<f64> = self
            .backs
            .iter()
            .map(|p| {
                let mut need = above;
                for i in 1..24 {
                    let t = i as f64 / 24.0;
                    let h = ground_at(cx + (p.x - cx) * t, cz + (p.z - cz) * t) + 1.5 * (1.0 - t);
                    need = need.max((h - p.y * t) / (1.0 - t) - ground);
                }
                need
            })
            .collect();
        needs.sort_by(f64::total_cmp);
        let need = needs
            .get(needs.len() * 3 / 4)
            .copied()
            .unwrap_or(above)
            .max(self.mid.y + 2.0 - ground);
        DVec3::new(cx, ground + need, cz)
    }

    /// Before the blocks are loaded: of the sixteen ways, the one standing least high (the
    /// first unless another is much lower).
    fn first_view(&self, lw: &LocalWorld, yaw: f32, above: f64) -> (DVec3, f32) {
        let mut best = (self.stand(lw, yaw, above), yaw);
        for y in Self::yaws(yaw).skip(1) {
            let eye = self.stand(lw, y, above);
            if eye.y - lw.surface_y(eye.x, eye.z)
                < best.0.y - lw.surface_y(best.0.x, best.0.z) - 2.0
            {
                best = (eye, y);
            }
        }
        best
    }

    /// With the blocks loaded: of the sixteen ways, each as high as it asks and 3 and 6 m
    /// higher, the one seeing the most of the animals past the trees, bushes and the lie of the
    /// ground (the first of those seeing as many).
    fn best_view(&self, lw: &LocalWorld, yaw: f32, above: f64) -> (DVec3, f32, usize) {
        let mut best = (DVec3::ZERO, yaw, 0, false);
        for y in Self::yaws(yaw) {
            let eye = self.stand(lw, y, above);
            for lift in [0.0, 3.0, 6.0] {
                let e = eye + DVec3::Y * lift;
                let n = self
                    .backs
                    .iter()
                    .filter(|&&p| clear(lw, e, p + DVec3::Y * 0.2))
                    .count();
                if !best.3 || n > best.2 {
                    best = (e, y, n, true);
                }
            }
        }
        (best.0, best.1, best.2)
    }
}

/// Whether only air and water lie between two points (the last half metre aside: what the
/// thing seen stands in).
fn clear(lw: &LocalWorld, from: DVec3, to: DVec3) -> bool {
    let planet = lw.map.planet();
    let d = to - from;
    let n = (d.length() / 0.25).ceil().max(1.0) as usize;
    let near = 0.5 / d.length().max(0.5);
    (1..n).all(|i| {
        let t = i as f64 / n as f64;
        if t > 1.0 - near {
            return true;
        }
        let p = from + d * t;
        let at = hearth_math::BlockPos::new(
            planet.wrap_x(p.x.floor() as i32),
            p.y.floor() as i32,
            p.z.floor() as i32,
        );
        lw.map
            .block(at)
            .is_none_or(|s| s.is_air() || lw.reg.fluid_amount(s) > 0)
    })
}

/// The bank of the nearest water at least a metre and a half deep within three kilometres:
/// a dry place four metres back from its edge (where it is half a metre deep, toward where the
/// search began), and the yaw that looks out over the water from it.
fn bank_near(lw: &LocalWorld, x: f64, z: f64) -> Option<(f64, f64, f32)> {
    let t = lw.terrain();
    let depth = |x: f64, z: f64| {
        let s = t.sample(x.floor() as i32, z.floor() as i32);
        if s.water.is_finite() {
            (s.water - s.height) as f64
        } else {
            0.0
        }
    };
    for r in (0..3000).step_by(8) {
        let r = r as f64;
        let n = ((r * std::f64::consts::TAU / 8.0).ceil() as usize).max(1);
        for k in 0..n {
            let a = k as f64 / n as f64 * std::f64::consts::TAU;
            let (wx, wz) = (x + a.cos() * r, z + a.sin() * r);
            if depth(wx, wz) < 1.5 {
                continue;
            }
            // Back toward where the search began until it is shallow, then four metres more.
            let back = glam::DVec2::new(x - wx, z - wz).normalize_or(glam::DVec2::X);
            let mut d = 0.0;
            while d < 200.0 && depth(wx + back.x * d, wz + back.y * d) > 0.5 {
                d += 1.0;
            }
            if d >= 200.0 {
                continue;
            }
            let (bx, bz) = (wx + back.x * (d + 4.0), wz + back.y * (d + 4.0));
            // The camera's forward (-sin yaw, cos yaw) out over the water.
            let yaw = (back.x as f32).atan2(-back.y as f32).to_degrees();
            return Some((bx, bz, yaw));
        }
    }
    None
}

/// What an animal in a shot does, by name.
fn act_of(
    name: &str,
) -> anyhow::Result<(hearth_fauna::live::Act, Pace, hearth_fauna::live::Medium)> {
    use hearth_fauna::live::{Act, Medium};
    let (act, pace) = match name {
        "graze" | "eat" => (Act::Graze, Pace::Still),
        "walk" => (Act::Walk, Pace::Walk),
        "trot" => (Act::Walk, Pace::Trot),
        "run" | "flee" | "gallop" => (Act::Flee, Pace::Run),
        "rest" | "lie" => (Act::Rest, Pace::Still),
        "sleep" => (Act::Sleep, Pace::Still),
        "alert" => (Act::Alert, Pace::Still),
        "groom" => (Act::Groom, Pace::Still),
        "drink" => (Act::Drink, Pace::Still),
        "rear" => (Act::Rear, Pace::Still),
        "attack" => (Act::Attack, Pace::Still),
        "fly" => (Act::Fly, Pace::Run),
        "swim" => (Act::Walk, Pace::Walk),
        "climb" => (Act::Flee, Pace::Walk),
        "perch" => (Act::Alert, Pace::Still),
        "dead" => (Act::Dead, Pace::Still),
        other => anyhow::bail!("an animal cannot {other:?}"),
    };
    let medium = match name {
        "fly" => Medium::Air,
        "swim" => Medium::Water,
        "climb" | "perch" => Medium::Tree,
        _ => Medium::Ground,
    };
    Ok((act, pace, medium))
}

/// An animal to draw in a shot: what it is and does, where, facing, how fast, and its
/// stride's phase.
struct Drawn {
    species: usize,
    stage: hearth_fauna::live::Stage,
    female: bool,
    act: hearth_fauna::live::Act,
    pos: DVec3,
    yaw: f32,
    speed: f32,
    phase: f32,
    medium: hearth_fauna::live::Medium,
}

/// The boxes of animals in a shot, posed on the ground under their feet in their coats,
/// camera-relative.
fn animal_instances(
    drawn: &[Drawn],
    catalog: &hearth_fauna::species::Catalog,
    bodies: &hearth_fauna::skin::Bodies,
    lw: &LocalWorld,
    camera: &hearth_render::camera::Camera,
    year_frac: f32,
) -> Vec<hearth_character::FigureInstance> {
    use hearth_fauna::anim::{Drive, Motion, pose, scale_of};
    let mut out = Vec::new();
    for a in drawn {
        let Some(sp) = catalog.species.get(a.species) else {
            continue;
        };
        let rig = bodies.rig(a.species, a.female);
        let southern = lw.map.planet().latitude(a.pos.z) < 0.0;
        let scale = scale_of(
            rig,
            a.stage,
            year_frac,
            sp.life.birth_frac,
            sp.life.birth_mass_kg,
        );
        // Alarmed, it watches the camera.
        let look = matches!(
            a.act,
            hearth_fauna::live::Act::Alert | hearth_fauna::live::Act::Attack
        )
        .then(|| glam::Quat::from_rotation_y(-a.yaw) * (camera.pos - a.pos).as_vec3() / scale);
        let drive = Drive {
            act: a.act,
            speed: a.speed,
            look,
            stage: a.stage,
            female: a.female,
            year_frac,
            southern,
            scale,
            medium: a.medium,
        };
        let mut m = Motion::new(a.species as u64 * 7919 + (a.pos.x.to_bits() >> 20));
        m.settle(rig, &drive);
        m.phase = a.phase;
        m.time = a.phase * 13.0;
        let ground = crate::fauna::CubeFooting {
            map: &lw.map,
            reg: &lw.reg,
            at: a.pos,
            yaw: a.yaw,
        };
        let p = pose(rig, &m, &drive, &ground);
        let coat = bodies.coat_of(
            a.species,
            a.female,
            a.stage,
            hearth_fauna::skin::winter_coat(year_frac, southern),
        );
        let place = glam::Affine3A::from_rotation_translation(
            glam::Quat::from_rotation_y(a.yaw),
            (a.pos - camera.pos).as_vec3(),
        );
        let chest =
            hearth_math::BlockPos::containing(a.pos + DVec3::Y * (rig.torso_y * scale) as f64);
        let light = (lw.map.sky_light(chest), lw.map.block_light(chest));
        for (b, skin) in bodies.skinned(a.species, rig, &p, coat) {
            out.push(hearth_character::skinned(place * b, skin, light));
        }
    }
    out
}

/// The animals of the world as a shot draws them.
fn drawn_views(views: &[hearth_fauna::live::AnimalView]) -> Vec<Drawn> {
    views
        .iter()
        .map(|v| Drawn {
            species: v.species as usize,
            stage: v.stage,
            female: v.female,
            act: v.act,
            pos: v.pos,
            yaw: v.yaw,
            speed: v.speed,
            phase: v.stride.rem_euclid(1.0),
            medium: v.medium,
        })
        .collect()
}

/// The animals placed in a shot, on the ground in front of the camera.
fn placed_animals(
    spec: &ShotSpec,
    lw: &LocalWorld,
    catalog: &hearth_fauna::species::Catalog,
    bodies: &hearth_fauna::skin::Bodies,
    camera: &hearth_render::camera::Camera,
    year_frac: f32,
) -> anyhow::Result<Vec<Drawn>> {
    let f = camera.forward().as_dvec3();
    let flat = DVec3::new(f.x, 0.0, f.z).normalize_or(DVec3::Z);
    let right = DVec3::new(-flat.z, 0.0, flat.x);
    let cam_yaw = flat.x.atan2(flat.z) as f32;
    let mut out = Vec::new();
    for a in &spec.animals {
        let species = catalog
            .index(&a.species)
            .ok_or_else(|| anyhow::anyhow!("animal={}: no such species", a.species))?;
        let sp = &catalog.species[species];
        let rig = bodies.rig(species, a.female);
        let p = camera.pos + flat * a.ahead + right * a.right;
        let ground = crate::fauna::surface_in(
            &lw.map,
            &lw.reg,
            p.x,
            p.z,
            lw.surface_y(p.x, p.z).floor() as i32 + 3,
            10,
        )
        .map_or_else(|| lw.surface_y(p.x, p.z), |g| g.y);
        let hip = rig.legs.iter().map(|l| l.joint.y).fold(0.1f32, f32::max);
        // A swimmer's back at the water's surface, where there is water.
        let ground = match a.medium {
            hearth_fauna::live::Medium::Water => crate::fauna::surface_in(
                &lw.map,
                &lw.reg,
                p.x,
                p.z,
                lw.surface_y(p.x, p.z).floor() as i32 + 3,
                10,
            )
            .filter(|f| f.water)
            .map_or(ground, |f| {
                if hearth_fauna::live::mover_of(sp) == hearth_fauna::live::Mover::Fish {
                    // A fish as deep as it keeps.
                    f.y + f.depth - hearth_fauna::live::swim_depth(sp, f.depth)
                } else {
                    // The back just out of the water, as big as it is.
                    let scale = hearth_fauna::anim::scale_of(
                        rig,
                        a.stage,
                        year_frac,
                        sp.life.birth_frac,
                        sp.life.birth_mass_kg,
                    );
                    f.level() - (sp.shoulder_m * scale) as f64 * 0.85
                }
            }),
            _ => ground,
        };
        let speed = if a.medium == hearth_fauna::live::Medium::Water {
            sp.swim_m_s.unwrap_or(0.8) * 0.7
        } else {
            0.0
        };
        let speed = match a.pace {
            _ if speed > 0.0 => speed,
            Pace::Still => 0.0,
            Pace::Walk => sp.walk_m_s,
            Pace::Trot => (1.2 * 9.81 * hip).sqrt(),
            Pace::Run => sp.run_m_s.min((6.0 * 9.81 * hip).sqrt()),
        };
        out.push(Drawn {
            species,
            stage: a.stage,
            female: a.female,
            act: a.act,
            pos: DVec3::new(p.x, ground + a.up, p.z),
            yaw: cam_yaw + a.yaw.to_radians(),
            speed,
            phase: ((a.ahead * 0.37 + a.right * 0.21).rem_euclid(1.0)) as f32,
            medium: a.medium,
        });
    }
    Ok(out)
}

/// The standing height (m) of one of a species profile's sex and age class (a year old, six,
/// grown), as `hearth_people::Species` reckons it.
fn stature(
    profile: &hearth_content::schema::humans::Species,
    female: bool,
    stage: hearth_fauna::live::Stage,
) -> f32 {
    use hearth_fauna::live::Stage;
    let range = profile.body.height_m.of(female);
    let maturity = profile.life.maturity_years.max(1.0);
    let age = match stage {
        Stage::Young => 0.5,
        Stage::Juvenile => (maturity * 0.5).min(6.0),
        Stage::Adult => maturity + 5.0,
    };
    let t = (age / maturity).min(1.0);
    let growth = 0.05 + 0.95 * t.powf(1.1);
    (range.0 + range.1) / 2.0 * growth.powf(0.4)
}

/// The hominins placed in a shot, on the ground in front of the camera.
fn placed_hominins(
    spec: &ShotSpec,
    lw: &LocalWorld,
    camera: &hearth_render::camera::Camera,
) -> Vec<hearth_people::PersonView> {
    use hearth_fauna::live::Medium;
    use hearth_people::Doing;
    if spec.hominins.is_empty() {
        return Vec::new();
    }
    let Some(profile) = lw.content.species.iter().find(|h| h.population.is_some()) else {
        return Vec::new();
    };
    let f = camera.forward().as_dvec3();
    let flat = DVec3::new(f.x, 0.0, f.z).normalize_or(DVec3::Z);
    let right = DVec3::new(-flat.z, 0.0, flat.x);
    let cam_yaw = flat.x.atan2(flat.z) as f32;
    spec.hominins
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let p = camera.pos + flat * h.ahead + right * h.right;
            let ground = crate::fauna::surface_in(
                &lw.map,
                &lw.reg,
                p.x,
                p.z,
                lw.surface_y(p.x, p.z).floor() as i32 + 3,
                10,
            )
            .map_or_else(|| lw.surface_y(p.x, p.z), |g| g.y);
            let (doing, speed, medium) = match h.act.as_str() {
                "walk" => (Doing::Idle, 1.1, Medium::Ground),
                "run" => (Doing::Idle, 4.0, Medium::Ground),
                "feed" => (Doing::Feeding, 0.0, Medium::Ground),
                "work" => (Doing::Working { recipe: 0 }, 0.0, Medium::Ground),
                "groom" => (Doing::Resting, 0.0, Medium::Ground),
                "climb" => (Doing::Idle, 0.5, Medium::Tree),
                "sleep" => (Doing::Sleeping, 0.0, Medium::Ground),
                _ => (Doing::Idle, 0.0, Medium::Ground),
            };
            hearth_people::PersonView {
                id: i as u64 + 1,
                plan: profile.body.plan,
                female: h.female,
                stage: h.stage,
                pos: DVec3::new(p.x, ground + h.up, p.z),
                yaw: cam_yaw + h.yaw.to_radians(),
                speed,
                medium,
                doing,
                height_m: stature(profile, h.female, h.stage),
            }
        })
        .collect()
}

/// What a hominin group sought comes to: its agents as drawn, and where to stand to see most
/// of them (the eye, its yaw and its pitch).
type Living = (Vec<hearth_people::PersonView>, Option<(DVec3, f32, f32)>);

/// The agents of a hominin group sought, drawn out about its place as the game draws them and
/// living a while of their day (`run` seconds, half a minute without) with no one about.
fn hominins_living(
    spec: &ShotSpec,
    lw: &mut LocalWorld,
    f: &mut crate::fauna::Fauna,
    centre: DVec3,
    camera: &hearth_render::camera::Camera,
    year_frac: f32,
) -> Living {
    let content = lw.content.clone();
    let items = hearth_items::Items::from_content(&content);
    let crafts = hearth_craft::Crafts::from_content(&content, &items);
    let graph = hearth_craft::Graph::from_content(&content);
    let body = hearth_body::BodyConfig::with_rates(
        &content,
        hearth_body::Rates::authentic(),
        hearth_content::TimeScales::defaults(&content.time),
    );
    let mut agents = crate::people::PeopleNear::new(&content, &graph, &body, spec.seed, None);
    let mut lying = hearth_items::WorldItems::default();
    let mut changed = Vec::new();
    let hour = spec.hour as f32;
    let day = (6.0..18.5).contains(&hour);
    let mut exposure = hearth_body::Exposure::mild();
    exposure.local_hour = hour;
    exposure.air_c = if day { 27.0 } else { 19.0 };
    let around = hearth_craft::Surroundings {
        daylight: day,
        air_c: exposure.air_c,
        humidity: 0.5,
        ..Default::default()
    };
    let seconds = spec.run.unwrap_or(30.0);
    let steps = ((seconds / 0.05).round() as u64).max(41);
    let started = std::time::Instant::now();
    for t in 0..steps {
        agents.tick(
            lw,
            f,
            &mut lying,
            &items,
            &crafts,
            &graph,
            &mut changed,
            exposure,
            around.clone(),
            year_frac,
            &[centre],
            &[],
            hearth_people::Now {
                tick: t,
                hour,
                day: t as f64 * 0.05 / 2880.0,
                year_days: 32.0,
            },
            0.05,
        );
    }
    let views = agents.views(centre);
    log::info!(
        "  {} hominins lived {seconds} s: {:.3} ms a step; {} things lying, {} nests",
        views.len(),
        started.elapsed().as_secs_f64() * 1000.0 / steps as f64,
        lying.items.len(),
        changed.len()
    );
    for v in &views {
        let d = v.pos - camera.pos;
        log::info!(
            "    {} {:?} {:?} {:?}: {:.0} m east, {:.0} m south, {:+.1} m up ({:.1} m over the              ground)",
            if v.female { "f" } else { "m" },
            v.stage,
            v.medium,
            v.doing,
            d.x,
            d.z,
            d.y,
            v.pos.y - lw.surface_y(v.pos.x, v.pos.z)
        );
    }
    if views.is_empty() {
        return (views, None);
    }
    let backs = views.iter().map(|v| v.pos + DVec3::Y * 0.8).collect();
    let h = Herd::new(backs);
    let (mut eye, yaw, n) = h.best_view(lw, spec.yaw, spec.above);
    // Nearer (or farther) along the way it looks, by `back`.
    let flat = DVec3::new(h.mid.x - eye.x, 0.0, h.mid.z - eye.z).normalize_or(DVec3::Z);
    eye -= flat * spec.back;
    let off = (h.mid.x - eye.x).hypot(h.mid.z - eye.z);
    let pitch = ((eye.y - h.mid.y) as f32).atan2(off as f32).to_degrees();
    log::info!("  {n} of {} in sight", views.len());
    (views, Some((eye, yaw, pitch)))
}

/// The signs a shot's trails lay on the ground: prints a stride apart either side of the way
/// (plain as snow where there is snow), blood a quarter metre apart, droppings in a line of
/// heaps.
fn trail_signs(
    spec: &ShotSpec,
    lw: &LocalWorld,
    catalog: &hearth_fauna::species::Catalog,
    camera: &hearth_render::camera::Camera,
) -> anyhow::Result<Vec<hearth_fauna::live::Sign>> {
    use hearth_fauna::live::{Sign, SignKind};
    let f = camera.forward().as_dvec3();
    let flat = DVec3::new(f.x, 0.0, f.z).normalize_or(DVec3::Z);
    let right = DVec3::new(-flat.z, 0.0, flat.x);
    let cam_yaw = flat.x.atan2(flat.z) as f32;
    let mut out = Vec::new();
    for (species, n, kind, ahead, r, yaw) in &spec.trails {
        let si = catalog
            .index(species)
            .ok_or_else(|| anyhow::anyhow!("trail={species}: no such species"))?;
        let sp = &catalog.species[si];
        let heading = cam_yaw + yaw.to_radians();
        let dir = DVec3::new(heading.sin() as f64, 0.0, heading.cos() as f64);
        let side = DVec3::new(dir.z, 0.0, -dir.x);
        let step = match kind {
            SignKind::Print => sp.track.map_or(0.8, |t| t.stride_m as f64).max(0.1),
            SignKind::Blood => 0.25,
            SignKind::Droppings => 1.5,
        };
        let start = camera.pos + flat * *ahead + right * *r;
        for k in 0..*n {
            let lr = if k % 2 == 0 { 1.0 } else { -1.0 };
            let at = start
                + dir * (k as f64 * step)
                + if *kind == SignKind::Print {
                    side * lr * (sp.shoulder_m as f64 * 0.12).clamp(0.03, 0.2)
                } else {
                    DVec3::ZERO
                };
            let top = lw.surface_y(at.x, at.z).floor() as i32 + 3;
            let ground = crate::fauna::surface_in(&lw.map, &lw.reg, at.x, at.z, top, 10);
            let y = ground.map_or_else(|| lw.surface_y(at.x, at.z), |g| g.y);
            let (plain, y) = crate::fauna::sign_surface(&lw.map, &lw.reg, at.x, at.z, y);
            out.push(Sign {
                kind: *kind,
                species: si as u16,
                pos: DVec3::new(at.x, y, at.z),
                yaw: heading,
                t: 0.0,
                plain: plain.max(0.7),
            });
        }
    }
    Ok(out)
}

fn shoot(
    ctx: &GpuContext,
    atlas: &TextureArray,
    lod: &hearth_lod::LodGen,
    lw: &mut LocalWorld,
    spec: &ShotSpec,
    out: &Path,
    time: Option<&TimeConfig>,
) -> anyhow::Result<()> {
    if let Some(zoom) = spec.globe {
        return shoot_globe(ctx, lw, spec, out, zoom);
    }
    let shot = render_shot(ctx, atlas, lod, lw, spec, out, time)?;
    write_png(out, spec.width, spec.height, &shot.pixels)?;
    let s = shot.terrain;
    log::info!(
        "wrote {} ({} visible cubes, {} draws, {} quads, {:.1} MiB mesh memory; {} LOD tiles drawn, {} quads, {:.1} MiB)",
        out.display(),
        s.visible_cubes,
        s.draws,
        s.quads_drawn,
        (s.packed_bytes + s.general_bytes) as f64 / (1 << 20) as f64,
        shot.lod.drawn,
        shot.lod.quads,
        shot.lod.bytes as f64 / (1 << 20) as f64
    );
    Ok(())
}

/// The planet as the globe, centred on the shot's place (marked).
fn shoot_globe(
    ctx: &GpuContext,
    lw: &LocalWorld,
    spec: &ShotSpec,
    out: &Path,
    zoom: f32,
) -> anyhow::Result<()> {
    let t0 = Instant::now();
    let (x, z) = place_of(lw, spec, 0.0)?;
    let map = crate::globe::planet_map(lw.terrain(), crate::globe::MAP_WIDTH);
    log::info!("globe map in {:.2}s", t0.elapsed().as_secs_f64());
    let mut globe = hearth_render::globe::GlobeRenderer::new(ctx, OFFSCREEN_FORMAT);
    let w = crate::globe::MAP_WIDTH as u32;
    globe.set_map(ctx, w, w / 2, &map);
    let (lat, lon) = crate::globe::lat_lon(lw.map.planet(), DVec3::new(x, 0.0, z));
    let view = hearth_render::globe::GlobeView { lat, lon, zoom };
    let target = OffscreenTarget::new(ctx, spec.width, spec.height);
    let size = (spec.width, spec.height);
    let mut enc = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("globe shot"),
        });
    globe.render(
        ctx,
        &mut enc,
        &target.color_view,
        size,
        &view,
        Some((lat, lon)),
        None,
    );
    ctx.queue.submit(Some(enc.finish()));
    write_png(out, spec.width, spec.height, &target.read_rgba(ctx))?;
    log::info!(
        "wrote {} (globe at {})",
        out.display(),
        crate::globe::describe(lw.terrain(), lat, lon)
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_specs_and_lists() {
        let s = ShotSpec::parse(
            "seed=7, x=10.5, z=-3, yaw=90, w=640, h=360, out=a.png, season=autumn, hour=16.5",
        )
        .unwrap();
        assert_eq!(s.seed, 7);
        assert_eq!(s.x, Some(10.5));
        assert_eq!(s.z, Some(-3.0));
        assert_eq!((s.width, s.height), (640, 360));
        assert_eq!(s.out, Some(PathBuf::from("a.png")));
        assert_eq!(s.season, Some(Season::Autumn));
        assert_eq!(s.hour, 16.5);
        assert!(ShotSpec::parse("seed=x").is_err());
        assert!(ShotSpec::parse("colour=red").is_err());
        assert!(ShotSpec::parse("season=monsoon").is_err());
        let list =
            ShotSpec::parse_list("# suite\ndefaults: seed=3, w=320\n\nyaw=0\nyaw=180 # back\n")
                .unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|s| s.seed == 3 && s.width == 320));
        assert_eq!(list[1].yaw, 180.0);
    }
}

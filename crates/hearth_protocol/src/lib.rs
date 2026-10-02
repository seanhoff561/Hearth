//! Messages between the game's client and its integrated server (v1 M4, v2 V2-3).
//!
//! The server owns the world: it generates, lights and meshes the terrain around the player,
//! keeps the clock, simulates the player's body in the weather and water of where they are, runs
//! the world's own changes and saves. The client renders, takes input and moves the player itself
//! (predicted against its own copy of the nearby blocks), telling the server where the player
//! went and what the movement did (landings, water, breath).
//!
//! In one process the server is a thread and these messages pass over channels carrying shared
//! data as it is (cubes, meshes, the generator). A network transport would serialise the cubes,
//! leave the meshing to the client and send the generator's seed instead of the generator.

use std::sync::Arc;

use glam::DVec3;
use hearth_body::{BodyConfig, Death, Exposure, InjuryState, Status};
use hearth_lod::LodGen;
use hearth_math::{CubePos, Planet};
use hearth_physics::{Ability, Motion, Mover};
use hearth_render::mesh::CubeMesh;
use hearth_render::precip::SkyHeights;
use hearth_render::water::WaterHeights;
use hearth_world::{BlockRegistry, Cube};
use hearth_worldgen::{PlanetGrid, WorldGenerator};

/// From the client.
#[derive(Debug, Clone)]
pub enum ToServer {
    /// Where the player's own movement took them, and what it did since the last report.
    Moved(Moved),
    /// Lie down to sleep (true) or get up.
    Sleep(bool),
    /// Put the player at a place (the globe's choice, a debug move): the server finds solid
    /// ground there.
    Place(DVec3),
    /// Live on as a new person after death (by the world's death rules).
    /// After death, live on as the world's rules allow (Legacy: as this person, or the same).
    Respawn(Option<hearth_character::Appearance>),
    /// Move a carried thing (or `count` of a stack) somewhere else carried.
    Shift {
        from: hearth_items::Path,
        count: Option<u16>,
        to: hearth_items::Target,
    },
    /// Pick up a thing lying in the world (or take hold to drag it if it is too heavy).
    PickUp(u64),
    /// Put a carried thing (or `count` of a stack) down on the ground at a point.
    PutDown {
        from: hearth_items::Path,
        count: Option<u16>,
        at: DVec3,
    },
    /// Take hold of a thing lying in the world to drag it.
    Drag(u64),
    /// Let go of what is dragged, where it is now.
    LetGo(DVec3),
    /// Do a process (its content id) to what is looked at; attended work goes on until it is
    /// done or stopped. `hand` is the quality the player's own hands reached (knapping by hand;
    /// 0: the piece snapped), if they did it so.
    Act {
        process: String,
        aim: AimAt,
        hand: Option<f32>,
    },
    /// Stop the work in hand.
    StopWork,
    /// What the player looks at now (for what sight teaches).
    Look(AimAt),
    /// Eat one of the carried food at a path.
    Eat(hearth_items::Path),
    /// Drink from water looked at, or from a carried water skin.
    Drink(DrinkFrom),
    /// Fill a carried water skin from water looked at.
    Fill {
        skin: hearth_items::Path,
        aim: AimAt,
    },
    /// Throw what is held in the right hand along a direction at a speed (m/s).
    Throw { dir: DVec3, speed: f64 },
    /// Thrust or strike with what is held in the right hand along a direction (at an animal in
    /// reach).
    Thrust { dir: DVec3 },
    /// Development and tests: put a thing in the player's hands or containers (or a drag).
    Give(hearth_items::Stack),
    /// Debug: move the clock on (or back) by game hours.
    SkipHours(f64),
    /// Debug: extra ticks per second of play (0 for none).
    TimeWarp(f64),
    /// Tests and bots: from now on the world ticks only when asked; run this many game ticks
    /// as fast as they go (warped ticks count as they pass; 0 stops the clock until asked).
    Run(u64),
    /// Tests and bots: lightning strikes this column (a tree there catches fire).
    Strike { x: i32, z: i32 },
    /// Tests and bots: the weather held as given wherever it is sampled (`None` lets it go).
    HoldWeather(Option<hearth_env::weather::WeatherHold>),
    /// Tests and bots: a block of vegetation catches fire.
    Ignite(hearth_math::BlockPos),
    /// Tests and bots: a grown animal of a species (content id) dies at a place, to lie there
    /// (a natural death a test may force).
    Die { species: String, at: DVec3 },
    /// Tests and bots: the vegetation about a place is cleared or burned, from now on.
    Disturb {
        kind: hearth_worldgen::vegetation::DisturbanceKind,
        x: i32,
        z: i32,
        radius: f32,
    },
    /// Stop the clock and the body (a single-player menu is open), or go on.
    Pause(bool),
    /// A shout: loud, to make an animal think again.
    Shout,
    /// Asks for a census of the groups of animals in the regions about the player (tools,
    /// tests, the debug map).
    Census,
    /// How much terrain to keep around the player (cubes).
    View { radius: i32, vertical: i32 },
    /// Save and stop.
    Quit,
}

/// What the player looks at, as the server is told it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AimAt {
    Nothing,
    /// A block; `top` when its top face is the one looked at.
    Block {
        pos: hearth_math::BlockPos,
        top: bool,
    },
    /// A thing lying in the world.
    Thing(u64),
}

/// Where a drink comes from.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DrinkFrom {
    Water(AimAt),
    Skin(hearth_items::Path),
}

/// Work being done, as the client shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkView {
    /// The process (content id) and its words.
    pub process: String,
    pub action: String,
    /// 0–1 done.
    pub done: f32,
    /// Seconds of play left at the present pace.
    pub play_s_left: f64,
}

/// What came of something done.
#[derive(Debug, Clone, PartialEq)]
pub struct Acted {
    pub process: String,
    pub done: bool,
    /// In words: what was made, or how it failed.
    pub words: String,
}

/// A report of the player's movement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moved {
    pub mover: Mover,
    /// The hardest landing since the last report (m/s, after cushioning).
    pub landed: Option<f64>,
    pub motion: Motion,
    pub speed: f64,
    pub straining: bool,
    /// Share of the body in water.
    pub immersion: f64,
    /// Seconds without air.
    pub airless_s: f64,
    /// Which way the player faces (radians; 0 toward +z, turning toward +x).
    pub yaw: f32,
}

/// What the client needs once the world is ready.
pub struct Ready {
    pub planet: Planet,
    pub grid: Arc<PlanetGrid>,
    /// For distant terrain and tints.
    pub generator: Arc<WorldGenerator>,
    pub lod: Arc<LodGen>,
    pub reg: Arc<BlockRegistry>,
    pub body: Arc<BodyConfig>,
    pub vertical_scale: f32,
    /// The world's calendar, the clock and where the player is.
    pub calendar: hearth_env::Calendar,
    pub ticks: u64,
    pub player: Mover,
    /// How the player looks.
    pub appearance: hearth_character::Appearance,
    /// What death means in this world.
    pub death_rules: hearth_save::DeathRules,
    /// The kinds of things.
    pub items: Arc<hearth_items::Items>,
    /// The game data, and the processes and knowledge the client lists from it.
    pub content: Arc<hearth_content::Content>,
    pub crafts: Arc<hearth_craft::Crafts>,
    pub graph: Arc<hearth_craft::Graph>,
    /// How knowledge is gained in this world.
    pub knowledge_mode: hearth_craft::Mode,
    /// The world ended (permadeath), and the life it ended with.
    pub ended: Option<LifeSummary>,
    /// Where the distant terrain's tiles of this world are kept on disk, if anywhere.
    pub lod_cache: Option<std::path::PathBuf>,
}

/// A life told after it ended (permadeath).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LifeSummary {
    pub name: String,
    /// Days of the world's calendar lived.
    pub days: f64,
    pub walked_km: f64,
    pub farthest_km: f64,
    pub cause: hearth_body::Death,
    /// What they learned (the names of the techniques).
    #[serde(default)]
    pub discovered: Vec<String>,
}

/// The player's body as the client shows it and lets it move.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyView {
    pub status: Status,
    pub ability: Ability,
    pub asleep: bool,
    /// Lying down (resting or asleep).
    pub lying: bool,
    /// Ticks of the world a second now (20, more while time is warped or the player sleeps).
    pub rate: f64,
    pub dead: Option<Death>,
    pub injuries: Vec<InjuryState>,
    /// Illnesses (content ids) whose symptoms have begun.
    pub illnesses: Vec<String>,
    /// The weather, water and shelter the body is in.
    pub exposure: Exposure,
}

/// Smoke rising from a fire in the vegetation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plume {
    /// Where it rises from.
    pub at: DVec3,
    /// How thick, 0–1.
    pub strength: f32,
    /// A far fire's (its smoke rises kilometres and is seen across the land).
    pub far: bool,
}

/// From the server.
pub enum ToClient {
    Ready(Box<Ready>),
    /// A cube's blocks and light, for the client's own collision.
    Cube(CubePos, Arc<Cube>),
    Mesh(Box<CubeMesh>),
    Unload(CubePos),
    /// What covers the sky and where the water's surface lies around the player.
    Heights(Box<SkyHeights>, Box<WaterHeights>),
    /// The smoke of the fires burning in the vegetation (empty: none).
    Smoke(Vec<Plume>),
    /// The animals near the player (ten times a second while there are any).
    Animals(Vec<hearth_fauna::live::AnimalView>),
    /// Calls the animals made (those in the world and those about it).
    Calls(Vec<hearth_fauna::voices::Called>),
    /// The signs animals left near the player (tracks, blood, droppings), with the world's
    /// seconds they are timed by and how long a day is (s).
    Signs {
        now: f64,
        day_s: f32,
        signs: Vec<hearth_fauna::live::Sign>,
    },
    /// The groups of animals in the regions about the player: species, where, how many.
    Census(Vec<(u16, glam::DVec2, u32)>),
    /// The vegetation the distant terrain is grown with (when it changes, and as the years
    /// turn).
    Vegetation(hearth_worldgen::vegetation::Vegetation),
    /// The player's changes as the distant terrain shows them (when they change).
    EditTops(Arc<hearth_lod::EditTops>),
    /// The world clock at the end of a tick.
    Clock(u64),
    Body(Box<BodyView>),
    /// The server put the player somewhere (a place chosen, a new life).
    Placed(Mover),
    /// The world was saved.
    Saved,
    Failed(String),
    /// The player woke, and why.
    Woke(hearth_body::Wake),
    /// The player is someone else now (Legacy) or again (Hardy).
    Person(hearth_character::Appearance),
    /// What the player carries, when it changed.
    Carried(hearth_items::Carry),
    /// The things lying near the player, when they changed.
    Items(Vec<hearth_items::WorldItem>),
    /// The world ended with its character's death (permadeath).
    Ended(LifeSummary),
    /// What the player knows, when it changed.
    Knowledge(Box<hearth_craft::KnowledgeState>),
    /// The work in hand, each tick it goes on (none: stopped or done).
    Work(Option<WorkView>),
    /// What came of a process, a meal or a drink.
    Acted(Acted),
    /// A tree falls: its blocks as they stood (gone from the world now), turning down about
    /// the edge `pivot` toward `toward` over `seconds`; where it comes to rest arrives as block
    /// changes when the fall is over.
    TreeFalls {
        blocks: Vec<(hearth_math::BlockPos, hearth_world::BlockStateId)>,
        pivot: glam::DVec3,
        toward: hearth_math::Direction,
        seconds: f32,
    },
    /// Something learned (true) or a hunch (false): the node's name and the journal's words.
    Learned {
        name: String,
        discovered: bool,
        text: String,
    },
}

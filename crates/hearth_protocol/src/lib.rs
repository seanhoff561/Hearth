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

/// The version of the messages between the client and the server, raised with every change to
/// them (D166); the network handshake checks it (Amendment R, R1).
pub const PROTOCOL: u32 = 5;

/// What a rest is for (E §4.3, Amendment P §7.1). The body lies down; sleep comes when it is
/// sleepy; the world goes faster meanwhile, up to the world's sleep speed, and anything that
/// needs the player ends it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rest {
    /// Asleep until the sun is up.
    SleepUntilMorning,
    /// Asleep until the body wakes rested.
    SleepUntilRested,
    /// Resting for some hours.
    Hours(f32),
    /// Resting until the sun is down.
    UntilDusk,
    /// Waiting until the work left to itself nearby is done (the meat dry, the pot fired).
    UntilDone,
    /// Resting until sleep comes, then asleep until rested (P §7.1).
    UntilSleepy,
}

/// How a rest ended: how long it lasted and how much of it asleep (hours), what it was for,
/// and what ended it.
#[derive(Debug, Clone, PartialEq)]
pub struct Rested {
    pub hours: f32,
    pub slept_h: f32,
    pub rest: Rest,
    pub end: RestEnd,
}

/// What ended a rest.
#[derive(Debug, Clone, PartialEq)]
pub enum RestEnd {
    /// What it was for came: the morning, the hours, dusk, the work done.
    Came,
    /// The body woke for a reason.
    Woke(hearth_body::Wake),
    /// Awake, the body's needs would not let it rest (the cold, hunger…).
    Needs(hearth_body::Wake),
    /// An animal came near (its name).
    Animal(String),
    /// The body was hurt.
    Hurt,
    /// The player got up.
    GotUp,
    /// Nothing nearby was left to its work.
    NothingWaiting,
}

/// From the client.
#[derive(Debug, Clone)]
pub enum ToServer {
    /// Where the player's own movement took them, and what it did since the last report.
    Moved(Moved),
    /// Watching the world (Creative's spectating, Amendment P §3.3): from where the eye is, or
    /// none to stop. The player's body is put aside meanwhile — still, unharmed, unseen.
    Observe(Option<DVec3>),
    /// Lie down to sleep or rest until something (E §4.3), or get up (`None`).
    Rest(Option<Rest>),
    /// Put the player at a place (the globe's choice, a debug move): the server finds solid
    /// ground there.
    Place(DVec3),
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
        /// The quality reached knapping by hand, when it was.
        hand: Option<f32>,
        /// The hand doing it (P §5.2): its tools are used (none: either hand's).
        with: Option<hearth_items::Hand>,
    },
    /// Stop the work in hand.
    StopWork,
    /// Look closely at work left to itself (E §7.5): the thing lying there (its id), and
    /// whether to tell the numbers too (Creative, Developer mode).
    Inspect { id: u64, numbers: bool },
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
    /// A blow along a direction (E §3.2): with what is in the hand (the right's, else the
    /// left's) as its use says (a thrust, a swing, a slash, a stab, a strike), with the fist
    /// that holds a thing with no blow of its own or an empty one, or (`kick`) with the foot.
    /// It lands after its wind-up, on whatever is along its path then.
    Blow {
        dir: DVec3,
        kick: bool,
        /// The hand that strikes (P §5.2; none: the right, else the left).
        with: Option<hearth_items::Hand>,
    },
    /// A bow drawn for `drawn_s` seconds, loosed along a direction: an arrow carried flies.
    Loose { dir: DVec3, drawn_s: f32 },
    /// A burning brand in hand held up high (or lowered): it lights the way and keeps hungry
    /// animals off as a fire does.
    HoldUp(bool),
    /// Creative (Amendment P §3.1): a thing of the inventory taken, placed or summoned at what
    /// is looked at (or before the player). Refused outside Creative.
    Creative { act: CreativeAct, aim: AimAt },
    /// Creative's remove tool: what is looked at (a block — a plant or a piece with it — a thing
    /// lying, an animal) taken out of the world. Refused outside Creative.
    Remove(AimAt),
    /// Creative's instant actions on or off: work done as soon as it is begun.
    Instant(bool),
    /// Development and tests: put a thing in the player's hands or containers (or a drag).
    Give(hearth_items::Stack),
    /// Debug: move the clock on (or back) by game hours.
    SkipHours(f64),
    /// The player, dead, begins a new life (Amendment E §6.6): a new adult about a place (none:
    /// near where they last lived), looking as chosen (none: as the last one did).
    NewLife {
        at: Option<DVec3>,
        appearance: Option<hearth_character::Appearance>,
    },
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
    /// Tests and bots: a wild animal of a species (content id) stands at a place, of no group
    /// (a young one alone, as if its mother were gone); gone when the player leaves.
    Bring {
        species: String,
        young: bool,
        female: bool,
        at: DVec3,
    },
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
    /// Save now; the world goes on (tools and bots keeping its moments).
    Save,
}

/// What Creative's inventory asks (Amendment P §3.1; no people, Amendment E §9.1).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CreativeAct {
    /// Items into the hands (or beside the player when they are full).
    Take { item: String, count: u32 },
    /// A block of a natural material.
    Place { block: String },
    /// A plant of a species, young or grown.
    Plant { species: String, young: bool },
    /// Animals of a species beside the player: one, or a herd.
    Summon {
        species: String,
        female: bool,
        young: bool,
        count: u32,
    },
    /// A building piece (of a material) or a workstation, finished.
    Build { piece: String },
    /// A piece of knowledge known (or forgotten).
    Learn { node: String, known: bool },
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
    /// The place beside a block across one of its faces: where a piece put up goes (V2-8).
    Beside {
        pos: hearth_math::BlockPos,
        face: hearth_math::Direction,
    },
    /// A live animal (V2-12: one to catch, tether, milk).
    Animal(u64),
}

impl AimAt {
    /// The block looked at, if it is one.
    pub fn block(self) -> Option<hearth_math::BlockPos> {
        match self {
            AimAt::Block { pos, .. } | AimAt::Beside { pos, .. } => Some(pos),
            _ => None,
        }
    }
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
    /// How the body does it (P §6.1, E §7.2): its pose, a stroke's seconds, and which hand
    /// leads.
    pub work: Option<hearth_content::schema::process::WorkModel>,
    pub with: Option<hearth_items::Hand>,
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
    /// The ground's rise over its run under the last steps (positive uphill), for the effort.
    pub grade: f64,
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
    /// What is kept of what was known when a new life begins (Amendment E §6.6).
    pub after_death: hearth_save::AfterDeath,
    /// The kinds of things.
    pub items: Arc<hearth_items::Items>,
    /// The game data, and the processes and knowledge the client lists from it.
    pub content: Arc<hearth_content::Content>,
    pub crafts: Arc<hearth_craft::Crafts>,
    pub graph: Arc<hearth_craft::Graph>,
    /// How knowledge is gained in this world.
    pub knowledge_mode: hearth_craft::Mode,
    /// The world's game mode (`balance/modes.ron`, Amendment P §2), none for a world of no mode.
    pub mode: Option<String>,
    /// Where the distant terrain's tiles of this world are kept on disk, if anywhere.
    pub lod_cache: Option<std::path::PathBuf>,
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
    /// Hours until sleepy enough to drop off lying down (0: now; none: not within a day).
    pub sleepy_in_h: Option<f32>,
    /// What the sun, the ground and wounds have done to the skin (as the body is drawn).
    pub skin: hearth_body::skin::Skin,
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
    /// The world being made or opened: how far it has come (0–1) and what is being done (a
    /// word key of the interface's, or the planet's own words).
    Progress {
        share: f32,
        stage: String,
    },
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
    /// seconds they are timed by.
    Signs {
        now: f64,
        signs: Vec<hearth_fauna::live::Sign>,
    },
    /// The groups of animals in the regions about the player: species, where, how many.
    Census(Vec<(u16, glam::DVec2, u32)>),
    /// The birds circling over remains beyond the near field (Amendment T §2.1), for the client
    /// to draw where `Flock::bird_at` puts them (when they change).
    Flocks(Vec<hearth_fauna::flock::Flock>),
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
    /// The player's person looks so now (a new life).
    Looks(hearth_character::Appearance),
    /// The world was saved.
    Saved,
    Failed(String),
    /// A rest ended: how long it was, and why it ended.
    Rested(Rested),
    /// What the player carries, when it changed.
    Carried(hearth_items::Carry),
    /// The things lying near the player, when they changed.
    Items(Vec<hearth_items::WorldItem>),
    /// What the player knows, when it changed.
    Knowledge(Box<hearth_craft::KnowledgeState>),
    /// The work in hand, each tick it goes on (none: stopped or done).
    Work(Option<WorkView>),
    /// What came of a process, a meal or a drink.
    Acted(Acted),
    /// A blow or a thing flung lands on an animal (Amendment T §0.2: heard, not told): where,
    /// how loud (0.2–1.5), and whether it glanced off.
    Struck {
        at: glam::DVec3,
        force: f32,
        glancing: bool,
    },
    /// A tree falls: its blocks as they stood (gone from the world now), turning down about
    /// the edge `pivot` toward `toward` over `seconds`; where it comes to rest arrives as block
    /// changes when the fall is over.
    TreeFalls {
        blocks: Vec<(hearth_math::BlockPos, hearth_world::BlockStateId)>,
        pivot: glam::DVec3,
        toward: hearth_math::Direction,
        seconds: f32,
    },
    /// Built pieces give way and fall (V2-8 (b)): their blocks as they stood (gone from the
    /// world now); what is left of them arrives as things lying where they fell.
    Collapse(Vec<(hearth_math::BlockPos, hearth_world::BlockStateId)>),
    /// How hard the built pieces about the player are pressed (V2-8 (f)): the largest share of
    /// what each can bear that it bears (over one, it gives way).
    Stress(Vec<(hearth_math::BlockPos, f32)>),
    /// Something learned (true) or a hunch (false): the node's name and the journal's words.
    Learned {
        name: String,
        discovered: bool,
        text: String,
    },
}

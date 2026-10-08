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
pub const PROTOCOL: u32 = 2;

/// From the client.
#[derive(Debug, Clone)]
pub enum ToServer {
    /// Where the player's own movement took them, and what it did since the last report.
    Moved(Moved),
    /// The person the developer's inspector looks at (F3), or none.
    Inspect(Option<u64>),
    /// The person the player looks at, near enough to speak with, or none (H9).
    Regard(Option<u64>),
    /// Watching the world (the Observer, V2.1 §15.4): from where the eye is, or none to stop. A
    /// living player is put aside meanwhile — its body still, unharmed, unseen.
    Observe(Option<DVec3>),
    /// The one the Observer follows (its life read), or none.
    Follow(Option<u64>),
    /// Asks for the chronicle: deep time's and the living world's notable events.
    Chronicle,
    /// Asks for a map overlay of the globe (none: no overlay).
    Overlay(Option<OverlayKind>),
    /// Lie down to sleep (true) or get up.
    Sleep(bool),
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
    /// Hand what the hands hold (the right first) to a person within reach: a gift (V2.1 §8.7).
    GiveTo { person: u64 },
    /// Say or do something to a person within speaking distance (V2.1 §16; H9): greet, tell
    /// one's name, thank, ask to be taught, offer to teach, ask to stay, propose to pair …
    Speak {
        person: u64,
        ask: hearth_people::player::Ask,
    },
    /// Words the player typed to a person within speaking distance (V2.1 §10.4; H10), read by
    /// the conversation backend as one of the acts of [`ToServer::Speak`]; with no backend, not
    /// heard.
    SayText { person: u64, text: String },
    /// The conversation backend as the player set it up (off by default).
    Conversation(hearth_core::options::ConversationOptions),
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
    /// The player's childhood: on to its next moment, or grown up now (Addendum A).
    Childhood(Skip),
    /// The player, dead, lives on as one of their people (Addendum B §2): that person's id.
    Inhabit(u64),
    /// Born into the household chosen of those offered (H8): its place in the list, a daughter
    /// or a son or as chance has it.
    BeBorn { choice: usize, female: Option<bool> },
    /// The player, dead, is born again (Addendum B §2.2): about a place on the globe (none: where
    /// they died), into a household of it offered by `Births` (in an era) or one of its families
    /// (Wild Earth), with a daughter, a son or chance.
    BornAgain {
        at: Option<DVec3>,
        female: Option<bool>,
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
}

/// Something said near the player, as the player makes it out (V2.1 §10.3).
#[derive(Debug, Clone, PartialEq)]
pub struct HeardLine {
    /// Its id (a phrasing of it comes as [`ToClient::Phrased`] with it).
    pub id: u64,
    /// Who said it (its name), and whether to the player.
    pub speaker: String,
    pub to_you: bool,
    /// The words as they sound, and what the player makes of them (words known in the player's
    /// own tongue, half-known ones doubted, the rest as dots).
    pub spoken: String,
    pub sense: String,
    /// The share made out (0–1).
    pub understood: f32,
    /// The gesture with it, if any (in words).
    pub gesture: Option<String>,
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
    /// What death means in this world (Addendum B §2): whom one may live on as, what is kept of
    /// what was known, and whether one may be born again.
    pub death: hearth_save::Death,
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

/// What the player asks of a birth (V2.1 Addendum A): a name, to be born a daughter or a son
/// or as chance has it, and the loincloth they first wear. Nothing of their looks: their
/// parents' genes give those.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Wish {
    pub name: String,
    /// A daughter (true) or a son (false); none, as the father's gamete falls.
    pub female: Option<bool>,
    pub loincloth: hearth_character::Loincloth,
}

/// A birth as the player is shown it (H1): the two parents of the place, and the child as their
/// genes made them (grown, until childhood is lived: H3).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Born {
    pub mother: hearth_character::Appearance,
    pub father: hearth_character::Appearance,
    pub you: hearth_character::Appearance,
    /// Where (degrees of latitude): its sun set the pool's colouring.
    pub latitude_deg: f64,
    /// The mother's, the player's and the father's ages (years) as the life begins.
    #[serde(default)]
    pub ages: [f32; 3],
    /// Their other children, eldest first: their ages and how they look.
    #[serde(default)]
    pub siblings: Vec<(f32, hearth_character::Appearance)>,
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
    /// Where the player is held, not moving of their own (a child carried, or the years of a
    /// childhood passing): the client keeps them there.
    pub held: Option<DVec3>,
    /// How tall the body stands to a grown one (a child's less than 1): its box and eyes.
    pub scale: f64,
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
    /// The people near the player (ten times a second while there are any).
    People(Vec<hearth_people::PersonView>),
    /// The record of the person the developer's inspector looks at (F3), once a second.
    Inspected(Option<Box<hearth_people::inspect::Report>>),
    /// What the player knows of the person it looks at (H9: its name if learned, kinship, how
    /// they seem to take the player, what it has heard of them).
    Regarded(Option<(u64, Vec<String>)>),
    /// A line heard (its id) as the conversation backend phrased it, the filter passing it
    /// (V2.1 §10.4; H10): shown in place of the templated sense.
    Phrased {
        line: u64,
        text: String,
    },
    /// The player's typed words were unclear: the acts they may be, each with its words, to
    /// choose from (none: nothing could be made of them).
    Clarify {
        person: u64,
        text: String,
        options: Vec<(hearth_people::player::Ask, String)>,
    },
    /// Whether the conversation backend is asked, whether the player may type to people, and
    /// why it is not when set up (no key, no model …).
    Conversing {
        on: bool,
        free_text: bool,
        trouble: Option<String>,
    },
    /// The life of the one the Observer follows.
    LifeOf(Option<(u64, Vec<String>)>),
    /// The chronicle, newest first.
    Chronicle(Vec<ChronicleEntry>),
    /// A map overlay of the globe.
    Overlay(Option<OverlayMap>),
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
    /// The player was born (a new world, or born again): their parents and themself.
    Born(Box<Born>),
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
    /// What the people near said, as the player makes it out.
    Heard(Vec<HeardLine>),
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
    /// The player's childhood as it goes: the moment being lived or the years passing (none:
    /// grown, or never a child).
    Childhood(Option<ChildhoodView>),
    /// The player's life told at its end, and who of their people they could live on as.
    Story(Box<Story>),
    /// Who the player is now, having taken up another's life: the briefing's lines.
    WhoYouAre(Vec<String>),
    /// The households of the place the player may be born into (H8): who they are, never how
    /// they look.
    Births(Vec<BirthChoice>),
}

/// A household a player may be born into (H8, Addendum A), told by who its people are.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BirthChoice {
    pub title: String,
    pub lines: Vec<String>,
}

/// A life told at its end (Addendum B §2).
#[derive(Debug, Clone, PartialEq)]
pub struct Story {
    pub lines: Vec<String>,
    /// Those the dead one may live on as, as the world's scope allows: their kin first.
    pub others: Vec<Other>,
}

/// One a dead player may live on as (Addendum B §2.2–2.3): their id, who they are to the dead
/// (a stranger by their household; never how they look), and which of the choosing's filters
/// they answer to — the dead one's family, their group, near where they died — and whether a
/// child, whose childhood would be lived on from its age.
#[derive(Debug, Clone, PartialEq)]
pub struct Other {
    pub id: u64,
    pub words: String,
    pub family: bool,
    pub group: bool,
    pub near: bool,
    pub child: bool,
}

/// Skipping ahead in a childhood.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    /// To the next moment (from a moment, the years passing to it).
    Next,
    /// Straight to coming of age, the years between lived at the household's pace.
    GrownUp,
}

/// A childhood as the player is told it (Addendum A).
#[derive(Debug, Clone, PartialEq)]
pub struct ChildhoodView {
    /// The moment's name, and what is said of it (the years passing: empty).
    pub name: String,
    pub text: String,
    /// The player's age (years).
    pub age: f32,
    /// The years passing quickly to the next moment.
    pub passing: bool,
}

/// What a map overlay of the globe shows (V2.1 §15.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OverlayKind {
    /// How many live where.
    People,
    /// Whose culture and language, each its own colour.
    Cultures,
    /// Where a technique is known (its index among the world's techniques followed).
    Knowledge(usize),
    /// The people's looks as their gene pools have them (skin, the darker the deeper).
    Looks,
}

/// An overlay: a picture over the globe's map (equirectangular, west to east from longitude
/// −180°, north to south), what it shows, and its key.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayMap {
    pub kind: OverlayKind,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<[u8; 4]>,
    pub legend: String,
}

/// A line of the chronicle: when, what, and where (to go there).
#[derive(Debug, Clone, PartialEq)]
pub struct ChronicleEntry {
    pub when: String,
    pub text: String,
    pub at: Option<DVec3>,
}

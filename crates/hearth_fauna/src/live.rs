//! Animals in the world (v2 §7.4): near the player a population's groups become animals that
//! stand and move on the ground — a herd of red deer with its calves, a sow and her piglets —
//! and the small species of the cells about the player are drawn from their numbers; as the
//! player leaves they fold back into their groups and cells, the dead staying dead.
//!
//! What they do here is the plainest life: grazing and wandering about the group, resting by
//! the hours of their kind, fleeing a person who comes within their flight distance. Their
//! minds (senses, needs, the utility AI, herds and packs) replace it in V2-7 (f).

use glam::{DVec2, DVec3};
use hearth_content::schema::fauna::{Activity, CallWhen};
use hearth_content::time::DAY_S;
use hearth_math::hash::{Rng, derive_seed, hash2};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::anim::scale_of;
use crate::danger::{Attack, Cause, Hostile, Kill, blow, closes, faced_down, lean, provoked};
use crate::ecology::{Ecology, REGION_LEN, about, dist};
use crate::habitat::CELL_M;
use crate::herd::{Kept, Tiding};
use crate::mind::{Air, Presence, Sense, Wary, sense, yaw_toward};
use crate::nav::{FISH_DEPTH, Flight, Walker, find_way, perch_near, trunk_near, water_near};
use crate::rig::{Frame, Rig, frame_of};
use crate::species::{Catalog, Species};
use crate::voices::{Called, call_for, calls_now};
use crate::wound::{Blow, Hurt, Part, strikes, wound};

/// Where an animal can stand.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Footing {
    /// The height of the ground's surface (m).
    pub y: f64,
    /// Water stands over it.
    pub water: bool,
    /// How deep (m; 0 where dry).
    pub depth: f64,
}

impl Footing {
    pub fn dry(y: f64) -> Self {
        Self {
            y,
            water: false,
            depth: 0.0,
        }
    }

    /// The level a body moves at: the ground, or the water's surface over it.
    pub fn level(&self) -> f64 {
        self.y + self.depth
    }
}

/// What fills a block, as animals go over, through and up it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// Air.
    Open,
    /// Grass, herbs and bushes a body pushes through (and is half hidden in).
    Plant,
    Solid,
    Water,
    /// A tree's trunk (climbable).
    Trunk,
    /// A tree's limb (a perch).
    Limb,
    /// Foliage (a perch on top, cover within).
    Leaves,
}

/// The ground animals walk on: the loaded terrain.
pub trait Ground {
    /// The ground near height `y` at (x, z), searching a few metres up and down; None where the
    /// terrain is not loaded or there is no footing.
    fn footing(&self, x: f64, z: f64, y: f64) -> Option<Footing>;
    /// The highest ground at (x, z) (under the trees' foliage); None where not loaded.
    fn top(&self, x: f64, z: f64) -> Option<Footing>;
    /// The surface at a foot's place as it takes signs: how plainly it takes a print (0 none
    /// … 1 fresh snow: bare earth, mud and sand do; grass, moss and leaf litter hardly; stone
    /// not at all) and its height (the top of lying snow, which a foot sinks into). By default
    /// no print, at the foot.
    fn sign_surface(&self, _x: f64, _z: f64, y: f64) -> (f32, f64) {
        (0.0, y)
    }
    /// What fills the block at (x, y, z); None where not loaded. By default, the ground with
    /// water and open air over it.
    fn cell(&self, x: i32, y: i32, z: i32) -> Option<Cell> {
        let f = self.footing(x as f64 + 0.5, z as f64 + 0.5, y as f64)?;
        let y = y as f64;
        Some(if y < f.y {
            Cell::Solid
        } else if y < f.level() {
            Cell::Water
        } else {
            Cell::Open
        })
    }
}

/// An animal's age.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stage {
    /// Born this year.
    Young,
    /// Older, not yet grown.
    Juvenile,
    Adult,
}

/// What an animal is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Act {
    /// Feeding where it stands, head down.
    Graze,
    /// Walking to somewhere near.
    Walk,
    /// Lying down.
    Rest,
    /// Head up, looking (something is near).
    Alert,
    /// Running away.
    Flee,
    /// Lying asleep.
    Sleep,
    /// Licking or nibbling its coat.
    Groom,
    /// Head down at water.
    Drink,
    /// Up on the hind legs.
    Rear,
    /// Lunging, biting, butting or striking.
    Attack,
    /// On the wing.
    Fly,
    /// Lying dead (a carcass as it is drawn).
    Dead,
}

/// Where an animal is: on the ground, in the water (swimming), on the wing, up a tree (a
/// climber on its trunk, a bird on a perch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Medium {
    #[default]
    Ground,
    Water,
    Air,
    Tree,
}

/// A climb: the foot of the trunk and the height to climb to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Climb {
    pub trunk: DVec3,
    pub to: f64,
}

/// A flight under way: the flight, how far along it (m), and whether it ends on a perch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flying {
    pub flight: Flight,
    pub flown: f64,
    pub perch: bool,
}

/// An animal in the world.
#[derive(Debug, Clone, PartialEq)]
pub struct Animal {
    pub id: u64,
    pub species: u16,
    /// The group it belongs to (large species).
    pub group: Option<u64>,
    /// The cell it was drawn from (small species): region key and local cell.
    pub cell: Option<((i64, i64), usize)>,
    pub stage: Stage,
    pub female: bool,
    /// The middle of its feet.
    pub pos: DVec3,
    /// Facing, radians (0 toward +z, turning toward +x).
    pub yaw: f32,
    /// Ground speed now, m/s.
    pub speed: f32,
    pub act: Act,
    /// Seconds left in the act.
    pub timer: f32,
    /// Where it is walking or running to.
    pub goal: Option<DVec2>,
    /// The gait's phase: strides taken.
    pub stride: f32,
    pub dead: bool,
    pub medium: Medium,
    /// The way it follows to its goal (the next point first): empty where it goes straight.
    pub way: Vec<DVec3>,
    /// Seconds before it looks for a way again (after a step it could not take).
    pub repath: f32,
    pub climb: Option<Climb>,
    pub flying: Option<Flying>,
    /// Its wariness of a person.
    pub wary: Wary,
    /// A young one's mother, whom it follows.
    pub mother: Option<u64>,
    /// How thirsty (a day without water is 1), and the water it is going to drink at.
    pub thirst: f32,
    pub water: Option<DVec3>,
    /// Turned on the person.
    pub hostile: Option<Hostile>,
    /// What it has learned to fear of people (0 … 1).
    pub fear: f32,
    /// Seconds before it weighs turning on the person again.
    pub weigh: f32,
    /// A hunt under way.
    pub hunt: Option<Hunt>,
    /// Where its kill lies while it feeds.
    pub kill_at: Option<DVec3>,
    /// Dead: what killed it (a hunter's species), and what has been eaten of it (kg).
    pub killed_by: Option<u16>,
    pub eaten: f32,
    /// Its wounds.
    pub hurt: Hurt,
    /// A blow's shove (m/s over the ground), dying away in a fraction of a second.
    pub knock: DVec2,
    /// The stride it last left a print at, and the way it has gone since it last let fall a
    /// drop of blood (m).
    pub printed: f32,
    pub drip: f32,
    /// What it was doing the step before (a run begun is an alarm called).
    pub was: Act,
    /// Kept by people (V2-12): its makeup, how tame it is, where it is tethered.
    pub kept: Option<Kept>,
    /// A bird come to remains (Amendment T §2.1): its flock and what it is doing there.
    pub attend: Option<crate::flock::Attend>,
}

/// What a sign is: a print of a foot, a drop of blood, droppings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignKind {
    Print,
    Blood,
    Droppings,
}

impl SignKind {
    /// How long it lasts, in days (a print in snow twice as long).
    pub fn lasts_days(self) -> f64 {
        match self {
            SignKind::Print => 1.5,
            SignKind::Blood => 1.0,
            SignKind::Droppings => 10.0,
        }
    }
}

/// A sign an animal leaves on the ground: what, whose, where (on the ground), which way it went,
/// when (the world's seconds), and how plain it is (fresh snow takes a clear print).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sign {
    pub kind: SignKind,
    pub species: u16,
    pub pos: DVec3,
    pub yaw: f32,
    pub t: f64,
    pub plain: f32,
}

/// Signs are left within this distance of the person.
pub const SIGNS_M: f64 = 80.0;
/// The most signs kept (the oldest go first).
const MAX_SIGNS: usize = 6000;

/// A dead animal taken from the world to lie as a carcass: of what species and age, where,
/// which way it lies, what is left of it, and what killed it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Body {
    pub species: u16,
    pub stage: Stage,
    pub female: bool,
    pub pos: DVec3,
    pub yaw: f32,
    pub left: f32,
    pub killed_by: Option<u16>,
    /// Dead of what a person did to it.
    pub by_person: bool,
}

/// Where a path (a thrown thing's flight, a thrust) first strikes an animal: which, along which
/// segment of the path and how far along it, the point, and the part of the body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub animal: u64,
    pub segment: usize,
    pub t: f32,
    pub at: DVec3,
    pub part: Part,
}

/// What a blow did to an animal: its kind, where it struck, whether deep, whether it killed
/// outright; in words.
#[derive(Debug, Clone, PartialEq)]
pub struct Struck {
    pub species: u16,
    pub part: Part,
    pub deep: bool,
    pub killed: bool,
    /// It glanced off, doing no harm.
    pub glancing: bool,
    /// What it did, for the log and the tests (the world shows it: Amendment T §0.2).
    pub words: String,
}

/// A hunt: the prey, how long it has gone on, whether the rush has begun.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hunt {
    pub prey: u64,
    pub t: f32,
    pub rushing: bool,
}

impl Animal {
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: u64,
        species: u16,
        group: Option<u64>,
        cell: Option<((i64, i64), usize)>,
        stage: Stage,
        female: bool,
        pos: DVec3,
        yaw: f32,
        timer: f32,
        medium: Medium,
    ) -> Self {
        Self {
            id,
            species,
            group,
            cell,
            stage,
            female,
            pos,
            yaw,
            speed: 0.0,
            act: Act::Graze,
            timer,
            goal: None,
            stride: 0.0,
            dead: false,
            medium,
            way: Vec::new(),
            repath: 0.0,
            climb: None,
            flying: None,
            wary: Wary::default(),
            mother: None,
            // Some drank lately, some not.
            thirst: (id % 7) as f32 * 0.1,
            water: None,
            hostile: None,
            fear: 0.0,
            weigh: 0.0,
            hunt: None,
            kill_at: None,
            killed_by: None,
            eaten: 0.0,
            hurt: Hurt::default(),
            knock: DVec2::ZERO,
            printed: 0.0,
            drip: 0.0,
            was: Act::Graze,
            kept: None,
            attend: None,
        }
    }
}

/// When and in what weather the animals live a step: the local hour (0–1), the air.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Now {
    pub hour: f32,
    pub air: Air,
    /// The year's fraction (0 at the March equinox) and whether it is the south.
    pub year_frac: f32,
    pub southern: bool,
}

impl Now {
    /// A still, light hour of a day in early summer.
    pub fn day(hour: f32) -> Self {
        Self {
            hour,
            air: Air::calm_day(),
            year_frac: 0.3,
            southern: false,
        }
    }
}

/// An animal as the client draws it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AnimalView {
    pub id: u64,
    pub species: u16,
    pub stage: Stage,
    pub female: bool,
    pub pos: DVec3,
    pub yaw: f32,
    pub speed: f32,
    pub act: Act,
    pub stride: f32,
    pub medium: Medium,
    /// Wounded (bleeding, lame).
    #[serde(default)]
    pub wounded: bool,
    /// Kept by people (V2-12).
    #[serde(default)]
    pub kept: bool,
    /// How woolly its coat is now (0 a wild coat … 1 a bred fleece at its full growth).
    #[serde(default)]
    pub fleece: f32,
    /// How easy a kept one is with people, as its keeper knows it (0 wild … 1 hand-tame).
    #[serde(default)]
    pub tame: f32,
}

/// The animals near the player.
#[derive(Debug, Clone)]
pub struct Live {
    pub animals: Vec<Animal>,
    next_id: u64,
    seed: u64,
    rng: Rng,
    /// Animals of small species taken out of each cell (region, slot, cell): adults, young.
    drawn: FxHashMap<((i64, i64), usize, usize), (u32, u32)>,
    /// How readily they turn on people against the species' own (the world's Predator Behavior
    /// setting: 1 authentic).
    pub aggression: f32,
    /// What they did to the person in the last step (charges that stopped short too), and the
    /// kills among them.
    pub attacks: Vec<Attack>,
    pub kills: Vec<Kill>,
    /// The signs they left about the person, oldest first, and the world's seconds they are
    /// timed by.
    pub signs: Vec<Sign>,
    pub clock: f64,
    /// The calls they made since these were last taken.
    pub calls: Vec<Called>,
    /// The world's years the kept animals were last tended to (V2-12).
    pub years: Option<f64>,
    /// The world's clock (its ticks in seconds), which the birds over remains circle by, set
    /// each tick and stepped with the animals (`flock::Flock::bird_at`).
    pub world_s: f64,
}

/// Within this distance of the player groups become animals.
pub const NEAR_M: f64 = 112.0;
/// Beyond this they fold back.
pub const FAR_M: f64 = 150.0;
/// Small species are drawn from the cells within this distance.
pub const SMALL_NEAR_M: f64 = 80.0;
/// The most of one small species drawn from a cell.
const SMALL_PER_CELL: u32 = 3;

/// The smallest animal drawn into the world here (smaller ones wait for their own ways of
/// being seen: voles in the grass, songbirds in the trees).
const SMALL_MIN_KG: f32 = 0.2;

fn hdist(a: DVec3, b: DVec3) -> f64 {
    ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

/// Whether a small species walks the ground where the player can meet it (not flying birds,
/// fish, the sea's animals but its penguins, or the very small).
pub fn walks(sp: &Species) -> bool {
    use hearth_content::schema::fauna::BodyPlan as B;
    sp.mass_kg >= SMALL_MIN_KG
        && !sp.aquatic
        && (!sp.marine || sp.plan == B::Penguin)
        && !sp.colony
        && !matches!(
            sp.plan,
            B::BirdPerching | B::Raptor | B::Waterfowl | B::Seabird | B::Insect | B::Snake
        )
}

/// How far under the surface (m) a fish or whale keeps in water `depth` deep: halfway down in
/// a river or a lake; in the sea within its sunlit top, a few of its lengths down (never below
/// halfway), and a whale or a sea cow with its back just under the surface, to breathe.
pub fn swim_depth(sp: &Species, depth: f64) -> f64 {
    use hearth_content::schema::fauna::BodyPlan as B;
    let half = depth * 0.5;
    if !sp.marine {
        return half;
    }
    let keep = if sp.plan == B::Cetacean {
        sp.shoulder_m as f64 * 1.25
    } else {
        3.0 + sp.length_m as f64 * 4.0
    };
    keep.min(half)
}

/// How a species goes about the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mover {
    /// Over the ground (swimming where it must and can).
    Walker,
    /// Over the ground and up trees.
    Climber,
    /// On the wing, to perches and the ground.
    Bird,
    /// In the water.
    Fish,
}

pub fn mover_of(sp: &Species) -> Mover {
    match frame_of(sp.plan) {
        Frame::Bird if sp.fly_m_s.is_some() => Mover::Bird,
        Frame::Fish => Mover::Fish,
        _ if sp.climbs => Mover::Climber,
        _ => Mover::Walker,
    }
}

/// Whether a small species is drawn into the world about the player: those that walk, the
/// birds big enough to see as they forage and fly (crows, owls), the fish of the streams.
pub fn drawn(sp: &Species) -> bool {
    use hearth_content::schema::fauna::BodyPlan as B;
    match mover_of(sp) {
        Mover::Fish => sp.mass_kg >= 0.1,
        Mover::Bird => sp.mass_kg >= SMALL_MIN_KG,
        // Snakes basking where a person may step.
        _ if sp.plan == B::Snake => sp.mass_kg >= 0.05,
        _ => walks(sp),
    }
}

/// Ways looked for in one step (each a bounded search); the others wait for the next.
const SEARCHES_PER_STEP: usize = 6;
/// The columns a way's search looks at: going somewhere near, and running away.
const WALK_BUDGET: usize = 1500;
const FLEE_BUDGET: usize = 700;

/// Whether an animal of its kind is up and about at the hour (0–1, local solar time).
pub fn awake(activity: Activity, hour: f32) -> bool {
    let h = hour * 24.0;
    match activity {
        Activity::Diurnal => (6.0..20.0).contains(&h),
        Activity::Nocturnal => !(7.0..19.0).contains(&h),
        Activity::Crepuscular => !(10.0..16.0).contains(&h) && !(1.0..4.0).contains(&h),
        Activity::Cathemeral => true,
    }
}

impl Live {
    pub fn new(seed: u64) -> Self {
        Self {
            animals: Vec::new(),
            next_id: 1,
            seed,
            rng: Rng::new(derive_seed(seed, "live animals")),
            drawn: FxHashMap::default(),
            aggression: 1.0,
            attacks: Vec::new(),
            kills: Vec::new(),
            signs: Vec::new(),
            clock: 0.0,
            calls: Vec::new(),
            years: None,
            world_s: 0.0,
        }
    }

    fn id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Brings every animal, what it heads for and the marks they left to the copy of the planet
    /// nearest `at` (x shifted by whole circumferences, `wrap` blocks): the player is wrapped
    /// around the planet, and the animals about them are kept in the same frame, so their
    /// distances and headings hold across the seam (E4.1 §4.5).
    pub fn reframe(&mut self, at: DVec3, wrap: f64) {
        if wrap <= 0.0 {
            return;
        }
        let shift = |x: f64| ((at.x - x) / wrap).round() * wrap;
        for a in &mut self.animals {
            let s = shift(a.pos.x);
            if s == 0.0 {
                continue;
            }
            a.pos.x += s;
            if let Some(g) = &mut a.goal {
                g.x += s;
            }
            for w in &mut a.way {
                w.x += s;
            }
            if let Some(w) = &mut a.water {
                w.x += s;
            }
            if let Some(k) = &mut a.kill_at {
                k.x += s;
            }
            if let Some(c) = &mut a.climb {
                c.trunk.x += s;
            }
            if let Some(f) = &mut a.flying {
                f.flight.from.x += s;
                f.flight.to.x += s;
            }
            if let Some((stake, _)) = a.kept.as_mut().and_then(|k| k.tether.as_mut()) {
                stake.x += s;
            }
        }
        for k in &mut self.kills {
            k.at.x += shift(k.at.x);
        }
        for sign in &mut self.signs {
            sign.pos.x += shift(sign.pos.x);
        }
        for c in &mut self.calls {
            c.pos.x += shift(c.pos.x);
        }
    }

    /// Brings the groups and small animals near `player` into the world.
    pub fn materialize(&mut self, eco: &mut Ecology, ground: &dyn Ground, player: DVec3) {
        let cat = eco.catalog.clone();
        let wrap = eco.cells_around as f64 * CELL_M;
        let at = [player.x, player.z];
        let mut keys: Vec<(i64, i64)> = eco.regions.keys().copied().collect();
        keys.sort_unstable();
        let (cells_around, year_offset) = (eco.cells_around, eco.year_offset);
        for key in keys {
            let r = eco.regions.get_mut(&key).expect("region");
            // Asleep in their dens or wintering elsewhere: not met about.
            let f = (r.time + year_offset).rem_euclid(1.0) as f32;
            let gone: Vec<bool> = r
                .groups
                .iter()
                .map(|g| {
                    r.cell_at(cells_around, g.pos[0], g.pos[1])
                        .is_some_and(|c| !about(&cat.species[g.species as usize], &r.habitat[c], f))
                })
                .collect();
            // The groups.
            for (gi, g) in r.groups.iter_mut().enumerate() {
                if g.live || gone[gi] || dist(g.pos, at, wrap) > NEAR_M || g.size() == 0 {
                    continue;
                }
                let sp = &cat.species[g.species as usize];
                // Somewhere to stand for every member, or the group waits.
                let Some(centre) = ground.top(g.pos[0], g.pos[1]) else {
                    continue;
                };
                if centre.water && !sp.aquatic && !sp.marine {
                    continue;
                }
                let spread = (g.size() as f64).sqrt() * (2.0 + sp.mass_kg.sqrt() as f64 * 0.25);
                let mut members: Vec<(Stage, bool)> = Vec::new();
                members.extend((0..g.females).map(|_| (Stage::Adult, true)));
                members.extend((0..g.males).map(|_| (Stage::Adult, false)));
                for _ in 0..g.juveniles {
                    let f = self.rng.next_f32() < 0.5;
                    members.push((Stage::Juvenile, f));
                }
                for _ in 0..g.young {
                    let f = self.rng.next_f32() < 0.5;
                    members.push((Stage::Young, f));
                }
                let mut placed = Vec::new();
                // The water's animals (a pod of whales, a raft of seals at sea) in the water, a
                // fish's depth down or swimming at the surface; the rest on dry ground.
                let swimmer = sp.aquatic || sp.marine;
                let fish = mover_of(sp) == Mover::Fish;
                for (stage, female) in members {
                    let mut spot = None;
                    for _ in 0..6 {
                        let a = self.rng.next_f64() * std::f64::consts::TAU;
                        let d = spread * self.rng.next_f64().sqrt();
                        let (x, z) = (g.pos[0] + a.cos() * d, g.pos[1] + a.sin() * d);
                        let Some(f) = ground.footing(x, z, centre.y) else {
                            continue;
                        };
                        if !f.water && !fish {
                            spot = Some((DVec3::new(x, f.y, z), Medium::Ground));
                            break;
                        }
                        if swimmer && f.water && f.depth >= FISH_DEPTH {
                            let y = if fish {
                                f.level() - swim_depth(sp, f.depth)
                            } else {
                                f.level() - sp.shoulder_m as f64 * 0.85
                            };
                            spot = Some((DVec3::new(x, y, z), Medium::Water));
                            break;
                        }
                    }
                    let (pos, medium) = spot.unwrap_or_else(|| {
                        if swimmer && centre.water {
                            let y = if fish {
                                centre.level() - swim_depth(sp, centre.depth)
                            } else {
                                centre.level() - sp.shoulder_m as f64 * 0.85
                            };
                            (DVec3::new(g.pos[0], y, g.pos[1]), Medium::Water)
                        } else {
                            (DVec3::new(g.pos[0], centre.y, g.pos[1]), Medium::Ground)
                        }
                    });
                    placed.push((stage, female, pos, medium));
                }
                g.live = true;
                let mut made = Vec::with_capacity(placed.len());
                for (stage, female, pos, medium) in placed {
                    let id = self.next_id;
                    self.next_id += 1;
                    let yaw = self.rng.next_f32() * std::f32::consts::TAU;
                    let timer = 2.0 + self.rng.next_f32() * 10.0;
                    made.push(Animal::new(
                        id,
                        g.species,
                        Some(g.id),
                        None,
                        stage,
                        female,
                        pos,
                        yaw,
                        timer,
                        medium,
                    ));
                }
                // The young of the year each with a mother among the females, beside her.
                let mothers: Vec<(u64, DVec3)> = made
                    .iter()
                    .filter(|a| a.stage == Stage::Adult && a.female)
                    .map(|a| (a.id, a.pos))
                    .collect();
                let young = made.iter_mut().filter(|a| a.stage == Stage::Young);
                for (k, a) in young.enumerate() {
                    if let Some(&(m, at)) = mothers.get(k % mothers.len().max(1)) {
                        a.mother = Some(m);
                        if let Some(f) = ground.footing(at.x + 1.2, at.z + 0.8, at.y)
                            && !f.water
                        {
                            a.pos = DVec3::new(at.x + 1.2, f.y, at.z + 0.8);
                        }
                    }
                }
                self.animals.extend(made);
            }
            // The small species of the cells near the player.
            for (slot, &si) in r.pool_species.iter().enumerate() {
                let sp = &cat.species[si as usize];
                if !drawn(sp) {
                    continue;
                }
                let mover = mover_of(sp);
                for c in 0..REGION_LEN {
                    let centre = r.cell_centre(c);
                    if dist(centre, at, wrap) > SMALL_NEAR_M + CELL_M * 0.71
                        || !about(sp, &r.habitat[c], f)
                    {
                        continue;
                    }
                    // The share of the cell within reach (a lattice of its points).
                    let inside: Vec<[f64; 2]> = (0..16)
                        .map(|k| {
                            let (u, v) = ((k % 4) as f64 + 0.5, (k / 4) as f64 + 0.5);
                            [
                                centre[0] + (u / 4.0 - 0.5) * CELL_M,
                                centre[1] + (v / 4.0 - 0.5) * CELL_M,
                            ]
                        })
                        .filter(|p| dist(*p, at, wrap) <= SMALL_NEAR_M)
                        .collect();
                    if inside.is_empty() {
                        continue;
                    }
                    let k = (key, slot, c);
                    if self.drawn.contains_key(&k) {
                        continue;
                    }
                    // How many are about this cell's part near the player: its numbers, the
                    // share of the cell within reach, drawn the same way each day.
                    let i = slot * REGION_LEN + c;
                    let (adults, young) = (r.adults[i], r.young[i] * sp.census_young());
                    let near_share = inside.len() as f32 / 16.0;
                    let h = hash2(
                        hash2(self.seed, key.0 as u64 ^ (key.1 as u64) << 20),
                        (c * 64 + slot) as u64,
                    );
                    let u = hearth_math::hash::unit_f32(h);
                    let draw = |n: f32| -> u32 {
                        let n = n * near_share;
                        (n.floor() + if u < n.fract() { 1.0 } else { 0.0 }) as u32
                    };
                    let na = draw(adults).min(SMALL_PER_CELL);
                    let ny = draw(young).min(SMALL_PER_CELL.saturating_sub(na));
                    if na + ny == 0 {
                        self.drawn.insert(k, (0, 0));
                        continue;
                    }
                    // Where they are: fish in water deep enough, the rest on dry ground (a bird,
                    // as often as not, on a perch in a tree near it).
                    let mut spots = Vec::new();
                    let mut unloaded = false;
                    for _ in 0..(na + ny) * 4 {
                        if spots.len() >= (na + ny) as usize {
                            break;
                        }
                        let p = inside[self.rng.below(inside.len() as u32) as usize];
                        let x = p[0] + (self.rng.next_f64() - 0.5) * CELL_M / 4.0;
                        let z = p[1] + (self.rng.next_f64() - 0.5) * CELL_M / 4.0;
                        let Some(f) = ground.top(x, z) else {
                            unloaded = true;
                            continue;
                        };
                        match mover {
                            Mover::Fish if f.water && f.depth >= FISH_DEPTH => {
                                let y = f.level() - swim_depth(sp, f.depth);
                                spots.push((DVec3::new(x, y, z), Medium::Water));
                            }
                            Mover::Fish => {}
                            _ if f.water => {}
                            Mover::Bird if self.rng.next_f32() < 0.5 => {
                                match perch_near(ground, DVec2::new(x, z), 8.0) {
                                    Some(p) => spots.push((p, Medium::Tree)),
                                    None => spots.push((DVec3::new(x, f.y, z), Medium::Ground)),
                                }
                            }
                            _ => spots.push((DVec3::new(x, f.y, z), Medium::Ground)),
                        }
                    }
                    if spots.len() < (na + ny) as usize && unloaded {
                        // Not loaded yet: wait.
                        continue;
                    }
                    // As many as found room (fish where the water is too shallow for all).
                    let na = na.min(spots.len() as u32);
                    let ny = ny.min(spots.len() as u32 - na);
                    r.adults[i] = (r.adults[i] - na as f32).max(0.0);
                    r.young[i] = (r.young[i] - ny as f32).max(0.0);
                    self.drawn.insert(k, (na, ny));
                    for (n, (pos, medium)) in spots.into_iter().take((na + ny) as usize).enumerate()
                    {
                        let id = self.next_id;
                        self.next_id += 1;
                        let yaw = self.rng.next_f32() * std::f32::consts::TAU;
                        let female = self.rng.next_f32() < 0.5;
                        let stage = if (n as u32) < na {
                            Stage::Adult
                        } else {
                            Stage::Young
                        };
                        let timer = 2.0 + self.rng.next_f32() * 8.0;
                        self.animals.push(Animal::new(
                            id,
                            si,
                            None,
                            Some((key, c)),
                            stage,
                            female,
                            pos,
                            yaw,
                            timer,
                            medium,
                        ));
                    }
                }
            }
        }
        // In the player's frame (across the seam).
        self.reframe(player, wrap);
    }

    /// Folds the groups and small animals far from `player` back into their numbers: the
    /// living into their group (by age and sex) or cell; the dead are left to be taken as bodies.
    pub fn fold(&mut self, eco: &mut Ecology, player: DVec3) {
        let wrap = eco.cells_around as f64 * CELL_M;
        self.reframe(player, wrap);
        // Groups whose animals are all far (or gone).
        let mut groups: FxHashMap<u64, (bool, Vec<usize>)> = FxHashMap::default();
        for (k, a) in self.animals.iter().enumerate() {
            if let Some(g) = a.group {
                let e = groups.entry(g).or_insert((true, Vec::new()));
                e.1.push(k);
                if !a.dead && hdist(a.pos, player) <= FAR_M {
                    e.0 = false;
                }
            }
        }
        let mut remove = vec![false; self.animals.len()];
        for r in eco.regions.values_mut() {
            for g in r.groups.iter_mut().filter(|g| g.live) {
                let Some((far, members)) = groups.get(&g.id) else {
                    // A live group with no animals (all taken): fold it empty.
                    if dist(g.pos, [player.x, player.z], wrap) > FAR_M {
                        g.live = false;
                        g.young = 0;
                        g.juveniles = 0;
                        g.females = 0;
                        g.males = 0;
                    }
                    continue;
                };
                if !*far {
                    continue;
                }
                let (mut young, mut juv, mut f, mut m) = (0u16, 0u16, 0u16, 0u16);
                let (mut cx, mut cz, mut n) = (0.0f64, 0.0f64, 0.0f64);
                for &k in members {
                    let a = &self.animals[k];
                    // The dead stay to be taken as bodies (a hunter's kill that ran far before
                    // it fell still lies where it fell).
                    if a.dead {
                        continue;
                    }
                    remove[k] = true;
                    match (a.stage, a.female) {
                        (Stage::Young, _) => young += 1,
                        (Stage::Juvenile, _) => juv += 1,
                        (Stage::Adult, true) => f += 1,
                        (Stage::Adult, false) => m += 1,
                    }
                    cx += a.pos.x;
                    cz += a.pos.z;
                    n += 1.0;
                }
                g.young = young;
                g.juveniles = juv;
                g.females = f;
                g.males = m;
                if n > 0.0 {
                    g.pos = [(cx / n).rem_euclid(wrap), cz / n];
                }
                g.live = false;
            }
        }
        // Small animals far from the player go back to their cells.
        let mut back: FxHashMap<((i64, i64), usize, usize), (u32, u32)> = FxHashMap::default();
        for (k, a) in self.animals.iter().enumerate() {
            let Some((key, c)) = a.cell else {
                continue;
            };
            let Some(r) = eco.regions.get(&key) else {
                remove[k] = true;
                continue;
            };
            let Some(slot) = r.pool_species.iter().position(|s| *s == a.species) else {
                remove[k] = true;
                continue;
            };
            let cell_at = r.cell_centre(c);
            if dist(cell_at, [player.x, player.z], wrap) <= FAR_M && !a.dead {
                // Its cell is still near: it stays while it is.
                if hdist(a.pos, player) <= FAR_M {
                    continue;
                }
            }
            remove[k] = true;
            let e = back.entry((key, slot, c)).or_default();
            if !a.dead {
                if a.stage == Stage::Adult {
                    e.0 += 1;
                } else {
                    e.1 += 1;
                }
            }
        }
        for ((key, slot, c), (na, ny)) in back {
            if let Some(r) = eco.regions.get_mut(&key) {
                let i = slot * REGION_LEN + c;
                r.adults[i] += na as f32;
                r.young[i] += ny as f32;
            }
            self.drawn.remove(&(key, slot, c));
        }
        // Cells far from the player are drawn afresh next time.
        self.drawn.retain(|(key, _, c), _| {
            eco.regions
                .get(key)
                .is_some_and(|r| dist(r.cell_centre(*c), [player.x, player.z], wrap) <= FAR_M)
        });
        let mut k = 0;
        self.animals.retain(|_| {
            let keep = !remove[k];
            k += 1;
            keep
        });
    }

    /// Every animal onto the ground under it (where the ground has changed or loaded since):
    /// a walker on it, a swimmer and a fish in its water; those on the wing and up trees stay.
    pub fn settle(&mut self, ground: &dyn Ground) {
        for a in &mut self.animals {
            if matches!(a.medium, Medium::Air | Medium::Tree) {
                continue;
            }
            if let Some(f) = ground.footing(a.pos.x, a.pos.z, a.pos.y) {
                a.pos.y = f.y;
                if a.medium == Medium::Water {
                    a.pos.y = f.y + f.depth * 0.5;
                }
            }
        }
    }

    /// One step of `dt` seconds: what each senses of the person and what it does about it —
    /// watching, freezing, running (a herd with the first of it to run) along a way found over
    /// the ground, up a tree for a climber, on the wing for a bird, through the water for a
    /// fish — and otherwise grazing, wandering, following its mother, going to water, resting by
    /// the hours of its kind.
    pub fn step(
        &mut self,
        eco: &Ecology,
        ground: &dyn Ground,
        presence: Option<&Presence>,
        now: &Now,
        dt: f32,
    ) {
        let cat = eco.catalog.clone();
        self.clock += dt as f64;
        self.world_s += dt as f64;
        if let Some(p) = presence {
            self.reframe(p.pos, eco.cells_around as f64 * CELL_M);
        }
        // Where each group's members are, about, and how many; where every animal is.
        let mut centres: FxHashMap<u64, (DVec2, f64)> = FxHashMap::default();
        let mut whereabouts: FxHashMap<u64, DVec3> = FxHashMap::default();
        for a in &self.animals {
            if a.dead {
                continue;
            }
            whereabouts.insert(a.id, a.pos);
            if let Some(g) = a.group {
                let e = centres.entry(g).or_insert((DVec2::ZERO, 0.0));
                e.0 += DVec2::new(a.pos.x, a.pos.z);
                e.1 += 1.0;
            }
        }
        let mut searches = SEARCHES_PER_STEP;
        let mut alarms: Vec<(u64, DVec3, DVec3)> = Vec::new();
        self.attacks.clear();
        self.kills.clear();
        // The young each mother has, where they are; what might be hunted, where; the hunters
        // abroad as their prey may sense them.
        let mut young_of: FxHashMap<u64, Vec<DVec3>> = FxHashMap::default();
        let mut quarry: Vec<(u64, u16, DVec3)> = Vec::new();
        let mut hunters: Vec<(u64, u16, Presence)> = Vec::new();
        for a in &self.animals {
            if a.dead {
                continue;
            }
            if let Some(m) = a.mother {
                young_of.entry(m).or_default().push(a.pos);
            }
            // A herd with its keeper beside it is left alone.
            let guarded = a.kept.is_some() && presence.is_some_and(|p| hdist(p.pos, a.pos) < 40.0);
            if matches!(a.medium, Medium::Ground | Medium::Water) && !guarded {
                quarry.push((a.id, a.species, a.pos));
            }
            if let Some(h) = a.hunt {
                let sp = &cat.species[a.species as usize];
                hunters.push((
                    a.id,
                    a.species,
                    Presence {
                        pos: a.pos,
                        noise: if h.rushing { 0.8 } else { 0.12 },
                        plain: if h.rushing { 1.0 } else { 0.45 },
                        height: sp.shoulder_m,
                        facing: a.yaw,
                        upright: false,
                        shouting: false,
                        vulnerable: 0.0,
                        by_fire: false,
                        running: h.rushing,
                    },
                ));
            }
        }
        let lean = lean(now.year_frac, now.southern);
        let season = {
            let f = if now.southern {
                (now.year_frac + 0.5).rem_euclid(1.0)
            } else {
                now.year_frac
            };
            ((f * 4.0).floor() as usize).min(3)
        };
        let mut killed: Vec<(u64, u16, DVec3)> = Vec::new();
        let mut packs: Vec<(u64, u64, DVec3)> = Vec::new();
        // The birds over each remains on their way down to them or at them now (they come down
        // in turns).
        let mut down: FxHashMap<u64, usize> = FxHashMap::default();
        for a in &self.animals {
            if let Some(at) = a.attend
                && matches!(
                    at.doing,
                    crate::flock::Doing::Landing | crate::flock::Doing::Feeding
                )
            {
                *down.entry(at.flock.key).or_default() += 1;
            }
        }
        let mut gone: Vec<u64> = Vec::new();
        for a in self.animals.iter_mut() {
            if a.dead {
                continue;
            }
            let sp = &cat.species[a.species as usize];
            if let Some(at) = a.attend
                && !a.hurt.wounded()
            {
                let down = down.entry(at.flock.key).or_default();
                if crate::flock::step(a, sp, self.world_s, down, dt as f64) {
                    gone.push(a.id);
                }
                continue;
            }
            let mover = mover_of(sp);
            let walker = Walker::of(sp);
            // Its wounds: bleeding (dead of it at the last), stunned, or too weak to go on.
            if a.hurt.wounded() || a.hurt.stunned > 0.0 || a.hurt.since.is_some() {
                if a.hurt.bleed(dt) {
                    a.dead = true;
                    a.speed = 0.0;
                    a.act = Act::Dead;
                    a.hostile = None;
                    log::info!("A {} dies of its wounds", sp.name.to_lowercase());
                    continue;
                }
                if a.hurt.stunned > 0.0 || a.hurt.lost > 0.3 {
                    a.act = Act::Rest;
                    a.speed = 0.0;
                    a.goal = None;
                    a.way.clear();
                    a.hostile = None;
                    a.timer = a.timer.max(1.0);
                    continue;
                }
            }
            a.repath = (a.repath - dt).max(0.0);
            a.thirst = (a.thirst + dt / DAY_S as f32).min(2.0);
            a.fear = (a.fear - dt / 600.0).max(0.0);
            // What it senses of the person, and of the hunters that hunt its kind.
            let mut sensed: Option<(f32, Sense, DVec3)> = presence.and_then(|p| {
                sense(sp, a.pos, a.yaw, p, &now.air, ground).map(|(r, s)| (r, s, p.pos))
            });
            for (id, hs, hp) in &hunters {
                if *id == a.id
                    || !cat.species[*hs as usize]
                        .prey
                        .iter()
                        .any(|(i, _)| *i == a.species as usize)
                {
                    continue;
                }
                if let Some((r, s)) = sense(sp, a.pos, a.yaw, hp, &now.air, ground)
                    && sensed.is_none_or(|b| r > b.0)
                {
                    sensed = Some((r, s, hp.pos));
                }
            }
            match sensed {
                Some((r, s, at)) => a.wary.update(Some((r, s)), at, dt),
                None => a.wary.update(None, a.pos, dt),
            }
            // Turning on the person, for its reasons, and its charge.
            if let Some(p) = presence
                && matches!(a.medium, Medium::Ground | Medium::Water)
            {
                let d = hdist(a.pos, p.pos);
                a.weigh -= dt;
                let snake_underfoot = sp.danger.venomous && d < 0.9;
                if a.hostile.is_none()
                    && a.hunt.is_none()
                    && (a.weigh <= 0.0 || snake_underfoot)
                    && d < 40.0
                    && (a.wary.aware() || a.wary.how == Some(Sense::Startle) || snake_underfoot)
                {
                    let young = young_of
                        .get(&a.id)
                        .and_then(|y| {
                            y.iter()
                                .filter(|q| hdist(**q, a.pos) < 25.0)
                                .map(|q| hdist(*q, p.pos))
                                .min_by(f64::total_cmp)
                        })
                        .filter(|_| a.female && a.stage == Stage::Adult);
                    let rut = !a.female
                        && a.stage == Stage::Adult
                        && sp.rut.is_some_and(|s| s as usize == season);
                    // Brought up short by its tether, a kept one strains and stamps; it is not
                    // cornered by the keeper it knows.
                    let cornered = a.kept.is_none()
                        && a.act == Act::Flee
                        && a.repath > 0.0
                        && a.way.is_empty();
                    // A kept one knows its keeper and is the calmer the tamer it is; a young one
                    // does not turn on a person.
                    let calm = match &a.kept {
                        Some(k) => 0.3 * (1.0 - k.tame) * (1.0 - k.tame),
                        None if a.stage != Stage::Adult => 0.0,
                        None => 1.0,
                    };
                    let aggression = sp.danger.aggression * self.aggression * (1.0 - a.fear) * calm;
                    let roll = self.rng.next_f32();
                    let weighed = provoked(
                        sp,
                        p,
                        d,
                        a.wary.how == Some(Sense::Startle),
                        young,
                        a.kill_at.is_some(),
                        cornered,
                        rut,
                        a.hurt.lately(120.0),
                        lean,
                        aggression,
                        roll,
                    );
                    if weighed.is_some() {
                        // Weighed this encounter: not again for a while.
                        a.weigh = 15.0;
                    }
                    if let Some(Some(cause)) = weighed {
                        // A defender faced down is likelier to stop short.
                        let mut close = closes(sp, cause);
                        if matches!(
                            cause,
                            Cause::DefendingYoung | Cause::DefendingKill | Cause::Rut
                        ) && faced_down(p, a.pos)
                        {
                            close *= 0.3;
                        }
                        a.hostile = Some(Hostile {
                            cause,
                            contact: self.rng.next_f32() < close,
                            stalking: cause == Cause::Hunger,
                            t: 0.0,
                            cooldown: 0.0,
                        });
                        a.way.clear();
                        a.goal = None;
                        a.water = None;
                        log::info!("{} turns on the person: {}", sp.name, cause.words());
                    }
                }
                if a.hostile.is_some() {
                    let moved = charge(
                        a,
                        sp,
                        &walker,
                        ground,
                        p,
                        dt,
                        &mut self.rng,
                        &mut self.attacks,
                    );
                    a.stride += moved / sp.stride_m().max(0.05);
                    continue;
                }
            }
            // A hunt under way: stalking its prey, then the rush.
            if a.hunt.is_some() && matches!(a.medium, Medium::Ground | Medium::Water) {
                let moved = hunt(a, sp, &walker, ground, &whereabouts, dt, &mut killed);
                a.stride += moved / sp.stride_m().max(0.05);
                continue;
            }
            // What it does about it.
            if let Some(threat) = a.wary.threat.filter(|_| a.wary.watching()) {
                let d = hdist(a.pos, threat);
                let away = DVec2::new(a.pos.x - threat.x, a.pos.z - threat.z).normalize_or_zero();
                let away = if away == DVec2::ZERO { DVec2::X } else { away };
                // Kept, as tame as it is; a young one whose mother is gone does not run (it
                // stands and calls for her).
                let orphan = a.stage != Stage::Adult
                    && a.mother.is_none_or(|m| !whereabouts.contains_key(&m));
                let shy: f64 = match &a.kept {
                    Some(k) => k.shyness() as f64,
                    None if orphan => 0.0,
                    None => 1.0,
                };
                let flight = sp.flight_m() as f64 * (1.15 - 0.3 * sp.boldness as f64) * shy;
                let warned = a.wary.how == Some(Sense::Alarm);
                let runs = a.wary.aware() && (d < flight || (warned && d < flight * 2.5));
                let running = matches!(a.act, Act::Flee | Act::Fly);
                if runs && sp.freezes && !warned && !running && d > flight * 0.45 {
                    // One that hides keeps still until it is too close.
                    a.act = Act::Alert;
                    a.timer = a.timer.max(1.0);
                    a.goal = None;
                    a.way.clear();
                    a.water = None;
                } else if runs {
                    flee(
                        a,
                        mover,
                        &walker,
                        ground,
                        away,
                        flight,
                        &mut searches,
                        &mut self.rng,
                    );
                    a.water = None;
                    if !running
                        && matches!(a.act, Act::Flee | Act::Fly)
                        && let Some(g) = a.group
                    {
                        alarms.push((g, threat, a.pos));
                    }
                } else if !running && a.medium != Medium::Air {
                    // Watching: head up, toward it.
                    a.act = Act::Alert;
                    a.timer = a.timer.max(2.0);
                    a.goal = None;
                    a.way.clear();
                    a.water = None;
                    if a.climb.is_none() {
                        a.yaw = turn_toward(
                            a.yaw,
                            (threat.x - a.pos.x) as f32,
                            (threat.z - a.pos.z) as f32,
                            4.0 * dt,
                        );
                    }
                }
            }
            a.timer -= dt;
            if a.timer <= 0.0 {
                // A kept one grazes about its stake, and one that follows its keeper keeps by
                // them as a young one by its mother.
                let tether = a.kept.as_ref().and_then(|k| k.tether);
                let (centre, herd) = match tether {
                    Some((stake, _)) => (DVec2::new(stake.x, stake.z), 3.0),
                    None => a
                        .group
                        .and_then(|g| centres.get(&g))
                        .map(|(s, n)| (*s / *n, *n))
                        .unwrap_or((DVec2::new(a.pos.x, a.pos.z), 1.0)),
                };
                let follows = a.kept.as_ref().is_some_and(|k| k.follows);
                let mother = if follows {
                    presence.map(|p| p.pos)
                } else {
                    a.mother.and_then(|m| whereabouts.get(&m).copied())
                };
                a.kill_at = None;
                next_act(
                    a,
                    sp,
                    mover,
                    &walker,
                    ground,
                    awake(sp.activity, now.hour),
                    centre,
                    herd,
                    mother,
                    &quarry,
                    &cat,
                    &mut searches,
                    &mut self.rng,
                );
                if let (Some(h), Some(g)) = (a.hunt, a.group)
                    && matches!(
                        sp.social,
                        hearth_content::schema::fauna::Social::Pack { .. }
                    )
                {
                    packs.push((g, h.prey, a.pos));
                }
            }
            let moved = match a.medium {
                Medium::Air => fly(a, sp, dt, &mut self.rng),
                Medium::Tree => climb(a, sp, dt),
                _ => go(
                    a,
                    sp,
                    mover,
                    &walker,
                    ground,
                    dt,
                    &mut searches,
                    &mut self.rng,
                ),
            };
            a.stride += moved / sp.stride_m().max(0.05);
            // A tethered one goes no further than its tether lets it.
            if let Some((stake, reach)) = a.kept.as_ref().and_then(|k| k.tether) {
                let off = DVec2::new(a.pos.x - stake.x, a.pos.z - stake.z);
                if off.length() > reach {
                    let at = DVec2::new(stake.x, stake.z) + off.normalize() * reach;
                    let y = ground.footing(at.x, at.y, a.pos.y).map_or(a.pos.y, |f| f.y);
                    a.pos = DVec3::new(at.x, y, at.y);
                    // Brought up short (one running from a person strains at it).
                    a.goal = None;
                    a.way.clear();
                    a.speed = 0.0;
                }
            }
        }
        // The birds that left remains, gone.
        if !gone.is_empty() {
            self.animals.retain(|a| !gone.contains(&a.id));
        }
        // The kills: the prey dead where it fell, the hunter (and its pack about it) making a
        // meal of it.
        for (prey, predator, at) in killed {
            let hunter = &cat.species[predator as usize];
            let feeders = 1 + self
                .animals
                .iter()
                .filter(|a| {
                    !a.dead
                        && a.species == predator
                        && a.stage != Stage::Young
                        && hdist(a.pos, at) < 40.0
                })
                .count()
                .saturating_sub(1);
            if let Some(v) = self.animals.iter_mut().find(|v| v.id == prey && !v.dead) {
                if let Some(c) = call_for(&cat.species[v.species as usize], CallWhen::Distress) {
                    self.calls.push(Called {
                        species: v.species,
                        call: c,
                        pos: v.pos,
                    });
                }
                v.dead = true;
                v.speed = 0.0;
                v.hunt = None;
                v.act = Act::Dead;
                v.killed_by = Some(predator);
                // A gorge: twice a day's need.
                v.eaten = hunter.need_kg * 2.0 * feeders as f32;
                self.kills.push(Kill {
                    predator,
                    prey: v.species,
                    at,
                    cause: Cause::Hunger,
                });
                log::info!(
                    "{} killed a {}: hunger",
                    cat.species[predator as usize].name,
                    cat.species[v.species as usize].name
                );
            }
        }
        // A pack hunts together: the others join the first of it to hunt.
        for (g, prey, from) in packs {
            for a in self.animals.iter_mut().filter(|a| {
                a.group == Some(g) && !a.dead && a.hunt.is_none() && a.hostile.is_none()
            }) {
                if hdist(a.pos, from) < 100.0 {
                    a.hunt = Some(Hunt {
                        prey,
                        t: 0.0,
                        rushing: false,
                    });
                    a.act = Act::Walk;
                    a.timer = 120.0;
                }
            }
        }
        self.leave_signs(&cat, ground, presence, dt);
        self.make_calls(&cat, now, dt);
        // Warned: the herd runs with the first of it to run.
        for (g, threat, from) in alarms {
            for a in self
                .animals
                .iter_mut()
                .filter(|a| a.group == Some(g) && !a.dead)
            {
                if hdist(a.pos, from) < 120.0 && !matches!(a.act, Act::Flee | Act::Fly) {
                    a.wary.alarm(threat);
                }
            }
        }
    }

    /// The calls they make this step: the alarm of one that starts to run (half of them), the
    /// threat of one that turns on the person, and those of habit while their occasion holds (a
    /// stag in the rut, a herd's contact, a pack's howl taken up by the others about the first,
    /// an owl's hoot, a bird's song).
    fn make_calls(&mut self, cat: &Catalog, now: &Now, dt: f32) {
        // Calls come as the animals live, second by second, whatever the calendar's pace.
        let hour_s = 3600.0;
        let season = {
            let f = if now.southern {
                (now.year_frac + 0.5).rem_euclid(1.0)
            } else {
                now.year_frac
            };
            ((f * 4.0).floor() as usize).min(3)
        };
        let mut out: Vec<Called> = Vec::new();
        let mut packs: Vec<(u64, u16, u8, DVec3)> = Vec::new();
        let rng = &mut self.rng;
        for a in self.animals.iter_mut() {
            if a.dead {
                continue;
            }
            let sp = &cat.species[a.species as usize];
            let was = std::mem::replace(&mut a.was, a.act);
            if sp.calls.is_empty() {
                continue;
            }
            let mut call = |c: Option<u8>| {
                if let Some(c) = c {
                    out.push(Called {
                        species: a.species,
                        call: c,
                        pos: a.pos,
                    });
                }
            };
            // A bird over remains calls the others to them as it circles (a raven's find is
            // heard a kilometre off), now and then as it feeds, and in alarm rising from them.
            if let Some(at) = a.attend
                && !a.hurt.wounded()
            {
                use crate::flock::Doing;
                if at.flock.risen && a.act == Act::Fly && matches!(was, Act::Graze | Act::Alert) {
                    call(call_for(sp, CallWhen::Alarm));
                    continue;
                }
                let p = match at.doing {
                    Doing::Circling => dt / 9.0,
                    Doing::Feeding => dt / 30.0,
                    _ => 0.0,
                };
                if rng.next_f32() < p {
                    call(call_for(sp, CallWhen::Contact));
                }
                continue;
            }
            if a.act == Act::Flee && was != Act::Flee && rng.next_f32() < 0.5 {
                call(call_for(sp, CallWhen::Alarm));
                continue;
            }
            if a.hostile.is_some_and(|h| h.t <= dt) {
                call(call_for(sp, CallWhen::Threat));
                continue;
            }
            if matches!(a.act, Act::Flee | Act::Attack | Act::Sleep) || a.hurt.wounded() {
                continue;
            }
            let male_adult = !a.female && a.stage == Stage::Adult;
            for when in [
                CallWhen::Rut,
                CallWhen::Contact,
                CallWhen::Territory,
                CallWhen::Dawn,
                CallWhen::Night,
            ] {
                let Some(c) = call_for(sp, when) else {
                    continue;
                };
                let p = calls_now(sp, when, male_adult, season, now) * dt / hour_s;
                if rng.next_f32() < p {
                    call(Some(c));
                    if when == CallWhen::Territory
                        && let Some(g) = a.group
                    {
                        packs.push((g, a.species, c, a.pos));
                    }
                    break;
                }
            }
        }
        // A pack takes up the howl of the first of it.
        for (g, species, c, from) in packs {
            for a in self
                .animals
                .iter()
                .filter(|a| a.group == Some(g) && !a.dead && hdist(a.pos, from) < 200.0)
            {
                if a.pos != from {
                    out.push(Called {
                        species,
                        call: c,
                        pos: a.pos,
                    });
                }
            }
        }
        self.calls.extend(out);
    }

    /// The signs the animals about the person leave this step: a print at each stride where
    /// the ground takes one, drops of blood where the wounded go (the more, the faster they
    /// bleed), droppings now and then (a plant-eater's a dozen times a day, a hunter's a couple);
    /// those faded with age go.
    fn leave_signs(
        &mut self,
        cat: &Catalog,
        ground: &dyn Ground,
        presence: Option<&Presence>,
        dt: f32,
    ) {
        let day_s = DAY_S;
        let clock = self.clock;
        let mut left: Vec<Sign> = Vec::new();
        let rng = &mut self.rng;
        for a in self.animals.iter_mut() {
            if a.dead || a.medium != Medium::Ground {
                a.printed = a.stride;
                continue;
            }
            if presence.is_none_or(|p| hdist(a.pos, p.pos) > SIGNS_M) {
                a.printed = a.stride;
                continue;
            }
            let sp = &cat.species[a.species as usize];
            let ahead = DVec3::new(a.yaw.sin() as f64, 0.0, a.yaw.cos() as f64);
            let side = DVec3::new(ahead.z, 0.0, -ahead.x);
            // On the surface (lying snow over the ground its feet find).
            let sign = |kind, pos: DVec3, plain| Sign {
                kind,
                species: a.species,
                pos: DVec3::new(pos.x, ground.sign_surface(pos.x, pos.z, pos.y).1, pos.z),
                yaw: a.yaw,
                t: clock,
                plain,
            };
            if a.stride.floor() > a.printed.floor() && sp.track.is_some() {
                let plain = ground.sign_surface(a.pos.x, a.pos.z, a.pos.y).0;
                if plain >= 0.5 {
                    // The feet fall either side of its line, the hind in the fore's print.
                    let lr = if (a.stride.floor() as i64) % 2 == 0 {
                        1.0
                    } else {
                        -1.0
                    };
                    let off = side * lr * (sp.shoulder_m as f64 * 0.12).clamp(0.03, 0.2);
                    left.push(sign(SignKind::Print, a.pos + off, plain));
                }
            }
            a.printed = a.stride;
            let bleed = a.hurt.bleeding + a.hurt.clotting;
            if bleed > 0.0005 {
                a.drip += a.speed.max(0.3) * dt;
                let every = 0.6 / (1.0 + 200.0 * bleed);
                while a.drip >= every {
                    a.drip -= every;
                    let j = DVec3::new(rng.next_f64() - 0.5, 0.0, rng.next_f64() - 0.5) * 0.2;
                    left.push(sign(SignKind::Blood, a.pos + j, 1.0));
                }
            }
            let per_day = if sp.hunts() { 2.0 } else { 12.0 };
            if rng.next_f64() < per_day * dt as f64 / day_s {
                let at = a.pos - ahead * (sp.length_m as f64 * 0.45);
                left.push(sign(SignKind::Droppings, at, 1.0));
            }
        }
        self.signs.extend(left);
        // The old fade away; the oldest go when there are too many.
        self.signs.retain(|s| {
            let lasts = s.kind.lasts_days() * if s.plain >= 0.95 { 2.0 } else { 1.0 };
            clock - s.t < lasts * day_s
        });
        if self.signs.len() > MAX_SIGNS {
            let over = self.signs.len() - MAX_SIGNS;
            self.signs.drain(..over);
        }
    }

    /// The animals as the client draws them.
    pub fn views(&self) -> Vec<AnimalView> {
        self.animals
            .iter()
            .filter(|a| !a.dead)
            .map(|a| AnimalView {
                id: a.id,
                species: a.species,
                stage: a.stage,
                female: a.female,
                pos: a.pos,
                yaw: a.yaw,
                speed: a.speed,
                act: a.act,
                stride: a.stride,
                medium: a.medium,
                wounded: a.hurt.wounded(),
                kept: a.kept.is_some(),
                fleece: a.kept.as_ref().map_or(0.0, |k| {
                    let grown = match (k.plucked, self.years) {
                        (Some(p), Some(y)) => ((y - p) as f32).clamp(0.0, 1.0),
                        _ => 1.0,
                    };
                    k.breed.wool * grown
                }),
                tame: a.kept.as_ref().map_or(0.0, |k| k.tame),
            })
            .collect()
    }

    /// Kills an animal (hunted, taken by a predator).
    pub fn kill(&mut self, id: u64) -> Option<&Animal> {
        let a = self.animals.iter_mut().find(|a| a.id == id)?;
        a.dead = true;
        a.speed = 0.0;
        a.act = Act::Dead;
        Some(a)
    }

    /// Where a path (points in the world, in order) first strikes an animal's body, at the
    /// time of year `year_frac` (a young one's size).
    pub fn hit_along(&self, cat: &Catalog, path: &[DVec3], year_frac: f32) -> Option<Hit> {
        for (i, seg) in path.windows(2).enumerate() {
            let (p0, p1) = (seg[0], seg[1]);
            let mid = (p0 + p1) * 0.5;
            let half = (p1 - p0).length() * 0.5;
            let mut best: Option<Hit> = None;
            for a in self.animals.iter().filter(|a| !a.dead) {
                let sp = &cat.species[a.species as usize];
                if (a.pos - mid).length() > half + sp.length_m.max(sp.shoulder_m) as f64 * 2.0 + 1.0
                {
                    continue;
                }
                let rig = Rig::of(sp, !a.female);
                let scale = scale_of(
                    &rig,
                    a.stage,
                    year_frac,
                    sp.life.birth_frac,
                    sp.life.birth_mass_kg,
                );
                if let Some((t, part)) = strikes(&rig, scale, a.pos, a.yaw, p0, p1)
                    && best.is_none_or(|b| t < b.t)
                {
                    best = Some(Hit {
                        animal: a.id,
                        segment: i,
                        t,
                        at: p0 + (p1 - p0) * t as f64,
                        part,
                    });
                }
            }
            if best.is_some() {
                return best;
            }
        }
        None
    }

    /// A blow of `what` (a weapon's name) from a person at `from` where a hit struck: the
    /// wound it makes, the animal aware of the person (and running, or turning on them, as it
    /// is); in words.
    pub fn strike(
        &mut self,
        cat: &Catalog,
        hit: &Hit,
        blow: &Blow,
        what: &str,
        from: DVec3,
        year_frac: f32,
    ) -> Option<Struck> {
        let roll = self.rng.next_f32();
        let a = self
            .animals
            .iter_mut()
            .find(|a| a.id == hit.animal && !a.dead)?;
        let sp = &cat.species[a.species as usize];
        let rig = Rig::of(sp, !a.female);
        let scale = scale_of(
            &rig,
            a.stage,
            year_frac,
            sp.life.birth_frac,
            sp.life.birth_mass_kg,
        );
        let mass = rig.mass * scale * scale * scale;
        let w = wound(&rig, scale, mass, hit.part, blow, roll);
        w.add_to(&mut a.hurt);
        // Shoved as the blow's momentum says: a hare sent tumbling, a deer barely moved.
        let shove = DVec2::new(blow.push.x, blow.push.z) / mass.max(0.05) as f64;
        a.knock += shove.clamp_length_max(6.0);
        a.wary.alarm(from);
        a.fear = (a.fear + 0.5).min(1.0);
        let name = sp.name.to_lowercase();
        if !w.killed
            && let Some(c) = call_for(sp, CallWhen::Distress)
        {
            self.calls.push(Called {
                species: a.species,
                call: c,
                pos: a.pos,
            });
        }
        let glancing =
            !w.killed && !w.deep && w.stunned <= 0.0 && w.bleeding + w.clotting + w.lame <= 0.0;
        let words = if w.killed {
            a.dead = true;
            a.speed = 0.0;
            a.act = Act::Dead;
            a.hostile = None;
            format!(
                "The {what} strikes the {name} {}: it falls dead.",
                hit.part.words()
            )
        } else if w.deep && blow.cutting > blow.piercing {
            format!("The {what} cuts the {name} {}, deep.", hit.part.words())
        } else if w.deep {
            format!("The {what} strikes the {name} {}, deep.", hit.part.words())
        } else if w.stunned > 0.0 {
            format!(
                "The {what} strikes the {name} {}: it is stunned.",
                hit.part.words()
            )
        } else if w.bleeding + w.clotting + w.lame > 0.0 {
            format!("The {what} strikes the {name} {}.", hit.part.words())
        } else {
            format!("The {what} glances off the {name}.")
        };
        log::info!("{words}");
        Some(Struck {
            species: a.species,
            part: hit.part,
            deep: w.deep,
            killed: w.killed,
            glancing,
            words,
        })
    }

    /// Takes the dead out of the world, to lie as carcasses: each with what is left of it after
    /// what was eaten.
    pub fn take_bodies(&mut self, cat: &Catalog) -> Vec<Body> {
        let mut out = Vec::new();
        self.animals.retain(|a| {
            if !a.dead {
                return true;
            }
            let sp = &cat.species[a.species as usize];
            let grown = crate::rig::mass_of(sp, !a.female);
            let mass = match a.stage {
                Stage::Adult => grown,
                _ => grown * hearth_content::butchery::YOUNG_SHARE,
            };
            out.push(Body {
                species: a.species,
                stage: a.stage,
                female: a.female,
                pos: a.pos,
                yaw: a.yaw,
                // Of the flesh: about three fifths of the body.
                left: (1.0 - a.eaten / (mass * 0.6).max(1e-6)).clamp(0.05, 1.0),
                killed_by: a.killed_by,
                by_person: a.killed_by.is_none() && a.hurt.since.is_some(),
            });
            false
        });
        out
    }

    /// Puts an animal of a species into the world (tests, screenshots): not of any group or
    /// cell, so it is gone when it folds.
    pub fn place(&mut self, species: u16, stage: Stage, female: bool, pos: DVec3, yaw: f32) -> u64 {
        let id = self.id();
        self.animals.push(Animal::new(
            id,
            species,
            None,
            None,
            stage,
            female,
            pos,
            yaw,
            5.0,
            Medium::Ground,
        ));
        id
    }

    /// A bird come to remains (Amendment T §2.1): bird `index` of `flock`, of species `sp`,
    /// circling on its ring.
    pub fn attend(&mut self, sp: &crate::species::Species, flock: crate::flock::Flock, index: u8) {
        let (attend, pos, yaw) = crate::flock::attending(flock, index, sp, self.world_s);
        let id = self.id();
        let mut a = Animal::new(
            id,
            flock.species,
            None,
            None,
            Stage::Adult,
            index.is_multiple_of(2),
            pos,
            yaw,
            5.0,
            Medium::Air,
        );
        a.act = Act::Fly;
        a.attend = Some(attend);
        self.animals.push(a);
    }
}

/// A kept animal as a save keeps it (V2-12): kept animals belong to no group or cell, so they
/// are kept apart from the populations' numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeptSaved {
    /// Its id when saved (its young's mother is found by it).
    pub id: u64,
    pub species: String,
    pub stage: Stage,
    pub female: bool,
    pub pos: [f64; 3],
    pub yaw: f32,
    pub mother: Option<u64>,
    pub kept: Kept,
}

/// Why a kept animal will not do what is asked, in words.
pub type Refused = String;

impl Live {
    /// The kept animal with an id (living).
    fn kept_mut(&mut self, id: u64) -> Result<&mut Animal, Refused> {
        let a = self
            .animals
            .iter_mut()
            .find(|a| a.id == id && !a.dead)
            .ok_or_else(|| "It is gone.".to_owned())?;
        if a.kept.is_none() {
            return Err("It is not yours: it is wild.".into());
        }
        Ok(a)
    }

    /// Whether an animal is a kept one, and living (or why not, in words).
    pub fn is_kept(&mut self, id: u64) -> Result<(), Refused> {
        self.kept_mut(id).map(|_| ())
    }

    /// A wild young one taken in hand to keep (V2-12): raised by hand, it follows its keeper. A
    /// grown wild one will not be held. Only weighed, unless `commit`.
    pub fn catch(
        &mut self,
        cat: &Catalog,
        id: u64,
        years: f64,
        commit: bool,
    ) -> Result<String, Refused> {
        let a = self
            .animals
            .iter_mut()
            .find(|a| a.id == id && !a.dead)
            .ok_or_else(|| "It is gone.".to_owned())?;
        let sp = &cat.species[a.species as usize];
        if a.kept.is_some() {
            return Err("It is already yours.".into());
        }
        if sp.domestication.is_none() {
            return Err(format!(
                "The {} will not be kept: it is not a kind that takes to people.",
                sp.name.to_lowercase()
            ));
        }
        if a.stage == Stage::Adult {
            return Err(format!(
                "The grown {} twists free: only a young one can be raised by hand.",
                sp.name.to_lowercase()
            ));
        }
        if matches!(a.act, Act::Flee) {
            return Err("It is off before your hands close on it.".into());
        }
        let words = format!(
            "You have the young {}: it will follow you now.",
            sp.name.to_lowercase()
        );
        if !commit {
            return Ok(words);
        }
        // Its age: a young one of the year was born at its kind's birth season.
        let age = if a.stage == Stage::Young { 0.2 } else { 0.8 };
        a.kept = Some(Kept::caught(
            crate::herd::Breed::wild(&mut self.rng),
            years - age,
        ));
        a.group = None;
        a.cell = None;
        a.mother = None;
        a.hostile = None;
        a.hunt = None;
        a.goal = None;
        a.way.clear();
        a.act = Act::Alert;
        a.timer = 2.0;
        Ok(words)
    }

    /// A kept animal tethered to a stake where it stands.
    pub fn tether(&mut self, id: u64, reach: f64) -> Result<DVec3, Refused> {
        let a = self.kept_mut(id)?;
        let at = a.pos;
        if let Some(k) = a.kept.as_mut() {
            k.tether = Some((at, reach));
            k.follows = false;
        }
        a.mother = None;
        a.goal = None;
        a.way.clear();
        Ok(at)
    }

    /// A kept animal led off on a halter: untethered, it follows its keeper. Where its stake
    /// was, if it was tethered. Only weighed, unless `commit`.
    pub fn lead(&mut self, id: u64, commit: bool) -> Result<Option<DVec3>, Refused> {
        let a = self.kept_mut(id)?;
        let k = a.kept.as_mut().expect("kept");
        if k.tame < 0.3 {
            return Err("It pulls back, wild-eyed, and will not be led.".into());
        }
        if !commit {
            return Ok(k.tether.map(|(s, _)| s));
        }
        let stake = k.tether.take().map(|(s, _)| s);
        k.follows = true;
        Ok(stake)
    }

    /// Milking a kept mother in milk (once a day): the milk (kg). Only weighed, unless
    /// `commit`.
    pub fn milk(
        &mut self,
        cat: &Catalog,
        id: u64,
        years: f64,
        day_years: f64,
        commit: bool,
    ) -> Result<f32, Refused> {
        let a = self.kept_mut(id)?;
        let sp = &cat.species[a.species as usize];
        let k = a.kept.as_mut().expect("kept");
        if !a.female || a.stage != Stage::Adult {
            return Err("Only a mother in milk gives milk.".into());
        }
        if k.in_milk(years).is_none() {
            return Err("She is dry: she has no young at foot.".into());
        }
        if k.tame < 0.5 {
            return Err("She kicks and will not stand to be milked.".into());
        }
        if k.milked.is_some_and(|m| years - m < day_years * 0.75) {
            return Err("She has been milked today.".into());
        }
        let kg = crate::herd::milk_kg(sp, k, years);
        if kg <= 0.0 {
            return Err("There is nothing to spare from her young.".into());
        }
        if commit {
            k.milked = Some(years);
        }
        Ok(kg)
    }

    /// Plucking a kept animal's fleece as it moults (or shearing it): the wool (kg). Only
    /// weighed, unless `commit`.
    pub fn pluck(
        &mut self,
        cat: &Catalog,
        id: u64,
        years: f64,
        commit: bool,
    ) -> Result<f32, Refused> {
        let a = self.kept_mut(id)?;
        let sp = &cat.species[a.species as usize];
        let k = a.kept.as_mut().expect("kept");
        if k.tame < 0.3 {
            return Err("It will not stand to be handled.".into());
        }
        let kg = crate::herd::fleece_kg(sp, k, years);
        if kg < 0.02 {
            return Err("There is no fleece on it to take.".into());
        }
        if commit {
            k.plucked = Some(years);
        }
        Ok(kg)
    }

    /// A kept animal killed for its meat: it lies dead, to be butchered.
    pub fn slaughter(&mut self, id: u64) -> Result<(), Refused> {
        let a = self.kept_mut(id)?;
        a.dead = true;
        a.speed = 0.0;
        a.act = Act::Dead;
        a.hostile = None;
        // Dead of what a person did.
        a.hurt.since = Some(0.0);
        Ok(())
    }

    /// The kept animals through the calendar to `years` (the world's years; `year_frac` the
    /// time of year, 0 at the March equinox): grown older and used to their keeping, in young
    /// after the rut where a male of their kind is near, giving birth in their season, dying of
    /// age.
    pub fn tend(
        &mut self,
        cat: &Catalog,
        years: f64,
        year_frac: f32,
        southern: bool,
    ) -> Vec<Tiding> {
        let Some(last) = self.years.replace(years) else {
            return Vec::new();
        };
        let dt = years - last;
        if dt <= 0.0 {
            return Vec::new();
        }
        let season = {
            let f = if southern {
                (year_frac + 0.5).rem_euclid(1.0)
            } else {
                year_frac
            };
            ((f * 4.0).floor() as usize).min(3)
        };
        let year = years.floor() as i64;
        // The grown males in the rut, where they are.
        let males: Vec<(u16, DVec3, crate::herd::Breed, u16)> = self
            .animals
            .iter()
            .filter(|a| !a.dead && !a.female && a.stage == Stage::Adult)
            .filter_map(|a| a.kept.as_ref().map(|k| (a, k)))
            .filter(|(a, _)| crate::herd::in_rut(&cat.species[a.species as usize], season))
            .map(|(a, k)| (a.species, a.pos, k.breed, k.generation))
            .collect();
        let mut tidings = Vec::new();
        let mut born: Vec<Animal> = Vec::new();
        for a in self.animals.iter_mut().filter(|a| !a.dead) {
            let Some(k) = a.kept.as_mut() else {
                continue;
            };
            let sp = &cat.species[a.species as usize];
            let age = years - k.born;
            let stage = crate::herd::stage_at(sp, k, age);
            if stage != a.stage {
                a.stage = stage;
                if stage != Stage::Young {
                    a.mother = None;
                }
            }
            crate::herd::settle(k, stage, dt);
            // A tethered one is watered by its keeper.
            if k.tether.is_some() {
                a.thirst = 0.0;
            }
            if age > sp.life.lifespan_years as f64 {
                a.dead = true;
                a.act = Act::Dead;
                a.speed = 0.0;
                tidings.push(Tiding::Died {
                    species: a.species,
                    at: a.pos,
                });
                continue;
            }
            // In young after the rut, beside a male of her kind.
            if a.female
                && stage == Stage::Adult
                && k.carrying.is_none()
                && k.rut_year != Some(year)
                && crate::herd::in_rut(sp, season)
                && let Some(m) = males
                    .iter()
                    .find(|m| m.0 == a.species && crate::herd::near_mate(m.1, a.pos))
            {
                k.rut_year = Some(year);
                if self.rng.next_f32() < crate::herd::conceives(sp, k) {
                    k.carrying = Some(crate::herd::Carrying {
                        sire: m.2,
                        sire_generation: m.3,
                        due: crate::herd::due_after(sp, years, year_frac),
                    });
                }
            }
            // Her young born in their season.
            if let Some(c) = k.carrying
                && years >= c.due
            {
                k.carrying = None;
                k.birth = Some(years);
                let (lo, hi) = sp.life.litter;
                let n = (lo + self.rng.next_f32() * (hi - lo + 1.0))
                    .floor()
                    .clamp(lo, hi) as u32;
                for i in 0..n.max(1) {
                    let ang = (i as f64 + 0.5) * 2.1;
                    let pos = a.pos + DVec3::new(ang.cos() * 0.8, 0.0, ang.sin() * 0.8);
                    let mut y = Animal::new(
                        0,
                        a.species,
                        None,
                        None,
                        Stage::Young,
                        self.rng.next_f32() < 0.5,
                        pos,
                        self.rng.next_f32() * std::f32::consts::TAU,
                        3.0,
                        Medium::Ground,
                    );
                    y.mother = Some(a.id);
                    y.kept = Some(Kept::born_of(k, &c, years, &mut self.rng));
                    born.push(y);
                }
                tidings.push(Tiding::Born {
                    species: a.species,
                    mother: a.id,
                    young: n.max(1),
                    at: a.pos,
                });
            }
        }
        for mut y in born {
            y.id = self.id();
            self.animals.push(y);
        }
        tidings
    }

    /// The kept animals, for a save.
    pub fn kept_saved(&self, cat: &Catalog) -> Vec<KeptSaved> {
        self.animals
            .iter()
            .filter(|a| !a.dead)
            .filter_map(|a| {
                a.kept.as_ref().map(|k| KeptSaved {
                    id: a.id,
                    species: cat.species[a.species as usize].id.clone(),
                    stage: a.stage,
                    female: a.female,
                    pos: a.pos.to_array(),
                    yaw: a.yaw,
                    mother: a.mother,
                    kept: k.clone(),
                })
            })
            .collect()
    }

    /// The kept animals of a save back in the world (their kinds by id; one of a kind no
    /// longer known is lost).
    pub fn restore_kept(&mut self, cat: &Catalog, saved: Vec<KeptSaved>) {
        let mut ids: FxHashMap<u64, u64> = FxHashMap::default();
        let mut made = Vec::new();
        for s in saved {
            let Some(species) = cat.index(&s.species) else {
                log::warn!("a kept {} is lost: its kind is no longer known", s.species);
                continue;
            };
            let id = self.id();
            ids.insert(s.id, id);
            let mut a = Animal::new(
                id,
                species as u16,
                None,
                None,
                s.stage,
                s.female,
                DVec3::from_array(s.pos),
                s.yaw,
                2.0,
                Medium::Ground,
            );
            a.mother = s.mother;
            a.kept = Some(s.kept);
            made.push(a);
        }
        for a in made.iter_mut() {
            a.mother = a.mother.and_then(|m| ids.get(&m).copied());
        }
        self.animals.extend(made);
    }
}

/// Turns a facing toward the direction (dx, dz) by at most `max` radians.
fn turn_toward(yaw: f32, dx: f32, dz: f32, max: f32) -> f32 {
    if dx == 0.0 && dz == 0.0 {
        return yaw;
    }
    let want = dx.atan2(dz);
    let mut d = (want - yaw).rem_euclid(std::f32::consts::TAU);
    if d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    yaw + d.clamp(-max, max)
}

/// A way to a goal for an animal, if it may look for one this step; it goes straight
/// otherwise (and looks again later).
fn way_to(
    a: &mut Animal,
    walker: &Walker,
    ground: &dyn Ground,
    goal: DVec2,
    budget: usize,
    searches: &mut usize,
) {
    a.goal = Some(goal);
    a.way.clear();
    if *searches > 0 {
        *searches -= 1;
        a.way = find_way(ground, a.pos, goal, walker, budget).points;
    }
}

/// Away from a threat: a bird takes wing for a tree or farther ground; a climber makes for a
/// tree and up it; a fish darts off; the rest run, along a way found over the ground.
#[allow(clippy::too_many_arguments)]
fn flee(
    a: &mut Animal,
    mover: Mover,
    walker: &Walker,
    ground: &dyn Ground,
    away: DVec2,
    flight: f64,
    searches: &mut usize,
    rng: &mut Rng,
) {
    let here = DVec2::new(a.pos.x, a.pos.z);
    let fresh = !matches!(a.act, Act::Flee | Act::Fly);
    match mover {
        Mover::Bird => {
            if a.medium != Medium::Air {
                take_off(a, ground, here + away * (flight * 2.0 + 15.0), true);
            }
        }
        Mover::Climber if a.medium == Medium::Tree => {
            // Safe up the tree: it watches.
            a.act = Act::Alert;
            a.timer = a.timer.max(8.0);
            if let Some(c) = &mut a.climb {
                c.to = c.to.max(a.pos.y);
            }
        }
        Mover::Climber if fresh || (a.climb.is_none() && a.way.is_empty() && a.repath <= 0.0) => {
            match trunk_near(ground, a.pos, 12.0) {
                Some((foot, height)) => {
                    let to = foot.y + (height * 0.7).clamp(2.0, 9.0) * (0.6 + 0.4 * rng.next_f64());
                    a.climb = Some(Climb { trunk: foot, to });
                    way_to(
                        a,
                        walker,
                        ground,
                        DVec2::new(foot.x, foot.z),
                        FLEE_BUDGET,
                        searches,
                    );
                    a.act = Act::Flee;
                    a.timer = 30.0;
                }
                None => run_from(a, walker, ground, here + away * (flight * 1.5), searches),
            }
        }
        Mover::Fish if fresh => {
            run_from(a, walker, ground, here + away * 6.0, searches);
            a.timer = 3.0;
        }
        Mover::Walker if fresh || (a.way.is_empty() && a.repath <= 0.0) => {
            run_from(a, walker, ground, here + away * (flight * 1.5), searches);
        }
        _ => {}
    }
}

fn run_from(
    a: &mut Animal,
    walker: &Walker,
    ground: &dyn Ground,
    goal: DVec2,
    searches: &mut usize,
) {
    way_to(a, walker, ground, goal, FLEE_BUDGET, searches);
    a.act = Act::Flee;
    a.timer = 8.0;
}

/// Into the air for a perch near `toward` (when `perch`) or the ground there.
fn take_off(a: &mut Animal, ground: &dyn Ground, toward: DVec2, perch: bool) {
    let dest = if perch {
        perch_near(ground, toward, 18.0).map(|p| (p, true))
    } else {
        None
    };
    let dest = dest.or_else(|| {
        ground
            .top(toward.x, toward.y)
            .filter(|f| !f.water)
            .map(|f| (DVec3::new(toward.x, f.y, toward.y), false))
    });
    let Some((to, perch)) = dest else {
        return;
    };
    a.flying = Some(Flying {
        flight: Flight::plan(ground, a.pos, to),
        flown: 0.0,
        perch,
    });
    a.medium = Medium::Air;
    a.act = Act::Fly;
    a.timer = 60.0;
    a.way.clear();
    a.goal = None;
    a.climb = None;
}

/// What an animal does next when its act is done: of what it might do, what suits it most
/// now — asleep out of its hours; a young one after its mother; to water when thirsty; grazing
/// (in a herd, looking up now and then, the less as the herd is bigger); grooming; wandering
/// near the middle of its group.
#[allow(clippy::too_many_arguments)]
fn next_act(
    a: &mut Animal,
    sp: &Species,
    mover: Mover,
    walker: &Walker,
    ground: &dyn Ground,
    up: bool,
    centre: DVec2,
    herd: f64,
    mother: Option<DVec3>,
    quarry: &[(u64, u16, DVec3)],
    cat: &crate::species::Catalog,
    searches: &mut usize,
    rng: &mut Rng,
) {
    let r = rng.next_f32();
    let here = DVec2::new(a.pos.x, a.pos.z);
    let near = |rng: &mut Rng, reach: f64| {
        let ang = rng.next_f64() * std::f64::consts::TAU;
        here + DVec2::new(ang.cos(), ang.sin()) * reach * (0.3 + 0.7 * rng.next_f64())
    };
    match (mover, a.medium) {
        (_, Medium::Air) => a.timer = 1.0,
        // Down from the tree again.
        (Mover::Climber, Medium::Tree) => {
            if let Some(c) = &mut a.climb {
                c.to = c.trunk.y;
            }
            a.act = Act::Walk;
            a.timer = 20.0;
        }
        (Mover::Bird, Medium::Tree) => {
            if !up {
                a.act = Act::Sleep;
                a.timer = 30.0 + r * 60.0;
            } else if r < 0.3 {
                // Down to forage.
                take_off(a, ground, near(rng, 15.0), false);
            } else {
                a.act = if r < 0.6 { Act::Alert } else { Act::Groom };
                a.timer = 5.0 + r * 10.0;
            }
        }
        (Mover::Bird, _) => {
            if !up {
                // To roost in a tree.
                take_off(a, ground, here, true);
                if a.medium != Medium::Air {
                    a.act = Act::Sleep;
                    a.timer = 30.0;
                }
            } else if r < 0.5 {
                a.act = Act::Graze;
                a.timer = 3.0 + r * 8.0;
            } else if r < 0.62 {
                take_off(a, ground, near(rng, 20.0), true);
            } else {
                // Hopping or walking a few steps.
                way_to(a, walker, ground, near(rng, 3.0), 200, searches);
                a.act = Act::Walk;
                a.timer = 6.0;
            }
        }
        (Mover::Fish, _) => {
            if r < 0.5 {
                a.act = Act::Graze;
                a.timer = 3.0 + r * 6.0;
            } else {
                way_to(a, walker, ground, near(rng, 8.0), 400, searches);
                a.act = Act::Walk;
                a.timer = 15.0;
            }
        }
        _ => {
            if !up {
                // Out of its hours: asleep mostly, now and then awake where it lies.
                a.act = if r < 0.75 { Act::Sleep } else { Act::Rest };
                a.timer = 20.0 + r * 40.0;
                a.goal = None;
                a.way.clear();
                return;
            }
            // What it might do, each as much as it suits it now (its habits' weights from its
            // species), a little at random.
            let h = sp.habits;
            let reach = 6.0 + sp.mass_kg.sqrt() as f64;
            let off_centre = (here.distance(centre) / reach) as f32;
            let behind = mother.map_or(0.0, |m| hdist(m, a.pos) as f32);
            let herding = herd > 1.5;
            let choices = [
                (Choice::Graze, 1.0),
                (
                    Choice::Look,
                    if herding {
                        0.45 * h.vigilance / (herd as f32).sqrt()
                    } else {
                        0.15 * h.vigilance
                    },
                ),
                (Choice::Groom, 0.12 * h.grooming),
                (
                    Choice::Wander,
                    0.45 * h.roaming + h.sociability * off_centre * off_centre,
                ),
                (
                    Choice::Drink,
                    if a.thirst > 0.6 && mover != Mover::Fish {
                        a.thirst * 1.5
                    } else {
                        0.0
                    },
                ),
                (
                    Choice::Follow,
                    if behind > 4.0 {
                        2.0 + behind * 0.1
                    } else {
                        0.0
                    },
                ),
                // A hunter goes after what it hunts when there is any about.
                (
                    Choice::Hunt,
                    if sp.hunts() && !sp.prey.is_empty() {
                        0.7 * h.roaming
                    } else {
                        0.0
                    },
                ),
            ];
            let choice = choices
                .iter()
                .map(|(c, u)| (*c, u * (0.6 + 0.8 * rng.next_f32())))
                .fold(
                    (Choice::Graze, f32::MIN),
                    |b, c| if c.1 > b.1 { c } else { b },
                )
                .0;
            match choice {
                Choice::Hunt => {
                    // The nearest of its prey within a hundred and fifty metres.
                    let target = quarry
                        .iter()
                        .filter(|(id, s, _)| {
                            *id != a.id
                                && sp.prey.iter().any(|(i, _)| *i == *s as usize)
                                && !cat.species[*s as usize].hunts()
                        })
                        .map(|(id, _, at)| (*id, hdist(*at, a.pos)))
                        .filter(|(_, d)| *d < 150.0)
                        .min_by(|x, y| x.1.total_cmp(&y.1));
                    match target {
                        Some((prey, _)) => {
                            a.hunt = Some(Hunt {
                                prey,
                                t: 0.0,
                                rushing: false,
                            });
                            a.act = Act::Walk;
                            a.timer = 120.0;
                            a.goal = None;
                            a.way.clear();
                        }
                        None => {
                            a.act = Act::Graze;
                            a.timer = 4.0;
                        }
                    }
                }
                Choice::Follow => {
                    let m = mother.unwrap_or(a.pos);
                    let goal = DVec2::new(
                        m.x + rng.next_f64() * 2.0 - 1.0,
                        m.z + rng.next_f64() * 2.0 - 1.0,
                    );
                    way_to(a, walker, ground, goal, 400, searches);
                    a.act = Act::Walk;
                    a.timer = 6.0;
                }
                Choice::Drink => match water_near(ground, a.pos, 120.0) {
                    Some((bank, water)) => {
                        way_to(
                            a,
                            walker,
                            ground,
                            DVec2::new(bank.x, bank.z),
                            WALK_BUDGET,
                            searches,
                        );
                        a.water = Some(water);
                        a.act = Act::Walk;
                        a.timer = 90.0;
                    }
                    None => {
                        // None near: it lives on the water in its food.
                        a.thirst = 0.3;
                        a.act = Act::Graze;
                        a.timer = 4.0;
                    }
                },
                Choice::Graze => {
                    a.act = Act::Graze;
                    a.timer = 4.0 + r * 12.0;
                    a.goal = None;
                    a.way.clear();
                }
                Choice::Look => {
                    // Head up from feeding to watch a while.
                    a.act = Act::Alert;
                    a.timer = 1.5 + rng.next_f32();
                    a.goal = None;
                    a.way.clear();
                }
                Choice::Groom => {
                    a.act = Act::Groom;
                    a.timer = 3.0 + r * 4.0;
                    a.goal = None;
                    a.way.clear();
                }
                Choice::Wander => {
                    // Somewhere near the group's middle.
                    let ang = rng.next_f64() * std::f64::consts::TAU;
                    let goal = centre + DVec2::new(ang.cos(), ang.sin()) * reach * rng.next_f64();
                    way_to(a, walker, ground, goal, WALK_BUDGET, searches);
                    a.act = Act::Walk;
                    a.timer = 20.0;
                }
            }
        }
    }
}

/// What an animal at its ease may do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Graze,
    Look,
    Groom,
    Wander,
    Drink,
    Follow,
    Hunt,
}

/// Along the way to the goal over the ground or through the water: a step at the speed of
/// what it does, if the ground there takes it. The distance gone.
#[allow(clippy::too_many_arguments)]
fn go(
    a: &mut Animal,
    sp: &Species,
    mover: Mover,
    walker: &Walker,
    ground: &dyn Ground,
    dt: f32,
    searches: &mut usize,
    rng: &mut Rng,
) -> f32 {
    // A blow's shove: it gives ground where the ground takes it.
    if a.knock != DVec2::ZERO {
        let step = a.knock * dt as f64;
        match ground
            .footing(a.pos.x + step.x, a.pos.z + step.y, a.pos.y)
            .filter(|f| walker.step(a.pos.y, f, step.length()).is_some())
        {
            Some(f) => a.pos = DVec3::new(a.pos.x + step.x, f.y, a.pos.z + step.y),
            None => a.knock = DVec2::ZERO,
        }
        a.knock *= (-dt as f64 / 0.15).exp();
        if a.knock.length() < 0.05 {
            a.knock = DVec2::ZERO;
        }
    }
    let swimming = a.medium == Medium::Water;
    let target = match (a.act, mover) {
        (Act::Walk, Mover::Fish) => sp.swim_m_s.unwrap_or(0.5) * 0.4,
        (Act::Flee, Mover::Fish) => sp.swim_m_s.unwrap_or(1.0),
        (Act::Walk, _) if swimming => sp.swim_m_s.unwrap_or(0.5) * 0.7,
        (Act::Flee, _) if swimming => sp.swim_m_s.unwrap_or(0.5),
        (Act::Walk, _) => sp.walk_speed(),
        // The young of the year cannot run as their parents do.
        (Act::Flee, _) if a.stage == Stage::Young => sp.run_speed() * 0.7,
        (Act::Flee, _) => sp.run_speed(),
        _ => 0.0,
    };
    // Wounded, it goes the slower.
    let target = target * a.hurt.vigour();
    let Some(goal) = a.goal.filter(|_| target > 0.0) else {
        a.speed *= (-6.0 * dt).exp();
        return 0.0;
    };
    // A way, when it has none and may look again.
    if a.way.is_empty() && a.repath <= 0.0 && *searches > 0 && a.act == Act::Flee {
        let here = DVec2::new(a.pos.x, a.pos.z);
        if here.distance(goal) > 2.0 {
            way_to(a, walker, ground, goal, FLEE_BUDGET, searches);
        }
    }
    let next = a.way.first().map_or(goal, |p| DVec2::new(p.x, p.z));
    let to = next - DVec2::new(a.pos.x, a.pos.z);
    // At a trunk it is there on reaching its side.
    let there = if a.climb.is_some() && a.way.is_empty() {
        1.3
    } else {
        0.6
    };
    if to.length() < there {
        if !a.way.is_empty() {
            a.way.remove(0);
            return 0.0;
        }
        // There.
        a.goal = None;
        if let Some(w) = a.water.take() {
            // At the water: a drink.
            a.yaw = yaw_toward(a.pos, w);
            a.act = Act::Drink;
            a.timer = 8.0 + rng.next_f32() * 10.0;
            a.thirst = 0.0;
            a.speed = 0.0;
            return 0.0;
        }
        if let Some(c) = a.climb {
            // At the tree: up it.
            let side = DVec2::new(a.pos.x - c.trunk.x, a.pos.z - c.trunk.z).normalize_or(DVec2::X);
            let off = 0.5 + sp.shoulder_m as f64 * 0.3;
            a.pos.x = c.trunk.x + side.x * off;
            a.pos.z = c.trunk.z + side.y * off;
            a.medium = Medium::Tree;
            a.timer = 15.0 + rng.next_f32() * 20.0;
            return 0.0;
        }
        a.act = if a.act == Act::Flee {
            Act::Alert
        } else {
            Act::Graze
        };
        a.timer = 3.0 + rng.next_f32() * 6.0;
        return 0.0;
    }
    a.yaw = turn_toward(a.yaw, to.x as f32, to.y as f32, 5.0 * dt);
    a.speed = accelerate(sp, a.speed, target, dt);
    let dir = DVec2::new(a.yaw.sin() as f64, a.yaw.cos() as f64);
    let step = dir * (a.speed * dt) as f64;
    let (nx, nz) = (a.pos.x + step.x, a.pos.z + step.y);
    let level = if swimming {
        ground
            .footing(a.pos.x, a.pos.z, a.pos.y)
            .map_or(a.pos.y, |f| Walker::level(&f))
    } else {
        a.pos.y
    };
    let next = ground
        .footing(nx, nz, level)
        .filter(|f| walker.step(level, f, step.length()).is_some());
    match next {
        Some(f) => {
            let deep = f.water && (walker.fish || f.depth > walker.wade);
            a.medium = if deep { Medium::Water } else { Medium::Ground };
            a.pos = DVec3::new(
                nx,
                if walker.fish {
                    f.level() - swim_depth(sp, f.depth)
                } else if deep {
                    // Swimming: the back just out of the water (a young one's lower).
                    let size = match a.stage {
                        Stage::Adult => 1.0,
                        Stage::Juvenile => 0.82,
                        Stage::Young => 0.55,
                    };
                    f.level() - sp.shoulder_m as f64 * 0.85 * size
                } else {
                    f.y
                },
                nz,
            );
            step.length() as f32
        }
        None => {
            // That way is closed: look again in a moment, and turn meanwhile.
            a.way.clear();
            a.repath = 1.0;
            a.yaw += std::f32::consts::FRAC_PI_2 * if rng.next_f32() < 0.5 { 1.0 } else { -1.0 };
            if a.act != Act::Flee && a.climb.is_none() {
                a.goal = None;
                a.act = Act::Graze;
                a.timer = 1.0;
            }
            0.0
        }
    }
}

/// On along a flight; landing at its end. The distance flown.
fn fly(a: &mut Animal, sp: &Species, dt: f32, rng: &mut Rng) -> f32 {
    let Some(f) = a.flying else {
        a.medium = Medium::Ground;
        return 0.0;
    };
    let speed = sp.fly_m_s.unwrap_or(8.0);
    a.speed = speed;
    let flown = f.flown + (speed * dt) as f64;
    let p = f.flight.at(flown);
    let d = p - a.pos;
    if d.x.abs() + d.z.abs() > 1e-6 {
        a.yaw = turn_toward(a.yaw, d.x as f32, d.z as f32, 0.6);
    }
    a.pos = p;
    if flown >= f.flight.length() {
        a.flying = None;
        a.medium = if f.perch {
            Medium::Tree
        } else {
            Medium::Ground
        };
        a.act = Act::Alert;
        a.timer = 2.0 + rng.next_f32() * 4.0;
        a.speed = 0.0;
    } else {
        a.flying = Some(Flying { flown, ..f });
    }
    speed * dt
}

/// Up or down the trunk to where it climbs; on the ground again at its foot. The distance gone.
fn climb(a: &mut Animal, sp: &Species, dt: f32) -> f32 {
    let Some(c) = a.climb else {
        // A bird on its perch.
        a.speed *= (-6.0 * dt).exp();
        return 0.0;
    };
    let to = DVec2::new(c.trunk.x - a.pos.x, c.trunk.z - a.pos.z);
    a.yaw = turn_toward(a.yaw, to.x as f32, to.y as f32, 8.0 * dt);
    let dy = c.to - a.pos.y;
    let speed = (sp.walk_speed() * 0.8).max(0.3) as f64;
    if dy.abs() < 0.02 {
        a.speed = 0.0;
        if c.to <= c.trunk.y + 0.01 {
            // Down: away from the trunk a little, on the ground.
            a.medium = Medium::Ground;
            a.climb = None;
            a.act = Act::Graze;
            a.timer = 4.0;
        }
        return 0.0;
    }
    let step = (speed * dt as f64).min(dy.abs());
    a.pos.y += step * dy.signum();
    a.speed = speed as f32;
    step as f32
}

/// Straight toward a point at a speed over the ground (no way searched: a rush), as far as the
/// ground takes the step. The distance gone.
#[allow(clippy::too_many_arguments)]
fn steer(
    a: &mut Animal,
    sp: &Species,
    walker: &Walker,
    ground: &dyn Ground,
    to: DVec2,
    speed: f32,
    dt: f32,
) -> f32 {
    let d = to - DVec2::new(a.pos.x, a.pos.z);
    let speed = speed * a.hurt.vigour();
    if d.length() < 0.05 || speed <= 0.0 {
        a.speed *= (-6.0 * dt).exp();
        return 0.0;
    }
    a.yaw = turn_toward(a.yaw, d.x as f32, d.y as f32, 8.0 * dt);
    a.speed = accelerate(sp, a.speed, speed, dt);
    let dir = DVec2::new(a.yaw.sin() as f64, a.yaw.cos() as f64);
    let step = dir * (a.speed * dt).min(d.length() as f32) as f64;
    let (nx, nz) = (a.pos.x + step.x, a.pos.z + step.y);
    let level = a.pos.y;
    let Some(f) = ground
        .footing(nx, nz, level)
        .filter(|f| walker.step(level, f, step.length()).is_some())
    else {
        a.speed *= 0.5;
        return 0.0;
    };
    let deep = f.water && f.depth > walker.wade;
    a.medium = if deep { Medium::Water } else { Medium::Ground };
    a.pos = DVec3::new(
        nx,
        if deep {
            f.level() - sp.shoulder_m as f64 * 0.85
        } else {
            f.y
        },
        nz,
    );
    step.length() as f32
}

/// A charge at the person: a hunter stalking low and slow until near, then the rush; at reach, a
/// blow if it means to close (and, for a hunter, more after it), or a stop short (a bluff), and
/// then it goes. Fire keeps a hunter off, and facing it down — upright, facing it, loud —
/// turns most back (and it fears people more after); a person running from a hunter sets it
/// after them. The distance gone.
#[allow(clippy::too_many_arguments)]
fn charge(
    a: &mut Animal,
    sp: &Species,
    walker: &Walker,
    ground: &dyn Ground,
    p: &Presence,
    dt: f32,
    rng: &mut Rng,
    attacks: &mut Vec<Attack>,
) -> f32 {
    let Some(mut h) = a.hostile else {
        return 0.0;
    };
    h.t += dt;
    h.cooldown -= dt;
    let d = hdist(a.pos, p.pos);
    let defender = matches!(
        h.cause,
        Cause::DefendingYoung | Cause::DefendingKill | Cause::Rut
    );
    let deterred = match h.cause {
        Cause::Hunger => p.by_fire || faced_down(p, a.pos),
        _ if defender => faced_down(p, a.pos) && rng.next_f32() < dt * 0.5,
        _ => false,
    };
    let backed_off = defender && d > crate::danger::guard_m(sp) * 1.6;
    if deterred || backed_off || h.t > 30.0 || d > 60.0 {
        // Turned back.
        if deterred {
            a.fear = (a.fear + 0.3).min(1.0);
            log::info!("{} is turned back ({})", sp.name, h.cause.words());
            let why = if p.by_fire && h.cause == Cause::Hunger {
                "the fire"
            } else {
                "you facing it"
            };
            attacks.push(Attack {
                animal: a.id,
                species: a.species,
                cause: h.cause,
                injury: "",
                region: hearth_content::schema::body::BodyRegion::Chest,
                severity: 0.0,
                venom: false,
                words: format!(
                    "The {} stops and turns away from {why}: {}.",
                    sp.name.to_lowercase(),
                    h.cause.words()
                ),
            });
        }
        a.hostile = None;
        a.act = Act::Alert;
        a.timer = 4.0;
        return 0.0;
    }
    // Running from a hunter sets it after them.
    if p.running && sp.danger.predatory {
        h.contact = true;
        h.stalking = false;
    }
    if h.stalking && d < 12.0 {
        h.stalking = false;
    }
    let reach = sp.length_m as f64 * 0.6 + 0.7;
    let mut moved = 0.0;
    if h.contact && d <= reach {
        a.act = Act::Attack;
        a.speed = 0.0;
        a.yaw = yaw_toward(a.pos, p.pos);
        if h.cooldown <= 0.0 {
            let (injury, region, severity, words) = blow(sp, h.cause, rng.next_f32());
            attacks.push(Attack {
                animal: a.id,
                species: a.species,
                cause: h.cause,
                injury,
                region,
                severity,
                venom: sp.danger.venomous,
                words: format!("{words}: {}.", h.cause.words()),
            });
            log::info!(
                "{} attacks the person ({injury}): {}",
                sp.name,
                h.cause.words()
            );
            h.cooldown = 2.5;
            if h.cause != Cause::Hunger {
                // Its point made, it goes.
                a.hostile = None;
                a.act = Act::Alert;
                a.timer = 3.0;
                a.fear = (a.fear + 0.1).min(1.0);
                return 0.0;
            }
        }
    } else if !h.contact && d <= reach + 3.0 {
        // Stopping short: a bluff. It stands a moment, then goes.
        a.act = Act::Alert;
        a.speed *= (-8.0 * dt).exp();
        if h.cooldown <= -2.5 {
            attacks.push(Attack {
                animal: a.id,
                species: a.species,
                cause: h.cause,
                injury: "",
                region: hearth_content::schema::body::BodyRegion::Chest,
                severity: 0.0,
                venom: false,
                words: format!(
                    "The {} charges and stops short: {}.",
                    sp.name.to_lowercase(),
                    h.cause.words()
                ),
            });
            a.hostile = None;
            a.timer = 3.0;
            return 0.0;
        }
    } else {
        let speed = if h.stalking {
            sp.walk_speed() * 0.6
        } else {
            sp.run_speed()
        };
        a.act = if h.stalking { Act::Walk } else { Act::Flee };
        moved = steer(
            a,
            sp,
            walker,
            ground,
            DVec2::new(p.pos.x, p.pos.z),
            speed,
            dt,
        );
        h.cooldown = h.cooldown.max(0.0);
    }
    if a.hostile.is_some() {
        a.hostile = Some(h);
    }
    moved
}

/// A hunt: stalking toward the prey, low and slow, until near enough to rush it; the rush; the
/// kill within reach (the predator feeding at it after), or giving up when the rush goes on too
/// long or the prey is gone. The distance gone.
#[allow(clippy::too_many_arguments)]
fn hunt(
    a: &mut Animal,
    sp: &Species,
    walker: &Walker,
    ground: &dyn Ground,
    whereabouts: &FxHashMap<u64, DVec3>,
    dt: f32,
    killed: &mut Vec<(u64, u16, DVec3)>,
) -> f32 {
    let Some(mut h) = a.hunt else {
        return 0.0;
    };
    let Some(&at) = whereabouts.get(&h.prey) else {
        a.hunt = None;
        a.act = Act::Alert;
        a.timer = 3.0;
        return 0.0;
    };
    h.t += dt;
    let d = hdist(a.pos, at);
    // Cats rush from close by and briefly; dogs from farther and long.
    let cat = matches!(sp.social, hearth_content::schema::fauna::Social::Solitary)
        && sp.plan != hearth_content::schema::fauna::BodyPlan::Bear;
    let (rush_from, rush_for) = if cat { (7.0, 8.0) } else { (35.0, 40.0) };
    if !h.rushing && d < rush_from {
        h.rushing = true;
        h.t = 0.0;
    }
    if (h.rushing && h.t > rush_for) || (!h.rushing && h.t > 120.0) {
        // Given up.
        a.hunt = None;
        a.act = Act::Alert;
        a.timer = 5.0;
        return 0.0;
    }
    let reach = sp.length_m as f64 * 0.5 + 0.6;
    if d <= reach {
        killed.push((h.prey, a.species, at));
        a.hunt = None;
        a.kill_at = Some(at);
        a.act = Act::Graze;
        a.timer = 90.0 + sp.mass_kg.sqrt() * 5.0;
        a.speed = 0.0;
        return 0.0;
    }
    let speed = if h.rushing {
        sp.run_speed()
    } else {
        sp.walk_speed() * 0.6
    };
    a.act = if h.rushing { Act::Flee } else { Act::Walk };
    a.hunt = Some(h);
    steer(a, sp, walker, ground, DVec2::new(at.x, at.z), speed, dt)
}

/// Toward a speed as fast as a body gets up to speed or slows: six metres a second every second
/// for most, more for a hunter's spring and a small body's dart; slowing twice as fast.
fn accelerate(sp: &Species, speed: f32, target: f32, dt: f32) -> f32 {
    let mut up = 6.0;
    if sp.hunts() {
        up += 3.0;
    }
    if sp.mass_kg < 10.0 {
        up += 3.0;
    }
    speed + (target - speed).clamp(-2.0 * up * dt, up * dt)
}

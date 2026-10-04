//! Doing things in the world (V2-5, v2 §11): the server's side of the process engine.
//!
//! * **Work**: a process started on what the player looks at goes on, tick by tick on the
//!   world's clock, while they stay; done, its outcome is applied: what it used is taken from
//!   where it was, what it made is stowed (or put down), tools wear, the target is dug, picked or
//!   lit, and what the doing taught is heard by the player's knowledge.
//! * **Unattended work** (meat on the rack, acorns in a stream) waits where it was set up and
//!   goes on while its conditions hold.
//! * **Workstations** stand as blocks; hearths and lamps keep their fires
//!   (`hearth_craft::fire`): light, warmth for those beside them, a place to cook.
//! * **Natural fires**: lightning in a storm now and then sets a tree burning for hours.
//! * **Kills**: until animals live in the world (V2-7), a predator's kill turns up nearby every
//!   day or two to be scavenged; ravens circling show where.
//! * Things carried and lying **go off**, and embers burn down.
//! * **Digging**: loose earth dug out falls in a spoil pile beside the hole and slumps to its
//!   angle of repose (a block's step of two slides down).

use std::sync::Arc;

use glam::DVec3;
use hearth_body::BodyConfig;
use hearth_content::Content;
use hearth_content::schema::process::Effect;
use hearth_content::schema::station::Capability;
use hearth_content::time::TimeScales;
use hearth_content::triggers::{self, block_keys, item_keys, with_verb};
use hearth_craft::engine::{finish_batch, perform_by, plan};
use hearth_craft::food::{bite_of, decay_per_hour, keeps_days};
use hearth_craft::{
    Aimed, Bench, Crafts, Event, Fire, FireSeen, FireState, Fuel, Graph, Lack, Mode, Outcome, Plan,
    Source, Surroundings,
};
use hearth_env::Moment;
use hearth_items::{Hand, Items, Path, Root, Stack, WorldItems};
use hearth_math::BlockPos;
use hearth_math::hash::Rng;
use hearth_player::Player;
use hearth_protocol::{Acted, AimAt, DrinkFrom, ToClient, WorkView};
use hearth_world::BlockStateId;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use crate::environment::EnvSampler;
use crate::scene::LocalWorld;
use crate::wildfire::{Danger, FarFire, FireWorld, FuelTable, Plume, Wildfire};

/// How far things lying about are within reach of the work (m).
const REACH_M: f64 = 2.5;
/// A natural fire's id among the stations.
const WILDFIRE: &str = "wildfire";

/// A workstation standing in the world, with its fire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Station {
    pub pos: BlockPos,
    pub id: String,
    #[serde(default)]
    pub fire: Option<Fire>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Harvest {
    pos: BlockPos,
    process: String,
    year: i64,
    count: u8,
}

/// What `crafts.json` holds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WorkshopSave {
    #[serde(default)]
    stations: Vec<Station>,
    #[serde(default)]
    wildfires: Vec<Station>,
    #[serde(default)]
    harvests: Vec<Harvest>,
    #[serde(default)]
    depleted: Vec<(BlockPos, f32)>,
}

/// Work in hand.
#[derive(Debug, Clone, PartialEq)]
pub struct Work {
    pub recipe: usize,
    pub aim: AimAt,
    /// Game ticks done and needed.
    pub ticks: f64,
    pub needed: f64,
    /// Where the player stood when it began.
    pub at: DVec3,
    /// The quality the player's own hands reached, when they did it by hand.
    pub hand: Option<f32>,
}

/// The world the workshop acts in, for one call.
pub struct Here<'a> {
    pub lw: &'a mut LocalWorld,
    pub items: &'a Items,
    pub cfg: &'a BodyConfig,
    pub env: &'a EnvSampler,
    pub scales: &'a TimeScales,
    pub moment: Moment,
    pub ticks: u64,
    pub ticks_per_day: f64,
    pub player: &'a mut Player,
    pub world_items: &'a mut WorldItems,
    /// Blocks changed, for meshing and lighting.
    pub changed: &'a mut Vec<BlockPos>,
    pub out: &'a mut Vec<ToClient>,
    /// The things lying about changed.
    pub items_changed: &'a mut bool,
    /// Which way the person faces (radians: 0 toward +z, turning toward +x).
    pub facing: f32,
}

/// Making things in the world.
pub struct Workshop {
    pub crafts: Arc<Crafts>,
    pub graph: Arc<Graph>,
    pub mode: Mode,
    pub stations: Vec<Station>,
    pub wildfires: Vec<Station>,
    harvests: FxHashMap<(BlockPos, String), (i64, u8)>,
    depleted: FxHashMap<BlockPos, f32>,
    pub work: Option<Work>,
    rng: Rng,
    /// Triggers of sight lately heard, and when (they are not heard again for a while).
    heard_at: FxHashMap<String, u64>,
    /// The tick the world was last brought up to date to.
    last_update: u64,
    /// The player's knowledge changed since it was last sent.
    pub knowledge_changed: bool,
    /// When salt water was last tasted and spat out (drinking again at once swallows it).
    tasted_salt: Option<u64>,
    /// Trees falling: where they come to rest, and the tick they get there.
    falls: Vec<(u64, Vec<(BlockPos, BlockStateId)>)>,
    /// Fire in the vegetation near the player, and far away.
    blaze: Wildfire,
    far_fire: FarFire,
    /// What each block burns as (from the registry, on first need).
    fuel: Option<Arc<FuelTable>>,
    /// The fire danger at the fire, and the game minute it was found.
    danger: Option<(u64, Danger)>,
}

/// The near fire's view of the world: the loaded terrain; what burns is nature's change (the
/// player's own changes stay theirs, as burned), and workstations are spared.
struct HereFire<'a, 'b> {
    h: &'a mut Here<'b>,
    spared: &'a FxHashSet<BlockPos>,
}

impl FireWorld for HereFire<'_, '_> {
    fn block(&self, p: BlockPos) -> Option<BlockStateId> {
        self.h.lw.map.block(p)
    }

    fn set(&mut self, p: BlockPos, s: BlockStateId) {
        let reg = self.h.lw.reg.clone();
        self.h.lw.map.set_block(p, s, &reg);
        if self.h.lw.edits.get(p).is_some() {
            self.h.lw.edits.set(p, s);
        }
        self.h.changed.push(p);
    }

    fn spared(&self, p: BlockPos) -> bool {
        self.spared.contains(&p)
    }
}

/// An action's outcome in words, as the client is told it.
fn acted(process: &str, done: bool, words: impl Into<String>) -> ToClient {
    ToClient::Acted(Acted {
        process: process.to_owned(),
        done,
        words: words.into(),
    })
}

fn lack_words(l: &Lack) -> String {
    match l {
        Lack::Knowledge => "You do not know how.".into(),
        Lack::Target => "Not here.".into(),
        Lack::Tool(p) => format!("You need something in hand for {p}."),
        Lack::Input(w) => format!("You need {w}."),
        Lack::Condition(c) => format!("It needs {c}."),
    }
}

/// A tree turned a quarter about its stump to lie toward `toward`: each block's place, its state
/// turned with it (logs lie along the fall, limbs' joins turn), and whether it is a log.
fn fallen_tree(
    standing: &[(BlockPos, BlockStateId, hearth_flora::Part)],
    foot: BlockPos,
    toward: hearth_math::Direction,
    blocks: &hearth_worldgen::trees::SpeciesBlocks,
) -> Vec<(BlockPos, BlockStateId, bool)> {
    use hearth_flora::Part;
    use hearth_flora::template::{DOWN, EAST, NORTH, SOUTH, UP, WEST};
    // Along x or z, toward + or −.
    let (along_x, sign) = match toward {
        hearth_math::Direction::East => (true, 1),
        hearth_math::Direction::West => (true, -1),
        hearth_math::Direction::South => (false, 1),
        _ => (false, -1),
    };
    // Faces as the tree turns: up goes toward the fall, the fall's way goes down.
    let (fwd, back) = match toward {
        hearth_math::Direction::East => (EAST, WEST),
        hearth_math::Direction::West => (WEST, EAST),
        hearth_math::Direction::South => (SOUTH, NORTH),
        _ => (NORTH, SOUTH),
    };
    let turn_joins = |j: u8| {
        let mut n = j & !(UP | DOWN | fwd | back);
        if j & UP != 0 {
            n |= fwd;
        }
        if j & fwd != 0 {
            n |= DOWN;
        }
        if j & DOWN != 0 {
            n |= back;
        }
        if j & back != 0 {
            n |= UP;
        }
        n
    };
    standing
        .iter()
        .map(|(p, _, part)| {
            let (dx, dy, dz) = (p.x - foot.x, p.y - foot.y, p.z - foot.z);
            let (da, other) = if along_x { (dx, dz) } else { (dz, dx) };
            // About the stump's middle: (along, up) → (sign·up, −sign·along).
            let (na, ny) = (sign * dy, -sign * da);
            let q = if along_x {
                BlockPos::new(foot.x + na, foot.y + ny, foot.z + other)
            } else {
                BlockPos::new(foot.x + other, foot.y + ny, foot.z + na)
            };
            let turned = match *part {
                Part::Log { axis } => Part::Log {
                    axis: match (axis, along_x) {
                        (1, true) => 0,
                        (1, false) => 2,
                        (0, true) | (2, false) => 1,
                        (a, _) => a,
                    },
                },
                Part::Branch { thickness, joins } => Part::Branch {
                    thickness,
                    joins: turn_joins(joins),
                },
                Part::Leaves => Part::Leaves,
            };
            (q, blocks.state(turned), matches!(turned, Part::Log { .. }))
        })
        .collect()
}

/// How soon after tasting salt water drinking again swallows it (game ticks: some seconds).
const TASTE_TICKS: u64 = 200;

fn center(p: BlockPos) -> DVec3 {
    DVec3::new(p.x as f64 + 0.5, p.y as f64 + 0.5, p.z as f64 + 0.5)
}

impl Workshop {
    pub fn new(
        content: &Content,
        items: &Items,
        mode: Mode,
        save: Option<WorkshopSave>,
        seed: u64,
        ticks: u64,
    ) -> Self {
        let save = save.unwrap_or_default();
        Self {
            crafts: Arc::new(Crafts::from_content(content, items)),
            graph: Arc::new(Graph::from_content(content)),
            mode,
            stations: save.stations,
            wildfires: save.wildfires,
            harvests: save
                .harvests
                .into_iter()
                .map(|h| ((h.pos, h.process), (h.year, h.count)))
                .collect(),
            depleted: save.depleted.into_iter().collect(),
            work: None,
            rng: Rng::new(seed ^ 0xc4af7),
            heard_at: FxHashMap::default(),
            last_update: ticks,
            knowledge_changed: true,
            tasted_salt: None,
            falls: Vec::new(),
            blaze: Wildfire::new(seed),
            far_fire: FarFire::new(seed),
            fuel: None,
            danger: None,
        }
    }

    pub fn save(&self) -> WorkshopSave {
        let mut harvests: Vec<Harvest> = self
            .harvests
            .iter()
            .map(|((pos, process), (year, count))| Harvest {
                pos: *pos,
                process: process.clone(),
                year: *year,
                count: *count,
            })
            .collect();
        harvests.sort_by(|a, b| (a.pos, &a.process).cmp(&(b.pos, &b.process)));
        let mut depleted: Vec<(BlockPos, f32)> =
            self.depleted.iter().map(|(p, kg)| (*p, *kg)).collect();
        depleted.sort_by_key(|(p, _)| *p);
        WorkshopSave {
            stations: self.stations.clone(),
            wildfires: self.wildfires.clone(),
            harvests,
            depleted,
        }
    }

    pub fn station_at(&self, pos: BlockPos) -> Option<&Station> {
        self.stations.iter().find(|s| s.pos == pos)
    }

    fn fire_seen(f: &Fire) -> FireSeen {
        FireSeen {
            lit: f.lit(),
            temp_c: f.temp_c,
        }
    }

    /// What the player looks at, as a process sees it.
    pub fn aimed(&self, h: &Here, aim: AimAt) -> Option<Aimed> {
        let room = || {
            crate::building::spot(&h.lw.map, &h.lw.reg, aim).is_some_and(|at| {
                crate::building::room(&h.lw.map, &h.lw.reg, at, h.player.mover.pos)
            })
        };
        match aim {
            AimAt::Nothing => None,
            AimAt::Thing(id) => h.world_items.get(id).map(|_| Aimed::Thing(id)),
            // Beside a block, to put a piece up: the block as it is, whatever it is.
            AimAt::Beside { pos, .. } => {
                let block = h.lw.reg.block_of(h.lw.map.block(pos)?);
                Some(Aimed::Block {
                    name: block.name.to_string(),
                    material: block.def.material.clone(),
                    ground: false,
                    room: room(),
                })
            }
            AimAt::Block { pos, top } => {
                if let Some(st) = self.station_at(pos) {
                    return Some(Aimed::Station {
                        id: st.id.clone(),
                        fire: st.fire.as_ref().map(Self::fire_seen),
                    });
                }
                if let Some(w) = self.wildfires.iter().find(|w| w.pos == pos) {
                    return w.fire.as_ref().map(|f| Aimed::Fire(Self::fire_seen(f)));
                }
                let state = h.lw.map.block(pos)?;
                let reg = &h.lw.reg;
                let block = reg.block_of(state);
                if block.def.fluid.is_some() || reg.get(state, "waterlogged") == Some("true") {
                    return Some(Aimed::Water);
                }
                let solid = !reg.collision_shape(state).is_empty();
                let above = h.lw.map.block(pos.up());
                let open = above.is_none_or(|a| {
                    a.is_air()
                        || reg.block_of(a).def.replaceable && reg.block_of(a).def.fluid.is_none()
                });
                Some(Aimed::Block {
                    name: block.name.to_string(),
                    material: block.def.material.clone(),
                    ground: top && solid && open,
                    room: room(),
                })
            }
        }
    }

    /// Where the aim points, for things lying within reach of the work.
    fn aim_point(&self, h: &Here, aim: AimAt) -> DVec3 {
        match aim {
            AimAt::Block { pos, .. } | AimAt::Beside { pos, .. } => center(pos),
            AimAt::Thing(id) => h
                .world_items
                .get(id)
                .map_or(h.player.mover.pos, |w| DVec3::from_array(w.pos)),
            AimAt::Nothing => h.player.mover.pos,
        }
    }

    /// The weather and place at a point, as work there feels it.
    pub fn around(&self, h: &Here, at: DVec3) -> Surroundings {
        let w = h.env.weather_at(&h.moment, at);
        let (x, z) = (at.x.floor() as i32, at.z.floor() as i32);
        let covered =
            h.lw.map
                .sky_top(x, z)
                .is_some_and(|top| top as f64 > at.y + 1.0);
        let mut water_near = false;
        let base = BlockPos::containing(at);
        'scan: for dy in -2..=1 {
            for dz in -2..=2 {
                for dx in -2..=2 {
                    let p = BlockPos::new(base.x + dx, base.y + dy, base.z + dz);
                    if h.lw
                        .map
                        .block(p)
                        .is_some_and(|s| h.lw.reg.block_of(s).def.fluid.is_some())
                    {
                        water_near = true;
                        break 'scan;
                    }
                }
            }
        }
        let mut near = Vec::new();
        if self
            .wildfires
            .iter()
            .any(|f| f.fire.as_ref().is_some_and(Fire::lit) && (center(f.pos) - at).length() < 30.0)
        {
            near.push(WILDFIRE.to_owned());
        }
        let southern = h.env.planet.latitude(at.z) < 0.0;
        Surroundings {
            raining: w.precip_mm_h > 0.1 && !covered,
            humidity: w.humidity as f32,
            daylight: h.env.sun_up(&h.moment, at),
            air_c: w.temperature_c as f32,
            sheltered: covered,
            water_near,
            near,
            season: Some(h.moment.season(southern)),
        }
    }

    /// The skill a recipe practises, and how practised it is.
    fn skill_of(&self, h: &Here, r: usize) -> (Option<String>, f32) {
        let def = &self.crafts.recipes[r].def;
        let name = def.skill.clone().or_else(|| {
            def.knowledge
                .as_ref()
                .and_then(|k| self.graph.node(k.as_str()))
                .and_then(|n| n.skill.clone())
        });
        let level = name.as_deref().map_or(0.0, |n| h.player.knowledge.skill(n));
        (name, level)
    }

    fn may_attempt(&self, h: &Here, r: usize) -> bool {
        self.mode == Mode::Open
            || h.player.knowledge.may_attempt(
                self.crafts.recipes[r]
                    .def
                    .knowledge
                    .as_ref()
                    .map(|k| k.as_str()),
            )
    }

    /// Plans recipe `r` on what is aimed at, with what is at hand now.
    fn plan_now(&self, h: &Here, r: usize, aim: AimAt) -> Result<Plan, Lack> {
        let aimed = self.aimed(h, aim);
        let at = self.aim_point(h, aim);
        let around = self.around(h, at);
        let feet = h.player.mover.pos;
        let lying = h.world_items.items.iter().filter(|w| {
            let p = DVec3::from_array(w.pos);
            w.work.is_none() && ((p - feet).length() < REACH_M || (p - at).length() < 1.5)
        });
        let bench = Bench::new(
            &h.lw.content,
            h.items,
            &h.player.carry,
            lying.map(|w| (w.id, &w.stack)),
            aimed,
            around,
        );
        let (_, skill) = self.skill_of(h, r);
        plan(&self.crafts, r, &bench, skill)
    }

    /// How many times a block has yielded a process this year.
    fn harvested(&self, pos: BlockPos, process: &str, year: i64) -> u8 {
        match self.harvests.get(&(pos, process.to_owned())) {
            Some((y, n)) if *y == year => *n,
            _ => 0,
        }
    }

    /// Starts the player doing a process.
    pub fn act(&mut self, h: &mut Here, process: &str, aim: AimAt, hand: Option<f32>) {
        let Some(r) = self.crafts.index_of(process) else {
            return;
        };
        if !h.player.can_act(h.cfg) || h.player.asleep {
            h.out.push(acted(process, false, "You cannot now."));
            return;
        }
        if !self.may_attempt(h, r) {
            h.out.push(acted(process, false, "You do not know how."));
            return;
        }
        let def = &self.crafts.recipes[r].def;
        if let (Some(limit), AimAt::Block { pos, .. }) = (def.harvests, aim)
            && self.harvested(pos, process, h.moment.year) >= limit
        {
            h.out.push(acted(
                process,
                false,
                "There is no more to take here this year.",
            ));
            return;
        }
        if let Some(pos) = aim.block()
            && (center(pos) - (h.player.mover.pos + DVec3::new(0.0, 1.6, 0.0))).length() > 4.0
        {
            h.out.push(acted(process, false, "Out of reach."));
            return;
        }
        let p = match self.plan_now(h, r, aim) {
            Ok(p) => p,
            Err(l) => {
                h.out.push(acted(process, false, lack_words(&l)));
                return;
            }
        };
        if let Some(why) = self.piece_lacks(h, r, aim) {
            h.out.push(acted(process, false, why));
            return;
        }
        let def = &self.crafts.recipes[r].def;
        if !def.attended {
            self.set_up(h, r, &p, aim);
            return;
        }
        // Felling takes time by the trunk's section (the process's time is for 30 cm); only a
        // standing tree is felled and only a lying trunk is cut into sections.
        let mut section = 1.0;
        match (def.effect, aim) {
            (Effect::Fell, AimAt::Block { pos, .. }) => match self.standing_tree(h, pos) {
                Some(t) => {
                    section = (t.template.diameter_m as f64 / 0.3)
                        .powi(2)
                        .clamp(0.15, 40.0)
                }
                None => {
                    h.out
                        .push(acted(process, false, "That is no standing tree to fell."));
                    return;
                }
            },
            (Effect::Buck, AimAt::Block { pos, .. }) => {
                let upright =
                    h.lw.map
                        .block(pos)
                        .is_some_and(|s| h.lw.reg.get(s, "axis").is_none_or(|a| a == "y"));
                if upright {
                    h.out.push(acted(process, false, "Fell it first."));
                    return;
                }
            }
            _ => {}
        }
        let needed = h.scales.play_seconds(p.hours as f64, p.scale)
            * hearth_content::time::TICKS_PER_SECOND
            * section;
        self.work = Some(Work {
            recipe: r,
            aim,
            ticks: 0.0,
            needed: needed.max(1.0),
            at: h.player.mover.pos,
            hand: hand.map(|q| q.clamp(0.0, 1.0)),
        });
        h.out
            .push(ToClient::Work(Some(self.work_view(0.0, needed / 20.0))));
    }

    fn work_view(&self, done: f32, play_s_left: f64) -> WorkView {
        let r = self.work.as_ref().map_or(0, |w| w.recipe);
        let def = &self.crafts.recipes[r].def;
        WorkView {
            process: def.id.clone(),
            action: def.action.clone(),
            done,
            play_s_left,
        }
    }

    /// Stops the work in hand.
    pub fn stop(&mut self, h: &mut Here) {
        if self.work.take().is_some() {
            h.out.push(ToClient::Work(None));
        }
    }

    /// How much faster the world goes while the player works at a long task: up to twenty
    /// times, so that any work takes at most a minute or so of waiting.
    pub fn work_warp(&self) -> f64 {
        match &self.work {
            Some(w) => ((w.needed - w.ticks) / 20.0 / 30.0).clamp(0.0, 19.0),
            None => 0.0,
        }
    }

    /// The METs of the work in hand, if any.
    pub fn work_mets(&self) -> Option<f32> {
        self.work
            .as_ref()
            .map(|w| self.crafts.recipes[w.recipe].def.mets)
    }

    /// Lives the world on by `advanced` game ticks: the work in hand, fires, unattended work,
    /// things going off, storms and kills.
    pub fn tick(&mut self, h: &mut Here, advanced: f64) {
        if let Some(w) = &mut self.work {
            let moved = (h.player.mover.pos - w.at).length() > 1.2;
            if h.player.body.dead.is_some() || h.player.asleep || h.player.lying || moved {
                self.work = None;
                h.out.push(ToClient::Work(None));
                if moved {
                    h.out
                        .push(acted("", false, "You leave the work unfinished."));
                }
            } else {
                w.ticks += advanced;
                if w.ticks >= w.needed {
                    let w = self.work.take().expect("work");
                    h.out.push(ToClient::Work(None));
                    self.finish(h, w.recipe, w.aim, w.hand);
                } else {
                    let (done, left) = (w.ticks / w.needed, (w.needed - w.ticks) / 20.0);
                    h.out
                        .push(ToClient::Work(Some(self.work_view(done as f32, left))));
                }
            }
        }
        // Falling trees come to rest.
        if self.falls.iter().any(|(at, _)| h.ticks >= *at) {
            let (done, going): (Vec<_>, Vec<_>) = std::mem::take(&mut self.falls)
                .into_iter()
                .partition(|(at, _)| h.ticks >= *at);
            self.falls = going;
            for (_, blocks) in done {
                for (p, st) in blocks {
                    if Self::crushed(h, p) || Self::foliage(h, p) {
                        self.set_block(h, p, st);
                    }
                }
            }
        }
        // Fire in the vegetation.
        if self.blaze.is_burning() {
            self.fire_step(h);
        }
        // The world, a game minute at a time.
        let minute = (h.ticks_per_day / 1440.0).max(1.0);
        while (h.ticks.saturating_sub(self.last_update)) as f64 >= minute {
            self.last_update += minute as u64;
            self.minute(h, 1.0 / 60.0);
        }
    }

    /// Finishes attended work: plans it again with what is at hand now and does it.
    fn finish(&mut self, h: &mut Here, r: usize, aim: AimAt, hand: Option<f32>) {
        let id = self.crafts.recipes[r].def.id.clone();
        let p = match self.plan_now(h, r, aim) {
            Ok(p) => p,
            Err(l) => {
                h.out.push(acted(&id, false, lack_words(&l)));
                return;
            }
        };
        if let Some(why) = self.piece_lacks(h, r, aim) {
            h.out.push(acted(&id, false, why));
            return;
        }
        let (skill_name, skill) = self.skill_of(h, r);
        let outcome = {
            let aimed = self.aimed(h, aim);
            let at = self.aim_point(h, aim);
            let around = self.around(h, at);
            let feet = h.player.mover.pos;
            let lying = h.world_items.items.iter().filter(|w| {
                let p = DVec3::from_array(w.pos);
                w.work.is_none() && ((p - feet).length() < REACH_M || (p - at).length() < 1.5)
            });
            let bench = Bench::new(
                &h.lw.content,
                h.items,
                &h.player.carry,
                lying.map(|w| (w.id, &w.stack)),
                aimed,
                around,
            );
            perform_by(&self.crafts, &p, &bench, skill, &mut self.rng, hand)
        };
        self.apply(h, r, &p, outcome, aim, skill_name);
    }

    /// Takes `n` of the thing at a source; what was taken.
    fn take(h: &mut Here, s: &Source, n: u16) -> Option<Stack> {
        match s {
            Source::Carried(path) => h.player.carry.take(h.items, path, Some(n)),
            Source::Lying(id) => {
                *h.items_changed = true;
                let w = h.world_items.get_mut(*id)?;
                if w.stack.count > n {
                    w.stack.count -= n;
                    let mut part = w.stack.clone();
                    part.count = n;
                    part.inside = None;
                    Some(part)
                } else {
                    h.world_items.take(*id).map(|w| w.stack)
                }
            }
        }
    }

    fn stack_mut<'a>(h: &'a mut Here, s: &Source) -> Option<&'a mut Stack> {
        match s {
            Source::Carried(path) => h.player.carry.get_mut(path),
            Source::Lying(id) => h.world_items.get_mut(*id).map(|w| &mut w.stack),
        }
    }

    /// Takes everything a plan used, deepest first so that places in containers keep their
    /// numbers; what was taken.
    fn take_all(h: &mut Here, used: &[(Source, u16)]) -> Vec<Stack> {
        let mut order: Vec<&(Source, u16)> = used.iter().collect();
        order.sort_by(|a, b| match (&a.0, &b.0) {
            (Source::Carried(x), Source::Carried(y)) => {
                (y.inside.len(), &y.inside).cmp(&(x.inside.len(), &x.inside))
            }
            _ => std::cmp::Ordering::Equal,
        });
        order
            .into_iter()
            .filter_map(|(s, n)| Self::take(h, s, *n))
            .collect()
    }

    /// Hears triggers: discoveries and hunches are told.
    fn hear(&mut self, h: &mut Here, triggers: &[String]) {
        for t in triggers {
            let events = h
                .player
                .knowledge
                .observe(&self.graph, t, h.ticks, self.mode);
            self.tell(h, events);
        }
        self.knowledge_changed = true;
    }

    fn tell(&mut self, h: &mut Here, events: Vec<Event>) {
        for e in events {
            let (id, discovered) = match e {
                Event::Learned(id) => (id, true),
                Event::Hunch(id) => (id, false),
            };
            let name = self
                .graph
                .node(&id)
                .map_or_else(|| id.clone(), |n| n.name.clone());
            let text = h
                .player
                .knowledge
                .journal
                .iter()
                .rev()
                .find(|n| n.node.as_deref() == Some(id.as_str()))
                .map_or_else(String::new, |n| n.text.clone());
            h.out.push(ToClient::Learned {
                name,
                discovered,
                text,
            });
            if discovered {
                // A new technique may suggest others.
                let more = h.player.knowledge.inferences(&self.graph, Some(&id));
                for t in more {
                    let ev = h
                        .player
                        .knowledge
                        .observe(&self.graph, &t, h.ticks, self.mode);
                    self.tell(h, ev);
                }
            }
        }
        self.knowledge_changed = true;
    }

    /// Hears a trigger of sight, once in a while.
    /// Shown how by one who knows (V2.1 §11.2): insight toward a node (or what it rests on), and
    /// what comes of it.
    pub(crate) fn taught(&mut self, h: &mut Here, node: &str, insight: f32) {
        let events = h
            .player
            .knowledge
            .taught(&self.graph, node, insight, h.ticks);
        self.tell(h, events);
        self.knowledge_changed = true;
    }

    pub(crate) fn hear_now_and_then(&mut self, h: &mut Here, trigger: String, every_ticks: u64) {
        let fresh = self
            .heard_at
            .get(&trigger)
            .is_none_or(|&t| h.ticks.saturating_sub(t) >= every_ticks);
        if fresh {
            self.heard_at.insert(trigger.clone(), h.ticks);
            self.hear(h, &[trigger]);
        }
    }

    /// Puts what was made into the player's hands and containers, or down at their feet.
    fn give(h: &mut Here, mut stacks: Vec<Stack>) {
        let body_kg = h.cfg.mass_kg as f32;
        for s in stacks.iter_mut() {
            if let Some(g) = h.items.get(&s.id).and_then(|k| k.property("glow_h")) {
                s.glow_h = g;
            }
        }
        for s in stacks {
            if let Err(s) = h.player.carry.stow(h.items, s, body_kg) {
                let at = h.player.mover.pos + DVec3::new(0.3, 0.2, 0.3);
                h.world_items.add(s, at.to_array(), 0.0);
                *h.items_changed = true;
            }
        }
    }

    /// Applies a process's outcome to the world and the player.
    fn apply(
        &mut self,
        h: &mut Here,
        r: usize,
        p: &Plan,
        o: Outcome,
        aim: AimAt,
        skill_name: Option<String>,
    ) {
        let def = self.crafts.recipes[r].def.clone();
        let builds = self.crafts.recipes[r].builds.clone();
        // Practice, whatever came of it; and what it taught.
        if let Some((s, hours)) = &o.practice {
            h.player.knowledge.practice(s, *hours, h.ticks);
        } else if let Some(s) = &skill_name {
            h.player.knowledge.practice(s, p.hours, h.ticks);
        }
        if let Some(k) = &def.knowledge {
            let ev = h
                .player
                .knowledge
                .practised(&self.graph, k.as_str(), h.ticks);
            self.tell(h, ev);
        }
        self.hear(h, &o.triggers);
        if let Some(k) = &def.knowledge {
            let more = h.player.knowledge.inferences(&self.graph, Some(k.as_str()));
            self.hear(h, &more);
        }
        // Tools wear; one worn through breaks.
        let mut broken = Vec::new();
        for (s, wear) in &o.wear {
            if let Some(stack) = Self::stack_mut(h, s) {
                stack.condition = (stack.condition - wear).max(0.0);
                if stack.condition <= 0.0 {
                    broken.push(s.clone());
                }
            }
        }
        for s in broken {
            if let Some(gone) = Self::take(h, &s, 1) {
                let name = h
                    .items
                    .get(&gone.id)
                    .map_or(gone.id.clone(), |k| k.name.clone());
                h.out
                    .push(acted(&def.id, false, format!("Your {name} is worn out.")));
            }
        }
        if let Some(injury) = &o.injury {
            use hearth_content::schema::body::BodyRegion;
            let side = if self.rng.chance(0.5) {
                hearth_body::Side::Left
            } else {
                hearth_body::Side::Right
            };
            h.player
                .body
                .injure(h.cfg, injury, BodyRegion::Hand, side, 0.25);
        }
        let taken = Self::take_all(h, &o.used);
        if !o.done {
            if !o.made.is_empty() {
                Self::give(h, o.made);
            }
            let words = o
                .failure
                .clone()
                .unwrap_or_else(|| "It does not work.".into());
            h.out.push(acted(&def.id, false, words));
            return;
        }
        // A poultice or dressing on the newest injury that has not had one.
        if let Some(t) = def.treats.clone() {
            match h.player.body.untreated(&t) {
                Some(i) => h.player.body.treat_injury(i, &t),
                None => h
                    .out
                    .push(acted(&def.id, false, "There is no wound to treat.")),
            }
        }
        // What it does to its target.
        match (o.effect, aim) {
            (Effect::Remove, AimAt::Block { pos, .. }) => {
                self.set_block(h, pos, BlockStateId::AIR);
                self.slump_around(h, pos);
            }
            (Effect::Excavate, AimAt::Block { pos, .. }) => self.excavate(h, pos),
            (Effect::Deplete, AimAt::Block { pos, .. }) => {
                let made_kg: f32 = o.made.iter().map(|s| s.mass(h.items)).sum();
                let kg = self.depleted.entry(pos).or_default();
                *kg += made_kg;
                let block_kg =
                    h.lw.map
                        .block(pos)
                        .and_then(|s| h.lw.reg.block_of(s).def.material.clone())
                        .and_then(|m| h.lw.content.materials.get(&m).map(|m| m.density_kg_m3))
                        .unwrap_or(1500.0);
                if *kg >= block_kg {
                    self.depleted.remove(&pos);
                    self.set_block(h, pos, BlockStateId::AIR);
                    self.slump_around(h, pos);
                }
            }
            (Effect::Ignite, AimAt::Block { pos, .. }) => {
                if let Some(st) = self.stations.iter_mut().find(|s| s.pos == pos)
                    && let Some(f) = st.fire.as_mut()
                    && !f.ignite()
                {
                    h.out
                        .push(acted(&def.id, false, "The fuel is too wet to catch."));
                }
                self.show_station(h, pos);
            }
            (Effect::Feed, AimAt::Block { pos, .. }) => {
                let fuel: Vec<Fuel> = taken
                    .iter()
                    .map(|s| fuel_of(&h.lw.content, h.items, s))
                    .collect();
                if let Some(st) = self.stations.iter_mut().find(|s| s.pos == pos)
                    && let Some(f) = st.fire.as_mut()
                {
                    for x in fuel {
                        f.feed(x);
                    }
                }
                self.show_station(h, pos);
            }
            (Effect::Bank, AimAt::Block { pos, .. }) => {
                if let Some(st) = self.stations.iter_mut().find(|s| s.pos == pos)
                    && let Some(f) = st.fire.as_mut()
                {
                    f.bank();
                }
                self.show_station(h, pos);
            }
            (Effect::SetAlight, AimAt::Block { pos, .. }) => {
                if !self.ignite_at(h, pos) {
                    h.out.push(acted(&def.id, false, "It will not catch."));
                }
            }
            (Effect::Fell, AimAt::Block { pos, .. }) => self.fell(h, pos),
            (Effect::Place, AimAt::Block { .. } | AimAt::Beside { .. }) => {
                self.place_piece(h, &def, p.material.as_deref(), aim)
            }
            (Effect::Lop, AimAt::Block { pos, .. }) => self.lop(h, pos),
            (Effect::Buck, AimAt::Block { pos, .. }) => self.set_block(h, pos, BlockStateId::AIR),
            (Effect::Mend, _) => {
                if let Some(s) = p.keeps.first()
                    && let Some(stack) = Self::stack_mut(h, s)
                {
                    stack.condition = 1.0;
                    stack.quality = (stack.quality - 0.02).max(0.05);
                }
            }
            _ => {}
        }
        if let (Some(limit), AimAt::Block { pos, .. }) = (def.harvests, aim)
            && limit > 0
        {
            let year = h.moment.year;
            let e = self
                .harvests
                .entry((pos, def.id.clone()))
                .or_insert((year, 0));
            if e.0 != year {
                *e = (year, 0);
            }
            e.1 = e.1.saturating_add(1);
        }
        // A workstation laid out.
        if let (Some(station), AimAt::Block { pos, .. }) = (builds, aim) {
            self.build(h, &station, pos.up(), &taken);
        }
        // Firsts go in the journal.
        for s in &o.made {
            if let Some(k) = h.items.get(&s.id)
                && (k.form.is_some() || k.wear.is_some())
                && !k.has_tag("bulk")
                && h.player
                    .knowledge
                    .first_made(&k.id, &k.name.to_lowercase(), h.ticks)
            {
                self.knowledge_changed = true;
            }
        }
        let words = made_words(h.items, &o.made, &def.name);
        Self::give(h, o.made);
        h.out.push(acted(&def.id, true, words));
    }

    /// Sets up unattended work: its inputs go where it is done and wait.
    fn set_up(&mut self, h: &mut Here, r: usize, p: &Plan, aim: AimAt) {
        let def = self.crafts.recipes[r].def.clone();
        let taken = Self::take_all(h, &p.uses);
        let at = match aim {
            AimAt::Block { pos, .. } => {
                let c = center(pos);
                if self.station_at(pos).is_some() {
                    DVec3::new(c.x, pos.y as f64 + 0.55, c.z)
                } else {
                    DVec3::new(c.x, pos.y as f64 + 0.1, c.z)
                }
            }
            _ => h.player.mover.pos,
        };
        // One batch of everything used (a stack of meat cuts).
        let mut batch: Option<Stack> = None;
        for s in taken {
            match &mut batch {
                Some(b) if b.id == s.id => b.count += s.count,
                Some(_) => Self::give(h, vec![s]),
                None => batch = Some(s),
            }
        }
        if let Some(b) = batch {
            let id = h.world_items.add(b, at.to_array(), 0.0);
            if let Some(w) = h.world_items.get_mut(id) {
                w.work = Some(hearth_items::Batch {
                    process: def.id.clone(),
                    hours: 0.0,
                    wet_hours: 0.0,
                });
            }
            *h.items_changed = true;
            let triggers = vec![format!("do:{}", triggers::key(&def.id))];
            self.hear(h, &triggers);
            h.out.push(acted(
                &def.id,
                true,
                format!("{}: left to its work.", def.name),
            ));
        }
    }

    /// Lays out a workstation at `at`.
    fn build(&mut self, h: &mut Here, station: &str, at: BlockPos, parts: &[Stack]) {
        let content = h.lw.content.clone();
        let Some(w) = content.workstations.get(station) else {
            return;
        };
        let Some(block) = &w.block else {
            return;
        };
        let Ok(state) = h.lw.reg.parse_state(block.as_str()) else {
            return;
        };
        let max_c = w
            .provides
            .iter()
            .find_map(|c| match c {
                Capability::MaxTempC(t) => Some(*t),
                _ => None,
            })
            .unwrap_or(0.0);
        let fire = if station.ends_with("fat_lamp") {
            let fat: f32 = parts
                .iter()
                .filter(|s| h.items.get(&s.id).is_some_and(|k| k.has_tag("fat")))
                .map(|s| s.mass(h.items))
                .sum();
            Some(Fire::lamp(max_c, fat))
        } else if max_c > 0.0 {
            let fuel = parts
                .iter()
                .map(|s| fuel_of(&content, h.items, s))
                .collect();
            Some(Fire::laid(max_c, fuel))
        } else {
            None
        };
        self.set_block(h, at, state);
        self.stations.retain(|s| s.pos != at);
        self.stations.push(Station {
            pos: at,
            id: station.to_owned(),
            fire,
        });
        self.show_station(h, at);
    }

    /// A band's camp as its people keep it (V2.1 §15.3; H8): its hearth — the campfire standing
    /// at the camp, or one laid there with the band's beds about it — lit, and fed with dry wood
    /// whenever it burns low while its people are about it. A fire its people have left burns
    /// out where it stands.
    pub fn keep_camp(&mut self, h: &mut Here, c: &CampKept) {
        let content = h.lw.content.clone();
        let Some(campfire) = station_named(&content, "campfire") else {
            return;
        };
        let near = self
            .stations
            .iter()
            .position(|s| s.id == campfire && (center(s.pos) - c.at).length() < CAMP_FIRE_M);
        let i = match near {
            Some(i) => i,
            None => {
                let Some(CampLayout {
                    fire: pos,
                    beds,
                    trodden,
                }) = camp_layout(h.lw, c)
                else {
                    return;
                };
                for p in trodden {
                    self.set_block(h, p, BlockStateId::AIR);
                }
                let max_c = content
                    .workstations
                    .get(&campfire)
                    .and_then(|w| {
                        w.provides.iter().find_map(|c| match c {
                            Capability::MaxTempC(t) => Some(*t),
                            _ => None,
                        })
                    })
                    .unwrap_or(800.0);
                // Tinder, sticks and a split log, lit from the embers they carry.
                let mut fire = Fire::laid(
                    max_c,
                    vec![
                        dry_wood(0.1, 0.004),
                        dry_wood(1.2, 0.03),
                        dry_wood(2.0, 0.06),
                    ],
                );
                fire.ignite();
                if !self.stand(h, &campfire, pos, Some(fire)) {
                    return;
                }
                // Its beds about it: grass heaped, or hides over it where they know fur bedding.
                let bed = station_named(&content, if c.fur { "fur_bed" } else { "grass_bed" });
                if let Some(bed) = bed {
                    for p in beds {
                        if self.station_at(p).is_none() {
                            self.stand(h, &bed, p, None);
                        }
                    }
                }
                self.stations
                    .iter()
                    .position(|s| s.pos == pos)
                    .unwrap_or(self.stations.len() - 1)
            }
        };
        if c.tend
            && let Some(f) = self.stations[i].fire.as_mut()
            && f.fuel_kg() < 1.5
        {
            f.feed(dry_wood(2.0, 0.05));
            if !f.lit() {
                f.ignite();
            }
            let pos = self.stations[i].pos;
            self.show_station(h, pos);
        }
    }

    /// Stands a workstation's block at `at` with its fire (whether it could).
    fn stand(&mut self, h: &mut Here, station: &str, at: BlockPos, fire: Option<Fire>) -> bool {
        let content = h.lw.content.clone();
        let Some(state) = content
            .workstations
            .get(station)
            .and_then(|w| w.block.as_ref())
            .and_then(|b| h.lw.reg.parse_state(b.as_str()).ok())
        else {
            return false;
        };
        self.set_block(h, at, state);
        self.stations.retain(|s| s.pos != at);
        self.stations.push(Station {
            pos: at,
            id: station.to_owned(),
            fire,
        });
        self.show_station(h, at);
        true
    }

    /// Shows a station's fire in its block's state.
    fn show_station(&mut self, h: &mut Here, pos: BlockPos) {
        let Some(st) = self.stations.iter().find(|s| s.pos == pos) else {
            return;
        };
        let Some(state) = h.lw.map.block(pos) else {
            return;
        };
        let reg = h.lw.reg.clone();
        let fire = st.fire.as_ref().map_or(FireState::Out, Fire::state);
        let mut next = reg.with_or_same(state, "fire", fire.name());
        next = reg.with_or_same(next, "lit", if fire.lit() { "true" } else { "false" });
        if next != state {
            self.set_block(h, pos, next);
        }
    }

    /// The generated tree whose trunk or limb is at `pos`, if it still stands.
    fn standing_tree(
        &self,
        h: &Here,
        pos: BlockPos,
    ) -> Option<hearth_worldgen::cubegen::features::PlacedTree> {
        let wg = h.lw.generator.clone();
        let t = wg.features().tree_at(&wg, &h.lw.vegetation, pos)?;
        // Still there: the block aimed at and the foot of the trunk.
        let here = t
            .blocks()
            .find(|(p, _)| *p == pos)
            .map(|(_, part)| t.state(&wg.forest, part));
        let foot = t
            .blocks()
            .find(|(p, part)| p.y == t.foot[1] && !matches!(part, hearth_flora::Part::Leaves))
            .map(|(p, part)| (p, t.state(&wg.forest, part)));
        let standing = here.is_some_and(|s| h.lw.map.block(pos) == Some(s))
            && foot.is_some_and(|(p, s)| h.lw.map.block(p) == Some(s));
        standing.then_some(t)
    }

    /// What a falling tree flattens where it comes to rest: anything a person walks through
    /// (grass, herbs, a low shrub), not water.
    fn crushed(h: &Here, p: BlockPos) -> bool {
        h.lw.map.block(p).is_some_and(|s| {
            s.is_air() || {
                let reg = &h.lw.reg;
                reg.block_of(s).def.fluid.is_none()
                    && !reg.has(s, hearth_world::StateFlags::HAS_COLLISION)
            }
        })
    }

    fn foliage(h: &Here, p: BlockPos) -> bool {
        h.lw.map
            .block(p)
            .is_some_and(|s| h.lw.reg.block_of(s).name.path().ends_with("_leaves"))
    }

    /// A tree is cut through: it is taken from where it stood (but the stump) and falls away
    /// from the one who cut it, turning about the stump down onto the ground, where it rests a
    /// few seconds later; what stood in its way is broken, and anyone under it is hurt.
    fn fell(&mut self, h: &mut Here, pos: BlockPos) {
        let Some(t) = self.standing_tree(h, pos) else {
            return;
        };
        let wg = h.lw.generator.clone();
        let blocks = match (&wg.forest.charred, t.remains) {
            (Some(c), hearth_worldgen::cubegen::features::Remains::Charred) => c,
            _ => &wg.forest.blocks[t.species],
        };
        let foot = BlockPos::new(t.foot[0], t.foot[1], t.foot[2]);
        // The tree as it stands now (what is already taken stays taken); the stump stays.
        let standing: Vec<(BlockPos, BlockStateId, hearth_flora::Part)> = t
            .blocks()
            .filter_map(|(p, part)| {
                let st = blocks.state(part);
                let stump = p.y <= foot.y && !matches!(part, hearth_flora::Part::Leaves);
                (!stump && h.lw.map.block(p) == Some(st)).then_some((p, st, part))
            })
            .collect();
        if standing.is_empty() {
            return;
        }
        // Away from the cutter, along the nearer axis.
        let away = center(foot) - h.player.mover.pos;
        let toward = if away.x.abs() >= away.z.abs() {
            if away.x >= 0.0 {
                hearth_math::Direction::East
            } else {
                hearth_math::Direction::West
            }
        } else if away.z >= 0.0 {
            hearth_math::Direction::South
        } else {
            hearth_math::Direction::North
        };
        let fallen = fallen_tree(&standing, foot, toward, blocks);
        // Lowered or raised until the trunk lies on the ground, not in it.
        let solid = |h: &Here, p: BlockPos| {
            h.lw.map.block(p).is_some_and(|s| {
                !s.is_air()
                    && !h.lw.reg.block_of(s).def.replaceable
                    && h.lw.reg.has(s, hearth_world::StateFlags::HAS_COLLISION)
                    && !standing.iter().any(|(q, _, _)| *q == p)
            })
        };
        let logs: Vec<BlockPos> = fallen
            .iter()
            .filter(|(_, _, log)| *log)
            .map(|(p, _, _)| *p)
            .collect();
        let fits = |k: i32| {
            logs.iter()
                .all(|p| !solid(h, BlockPos::new(p.x, p.y + k, p.z)))
        };
        let rests = |k: i32| {
            logs.iter()
                .any(|p| solid(h, BlockPos::new(p.x, p.y + k - 1, p.z)))
        };
        let k = (-4..=4)
            .filter(|k| fits(*k))
            .min_by_key(|k| (!rests(*k), k.abs()))
            .unwrap_or(0);
        // Taken from where it stood: felled, as the vegetation keeps it (the generator leaves
        // the stump, and in a few years another tree takes the place), not as the player's
        // change to the blocks.
        for (p, _, _) in &standing {
            self.set_natural(h, *p, BlockStateId::AIR);
        }
        let year = h.lw.vegetation.year;
        h.lw.vegetation =
            h.lw.vegetation
                .with(hearth_worldgen::vegetation::Disturbance {
                    kind: hearth_worldgen::vegetation::DisturbanceKind::Felled,
                    year,
                    x: foot.x,
                    z: foot.z,
                    radius: (t.template.reach() as f32).max(2.0),
                    severity: 1.0,
                    patches: Vec::new(),
                });
        let height = standing
            .iter()
            .map(|(p, _, _)| p.y - foot.y)
            .max()
            .unwrap_or(1) as f64;
        let seconds = (0.9 * (height.max(2.0) / 3.0).sqrt()).clamp(1.2, 5.0) as f32;
        let pivot = center(foot) + toward.normal_f64() * 0.5 + DVec3::new(0.0, -0.5, 0.0);
        h.out.push(ToClient::TreeFalls {
            blocks: standing.iter().map(|(p, s, _)| (*p, *s)).collect(),
            pivot,
            toward,
            seconds,
        });
        let at = h.ticks + (seconds as f64 * hearth_content::time::TICKS_PER_SECOND) as u64;
        self.falls.push((
            at,
            fallen
                .into_iter()
                .map(|(p, s, _)| (BlockPos::new(p.x, p.y + k, p.z), s))
                .collect(),
        ));
        // Anyone in its way: within its length along the fall and its crown across it.
        let me = h.player.mover.pos - center(foot);
        let along = me.dot(toward.normal_f64());
        let across = (me - toward.normal_f64() * along).length();
        let crown = t.template.reach() as f64 * 0.6;
        if along > 0.5 && along < height && across < crown.max(1.5) && me.y > -2.0 {
            let d = t.template.diameter_m;
            let side = if self.rng.chance(0.5) {
                hearth_body::Side::Left
            } else {
                hearth_body::Side::Right
            };
            use hearth_content::schema::body::BodyRegion;
            let hit = (0.3 + d * 1.5).min(1.0);
            h.player
                .body
                .injure(h.cfg, "bruise", BodyRegion::Chest, side, hit);
            if d > 0.15 {
                h.player
                    .body
                    .injure(h.cfg, "fracture", BodyRegion::UpperArm, side, hit);
            }
            h.out.push(acted("", false, "The tree comes down on you."));
        }
    }

    /// A limb is cut off with what grows from it: the limbs joined to it no thicker than it,
    /// out to their twigs, and the foliage about them.
    fn lop(&mut self, h: &mut Here, pos: BlockPos) {
        let thick = |h: &Here, p: BlockPos| {
            h.lw.map.block(p).and_then(|s| {
                let b = h.lw.reg.block_of(s);
                b.name
                    .path()
                    .ends_with("_branch")
                    .then(|| {
                        h.lw.reg
                            .get(s, "thickness")
                            .and_then(|v| v.parse::<u8>().ok())
                    })
                    .flatten()
            })
        };
        let Some(t0) = thick(h, pos) else {
            return;
        };
        let mut limb = vec![pos];
        let mut seen: rustc_hash::FxHashSet<BlockPos> = [pos].into_iter().collect();
        let mut i = 0;
        while i < limb.len() && limb.len() < 400 {
            let p = limb[i];
            i += 1;
            for d in hearth_math::Direction::ALL {
                let q = p.offset(d);
                if seen.insert(q) && thick(h, q).is_some_and(|t| t <= t0) {
                    limb.push(q);
                }
            }
        }
        let mut leaves = Vec::new();
        for p in &limb {
            for d in hearth_math::Direction::ALL {
                let q = p.offset(d);
                if seen.insert(q) && Self::foliage(h, q) {
                    leaves.push(q);
                }
            }
        }
        for p in limb.into_iter().chain(leaves) {
            self.set_block(h, p, BlockStateId::AIR);
        }
    }

    /// Sets a block as nature changed it: not kept as the player's change, so the terrain
    /// generated again (as the vegetation has it) shows what is there.
    fn set_natural(&mut self, h: &mut Here, pos: BlockPos, state: BlockStateId) {
        let reg = h.lw.reg.clone();
        h.lw.map.set_block(pos, state, &reg);
        h.lw.edits.forget(pos);
        h.changed.push(pos);
        if state.is_air() {
            self.stations.retain(|s| s.pos != pos);
        }
    }

    fn set_block(&mut self, h: &mut Here, pos: BlockPos, state: BlockStateId) {
        let reg = h.lw.reg.clone();
        h.lw.map.set_block(pos, state, &reg);
        // Kept, so the change outlasts the cube being unloaded and generated again.
        h.lw.edits.set(pos, state);
        h.changed.push(pos);
        if state.is_air() {
            self.stations.retain(|s| s.pos != pos);
        }
    }

    fn open(h: &Here, p: BlockPos) -> bool {
        h.lw.map.block(p).is_some_and(|s| {
            s.is_air() || {
                let d = &h.lw.reg.block_of(s).def;
                d.replaceable && d.fluid.is_none()
            }
        })
    }

    fn loose(h: &Here, p: BlockPos) -> bool {
        h.lw.map.block(p).is_some_and(|s| {
            let b = h.lw.reg.block_of(s);
            b.name.path() == "spoil"
                || b.def
                    .material
                    .as_deref()
                    .and_then(|m| h.lw.content.materials.get(m))
                    .is_some_and(|m| m.tags.iter().any(|t| t == "falls"))
        })
    }

    /// Digs out a block: snow packs away, earth falls in a spoil pile beside the hole.
    fn excavate(&mut self, h: &mut Here, pos: BlockPos) {
        let Some(state) = h.lw.map.block(pos) else {
            return;
        };
        let block = h.lw.reg.block_of(state);
        let snow = block.def.material.as_deref().is_some_and(|m| {
            h.lw.content.materials.get(m).is_some_and(|m| {
                m.category == hearth_content::schema::material::MaterialCategory::Snow
            })
        });
        let spoil = if Self::loose(h, pos) {
            Some(state)
        } else if snow {
            None
        } else {
            h.lw.reg.parse_state("hearth:spoil").ok()
        };
        self.set_block(h, pos, BlockStateId::AIR);
        if let Some(sp) = spoil {
            // The lowest place beside the hole, away from where the player stands.
            let feet = h.player.mover.pos;
            let mut best: Option<(i32, f64, BlockPos)> = None;
            for d in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
                let mut p = BlockPos::new(pos.x + d[0], pos.y, pos.z + d[1]);
                let mut ok = false;
                for _ in 0..6 {
                    if Self::open(h, p) {
                        if Self::open(h, p.down()) && p.down() != pos {
                            p = p.down();
                            continue;
                        }
                        ok = true;
                        break;
                    }
                    p = p.up();
                }
                if !ok || p.down() == pos {
                    continue;
                }
                let away = -(center(p) - feet).length();
                if best.is_none_or(|(y, a, _)| (p.y, away) < (y, a)) {
                    best = Some((p.y, away, p));
                }
            }
            if let Some((_, _, p)) = best {
                self.set_block(h, p, sp);
                self.slump(h, p);
            }
        }
        self.slump_around(h, pos);
    }

    /// Loose blocks next to and above an opened place slump into it.
    fn slump_around(&mut self, h: &mut Here, pos: BlockPos) {
        for p in [
            pos.up(),
            BlockPos::new(pos.x + 1, pos.y + 1, pos.z),
            BlockPos::new(pos.x - 1, pos.y + 1, pos.z),
            BlockPos::new(pos.x, pos.y + 1, pos.z + 1),
            BlockPos::new(pos.x, pos.y + 1, pos.z - 1),
        ] {
            if Self::loose(h, p) {
                self.slump(h, p);
            }
        }
    }

    /// A loose block falls and slides down any step of two until it rests: piles stand at
    /// their angle of repose (a block's 45°, near the 35–40° of loose earth and gravel).
    fn slump(&mut self, h: &mut Here, mut p: BlockPos) {
        for _ in 0..24 {
            let Some(state) = h.lw.map.block(p) else {
                return;
            };
            if Self::open(h, p.down()) {
                self.set_block(h, p, BlockStateId::AIR);
                p = p.down();
                self.set_block(h, p, state);
                continue;
            }
            let start = (self.rng.next_u32() % 4) as usize;
            let dirs = [[1, 0], [0, 1], [-1, 0], [0, -1]];
            let mut moved = false;
            for k in 0..4 {
                let d = dirs[(start + k) % 4];
                let side = BlockPos::new(p.x + d[0], p.y, p.z + d[1]);
                if Self::open(h, side) && Self::open(h, side.down()) {
                    self.set_block(h, p, BlockStateId::AIR);
                    let above = p.up();
                    p = side.down();
                    self.set_block(h, p, state);
                    if Self::loose(h, above) {
                        self.slump(h, above);
                    }
                    moved = true;
                    break;
                }
            }
            if !moved {
                return;
            }
        }
    }

    /// Radiant heat from fires near a point, as a body there absorbs it averaged over its skin
    /// (W/m²): about half the body faces the fire and skin takes nine tenths of what reaches
    /// it. A fire in the vegetation about it adds its own.
    pub fn radiant_w_m2(&self, at: DVec3) -> f32 {
        let facing: f32 = self
            .stations
            .iter()
            .chain(&self.wildfires)
            .filter_map(|s| {
                let f = s.fire.as_ref()?;
                let d = (center(s.pos) - at).length() as f32;
                (d < 16.0).then(|| f.radiant_w_m2(d))
            })
            .sum();
        (0.45 * facing).min(600.0) + self.blaze.radiant_w_m2(at)
    }

    /// The heat the fires burning within `r` of a place give (kW): what warms the air of a hut
    /// about them.
    pub fn fire_kw_near(&self, at: DVec3, r: f64) -> f32 {
        self.stations
            .iter()
            .filter_map(|s| {
                let f = s.fire.as_ref()?;
                ((center(s.pos) - at).length() <= r).then_some(f.power_kw)
            })
            .sum()
    }

    /// What each block burns as.
    fn fuel_table(&mut self, h: &Here) -> Arc<FuelTable> {
        self.fuel
            .get_or_insert_with(|| Arc::new(FuelTable::new(&h.lw.reg)))
            .clone()
    }

    /// The fire danger where the near fire burns (or at the player), found once a game minute.
    fn danger_near(&mut self, h: &Here) -> Danger {
        let minute = (h.ticks as f64 / (h.ticks_per_day / 1440.0).max(1.0)) as u64;
        if let Some((m, d)) = self.danger
            && m == minute
        {
            return d;
        }
        let at = self
            .blaze
            .plumes()
            .first()
            .map_or(h.player.mover.pos, |p| p.at);
        let d = crate::wildfire::danger(h.env, h.ticks, at);
        self.danger = Some((minute, d));
        d
    }

    /// A block of vegetation is set alight (lightning, a brand, a test); whether it caught.
    pub fn ignite_at(&mut self, h: &mut Here, pos: BlockPos) -> bool {
        let table = self.fuel_table(h);
        let tick_s = (86_400.0 / h.ticks_per_day) as f32;
        let spared: FxHashSet<BlockPos> = self
            .stations
            .iter()
            .chain(&self.wildfires)
            .map(|s| s.pos)
            .collect();
        let ticks = h.ticks;
        let mut world = HereFire { h, spared: &spared };
        // A plant over turf, or the turf under it.
        self.blaze.ignite(&mut world, &table, pos, ticks, tick_s)
            || self
                .blaze
                .ignite(&mut world, &table, pos.down(), ticks, tick_s)
    }

    /// A step of the near fire; where it runs on into terrain not loaded, the far fire takes it.
    fn fire_step(&mut self, h: &mut Here) {
        let table = self.fuel_table(h);
        let danger = self.danger_near(h);
        let tick_s = (86_400.0 / h.ticks_per_day) as f32;
        let spared: FxHashSet<BlockPos> = self
            .stations
            .iter()
            .chain(&self.wildfires)
            .map(|s| s.pos)
            .collect();
        let ticks = h.ticks;
        let mut beyond = Vec::new();
        {
            let mut world = HereFire { h, spared: &spared };
            self.blaze
                .step(&mut world, &table, danger, ticks, tick_s, &mut beyond);
        }
        let mut cells: Vec<(i32, i32)> = beyond
            .iter()
            .map(|p| FarFire::cell_of(p.x as f64, p.z as f64))
            .collect();
        cells.sort_unstable();
        cells.dedup();
        for cell in cells {
            let fuel = FarFire::fuel(h.env, h.lw.terrain(), cell);
            self.far_fire.ignite(cell, fuel, ticks, h.ticks_per_day);
        }
    }

    /// A game minute of fire: the ground the near fire has left is kept as burned; the far
    /// fire's hours; flames burn whoever stands in them.
    fn fire_minute(&mut self, h: &mut Here) {
        if let Some(d) = self.blaze.take_burned(h.lw.vegetation.year) {
            h.lw.vegetation = h.lw.vegetation.with(d);
        }
        if self.far_fire.is_burning() {
            let feet = h.player.mover.pos;
            let near = |cell: (i32, i32)| {
                let c = DVec3::new(
                    (cell.0 * hearth_worldgen::vegetation::ECO_CELL
                        + hearth_worldgen::vegetation::ECO_CELL / 2) as f64,
                    feet.y,
                    (cell.1 * hearth_worldgen::vegetation::ECO_CELL
                        + hearth_worldgen::vegetation::ECO_CELL / 2) as f64,
                );
                (c - feet).length() < 220.0
            };
            let mut handed = Vec::new();
            let terrain = h.lw.generator.terrain.clone();
            self.far_fire
                .hour(h.env, &terrain, h.ticks, feet, near, &mut handed);
            // A far fire coming near: the near fire takes it up at the cell's edge nearest it.
            for cell in handed {
                let e = hearth_worldgen::vegetation::ECO_CELL;
                for _ in 0..24 {
                    let x = cell.0 * e + self.rng.range_i32(0, e);
                    let z = cell.1 * e + self.rng.range_i32(0, e);
                    if let Some(top) = h.lw.map.sky_top(x, z)
                        && self.ignite_at(h, BlockPos::new(x, top, z))
                    {
                        break;
                    }
                }
            }
            for d in self.far_fire.take_burned(h.lw.vegetation.year) {
                h.lw.vegetation = h.lw.vegetation.with(d);
            }
        }
        // Flames burn.
        let feet = BlockPos::containing(h.player.mover.pos + DVec3::new(0.0, 0.1, 0.0));
        if self.blaze.is_burning()
            && h.player.body.dead.is_none()
            && (self.blaze.in_flames(feet) || self.blaze.in_flames(feet.up()))
        {
            use hearth_content::schema::body::BodyRegion;
            let side = if self.rng.chance(0.5) {
                hearth_body::Side::Left
            } else {
                hearth_body::Side::Right
            };
            h.player
                .body
                .injure(h.cfg, "burn", BodyRegion::Foot, side, 0.3);
            h.out.push(acted("", false, "The flames burn you."));
        }
    }

    /// The smoke of the fires in the vegetation, near and far.
    pub fn plumes(&self, h: &Here) -> Vec<Plume> {
        let mut v = self.blaze.plumes();
        v.extend(self.far_fire.plumes(h.lw.terrain(), *h.lw.map.planet()));
        v
    }

    /// Whether any fire burns in the vegetation.
    pub fn fire_burning(&self) -> bool {
        self.blaze.is_burning() || self.far_fire.is_burning()
    }

    /// The bedding under someone lying at `feet` (insulation from the ground, clo).
    pub fn bedding_clo(&self, feet: DVec3) -> f32 {
        let p = BlockPos::containing(feet + DVec3::new(0.0, 0.05, 0.0));
        match self
            .station_at(p)
            .map(|s| triggers::key(&s.id).to_owned())
            .as_deref()
        {
            Some("fur_bed") => 1.5,
            Some("grass_bed") => 0.8,
            _ => 0.0,
        }
    }

    /// What sight teaches: the keys of what the player looks at.
    pub fn look(&mut self, h: &mut Here, aim: AimAt) {
        let keys = match aim {
            AimAt::Block { pos, .. } | AimAt::Beside { pos, .. } => {
                if self.wildfires.iter().any(|w| w.pos == pos) {
                    vec![WILDFIRE.to_owned(), "fire".to_owned()]
                } else {
                    let Some(state) = h.lw.map.block(pos) else {
                        return;
                    };
                    let b = h.lw.reg.block_of(state);
                    let m = b
                        .def
                        .material
                        .as_deref()
                        .and_then(|m| h.lw.content.materials.get(m));
                    block_keys(&b.name.to_string(), m)
                }
            }
            AimAt::Thing(id) => {
                let Some(w) = h.world_items.get(id) else {
                    return;
                };
                match h.lw.content.items.get(&w.stack.id) {
                    Some(d) => item_keys(d, &h.lw.content),
                    None => return,
                }
            }
            AimAt::Nothing => return,
        };
        let hour = (h.ticks_per_day / 24.0) as u64;
        for t in with_verb("see", &keys) {
            self.hear_now_and_then(h, t, hour);
        }
    }

    /// Eats one of the food carried at a path.
    pub fn eat(&mut self, h: &mut Here, path: &Path) {
        let Some(stack) = h.player.carry.get(path) else {
            return;
        };
        let Some(kind) = h.items.get(&stack.id) else {
            return;
        };
        let Some(bite) = bite_of(&h.lw.content, kind, stack) else {
            h.out.push(acted("eat", false, "That is not food."));
            return;
        };
        let name = kind.name.clone();
        let food = hearth_body::Food {
            kcal: bite.kcal as f64,
            protein_g: bite.protein_g as f64,
            fat_g: bite.fat_g as f64,
            carb_g: bite.carb_g as f64,
            water_l: bite.water_l as f64,
            volume_l: bite.volume_l as f64,
            fresh_days: bite.fresh_days as f64,
        };
        match h.player.body.eat(h.cfg, &food) {
            Ok(()) => {
                h.player.carry.take(h.items, path, Some(1));
                for (cause, chance) in &bite.risks {
                    h.player.body.expose(h.cfg, cause, *chance as f64);
                }
                let keys =
                    h.lw.content
                        .items
                        .get(&kind.id)
                        .map(|d| item_keys(d, &h.lw.content))
                        .unwrap_or_default();
                let t: Vec<String> = with_verb("eat", &keys).collect();
                self.hear(h, &t);
                // Medicine works for some hours (willow bark eases pain).
                if let Some(m) = kind
                    .material
                    .as_deref()
                    .and_then(|m| h.lw.content.materials.get(m))
                    .and_then(|m| m.medicine.clone())
                {
                    h.player
                        .body
                        .take_medicine(&m.kind, m.strength, m.hours as f64);
                }
                h.out.push(acted(
                    "eat",
                    true,
                    format!("You eat the {}.", name.to_lowercase()),
                ));
            }
            Err(hearth_body::Refusal::Full) => h.out.push(acted("eat", false, "You are full.")),
            Err(_) => h.out.push(acted("eat", false, "You cannot eat now.")),
        }
    }

    /// Drinks a mouthful (a quarter litre).
    pub fn drink(&mut self, h: &mut Here, from: &DrinkFrom) {
        match from {
            DrinkFrom::Water(AimAt::Block { pos, .. }) => {
                if !matches!(
                    self.aimed(
                        h,
                        AimAt::Block {
                            pos: *pos,
                            top: false
                        }
                    ),
                    Some(Aimed::Water)
                ) {
                    return;
                }
                if (center(*pos) - h.player.mover.pos).length() > 3.5 {
                    h.out
                        .push(acted("drink", false, "The water is out of reach."));
                    return;
                }
                let w = h.env.weather_at(&h.moment, center(*pos));
                let q = crate::water_env::WorldWater {
                    generator: &h.lw.generator,
                    air_c: w.temperature_c as f32,
                    humidity: w.humidity as f32,
                    wind_m_s: w.wind_speed_m_s as f32,
                }
                .quality_at(*pos);
                // Salt water is tasted first and spat out; only drinking again at once (within
                // some seconds) swallows it.
                let salty = q.salinity_g_l > 5.0;
                let just_tasted = self
                    .tasted_salt
                    .is_some_and(|t| h.ticks.saturating_sub(t) < TASTE_TICKS);
                if salty && !just_tasted {
                    self.tasted_salt = Some(h.ticks);
                    h.out.push(acted(
                        "drink",
                        false,
                        "The water is salty. You spit it out.",
                    ));
                    return;
                }
                let l =
                    h.player
                        .body
                        .drink(h.cfg, 0.25, q.salinity_g_l as f64, q.pathogen_risk as f64);
                let words = if l <= 0.0 {
                    "You cannot drink more."
                } else if q.salinity_g_l > 5.0 {
                    "The water is salty."
                } else {
                    "You drink."
                };
                h.out.push(acted("drink", l > 0.0, words));
            }
            DrinkFrom::Skin(path) => {
                let Some(skin) = h.player.carry.get_mut(path) else {
                    return;
                };
                let l = skin.liquid_l.min(0.25);
                if l <= 0.0 {
                    h.out.push(acted("drink", false, "The skin is empty."));
                    return;
                }
                skin.liquid_l -= l;
                let drunk = h.player.body.drink(h.cfg, l as f64, 0.2, 0.01);
                if let Some(skin) = h.player.carry.get_mut(path) {
                    skin.liquid_l += (l as f64 - drunk).max(0.0) as f32;
                }
                h.out
                    .push(acted("drink", drunk > 0.0, "You drink from the skin."));
            }
            DrinkFrom::Water(_) => {}
        }
    }

    /// Fills a carried water skin from water looked at.
    pub fn fill(&mut self, h: &mut Here, skin: &Path, aim: AimAt) {
        if !matches!(self.aimed(h, aim), Some(Aimed::Water)) {
            return;
        }
        let Some(stack) = h.player.carry.get(skin) else {
            return;
        };
        let Some(cap) = h
            .items
            .get(&stack.id)
            .and_then(|k| k.container)
            .map(|c| c.liquid_l)
        else {
            return;
        };
        if cap <= 0.0 {
            return;
        }
        if let Some(s) = h.player.carry.get_mut(skin) {
            s.liquid_l = cap;
        }
        h.out.push(acted("fill", true, "You fill the skin."));
    }

    /// Throws what is in the right hand: it flies and lands as a thing lying in the world.
    pub fn throw(&mut self, h: &mut Here, dir: DVec3, speed: f64) -> Option<Flight> {
        if !h.player.can_act(h.cfg) {
            return None;
        }
        let stack = h
            .player
            .carry
            .take(h.items, &Path::at(Root::Hand(Hand::Right)), Some(1))?;
        let mass = stack.mass(h.items).max(0.05) as f64;
        // A strong overarm throw: about 20 m/s for a stone, slower for heavy things.
        let v0 = speed.clamp(0.0, 25.0) * (0.6 / mass).sqrt().clamp(0.3, 1.0);
        // As true as the thrower's practice: a novice's throw strays three degrees or so, a
        // practised one's half a degree.
        let skill = h.player.knowledge.skill(THROWING);
        let spread = (3.0 - 2.4 * skill.clamp(0.0, 1.0) as f64).to_radians();
        let dir = dir.normalize_or_zero();
        let side = dir.cross(DVec3::Y).normalize_or(DVec3::X);
        let up = side.cross(dir).normalize_or(DVec3::Y);
        let (a, r) = (
            self.rng.range_f64(0.0, std::f64::consts::TAU),
            self.rng.normal() * spread,
        );
        let dir = (dir + (side * a.cos() + up * a.sin()) * r.tan()).normalize_or_zero();
        h.player.knowledge.practice(THROWING, 0.05, h.ticks);
        let mut v = dir * v0;
        let mut p = h.player.mover.pos + DVec3::new(0.0, 1.5, 0.0);
        let dt = 0.02;
        let reg = h.lw.reg.clone();
        let mut path = vec![p];
        for _ in 0..800 {
            let next = p + v * dt;
            let b = BlockPos::containing(next);
            let solid =
                h.lw.map
                    .block(b)
                    .is_some_and(|s| !reg.collision_shape(s).is_empty());
            if solid {
                break;
            }
            p = next;
            path.push(p);
            v.y -= 9.81 * dt;
            if h.lw.map.block(b).is_none() {
                break;
            }
        }
        let keys =
            h.lw.content
                .items
                .get(&stack.id)
                .map(|d| item_keys(d, &h.lw.content))
                .unwrap_or_default();
        let t: Vec<String> = with_verb("throw", &keys).collect();
        self.hear(h, &t);
        Some(Flight {
            stack,
            path,
            dt,
            mass,
        })
    }

    /// Reaching for a thing with full hands and nowhere to put it.
    pub fn hands_full(&mut self, h: &mut Here) {
        self.hear(h, &[triggers::CARRY_FULL_HANDS.to_owned()]);
    }

    /// A game minute: fires burn, unattended work goes on, things go off, storms and kills.
    fn minute(&mut self, h: &mut Here, dt_h: f32) {
        let feet = h.player.mover.pos;
        let w = h.env.weather_at(&h.moment, feet);
        let air_c = w.temperature_c as f32;
        let rain = w.precip_mm_h as f32;
        let wind = w.wind_speed_m_s as f32;
        // Fires.
        let mut shown = Vec::new();
        for st in self.stations.iter_mut().chain(self.wildfires.iter_mut()) {
            let Some(f) = st.fire.as_mut() else {
                continue;
            };
            let before = f.state();
            let covered =
                h.lw.map
                    .sky_top(st.pos.x, st.pos.z)
                    .is_some_and(|top| top > st.pos.y + 1);
            f.step(dt_h, air_c, if covered { 0.0 } else { rain }, wind);
            if f.state() != before {
                shown.push(st.pos);
            }
        }
        for pos in shown {
            if self.wildfires.iter().any(|w| w.pos == pos) {
                // A natural fire burnt out leaves nothing standing in the flames.
                let out = self
                    .wildfires
                    .iter()
                    .any(|w| w.pos == pos && w.fire.as_ref().is_none_or(|f| !f.lit()));
                if out {
                    self.set_block(h, pos, BlockStateId::AIR);
                    self.wildfires.retain(|w| w.pos != pos);
                }
            } else {
                self.show_station(h, pos);
            }
        }
        // A burning tree within sight teaches what fire is.
        let seen_fire = self.wildfires.iter().any(|w| {
            w.fire.as_ref().is_some_and(Fire::lit) && (center(w.pos) - feet).length() < 64.0
        });
        if seen_fire {
            let every = (h.ticks_per_day / 72.0) as u64;
            self.hear_now_and_then(h, triggers::SEE_WILDFIRE.to_owned(), every);
        }
        // Sleeping on a hide.
        if h.player.asleep {
            let on_hide = self.bedding_clo(feet) >= 1.0
                || h.world_items.items.iter().any(|w| {
                    (DVec3::from_array(w.pos) - feet).length() < 1.5
                        && h.items.get(&w.stack.id).is_some_and(|k| k.has_tag("hide"))
                });
            if on_hide {
                let every = (h.ticks_per_day / 4.0) as u64;
                self.hear_now_and_then(h, triggers::SLEEP_ON_HIDE.to_owned(), every);
            }
        }
        self.batches(h, dt_h, air_c);
        self.go_off(h, dt_h, air_c);
        if self.fire_burning() {
            self.fire_minute(h);
        }
        self.storm(h, &w);
    }

    /// Unattended work goes on while its conditions hold.
    fn batches(&mut self, h: &mut Here, dt_h: f32, air_c: f32) {
        let mut done = Vec::new();
        for wi in h.world_items.items.iter_mut() {
            let Some(work) = wi.work.as_mut() else {
                continue;
            };
            let Some(r) = self.crafts.index_of(&work.process) else {
                wi.work = None;
                continue;
            };
            let def = &self.crafts.recipes[r].def;
            let pos = DVec3::from_array(wi.pos);
            let block = BlockPos::containing(pos);
            let covered =
                h.lw.map
                    .sky_top(block.x, block.z)
                    .is_some_and(|top| top > block.y + 1);
            let raining = !covered && h.env.weather_at(&h.moment, pos).precip_mm_h > 0.1;
            let in_water =
                h.lw.map
                    .block(block)
                    .is_some_and(|s| h.lw.reg.block_of(s).def.fluid.is_some());
            let at_station = def.station.as_ref().is_none_or(|s| {
                self.stations
                    .iter()
                    .any(|st| st.id == s.as_str() && st.pos == block)
            });
            use hearth_content::schema::process::Condition;
            let ok = at_station
                && def.conditions.iter().all(|c| match c {
                    Condition::Dry => !raining,
                    Condition::Water => in_water,
                    Condition::ColdBelowC(t) => air_c < *t,
                    _ => true,
                });
            if ok {
                work.hours += dt_h;
            }
            if raining {
                work.wet_hours += dt_h;
            }
            if work.hours >= def.duration.hours {
                done.push(wi.id);
            }
        }
        for id in done {
            let Some(wi) = h.world_items.take(id) else {
                continue;
            };
            let work = wi.work.clone().expect("work");
            let Some(r) = self.crafts.index_of(&work.process) else {
                continue;
            };
            let wet = work.wet_hours / (work.hours + work.wet_hours).max(0.01);
            match finish_batch(
                &self.crafts,
                r,
                &wi.stack,
                wet,
                &h.lw.content,
                h.items,
                &mut self.rng,
            ) {
                Ok(made) => {
                    for (k, s) in made.into_iter().enumerate() {
                        let p = [wi.pos[0] + 0.1 * k as f64, wi.pos[1], wi.pos[2]];
                        h.world_items.add(s, p, wi.yaw);
                    }
                }
                Err(why) => {
                    let name = self.crafts.recipes[r].def.name.clone();
                    h.out
                        .push(acted(&work.process, false, format!("{name}: {why}")));
                }
            }
            *h.items_changed = true;
        }
    }

    /// Food and carcasses go off; embers burn down. What is being worked unattended (meat on
    /// the rack) goes off at a quarter of the rate, and the less the drier it gets: dried, it
    /// keeps.
    fn go_off(&mut self, h: &mut Here, dt_h: f32, air_c: f32) {
        let content = h.lw.content.clone();
        let items = h.items;
        let age = |s: &mut Stack, temp: f32, pace: f32| {
            if let Some(kind) = items.get(&s.id)
                && let Some(keeps) = keeps_days(&content, kind)
            {
                let rate = decay_per_hour(keeps, temp, s.wet);
                s.decay = (s.decay + rate * dt_h * pace).min(1.6);
            }
            if s.glow_h > 0.0 {
                s.glow_h = (s.glow_h - dt_h).max(-1.0);
            }
        };
        h.player.carry.for_each_mut(&mut |s| age(s, air_c, 1.0));
        h.player.carry.retain(&mut |s| {
            !(s.glow_h < 0.0
                && items
                    .get(&s.id)
                    .is_some_and(|k| k.property("glow_h").is_some()))
        });
        let before = h.world_items.items.len();
        for wi in h.world_items.items.iter_mut() {
            let pace = match &wi.work {
                Some(work) => {
                    let done = self.crafts.index_of(&work.process).map_or(0.0, |r| {
                        work.hours / self.crafts.recipes[r].def.duration.hours.max(1e-3)
                    });
                    0.25 * (1.0 - done.clamp(0.0, 1.0))
                }
                None => 1.0,
            };
            age(&mut wi.stack, air_c, pace);
        }
        h.world_items.items.retain(|wi| {
            let cold = wi.stack.glow_h < 0.0
                && items
                    .get(&wi.stack.id)
                    .is_some_and(|k| k.property("glow_h").is_some());
            !cold && !rotted_away(&content, items, &wi.stack)
        });
        if h.world_items.items.len() != before {
            *h.items_changed = true;
        }
    }

    /// Lightning now and then sets a tree burning.
    fn storm(&mut self, h: &mut Here, w: &hearth_env::WeatherState) {
        if w.thunder <= 0.0 {
            return;
        }
        // Few strikes light anything; rain puts most out at once.
        let p = w.thunder * 0.2 * (1.0 - (w.precip_mm_h / 10.0).min(0.8));
        if !self.rng.chance(p) {
            return;
        }
        let feet = h.player.mover.pos;
        // Strikes far off light the land where it is dry enough to burn.
        if self.rng.chance(0.3) {
            let a = self.rng.range_f64(0.0, std::f64::consts::TAU);
            let d = self.rng.range_f64(800.0, 6000.0);
            let (x, z) = (feet.x + a.cos() * d, feet.z + a.sin() * d);
            let danger = crate::wildfire::danger(h.env, h.ticks, DVec3::new(x, feet.y, z));
            if danger.level > 0.35 {
                let cell = FarFire::cell_of(x, z);
                let fuel = FarFire::fuel(h.env, h.lw.terrain(), cell);
                self.far_fire.ignite(cell, fuel, h.ticks, h.ticks_per_day);
            }
        }
        for _ in 0..16 {
            let a = self.rng.range_f64(0.0, std::f64::consts::TAU);
            let d = self.rng.range_f64(30.0, 150.0);
            let (x, z) = (
                (feet.x + a.cos() * d).floor() as i32,
                (feet.z + a.sin() * d).floor() as i32,
            );
            if self.strike(h, x, z) {
                h.out.push(acted(
                    "",
                    true,
                    "Lightning strikes close by. Smoke rises from a tree.",
                ));
                return;
            }
        }
    }

    /// Lightning strikes the column at `(x, z)`: a tree there catches and burns for hours.
    /// Whether anything caught.
    pub fn strike(&mut self, h: &mut Here, x: i32, z: i32) -> bool {
        let Some(top) = h.lw.map.sky_top(x, z) else {
            return false;
        };
        let at = BlockPos::new(x, top, z);
        let Some(state) = h.lw.map.block(at) else {
            return false;
        };
        let content = h.lw.content.clone();
        let wood =
            h.lw.reg
                .block_of(state)
                .def
                .material
                .as_deref()
                .and_then(|m| content.materials.get(m))
                .is_some_and(|m| {
                    m.category == hearth_content::schema::material::MaterialCategory::Wood
                });
        if !wood {
            return false;
        }
        // The struck tree's burning limbs fall: the fire burns at its foot, beside the trunk.
        let is_wood = |h: &Here, p: BlockPos| {
            h.lw.map
                .block(p)
                .and_then(|s| h.lw.reg.block_of(s).def.material.clone())
                .and_then(|m| content.materials.get(&m).map(|m| m.category))
                .is_some_and(|c| c == hearth_content::schema::material::MaterialCategory::Wood)
        };
        let mut base = at;
        while base.y > at.y - 40 && (is_wood(h, base.down()) || Self::open(h, base.down())) {
            base = base.down();
        }
        let mut spot = None;
        'find: for r in 1..=3 {
            for (dx, dz) in [
                (r, 0),
                (-r, 0),
                (0, r),
                (0, -r),
                (r, r),
                (-r, -r),
                (r, -r),
                (-r, r),
            ] {
                for dy in -2..=2 {
                    let p = BlockPos::new(base.x + dx, base.y + dy, base.z + dz);
                    let ground =
                        h.lw.map
                            .block(p.down())
                            .is_some_and(|s| !h.lw.reg.collision_shape(s).is_empty());
                    if ground && Self::open(h, p) && !is_wood(h, p) {
                        spot = Some(p);
                        break 'find;
                    }
                }
            }
        }
        let Some(above) = spot else {
            return false;
        };
        let Ok(flames) = h.lw.reg.parse_state("hearth:flames") else {
            return false;
        };
        let mut fire = Fire::laid(
            900.0,
            vec![
                Fuel {
                    kg: 30.0,
                    mj_kg: 18.0,
                    thick_m: 0.3,
                    wet: 0.0,
                };
                3
            ],
        );
        fire.ignite();
        // Struck, the tree is already burning hard.
        fire.involved = 1.0;
        fire.power_kw = 40.0;
        fire.temp_c = 800.0;
        self.set_block(h, above, flames);
        self.wildfires.push(Station {
            pos: above,
            id: WILDFIRE.to_owned(),
            fire: Some(fire),
        });
        // The crown where it was struck catches: in dry weather the fire spreads from it.
        for p in [
            at,
            at.down(),
            BlockPos::new(at.x + 1, at.y - 1, at.z),
            BlockPos::new(at.x - 1, at.y - 1, at.z),
            BlockPos::new(at.x, at.y - 1, at.z + 1),
            BlockPos::new(at.x, at.y - 1, at.z - 1),
        ] {
            self.ignite_at(h, p);
        }
        true
    }
}

/// The skill of throwing true.
pub const THROWING: &str = "throwing";

/// A thing thrown: what, its flight (points from the hand, `dt` seconds apart, to where it
/// fell or struck), its mass (kg). It lies where the flight ends, or where it struck.
pub struct Flight {
    pub stack: Stack,
    pub path: Vec<DVec3>,
    pub dt: f64,
    pub mass: f64,
}

impl Flight {
    /// Its speed (m/s) along a segment of its flight.
    pub fn speed_at(&self, segment: usize) -> f64 {
        match (self.path.get(segment), self.path.get(segment + 1)) {
            (Some(a), Some(b)) => (*b - *a).length() / self.dt,
            _ => 0.0,
        }
    }
}

impl Workshop {
    /// A construction piece put up in the block over the one aimed at, of the material of what
    /// it used, facing the way the person faces (V2-8).
    /// Puts a piece up where the aim puts it, facing the way the builder faces.
    fn place_piece(
        &mut self,
        h: &mut Here,
        def: &hearth_content::schema::process::Process,
        material: Option<&str>,
        aim: AimAt,
    ) {
        let (Some(piece), Some(material)) = (def.places.as_ref(), material) else {
            return;
        };
        let Some(at) = crate::building::spot(&h.lw.map, &h.lw.reg, aim)
            .filter(|at| crate::building::room(&h.lw.map, &h.lw.reg, *at, h.player.mover.pos))
        else {
            h.out
                .push(acted(&def.id, false, "There is no room for it there."));
            return;
        };
        let Some(state) =
            crate::building::piece_state(&h.lw.reg, piece.as_str(), material, h.facing)
        else {
            h.out
                .push(acted(&def.id, false, "It cannot be made of that."));
            return;
        };
        self.set_block(h, at, state);
    }

    /// Why a piece cannot go up where the aim puts it, if it cannot: it would rest on nothing
    /// there (a roof before its frame, a post in the air).
    fn piece_lacks(&self, h: &Here, r: usize, aim: AimAt) -> Option<&'static str> {
        let piece = self.crafts.recipes[r].def.places.as_ref()?;
        let shape = h.lw.content.construction.get(piece.as_str())?.shape;
        let at = crate::building::spot(&h.lw.map, &h.lw.reg, aim)?;
        (!crate::building::rests_at(&h.lw.map, &h.lw.reg, &h.lw.content, shape, at))
            .then_some("It would have nothing to rest on there.")
    }
}

/// The way (north, east, south or west) nearest a facing (radians: 0 toward +z, which is south,
/// turning toward +x, east).
/// Whether a thing lying about has rotted away: long rotten, a carcass is gone to the
/// scavengers, and meat and other food to the flies and the beetles.
pub(crate) fn rotted_away(content: &Content, items: &Items, s: &Stack) -> bool {
    s.decay >= 1.5
        && items
            .get(&s.id)
            .is_some_and(|k| k.has_tag("carcass") || keeps_days(content, k).is_some())
}

pub(crate) fn facing_name(facing: f32) -> &'static str {
    let a = facing.rem_euclid(std::f32::consts::TAU);
    let q = ((a / std::f32::consts::FRAC_PI_2).round() as i32).rem_euclid(4);
    match q {
        0 => "south",
        1 => "east",
        2 => "north",
        _ => "west",
    }
}

/// A direction in words, from a step east (`dx`) and south (`dz`).
pub(crate) fn compass(dx: f64, dz: f64) -> &'static str {
    let a = dx.atan2(-dz).rem_euclid(std::f64::consts::TAU);
    const NAMES: [&str; 8] = [
        "north",
        "north-east",
        "east",
        "south-east",
        "south",
        "south-west",
        "west",
        "north-west",
    ];
    NAMES[((a / std::f64::consts::TAU * 8.0).round() as usize) % 8]
}

/// What burns of a thing laid on a fire.
fn fuel_of(c: &Content, items: &Items, s: &Stack) -> Fuel {
    let kind = items.get(&s.id);
    let mj = kind
        .and_then(|k| k.material.as_deref())
        .and_then(|m| c.materials.get(m))
        .and_then(|m| m.fuel_mj_kg)
        .unwrap_or(17.0);
    let thick = kind.map_or(0.02, |k| {
        let d = k.size_m.iter().copied().fold(f32::INFINITY, f32::min);
        if k.has_tag("tinder") {
            0.002
        } else {
            d.max(0.002)
        }
    });
    Fuel {
        kg: s.mass(items),
        mj_kg: mj,
        thick_m: thick,
        wet: s.wet,
    }
}

/// What was made, in words.
fn made_words(items: &Items, made: &[Stack], name: &str) -> String {
    if made.is_empty() {
        return format!("{name}: done.");
    }
    let mut parts: Vec<String> = Vec::new();
    for s in made {
        let n = items
            .get(&s.id)
            .map_or(s.id.clone(), |k| k.name.to_lowercase());
        if s.count > 1 {
            parts.push(format!("{} {n}", s.count));
        } else {
            parts.push(n);
        }
    }
    format!("{name}: {}.", parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn food_left_to_rot_is_gone_and_a_tool_is_not() {
        let content = Content::load_base();
        let items = Items::from_content(&content);
        let mut meat = Stack::one("hearth:cut/meat");
        meat.decay = 1.2;
        assert!(
            !rotted_away(&content, &items, &meat),
            "spoiled, still lying there"
        );
        meat.decay = 1.55;
        assert!(rotted_away(&content, &items, &meat), "rotten, gone");
        let mut axe = Stack::one("hearth:hand_axe/flint");
        axe.decay = 1.6;
        assert!(!rotted_away(&content, &items, &axe), "stone does not rot");
    }
}

/// A band's camp as its people keep it (H8): where it is, whether its people are about it to
/// tend the fire, and its beds — how many, and whether of furs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CampKept {
    pub at: DVec3,
    pub tend: bool,
    pub beds: u8,
    pub fur: bool,
}

/// How near a camp's place its fire stands (m).
const CAMP_FIRE_M: f64 = 8.0;

/// Dry wood of a thickness for a fire.
fn dry_wood(kg: f32, thick_m: f32) -> Fuel {
    Fuel {
        kg,
        mj_kg: 17.0,
        thick_m,
        wet: 0.1,
    }
}

/// A workstation's id by its key ("campfire").
fn station_named(content: &Content, key: &str) -> Option<String> {
    content
        .workstations
        .iter()
        .find(|w| triggers::key(&w.id) == key)
        .map(|w| w.id.clone())
}

/// Where a camp's fire and beds stand (see [`Workshop::keep_camp`]): the fire on the open ground
/// at the camp, a bed for each household in a ring a few steps out (as many as find open ground
/// there), and the plants of its floor, trodden down about them.
pub struct CampLayout {
    pub fire: BlockPos,
    pub beds: Vec<BlockPos>,
    pub trodden: Vec<BlockPos>,
}

/// How far about its fire a camp's floor is trodden clear (m).
const CAMP_FLOOR_M: f64 = 4.5;

pub fn camp_layout(lw: &LocalWorld, c: &CampKept) -> Option<CampLayout> {
    let fire = open_ground(lw, c.at)?;
    let n = c.beds.min(10) as usize;
    let mut beds = Vec::new();
    for k in 0..n {
        let a = (k as f64 + 0.5) / n as f64 * std::f64::consts::TAU;
        let r = 2.5 + 0.5 * (k % 2) as f64;
        let at = center(fire) + DVec3::new(a.cos() * r, 0.0, a.sin() * r);
        if let Some(p) = open_ground(lw, at)
            && p != fire
            && !beds.contains(&p)
        {
            beds.push(p);
        }
    }
    // The plants standing on the floor about the fire (the ground's own block over it, and a
    // tall plant's upper half).
    let mut trodden = Vec::new();
    let r = CAMP_FLOOR_M.ceil() as i32;
    for dz in -r..=r {
        for dx in -r..=r {
            if ((dx * dx + dz * dz) as f64) > CAMP_FLOOR_M * CAMP_FLOOR_M {
                continue;
            }
            let at = center(fire) + DVec3::new(dx as f64, 0.0, dz as f64);
            let Some(p) = open_ground(lw, at) else {
                continue;
            };
            for up in 0..2 {
                let q = BlockPos::new(p.x, p.y + up, p.z);
                if q == fire || beds.contains(&q) {
                    continue;
                }
                if let Some(s) = lw.map.block(q)
                    && s != BlockStateId::AIR
                    && lw.reg.has(s, hearth_world::StateFlags::REPLACEABLE)
                    && !lw.reg.has(s, hearth_world::StateFlags::FLUID)
                {
                    trodden.push(q);
                }
            }
        }
    }
    Some(CampLayout {
        fire,
        beds,
        trodden,
    })
}

/// The free place on the ground about a point: the block over the solid ground there, if it is
/// loaded and dry, and air or a plant to tread down (not up in a tree).
fn open_ground(lw: &LocalWorld, at: DVec3) -> Option<BlockPos> {
    let (x, z) = (at.x.floor() as i32, at.z.floor() as i32);
    let y0 = at.y.round() as i32;
    for y in (y0 - 8..=y0 + 4).rev() {
        let p = BlockPos::new(x, y, z);
        let s = lw.map.block(p)?;
        if lw.reg.has(s, hearth_world::StateFlags::FULL_COLLISION) {
            if lw.reg.has(s, hearth_world::StateFlags::LAYER_CUTOUT) {
                return None;
            }
            let above = lw.map.block(p.up())?;
            let free = above == BlockStateId::AIR
                || (lw.reg.has(above, hearth_world::StateFlags::REPLACEABLE)
                    && !lw.reg.has(above, hearth_world::StateFlags::FLUID));
            return free.then_some(p.up());
        }
    }
    None
}

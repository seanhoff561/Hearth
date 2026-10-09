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
//! * Things carried and lying **go off**, and embers burn down.
//! * **Digging**: loose earth dug out falls in a spoil pile beside the hole and slumps to its
//!   angle of repose (a block's step of two slides down).

use std::sync::Arc;

use glam::DVec3;
use hearth_body::BodyConfig;
use hearth_content::Content;
use hearth_content::schema::process::{Effect, Process};
use hearth_content::schema::station::Capability;
use hearth_content::triggers::{self, block_keys, item_keys, key, with_verb};
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
    /// The fields' plots (V2-12).
    #[serde(default)]
    plots: Vec<crate::fields::Plot>,
    /// Work left part done (E §7.2): the process, what it was done to, and the share done.
    #[serde(default)]
    begun: Vec<(String, String, f64)>,
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
    /// The hand doing it (P §5.2), whose tools are used.
    pub with: Option<hearth_items::Hand>,
}

/// The world the workshop acts in, for one call.
pub struct Here<'a> {
    pub lw: &'a mut LocalWorld,
    pub items: &'a Items,
    pub cfg: &'a BodyConfig,
    pub env: &'a EnvSampler,
    pub moment: Moment,
    pub ticks: u64,
    pub ticks_per_day: f64,
    /// Game days in a year.
    pub days_per_year: f64,
    pub player: &'a mut Player,
    pub world_items: &'a mut WorldItems,
    /// Blocks changed, for meshing and lighting.
    pub changed: &'a mut Vec<BlockPos>,
    pub out: &'a mut Vec<ToClient>,
    /// The things lying about changed.
    pub items_changed: &'a mut bool,
    /// Which way the person faces (radians: 0 toward +z, turning toward +x).
    pub facing: f32,
    /// The animals (V2-12: the kept ones are worked with).
    pub fauna: &'a mut crate::fauna::Fauna,
}

/// The firing range of what a stack is made of (°C), if it fires (clay).
fn firing_range(content: &Content, items: &Items, stack: &Stack) -> Option<(f32, f32)> {
    let m = items.get(&stack.id)?.material.as_deref()?;
    content.materials.get(m)?.thermal.firing_c
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
    /// Work left part done, by process and what it was done to: the share done (E §7.2).
    begun: FxHashMap<(String, String), f64>,
    /// The crops there are, and the fields' plots by their tilled block (V2-12).
    pub crops: Arc<crate::fields::Crops>,
    pub plots: FxHashMap<BlockPos, crate::fields::Plot>,
    pub work: Option<Work>,
    /// The hand doing the work being planned (its tools are that hand's), if one is.
    tool_hand: Option<hearth_items::Hand>,
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
    /// The ground dug so far by the dig in hand (m³ of `ground::DIG_M3`).
    dug: f64,
    /// Where the player's look met the ground when the dig in hand began: the patch its preview
    /// showed (T §2.3), each stroke taken there.
    dig_point: Option<DVec3>,
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
mod creative;
mod ground;

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
/// turned with it (logs lie along the fall, limbs' joins turn), and whether it is of the stem.
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
            // The stem: logs, or a slim tree's thick limbs (8 or 12 px through).
            let stem = match turned {
                Part::Log { .. } => true,
                Part::Branch { thickness, .. } => thickness >= 8,
                Part::Leaves => false,
            };
            (q, blocks.state(turned), stem)
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
            begun: save
                .begun
                .into_iter()
                .map(|(p, k, d)| ((p, k), d))
                .collect(),
            crops: Arc::new(crate::fields::Crops::from_content(content)),
            plots: save.plots.into_iter().map(|p| (p.pos, p)).collect(),
            work: None,
            tool_hand: None,
            rng: Rng::new(seed ^ 0xc4af7),
            heard_at: FxHashMap::default(),
            last_update: ticks,
            knowledge_changed: true,
            tasted_salt: None,
            falls: Vec::new(),
            dug: 0.0,
            dig_point: None,
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
        let mut plots: Vec<crate::fields::Plot> = self.plots.values().cloned().collect();
        plots.sort_by_key(|p| p.pos);
        let mut begun: Vec<(String, String, f64)> = self
            .begun
            .iter()
            .map(|((p, k), d)| (p.clone(), k.clone(), *d))
            .collect();
        begun.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        WorkshopSave {
            stations: self.stations.clone(),
            wildfires: self.wildfires.clone(),
            harvests,
            depleted,
            plots,
            begun,
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
            AimAt::Animal(id) => {
                let a = h
                    .fauna
                    .live
                    .animals
                    .iter()
                    .find(|a| a.id == id && !a.dead)?;
                let sp = &h.fauna.eco.catalog.species[a.species as usize];
                Some(Aimed::Animal {
                    species: sp.id.clone(),
                    kept: a.kept.is_some(),
                    young: a.stage != hearth_fauna::live::Stage::Adult,
                    female: a.female,
                    domesticable: sp.domestication.is_some(),
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
            AimAt::Animal(id) => h
                .fauna
                .live
                .animals
                .iter()
                .find(|a| a.id == id)
                .map_or(h.player.mover.pos, |a| a.pos),
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
            altitude_m: (at.y / h.lw.generator.terrain.vertical_scale() as f64) as f32,
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
        )
        .by_hand(self.tool_hand);
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

    /// Starts the player doing a process; `point` is where on what is aimed at the look rests
    /// (the ground a dig takes from).
    pub fn act(
        &mut self,
        h: &mut Here,
        process: &str,
        aim: AimAt,
        point: Option<DVec3>,
        hand: Option<f32>,
        with: Option<hearth_items::Hand>,
    ) {
        let Some(r) = self.crafts.index_of(process) else {
            return;
        };
        self.tool_hand = with;
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
        if let AimAt::Animal(id) = aim {
            let eye = h.player.mover.pos + DVec3::new(0.0, 1.6, 0.0);
            let near = h
                .fauna
                .live
                .animals
                .iter()
                .find(|a| a.id == id && !a.dead)
                .is_some_and(|a| (a.pos + DVec3::new(0.0, 0.5, 0.0) - eye).length() <= 4.0);
            if !near {
                h.out.push(acted(process, false, "Out of reach."));
                return;
            }
        }
        let p = match self.plan_now(h, r, aim) {
            Ok(p) => p,
            Err(l) => {
                h.out.push(acted(process, false, lack_words(&l)));
                return;
            }
        };
        // What it would come to with the animal, weighed before the work begins.
        if let AimAt::Animal(id) = aim {
            let effect = self.crafts.recipes[r].def.effect;
            if let Err(why) = self.on_animal(h, process, effect, id, false) {
                h.out.push(acted(process, false, why));
                return;
            }
        }
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
        let needed = p.hours as f64 * 3600.0 * hearth_content::time::TICKS_PER_SECOND * section;
        // Work left part done is taken up where it was left (E §7.2).
        let done = Self::begun_key(aim)
            .and_then(|k| self.begun.get(&(self.crafts.recipes[r].def.id.clone(), k)))
            .copied()
            .unwrap_or(0.0);
        // A dig taken up again has dug its share already.
        self.dug = done * ground::DIG_M3;
        self.dig_point = point;
        self.work = Some(Work {
            recipe: r,
            aim,
            ticks: done * needed.max(1.0),
            needed: needed.max(1.0),
            at: h.player.mover.pos,
            hand: hand.map(|q| q.clamp(0.0, 1.0)),
            with,
        });
        let left_s = needed * (1.0 - done) / hearth_content::time::TICKS_PER_SECOND;
        h.out
            .push(ToClient::Work(Some(self.work_view(done as f32, left_s))));
    }

    fn work_view(&self, done: f32, play_s_left: f64) -> WorkView {
        let r = self.work.as_ref().map_or(0, |w| w.recipe);
        let def = &self.crafts.recipes[r].def;
        WorkView {
            process: def.id.clone(),
            action: def.action.clone(),
            done,
            play_s_left,
            work: def.work.clone(),
            with: self.work.as_ref().and_then(|w| w.with),
        }
    }

    /// Stops the work in hand: what is done of it stays done, to be taken up again.
    pub fn stop(&mut self, h: &mut Here) {
        if let Some(w) = self.work.take() {
            self.keep_begun(&w);
            h.out.push(ToClient::Work(None));
        }
    }

    /// Looking closely at work left to itself (E §7.5): what has happened to it and how it looks
    /// now, as the player can tell; what they know to expect, if they know the work; the
    /// numbers too when asked (Creative, Developer mode).
    pub fn inspect(&mut self, h: &mut Here, id: u64, numbers: bool) {
        use hearth_content::schema::process::StateModel;
        let Some(wi) = h.world_items.get(id) else {
            return;
        };
        let (Some(work), Some(kind)) = (wi.work.clone(), h.items.get(&wi.stack.id)) else {
            h.out
                .push(acted("inspect", true, "It is just lying there."));
            return;
        };
        let Some(r) = self.crafts.index_of(&work.process) else {
            return;
        };
        let def = &self.crafts.recipes[r].def;
        let knows = def
            .knowledge
            .as_ref()
            .is_none_or(|k| h.player.knowledge.knows(k.as_str()));
        let name = kind.name.to_lowercase();
        let mut words = vec![format!(
            "The {name} has been left to {} {}.",
            def.action.split_whitespace().next().unwrap_or("work"),
            about_hours(work.age_h)
        )];
        let pos = DVec3::from_array(wi.pos);
        match def.state {
            Some(StateModel::Drying {
                from,
                to,
                rate_per_h,
            }) => {
                // Not yet weathered a minute: as wet as it began.
                let moisture = if work.moisture > 0.0 {
                    work.moisture
                } else {
                    from
                };
                let done = ((from - moisture.max(to)) / (from - to).max(1e-3)).clamp(0.0, 1.0);
                words.push(
                    match done {
                        d if d < 0.2 => "It is still wet through.",
                        d if d < 0.55 => {
                            "The thin edges are drying; the thick parts are still soft."
                        }
                        d if d < 0.9 => "It is leathery and stiff, nearly dry.",
                        _ => "It is dry through.",
                    }
                    .to_owned(),
                );
                if work.wet_hours > 0.5 {
                    words.push(format!("Rain has wet it {}.", about_hours(work.wet_hours)));
                }
                if knows && done < 1.0 {
                    let w = h.env.weather_at(&h.moment, pos);
                    let sunny = !covered_at(&h.lw.map, BlockPos::containing(pos))
                        && sun_on(h.env, &h.moment, pos, &w);
                    let k = rate_per_h * drying_pace(&w, sunny);
                    let eq = equilibrium(to, &w);
                    words.push(if eq >= to {
                        "In air this damp it will dry no further.".to_owned()
                    } else {
                        let left = ((moisture - eq).max(1e-3) / (to - eq)).ln() / k;
                        format!(
                            "In weather like this it would want {} more.",
                            about_hours(left.max(0.5))
                        )
                    });
                }
            }
            Some(StateModel::Soaking { .. }) => {
                let left = (def.duration.hours - work.hours).max(0.0);
                words.push(if left > def.duration.hours * 0.5 {
                    "It is soaking; little has changed yet.".to_owned()
                } else {
                    "It has softened in the water.".to_owned()
                });
                if knows {
                    words.push(format!("About {} more would do it.", about_hours(left)));
                }
            }
            Some(StateModel::Firing) => {
                words.push(if work.peak_c >= 700.0 {
                    "The fire has glowed red-hot about it.".to_owned()
                } else if work.peak_c >= 400.0 {
                    "The fire is hot about it, but not yet glowing.".to_owned()
                } else {
                    "The fire has barely warmed it.".to_owned()
                });
            }
            None => {}
        }
        if numbers {
            words.push(format!(
                "[{:.1} of {:.1} h; moisture {:.2} kg/kg; peak {:.0} °C, hot {:.1} h; wet {:.1} h, sun {:.1} h]",
                work.hours, def.duration.hours, work.moisture, work.peak_c, work.hot_h,
                work.wet_hours, work.sun_h
            ));
        }
        h.out.push(acted("inspect", true, words.join(" ")));
    }

    /// What work is done to, as work left part done is kept (none: it is not kept: a blow at
    /// an animal, a catch).
    fn begun_key(aim: AimAt) -> Option<String> {
        match aim {
            AimAt::Block { pos, .. } | AimAt::Beside { pos, .. } => {
                Some(format!("block {} {} {}", pos.x, pos.y, pos.z))
            }
            AimAt::Thing(id) => Some(format!("thing {id}")),
            AimAt::Nothing => Some("in hand".to_owned()),
            AimAt::Animal(_) => None,
        }
    }

    /// Keeps the share done of work stopped part way.
    fn keep_begun(&mut self, w: &Work) {
        let share = (w.ticks / w.needed.max(1.0)).clamp(0.0, 1.0);
        if share > 0.0
            && let Some(k) = Self::begun_key(w.aim)
        {
            let id = self.crafts.recipes[w.recipe].def.id.clone();
            self.begun.insert((id, k), share);
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
                if let Some(w) = self.work.take() {
                    self.keep_begun(&w);
                }
                h.out.push(ToClient::Work(None));
                if moved {
                    h.out.push(acted(
                        "",
                        false,
                        "You leave the work; what is done of it stays done.",
                    ));
                }
            } else {
                w.ticks += advanced;
                // A dig takes its earth stroke by stroke (S §8.3).
                let digging = match (self.crafts.recipes[w.recipe].def.effect, w.aim) {
                    (Effect::Excavate, AimAt::Block { pos, .. }) => Some(pos),
                    _ => None,
                };
                let share = (w.ticks / w.needed.max(1.0)).min(1.0);
                if let Some(pos) = digging {
                    let want = share * ground::DIG_M3 - self.dug;
                    if want >= hearth_world::ground::STEP as f64 {
                        self.dug += self.dig_ground(h, pos, want);
                    }
                }
                let Some(w) = self.work.as_mut() else {
                    return;
                };
                if w.ticks >= w.needed {
                    let w = self.work.take().expect("work");
                    if let Some(k) = Self::begun_key(w.aim) {
                        let id = self.crafts.recipes[w.recipe].def.id.clone();
                        self.begun.remove(&(id, k));
                    }
                    h.out.push(ToClient::Work(None));
                    self.tool_hand = w.with;
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
        // The world, a game minute at a time — an hour at a time when a day or more has passed
        // at a stroke, a day at a time when a month has (the Observer's fast-forward, D210).
        let minute = (h.ticks_per_day / 1440.0).max(1.0);
        while (h.ticks.saturating_sub(self.last_update)) as f64 >= minute {
            let behind = h.ticks.saturating_sub(self.last_update) as f64 / h.ticks_per_day.max(1.0);
            let (step, hours) = if behind > 30.0 {
                (minute * 1440.0, 24.0)
            } else if behind > 1.0 {
                (minute * 60.0, 1.0)
            } else {
                (minute, 1.0 / 60.0)
            };
            self.last_update += step as u64;
            self.minute(h, hours);
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
        // What it does to its target (the milk or wool an animal gives, kg).
        let mut animal_kg: Option<f32> = None;
        match (o.effect, aim) {
            (Effect::Remove, AimAt::Block { pos, .. }) => {
                self.set_block(h, pos, BlockStateId::AIR);
                self.settle_ground(h, pos, 2);
            }
            (Effect::Excavate, AimAt::Block { pos, .. }) => {
                // What the strokes left (all of it, done at a stroke in Creative).
                let rest = ground::DIG_M3 - self.dug;
                self.dig_ground(h, pos, rest);
                self.dug = 0.0;
            }
            (Effect::Level, AimAt::Block { pos, .. }) => {
                self.level_ground(h, pos);
            }
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
                    self.settle_ground(h, pos, 2);
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
            (Effect::Till, AimAt::Block { pos, .. }) => self.till(h, pos),
            (Effect::Sow, AimAt::Block { pos, .. }) => self.sow(h, &def.id, pos, &taken),
            (Effect::Weed, AimAt::Block { pos, .. }) => {
                let at = self.plot_at(h, pos);
                match at.and_then(|p| self.plots.get_mut(&p)) {
                    Some(plot) => {
                        plot.weeds = plot.weeds.min(0.05);
                        h.out.push(acted(&def.id, true, "Weeded."));
                    }
                    None => h.out.push(acted(&def.id, false, "There is no field here.")),
                }
            }
            (Effect::Manure, AimAt::Block { pos, .. }) => {
                let at = self.plot_at(h, pos);
                match at.and_then(|p| self.plots.get_mut(&p)) {
                    Some(plot) => {
                        plot.n = (plot.n + crate::fields::DUNG_N).min(1.5);
                        h.out
                            .push(acted(&def.id, true, "The dung is spread and dug in."));
                    }
                    None => h.out.push(acted(&def.id, false, "There is no field here.")),
                }
            }
            (Effect::Reap, AimAt::Block { pos, .. }) => self.reap(h, &def.id, pos),
            (
                Effect::Catch
                | Effect::Tether
                | Effect::Lead
                | Effect::Milk
                | Effect::Pluck
                | Effect::Slaughter,
                AimAt::Animal(id),
            ) => match self.on_animal(h, &def.id, o.effect, id, true) {
                Ok(Some(kg)) => animal_kg = Some(kg),
                Ok(None) => {}
                Err(why) => {
                    h.out.push(acted(&def.id, false, why));
                    return;
                }
            },
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
        // Seed keeps its lot through what is made of it (V2-12); an animal gives what it has.
        let made = match animal_kg {
            Some(kg) => self.animal_gives(h, &def, kg),
            None => self.lots(h, &def, &taken, o.made.clone()),
        };
        // Firsts go in the journal.
        for s in &made {
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
        let words = made_words(h.items, &made, &def.name);
        Self::give(h, made);
        // The fields' and the herds' doings tell what came of them themselves.
        if !matches!(
            o.effect,
            Effect::Till
                | Effect::Sow
                | Effect::Reap
                | Effect::Weed
                | Effect::Manure
                | Effect::Catch
                | Effect::Tether
                | Effect::Lead
                | Effect::Milk
                | Effect::Pluck
                | Effect::Slaughter
        ) {
            h.out.push(acted(&def.id, true, words));
        }
    }

    /// The plot a block aimed at belongs to: the tilled block itself, or the one under a crop.
    fn plot_at(&self, h: &Here, pos: BlockPos) -> Option<BlockPos> {
        if self.plots.contains_key(&pos) {
            return Some(pos);
        }
        let below = pos.down();
        let name =
            h.lw.map
                .block(pos)
                .map(|s| h.lw.reg.block_of(s).name.path().to_owned())?;
        (self.crops.by_block(&name).is_some() && self.plots.contains_key(&below)).then_some(below)
    }

    /// Breaks up a block of soil for a field (V2-12): it becomes a plot.
    fn till(&mut self, h: &mut Here, pos: BlockPos) {
        let Ok(tilled) = h.lw.reg.parse_state("hearth:tilled_soil") else {
            return;
        };
        let material =
            h.lw.map
                .block(pos)
                .and_then(|s| h.lw.reg.block_of(s).def.material.clone());
        let n = crate::fields::soil_n(&h.lw.content, material.as_deref());
        // What grew on it is cleared.
        if h.lw
            .map
            .block(pos.up())
            .is_some_and(|s| h.lw.reg.block_of(s).def.replaceable)
        {
            self.set_block(h, pos.up(), BlockStateId::AIR);
        }
        self.set_block(h, pos, tilled);
        let day = h.moment.days;
        let plot = self
            .plots
            .entry(pos)
            .or_insert_with(|| crate::fields::Plot::tilled(pos, n, day));
        plot.weeds = 0.0;
        plot.since = day;
        h.out.push(acted(
            "hearth:till_soil",
            true,
            "The ground is broken for sowing.",
        ));
    }

    /// Sows the seed taken in the plot aimed at, if it is the time of year for it; else the seed
    /// is kept.
    fn sow(&mut self, h: &mut Here, process: &str, pos: BlockPos, taken: &[Stack]) {
        let Some(seed) = taken.first().cloned() else {
            return;
        };
        let refuse = |h: &mut Here, words: &str| {
            Self::give(h, vec![seed.clone()]);
            h.out.push(acted(process, false, words));
        };
        let Some(at) = self.plot_at(h, pos) else {
            refuse(h, "Sow in tilled ground.");
            return;
        };
        let material = h.items.get(&seed.id).and_then(|k| k.material.clone());
        let Some(crop) = material
            .as_deref()
            .and_then(|m| self.crops.by_grain(m))
            .cloned()
        else {
            refuse(h, "That is no seed to sow.");
            return;
        };
        if self.plots.get(&at).is_some_and(|p| p.growing.is_some()) {
            refuse(h, "Something is sown here already.");
            return;
        }
        let feet = DVec3::new(at.x as f64 + 0.5, at.y as f64, at.z as f64 + 0.5);
        let southern = h.lw.map.planet().latitude_deg(feet.z) < 0.0;
        let local = crate::fields::local_year(&h.moment, southern);
        let days_per_year = h.days_per_year;
        let (ripe_in, spring) = match crate::fields::sowing(&crop, local, days_per_year) {
            Ok(x) => x,
            Err(why) => {
                refuse(h, why);
                return;
            }
        };
        let normals = h.env.normals(feet);
        let coldest = normals.t_mean - normals.t_range / 2.0;
        let lot = seed.lot.unwrap_or(crop.wild);
        let day = h.moment.days;
        let Some(plot) = self.plots.get_mut(&at) else {
            return;
        };
        plot.sow(
            &crop,
            lot,
            day,
            ripe_in,
            spring,
            coldest < -8.0,
            &mut self.rng,
        );
        if let Ok(state) = h.lw.reg.parse_state(&format!("{}[stage=0]", crop.block)) {
            self.set_block(h, at.up(), state);
        }
        let when = if spring { "this summer" } else { "next summer" };
        h.out.push(acted(
            process,
            true,
            format!(
                "Sown with {}: it should ripen {when}.",
                crop.name.to_lowercase()
            ),
        ));
    }

    /// Reaps the crop aimed at: sheaves as the plot bore and the reaping kept, its lot the
    /// reaping's; a crop still green gives nothing.
    fn reap(&mut self, h: &mut Here, process: &str, pos: BlockPos) {
        let Some(at) = self.plot_at(h, pos) else {
            h.out.push(acted(process, false, "There is no crop here."));
            return;
        };
        let Some(g) = self.plots.get(&at).and_then(|p| p.growing.clone()) else {
            h.out.push(acted(process, false, "Nothing grows here."));
            return;
        };
        let Some(crop) = self.crops.get(&g.crop).cloned() else {
            return;
        };
        let day = h.moment.days;
        let after = day - g.ripe;
        if after < -0.25 * (g.ripe - g.sown) {
            h.out.push(acted(
                process,
                false,
                "The ears are still green: there is nothing in them yet.",
            ));
            return;
        }
        let feet = DVec3::new(at.x as f64 + 0.5, at.y as f64, at.z as f64 + 0.5);
        let normals = h.env.normals(feet);
        let irrigated = self.irrigated(h, at);
        let fit = crate::fields::climate_fit(&crop, &normals);
        let water = crate::fields::watered(&crop, &normals, irrigated);
        let Some(plot) = self.plots.get(&at).cloned() else {
            return;
        };
        let bore = crate::fields::bears(&crop, &g, &plot, fit, water);
        let (kg, lot) = crate::fields::reaped(&crop, &g, bore, after);
        // Whole sheaves, rounded at random so the grain comes out right on average.
        let units = kg / crate::fields::SHEAF_GRAIN_KG;
        let mut n = units.floor() as u16;
        if self.rng.next_f32() < units.fract() {
            n += 1;
        }
        let mut made = Vec::new();
        let grain_key = crop
            .grain
            .rsplit(':')
            .next()
            .unwrap_or(&crop.grain)
            .to_owned();
        let sheaf = hearth_content::generate::generated_id(
            "hearth:sheaf_of",
            &format!("hearth:{grain_key}"),
        );
        if n > 0 && h.items.get(&sheaf).is_some() {
            let mut st = Stack::of(&sheaf, n);
            st.lot = Some(lot);
            made.push(st);
        }
        if let Some(f) = &crop.fibre
            && let Some(k) = self.crafts.bulk_item(h.items, f)
        {
            let units = (0.3 / k.mass_kg.max(1e-3)).round().max(1.0) as u16;
            made.push(Stack::of(&k.id, units));
        }
        if let Some(p) = self.plots.get_mut(&at) {
            p.reap_done(&crop, bore, day);
        }
        self.set_block(h, at.up(), BlockStateId::AIR);
        let state = if after < 0.0 {
            "green"
        } else if after < 1.0 {
            "ripe"
        } else {
            "late"
        };
        h.out.push(acted(
            process,
            n > 0,
            format!(
                "Reaped {state}: {n} sheaf{} ({kg:.2} kg of grain); {:.0} % of the ears held their grain.",
                if n == 1 { "" } else { "s" },
                crate::fields::held(&g.lot, after) * 100.0
            ),
        ));
        Self::give(h, made);
    }

    /// What a process's effect on a live animal comes to (V2-12), done only if `commit`: the
    /// milk or wool it gives (kg), or why it cannot be done; said when done.
    fn on_animal(
        &mut self,
        h: &mut Here,
        process: &str,
        effect: Effect,
        id: u64,
        commit: bool,
    ) -> Result<Option<f32>, String> {
        let cat = h.fauna.eco.catalog.clone();
        let years = h.fauna.now;
        let name = h
            .fauna
            .live
            .animals
            .iter()
            .find(|a| a.id == id)
            .map_or("animal".to_owned(), |a| {
                cat.species[a.species as usize].name.to_lowercase()
            });
        let live = &mut h.fauna.live;
        let (kg, words) = match effect {
            Effect::Catch => (None, live.catch(&cat, id, years, commit)?),
            Effect::Tether => {
                live.is_kept(id)?;
                if commit {
                    let at = live.tether(id, hearth_fauna::herd::TETHER_M)?;
                    self.drive_stake(h, at);
                }
                (None, format!("The {name} is tethered to a stake."))
            }
            Effect::Lead => {
                let stake = live.lead(id, commit)?;
                if commit && let Some(at) = stake {
                    self.pull_stake(h, at);
                }
                (None, format!("You lead the {name} on a halter."))
            }
            Effect::Milk => {
                let kg = live.milk(&cat, id, years, 1.0 / h.days_per_year.max(1.0), commit)?;
                (Some(kg), format!("Milked: {kg:.2} kg of milk."))
            }
            Effect::Pluck => {
                let kg = live.pluck(&cat, id, years, commit)?;
                (Some(kg), format!("Plucked: {kg:.2} kg of wool."))
            }
            Effect::Slaughter => {
                live.is_kept(id)?;
                if commit {
                    live.slaughter(id)?;
                }
                (None, format!("The {name} is killed, quickly."))
            }
            _ => return Ok(None),
        };
        if commit {
            h.out.push(acted(process, true, words));
        }
        Ok(kg)
    }

    /// What an animal gives (`kg` of the process's first output's material), in its bulk form.
    fn animal_gives(&self, h: &Here, def: &Process, kg: f32) -> Vec<Stack> {
        let Some(m) = def.outputs.first().and_then(|o| match &o.item {
            hearth_content::schema::process::Match::Material(m) => Some(m.as_str().to_owned()),
            _ => None,
        }) else {
            return Vec::new();
        };
        let Some(k) = self.crafts.bulk_item(h.items, &m) else {
            return Vec::new();
        };
        let n = self.crafts.units_for(h.items, &m, kg);
        vec![Stack::of(&k.id, n)]
    }

    /// A stake driven in where a tethered animal stands (into open ground or low plants).
    fn drive_stake(&mut self, h: &mut Here, at: DVec3) {
        let pos = BlockPos::containing(at + DVec3::new(0.0, 0.1, 0.0));
        let open =
            h.lw.map
                .block(pos)
                .is_some_and(|s| s.is_air() || h.lw.reg.block_of(s).def.replaceable);
        if open && let Ok(stake) = h.lw.reg.parse_state("hearth:tether_stake") {
            self.set_block(h, pos, stake);
        }
    }

    /// The stake pulled up when its animal is led away.
    fn pull_stake(&mut self, h: &mut Here, at: DVec3) {
        let pos = BlockPos::containing(at + DVec3::new(0.0, 0.1, 0.0));
        if h.lw
            .map
            .block(pos)
            .is_some_and(|s| h.lw.reg.block_of(s).name.path() == "tether_stake")
        {
            self.set_block(h, pos, BlockStateId::AIR);
        }
    }

    /// Whether a plot has water to hand for irrigation: open water within four metres, at its
    /// level or a little above.
    fn irrigated(&self, h: &Here, at: BlockPos) -> bool {
        for dx in -4..=4 {
            for dz in -4..=4 {
                for dy in -1..=1 {
                    let p = BlockPos::new(at.x + dx, at.y + dy, at.z + dz);
                    if h.lw
                        .map
                        .block(p)
                        .is_some_and(|s| h.lw.reg.block_of(s).def.fluid.is_some())
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Seed keeps its lot through what is made of it (V2-12): grain stripped from a wild stand
    /// is the wild's; threshed, it is its sheaf's; picked over, the plumpest third is the better
    /// and the rest the worse.
    fn lots(
        &mut self,
        h: &Here,
        def: &Process,
        taken: &[Stack],
        mut made: Vec<Stack>,
    ) -> Vec<Stack> {
        let crop_of = |s: &Stack| {
            h.items
                .get(&s.id)
                .and_then(|k| k.material.as_deref())
                .and_then(|m| self.crops.by_grain(m))
                .cloned()
        };
        // The lot of what went in: the taken seed's, mixed by count.
        let mut from: Option<(hearth_items::Lot, f32)> = None;
        for s in taken.iter().filter(|s| s.lot.is_some()) {
            let lot = s.lot.expect("a lot");
            from = Some(match from {
                Some((l, n)) => (l.mixed(&lot, n, s.count as f32), n + s.count as f32),
                None => (lot, s.count as f32),
            });
        }
        // Seed with no lot of its own is the wild's.
        if from.is_none() {
            from = taken
                .iter()
                .find_map(|s| crop_of(s).map(|c| (c.wild, s.count as f32)));
        }
        // Seed is seed: no better or worse made (its lot is what it is), so lots alike pour
        // together.
        for s in made.iter_mut() {
            if crop_of(s).is_some() {
                s.quality = 0.5;
            }
        }
        if def.effect == Effect::Select {
            let mut out = Vec::new();
            for s in made {
                if let (Some(crop), Some((lot, _))) = (crop_of(&s), from)
                    && s.count >= 2
                {
                    let (up, down) = crate::fields::selected(&crop, &lot);
                    let mut best = s.clone();
                    best.count = s.count / 3 + u16::from(s.count % 3 != 0);
                    best.lot = Some(up);
                    let mut rest = s.clone();
                    rest.count = s.count - best.count;
                    rest.lot = Some(down);
                    out.push(best);
                    out.push(rest);
                } else {
                    out.push(s);
                }
            }
            return out;
        }
        for s in made.iter_mut() {
            if s.lot.is_some() {
                continue;
            }
            if let Some(crop) = crop_of(s) {
                s.lot = Some(from.map_or(crop.wild, |(l, _)| l));
            }
        }
        made
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
                    peak_c: 0.0,
                    hot_h: 0.0,
                    moisture: 0.0,
                    sun_h: 0.0,
                    age_h: 0.0,
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
            .filter(|(_, _, stem)| *stem)
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
        let planet = *h.lw.generator.planet();
        if let Some(d) = self.blaze.take_burned(&planet, h.lw.vegetation.year) {
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
            AimAt::Animal(id) => {
                let Some(a) = h.fauna.live.animals.iter().find(|a| a.id == id) else {
                    return;
                };
                let sp = &h.fauna.eco.catalog.species[a.species as usize];
                vec!["animal".to_owned(), key(&sp.id).to_owned()]
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
        let keys =
            h.lw.content
                .items
                .get(&stack.id)
                .map(|d| item_keys(d, &h.lw.content))
                .unwrap_or_default();
        let t: Vec<String> = with_verb("throw", &keys).collect();
        self.hear(h, &t);
        Some(self.launch(h, stack, mass, dir, v0, THROWING))
    }

    /// A bow in hand drawn `drawn_s` seconds and loosed along `dir` (E §3.2): an arrow carried
    /// (in the other hand, a container, anywhere about the body) flies at the speed the bow's
    /// draw gives it.
    pub fn shoot(&mut self, h: &mut Here, dir: DVec3, drawn_s: f32) -> Option<Flight> {
        if !h.player.can_act(h.cfg) {
            return None;
        }
        let items = h.items;
        let bow = [&h.player.carry.right, &h.player.carry.left]
            .into_iter()
            .flatten()
            .filter_map(|s| items.get(&s.id))
            .find(|k| k.primary == Some(hearth_content::schema::item::Use::Draw))?;
        let draw_kg = bow.property("draw_kg").unwrap_or(15.0);
        let Some(at) = h.player.carry.find(&|s| crate::strikes::is_arrow(items, s)) else {
            h.out.push(acted("", false, "There is no arrow to nock."));
            return None;
        };
        let stack = h.player.carry.take(items, &at, Some(1))?;
        let mass = stack.mass(items).max(0.005) as f64;
        let v0 = crate::strikes::bow_speed(draw_kg, drawn_s, mass);
        Some(self.launch(h, stack, mass, dir, v0, ARCHERY))
    }

    /// A thing sent flying from the eye along `dir` at `v0` (m/s), as true as the `skill`'s
    /// practice: its path until it strikes the ground or leaves the world loaded.
    fn launch(
        &mut self,
        h: &mut Here,
        stack: Stack,
        mass: f64,
        dir: DVec3,
        v0: f64,
        skill: &str,
    ) -> Flight {
        // A novice's throw or shot strays three degrees or so, a practised one's half a degree.
        let practice = h.player.knowledge.skill(skill);
        let spread = (3.0 - 2.4 * practice.clamp(0.0, 1.0) as f64).to_radians();
        let dir = dir.normalize_or_zero();
        let side = dir.cross(DVec3::Y).normalize_or(DVec3::X);
        let up = side.cross(dir).normalize_or(DVec3::Y);
        let (a, r) = (
            self.rng.range_f64(0.0, std::f64::consts::TAU),
            self.rng.normal() * spread,
        );
        let dir = (dir + (side * a.cos() + up * a.sin()) * r.tan()).normalize_or_zero();
        h.player.knowledge.practice(skill, 0.05, h.ticks);
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
        Flight {
            stack,
            path,
            dt,
            mass,
        }
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
        self.slake(h, dt_h);
        self.grow_fields(h);
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
            let covered = covered_at(&h.lw.map, block);
            let raining = !covered && h.env.weather_at(&h.moment, pos).precip_mm_h > 0.1;
            let in_water =
                h.lw.map
                    .block(block)
                    .is_some_and(|s| h.lw.reg.block_of(s).def.fluid.is_some());
            let station = def.station.as_ref().and_then(|s| {
                self.stations
                    .iter()
                    .find(|st| st.id == s.as_str() && st.pos == block)
            });
            let at_station = def.station.is_none() || station.is_some();
            // The station's fire: how hot, and whether it burns.
            let fire = station.and_then(|st| st.fire.as_ref());
            let fire_c = fire.map_or(air_c, |f| f.temp_c);
            use hearth_content::schema::process::Condition;
            let mut ok = at_station
                && def.conditions.iter().all(|c| match c {
                    Condition::Dry => !raining,
                    Condition::Water => in_water,
                    Condition::ColdBelowC(t) => air_c < *t,
                    Condition::HeatAtLeastC(t) => fire_c >= *t,
                    _ => true,
                });
            // A firing goes on while its fire burns, its heat kept: the hottest it got, and the
            // hours at or above the bottom of what is fired's range.
            if def.firing.is_some() {
                ok = ok && fire.is_some_and(Fire::lit);
                if ok {
                    work.peak_c = work.peak_c.max(fire_c);
                    let lo =
                        firing_range(&h.lw.content, h.items, &wi.stack).map_or(f32::MAX, |r| r.0);
                    if fire_c >= lo {
                        work.hot_h += dt_h;
                    }
                }
            }
            use hearth_content::schema::process::StateModel;
            work.age_h += dt_h;
            let w = h.env.weather_at(&h.moment, pos);
            let sunny = !covered && sun_on(h.env, &h.moment, pos, &w);
            if sunny {
                work.sun_h += dt_h;
            }
            if raining {
                work.wet_hours += dt_h;
            }
            let finished = match def.state {
                // Drying by the weather (E §7.4): toward the air's own moisture, faster warm,
                // dry, windy and in sun; rain wets it again.
                Some(StateModel::Drying {
                    from,
                    to,
                    rate_per_h,
                }) => {
                    if work.moisture <= 0.0 {
                        work.moisture = from;
                    }
                    if at_station {
                        work.moisture = dry_step(
                            work.moisture,
                            (from, to, rate_per_h),
                            &w,
                            sunny,
                            raining,
                            dt_h,
                        );
                        if !raining {
                            work.hours += dt_h;
                        }
                    }
                    work.moisture <= to
                }
                // Soaking: hours as long as the water's warmth makes them.
                Some(StateModel::Soaking { q10 }) => {
                    if ok {
                        let water_c = (air_c - 2.0).clamp(1.0, 30.0);
                        work.hours += dt_h * q10.powf((water_c - 20.0) / 10.0);
                    }
                    work.hours >= def.duration.hours
                }
                Some(StateModel::Firing) | None => {
                    if ok {
                        work.hours += dt_h;
                    }
                    work.hours >= def.duration.hours
                }
            };
            if finished {
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
            // A firing comes out as its heat made it (v2 §11.4).
            let def = &self.crafts.recipes[r].def;
            let mut quality = None;
            if let Some(firing) = def.firing {
                let name = def.name.clone();
                let range = firing_range(&h.lw.content, h.items, &wi.stack);
                let result = range.map_or(hearth_craft::firing::Fired::Under, |range| {
                    hearth_craft::firing::fired(range, work.peak_c, work.hot_h, firing.hold_h)
                });
                match result {
                    hearth_craft::firing::Fired::Fired(q) => quality = Some(q),
                    hearth_craft::firing::Fired::Under => {
                        // Still clay: back where it lay, to be fired again.
                        h.world_items.add(wi.stack.clone(), wi.pos, wi.yaw);
                        *h.items_changed = true;
                        h.out.push(acted(
                            &work.process,
                            false,
                            format!(
                                "{name}: it never got hot enough to fire through (at most {:.0} °C, \
                                 {:.1} h hot enough): still clay, that rain or water would turn \
                                 back to mud. Fire it again, hotter and longer.",
                                work.peak_c, work.hot_h
                            ),
                        ));
                        continue;
                    }
                    hearth_craft::firing::Fired::Over => {
                        *h.items_changed = true;
                        h.out.push(acted(
                            &work.process,
                            false,
                            format!(
                                "{name}: far too hot ({:.0} °C): the clay slumped and bloated.",
                                work.peak_c
                            ),
                        ));
                        continue;
                    }
                }
            }
            match finish_batch(
                &self.crafts,
                r,
                &wi.stack,
                wet,
                quality,
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

    /// The fields grow (V2-12): each plot brought up to now, its crop drawn at its stage, a crop
    /// left standing too long lost, a plot long left unsown gone back to grass.
    fn grow_fields(&mut self, h: &mut Here) {
        if self.plots.is_empty() {
            return;
        }
        let day = h.moment.days;
        let dpy = h.days_per_year;
        let mut draw: Vec<(BlockPos, Option<String>)> = Vec::new();
        let mut gone = Vec::new();
        for plot in self.plots.values_mut() {
            let before = plot
                .growing
                .as_ref()
                .map(|g| crate::fields::stage(g, plot.day));
            let lost = plot.advance(day, dpy);
            let now = plot.growing.as_ref().map(|g| crate::fields::stage(g, day));
            if lost {
                draw.push((plot.pos.up(), None));
            } else if now != before
                && let (Some(st), Some(g)) = (now, plot.growing.as_ref())
                && let Some(crop) = self.crops.get(&g.crop)
            {
                draw.push((plot.pos.up(), Some(format!("{}[stage={st}]", crop.block))));
            }
            if plot.growing.is_none() && day - plot.since > crate::fields::ABANDONED_YEARS * dpy {
                gone.push(plot.pos);
            }
        }
        for (pos, state) in draw {
            let id = match state {
                Some(s) => h.lw.reg.parse_state(&s).unwrap_or(BlockStateId::AIR),
                None => BlockStateId::AIR,
            };
            self.set_block(h, pos, id);
        }
        for pos in gone {
            self.plots.remove(&pos);
            if let Ok(grass) = h.lw.reg.parse_state("hearth:grass_block") {
                self.set_block(h, pos, grass);
            }
        }
    }

    /// Unfired clay left out in the rain soaks through and slumps back to clay (V2-12): a pot
    /// is pottery only once it has been fired.
    fn slake(&mut self, h: &mut Here, dt_h: f32) {
        let mut slumped = Vec::new();
        for wi in h.world_items.items.iter_mut() {
            let Some(kind) = h.items.get(&wi.stack.id) else {
                continue;
            };
            if !kind.has_tag("slakes") {
                continue;
            }
            let pos = DVec3::from_array(wi.pos);
            let block = BlockPos::containing(pos);
            let covered =
                h.lw.map
                    .sky_top(block.x, block.z)
                    .is_some_and(|top| top > block.y + 1);
            let rain = if covered {
                0.0
            } else {
                h.env.weather_at(&h.moment, pos).precip_mm_h as f32
            };
            if rain > 0.1 {
                // Half an hour of steady rain (2 mm an hour) soaks a pot through.
                wi.stack.wet = (wi.stack.wet + rain * dt_h).min(1.0);
                if wi.stack.wet >= 1.0 {
                    slumped.push(wi.id);
                }
            } else {
                wi.stack.wet = (wi.stack.wet - 0.5 * dt_h).max(0.0);
            }
        }
        for id in slumped {
            let Some(wi) = h.world_items.take(id) else {
                continue;
            };
            let Some(m) = h.items.get(&wi.stack.id).and_then(|k| k.material.clone()) else {
                continue;
            };
            let lump = hearth_content::generate::generated_id("hearth:lump", &m);
            if let Some(k) = h.items.get(&lump) {
                let kg = wi.stack.mass(h.items);
                let n = (kg / k.mass_kg.max(0.01)).round().max(1.0) as u16;
                h.world_items
                    .add(Stack::of(&lump, n.min(64)), wi.pos, wi.yaw);
            }
            if (DVec3::from_array(wi.pos) - h.player.mover.pos).length() < 32.0 {
                let name = h
                    .items
                    .get(&wi.stack.id)
                    .map_or("the clay", |k| k.name.as_str());
                h.out.push(acted(
                    "",
                    false,
                    format!("The rain soaks {name} through: it slumps back to clay."),
                ));
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
        // Storage pits (V2-12): what lies sealed in one keeps, from damp and from mice.
        let pits: Vec<BlockPos> = self
            .stations
            .iter()
            .filter(|s| s.id.ends_with("storage_pit"))
            .map(|s| s.pos)
            .collect();
        let mut gnawed = Vec::new();
        for wi in h.world_items.items.iter_mut() {
            let at = BlockPos::containing(DVec3::from_array(wi.pos));
            let stored = pits.iter().any(|p| *p == at || p.up() == at);
            let pace = match &wi.work {
                Some(work) => {
                    let done = self.crafts.index_of(&work.process).map_or(0.0, |r| {
                        work.hours / self.crafts.recipes[r].def.duration.hours.max(1e-3)
                    });
                    0.25 * (1.0 - done.clamp(0.0, 1.0))
                }
                None if stored => 0.1,
                None => 1.0,
            };
            age(&mut wi.stack, air_c, pace);
            // Grain left lying in the open: the mice and birds find it, a hundredth of it a day.
            let grain = items
                .get(&wi.stack.id)
                .is_some_and(|k| k.has_tag("sowable") || k.has_tag("sheaf"));
            if grain && !stored && wi.work.is_none() {
                let days = dt_h / 24.0;
                let lost = wi.stack.count as f32 * 0.01 * days;
                let mut n = lost.floor() as u16;
                if self.rng.next_f32() < lost.fract() {
                    n += 1;
                }
                if n > 0 {
                    gnawed.push((wi.id, n));
                }
            }
        }
        for (id, n) in gnawed {
            if let Some(w) = h.world_items.get_mut(id) {
                if w.stack.count > n {
                    w.stack.count -= n;
                } else {
                    h.world_items.take(id);
                }
                *h.items_changed = true;
            }
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
/// The skill a bow's shots grow.
pub const ARCHERY: &str = "archery";

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

/// Moisture (kg a kg dry) after `dt_h` hours drying in weather `w` (E §7.4): toward the air's
/// own at the model's pace in this weather; rain wets it again, up to where it began.
fn dry_step(
    moisture: f32,
    (from, to, rate_per_h): (f32, f32, f32),
    w: &hearth_env::WeatherState,
    sunny: bool,
    raining: bool,
    dt_h: f32,
) -> f32 {
    if raining {
        return (moisture + 0.04 * dt_h).min(from);
    }
    let k = rate_per_h * drying_pace(w, sunny);
    let eq = equilibrium(to, w);
    moisture - (moisture - eq).max(0.0) * (1.0 - (-k * dt_h).exp())
}

/// The moisture a thing comes to in the air of `w` (kg a kg dry): its water activity meets the
/// air's relative humidity. `to`, dry through, is reached at three-quarters (the water activity
/// below which moulds and most spoilage stop); in damper air it stays moister.
fn equilibrium(to: f32, w: &hearth_env::WeatherState) -> f32 {
    to * w.humidity as f32 / 0.75
}

/// Whether a place is under a roof or a canopy (something solid above its head).
fn covered_at(map: &hearth_world::CubeMap, b: BlockPos) -> bool {
    map.sky_top(b.x, b.z).is_some_and(|top| top > b.y + 1)
}

/// Whether the sun shines on a place in the open now.
fn sun_on(env: &EnvSampler, moment: &Moment, pos: DVec3, w: &hearth_env::WeatherState) -> bool {
    env.sun_up(moment, pos) && w.cloud_cover < 0.6
}

/// How much faster than the reference (25 °C, half humidity, still air, shade) things dry in
/// this weather: twice for every ten degrees warmer, with the air's dryness, the wind and the
/// sun on it.
fn drying_pace(w: &hearth_env::WeatherState, sunny: bool) -> f32 {
    let warmth = 2f32.powf((w.temperature_c as f32 - 25.0) / 10.0);
    let dryness = ((1.0 - w.humidity as f32) / 0.5).clamp(0.05, 2.0);
    let wind = 1.0 + 0.25 * (w.wind_speed_m_s as f32).min(10.0);
    let sun = if sunny { 1.5 } else { 1.0 };
    (warmth * dryness * wind * sun).clamp(0.02, 8.0)
}

/// Hours in rough words: "about an hour", "about five hours", "about two days".
fn about_hours(h: f32) -> String {
    if h < 1.5 {
        "about an hour".into()
    } else if h < 36.0 {
        format!("about {} hours", h.round() as u32)
    } else {
        format!("about {} days", (h / 24.0).round() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Meat strips on a rack (E §7.4): dry in a day or two of warm, dry, breezy weather with
    /// sun by day; still soft after as long in cool, damp, still air; set back by rain.
    #[test]
    fn meat_dries_by_the_weather() {
        let model = (3.0, 0.35, 0.075);
        let weather = |t: f64, rh: f64, wind: f64| hearth_env::WeatherState {
            cloud_cover: 0.2,
            precip_mm_h: 0.0,
            precip: hearth_env::Precip::None,
            thunder: 0.0,
            temperature_c: t,
            wind_dir: 0.0,
            wind_speed_m_s: wind,
            humidity: rh,
        };
        let days = |w: &hearth_env::WeatherState, rain_every: Option<u32>| {
            let mut m = model.0;
            for hour in 0..24 * 10 {
                let sunny = (8..18).contains(&(hour % 24));
                let raining = rain_every.is_some_and(|n| hour % n == 0);
                m = dry_step(m, model, w, sunny, raining, 1.0);
                if m <= model.1 {
                    return hour as f32 / 24.0;
                }
            }
            f32::INFINITY
        };
        let fair = days(&weather(24.0, 0.4, 3.0), None);
        assert!((0.5..2.5).contains(&fair), "fair weather: {fair} days");
        let damp = days(&weather(12.0, 0.85, 0.5), None);
        assert!(
            damp > 2.0 * fair,
            "cool and damp: {damp} days against {fair}"
        );
        let showers = days(&weather(24.0, 0.4, 3.0), Some(6));
        assert!(
            showers > fair,
            "showers set it back: {showers} against {fair}"
        );
    }

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

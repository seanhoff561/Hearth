//! Hominins in the world (V2-11 (c)): the groups of a hominin's population drawn out as agents
//! near the player and folded back into their numbers when the player goes, and their days —
//! walking and climbing about their range, feeding under the fruit trees and over the open ground,
//! drinking at the water, gathering marula stones and cracking them at their anvils, striking
//! flakes, up into their trees to make their nests at dusk and down again at dawn; calling the
//! alarm at a hunter, facing it together or fleeing up the trees; wary of a person.
//!
//! The world is the game's: it answers through [`AgentWorld`] what lies where (the ground, the
//! trees, the water, the things lying about, what there is to eat), the weather on a body, the
//! hunters about, and hears their calls and nests.

use glam::{DVec2, DVec3};
use hearth_body::{Exposure, Food};
use hearth_content::Content;
use hearth_content::schema::era::Behavior;
use hearth_craft::engine::{Aimed, Surroundings};
use hearth_craft::{Crafts, Graph};
use hearth_fauna::ecology::Ecology;
use hearth_fauna::habitat::CELL_M;
use hearth_fauna::live::{Cell, Ground, Medium, Stage};
use hearth_fauna::nav::{trunk_near, water_near};
use hearth_items::{Items, Stack};
use hearth_math::hash::Rng;

use crate::agent::Agent;
use crate::group::{Culture, Places, SocialGroup};
use crate::kind::{Kind, Kinds};
use crate::mind::{Doing, Intent, Needs, Offer, Situation, Threat, choose};
use crate::work::{REACH_M, Things, finish_work, plan_work};

/// A hominin group within this of the player is drawn out as agents (m); one whose agents are all
/// beyond [`FAR_M`] is folded back into its numbers.
pub const NEAR_M: f64 = 112.0;
pub const FAR_M: f64 = 150.0;
/// How far an agent looks about for a tree, water or food it does not yet know (m).
const LOOK_M: f64 = 40.0;
/// How far it sees a hunter or a person (m), by day.
const SEE_M: f64 = 120.0;
/// How far a hunter or a person may come before it runs, when it has not come to tolerate them.
const FLIGHT_M: f32 = 60.0;
/// A calm person holds its eye within this many times its flight distance; beyond, it goes
/// about its day.
const HEED: f32 = 1.5;
/// How long the body goes between its steps (seconds of play).
const BODY_S: f32 = 1.0;
/// Food eaten a second of feeding, of a minute's worth (kg a minute → per second).
const PER_MIN: f32 = 1.0 / 60.0;
/// Water drunk a second at the water (l).
const DRINK_L_S: f64 = 0.05;
/// Seconds of play of a calm person's company within sight that bring a group to be at ease
/// with them (four play days); and of a person running at them or hunting near them that undo
/// it.
const HABITUATE_S: f32 = 1.5 * 2880.0;
const UNLEARN_S: f32 = 60.0;

/// What there is to eat where an agent stands.
#[derive(Debug, Clone, PartialEq)]
pub struct FoodHere {
    /// The food (a material) and how much a minute of feeding gives (kg).
    pub material: String,
    pub kg_min: f32,
    /// Nuts to gather to crack later (a material: marula stones under a marula).
    pub nuts: Option<String>,
}

/// What the world gives the agents and takes from them.
pub trait AgentWorld {
    /// The ground: footing, the trees' trunks, the water.
    fn ground(&self) -> &dyn Ground;
    /// The things lying about.
    fn things(&mut self) -> &mut dyn Things;
    /// What there is to eat where it stands, this season.
    fn food_at(&self, at: DVec3) -> Option<FoodHere>;
    /// The nearest place within `within` m with food to pick.
    fn food_near(&self, at: DVec3, within: f64) -> Option<DVec3>;
    /// The air and weather on a body at a place (up in a tree's crown or not).
    fn exposure(&self, at: DVec3, in_tree: bool) -> Exposure;
    /// The weather and the season for work at a place.
    fn surroundings(&self, at: DVec3) -> Surroundings;
    /// Hunters of its kind within `within` m of a place.
    fn hunters_near(&self, at: DVec3, within: f64) -> Vec<DVec3>;
    /// A call: the alarm at a hunter, or a contact call.
    fn call(&mut self, at: DVec3, alarm: bool);
    /// A nest bent in a tree's crown about a place: the trace it leaves, and where one lies on
    /// it (none where no nest could be made: it sleeps where it is).
    fn nest(&mut self, at: DVec3) -> Option<DVec3>;
    /// A group of a kind drawn out about a place: the world may lay there the traces of its
    /// past (its anvil and stones under a nut tree, the scatter of its knapping).
    fn settle(&mut self, _at: DVec3, _kind: &Kind) {}
}

/// A person as the agents may notice them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Person {
    pub pos: DVec3,
    /// Coming on fast (running, sprinting).
    pub running: bool,
    /// Hunting near them (a throw, a kill).
    pub hunting: bool,
    /// How plain to see: 1 standing in the open, less crouched or crawling, in cover (they are
    /// noticed within [`SEE_M`] times this).
    pub plain: f32,
}

impl Person {
    /// Someone standing calmly in the open.
    pub fn standing(pos: DVec3) -> Self {
        Self {
            pos,
            running: false,
            hunting: false,
            plain: 1.0,
        }
    }
}

/// The moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Now {
    pub tick: u64,
    /// Local solar time, hours.
    pub hour: f32,
}

/// Something an agent did that someone could see: the process and its triggers, where.
#[derive(Debug, Clone, PartialEq)]
pub struct Done {
    pub agent: u64,
    pub at: DVec3,
    pub recipe: usize,
    pub triggers: Vec<String>,
}

/// An agent as the client draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentView {
    pub id: u64,
    pub kind: usize,
    pub female: bool,
    pub stage: Stage,
    pub pos: DVec3,
    pub yaw: f32,
    pub speed: f32,
    pub medium: Medium,
    pub doing: Doing,
    /// Its height (m), for its figure.
    pub height_m: f32,
}

/// The agents about the player and their groups.
#[derive(Debug, Clone)]
pub struct Hominins {
    pub agents: Vec<Agent>,
    pub groups: Vec<SocialGroup>,
    next_id: u64,
    rng: Rng,
    /// Seconds of play to the bodies' next step.
    body_s: f32,
    /// What was done in the last step that someone could have watched.
    pub done: Vec<Done>,
}

/// The horizontal distance between two places on the planet (wrapping in x).
fn hdist(a: DVec3, b: DVec3, wrap: f64) -> f64 {
    let mut dx = (a.x - b.x).abs();
    if wrap > 0.0 {
        dx = dx.min(wrap - dx);
    }
    dx.hypot(a.z - b.z)
}

impl Hominins {
    pub fn new(seed: u64) -> Self {
        Self {
            agents: Vec::new(),
            groups: Vec::new(),
            next_id: 1,
            rng: Rng::new(seed ^ 0x40a1_1b1d),
            body_s: 0.0,
            done: Vec::new(),
        }
    }

    /// The agents of a group.
    pub fn members(&self, group: u64) -> impl Iterator<Item = &Agent> {
        self.agents.iter().filter(move |a| a.group == group)
    }

    /// Draws out as agents the hominin groups of the ecology within [`NEAR_M`] of the player
    /// that are not drawn out yet: each member of its numbers (its grown females and males, its
    /// half-grown and its young) an agent about the group's place, the group knowing the water,
    /// the trees and the anvils near it.
    #[allow(clippy::too_many_arguments)]
    pub fn materialize(
        &mut self,
        eco: &mut Ecology,
        kinds: &Kinds,
        graph: &Graph,
        items: &Items,
        world: &mut dyn AgentWorld,
        player: DVec3,
        tick: u64,
    ) {
        let wrap = eco.cells_around as f64 * CELL_M;
        let cat = eco.catalog.clone();
        for r in eco.regions.values_mut() {
            for g in r.groups.iter_mut() {
                let sp = &cat.species[g.species as usize];
                if !sp.hominin || g.live || g.size() == 0 {
                    continue;
                }
                let at = DVec3::new(g.pos[0], 0.0, g.pos[1]);
                if hdist(at, player, wrap) > NEAR_M {
                    continue;
                }
                let Some(ki) = kinds
                    .kinds
                    .iter()
                    .position(|k| k.population.as_deref().is_some_and(|p| p == sp.id))
                else {
                    continue;
                };
                let kind = &kinds.kinds[ki];
                let Some(centre) = world.ground().top(g.pos[0], g.pos[1]) else {
                    // Not loaded yet: it waits.
                    continue;
                };
                let mut members: Vec<(Stage, bool)> = Vec::new();
                members.extend((0..g.females).map(|_| (Stage::Adult, true)));
                members.extend((0..g.males).map(|_| (Stage::Adult, false)));
                for _ in 0..g.juveniles {
                    members.push((Stage::Juvenile, self.rng.next_f32() < 0.5));
                }
                for _ in 0..g.young {
                    members.push((Stage::Young, self.rng.next_f32() < 0.5));
                }
                let mut group = SocialGroup {
                    id: g.id,
                    kind: ki,
                    members: Vec::new(),
                    home: DVec2::new(g.home[0], g.home[1]),
                    range_m: (sp.home_range_km2 as f64 / std::f64::consts::PI).sqrt() * 1000.0,
                    places: Places::default(),
                    culture: Culture {
                        techniques: kind.techniques.clone(),
                        traditions: Vec::new(),
                    },
                    population_group: Some(g.id),
                    habituation: g.tolerance,
                    ..SocialGroup::default()
                };
                let here = DVec3::new(g.pos[0], centre.level(), g.pos[1]);
                world.settle(here, kind);
                know_about(&mut group, world, items, here);
                for (stage, female) in members {
                    let a = self.rng.next_f64() * std::f64::consts::TAU;
                    let d = 2.0 + self.rng.next_f64() * 8.0;
                    let (x, z) = (here.x + a.cos() * d, here.z + a.sin() * d);
                    let pos = world
                        .ground()
                        .footing(x, z, here.y)
                        .filter(|f| !f.water)
                        .map_or(here, |f| DVec3::new(x, f.y, z));
                    let id = self.next_id;
                    self.next_id += 1;
                    let mut agent = Agent::new(id, ki, kind, graph, g.id, female, stage, pos, tick);
                    agent.yaw = self.rng.next_f32() * std::f32::consts::TAU;
                    agent.mind.timer = self.rng.next_f32() * 5.0;
                    group.members.push(id);
                    self.agents.push(agent);
                }
                // Mothers and their young, closest of all.
                let mothers: Vec<u64> = self
                    .agents
                    .iter()
                    .filter(|a| a.group == g.id && a.female && a.stage == Stage::Adult)
                    .map(|a| a.id)
                    .collect();
                let young: Vec<u64> = self
                    .agents
                    .iter()
                    .filter(|a| a.group == g.id && a.stage != Stage::Adult)
                    .map(|a| a.id)
                    .collect();
                for (k, y) in young.iter().enumerate() {
                    if let Some(m) = mothers.get(k % mothers.len().max(1)) {
                        group.set_tie(*m, *y, 1.0);
                    }
                }
                g.live = true;
                self.groups.push(group);
            }
        }
    }

    /// Sets a group of a kind down at a place (a test's, a screenshot's): its grown females and
    /// males, half-grown and young, knowing the places about it. Its id.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_group(
        &mut self,
        kinds: &Kinds,
        graph: &Graph,
        items: &Items,
        world: &mut dyn AgentWorld,
        kind: usize,
        members: [u16; 4],
        at: DVec3,
        tick: u64,
    ) -> u64 {
        let id = 1_000_000_000 + self.next_id;
        self.next_id += 1;
        let k = &kinds.kinds[kind];
        let mut group = SocialGroup {
            id,
            kind,
            range_m: 2000.0,
            home: DVec2::new(at.x, at.z),
            culture: Culture {
                techniques: k.techniques.clone(),
                traditions: Vec::new(),
            },
            ..SocialGroup::default()
        };
        know_about(&mut group, world, items, at);
        let [females, males, juveniles, young] = members;
        let mut list: Vec<(Stage, bool)> = Vec::new();
        list.extend((0..females).map(|_| (Stage::Adult, true)));
        list.extend((0..males).map(|_| (Stage::Adult, false)));
        list.extend((0..juveniles).map(|n| (Stage::Juvenile, n % 2 == 0)));
        list.extend((0..young).map(|n| (Stage::Young, n % 2 == 1)));
        let mut mothers = Vec::new();
        for (n, (stage, female)) in list.into_iter().enumerate() {
            let a = n as f64 * 2.4;
            let pos = at
                + DVec3::new(
                    a.cos() * (2.0 + n as f64 * 0.6),
                    0.0,
                    a.sin() * (2.0 + n as f64 * 0.6),
                );
            let pos = world
                .ground()
                .footing(pos.x, pos.z, at.y)
                .map_or(at, |f| DVec3::new(pos.x, f.y, pos.z));
            let aid = self.next_id;
            self.next_id += 1;
            let agent = Agent::new(aid, kind, k, graph, id, female, stage, pos, tick);
            if stage == Stage::Adult && female {
                mothers.push(aid);
            } else if stage != Stage::Adult
                && let Some(m) = mothers.get(n % mothers.len().max(1))
            {
                group.set_tie(*m, aid, 1.0);
            }
            group.members.push(aid);
            self.agents.push(agent);
        }
        self.groups.push(group);
        id
    }

    /// Folds back into their numbers the groups whose agents are all beyond [`FAR_M`] of the
    /// player (or dead): the living by age and sex, where they are; the dead not at all.
    pub fn fold(&mut self, eco: &mut Ecology, player: DVec3) {
        let wrap = eco.cells_around as f64 * CELL_M;
        let mut folded = Vec::new();
        for (gi, group) in self.groups.iter().enumerate() {
            let far = self
                .agents
                .iter()
                .filter(|a| a.group == group.id && a.alive())
                .all(|a| hdist(a.pos, player, wrap) > FAR_M);
            if far {
                folded.push(gi);
            }
        }
        for &gi in folded.iter().rev() {
            let group = self.groups.remove(gi);
            let (mut young, mut juv, mut f, mut m) = (0u16, 0u16, 0u16, 0u16);
            let (mut cx, mut cz, mut n) = (0.0f64, 0.0f64, 0.0f64);
            for a in self
                .agents
                .iter()
                .filter(|a| a.group == group.id && a.alive())
            {
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
            self.agents.retain(|a| a.group != group.id);
            let Some(id) = group.population_group else {
                continue;
            };
            for r in eco.regions.values_mut() {
                if let Some(g) = r.groups.iter_mut().find(|g| g.id == id) {
                    g.young = young;
                    g.juveniles = juv;
                    g.females = f;
                    g.males = m;
                    g.tolerance = group.habituation;
                    if n > 0.0 {
                        g.pos = [(cx / n).rem_euclid(wrap.max(1.0)), cz / n];
                    }
                    g.live = false;
                }
            }
        }
    }

    /// The agents as the client draws them.
    pub fn views(&self, kinds: &Kinds) -> Vec<AgentView> {
        self.agents
            .iter()
            .filter(|a| a.alive())
            .map(|a| AgentView {
                id: a.id,
                kind: a.kind,
                female: a.female,
                stage: a.stage,
                pos: a.pos,
                yaw: a.yaw,
                speed: a.speed,
                medium: a.medium,
                doing: a.mind.doing.clone(),
                height_m: a.height_m(&kinds.kinds[a.kind]),
            })
            .collect()
    }

    /// Lives `dt` seconds of play: each agent senses what is about it, chooses again when its
    /// choice has run its time or danger interrupts, and does what it chose; the bodies step.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        kinds: &Kinds,
        crafts: &Crafts,
        content: &Content,
        items: &Items,
        world: &mut dyn AgentWorld,
        person: Option<Person>,
        now: Now,
        dt: f32,
    ) {
        self.done.clear();
        self.body_s -= dt;
        let body_step = self.body_s <= 0.0;
        if body_step {
            self.body_s += BODY_S;
        }
        // A group comes in time to tolerate a person who keeps calm within its sight, and soon
        // unlearns it when they run at it or hunt near it.
        if let Some(p) = person {
            for g in self.groups.iter_mut() {
                if !kinds.kinds[g.kind].does(Behavior::Habituate) {
                    continue;
                }
                let noticed = SEE_M * p.plain.clamp(0.1, 1.0) as f64;
                let seen = self.agents.iter().any(|a| {
                    a.group == g.id
                        && a.alive()
                        && a.mind.doing != Doing::Sleeping
                        && (a.pos - p.pos).length() < noticed
                });
                if !seen {
                    continue;
                }
                g.habituation = if p.running || p.hunting {
                    (g.habituation - dt / UNLEARN_S).max(0.0)
                } else {
                    (g.habituation + dt / HABITUATE_S).min(1.0)
                };
            }
        }
        let n = self.agents.len();
        for i in 0..n {
            if !self.agents[i].alive() {
                continue;
            }
            let group_i = self
                .groups
                .iter()
                .position(|g| g.id == self.agents[i].group);
            let Some(gi) = group_i else {
                continue;
            };
            let (threat, flight_m) = self.threat_of(i, gi, world, person);
            let a = &mut self.agents[i];
            // Fear rises at a threat and fades in safety.
            let scare = threat.map_or(0.0, |t| {
                ((flight_m * 1.5 - t.dist) / (flight_m * 1.5)).clamp(0.0, 1.0)
            });
            a.mind.fear = a.mind.fear.max(scare) * (1.0 - 0.05 * dt);
            let danger = threat.is_some_and(|t| t.dist < flight_m);
            let calm_while_fleeing = matches!(a.mind.doing, Doing::Fleeing { .. }) && !danger;
            a.mind.timer -= dt;
            let interrupt = danger
                && !matches!(
                    a.mind.doing,
                    Doing::Fleeing { .. } | Doing::Mobbing { .. } | Doing::Watching { .. }
                );
            let choosing = a.mind.timer <= 0.0
                || interrupt
                || calm_while_fleeing
                || a.mind.doing == Doing::Idle;
            if choosing {
                let mut situation =
                    self.situation(i, gi, kinds, crafts, content, items, world, now);
                situation.threat = threat;
                situation.flight_m = flight_m;
                let kind = &kinds.kinds[self.agents[i].kind];
                let a = &mut self.agents[i];
                let needs = Needs::of(&a.body, &kind.body, a.mind.fear);
                let roll = self.rng.next_f32();
                a.mind.doing = choose(kind, &needs, &situation, roll);
                a.mind.timer = hold_for(&a.mind.doing) * (0.75 + 0.5 * self.rng.next_f32());
            }
            let kind = &kinds.kinds[self.agents[i].kind];
            self.act(i, gi, kind, crafts, content, items, world, now, dt);
            if body_step {
                let a = &mut self.agents[i];
                let activity = kind.body.activity(activity_of(&a.mind.doing, a.speed));
                let exposure = world.exposure(a.pos, a.medium == Medium::Tree);
                a.body
                    .step(&kind.body, BODY_S as f64, &exposure, &kind.coat, &activity);
            }
        }
    }

    /// The nearest threat an agent sees (a hunter of its kind, else the person), and how near it
    /// may come before it runs: its kind's flight distance, less for a person its group has come
    /// to tolerate.
    fn threat_of(
        &self,
        i: usize,
        gi: usize,
        world: &mut dyn AgentWorld,
        person: Option<Person>,
    ) -> (Option<Threat>, f32) {
        let pos = self.agents[i].pos;
        let hunter: Option<Threat> = world
            .hunters_near(pos, SEE_M)
            .into_iter()
            .map(|h| Threat {
                at: h,
                dist: (h - pos).length() as f32,
                hunter: true,
            })
            .min_by(|x, y| x.dist.total_cmp(&y.dist));
        if hunter.is_some() {
            return (hunter, FLIGHT_M);
        }
        let Some(p) = person else {
            return (None, FLIGHT_M);
        };
        let d = (p.pos - pos).length() as f32;
        // Unnoticed: too far, or crouched and crawling in the grass.
        if d >= SEE_M as f32 * p.plain.clamp(0.1, 1.0) {
            return (None, FLIGHT_M);
        }
        // A person coming on fast, or hunting near them, is a hunter for the while; a calm one
        // may come nearer the more the group has come to tolerate them, and beyond what holds
        // its eye is let be.
        let hunting = p.running || p.hunting;
        let tolerated = FLIGHT_M * (1.0 - 0.85 * self.groups[gi].habituation);
        if !hunting && d > tolerated * HEED {
            return (None, FLIGHT_M);
        }
        (
            Some(Threat {
                at: p.pos,
                dist: d,
                hunter: hunting,
            }),
            if hunting { FLIGHT_M } else { tolerated },
        )
    }

    /// What an agent knows of the moment about it (its threat filled in by the caller).
    #[allow(clippy::too_many_arguments)]
    fn situation(
        &mut self,
        i: usize,
        gi: usize,
        kinds: &Kinds,
        crafts: &Crafts,
        content: &Content,
        items: &Items,
        world: &mut dyn AgentWorld,
        now: Now,
    ) -> Situation {
        let a = &self.agents[i];
        let kind = &kinds.kinds[a.kind];
        let group = &self.groups[gi];
        let pos = a.pos;
        // The group: its middle, its grown ones near, an alarm raised.
        let (mut cx, mut cz, mut cn) = (0.0, 0.0, 0.0);
        let mut grown_near = 0u16;
        let mut alarm_raised = false;
        let mut mother: Option<DVec3> = None;
        for b in self
            .agents
            .iter()
            .filter(|b| b.group == a.group && b.alive() && b.id != a.id)
        {
            cx += b.pos.x;
            cz += b.pos.z;
            cn += 1.0;
            if b.stage == Stage::Adult && (b.pos - pos).length() < 20.0 {
                grown_near += 1;
            }
            if matches!(b.mind.doing, Doing::Alarm { .. }) {
                alarm_raised = true;
            }
            if a.stage != Stage::Adult && group.tie(a.id, b.id) >= 1.0 {
                mother = Some(b.pos);
            }
        }
        let group_at = if cn > 0.0 {
            mother.or(Some(DVec3::new(cx / cn, pos.y, cz / cn)))
        } else {
            None
        };
        let from_group_m = group_at.map_or(0.0, |g| {
            let d = (g - pos).length() as f32;
            // The young keep to their mothers.
            if a.stage != Stage::Adult { d * 3.0 } else { d }
        });
        // Trees, water and food it knows of, or sees.
        let tree = Places::nearest(&group.places.sleep, pos)
            .filter(|t| (*t - pos).length() < LOOK_M * 2.0)
            .or_else(|| trunk_near(world.ground(), pos, 12.0).map(|(t, _)| t));
        let water = Places::nearest(&group.places.water, pos);
        let water_here = water.is_some_and(|w| (w - pos).length() < 2.0);
        let food_here = world.food_at(pos).is_some();
        let food = if food_here {
            None
        } else {
            world.food_near(pos, LOOK_M)
        };
        // What it could do with what lies about: crack nuts at an anvil, strike a flake.
        let offers = offers_for(a, kind, crafts, content, items, world, &group.places);
        Situation {
            pos,
            hour: now.hour,
            threat: None,
            flight_m: FLIGHT_M,
            in_tree: a.medium == Medium::Tree,
            in_nest: a.medium == Medium::Tree && a.mind.doing == Doing::Sleeping,
            tree,
            water,
            water_here,
            food,
            food_here,
            offers,
            alarm_raised,
            grown_near,
            group_at,
            from_group_m,
            grown: a.stage == Stage::Adult,
        }
    }

    /// Does what an agent chose, for `dt`.
    #[allow(clippy::too_many_arguments)]
    fn act(
        &mut self,
        i: usize,
        gi: usize,
        kind: &Kind,
        crafts: &Crafts,
        content: &Content,
        items: &Items,
        world: &mut dyn AgentWorld,
        now: Now,
        dt: f32,
    ) {
        let doing = self.agents[i].mind.doing.clone();
        let walk = kind.body.params.walk_m_s * self.agents[i].growth().powf(0.3);
        let run = kind.body.params.jog_m_s * 1.5;
        match doing {
            Doing::Going { to, then } => {
                if self.agents[i].medium == Medium::Tree {
                    self.climb_down(i, world, dt);
                    return;
                }
                if self.move_toward(i, world, to, walk, dt) {
                    let a = &mut self.agents[i];
                    a.speed = 0.0;
                    a.mind.doing = match then {
                        Intent::Feed => Doing::Feeding,
                        Intent::Drink => Doing::Drinking,
                        Intent::Work(r) => Doing::Working { recipe: r },
                        Intent::Nest => Doing::Nesting,
                        Intent::Rejoin | Intent::Roam => Doing::Idle,
                    };
                    a.mind.timer = hold_for(&a.mind.doing);
                    if then == Intent::Drink {
                        let w = a.pos;
                        Places::remember(&mut self.groups[gi].places.water, w, 10.0, 6);
                    }
                }
            }
            Doing::Feeding => {
                let a = &mut self.agents[i];
                a.speed = 0.0;
                if let Some(f) = world.food_at(a.pos) {
                    eat(a, kind, content, &f.material, f.kg_min * PER_MIN * dt);
                    // A handful of marula stones to crack later, now and then.
                    if let Some(nuts) = &f.nuts
                        && self.rng.next_f32() < 0.02 * dt
                        && a.stage == Stage::Adult
                        && let Some(id) = items_of(items, nuts)
                    {
                        let mass = a.mass_kg(kind);
                        let _ = a.carry.stow(items, Stack::of(&id, 4), mass);
                    }
                    let here = a.pos;
                    Places::remember(&mut self.groups[gi].places.food, here, 15.0, 12);
                }
                if Needs::of(&self.agents[i].body, &kind.body, 0.0).hunger <= 0.0 {
                    self.agents[i].mind.timer = 0.0;
                }
            }
            Doing::Drinking => {
                let a = &mut self.agents[i];
                a.speed = 0.0;
                a.body.drink(&kind.body, DRINK_L_S * dt as f64, 0.0, 0.0);
                if Needs::of(&a.body, &kind.body, 0.0).thirst <= 0.0 {
                    a.mind.timer = 0.0;
                }
            }
            Doing::Working { recipe } => {
                self.agents[i].speed = 0.0;
                self.work(i, gi, recipe, kind, crafts, content, items, world, now, dt);
            }
            Doing::Nesting => {
                let a = &mut self.agents[i];
                if a.medium != Medium::Tree || a.climbing() {
                    // Up the tree to its place in the crown first.
                    if !self.climb_up(i, world, dt) {
                        let a = &mut self.agents[i];
                        a.speed = 0.0;
                        a.mind.doing = Doing::Sleeping;
                    }
                    return;
                }
                a.speed = 0.0;
                if a.mind.timer <= hold_for(&Doing::Nesting) * 0.5 {
                    let at = a.pos;
                    if let Some(bed) = world.nest(at) {
                        a.pos = bed;
                        a.perch = Some(bed);
                    }
                    a.mind.doing = Doing::Sleeping;
                    a.mind.timer = hold_for(&Doing::Sleeping);
                    let tree = at;
                    Places::remember(&mut self.groups[gi].places.sleep, tree, 6.0, 12);
                }
            }
            Doing::Sleeping => {
                let a = &mut self.agents[i];
                a.speed = 0.0;
                // Morning: down from the tree.
                if (6.0..18.5).contains(&now.hour) {
                    a.mind.timer = 0.0;
                }
            }
            Doing::Resting | Doing::Grooming { .. } | Doing::Idle => {
                self.agents[i].speed = 0.0;
            }
            Doing::Watching { at } => {
                let a = &mut self.agents[i];
                a.speed = 0.0;
                a.yaw = yaw_toward(a.pos, at);
            }
            Doing::Alarm { at } => {
                let a = &mut self.agents[i];
                a.speed = 0.0;
                a.yaw = yaw_toward(a.pos, at);
                if a.mind.timer > hold_for(&Doing::Alarm { at }) * 0.6 {
                    world.call(a.pos, true);
                    a.mind.timer = hold_for(&Doing::Alarm { at }) * 0.6;
                }
            }
            Doing::Mobbing { at } => {
                // Up to a stone's throw, shouting and brandishing.
                let a = &self.agents[i];
                let d = (at - a.pos).length();
                if d > 9.0 {
                    let to = at + (a.pos - at).normalize_or(DVec3::X) * 8.0;
                    self.move_toward(i, world, to, walk, dt);
                } else {
                    let a = &mut self.agents[i];
                    a.speed = 0.0;
                    a.yaw = yaw_toward(a.pos, at);
                    if self.rng.next_f32() < 0.5 * dt {
                        world.call(a.pos, true);
                    }
                }
            }
            Doing::Fleeing { to } => {
                if self.agents[i].medium == Medium::Tree {
                    if self.agents[i].climbing() {
                        self.climb_up(i, world, dt);
                    } else {
                        self.agents[i].speed = 0.0;
                    }
                    return;
                }
                if self.move_toward(i, world, to, run, dt)
                    && trunk_near(world.ground(), self.agents[i].pos, 2.0).is_some()
                {
                    self.climb_up(i, world, dt);
                }
            }
        }
    }

    /// Works a process at hand: the engine's plan with what it carries and what lies within
    /// reach, done to the thing it needs (an anvil) when there is one; finished when its time is
    /// up, what it made eaten, kept or laid down.
    #[allow(clippy::too_many_arguments)]
    fn work(
        &mut self,
        i: usize,
        gi: usize,
        recipe: usize,
        kind: &Kind,
        crafts: &Crafts,
        content: &Content,
        items: &Items,
        world: &mut dyn AgentWorld,
        now: Now,
        dt: f32,
    ) {
        let pos = self.agents[i].pos;
        // The hammerstone left by the anvil, into its hand.
        let lying = world.things().near(pos, REACH_M);
        let took = take_up_tools(&mut self.agents[i], kind, crafts, recipe, &lying, items);
        for id in &took {
            world.things().take(*id, None);
        }
        let lying = world.things().near(pos, REACH_M);
        let aimed = aim_for(crafts, content, recipe, &lying, items);
        let around = world.surroundings(pos);
        let plan = plan_work(
            crafts,
            content,
            items,
            &self.agents[i],
            recipe,
            &lying,
            aimed.clone(),
            around.clone(),
        );
        let Ok(plan) = plan else {
            // It cannot here (the nuts eaten, the stone gone): it puts down what it took up and
            // looks about again.
            lay_down_tools(&mut self.agents[i], items, world, pos);
            self.agents[i].mind.doing = Doing::Idle;
            return;
        };
        let a = &mut self.agents[i];
        // The work's length: its real hours on the day's scale, as the player's.
        let total = (plan.hours as f64 * 3600.0 * kind.body.scales.factor(plan.scale)) as f32;
        let total = total.clamp(4.0, 120.0);
        if a.mind.timer > total {
            a.mind.timer = total;
        }
        if a.mind.timer > dt {
            return;
        }
        let tick = now.tick;
        let mut rng = Rng::new(self.rng.next_u64());
        let outcome = finish_work(
            crafts,
            content,
            items,
            kind,
            &mut self.agents[i],
            &plan,
            &lying,
            aimed,
            around,
            world.things(),
            &mut rng,
            tick,
        );
        if outcome.done {
            Places::remember(&mut self.groups[gi].places.anvils, pos, 8.0, 8);
        }
        self.done.push(Done {
            agent: self.agents[i].id,
            at: pos,
            recipe,
            triggers: outcome.triggers,
        });
        // The hammerstone back by the anvil for the next time.
        lay_down_tools(&mut self.agents[i], items, world, pos);
        let a = &mut self.agents[i];
        a.mind.doing = Doing::Idle;
        a.mind.timer = 0.0;
    }

    /// A step toward a place on the ground; true when there.
    fn move_toward(
        &mut self,
        i: usize,
        world: &mut dyn AgentWorld,
        to: DVec3,
        speed: f32,
        dt: f32,
    ) -> bool {
        let a = &mut self.agents[i];
        let d = DVec2::new(to.x - a.pos.x, to.z - a.pos.z);
        if d.length() < 1.0 {
            a.speed = 0.0;
            return true;
        }
        a.yaw = (d.x as f32).atan2(d.y as f32);
        a.speed = speed;
        let step = d.normalize() * (speed * dt) as f64;
        let (nx, nz) = (a.pos.x + step.x, a.pos.z + step.y);
        match world.ground().footing(nx, nz, a.pos.y) {
            Some(f) if (f.y - a.pos.y).abs() < 1.2 && !(f.water && f.depth > 0.8) => {
                a.pos = DVec3::new(nx, f.y, nz);
                a.medium = Medium::Ground;
                false
            }
            _ => {
                // Blocked: around it, a little to the side.
                let side = DVec2::new(-step.y, step.x);
                let (sx, sz) = (a.pos.x + side.x, a.pos.z + side.y);
                if let Some(f) = world.ground().footing(sx, sz, a.pos.y)
                    && (f.y - a.pos.y).abs() < 1.2
                    && !f.water
                {
                    a.pos = DVec3::new(sx, f.y, sz);
                }
                false
            }
        }
    }

    /// A step up the tree it is by toward its place in the crown (found as it starts up: each
    /// to its own place about the trunk, up where the trunk runs into the crown — the crown is
    /// shared); whether there is a tree to climb.
    fn climb_up(&mut self, i: usize, world: &mut dyn AgentWorld, dt: f32) -> bool {
        let a = &mut self.agents[i];
        if a.medium != Medium::Tree || a.perch.is_none() {
            let Some((foot, height)) = trunk_near(world.ground(), a.pos, 2.5) else {
                return false;
            };
            let perch = crown_perch(world.ground(), foot, height, a.id as f64);
            a.pos.x = perch.x;
            a.pos.z = perch.z;
            a.pos.y = a.pos.y.max(foot.y);
            a.perch = Some(perch);
            a.medium = Medium::Tree;
        }
        let top = a.perch.map_or(a.pos.y, |p| p.y);
        if a.pos.y < top {
            a.speed = 0.4;
            a.pos.y = (a.pos.y + 0.6 * dt as f64).min(top);
        } else {
            a.speed = 0.0;
        }
        true
    }

    /// Down the tree to its foot.
    fn climb_down(&mut self, i: usize, world: &mut dyn AgentWorld, dt: f32) {
        let a = &mut self.agents[i];
        let ground = world
            .ground()
            .footing(a.pos.x, a.pos.z, a.pos.y - 1.0)
            .or_else(|| world.ground().top(a.pos.x, a.pos.z));
        let floor = ground.map_or(a.pos.y - 1.0, |f| f.y);
        a.speed = 0.4;
        a.pos.y -= 0.8 * dt as f64;
        if a.pos.y <= floor {
            a.pos.y = floor;
            a.medium = Medium::Ground;
            a.perch = None;
            a.speed = 0.0;
        }
    }
}

/// Where one climbs to in a tree: onto one of the limbs where its crown begins (each its own,
/// by `k`), or, a tree with none in reach, beside the trunk up near its top.
fn crown_perch(ground: &dyn Ground, foot: DVec3, height: f64, k: f64) -> DVec3 {
    let top = trunk_top(ground, foot, height);
    let (cx, cz) = (foot.x.floor() as i32, foot.z.floor() as i32);
    let mut limbs: Vec<DVec3> = Vec::new();
    for y in foot.y.floor() as i32 + 2..=top.ceil() as i32 + 3 {
        for dz in -3..=3 {
            for dx in -3..=3 {
                let (x, z) = (cx + dx, cz + dz);
                let room = matches!(
                    ground.cell(x, y + 1, z),
                    Some(Cell::Open | Cell::Leaves | Cell::Plant)
                );
                if ground.cell(x, y, z) == Some(Cell::Limb) && room {
                    limbs.push(DVec3::new(x as f64 + 0.5, y as f64 + 0.6, z as f64 + 0.5));
                }
            }
        }
        // The crown's lowest limbs.
        if limbs.len() >= 6 {
            break;
        }
    }
    if !limbs.is_empty() {
        let pick = ((k * 0.618).fract() * limbs.len() as f64) as usize;
        return limbs[pick.min(limbs.len() - 1)];
    }
    let turn = k * 2.399_963;
    let out = 0.8 + 0.9 * (k * 0.371).fract();
    let high = (top - foot.y - 1.0).max(2.0);
    DVec3::new(
        foot.x + turn.cos() * out,
        foot.y + high - (k * 0.618).fract() * (high * 0.4).min(3.0),
        foot.z + turn.sin() * out,
    )
}

/// How high (y) a tree's trunk runs where it is climbed: the tallest of its columns within two
/// blocks of the foot found (a broad trunk's middle runs up into the crown, its flared foot and
/// buttresses not).
fn trunk_top(ground: &dyn Ground, foot: DVec3, height: f64) -> f64 {
    let (cx, cz, y0) = (
        foot.x.floor() as i32,
        foot.z.floor() as i32,
        foot.y.floor() as i32,
    );
    let mut best = foot.y + height;
    for dz in -2..=2 {
        for dx in -2..=2 {
            let (x, z) = (cx + dx, cz + dz);
            let Some(base) = (y0 - 1..=y0 + 2).find(|y| ground.cell(x, *y, z) == Some(Cell::Trunk))
            else {
                continue;
            };
            let mut top = base;
            while top < base + 40 && ground.cell(x, top + 1, z) == Some(Cell::Trunk) {
                top += 1;
            }
            best = best.max((top + 1) as f64);
        }
    }
    best
}

/// How far about its place a group knows the anvils lying (m).
const ANVILS_M: f64 = 80.0;

/// Learns the places about a group as it is drawn out: the water, the trees to sleep in, the
/// anvils lying about.
fn know_about(group: &mut SocialGroup, world: &mut dyn AgentWorld, items: &Items, here: DVec3) {
    if let Some((bank, _)) = water_near(world.ground(), here, 80.0) {
        group.places.water.push(bank);
    }
    let anvils: Vec<DVec3> = world
        .things()
        .near(here, ANVILS_M)
        .into_iter()
        .filter(|(_, s)| {
            items
                .get(&s.id)
                .and_then(|k| k.property("anvil"))
                .is_some_and(|v| v > 0.0)
        })
        .filter_map(|(id, _)| world.things().place(id))
        .collect();
    for at in anvils {
        Places::remember(&mut group.places.anvils, at, 4.0, 8);
    }
    for (dx, dz) in [
        (0.0, 0.0),
        (15.0, 0.0),
        (-15.0, 0.0),
        (0.0, 15.0),
        (0.0, -15.0),
    ] {
        let at = here + DVec3::new(dx, 0.0, dz);
        if let Some((t, _)) = trunk_near(world.ground(), at, 10.0) {
            Places::remember(&mut group.places.sleep, t, 3.0, 12);
        }
    }
}

/// How long it keeps at a choice before looking about again (seconds of play).
fn hold_for(d: &Doing) -> f32 {
    match d {
        Doing::Idle => 1.0,
        Doing::Going { .. } => 60.0,
        Doing::Feeding => 40.0,
        Doing::Drinking => 15.0,
        Doing::Working { .. } => 120.0,
        Doing::Resting => 20.0,
        Doing::Grooming { .. } => 15.0,
        Doing::Nesting => 20.0,
        Doing::Sleeping => 60.0,
        Doing::Watching { .. } => 6.0,
        Doing::Alarm { .. } => 4.0,
        Doing::Mobbing { .. } => 12.0,
        Doing::Fleeing { .. } => 15.0,
    }
}

/// The body's activity for what it is doing (the content's table of METs).
fn activity_of(d: &Doing, speed: f32) -> &'static str {
    match d {
        Doing::Sleeping => "sleeping",
        Doing::Resting | Doing::Grooming { .. } | Doing::Feeding | Doing::Drinking => "resting",
        Doing::Watching { .. } | Doing::Alarm { .. } | Doing::Idle => "standing",
        Doing::Working { .. } | Doing::Nesting => "carrying_heavy",
        Doing::Going { .. } | Doing::Mobbing { .. } | Doing::Fleeing { .. } => {
            if speed > 2.0 {
                "jogging"
            } else if speed > 0.1 {
                "walking"
            } else {
                "standing"
            }
        }
    }
}

/// The yaw facing from one place to another.
fn yaw_toward(from: DVec3, to: DVec3) -> f32 {
    ((to.x - from.x) as f32).atan2((to.z - from.z) as f32)
}

/// Eats `kg` of a material.
fn eat(a: &mut Agent, kind: &Kind, content: &Content, material: &str, kg: f32) {
    let Some(m) = content.materials.get(material) else {
        return;
    };
    let Some(b) = hearth_craft::food::bite(m, kg, 0.0) else {
        return;
    };
    let f = Food {
        kcal: b.kcal as f64,
        protein_g: b.protein_g as f64,
        fat_g: b.fat_g as f64,
        carb_g: b.carb_g as f64,
        water_l: b.water_l as f64,
        volume_l: b.volume_l as f64,
        fresh_days: b.fresh_days as f64,
    };
    let _ = a.body.eat(&kind.body, &f);
}

/// The item of a material's bulk form.
fn items_of(items: &Items, material: &str) -> Option<String> {
    let key = material.rsplit(':').next().unwrap_or(material);
    items
        .iter()
        .find(|k| k.material.as_deref().is_some_and(|m| m.ends_with(key)) && k.has_tag("bulk"))
        .map(|k| k.id.clone())
}

/// Takes up in its hands, from what lies within reach, the tools a process needs that it does
/// not hold (the hammerstone by the anvil): the ids of what it took.
fn take_up_tools(
    a: &mut Agent,
    kind: &Kind,
    crafts: &Crafts,
    recipe: usize,
    lying: &[(u64, Stack)],
    items: &Items,
) -> Vec<u64> {
    let def = &crafts.recipes[recipe].def;
    // As strong as the engine reckons it (its quality and wear told).
    let strong =
        |s: &Stack, prop: &str, min: f32| s.property(items, prop).is_some_and(|v| v >= min);
    let mut took = Vec::new();
    for t in &def.tools {
        let held = [a.carry.right.as_ref(), a.carry.left.as_ref()]
            .into_iter()
            .flatten()
            .any(|s| strong(s, &t.property, t.min));
        if held {
            continue;
        }
        let found = lying
            .iter()
            .find(|(id, s)| !took.contains(id) && strong(s, &t.property, t.min));
        if let Some((id, s)) = found {
            let hand = if a.carry.right.is_none() {
                hearth_items::Hand::Right
            } else {
                hearth_items::Hand::Left
            };
            let mass = a.mass_kg(kind);
            if a.carry.hold(items, s.clone(), hand, mass).is_ok() {
                took.push(*id);
            }
        }
    }
    took
}

/// Lays down beside it what it holds in its hands (its tools, after the work).
fn lay_down_tools(a: &mut Agent, items: &Items, world: &mut dyn AgentWorld, at: DVec3) {
    for hand in [hearth_items::Hand::Right, hearth_items::Hand::Left] {
        if let Some(s) = a.carry.release(hand) {
            world.things().lay(at + DVec3::new(0.3, 0.0, 0.2), s);
        }
    }
    let _ = items;
}

/// The thing a process is done to, among what lies at hand (an anvil stone for cracking), if it
/// needs one.
fn aim_for(
    crafts: &Crafts,
    content: &Content,
    recipe: usize,
    lying: &[(u64, Stack)],
    items: &Items,
) -> Option<Aimed> {
    use hearth_content::schema::process::Target;
    match &crafts.recipes[recipe].def.target {
        Some(Target::Thing(m)) => lying
            .iter()
            .find(|(_, s)| {
                items
                    .get(&s.id)
                    .is_some_and(|k| hearth_craft::engine::matches(m, k, content))
            })
            .map(|(id, _)| Aimed::Thing(*id)),
        _ => None,
    }
}

/// The processes an agent could do here with what it carries and what lies within reach: its
/// culture's techniques that the engine would plan now.
fn offers_for(
    a: &Agent,
    kind: &Kind,
    crafts: &Crafts,
    content: &Content,
    items: &Items,
    world: &mut dyn AgentWorld,
    places: &Places,
) -> Vec<Offer> {
    if a.stage != Stage::Adult {
        return Vec::new();
    }
    let mut out = Vec::new();
    // Here, and at the group's anvils it carries nuts to.
    let mut spots = vec![a.pos];
    if let Some(anvil) = Places::nearest(&places.anvils, a.pos) {
        spots.push(anvil);
    }
    for spot in spots {
        let lying = world.things().near(spot, REACH_M);
        let around = world.surroundings(spot);
        for t in &kind.techniques {
            let Some(r) = crafts.index_of(t).or_else(|| {
                crafts.index_of(&format!("hearth:{}", t.rsplit(':').next().unwrap_or(t)))
            }) else {
                continue;
            };
            // They strike new flakes rather than mend the old (retouch is rare before the
            // later Oldowan).
            if crafts.recipes[r].def.effect == hearth_content::schema::process::Effect::Mend {
                continue;
            }
            // At the anvil the agent stands by the things there, the tools lying by it in its
            // hands.
            let mut at = a.clone();
            at.pos = spot;
            let took = take_up_tools(&mut at, kind, crafts, r, &lying, items);
            let rest: Vec<(u64, Stack)> = lying
                .iter()
                .filter(|(id, _)| !took.contains(id))
                .cloned()
                .collect();
            let aimed = aim_for(crafts, content, r, &rest, items);
            if plan_work(crafts, content, items, &at, r, &rest, aimed, around.clone()).is_ok() {
                let feeds = crafts.recipes[r]
                    .def
                    .outputs
                    .iter()
                    .any(|o| output_is_food(content, &o.item));
                if out.iter().any(|o: &Offer| o.recipe == r) {
                    continue;
                }
                out.push(Offer {
                    recipe: r,
                    at: spot,
                    feeds,
                });
            }
        }
    }
    out
}

/// Whether what a process makes is food.
fn output_is_food(content: &Content, m: &hearth_content::schema::process::Match) -> bool {
    use hearth_content::schema::process::Match;
    match m {
        Match::Material(id) => content
            .materials
            .get(id.as_str())
            .is_some_and(|mat| mat.tags.iter().any(|t| t == "food")),
        _ => false,
    }
}

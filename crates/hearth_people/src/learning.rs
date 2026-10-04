//! Knowledge and social learning (V2.1 §11; H6). Each person knows what it has learned, and a band
//! knows what its living members know (the collective brain, §11.4). Each year of its life course
//! the young learn from those who know — their band's, and its neighbours' as far as they meet —
//! each technique whose groundwork they have, with a chance that grows with the number of knowers
//! and falls steeply the deeper the technique rests (a complex skill copied less faithfully, after
//! Henrich 2004); the grown learn too, more slowly; and now and then a grown one tries something
//! new, the curious the more, and in time finds it out. A technique that no living member knows
//! any longer is lost to the band — its grown keep it as a story, a legend to be learned again by
//! doing — and the processes it opened go with it. So a small band cut off loses the skills too
//! few hold, and a large one in touch with its neighbours keeps and gathers them. Persons work only
//! what they know (the anachronism guard, §11.1).
//!
//! In play (§11.2): one at its work shows how to one of its band watching it — or to a guest it
//! trusts, the player among them — as fast as its people teach, the faster for its apprentice (a
//! young one takes a master each year: the most skilled of its band in what it does not yet know,
//! of its own household and sex the more readily); and in the evening's company the grown tell
//! stories — a place they know, or a technique of their people's, kept by those who hear it as a
//! legend to learn again by doing.

use std::collections::BTreeMap;

use hearth_content::Content;
use hearth_content::schema::knowledge::Route;
use hearth_craft::Graph;
use hearth_craft::knowledge::Learned;
use hearth_math::hash::{Rng, hash2};

use glam::DVec3;
use hearth_craft::Crafts;

use crate::memory::PlaceKind;
use crate::mind::Doing;
use crate::person::{Event, Person, Tier};
use crate::psyche::Tendency;
use crate::sim::People;
use crate::species::{Species, SpeciesSet};
use crate::speech::{Act, Gesture, words};
use crate::world::Now;

/// The insight a minute of being shown a work gives toward its knowledge, at a teaching factor of
/// one (a people's `teach` multiplies it).
pub const TAUGHT_PER_MIN: f32 = 0.05;
/// How near one at its work must be to show the player how, in front of the player's eyes (m).
pub const SHOW_M: f64 = 3.0;
/// How near those hearing a story sit (m).
const STORY_M: f64 = 8.0;

/// What the life course needs of a node of the technology graph.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeFacts {
    pub id: String,
    /// The nodes it requires.
    pub requires: Vec<String>,
    /// How deep it rests: 0 on nothing, else one more than the deepest it requires.
    pub depth: u8,
    pub skill: Option<String>,
    /// The processes it opens.
    pub enables: Vec<String>,
    pub implemented: bool,
    /// The insight one try gives toward it (0: it is not found by trying).
    pub tried: f32,
}

/// The technology graph as the life course reads it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lore {
    pub nodes: Vec<NodeFacts>,
    /// Each node's place in `nodes` by its id (looked up for every learner every year).
    index: BTreeMap<String, usize>,
}

impl Lore {
    pub fn from_graph(graph: &Graph) -> Self {
        let n = graph.nodes.len();
        let mut depth: Vec<Option<u8>> = vec![None; n];
        // Depths in order of need (the graph is acyclic; a bound keeps a bad one from looping).
        for _ in 0..64 {
            let mut changed = false;
            for i in 0..n {
                if depth[i].is_some() {
                    continue;
                }
                let req = &graph.nodes[i].requires;
                if req.iter().all(|&r| depth[r].is_some()) {
                    let d = req
                        .iter()
                        .filter_map(|&r| depth[r])
                        .max()
                        .map_or(0, |d| d + 1);
                    depth[i] = Some(d);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let nodes = graph
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| NodeFacts {
                id: node.id.clone(),
                requires: node
                    .requires
                    .iter()
                    .map(|&r| graph.nodes[r].id.clone())
                    .collect(),
                depth: depth[i].unwrap_or(8),
                skill: node.skill.clone(),
                enables: node.enables.clone(),
                implemented: node.implemented,
                tried: node
                    .routes
                    .iter()
                    .filter(|r| r.route == Route::Experiment)
                    .map(|r| r.insight)
                    .fold(0.0, f32::max),
            })
            .collect::<Vec<NodeFacts>>();
        let index = nodes
            .iter()
            .enumerate()
            .map(|(i, f)| (f.id.clone(), i))
            .collect();
        Self { nodes, index }
    }

    pub fn get(&self, id: &str) -> Option<&NodeFacts> {
        self.index.get(id).map(|&i| &self.nodes[i])
    }

    /// The processes a set of nodes opens, each once, in order.
    pub fn opened(&self, known: &[String]) -> Vec<String> {
        let mut t: Vec<String> = known
            .iter()
            .filter_map(|k| self.get(k))
            .flat_map(|f| f.enables.iter().cloned())
            .collect();
        t.sort();
        t.dedup();
        t
    }
}

/// How a people's knowledge passes between them (`humans/learning/transmission.ron`), resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct Transmission {
    pub species: String,
    pub learn_year: f32,
    pub depth_factor: f32,
    pub learn_ages: (f32, f32),
    pub grown_factor: f32,
    pub contact_weight: f32,
    pub innovate_month: f32,
    pub innovate_insight: f32,
    pub teach: f32,
    pub story_second: f32,
}

fn key(id: &str) -> &str {
    id.rsplit(':').next().unwrap_or(id)
}

impl Transmission {
    /// Every people's in the content.
    pub fn from_content(c: &Content) -> Vec<Self> {
        c.transmission
            .iter()
            .map(|t| Transmission {
                species: t.species.to_string(),
                learn_year: t.learn_year,
                depth_factor: t.depth_factor,
                learn_ages: t.learn_ages,
                grown_factor: t.grown_factor,
                contact_weight: t.contact_weight,
                innovate_month: t.innovate_month,
                innovate_insight: t.innovate_insight,
                teach: t.teach.max(1.0),
                story_second: t.story_second,
            })
            .collect()
    }

    /// A species' among a content's.
    pub fn of<'a>(list: &'a [Self], species: &str) -> Option<&'a Self> {
        list.iter().find(|t| key(&t.species) == key(species))
    }

    /// A learner's chance a year to take a technique of this depth from one knower, at an age.
    pub fn per_knower(&self, depth: u8, age: f64) -> f32 {
        let young = age <= self.learn_ages.1 as f64;
        let factor = if young { 1.0 } else { self.grown_factor };
        (self.learn_year * (-self.depth_factor * depth as f32).exp() * factor).clamp(0.0, 1.0)
    }
}

/// How far apart two bands' homes may be and the knowers of one still count for the other (m).
const CONTACT_M: f64 = 20_000.0;
/// The salt of the streams the bands' learning is drawn on.
const STREAM: u64 = 0x6b0e_1ed6_0000_0000;
/// The age from which one is counted among those who know, to be learned from.
const KNOWER_AGE: f64 = 10.0;
/// The age from which one's lost knowledge is kept as a story.
const REMEMBERS_AGE: f64 = 15.0;
/// The practice a technique learned from others begins its skill with (hours).
const LEARNED_PRACTICE_H: f32 = 5.0;

/// One comes to know a technique, as learned from others or found out: its skill begun.
pub(crate) fn learn(p: &mut Person, lore: &Lore, node: &str, route: Route, tick: u64) {
    p.knowledge.known.insert(
        node.to_owned(),
        Learned {
            tick,
            route: Some(route),
        },
    );
    p.knowledge.insight.remove(node);
    p.knowledge.legends.remove(node);
    if let Some(skill) = lore.get(node).and_then(|f| f.skill.clone())
        && p.knowledge.skill(&skill) == 0.0
    {
        p.knowledge.practice(&skill, LEARNED_PRACTICE_H, tick);
    }
}

/// Each band's knowers of each technique as last counted — its id, the year counted and the
/// counts — so that a band's neighbours count it once a year, not each of them again.
#[derive(Clone, Default)]
pub(crate) struct KnowerCounts(BTreeMap<u64, (u64, std::sync::Arc<BTreeMap<String, f32>>)>);

impl PartialEq for KnowerCounts {
    /// A cache: two people are alike whatever it holds.
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for KnowerCounts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "KnowerCounts({} bands)", self.0.len())
    }
}

impl People {
    /// How many of a band's living, ten and over, know each technique, in a year.
    fn knowers_of(
        &mut self,
        bi: usize,
        now: &Now,
        year: u64,
    ) -> std::sync::Arc<BTreeMap<String, f32>> {
        let id = self.bands[bi].id;
        if let Some((y, counts)) = self.knower_counts.0.get(&id)
            && *y == year
        {
            return counts.clone();
        }
        let mut counts: BTreeMap<String, f32> = BTreeMap::new();
        for i in self.band_members(bi) {
            let p = &self.persons[i];
            if !p.alive() || p.age(now) < KNOWER_AGE {
                continue;
            }
            for k in p.knowledge.known.keys() {
                match counts.get_mut(k) {
                    Some(n) => *n += 1.0,
                    None => {
                        counts.insert(k.clone(), 1.0);
                    }
                }
            }
        }
        let counts = std::sync::Arc::new(counts);
        self.knower_counts.0.insert(id, (year, counts.clone()));
        counts
    }

    /// A year of a band's knowledge, as its life course reaches it (see the module).
    pub(crate) fn knowledge_year(
        &mut self,
        bi: usize,
        sp: &Species,
        lore: &Lore,
        now: &Now,
        year: u64,
    ) {
        let Some(t) = &sp.learning else {
            return;
        };
        let members = self.band_members(bi);
        if members.is_empty() {
            return;
        }
        let (id, home) = (self.bands[bi].id, self.bands[bi].home);
        let salt = year.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let mut rng = Rng::new(hash2(self.seed ^ STREAM ^ salt, id));
        // Those it could learn from: its own knowers, and its neighbours' as far as they meet
        // (each band's counted once a year).
        let mut knowers: BTreeMap<String, f32> = (*self.knowers_of(bi, now, year)).clone();
        let kind = self.bands[bi].species.clone();
        for b in 0..self.bands.len() {
            if b == bi || self.bands[b].species != kind {
                continue;
            }
            let d = (self.bands[b].home - home).length();
            // Those met at the people's gathering this year, as near neighbours (H8).
            let met = if self.gathered_together(bi, b, now.day, now.year_days.max(1.0)) {
                crate::rounds::GATHERED_WEIGHT
            } else {
                0.0
            };
            if d >= CONTACT_M && met == 0.0 {
                continue;
            }
            let w = (t.contact_weight * (1.0 - d / CONTACT_M).max(0.0) as f32).max(met);
            let theirs = self.knowers_of(b, now, year);
            for (k, n) in theirs.iter() {
                match knowers.get_mut(k) {
                    Some(m) => *m += w * n,
                    None => {
                        knowers.insert(k.clone(), w * n);
                    }
                }
            }
        }
        // The learners: each, for each technique it lacks whose groundwork it has.
        for &i in &members {
            let p = &self.persons[i];
            if !p.alive() || p.player.is_some() {
                continue;
            }
            let age = p.age(now);
            if age < t.learn_ages.0 as f64 {
                continue;
            }
            let mut learned: Vec<String> = Vec::new();
            for (node, &k) in &knowers {
                if p.knowledge.knows(node) {
                    continue;
                }
                let Some(f) = lore.get(node) else {
                    continue;
                };
                if !f.implemented || !f.requires.iter().all(|r| p.knowledge.knows(r)) {
                    continue;
                }
                let one = t.per_knower(f.depth, age);
                let chance = 1.0 - (1.0 - one).powf(k);
                if rng.next_f32() < chance {
                    learned.push(node.clone());
                }
            }
            let p = &mut self.persons[i];
            for node in learned {
                learn(p, lore, &node, Route::Observation, now.tick);
            }
        }
        // Trying things: now and then a grown one tries something new, the curious the more,
        // and in time finds it out.
        for &i in &members {
            let p = &self.persons[i];
            if !p.alive() || p.player.is_some() || p.age(now) < t.learn_ages.1 as f64 {
                continue;
            }
            let chance = 12.0 * t.innovate_month * 2.0 * p.psyche.tendency(Tendency::Curiosity);
            if rng.next_f32() >= chance {
                continue;
            }
            let open: Vec<&NodeFacts> = lore
                .nodes
                .iter()
                .filter(|f| {
                    f.implemented
                        && f.tried > 0.0
                        && !p.knowledge.knows(&f.id)
                        && f.requires.iter().all(|r| p.knowledge.knows(r))
                })
                .collect();
            if open.is_empty() {
                continue;
            }
            let k = ((rng.next_f32() * open.len() as f32) as usize).min(open.len() - 1);
            let node = open[k].id.clone();
            let p = &mut self.persons[i];
            let v = p.knowledge.insight.entry(node.clone()).or_insert(0.0);
            *v += t.innovate_insight;
            if *v >= 1.0 {
                learn(p, lore, &node, Route::Experiment, now.tick);
                p.record(now.day, Event::Discovered { node });
            }
        }
        // What the band knows now: what its living members know. What none of them knows any
        // longer is lost; its grown keep it as a story.
        let mut known: Vec<String> = Vec::new();
        for &i in &members {
            let p = &self.persons[i];
            if p.alive() {
                for k in p.knowledge.known.keys() {
                    if !known.contains(k) {
                        known.push(k.clone());
                    }
                }
            }
        }
        known.sort();
        let lost: Vec<String> = self.bands[bi]
            .culture
            .knowledge
            .iter()
            .filter(|k| !known.contains(k))
            .cloned()
            .collect();
        if !lost.is_empty() {
            for &i in &members {
                let p = &mut self.persons[i];
                if p.alive() && p.age(now) >= REMEMBERS_AGE {
                    for k in &lost {
                        p.knowledge.legends.insert(k.clone());
                    }
                    p.record(
                        now.day,
                        Event::KnowledgeLost {
                            nodes: lost.clone(),
                        },
                    );
                }
            }
        }
        // The processes it opens, worked out again only when what it knows has changed.
        if self.bands[bi].culture.knowledge != known {
            let techniques = lore.opened(&known);
            let c = &mut self.bands[bi].culture;
            c.knowledge = known;
            c.techniques = techniques;
        }
    }
}

impl People {
    /// Each year a young one of a band takes a master (V2.1 §11.2): the grown one of its band
    /// most skilled in what it does not yet know, of its own household and sex the more readily.
    pub(crate) fn apprentice(&mut self, bi: usize, lore: &Lore, now: &Now) {
        let members = self.band_members(bi);
        let grown: Vec<usize> = members
            .iter()
            .copied()
            .filter(|&i| self.persons[i].alive() && self.persons[i].age(now) >= 20.0)
            .collect();
        for &i in &members {
            let p = &self.persons[i];
            let age = p.age(now);
            if !p.alive() || p.player.is_some() || !(8.0..16.0).contains(&age) {
                continue;
            }
            let lacks: Vec<&str> = lore
                .nodes
                .iter()
                .filter(|f| f.implemented && !p.knowledge.knows(&f.id))
                .filter_map(|f| f.skill.as_deref())
                .collect();
            let master = grown
                .iter()
                .map(|&g| {
                    let m = &self.persons[g];
                    let skill: f32 = m
                        .knowledge
                        .skills
                        .keys()
                        .filter(|s| lacks.contains(&s.as_str()))
                        .map(|s| m.knowledge.skill(s))
                        .sum();
                    let home =
                        p.social.household.is_some() && m.social.household == p.social.household;
                    let kin = if home { 0.5 } else { 0.0 };
                    let same = if m.life.female == p.life.female {
                        0.3
                    } else {
                        0.0
                    };
                    (m.id, skill + kin + same)
                })
                .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
                .map(|(id, _)| id);
            self.persons[i].mind.master = master;
        }
    }

    /// In the evening's company, now and then one of the grown tells a story to those sitting
    /// near (V2.1 §11.2): a place it is sure of — water, food, danger — which they come to know,
    /// less surely and less exactly; or a technique of its people's one of them does not know,
    /// kept by them as a legend.
    pub(crate) fn stories(&mut self, species: &SpeciesSet, now: &Now) {
        let mut tellers: Vec<(usize, Vec<usize>, f32)> = Vec::new();
        for (i, p) in self.persons.iter().enumerate() {
            if p.tier != Tier::Full
                || !p.alive()
                || p.player.is_some()
                || p.age(now) < 15.0
                || matches!(p.mind.doing, Doing::Sleeping)
            {
                continue;
            }
            let Some(chance) = species
                .get(&p.species)
                .and_then(|sp| sp.learning.as_ref())
                .map(|t| t.story_second)
                .filter(|c| *c > 0.0)
            else {
                continue;
            };
            let listeners: Vec<usize> = (0..self.persons.len())
                .filter(|&j| {
                    let q = &self.persons[j];
                    j != i
                        && q.tier == Tier::Full
                        && q.alive()
                        && q.social.band == p.social.band
                        && !matches!(q.mind.doing, Doing::Sleeping)
                        && (q.place.pos - p.place.pos).length() < STORY_M
                })
                .collect();
            if !listeners.is_empty() {
                tellers.push((i, listeners, chance));
            }
        }
        for (i, listeners, chance) in tellers {
            if self.persons[i].rng.next_f32() >= chance {
                continue;
            }
            let teller = &self.persons[i];
            let place = teller
                .memory
                .places
                .iter()
                .filter(|r| {
                    r.sure >= 0.5
                        && matches!(
                            r.kind,
                            PlaceKind::Water | PlaceKind::Food | PlaceKind::Danger
                        )
                })
                .max_by(|a, b| a.sure.total_cmp(&b.sure))
                .map(|r| (r.kind, r.at, r.sure));
            let technique = teller
                .knowledge
                .known
                .keys()
                .find(|k| {
                    listeners.iter().any(|&j| {
                        let q = &self.persons[j].knowledge;
                        !q.knows(k) && !q.legends.contains(*k)
                    })
                })
                .cloned();
            let me = teller.id;
            let tell_place =
                place.is_some() && (technique.is_none() || self.persons[i].rng.next_f32() < 0.6);
            if tell_place && let Some((kind, at, sure)) = place {
                for &j in &listeners {
                    let q = &mut self.persons[j];
                    let a = q.rng.next_f64() * std::f64::consts::TAU;
                    let off = DVec3::new(a.cos(), 0.0, a.sin()) * 15.0 * q.rng.next_f64();
                    q.memory.remember(kind, at + off, now.day, sure * 0.6);
                }
                let what: &[&str] = match kind {
                    PlaceKind::Water => &["water", "there"],
                    PlaceKind::Food => &["food", "there", "many"],
                    _ => &["animal", "bad", "there"],
                };
                self.say(
                    me,
                    None,
                    Act::TellStory,
                    words(what),
                    Some(Gesture::Point),
                    now.day,
                );
            } else if let Some(k) = technique {
                for &j in &listeners {
                    let q = &mut self.persons[j].knowledge;
                    if !q.knows(&k) {
                        q.legends.insert(k.clone());
                    }
                }
                let told = words(&["they", "make", "thing"]);
                self.say(me, None, Act::TellStory, told, Some(Gesture::Show), now.day);
            }
        }
    }

    /// What a player is shown, one tick of it (V2.1 §11.2): of the people at their work within
    /// reach in front of the player's eyes, those who would teach the player — of the player's
    /// own band, or trusting it — each give insight toward its work's knowledge, as fast as their
    /// people teach; and now and then they say so. The knowledge and the insight, for the
    /// player's own.
    #[allow(clippy::too_many_arguments)]
    pub fn lessons_for(
        &mut self,
        player: u64,
        crafts: &Crafts,
        species: &SpeciesSet,
        eye: DVec3,
        yaw: f32,
        now: &Now,
        dt: f32,
    ) -> Vec<(String, f32)> {
        let Some((me, band)) = self
            .persons
            .iter()
            .find(|p| p.player == Some(player))
            .map(|p| (p.id, p.social.band))
        else {
            return Vec::new();
        };
        let facing = DVec3::new(yaw.sin() as f64, 0.0, yaw.cos() as f64);
        let mut out = Vec::new();
        let mut showing: Vec<usize> = Vec::new();
        for (i, q) in self.persons.iter().enumerate() {
            if q.tier != Tier::Full || !q.alive() || q.id == me {
                continue;
            }
            let Doing::Working { recipe } = q.mind.doing else {
                continue;
            };
            let to = q.place.pos - eye;
            let flat = DVec3::new(to.x, 0.0, to.z);
            if flat.length() > SHOW_M || flat.normalize_or_zero().dot(facing) < 0.5 {
                continue;
            }
            let Some(node) = crafts
                .recipes
                .get(recipe)
                .and_then(|r| r.def.knowledge.as_ref())
            else {
                continue;
            };
            let teach = species
                .get(&q.species)
                .and_then(|sp| sp.learning.as_ref())
                .map_or(1.0, |t| t.teach);
            let willing = q.social.band == band
                || q.social.ties.iter().any(|t| t.who == me && t.trust >= 0.3);
            if teach <= 1.0 || !willing {
                continue;
            }
            out.push((node.to_string(), TAUGHT_PER_MIN * teach * dt / 60.0));
            showing.push(i);
        }
        for i in showing {
            if self.persons[i].rng.next_f32() < dt / 20.0 {
                let teacher = self.persons[i].id;
                let show = words(&["see", "this"]);
                self.say(
                    teacher,
                    Some(me),
                    Act::Teach,
                    show,
                    Some(Gesture::Show),
                    now.day,
                );
            }
        }
        out
    }
}

//! What a person knows (v2 §12): the nodes of the technology graph they have discovered, the
//! insight gathered towards others, their skills and their journal.
//!
//! Knowledge is gained, never granted: each node listens for discovery triggers (doing,
//! seeing, inferring); every trigger heard adds its route's insight, and a node whose insight
//! reaches 1 is discovered once everything it requires is known. A first insight from a route
//! with a hint writes a hunch in the journal. Practising a technique raises its skill with
//! diminishing returns.

use std::collections::{BTreeMap, BTreeSet};

use hearth_content::Content;
use hearth_content::schema::Status;
use hearth_content::schema::knowledge::{Discovery, Route};
use hearth_content::triggers::key;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

/// How knowledge is gained in a world (v2 §12.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Only by discovery; hunches are what the content says.
    #[default]
    Discovery,
    /// By discovery, with hunches that name the technique and how near it is.
    Guided,
    /// Everything implemented is known (sandbox, testing).
    Open,
}

/// A node of the technology graph as the game reads it.
#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub era: u8,
    /// Indices of the nodes it requires.
    pub requires: Vec<usize>,
    pub routes: Vec<Discovery>,
    /// Processes it enables.
    pub enables: Vec<String>,
    pub skill: Option<String>,
    /// The real history, in a line, and its date.
    pub summary: String,
    pub date: String,
    pub implemented: bool,
}

/// The technology graph: its nodes and which of them listen for which trigger.
#[derive(Debug, Clone, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    index: FxHashMap<String, usize>,
    /// Trigger → (node, route).
    listeners: FxHashMap<String, Vec<(usize, usize)>>,
    /// Node → the nodes that require it.
    dependents: Vec<Vec<usize>>,
    /// Process → the node that enables it.
    enabled_by: FxHashMap<String, usize>,
}

impl Graph {
    pub fn from_content(c: &Content) -> Self {
        let mut g = Graph::default();
        for k in c.knowledge.iter() {
            g.index.insert(k.id.clone(), g.nodes.len());
            g.nodes.push(Node {
                id: k.id.clone(),
                name: k.name.clone(),
                era: k.era,
                requires: Vec::new(),
                routes: k.discovery.clone(),
                enables: k.enables.iter().map(|p| p.to_string()).collect(),
                skill: k.skill.clone(),
                summary: k.history.summary.clone(),
                date: k.history.date.clone(),
                implemented: k.status == Status::Implemented,
            });
        }
        g.dependents = vec![Vec::new(); g.nodes.len()];
        for (i, k) in c.knowledge.iter().enumerate() {
            let req: Vec<usize> = k
                .requires
                .iter()
                .filter_map(|r| g.index.get(r.as_str()).copied())
                .collect();
            for &r in &req {
                g.dependents[r].push(i);
            }
            g.nodes[i].requires = req;
            for p in &k.enables {
                g.enabled_by.insert(p.to_string(), i);
            }
            for (j, route) in k.discovery.iter().enumerate() {
                // Inference names the node itself, whatever the data's trigger says.
                let trigger = if route.route == Route::Inference {
                    format!("infer:{}", key(&k.id))
                } else {
                    route.trigger.clone()
                };
                g.listeners.entry(trigger).or_default().push((i, j));
            }
        }
        g
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.index.get(id).copied()
    }

    pub fn node(&self, id: &str) -> Option<&Node> {
        self.index_of(id).map(|i| &self.nodes[i])
    }

    /// The node that enables a process.
    pub fn enabled_by(&self, process: &str) -> Option<&Node> {
        self.enabled_by.get(process).map(|&i| &self.nodes[i])
    }

    /// The nodes and routes listening for a trigger.
    pub fn listeners(&self, trigger: &str) -> &[(usize, usize)] {
        self.listeners.get(trigger).map_or(&[], |v| v.as_slice())
    }

    /// Whether a node can be inferred (it has an inference route).
    pub fn inferable(&self, i: usize) -> bool {
        let n = &self.nodes[i];
        !n.requires.is_empty() && n.routes.iter().any(|r| r.route == Route::Inference)
    }
}

/// When and how a node was learned.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Learned {
    pub tick: u64,
    /// None: known from the start (Open mode) or practised again from a legend.
    #[serde(default)]
    pub route: Option<Route>,
}

/// A journal entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub tick: u64,
    pub kind: NoteKind,
    #[serde(default)]
    pub node: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteKind {
    /// Something learned.
    Discovery,
    /// A hint of something not yet understood.
    Hunch,
    /// What a person before knew, passed on (Legacy).
    Legend,
    /// The first of a kind of thing made.
    Made,
}

/// A skill: how practised a technique is.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Skill {
    /// 0–1.
    pub level: f32,
    /// The highest it has been.
    pub peak: f32,
    /// Real hours of practice.
    pub hours: f32,
    /// The tick it was last practised.
    pub last_tick: u64,
}

/// Real hours of practice that take a skill about two thirds of the way to mastery.
pub const PRACTICE_H: f32 = 25.0;
/// Game days a skill keeps before it starts to slip (Authentic).
pub const SKILL_KEEPS_DAYS: f64 = 30.0;

impl Skill {
    /// Practice of `hours` (real hours of work): diminishing returns towards 1.
    pub fn practice(&mut self, hours: f32, tick: u64) {
        let h = hours.max(0.0);
        self.level = 1.0 - (1.0 - self.level) * (-h / PRACTICE_H).exp();
        self.peak = self.peak.max(self.level);
        self.hours += h;
        self.last_tick = tick;
    }

    /// Unused for `days_idle` game days: after a month it slips a thousandth a day, never below
    /// three fifths of the best it was.
    pub fn slip(&mut self, days: f64, days_idle: f64) {
        let slipping = (days_idle - SKILL_KEEPS_DAYS).max(0.0).min(days);
        if slipping > 0.0 {
            let floor = 0.6 * self.peak;
            self.level = (self.level - 0.001 * slipping as f32).max(floor);
        }
    }

    /// The skill in words.
    pub fn words(level: f32) -> &'static str {
        match level {
            l if l < 0.1 => "a novice",
            l if l < 0.3 => "a beginner",
            l if l < 0.55 => "practised",
            l if l < 0.8 => "skilled",
            _ => "a master",
        }
    }
}

/// What hearing a trigger did.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A node discovered.
    Learned(String),
    /// A hunch written.
    Hunch(String),
}

/// Everything a person knows.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeState {
    #[serde(default)]
    pub known: BTreeMap<String, Learned>,
    /// Insight towards nodes not yet known (1 discovers).
    #[serde(default)]
    pub insight: BTreeMap<String, f32>,
    #[serde(default)]
    pub skills: BTreeMap<String, Skill>,
    #[serde(default)]
    pub journal: Vec<Note>,
    /// What a person before knew (Legacy): learned again by practising it.
    #[serde(default)]
    pub legends: BTreeSet<String>,
    /// Kinds of things made at least once.
    #[serde(default)]
    pub made: BTreeSet<String>,
}

/// The look-alike groups the player cannot yet tell apart, and the materials in each: a
/// material is in a group when it carries the group's tag (umbellifer, white_mushroom,
/// dark_berry), and the group is told apart by knowing `telling_<group>`.
pub fn hidden_looks(c: &hearth_content::Content, k: &KnowledgeState) -> Vec<(String, Vec<String>)> {
    let mut groups: Vec<String> = c
        .plants
        .iter()
        .filter_map(|p| p.look_alike.clone())
        .collect();
    groups.sort();
    groups.dedup();
    groups
        .into_iter()
        .filter(|g| !k.knows(&format!("hearth:telling_{g}")))
        .map(|g| {
            let mats = c
                .materials
                .iter()
                .filter(|m| m.tags.contains(&g))
                .map(|m| m.id.clone())
                .collect();
            (g, mats)
        })
        .collect()
}

impl KnowledgeState {
    /// Everything implemented, known (Open mode).
    pub fn open(graph: &Graph, tick: u64) -> Self {
        let mut s = Self::default();
        for n in graph.nodes.iter().filter(|n| n.implemented) {
            s.known.insert(n.id.clone(), Learned { tick, route: None });
        }
        s
    }

    pub fn knows(&self, id: &str) -> bool {
        self.known.contains_key(id)
    }

    /// Whether a process needing node `needs` (none: anyone) may be attempted: the node is
    /// known, or it is a legend to practise again.
    pub fn may_attempt(&self, needs: Option<&str>) -> bool {
        needs.is_none_or(|k| self.knows(k) || self.legends.contains(k))
    }

    pub fn skill(&self, name: &str) -> f32 {
        self.skills.get(name).map_or(0.0, |s| s.level)
    }

    pub fn practice(&mut self, skill: &str, hours: f32, tick: u64) {
        self.skills
            .entry(skill.to_owned())
            .or_default()
            .practice(hours, tick);
    }

    /// Skills unused for long slip a little (call once a game day).
    pub fn slip_skills(&mut self, tick: u64, ticks_per_day: f64) {
        for s in self.skills.values_mut() {
            let idle = tick.saturating_sub(s.last_tick) as f64 / ticks_per_day.max(1.0);
            s.slip(1.0, idle);
        }
    }

    /// Hears a trigger: insight for every node listening; discoveries and hunches.
    pub fn observe(&mut self, graph: &Graph, trigger: &str, tick: u64, mode: Mode) -> Vec<Event> {
        let mut events = Vec::new();
        for &(i, j) in graph.listeners(trigger) {
            let node = &graph.nodes[i];
            if !node.implemented || self.knows(&node.id) {
                continue;
            }
            let route = &node.routes[j];
            // A legend comes back quickly.
            let gain = if self.legends.contains(&node.id) {
                route.insight * 4.0
            } else {
                route.insight
            };
            let before = self.insight.get(&node.id).copied().unwrap_or(0.0);
            let now = (before + gain).min(1.0);
            self.insight.insert(node.id.clone(), now);
            if before == 0.0 && now < 1.0 {
                let hint = match (mode, &route.hint) {
                    (Mode::Guided, Some(h)) => Some(format!("{h} ({})", node.name)),
                    (Mode::Guided, None) => {
                        Some(format!("There is more to learn here: {}.", node.name))
                    }
                    (_, Some(h)) => Some(h.clone()),
                    _ => None,
                };
                if let Some(text) = hint {
                    self.journal.push(Note {
                        tick,
                        kind: NoteKind::Hunch,
                        node: Some(node.id.clone()),
                        text,
                    });
                    events.push(Event::Hunch(node.id.clone()));
                }
            }
            if now >= 1.0 {
                self.learn(graph, i, Some(route.route), tick, &mut events);
            }
        }
        events
    }

    /// Learns node `i` if everything it requires is known, then whatever was waiting on it.
    fn learn(
        &mut self,
        graph: &Graph,
        i: usize,
        route: Option<Route>,
        tick: u64,
        events: &mut Vec<Event>,
    ) {
        let node = &graph.nodes[i];
        if self.knows(&node.id)
            || !node
                .requires
                .iter()
                .all(|&r| self.knows(&graph.nodes[r].id))
        {
            return;
        }
        self.known.insert(node.id.clone(), Learned { tick, route });
        self.insight.remove(&node.id);
        let legend = self.legends.remove(&node.id);
        let lead = match (legend, route) {
            (true, _) => "Doing it myself at last, I have it as they told it:",
            (_, Some(Route::Experiment)) => "Trying it for myself, I found out how:",
            (_, Some(Route::Observation)) => "Watching closely, I understood:",
            (_, Some(Route::Inference)) => "Thinking it over, it came to me:",
            (_, Some(Route::Evidence)) => "From what others left behind, I worked out:",
            (_, Some(Route::Taught)) => "Shown by one who knew how, I learned:",
            (_, None) => "I know it:",
        };
        self.journal.push(Note {
            tick,
            kind: NoteKind::Discovery,
            node: Some(node.id.clone()),
            text: format!("{lead} {}.", node.name),
        });
        events.push(Event::Learned(node.id.clone()));
        // Nodes that had their insight but waited on this one.
        for &d in &graph.dependents[i] {
            let waiting = self
                .insight
                .get(&graph.nodes[d].id)
                .is_some_and(|&v| v >= 1.0);
            if waiting {
                self.learn(graph, d, route, tick, events);
            }
        }
    }

    /// Taught a node by one who knows it (V2.1 §11.2): insight toward it — or, while what it
    /// rests on is not yet known, toward the first of that — and the knowing of it when full.
    pub fn taught(&mut self, graph: &Graph, node: &str, insight: f32, tick: u64) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(mut i) = graph.index_of(node) else {
            return events;
        };
        for _ in 0..32 {
            match graph.nodes[i]
                .requires
                .iter()
                .copied()
                .find(|&r| !self.knows(&graph.nodes[r].id))
            {
                Some(r) => i = r,
                None => break,
            }
        }
        let n = &graph.nodes[i];
        if !n.implemented || self.knows(&n.id) {
            return events;
        }
        let before = self.insight.get(&n.id).copied().unwrap_or(0.0);
        let now = (before + insight.max(0.0)).min(1.0);
        self.insight.insert(n.id.clone(), now);
        if now >= 1.0 {
            self.learn(graph, i, Some(Route::Taught), tick, &mut events);
        }
        events
    }

    /// The inference triggers that thinking now would give: every node with an inference
    /// route whose prerequisites are all known, or (with `from`) only those that require
    /// `from`.
    pub fn inferences(&self, graph: &Graph, from: Option<&str>) -> Vec<String> {
        let from = from.and_then(|f| graph.index_of(f));
        graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(i, n)| {
                n.implemented
                    && !self.knows(&n.id)
                    && graph.inferable(*i)
                    && n.requires.iter().all(|&r| self.knows(&graph.nodes[r].id))
                    && from.is_none_or(|f| n.requires.contains(&f))
            })
            .map(|(_, n)| format!("infer:{}", key(&n.id)))
            .collect()
    }

    /// Doing a process whose node is a legend learns it again.
    pub fn practised(&mut self, graph: &Graph, node: &str, tick: u64) -> Vec<Event> {
        let mut events = Vec::new();
        if self.legends.contains(node)
            && let Some(i) = graph.index_of(node)
        {
            self.learn(graph, i, None, tick, &mut events);
        }
        events
    }

    /// Notes the first of a kind of thing made; true if it was the first.
    pub fn first_made(&mut self, kind: &str, name: &str, tick: u64) -> bool {
        if !self.made.insert(kind.to_owned()) {
            return false;
        }
        self.journal.push(Note {
            tick,
            kind: NoteKind::Made,
            node: None,
            text: format!("I made my first {name}."),
        });
        true
    }

    /// What a new person inherits under the Legacy rule (v2 §9.8): what the dead knew, as
    /// legends to practise again, and the journal; no skills.
    pub fn passed_on(&self, graph: &Graph, tick: u64) -> Self {
        let mut legends: BTreeSet<String> = self.legends.clone();
        legends.extend(self.known.keys().cloned());
        let mut journal = self.journal.clone();
        for id in &legends {
            let name = graph.node(id).map_or(id.as_str(), |n| n.name.as_str());
            journal.push(Note {
                tick,
                kind: NoteKind::Legend,
                node: Some(id.clone()),
                text: format!("They say the one before me knew {name}. I must learn it by doing."),
            });
        }
        Self {
            legends,
            journal,
            ..Self::default()
        }
    }
}

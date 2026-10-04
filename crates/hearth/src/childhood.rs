//! The player's childhood (V2.1 Addendum A; H3, D181). A new player is born a newborn into their
//! family and lives the moments of their people's childhood (`humans/life/moments.ron`) at the
//! world's pace — a few minutes each, at the ages and in the doings such a childhood holds —
//! while the years between pass quickly over the family's camp, until they come of age. A child
//! is safe from everything, carried while an infant, kept near their family, and sized to their
//! age; what the years teach them is their family's ways.

use hearth_content::Content;
use hearth_content::schema::life::Moment;
use hearth_craft::knowledge::Learned;
use hearth_craft::{Graph, KnowledgeState};
use hearth_protocol::ChildhoodView;
use serde::{Deserialize, Serialize};

/// Real seconds a year of childhood takes to pass between moments: with the moments' half hour
/// or so, about two hours of play from birth to coming of age.
pub const YEAR_S: f64 = 300.0;
/// How far a child strays from its family before it is fetched back (m).
pub const STRAY_M: f64 = 40.0;
/// Insight a year of childhood gives toward each of the family's ways, and the hours of
/// practice a year at each of their skills (a child knows its family's ways by twelve or so,
/// and comes of age a beginner's hand at them).
pub const INSIGHT_A_YEAR: f32 = 0.085;
pub const PRACTICE_H_A_YEAR: f32 = 0.5;

/// Where a childhood is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Phase {
    /// The years passing quickly to the next moment (or coming of age).
    Passing,
    /// A moment lived at the world's pace, until the tick it ends at.
    Moment { index: usize, until: u64 },
    /// Grown: the childhood is over.
    Grown,
}

/// A player's childhood.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Childhood {
    /// The day of the world they were born.
    pub born: f64,
    /// The next moment to live.
    pub next: usize,
    pub phase: Phase,
    /// The age (years) to which the family's ways have been learned.
    #[serde(default)]
    pub learned_to: f64,
}

/// The moments of a species' childhood, youngest first.
pub fn curriculum(content: &Content, species: &str) -> Vec<Moment> {
    let key = |id: &str| id.rsplit(':').next().unwrap_or(id).to_owned();
    let mut out: Vec<Moment> = content
        .moments
        .iter()
        .filter(|m| key(m.species.as_str()) == key(species))
        .cloned()
        .collect();
    out.sort_by(|a, b| a.age.total_cmp(&b.age));
    out
}

impl Childhood {
    /// A childhood beginning with a birth on day `born`.
    pub fn new(born: f64) -> Self {
        Self {
            born,
            next: 0,
            phase: Phase::Passing,
            learned_to: 0.0,
        }
    }

    /// The child's age (years) on a day.
    pub fn age(&self, day: f64, year_days: f64) -> f64 {
        ((day - self.born) / year_days.max(1.0)).max(0.0)
    }

    /// Whether it is over.
    pub fn grown(&self) -> bool {
        self.phase == Phase::Grown
    }

    /// The age the next moment comes at (none: the childhood's last is lived).
    pub fn next_age(&self, moments: &[Moment]) -> Option<f64> {
        moments.get(self.next).map(|m| m.age as f64)
    }

    /// The childhood as the player is told it at an age.
    pub fn view(&self, moments: &[Moment], age: f64) -> Option<ChildhoodView> {
        match &self.phase {
            Phase::Grown => None,
            Phase::Passing => Some(ChildhoodView {
                name: String::new(),
                text: String::new(),
                age: age as f32,
                passing: true,
            }),
            Phase::Moment { index, .. } => moments.get(*index).map(|m| ChildhoodView {
                name: m.name.clone(),
                text: m.text.clone(),
                age: age as f32,
                passing: false,
            }),
        }
    }
}

/// What `years` of a childhood in a family teach a child of its ways (`ways`: the family's
/// knowledge nodes): insight toward each, practice at its skill; a node is known once the
/// insight is whole. Whether anything changed.
pub fn learn(
    knowledge: &mut KnowledgeState,
    graph: &Graph,
    ways: &[String],
    years: f64,
    tick: u64,
) -> bool {
    if years <= 0.0 {
        return false;
    }
    let mut changed = false;
    for id in ways {
        let Some(node) = graph.node(id) else {
            continue;
        };
        if let Some(skill) = &node.skill {
            knowledge.practice(skill, PRACTICE_H_A_YEAR * years as f32, tick);
            changed = true;
        }
        if knowledge.knows(&node.id) {
            continue;
        }
        let before = knowledge.insight.get(&node.id).copied().unwrap_or(0.0);
        let now = before + INSIGHT_A_YEAR * years as f32;
        if now >= 1.0 {
            knowledge.insight.remove(&node.id);
            knowledge
                .known
                .insert(node.id.clone(), Learned { tick, route: None });
        } else {
            knowledge.insight.insert(node.id.clone(), now);
        }
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_forager_childhood_runs_from_birth_to_coming_of_age() {
        let content = Content::load_base();
        let m = curriculum(&content, crate::born::PLAYER_SPECIES);
        assert!(m.len() >= 5, "a curriculum of moments");
        assert_eq!(m[0].age, 0.0, "it begins at birth");
        let last = m.last().expect("moments");
        assert!(
            (last.age as f64 - crate::born::coming_of_age(&content)).abs() < 0.01,
            "it ends at coming of age"
        );
        // Sixteen years of a childhood teach a family's ways.
        let graph = Graph::from_content(&content);
        let ways: Vec<String> = ["stone_as_hammer", "digging_stick"]
            .iter()
            .filter_map(|k| {
                graph
                    .nodes
                    .iter()
                    .find(|n| n.id.ends_with(k))
                    .map(|n| n.id.clone())
            })
            .collect();
        let mut k = KnowledgeState::default();
        assert!(learn(&mut k, &graph, &ways, 4.0, 0));
        assert!(ways.iter().all(|w| !k.knows(w)), "not yet at four");
        learn(&mut k, &graph, &ways, 12.0, 0);
        assert!(ways.iter().all(|w| k.knows(w)), "known by sixteen");
    }
}

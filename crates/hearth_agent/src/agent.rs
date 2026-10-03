//! An agent: its body, what it knows and carries, its mind, and where it is.

use glam::DVec3;
use hearth_body::Body;
use hearth_craft::Graph;
use hearth_craft::knowledge::{KnowledgeState, Learned};
use hearth_fauna::live::{Medium, Stage};
use hearth_items::Carry;

use crate::kind::Kind;
use crate::mind::Mind;

/// Real hours a grown one has practised its group's skills: years of it, near mastery.
const GROWN_PRACTICE_H: f32 = 60.0;
/// A half-grown one's: learning still.
const YOUNG_PRACTICE_H: f32 = 8.0;

/// One agent.
#[derive(Debug, Clone)]
pub struct Agent {
    pub id: u64,
    /// Its kind (index into [`crate::Kinds`]).
    pub kind: usize,
    /// Its group's id.
    pub group: u64,
    pub female: bool,
    pub stage: Stage,
    /// The player's physiology at its kind's size.
    pub body: Body,
    /// What it knows: nodes, insight and skills, as the player's.
    pub knowledge: KnowledgeState,
    /// What it carries, as the player carries.
    pub carry: Carry,
    pub mind: Mind,
    /// Where its feet are, which way it faces (0 toward +z, turning toward +x), how fast it goes.
    pub pos: DVec3,
    pub yaw: f32,
    pub speed: f32,
    pub medium: Medium,
    /// Where it is going up a tree: its place in the crown.
    pub perch: Option<DVec3>,
}

impl Agent {
    /// A new one of a kind, healthy, fed and rested: a grown or half-grown one knowing what its
    /// group knows (practised, the more if grown), a young one nothing yet.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: u64,
        kind_index: usize,
        kind: &Kind,
        graph: &Graph,
        group: u64,
        female: bool,
        stage: Stage,
        pos: DVec3,
        tick: u64,
    ) -> Self {
        let mut knowledge = KnowledgeState::default();
        if stage != Stage::Young {
            let practice = if stage == Stage::Adult {
                GROWN_PRACTICE_H
            } else {
                YOUNG_PRACTICE_H
            };
            for k in &kind.knowledge {
                knowledge
                    .known
                    .insert(k.clone(), Learned { tick, route: None });
                if let Some(skill) = graph.node(k).and_then(|n| n.skill.clone())
                    && knowledge.skill(&skill) == 0.0
                {
                    knowledge.practice(&skill, practice, tick);
                }
            }
        }
        Self {
            id,
            kind: kind_index,
            group,
            female,
            stage,
            body: Body::new(&kind.body, id.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ tick),
            knowledge,
            carry: Carry::default(),
            mind: Mind::default(),
            pos,
            yaw: 0.0,
            speed: 0.0,
            medium: Medium::Ground,
            perch: None,
        }
    }

    /// Still on its way up a tree.
    pub fn climbing(&self) -> bool {
        self.medium == Medium::Tree && self.perch.is_some_and(|p| self.pos.y < p.y - 0.01)
    }

    /// The share of a grown one's size it has reached.
    pub fn growth(&self) -> f32 {
        growth_of(self.stage)
    }

    /// Its mass (kg): its kind's, more for a male (the sexes differ as a gorilla's do not, as a
    /// chimpanzee's a little), less when young.
    pub fn mass_kg(&self, kind: &Kind) -> f32 {
        let sex = if self.female { 0.85 } else { 1.15 };
        kind.mass_kg * sex * self.growth()
    }

    /// Its standing height (m).
    pub fn height_m(&self, kind: &Kind) -> f32 {
        stature(kind, self.female, self.stage)
    }

    /// Whether it is alive.
    pub fn alive(&self) -> bool {
        self.body.dead.is_none()
    }
}

/// How grown one of an age is (a fraction of the grown body's mass).
pub fn growth_of(stage: Stage) -> f32 {
    match stage {
        Stage::Young => 0.15,
        Stage::Juvenile => 0.55,
        Stage::Adult => 1.0,
    }
}

/// The standing height (m) of one of a kind by its sex and age.
pub fn stature(kind: &Kind, female: bool, stage: Stage) -> f32 {
    let sex = if female { 0.93 } else { 1.07 };
    kind.height_m * sex * growth_of(stage).powf(0.4)
}

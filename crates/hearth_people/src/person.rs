//! A person (V2.1 §2): an id and its components — what it has lived, its body, its mind, what it
//! knows, the band it belongs to, what it carries, where it is, and its own random stream. Each
//! component is its own type, its fields defaulting when a save from before they were added is
//! read (D165).

use glam::DVec3;
use hearth_body::Body;
use hearth_craft::Graph;
use hearth_craft::knowledge::{KnowledgeState, Learned};
use hearth_fauna::live::{Medium, Stage};
use hearth_items::Carry;
use hearth_math::hash::Rng;
use serde::{Deserialize, Serialize};

use crate::mind::Mind;
use crate::species::Species;
use crate::world::Now;

/// A person's id: unique in the world, never reused.
pub type PersonId = u64;

/// Real hours a grown one has practised its band's skills: years of it, near mastery.
const GROWN_PRACTICE_H: f32 = 60.0;
/// A weaned, half-grown one's: learning still.
const YOUNG_PRACTICE_H: f32 = 8.0;

/// How a person is lived now (V2.1 §17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Tier {
    /// Near the player: lived moment to moment.
    Full,
    /// A record waiting: its band's numbers live on in the ecological cells.
    #[default]
    Dormant,
}

/// Something that befell a person.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Event {
    /// Born, into a band.
    Born {
        band: u64,
    },
    /// Already alive when its band was first drawn out: its birth reckoned back from its age.
    Found {
        band: u64,
    },
    /// Came into a band from elsewhere while the player was away.
    Joined {
        band: u64,
    },
    Died {
        cause: Cause,
    },
}

/// A life's event and its day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LifeEvent {
    /// The day of the world's calendar (days since its start).
    pub day: f64,
    pub event: Event,
}

/// What a person died of.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Cause {
    /// In the player's absence, as its band's numbers fell in the ecological cells (to hunger, a
    /// hunter, sickness or age: the cells do not say which).
    WhileAway,
    /// Its body's own death, as the player's would be told.
    Body(String),
}

/// A death: when, and of what.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Died {
    pub day: f64,
    pub cause: Cause,
}

/// What a person has lived: its sex, when and where it was born and to whom, what befell it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LifeHistory {
    pub female: bool,
    /// The day it was born (days of the world's calendar since its start; before it for those
    /// already alive when first drawn out).
    pub born: f64,
    pub mother: Option<PersonId>,
    pub father: Option<PersonId>,
    /// Where it was born (or first found): world x, z.
    pub birthplace: [f64; 2],
    pub events: Vec<LifeEvent>,
    pub died: Option<Died>,
}

/// Where a person is and how it moves.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Place {
    /// Where its feet are.
    pub pos: DVec3,
    /// Which way it faces (0 toward +z, turning toward +x) and how fast it goes (m/s).
    pub yaw: f32,
    pub speed: f32,
    pub medium: Medium,
    /// Where it is going up a tree: its place in the crown.
    pub perch: Option<DVec3>,
}

/// Who a person belongs with (relationships join in H4).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Social {
    /// Its band's id.
    pub band: u64,
}

/// What a person carries (property and claims join in H11).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Possessions {
    /// What it carries, as the player carries.
    pub carry: Carry,
}

/// A person.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Person {
    pub id: PersonId,
    /// Its species' id (the profile's).
    pub species: String,
    #[serde(default)]
    pub tier: Tier,
    #[serde(default)]
    pub life: LifeHistory,
    /// The player's physiology at its species' size.
    pub body: Body,
    #[serde(default)]
    pub mind: Mind,
    /// What it knows: nodes, insight and skills, as the player's.
    #[serde(default)]
    pub knowledge: KnowledgeState,
    #[serde(default)]
    pub social: Social,
    #[serde(default)]
    pub possessions: Possessions,
    #[serde(default)]
    pub place: Place,
    /// Its two copies of every locus, from its mother and its father (V2.1 §4).
    #[serde(default)]
    pub genome: Option<crate::genome::Genome>,
    /// What its genes, development and chance make of it.
    #[serde(default)]
    pub phenotype: Option<crate::genome::Phenotype>,
    /// Its tendencies, feelings, mood, stress and values (V2.1 §5).
    #[serde(default)]
    pub psyche: crate::psyche::Psyche,
    /// Its own random stream: what it draws does not depend on who else drew first.
    pub rng: Rng,
}

/// A person's own stream, from the world's seed and its id.
pub fn stream(seed: u64, id: PersonId) -> Rng {
    Rng::new(hearth_math::hash::hash2(seed ^ 0x0070_e091_e5ee_d000, id))
}

impl Person {
    /// A new person of a species, healthy, fed and rested, born on day `born`: a weaned one
    /// knowing what its band knows (practised, the more if grown), a suckling nothing yet.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: PersonId,
        species: &Species,
        graph: &Graph,
        band: u64,
        female: bool,
        born: f64,
        pos: DVec3,
        now: Now,
        seed: u64,
    ) -> Self {
        let age = (now.day - born) / now.year_days;
        let mut knowledge = KnowledgeState::default();
        if species.weaned(age) {
            let practice = if species.stage(age) == Stage::Adult {
                GROWN_PRACTICE_H
            } else {
                YOUNG_PRACTICE_H
            };
            for k in &species.knowledge {
                knowledge.known.insert(
                    k.clone(),
                    Learned {
                        tick: now.tick,
                        route: None,
                    },
                );
                if let Some(skill) = graph.node(k).and_then(|n| n.skill.clone())
                    && knowledge.skill(&skill) == 0.0
                {
                    knowledge.practice(&skill, practice, now.tick);
                }
            }
        }
        let mut rng = stream(seed, id);
        let body = Body::new(species.body(female), rng.next_u64());
        Self {
            id,
            species: species.id.clone(),
            tier: Tier::Full,
            life: LifeHistory {
                female,
                born,
                birthplace: [pos.x, pos.z],
                ..LifeHistory::default()
            },
            body,
            mind: Mind::default(),
            knowledge,
            social: Social { band },
            possessions: Possessions::default(),
            place: Place {
                pos,
                ..Place::default()
            },
            genome: None,
            phenotype: None,
            psyche: crate::psyche::Psyche::default(),
            rng,
        }
    }

    /// Gives it a genome (and the phenotype it makes) in its species' pool: the meiosis of its
    /// parents' where they are known (a parent not met stands in as one of the pool at the
    /// place's sunlight), else a founder's drawn from the pool.
    pub fn inherit(
        &mut self,
        genetics: &crate::genome::Genetics,
        mother: Option<&crate::genome::Genome>,
        father: Option<&crate::genome::Genome>,
        sun: f32,
    ) {
        let Some(pool) = genetics.pool(&self.species) else {
            return;
        };
        let genome = if mother.is_none() && father.is_none() {
            genetics.founder(pool, sun, self.life.female, &mut self.rng)
        } else {
            let mut parent = |known: Option<&crate::genome::Genome>, female: bool| {
                known
                    .cloned()
                    .unwrap_or_else(|| genetics.founder(pool, sun, female, &mut self.rng))
            };
            let (m, f) = (parent(mother, true), parent(father, false));
            genetics.child(&m, &f, Some(self.life.female), &mut self.rng)
        };
        self.phenotype = Some(genetics.phenotype(&genome, &mut self.rng));
        self.genome = Some(genome);
    }

    /// Its stature in its species' standard deviations (0 without a phenotype).
    pub fn stature_z(&self) -> f32 {
        self.phenotype.as_ref().map_or(0.0, |p| p.z("stature"))
    }

    /// Whether it is alive.
    pub fn alive(&self) -> bool {
        self.life.died.is_none() && self.body.dead.is_none()
    }

    /// Its age (years) at a moment.
    pub fn age(&self, now: &Now) -> f64 {
        (now.day - self.life.born) / now.year_days.max(1.0)
    }

    /// Its age class at a moment.
    pub fn stage(&self, species: &Species, now: &Now) -> Stage {
        species.stage(self.age(now))
    }

    /// Its mass (kg) at a moment.
    pub fn mass_kg(&self, species: &Species, now: &Now) -> f32 {
        species.mass_kg(self.life.female, self.age(now))
    }

    /// Its standing height (m) at a moment.
    pub fn height_m(&self, species: &Species, now: &Now) -> f32 {
        species.height_m(self.life.female, self.age(now), self.stature_z())
    }

    /// The share of a grown one's size it has reached.
    pub fn growth(&self, species: &Species, now: &Now) -> f32 {
        species.growth(self.age(now))
    }

    /// Still on its way up a tree.
    pub fn climbing(&self) -> bool {
        self.place.medium == Medium::Tree
            && self
                .place
                .perch
                .is_some_and(|p| self.place.pos.y < p.y - 0.01)
    }

    /// Records an event of its life.
    pub fn record(&mut self, day: f64, event: Event) {
        self.life.events.push(LifeEvent { day, event });
    }
}

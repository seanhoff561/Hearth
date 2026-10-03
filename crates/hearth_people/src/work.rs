//! Work: a person doing a process — the same data and the same engine as the player. What it has
//! at hand is what it carries and what lies within reach; what the doing changes is applied to it
//! and to the things about it: what it used is taken from its hands and the ground, what it made
//! goes into its hands, into its mouth when it is food and it is hungry, or down beside it (a
//! knapper's flakes, a nut-cracker's shells: the traces a group leaves), its tools wear, its skill
//! grows, a slip cuts it.

use glam::DVec3;
use hearth_body::Food;
use hearth_content::Content;
use hearth_content::schema::body::BodyRegion;
use hearth_craft::engine::{self, Aimed, Bench, Lack, Outcome, Plan, Source, Surroundings};
use hearth_craft::{Crafts, food};
use hearth_items::{Items, Stack};
use hearth_math::hash::Rng;

use crate::person::Person;
use crate::species::Species;
use crate::world::Now;

/// How far (m) a person reaches: what lies within it is at hand (the player's reach).
pub const REACH_M: f64 = 2.5;

/// Things lying about that a person can reach and change: the world's things in play, a pile in
/// a test.
pub trait Things {
    /// The things lying within `reach` m of `at`: their ids and stacks.
    fn near(&self, at: DVec3, reach: f64) -> Vec<(u64, Stack)>;
    /// Takes a thing lying about (or `count` of it).
    fn take(&mut self, id: u64, count: Option<u16>) -> Option<Stack>;
    /// Lays a thing down at a place; its id.
    fn lay(&mut self, at: DVec3, stack: Stack) -> u64;
    /// Where a thing lies.
    fn place(&self, id: u64) -> Option<DVec3>;
}

/// Things lying about in a heap: each with its id and place.
#[derive(Debug, Clone, Default)]
pub struct Pile {
    pub things: Vec<(u64, DVec3, Stack)>,
    next: u64,
}

impl Pile {
    /// Lays a thing down, giving its id.
    pub fn add(&mut self, at: DVec3, stack: Stack) -> u64 {
        self.next += 1;
        self.things.push((self.next, at, stack));
        self.next
    }
}

impl Things for Pile {
    fn near(&self, at: DVec3, reach: f64) -> Vec<(u64, Stack)> {
        self.things
            .iter()
            .filter(|(_, p, _)| (*p - at).length() <= reach)
            .map(|(id, _, s)| (*id, s.clone()))
            .collect()
    }

    fn take(&mut self, id: u64, count: Option<u16>) -> Option<Stack> {
        let i = self.things.iter().position(|(t, _, _)| *t == id)?;
        let stack = &mut self.things[i].2;
        match count {
            Some(n) if n < stack.count => {
                stack.count -= n;
                let mut part = stack.clone();
                part.count = n;
                part.inside = None;
                Some(part)
            }
            _ => Some(self.things.remove(i).2),
        }
    }

    fn lay(&mut self, at: DVec3, stack: Stack) -> u64 {
        self.add(at, stack)
    }

    fn place(&self, id: u64) -> Option<DVec3> {
        self.things
            .iter()
            .find(|(t, _, _)| *t == id)
            .map(|(_, p, _)| *p)
    }
}

/// Whether a person can do a process here, and how: the engine's plan with what it carries and
/// what lies within reach (`lying`), done to what it aims at; or what it lacks — knowledge first,
/// as the player's.
#[allow(clippy::too_many_arguments)]
pub fn plan_work(
    crafts: &Crafts,
    content: &Content,
    items: &Items,
    agent: &Person,
    recipe: usize,
    lying: &[(u64, Stack)],
    aimed: Option<Aimed>,
    around: Surroundings,
) -> Result<Plan, Lack> {
    let def = &crafts.recipes[recipe].def;
    if !agent
        .knowledge
        .may_attempt(def.knowledge.as_ref().map(|k| k.as_str()))
    {
        return Err(Lack::Knowledge);
    }
    let bench = Bench::new(
        content,
        items,
        &agent.possessions.carry,
        lying.iter().map(|(id, s)| (*id, s)),
        aimed,
        around,
    );
    engine::plan(crafts, recipe, &bench, skill_for(crafts, agent, recipe))
}

/// The person's skill at a process (a middling hand where it names none).
fn skill_for(crafts: &Crafts, agent: &Person, recipe: usize) -> f32 {
    crafts.recipes[recipe]
        .def
        .skill
        .as_deref()
        .map_or(0.5, |s| agent.knowledge.skill(s))
}

/// Does a planned process: the engine rolls it, and what came of it is applied to the person and
/// the things about it. Returns the outcome; its triggers are what someone watching sees done.
#[allow(clippy::too_many_arguments)]
pub fn finish_work(
    crafts: &Crafts,
    content: &Content,
    items: &Items,
    species: &Species,
    agent: &mut Person,
    now: &Now,
    plan: &Plan,
    lying: &[(u64, Stack)],
    aimed: Option<Aimed>,
    around: Surroundings,
    things: &mut dyn Things,
    rng: &mut Rng,
) -> Outcome {
    let tick = now.tick;
    let body = species.body(agent.life.female);
    let outcome = {
        let bench = Bench::new(
            content,
            items,
            &agent.possessions.carry,
            lying.iter().map(|(id, s)| (*id, s)),
            aimed,
            around,
        );
        let skill = skill_for(crafts, agent, plan.recipe);
        engine::perform(crafts, plan, &bench, skill, rng)
    };
    // The skill grows with the work, failures too.
    if let Some((skill, hours)) = &outcome.practice {
        agent.knowledge.practice(skill, *hours, tick);
    }
    // Tools wear; one worn through breaks and is dropped.
    for (s, wear) in &outcome.wear {
        if let Source::Carried(path) = s
            && let Some(stack) = agent.possessions.carry.get_mut(path)
        {
            stack.condition = (stack.condition - wear).max(0.0);
            if stack.condition <= 0.0 {
                agent.possessions.carry.take(items, path, Some(1));
            }
        }
    }
    if let Some(injury) = &outcome.injury {
        let side = if rng.chance(0.5) {
            hearth_body::Side::Left
        } else {
            hearth_body::Side::Right
        };
        agent
            .body
            .injure(body, injury, BodyRegion::Hand, side, 0.25);
    }
    // What it used, from its hands and the ground (carried paths deepest first, so that taking
    // one does not move another).
    let mut used = outcome.used.clone();
    used.sort_by_key(|(s, _)| match s {
        Source::Carried(p) => std::cmp::Reverse(p.inside.len()),
        Source::Lying(_) => std::cmp::Reverse(0),
    });
    for (s, n) in &used {
        match s {
            Source::Carried(path) => {
                agent.possessions.carry.take(items, path, Some(*n));
            }
            Source::Lying(id) => {
                things.take(*id, Some(*n));
            }
        }
    }
    // What it made: eaten if it is food and it is hungry, kept if its hands take it, else laid
    // down beside it.
    let mass = agent.mass_kg(species, now);
    let hungry = crate::mind::Needs::of(&agent.body, body, 0.0).hunger > 0.1;
    for stack in outcome.made.clone() {
        let bite = items
            .get(&stack.id)
            .and_then(|k| food::bite_of(content, k, &stack));
        if hungry && let Some(b) = bite {
            let mut left = stack.count;
            while left > 0 {
                let f = Food {
                    kcal: b.kcal as f64,
                    protein_g: b.protein_g as f64,
                    fat_g: b.fat_g as f64,
                    carb_g: b.carb_g as f64,
                    water_l: b.water_l as f64,
                    volume_l: b.volume_l as f64,
                    fresh_days: b.fresh_days as f64,
                };
                if agent.body.eat(body, &f).is_err() {
                    break;
                }
                left -= 1;
            }
            if left == 0 {
                continue;
            }
            let mut rest = stack.clone();
            rest.count = left;
            keep_or_lay(agent, items, mass, rest, things);
            continue;
        }
        keep_or_lay(agent, items, mass, stack, things);
    }
    outcome
}

/// Into its hands or down beside it.
fn keep_or_lay(
    agent: &mut Person,
    items: &Items,
    mass: f32,
    stack: Stack,
    things: &mut dyn Things,
) {
    if let Err(stack) = agent.possessions.carry.stow(items, stack, mass) {
        let yaw = agent.place.yaw;
        let side = glam::DVec3::new(yaw.cos() as f64, 0.0, -yaw.sin() as f64) * 0.4;
        things.lay(agent.place.pos + side, stack);
    }
}

//! Planning (V2.1 §6.1, the fourth layer; H2): a goal turned into the steps that reach it — a
//! hierarchical task network over the player's process engine, as deep as a species plans. To
//! *have* a thing is met, in order of what costs least: by what it carries (or makes earlier in
//! the plan); by a thing lying in sight, gone to and picked up; or by a process that makes it —
//! each of the process's tools and inputs in turn a thing to have, its target a place to go to
//! (stones from a scatter, resin from a pine, fibre from nettles, a dead pole pulled up). The
//! plan is a list of steps the person does one by one (`sim.rs`), each checked by the engine as
//! it is done; when the world has moved on — the stone taken, the point snapped in the knapping —
//! it is made again from where things stand.

use std::collections::HashMap;

use glam::DVec3;
use hearth_content::Content;
use hearth_content::schema::process::{Effect, Input, Match, Target};
use hearth_craft::Crafts;
use hearth_craft::engine::{Aimed, matches};
use hearth_items::{ItemKind, Items, Stack};
use serde::{Deserialize, Serialize};

use crate::person::Person;
use crate::world::Senses;

/// What a person wants to have.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Want {
    /// Things answering to a process's input (an item, a form, a material, a tag), and how many.
    Item { what: Match, count: u16 },
    /// A tool of a property at least this strong.
    Tool { property: String, min: f32 },
}

/// One step of a plan.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Walk to a place.
    Go(DVec3),
    /// Pick up a thing lying there (its id).
    Take(u64),
    /// Do a process (its recipe) with what is at hand, aimed at a target where there is one.
    Do { recipe: usize, aim: Option<Aimed> },
}

/// A plan: its steps, and the next to do.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Plan {
    pub steps: Vec<Step>,
    pub next: usize,
}

impl Plan {
    pub fn step(&self) -> Option<&Step> {
        self.steps.get(self.next)
    }

    pub fn done(&self) -> bool {
        self.next >= self.steps.len()
    }
}

/// What planning draws on.
pub struct Ctx<'a> {
    pub crafts: &'a Crafts,
    pub content: &'a Content,
    pub items: &'a Items,
    pub senses: &'a dyn Senses,
    /// How deep its plans go: processes within processes (its species').
    pub depth: u8,
    /// How far (m) it looks about for things and places.
    pub look_m: f64,
}

/// How many ways of making a thing are tried before it is given up.
const TRIES: usize = 3;
/// How many wants a plan may weigh before it is given up (a person does not think forever).
const BUDGET: u32 = 20_000;
/// A tool's strength reckoned at planning: a fresh one's of middling quality.
const TOOL_SHARE: f32 = 0.85;

/// The kinds of things by form, and the processes found to make each want (built for each plan
/// made).
struct Kinds<'a> {
    by_form: HashMap<&'a str, Vec<&'a ItemKind>>,
    makers: std::cell::RefCell<HashMap<String, Vec<usize>>>,
}

impl<'a> Kinds<'a> {
    fn new(items: &'a Items) -> Self {
        let mut by_form: HashMap<&'a str, Vec<&'a ItemKind>> = HashMap::new();
        for k in items.iter() {
            if let Some(f) = k.form.as_deref() {
                by_form.entry(f).or_default().push(k);
            }
        }
        Self {
            by_form,
            makers: std::cell::RefCell::new(HashMap::new()),
        }
    }
}

/// What the plan reckons it will have as it goes.
#[derive(Debug, Clone)]
struct State {
    /// Things carried, or made earlier in the plan: their kinds and how many are free.
    have: Vec<(String, u16)>,
    /// Things lying about already meant to be taken.
    claimed: Vec<u64>,
    /// Targets already meant to be worked (a scatter of stones gathered is gone).
    worked: Vec<DVec3>,
    /// Where it will be.
    at: DVec3,
    steps: Vec<Step>,
}

impl State {
    fn add(&mut self, id: &str, n: u16) {
        if n == 0 {
            return;
        }
        match self.have.iter_mut().find(|(k, _)| k == id) {
            Some(e) => e.1 = e.1.saturating_add(n),
            None => self.have.push((id.to_owned(), n)),
        }
    }
}

/// How many of a thing an input asks for (a material's kilograms in its bulk units).
fn count_of(ctx: &Ctx, input: &Input) -> u16 {
    match &input.item {
        Match::Material(m) => ctx.crafts.units_for(ctx.items, m.as_str(), input.amount),
        _ => input.amount.round().max(1.0) as u16,
    }
}

/// Whether a kind of thing serves a want.
fn serves(ctx: &Ctx, want: &Want, kind: &ItemKind) -> bool {
    serves_in(ctx.content, want, kind)
}

fn serves_in(content: &Content, want: &Want, kind: &ItemKind) -> bool {
    match want {
        Want::Item { what, .. } => matches(what, kind, content),
        Want::Tool { property, min } => kind
            .property(property)
            .is_some_and(|v| v * TOOL_SHARE >= *min),
    }
}

/// Whether a person carries what it wants.
pub fn has(items: &Items, content: &Content, p: &Person, want: &Want) -> bool {
    let n: u32 = carried(&p.possessions.carry)
        .into_iter()
        .filter(|s| {
            items
                .get(&s.id)
                .is_some_and(|k| serves_in(content, want, k))
        })
        .map(|s| s.count as u32)
        .sum();
    n >= wanted(want) as u32
}

/// How many a want asks for (a tool, one).
fn wanted(want: &Want) -> u16 {
    match want {
        Want::Item { count, .. } => (*count).max(1),
        Want::Tool { .. } => 1,
    }
}

/// Makes a plan for a person to have what it wants, from where it stands with what it carries
/// and what it sees; none if it knows no way within its depth.
pub fn plan(ctx: &Ctx, p: &Person, want: &Want) -> Option<Plan> {
    let mut have: Vec<(String, u16)> = Vec::new();
    for s in carried(&p.possessions.carry) {
        match have.iter_mut().find(|(k, _)| *k == s.id) {
            Some(e) => e.1 = e.1.saturating_add(s.count),
            None => have.push((s.id.clone(), s.count)),
        }
    }
    let mut s = State {
        have,
        claimed: Vec::new(),
        worked: Vec::new(),
        at: p.place.pos,
        steps: Vec::new(),
    };
    let kinds = Kinds::new(ctx.items);
    let mut budget = BUDGET;
    if achieve(ctx, &kinds, p, &mut s, want, ctx.depth, &mut budget).is_some() {
        Some(Plan {
            steps: s.steps,
            next: 0,
        })
    } else {
        None
    }
}

/// Every stack a carry holds, containers' contents too.
fn carried(c: &hearth_items::Carry) -> Vec<&Stack> {
    fn walk<'a>(s: &'a Stack, out: &mut Vec<&'a Stack>) {
        out.push(s);
        if let Some(inside) = s.contents() {
            for i in &inside.items {
                walk(&i.stack, out);
            }
        }
    }
    let mut out = Vec::new();
    for s in [&c.right, &c.left, &c.back].into_iter().flatten() {
        walk(s, &mut out);
    }
    for w in &c.worn {
        for s in w.hung.iter().flatten() {
            walk(s, &mut out);
        }
    }
    out
}

/// Meets a want in the plan's state (adding the steps): the kinds of thing it will use for it;
/// none, and the state as it was, when it cannot.
fn achieve(
    ctx: &Ctx,
    kinds: &Kinds,
    p: &Person,
    s: &mut State,
    want: &Want,
    depth: u8,
    budget: &mut u32,
) -> Option<Vec<String>> {
    if *budget == 0 {
        return None;
    }
    *budget -= 1;
    let need = wanted(want);
    // Had already (carried, or made earlier in the plan).
    if let Some(used) = reserve(ctx, s, want, need) {
        return Some(used);
    }
    // Lying in sight: gone to and picked up.
    let mut lying: Vec<(u64, DVec3, Stack)> = ctx
        .senses
        .things_near(s.at, ctx.look_m)
        .into_iter()
        .filter(|(id, _)| !s.claimed.contains(id))
        .filter_map(|(id, st)| ctx.senses.place_of(id).map(|at| (id, at, st)))
        .filter(|(_, _, st)| ctx.items.get(&st.id).is_some_and(|k| serves(ctx, want, k)))
        .collect();
    lying.sort_by(|a, b| {
        (a.1 - s.at)
            .length_squared()
            .total_cmp(&(b.1 - s.at).length_squared())
            .then(a.0.cmp(&b.0))
    });
    let mut t = s.clone();
    for (id, at, st) in lying {
        t.steps.push(Step::Go(at));
        t.steps.push(Step::Take(id));
        t.at = at;
        t.claimed.push(id);
        t.add(&st.id, st.count);
        if let Some(used) = reserve(ctx, &mut t, want, need) {
            *s = t;
            return Some(used);
        }
    }
    if depth == 0 {
        return None;
    }
    // Made, or gathered, by a process it may do: the cheapest few ways tried.
    for (r, target) in ways(ctx, kinds, p, s, want).into_iter().take(TRIES) {
        let mut t = s.clone();
        let mut made = true;
        // As many times over as the want needs (a few cobbles gathered twice).
        for _ in 0..need.min(4) {
            if !work(ctx, kinds, p, &mut t, r, target.clone(), depth - 1, budget) {
                made = false;
                break;
            }
            if reserve(ctx, &mut t.clone(), want, need).is_some() {
                break;
            }
        }
        if made && let Some(used) = reserve(ctx, &mut t, want, need) {
            *s = t;
            return Some(used);
        }
    }
    None
}

/// Takes `need` of what serves a want from what the plan has (a tool is not used up): the kinds
/// taken, the best of them first.
fn reserve(ctx: &Ctx, s: &mut State, want: &Want, need: u16) -> Option<Vec<String>> {
    let tool = matches!(want, Want::Tool { .. });
    let mut left = need;
    let mut takes: Vec<(usize, u16)> = Vec::new();
    // The things that serve, a tool's strongest first (the flint flake before the basalt).
    let mut order: Vec<usize> = (0..s.have.len())
        .filter(|&i| s.have[i].1 > 0)
        .filter(|&i| {
            ctx.items
                .get(&s.have[i].0)
                .is_some_and(|k| serves(ctx, want, k))
        })
        .collect();
    if let Want::Tool { property, .. } = want {
        let strength = |i: &usize| {
            ctx.items
                .get(&s.have[*i].0)
                .and_then(|k| k.property(property))
                .unwrap_or(0.0)
        };
        order.sort_by(|a, b| strength(b).total_cmp(&strength(a)));
    }
    for i in order {
        if left == 0 {
            break;
        }
        let take = s.have[i].1.min(left);
        takes.push((i, take));
        left -= take;
    }
    if left > 0 {
        return None;
    }
    let used: Vec<String> = takes.iter().map(|(i, _)| s.have[*i].0.clone()).collect();
    if !tool {
        for (i, n) in takes {
            s.have[i].1 -= n;
        }
    }
    Some(used)
}

/// The processes that could make what is wanted, with their targets, cheapest first.
fn ways(
    ctx: &Ctx,
    kinds: &Kinds,
    p: &Person,
    s: &State,
    want: &Want,
) -> Vec<(usize, Option<(DVec3, Aimed)>)> {
    let mut out: Vec<(f32, usize, Option<(DVec3, Aimed)>)> = Vec::new();
    let key = format!("{want:?}");
    let known = kinds.makers.borrow().get(&key).cloned();
    let makers = known.unwrap_or_else(|| {
        let found: Vec<usize> = ctx
            .crafts
            .recipes
            .iter()
            .enumerate()
            .filter(|(_, recipe)| {
                let def = &recipe.def;
                // What a person plans: making and gathering, not building, felling or
                // fire-making (those come with their systems).
                matches!(def.effect, Effect::Keep | Effect::Remove | Effect::Deplete)
                    && def.station.is_none()
                    && def.places.is_none()
                    && p.knowledge
                        .may_attempt(def.knowledge.as_ref().map(|k| k.as_str()))
                    && def.outputs.iter().filter(|o| o.chance >= 0.5).any(|o| {
                        produced(ctx, kinds, &o.item)
                            .iter()
                            .any(|k| serves(ctx, want, k))
                    })
            })
            .map(|(r, _)| r)
            .collect();
        kinds.makers.borrow_mut().insert(key, found.clone());
        found
    });
    for r in makers {
        let def = &ctx.crafts.recipes[r].def;
        let (target, walk) = match &def.target {
            None => (None, 0.0),
            Some(Target::Block(b)) => {
                let found = ctx
                    .senses
                    .targets_near(s.at, ctx.look_m)
                    .into_iter()
                    .filter(|(at, _)| !s.worked.iter().any(|w| (*w - *at).length() < 0.5))
                    .filter(|(_, a)| match a {
                        Aimed::Block { name, material, .. } => {
                            let m = material
                                .as_deref()
                                .and_then(|m| ctx.content.materials.get(m));
                            hearth_content::triggers::block_matches(b, name, m)
                        }
                        _ => false,
                    })
                    .min_by(|a, b| {
                        (a.0 - s.at)
                            .length_squared()
                            .total_cmp(&(b.0 - s.at).length_squared())
                    });
                match found {
                    Some((at, aimed)) => (Some((at, aimed)), (at - s.at).length() as f32),
                    None => continue,
                }
            }
            // Water, fires, ground and things as targets come with their systems.
            Some(_) => continue,
        };
        // Its hours, the walk to its target (at a walk), a little for each thing it asks for.
        let cost =
            def.duration.hours + walk / 4000.0 + 0.05 * (def.inputs.len() + def.tools.len()) as f32;
        out.push((cost, r, target));
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    out.into_iter().map(|(_, r, a)| (r, a)).collect()
}

/// The kinds of thing an output makes (a form's in any material it may come in).
fn produced<'a>(ctx: &Ctx<'a>, kinds: &Kinds<'a>, m: &Match) -> Vec<&'a ItemKind> {
    match m {
        Match::Item(id) => ctx.items.get(id.as_str()).into_iter().collect(),
        Match::Form { form, materials } => kinds
            .by_form
            .get(form.as_str())
            .map(|v| {
                v.iter()
                    .copied()
                    .filter(|k| {
                        materials.as_ref().is_none_or(|f| {
                            k.material
                                .as_deref()
                                .and_then(|mid| {
                                    ctx.content
                                        .materials
                                        .get(mid)
                                        .map(|mat| f.matches(mid, mat))
                                })
                                .unwrap_or(false)
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Match::Material(id) => ctx
            .crafts
            .bulk_item(ctx.items, id.as_str())
            .into_iter()
            .collect(),
        Match::Tag(_) | Match::Garment(_) => Vec::new(),
    }
}

/// Plans a process: its tools and inputs had, the place gone to, the work done; what it makes
/// counted as had.
#[allow(clippy::too_many_arguments)]
fn work(
    ctx: &Ctx,
    kinds: &Kinds,
    p: &Person,
    s: &mut State,
    r: usize,
    target: Option<(DVec3, Aimed)>,
    depth: u8,
    budget: &mut u32,
) -> bool {
    let def = &ctx.crafts.recipes[r].def;
    // Its tools, each held aside while the inputs are found (the hammer is not the stone struck).
    let mut held: Vec<String> = Vec::new();
    for t in &def.tools {
        let w = Want::Tool {
            property: t.property.clone(),
            min: t.min,
        };
        let Some(used) = achieve(ctx, kinds, p, s, &w, depth, budget) else {
            return false;
        };
        if let Some(id) = used.first()
            && let Some(e) = s.have.iter_mut().find(|(k, n)| k == id && *n > 0)
        {
            e.1 -= 1;
            held.push(id.clone());
        }
    }
    // Inputs, and what material the outputs will be made of (the first input's, else the
    // target's).
    let mut material: Option<String> = match &target {
        Some((_, Aimed::Block { material, .. })) => material.clone(),
        _ => None,
    };
    for input in &def.inputs {
        let n = count_of(ctx, input);
        let w = Want::Item {
            what: input.item.clone(),
            count: n,
        };
        let Some(used) = achieve(ctx, kinds, p, s, &w, depth, budget) else {
            return false;
        };
        // The outputs are of the first input's material (as the engine makes them).
        if material.is_none()
            && let Some(id) = used.first()
        {
            material = ctx.items.get(id).and_then(|k| k.material.clone());
        }
        if !input.consumed {
            // Used and kept: had again after.
            for id in used {
                s.add(&id, n);
            }
        }
    }
    // The tools back in hand.
    for id in held {
        s.add(&id, 1);
    }
    // The place: gone to and worked (a scatter gathered is gone).
    let aim = match target {
        Some((at, aimed)) => {
            s.steps.push(Step::Go(at));
            s.at = at;
            if def.effect != Effect::Keep {
                s.worked.push(at);
            }
            Some(aimed)
        }
        None => None,
    };
    s.steps.push(Step::Do { recipe: r, aim });
    // What it makes (the least it will make; a chancy byproduct not counted on).
    for o in def.outputs.iter().chain(&def.byproducts) {
        if o.chance < 0.5 {
            continue;
        }
        let made = match &o.item {
            Match::Material(m) => ctx.crafts.bulk_item(ctx.items, m.as_str()).map(|k| {
                (
                    k.id.clone(),
                    ctx.crafts
                        .units_for(ctx.items, m.as_str(), o.amount.0.max(1e-3)),
                )
            }),
            Match::Item(id) => Some((id.to_string(), o.amount.0.round().max(1.0) as u16)),
            Match::Form { form, materials } => {
                let id = material
                    .as_deref()
                    .map(|m| hearth_content::generate::generated_id(form.as_str(), m))
                    .filter(|id| ctx.items.get(id).is_some())
                    .or_else(|| {
                        produced(
                            ctx,
                            kinds,
                            &Match::Form {
                                form: form.clone(),
                                materials: materials.clone(),
                            },
                        )
                        .first()
                        .map(|k| k.id.clone())
                    });
                id.map(|id| (id, o.amount.0.round().max(1.0) as u16))
            }
            Match::Tag(_) | Match::Garment(_) => None,
        };
        if let Some((id, n)) = made {
            s.add(&id, n);
        }
    }
    true
}

//! The process engine (v2 §11.3): which processes a person can do with what is at hand and what
//! they look at, how long each takes, and what comes of it.
//!
//! The engine reads the world through a [`Bench`] (the things at hand, what is aimed at, the
//! surroundings) and returns [`Plan`]s and [`Outcome`]s; it changes nothing itself, so the
//! server applies outcomes and the client can list what is possible from the same code.
//!
//! * **Inputs** come from the target thing, the hands, the containers carried and things lying
//!   within reach, in that order. Bulk materials (`Material` inputs, by the kilogram) are
//!   counted in the units their bulk form holds (cuts of meat, handfuls of grass).
//! * **Tools** are things in hand with a property at or above the minimum; their strength is
//!   what their making and wear leave of it.
//! * **Time** is the real duration of a skilled person, slower for a novice (twice as long at
//!   no skill) and with poor tools, faster with fine ones.
//! * **Failures** come at their chance for a novice, falling with skill towards their least;
//!   coarse stone fails more often in knapping, damp air in fire-making.
//! * **Outputs** are made of the material in play (that of the first input, the target thing or
//!   the target block) unless they say; their quality comes from skill and material.

use hearth_content::Content;
use hearth_content::generate::generated_id;
use hearth_content::schema::process::{Condition, Effect, Input, Match, Output, Process, Target};
use hearth_content::schema::{Season, Status};
use hearth_content::triggers::{block_keys, item_keys, key, material_keys, with_verb};

use crate::food::keeps_days;
use hearth_items::{Carry, Hand, ItemKind, Items, Path, Root, Stack};
use hearth_math::hash::Rng;
use rustc_hash::FxHashMap;

/// A process the engine knows: from the content, or made for laying out a workstation.
#[derive(Debug, Clone)]
pub struct Recipe {
    pub def: Process,
    /// The workstation it lays out, for a building process.
    pub builds: Option<String>,
}

impl Recipe {
    pub fn id(&self) -> &str {
        &self.def.id
    }
}

/// Every process there is, and how bulk materials are counted.
#[derive(Debug, Clone, Default)]
pub struct Crafts {
    pub recipes: Vec<Recipe>,
    index: FxHashMap<String, usize>,
    /// Material → the item its bulk form makes of it.
    bulk: FxHashMap<String, String>,
}

impl Crafts {
    /// The content's implemented processes, and a building process for each implemented
    /// workstation (`build_<station>`).
    pub fn from_content(c: &Content, items: &Items) -> Self {
        let mut recipes: Vec<Recipe> = c
            .processes
            .iter()
            .filter(|p| p.status == Status::Implemented)
            .map(|p| Recipe {
                def: p.clone(),
                builds: None,
            })
            .collect();
        for w in c
            .workstations
            .iter()
            .filter(|w| w.status == Status::Implemented)
        {
            let (ns, path) = w.id.split_once(':').unwrap_or(("hearth", &w.id));
            recipes.push(Recipe {
                def: Process {
                    id: format!("{ns}:build_{path}"),
                    name: w.name.clone(),
                    action: w
                        .action
                        .clone()
                        .unwrap_or_else(|| format!("build a {}", w.name.to_lowercase())),
                    verb: Some("build".into()),
                    target: Some(Target::Ground),
                    effect: Effect::Keep,
                    inputs: w.parts.clone(),
                    tools: Vec::new(),
                    station: None,
                    conditions: Vec::new(),
                    duration: w.build,
                    knowledge: w.knowledge.clone(),
                    skill: None,
                    outputs: Vec::new(),
                    byproducts: Vec::new(),
                    failures: Vec::new(),
                    teaches: Vec::new(),
                    attended: true,
                    mets: 3.0,
                    wear: 0.0,
                    harvests: None,
                    treats: None,
                    places: None,
                    firing: None,
                    status: Status::Implemented,
                    notes: None,
                    realism_source: None,
                    uncertain: false,
                },
                builds: Some(w.id.clone()),
            });
        }
        let index = recipes
            .iter()
            .enumerate()
            .map(|(i, r)| (r.def.id.clone(), i))
            .collect();
        let mut bulk = FxHashMap::default();
        for k in items.iter().filter(|k| k.has_tag("bulk")) {
            if let Some(m) = &k.material {
                bulk.entry(m.clone()).or_insert_with(|| k.id.clone());
            }
        }
        Self {
            recipes,
            index,
            bulk,
        }
    }

    pub fn get(&self, id: &str) -> Option<&Recipe> {
        self.index.get(id).map(|&i| &self.recipes[i])
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.index.get(id).copied()
    }

    /// The workstation a block stands for.
    pub fn station_of_block<'a>(&self, c: &'a Content, block: &str) -> Option<&'a str> {
        c.workstations
            .iter()
            .find(|w| {
                w.block
                    .as_ref()
                    .is_some_and(|b| key(b.as_str()) == key(block))
            })
            .map(|w| w.id.as_str())
    }

    /// The item a material comes in, in bulk.
    pub fn bulk_item<'a>(&self, items: &'a Items, material: &str) -> Option<&'a ItemKind> {
        self.bulk.get(material).and_then(|id| items.get(id))
    }

    /// How many units of the bulk form make `kg` of a material (at least one).
    pub fn units_for(&self, items: &Items, material: &str, kg: f32) -> u16 {
        let unit = self
            .bulk_item(items, material)
            .map_or(1.0, |k| k.mass_kg.max(1e-4));
        ((kg / unit) - 1e-3).ceil().max(1.0) as u16
    }
}

/// Where a thing used in a process is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Carried, at this path.
    Carried(Path),
    /// Lying in the world (its id).
    Lying(u64),
}

/// A thing at hand.
#[derive(Debug, Clone)]
pub struct Handy<'a> {
    pub source: Source,
    pub stack: &'a Stack,
    /// Held in this hand.
    pub hand: Option<Hand>,
}

/// What a person looks at.
#[derive(Debug, Clone, PartialEq)]
pub enum Aimed {
    /// A block: its name and material, whether its top is open ground to work on, and whether
    /// there is room for a piece to be put up where the aim puts it (V2-8).
    Block {
        name: String,
        material: Option<String>,
        ground: bool,
        room: bool,
    },
    /// Water.
    Water,
    /// A thing lying in the world (its id).
    Thing(u64),
    /// A workstation (its id) and its fire, if it has one.
    Station { id: String, fire: Option<FireSeen> },
    /// A natural fire.
    Fire(FireSeen),
    /// A live animal (V2-12): its kind's content id, whether people keep it, whether it is
    /// young, female, and of a kind people can keep.
    Animal {
        species: String,
        kept: bool,
        young: bool,
        female: bool,
        domesticable: bool,
    },
}

/// A fire as a process sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FireSeen {
    pub lit: bool,
    pub temp_c: f32,
}

/// The weather and place where the work is done.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Surroundings {
    /// Rain or snow falling on the work.
    pub raining: bool,
    /// 0–1 relative humidity of the air.
    pub humidity: f32,
    pub daylight: bool,
    pub air_c: f32,
    /// Under a roof.
    pub sheltered: bool,
    /// Open water within reach.
    pub water_near: bool,
    /// Features within sight ("wildfire", "river").
    pub near: Vec<String>,
    /// The season where the work is.
    pub season: Option<Season>,
    /// Height above the sea (m): water boils cooler up high, and boiling takes longer.
    pub altitude_m: f32,
}

/// What a process can draw on: the things at hand, what is looked at, the surroundings.
pub struct Bench<'a> {
    pub content: &'a Content,
    pub items: &'a Items,
    pub at_hand: Vec<Handy<'a>>,
    pub aimed: Option<Aimed>,
    pub around: Surroundings,
}

impl<'a> Bench<'a> {
    /// The things a person has at hand: what they carry (hands first, then the back, what
    /// hangs from their garments and what every container holds) and things lying in reach.
    pub fn new(
        content: &'a Content,
        items: &'a Items,
        carry: &'a Carry,
        lying: impl IntoIterator<Item = (u64, &'a Stack)>,
        aimed: Option<Aimed>,
        around: Surroundings,
    ) -> Self {
        let mut at_hand = Vec::new();
        let mut roots: Vec<(Root, &'a Stack)> = Vec::new();
        if let Some(s) = &carry.right {
            roots.push((Root::Hand(Hand::Right), s));
        }
        if let Some(s) = &carry.left {
            roots.push((Root::Hand(Hand::Left), s));
        }
        if let Some(s) = &carry.back {
            roots.push((Root::Back, s));
        }
        for (i, w) in carry.worn.iter().enumerate() {
            for (p, h) in w.hung.iter().enumerate() {
                if let Some(s) = h {
                    roots.push((Root::Hung(i, p), s));
                }
            }
        }
        for (root, stack) in roots {
            let hand = match root {
                Root::Hand(h) => Some(h),
                _ => None,
            };
            walk(&Path::at(root), stack, hand, &mut at_hand);
        }
        // Things lying in reach, the one looked at first.
        let target = match aimed {
            Some(Aimed::Thing(id)) => Some(id),
            _ => None,
        };
        let mut lying: Vec<(u64, &'a Stack)> = lying.into_iter().collect();
        lying.sort_by_key(|(id, _)| Some(*id) != target);
        for (id, stack) in lying {
            at_hand.push(Handy {
                source: Source::Lying(id),
                stack,
                hand: None,
            });
        }
        Self {
            content,
            items,
            at_hand,
            aimed,
            around,
        }
    }

    fn kind(&self, h: &Handy) -> Option<&'a ItemKind> {
        self.items.get(&h.stack.id)
    }

    /// The thing looked at, if it is one lying in the world.
    fn target_thing(&self) -> Option<&Handy<'a>> {
        match self.aimed {
            Some(Aimed::Thing(id)) => self.at_hand.iter().find(|h| h.source == Source::Lying(id)),
            _ => None,
        }
    }
}

fn walk<'a>(path: &Path, stack: &'a Stack, hand: Option<Hand>, out: &mut Vec<Handy<'a>>) {
    out.push(Handy {
        source: Source::Carried(path.clone()),
        stack,
        hand,
    });
    if let Some(c) = stack.contents() {
        for (i, p) in c.items.iter().enumerate() {
            walk(&path.inner(i), &p.stack, None, out);
        }
    }
}

/// Whether a kind of thing answers to what a process asks for.
pub fn matches(m: &Match, kind: &ItemKind, c: &Content) -> bool {
    match m {
        Match::Item(r) => kind.id == r.as_str(),
        Match::Form { form, materials } => {
            kind.form.as_deref() == Some(form.as_str())
                && materials.as_ref().is_none_or(|f| {
                    kind.material
                        .as_deref()
                        .and_then(|mid| c.materials.get(mid).map(|mat| f.matches(mid, mat)))
                        .unwrap_or(false)
                })
        }
        Match::Material(r) => kind.material.as_deref() == Some(r.as_str()) && kind.has_tag("bulk"),
        Match::Tag(t) => kind.has_tag(t),
        Match::Garment(g) => kind.wear.as_ref().is_some_and(|w| w.garment == g.as_str()),
    }
}

/// Why a process cannot be done now.
#[derive(Debug, Clone, PartialEq)]
pub enum Lack {
    /// It needs knowledge the person lacks.
    Knowledge,
    /// Not what it is done to.
    Target,
    /// No tool in hand with this property strong enough.
    Tool(String),
    /// Too little of this (its words).
    Input(String),
    /// The conditions are wrong (in words).
    Condition(String),
}

/// How a process will be done: what it uses, with which tools, for how long.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub recipe: usize,
    /// Used up: where, and how many.
    pub uses: Vec<(Source, u16)>,
    /// Used but kept (an anvil stone, the core struck).
    pub keeps: Vec<Source>,
    /// Tools in hand and their strength.
    pub tools: Vec<(Source, f32)>,
    /// What outputs are made of when they do not say.
    pub material: Option<String>,
    /// Hours of work (played as long as they really take).
    pub hours: f32,
}

fn input_words(i: &Input, c: &Content) -> String {
    let what = match &i.item {
        Match::Item(r) => c
            .items
            .get(r.as_str())
            .map_or(key(r.as_str()).replace('_', " "), |d| d.name.clone()),
        Match::Form { form, .. } => c.forms.get(form.as_str()).map_or_else(
            || key(form.as_str()).replace('_', " "),
            |f| f.name.replace("{material}", "").trim().to_owned(),
        ),
        Match::Material(r) => c
            .materials
            .get(r.as_str())
            .map_or(key(r.as_str()).replace('_', " "), |m| m.name.clone()),
        Match::Tag(t) => t.replace('_', " "),
        Match::Garment(g) => key(g.as_str()).replace('_', " "),
    };
    match &i.item {
        Match::Material(_) => format!("{:.2} kg of {what}", i.amount),
        _ if i.amount > 1.0 => format!("{} × {what}", i.amount.round() as u32),
        _ => what,
    }
}

/// Whether what is looked at is what a process is done to.
fn target_ok(r: &Recipe, bench: &Bench) -> bool {
    let c = bench.content;
    if let Some(station) = &r.def.station {
        let here =
            matches!(&bench.aimed, Some(Aimed::Station { id, .. }) if id == station.as_str());
        // A fire is lit only when it is not, and banked only when it burns.
        let fire = fire_at(bench);
        return here
            && match r.def.effect {
                Effect::Ignite => fire.is_some_and(|f| !f.lit),
                Effect::Bank => fire.is_some_and(|f| f.lit),
                _ => true,
            };
    }
    match (&r.def.target, &bench.aimed) {
        (None, _) => true,
        (Some(Target::Water), Some(Aimed::Water)) => true,
        (Some(Target::Fire), Some(Aimed::Fire(f))) => f.lit,
        (Some(Target::Fire), Some(Aimed::Station { fire: Some(f), .. })) => f.lit,
        // A piece goes up where there is room for it; other work on open ground.
        (Some(Target::Ground), Some(Aimed::Block { ground, room, .. })) => {
            if r.def.effect == Effect::Place {
                *room
            } else {
                *ground
            }
        }
        (Some(Target::Block(b)), Some(Aimed::Block { name, material, .. })) => {
            let m = material.as_deref().and_then(|m| c.materials.get(m));
            hearth_content::triggers::block_matches(b, name, m)
        }
        (Some(Target::Thing(m)), Some(Aimed::Thing(_))) => bench
            .target_thing()
            .and_then(|h| bench.kind(h))
            .is_some_and(|k| matches(m, k, c)),
        (
            Some(Target::Animal(m)),
            Some(Aimed::Animal {
                kept,
                young,
                female,
                domesticable,
                ..
            }),
        ) => m.accepts(*kept, *young, *female, *domesticable),
        _ => false,
    }
}

/// The fire the work is done at, if any.
fn fire_at(bench: &Bench) -> Option<FireSeen> {
    match &bench.aimed {
        Some(Aimed::Station { fire, .. }) => *fire,
        Some(Aimed::Fire(f)) => Some(*f),
        _ => None,
    }
}

fn condition_ok(cond: &Condition, bench: &Bench, carried_water: bool) -> Result<(), String> {
    let a = &bench.around;
    let ok = match cond {
        Condition::HeatAtLeastC(t) => fire_at(bench).is_some_and(|f| f.lit && f.temp_c >= *t),
        Condition::Water => {
            a.water_near || matches!(bench.aimed, Some(Aimed::Water)) || carried_water
        }
        Condition::Dry => !a.raining,
        Condition::Daylight => a.daylight,
        Condition::ColdBelowC(t) => a.air_c < *t,
        Condition::Sheltered => a.sheltered,
        Condition::Near(f) => a.near.iter().any(|n| n == f),
        Condition::Season(s) => a.season.is_none_or(|now| s.contains(&now)),
    };
    if ok {
        return Ok(());
    }
    Err(match cond {
        // What a person can see: a fire too small for the work, or one not burning.
        Condition::HeatAtLeastC(t) => match fire_at(bench) {
            Some(f) if f.lit => "a hotter fire (more wood on it)".into(),
            Some(_) => "the fire lit".into(),
            None => format!("a fire of {t:.0} °C"),
        },
        Condition::Water => "water to hand".into(),
        Condition::Dry => "dry weather".into(),
        Condition::Daylight => "daylight".into(),
        Condition::ColdBelowC(t) => format!("air below {t:.0} °C"),
        Condition::Sheltered => "a roof overhead".into(),
        Condition::Near(f) => format!("a {f} nearby"),
        Condition::Season(s) => {
            let names: Vec<&str> = s
                .iter()
                .map(|s| match s {
                    Season::Spring => "spring",
                    Season::Summer => "summer",
                    Season::Autumn => "autumn",
                    Season::Winter => "winter",
                })
                .collect();
            format!("the season ({})", names.join(", "))
        }
    })
}

/// Speed factor of a tool of strength `v` (1 for a middling tool, faster for a fine one,
/// slower for a poor one).
fn tool_factor(v: f32) -> f32 {
    (0.5 / v.max(0.05)).sqrt().clamp(0.6, 2.0)
}

/// Plans recipe `r` with what is on the bench, at skill `skill` (0–1). What is worked is
/// chosen first and the tools from what is left in hand (the flint struck, the granite the
/// hammer); if that leaves no tool, the tools are chosen first.
pub fn plan(crafts: &Crafts, r: usize, bench: &Bench, skill: f32) -> Result<Plan, Lack> {
    let recipe = &crafts.recipes[r];
    if !target_ok(recipe, bench) {
        return Err(Lack::Target);
    }
    match plan_ordered(crafts, r, bench, skill, false) {
        Err(Lack::Tool(_)) => plan_ordered(crafts, r, bench, skill, true),
        other => other,
    }
}

fn choose_tools(
    def: &Process,
    bench: &Bench,
    free: &mut [u16],
) -> Result<Vec<(Source, f32)>, Lack> {
    let mut tools = Vec::new();
    for t in &def.tools {
        // The strongest thing in hand with the property.
        let best = bench
            .at_hand
            .iter()
            .enumerate()
            .filter(|(i, h)| h.hand.is_some() && free[*i] > 0)
            .filter_map(|(i, h)| h.stack.property(bench.items, &t.property).map(|v| (i, v)))
            .filter(|(_, v)| *v >= t.min)
            .max_by(|a, b| a.1.total_cmp(&b.1));
        let Some((i, v)) = best else {
            return Err(Lack::Tool(t.property.replace('_', " ")));
        };
        free[i] = 0;
        tools.push((bench.at_hand[i].source.clone(), v));
    }
    Ok(tools)
}

fn plan_ordered(
    crafts: &Crafts,
    r: usize,
    bench: &Bench,
    skill: f32,
    tools_first: bool,
) -> Result<Plan, Lack> {
    let def = &crafts.recipes[r].def;
    let c = bench.content;
    // Units still free on each stack at hand.
    let mut free: Vec<u16> = bench.at_hand.iter().map(|h| h.stack.count).collect();
    let mut tools = if tools_first {
        choose_tools(def, bench, &mut free)?
    } else {
        Vec::new()
    };
    // Inputs, the target thing first.
    let target_i = bench
        .target_thing()
        .and_then(|t| bench.at_hand.iter().position(|h| h.source == t.source));
    let mut order: Vec<usize> = Vec::with_capacity(bench.at_hand.len());
    order.extend(target_i);
    // Then what is at hand, of what goes off the freshest first.
    let mut rest: Vec<usize> = (0..bench.at_hand.len())
        .filter(|i| Some(*i) != target_i)
        .collect();
    rest.sort_by(|a, b| {
        bench.at_hand[*a]
            .stack
            .decay
            .total_cmp(&bench.at_hand[*b].stack.decay)
    });
    order.extend(rest);
    let mut uses: Vec<(Source, u16)> = Vec::new();
    let mut keeps = Vec::new();
    let mut material: Option<String> = None;
    for input in &def.inputs {
        let need = match &input.item {
            Match::Material(m) => crafts.units_for(bench.items, m.as_str(), input.amount),
            _ => input.amount.round().max(1.0) as u16,
        };
        let mut got = 0u16;
        for &i in &order {
            if got >= need {
                break;
            }
            let h = &bench.at_hand[i];
            if free[i] == 0 || !bench.kind(h).is_some_and(|k| matches(&input.item, k, c)) {
                continue;
            }
            // A container with things in it is not used up.
            if input.consumed && h.stack.contents().is_some_and(|c| !c.items.is_empty()) {
                continue;
            }
            let n = free[i].min(need - got);
            free[i] -= n;
            got += n;
            if material.is_none() {
                material = bench.kind(h).and_then(|k| k.material.clone());
            }
            if input.consumed {
                uses.push((h.source.clone(), n));
            } else {
                keeps.push(h.source.clone());
            }
        }
        if got < need {
            return Err(Lack::Input(input_words(input, c)));
        }
    }
    if !tools_first {
        tools = choose_tools(def, bench, &mut free)?;
    }
    // What the work is done to gives the material when the inputs do not.
    if material.is_none() {
        material = match &bench.aimed {
            Some(Aimed::Thing(_)) => bench
                .target_thing()
                .and_then(|h| bench.kind(h))
                .and_then(|k| k.material.clone()),
            Some(Aimed::Block { material, .. }) => material.clone(),
            _ => None,
        };
    }
    let carried_water = bench.at_hand.iter().any(|h| h.stack.liquid_l >= 0.25);
    for cond in &def.conditions {
        condition_ok(cond, bench, carried_water).map_err(Lack::Condition)?;
    }
    // Time: the skilled person's, slower for a novice, by the tools; boiling the slower the
    // cooler water boils up high (cooking's pace about halves for each 10 °C: Q10 ≈ 2).
    let mut hours = def.duration.hours;
    if def.verb.as_deref() == Some("boil") {
        hours *= boiling_slowdown(bench.around.altitude_m);
    }
    if def.attended {
        hours *= 2.0 - skill.clamp(0.0, 1.0);
        for (_, v) in &tools {
            hours *= tool_factor(*v);
        }
    }
    Ok(Plan {
        recipe: r,
        uses,
        keeps,
        tools,
        material,
        hours,
    })
}

/// How much longer boiling takes at a height (m) than at the sea: water boils some 10 °C cooler
/// at 3,000 m, and cooking's pace about halves for each 10 °C (Q10 ≈ 2).
pub fn boiling_slowdown(altitude_m: f32) -> f32 {
    let boiling = hearth_math::atmosphere::boiling_c(altitude_m as f64);
    2f64.powf(((100.0 - boiling) / 10.0).max(0.0)) as f32
}

/// What may be done with what is on the bench: every recipe the person may attempt that is
/// done to what they look at (or to nothing, when its first input is in hand), with its plan
/// or what it lacks. Doable ones come first.
pub fn offers(
    crafts: &Crafts,
    bench: &Bench,
    may_attempt: &dyn Fn(&Recipe) -> bool,
    skill: &dyn Fn(&Recipe) -> f32,
) -> Vec<(usize, Result<Plan, Lack>)> {
    let c = bench.content;
    let mut out: Vec<(usize, Result<Plan, Lack>)> = Vec::new();
    for (i, r) in crafts.recipes.iter().enumerate() {
        if !may_attempt(r) {
            continue;
        }
        let p = plan(crafts, i, bench, skill(r));
        match &p {
            Err(Lack::Target) | Err(Lack::Knowledge) => continue,
            Err(_) if r.def.target == Some(Target::Ground) => continue,
            Err(_) => {
                // Things in hand with nothing aimed at: only if the first input is held.
                let aimed = r.def.target.is_some() || r.def.station.is_some();
                let held = r.def.inputs.first().is_some_and(|input| {
                    bench.at_hand.iter().any(|h| {
                        h.hand.is_some()
                            && bench.kind(h).is_some_and(|k| matches(&input.item, k, c))
                    })
                });
                if !aimed && !held {
                    continue;
                }
            }
            Ok(_) => {}
        }
        out.push((i, p));
    }
    out.sort_by_key(|(_, p)| p.is_err());
    out
}

/// What came of doing a process.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Outcome {
    /// It worked.
    pub done: bool,
    /// The failure, in words.
    pub failure: Option<String>,
    /// An injury the failure caused (an injury id).
    pub injury: Option<String>,
    /// What is used up.
    pub used: Vec<(Source, u16)>,
    /// What was made.
    pub made: Vec<Stack>,
    /// Condition each tool lost.
    pub wear: Vec<(Source, f32)>,
    /// Discovery triggers the doing emitted.
    pub triggers: Vec<String>,
    /// The skill practised and the real hours of practice.
    pub practice: Option<(String, f32)>,
    /// What it does to its target (when it worked).
    pub effect: Effect,
}

/// The kinds of a thing at a source on the bench.
fn kind_at<'a>(bench: &Bench<'a>, s: &Source) -> Option<&'a ItemKind> {
    bench
        .at_hand
        .iter()
        .find(|h| &h.source == s)
        .and_then(|h| bench.items.get(&h.stack.id))
}

fn keys_of_kind(kind: &ItemKind, c: &Content) -> Vec<String> {
    match c.items.get(&kind.id) {
        Some(def) => item_keys(def, c),
        None => vec![key(&kind.id).to_owned()],
    }
}

/// Every trigger doing this plan emits.
pub fn triggers_of(crafts: &Crafts, plan: &Plan, bench: &Bench) -> Vec<String> {
    let def = &crafts.recipes[plan.recipe].def;
    let c = bench.content;
    let mut out = vec![format!("do:{}", key(&def.id))];
    out.extend(def.teaches.iter().cloned());
    if let Some(verb) = &def.verb {
        let used = plan.uses.iter().map(|(s, _)| s).chain(&plan.keeps);
        for s in used {
            if let Some(k) = kind_at(bench, s) {
                out.extend(with_verb(verb, &keys_of_kind(k, c)));
                if let Some(m) = k.material.as_deref().and_then(|m| c.materials.get(m)) {
                    out.extend(with_verb(verb, &material_keys(m)));
                }
            }
        }
        match &bench.aimed {
            Some(Aimed::Block { name, material, .. }) => {
                let m = material.as_deref().and_then(|m| c.materials.get(m));
                out.extend(with_verb(verb, &block_keys(name, m)));
            }
            Some(Aimed::Thing(_)) => {
                if let Some(k) = bench.target_thing().and_then(|h| bench.kind(h)) {
                    out.extend(with_verb(verb, &keys_of_kind(k, c)));
                }
            }
            Some(Aimed::Water) => out.push(format!("{verb}:water")),
            Some(Aimed::Fire(_)) | Some(Aimed::Station { fire: Some(_), .. }) => {
                out.push(format!("{verb}:fire"))
            }
            Some(Aimed::Animal { species, .. }) => {
                out.push(format!("{verb}:animal"));
                out.push(format!("{verb}:{}", key(species)));
            }
            _ => {}
        }
    }
    for (s, _) in &plan.tools {
        if let Some(k) = kind_at(bench, s) {
            out.extend(with_verb("use", &keys_of_kind(k, c)));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The 0–1 quality of a material for the work: how well stone knaps for knapping, how
/// workable it is otherwise.
fn material_quality(c: &Content, material: Option<&str>, skill_name: Option<&str>) -> f32 {
    let Some(m) = material.and_then(|m| c.materials.get(m)) else {
        return 0.5;
    };
    if skill_name == Some("knapping") {
        m.knapping.unwrap_or(0.3)
    } else {
        m.workability.or(m.knapping).unwrap_or(0.5)
    }
}

/// The chance a failure strikes, at this skill and with this material and air.
fn failure_chance(
    f: &hearth_content::schema::process::Failure,
    def: &Process,
    skill: f32,
    matq: f32,
    humidity: f32,
    density: f32,
) -> f32 {
    let mut p = f.chance - (f.chance - f.min_chance) * skill.clamp(0.0, 1.0);
    match def.skill.as_deref() {
        // Coarse stone breaks unpredictably.
        Some("knapping") => p *= 1.3 - 0.5 * matq,
        // Damp air makes friction fire hard, and so does a dense drill (oak is poor; lime,
        // willow and other soft woods are good).
        Some("firemaking") => {
            p += 0.5 * (humidity - 0.6).max(0.0);
            if def.effect == Effect::Ignite {
                p += 0.3 * ((density - 550.0) / 300.0).clamp(0.0, 1.0);
            }
        }
        _ => {}
    }
    p.clamp(0.0, 0.98)
}

/// The output stacks of `out` from `material` in play.
/// Whether an output comes in a season (one that does not say comes always; an unknown season
/// is taken for summer).
fn in_season(out: &Output, season: Option<Season>) -> bool {
    out.seasons.is_empty() || out.seasons.contains(&season.unwrap_or(Season::Summer))
}

/// Makes an output: `left` of the kilograms of a material (what is left of a carcass).
#[allow(clippy::too_many_arguments)]
fn produce(
    crafts: &Crafts,
    out: &Output,
    material: Option<&str>,
    quality: f32,
    left: f32,
    c: &Content,
    items: &Items,
    rng: &mut Rng,
) -> Option<Stack> {
    if out.chance < 1.0 && !rng.chance(out.chance as f64) {
        return None;
    }
    let (lo, hi) = (
        out.amount.0.min(out.amount.1),
        out.amount.0.max(out.amount.1),
    );
    let id = match &out.item {
        Match::Item(r) => r.to_string(),
        Match::Material(m) => {
            let kind = crafts.bulk_item(items, m.as_str())?;
            let kg = rng.range_f32(lo, hi + 1e-6) * left.clamp(0.0, 1.0);
            // Whole units, rounded at random so the mass comes out right on average.
            let units = kg / kind.mass_kg.max(1e-4);
            let mut n = units.floor() as u16;
            if rng.next_f32() < units.fract() {
                n += 1;
            }
            if n == 0 {
                return None;
            }
            return Some(Stack::made(&kind.id, n, quality));
        }
        Match::Form { form, materials } => {
            let m = match materials {
                Some(f) if f.ids.len() == 1 => f.ids[0].to_string(),
                Some(f) => match material {
                    Some(m) if c.materials.get(m).is_some_and(|mat| f.matches(m, mat)) => {
                        m.to_owned()
                    }
                    _ => c.materials.iter().find(|m| f.matches(&m.id, m))?.id.clone(),
                },
                None => material?.to_owned(),
            };
            generated_id(form.as_str(), &m)
        }
        Match::Garment(g) => generated_id(g.as_str(), material?),
        Match::Tag(_) => return None,
    };
    items.get(&id)?;
    let n = rng.range_f32(lo, hi + 0.999).floor().max(1.0) as u16;
    Some(Stack::made(&id, n, quality))
}

/// Does the plan: rolls its failures and makes its outputs (for attended work, or when
/// unattended work is finished).
pub fn perform(crafts: &Crafts, plan: &Plan, bench: &Bench, skill: f32, rng: &mut Rng) -> Outcome {
    perform_by(crafts, plan, bench, skill, rng, None)
}

/// As [`perform`], with the player's own hands deciding (knapping by hand): `Some(q)` is the
/// quality they reached, which their making mostly takes; `Some(0.0)` is a piece that snapped.
pub fn perform_by(
    crafts: &Crafts,
    plan: &Plan,
    bench: &Bench,
    skill: f32,
    rng: &mut Rng,
    hand: Option<f32>,
) -> Outcome {
    let def = &crafts.recipes[plan.recipe].def;
    let c = bench.content;
    let matq = material_quality(c, plan.material.as_deref(), def.skill.as_deref());
    // A carcass gives what is left of it (a kill the scavengers have been at); what goes off
    // comes out as far gone as the worst of what went into it.
    let used: Vec<&Handy> = plan
        .uses
        .iter()
        .filter_map(|(s, _)| bench.at_hand.iter().find(|h| &h.source == s))
        .collect();
    let left = used
        .iter()
        .filter(|h| bench.kind(h).is_some_and(|k| k.has_tag("carcass")))
        .map(|h| h.stack.condition)
        .fold(1.0f32, f32::min);
    let gone = used
        .iter()
        .filter(|h| bench.kind(h).is_some_and(|k| keeps_days(c, k).is_some()))
        .map(|h| h.stack.decay)
        .fold(0.0f32, f32::max);
    let season = bench.around.season;
    let mut o = Outcome {
        triggers: triggers_of(crafts, plan, bench),
        practice: def.skill.clone().map(|s| (s, plan.hours)),
        ..Outcome::default()
    };
    for (s, _) in &plan.tools {
        let q = bench
            .at_hand
            .iter()
            .find(|h| &h.source == s)
            .map_or(0.5, |h| h.stack.quality);
        o.wear.push((s.clone(), def.wear * (1.2 - 0.4 * q)));
    }
    for f in &def.failures {
        let p = match hand {
            Some(q) if q <= 0.0 => {
                if f.loses_inputs {
                    1.0
                } else {
                    0.0
                }
            }
            Some(_) => 0.0,
            None => {
                let density = plan
                    .material
                    .as_deref()
                    .and_then(|m| c.materials.get(m))
                    .map_or(600.0, |m| m.density_kg_m3);
                failure_chance(f, def, skill, matq, bench.around.humidity, density)
            }
        };
        if rng.next_f32() < p {
            o.failure = Some(f.outcome.clone());
            o.injury = f.injury.clone();
            if f.loses_inputs {
                o.used = plan.uses.clone();
                // What breaks still leaves its waste.
                for b in def.byproducts.iter().filter(|b| in_season(b, season)) {
                    if let Some(s) = produce(
                        crafts,
                        b,
                        plan.material.as_deref(),
                        0.2,
                        left,
                        c,
                        bench.items,
                        rng,
                    ) {
                        o.made.push(s);
                    }
                }
                gone_off(&mut o.made, gone, c, bench.items);
            }
            return o;
        }
    }
    o.done = true;
    o.used = plan.uses.clone();
    o.effect = def.effect;
    for out in def.outputs.iter().filter(|o| in_season(o, season)) {
        let q = &out.quality;
        let rolled = if q.base == 0.0 && q.from_skill == 0.0 && q.from_material == 0.0 {
            0.3 + 0.4 * skill
        } else {
            q.base + q.from_skill * skill + q.from_material * matq
        } + 0.05 * rng.normal() as f32;
        let quality = match hand {
            Some(h) => 0.7 * h + 0.3 * rolled,
            None => rolled,
        };
        if let Some(s) = produce(
            crafts,
            out,
            plan.material.as_deref(),
            quality.clamp(0.05, 1.0),
            left,
            c,
            bench.items,
            rng,
        ) {
            o.made.push(s);
        }
    }
    for b in def.byproducts.iter().filter(|b| in_season(b, season)) {
        if let Some(s) = produce(
            crafts,
            b,
            plan.material.as_deref(),
            0.3,
            left,
            c,
            bench.items,
            rng,
        ) {
            o.made.push(s);
        }
    }
    gone_off(&mut o.made, gone, c, bench.items);
    o
}

/// What goes off among things made starts as far gone as `gone`.
fn gone_off(made: &mut [Stack], gone: f32, c: &Content, items: &Items) {
    if gone <= 0.0 {
        return;
    }
    for s in made {
        if items.get(&s.id).is_some_and(|k| keeps_days(c, k).is_some()) {
            s.decay = s.decay.max(gone);
        }
    }
}

/// Finishes unattended work on `stack` (meat on the rack, acorns in the stream): the outputs,
/// made of the stack's material, or nothing if it failed. Its failures grow with the share of
/// the time it spent wet. A firing's outputs take the quality its heat gave (`quality`).
#[allow(clippy::too_many_arguments)]
pub fn finish_batch(
    crafts: &Crafts,
    recipe: usize,
    stack: &Stack,
    wet_share: f32,
    quality: Option<f32>,
    c: &Content,
    items: &Items,
    rng: &mut Rng,
) -> Result<Vec<Stack>, String> {
    let def = &crafts.recipes[recipe].def;
    let material = items.get(&stack.id).and_then(|k| k.material.clone());
    for f in &def.failures {
        let p = (f.chance * (1.0 + 2.0 * wet_share.clamp(0.0, 1.0))).min(0.95);
        if rng.next_f32() < p {
            return Err(f.outcome.clone());
        }
    }
    // The batch holds `count` of what the inputs asked for in total: outputs scale with it.
    let asked: f32 = def
        .inputs
        .iter()
        .filter(|i| i.consumed)
        .map(|i| match &i.item {
            Match::Material(m) => crafts.units_for(items, m.as_str(), i.amount) as f32,
            _ => i.amount.max(1.0),
        })
        .sum::<f32>()
        .max(1.0);
    let batches = (stack.count as f32 / asked).max(1.0).round() as u32;
    let mut out = Vec::new();
    for _ in 0..batches {
        for o in def
            .outputs
            .iter()
            .chain(&def.byproducts)
            .filter(|o| in_season(o, None))
        {
            let q = quality.unwrap_or(0.5);
            if let Some(s) = produce(crafts, o, material.as_deref(), q, 1.0, c, items, rng) {
                out.push(s);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod altitude_tests {
    use super::boiling_slowdown;

    #[test]
    fn boiling_takes_longer_up_high() {
        assert_eq!(boiling_slowdown(0.0), 1.0);
        let at_3000 = boiling_slowdown(3000.0);
        assert!((1.8..2.2).contains(&at_3000), "{at_3000}");
        assert!(boiling_slowdown(5000.0) > 3.0);
    }
}

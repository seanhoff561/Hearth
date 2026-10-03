//! `hearth content lint`: cross-references, reachability of processes and knowledge, habitats,
//! food webs, unit sanity, and the "cost of each technology from scratch" rollup (v2 §3.2,
//! §12.5).

use std::collections::BTreeMap;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::content::{Content, Origin, Table};
use crate::diag::Report;
use crate::generate::ItemDef;
use crate::id::IdRef;
use crate::schema::fauna::Social;
use crate::schema::flora::Edibility;
use crate::schema::knowledge::{Knowledge, Need};
use crate::schema::material::MaterialCategory;
use crate::schema::process::{BlockMatch, Input, Match, Output, Process, Target};
use crate::schema::{Entry, Status};
use crate::time::TimeScales;

/// Things the lint needs from outside the content (e.g. the world generator's biome names).
#[derive(Debug, Clone, Default)]
pub struct LintContext {
    /// Valid biome names; `None` skips the biome check.
    pub biomes: Option<Vec<String>>,
    /// The world's blocks (name, material), for block targets and sight; `None` takes every
    /// block target and sight as possible.
    pub blocks: Option<Vec<(String, Option<String>)>>,
}

/// Results of the reachability analysis, useful beyond the lint (graphs, tests).
#[derive(Debug, Clone, Default)]
pub struct Reach {
    pub items: FxHashSet<String>,
    pub materials: FxHashSet<String>,
    pub stations: FxHashSet<String>,
    pub knowledge: FxHashSet<String>,
    pub processes: FxHashSet<String>,
    /// For each obtainable item/material, the process that first produced it (none = natural).
    pub producer: FxHashMap<String, String>,
    /// Fixpoint order of processes.
    pub order: Vec<String>,
    /// Discovery triggers something obtainable emits.
    pub triggers: FxHashSet<String>,
    /// The world's blocks and their materials, when known (what a process done to a block
    /// makes its outputs of).
    pub blocks: Vec<(String, Option<String>)>,
}

/// Rolled-up cost of reaching a knowledge node from nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct Effort {
    pub knowledge: String,
    pub era: u8,
    /// Distinct processes in the chain.
    pub steps: usize,
    /// Real-world hours of work across the chain (before time-scale compression).
    pub real_hours: f64,
    /// Minutes of play at the default calendar.
    pub play_minutes: f64,
    /// Natural materials gathered (kg, items counted by their mass).
    pub gathered_kg: f64,
    /// The node is implemented (planned eras are still partly authored).
    pub implemented: bool,
}

struct Refs<'a> {
    c: &'a Content,
    report: &'a mut Report,
}

impl Refs<'_> {
    fn check<T>(&mut self, table: &Table<T>, what: &str, r: &IdRef, origin: &Origin, from: &str) {
        if !table.contains(r) {
            self.report.error(
                "unknown-ref",
                Some(origin.file.clone()),
                origin.line,
                format!("`{from}` refers to unknown {what} `{r}`"),
            );
        }
    }

    fn material(&mut self, r: &IdRef, o: &Origin, from: &str) {
        let t = &self.c.materials;
        self.check(t, "material", r, o, from);
    }

    fn matcher(&mut self, m: &Match, o: &Origin, from: &str) {
        match m {
            Match::Item(r) => {
                let t = &self.c.items;
                self.check(t, "item", r, o, from);
            }
            Match::Form { form, materials } => {
                let t = &self.c.forms;
                self.check(t, "item form", form, o, from);
                if let Some(f) = materials {
                    for r in &f.ids {
                        self.material(r, o, from);
                    }
                }
            }
            Match::Material(r) => self.material(r, o, from),
            Match::Garment(g) => {
                let t = &self.c.garments;
                self.check(t, "garment", g, o, from);
            }
            Match::Tag(t) => {
                let known = self.c.items.iter().any(|i| i.tags.contains(t))
                    || self.c.materials.iter().any(|m| m.tags.contains(t));
                if !known {
                    self.report.warning(
                        "unknown-tag",
                        Some(o.file.clone()),
                        o.line,
                        format!("`{from}` matches tag `{t}` that no item or material has"),
                    );
                }
            }
        }
    }
}

fn refs(c: &Content, report: &mut Report, ctx: &LintContext) {
    let mut r = Refs { c, report };
    for (e, o) in c.rocks.iter_with_origin() {
        r.material(&e.material, o, e.id());
        for w in &e.weathers_to {
            r.material(w, o, e.id());
        }
    }
    for (e, o) in c.minerals.iter_with_origin() {
        r.material(&e.material, o, e.id());
        for y in &e.yields {
            r.material(&y.material, o, e.id());
        }
    }
    for (e, o) in c.provinces.iter_with_origin() {
        for l in &e.sequence {
            r.check(&c.rocks, "rock", &l.rock, o, e.id());
        }
        r.check(&c.rocks, "rock", &e.basement, o, e.id());
        for i in &e.intrusions {
            r.check(&c.rocks, "rock", i, o, e.id());
        }
    }
    for (e, o) in c.deposits.iter_with_origin() {
        if !(c.minerals.contains(&e.resource)
            || c.rocks.contains(&e.resource)
            || c.materials.contains(&e.resource))
        {
            r.report.error(
                "unknown-ref",
                Some(o.file.clone()),
                o.line,
                format!("`{}` refers to unknown resource `{}`", e.id(), e.resource),
            );
        }
        for p in &e.provinces {
            r.check(&c.provinces, "province", p, o, e.id());
        }
        for h in &e.host_rocks {
            r.check(&c.rocks, "rock", h, o, e.id());
        }
    }
    for (e, o) in c.soils.iter_with_origin() {
        for h in &e.horizons {
            r.material(&h.material, o, e.id());
        }
        for p in &e.formation.parent_rocks {
            r.check(&c.rocks, "rock", p, o, e.id());
        }
    }
    for (e, o) in c.plants.iter_with_origin() {
        for s in &e.soil.soils {
            r.check(&c.soils, "soil", s, o, e.id());
        }
        if let Some(w) = &e.wood {
            r.material(w, o, e.id());
        }
        for p in &e.parts {
            r.material(&p.material, o, e.id());
            if let Edibility::AfterProcess(pr) = &p.edibility {
                r.check(&c.processes, "process", pr, o, e.id());
            }
        }
        if let Some(a) = &e.domesticated_from {
            r.check(&c.plants, "plant", a, o, e.id());
        }
    }
    for (e, o) in c.animals.iter_with_origin() {
        for f in &e.diet.foods {
            if !(c.plants.contains(&f.food)
                || c.animals.contains(&f.food)
                || c.materials.contains(&f.food))
            {
                r.report.error(
                    "unknown-ref",
                    Some(o.file.clone()),
                    o.line,
                    format!("`{}` eats unknown food `{}`", e.id(), f.food),
                );
            }
        }
        if let Some(y) = &e.yields {
            for m in y.hide_material.iter().chain(&y.meat_material) {
                r.material(m, o, e.id());
            }
            for x in &y.extras {
                r.material(&x.material, o, e.id());
            }
        }
        for h in e.habitat.iter().chain(&e.also_in) {
            r.check(&c.ecosystems, "ecosystem", h, o, e.id());
        }
        if e.habitat.is_empty() {
            r.report.error(
                "no-habitat",
                Some(o.file.clone()),
                o.line,
                format!("`{}` has no habitat", e.id()),
            );
        }
    }
    for (e, o) in c.ecosystems.iter_with_origin() {
        for p in &e.producers {
            r.check(&c.plants, "plant", p, o, e.id());
        }
        for a in &e.consumers {
            r.check(&c.animals, "animal", a, o, e.id());
        }
        for s in &e.succession {
            for p in &s.plants {
                r.check(&c.plants, "plant", p, o, e.id());
            }
        }
        if let Some(biomes) = &ctx.biomes {
            for b in &e.biomes {
                if !biomes.contains(b) {
                    r.report.error(
                        "unknown-biome",
                        Some(o.file.clone()),
                        o.line,
                        format!(
                            "`{}` lists biome `{b}` that the world generator never produces",
                            e.id()
                        ),
                    );
                }
            }
        }
        if e.biomes.is_empty() {
            r.report.error(
                "no-biome",
                Some(o.file.clone()),
                o.line,
                format!("ecosystem `{}` occurs in no biome", e.id()),
            );
        }
    }
    for (e, o) in c.hominins.iter_with_origin() {
        for h in &e.habitat {
            r.check(&c.ecosystems, "ecosystem", h, o, e.id());
        }
        for k in &e.knowledge {
            r.check(&c.knowledge, "knowledge", k, o, e.id());
        }
    }
    for (e, o) in c.forms.iter_with_origin() {
        for m in &e.materials.ids {
            r.material(m, o, e.id());
        }
    }
    for (e, o) in c.explicit_items.iter_with_origin() {
        if let Some(m) = &e.material {
            r.material(m, o, e.id());
        }
    }
    for (e, o) in c.processes.iter_with_origin() {
        for i in &e.inputs {
            r.matcher(&i.item, o, e.id());
        }
        for out in e.outputs.iter().chain(&e.byproducts) {
            r.matcher(&out.item, o, e.id());
        }
        if let Some(s) = &e.station {
            r.check(&c.workstations, "workstation", s, o, e.id());
        }
        if let Some(k) = &e.knowledge {
            r.check(&c.knowledge, "knowledge", k, o, e.id());
        }
        for t in &e.tools {
            let provided = c.items.iter().any(|i| i.property(&t.property).is_some());
            if !provided {
                r.report.error(
                    "unknown-property",
                    Some(o.file.clone()),
                    o.line,
                    format!(
                        "`{}` needs tool property `{}` that no item has",
                        e.id(),
                        t.property
                    ),
                );
            }
        }
        // Experiments teach, and some processes act on their target (dig, light, feed, mend)
        // rather than make things.
        let acts = e.effect != crate::schema::process::Effect::Keep
            || e.verb.is_some()
            || !e.teaches.is_empty();
        if e.outputs.is_empty() && !acts {
            r.report.error(
                "no-output",
                Some(o.file.clone()),
                o.line,
                format!("process `{}` produces nothing", e.id()),
            );
        }
    }
    for (e, o) in c.knowledge.iter_with_origin() {
        for q in &e.requires {
            r.check(&c.knowledge, "knowledge", q, o, e.id());
        }
        for n in &e.needs {
            match n {
                Need::Material(m) => r.material(m, o, e.id()),
                Need::Item(i) => r.check(&c.items, "item", i, o, e.id()),
                Need::Station(s) => r.check(&c.workstations, "workstation", s, o, e.id()),
                Need::Environment(_) => {}
            }
        }
        for p in &e.enables {
            r.check(&c.processes, "process", p, o, e.id());
        }
    }
    for (e, o) in c.workstations.iter_with_origin() {
        for p in &e.parts {
            r.matcher(&p.item, o, e.id());
        }
        if let Some(k) = &e.knowledge {
            r.check(&c.knowledge, "knowledge", k, o, e.id());
        }
    }
    for (e, o) in c.construction.iter_with_origin() {
        if let Some(k) = &e.knowledge {
            r.check(&c.knowledge, "knowledge", k, o, e.id());
        }
    }
    for (e, o) in c.garments.iter_with_origin() {
        if let Some(k) = &e.knowledge {
            r.check(&c.knowledge, "knowledge", k, o, e.id());
        }
    }
    for (e, o) in c.eras.iter_with_origin() {
        for h in &e.hominins {
            r.check(&c.hominins, "hominin", h, o, e.id());
        }
        for k in &e.knowledge_baseline {
            r.check(&c.knowledge, "knowledge", k, o, e.id());
        }
    }
    for (p, o) in c.balance_presets.iter_with_origin() {
        for (key, v) in &p.values {
            let full = if key.contains(':') {
                key.clone()
            } else {
                format!("hearth:{key}")
            };
            match c.balance_keys.get(&full) {
                Some(k) if (k.min..=k.max).contains(v) => {}
                Some(k) => r.report.error(
                    "unit-range",
                    Some(o.file.clone()),
                    o.line,
                    format!(
                        "preset `{}` sets {key} = {v} outside {}..={}",
                        p.id(),
                        k.min,
                        k.max
                    ),
                ),
                None => r.report.error(
                    "unknown-ref",
                    Some(o.file.clone()),
                    o.line,
                    format!("preset `{}` sets unknown balance key `{key}`", p.id()),
                ),
            }
        }
    }
    let available: Vec<&str> = c
        .eras
        .iter()
        .filter(|e| e.available)
        .map(|e| e.id())
        .collect();
    if available.is_empty() && !c.eras.is_empty() {
        r.report
            .error("no-era", None, None, "no era is available to play");
    }
}

/// Knowledge graph structure: cycles, era order, implemented-on-planned dependencies.
fn knowledge_graph(c: &Content, report: &mut Report) {
    // Cycle detection by DFS colouring.
    let n = c.knowledge.len();
    let mut state = vec![0u8; n];
    fn visit(c: &Content, i: usize, state: &mut [u8], stack: &mut Vec<usize>, report: &mut Report) {
        state[i] = 1;
        stack.push(i);
        for r in &c.knowledge.at(i).requires {
            if let Some(j) = c.knowledge.index_of(r.as_str()) {
                if state[j] == 1 {
                    let o = c.knowledge.origin(i);
                    let cycle: Vec<&str> = stack.iter().map(|&k| c.knowledge.at(k).id()).collect();
                    report.error(
                        "cycle",
                        Some(o.file.clone()),
                        o.line,
                        format!(
                            "knowledge cycle: {} -> {}",
                            cycle.join(" -> "),
                            c.knowledge.at(j).id()
                        ),
                    );
                } else if state[j] == 0 {
                    visit(c, j, state, stack, report);
                }
            }
        }
        stack.pop();
        state[i] = 2;
    }
    for i in 0..n {
        if state[i] == 0 {
            visit(c, i, &mut state, &mut Vec::new(), report);
        }
    }
    for (k, o) in c.knowledge.iter_with_origin() {
        for r in &k.requires {
            let Some(p) = c.knowledge.get(r.as_str()) else {
                continue;
            };
            if p.era > k.era {
                report.warning(
                    "era-order",
                    Some(o.file.clone()),
                    o.line,
                    format!(
                        "`{}` (era {}) requires `{}` from a later era ({})",
                        k.id(),
                        k.era,
                        p.id(),
                        p.era
                    ),
                );
            }
            if k.status == Status::Implemented && p.status == Status::Planned {
                report.error(
                    "planned-dependency",
                    Some(o.file.clone()),
                    o.line,
                    format!("implemented `{}` requires planned `{}`", k.id(), p.id()),
                );
            }
        }
        if k.status == Status::Implemented && k.discovery.is_empty() {
            report.error(
                "no-discovery",
                Some(o.file.clone()),
                o.line,
                format!("implemented `{}` has no discovery route", k.id()),
            );
        }
    }
}

fn natural_materials(c: &Content) -> FxHashSet<String> {
    let mut out = FxHashSet::default();
    let mut rock_ids: FxHashSet<&str> = FxHashSet::default();
    for p in c.provinces.iter() {
        rock_ids.extend(p.sequence.iter().map(|l| l.rock.as_str()));
        rock_ids.insert(p.basement.as_str());
        rock_ids.extend(p.intrusions.iter().map(|r| r.as_str()));
    }
    for d in c.deposits.iter() {
        rock_ids.insert(d.resource.as_str());
        rock_ids.extend(d.host_rocks.iter().map(|r| r.as_str()));
        if let Some(m) = c.minerals.get(d.resource.as_str()) {
            out.insert(m.material.to_string());
        }
        if c.materials.contains(&d.resource) {
            out.insert(d.resource.to_string());
        }
    }
    for id in rock_ids {
        if let Some(r) = c.rocks.get(id) {
            out.insert(r.material.to_string());
            out.extend(r.weathers_to.iter().map(|w| w.to_string()));
        }
    }
    for s in c.soils.iter() {
        out.extend(s.horizons.iter().map(|h| h.material.to_string()));
    }
    for p in c.plants.iter() {
        if let Some(w) = &p.wood {
            out.insert(w.to_string());
        }
        out.extend(p.parts.iter().map(|x| x.material.to_string()));
    }
    let any_animals = !c.animals.is_empty();
    for m in c.materials.iter() {
        let natural = m.tags.iter().any(|t| t == "natural")
            || matches!(
                m.category,
                MaterialCategory::Water | MaterialCategory::Ice | MaterialCategory::Snow
            )
            || (any_animals
                && matches!(
                    m.category,
                    MaterialCategory::AnimalTissue
                        | MaterialCategory::Bone
                        | MaterialCategory::AnimalFibre
                ));
        if natural {
            out.insert(m.id().to_owned());
        }
    }
    out
}

fn item_matches(m: &Match, item: &ItemDef, c: &Content) -> bool {
    crate::triggers::item_matches(m, item, c)
}

fn satisfied(m: &Match, reach: &Reach, c: &Content) -> bool {
    match m {
        Match::Material(r) => reach.materials.contains(r.as_str()),
        Match::Tag(t) => {
            reach
                .items
                .iter()
                .any(|i| c.items.get(i).is_some_and(|d| d.tags.contains(t)))
                || reach
                    .materials
                    .iter()
                    .any(|mid| c.materials.get(mid).is_some_and(|m| m.tags.contains(t)))
        }
        _ => reach
            .items
            .iter()
            .any(|i| c.items.get(i).is_some_and(|d| item_matches(m, d, c))),
    }
}

fn inputs_ok(inputs: &[Input], reach: &Reach, c: &Content) -> bool {
    inputs.iter().all(|i| satisfied(&i.item, reach, c))
}

/// The materials a process's outputs may be made of when they do not say: those of the
/// obtainable things matching its first input, or of the blocks its target accepts.
fn materials_in_play(p: &Process, reach: &Reach, c: &Content) -> Vec<String> {
    let mut mats = Vec::new();
    if let Some(first) = p.inputs.first() {
        for id in reach.items.iter() {
            if let Some(d) = c.items.get(id)
                && item_matches(&first.item, d, c)
                && let Some(m) = &d.material
            {
                mats.push(m.clone());
            }
        }
        if let Match::Material(m) = &first.item {
            mats.push(m.to_string());
        }
    } else if let Some(Target::Block(BlockMatch::Material(f))) = &p.target {
        mats.extend(
            c.materials
                .iter()
                .filter(|m| f.matches(m.id(), m))
                .map(|m| m.id().to_owned()),
        );
    } else if let Some(Target::Block(b)) = &p.target {
        // Done to blocks by name (a log of any tree): the materials of the world's blocks it
        // accepts.
        for (name, mat) in &reach.blocks {
            if let Some(m) = mat
                && crate::triggers::block_matches(b, name, c.materials.get(m))
                && !mats.contains(m)
            {
                mats.push(m.clone());
            }
        }
    }
    mats
}

/// Adds what `out` produces; `Form` outputs without a material filter inherit the materials
/// of the obtainable items matching the first input (a flake is struck from a core of the
/// same stone).
fn produce(out: &Output, p: &Process, reach: &mut Reach, c: &Content, new: &mut Vec<String>) {
    let mut add = |id: String, reach: &mut Reach| {
        if reach.items.insert(id.clone()) {
            reach.producer.insert(id.clone(), p.id.clone());
            new.push(id);
        }
    };
    match &out.item {
        Match::Item(r) => add(r.to_string(), reach),
        Match::Material(r) => {
            if reach.materials.insert(r.to_string()) {
                reach
                    .producer
                    .entry(r.to_string())
                    .or_insert_with(|| p.id.clone());
            }
        }
        Match::Form { form, materials } => {
            let mut mats = match materials {
                Some(f) => c
                    .materials
                    .iter()
                    .filter(|m| f.matches(m.id(), m))
                    .map(|m| m.id().to_owned())
                    .collect(),
                None => materials_in_play(p, reach, c),
            };
            // Done to a block by name with the world's blocks unknown: whatever the form may
            // be made of.
            if mats.is_empty()
                && reach.blocks.is_empty()
                && matches!(p.target, Some(Target::Block(_)))
                && let Some(f) = c.forms.get(form.as_str())
            {
                mats = c
                    .materials
                    .iter()
                    .filter(|m| f.materials.matches(m.id(), m))
                    .map(|m| m.id().to_owned())
                    .collect();
            }
            for m in mats {
                let id = crate::generate::generated_id(form.as_str(), &m);
                if c.items.get(&id).is_some() {
                    add(id, reach);
                }
            }
        }
        Match::Garment(g) => {
            for m in materials_in_play(p, reach, c) {
                let id = crate::generate::generated_id(g.as_str(), &m);
                if c.items.get(&id).is_some() {
                    add(id, reach);
                }
            }
        }
        Match::Tag(_) => {}
    }
}

/// Fixpoint of what can be obtained from nothing: natural materials and gatherable items,
/// then everything executable processes produce, stations that can be built and knowledge
/// that can be reached.
pub fn reachability(c: &Content) -> Reach {
    reachability_in(c, None)
}

/// Whether a discovery route's trigger is emitted by something obtainable.
fn heard(
    k: &Knowledge,
    route: &crate::schema::knowledge::Discovery,
    reach: &Reach,
    blocks: Option<&[(String, Option<String>)]>,
) -> bool {
    use crate::schema::knowledge::Route;
    let t = route.trigger.as_str();
    if route.route == Route::Inference {
        // Inferred from known prerequisites: the trigger names the node itself.
        return !k.requires.is_empty() && t == format!("infer:{}", crate::triggers::key(k.id()));
    }
    if reach.triggers.contains(t) {
        return true;
    }
    // Sight of a world's block when the blocks are not known.
    blocks.is_none() && t.starts_with("see:")
}

/// [`reachability`] in a world with these blocks (none: any block target and sight count).
pub fn reachability_in(c: &Content, blocks: Option<&[(String, Option<String>)]>) -> Reach {
    let mut reach = Reach {
        materials: natural_materials(c),
        blocks: blocks.map(<[_]>::to_vec).unwrap_or_default(),
        ..Reach::default()
    };
    reach
        .triggers
        .extend(crate::triggers::SYSTEM.iter().map(|s| (*s).to_owned()));
    // Sight of the world's blocks.
    for (name, mat) in blocks.unwrap_or(&[]) {
        let material = mat.as_deref().and_then(|m| c.materials.get(m));
        reach.triggers.extend(crate::triggers::with_verb(
            "see",
            &crate::triggers::block_keys(name, material),
        ));
    }
    for it in c.items.iter() {
        let gatherable_form = it
            .form
            .as_deref()
            .and_then(|f| c.forms.get(f))
            .is_some_and(|f| f.tags.iter().any(|t| t == "gatherable"));
        let natural_material = it
            .material
            .as_deref()
            .is_some_and(|m| reach.materials.contains(m));
        if it.tags.iter().any(|t| t == "natural") || (gatherable_form && natural_material) {
            reach.items.insert(it.id.clone());
        }
    }
    loop {
        let mut changed = false;
        // A material that can be had can be had by the handful, the cut, the lump.
        for it in c.items.iter() {
            if it.tags.iter().any(|t| t == "bulk")
                && it
                    .material
                    .as_deref()
                    .is_some_and(|m| reach.materials.contains(m))
                && reach.items.insert(it.id.clone())
            {
                changed = true;
            }
        }
        // Whatever can be had can be seen lying about, and thrown.
        let mut seen = Vec::new();
        for id in reach.items.iter() {
            if let Some(it) = c.items.get(id) {
                let keys = crate::triggers::item_keys(it, c);
                seen.extend(crate::triggers::with_verb("see", &keys));
                seen.extend(crate::triggers::with_verb("throw", &keys));
            }
        }
        reach.triggers.extend(seen);
        for w in c.workstations.iter() {
            let known = w
                .knowledge
                .as_ref()
                .is_none_or(|k| reach.knowledge.contains(k.as_str()));
            if !reach.stations.contains(w.id()) && known && inputs_ok(&w.parts, &reach, c) {
                reach.stations.insert(w.id().to_owned());
                reach
                    .triggers
                    .insert(format!("do:build_{}", crate::triggers::key(w.id())));
                changed = true;
            }
        }
        for k in c.knowledge.iter() {
            if reach.knowledge.contains(k.id()) {
                continue;
            }
            let prereqs = k
                .requires
                .iter()
                .all(|r| reach.knowledge.contains(r.as_str()));
            let needs = k.needs.iter().all(|n| match n {
                Need::Material(m) => reach.materials.contains(m.as_str()),
                Need::Item(i) => reach.items.contains(i.as_str()),
                Need::Station(s) => reach.stations.contains(s.as_str()),
                Need::Environment(_) => true,
            });
            // Planned nodes are checked for their prerequisites only: the systems that will emit
            // their triggers may not exist yet.
            let discovered = k.status == Status::Planned
                || k.discovery.iter().any(|r| heard(k, r, &reach, blocks));
            if prereqs && needs && discovered {
                reach.knowledge.insert(k.id().to_owned());
                changed = true;
            }
        }
        for p in c.processes.iter() {
            if reach.processes.contains(p.id()) {
                continue;
            }
            let known = p
                .knowledge
                .as_ref()
                .is_none_or(|k| reach.knowledge.contains(k.as_str()));
            let station = p
                .station
                .as_ref()
                .is_none_or(|s| reach.stations.contains(s.as_str()));
            let tools = p.tools.iter().all(|t| {
                reach.items.iter().any(|i| {
                    c.items
                        .get(i)
                        .and_then(|d| d.property(&t.property))
                        .is_some_and(|v| v >= t.min)
                })
            });
            let target = match &p.target {
                Some(Target::Thing(m)) => satisfied(m, &reach, c),
                Some(Target::Block(b)) => blocks.is_none_or(|list| {
                    list.iter().any(|(name, mat)| {
                        let m = mat.as_deref().and_then(|m| c.materials.get(m));
                        crate::triggers::block_matches(b, name, m)
                    })
                }),
                _ => true,
            };
            if known && station && tools && target && inputs_ok(&p.inputs, &reach, c) {
                reach.processes.insert(p.id().to_owned());
                reach.order.push(p.id().to_owned());
                let mut new = Vec::new();
                for out in p.outputs.iter().chain(&p.byproducts) {
                    produce(out, p, &mut reach, c, &mut new);
                }
                changed = true;
            }
        }
        // What doing the processes that can be done teaches, with everything that can be
        // had now (things become obtainable after the processes that use them).
        let mut heard_now = FxHashSet::default();
        {
            let items = &reach.items;
            let materials = &reach.materials;
            let usable = crate::triggers::Usable {
                item: &|id| items.contains(id),
                material: &|id| materials.contains(id),
                blocks,
            };
            for p in c
                .processes
                .iter()
                .filter(|p| reach.processes.contains(p.id()))
            {
                crate::triggers::process_triggers(p, c, &usable, &mut heard_now);
            }
        }
        let before = reach.triggers.len();
        reach.triggers.extend(heard_now);
        if reach.triggers.len() != before {
            changed = true;
        }
        if !changed {
            break;
        }
    }
    reach
}

fn reachability_report(c: &Content, reach: &Reach, report: &mut Report) {
    for (p, o) in c.processes.iter_with_origin() {
        if !reach.processes.contains(p.id()) {
            let (sev_error, what) = (p.status == Status::Implemented, "process");
            let msg = format!(
                "{what} `{}` can never be performed from a fresh start",
                p.id()
            );
            if sev_error {
                report.error("unreachable", Some(o.file.clone()), o.line, msg);
            } else {
                report.warning("unreachable", Some(o.file.clone()), o.line, msg);
            }
        }
    }
    for (k, o) in c.knowledge.iter_with_origin() {
        if !reach.knowledge.contains(k.id()) {
            let msg = format!(
                "knowledge `{}` can never be reached (missing prerequisites or needs)",
                k.id()
            );
            if k.status == Status::Implemented {
                report.error("unreachable", Some(o.file.clone()), o.line, msg);
            } else {
                report.warning("unreachable", Some(o.file.clone()), o.line, msg);
            }
        }
    }
}

/// Food webs: every ecosystem has producers, and every animal finds food where it lives.
fn food_webs(c: &Content, report: &mut Report) {
    for (e, o) in c.ecosystems.iter_with_origin() {
        if e.producers.is_empty() {
            report.error(
                "no-producers",
                Some(o.file.clone()),
                o.line,
                format!("ecosystem `{}` has no producers", e.id()),
            );
        }
    }
    let lives_in = |a: &crate::schema::fauna::Animal, h: &crate::IdRef| {
        a.habitat.contains(h) || a.also_in.contains(h)
    };
    for (a, o) in c.animals.iter_with_origin() {
        for h in a.habitat.iter().chain(&a.also_in) {
            let Some(eco) = c.ecosystems.get(h.as_str()) else {
                continue;
            };
            let edible_materials: FxHashSet<String> = eco
                .producers
                .iter()
                .filter_map(|p| c.plants.get(p.as_str()))
                .flat_map(|p| p.parts.iter().map(|x| x.material.to_string()))
                .collect();
            // Carrion is wherever other animals live.
            let carrion = c.animals.iter().any(|x| x.id() != a.id() && lives_in(x, h));
            let has_food = a.diet.foods.iter().any(|f| {
                eco.producers.contains(&f.food)
                    || c.animals
                        .get(f.food.as_str())
                        .is_some_and(|prey| lives_in(prey, h))
                    || edible_materials.contains(f.food.as_str())
                    || c.materials.get(f.food.as_str()).is_some_and(|m| {
                        m.tags.iter().any(|t| t == "natural")
                            || (carrion && m.tags.iter().any(|t| t == "meat"))
                    })
            });
            if !has_food {
                report.warning(
                    "no-food",
                    Some(o.file.clone()),
                    o.line,
                    format!("`{}` finds nothing it eats in `{}`", a.id(), eco.id()),
                );
            }
        }
        if let Social::Herd { size }
        | Social::Pack { size }
        | Social::Pride { size }
        | Social::Flock { size }
        | Social::School { size }
        | Social::Colony { size } = a.social
            && (size.0 < 1.0 || size.0 > size.1)
        {
            report.error(
                "unit-range",
                Some(o.file.clone()),
                o.line,
                format!("`{}` has group size range ({}, {})", a.id(), size.0, size.1),
            );
        }
    }
}

/// Cost of every reachable knowledge node from scratch, using the first producer found for
/// each non-natural input.
pub fn effort(c: &Content, reach: &Reach) -> Vec<Effort> {
    let scales = TimeScales::defaults(&c.time);
    let mut memo: FxHashMap<String, FxHashSet<String>> = FxHashMap::default();
    fn process_closure(
        c: &Content,
        reach: &Reach,
        pid: &str,
        memo: &mut FxHashMap<String, FxHashSet<String>>,
        depth: usize,
    ) -> FxHashSet<String> {
        if let Some(s) = memo.get(pid) {
            return s.clone();
        }
        let mut set = FxHashSet::default();
        set.insert(pid.to_owned());
        if depth > 64 {
            return set;
        }
        memo.insert(pid.to_owned(), set.clone());
        if let Some(p) = c.processes.get(pid) {
            let mut deps: Vec<String> = Vec::new();
            for i in &p.inputs {
                if let Some(prod) = producer_of(&i.item, reach, c) {
                    deps.push(prod);
                }
            }
            if let Some(s) = &p.station
                && let Some(w) = c.workstations.get(s.as_str())
            {
                for part in &w.parts {
                    if let Some(prod) = producer_of(&part.item, reach, c) {
                        deps.push(prod);
                    }
                }
            }
            for d in deps {
                if d != pid {
                    set.extend(process_closure(c, reach, &d, memo, depth + 1));
                }
            }
        }
        memo.insert(pid.to_owned(), set.clone());
        set
    }
    fn producer_of(m: &Match, reach: &Reach, c: &Content) -> Option<String> {
        let direct = |id: &str| reach.producer.get(id).cloned();
        match m {
            Match::Item(r) | Match::Material(r) => direct(r.as_str()),
            _ => {
                // Natural sources need no producer; otherwise the first produced match.
                let natural = reach.items.iter().any(|i| {
                    c.items.get(i).is_some_and(|d| item_matches(m, d, c))
                        && !reach.producer.contains_key(i)
                });
                if natural {
                    return None;
                }
                reach
                    .items
                    .iter()
                    .filter(|i| c.items.get(i).is_some_and(|d| item_matches(m, d, c)))
                    .find_map(|i| direct(i))
            }
        }
    }
    let mut knowledge_sets: FxHashMap<String, FxHashSet<String>> = FxHashMap::default();
    // Knowledge in era order so prerequisites are computed first.
    let mut nodes: Vec<&Knowledge> = c
        .knowledge
        .iter()
        .filter(|k| reach.knowledge.contains(k.id()))
        .collect();
    nodes.sort_by(|a, b| {
        a.era
            .cmp(&b.era)
            .then(b.history.years_bp.total_cmp(&a.history.years_bp))
    });
    let mut out = Vec::new();
    for k in nodes {
        let mut set: FxHashSet<String> = FxHashSet::default();
        for r in &k.requires {
            if let Some(s) = knowledge_sets.get(r.as_str()) {
                set.extend(s.iter().cloned());
            }
        }
        for p in &k.enables {
            if reach.processes.contains(p.as_str()) {
                set.extend(process_closure(c, reach, p.as_str(), &mut memo, 0));
            }
        }
        let mut real_hours = 0.0;
        let mut play_s = 0.0;
        let mut gathered = 0.0;
        for pid in &set {
            if let Some(p) = c.processes.get(pid) {
                real_hours += p.duration.hours as f64;
                play_s += scales.play_seconds(p.duration.hours as f64, p.duration.scale);
                for i in &p.inputs {
                    if !i.consumed || producer_of(&i.item, reach, c).is_some() {
                        continue;
                    }
                    gathered += match &i.item {
                        Match::Material(_) => i.amount as f64,
                        other => {
                            let mass = reach
                                .items
                                .iter()
                                .filter_map(|id| c.items.get(id))
                                .find(|d| item_matches(other, d, c))
                                .map_or(0.0, |d| d.mass_kg as f64);
                            mass * i.amount as f64
                        }
                    };
                }
            }
        }
        out.push(Effort {
            knowledge: k.id().to_owned(),
            era: k.era,
            steps: set.len(),
            real_hours,
            play_minutes: play_s / 60.0,
            gathered_kg: gathered,
            implemented: k.status == Status::Implemented,
        });
        knowledge_sets.insert(k.id().to_owned(), set);
    }
    out
}

fn effort_report(efforts: &[Effort], report: &mut Report) {
    let mut by_era: BTreeMap<u8, Vec<&Effort>> = BTreeMap::new();
    for e in efforts {
        by_era.entry(e.era).or_default().push(e);
    }
    let mut last_mean = -1.0;
    for (era, list) in &by_era {
        // Eras whose nodes are all planned have few processes yet; they are reported, not
        // ordered.
        let planned = list.iter().all(|e| !e.implemented);
        let n = list.len() as f64;
        let steps = list.iter().map(|e| e.steps as f64).sum::<f64>() / n;
        let hours = list.iter().map(|e| e.real_hours).sum::<f64>() / n;
        let play = list.iter().map(|e| e.play_minutes).sum::<f64>() / n;
        let kg = list.iter().map(|e| e.gathered_kg).sum::<f64>() / n;
        report.info(
            "effort",
            format!(
                "era {era}{}: {} reachable nodes, mean {steps:.1} steps, {hours:.1} h of real work \
                 ({play:.1} min of play), {kg:.1} kg gathered from scratch",
                if planned { " (planned)" } else { "" },
                list.len()
            ),
        );
        if planned {
            continue;
        }
        // Ordered by the nodes made so far: the planned ones of a part-made era have few
        // processes yet and would make it look cheap.
        let made: Vec<&&Effort> = list.iter().filter(|e| e.implemented).collect();
        let steps = made.iter().map(|e| e.steps as f64).sum::<f64>() / made.len().max(1) as f64;
        if steps < last_mean {
            report.warning(
                "effort-order",
                None,
                None,
                format!("era {era} is cheaper from scratch ({steps:.1} steps) than the era before ({last_mean:.1})"),
            );
        }
        last_mean = steps;
    }
}

/// Runs every check.
/// Every tree's growth form names blocks the world has.
fn tree_blocks(c: &Content, blocks: &[(String, Option<String>)], report: &mut Report) {
    let known = |id: &str| {
        let full = if id.contains(':') {
            id.to_owned()
        } else {
            format!("hearth:{id}")
        };
        blocks.iter().any(|(b, _)| *b == full)
    };
    for (p, o) in c.plants.iter_with_origin() {
        if let Some(u) = &p.understory
            && !known(u.block.as_str())
        {
            report.error(
                "unknown-ref",
                Some(o.file.clone()),
                o.line,
                format!("`{}` grows as unknown block `{}`", p.id(), u.block),
            );
        }
        // Look-alikes are told apart by a knowledge of their group.
        if let Some(g) = &p.look_alike
            && c.knowledge.get(&format!("hearth:telling_{g}")).is_none()
        {
            report.error(
                "unknown-ref",
                Some(o.file.clone()),
                o.line,
                format!(
                    "`{}` looks like the other {g} plants, but no `telling_{g}` tells them apart",
                    p.id()
                ),
            );
        }
        let Some(t) = &p.tree else {
            continue;
        };
        for (what, id) in [
            ("log", &t.log),
            ("branch", &t.branch),
            ("leaves", &t.leaves),
        ] {
            if !known(id.as_str()) {
                report.error(
                    "unknown-ref",
                    Some(o.file.clone()),
                    o.line,
                    format!("`{}` grows its {what} as unknown block `{id}`", p.id()),
                );
            }
        }
    }
}

pub fn lint(c: &Content, ctx: &LintContext) -> Report {
    let mut report = Report::default();
    crate::validate::validate(c, &mut report);
    refs(c, &mut report, ctx);
    knowledge_graph(c, &mut report);
    if let Some(blocks) = &ctx.blocks {
        tree_blocks(c, blocks, &mut report);
    }
    let reach = reachability_in(c, ctx.blocks.as_deref());
    reachability_report(c, &reach, &mut report);
    food_webs(c, &mut report);
    effort_report(&effort(c, &reach), &mut report);
    let uncertain = c.uncertain_entries();
    report.info(
        "uncertain",
        format!(
            "{} entries are marked uncertain (review list: `hearth content uncertain`)",
            uncertain.len()
        ),
    );
    report
}

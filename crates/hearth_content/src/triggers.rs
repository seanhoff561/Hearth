//! Discovery triggers (v2 §12.3): the events that doing and seeing things emit, which the
//! knowledge graph's discovery routes listen for.
//!
//! A trigger is `verb:key`:
//! * A process with a `verb` emits it for every key of each thing it uses and of its target. A
//!   thing's keys are its form, its material, its garment and the tags of all of them (a flint
//!   cobble answers to `cobble`, `flint`, `knappable`, `stone`...). A block's keys are its own
//!   name and its material's keys.
//! * Every process also emits `do:<its id>`, what it `teaches`, and `use:<key>` for each tool in
//!   hand.
//! * Looking at something emits `see:<key>`; throwing a thing emits `throw:<key>`.
//! * Knowing every prerequisite of a node with an inference route lets it be inferred
//!   (`infer:<node>`).
//! * A few come from the game's systems ([`SYSTEM`]).
//!
//! Keys are ids without their namespace.

use rustc_hash::FxHashSet;

use crate::content::Content;
use crate::generate::ItemDef;
use crate::schema::material::Material;
use crate::schema::process::{BlockMatch, Match, Process, Target};

/// Triggers the game's systems emit, besides processes, sight and throwing.
pub const SYSTEM: &[&str] = &[
    // Reaching for a thing with both hands full.
    CARRY_FULL_HANDS,
    // A natural fire within sight.
    SEE_WILDFIRE,
    // Sleeping lying on a hide or fur.
    SLEEP_ON_HIDE,
    // Wood put down in water floats.
    FLOAT_WOOD,
    // Fresh prints of an animal underfoot.
    SEE_TRACKS,
];

pub const CARRY_FULL_HANDS: &str = "carry:full_hands";
pub const SEE_WILDFIRE: &str = "see:wildfire";
pub const SLEEP_ON_HIDE: &str = "sleep:on_hide";
pub const FLOAT_WOOD: &str = "float:wood";
pub const SEE_TRACKS: &str = "see:tracks";

/// The part of an id after its namespace.
pub fn key(id: &str) -> &str {
    id.split_once(':').map_or(id, |(_, p)| p)
}

/// A material's keys: its id and its tags.
pub fn material_keys(m: &Material) -> Vec<String> {
    let mut out = vec![key(&m.id).to_owned()];
    out.extend(m.tags.iter().cloned());
    out
}

/// A thing's keys: its form (or, for explicit items, its own id), material and garment, and
/// their tags.
pub fn item_keys(item: &ItemDef, c: &Content) -> Vec<String> {
    let mut out = Vec::new();
    match &item.form {
        Some(f) => out.push(key(f).to_owned()),
        None if item.garment.is_none() => out.push(key(&item.id).to_owned()),
        None => {}
    }
    if let Some(g) = &item.garment {
        out.push(key(g).to_owned());
    }
    if let Some(m) = item.material.as_deref() {
        out.push(key(m).to_owned());
        if let Some(mat) = c.materials.get(m) {
            out.extend(mat.tags.iter().cloned());
        }
    }
    out.extend(item.tags.iter().cloned());
    out.sort();
    out.dedup();
    out
}

/// A block's keys: its name and its material's keys.
pub fn block_keys(name: &str, material: Option<&Material>) -> Vec<String> {
    let mut out = vec![key(name).to_owned()];
    if let Some(m) = material {
        out.extend(material_keys(m));
    }
    out
}

/// Whether a block (by name, with its material) is one a target accepts.
pub fn block_matches(b: &BlockMatch, name: &str, material: Option<&Material>) -> bool {
    match b {
        BlockMatch::Id(id) => key(id.as_str()) == key(name),
        BlockMatch::Material(f) => material.is_some_and(|m| f.matches(m.id.as_str(), m)),
        BlockMatch::Suffix(sfx) => name.ends_with(sfx.as_str()),
        BlockMatch::Any(list) => list.iter().any(|b| block_matches(b, name, material)),
    }
}

/// The materials a target's filters accept, wherever they are in it.
fn target_materials<'a>(b: &BlockMatch, c: &'a Content, out: &mut Vec<&'a Material>) {
    match b {
        BlockMatch::Material(f) => out.extend(c.materials.iter().filter(|m| f.matches(&m.id, m))),
        BlockMatch::Any(list) => {
            for b in list {
                target_materials(b, c, out);
            }
        }
        _ => {}
    }
}

/// `verb:key` for each key.
pub fn with_verb(verb: &str, keys: &[String]) -> impl Iterator<Item = String> {
    keys.iter().map(move |k| format!("{verb}:{k}"))
}

/// Whether an item matches what a process input, output or target asks for.
pub fn item_matches(m: &Match, item: &ItemDef, c: &Content) -> bool {
    match m {
        Match::Item(r) => item.id == r.as_str(),
        Match::Form { form, materials } => {
            item.form.as_deref() == Some(form.as_str())
                && materials.as_ref().is_none_or(|f| {
                    item.material
                        .as_deref()
                        .and_then(|mid| c.materials.get(mid).map(|mat| f.matches(mid, mat)))
                        .unwrap_or(false)
                })
        }
        Match::Material(r) => item.material.as_deref() == Some(r.as_str()),
        Match::Tag(t) => item.tags.contains(t),
        Match::Garment(g) => item.garment.as_deref() == Some(g.as_str()),
    }
}

/// What may stand for a process's inputs, tools and target when listing its triggers.
pub struct Usable<'a> {
    /// Items that can be had.
    pub item: &'a dyn Fn(&str) -> bool,
    /// Materials that can be had.
    pub material: &'a dyn Fn(&str) -> bool,
    /// The world's blocks (name, material), for block targets; `None`: unknown.
    pub blocks: Option<&'a [(String, Option<String>)]>,
}

/// Every trigger doing `p` can emit with what is usable: `do:`, what it teaches, its verb on the
/// keys of every thing that can stand for its inputs and target, and `use:` on its tools.
pub fn process_triggers(p: &Process, c: &Content, usable: &Usable, out: &mut FxHashSet<String>) {
    let blocks = usable.blocks;
    out.insert(format!("do:{}", key(&p.id)));
    out.extend(p.teaches.iter().cloned());
    let item_keys_of = |m: &Match, out: &mut FxHashSet<String>, verb: &str| {
        if let Match::Material(r) = m
            && (usable.material)(r.as_str())
            && let Some(mat) = c.materials.get(r.as_str())
        {
            out.extend(with_verb(verb, &material_keys(mat)));
        }
        for it in c
            .items
            .iter()
            .filter(|it| (usable.item)(&it.id) && item_matches(m, it, c))
        {
            out.extend(with_verb(verb, &item_keys(it, c)));
        }
    };
    if let Some(verb) = &p.verb {
        for i in &p.inputs {
            item_keys_of(&i.item, out, verb);
        }
        match &p.target {
            Some(Target::Thing(m)) => item_keys_of(m, out, verb),
            Some(Target::Block(b)) => {
                for (name, mat) in blocks.unwrap_or(&[]) {
                    let material = mat.as_deref().and_then(|m| c.materials.get(m));
                    if block_matches(b, name, material) {
                        out.extend(with_verb(verb, &block_keys(name, material)));
                    }
                }
                let mut mats = Vec::new();
                target_materials(b, c, &mut mats);
                for m in mats {
                    out.extend(with_verb(verb, &material_keys(m)));
                }
            }
            Some(Target::Water) => {
                out.insert(format!("{verb}:water"));
            }
            Some(Target::Fire) => {
                out.insert(format!("{verb}:fire"));
            }
            Some(Target::Ground) | None => {}
        }
    }
    for t in &p.tools {
        for it in c.items.iter().filter(|it| {
            (usable.item)(&it.id) && it.property(&t.property).is_some_and(|v| v >= t.min)
        }) {
            out.extend(with_verb("use", &item_keys(it, c)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_drop_namespaces() {
        assert_eq!(key("hearth:flint"), "flint");
        assert_eq!(key("flint"), "flint");
    }

    #[test]
    fn a_flint_cobble_answers_to_its_form_material_and_tags() {
        let c = Content::load_base();
        let cobble = c.items.get("hearth:cobble/flint").expect("flint cobble");
        let keys = item_keys(cobble, &c);
        for k in ["cobble", "flint", "knappable", "stone"] {
            assert!(keys.iter().any(|x| x == k), "{k} in {keys:?}");
        }
    }
}

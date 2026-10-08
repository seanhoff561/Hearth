//! The hands' natural uses (Amendment P §5.2): which of the `interaction/` rules a hand follows
//! now, with what it holds, on what is looked at. Mind-agnostic (Amendment E's Actor rule): the
//! player's buttons ask it, and any other mind driving a body may.

use hearth_content::Content;
use hearth_content::schema::interaction::{Holding, Intent, IntentAct, IntentTarget};
use hearth_content::schema::item::Use;
use hearth_items::{ItemKind, Stack};

use crate::engine::{Aimed, Bench, Crafts, Recipe, plan};

/// What a hand does.
#[derive(Debug, Clone, PartialEq)]
pub enum HandAct {
    /// Do this process (an index into the recipes).
    Process(usize),
    PickUp,
    /// Drink: from the water with cupped hands, or from the vessel held.
    Drink,
    Fill,
    Eat,
    /// The thing held's own use, or a blow.
    Blow,
}

/// A hand's use now: the rule followed (none: the process preferred), its hint's words, what it
/// does, and whether the hand must first put away what it holds (its rule wants an empty hand).
#[derive(Debug, Clone)]
pub struct HandUse<'a> {
    pub rule: Option<&'a Intent>,
    pub hint: String,
    pub act: HandAct,
    pub stow: bool,
}

/// What the resolver asks of the one acting: whether they may try a recipe, their skill in it,
/// whether a thing is food they know to be food (an unknown plant is not eaten by default), and
/// a process they prefer for this target and thing held (learned from the action menu).
pub struct Ask<'a> {
    pub may: &'a dyn Fn(&Recipe) -> bool,
    pub skill: &'a dyn Fn(&Recipe) -> f32,
    pub known_food: &'a dyn Fn(&ItemKind, &Stack) -> bool,
    pub prefer: Option<&'a str>,
}

/// The rules in their order.
pub fn rules(content: &Content) -> Vec<&Intent> {
    let mut v: Vec<&Intent> = content.intents.iter().collect();
    v.sort_by_key(|r| r.order);
    v
}

/// The use of `hand`, holding `held`, on what the bench looks at (`bench.tool_hand` must be
/// `hand`, so the tools are that hand's): the preferred rule if it can be done, else the first
/// rule that can be; a hand that holds something tries, after its own rules, an empty hand's
/// (putting the thing away first).
pub fn resolve<'a>(
    content: &'a Content,
    crafts: &Crafts,
    bench: &Bench,
    held: Option<&Stack>,
    ask: &Ask,
) -> Option<HandUse<'a>> {
    debug_assert!(bench.tool_hand.is_some());
    let rules = rules(content);
    let kind = held.and_then(|s| bench.items.get(&s.id).map(|k| (k, s)));
    let try_rule = |r: &'a Intent, empty: bool| -> Option<HandUse<'a>> {
        let holding = if empty { None } else { kind };
        if !holds(&r.holding, holding, bench.items, ask) || !targets(&r.target, bench) {
            return None;
        }
        let act = act_of(r, crafts, bench, holding, ask)?;
        Some(HandUse {
            rule: Some(r),
            hint: r.hint.clone(),
            act,
            stow: empty && held.is_some(),
        })
    };
    let passes: &[bool] = if held.is_some() {
        &[false, true]
    } else {
        &[true]
    };
    // The process preferred here, if it can be done with this hand.
    if let Some(p) = ask.prefer
        && let Some(i) = crafts.index_of(p)
    {
        let recipe = &crafts.recipes[i];
        if (ask.may)(recipe) && plan(crafts, i, bench, (ask.skill)(recipe)).is_ok() {
            return Some(HandUse {
                rule: None,
                hint: recipe
                    .def
                    .verb
                    .clone()
                    .unwrap_or_else(|| recipe.def.action.clone()),
                act: HandAct::Process(i),
                stow: false,
            });
        }
    }
    for &empty in passes {
        for r in &rules {
            // Only an empty hand's rules are tried again with the hand emptied.
            if empty && held.is_some() && r.holding != Holding::Empty {
                continue;
            }
            if let Some(u) = try_rule(r, empty) {
                return Some(u);
            }
        }
    }
    None
}

/// Whether what the hand holds (none: nothing) is what a rule asks.
fn holds(
    h: &Holding,
    held: Option<(&ItemKind, &Stack)>,
    items: &hearth_items::Items,
    ask: &Ask,
) -> bool {
    match (h, held) {
        (Holding::Anything, _) => true,
        (Holding::Empty, None) => true,
        (_, None) => false,
        (Holding::Empty, Some(_)) => false,
        (Holding::Property { property, min }, Some((_, s))) => {
            s.property(items, property).is_some_and(|v| v >= *min)
        }
        (Holding::Use(u), Some((k, _))) => k.primary == Some(*u),
        (Holding::Weapon, Some((k, _))) => matches!(
            k.primary,
            Some(Use::Thrust | Use::Swing | Use::Slash | Use::Stab | Use::Strike)
        ),
        (Holding::Vessel, Some((k, _))) => k.container.is_some_and(|c| c.liquid_l > 0.0),
        (Holding::Food, Some((k, s))) => (ask.known_food)(k, s),
    }
}

/// Whether what is looked at is what a rule is for.
fn targets(t: &IntentTarget, bench: &Bench) -> bool {
    match (t, &bench.aimed) {
        (IntentTarget::Nothing, None) => true,
        (IntentTarget::AnyBlock, Some(Aimed::Block { .. })) => true,
        (IntentTarget::Block(m), Some(Aimed::Block { name, material, .. })) => {
            let mat = material
                .as_deref()
                .and_then(|m| bench.content.materials.get(m));
            hearth_content::triggers::block_matches(m, name, mat)
        }
        (IntentTarget::Thing, Some(Aimed::Thing(_))) => true,
        (IntentTarget::Water, Some(Aimed::Water)) => true,
        (IntentTarget::Fire, Some(Aimed::Fire(f))) => f.lit,
        (IntentTarget::Fire, Some(Aimed::Station { fire: Some(f), .. })) => f.lit,
        (IntentTarget::Animal, Some(Aimed::Animal { .. })) => true,
        _ => false,
    }
}

/// What a rule does here, if it can be done now.
fn act_of(
    r: &Intent,
    crafts: &Crafts,
    bench: &Bench,
    held: Option<(&ItemKind, &Stack)>,
    ask: &Ask,
) -> Option<HandAct> {
    let doable = |i: usize| {
        let recipe = &crafts.recipes[i];
        (ask.may)(recipe) && plan(crafts, i, bench, (ask.skill)(recipe)).is_ok()
    };
    match &r.act {
        IntentAct::Verb(v) => (0..crafts.recipes.len())
            .find(|&i| crafts.recipes[i].def.verb.as_deref() == Some(v.as_str()) && doable(i))
            .map(HandAct::Process),
        IntentAct::Process(id) => crafts
            .index_of(hearth_content::IdRef::qualify(id).as_str())
            .filter(|&i| doable(i))
            .map(HandAct::Process),
        IntentAct::PickUp => Some(HandAct::PickUp),
        IntentAct::Drink => match held {
            Some((_, s)) if s.liquid_l < 0.1 => None,
            _ => Some(HandAct::Drink),
        },
        IntentAct::Fill => match held {
            Some((k, s)) if k.container.is_some_and(|c| s.liquid_l < c.liquid_l) => {
                Some(HandAct::Fill)
            }
            _ => None,
        },
        IntentAct::Eat => Some(HandAct::Eat),
        IntentAct::Blow => Some(HandAct::Blow),
    }
}

//! The game modes (Amendment P §2): Realistic, Easy and Creative, each setting every world rule
//! from `data/hearth/balance/modes.ron` — the realism preset, how knowledge is gained, what a new
//! life keeps, predators' ways, hints, the clock, watching the world, Creative's powers and how a first
//! life starts. A world's mode may change only toward a less strict one.

use hearth_content::Content;
use hearth_content::schema::config::GameMode;
use hearth_save::{AfterDeath, KnowledgeMode, LifeSettings, PredatorBehavior};
use serde::de::DeserializeOwned;

/// The mode a world is made in when none is named.
pub const DEFAULT: &str = "hearth:realistic";

/// The clock a mode shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clock {
    /// No clock: the time in words.
    None,
    /// A small clock and day counter may be shown.
    Optional,
    /// An exact clock and date, editable.
    Exact,
}

/// What a mode sets.
#[derive(Debug, Clone, PartialEq)]
pub struct Rules {
    pub id: String,
    /// The realism preset; `None`: no needs and no harm (Creative).
    pub realism: Option<String>,
    pub knowledge: KnowledgeMode,
    /// What a new life keeps of what earlier lives knew (Amendment E §6.6).
    pub after_death: AfterDeath,
    pub predators: PredatorBehavior,
    pub hints: bool,
    pub clock: Clock,
    pub observer: bool,
    pub creative: bool,
    pub strictness: u32,
}

fn word<T: DeserializeOwned>(s: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(s.to_owned())).ok()
}

/// A mode's rules (unknown words fall back to the strictest reading).
pub fn rules(m: &GameMode) -> Rules {
    Rules {
        id: m.id.clone(),
        realism: (!m.realism.is_empty()).then(|| m.realism.clone()),
        knowledge: word(&m.knowledge).unwrap_or_default(),
        after_death: word::<AfterDeath>(&m.after_death).unwrap_or_default(),
        predators: word(&m.predators).unwrap_or_default(),
        hints: m.hints,
        clock: match m.clock.as_str() {
            "exact" => Clock::Exact,
            "optional" => Clock::Optional,
            _ => Clock::None,
        },
        observer: m.observer,
        creative: m.creative,
        strictness: m.strictness,
    }
}

/// The modes there are, in Create World's order.
pub fn all(content: &Content) -> Vec<&GameMode> {
    let mut v: Vec<&GameMode> = content.modes.iter().collect();
    v.sort_by_key(|m| m.order);
    v
}

/// A mode by its id (with or without its namespace).
pub fn find<'a>(content: &'a Content, id: &str) -> Option<&'a GameMode> {
    let bare = |s: &str| s.rsplit(':').next().unwrap_or(s).to_owned();
    content.modes.iter().find(|m| bare(&m.id) == bare(id))
}

/// Sets a world's life settings to a mode's rules.
pub fn apply(r: &Rules, life: &mut LifeSettings) {
    life.realism.preset = r.realism.clone().unwrap_or_else(|| "authentic".to_owned());
    life.knowledge_mode = r.knowledge;
    life.after_death = r.after_death;
    life.predator_behavior = r.predators;
}

/// Whether a world in mode `from` may be changed to `to`: only toward a less strict one.
pub fn may_change(from: &Rules, to: &Rules) -> bool {
    to.strictness < from.strictness
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_modes_set_the_rules() {
        let content = Content::load_base();
        let modes = all(&content);
        assert_eq!(
            modes.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
            ["Creative", "Easy", "Realistic"]
        );
        let r = |id: &str| rules(find(&content, id).expect(id));
        let (real, easy, creative) = (r("realistic"), r("easy"), r("creative"));
        assert_eq!(real.realism.as_deref(), Some("authentic"));
        assert_eq!(real.knowledge, KnowledgeMode::Discovery);
        assert!(
            !real.observer && !easy.observer,
            "no watching but in Creative"
        );
        assert!(!real.creative && !easy.creative && creative.creative);
        assert_eq!(easy.realism.as_deref(), Some("hardy"));
        assert_eq!(easy.knowledge, KnowledgeMode::Guided);
        assert_eq!(easy.after_death, AfterDeath::KeepEverything);
        assert!(easy.hints && !real.hints);
        assert_eq!(creative.realism, None);
        assert_eq!(creative.knowledge, KnowledgeMode::Open);
        assert_eq!(creative.clock, Clock::Exact);
        // Only toward less strict.
        assert!(may_change(&real, &easy) && may_change(&easy, &creative));
        assert!(!may_change(&easy, &real) && !may_change(&creative, &real));
        let mut life = LifeSettings::default();
        apply(&easy, &mut life);
        assert_eq!(life.realism.preset, "hardy");
        assert_eq!(life.predator_behavior, PredatorBehavior::Tranquil);
        // Every mode's realism is a preset there is.
        for m in modes {
            let r = rules(m);
            if let Some(p) = r.realism {
                assert!(
                    content.balance_presets.iter().any(|q| q.id.ends_with(&p)),
                    "{}: no preset {p}",
                    m.id
                );
            }
        }
    }
}

//! A stack: one item, or several of a kind that share a cell, with what a container holds and
//! the liquid in it. Kinds are named by id, so saves survive changes to the content.

use serde::{Deserialize, Serialize};

use crate::container::Container;
use crate::registry::{ItemKind, Items};

fn one() -> u16 {
    1
}

fn is_one(n: &u16) -> bool {
    *n == 1
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

fn whole() -> f32 {
    1.0
}

fn is_whole(v: &f32) -> bool {
    *v == 1.0
}

fn middling() -> f32 {
    0.5
}

fn is_middling(v: &f32) -> bool {
    *v == 0.5
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stack {
    pub id: String,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub count: u16,
    /// What it holds, if it is a container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inside: Option<Box<Container>>,
    /// Liquid in it (litres of water).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub liquid_l: f32,
    /// 0–1 how wet it is.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub wet: f32,
    /// 0–1 how well it was made (0.5 for what nature gives); it scales what a tool does.
    #[serde(default = "middling", skip_serializing_if = "is_middling")]
    pub quality: f32,
    /// 0–1 what is left of its edge, point or binding (1 as made); of a carcass, what is left of
    /// it (a kill the hunter or the scavengers have eaten from).
    #[serde(default = "whole", skip_serializing_if = "is_whole")]
    pub condition: f32,
    /// 0–1 how far it has gone off (1 spoiled).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub decay: f32,
    /// Hours an ember has left to glow.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub glow_h: f32,
}

impl Stack {
    pub fn one(id: &str) -> Self {
        Self::of(id, 1)
    }

    pub fn of(id: &str, count: u16) -> Self {
        Self {
            id: id.to_owned(),
            count: count.max(1),
            inside: None,
            liquid_l: 0.0,
            wet: 0.0,
            quality: 0.5,
            condition: 1.0,
            decay: 0.0,
            glow_h: 0.0,
        }
    }

    /// Made at a quality.
    pub fn made(id: &str, count: u16, quality: f32) -> Self {
        Self {
            quality: quality.clamp(0.0, 1.0),
            ..Self::of(id, count)
        }
    }

    /// A property of its kind as this one has it: what its making and wear leave of it (a
    /// fine, fresh edge cuts best).
    pub fn property(&self, items: &Items, name: &str) -> Option<f32> {
        let base = self.kind(items)?.property(name)?;
        Some(base * (0.7 + 0.6 * self.quality) * (0.4 + 0.6 * self.condition.clamp(0.0, 1.0)))
    }

    pub fn kind<'a>(&self, items: &'a Items) -> Option<&'a ItemKind> {
        items.get(&self.id)
    }

    /// Its mass with everything in it (kg); an unknown kind weighs nothing.
    pub fn mass(&self, items: &Items) -> f32 {
        let own = self.kind(items).map_or(0.0, |k| k.mass_kg) * self.count as f32;
        let inside = self.inside.as_ref().map_or(0.0, |c| c.mass(items));
        own + inside + self.liquid_l
    }

    /// What it holds, opened (empty if nothing yet); `None` if it holds nothing at all.
    pub fn contents_mut(&mut self, items: &Items) -> Option<&mut Container> {
        let spec = self.kind(items)?.container?;
        if spec.grid.0 == 0 || spec.grid.1 == 0 {
            return None;
        }
        Some(self.inside.get_or_insert_with(Default::default))
    }

    pub fn contents(&self) -> Option<&Container> {
        self.inside.as_deref()
    }

    /// Whether two stacks may merge into one (same kind, nothing inside either, made and worn
    /// alike, gone off about as far, no embers).
    pub fn joins(&self, other: &Stack) -> bool {
        self.id == other.id
            && self.inside.as_ref().is_none_or(|c| c.items.is_empty())
            && other.inside.as_ref().is_none_or(|c| c.items.is_empty())
            && self.liquid_l == 0.0
            && other.liquid_l == 0.0
            && (self.quality - other.quality).abs() < 0.05
            && (self.condition - other.condition).abs() < 0.05
            && (self.decay - other.decay).abs() < 0.1
            && self.glow_h == 0.0
            && other.glow_h == 0.0
    }

    /// Takes `other` (which [`Stack::joins`] this) into this stack: counts add, wetness and
    /// decay average by count.
    pub fn absorb(&mut self, other: Stack) {
        let (a, b) = (self.count as f32, other.count as f32);
        let mix = |x: f32, y: f32| (x * a + y * b) / (a + b).max(1.0);
        self.wet = mix(self.wet, other.wet);
        self.decay = mix(self.decay, other.decay);
        self.count = self.count.saturating_add(other.count);
    }
}

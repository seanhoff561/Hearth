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
        }
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

    /// Whether two stacks may merge into one (same kind, nothing inside either).
    pub fn joins(&self, other: &Stack) -> bool {
        self.id == other.id
            && self.inside.as_ref().is_none_or(|c| c.items.is_empty())
            && other.inside.as_ref().is_none_or(|c| c.items.is_empty())
            && self.liquid_l == 0.0
            && other.liquid_l == 0.0
    }
}

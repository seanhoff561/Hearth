//! The mind (`humans/mind/`), V2.1 §6.1; H2: the routines that give a people's days their
//! ordinary shape — when they sleep, forage, rest, work and keep company — as data. A culture's
//! own (H5) will replace a species' where it has one.

use serde::{Deserialize, Serialize};

use super::entry;
use crate::IdRef;

/// What a routine's hours are for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Routinely {
    /// Asleep (in a nest, or where they are).
    Sleep,
    /// Feeding and finding food.
    Forage,
    /// Resting through the heat of the day.
    Rest,
    /// Working: making and mending what they use.
    Work,
    /// Keeping company: grooming, sitting together.
    Socialize,
}

/// A stretch of the day and what it is for, and how strongly it pulls (added to that choice's
/// worth; sleep's hours are simply night).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
    /// Local solar hours, from and to (to before from: across midnight).
    pub from: f32,
    pub to: f32,
    pub doing: Routinely,
    #[serde(default)]
    pub weight: f32,
}

impl Block {
    /// Whether an hour falls in it.
    pub fn holds(&self, hour: f32) -> bool {
        let h = hour.rem_euclid(24.0);
        if self.from <= self.to {
            (self.from..self.to).contains(&h)
        } else {
            h >= self.from || h < self.to
        }
    }
}

entry! {
    /// A species' routine (V2.1 §6.1): the shape of its days.
    pub struct Routine in "humans/mind/routines", schema 1, name name {
        pub name: String,
        pub species: IdRef,
        pub blocks: Vec<Block>,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_hold_their_hours_across_midnight() {
        let night = Block {
            from: 18.5,
            to: 6.0,
            doing: Routinely::Sleep,
            weight: 1.0,
        };
        assert!(night.holds(23.0) && night.holds(2.0) && !night.holds(12.0));
        let noon = Block {
            from: 10.5,
            to: 14.5,
            doing: Routinely::Rest,
            weight: 0.2,
        };
        assert!(noon.holds(12.0) && !noon.holds(15.0));
    }
}

//! Groups: the company an agent keeps — who belongs, the range they share and the places in it
//! they know, the ties between them, how they have come to take a person, and their ways.

use std::collections::BTreeMap;

use glam::{DVec2, DVec3};

/// A group's ways: the techniques it practises (processes) and its traditions (later: what it
/// teaches its young, how it greets, what it will not eat).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Culture {
    pub techniques: Vec<String>,
    pub traditions: Vec<String>,
}

/// The places in its range a group knows, shared by all of it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Places {
    /// Where it drinks.
    pub water: Vec<DVec3>,
    /// Trees it sleeps in.
    pub sleep: Vec<DVec3>,
    /// Its anvils: stones it cracks nuts on, with the hammerstones left by them.
    pub anvils: Vec<DVec3>,
    /// Trees and patches it feeds at.
    pub food: Vec<DVec3>,
}

impl Places {
    /// The place of a list nearest to a point.
    pub fn nearest(list: &[DVec3], to: DVec3) -> Option<DVec3> {
        list.iter().copied().min_by(|a, b| {
            (*a - to)
                .length_squared()
                .total_cmp(&(*b - to).length_squared())
        })
    }

    /// Remembers a place, unless one of the list is within `near` m of it; keeps at most `most`.
    pub fn remember(list: &mut Vec<DVec3>, at: DVec3, near: f64, most: usize) {
        if list.iter().any(|p| (*p - at).length() < near) {
            return;
        }
        list.push(at);
        if list.len() > most {
            list.remove(0);
        }
    }
}

/// A group of agents.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SocialGroup {
    pub id: u64,
    /// Its kind (index into [`crate::Kinds`]).
    pub kind: usize,
    /// Its members' ids.
    pub members: Vec<u64>,
    /// The middle of its range and how far the range reaches (m).
    pub home: DVec2,
    pub range_m: f64,
    pub places: Places,
    /// Ties between members, 0 strangers … 1 mother and young, keyed by the pair (lower id
    /// first).
    pub ties: BTreeMap<(u64, u64), f32>,
    /// How well it has come to tolerate a person: 0 it flees at sight … 1 it lets them sit near
    /// and goes on with its day.
    pub habituation: f32,
    pub culture: Culture,
    /// The ecology's group it was drawn out of, to fold back into.
    pub population_group: Option<u64>,
}

impl SocialGroup {
    /// The tie between two members.
    pub fn tie(&self, a: u64, b: u64) -> f32 {
        let key = if a < b { (a, b) } else { (b, a) };
        self.ties.get(&key).copied().unwrap_or(0.0)
    }

    /// Sets the tie between two members.
    pub fn set_tie(&mut self, a: u64, b: u64, t: f32) {
        let key = if a < b { (a, b) } else { (b, a) };
        self.ties.insert(key, t.clamp(0.0, 1.0));
    }
}

//! Bands: the company a person keeps — who belongs, the range they share and the places in it
//! they know between them, how they have come to take the player, their ways. (Households,
//! kinship and relationships replace and extend it in H4.) The band's places are what its people
//! know of their range between them: each member's own mental map (`memory.rs`, H2) starts from
//! them and grows by its own days.

use glam::{DVec2, DVec3};
use hearth_math::hash::Rng;
use serde::{Deserialize, Serialize};

use crate::person::{PersonId, Tier};
use crate::world::PlayerId;

/// A band's ways: the techniques it practises (processes) and its traditions (H5: generated
/// cultures).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Culture {
    /// What its people know (knowledge nodes), and the processes that opens.
    #[serde(default)]
    pub knowledge: Vec<String>,
    pub techniques: Vec<String>,
    pub traditions: Vec<String>,
}

/// The places in its range a band knows, shared by all of it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
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

/// A band of persons.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Band {
    pub id: u64,
    /// Its species' id.
    pub species: String,
    /// Its members' ids (the dead among them until its life course takes them off).
    pub members: Vec<PersonId>,
    /// The middle of its range and how far the range reaches (m).
    pub home: DVec2,
    pub range_m: f64,
    #[serde(default)]
    pub places: Places,
    /// How well it has come to tolerate each player it has met: 0 it flees at sight … 1 it lets
    /// them sit near and goes on with its day.
    #[serde(default)]
    pub tolerance: Vec<(PlayerId, f32)>,
    #[serde(default)]
    pub culture: Culture,
    /// The ecological cells' group it lives on in while the player is away.
    #[serde(default)]
    pub population_group: Option<u64>,
    #[serde(default)]
    pub tier: Tier,
    /// The day it was last folded back into the cells' numbers.
    #[serde(default)]
    pub dormant_since: Option<f64>,
    /// The day its life course was last reckoned to (a band of a people with a life table).
    #[serde(default)]
    pub lived_to: Option<f64>,
    /// Its own random stream: where its members come down, who dies and who is born while the
    /// player is away.
    pub rng: Rng,
}

impl Band {
    /// How well it tolerates a player (0 for one it has never met).
    pub fn tolerance_of(&self, player: PlayerId) -> f32 {
        self.tolerance
            .iter()
            .find(|(p, _)| *p == player)
            .map_or(0.0, |(_, t)| *t)
    }

    /// Sets how well it tolerates a player.
    pub fn set_tolerance(&mut self, player: PlayerId, t: f32) {
        let t = t.clamp(0.0, 1.0);
        match self.tolerance.iter_mut().find(|(p, _)| *p == player) {
            Some(e) => e.1 = t,
            None => self.tolerance.push((player, t)),
        }
    }
}

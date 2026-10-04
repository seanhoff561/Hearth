//! Memory and belief (V2.1 §6.3; H2): what a person remembers — the places of its range as its
//! own mental map (where it drank, slept, fed and worked, where it saw danger), the people it
//! knows, and the things that happened to it that mattered — and what it believes from them,
//! which can be wrong: a remembered food tree may be bare when it gets there, a danger long gone.
//! Memory is bounded: what is slight fades and is let go; what defines a life (a loss, meeting a
//! stranger of another kind, a bad hurt) goes into the life history.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::person::PersonId;
use crate::world::PlayerId;

/// A kind of place a person remembers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceKind {
    Water,
    /// A tree it slept in (or a place to sleep).
    Sleep,
    /// A tree or patch it fed at.
    Food,
    /// A stone it worked on (an anvil), with the tools left by it.
    Anvil,
    /// Where it saw a hunter of its kind: a belief that danger is there, fading as days pass.
    Danger,
}

impl PlaceKind {
    /// How many of a kind it keeps.
    fn most(self) -> usize {
        match self {
            PlaceKind::Water => 6,
            PlaceKind::Sleep => 12,
            PlaceKind::Food => 12,
            PlaceKind::Anvil => 8,
            PlaceKind::Danger => 6,
        }
    }

    /// How near two of a kind are the same place (m).
    fn same_m(self) -> f64 {
        match self {
            PlaceKind::Water => 10.0,
            PlaceKind::Sleep => 6.0,
            PlaceKind::Food => 15.0,
            PlaceKind::Anvil => 8.0,
            PlaceKind::Danger => 40.0,
        }
    }

    /// Days for a memory of it to lose half its hold, unvisited.
    fn half_days(self) -> f64 {
        match self {
            PlaceKind::Water => 60.0,
            PlaceKind::Sleep => 30.0,
            PlaceKind::Food => 15.0,
            PlaceKind::Anvil => 60.0,
            PlaceKind::Danger => 2.0,
        }
    }
}

/// A place remembered: what, where, when last seen so, and how sure it is (0–1).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Remembered {
    pub kind: PlaceKind,
    pub at: DVec3,
    pub day: f64,
    pub sure: f32,
}

/// Someone it knows: a person of its kind or a player, where and when last seen, and how familiar
/// (0–1) — grown by time in each other's sight.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Known {
    pub who: Who,
    pub at: DVec3,
    pub day: f64,
    pub familiarity: f32,
}

/// A person or a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Who {
    Person(PersonId),
    Player(PlayerId),
}

/// Something that happened to it that mattered.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Happened {
    /// It saw a hunter of its kind near.
    Threat,
    /// It was hurt.
    Hurt,
    /// One of its kin died.
    Loss { who: PersonId },
    /// It first came near a player.
    Met { player: PlayerId },
    /// It ate its fill of a rich food.
    Feast,
}

/// An episode: what happened, when and where, and how much it weighs (0–1: what was felt).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub day: f64,
    pub what: Happened,
    pub at: DVec3,
    pub weight: f32,
}

/// How many episodes it keeps.
const EPISODES: usize = 24;
/// How many it knows by sight before the least familiar are forgotten.
const KNOWN: usize = 48;
/// Days for an episode to lose half its weight (more for the weightier: rehearsal).
const EPISODE_HALF_DAYS: f64 = 8.0;
/// The weakest a memory is kept.
const KEEP: f32 = 0.05;

/// What a person remembers.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Memory {
    pub places: Vec<Remembered>,
    pub known: Vec<Known>,
    pub episodes: Vec<Episode>,
    /// The day it was last faded to.
    pub day: f64,
}

impl Memory {
    /// Remembers a place of a kind as seen now, `sure` as it is of it: an old memory of the same
    /// place refreshed, else a new one (the least sure of the kind let go when there are too
    /// many).
    pub fn remember(&mut self, kind: PlaceKind, at: DVec3, day: f64, sure: f32) {
        let same = kind.same_m();
        if let Some(r) = self
            .places
            .iter_mut()
            .find(|r| r.kind == kind && (r.at - at).length() < same)
        {
            r.day = day;
            r.sure = r.sure.max(sure);
            if kind == PlaceKind::Danger {
                r.at = at;
            }
            return;
        }
        self.places.push(Remembered {
            kind,
            at,
            day,
            sure: sure.clamp(0.0, 1.0),
        });
        let of_kind = self.places.iter().filter(|r| r.kind == kind).count();
        if of_kind > kind.most()
            && let Some(i) = self
                .places
                .iter()
                .enumerate()
                .filter(|(_, r)| r.kind == kind)
                .min_by(|a, b| {
                    a.1.sure
                        .total_cmp(&b.1.sure)
                        .then(a.1.day.total_cmp(&b.1.day))
                })
                .map(|(i, _)| i)
        {
            self.places.remove(i);
        }
    }

    /// Found wrong: the place of a kind about `at` is not what it was (the fruit eaten, the
    /// water dry); it is let go.
    pub fn forget(&mut self, kind: PlaceKind, at: DVec3) {
        let same = kind.same_m();
        self.places
            .retain(|r| !(r.kind == kind && (r.at - at).length() < same));
    }

    /// The remembered place of a kind nearest to a point.
    pub fn nearest(&self, kind: PlaceKind, to: DVec3) -> Option<DVec3> {
        self.places
            .iter()
            .filter(|r| r.kind == kind)
            .min_by(|a, b| {
                (a.at - to)
                    .length_squared()
                    .total_cmp(&(b.at - to).length_squared())
            })
            .map(|r| r.at)
    }

    /// The places of a kind it remembers.
    pub fn all(&self, kind: PlaceKind) -> impl Iterator<Item = DVec3> + '_ {
        self.places
            .iter()
            .filter(move |r| r.kind == kind)
            .map(|r| r.at)
    }

    /// How strongly it believes danger is about a place (0–1).
    pub fn danger_at(&self, at: DVec3) -> f32 {
        self.places
            .iter()
            .filter(|r| r.kind == PlaceKind::Danger)
            .filter(|r| (r.at - at).length() < PlaceKind::Danger.same_m() * 1.5)
            .map(|r| r.sure)
            .fold(0.0, f32::max)
    }

    /// Someone in sight for `dt` seconds of play: where and when, and a little more familiar.
    /// True the first time it is seen.
    pub fn see(&mut self, who: Who, at: DVec3, day: f64, dt: f32) -> bool {
        // An hour of play together makes a stranger familiar.
        let grow = dt / 3600.0;
        if let Some(k) = self.known.iter_mut().find(|k| k.who == who) {
            k.at = at;
            k.day = day;
            k.familiarity = (k.familiarity + grow).min(1.0);
            return false;
        }
        self.known.push(Known {
            who,
            at,
            day,
            familiarity: grow,
        });
        if self.known.len() > KNOWN
            && let Some(i) = self
                .known
                .iter()
                .enumerate()
                .min_by(|a, b| {
                    a.1.familiarity
                        .total_cmp(&b.1.familiarity)
                        .then(a.1.day.total_cmp(&b.1.day))
                })
                .map(|(i, _)| i)
        {
            self.known.remove(i);
        }
        true
    }

    /// How familiar someone is (0 for a stranger).
    pub fn familiarity(&self, who: Who) -> f32 {
        self.known
            .iter()
            .find(|k| k.who == who)
            .map_or(0.0, |k| k.familiarity)
    }

    /// Where it last saw someone, and when.
    pub fn last_seen(&self, who: Who) -> Option<(DVec3, f64)> {
        self.known
            .iter()
            .find(|k| k.who == who)
            .map(|k| (k.at, k.day))
    }

    /// Something happened that weighed `weight` (0–1): kept among its episodes (the lightest
    /// let go when there are too many). An earlier episode of the same kind within the hour and
    /// thirty metres is the same one, made weightier.
    pub fn happened(&mut self, what: Happened, at: DVec3, day: f64, weight: f32) {
        let weight = weight.clamp(0.0, 1.0);
        if weight < KEEP {
            return;
        }
        if let Some(e) = self.episodes.iter_mut().find(|e| {
            std::mem::discriminant(&e.what) == std::mem::discriminant(&what)
                && (day - e.day).abs() < 1.0 / 24.0
                && (e.at - at).length() < 30.0
        }) {
            e.weight = e.weight.max(weight);
            return;
        }
        self.episodes.push(Episode {
            day,
            what,
            at,
            weight,
        });
        if self.episodes.len() > EPISODES
            && let Some(i) = self
                .episodes
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.weight.total_cmp(&b.1.weight))
                .map(|(i, _)| i)
        {
            self.episodes.remove(i);
        }
    }

    /// Days passing to `day`: places unvisited lose their hold (danger soonest), episodes their
    /// weight (the weightier the slower); what is faint is let go.
    pub fn fade(&mut self, day: f64) {
        let days = (day - self.day).max(0.0);
        if days < 0.01 {
            return;
        }
        self.day = day;
        for r in &mut self.places {
            r.sure *= 0.5f64.powf(days / r.kind.half_days()) as f32;
        }
        self.places
            .retain(|r| r.sure >= KEEP || (r.kind != PlaceKind::Danger && r.sure >= KEEP / 2.0));
        for e in &mut self.episodes {
            let half = EPISODE_HALF_DAYS * (1.0 + 4.0 * e.weight as f64);
            e.weight *= 0.5f64.powf(days / half) as f32;
        }
        self.episodes.retain(|e| e.weight >= KEEP);
    }

    /// Learns another's places of a kind it has seen them use (seen from near), less sure of
    /// them than of its own.
    pub fn learn_from(&mut self, other: &Memory, kind: PlaceKind, day: f64) {
        for r in other.places.iter().filter(|r| r.kind == kind) {
            if self
                .nearest(kind, r.at)
                .is_none_or(|n| (n - r.at).length() >= kind.same_m())
            {
                self.remember(kind, r.at, day, r.sure * 0.6);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_are_kept_found_wrong_and_faded() {
        let mut m = Memory::default();
        let tree = DVec3::new(10.0, 0.0, 0.0);
        m.remember(PlaceKind::Food, tree, 0.0, 1.0);
        m.remember(PlaceKind::Food, tree + DVec3::X * 3.0, 0.5, 0.8);
        assert_eq!(m.all(PlaceKind::Food).count(), 1, "the same tree");
        assert_eq!(m.nearest(PlaceKind::Food, DVec3::ZERO), Some(tree));
        // The tree is bare when it gets there: let go.
        m.forget(PlaceKind::Food, tree);
        assert_eq!(m.nearest(PlaceKind::Food, DVec3::ZERO), None);
        // A danger seen fades in days; water stays known.
        m.remember(PlaceKind::Danger, tree, 0.0, 0.9);
        m.remember(PlaceKind::Water, tree, 0.0, 1.0);
        assert!(m.danger_at(tree + DVec3::X * 20.0) > 0.8);
        m.fade(10.0);
        assert_eq!(m.danger_at(tree), 0.0, "five half-lives: forgotten");
        assert!(m.nearest(PlaceKind::Water, tree).is_some());
    }

    #[test]
    fn people_grow_familiar_and_episodes_weigh_and_fade() {
        let mut m = Memory::default();
        let who = Who::Player(1);
        assert!(m.see(who, DVec3::ZERO, 0.0, 600.0), "the first sight");
        assert!(!m.see(who, DVec3::ONE, 0.1, 1800.0));
        assert!((m.familiarity(who) - 2400.0 / 3600.0).abs() < 1e-4);
        m.happened(Happened::Threat, DVec3::ZERO, 0.0, 0.9);
        m.happened(Happened::Threat, DVec3::X * 5.0, 0.01, 0.5);
        assert_eq!(m.episodes.len(), 1, "one fright, not two");
        m.happened(Happened::Feast, DVec3::ZERO, 0.0, 0.2);
        m.fade(40.0);
        assert_eq!(m.episodes.len(), 1, "the light one let go, the heavy kept");
        assert!(matches!(m.episodes[0].what, Happened::Threat));
    }
}

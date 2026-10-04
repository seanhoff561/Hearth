//! Conflict (V2.1 §8.6; H4). A wrong — a thing taken, food kept from one hungry, a mocking, a
//! warning off — leaves rivalry in the wronged one's tie to the one who did it. Near them, and
//! angry enough for its temper, the wronged one has it out with them: an argument, which may rise
//! to threats and, rarely, to blows — a short scuffle, between the grown only, never a child, and
//! never with a player. Most quarrels ease before that: one backs down and keeps away, one of
//! standing or kin to both steps in and talks them round, or the one in the wrong makes amends
//! with food it carries; a feud that will not ease ends with the weaker side leaving the band,
//! its household with it. How long each rung lasts and when a feud parts a band are the people's
//! ways (`humans/social/ways.ron`).

use glam::{DVec2, DVec3};
use hearth_content::Content;
use hearth_content::schema::body::BodyRegion;
use hearth_items::Items;
use serde::{Deserialize, Serialize};

use crate::kin::kin_of;
use crate::mind::{Doing, Intent};
use crate::person::{Event, Person, PersonId, Tier};
use crate::psyche::{Feeling, Tendency, Value};
use crate::sim::{People, carried_food, move_toward, species_body, yaw_toward};
use crate::species::SpeciesSet;
use crate::world::{Now, World};

/// A people's ways with strangers and with quarrels (`humans/social/ways.ron`), resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct Ways {
    pub species: String,
    /// How near a stranger comes before it is watched, and before it is met (m).
    pub wary_m: f64,
    pub greet_m: f64,
    /// A guest's first trust; how readily a hungry guest is fed, to one of their own.
    pub greeting_trust: f32,
    pub hospitality: f32,
    /// How crowded their country before strangers are warned off.
    pub warn_off_crowding: f64,
    /// The trust most of the grown must have in a guest, and the days it must have been among
    /// them, for it to be taken in.
    pub take_in_trust: f32,
    pub take_in_days: f64,
    /// Seconds of argument, of threat and of blows.
    pub argue_s: f32,
    pub threat_s: f32,
    pub blows_s: f32,
    /// A feud: the rivalry and the quarrels at which one side leaves.
    pub leave_rivalry: f32,
    pub leave_quarrels: u8,
}

/// Every people's ways.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WaysSet {
    pub list: Vec<Ways>,
}

fn key(id: &str) -> &str {
    id.rsplit(':').next().unwrap_or(id)
}

impl WaysSet {
    pub fn from_content(c: &Content) -> Self {
        Self {
            list: c
                .ways
                .iter()
                .map(|w| Ways {
                    species: w.species.to_string(),
                    wary_m: w.wary_m as f64,
                    greet_m: w.greet_m as f64,
                    greeting_trust: w.greeting_trust,
                    hospitality: w.hospitality,
                    warn_off_crowding: w.warn_off_crowding as f64,
                    take_in_trust: w.take_in_trust,
                    take_in_days: w.take_in_days as f64,
                    argue_s: w.argue_s,
                    threat_s: w.threat_s,
                    blows_s: w.blows_s,
                    leave_rivalry: w.leave_rivalry,
                    leave_quarrels: w.leave_quarrels,
                })
                .collect(),
        }
    }

    /// A species' ways, if its people have them.
    pub fn of(&self, species: &str) -> Option<&Ways> {
        self.list.iter().find(|w| key(&w.species) == key(species))
    }
}

/// How far a quarrel has gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rung {
    /// Words, voices raised.
    Argument,
    /// Threats: closer, louder, the body made big.
    Threat,
    /// A short scuffle: shoves and blows.
    Blows,
}

/// How a quarrel ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settled {
    /// The words spent, each went its way.
    TalkedOut,
    /// One backed down (who), and keeps away.
    BackedDown(PersonId),
    /// One of standing or kin to both talked them round (who).
    Mediated(PersonId),
    /// The one in the wrong made amends.
    Amends,
    /// It came to blows, and was over.
    Fought,
    /// They went apart (one walked off, or could not go on).
    Parted,
}

/// A quarrel under way.
#[derive(Debug, Clone, PartialEq)]
pub struct Quarrel {
    /// The one with the grievance, who began it, and the one it holds against.
    pub a: PersonId,
    pub b: PersonId,
    pub rung: Rung,
    /// Seconds on this rung, and the whole seconds of them already weighed.
    pub held: f32,
    rolled: u32,
    /// One who has stepped in, and how long it has talked with them.
    pub mediator: Option<PersonId>,
    talked: f32,
}

/// Rivalry from which a grievance is had out with the one it holds against.
pub const GRUDGE: f32 = 0.25;
/// How near the two must be for it (m).
const QUARREL_M: f64 = 10.0;
/// The chance a second that a grievance at heat 1 is had out.
const QUARREL_A_SECOND: f32 = 0.03;
/// Two this far apart have parted (m).
const PARTED_M: f64 = 25.0;
/// How far off one may step in (m), and how long it talks them round (s).
const MEDIATE_M: f64 = 30.0;
const MEDIATE_S: f32 = 6.0;
/// One who has had it out with another lets it rest this long (days).
const RESTS_DAYS: f64 = 1.0;
/// The age (years) from which one quarrels and is quarrelled with.
pub const GROWN: f64 = 15.0;
/// How fast those quarrelling, and one stepping in, close (m/s).
const STRIDE_M_S: f32 = 1.3;
/// How far one who backs down goes off (m).
const OFF_M: f64 = 15.0;
/// How far a band's household goes when it leaves after a feud (m): this and as far again.
const LEAVE_M: f64 = 4_000.0;

/// Whether a person can go on with a quarrel (or step into one).
fn able(p: &Person) -> bool {
    p.tier == Tier::Full
        && p.alive()
        && !matches!(p.mind.doing, Doing::Fleeing { .. } | Doing::Sleeping)
}

impl People {
    /// The index of a person by its id.
    pub(crate) fn index_of_person(&self, id: PersonId) -> Option<usize> {
        self.persons.binary_search_by_key(&id, |p| p.id).ok()
    }

    /// A wrong one has suffered at another's hands: rivalry in its tie to them, the deeper the
    /// worse the wrong (0–1).
    pub(crate) fn aggrieve(&mut self, wronged: usize, by: PersonId, how: f32, day: f64) {
        if self.persons[wronged].id == by {
            return;
        }
        let k = self.tie_index(wronged, by, day);
        let t = &mut self.persons[wronged].social.ties[k];
        t.rivalry = (t.rivalry + how * (1.0 - t.rivalry)).min(1.0);
    }

    /// A wrong `who` has suffered at `by`'s hands, as bad as `how` (0–1): rivalry in its tie
    /// to them.
    pub fn wronged(&mut self, who: PersonId, by: PersonId, how: f32, day: f64) {
        if let Some(i) = self.index_of_person(who) {
            self.aggrieve(i, by, how, day);
        }
    }

    /// Whether a person is in a quarrel.
    pub fn quarrelling(&self, id: PersonId) -> bool {
        self.quarrels.iter().any(|q| q.a == id || q.b == id)
    }

    /// `a` has it out with `b`, from a rung (an argument; a threat to a stranger who will not
    /// go). Whether it began: neither may be in a quarrel already.
    pub fn quarrel(&mut self, a: PersonId, b: PersonId, rung: Rung, day: f64) -> bool {
        if a == b || self.quarrelling(a) || self.quarrelling(b) {
            return false;
        }
        let (Some(i), Some(j)) = (self.index_of_person(a), self.index_of_person(b)) else {
            return false;
        };
        self.quarrels.push(Quarrel {
            a,
            b,
            rung,
            held: 0.0,
            rolled: 0,
            mediator: None,
            talked: 0.0,
        });
        self.persons[i].record(day, Event::Quarrelled { with: b });
        self.persons[j].record(day, Event::Quarrelled { with: a });
        true
    }

    /// Each second: a grown one holding a grievance against another grown near, angry enough
    /// for its temper, has it out with them.
    pub(crate) fn grudges(&mut self, species: &SpeciesSet, now: &Now) {
        let mut heated: Vec<(usize, PersonId, f32)> = Vec::new();
        for (i, p) in self.persons.iter().enumerate() {
            if p.tier != Tier::Full
                || !p.alive()
                || p.player.is_some()
                || p.age(now) < GROWN
                || matches!(
                    p.mind.doing,
                    Doing::Sleeping
                        | Doing::Fleeing { .. }
                        | Doing::Quarrelling { .. }
                        | Doing::Mediating { .. }
                )
                || species.get(&p.species).is_none_or(|sp| sp.ways.is_none())
            {
                continue;
            }
            let mut worst: Option<(PersonId, f32, f32)> = None;
            for t in &p.social.ties {
                if t.rivalry < GRUDGE || t.quarrelled.is_some_and(|d| now.day - d < RESTS_DAYS) {
                    continue;
                }
                let Some(j) = self.index_of_person(t.who) else {
                    continue;
                };
                let q = &self.persons[j];
                if !able(q)
                    || q.age(now) < GROWN
                    || (q.place.pos - p.place.pos).length() > QUARREL_M
                {
                    continue;
                }
                if worst.is_none_or(|w| t.rivalry > w.1) {
                    worst = Some((t.who, t.rivalry, t.fear));
                }
            }
            let Some((who, rivalry, fear)) = worst else {
                continue;
            };
            let s = &p.psyche;
            let temper =
                (0.3 + s.feeling(Feeling::Anger) + 0.5 * s.feeling(Feeling::Indignation)).min(1.0);
            let heat =
                rivalry * temper * (1.3 - s.tendency(Tendency::AggressionThreshold)) * (1.0 - fear);
            heated.push((i, who, heat));
        }
        for (i, who, heat) in heated {
            let me = self.persons[i].id;
            if self.persons[i].rng.next_f32() < QUARREL_A_SECOND * heat {
                self.quarrel(me, who, Rung::Argument, now.day);
            }
        }
    }

    /// The quarrels under way, for `dt` seconds of play.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn quarrels_step(
        &mut self,
        species: &SpeciesSet,
        items: &Items,
        content: &Content,
        world: &mut dyn World,
        now: Now,
        dt: f32,
    ) {
        let mut k = 0;
        while k < self.quarrels.len() {
            match self.quarrel_step(k, species, items, content, world, &now, dt) {
                Some(how) => {
                    let q = self.quarrels.remove(k);
                    self.settle(q, how, now.day);
                }
                None => k += 1,
            }
        }
    }

    /// One quarrel's step: how it ended, if it did.
    #[allow(clippy::too_many_arguments)]
    fn quarrel_step(
        &mut self,
        k: usize,
        species: &SpeciesSet,
        items: &Items,
        content: &Content,
        world: &mut dyn World,
        now: &Now,
        dt: f32,
    ) -> Option<Settled> {
        let q = self.quarrels[k].clone();
        let (Some(i), Some(j)) = (self.index_of_person(q.a), self.index_of_person(q.b)) else {
            return Some(Settled::Parted);
        };
        if !able(&self.persons[i]) || !able(&self.persons[j]) {
            return Some(Settled::Parted);
        }
        let (at_a, at_b) = (self.persons[i].place.pos, self.persons[j].place.pos);
        if (at_a - at_b).length() > PARTED_M {
            return Some(Settled::Parted);
        }
        let Some(ways) = species
            .get(&self.persons[i].species)
            .and_then(|sp| sp.ways.clone())
        else {
            return Some(Settled::Parted);
        };
        // A player is moved by the player, and a quarrel with one never comes to blows.
        let player = self.persons[i].player.is_some() || self.persons[j].player.is_some();
        // Each holds to it, facing the other, the angrier the further it has gone.
        let (close, anger) = match q.rung {
            Rung::Argument => (2.5, 0.35),
            Rung::Threat => (1.5, 0.55),
            Rung::Blows => (0.7, 0.75),
        };
        for (x, other, there) in [(i, q.b, at_b), (j, q.a, at_a)] {
            let p = &mut self.persons[x];
            p.psyche.feel(Feeling::Anger, anger);
            if p.player.is_some() {
                continue;
            }
            p.mind.doing = Doing::Quarrelling {
                with: other,
                rung: q.rung,
            };
            p.mind.timer = 2.0;
            if (p.place.pos - there).length() > close + 0.3 {
                move_toward(p, &*world, there, STRIDE_M_S, dt);
            } else {
                p.place.speed = 0.0;
                p.place.yaw = yaw_toward(p.place.pos, there);
            }
        }
        // One who has stepped in comes over and talks them round.
        let middle = (at_a + at_b) * 0.5;
        if let Some(m) = q.mediator {
            let x = self.index_of_person(m).filter(|&x| {
                able(&self.persons[x])
                    && matches!(self.persons[x].mind.doing, Doing::Mediating { .. })
            });
            match x {
                None => self.quarrels[k].mediator = None,
                Some(x) => {
                    let p = &mut self.persons[x];
                    p.mind.timer = 2.0;
                    if (p.place.pos - middle).length() > 2.5 {
                        move_toward(p, &*world, middle, STRIDE_M_S, dt);
                    } else {
                        p.place.speed = 0.0;
                        p.place.yaw = yaw_toward(p.place.pos, at_a);
                        self.quarrels[k].talked += dt;
                        if self.quarrels[k].talked >= MEDIATE_S {
                            return Some(Settled::Mediated(m));
                        }
                    }
                }
            }
        }
        let held = q.held + dt;
        self.quarrels[k].held = held;
        // Once a second, how it goes.
        let second = held.floor() as u32;
        if second <= q.rolled {
            return None;
        }
        self.quarrels[k].rolled = second;
        if q.rung == Rung::Blows {
            for (x, by) in [(j, i), (i, j)] {
                if self.persons[by].rng.next_f32() < 0.3 {
                    self.bruise(x, species, now);
                }
            }
        }
        // One may step in: of standing, or kin to both; near, and at odds with neither.
        if self.quarrels[k].mediator.is_none()
            && !player
            && let Some(x) = self.mediator_for(i, j, middle, now)
        {
            let coop = self.persons[x].psyche.tendency(Tendency::Cooperativeness);
            if self.persons[x].rng.next_f32() < 0.1 * (0.5 + coop) {
                self.quarrels[k].mediator = Some(self.persons[x].id);
                let p = &mut self.persons[x];
                p.mind.doing = Doing::Mediating { a: q.a, b: q.b };
                p.mind.timer = 2.0;
            }
        }
        // The one in the wrong may make amends with food it carries, the fair and the guilty
        // the more readily.
        if self.persons[j].player.is_none()
            && carried_food(&self.persons[j], items, content).is_some()
        {
            let s = &self.persons[j].psyche;
            let fair = (s.value(Value::Fairness) + s.feeling(Feeling::Guilt) + 1.0
                - s.tendency(Tendency::Dominance))
                / 3.0;
            if self.persons[j].rng.next_f32() < 0.1 * fair
                && self.hand_over(j, i, species, items, content, now)
            {
                return Some(Settled::Amends);
            }
        }
        // The weaker may back down, the readier the less dominant and the more afraid.
        let force = |p: &Person, other: PersonId| {
            p.psyche.tendency(Tendency::Dominance) + 0.3 * p.psyche.value(Value::Courage)
                - p.social
                    .ties
                    .iter()
                    .find(|t| t.who == other)
                    .map_or(0.0, |t| t.fear)
        };
        let (weak, strong) = if force(&self.persons[i], q.b) < force(&self.persons[j], q.a) {
            (i, j)
        } else {
            (j, i)
        };
        let weak_id = self.persons[weak].id;
        if self.persons[weak].player.is_none() {
            let w = &self.persons[weak];
            let other = self.persons[strong].id;
            let fear = w
                .social
                .ties
                .iter()
                .find(|t| t.who == other)
                .map_or(0.0, |t| t.fear);
            let ready = 0.05 * (1.0 - w.psyche.tendency(Tendency::Dominance) + fear).min(1.0);
            if self.persons[weak].rng.next_f32() < ready {
                return Some(Settled::BackedDown(weak_id));
            }
        }
        // Its rung run out: on to the next, or over — the hotter their tempers, the likelier
        // the rise.
        let heat = |p: &Person| {
            p.psyche.feeling(Feeling::Anger)
                * (1.2 - p.psyche.tendency(Tendency::AggressionThreshold))
        };
        let hot = heat(&self.persons[i]).max(heat(&self.persons[j]));
        let roll = self.persons[i].rng.next_f32();
        let up = |people: &mut People, rung: Rung| {
            let q = &mut people.quarrels[k];
            q.rung = rung;
            q.held = 0.0;
            q.rolled = 0;
        };
        match q.rung {
            Rung::Argument if held >= ways.argue_s => {
                if roll < 0.5 * hot {
                    up(self, Rung::Threat);
                    None
                } else {
                    Some(Settled::TalkedOut)
                }
            }
            Rung::Threat if held >= ways.threat_s => {
                if player {
                    Some(Settled::TalkedOut)
                } else if roll < 0.35 * hot {
                    up(self, Rung::Blows);
                    None
                } else {
                    Some(Settled::BackedDown(weak_id))
                }
            }
            Rung::Blows if held >= ways.blows_s => Some(Settled::Fought),
            _ => None,
        }
    }

    /// The one best placed to step into a quarrel between `i` and `j`: grown, of either's band,
    /// near, at odds with neither, kin to both or well respected in its band.
    fn mediator_for(&self, i: usize, j: usize, middle: DVec3, now: &Now) -> Option<usize> {
        let (a, b) = (self.persons[i].id, self.persons[j].id);
        let bands = [self.persons[i].social.band, self.persons[j].social.band];
        let mut best: Option<(usize, f32)> = None;
        for (x, m) in self.persons.iter().enumerate() {
            if x == i
                || x == j
                || !able(m)
                || m.player.is_some()
                || m.age(now) < GROWN
                || !bands.contains(&m.social.band)
                || (m.place.pos - middle).length() > MEDIATE_M
                || matches!(
                    m.mind.doing,
                    Doing::Quarrelling { .. } | Doing::Mediating { .. }
                )
                || self.quarrelling(m.id)
            {
                continue;
            }
            if m.social
                .ties
                .iter()
                .any(|t| (t.who == a || t.who == b) && t.rivalry >= GRUDGE)
            {
                continue;
            }
            let kin = kin_of(self, m.id, a).is_some() && kin_of(self, m.id, b).is_some();
            let Some(bi) = self.bands.iter().position(|bd| bd.id == m.social.band) else {
                continue;
            };
            let standing = self.standing(bi, m.id, now);
            let weight = standing + if kin { 0.3 } else { 0.0 };
            if (kin || standing >= 0.4) && best.is_none_or(|w| weight > w.1) {
                best = Some((x, weight));
            }
        }
        best.map(|(x, _)| x)
    }

    /// One unit of the food `from` carries handed to `to` (into its keeping, if it has room).
    /// Whether it was.
    fn hand_over(
        &mut self,
        from: usize,
        to: usize,
        species: &SpeciesSet,
        items: &Items,
        content: &Content,
        now: &Now,
    ) -> bool {
        let Some(path) = carried_food(&self.persons[from], items, content) else {
            return false;
        };
        let Some(stack) = self.persons[from]
            .possessions
            .carry
            .take(items, &path, Some(1))
        else {
            return false;
        };
        let worth = items
            .get(&stack.id)
            .and_then(|k| hearth_craft::food::bite_of(content, k, &stack))
            .map_or(0.1, |b| (b.kcal / 2000.0).max(0.05));
        let mass = species
            .get(&self.persons[to].species)
            .map_or(60.0, |sp| self.persons[to].mass_kg(sp, now));
        let given = self.persons[to].possessions.carry.stow(items, stack, mass);
        if let Err(stack) = given {
            // No room: kept.
            let mass = species
                .get(&self.persons[from].species)
                .map_or(60.0, |sp| self.persons[from].mass_kg(sp, now));
            let _ = self.persons[from]
                .possessions
                .carry
                .stow(items, stack, mass);
            return false;
        }
        let (a, b) = (self.persons[from].id, self.persons[to].id);
        self.give(a, b, worth, now.day);
        true
    }

    /// A blow landed: a bruise on an arm, the chest, a leg, now and then the head.
    fn bruise(&mut self, x: usize, species: &SpeciesSet, now: &Now) {
        let Some(sp) = species.get(&self.persons[x].species) else {
            return;
        };
        let p = &mut self.persons[x];
        let cfg = species_body(p, sp, now);
        let r = p.rng.next_f32();
        let (region, paired) = if r < 0.35 {
            (BodyRegion::UpperArm, true)
        } else if r < 0.6 {
            (BodyRegion::Chest, false)
        } else if r < 0.8 {
            (BodyRegion::LowerArm, true)
        } else if r < 0.92 {
            (BodyRegion::UpperLeg, true)
        } else {
            (BodyRegion::Head, false)
        };
        let side = if !paired {
            hearth_body::Side::Middle
        } else if p.rng.next_f32() < 0.5 {
            hearth_body::Side::Left
        } else {
            hearth_body::Side::Right
        };
        let severity = 0.1 + 0.2 * p.rng.next_f32();
        let _ = p.body.injure(&cfg, "bruise", region, side, severity);
    }

    /// A quarrel over: the grievance eased as far as the ending eases it, each its own way.
    fn settle(&mut self, q: Quarrel, how: Settled, day: f64) {
        let ease = match how {
            Settled::Mediated(_) => 0.4,
            Settled::Amends => 0.3,
            Settled::Fought => 0.7,
            Settled::BackedDown(_) => 0.85,
            Settled::TalkedOut => 0.8,
            Settled::Parted => 0.9,
        };
        let mended = matches!(how, Settled::Mediated(_) | Settled::Amends);
        for (x, other) in [(q.a, q.b), (q.b, q.a)] {
            let Some(i) = self.index_of_person(x) else {
                continue;
            };
            let k = self.tie_index(i, other, day);
            let p = &mut self.persons[i];
            let t = &mut p.social.ties[k];
            t.quarrels = t.quarrels.saturating_add(1);
            t.quarrelled = Some(day);
            t.rivalry *= ease;
            if mended {
                t.affection = (t.affection + 0.05).min(1.0);
            }
            p.psyche.feelings[Feeling::Anger] *= if mended { 0.3 } else { 0.6 };
            if matches!(p.mind.doing, Doing::Quarrelling { .. }) {
                p.mind.doing = Doing::Idle;
                p.mind.timer = 0.0;
            }
        }
        if let Some(m) = q.mediator
            && let Some(x) = self.index_of_person(m)
            && matches!(self.persons[x].mind.doing, Doing::Mediating { .. })
        {
            self.persons[x].mind.doing = Doing::Idle;
            self.persons[x].mind.timer = 0.0;
        }
        match how {
            Settled::BackedDown(w) => {
                // It fears the other the more, and goes off.
                let other = if w == q.a { q.b } else { q.a };
                if let (Some(i), Some(o)) = (self.index_of_person(w), self.index_of_person(other)) {
                    let from = self.persons[o].place.pos;
                    let k = self.tie_index(i, other, day);
                    let p = &mut self.persons[i];
                    let t = &mut p.social.ties[k];
                    t.fear = (t.fear + 0.15).min(1.0);
                    if p.player.is_none() {
                        let away = (p.place.pos - from).normalize_or(DVec3::X);
                        p.mind.doing = Doing::Going {
                            to: p.place.pos + away * OFF_M,
                            then: Intent::Roam,
                        };
                        p.mind.timer = 30.0;
                    }
                }
            }
            Settled::Mediated(m) => {
                if let Some(x) = self.index_of_person(m) {
                    let at = self.persons[x].place.pos;
                    let p = &mut self.persons[x];
                    p.record(day, Event::Mediated { a: q.a, b: q.b });
                    p.psyche.feel(Feeling::Pride, 0.4);
                    self.respect(m, at, 0.1, day);
                }
            }
            Settled::Amends => {
                if let Some(j) = self.index_of_person(q.b) {
                    self.persons[j].record(day, Event::MadeAmends { to: q.a });
                }
            }
            Settled::Fought => {
                // The worse of it fears the other the more; the better has spent its grudge.
                let (Some(i), Some(j)) = (self.index_of_person(q.a), self.index_of_person(q.b))
                else {
                    return;
                };
                let force = |p: &mut Person| {
                    p.psyche.tendency(Tendency::Dominance)
                        + 0.3 * p.psyche.value(Value::Courage)
                        + 0.5 * p.rng.next_f32()
                };
                let (fa, fb) = (force(&mut self.persons[i]), force(&mut self.persons[j]));
                let (loser, winner) = if fa < fb { (i, j) } else { (j, i) };
                let (lid, wid) = (self.persons[loser].id, self.persons[winner].id);
                self.persons[i].record(day, Event::Fought { with: q.b });
                self.persons[j].record(day, Event::Fought { with: q.a });
                let k = self.tie_index(loser, wid, day);
                let t = &mut self.persons[loser].social.ties[k];
                t.fear = (t.fear + 0.3).min(1.0);
                let k = self.tie_index(winner, lid, day);
                self.persons[winner].social.ties[k].rivalry *= 0.6;
            }
            Settled::TalkedOut | Settled::Parted => {}
        }
    }

    /// About monthly, a band's worst feud — deep rivalry after quarrel upon quarrel — if it
    /// has come to that, parts it: the weaker side (the less respected, else the younger)
    /// leaves with its household. A household with a player in it stays (the other leaves).
    pub(crate) fn feuds(&mut self, bi: usize, ways: &Ways, now: &Now) {
        let members = self.band_members(bi);
        let mut worst: Option<(usize, usize, f32)> = None;
        for &i in &members {
            for t in &self.persons[i].social.ties {
                if t.rivalry < ways.leave_rivalry || t.quarrels < ways.leave_quarrels {
                    continue;
                }
                if let Some(&j) = members.iter().find(|&&j| self.persons[j].id == t.who)
                    && worst.is_none_or(|w| t.rivalry > w.2)
                {
                    worst = Some((i, j, t.rivalry));
                }
            }
        }
        let Some((i, j, _)) = worst else {
            return;
        };
        let (si, sj) = (
            self.standing(bi, self.persons[i].id, now),
            self.standing(bi, self.persons[j].id, now),
        );
        let younger_i = self.persons[i].life.born > self.persons[j].life.born;
        let (first, second) = if si < sj || (si == sj && younger_i) {
            (i, j)
        } else {
            (j, i)
        };
        if !self.leave(bi, first, now.day) {
            self.leave(bi, second, now.day);
        }
    }

    /// A person leaves its band after a feud, its household with it, for a band of their own
    /// some way off. Whether they went (not with a player among them).
    fn leave(&mut self, bi: usize, i: usize, day: f64) -> bool {
        let household = self.persons[i].social.household;
        let going: Vec<usize> = match household {
            Some(h) => self
                .band_members(bi)
                .into_iter()
                .filter(|&x| self.persons[x].social.household == Some(h))
                .collect(),
            None => vec![i],
        };
        if going.iter().any(|&x| self.persons[x].player.is_some()) {
            return false;
        }
        let id = 2_000_000_000 + self.take_id();
        let from = self.bands[bi].id;
        let rng = &mut self.bands[bi].rng;
        let a = rng.next_f64() * std::f64::consts::TAU;
        let d = LEAVE_M * (1.0 + rng.next_f64());
        let mut band = self.bands[bi].clone();
        band.id = id;
        band.home += DVec2::new(a.cos(), a.sin()) * d;
        band.population_group = None;
        band.lived_to = Some(day);
        band.council = None;
        band.weighed = 0.0;
        band.guests = Vec::new();
        band.rng = crate::sim::band_stream(self.seed, id);
        // They make for their new country, and keep camp there.
        let y = self.persons[i].place.pos.y;
        band.camp = Some(DVec3::new(band.home.x, y, band.home.y));
        band.members = going.iter().map(|&x| self.persons[x].id).collect();
        let moved = band.members.clone();
        self.bands[bi].members.retain(|m| !moved.contains(m));
        for &x in &going {
            let p = &mut self.persons[x];
            p.social.band = id;
            p.record(day, Event::Left { from });
        }
        self.bands.push(band);
        true
    }
}

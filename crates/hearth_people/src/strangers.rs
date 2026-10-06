//! Strangers (V2.1 §8.7; H4). One not of a band whom its people do not know, or do not yet
//! trust, is watched as it comes, the young called in to their mothers; one who comes openly is
//! met by one of the grown and greeted — a guest now, with the first trust the people's ways give
//! a greeting. A guest is let near and fed when hungry as their own are; it comes to be known by
//! its doings, slowly by time spent near them and above all by gifts (a gift draws the one given
//! it closer, sure the giver is generous, and its band the readier to have a player near); and
//! it is taken in when most of the grown trust it after days among them. Where their country is
//! crowded, or the stranger has a bad name with them or a grievance holds, it is warned off
//! instead, and one who stays is threatened, as in a quarrel. A child is never warned off.

use glam::DVec3;
use hearth_content::Content;
use hearth_items::{Items, Stack};
use serde::{Deserialize, Serialize};

use crate::conflict::{GROWN, Ways};
use crate::memory::Who;
use crate::person::{Event, PersonId};
use crate::psyche::Feeling;
use crate::repute::{Deed, Seen};
use crate::sim::People;
use crate::species::SpeciesSet;
use crate::speech::{Act, Gesture, words};
use crate::world::Now;

/// A stranger a band has met and greeted: who, the day, and who greeted it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Guest {
    pub who: PersonId,
    pub since: f64,
    pub by: PersonId,
}

/// Trust below which one not of the band is a stranger to a person.
pub const STRANGER_TRUST: f32 = 0.3;
/// How far a guest may go from its hosts and stay their guest (m).
const GUEST_M: f64 = 2_000.0;
/// The share of the grown who must trust a guest for it to be taken in.
const TAKE_IN_SHARE: f32 = 0.6;

impl People {
    /// `i` meets the stranger `j` with a greeting: the first trust between them their ways
    /// give it, and `j` its band's guest.
    pub(crate) fn greet(&mut self, i: usize, j: usize, ways: &Ways, day: f64) {
        let (me, them) = (self.persons[i].id, self.persons[j].id);
        for (x, other) in [(i, them), (j, me)] {
            let k = self.tie_index(x, other, day);
            let t = &mut self.persons[x].social.ties[k];
            t.trust = t.trust.max(ways.greeting_trust);
            t.affection = (t.affection + 0.05).min(1.0);
            t.day = day;
            let p = &mut self.persons[x];
            p.psyche.feel(Feeling::Joy, 0.2);
            p.record(day, Event::Greeted { who: other });
        }
        let band = self.persons[i].social.band;
        let player = self.persons[j].player;
        if let Some(b) = self.bands.iter_mut().find(|b| b.id == band) {
            if !b.guests.iter().any(|g| g.who == them) {
                b.guests.push(Guest {
                    who: them,
                    since: day,
                    by: me,
                });
            }
            if let Some(pl) = player {
                let t = b.tolerance_of(pl);
                b.set_tolerance(pl, t.max(0.75));
            }
        }
        // The greeting in words, and as their culture greets.
        use hearth_content::schema::culture::Greeting;
        let gesture = self
            .bands
            .iter()
            .find(|b| b.id == band && b.culture.drawn())
            .map(|b| match b.culture.greeting {
                Greeting::Embrace => Gesture::Embrace,
                Greeting::Hands => Gesture::Hands,
                Greeting::Call => Gesture::Beckon,
                Greeting::Gift => Gesture::Offer,
            });
        self.say(
            me,
            Some(them),
            Act::Greet,
            words(&["hello", "friend"]),
            gesture,
            day,
        );
        self.say(them, Some(me), Act::Greet, words(&["hello"]), gesture, day);
    }

    /// Whether a person is a guest of a band.
    pub fn guest_of(&self, band: u64, who: PersonId) -> bool {
        self.bands
            .iter()
            .any(|b| b.id == band && b.guests.iter().any(|g| g.who == who))
    }

    /// A gift: `from` hands `to` a thing, which it takes into its keeping (given back if it has
    /// no room). The ledger moves, and more: the one given it is the fonder and the more
    /// trusting of the giver and sure it is generous, its band the readier to have a player
    /// near; those who see it think the better of the giver.
    #[allow(clippy::too_many_arguments)]
    pub fn gift(
        &mut self,
        from: PersonId,
        to: PersonId,
        stack: Stack,
        species: &SpeciesSet,
        items: &Items,
        content: &Content,
        now: Now,
    ) -> Result<(), Stack> {
        let (Some(a), Some(b)) = (self.index_of_person(from), self.index_of_person(to)) else {
            return Err(stack);
        };
        if a == b || !self.persons[b].alive() {
            return Err(stack);
        }
        let count = stack.count.max(1) as f32;
        let worth = items
            .get(&stack.id)
            .and_then(|k| hearth_craft::food::bite_of(content, k, &stack))
            .map_or(0.3, |bite| (bite.kcal / 2000.0).max(0.05) * count)
            .min(2.0);
        let mass = species
            .get(&self.persons[b].species)
            .map_or(60.0, |sp| self.persons[b].mass_kg(sp, &now));
        self.persons[b].possessions.carry.stow(items, stack, mass)?;
        let q = &mut self.persons[b];
        q.psyche.feel(Feeling::Joy, 0.3);
        q.psyche.feel(Feeling::Affection, 0.3);
        q.record(now.day, Event::GiftFrom { who: from });
        self.give(from, to, worth, now.day);
        self.say(
            to,
            Some(from),
            Act::Thank,
            words(&["thanks"]),
            None,
            now.day,
        );
        let k = self.tie_index(b, from, now.day);
        let t = &mut self.persons[b].social.ties[k];
        t.trust = (t.trust + 0.15 * (1.0 - t.trust)).min(1.0);
        t.affection = (t.affection + 0.1 * (1.0 - t.affection)).min(1.0);
        let k = self.repute_index(b, Who::Person(from), now.day);
        let r = &mut self.persons[b].social.reputes[k];
        r.generous = (r.generous + 0.25).min(1.0);
        r.sure = 1.0;
        r.day = now.day;
        if let Some(pl) = self.persons[a].player {
            let band = self.persons[b].social.band;
            if let Some(bd) = self.bands.iter_mut().find(|x| x.id == band) {
                let t = bd.tolerance_of(pl);
                bd.set_tolerance(pl, t + 0.2);
            }
        }
        let at = self.persons[a].place.pos;
        self.deeds.push(Seen {
            who: Who::Person(from),
            deed: Deed::Shared { with: to },
            at,
        });
        Ok(())
    }

    /// A band's evening weighing of its guests: one gone off or dead is a guest no more; one
    /// among them long enough whom most of the grown trust is taken in. Those taken in.
    pub fn weigh_guests(&mut self, bi: usize, ways: &Ways, now: &Now) -> Vec<PersonId> {
        let band = self.bands[bi].id;
        let members = self.band_members(bi);
        let grown: Vec<usize> = members
            .iter()
            .copied()
            .filter(|&i| self.persons[i].age(now) >= GROWN && self.persons[i].player.is_none())
            .collect();
        let centre = self.bands[bi].camp.or_else(|| {
            (!members.is_empty()).then(|| {
                members
                    .iter()
                    .map(|&i| self.persons[i].place.pos)
                    .sum::<DVec3>()
                    / members.len() as f64
            })
        });
        let mut kept = Vec::new();
        let mut taken = Vec::new();
        for g in self.bands[bi].guests.clone() {
            let Some(j) = self.index_of_person(g.who) else {
                continue;
            };
            let q = &self.persons[j];
            if !q.alive() || q.social.band == band {
                continue;
            }
            if centre.is_some_and(|c| (q.place.pos - c).length() > GUEST_M) {
                continue;
            }
            let trusting = grown
                .iter()
                .filter(|&&i| {
                    self.persons[i]
                        .social
                        .ties
                        .iter()
                        .any(|t| t.who == g.who && t.trust >= ways.take_in_trust)
                })
                .count();
            let long = now.day - g.since >= ways.take_in_days;
            if long && !grown.is_empty() && trusting as f32 / grown.len() as f32 >= TAKE_IN_SHARE {
                taken.push((g.who, g.by));
            } else {
                kept.push(g);
            }
        }
        self.bands[bi].guests = kept;
        for &(who, by) in &taken {
            self.take_in(bi, who, now.day);
            // The one who greeted it tells it so.
            let stay = words(&["yes", "stay", "here"]);
            self.say(
                by,
                Some(who),
                Act::Accept,
                stay,
                Some(Gesture::Embrace),
                now.day,
            );
        }
        taken.into_iter().map(|(who, _)| who).collect()
    }

    /// A guest taken into a band: one of them now, knowing them all.
    fn take_in(&mut self, bi: usize, who: PersonId, day: f64) {
        let Some(j) = self.index_of_person(who) else {
            return;
        };
        let from = self.persons[j].social.band;
        if let Some(old) = self.bands.iter_mut().find(|b| b.id == from) {
            old.members.retain(|m| *m != who);
        }
        let band = self.bands[bi].id;
        let text = format!("{} is taken into a band.", self.called(who));
        let at = self.persons[j].place.pos;
        self.note(day, text, at);
        self.bands[bi].members.push(who);
        let p = &mut self.persons[j];
        p.social.band = band;
        p.social.household = None;
        p.psyche.feel(Feeling::Joy, 0.5);
        p.record(day, Event::TakenIn { band });
        if let Some(pl) = p.player {
            self.bands[bi].set_tolerance(pl, 1.0);
        }
        self.settle_households(bi);
        self.acquaint(bi, day);
    }
}

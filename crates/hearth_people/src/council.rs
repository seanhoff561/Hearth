//! Status and group decisions (V2.1 §8.5; H4). A person's standing in its band is the respect
//! its grown members hold it in — earned by skill seen at work, by generosity, by age. Where the
//! band keeps camp is decided together: in the evening, when the camp's country has gone poor
//! (hunger among them, no food or water remembered near) the grown each argue for the place their
//! own memory and interests favour — food and water there, danger, how far to go, the more for
//! those who carry young or are old — then come round, the conforming toward the many and those
//! who heed prestige toward the respected, until most agree; and the band moves its camp there.
//! Without agreement it stays.

use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

use crate::memory::PlaceKind;
use crate::mind::{Doing, Intent};
use crate::person::{Event, PersonId, Tier};
use crate::psyche::Tendency;
use crate::sim::People;
use crate::world::Now;

/// A council held, as the band remembers it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Council {
    pub day: f64,
    /// The places argued for, and how many came to support each.
    pub options: Vec<(DVec3, u16)>,
    /// Where they agreed to go (the camp they kept, if they stayed).
    pub chosen: Option<DVec3>,
    /// The rounds of talk it took.
    pub rounds: u8,
}

/// Places within this of one another are one place (m).
const PLACE_M: f64 = 150.0;
/// Water this near a place serves it (m).
const WATER_M: f64 = 300.0;
/// How far a camp's country reaches for its food (m).
const COUNTRY_M: f64 = 200.0;
/// The most rounds of talk.
const ROUNDS: u8 = 6;
/// The share that must agree.
const AGREE: f32 = 0.6;
/// Days a band gives a camp its round has made before it may hold a council on it.
const SETTLING_DAYS: f64 = 10.0;

impl People {
    /// How a band's grown hold one of them, on the whole: the mean of the respect in their ties
    /// to it (0–1).
    pub fn standing(&self, bi: usize, who: PersonId, now: &Now) -> f32 {
        let (mut sum, mut n) = (0.0, 0.0);
        for i in self.band_members(bi) {
            let p = &self.persons[i];
            if p.id == who || p.age(now) < 15.0 {
                continue;
            }
            n += 1.0;
            sum += p
                .social
                .ties
                .iter()
                .find(|t| t.who == who)
                .map_or(0.3, |t| t.respect);
        }
        if n > 0.0 { sum / n } else { 0.3 }
    }

    /// Respect for one seen doing well: its skill at work, its generosity.
    pub(crate) fn respect(&mut self, seen: PersonId, at: DVec3, by: f32, day: f64) {
        let witnesses: Vec<usize> = (0..self.persons.len())
            .filter(|&i| {
                let p = &self.persons[i];
                p.tier == Tier::Full
                    && p.alive()
                    && p.id != seen
                    && (p.place.pos - at).length() < 30.0
            })
            .collect();
        for i in witnesses {
            let k = self.tie_index(i, seen, day);
            let t = &mut self.persons[i].social.ties[k];
            t.respect = (t.respect + by * (1.0 - t.respect)).min(1.0);
        }
    }

    /// Whether a band's camp has gone poor: many of its grown hungry, or none of them remembering
    /// food or water about it.
    fn camp_is_poor(
        &self,
        bi: usize,
        camp: DVec3,
        now: &Now,
        hungry: &dyn Fn(usize) -> bool,
    ) -> bool {
        let grown: Vec<usize> = self
            .band_members(bi)
            .into_iter()
            .filter(|&i| self.persons[i].age(now) >= 15.0)
            .collect();
        if grown.is_empty() {
            return false;
        }
        let hungry_share = grown.iter().filter(|&&i| hungry(i)).count() as f32 / grown.len() as f32;
        let remembered = |kind: PlaceKind, within: f64| {
            grown.iter().any(|&i| {
                self.persons[i]
                    .memory
                    .places
                    .iter()
                    .any(|r| r.kind == kind && r.sure > 0.3 && (r.at - camp).length() < within)
            })
        };
        hungry_share > 0.5
            || !remembered(PlaceKind::Food, COUNTRY_M)
            || !remembered(PlaceKind::Water, WATER_M)
    }

    /// A council of a band's grown on where to keep camp (see the module). Whether they moved.
    pub fn council(&mut self, bi: usize, now: Now, hungry: &dyn Fn(usize) -> bool) -> bool {
        let members: Vec<usize> = self
            .band_members(bi)
            .into_iter()
            .filter(|&i| {
                let p = &self.persons[i];
                p.tier == Tier::Full && p.player.is_none() && p.age(&now) >= 15.0
            })
            .collect();
        if members.len() < 2 {
            return false;
        }
        let centre = members
            .iter()
            .map(|&i| self.persons[i].place.pos)
            .sum::<DVec3>()
            / members.len() as f64;
        // A band that keeps no camp yet keeps it where its people are. A camp just made for the
        // season or at a gathering is given time to be learned (H8).
        let camp = *self.bands[bi].camp.get_or_insert(centre);
        let since = self.bands[bi].round.since;
        if self.gathered(bi, now.day).is_some() || (since > 0.0 && now.day - since < SETTLING_DAYS)
        {
            return false;
        }
        if !self.camp_is_poor(bi, camp, &now, hungry) {
            return false;
        }
        // The places argued for: the camp, and the food places the grown remember, as one place
        // where they lie close.
        let mut options: Vec<DVec3> = vec![camp];
        for &i in &members {
            for r in &self.persons[i].memory.places {
                if r.kind == PlaceKind::Food
                    && r.sure > 0.3
                    && options.iter().all(|o| (*o - r.at).length() > PLACE_M)
                {
                    options.push(r.at);
                }
            }
        }
        options.truncate(6);
        // What a place offers as one remembers it: its food and water, less its danger.
        let offers = |people: &People, i: usize, o: DVec3| -> f32 {
            let p = &people.persons[i];
            let sure = |kind: PlaceKind, within: f64| -> f32 {
                p.memory
                    .places
                    .iter()
                    .filter(|r| r.kind == kind && (r.at - o).length() < within)
                    .map(|r| r.sure)
                    .sum::<f32>()
                    .min(2.0)
            };
            sure(PlaceKind::Food, PLACE_M) + 0.8 * sure(PlaceKind::Water, WATER_M).min(1.0)
                - 1.5 * sure(PlaceKind::Danger, WATER_M)
        };
        // Each one's own reckoning of each place: what it offers, as remembered or as heard
        // argued for it (`heard`), the way there and the staying.
        let worth = |people: &People, i: usize, o: DVec3, heard: f32| -> f32 {
            let p = &people.persons[i];
            // Those who carry young or are old weigh the way the more.
            let burdened = people
                .persons
                .iter()
                .any(|c| c.alive() && c.life.mother == Some(p.id) && c.age(&now) < 4.0)
                || p.age(&now) >= 55.0;
            let way =
                ((o - p.place.pos).length() / 1000.0) as f32 * if burdened { 0.6 } else { 0.3 };
            let stay = if (o - camp).length() < 1.0 {
                if hungry(i) { 0.0 } else { 0.25 }
            } else {
                0.0
            };
            offers(people, i, o).max(heard) - way + stay
        };
        let mut support: Vec<usize> = members
            .iter()
            .map(|&i| {
                (0..options.len())
                    .max_by(|&a, &b| {
                        worth(self, i, options[a], 0.0)
                            .total_cmp(&worth(self, i, options[b], 0.0))
                            .then(b.cmp(&a))
                    })
                    .unwrap_or(0)
            })
            .collect();
        // Each says where it would keep camp: here, or where the food is.
        for (k, &i) in members.iter().enumerate() {
            use crate::speech::{Act, Gesture, clause};
            let me = self.persons[i].id;
            let order = self.order_of(me);
            let case = if (options[support[k]] - camp).length() < 1.0 {
                clause(order, Some("we"), Some("stay"), None, &["here"])
            } else {
                clause(order, Some("we"), Some("go"), None, &["food", "there"])
            };
            self.say(me, None, Act::ArgueFor, case, Some(Gesture::Point), now.day);
        }
        let standing: Vec<f32> = members
            .iter()
            .map(|&i| self.standing(bi, self.persons[i].id, &now))
            .collect();
        let mut rounds = 0;
        loop {
            let n = members.len() as f32;
            let count = |o: usize, support: &[usize]| support.iter().filter(|s| **s == o).count();
            let top = (0..options.len())
                .max_by_key(|&o| (count(o, &support), std::cmp::Reverse(o)))
                .unwrap_or(0);
            if count(top, &support) as f32 / n >= AGREE || rounds >= ROUNDS {
                break;
            }
            rounds += 1;
            // Each place's case, as its supporters make it: what they remember it offers, heard
            // at six tenths of what one knows oneself.
            let before = support.clone();
            let case: Vec<f32> = (0..options.len())
                .map(|o| {
                    let backers: Vec<f32> = members
                        .iter()
                        .zip(&before)
                        .filter(|(_, s)| **s == o)
                        .map(|(&i, _)| offers(self, i, options[o]))
                        .collect();
                    if backers.is_empty() {
                        0.0
                    } else {
                        0.6 * backers.iter().sum::<f32>() / backers.len() as f32
                    }
                })
                .collect();
            // Each comes round a little: toward the case made best, toward the many (the
            // conforming), toward the respected (those who heed prestige).
            for (k, &i) in members.iter().enumerate() {
                let p = &self.persons[i];
                let conform = p.psyche.tendency(Tendency::Conformity);
                let prestige = p.psyche.tendency(Tendency::PrestigeBias);
                support[k] = (0..options.len())
                    .max_by(|&a, &b| {
                        let pull = |o: usize| {
                            let many = count(o, &before) as f32 / n;
                            let respected: f32 = before
                                .iter()
                                .zip(&standing)
                                .filter(|(s, _)| **s == o)
                                .map(|(_, st)| *st)
                                .sum::<f32>()
                                / n;
                            worth(self, i, options[o], case[o])
                                + 1.2 * conform * many * rounds as f32
                                + 1.2 * prestige * respected * rounds as f32
                        };
                        pull(a).total_cmp(&pull(b)).then(b.cmp(&a))
                    })
                    .unwrap_or(0);
            }
        }
        let n = members.len() as f32;
        let tally: Vec<(DVec3, u16)> = options
            .iter()
            .enumerate()
            .map(|(o, at)| (*at, support.iter().filter(|s| **s == o).count() as u16))
            .collect();
        let (best, votes) = tally
            .iter()
            .enumerate()
            .max_by_key(|(o, (_, c))| (*c, std::cmp::Reverse(*o)))
            .map(|(o, (_, c))| (o, *c))
            .unwrap_or((0, 0));
        let agreed = votes as f32 / n >= AGREE || (rounds >= ROUNDS && votes as f32 / n > 0.5);
        let chosen = agreed.then_some(options[best]);
        self.bands[bi].council = Some(Council {
            day: now.day,
            options: tally,
            chosen,
            rounds,
        });
        let Some(to) = chosen.filter(|to| (*to - camp).length() > 1.0) else {
            self.bands[bi].camp = Some(camp);
            return false;
        };
        // The band moves its camp there, all of it.
        let kind = crate::notable::technique(&self.bands[bi].species);
        self.note(now.day, format!("A band of {kind} moves its camp."), to);
        self.bands[bi].camp = Some(to);
        self.bands[bi].home = DVec2::new(to.x, to.z);
        let band = self.bands[bi].id;
        for i in self.band_members(bi) {
            let p = &mut self.persons[i];
            if p.player.is_some() {
                continue;
            }
            p.mind.doing = Doing::Going {
                to,
                then: Intent::Rejoin,
            };
            p.mind.timer = 120.0;
            p.record(now.day, Event::MovedCamp { band });
        }
        true
    }
}

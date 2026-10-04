//! Relationships and obligations (V2.1 §8.2; H4). Each person's ties to those they know: how
//! fond of them, how trusting, how respectful, how afraid, how rivalrous, and the ledger between
//! them — help and food given, and owed. Kin begin close; time together and grooming draw people
//! closer, what passes between them moves the ledger, and ties fade back toward where they began
//! without contact. A person keeps a social world of at most about a hundred and fifty (Dunbar's
//! number), the weakest let go.

use serde::{Deserialize, Serialize};

use crate::kin::{Kin, kin_of};
use crate::person::{PersonId, Tier};
use crate::sim::People;

/// A tie to another person.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tie {
    pub who: PersonId,
    /// 0–1 each.
    pub affection: f32,
    pub trust: f32,
    pub respect: f32,
    pub fear: f32,
    pub rivalry: f32,
    /// Where affection and trust settle without contact: what kinship and a shared band give.
    pub base: f32,
    /// The ledger: what one has given them (help, food, gifts) and what one owes them for what
    /// they gave, in a common measure (a day's food about 1).
    pub given: f32,
    pub owed: f32,
    /// The day of the world they were last together.
    pub day: f64,
}

/// The most ties a person keeps.
pub const WORLD: usize = 150;
/// How close kin begin, and those of one's band who are not kin, and others.
fn base_of(kin: Option<Kin>, same_band: bool) -> f32 {
    match kin {
        Some(Kin::Partner) | Some(Kin::Parent) | Some(Kin::Child) => 0.75,
        Some(Kin::Sibling) => 0.6,
        Some(Kin::HalfSibling) | Some(Kin::Grandparent) | Some(Kin::Grandchild) => 0.5,
        Some(Kin::ParentsSibling) | Some(Kin::SiblingsChild) => 0.4,
        Some(Kin::ParentInLaw) | Some(Kin::ChildInLaw) | Some(Kin::SiblingInLaw) => 0.35,
        Some(Kin::Cousin) => 0.3,
        None if same_band => 0.15,
        None => 0.05,
    }
}

/// How fast time near another draws one closer (a share of the way left, a second of play).
const NEAR_RATE: f32 = 0.000_06;
/// How near is near (m).
const NEAR_M: f64 = 8.0;
/// Grooming another draws both closer the faster (a second of play).
const GROOM_RATE: f32 = 0.002;
/// Ties untended fade back to where they began: the share of the way a month of the life course
/// (four of its steps).
const FADE_A_WEEK: f32 = 0.078;
/// The ledger fades as well (old debts forgiven), more slowly.
const LEDGER_FADE_A_WEEK: f32 = 0.04;

impl Tie {
    fn new(who: PersonId, base: f32, day: f64) -> Self {
        Self {
            who,
            affection: base,
            trust: base,
            respect: 0.3,
            fear: 0.0,
            rivalry: 0.0,
            base,
            given: 0.0,
            owed: 0.0,
            day,
        }
    }

    /// How much the tie holds (which to keep when there are too many).
    pub fn strength(&self) -> f32 {
        self.affection + self.trust + 0.5 * self.respect + 0.3 * (self.given + self.owed).min(2.0)
    }

    /// Closer by a share of the way left.
    fn closer(&mut self, by: f32, day: f64) {
        self.affection += (1.0 - self.affection) * by;
        self.trust += (1.0 - self.trust) * by * 0.7;
        self.day = day;
    }
}

impl People {
    /// A person's tie to another, made if it is not (begun from their kinship and band).
    pub(crate) fn tie_index(&mut self, i: usize, other: PersonId, day: f64) -> usize {
        if let Some(k) = self.persons[i]
            .social
            .ties
            .iter()
            .position(|t| t.who == other)
        {
            return k;
        }
        let me = self.persons[i].id;
        let same_band = self
            .get(other)
            .is_some_and(|q| q.social.band == self.persons[i].social.band);
        let base = base_of(kin_of(self, me, other), same_band);
        let ties = &mut self.persons[i].social.ties;
        if ties.len() >= WORLD
            && let Some(weakest) = (0..ties.len()).min_by(|&a, &b| {
                ties[a]
                    .strength()
                    .total_cmp(&ties[b].strength())
                    .then(ties[a].day.total_cmp(&ties[b].day))
            })
        {
            ties.swap_remove(weakest);
        }
        ties.push(Tie::new(other, base, day));
        ties.len() - 1
    }

    /// The dead are let go from the ties of the living (their memory kept in what happened).
    pub(crate) fn forget_dead(&mut self, dead: PersonId) {
        for p in self.persons.iter_mut().filter(|p| p.alive()) {
            p.social.ties.retain(|t| t.who != dead);
        }
    }

    /// A band's living members all know one another (those born or come since included), each
    /// tie begun from their kinship.
    pub(crate) fn acquaint(&mut self, bi: usize, day: f64) {
        let members: Vec<(usize, PersonId)> = self
            .band_members(bi)
            .into_iter()
            .map(|i| (i, self.persons[i].id))
            .collect();
        for &(i, me) in &members {
            for &(_, other) in &members {
                if other != me && !self.persons[i].social.ties.iter().any(|t| t.who == other) {
                    self.tie_index(i, other, day);
                }
            }
        }
    }

    /// What one gives another (help, food: a day's food about 1): their ledgers moved, the one
    /// given to the fonder and the more trusting.
    pub fn give(&mut self, from: PersonId, to: PersonId, worth: f32, day: f64) {
        let (Ok(a), Ok(b)) = (
            self.persons.binary_search_by_key(&from, |p| p.id),
            self.persons.binary_search_by_key(&to, |p| p.id),
        ) else {
            return;
        };
        let k = self.tie_index(a, to, day);
        self.persons[a].social.ties[k].given += worth;
        let k = self.tie_index(b, from, day);
        let t = &mut self.persons[b].social.ties[k];
        t.owed += worth;
        t.closer(0.1 * worth.min(1.0), day);
    }

    /// Time together in a band lived in full: those near one another drawn closer, a groomer
    /// and the groomed much the more (`dt` seconds of play).
    pub(crate) fn keep_company(&mut self, dt: f32, day: f64) {
        let near: Vec<(usize, PersonId, u64, glam::DVec3, Option<u64>)> = self
            .persons
            .iter()
            .enumerate()
            .filter(|(_, p)| p.tier == Tier::Full && p.alive())
            .map(|(i, p)| {
                let grooming = match p.mind.doing {
                    crate::mind::Doing::Grooming { other } => other.or(Some(0)),
                    _ => None,
                };
                (i, p.id, p.social.band, p.place.pos, grooming)
            })
            .collect();
        for &(i, _, band, pos, grooming) in &near {
            for &(_, other, band_o, pos_o, _) in &near {
                if band_o != band || other == self.persons[i].id {
                    continue;
                }
                let d = (pos_o - pos).length();
                if d > NEAR_M {
                    continue;
                }
                let rate = match grooming {
                    Some(g) if g == other || (g == 0 && d < 2.0) => GROOM_RATE,
                    _ => NEAR_RATE,
                };
                let k = self.tie_index(i, other, day);
                self.persons[i].social.ties[k].closer(rate * dt, day);
            }
        }
    }

    /// A month of a band's life course for its members' ties (`step` days long): those untended
    /// through it fade back to where they began, and the ledgers slowly.
    pub(crate) fn fade_ties(&mut self, bi: usize, day: f64, step: f64) {
        for i in self.band_members(bi) {
            for t in &mut self.persons[i].social.ties {
                if day - t.day > step {
                    t.affection += (t.base - t.affection) * FADE_A_WEEK;
                    t.trust += (t.base - t.trust) * FADE_A_WEEK;
                }
                t.fear *= 1.0 - FADE_A_WEEK;
                t.rivalry *= 1.0 - FADE_A_WEEK;
                t.given *= 1.0 - LEDGER_FADE_A_WEEK;
                t.owed *= 1.0 - LEDGER_FADE_A_WEEK;
            }
        }
    }
}

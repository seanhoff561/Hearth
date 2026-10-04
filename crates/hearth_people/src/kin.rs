//! Kinship and households (V2.1 §8.1; H4). Who another is to a person, read from the pedigree
//! and the pair bonds — parents and children, brothers and sisters, grandparents, aunts and
//! uncles, cousins, partners and in-laws — for cultures to name and group as their kinship
//! systems do (H5); and households, the hearth groups that share food: a pair and the children
//! they raise, the widowed and their young, the grown not yet paired staying in their parents'.

use serde::{Deserialize, Serialize};

use crate::lineage::Pedigree;
use crate::person::PersonId;
use crate::sim::People;

/// Who one is to another, as the pedigree and the bonds tell it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kin {
    Partner,
    Parent,
    Child,
    /// A brother or sister by both parents, or by one.
    Sibling,
    HalfSibling,
    Grandparent,
    Grandchild,
    /// A parent's brother or sister.
    ParentsSibling,
    /// A brother's or sister's child.
    SiblingsChild,
    /// A child of a parent's brother or sister.
    Cousin,
    /// A partner's parent, a child's partner, a partner's brother or sister or a sibling's
    /// partner.
    ParentInLaw,
    ChildInLaw,
    SiblingInLaw,
}

impl Kin {
    /// How near (0 nearest): the order kin are named in and turned to.
    pub fn nearness(self) -> u8 {
        match self {
            Kin::Partner => 0,
            Kin::Child | Kin::Parent => 1,
            Kin::Sibling => 2,
            Kin::HalfSibling | Kin::Grandparent | Kin::Grandchild => 3,
            Kin::ParentsSibling | Kin::SiblingsChild => 4,
            Kin::ParentInLaw | Kin::ChildInLaw | Kin::SiblingInLaw => 5,
            Kin::Cousin => 6,
        }
    }

    /// The plain word for one of this kin of a sex (as the player's own language has it).
    pub fn word(self, female: bool) -> &'static str {
        let w = |f: &'static str, m: &'static str| if female { f } else { m };
        match self {
            Kin::Partner => "partner",
            Kin::Parent => w("mother", "father"),
            Kin::Child => w("daughter", "son"),
            Kin::Sibling => w("sister", "brother"),
            Kin::HalfSibling => w("half-sister", "half-brother"),
            Kin::Grandparent => w("grandmother", "grandfather"),
            Kin::Grandchild => w("granddaughter", "grandson"),
            Kin::ParentsSibling => w("aunt", "uncle"),
            Kin::SiblingsChild => w("niece", "nephew"),
            Kin::Cousin => "cousin",
            Kin::ParentInLaw => w("mother-in-law", "father-in-law"),
            Kin::ChildInLaw => w("daughter-in-law", "son-in-law"),
            Kin::SiblingInLaw => w("sister-in-law", "brother-in-law"),
        }
    }
}

/// The parents of someone, as far as the record knows them.
fn parents(people: &People, id: PersonId) -> [Option<PersonId>; 2] {
    let (m, f) = people.parents(id);
    [m, f]
}

/// Whether two share a parent, and both.
fn siblings(people: &People, a: PersonId, b: PersonId) -> Option<bool> {
    if a == b {
        return None;
    }
    let (pa, pb) = (parents(people, a), parents(people, b));
    let shared = pa
        .iter()
        .flatten()
        .filter(|p| pb.contains(&Some(**p)))
        .count();
    match shared {
        0 => None,
        1 => Some(false),
        _ => Some(true),
    }
}

/// Who `b` is to `a`, if kin by blood within cousins or by a bond within in-laws.
pub fn kin_of(people: &People, a: PersonId, b: PersonId) -> Option<Kin> {
    if a == b {
        return None;
    }
    let bond = |x: PersonId| people.get(x).and_then(|p| p.social.bond);
    if bond(a) == Some(b) || bond(b) == Some(a) {
        return Some(Kin::Partner);
    }
    let (pa, pb) = (parents(people, a), parents(people, b));
    if pa.contains(&Some(b)) {
        return Some(Kin::Parent);
    }
    if pb.contains(&Some(a)) {
        return Some(Kin::Child);
    }
    if let Some(full) = siblings(people, a, b) {
        return Some(if full { Kin::Sibling } else { Kin::HalfSibling });
    }
    let grand = |x: [Option<PersonId>; 2]| -> Vec<PersonId> {
        x.iter()
            .flatten()
            .flat_map(|p| parents(people, *p))
            .flatten()
            .collect()
    };
    if grand(pa).contains(&b) {
        return Some(Kin::Grandparent);
    }
    if grand(pb).contains(&a) {
        return Some(Kin::Grandchild);
    }
    if pa
        .iter()
        .flatten()
        .any(|p| siblings(people, *p, b).is_some())
    {
        return Some(Kin::ParentsSibling);
    }
    if pb
        .iter()
        .flatten()
        .any(|p| siblings(people, *p, a).is_some())
    {
        return Some(Kin::SiblingsChild);
    }
    if pa.iter().flatten().any(|p| {
        pb.iter()
            .flatten()
            .any(|q| siblings(people, *p, *q).is_some())
    }) {
        return Some(Kin::Cousin);
    }
    // By a bond: the partner's parents and brothers and sisters; a child's partner, a
    // brother's or sister's partner.
    if let Some(p) = bond(a) {
        if parents(people, p).contains(&Some(b)) {
            return Some(Kin::ParentInLaw);
        }
        if siblings(people, p, b).is_some() {
            return Some(Kin::SiblingInLaw);
        }
    }
    if let Some(q) = bond(b) {
        if parents(people, q).contains(&Some(a)) {
            return Some(Kin::ChildInLaw);
        }
        if siblings(people, a, q).is_some() {
            return Some(Kin::SiblingInLaw);
        }
    }
    None
}

/// The age (years) from which one keeps a household (counts as grown in it).
const KEEPS_HOUSE: f64 = 15.0;

impl People {
    /// The living members of a household.
    pub fn household_of(&self, household: u64) -> Vec<PersonId> {
        self.persons
            .iter()
            .filter(|p| p.alive() && p.social.household == Some(household))
            .map(|p| p.id)
            .collect()
    }

    /// Gives each living member of a band without a household one (a band drawn out, or saved
    /// from before households): a woman with her partner and the children she raises; the
    /// unpaired, grown or not, with their mother's while she lives in the band; the rest each
    /// alone.
    pub(crate) fn settle_households(&mut self, bi: usize) {
        let band = self.bands[bi].id;
        let members = self.band_members(bi);
        if members
            .iter()
            .all(|&i| self.persons[i].social.household.is_some())
        {
            return;
        }
        let index = |people: &People, id: Option<PersonId>| {
            id.and_then(|id| people.persons.binary_search_by_key(&id, |q| q.id).ok())
                .filter(|&j| {
                    let q = &people.persons[j];
                    q.alive() && q.social.band == band
                })
        };
        // The women first, eldest first: each her own, or her partner's if he has one.
        let mut women: Vec<usize> = members
            .iter()
            .copied()
            .filter(|&i| self.persons[i].life.female)
            .collect();
        women.sort_by(|&a, &b| {
            let (p, q) = (&self.persons[a], &self.persons[b]);
            p.life.born.total_cmp(&q.life.born).then(p.id.cmp(&q.id))
        });
        for &w in &women {
            let p = &self.persons[w];
            if p.social.household.is_some() {
                continue;
            }
            // A daughter not yet paired stays with her mother.
            if p.social.bond.is_none() && index(self, p.life.mother).is_some() {
                continue;
            }
            let partner = index(self, p.social.bond);
            let h = partner
                .and_then(|j| self.persons[j].social.household)
                .unwrap_or_else(|| self.take_id());
            self.persons[w].social.household = Some(h);
            if let Some(j) = partner {
                self.persons[j].social.household = Some(h);
            }
        }
        // The rest, eldest first (a grandmother placed before her daughter's children): with
        // their mother, else alone.
        let mut rest: Vec<usize> = members
            .iter()
            .copied()
            .filter(|&i| self.persons[i].social.household.is_none())
            .collect();
        rest.sort_by(|&a, &b| {
            let (p, q) = (&self.persons[a], &self.persons[b]);
            p.life.born.total_cmp(&q.life.born).then(p.id.cmp(&q.id))
        });
        for i in rest {
            let mothers = index(self, self.persons[i].life.mother)
                .and_then(|m| self.persons[m].social.household);
            let h = mothers.unwrap_or_else(|| self.take_id());
            self.persons[i].social.household = Some(h);
        }
    }

    /// A pair just bonded keep a hearth of their own, and the children either raises who are
    /// not yet paired come with them.
    pub(crate) fn house_pair(&mut self, w: usize, m: usize) {
        let h = self.take_id();
        let (a, b) = (self.persons[w].id, self.persons[m].id);
        let band = self.persons[w].social.band;
        for p in self.persons.iter_mut().filter(|p| {
            p.id == a
                || p.id == b
                || (p.alive()
                    && p.social.band == band
                    && p.social.bond.is_none()
                    && (p.life.mother == Some(a) || p.life.father == Some(b)))
        }) {
            p.social.household = Some(h);
        }
    }

    /// When the last grown one of a household dies, the young left go to the household of
    /// their nearest kin in the band (a grandmother, an aunt, an elder brother or sister),
    /// else keep on together.
    pub(crate) fn rehome(&mut self, household: u64, band: u64, day: f64, year_days: f64) {
        let grown = |p: &crate::person::Person| (day - p.life.born) / year_days >= KEEPS_HOUSE;
        let left: Vec<usize> = (0..self.persons.len())
            .filter(|&i| {
                let p = &self.persons[i];
                p.alive() && p.social.household == Some(household)
            })
            .collect();
        if left.is_empty() || left.iter().any(|&i| grown(&self.persons[i])) {
            return;
        }
        for i in left {
            let id = self.persons[i].id;
            let to = self
                .persons
                .iter()
                .filter(|q| q.alive() && q.social.band == band && grown(q))
                .filter(|q| q.social.household.is_some_and(|h| h != household))
                .filter_map(|q| {
                    kin_of(self, id, q.id).map(|k| (k.nearness(), q.id, q.social.household))
                })
                .min_by_key(|(n, qid, _)| (*n, *qid))
                .and_then(|(_, _, h)| h);
            if let Some(h) = to {
                self.persons[i].social.household = Some(h);
            }
        }
    }
}

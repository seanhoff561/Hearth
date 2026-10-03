//! Lineage and kinship (V2.1 §4.2; H1): who descends from whom, and how closely two people are
//! related. The coefficient of kinship of two people is the chance that an allele drawn at random
//! from each at a locus is one allele by descent, reckoned on the pedigree; a person's
//! coefficient of inbreeding is their parents' kinship. The genomes carry the consequence (a
//! child of close kin is more often homozygous for a broken allele); these numbers let the
//! social systems (H4) and the inspector say it.

use std::collections::HashMap;

use crate::person::PersonId;
use crate::sim::People;

/// What the reckoning needs of a person: their parents, and an order in which nobody comes
/// before a parent of theirs (the day of birth, then the id).
pub trait Pedigree {
    fn parents(&self, id: PersonId) -> (Option<PersonId>, Option<PersonId>);
    fn order(&self, id: PersonId) -> Option<(f64, PersonId)>;
}

impl Pedigree for People {
    fn parents(&self, id: PersonId) -> (Option<PersonId>, Option<PersonId>) {
        self.get(id)
            .map_or((None, None), |p| (p.life.mother, p.life.father))
    }

    fn order(&self, id: PersonId) -> Option<(f64, PersonId)> {
        self.get(id).map(|p| (p.life.born, p.id))
    }
}

/// Generations reckoned back at most: ancestry beyond counts as unrelated (its share is below a
/// part in four billion).
const GENERATIONS: u32 = 32;

/// Kinship coefficients over a pedigree, remembered as they are reckoned.
pub struct Kinship<'a, P: Pedigree> {
    pedigree: &'a P,
    memo: HashMap<(PersonId, PersonId), f64>,
}

impl<'a, P: Pedigree> Kinship<'a, P> {
    pub fn new(pedigree: &'a P) -> Self {
        Self {
            pedigree,
            memo: HashMap::new(),
        }
    }

    /// The coefficient of kinship of two people: ½ of one with themself (more if inbred), ¼ of a
    /// parent and child or of full siblings, ⅛ of half siblings or a grandparent and grandchild,
    /// 1/16 of first cousins; 0 of strangers.
    pub fn of(&mut self, a: PersonId, b: PersonId) -> f64 {
        self.phi(a, b, 0)
    }

    /// A person's coefficient of inbreeding: their parents' kinship (¼ for a child of full
    /// siblings, 1/16 for one of first cousins).
    pub fn inbreeding(&mut self, a: PersonId) -> f64 {
        self.inbred(a, 0)
    }

    fn inbred(&mut self, a: PersonId, depth: u32) -> f64 {
        match self.pedigree.parents(a) {
            (Some(m), Some(f)) => self.phi(m, f, depth + 1),
            _ => 0.0,
        }
    }

    fn phi(&mut self, a: PersonId, b: PersonId, depth: u32) -> f64 {
        if depth > GENERATIONS {
            return 0.0;
        }
        let key = (a.min(b), a.max(b));
        if let Some(v) = self.memo.get(&key) {
            return *v;
        }
        let v = if a == b {
            0.5 * (1.0 + self.inbred(a, depth))
        } else {
            match (self.pedigree.order(a), self.pedigree.order(b)) {
                (Some(x), Some(y)) => {
                    // Back through the younger one's parents: the older cannot descend from them.
                    let (older, younger) = if (x.0, x.1) > (y.0, y.1) {
                        (b, a)
                    } else {
                        (a, b)
                    };
                    let (m, f) = self.pedigree.parents(younger);
                    let via = |k: &mut Self, p: Option<PersonId>| {
                        p.map_or(0.0, |p| k.phi(older, p, depth + 1))
                    };
                    0.5 * (via(self, m) + via(self, f))
                }
                // Someone not in the record: no known kinship.
                _ => 0.0,
            }
        };
        self.memo.insert(key, v);
        v
    }
}

/// A person's known ancestors, nearest first, with the generations back to each (1: a parent).
pub fn ancestors<P: Pedigree>(
    pedigree: &P,
    id: PersonId,
    generations: u32,
) -> Vec<(PersonId, u32)> {
    let mut out: Vec<(PersonId, u32)> = Vec::new();
    let mut front = vec![id];
    for g in 1..=generations {
        let mut next = Vec::new();
        for p in front {
            let (m, f) = pedigree.parents(p);
            for a in [m, f].into_iter().flatten() {
                if !out.iter().any(|(x, _)| *x == a) {
                    out.push((a, g));
                    next.push(a);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        front = next;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pedigree written out: (id, born, mother, father).
    struct Tree(Vec<(PersonId, f64, Option<PersonId>, Option<PersonId>)>);

    impl Pedigree for Tree {
        fn parents(&self, id: PersonId) -> (Option<PersonId>, Option<PersonId>) {
            self.0
                .iter()
                .find(|r| r.0 == id)
                .map_or((None, None), |r| (r.2, r.3))
        }
        fn order(&self, id: PersonId) -> Option<(f64, PersonId)> {
            self.0.iter().find(|r| r.0 == id).map(|r| (r.1, r.0))
        }
    }

    #[test]
    fn kinship_follows_the_pedigree() {
        // Grandparents 1 and 2; their children 3 (a daughter) and 4 (a son); 5 and 6, strangers
        // who marry them; 7 the child of 3 and 5, 8 of 4 and 6: first cousins. 9 a child of the
        // siblings 3 and 4; 10 a half sibling of 7 (3 with 6).
        let t = Tree(vec![
            (1, 0.0, None, None),
            (2, 0.0, None, None),
            (3, 20.0, Some(1), Some(2)),
            (4, 22.0, Some(1), Some(2)),
            (5, 19.0, None, None),
            (6, 21.0, None, None),
            (7, 45.0, Some(3), Some(5)),
            (8, 46.0, Some(6), Some(4)),
            (9, 47.0, Some(3), Some(4)),
            (10, 48.0, Some(6), Some(5)),
        ]);
        let mut k = Kinship::new(&t);
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(close(k.of(1, 1), 0.5));
        assert!(close(k.of(1, 2), 0.0), "strangers");
        assert!(close(k.of(1, 3), 0.25), "mother and daughter");
        assert!(close(k.of(3, 4), 0.25), "full siblings");
        assert!(close(k.of(1, 7), 0.125), "grandmother and grandchild");
        assert!(close(k.of(7, 8), 0.0625), "first cousins");
        assert!(close(k.of(7, 10), 0.125), "half siblings");
        assert!(close(k.inbreeding(9), 0.25), "a child of siblings");
        assert!(close(k.inbreeding(7), 0.0));
        assert!(
            close(k.of(9, 9), 0.625),
            "the inbred share more with themselves"
        );
        let a = ancestors(&t, 9, 4);
        assert_eq!(a, vec![(3, 1), (4, 1), (1, 2), (2, 2)]);
    }
}

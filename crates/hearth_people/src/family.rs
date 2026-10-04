//! The family a player is born into (V2.1 Addendum A; H3, D180): a household of the player's
//! species set down in full where the player begins — the mother and the father of the player's
//! birth, paired; their other children, three or four years apart, some of those born not living
//! (as among foragers), their genomes the parents' meiosis; and the player's own person, of the
//! birth's genome — one of Wild Earth's wandering families (D164), its numbers a group of the
//! ecological cells so it folds and wakes as any band does.

use glam::{DVec2, DVec3};
use hearth_craft::Graph;
use hearth_items::Items;
use hearth_math::hash::Rng;
use serde::{Deserialize, Serialize};

use crate::band::{Band, Culture, Places};
use crate::birth::Birth;
use crate::genome::{Genetics, Genome, Phenotype};
use crate::person::{Event, Person, PersonId, Tier};
use crate::sim::{People, band_stream, know_about};
use crate::species::SpeciesSet;
use crate::world::{Now, World};

/// Years between a mother's children, at least and at most.
const SPACING: (f64, f64) = (3.0, 4.5);
/// The share of the children a mother bears who are living when the player is born: the others
/// were lost before or did not live long (forager infant and child deaths).
const LIVING: f64 = 0.65;
/// A mother's age at her first child, at least; and the years more it may have been.
const FIRST_BIRTH: (f64, f64) = (18.0, 4.0);
/// The oldest a mother bears a child at.
const LAST_BIRTH: f64 = 44.0;

/// One of the player's brothers and sisters as the household is drawn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sibling {
    /// Years old when the player's life begins.
    pub age: f64,
    pub genome: Genome,
    pub phenotype: Phenotype,
}

/// The household a player is born into, drawn when the world is made (and shown at the birth):
/// the parents' ages, the player's, and the parents' other children — three or four years apart,
/// some born not living, the eldest born when the mother was eighteen or more, their genomes the
/// parents' meiosis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Household {
    pub mother_age: f64,
    pub father_age: f64,
    pub age: f64,
    pub siblings: Vec<Sibling>,
}

impl Household {
    /// The household of a birth, for a player of `age` years when their life begins.
    pub fn draw(birth: &Birth, genetics: &Genetics, age: f64, rng: &mut Rng) -> Self {
        let mut ages: Vec<f64> = Vec::new();
        let mut a = age;
        while ages.len() < 5 {
            a += SPACING.0 + (SPACING.1 - SPACING.0) * rng.next_f64();
            if a > age + 14.0 {
                break;
            }
            if rng.next_f64() < LIVING {
                ages.push(a);
            }
        }
        let eldest = ages.iter().copied().fold(age, f64::max);
        let mother_age = eldest + FIRST_BIRTH.0 + FIRST_BIRTH.1 * rng.next_f64();
        let father_age = mother_age - 1.0 + 6.0 * rng.next_f64();
        let mut a = age;
        loop {
            a -= SPACING.0 + (SPACING.1 - SPACING.0) * rng.next_f64();
            if a < 0.0 || mother_age - a > LAST_BIRTH {
                break;
            }
            if rng.next_f64() < LIVING {
                ages.push(a);
            }
        }
        ages.sort_by(|a, b| b.total_cmp(a));
        let siblings = ages
            .into_iter()
            .map(|age| {
                let female = rng.next_f32() < 0.5;
                let genome = genetics.child(&birth.mother, &birth.father, Some(female), rng);
                let phenotype = genetics.phenotype(&genome, rng);
                Sibling {
                    age,
                    genome,
                    phenotype,
                }
            })
            .collect();
        Self {
            mother_age,
            father_age,
            age,
            siblings,
        }
    }
}

/// What a family founding needs: the birth and its household, whose player, where.
pub struct Founding<'a> {
    pub birth: &'a Birth,
    pub household: &'a Household,
    /// The player's species (the profile's id).
    pub species: &'a str,
    pub player: u64,
    pub at: DVec3,
    /// Its country's winters are cold.
    pub cold: bool,
}

impl People {
    /// Founds the family a player is born into, as its household was drawn. The band's id and
    /// the player's person's; none where the player's species is missing.
    pub fn found_family(
        &mut self,
        species: &SpeciesSet,
        graph: &Graph,
        items: &Items,
        world: &mut dyn World,
        f: Founding<'_>,
        now: Now,
    ) -> Option<(u64, PersonId)> {
        let key = |id: &str| id.rsplit(':').next().unwrap_or(id).to_owned();
        let sp = species
            .list
            .iter()
            .find(|s| key(&s.id) == key(f.species))?
            .clone();
        let id = 1_000_000_000 + self.next_id;
        let years = now.year_days.max(1.0);
        let h = f.household;
        // The household's band, at ease with the player (the game makes its numbers a group of
        // the ecological cells).
        let (knowledge, techniques) = sp.ways(f.cold);
        self.bands.push(Band {
            id,
            species: sp.id.clone(),
            members: Vec::new(),
            home: DVec2::new(f.at.x, f.at.z),
            range_m: 4000.0,
            places: Places::default(),
            tolerance: vec![(f.player, 1.0)],
            culture: Culture {
                knowledge: knowledge.clone(),
                techniques,
                traditions: Vec::new(),
            },
            population_group: None,
            tier: Tier::Full,
            dormant_since: None,
            lived_to: None,
            rng: band_stream(self.seed, id),
        });
        let bi = self.bands.len() - 1;
        let born = |age: f64| now.day - age * years;
        // The parents, of the birth's genomes, paired before their first child.
        let add = |people: &mut People, female: bool, age: f64| -> PersonId {
            let pid = people.take_id();
            let pos = people.scatter(bi, f.at, 1.0, 6.0);
            let mut p = Person::new(
                pid,
                &sp,
                &knowledge,
                graph,
                id,
                female,
                born(age),
                pos,
                now,
                people.seed,
            );
            p.record(born(age), Event::Born { band: id });
            people.bands[bi].members.push(pid);
            people.persons.push(p);
            pid
        };
        let mother = add(self, true, h.mother_age);
        let father = add(self, false, h.father_age);
        let eldest = h.siblings.iter().map(|s| s.age).fold(h.age, f64::max);
        let paired = born(eldest + 1.0);
        let at = |people: &People, pid: PersonId| {
            people
                .persons
                .binary_search_by_key(&pid, |p| p.id)
                .expect("just added")
        };
        {
            let (m, d) = (at(self, mother), at(self, father));
            let (b, mp) = (f.birth, &mut self.persons[m]);
            mp.genome = Some(b.mother.clone());
            mp.phenotype = Some(b.mother_phenotype.clone());
            mp.social.bond = Some(father);
            mp.record(paired, Event::Paired { with: father });
            let dp = &mut self.persons[d];
            dp.genome = Some(b.father.clone());
            dp.phenotype = Some(b.father_phenotype.clone());
            dp.social.bond = Some(mother);
            dp.record(paired, Event::Paired { with: mother });
        }
        // Their children, eldest first; the player's person among them.
        let mut children: Vec<(f64, Option<&Sibling>)> =
            h.siblings.iter().map(|s| (s.age, Some(s))).collect();
        children.push((h.age, None));
        children.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut you = None;
        for (age, sibling) in children {
            let (genome, phenotype) = match sibling {
                Some(s) => (s.genome.clone(), s.phenotype.clone()),
                None => (f.birth.genome.clone(), f.birth.phenotype.clone()),
            };
            let pid = add(self, genome.female, age);
            let i = at(self, pid);
            let p = &mut self.persons[i];
            p.life.mother = Some(mother);
            p.life.father = Some(father);
            p.genome = Some(genome);
            p.phenotype = Some(phenotype);
            if sibling.is_none() {
                // The player's own: what it knows is the player's.
                p.player = Some(f.player);
                p.knowledge = Default::default();
                you = Some(pid);
            }
            let m = at(self, mother);
            self.persons[m].record(born(age), Event::Bore { child: pid });
        }
        self.form(bi, &species.psyche, now);
        self.onto_the_ground(bi, &*world, f.at);
        know_about(&mut self.bands[bi], world, items, f.at);
        Some((id, you?))
    }

    /// The person a player is, if they are one.
    pub fn player_person(&self, player: u64) -> Option<&Person> {
        self.persons.iter().find(|p| p.player == Some(player))
    }

    /// A player's person where the player is.
    pub fn place_player(&mut self, player: u64, pos: DVec3, yaw: f32) {
        if let Some(p) = self.persons.iter_mut().find(|p| p.player == Some(player)) {
            p.place.pos = pos;
            p.place.yaw = yaw;
        }
    }
}

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

/// A player's kin in their band (see [`People::kin_of_player`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerKin {
    pub band: u64,
    pub mother: Option<PersonId>,
    pub father: Option<PersonId>,
    pub eldest: Option<PersonId>,
    pub youngest: Option<PersonId>,
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
                ..Culture::default()
            },
            population_group: None,
            tier: Tier::Full,
            dormant_since: None,
            lived_to: None,
            camp: Some(f.at),
            council: None,
            weighed: 0.0,
            guests: Vec::new(),
            rng: band_stream(self.seed, id),
        });
        let bi = self.bands.len() - 1;
        self.enculture(bi, &sp, now.day);
        let hearth = self.take_id();
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
            p.social.household = Some(hearth);
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
        self.acquaint(bi, now.day);
        self.name_members(bi, &sp);
        // The player's mother tongue is its family's.
        if let (Some(y), Some(language)) = (
            you.and_then(|y| self.index_of_person(y)),
            self.bands[bi].culture.language.as_ref().map(|l| l.id),
        ) {
            self.persons[y].tongues = vec![crate::speech::Tongue {
                language,
                native: true,
                words: Vec::new(),
            }];
        }
        self.onto_the_ground(bi, &*world, f.at);
        know_about(&mut self.bands[bi], world, items, f.at);
        Some((id, you?))
    }

    /// A player's kin in their band: their mother and father, the eldest and the youngest of
    /// their brothers and sisters at home (alive, of the band), and the band.
    pub fn kin_of_player(&self, player: u64) -> Option<PlayerKin> {
        let me = self.player_person(player)?;
        let band = me.social.band;
        let here = |q: &&Person| q.alive() && q.social.band == band && q.id != me.id;
        let mine = |id: Option<PersonId>| id.and_then(|id| self.get(id)).filter(here).map(|q| q.id);
        let mut siblings: Vec<&Person> = self
            .persons
            .iter()
            .filter(here)
            .filter(|q| q.life.mother.is_some() && q.life.mother == me.life.mother)
            .collect();
        siblings.sort_by(|a, b| a.life.born.total_cmp(&b.life.born));
        Some(PlayerKin {
            band,
            mother: mine(me.life.mother),
            father: mine(me.life.father),
            eldest: siblings.first().map(|q| q.id),
            youngest: siblings.last().map(|q| q.id),
        })
    }

    /// Who would carry a player's person if it were an infant, or keep it as a child: its
    /// mother, else the nearest grown one of its band. Where they stand, which way they face,
    /// and how tall they are.
    pub fn keeper_of_player(
        &self,
        player: u64,
        species: &SpeciesSet,
        now: &Now,
    ) -> Option<(PersonId, DVec3, f32, f32)> {
        let me = self.player_person(player)?;
        let band = me.social.band;
        let grown = |q: &Person| {
            species
                .get(&q.species)
                .is_some_and(|sp| q.age(now) >= sp.life.maturity_years as f64)
        };
        let keeper = me
            .life
            .mother
            .and_then(|m| self.get(m))
            .filter(|q| q.alive() && q.social.band == band && q.tier == Tier::Full)
            .or_else(|| {
                self.persons
                    .iter()
                    .filter(|q| q.alive() && q.social.band == band && q.id != me.id)
                    .filter(|q| q.tier == Tier::Full && grown(q))
                    .min_by(|a, b| {
                        let d = |q: &Person| (q.place.pos - me.place.pos).length();
                        d(a).total_cmp(&d(b)).then(a.id.cmp(&b.id))
                    })
            })?;
        let height = species
            .get(&keeper.species)
            .map_or(1.6, |sp| keeper.height_m(sp, now));
        Some((keeper.id, keeper.place.pos, keeper.place.yaw, height))
    }

    /// Sends a person off to the water its band knows nearest (an elder taking a child along).
    pub fn lead_to_water(&mut self, id: PersonId) -> bool {
        let Ok(i) = self.persons.binary_search_by_key(&id, |q| q.id) else {
            return false;
        };
        let pos = self.persons[i].place.pos;
        let band = self.persons[i].social.band;
        let water = self.persons[i]
            .memory
            .nearest(crate::memory::PlaceKind::Water, pos)
            .or_else(|| {
                self.bands
                    .iter()
                    .find(|b| b.id == band)
                    .and_then(|b| crate::band::Places::nearest(&b.places.water, pos))
            });
        let Some(to) = water else {
            return false;
        };
        let p = &mut self.persons[i];
        p.mind.doing = crate::mind::Doing::Going {
            to,
            then: crate::mind::Intent::Drink,
        };
        p.mind.timer = 60.0;
        true
    }

    /// Brings a band's living members about a place (a coming of age).
    pub fn gather_to(&mut self, band: u64, at: DVec3) {
        for p in self
            .persons
            .iter_mut()
            .filter(|p| p.social.band == band && p.alive() && p.player.is_none())
        {
            p.mind.doing = crate::mind::Doing::Going {
                to: at,
                then: crate::mind::Intent::Rejoin,
            };
            p.mind.timer = 30.0;
        }
    }

    /// A player's person dies as the player's body did (Addendum B §2): the death an event of
    /// the world, its kin mourning, its body lying where it fell once another is lived.
    pub fn player_died(&mut self, player: u64, cause: String, day: f64) {
        let Some(i) = self.persons.iter().position(|p| p.player == Some(player)) else {
            return;
        };
        let p = &mut self.persons[i];
        if p.life.died.is_some() {
            return;
        }
        let cause = crate::person::Cause::Body(cause);
        p.life.died = Some(crate::person::Died {
            day,
            cause: cause.clone(),
        });
        p.record(day, Event::Died { cause });
        let (dead, band) = (p.id, p.social.band);
        for b in self.bands.iter_mut().filter(|b| b.id == band) {
            b.members.retain(|m| *m != dead);
        }
        self.mourn(dead, band, day);
    }

    /// The player lives on as another (Addendum B §2): the record they were is theirs no more
    /// (it lies where it fell, if it died) and the living one is. The one taken up, as it was —
    /// its body, what it knows, what it carries, where it is — or none where it may not be
    /// (dead, another player's, or not lived in full).
    pub fn inhabit(&mut self, player: u64, id: PersonId) -> Option<Person> {
        let i = self.persons.binary_search_by_key(&id, |p| p.id).ok()?;
        let q = &self.persons[i];
        if !q.alive() || q.player.is_some() || q.tier != Tier::Full {
            return None;
        }
        for p in self.persons.iter_mut().filter(|p| p.player == Some(player)) {
            p.player = None;
        }
        let q = &mut self.persons[i];
        q.player = Some(player);
        q.mind = Default::default();
        let taken = q.clone();
        // What it carried and knew is the player's now.
        q.possessions = Default::default();
        q.knowledge = Default::default();
        Some(taken)
    }

    /// A player is no longer the person they were (born again elsewhere): the record stays
    /// as it is, its own again.
    pub fn release_player(&mut self, player: u64) {
        for p in self.persons.iter_mut().filter(|p| p.player == Some(player)) {
            p.player = None;
        }
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

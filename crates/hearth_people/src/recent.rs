//! The recent past about a birthplace, and births into its households (V2.1 §15.2, Addendum A;
//! H8, D196–D197): the era's bands of the ecological cells about the place are founded a century
//! ago from their people of deep time and lived as households, birth by birth and death by death,
//! to the era's date — so that when a player is born among them, the people have lives: parents
//! and grandparents, the dead remembered, kin in the next band. A player is born into one of
//! their households: two to four offered, of as many peoples and bands as the place has.

use glam::{DVec2, DVec3};
use hearth_craft::Graph;
use hearth_fauna::ecology::{Ecology, dist};
use hearth_fauna::habitat::CELL_M;
use hearth_items::Items;
use hearth_math::hash::{Rng, hash2};
use serde::{Deserialize, Serialize};

use crate::birth::Birth;
use crate::family::{Household, Sibling};
use crate::genome::Genetics;
use crate::person::{Event, Person, PersonId, Tier};
use crate::sim::{Numbers, People, cold_country};
use crate::species::SpeciesSet;
use crate::world::Now;

/// The oldest a mother bears a child at (years).
const LAST_BIRTH: f64 = 44.0;
/// The least years since a mother's last child for another to come.
const SPACING: f64 = 1.5;

/// A household a player may be born into: the band, the mother and the father.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BirthOption {
    pub band: u64,
    pub mother: PersonId,
    pub father: PersonId,
}

impl People {
    /// Lives the recent past about a place (V2.1 §15.2): each band of the era's peoples in the
    /// ecological cells within `reach_m` of `at` founded `years` before `now` from its people of
    /// deep time, and every band so founded lived as households to `now`, a year at a time
    /// (`progress` told the share done). The number of bands founded.
    #[allow(clippy::too_many_arguments)]
    pub fn recent_history(
        &mut self,
        eco: &mut Ecology,
        species: &SpeciesSet,
        graph: &Graph,
        items: &Items,
        latitude: &dyn Fn(DVec3) -> f64,
        at: DVec2,
        reach_m: f64,
        years: f64,
        now: Now,
        progress: &dyn Fn(f32),
    ) -> usize {
        let wrap = eco.cells_around as f64 * CELL_M;
        let cat = eco.catalog.clone();
        let past = Now {
            day: now.day - years * now.year_days,
            ..now
        };
        let mut groups: Vec<(u64, usize, Numbers, [f64; 2], [f64; 2], f64)> = Vec::new();
        for r in eco.regions.values() {
            for g in &r.groups {
                let sp = &cat.species[g.species as usize];
                if !sp.hominin
                    || g.live
                    || g.size() == 0
                    || dist(g.pos, [at.x, at.y], wrap) > reach_m
                {
                    continue;
                }
                let Some(si) = species
                    .list
                    .iter()
                    .position(|k| k.population.as_deref().is_some_and(|p| p == sp.id))
                else {
                    continue;
                };
                let n = Numbers {
                    young: g.young,
                    juveniles: g.juveniles,
                    females: g.females,
                    males: g.males,
                };
                let range_m = (sp.home_range_km2 as f64 / std::f64::consts::PI).sqrt() * 1000.0;
                groups.push((g.id, si, n, g.pos, g.home, range_m));
            }
        }
        groups.sort_by_key(|g| g.0);
        let mut founded = Vec::new();
        for (gid, si, n, pos, home, range_m) in groups {
            let sp = &species.list[si];
            let here = DVec3::new(pos[0], 0.0, pos[1]);
            let cold = cold_country(eco, pos[0], pos[1]);
            let bi = self.found(sp, graph, gid, n, here, DVec2::from_array(home), cold, past);
            self.bands[bi].range_m = range_m;
            // Its camp where its people are, on dry ground (its height found when it comes into
            // full).
            if sp.does(hearth_content::schema::humans::Behavior::KeepCamp)
                && self.bands[bi].camp.is_none()
            {
                let taken: Vec<DVec2> = self
                    .bands
                    .iter()
                    .filter(|b| b.id != self.bands[bi].id)
                    .filter_map(|b| b.camp.map(|c| DVec2::new(c.x, c.z)))
                    .collect();
                let dry = self.era.country.as_ref().and_then(|c| {
                    c.dry_ground(
                        DVec2::from_array(pos),
                        3000.0,
                        &taken,
                        crate::rounds::CAMPS_APART_M,
                    )
                });
                self.bands[bi].camp = Some(dry.unwrap_or(here));
            }
            self.bands[bi].population_group = Some(gid);
            let genes = species.genetics.as_ref();
            let sun = genes.map_or(1.0, |g| g.sunlight(latitude(here)));
            self.endow(bi, sp, genes, sun, past);
            self.form(bi, &species.psyche, past);
            self.to_households(bi, past);
            self.bands[bi].lived_to = Some(past.day);
            founded.push(gid);
        }
        for r in eco.regions.values_mut() {
            for g in r.groups.iter_mut().filter(|g| founded.contains(&g.id)) {
                g.live = true;
            }
        }
        // A century of their lives, a year at a time.
        let whole = years.ceil().max(1.0) as u64;
        for y in 1..=whole {
            let day = (past.day + y as f64 * now.year_days).min(now.day);
            self.live_course_by(species, items, latitude, Now { day, ..now });
            progress(y as f32 / whole as f32);
        }
        founded.len()
    }

    /// The households a player may be born into about a place: a living mother, grown and
    /// bearing still (old enough to have borne a child `age` years ago, which a player starting
    /// grown is), no other child of hers born within a year and a half of that birth, paired with
    /// a living father of her band — at most `most`, of as many species, peoples and bands as
    /// there are, then of as many households. Where a place's few have no such household (a
    /// player starting grown asks a mother of years), the rule eases a step at a time: births
    /// closer together, a mother as young as her people pair, a father since dead, then a mother
    /// since dead too (the player grown up an orphan of the band).
    pub fn birth_options(
        &self,
        species: &SpeciesSet,
        at: DVec2,
        reach_m: f64,
        age: f64,
        now: &Now,
        most: usize,
    ) -> Vec<BirthOption> {
        let mut found = Vec::new();
        for ease in 0..4 {
            found = self.households_at(species, at, reach_m, age, now, ease);
            if !found.is_empty() {
                break;
            }
        }
        // The nearest bands of each people first.
        found.sort_by(|a, b| {
            (&a.0, a.1)
                .cmp(&(&b.0, b.1))
                .then(a.2.total_cmp(&b.2))
                .then((a.3, a.4.mother).cmp(&(b.3, b.4.mother)))
        });
        // Spread over what differs most: species, then peoples, then bands, then households.
        let mut out: Vec<BirthOption> = Vec::new();
        let mut taken: Vec<(String, u32, u64)> = Vec::new();
        for pass in 0..4 {
            for (sp, lin, _, band, o) in &found {
                if out.len() >= most {
                    return out;
                }
                if out.contains(o) {
                    continue;
                }
                let fresh = match pass {
                    0 => !taken.iter().any(|t| &t.0 == sp),
                    1 => !taken.iter().any(|t| t.1 == *lin && &t.0 == sp),
                    2 => !taken.iter().any(|t| t.2 == *band),
                    _ => true,
                };
                if fresh {
                    taken.push((sp.clone(), *lin, *band));
                    out.push(*o);
                }
            }
        }
        out
    }

    /// The households about a place that could have borne a child `age` years ago, by the rule
    /// eased `ease` steps (see [`Self::birth_options`]): each with its species, lineage, how far
    /// its band keeps camp, and its band.
    fn households_at(
        &self,
        species: &SpeciesSet,
        at: DVec2,
        reach_m: f64,
        age: f64,
        now: &Now,
        ease: u8,
    ) -> Vec<(String, u32, f64, u64, BirthOption)> {
        let years = now.year_days.max(1.0);
        let mut found = Vec::new();
        for b in &self.bands {
            if !matches!(b.tier, Tier::Household | Tier::Full) {
                continue;
            }
            let home = b.camp.map_or(b.home, |c| DVec2::new(c.x, c.z));
            let far = (home - at).length();
            if far > reach_m {
                continue;
            }
            let Some(sp) = species.get(&b.species) else {
                continue;
            };
            let youngest = if ease >= 1 {
                species
                    .life
                    .of(&b.species)
                    .map_or(sp.life.maturity_years as f64, |t| t.pairing_age.0 as f64)
            } else {
                sp.life.maturity_years as f64
            };
            // A mother since dead was of the band when she died.
            let mothers: Vec<&Person> = if ease >= 3 {
                self.persons
                    .iter()
                    .filter(|p| p.social.band == b.id && p.life.female)
                    .collect()
            } else {
                self.members(b.id).collect()
            };
            for m in mothers {
                if (!m.alive() && ease < 3)
                    || !m.life.female
                    || m.player.is_some()
                    || m.genome.is_none()
                {
                    continue;
                }
                let a = m.age(now);
                if a - age < youngest || a - age > LAST_BIRTH {
                    continue;
                }
                // Living when the player was born.
                let born = now.day - age * years;
                if m.life.died.as_ref().is_some_and(|d| d.day <= born) {
                    continue;
                }
                // No other child of hers born within a year and a half of the player's birth,
                // before it or (for a player starting grown) after it.
                let spacing = if ease >= 1 { 0.75 } else { SPACING };
                let crowded = m.life.events.iter().any(|e| {
                    matches!(e.event, Event::Bore { .. }) && (e.day - born).abs() < spacing * years
                });
                if crowded {
                    continue;
                }
                let Some(f) = m.social.bond.and_then(|f| self.get(f)) else {
                    continue;
                };
                let living = f.alive() && f.social.band == b.id;
                let conceived = born - 0.75 * years;
                if (!living && ease < 2)
                    || f.life.died.as_ref().is_some_and(|d| d.day <= conceived)
                    || f.genome.is_none()
                    || f.player.is_some()
                {
                    continue;
                }
                let lineage = b.deep.as_ref().map_or(0, |d| d.lineage);
                found.push((
                    b.species.clone(),
                    lineage,
                    far,
                    b.id,
                    BirthOption {
                        band: b.id,
                        mother: m.id,
                        father: f.id,
                    },
                ));
            }
        }
        found
    }

    /// A player born into a household (Addendum A): their genome the meiosis of the mother's and
    /// the father's, a daughter or a son as asked or as chance has it; their person a child of the
    /// two in their band, `age` years old (a player starting grown), speaking its language as
    /// a mother tongue. The birth (the parents' genomes and the child's), the household as the
    /// birth screen tells it, and the player's person.
    #[allow(clippy::too_many_arguments)]
    pub fn born_into(
        &mut self,
        option: BirthOption,
        player: u64,
        female: Option<bool>,
        age: f64,
        genetics: &Genetics,
        species: &SpeciesSet,
        graph: &Graph,
        now: Now,
    ) -> Option<(Birth, Household, PersonId)> {
        let m = self.get(option.mother)?.clone();
        let f = self.get(option.father)?.clone();
        let sp = species.get(&m.species)?.clone();
        let (mg, fg) = (m.genome.clone()?, f.genome.clone()?);
        let mut rng = Rng::new(hash2(self.seed ^ 0xb0_2e, player ^ (option.mother << 20)));
        let genome = genetics.child(&mg, &fg, female, &mut rng);
        let birth = Birth {
            mother_phenotype: m
                .phenotype
                .clone()
                .unwrap_or_else(|| genetics.phenotype(&mg, &mut rng)),
            father_phenotype: f
                .phenotype
                .clone()
                .unwrap_or_else(|| genetics.phenotype(&fg, &mut rng)),
            phenotype: genetics.phenotype(&genome, &mut rng),
            mother: mg,
            father: fg,
            genome,
        };
        let years = now.year_days.max(1.0);
        let siblings: Vec<Sibling> = self
            .persons
            .iter()
            .filter(|q| q.life.mother == Some(m.id) && q.alive())
            .filter_map(|q| {
                Some(Sibling {
                    age: q.age(&now) - age,
                    genome: q.genome.clone()?,
                    phenotype: q.phenotype.clone()?,
                })
            })
            .filter(|s| s.age > 0.0)
            .collect();
        let household = Household {
            mother_age: m.age(&now) - age,
            father_age: f.age(&now) - age,
            age,
            siblings,
        };
        let bi = self.bands.iter().position(|b| b.id == option.band)?;
        let pid = self.take_id();
        let born = now.day - age * years;
        // With the mother, or (she since dead) at the band's camp.
        let band = &self.bands[bi];
        let at = if m.alive() {
            m.place.pos
        } else {
            band.camp
                .unwrap_or_else(|| glam::DVec3::new(band.home.x, 0.0, band.home.y))
        };
        let mut p = Person::new(
            pid,
            &sp,
            &[],
            graph,
            option.band,
            birth.genome.female,
            born,
            at,
            now,
            self.seed,
        );
        p.record(born, Event::Born { band: option.band });
        p.life.mother = Some(m.id);
        p.life.father = Some(f.id);
        p.genome = Some(birth.genome.clone());
        p.phenotype = Some(birth.phenotype.clone());
        p.player = Some(player);
        p.knowledge = Default::default();
        p.social.household = m.social.household;
        p.tier = self.bands[bi].tier;
        if let Some(l) = self.bands[bi].culture.language.as_ref() {
            p.tongues = vec![crate::speech::Tongue {
                language: l.id,
                native: true,
                words: Vec::new(),
            }];
        }
        self.persons.push(p);
        self.persons.sort_by_key(|q| q.id);
        self.bands[bi].members.push(pid);
        if let Some(i) = self.index_of_person(m.id) {
            self.persons[i].record(born, Event::Bore { child: pid });
        }
        self.acquaint(bi, now.day);
        Some((birth, household, pid))
    }
}

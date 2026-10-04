//! The life course (V2.1 §7, §14.3; H3). Every band lived in full of a species with a life
//! table lives it a fifty-second of a calendar year at a time, the one furthest behind first so
//! neighbours keep pace (a dormant band's numbers live on in the ecological cells meanwhile):
//! deaths by its people's ages (more among the children where more live about than the land
//! feeds well), pairing (the unpaired grown with the nearest in age who is not close kin, from
//! the band or a band of their kind nearby, the one who disperses moving), conception (a paired
//! woman's chance by her age, held back while she nurses and where the land is crowded), births
//! (twins now and then; a mother's death in it), and bands splitting in two as they outgrow
//! themselves.

use glam::{DVec2, DVec3};
use hearth_content::Content;
use hearth_content::schema::humans::Disperser;
use hearth_content::schema::life::{Crowding, Siler};

use crate::lineage::ancestors;
use crate::person::{Cause, Died, Event, Person, PersonId, Pregnancy, Tier};
use crate::sim::People;
use crate::species::{Species, SpeciesSet};
use crate::world::{Now, Senses};

/// A people's life table, resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub species: String,
    pub mortality: Siler,
    pub fecundability: Vec<(f32, f32)>,
    pub nursing_years: f32,
    pub nursing_factor: f32,
    pub twins: f32,
    pub maternal_death: f32,
    pub pairing_age: (f32, f32),
    /// The persons a square kilometre feeds well.
    pub density: f32,
    pub splits_at: u16,
    pub crowding: Crowding,
}

impl Table {
    /// A paired woman's chance to conceive in a month at an age (the straight line between the
    /// ages the table lists; none outside them).
    pub fn fecundity(&self, age: f64) -> f64 {
        let a = age as f32;
        for w in self.fecundability.windows(2) {
            let ((x0, y0), (x1, y1)) = (w[0], w[1]);
            if (x0..=x1).contains(&a) {
                let t = if x1 > x0 { (a - x0) / (x1 - x0) } else { 0.0 };
                return (y0 + (y1 - y0) * t) as f64;
            }
        }
        0.0
    }

    /// The persons the land within [`ROOM_M`] of a band's home feeds well.
    pub fn feeds(&self) -> f64 {
        let km = ROOM_M / 1000.0;
        (self.density as f64 * std::f64::consts::PI * km * km).max(1.0)
    }
}

/// Every species' life table.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tables {
    pub list: Vec<Table>,
}

fn key(id: &str) -> &str {
    id.rsplit(':').next().unwrap_or(id)
}

impl Tables {
    pub fn from_content(c: &Content) -> Self {
        Self {
            list: c
                .life_tables
                .iter()
                .map(|t| Table {
                    species: t.species.to_string(),
                    mortality: t.mortality,
                    fecundability: t.fecundability.clone(),
                    nursing_years: t.nursing_years,
                    nursing_factor: t.nursing_factor,
                    twins: t.twins,
                    maternal_death: t.maternal_death,
                    pairing_age: t.pairing_age,
                    density: t.density,
                    splits_at: t.band_splits_at.max(2),
                    crowding: t.crowding,
                })
                .collect(),
        }
    }

    /// The life table of a species, if it has one.
    pub fn of(&self, species: &str) -> Option<&Table> {
        self.list.iter().find(|t| key(&t.species) == key(species))
    }
}

/// Steps of the life course in a calendar year.
pub const STEPS_A_YEAR: f64 = 52.0;
/// Every so many steps the unpaired look for a partner (about monthly).
const PAIRING_EVERY: u64 = 4;
/// The widest age gap between partners (years).
const AGE_GAP: f64 = 15.0;
/// Women past this age pair no more (years).
const PAIRS_UNTIL: f64 = 45.0;
/// How far about its home a band feels the numbers of its kind (m).
pub const ROOM_M: f64 = 15_000.0;
/// How far the unpaired look for a partner among other bands (m).
const MATES_M: f64 = 40_000.0;
/// How far a band that splits off goes (m): this and up to as far again.
const SPLIT_M: f64 = 8_000.0;

/// A person and their parents and grandparents: two who share one are close kin (siblings, half
/// siblings, first cousins, an aunt and her nephew, a parent or grandparent and its child).
fn line(people: &People, id: PersonId) -> Vec<PersonId> {
    let mut v: Vec<PersonId> = ancestors(people, id, 2)
        .into_iter()
        .map(|(a, _)| a)
        .collect();
    v.push(id);
    v
}

impl People {
    /// Lives the life course of every band lived in full of a species with a life table, from
    /// the day each was last reckoned to now, a step at a time (a band met again, from now).
    pub fn live_course(&mut self, species: &SpeciesSet, senses: &dyn Senses, now: Now) {
        let step = now.year_days.max(1.0) / STEPS_A_YEAR;
        for band in &mut self.bands {
            if band.tier == Tier::Full {
                band.lived_to.get_or_insert(now.day);
            } else {
                band.lived_to = None;
            }
        }
        // The band furthest behind goes first (bands split off join in from their day).
        loop {
            let next = (0..self.bands.len())
                .filter_map(|b| self.bands[b].lived_to.map(|d| (d, b)))
                .filter(|(d, _)| d + step <= now.day)
                .min_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
            let Some((from, bi)) = next else {
                break;
            };
            let day = from + step;
            self.bands[bi].lived_to = Some(day);
            let kind = &self.bands[bi].species;
            let (Some(table), Some(sp)) = (species.life.of(kind), species.get(kind)) else {
                // A people without a life table has no course to live.
                self.bands[bi].lived_to = Some(now.day);
                continue;
            };
            let home = self.bands[bi].home;
            let sun = species.genetics.as_ref().map_or(1.0, |g| {
                g.sunlight(senses.latitude(DVec3::new(home.x, 0.0, home.y)))
            });
            let n = (day / step).round() as u64;
            self.course_step(bi, sp, species, table, sun, day, n, now);
        }
    }

    /// The indices of a band's living members.
    fn living(&self, bi: usize) -> Vec<usize> {
        let band = self.bands[bi].id;
        self.bands[bi]
            .members
            .iter()
            .filter_map(|id| self.persons.binary_search_by_key(id, |p| p.id).ok())
            .filter(|&i| {
                let p = &self.persons[i];
                p.alive() && p.social.band == band
            })
            .collect()
    }

    /// How many more of its kind live within [`ROOM_M`] of a band's home than the land feeds
    /// well, as a part (1 where no more).
    pub fn crowd(&self, bi: usize, table: &Table) -> f64 {
        let (home, kind) = (self.bands[bi].home, &self.bands[bi].species);
        let n: usize = self
            .bands
            .iter()
            .filter(|b| b.tier == Tier::Full && &b.species == kind)
            .filter(|b| (b.home - home).length() <= ROOM_M)
            .map(|b| b.members.len())
            .sum();
        (n as f64 / table.feeds()).max(1.0)
    }

    /// One step of a band's life course, ending on `day`.
    #[allow(clippy::too_many_arguments)]
    fn course_step(
        &mut self,
        bi: usize,
        sp: &Species,
        species: &SpeciesSet,
        table: &Table,
        sun: f32,
        day: f64,
        n: u64,
        now: Now,
    ) {
        let years = 1.0 / STEPS_A_YEAR;
        let year_days = now.year_days.max(1.0);
        let age = |p: &Person| (day - p.life.born) / year_days;
        let band = self.bands[bi].id;
        // The dead and the gone off the band's list.
        let living = self.living(bi);
        self.bands[bi].members = living.iter().map(|&i| self.persons[i].id).collect();
        let crowd = self.crowd(bi, table);
        // Deaths, by age; more children die where the land is crowded.
        for &i in &living {
            let a = age(&self.persons[i]);
            let mut h = table.mortality.hazard(a.max(0.0));
            if a < 15.0 {
                h *= 1.0 + table.crowding.children as f64 * (crowd - 1.0);
            }
            if self.bands[bi].rng.next_f64() < 1.0 - (-h * years).exp() {
                self.dies(i, day, Cause::Course);
            }
        }
        // Pairing, about monthly.
        if n.is_multiple_of(PAIRING_EVERY) {
            self.pair(bi, sp, table, day, year_days);
        }
        // Conception and birth.
        let living = self.living(bi);
        let gestation = sp.life.gestation_days as f64 / 365.0 * year_days;
        let months = 12.0 / STEPS_A_YEAR;
        let mut born_any = false;
        for &i in &living {
            let p = &self.persons[i];
            if !p.life.female || !p.alive() {
                continue;
            }
            if let Some(due) = p.life.pregnant.clone() {
                if due.due <= day {
                    self.give_birth(bi, i, sp, table, due, day);
                    born_any = true;
                }
                continue;
            }
            let Some(partner) = p.social.bond else {
                continue;
            };
            let together = self
                .get(partner)
                .is_some_and(|q| q.alive() && q.social.band == band);
            if !together {
                continue;
            }
            let mut f = table.fecundity(age(p));
            if f <= 0.0 {
                continue;
            }
            // Nursing her youngest holds conception back.
            let nursing = living.iter().any(|&c| {
                let c = &self.persons[c];
                c.life.mother == Some(p.id)
                    && c.alive()
                    && (day - c.life.born) / year_days < table.nursing_years as f64
            });
            if nursing {
                f *= table.nursing_factor as f64;
            }
            f /= crowd.powf(table.crowding.conception as f64);
            let chance = 1.0 - (1.0 - f.min(0.99)).powf(months);
            if self.bands[bi].rng.next_f64() < chance {
                self.persons[i].life.pregnant = Some(Pregnancy {
                    due: day + gestation,
                    father: Some(partner),
                });
            }
        }
        if born_any {
            self.endow(bi, sp, species.genetics.as_ref(), sun, now);
            self.form(bi, &species.psyche, now);
        }
        // A band grown past itself splits in two.
        if self.bands[bi].members.len() > table.splits_at as usize {
            self.split(bi, day);
        }
    }

    /// A person dies of what its people die of at its age, or in giving birth: off its band's
    /// list, its partner free, its kin mourning.
    fn dies(&mut self, i: usize, day: f64, cause: Cause) {
        let p = &mut self.persons[i];
        if p.life.died.is_some() {
            return;
        }
        p.life.died = Some(Died {
            day,
            cause: cause.clone(),
        });
        p.life.pregnant = None;
        p.record(day, Event::Died { cause });
        let (dead, band, bond) = (p.id, p.social.band, p.social.bond);
        for b in self.bands.iter_mut().filter(|b| b.id == band) {
            b.members.retain(|m| *m != dead);
        }
        if let Some(j) = bond.and_then(|b| self.persons.binary_search_by_key(&b, |q| q.id).ok())
            && self.persons[j].social.bond == Some(dead)
        {
            self.persons[j].social.bond = None;
        }
        self.mourn(dead, band, day);
    }

    /// The unpaired grown women of a band, eldest first, each paired with the unpaired grown man
    /// nearest her in age who is not her close kin — of her band, or else of the nearest band of
    /// their kind within [`MATES_M`], the one of them who disperses moving to the other's band.
    fn pair(&mut self, bi: usize, sp: &Species, table: &Table, day: f64, year_days: f64) {
        let age = |p: &Person| (day - p.life.born) / year_days;
        let free = |p: &Person| p.alive() && p.social.bond.is_none();
        let mut women: Vec<usize> = self
            .living(bi)
            .into_iter()
            .filter(|&i| {
                let p = &self.persons[i];
                p.life.female
                    && free(p)
                    && (table.pairing_age.0 as f64..PAIRS_UNTIL).contains(&age(p))
            })
            .collect();
        if women.is_empty() {
            return;
        }
        women.sort_by(|a, b| {
            let (p, q) = (&self.persons[*a], &self.persons[*b]);
            p.life.born.total_cmp(&q.life.born).then(p.id.cmp(&q.id))
        });
        // Where to look: her band, then the others of her kind nearby, nearest first.
        let (home, kind) = (self.bands[bi].home, self.bands[bi].species.clone());
        let mut bands: Vec<(f64, usize)> = (0..self.bands.len())
            .filter(|&b| self.bands[b].tier == Tier::Full && self.bands[b].species == kind)
            .map(|b| {
                let d = if b == bi {
                    -1.0
                } else {
                    (self.bands[b].home - home).length()
                };
                (d, b)
            })
            .filter(|(d, _)| *d <= MATES_M)
            .collect();
        bands.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
        for w in women {
            if !free(&self.persons[w]) {
                continue;
            }
            let her = self.persons[w].id;
            let her_age = age(&self.persons[w]);
            let hers = line(self, her);
            let found = bands.iter().find_map(|&(_, b)| {
                self.living(b)
                    .into_iter()
                    .filter(|&m| {
                        let q = &self.persons[m];
                        !q.life.female
                            && free(q)
                            && age(q) >= table.pairing_age.1 as f64
                            && (age(q) - her_age).abs() <= AGE_GAP
                    })
                    .filter(|&m| {
                        let his = line(self, self.persons[m].id);
                        !his.iter().any(|a| hers.contains(a))
                    })
                    .min_by(|x, y| {
                        let gap = |m: usize| (age(&self.persons[m]) - her_age).abs();
                        gap(*x).total_cmp(&gap(*y)).then(x.cmp(y))
                    })
                    .map(|m| (b, m))
            });
            let Some((b, m)) = found else {
                continue;
            };
            let him = self.persons[m].id;
            self.persons[w].social.bond = Some(him);
            self.persons[m].social.bond = Some(her);
            self.persons[w].record(day, Event::Paired { with: him });
            self.persons[m].record(day, Event::Paired { with: her });
            if b != bi {
                // One of them goes to the other's band: whoever disperses in their kind.
                let (mover, to) = match sp.social.disperses {
                    Disperser::Males => (m, bi),
                    Disperser::Females | Disperser::Both => (w, b),
                };
                self.move_to(mover, to, day, year_days);
            }
        }
    }

    /// A person goes to another band, and with it its children not yet grown.
    fn move_to(&mut self, i: usize, to: usize, day: f64, year_days: f64) {
        let id = self.persons[i].id;
        let from = self.persons[i].social.band;
        let mut going = vec![i];
        going.extend((0..self.persons.len()).filter(|&j| {
            let c = &self.persons[j];
            c.life.mother == Some(id)
                && c.alive()
                && c.social.band == from
                && c.social.bond.is_none()
                && (day - c.life.born) / year_days < 15.0
        }));
        let (new, home, tier) = (self.bands[to].id, self.bands[to].home, self.bands[to].tier);
        for j in going {
            let pid = self.persons[j].id;
            for b in self.bands.iter_mut().filter(|b| b.id == from) {
                b.members.retain(|m| *m != pid);
            }
            self.bands[to].members.push(pid);
            let p = &mut self.persons[j];
            p.social.band = new;
            p.tier = tier;
            p.place.pos.x = home.x;
            p.place.pos.z = home.y;
            p.record(day, Event::Joined { band: new });
        }
    }

    /// A woman gives birth to the child (or twins) she carries; it may cost her life.
    fn give_birth(
        &mut self,
        bi: usize,
        i: usize,
        sp: &Species,
        table: &Table,
        due: Pregnancy,
        day: f64,
    ) {
        self.persons[i].life.pregnant = None;
        let band = self.bands[bi].id;
        let mother = self.persons[i].id;
        let (pos, tier) = (self.persons[i].place.pos, self.persons[i].tier);
        let twins = self.bands[bi].rng.next_f32() < table.twins;
        for _ in 0..if twins { 2 } else { 1 } {
            let id = self.take_id();
            let female = self.bands[bi].rng.next_f32() < 0.5;
            let mut child = Person::newborn(id, sp, band, female, due.due.min(day), pos, self.seed);
            child.tier = tier;
            child.life.mother = Some(mother);
            child.life.father = due.father;
            child.record(day, Event::Born { band });
            self.bands[bi].members.push(id);
            self.persons.push(child);
            self.persons[i].record(day, Event::Bore { child: id });
        }
        if self.bands[bi].rng.next_f32() < table.maternal_death {
            self.dies(i, day, Cause::Childbirth);
        }
    }

    /// A band past what holds together splits: its households — a pair with the children of
    /// theirs not yet paired, one alone — go one by one, every other, to a new band nearby.
    fn split(&mut self, bi: usize, day: f64) {
        let living = self.living(bi);
        let index = |id: PersonId| living.iter().copied().find(|&j| self.persons[j].id == id);
        // A grown one's household: its pair's, or its own.
        let own = |i: usize| {
            let p = &self.persons[i];
            p.social
                .bond
                .and_then(index)
                .map_or(p.id, |b| p.id.min(self.persons[b].id))
        };
        let mut household: Vec<(PersonId, usize)> = Vec::new();
        for &i in &living {
            let p = &self.persons[i];
            // The unpaired young go with their mother's household.
            let head = match (p.social.bond, p.life.mother.and_then(index)) {
                (None, Some(m)) => own(m),
                _ => own(i),
            };
            household.push((head, i));
        }
        let mut heads: Vec<PersonId> = household.iter().map(|(h, _)| *h).collect();
        heads.sort_unstable();
        heads.dedup();
        let going: Vec<PersonId> = heads.into_iter().skip(1).step_by(2).collect();
        let id = 2_000_000_000 + self.take_id();
        let rng = &mut self.bands[bi].rng;
        let a = rng.next_f64() * std::f64::consts::TAU;
        let d = SPLIT_M * (1.0 + rng.next_f64());
        let mut new = self.bands[bi].clone();
        new.id = id;
        new.home = self.bands[bi].home + DVec2::new(a.cos(), a.sin()) * d;
        new.members = Vec::new();
        new.population_group = None;
        new.lived_to = Some(day);
        new.rng = crate::sim::band_stream(self.seed, id);
        for (head, i) in household {
            if !going.contains(&head) {
                continue;
            }
            let p = &mut self.persons[i];
            p.social.band = id;
            p.record(day, Event::Joined { band: id });
            new.members.push(p.id);
        }
        let moved = new.members.clone();
        self.bands[bi].members.retain(|m| !moved.contains(m));
        self.bands.push(new);
    }
}

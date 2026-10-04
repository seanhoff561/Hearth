//! The people (V2.1 §2–3, §17; H0): the registry of everyone ever drawn out and their bands; the
//! bands of the ecological cells drawn out near the player as persons — the same persons each
//! time — and folded back into their numbers as the player goes; and their days, lived a step at
//! a time in two phases so that neither the order of the persons nor the number of threads
//! changes what happens:
//!
//! 1. **Deciding** (in parallel): each person in full senses what is about it — the world, read
//!    only, and the others as the step found them — and, when its choice has run its time or
//!    danger interrupts, chooses again, drawing from its own random stream.
//! 2. **Acting** (in the persons' order): each does what it chose — walking, climbing, feeding,
//!    drinking, working the player's processes, calling, nesting — touching the shared world one
//!    at a time; and its body lives the moment.
//!
//! The world is the game's: it answers through [`Senses`] and [`World`] what lies where, what
//! there is to eat, the weather on a body, the hunters about, and takes their calls and nests.

use glam::{DVec2, DVec3};
use hearth_body::{BodyConfig, Food};
use hearth_content::Content;
use hearth_content::schema::humans::{Behavior, BodyPlan, Disperser, LifeStage};
use hearth_craft::engine::Aimed;
use hearth_craft::{Crafts, Graph};
use hearth_fauna::ecology::Ecology;
use hearth_fauna::habitat::CELL_M;
use hearth_fauna::live::{Cell, Ground, Medium, Stage};
use hearth_fauna::nav::{trunk_near, water_near};
use hearth_items::{Items, Stack};
use hearth_math::hash::Rng;
use rayon::prelude::*;

use crate::band::{Band, Culture, Places};
use crate::genome::Genetics;
use crate::lineage::Kinship;
use crate::memory::{Happened, PlaceKind, Who};
use crate::mind::{Doing, Intent, Needs, Offer, Situation, Stranger, Threat, choose};
use crate::person::{Cause, Died, Event, Person, PersonId, Tier};
use crate::plan::Step;
use crate::psyche::{Feeling, PsycheDefs, Tendency};
use crate::repute::Norms;
use crate::species::{Species, SpeciesSet};
use crate::work::{REACH_M, finish_work, plan_work};
use crate::world::{Now, PlayerSeen, Senses, World};
use hearth_content::schema::social::Sanction;
use serde::{Deserialize, Serialize};

/// A band within this of the player is drawn out as persons (m); one whose persons are all
/// beyond [`FAR_M`] is folded back into its numbers.
pub const NEAR_M: f64 = 112.0;
pub const FAR_M: f64 = 150.0;
/// How far a person looks about for a tree, water or food it does not yet know (m).
const LOOK_M: f64 = 40.0;
/// How far it sees a hunter or the player (m), by day; at night a quarter of it.
const SEE_M: f64 = 120.0;
/// How far an alarm call carries (m).
const HEAR_M: f64 = 300.0;
/// How near (m) one must be to see where another drinks, sleeps or works, and learn the place.
const LEARN_M: f64 = 30.0;
/// Plans that come to nothing before a goal is given up.
const GIVE_UP: u8 = 3;
/// Food this rich where it stands (kg a minute) keeps a person feeding there.
const RICH_KG_MIN: f32 = 0.1;
/// How near (m) another must be for what it shows (fear, joy) to be caught at all.
const CATCH_M: f32 = 25.0;
/// How far a hunter or the player may come before it runs, when it has not come to tolerate them.
const FLIGHT_M: f32 = 60.0;
/// A calm player holds its eye within this many times its flight distance; beyond, it goes about
/// its day.
const HEED: f32 = 1.5;
/// How long the bodies go between their steps (seconds of play).
const BODY_S: f32 = 1.0;
/// Food eaten a second of feeding, of a minute's worth (kg a minute → per second).
const PER_MIN: f32 = 1.0 / 60.0;
/// Water drunk a second at the water (l).
const DRINK_L_S: f64 = 0.05;
/// Seconds of play of a calm player's company within sight that bring a band to be at ease with
/// them (a play day and a half); and of a player running at them or hunting near them that undo
/// it.
const HABITUATE_S: f32 = 1.5 * 2880.0;
const UNLEARN_S: f32 = 60.0;
/// How far about its place a band knows the anvils lying (m).
const ANVILS_M: f64 = 80.0;
/// A band dormant longer than this (days) comes back rested and fed.
const REFRESH_DAYS: f64 = 1.0;

/// Something a person did that someone could see: the process and its triggers, where.
#[derive(Debug, Clone, PartialEq)]
pub struct Done {
    pub person: PersonId,
    pub at: DVec3,
    pub recipe: usize,
    pub triggers: Vec<String>,
}

/// A person as the client draws it (a message's payload: serializable, D166).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonView {
    pub id: PersonId,
    /// Its species' body plan, for its figure.
    pub plan: BodyPlan,
    pub female: bool,
    pub stage: Stage,
    pub pos: DVec3,
    pub yaw: f32,
    pub speed: f32,
    pub medium: Medium,
    pub doing: Doing,
    /// Its height (m), how grown its body is (0 a newborn's … 1), and its looks, for its figure.
    pub height_m: f32,
    #[serde(default = "grown")]
    pub grown: f32,
    /// Dead, lying where it fell until its people lay it to rest.
    #[serde(default)]
    pub dead: bool,
    pub look: crate::looks::Look,
    /// The feeling it shows on its body, and how strongly.
    #[serde(default)]
    pub shows: Option<(hearth_content::schema::psyche::Display, f32)>,
}

fn grown() -> f32 {
    1.0
}

/// A band's numbers as the ecological cells count them: the young of the year, the young not yet
/// grown, the grown females and males.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Numbers {
    pub young: u16,
    pub juveniles: u16,
    pub females: u16,
    pub males: u16,
}

impl Numbers {
    pub fn total(&self) -> usize {
        (self.young + self.juveniles + self.females + self.males) as usize
    }
}

/// A person as the others see it at the start of a step.
#[derive(Debug, Clone, Copy)]
struct Glimpse {
    id: PersonId,
    band: u64,
    pos: DVec3,
    grown: bool,
    /// A child or a juvenile, to play with.
    young: bool,
    alive: bool,
    alarm: bool,
    /// The work it is at, if any.
    working: Option<usize>,
    /// Hungry enough to be fed.
    hungry: bool,
    /// Its household.
    household: Option<u64>,
    /// Hurt (a wound not healed).
    hurt: bool,
    /// The feeling it shows, and how strongly.
    shows: Option<(Feeling, f32)>,
    /// The stranger it is greeting, or warning off.
    greeting: Option<PersonId>,
    warning: Option<PersonId>,
}

/// Everyone ever drawn out, and their bands.
#[derive(Debug, Clone, PartialEq)]
pub struct People {
    /// Every person, in the order of their ids (they are only ever added).
    pub persons: Vec<Person>,
    pub bands: Vec<Band>,
    pub next_id: u64,
    /// The world's seed: every person's and band's own stream comes from it.
    pub seed: u64,
    /// Seconds of play to the bodies' next step.
    pub(crate) body_s: f32,
    /// What was done in the last step that someone could have watched.
    pub done: Vec<Done>,
    /// Deeds done in the last step that tell of their doers (V2.1 §8.4).
    pub deeds: Vec<crate::repute::Seen>,
    /// The quarrels under way (V2.1 §8.6).
    pub quarrels: Vec<crate::conflict::Quarrel>,
}

/// The horizontal distance between two places on the planet (wrapping in x).
fn hdist(a: DVec3, b: DVec3, wrap: f64) -> f64 {
    let mut dx = (a.x - b.x).abs();
    if wrap > 0.0 {
        dx = dx.min(wrap - dx);
    }
    dx.hypot(a.z - b.z)
}

/// How much of the day's sight is left at a local hour: full by day, a quarter by moon and
/// starlight, between them at dawn and dusk.
pub fn light(hour: f32) -> f32 {
    let h = hour.rem_euclid(24.0);
    let up = ((h - 5.0) / 2.0).clamp(0.0, 1.0);
    let down = 1.0 - ((h - 17.5) / 2.0).clamp(0.0, 1.0);
    0.25 + 0.75 * up.min(down)
}

/// Generations reckoned back for mourning and for passing over close kin: enough for any kin
/// closer than second cousins.
const NEAR_KIN: u32 = 6;

/// Days the dead lie where they fell before their people lay them to rest.
const LAID_TO_REST_DAYS: f64 = 1.0;

/// A country whose coldest month is colder than this (°C) asks fire, warm clothes and shelter
/// from the wind of the people who live in it (D164).
const COLD_C: f32 = 8.0;

/// Whether a country's winters are cold, by its ecological cell's climate (D164).
pub fn cold_country(eco: &Ecology, x: f64, z: f64) -> bool {
    eco.regions
        .get(&eco.region_key(x, z))
        .and_then(|r| {
            r.cell_at(eco.cells_around, x, z)
                .and_then(|c| r.habitat.get(c))
        })
        .is_some_and(|h| h.coldest_c() < COLD_C)
}

/// How near the young play together, and see a grown one's work to go and watch it (m); how
/// close they crouch to watch it, and to take it in.
const PLAY_M: f64 = 12.0;
const WATCH_M: f64 = 30.0;
const BY_M: f64 = 1.5;
/// How far from its mother a child's play takes it (m).
const PLAY_ABOUT_M: f64 = 8.0;
/// Hours of practice a second of watching and trying gives (half of doing, at the default day).
const IMITATE_H_PER_S: f32 = 1.0 / 240.0;
/// The share of the works a child watches finished that teach it something.
const TAKEN_IN: f32 = 0.25;

/// How far off one sees another hungry to bring food to, and one hurt to stay by, and one it
/// thinks ill of to mock or keep from (m).
const SHARE_M: f64 = 15.0;
const TEND_M: f64 = 30.0;
const SCORN_M: f64 = 15.0;
/// How far one goes over to groom another (m).
const GROOM_M: f64 = 20.0;

/// Where in what it carries a person has food: in a hand, or in a basket or pouch.
pub(crate) fn carried_food(
    p: &Person,
    items: &Items,
    content: &Content,
) -> Option<hearth_items::Path> {
    use hearth_items::{Hand, Path, Root};
    let edible = |s: &Stack| {
        items
            .get(&s.id)
            .and_then(|k| hearth_craft::food::bite_of(content, k, s))
            .is_some()
    };
    let c = &p.possessions.carry;
    for (hand, held) in [(Hand::Right, &c.right), (Hand::Left, &c.left)] {
        if held.as_ref().is_some_and(&edible) {
            return Some(Path::at(Root::Hand(hand)));
        }
    }
    in_containers(c, &edible)
}

/// A person's physiology now, by its size.
pub(crate) fn species_body(p: &Person, sp: &Species, now: &Now) -> hearth_body::BodyConfig {
    p.body_config(sp, now).into_owned()
}

/// Breast milk, `l` litres of it: its energy, protein, fat, sugar and water (about 70 kcal, a
/// gram of protein, 4 of fat and 7 of sugar to the 100 ml), and the vitamins of fresh food.
fn milk(l: f64) -> Food {
    let k = l / 0.1;
    Food {
        kcal: 70.0 * k,
        protein_g: k,
        fat_g: 4.2 * k,
        carb_g: 7.0 * k,
        water_l: 0.088 * k,
        volume_l: l,
        fresh_days: k,
    }
}

/// The salt of the streams bands' cultures are drawn from.
const CULTURE_STREAM: u64 = 0x0c17_70e5_0000_0000;

/// A band's own stream, from the world's seed and its id.
pub(crate) fn band_stream(seed: u64, id: u64) -> Rng {
    Rng::new(hearth_math::hash::hash2(seed ^ 0x00ba_4d5e_ed00_0000, id))
}

impl People {
    pub fn new(seed: u64) -> Self {
        Self {
            persons: Vec::new(),
            bands: Vec::new(),
            next_id: 1,
            seed,
            body_s: 0.0,
            done: Vec::new(),
            deeds: Vec::new(),
            quarrels: Vec::new(),
        }
    }

    /// A person by id.
    pub fn get(&self, id: PersonId) -> Option<&Person> {
        self.persons
            .binary_search_by_key(&id, |p| p.id)
            .ok()
            .map(|i| &self.persons[i])
    }

    /// The persons of a band.
    pub fn members(&self, band: u64) -> impl Iterator<Item = &Person> {
        self.persons.iter().filter(move |p| p.social.band == band)
    }

    /// The persons lived in full now.
    pub fn full(&self) -> impl Iterator<Item = &Person> {
        self.persons
            .iter()
            .filter(|p| p.tier == Tier::Full && p.alive())
    }

    fn band_index(&self, id: u64) -> Option<usize> {
        self.bands.iter().position(|b| b.id == id)
    }

    /// The indices of a band's living members: by its list where it keeps one (a band whose
    /// course is lived), else by looking through everyone.
    pub(crate) fn band_members(&self, bi: usize) -> Vec<usize> {
        let band = self.bands[bi].id;
        let listed: Vec<usize> = self.bands[bi]
            .members
            .iter()
            .filter_map(|id| self.persons.binary_search_by_key(id, |p| p.id).ok())
            .filter(|&i| {
                let p = &self.persons[i];
                p.alive() && p.social.band == band
            })
            .collect();
        if !listed.is_empty() {
            return listed;
        }
        (0..self.persons.len())
            .filter(|&i| {
                let p = &self.persons[i];
                p.alive() && p.social.band == band
            })
            .collect()
    }

    pub(crate) fn take_id(&mut self) -> PersonId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// A band's living members counted as the ecological cells count them.
    pub fn numbers(&self, band: u64, species: &Species, now: &Now) -> Numbers {
        let mut n = Numbers::default();
        for p in self.members(band).filter(|p| p.alive()) {
            match (p.stage(species, now), p.life.female) {
                (Stage::Young, _) => n.young += 1,
                (Stage::Juvenile, _) => n.juveniles += 1,
                (Stage::Adult, true) => n.females += 1,
                (Stage::Adult, false) => n.males += 1,
            }
        }
        n
    }

    /// Draws out as persons the hominin groups of the ecology within [`NEAR_M`] of any player
    /// that are not out: a band met before as the same persons (reconciled with what the cells
    /// did with its numbers meanwhile), a new one as persons of the ages and sexes its numbers
    /// say, knowing the water, the trees and the anvils about it.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_out(
        &mut self,
        eco: &mut Ecology,
        species: &SpeciesSet,
        graph: &Graph,
        items: &Items,
        world: &mut dyn World,
        players: &[DVec3],
        now: Now,
    ) {
        let wrap = eco.cells_around as f64 * CELL_M;
        let cat = eco.catalog.clone();
        // The groups to draw out, in the order of their ids (and whether their country's
        // winters are cold).
        let mut groups: Vec<(u64, usize, Numbers, [f64; 2], [f64; 2], f64, bool)> = Vec::new();
        for r in eco.regions.values() {
            for g in &r.groups {
                let sp = &cat.species[g.species as usize];
                if !sp.hominin || g.live || g.size() == 0 {
                    continue;
                }
                let at = DVec3::new(g.pos[0], 0.0, g.pos[1]);
                if !players.iter().any(|p| hdist(at, *p, wrap) <= NEAR_M) {
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
                let cold = cold_country(eco, g.pos[0], g.pos[1]);
                groups.push((g.id, si, n, g.pos, g.home, range_m, cold));
            }
        }
        groups.sort_by_key(|g| g.0);
        let mut drawn = Vec::new();
        for (gid, si, want, pos, home, range_m, cold) in groups {
            let sp = &species.list[si];
            let Some(centre) = world.ground().top(pos[0], pos[1]) else {
                // Not loaded yet: it waits.
                continue;
            };
            let here = DVec3::new(pos[0], centre.level(), pos[1]);
            let genes = species.genetics.as_ref();
            let sun = genes.map_or(1.0, |g| g.sunlight(world.latitude(here)));
            let bi = match self
                .bands
                .iter()
                .position(|b| b.population_group == Some(gid))
            {
                Some(bi) => {
                    self.redraw(bi, sp, genes, sun, graph, want, here, now);
                    bi
                }
                None => {
                    let bi = self.found(
                        sp,
                        graph,
                        gid,
                        want,
                        here,
                        DVec2::from_array(home),
                        cold,
                        now,
                    );
                    self.bands[bi].range_m = range_m;
                    self.bands[bi].population_group = Some(gid);
                    self.endow(bi, sp, genes, sun, now);
                    world.settle(here, sp);
                    bi
                }
            };
            self.onto_the_ground(bi, &*world, here);
            know_about(&mut self.bands[bi], world, items, here);
            self.form(bi, &species.psyche, now);
            drawn.push(gid);
        }
        for r in eco.regions.values_mut() {
            for g in r.groups.iter_mut().filter(|g| drawn.contains(&g.id)) {
                g.live = true;
            }
        }
    }

    /// Founds a band of a species at a place: persons of the ages and sexes its numbers say, the
    /// young and half-grown with mothers among its grown females. Its index.
    #[allow(clippy::too_many_arguments)]
    fn found(
        &mut self,
        sp: &Species,
        graph: &Graph,
        id: u64,
        n: Numbers,
        here: DVec3,
        home: DVec2,
        cold: bool,
        now: Now,
    ) -> usize {
        let (knowledge, techniques) = sp.ways(cold);
        let mut rng = band_stream(self.seed, id);
        let maturity = sp.life.maturity_years as f64;
        let prime = (sp.life.adult_death_years.0 as f64).max(maturity + 5.0);
        let mut ages: Vec<(f64, bool)> = Vec::new();
        ages.extend((0..n.females).map(|_| (maturity + (prime - maturity) * rng.next_f64(), true)));
        ages.extend((0..n.males).map(|_| (maturity + (prime - maturity) * rng.next_f64(), false)));
        for _ in 0..n.juveniles {
            let age = 1.0 + (maturity - 1.0) * rng.next_f64();
            ages.push((age, rng.next_f32() < 0.5));
        }
        for _ in 0..n.young {
            ages.push((rng.next_f64(), rng.next_f32() < 0.5));
        }
        let band = Band {
            id,
            species: sp.id.clone(),
            members: Vec::new(),
            home,
            range_m: 2000.0,
            places: Places::default(),
            tolerance: Vec::new(),
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
            camp: None,
            council: None,
            weighed: 0.0,
            guests: Vec::new(),
            rng,
        };
        self.bands.push(band);
        let bi = self.bands.len() - 1;
        self.enculture(bi, sp, now.day);
        for (age, female) in ages {
            let born = now.day - age * now.year_days;
            let pos = self.scatter(bi, here, 2.0, 10.0);
            let pid = self.take_id();
            let mut p = Person::new(
                pid, sp, &knowledge, graph, id, female, born, pos, now, self.seed,
            );
            p.place.yaw = self.bands[bi].rng.next_f32() * std::f32::consts::TAU;
            p.mind.timer = self.bands[bi].rng.next_f32() * 5.0;
            p.record(now.day, Event::Found { band: id });
            self.bands[bi].members.push(pid);
            self.persons.push(p);
        }
        self.mothers(bi, sp, now);
        self.settle_households(bi);
        self.acquaint(bi, now.day);
        bi
    }

    /// Draws a band's culture if it has none yet (V2.1 §9): from its people's generator, on a
    /// stream of its own so that nothing else drawn for the band is moved.
    pub(crate) fn enculture(&mut self, bi: usize, sp: &Species, day: f64) {
        let Some(g) = &sp.culture else {
            return;
        };
        let b = &mut self.bands[bi];
        if b.culture.drawn() {
            return;
        }
        let mut rng = Rng::new(hearth_math::hash::hash2(self.seed ^ CULTURE_STREAM, b.id));
        let id = b.id;
        b.culture.draw(id, g, sp.ways.as_ref(), &mut rng, day);
    }

    /// A band's ways with strangers and quarrels: its culture's own, else its people's.
    pub(crate) fn ways_of(&self, bi: usize, sp: &Species) -> Option<crate::conflict::Ways> {
        self.bands[bi]
            .culture
            .ways
            .clone()
            .or_else(|| sp.ways.clone())
    }

    /// Gives every living member of a band without a genome one (V2.1 §4): a child of its
    /// mother's and father's where both are known — a father found among the band's grown males
    /// where none is — else one drawn from its species' pool at the place's sunlight (a founder,
    /// or a record from before genes). The eldest first, so parents have theirs before their
    /// children.
    pub(crate) fn endow(
        &mut self,
        bi: usize,
        sp: &Species,
        genetics: Option<&Genetics>,
        sun: f32,
        now: Now,
    ) {
        let Some(genetics) = genetics else {
            return;
        };
        let band = self.bands[bi].id;
        let mut todo: Vec<usize> = (0..self.persons.len())
            .filter(|&i| {
                let p = &self.persons[i];
                p.social.band == band && p.alive() && p.genome.is_none()
            })
            .collect();
        todo.sort_by(|&a, &b| {
            let (p, q) = (&self.persons[a], &self.persons[b]);
            p.life.born.total_cmp(&q.life.born).then(p.id.cmp(&q.id))
        });
        for i in todo {
            if let Some(m) = self.persons[i].life.mother
                && self.persons[i].life.father.is_none()
            {
                let born = self.persons[i].life.born;
                self.persons[i].life.father = self.father_for(bi, sp, m, born, now);
            }
            let genome = |id: Option<PersonId>| {
                id.and_then(|id| self.get(id))
                    .and_then(|q| q.genome.clone())
            };
            let mother = genome(self.persons[i].life.mother);
            let father = genome(self.persons[i].life.father);
            self.persons[i].inherit(genetics, mother.as_ref(), father.as_ref(), sun);
        }
    }

    /// Forms the psyche of every living member of a band who has none yet, from its phenotype;
    /// one who remembers nothing of the range starts from what the band knows of it.
    pub(crate) fn form(&mut self, bi: usize, defs: &PsycheDefs, now: Now) {
        let band = self.bands[bi].id;
        let places = self.bands[bi].places.clone();
        for p in self
            .persons
            .iter_mut()
            .filter(|p| p.social.band == band && p.alive())
        {
            if !p.psyche.formed {
                p.psyche = crate::psyche::Psyche::form(defs, p.phenotype.as_ref(), now.day);
            }
            if p.memory.places.is_empty() {
                p.memory.day = now.day;
                let known = [
                    (PlaceKind::Water, &places.water),
                    (PlaceKind::Sleep, &places.sleep),
                    (PlaceKind::Anvil, &places.anvils),
                    (PlaceKind::Food, &places.food),
                ];
                for (kind, list) in known {
                    for at in list {
                        p.memory.remember(kind, *at, now.day, 0.7);
                    }
                }
            }
        }
    }

    /// A place of a kind come to: remembered by the one who came to it, by those of its band who
    /// saw it there, and by the band.
    fn remember_place(&mut self, i: usize, bi: usize, kind: PlaceKind, at: DVec3, now: Now) {
        let band = self.persons[i].social.band;
        let from = self.persons[i].place.pos;
        for (j, q) in self.persons.iter_mut().enumerate() {
            if q.social.band != band || !q.alive() || q.tier != Tier::Full {
                continue;
            }
            if j == i {
                q.memory.remember(kind, at, now.day, 1.0);
            } else if (q.place.pos - from).length() < LEARN_M {
                q.memory.remember(kind, at, now.day, 0.6);
            }
        }
        let b = &mut self.bands[bi].places;
        match kind {
            PlaceKind::Water => Places::remember(&mut b.water, at, 10.0, 6),
            PlaceKind::Sleep => Places::remember(&mut b.sleep, at, 6.0, 12),
            PlaceKind::Food => Places::remember(&mut b.food, at, 15.0, 12),
            PlaceKind::Anvil => Places::remember(&mut b.anvils, at, 8.0, 8),
            PlaceKind::Danger => {}
        }
    }

    /// A father for a child born on day `born` to `mother`: one of her band's living grown males,
    /// grown when it was born and not of her close kin (her sons, brothers and father are passed
    /// over, as apes and people avoid them), drawn from the band's stream.
    fn father_for(
        &mut self,
        bi: usize,
        sp: &Species,
        mother: PersonId,
        born: f64,
        now: Now,
    ) -> Option<PersonId> {
        let band = self.bands[bi].id;
        let grown = sp.life.maturity_years as f64 * now.year_days;
        let fathers: Vec<PersonId> = {
            let mut kin = Kinship::near(&*self, NEAR_KIN);
            self.persons
                .iter()
                .filter(|q| q.social.band == band && q.alive() && !q.life.female)
                .filter(|q| q.stage(sp, &now) == Stage::Adult && q.life.born + grown <= born)
                .map(|q| q.id)
                .filter(|&q| kin.of(mother, q) < 0.125)
                .collect()
        };
        if fathers.is_empty() {
            return None;
        }
        let k = self.bands[bi].rng.below(fathers.len() as u32) as usize;
        Some(fathers[k])
    }

    /// Gives the band's young and half-grown without a mother one among its grown females old
    /// enough to have borne them, the least burdened first.
    fn mothers(&mut self, bi: usize, sp: &Species, now: Now) {
        let band = self.bands[bi].id;
        let maturity = sp.life.maturity_years as f64;
        let mut females: Vec<(PersonId, f64, usize)> = self
            .persons
            .iter()
            .filter(|p| p.social.band == band && p.alive() && p.life.female)
            .filter(|p| p.stage(sp, &now) == Stage::Adult)
            .map(|p| (p.id, p.age(&now), 0))
            .collect();
        for i in 0..self.persons.len() {
            let p = &self.persons[i];
            if p.social.band != band || !p.alive() || p.life.mother.is_some() {
                continue;
            }
            if p.stage(sp, &now) == Stage::Adult {
                continue;
            }
            let age = p.age(&now);
            let mother = females
                .iter_mut()
                .filter(|(_, a, _)| *a - age >= maturity)
                .min_by_key(|(id, _, k)| (*k, *id));
            if let Some((m, _, k)) = mother {
                *k += 1;
                self.persons[i].life.mother = Some(*m);
            }
        }
    }

    /// Sets a band's living members down on the ground they stand over (not in the water).
    pub(crate) fn onto_the_ground(&mut self, bi: usize, world: &dyn Senses, here: DVec3) {
        let band = self.bands[bi].id;
        for p in self
            .persons
            .iter_mut()
            .filter(|p| p.social.band == band && p.alive() && p.tier == Tier::Full)
        {
            let q = p.place.pos;
            p.place.pos = world
                .ground()
                .footing(q.x, q.z, here.y)
                .filter(|f| !f.water)
                .map_or(here, |f| DVec3::new(q.x, f.y, q.z));
        }
    }

    /// A place about `here`, from the band's stream.
    pub(crate) fn scatter(&mut self, bi: usize, here: DVec3, near: f64, far: f64) -> DVec3 {
        let rng = &mut self.bands[bi].rng;
        let a = rng.next_f64() * std::f64::consts::TAU;
        let d = near + rng.next_f64() * (far - near);
        DVec3::new(here.x + a.cos() * d, here.y, here.z + a.sin() * d)
    }

    /// A band met before, drawn out again as the same persons: those the cells lost while the
    /// player was away have died (the frailest first: the young of the year, the old, then the
    /// half-grown), those they gained are born to its mothers or join it; everyone back about its
    /// place, rested and fed if it has been long.
    #[allow(clippy::too_many_arguments)]
    fn redraw(
        &mut self,
        bi: usize,
        sp: &Species,
        genetics: Option<&Genetics>,
        sun: f32,
        graph: &Graph,
        want: Numbers,
        here: DVec3,
        now: Now,
    ) {
        let band = self.bands[bi].id;
        let living: Vec<usize> = (0..self.persons.len())
            .filter(|&i| self.persons[i].social.band == band && self.persons[i].alive())
            .collect();
        let have = living.len();
        let target = want.total();
        let old_age = sp.life.adult_death_years.0 as f64;
        if have > target {
            let mut frail: Vec<(f64, usize)> = living
                .iter()
                .filter(|&&i| self.persons[i].player.is_none())
                .map(|&i| {
                    let p = &self.persons[i];
                    let age = p.age(&now);
                    let base = if age < 1.0 {
                        3.0
                    } else if age > old_age {
                        2.0
                    } else if p.stage(sp, &now) != Stage::Adult {
                        1.0
                    } else {
                        0.0
                    };
                    // A recessive condition makes one frailer (V2.1 §4.2).
                    let burden = genetics.map_or(0.0, |g| g.recessive_burden as f64)
                        * p.phenotype.as_ref().map_or(0.0, |ph| ph.conditions as f64);
                    (base + burden + self.bands[bi].rng.next_f64(), i)
                })
                .collect();
            frail.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            for &(_, i) in frail.iter().take(have - target) {
                let p = &mut self.persons[i];
                p.life.died = Some(Died {
                    day: now.day,
                    cause: Cause::WhileAway,
                });
                p.record(
                    now.day,
                    Event::Died {
                        cause: Cause::WhileAway,
                    },
                );
            }
        }
        let away_days = self.bands[bi]
            .dormant_since
            .map_or(0.0, |d| (now.day - d).max(0.0));
        if target > have {
            let mothers: Vec<PersonId> = self
                .persons
                .iter()
                .filter(|p| p.social.band == band && p.alive() && p.life.female)
                .filter(|p| p.stage(sp, &now) == Stage::Adult)
                .map(|p| p.id)
                .collect();
            for k in 0..target - have {
                let pid = self.take_id();
                let pos = self.scatter(bi, here, 2.0, 10.0);
                let rng = &mut self.bands[bi].rng;
                let (female, born, event, mother) = if mothers.is_empty() {
                    // None to bear it: one of the dispersing sex has come from another band.
                    let female = match sp.social.disperses {
                        Disperser::Females => true,
                        Disperser::Males => false,
                        Disperser::Both => rng.next_f32() < 0.5,
                    };
                    let age = sp.life.maturity_years as f64 + 3.0 * rng.next_f64();
                    (
                        female,
                        now.day - age * now.year_days,
                        Event::Joined { band },
                        None,
                    )
                } else {
                    let since = away_days.min(now.year_days) * rng.next_f64();
                    (
                        rng.next_f32() < 0.5,
                        now.day - since,
                        Event::Born { band },
                        Some(mothers[k % mothers.len()]),
                    )
                };
                let knows = if self.bands[bi].culture.knowledge.is_empty() {
                    sp.knowledge.clone()
                } else {
                    self.bands[bi].culture.knowledge.clone()
                };
                let mut p = Person::new(
                    pid, sp, &knows, graph, band, female, born, pos, now, self.seed,
                );
                p.life.mother = mother;
                p.life.father = mother.and_then(|m| self.father_for(bi, sp, m, born, now));
                p.record(born.min(now.day), event);
                self.bands[bi].members.push(pid);
                self.persons.push(p);
            }
        }
        // Everyone back about its place.
        let members: Vec<usize> = (0..self.persons.len())
            .filter(|&i| self.persons[i].social.band == band && self.persons[i].alive())
            .collect();
        for i in members {
            let pos = self.scatter(bi, here, 2.0, 10.0);
            let p = &mut self.persons[i];
            p.tier = Tier::Full;
            p.place.pos = pos;
            p.place.medium = Medium::Ground;
            p.place.perch = None;
            p.place.speed = 0.0;
            p.mind = crate::mind::Mind::default();
            if away_days > REFRESH_DAYS {
                let seed = p.rng.next_u64();
                let injuries = std::mem::take(&mut p.body.injuries);
                p.body = hearth_body::Body::new(sp.body(p.life.female), seed);
                p.body.injuries = injuries;
            }
        }
        self.bands[bi].tier = Tier::Full;
        self.bands[bi].dormant_since = None;
        self.endow(bi, sp, genetics, sun, now);
        self.settle_households(bi);
        self.acquaint(bi, now.day);
    }

    /// Sets a band of a species down at a place (a test's, a screenshot's): its grown females and
    /// males, half-grown and young, knowing the places about it. Its id.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_band(
        &mut self,
        species: &SpeciesSet,
        graph: &Graph,
        items: &Items,
        world: &mut dyn World,
        si: usize,
        members: [u16; 4],
        at: DVec3,
        now: Now,
    ) -> u64 {
        let sp = &species.list[si];
        let id = 1_000_000_000 + self.next_id;
        let [females, males, juveniles, young] = members;
        let n = Numbers {
            young,
            juveniles,
            females,
            males,
        };
        let bi = self.found(sp, graph, id, n, at, DVec2::new(at.x, at.z), false, now);
        let genes = species.genetics.as_ref();
        let sun = genes.map_or(1.0, |g| g.sunlight(world.latitude(at)));
        self.endow(bi, sp, genes, sun, now);
        self.onto_the_ground(bi, &*world, at);
        know_about(&mut self.bands[bi], world, items, at);
        self.form(bi, &species.psyche, now);
        id
    }

    /// Lays a band to rest: its persons' records wait, dormant. Its numbers as the cells count
    /// them and the middle of where its living members were.
    pub fn rest(&mut self, band: u64, species: &SpeciesSet, now: Now) -> Option<(Numbers, DVec3)> {
        let bi = self.band_index(band)?;
        self.bands[bi].tier = Tier::Dormant;
        self.bands[bi].dormant_since = Some(now.day);
        let sp = species.get(&self.bands[bi].species)?;
        let n = self.numbers(band, sp, &now);
        let (mut sum, mut k) = (DVec3::ZERO, 0.0);
        for p in self.persons.iter_mut().filter(|p| p.social.band == band) {
            p.tier = Tier::Dormant;
            if p.alive() {
                sum += p.place.pos;
                k += 1.0;
            }
        }
        Some((n, if k > 0.0 { sum / k } else { DVec3::ZERO }))
    }

    /// Wakes a band met before at a place, its numbers being `want` now: the same persons,
    /// reconciled with those numbers (see `redraw`), back on the ground about the place, knowing
    /// what lies about it.
    #[allow(clippy::too_many_arguments)]
    pub fn wake(
        &mut self,
        band: u64,
        species: &SpeciesSet,
        graph: &Graph,
        world: &mut dyn World,
        items: &Items,
        want: Numbers,
        here: DVec3,
        now: Now,
    ) {
        let Some(bi) = self.band_index(band) else {
            return;
        };
        let Some(sp) = species.get(&self.bands[bi].species) else {
            return;
        };
        let genes = species.genetics.as_ref();
        let sun = genes.map_or(1.0, |g| g.sunlight(world.latitude(here)));
        self.redraw(bi, sp, genes, sun, graph, want, here, now);
        self.onto_the_ground(bi, &*world, here);
        know_about(&mut self.bands[bi], world, items, here);
        self.form(bi, &species.psyche, now);
    }

    /// Folds back into their numbers the bands whose persons are all beyond [`FAR_M`] of every
    /// player (or dead): their records wait, dormant; the cells count them again.
    pub fn fold(&mut self, eco: &mut Ecology, species: &SpeciesSet, players: &[DVec3], now: Now) {
        let wrap = eco.cells_around as f64 * CELL_M;
        let far: Vec<usize> = (0..self.bands.len())
            .filter(|&bi| self.bands[bi].tier == Tier::Full)
            .filter(|&bi| {
                let id = self.bands[bi].id;
                self.members(id)
                    .filter(|p| p.alive())
                    .all(|p| players.iter().all(|q| hdist(p.place.pos, *q, wrap) > FAR_M))
            })
            .collect();
        for bi in far {
            self.demote(bi, eco, species, now);
        }
    }

    /// Folds every band back (a save's copy: as if the player were far away).
    pub fn fold_all(&mut self, eco: &mut Ecology, species: &SpeciesSet, now: Now) {
        for bi in 0..self.bands.len() {
            if self.bands[bi].tier == Tier::Full {
                self.demote(bi, eco, species, now);
            }
        }
    }

    fn demote(&mut self, bi: usize, eco: &mut Ecology, species: &SpeciesSet, now: Now) {
        let id = self.bands[bi].id;
        let Some((n, middle)) = self.rest(id, species, now) else {
            return;
        };
        let wrap = eco.cells_around as f64 * CELL_M;
        let Some(gid) = self.bands[bi].population_group else {
            return;
        };
        let alive = n.total() > 0;
        for r in eco.regions.values_mut() {
            if let Some(g) = r.groups.iter_mut().find(|g| g.id == gid) {
                g.young = n.young;
                g.juveniles = n.juveniles;
                g.females = n.females;
                g.males = n.males;
                if alive {
                    g.pos = [middle.x.rem_euclid(wrap.max(1.0)), middle.z];
                }
                g.live = false;
            }
        }
    }

    /// The persons in full within `within` m of a place (a player's), as their client draws them.
    pub fn views(
        &self,
        species: &SpeciesSet,
        now: &Now,
        near: DVec3,
        within: f64,
    ) -> Vec<PersonView> {
        self.persons
            .iter()
            .filter(|p| p.tier == Tier::Full && p.player.is_none())
            .filter(|p| {
                p.life
                    .died
                    .as_ref()
                    .is_none_or(|d| now.day - d.day < LAID_TO_REST_DAYS)
            })
            .filter(|p| (p.place.pos - near).length() <= within)
            .filter_map(|p| {
                let sp = species.get(&p.species)?;
                Some(PersonView {
                    id: p.id,
                    plan: sp.plan,
                    female: p.life.female,
                    stage: p.stage(sp, now),
                    pos: p.place.pos,
                    yaw: p.place.yaw,
                    speed: p.place.speed,
                    medium: p.place.medium,
                    doing: p.mind.doing.clone(),
                    // To the centimetre and the hundredth, so a figure is made again only as it
                    // grows.
                    height_m: (p.height_m(sp, now) * 100.0).round() / 100.0,
                    dead: !p.alive(),
                    grown: ((p.age(now) as f32 / sp.life.maturity_years.max(1.0)).min(1.0) * 100.0)
                        .round()
                        / 100.0,
                    look: p.phenotype.as_ref().map_or_else(Default::default, |ph| {
                        crate::looks::look(
                            ph,
                            sp.plan,
                            p.life.female,
                            p.age(now) as f32,
                            sp.life.maturity_years,
                        )
                    }),
                    shows: p
                        .psyche
                        .shown(&species.psyche)
                        .map(|(f, v)| (species.psyche.feeling(f).display, v)),
                })
            })
            .collect()
    }

    /// Lives `dt` seconds of play: the bands come to tolerate each calm player about them or
    /// unlearn it; every person in full decides (in parallel, from the step's start) and then
    /// acts (in order); the bodies step.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        species: &SpeciesSet,
        crafts: &Crafts,
        graph: &Graph,
        content: &Content,
        items: &Items,
        world: &mut dyn World,
        players: &[PlayerSeen],
        now: Now,
        dt: f32,
    ) {
        self.done.clear();
        // Bands met again from before cultures (or drawn out without one) are given theirs.
        for bi in 0..self.bands.len() {
            if !self.bands[bi].culture.drawn()
                && let Some(sp) = species.get(&self.bands[bi].species)
            {
                self.enculture(bi, sp, now.day);
            }
        }
        self.live_course(species, items, &*world, now);
        self.body_s -= dt;
        let body_step = self.body_s <= 0.0;
        if body_step {
            self.body_s += BODY_S;
            // Time together draws people closer (V2.1 §8.2), and those close talk (§8.4).
            self.keep_company(BODY_S, now.day);
            self.talk(BODY_S, now.day);
        }
        self.habituate(species, players, dt);
        // The others as the step finds them.
        let glimpses: Vec<Glimpse> = self
            .persons
            .iter()
            .filter(|p| p.tier == Tier::Full)
            .map(|p| Glimpse {
                id: p.id,
                band: p.social.band,
                pos: p.place.pos,
                grown: species
                    .get(&p.species)
                    .is_some_and(|sp| p.stage(sp, &now) == Stage::Adult),
                young: species.get(&p.species).is_some_and(|sp| {
                    matches!(
                        p.life_stage(sp, &now),
                        LifeStage::Child | LifeStage::Juvenile
                    )
                }),
                alive: p.alive(),
                alarm: matches!(p.mind.doing, Doing::Alarm { .. }),
                working: match p.mind.doing {
                    Doing::Working { recipe } => Some(recipe),
                    _ => None,
                },
                hungry: species.get(&p.species).is_some_and(|sp| {
                    Needs::of(&p.body, &p.body_config(sp, &now), 0.0).hunger > 0.35
                }),
                household: p.social.household,
                hurt: p.body.injuries.iter().any(|i| i.healed < 1.0),
                shows: p.psyche.shown(&species.psyche),
                greeting: match p.mind.doing {
                    Doing::Greeting { who } => Some(who),
                    _ => None,
                },
                warning: match p.mind.doing {
                    Doing::WarningOff { who } => Some(who),
                    _ => None,
                },
            })
            .collect();
        // Whose country is crowded past what their ways bear with strangers, and who is whose
        // guest (V2.1 §8.7).
        let crowded: Vec<u64> = (0..self.bands.len())
            .filter(|&bi| {
                let b = &self.bands[bi];
                b.tier == Tier::Full
                    && species
                        .get(&b.species)
                        .and_then(|sp| b.culture.ways.as_ref().or(sp.ways.as_ref()))
                        .zip(species.life.of(&b.species))
                        .is_some_and(|(w, t)| self.crowd(bi, t) > w.warn_off_crowding)
            })
            .map(|bi| self.bands[bi].id)
            .collect();
        let guests: Vec<(PersonId, u64)> = self
            .bands
            .iter()
            .filter(|b| b.tier == Tier::Full)
            .flat_map(|b| b.guests.iter().map(move |g| (g.who, b.id)))
            .collect();
        // 1. Deciding, everyone at once.
        {
            let senses: &dyn Senses = &*world;
            let bands = &self.bands;
            self.persons
                .par_iter_mut()
                .filter(|p| p.tier == Tier::Full && p.alive() && p.player.is_none())
                .for_each(|p| {
                    let Some(sp) = species.get(&p.species) else {
                        return;
                    };
                    let Some(band) = bands.iter().find(|b| b.id == p.social.band) else {
                        return;
                    };
                    let hosts: Vec<u64> = guests
                        .iter()
                        .filter(|(g, _)| *g == p.id)
                        .map(|(_, b)| *b)
                        .collect();
                    decide(
                        p,
                        sp,
                        &species.psyche,
                        &species.norms,
                        band,
                        &glimpses,
                        senses,
                        crafts,
                        content,
                        items,
                        players,
                        crowded.contains(&band.id),
                        &hosts,
                        now,
                        dt,
                    );
                });
        }
        // 2. Acting, one at a time in the persons' order.
        for i in 0..self.persons.len() {
            let p = &self.persons[i];
            // A player's person is moved by the player.
            if p.tier != Tier::Full || p.player.is_some() {
                continue;
            }
            if !p.alive() && p.body.dead.is_none() {
                continue;
            }
            let Some(sp) = species.get(&p.species) else {
                continue;
            };
            // A body that died otherwise (a hunter's kill, a fall): its death recorded.
            if p.life.died.is_none() && p.body.dead.is_some() {
                self.died(i, sp, items, now);
                continue;
            }
            if !p.alive() {
                continue;
            }
            let Some(bi) = self.band_index(p.social.band) else {
                continue;
            };
            if let Some(from) = self.persons[i].mind.stingy_to.take() {
                let p = &self.persons[i];
                self.deeds.push(crate::repute::Seen {
                    who: Who::Person(p.id),
                    deed: crate::repute::Deed::Withheld { from },
                    at: p.place.pos,
                });
            }
            self.act(i, bi, sp, crafts, content, items, world, now, dt);
            let p = &mut self.persons[i];
            if body_step {
                let sized = p.body_config(sp, &now);
                let cfg: &BodyConfig = &sized;
                let activity = cfg.activity(activity_of(&p.mind.doing, p.place.speed));
                let exposure = world.exposure(p.place.pos, p.place.medium == Medium::Tree);
                let hurt = p.body.injuries.len();
                p.body
                    .step(cfg, BODY_S as f64, &exposure, &sp.coat, &activity);
                // A new hurt frightens and angers; the body's needs and pains strain it.
                if p.body.injuries.len() > hurt {
                    p.psyche.feel(Feeling::Fear, 0.6);
                    p.psyche.feel(Feeling::Anger, 0.4);
                    let at = p.place.pos;
                    p.memory.happened(Happened::Hurt, at, now.day, 0.7);
                    p.record(now.day, Event::Hurt);
                }
                let n = Needs::of(&p.body, cfg, 0.0);
                let pain = (p.body.injuries.len() as f32 * 0.2).min(0.6);
                p.mind.strain =
                    (0.35 * n.hunger + 0.35 * n.thirst + 0.15 * n.tiredness + pain).min(1.0);
                if p.body.dead.is_some() {
                    self.died(i, sp, items, now);
                }
            }
        }
        // Quarrels (V2.1 §8.6): grievances had out, those under way carried on.
        if body_step {
            self.grudges(species, &now);
        }
        self.quarrels_step(species, items, content, world, now, dt);
        // 3. The deeds of the step, seen by those near: what they think of their doers moved;
        // work done well before them, respected.
        let deeds = std::mem::take(&mut self.deeds);
        for seen in deeds {
            if let (crate::memory::Who::Person(who), crate::repute::Deed::Shared { .. }) =
                (seen.who, &seen.deed)
            {
                self.respect(who, seen.at, 0.05, now.day);
            }
            self.deed(seen, &species.norms, light(now.hour), now.day);
        }
        let worked: Vec<(PersonId, DVec3)> = self.done.iter().map(|d| (d.person, d.at)).collect();
        for (who, at) in worked {
            self.respect(who, at, 0.01, now.day);
        }
        // Each evening, a band whose camp has gone poor holds council on where to keep it.
        if (17.0..18.0).contains(&now.hour) {
            let today = now.day.floor();
            let hungry: Vec<bool> = self
                .persons
                .iter()
                .map(|p| {
                    species.get(&p.species).is_some_and(|sp| {
                        Needs::of(&p.body, &p.body_config(sp, &now), 0.0).hunger > 0.35
                    })
                })
                .collect();
            for bi in 0..self.bands.len() {
                let Some(sp) = species.get(&self.bands[bi].species) else {
                    continue;
                };
                let keeps_camp = sp.does(Behavior::KeepCamp);
                if (!keeps_camp && sp.ways.is_none())
                    || self.bands[bi].tier != Tier::Full
                    || self.bands[bi].weighed >= today
                {
                    continue;
                }
                self.bands[bi].weighed = today;
                if keeps_camp {
                    self.council(bi, now, &|i| hungry[i]);
                }
                // Its guests weighed: one long among them whom most trust, taken in.
                if let Some(w) = self.ways_of(bi, sp) {
                    self.weigh_guests(bi, &w, &now);
                }
            }
        }
        // 4. The young watching the grown at their work take in what it shows, as the player
        // does by watching (V2-11): now and then as they watch, about once a minute, and the
        // more often as a work is finished before them — insight toward its knowledge and what
        // that rests on, and in time the knowing of it.
        let working: Vec<(PersonId, DVec3)> = self
            .persons
            .iter()
            .filter(|q| q.tier == Tier::Full && q.alive())
            .filter(|q| matches!(q.mind.doing, Doing::Working { .. }))
            .map(|q| (q.id, q.place.pos))
            .collect();
        for q in self.persons.iter_mut() {
            let Doing::Imitating { whom, recipe, .. } = q.mind.doing else {
                continue;
            };
            if q.tier != Tier::Full || !q.alive() {
                continue;
            }
            let by = |at: DVec3| (q.place.pos - at).length() < BY_M * 2.0;
            let finished = self.done.iter().any(|d| d.person == whom && by(d.at));
            let at_it = working.iter().any(|(id, at)| *id == whom && by(*at));
            let chance = if finished {
                TAKEN_IN
            } else if at_it {
                dt / 60.0
            } else {
                0.0
            };
            if chance > 0.0 && q.rng.next_f32() < chance {
                for t in crate::watch::watched(graph, crafts, recipe) {
                    q.knowledge.observe(
                        graph,
                        &t,
                        now.tick,
                        hearth_craft::knowledge::Mode::Discovery,
                    );
                }
            }
        }
    }

    /// Records a person's death (its body's cause), and its band's mourning.
    fn died(&mut self, i: usize, sp: &Species, items: &Items, now: Now) {
        let p = &mut self.persons[i];
        let Some(d) = &p.body.dead else {
            return;
        };
        if p.life.died.is_some() {
            return;
        }
        let cause = Cause::Body(format!("{d:?}"));
        p.life.died = Some(Died {
            day: now.day,
            cause: cause.clone(),
        });
        p.record(now.day, Event::Died { cause });
        let (dead, band, household) = (p.id, p.social.band, p.social.household);
        self.laid_to_rest(i, now.day);
        self.bequeath(i, now.day, sp, items, &now);
        self.mourn(dead, band, now.day);
        if let Some(h) = household {
            self.rehome(h, band, now.day, now.year_days.max(1.0));
        }
        self.forget_dead(dead);
    }

    /// A death felt by the dead one's band: grief in its kin by how close they were (a mother,
    /// a child, a brother or sister most; a grandparent or a half-sibling half as much).
    pub(crate) fn mourn(&mut self, dead: PersonId, band: u64, day: f64) {
        let kin: Vec<(usize, f64)> = {
            let mut k = Kinship::near(&*self, NEAR_KIN);
            (0..self.persons.len())
                .filter(|&j| {
                    let q = &self.persons[j];
                    q.social.band == band && q.alive() && q.id != dead
                })
                .map(|j| (j, k.of(dead, self.persons[j].id)))
                .filter(|(_, r)| *r >= 0.1)
                .collect()
        };
        for (j, r) in kin {
            let q = &mut self.persons[j];
            let grief = (4.0 * r).min(1.0) as f32;
            q.psyche.feel(Feeling::Grief, grief);
            let at = q.place.pos;
            let felt = q.psyche.feeling(Feeling::Grief);
            q.memory
                .happened(Happened::Loss { who: dead }, at, day, felt);
            if r >= 0.25 {
                q.record(day, Event::Mourned { who: dead });
            }
        }
    }

    /// A band comes in time to tolerate each player who keeps calm within its sight, and soon
    /// unlearns it of one who runs at it or hunts near it.
    fn habituate(&mut self, species: &SpeciesSet, players: &[PlayerSeen], dt: f32) {
        for b in self.bands.iter_mut().filter(|b| b.tier == Tier::Full) {
            if !species
                .get(&b.species)
                .is_some_and(|sp| sp.does(Behavior::Habituate))
            {
                continue;
            }
            for p in players {
                let noticed = SEE_M * p.plain.clamp(0.1, 1.0) as f64;
                let seen = self.persons.iter().any(|a| {
                    a.social.band == b.id
                        && a.tier == Tier::Full
                        && a.alive()
                        && a.mind.doing != Doing::Sleeping
                        && (a.place.pos - p.pos).length() < noticed
                });
                if !seen {
                    continue;
                }
                let t = b.tolerance_of(p.id);
                let t = if p.running || p.hunting {
                    t - dt / UNLEARN_S
                } else {
                    t + dt / HABITUATE_S
                };
                b.set_tolerance(p.id, t);
            }
        }
    }

    /// Does what a person chose, for `dt`.
    #[allow(clippy::too_many_arguments)]
    fn act(
        &mut self,
        i: usize,
        bi: usize,
        sp: &Species,
        crafts: &Crafts,
        content: &Content,
        items: &Items,
        world: &mut dyn World,
        now: Now,
        dt: f32,
    ) {
        let doing = self.persons[i].mind.doing.clone();
        let sized = self.persons[i].body_config(sp, &now);
        let cfg: &BodyConfig = &sized;
        // Its gaits by its size; an elder's slower.
        let elder = if self.persons[i].life_stage(sp, &now) == LifeStage::Elder {
            0.85
        } else {
            1.0
        };
        let walk = cfg.params.walk_m_s * elder;
        let run = cfg.params.jog_m_s * 1.5 * elder;
        match doing {
            Doing::Going { to, then } => {
                if self.persons[i].place.medium == Medium::Tree {
                    climb_down(&mut self.persons[i], &*world, dt);
                    return;
                }
                if move_toward(&mut self.persons[i], &*world, to, walk, dt) {
                    let p = &mut self.persons[i];
                    p.place.speed = 0.0;
                    p.mind.doing = match then {
                        Intent::Feed => Doing::Feeding,
                        Intent::Drink => Doing::Drinking,
                        Intent::Work(r) => Doing::Working { recipe: r },
                        Intent::Nest => Doing::Nesting,
                        Intent::Rejoin | Intent::Roam | Intent::Step => Doing::Idle,
                    };
                    p.mind.timer = hold_for(&p.mind.doing);
                    if then == Intent::Step {
                        advance(p, &Step::Go(to));
                        p.mind.timer = 0.0;
                    }
                    if then == Intent::Drink {
                        let w = p.place.pos;
                        self.remember_place(i, bi, PlaceKind::Water, w, now);
                    }
                }
            }
            Doing::Feeding => {
                let p = &mut self.persons[i];
                p.place.speed = 0.0;
                // What its culture forbids it leaves be, unless starving.
                let starving = Needs::of(&p.body, cfg, 0.0).hunger >= 1.0;
                let culture = &self.bands[bi].culture;
                let food = world
                    .food_at(p.place.pos)
                    .filter(|f| starving || !culture.forbids(&f.material));
                if let Some(f) = food {
                    let hunger = Needs::of(&p.body, cfg, 0.0).hunger;
                    p.psyche.feel(Feeling::Joy, 0.3 * hunger);
                    eat(p, cfg, content, &f.material, f.kg_min * PER_MIN * dt);
                    // A handful of marula stones to crack later, now and then.
                    if let Some(nuts) = &f.nuts
                        && p.rng.next_f32() < 0.02 * dt
                        && p.stage(sp, &now) == Stage::Adult
                        && let Some(id) = items_of(items, nuts)
                    {
                        let mass = p.mass_kg(sp, &now);
                        let _ = p.possessions.carry.stow(items, Stack::of(&id, 4), mass);
                    }
                    let here = p.place.pos;
                    self.remember_place(i, bi, PlaceKind::Food, here, now);
                } else {
                    // Nothing here after all (eaten, out of season): the place let go.
                    let p = &mut self.persons[i];
                    let here = p.place.pos;
                    p.memory.forget(PlaceKind::Food, here);
                    p.mind.timer = 0.0;
                }
                if Needs::of(&self.persons[i].body, cfg, 0.0).hunger <= 0.0 {
                    self.persons[i].mind.timer = 0.0;
                }
            }
            Doing::Drinking => {
                let p = &mut self.persons[i];
                p.place.speed = 0.0;
                p.body.drink(cfg, DRINK_L_S * dt as f64, 0.0, 0.0);
                if Needs::of(&p.body, cfg, 0.0).thirst <= 0.0 {
                    p.mind.timer = 0.0;
                }
            }
            Doing::Working { recipe } => {
                self.persons[i].place.speed = 0.0;
                self.work(i, bi, recipe, sp, crafts, content, items, world, now, dt);
            }
            Doing::Nesting => {
                let p = &mut self.persons[i];
                if p.place.medium != Medium::Tree || p.climbing() {
                    // Up the tree to its place in the crown first.
                    if !climb_up(p, &*world, dt) {
                        p.place.speed = 0.0;
                        p.mind.doing = Doing::Sleeping;
                    }
                    return;
                }
                p.place.speed = 0.0;
                if p.mind.timer <= hold_for(&Doing::Nesting) * 0.5 {
                    let at = p.place.pos;
                    if let Some(bed) = world.nest(at) {
                        p.place.pos = bed;
                        p.place.perch = Some(bed);
                    }
                    p.mind.doing = Doing::Sleeping;
                    p.mind.timer = hold_for(&Doing::Sleeping);
                    self.remember_place(i, bi, PlaceKind::Sleep, at, now);
                }
            }
            Doing::Sleeping => {
                let p = &mut self.persons[i];
                p.place.speed = 0.0;
                // Morning (by its kind's routine): up.
                if !sp.asleep_at(now.hour) {
                    p.mind.timer = 0.0;
                }
            }
            Doing::Grooming { other } => {
                // Over to the one of its band it is fondest of near (whom it grooms, and talks
                // with), then grooming them.
                let partner = other.or_else(|| {
                    let p = &self.persons[i];
                    let (me, band, pos) = (p.id, p.social.band, p.place.pos);
                    self.persons
                        .iter()
                        .filter(|q| {
                            q.id != me
                                && q.alive()
                                && q.tier == Tier::Full
                                && q.social.band == band
                                && (q.place.pos - pos).length() < GROOM_M
                        })
                        .max_by(|a, b| {
                            let fond = |q: &Person| {
                                p.social
                                    .ties
                                    .iter()
                                    .find(|t| t.who == q.id)
                                    .map_or(0.0, |t| t.affection)
                            };
                            fond(a).total_cmp(&fond(b)).then(b.id.cmp(&a.id))
                        })
                        .map(|q| q.id)
                });
                let at = partner
                    .and_then(|o| self.persons.binary_search_by_key(&o, |q| q.id).ok())
                    .map(|j| self.persons[j].place.pos);
                let p = &mut self.persons[i];
                if let Doing::Grooming { other } = &mut p.mind.doing {
                    *other = partner;
                }
                match at {
                    Some(at) if (p.place.pos - at).length() > 1.2 => {
                        move_toward(p, &*world, at, walk, dt);
                    }
                    _ => {
                        p.place.speed = 0.0;
                        if let Some(at) = at {
                            p.place.yaw = yaw_toward(p.place.pos, at);
                        }
                        p.psyche.feel(Feeling::Affection, 0.35);
                    }
                }
            }
            Doing::Taking { id } => {
                let p = &mut self.persons[i];
                p.place.speed = 0.0;
                let mass = p.mass_kg(sp, &now);
                match world.things().take(id, None) {
                    Some(stack) => match p.possessions.carry.stow(items, stack, mass) {
                        Ok(()) => advance(p, &Step::Take(id)),
                        Err(stack) => {
                            // Hands and basket full: it is put back, and the plan made again.
                            world.things().lay(p.place.pos, stack);
                            p.mind.plan = None;
                        }
                    },
                    // Gone: the plan made again.
                    None => p.mind.plan = None,
                }
                p.mind.doing = Doing::Idle;
                p.mind.timer = 0.0;
            }
            Doing::Resting | Doing::Idle => {
                self.persons[i].place.speed = 0.0;
            }
            Doing::Playing { to, with } => {
                // Off after the other (or about on its own), never far from its mother; caught
                // up, away again.
                let p = &mut self.persons[i];
                if move_toward(p, &*world, to, run * 0.5, dt) {
                    let a = p.rng.next_f64() * std::f64::consts::TAU;
                    let at = |id: Option<u64>| {
                        id.and_then(|w| self.persons.binary_search_by_key(&w, |q| q.id).ok())
                            .map(|j| self.persons[j].place.pos)
                    };
                    let me = self.persons[i].place.pos;
                    let there = at(with).unwrap_or(me);
                    let anchor = at(self.persons[i].life.mother).unwrap_or(me);
                    let mut next = there + DVec3::new(a.cos(), 0.0, a.sin()) * 4.0;
                    let off = next - anchor;
                    if off.length() > PLAY_ABOUT_M {
                        next = anchor + off.normalize() * PLAY_ABOUT_M;
                    }
                    self.persons[i].mind.doing = Doing::Playing { to: next, with };
                }
            }
            Doing::Sharing { to, .. } => {
                // To the hungry one; there, what it carries of food given into their hands.
                let Ok(j) = self.persons.binary_search_by_key(&to, |q| q.id) else {
                    self.persons[i].mind.doing = Doing::Idle;
                    return;
                };
                let at = self.persons[j].place.pos;
                let p = &mut self.persons[i];
                if (p.place.pos - at).length() > 1.5 {
                    move_toward(p, &*world, at, walk, dt);
                    return;
                }
                p.place.speed = 0.0;
                p.place.yaw = yaw_toward(p.place.pos, at);
                if let Some(path) = carried_food(p, items, content)
                    && let Some(stack) = p.possessions.carry.take(items, &path, Some(1))
                {
                    let worth = items
                        .get(&stack.id)
                        .and_then(|k| hearth_craft::food::bite_of(content, k, &stack))
                        .map_or(0.1, |b| (b.kcal / 2000.0).max(0.05));
                    let from = p.id;
                    let q = &mut self.persons[j];
                    let qcfg = species_body(q, sp, &now);
                    if let Some(b) = items
                        .get(&stack.id)
                        .and_then(|k| hearth_craft::food::bite_of(content, k, &stack))
                    {
                        let _ = q.body.eat(
                            &qcfg,
                            &Food {
                                kcal: b.kcal as f64,
                                protein_g: b.protein_g as f64,
                                fat_g: b.fat_g as f64,
                                carb_g: b.carb_g as f64,
                                water_l: b.water_l as f64,
                                volume_l: b.volume_l as f64,
                                fresh_days: b.fresh_days as f64,
                            },
                        );
                    }
                    q.psyche.feel(Feeling::Affection, 0.4);
                    self.give(from, to, worth, now.day);
                    self.persons[i].psyche.feel(Feeling::Joy, 0.2);
                    let at = self.persons[i].place.pos;
                    self.deeds.push(crate::repute::Seen {
                        who: Who::Person(from),
                        deed: crate::repute::Deed::Shared { with: to },
                        at,
                    });
                }
                self.persons[i].mind.doing = Doing::Idle;
                self.persons[i].mind.timer = 0.0;
            }
            Doing::Tending { who, .. } => {
                // By the hurt one's side, crouched with them: their fear eased, the tie warmed.
                let Ok(j) = self.persons.binary_search_by_key(&who, |q| q.id) else {
                    self.persons[i].mind.doing = Doing::Idle;
                    return;
                };
                let at = self.persons[j].place.pos;
                let healed = !self.persons[j].alive()
                    || self.persons[j]
                        .body
                        .injuries
                        .iter()
                        .all(|inj| inj.healed >= 1.0);
                let p = &mut self.persons[i];
                if healed {
                    p.mind.doing = Doing::Idle;
                    p.mind.timer = 0.0;
                    return;
                }
                if (p.place.pos - at).length() > 1.2 {
                    move_toward(p, &*world, at, walk, dt);
                    return;
                }
                p.place.speed = 0.0;
                p.place.yaw = yaw_toward(p.place.pos, at);
                let from = p.id;
                let q = &mut self.persons[j];
                q.psyche.feelings[Feeling::Fear] *= 1.0 - (0.05 * dt).min(1.0);
                q.psyche.feel(Feeling::Affection, 0.2);
                self.give(from, who, 0.01 * dt, now.day);
            }
            Doing::Mocking { who, at } => {
                // Up to them, and to their face: they feel shame and the less fond of the
                // mocker; the indignation spent.
                let at = self
                    .persons
                    .binary_search_by_key(&who, |q| q.id)
                    .map_or(at, |j| self.persons[j].place.pos);
                let p = &mut self.persons[i];
                if (p.place.pos - at).length() > 4.0 {
                    move_toward(p, &*world, at, walk, dt);
                    return;
                }
                p.place.speed = 0.0;
                p.place.yaw = yaw_toward(p.place.pos, at);
                let me = p.id;
                let near = (p.place.pos - at).length() < 6.0;
                p.psyche.feelings[Feeling::Indignation] *= 0.6;
                if near && let Ok(j) = self.persons.binary_search_by_key(&who, |q| q.id) {
                    let q = &mut self.persons[j];
                    q.psyche.feel(Feeling::Shame, 0.5);
                    if let Some(t) = q.social.ties.iter_mut().find(|t| t.who == me) {
                        t.affection = (t.affection - 0.05).max(0.0);
                    }
                    self.aggrieve(j, me, 0.08, now.day);
                }
                let p = &mut self.persons[i];
                p.mind.doing = Doing::Idle;
                p.mind.timer = 2.0;
            }
            Doing::Greeting { who } => {
                // Over to the stranger; there, the greeting.
                let Some(j) = self.index_of_person(who) else {
                    let p = &mut self.persons[i];
                    p.mind.doing = Doing::Idle;
                    p.mind.timer = 0.0;
                    return;
                };
                let at = self.persons[j].place.pos;
                let p = &mut self.persons[i];
                if (p.place.pos - at).length() > 2.0 {
                    move_toward(p, &*world, at, walk, dt);
                    return;
                }
                p.place.speed = 0.0;
                p.place.yaw = yaw_toward(p.place.pos, at);
                p.mind.doing = Doing::Idle;
                p.mind.timer = 0.0;
                if let Some(w) = self.ways_of(bi, sp) {
                    self.greet(i, j, &w, now.day);
                }
            }
            Doing::WarningOff { who } => {
                // Up to the stranger, shouting, the body made big — the first shout a wrong to
                // it; one still here when the warning is spent is threatened.
                let Some(j) = self.index_of_person(who) else {
                    let p = &mut self.persons[i];
                    p.mind.doing = Doing::Idle;
                    p.mind.timer = 0.0;
                    return;
                };
                let at = self.persons[j].place.pos;
                let p = &mut self.persons[i];
                let d = (p.place.pos - at).length();
                if d > 6.0 {
                    move_toward(p, &*world, at, walk, dt);
                    return;
                }
                p.place.speed = 0.0;
                p.place.yaw = yaw_toward(p.place.pos, at);
                if p.rng.next_f32() < 0.4 * dt {
                    world.call(p.place.pos, true);
                }
                let me = p.id;
                let fresh = !p.life.events.iter().rev().take(8).any(|e| {
                    now.day - e.day < 0.05
                        && matches!(e.event, Event::WarnedOff { who: w } if w == who)
                });
                if fresh {
                    p.record(now.day, Event::WarnedOff { who });
                    self.persons[j].record(now.day, Event::Unwelcome { by: me });
                    self.aggrieve(j, me, 0.1, now.day);
                }
                let stays = self.ways_of(bi, sp).is_some_and(|w| d < w.greet_m);
                if self.persons[i].mind.timer < 1.0
                    && stays
                    && self.persons[j].age(&now) >= crate::conflict::GROWN
                {
                    self.quarrel(me, who, crate::conflict::Rung::Threat, now.day);
                }
            }
            Doing::Quarrelling { .. } | Doing::Mediating { .. } => {
                // Held to it by its quarrel (`conflict.rs`); with none under way, it is over.
                let me = self.persons[i].id;
                if !self
                    .quarrels
                    .iter()
                    .any(|q| q.a == me || q.b == me || q.mediator == Some(me))
                {
                    let p = &mut self.persons[i];
                    p.mind.doing = Doing::Idle;
                    p.mind.timer = 0.0;
                }
            }
            Doing::Imitating { at, recipe, .. } => {
                // Over to the work, then crouched by it, watching it and trying it after:
                // practice, at half the rate of doing it.
                let p = &mut self.persons[i];
                let off = p.place.pos - at;
                if off.x.hypot(off.z) > BY_M {
                    move_toward(p, &*world, at, walk, dt);
                    return;
                }
                p.place.speed = 0.0;
                p.place.yaw = yaw_toward(p.place.pos, at);
                if let Some(skill) = crafts.recipes[recipe].def.skill.clone() {
                    p.knowledge.practice(&skill, dt * IMITATE_H_PER_S, now.tick);
                }
            }
            Doing::Carried { by } => {
                // On its carer's hip, or asleep at its side (in the nest with it at night);
                // nursed when it wants.
                let Ok(c) = self.persons.binary_search_by_key(&by, |q| q.id) else {
                    return;
                };
                let carer = self.persons[c].place.clone();
                let asleep = self.persons[c].mind.doing == Doing::Sleeping;
                let hip = 0.42 * self.persons[c].height_m(sp, &now) as f64;
                let p = &mut self.persons[i];
                let right = DVec3::new(carer.yaw.cos() as f64, 0.0, -carer.yaw.sin() as f64);
                p.place = carer;
                p.place.speed = 0.0;
                if asleep {
                    p.place.pos += right * 0.3;
                    p.mind.doing = Doing::Sleeping;
                } else {
                    p.place.pos += right * 0.16 + DVec3::Y * hip;
                }
                let n = Needs::of(&p.body, cfg, 0.0);
                if n.hunger > 0.2 || n.thirst > 0.2 {
                    let room = p.body.energy.room_l(cfg.params.stomach_capacity_l as f64);
                    let _ = p.body.eat(cfg, &milk(room.min(0.15)));
                }
            }
            Doing::Watching { at } => {
                let p = &mut self.persons[i];
                p.place.speed = 0.0;
                p.place.yaw = yaw_toward(p.place.pos, at);
            }
            Doing::Alarm { at } => {
                let p = &mut self.persons[i];
                p.place.speed = 0.0;
                p.place.yaw = yaw_toward(p.place.pos, at);
                if p.mind.timer > hold_for(&Doing::Alarm { at }) * 0.6 {
                    world.call(p.place.pos, true);
                    p.mind.timer = hold_for(&Doing::Alarm { at }) * 0.6;
                }
            }
            Doing::Mobbing { at } => {
                // Up to a stone's throw, shouting and brandishing.
                let p = &mut self.persons[i];
                let d = (at - p.place.pos).length();
                if d > 9.0 {
                    let to = at + (p.place.pos - at).normalize_or(DVec3::X) * 8.0;
                    move_toward(p, &*world, to, walk, dt);
                } else {
                    p.place.speed = 0.0;
                    p.place.yaw = yaw_toward(p.place.pos, at);
                    if p.rng.next_f32() < 0.5 * dt {
                        world.call(p.place.pos, true);
                    }
                }
            }
            Doing::Fleeing { to } => {
                let p = &mut self.persons[i];
                if p.place.medium == Medium::Tree {
                    if p.climbing() {
                        climb_up(p, &*world, dt);
                    } else {
                        p.place.speed = 0.0;
                    }
                    return;
                }
                if move_toward(p, &*world, to, run, dt)
                    && trunk_near(world.ground(), p.place.pos, 2.0).is_some()
                {
                    climb_up(p, &*world, dt);
                }
            }
        }
    }

    /// Works a process at hand: the engine's plan with what it carries and what lies within
    /// reach, done to the thing it needs (an anvil) when there is one; finished when its time is
    /// up, what it made eaten, kept or laid down.
    #[allow(clippy::too_many_arguments)]
    fn work(
        &mut self,
        i: usize,
        bi: usize,
        recipe: usize,
        sp: &Species,
        crafts: &Crafts,
        content: &Content,
        items: &Items,
        world: &mut dyn World,
        now: Now,
        dt: f32,
    ) {
        let pos = self.persons[i].place.pos;
        // The hammerstone left by the anvil, into its hand.
        let lying = world.things_near(pos, REACH_M);
        let took = take_up_tools(
            &mut self.persons[i],
            sp,
            crafts,
            recipe,
            &lying,
            items,
            &now,
        );
        for id in &took {
            world.things().take(*id, None);
        }
        let lying = world.things_near(pos, REACH_M);
        // A step of its plan: done to what the plan aims at.
        let planned: Option<Option<Aimed>> =
            self.persons[i]
                .mind
                .plan
                .as_ref()
                .and_then(|pl| match pl.step() {
                    Some(Step::Do { recipe: r, aim }) if *r == recipe => Some(aim.clone()),
                    _ => None,
                });
        let aimed = match &planned {
            Some(aim) => aim.clone(),
            None => aim_for(crafts, content, recipe, &lying, items),
        };
        let at_anvil = matches!(aimed, Some(Aimed::Thing(_)));
        let around = world.surroundings(pos);
        let plan = plan_work(
            crafts,
            content,
            items,
            &self.persons[i],
            recipe,
            &lying,
            aimed.clone(),
            around.clone(),
        );
        let Ok(plan) = plan else {
            // It cannot here (the nuts eaten, the stone gone): it puts down what it took up and
            // looks about again; a plan it was following is made again.
            if planned.is_some() {
                self.persons[i].mind.plan = None;
            } else {
                lay_down_tools(&mut self.persons[i], world, pos);
            }
            self.persons[i].mind.doing = Doing::Idle;
            return;
        };
        let p = &mut self.persons[i];
        // The work's length: its real hours on the day's scale, as the player's.
        let cfg = sp.body(p.life.female);
        let total = (plan.hours as f64 * 3600.0 * cfg.scales.factor(plan.scale)) as f32;
        let total = total.clamp(4.0, 120.0);
        if p.mind.timer > total {
            p.mind.timer = total;
        }
        if p.mind.timer > dt {
            return;
        }
        let mut rng = Rng::new(p.rng.next_u64());
        let outcome = finish_work(
            crafts,
            content,
            items,
            sp,
            p,
            &now,
            &plan,
            &lying,
            aimed,
            around,
            world.things(),
            &mut rng,
        );
        if outcome.done {
            if at_anvil {
                self.remember_place(i, bi, PlaceKind::Anvil, pos, now);
            }
            let p = &mut self.persons[i];
            p.psyche.feel(Feeling::Joy, 0.25);
            p.psyche.feel(Feeling::Pride, 0.3);
        }
        // Following a plan: the step done (what it did to its target told to the world), or the
        // plan made again from where things stand (the point snapped in the knapping).
        if let Some(aim) = planned {
            let p = &mut self.persons[i];
            if outcome.done {
                let target_at = p.mind.plan.as_ref().and_then(|pl| {
                    match pl.steps.get(pl.next.wrapping_sub(1)) {
                        Some(Step::Go(at)) => Some(*at),
                        _ => None,
                    }
                });
                let step = Step::Do {
                    recipe,
                    aim: aim.clone(),
                };
                advance(p, &step);
                if let (Some(a), Some(at)) = (&aim, target_at) {
                    world.worked(at, a, crafts.recipes[recipe].def.effect);
                }
            } else {
                p.mind.plan = None;
            }
            p.mind.doing = Doing::Idle;
            p.mind.timer = 0.0;
            self.done.push(Done {
                person: self.persons[i].id,
                at: pos,
                recipe,
                triggers: outcome.triggers,
            });
            return;
        }
        self.done.push(Done {
            person: self.persons[i].id,
            at: pos,
            recipe,
            triggers: outcome.triggers,
        });
        // The hammerstone back by the anvil for the next time.
        let p = &mut self.persons[i];
        lay_down_tools(p, world, pos);
        p.mind.doing = Doing::Idle;
        p.mind.timer = 0.0;
    }
}

/// Decides for a person, from the step's start: its fear, and — when its choice has run its time,
/// danger interrupts or it has nothing to do — what to do now.
#[allow(clippy::too_many_arguments)]
fn decide(
    p: &mut Person,
    sp: &Species,
    defs: &PsycheDefs,
    norms: &Norms,
    band: &Band,
    glimpses: &[Glimpse],
    senses: &dyn Senses,
    crafts: &Crafts,
    content: &Content,
    items: &Items,
    players: &[PlayerSeen],
    crowded: bool,
    hosts: &[u64],
    now: Now,
    dt: f32,
) {
    let sight = SEE_M * light(now.hour) as f64;
    let (threat, flight_m) = threat_of(p, band, senses, players, sight, sp.ways.is_some());
    // The bold let a threat come nearer before they run.
    let flight_m = flight_m * (1.3 - 0.6 * p.psyche.tendency(Tendency::RiskTolerance));
    // Fear rises at a threat; what the others nearby show is caught; feelings fade.
    let scare = threat.map_or(0.0, |t| {
        ((flight_m * 1.5 - t.dist) / (flight_m * 1.5)).clamp(0.0, 1.0)
    });
    p.psyche.feel(Feeling::Fear, scare);
    let pos = p.place.pos;
    // A hunter seen: the place believed dangerous a while, the fright kept.
    if let Some(t) = threat.filter(|t| t.hunter) {
        p.memory
            .remember(PlaceKind::Danger, t.at, now.day, scare.max(0.5));
        p.memory.happened(
            Happened::Threat,
            pos,
            now.day,
            p.psyche.feeling(Feeling::Fear),
        );
    }
    // The players it sees: known, and the first sight of one is a thing to remember.
    for q in players {
        let d = (q.pos - pos).length();
        if d < sight * q.plain.clamp(0.1, 1.0) as f64
            && p.memory.see(Who::Player(q.id), q.pos, now.day, dt)
        {
            p.memory
                .happened(Happened::Met { player: q.id }, pos, now.day, 0.6);
            p.record(now.day, Event::Met { player: q.id });
        }
    }
    p.memory.fade(now.day);
    for g in glimpses
        .iter()
        .filter(|g| g.band == p.social.band && g.alive && g.id != p.id)
    {
        let d = (g.pos - pos).length();
        if d < sight {
            p.memory.see(Who::Person(g.id), g.pos, now.day, dt);
        }
        if let Some((f, v)) = g.shows {
            let near = (1.0 - d as f32 / CATCH_M).clamp(0.0, 1.0);
            if near > 0.0 {
                p.psyche.catch(defs, f, v * near, dt);
            }
        }
        // An alarm heard frightens.
        if g.alarm && d < HEAR_M {
            p.psyche.feel(Feeling::Fear, 0.45);
        }
    }
    p.psyche.pass(defs, dt, now.day, p.mind.strain);
    // An infant feels and sees but does not choose: it is carried, by its mother or else the
    // nearest grown one of its band.
    if p.life_stage(sp, &now) == LifeStage::Infant {
        let carer = |id: Option<u64>| {
            glimpses
                .iter()
                .find(|g| Some(g.id) == id && g.alive && g.band == p.social.band)
        };
        let by = carer(p.life.mother).map(|g| g.id).or_else(|| {
            glimpses
                .iter()
                .filter(|g| g.band == p.social.band && g.alive && g.grown && g.id != p.id)
                .min_by(|a, b| {
                    let d = |g: &Glimpse| (g.pos - p.place.pos).length();
                    d(a).total_cmp(&d(b)).then(a.id.cmp(&b.id))
                })
                .map(|g| g.id)
        });
        p.mind.doing = by.map_or(Doing::Resting, |by| Doing::Carried { by });
        return;
    }
    let danger = threat.is_some_and(|t| t.dist < flight_m);
    let calm_while_fleeing = matches!(p.mind.doing, Doing::Fleeing { .. }) && !danger;
    p.mind.timer -= dt;
    let interrupt = danger
        && !matches!(
            p.mind.doing,
            Doing::Fleeing { .. } | Doing::Mobbing { .. } | Doing::Watching { .. }
        );
    let choosing =
        p.mind.timer <= 0.0 || interrupt || calm_while_fleeing || p.mind.doing == Doing::Idle;
    if !choosing {
        return;
    }
    // A goal: had already, or planned for — as deep as its species plans, one person in four
    // each step, so the planning is spread over the steps.
    if let Some(goal) = p.mind.goal.clone() {
        if crate::plan::has(items, content, p, &goal) {
            p.mind.goal = None;
            p.mind.plan = None;
            p.mind.failures = 0;
            p.psyche.feel(Feeling::Joy, 0.4);
            p.psyche.feel(Feeling::Pride, 0.5);
        } else if p.mind.plan.is_none() && (p.id + now.tick).is_multiple_of(4) {
            let ctx = crate::plan::Ctx {
                crafts,
                content,
                items,
                senses,
                depth: sp.cognition.planning_depth,
                look_m: LOOK_M * 2.0,
            };
            match crate::plan::plan(&ctx, p, &goal) {
                Some(plan) => p.mind.plan = Some(plan),
                None => {
                    p.mind.failures += 1;
                    if p.mind.failures >= GIVE_UP {
                        p.mind.goal = None;
                        p.mind.failures = 0;
                    }
                }
            }
        }
    }
    // What its band knows how to do (its people's, and where it is cold what the cold asks).
    let techniques: &[String] = if band.culture.techniques.is_empty() {
        &sp.techniques
    } else {
        &band.culture.techniques
    };
    let mut s = situation(
        p, sp, techniques, norms, glimpses, senses, crafts, content, items, now, sight, band,
        crowded, hosts,
    );
    s.camp = band.camp;
    s.threat = threat;
    s.flight_m = flight_m;
    let needs = Needs::of(
        &p.body,
        &p.body_config(sp, &now),
        p.psyche.feeling(Feeling::Fear),
    );
    let roll = p.rng.next_f32();
    p.mind.doing = choose(sp, &needs, &s, &p.psyche, roll);
    // Food kept from one hungry near, again and again: a breach of the sharing norm.
    match (&s.share_with, &p.mind.doing) {
        (Some((to, _, _)), d) if !matches!(d, Doing::Sharing { .. }) && needs.hunger < 0.3 => {
            p.mind.withheld = p.mind.withheld.saturating_add(1);
            if p.mind.withheld >= 3 {
                p.mind.withheld = 0;
                p.mind.stingy_to = Some(*to);
            }
        }
        _ => p.mind.withheld = 0,
    }
    // The patient keep at a thing longer.
    let patience = 0.8 + 0.4 * p.psyche.tendency(Tendency::Patience);
    p.mind.timer = hold_for(&p.mind.doing) * patience * (0.75 + 0.5 * p.rng.next_f32());
}

/// The nearest threat a person sees (a hunter of people, else the player it is most wary of),
/// and how near it may come before it runs: its flight distance, less for a player its band has
/// come to tolerate. People with ways with strangers (`humane`) do not run from a calm one: they
/// meet it (V2.1 §8.7).
fn threat_of(
    p: &Person,
    band: &Band,
    senses: &dyn Senses,
    players: &[PlayerSeen],
    sight: f64,
    humane: bool,
) -> (Option<Threat>, f32) {
    let pos = p.place.pos;
    let hunter: Option<Threat> = senses
        .hunters_near(pos, sight)
        .into_iter()
        .map(|h| Threat {
            at: h,
            dist: (h - pos).length() as f32,
            hunter: true,
        })
        .min_by(|x, y| x.dist.total_cmp(&y.dist));
    if hunter.is_some() {
        return (hunter, FLIGHT_M);
    }
    // Of the players it notices, the one nearest to its flight distance.
    let mut worst: Option<(f32, Threat, f32)> = None;
    for q in players {
        let d = (q.pos - pos).length() as f32;
        // Unnoticed: too far, or crouched and crawling in the grass.
        if d >= sight as f32 * q.plain.clamp(0.1, 1.0) {
            continue;
        }
        // A player coming on fast, or hunting near them, is a hunter for the while; a calm one
        // may come nearer the more the band has come to tolerate them, and beyond what holds its
        // eye is let be.
        let hunting = q.running || q.hunting;
        if humane && !hunting {
            continue;
        }
        let tolerated = FLIGHT_M * (1.0 - 0.85 * band.tolerance_of(q.id));
        if !hunting && d > tolerated * HEED {
            continue;
        }
        let flight = if hunting { FLIGHT_M } else { tolerated };
        let threat = Threat {
            at: q.pos,
            dist: d,
            hunter: hunting,
        };
        let margin = d / flight.max(1.0);
        if worst.as_ref().is_none_or(|w| margin < w.0) {
            worst = Some((margin, threat, flight));
        }
    }
    worst.map_or((None, FLIGHT_M), |(_, t, f)| (Some(t), f))
}

/// What a person knows of the moment about it (its threat filled in by the caller): the others
/// as the step found them, the places its band knows, what it senses.
#[allow(clippy::too_many_arguments)]
fn situation(
    p: &Person,
    sp: &Species,
    techniques: &[String],
    norms: &Norms,
    glimpses: &[Glimpse],
    senses: &dyn Senses,
    crafts: &Crafts,
    content: &Content,
    items: &Items,
    now: Now,
    sight: f64,
    band: &Band,
    crowded: bool,
    hosts: &[u64],
) -> Situation {
    let pos = p.place.pos;
    let grown = p.stage(sp, &now) == Stage::Adult;
    let stage = p.life_stage(sp, &now);
    let plays = matches!(stage, LifeStage::Child | LifeStage::Juvenile);
    let learns = plays || stage == LifeStage::Adolescent;
    // One hungry near whom it would feed with the food it carries: its household's first, then
    // those it is fondest of.
    let carries_food = carried_food(p, items, content).is_some();
    let mut share_with: Option<(u64, DVec3, f32)> = None;
    let guest = |id: PersonId| band.guests.iter().any(|g| g.who == id);
    let ways = band.culture.ways.as_ref().or(sp.ways.as_ref());
    let hospitality = ways.map_or(0.0, |w| w.hospitality);
    // How readily its people sanction, by how tight their norms are.
    let tight = band.culture.tightness();
    if carries_food {
        for g in glimpses.iter().filter(|g| {
            (g.band == p.social.band || guest(g.id)) && g.alive && g.hungry && g.id != p.id
        }) {
            if (g.pos - pos).length() > SHARE_M {
                continue;
            }
            // Nothing for one whose name brings food withheld, but its own household's young.
            let withheld = p
                .social
                .reputes
                .iter()
                .find(|r| r.about == Who::Person(g.id))
                .is_some_and(|r| norms.sanctions_in(r, Sanction::Withholding, tight));
            if withheld && g.household != p.social.household {
                continue;
            }
            let fond = p
                .social
                .ties
                .iter()
                .find(|t| t.who == g.id)
                .map_or(0.1, |t| t.affection);
            let home = p.social.household.is_some() && g.household == p.social.household;
            // A hungry guest is fed as their ways have it.
            let dear = if home {
                fond.max(0.8)
            } else if g.band != p.social.band {
                hospitality * fond.max(0.3)
            } else {
                fond
            };
            if share_with.is_none_or(|s| dear > s.2) {
                share_with = Some((g.id, g.pos, dear));
            }
        }
    }
    // The nearest stranger it sees — not of its band, nor its band's guest, nor its hosts, nor
    // one it trusts — and one warning it off.
    let mut stranger: Option<Stranger> = None;
    let mut warned: Option<DVec3> = None;
    if let Some(w) = ways {
        for g in glimpses
            .iter()
            .filter(|g| g.band != p.social.band && g.alive && g.id != p.id)
        {
            let d = (g.pos - pos).length();
            if g.warning == Some(p.id) && d < w.wary_m {
                warned = Some(g.pos);
            }
            if d > w.wary_m.min(sight) || guest(g.id) || hosts.contains(&g.band) {
                continue;
            }
            let tie = p.social.ties.iter().find(|t| t.who == g.id);
            if tie.is_some_and(|t| t.trust >= crate::strangers::STRANGER_TRUST)
                || stranger.is_some_and(|s| s.dist <= d)
            {
                continue;
            }
            let bad = p
                .social
                .reputes
                .iter()
                .find(|r| r.about == Who::Person(g.id))
                .is_some_and(|r| r.badness() <= -0.35);
            let grudge = tie.is_some_and(|t| t.rivalry >= crate::conflict::GRUDGE);
            let met = glimpses.iter().any(|o| {
                o.band == p.social.band
                    && o.id != p.id
                    && (o.greeting == Some(g.id) || o.warning == Some(g.id))
            });
            stranger = Some(Stranger {
                id: g.id,
                at: g.pos,
                dist: d,
                grown: g.grown,
                unwelcome: g.grown && (crowded || bad || grudge),
                greet_m: w.greet_m,
                met,
            });
        }
    }
    // One near it thinks ill of, and whether the name brings mockery or keeping away.
    let mut scorn: Option<(u64, DVec3, f32, bool, bool)> = None;
    for g in glimpses
        .iter()
        .filter(|g| g.band == p.social.band && g.alive && g.id != p.id)
    {
        if (g.pos - pos).length() > SCORN_M {
            continue;
        }
        let Some(r) = p
            .social
            .reputes
            .iter()
            .find(|r| r.about == Who::Person(g.id))
        else {
            continue;
        };
        let bad = r.badness();
        if bad < 0.0 && scorn.is_none_or(|s| bad < s.2) {
            let ridicule = norms.sanctions_in(r, Sanction::Ridicule, tight);
            let avoid = norms.sanctions_in(r, Sanction::Avoidance, tight);
            scorn = Some((g.id, g.pos, bad, ridicule, avoid));
        }
    }
    // One of its own hurt near, whom it is fond of.
    let mut hurt_near: Option<(u64, DVec3, f32)> = None;
    for g in glimpses
        .iter()
        .filter(|g| g.band == p.social.band && g.alive && g.hurt && g.id != p.id)
    {
        if (g.pos - pos).length() > TEND_M {
            continue;
        }
        let fond = p
            .social
            .ties
            .iter()
            .find(|t| t.who == g.id)
            .map_or(0.0, |t| t.affection);
        if fond >= 0.4 && hurt_near.is_none_or(|h| fond > h.2) {
            hurt_near = Some((g.id, g.pos, fond));
        }
    }
    // The young of the band to play with, and a grown one at its work to watch.
    let mut playmate: Option<(u64, DVec3, f64)> = None;
    let mut work_near: Option<(u64, DVec3, usize, f64)> = None;
    for g in glimpses
        .iter()
        .filter(|g| g.band == p.social.band && g.alive && g.id != p.id)
    {
        let d = (g.pos - pos).length();
        if plays && g.young && d < PLAY_M && playmate.is_none_or(|m| d < m.2) {
            playmate = Some((g.id, g.pos, d));
        }
        if learns
            && g.grown
            && d < WATCH_M
            && let Some(r) = g.working
            && work_near.is_none_or(|w| d < w.3)
        {
            work_near = Some((g.id, g.pos, r, d));
        }
    }
    // The band: its middle, its grown ones near, an alarm raised, the mother.
    let (mut cx, mut cz, mut cn) = (0.0, 0.0, 0.0);
    let mut grown_near = 0u16;
    let mut alarm_raised = false;
    let mut mother: Option<DVec3> = None;
    for b in glimpses
        .iter()
        .filter(|b| b.band == p.social.band && b.alive && b.id != p.id)
    {
        let d = (b.pos - pos).length();
        if b.alarm && d < HEAR_M {
            alarm_raised = true;
        }
        // The others it sees; of those it does not, where it last saw them.
        let at = if d < sight {
            b.pos
        } else {
            match p.memory.last_seen(Who::Person(b.id)) {
                Some((at, _)) => at,
                None => continue,
            }
        };
        let b = Glimpse { pos: at, ..*b };
        cx += b.pos.x;
        cz += b.pos.z;
        cn += 1.0;
        if b.grown && (b.pos - pos).length() < 20.0 {
            grown_near += 1;
        }
        if !grown && p.life.mother == Some(b.id) {
            mother = Some(b.pos);
        }
    }
    let group_at = if cn > 0.0 {
        mother.or(Some(DVec3::new(cx / cn, pos.y, cz / cn)))
    } else {
        None
    };
    let from_group_m = group_at.map_or(0.0, |g| {
        let d = (g - pos).length() as f32;
        // The young keep to their mothers.
        if grown { d } else { d * 3.0 }
    });
    // Trees, water and food it remembers, or sees — not where it believes danger waits.
    let safe = |at: &DVec3| p.memory.danger_at(*at) < 0.5;
    let tree = p
        .memory
        .nearest(PlaceKind::Sleep, pos)
        .filter(|t| (*t - pos).length() < LOOK_M * 2.0)
        .or_else(|| trunk_near(senses.ground(), pos, 12.0).map(|(t, _)| t));
    let water = p.memory.nearest(PlaceKind::Water, pos).filter(safe);
    let water_here = water.is_some_and(|w| (w - pos).length() < 2.0);
    // What its culture forbids it does not eat, unless starving.
    let starving = Needs::of(&p.body, &p.body_config(sp, &now), 0.0).hunger >= 1.0;
    let food_rate = senses
        .food_at(pos)
        .filter(|f| starving || !band.culture.forbids(&f.material))
        .map_or(0.0, |f| f.kg_min);
    let food_here = food_rate > 0.0;
    // Food elsewhere, when what is here is thin (or nothing): in sight, else remembered.
    let food = if food_rate >= RICH_KG_MIN {
        None
    } else {
        senses
            .food_near(pos, LOOK_M)
            .filter(|f| (*f - pos).length() > 5.0)
            .filter(safe)
            .or_else(|| {
                p.memory
                    .nearest(PlaceKind::Food, pos)
                    .filter(|f| (*f - pos).length() > 5.0)
                    .filter(safe)
            })
    };
    // What it could do with what lies about: crack nuts at an anvil, strike a flake.
    let offers = if grown {
        offers_for(
            p,
            sp,
            techniques,
            &band.culture,
            crafts,
            content,
            items,
            senses,
            &now,
        )
    } else {
        Vec::new()
    };
    Situation {
        pos,
        hour: now.hour,
        threat: None,
        flight_m: FLIGHT_M,
        in_tree: p.place.medium == Medium::Tree,
        in_nest: p.place.medium == Medium::Tree && p.mind.doing == Doing::Sleeping,
        tree,
        water,
        water_here,
        food,
        food_here,
        food_rate,
        offers,
        alarm_raised,
        grown_near,
        group_at,
        from_group_m,
        grown,
        plays,
        learns,
        playmate: playmate.map(|(id, at, _)| (id, at)),
        work_near: work_near.map(|(id, at, r, _)| (id, at, r)),
        share_with,
        hurt_near,
        scorn,
        camp: None,
        stranger,
        warned,
        project: p
            .mind
            .plan
            .as_ref()
            .and_then(|pl| pl.step())
            .map(step_doing),
    }
}

/// What a plan's step has a person do.
fn step_doing(s: &Step) -> Doing {
    match s {
        Step::Go(at) => Doing::Going {
            to: *at,
            then: Intent::Step,
        },
        Step::Take(id) => Doing::Taking { id: *id },
        Step::Do { recipe, .. } => Doing::Working { recipe: *recipe },
    }
}

/// A plan's step done: on to the next; a plan gone through is let go (the goal is looked at
/// again: had, or planned anew).
fn advance(p: &mut Person, done: &Step) {
    if let Some(plan) = &mut p.mind.plan
        && plan.step() == Some(done)
    {
        plan.next += 1;
        if plan.done() {
            p.mind.plan = None;
        }
    }
}

/// A step toward a place on the ground; true when there.
pub(crate) fn move_toward(
    p: &mut Person,
    senses: &dyn Senses,
    to: DVec3,
    speed: f32,
    dt: f32,
) -> bool {
    let d = DVec2::new(to.x - p.place.pos.x, to.z - p.place.pos.z);
    if d.length() < 1.0 {
        p.place.speed = 0.0;
        return true;
    }
    p.place.yaw = (d.x as f32).atan2(d.y as f32);
    p.place.speed = speed;
    let step = d.normalize() * (speed * dt) as f64;
    let (nx, nz) = (p.place.pos.x + step.x, p.place.pos.z + step.y);
    match senses.ground().footing(nx, nz, p.place.pos.y) {
        Some(f) if (f.y - p.place.pos.y).abs() < 1.2 && !(f.water && f.depth > 0.8) => {
            p.place.pos = DVec3::new(nx, f.y, nz);
            p.place.medium = Medium::Ground;
            false
        }
        _ => {
            // Blocked: around it, a little to the side.
            let side = DVec2::new(-step.y, step.x);
            let (sx, sz) = (p.place.pos.x + side.x, p.place.pos.z + side.y);
            if let Some(f) = senses.ground().footing(sx, sz, p.place.pos.y)
                && (f.y - p.place.pos.y).abs() < 1.2
                && !f.water
            {
                p.place.pos = DVec3::new(sx, f.y, sz);
            }
            false
        }
    }
}

/// A step up the tree it is by toward its place in the crown (found as it starts up: each to its
/// own place on the limbs where the crown begins — the crown is shared); whether there is a tree
/// to climb.
fn climb_up(p: &mut Person, senses: &dyn Senses, dt: f32) -> bool {
    if p.place.medium != Medium::Tree || p.place.perch.is_none() {
        let Some((foot, height)) = trunk_near(senses.ground(), p.place.pos, 2.5) else {
            return false;
        };
        let perch = crown_perch(senses.ground(), foot, height, p.id as f64);
        p.place.pos.x = perch.x;
        p.place.pos.z = perch.z;
        p.place.pos.y = p.place.pos.y.max(foot.y);
        p.place.perch = Some(perch);
        p.place.medium = Medium::Tree;
    }
    let top = p.place.perch.map_or(p.place.pos.y, |q| q.y);
    if p.place.pos.y < top {
        p.place.speed = 0.4;
        p.place.pos.y = (p.place.pos.y + 0.6 * dt as f64).min(top);
    } else {
        p.place.speed = 0.0;
    }
    true
}

/// Down the tree to its foot.
fn climb_down(p: &mut Person, senses: &dyn Senses, dt: f32) {
    let q = p.place.pos;
    let ground = senses
        .ground()
        .footing(q.x, q.z, q.y - 1.0)
        .or_else(|| senses.ground().top(q.x, q.z));
    let floor = ground.map_or(q.y - 1.0, |f| f.y);
    p.place.speed = 0.4;
    p.place.pos.y -= 0.8 * dt as f64;
    if p.place.pos.y <= floor {
        p.place.pos.y = floor;
        p.place.medium = Medium::Ground;
        p.place.perch = None;
        p.place.speed = 0.0;
    }
}

/// Where one climbs to in a tree: onto one of the limbs where its crown begins (each its own, by
/// `k`), or, a tree with none in reach, beside the trunk up near its top.
fn crown_perch(ground: &dyn Ground, foot: DVec3, height: f64, k: f64) -> DVec3 {
    let top = trunk_top(ground, foot, height);
    let (cx, cz) = (foot.x.floor() as i32, foot.z.floor() as i32);
    let mut limbs: Vec<DVec3> = Vec::new();
    for y in foot.y.floor() as i32 + 2..=top.ceil() as i32 + 3 {
        for dz in -3..=3 {
            for dx in -3..=3 {
                let (x, z) = (cx + dx, cz + dz);
                let room = matches!(
                    ground.cell(x, y + 1, z),
                    Some(Cell::Open | Cell::Leaves | Cell::Plant)
                );
                if ground.cell(x, y, z) == Some(Cell::Limb) && room {
                    limbs.push(DVec3::new(x as f64 + 0.5, y as f64 + 0.6, z as f64 + 0.5));
                }
            }
        }
        // The crown's lowest limbs.
        if limbs.len() >= 6 {
            break;
        }
    }
    if !limbs.is_empty() {
        let pick = ((k * 0.618).fract() * limbs.len() as f64) as usize;
        return limbs[pick.min(limbs.len() - 1)];
    }
    let turn = k * 2.399_963;
    let out = 0.8 + 0.9 * (k * 0.371).fract();
    let high = (top - foot.y - 1.0).max(2.0);
    DVec3::new(
        foot.x + turn.cos() * out,
        foot.y + high - (k * 0.618).fract() * (high * 0.4).min(3.0),
        foot.z + turn.sin() * out,
    )
}

/// How high (y) a tree's trunk runs where it is climbed: the tallest of its columns within two
/// blocks of the foot found (a broad trunk's middle runs up into the crown, its flared foot and
/// buttresses not).
fn trunk_top(ground: &dyn Ground, foot: DVec3, height: f64) -> f64 {
    let (cx, cz, y0) = (
        foot.x.floor() as i32,
        foot.z.floor() as i32,
        foot.y.floor() as i32,
    );
    let mut best = foot.y + height;
    for dz in -2..=2 {
        for dx in -2..=2 {
            let (x, z) = (cx + dx, cz + dz);
            let Some(base) = (y0 - 1..=y0 + 2).find(|y| ground.cell(x, *y, z) == Some(Cell::Trunk))
            else {
                continue;
            };
            let mut top = base;
            while top < base + 40 && ground.cell(x, top + 1, z) == Some(Cell::Trunk) {
                top += 1;
            }
            best = best.max((top + 1) as f64);
        }
    }
    best
}

/// Learns the places about a band as it is drawn out: the water, the trees to sleep in, the
/// anvils lying about.
pub(crate) fn know_about(band: &mut Band, world: &mut dyn World, items: &Items, here: DVec3) {
    if let Some((bank, _)) = water_near(world.ground(), here, 80.0) {
        Places::remember(&mut band.places.water, bank, 10.0, 6);
    }
    let anvils: Vec<DVec3> = world
        .things_near(here, ANVILS_M)
        .into_iter()
        .filter(|(_, s)| {
            items
                .get(&s.id)
                .and_then(|k| k.property("anvil"))
                .is_some_and(|v| v > 0.0)
        })
        .filter_map(|(id, _)| world.place_of(id))
        .collect();
    for at in anvils {
        Places::remember(&mut band.places.anvils, at, 4.0, 8);
    }
    for (dx, dz) in [
        (0.0, 0.0),
        (15.0, 0.0),
        (-15.0, 0.0),
        (0.0, 15.0),
        (0.0, -15.0),
    ] {
        let at = here + DVec3::new(dx, 0.0, dz);
        if let Some((t, _)) = trunk_near(world.ground(), at, 10.0) {
            Places::remember(&mut band.places.sleep, t, 3.0, 12);
        }
    }
}

/// How long it keeps at a choice before looking about again (seconds of play).
fn hold_for(d: &Doing) -> f32 {
    match d {
        Doing::Idle => 1.0,
        Doing::Going { .. } => 60.0,
        Doing::Feeding => 40.0,
        Doing::Drinking => 15.0,
        Doing::Working { .. } => 120.0,
        Doing::Taking { .. } => 2.0,
        Doing::Resting => 20.0,
        Doing::Grooming { .. } => 15.0,
        Doing::Nesting => 20.0,
        Doing::Sleeping => 60.0,
        Doing::Watching { .. } => 6.0,
        Doing::Alarm { .. } => 4.0,
        Doing::Mobbing { .. } => 12.0,
        Doing::Fleeing { .. } => 15.0,
        Doing::Carried { .. } => 5.0,
        Doing::Playing { .. } => 8.0,
        Doing::Imitating { .. } => 20.0,
        Doing::Sharing { .. } => 20.0,
        Doing::Tending { .. } => 30.0,
        Doing::Mocking { .. } => 4.0,
        Doing::Quarrelling { .. } | Doing::Mediating { .. } => 2.0,
        Doing::Greeting { .. } => 20.0,
        Doing::WarningOff { .. } => 15.0,
    }
}

/// The body's activity for what it is doing (the content's table of METs).
fn activity_of(d: &Doing, speed: f32) -> &'static str {
    match d {
        Doing::Sleeping => "sleeping",
        Doing::Resting
        | Doing::Grooming { .. }
        | Doing::Feeding
        | Doing::Drinking
        | Doing::Carried { .. } => "resting",
        Doing::Watching { .. } | Doing::Alarm { .. } | Doing::Idle | Doing::Taking { .. } => {
            "standing"
        }
        Doing::Working { .. } | Doing::Nesting => "carrying_heavy",
        Doing::Imitating { .. } | Doing::Tending { .. } | Doing::Mocking { .. } => "resting",
        Doing::Going { .. }
        | Doing::Mobbing { .. }
        | Doing::Fleeing { .. }
        | Doing::Playing { .. }
        | Doing::Sharing { .. }
        | Doing::Quarrelling { .. }
        | Doing::Mediating { .. }
        | Doing::Greeting { .. }
        | Doing::WarningOff { .. } => {
            if speed > 2.0 {
                "jogging"
            } else if speed > 0.1 {
                "walking"
            } else {
                "standing"
            }
        }
    }
}

/// The yaw facing from one place to another.
pub(crate) fn yaw_toward(from: DVec3, to: DVec3) -> f32 {
    ((to.x - from.x) as f32).atan2((to.z - from.z) as f32)
}

/// Eats `kg` of a material.
fn eat(p: &mut Person, cfg: &hearth_body::BodyConfig, content: &Content, material: &str, kg: f32) {
    let Some(m) = content
        .materials
        .get(material)
        .or_else(|| content.materials.get(&format!("hearth:{material}")))
    else {
        return;
    };
    let Some(b) = hearth_craft::food::bite(m, kg, 0.0) else {
        return;
    };
    let f = Food {
        kcal: b.kcal as f64,
        protein_g: b.protein_g as f64,
        fat_g: b.fat_g as f64,
        carb_g: b.carb_g as f64,
        water_l: b.water_l as f64,
        volume_l: b.volume_l as f64,
        fresh_days: b.fresh_days as f64,
    };
    let _ = p.body.eat(cfg, &f);
}

/// The item of a material's bulk form.
fn items_of(items: &Items, material: &str) -> Option<String> {
    let key = material.rsplit(':').next().unwrap_or(material);
    items
        .iter()
        .find(|k| k.material.as_deref().is_some_and(|m| m.ends_with(key)) && k.has_tag("bulk"))
        .map(|k| k.id.clone())
}

/// Takes up in its hands, from what lies within reach, the tools a process needs that it does
/// not hold (the hammerstone by the anvil): the ids of what it took.
fn take_up_tools(
    p: &mut Person,
    sp: &Species,
    crafts: &Crafts,
    recipe: usize,
    lying: &[(u64, Stack)],
    items: &Items,
    now: &Now,
) -> Vec<u64> {
    let def = &crafts.recipes[recipe].def;
    // As strong as the engine reckons it (its quality and wear told).
    let strong =
        |s: &Stack, prop: &str, min: f32| s.property(items, prop).is_some_and(|v| v >= min);
    let mut took = Vec::new();
    let mass = p.mass_kg(sp, now);
    for t in &def.tools {
        let carry = &p.possessions.carry;
        let held = [carry.right.as_ref(), carry.left.as_ref()]
            .into_iter()
            .flatten()
            .any(|s| strong(s, &t.property, t.min));
        if held {
            continue;
        }
        let found = lying
            .iter()
            .find(|(id, s)| !took.contains(id) && strong(s, &t.property, t.min));
        if let Some((id, s)) = found {
            let hand = if p.possessions.carry.right.is_none() {
                hearth_items::Hand::Right
            } else {
                hearth_items::Hand::Left
            };
            if p.possessions
                .carry
                .hold(items, s.clone(), hand, mass)
                .is_ok()
            {
                took.push(*id);
            }
            continue;
        }
        // In its basket: taken into a hand, what fills the left hand put in the basket first.
        let in_basket =
            |c: &hearth_items::Carry| in_containers(c, &|s: &Stack| strong(s, &t.property, t.min));
        if in_basket(&p.possessions.carry).is_none() {
            continue;
        }
        let carry = &mut p.possessions.carry;
        let hand = if carry.right.is_none() {
            Some(hearth_items::Hand::Right)
        } else if carry.left.is_none() {
            Some(hearth_items::Hand::Left)
        } else {
            carry
                .release(hearth_items::Hand::Left)
                .and_then(|s| match carry.stow(items, s, mass) {
                    Ok(()) => Some(hearth_items::Hand::Left),
                    Err(s) => {
                        let _ = carry.hold(items, s, hearth_items::Hand::Left, mass);
                        None
                    }
                })
        };
        if let (Some(hand), Some(path)) = (hand, in_basket(carry))
            && let Some(tool) = carry.take(items, &path, Some(1))
            && let Err((tool, _)) = carry.hold(items, tool, hand, mass)
        {
            let _ = carry.stow(items, tool, mass);
        }
    }
    took
}

/// Where in the containers carried (a basket on the back, a pouch hung from a belt) the first
/// thing lies that `pick` takes.
fn in_containers(
    c: &hearth_items::Carry,
    pick: &dyn Fn(&Stack) -> bool,
) -> Option<hearth_items::Path> {
    fn walk(
        path: &hearth_items::Path,
        s: &Stack,
        pick: &dyn Fn(&Stack) -> bool,
    ) -> Option<hearth_items::Path> {
        let inside = s.contents()?;
        for (i, it) in inside.items.iter().enumerate() {
            let p = path.inner(i);
            if pick(&it.stack) {
                return Some(p);
            }
            if let Some(found) = walk(&p, &it.stack, pick) {
                return Some(found);
            }
        }
        None
    }
    use hearth_items::{Path, Root};
    let mut roots: Vec<(Root, &Stack)> = Vec::new();
    if let Some(b) = &c.back {
        roots.push((Root::Back, b));
    }
    for (i, w) in c.worn.iter().enumerate() {
        for (k, h) in w.hung.iter().enumerate() {
            if let Some(s) = h {
                roots.push((Root::Hung(i, k), s));
            }
        }
    }
    roots
        .into_iter()
        .find_map(|(r, s)| walk(&Path::at(r), s, pick))
}

/// Lays down beside it what it holds in its hands (its tools, after the work).
fn lay_down_tools(p: &mut Person, world: &mut dyn World, at: DVec3) {
    for hand in [hearth_items::Hand::Right, hearth_items::Hand::Left] {
        if let Some(s) = p.possessions.carry.release(hand) {
            world.things().lay(at + DVec3::new(0.3, 0.0, 0.2), s);
        }
    }
}

/// The thing a process is done to, among what lies at hand (an anvil stone for cracking), if it
/// needs one.
fn aim_for(
    crafts: &Crafts,
    content: &Content,
    recipe: usize,
    lying: &[(u64, Stack)],
    items: &Items,
) -> Option<Aimed> {
    use hearth_content::schema::process::Target;
    match &crafts.recipes[recipe].def.target {
        Some(Target::Thing(m)) => lying
            .iter()
            .find(|(_, s)| {
                items
                    .get(&s.id)
                    .is_some_and(|k| hearth_craft::engine::matches(m, k, content))
            })
            .map(|(id, _)| Aimed::Thing(*id)),
        _ => None,
    }
}

/// The processes a grown person could do here with what it carries and what lies within reach:
/// its band's techniques that the engine would plan now.
#[allow(clippy::too_many_arguments)]
fn offers_for(
    p: &Person,
    sp: &Species,
    techniques: &[String],
    culture: &Culture,
    crafts: &Crafts,
    content: &Content,
    items: &Items,
    senses: &dyn Senses,
    now: &Now,
) -> Vec<Offer> {
    let mut out = Vec::new();
    // Here, and at the band's anvils it carries nuts to.
    let mut spots = vec![p.place.pos];
    if let Some(anvil) = p.memory.nearest(PlaceKind::Anvil, p.place.pos) {
        spots.push(anvil);
    }
    for spot in spots {
        let lying = senses.things_near(spot, REACH_M);
        let around = senses.surroundings(spot);
        for t in techniques {
            let Some(r) = crafts.index_of(t).or_else(|| {
                crafts.index_of(&format!("hearth:{}", t.rsplit(':').next().unwrap_or(t)))
            }) else {
                continue;
            };
            // They strike new flakes rather than mend the old (retouch is rare before the
            // later Oldowan).
            if crafts.recipes[r].def.effect == hearth_content::schema::process::Effect::Mend {
                continue;
            }
            if out.iter().any(|o: &Offer| o.recipe == r) {
                continue;
            }
            // At the anvil it stands by the things there, the tools lying by it in its hands.
            let mut at = p.clone();
            at.place.pos = spot;
            let took = take_up_tools(&mut at, sp, crafts, r, &lying, items, now);
            let rest: Vec<(u64, Stack)> = lying
                .iter()
                .filter(|(id, _)| !took.contains(id))
                .cloned()
                .collect();
            let aimed = aim_for(crafts, content, r, &rest, items);
            if plan_work(crafts, content, items, &at, r, &rest, aimed, around.clone()).is_ok() {
                let feeds = crafts.recipes[r]
                    .def
                    .outputs
                    .iter()
                    .any(|o| output_is_food(content, &o.item));
                // How much the work is its own by its culture's division of labour.
                let labour = match crafts.recipes[r].def.skill.as_deref() {
                    Some(skill) if culture.drawn() => {
                        let women = culture.women_share(skill);
                        0.3 + 1.4 * if p.life.female { women } else { 1.0 - women }
                    }
                    _ => 1.0,
                };
                out.push(Offer {
                    recipe: r,
                    at: spot,
                    feeds,
                    labour,
                });
            }
        }
    }
    out
}

/// Whether what a process makes is food.
fn output_is_food(content: &Content, m: &hearth_content::schema::process::Match) -> bool {
    use hearth_content::schema::process::Match;
    match m {
        Match::Material(id) => content
            .materials
            .get(id.as_str())
            .is_some_and(|mat| mat.tags.iter().any(|t| t == "food")),
        _ => false,
    }
}

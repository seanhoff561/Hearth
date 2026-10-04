//! A small savanna for the people's headless tests: a stand of trees, a marula among them
//! dropping its fruit and stones, a pool, an anvil stone with a cobble by it; and a band of
//! *Australopithecus* in it, lived on a step at a time.

#![allow(dead_code)]

use std::sync::OnceLock;

use glam::DVec3;
use hearth_body::{BodyConfig, Exposure, Rates};
use hearth_content::{Content, TimeScales};
use hearth_craft::engine::Surroundings;
use hearth_craft::{Crafts, Graph};
use hearth_fauna::live::{Cell, Footing, Ground};
use hearth_items::{Items, Stack};
use hearth_people::{FoodHere, Now, People, Pile, PlayerSeen, Senses, SpeciesSet, Things, World};

pub struct Base {
    pub content: Content,
    pub items: Items,
    pub crafts: Crafts,
    pub graph: Graph,
    pub species: SpeciesSet,
}

pub fn base() -> &'static Base {
    static B: OnceLock<Base> = OnceLock::new();
    B.get_or_init(|| {
        let content = Content::load_base();
        let items = Items::from_content(&content);
        let crafts = Crafts::from_content(&content, &items);
        let graph = Graph::from_content(&content);
        let human = BodyConfig::with_rates(
            &content,
            Rates::authentic(),
            TimeScales::defaults(&content.time),
        );
        let species = SpeciesSet::from_content(&content, &graph, &human);
        Base {
            content,
            items,
            crafts,
            graph,
            species,
        }
    })
}

pub fn item(suffix: &str) -> String {
    base()
        .items
        .iter()
        .find(|k| k.id.ends_with(suffix))
        .map(|k| k.id.clone())
        .unwrap_or_else(|| panic!("no item {suffix}"))
}

/// The ground of the savanna: flat at 10 m, a pool of water a metre deep from x 40 to 50 and z
/// 0 to 10, trees standing seven metres at their trunks' columns.
#[derive(Debug, Clone)]
pub struct Land {
    pub trees: Vec<(i32, i32)>,
}

pub const GROUND: f64 = 10.0;

impl Land {
    fn pool(x: f64, z: f64) -> bool {
        (40.0..50.0).contains(&x) && (0.0..10.0).contains(&z)
    }
}

impl Ground for Land {
    fn footing(&self, x: f64, z: f64, _y: f64) -> Option<Footing> {
        Some(if Land::pool(x, z) {
            Footing {
                y: GROUND - 1.0,
                water: true,
                depth: 1.0,
            }
        } else {
            Footing::dry(GROUND)
        })
    }

    fn top(&self, x: f64, z: f64) -> Option<Footing> {
        self.footing(x, z, GROUND)
    }

    fn cell(&self, x: i32, y: i32, z: i32) -> Option<Cell> {
        let yf = y as f64;
        if self.trees.contains(&(x, z)) && (GROUND as i32..GROUND as i32 + 7).contains(&y) {
            return Some(Cell::Trunk);
        }
        let f = self.footing(x as f64 + 0.5, z as f64 + 0.5, yf)?;
        Some(if yf < f.y {
            Cell::Solid
        } else if yf < f.level() {
            Cell::Water
        } else {
            Cell::Open
        })
    }
}

/// The savanna as the people have it.
#[derive(Debug, Clone)]
pub struct Savanna {
    pub land: Land,
    pub pile: Pile,
    pub marula: DVec3,
    pub leopard: Option<DVec3>,
    /// The local hour, the day of the calendar and the tick.
    pub hour: f32,
    pub day: f64,
    pub tick: u64,
    pub nests: usize,
    pub alarms: usize,
    /// What processes can be done to: where, and how it is aimed at.
    pub targets: Vec<(DVec3, hearth_craft::engine::Aimed)>,
}

impl Savanna {
    pub fn new() -> Self {
        let trees = vec![
            (0, 0),
            (6, 3),
            (-5, 7),
            (3, -8),
            (12, 10),
            (-10, -4),
            (20, 0),
        ];
        let marula = DVec3::new(20.5, GROUND, 0.5);
        let mut pile = Pile::default();
        // The anvil under the marula, with a cobble by it.
        pile.add(
            DVec3::new(18.0, GROUND, 2.0),
            Stack::of(&item("anvil_stone/basalt"), 1),
        );
        pile.add(
            DVec3::new(18.4, GROUND, 2.2),
            Stack::of(&item("cobble/basalt"), 1),
        );
        Self {
            land: Land { trees },
            pile,
            marula,
            leopard: None,
            hour: 8.0,
            day: 0.0,
            tick: 0,
            nests: 0,
            alarms: 0,
            targets: Vec::new(),
        }
    }

    /// The moment now.
    pub fn now(&self) -> Now {
        Now {
            tick: self.tick,
            hour: self.hour,
            day: self.day,
            year_days: YEAR_DAYS,
        }
    }

    /// On by a step of `DT`.
    pub fn advance(&mut self) {
        self.tick += 1;
        self.hour = (self.hour + DT * 24.0 / DAY_S) % 24.0;
        self.day += (DT / DAY_S) as f64;
    }
}

impl Default for Savanna {
    fn default() -> Self {
        Self::new()
    }
}

impl Senses for Savanna {
    fn ground(&self) -> &(dyn Ground + Sync) {
        &self.land
    }

    fn things_near(&self, at: DVec3, reach: f64) -> Vec<(u64, Stack)> {
        self.pile.near(at, reach)
    }

    fn place_of(&self, id: u64) -> Option<DVec3> {
        self.pile.place(id)
    }

    fn food_at(&self, at: DVec3) -> Option<FoodHere> {
        // Under the marula, its plums and stones; elsewhere seeds and grubs, thinly.
        if (at - self.marula).length() < 5.0 {
            return Some(FoodHere {
                material: "marula_fruit".into(),
                kg_min: 0.3,
                nuts: Some("marula_stone".into()),
            });
        }
        ((at - self.marula).length() < 60.0).then(|| FoodHere {
            material: "insect_larvae".into(),
            kg_min: 0.01,
            nuts: None,
        })
    }

    fn food_near(&self, _at: DVec3, _within: f64) -> Option<DVec3> {
        Some(self.marula)
    }

    fn exposure(&self, _at: DVec3, _in_tree: bool) -> Exposure {
        let mut e = Exposure::mild();
        e.air_c = if (6.0..18.0).contains(&self.hour) {
            27.0
        } else {
            18.0
        };
        e.local_hour = self.hour;
        e
    }

    fn surroundings(&self, _at: DVec3) -> Surroundings {
        Surroundings {
            daylight: (6.0..18.5).contains(&self.hour),
            air_c: 25.0,
            humidity: 0.5,
            ..Surroundings::default()
        }
    }

    fn hunters_near(&self, at: DVec3, within: f64) -> Vec<DVec3> {
        self.leopard
            .filter(|l| (*l - at).length() < within)
            .into_iter()
            .collect()
    }

    fn targets_near(&self, at: DVec3, within: f64) -> Vec<(DVec3, hearth_craft::engine::Aimed)> {
        self.targets
            .iter()
            .filter(|(p, _)| (*p - at).length() < within)
            .cloned()
            .collect()
    }
}

impl World for Savanna {
    fn things(&mut self) -> &mut dyn Things {
        &mut self.pile
    }

    fn call(&mut self, _at: DVec3, alarm: bool) {
        if alarm {
            self.alarms += 1;
        }
    }

    fn nest(&mut self, _at: DVec3) -> Option<DVec3> {
        self.nests += 1;
        None
    }

    fn worked(
        &mut self,
        at: DVec3,
        _aimed: &hearth_craft::engine::Aimed,
        effect: hearth_content::schema::process::Effect,
    ) {
        use hearth_content::schema::process::Effect;
        if matches!(effect, Effect::Remove | Effect::Deplete) {
            self.targets.retain(|(p, _)| (*p - at).length() > 0.5);
        }
    }
}

/// A play day (s), a calendar year (days) and the steps of play.
pub const DAY_S: f32 = 2880.0;
pub const YEAR_DAYS: f64 = 32.0;
pub const DT: f32 = 0.5;

/// A band of four grown females, three males, two half-grown and a baby, by the trees.
pub fn band(world: &mut Savanna) -> People {
    let b = base();
    let mut p = People::new(7);
    let k = b.species.index_of("australopithecus").expect("the hominin");
    let now = world.now();
    p.spawn_band(
        &b.species,
        &b.graph,
        &b.items,
        world,
        k,
        [4, 3, 2, 1],
        DVec3::new(2.0, GROUND, 2.0),
        now,
    );
    p
}

/// One step of play with the players about.
pub fn step(p: &mut People, world: &mut Savanna, players: &[PlayerSeen]) {
    let b = base();
    world.advance();
    let now = world.now();
    p.step(
        &b.species, &b.crafts, &b.content, &b.items, world, players, now, DT,
    );
}

/// Lives `seconds` of play with the players about.
pub fn live(p: &mut People, world: &mut Savanna, seconds: f32, players: &[PlayerSeen]) {
    for _ in 0..(seconds / DT) as u64 {
        step(p, world, players);
    }
}

/// The players of the tests.
pub const ONE: u64 = 1;
pub const TWO: u64 = 2;

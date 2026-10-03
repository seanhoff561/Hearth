//! V2-11 (c): a group of *Australopithecus* lives its days in a small savanna — a stand of
//! trees, a marula among them dropping its fruit and stones, a pool, an anvil stone with a cobble
//! by it — feeding, drinking, gathering marula stones and cracking them at the anvil, up into the
//! trees to nest at dusk and down at dawn; a leopard sends them up the trees with the alarm, or,
//! enough of them together, they face it; a person running at them is fled, a calm one watched.
//! V2-11 (d): a calm person comes in time to be let be, and one they let be learns from watching
//! their work.

use std::sync::OnceLock;

use glam::DVec3;
use hearth_agent::live::{AgentWorld, FoodHere, Hominins, Now, Person};
use hearth_agent::{Doing, Kinds, Pile, Things};
use hearth_body::{BodyConfig, Exposure, Rates};
use hearth_content::{Content, TimeScales};
use hearth_craft::engine::Surroundings;
use hearth_craft::{Crafts, Graph};
use hearth_fauna::live::{Cell, Footing, Ground, Medium};
use hearth_items::{Items, Stack};

struct Base {
    content: Content,
    items: Items,
    crafts: Crafts,
    graph: Graph,
    kinds: Kinds,
}

fn base() -> &'static Base {
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
        let kinds = Kinds::from_content(&content, &graph, &human);
        Base {
            content,
            items,
            crafts,
            graph,
            kinds,
        }
    })
}

fn item(suffix: &str) -> String {
    base()
        .items
        .iter()
        .find(|k| k.id.ends_with(suffix))
        .map(|k| k.id.clone())
        .unwrap_or_else(|| panic!("no item {suffix}"))
}

/// The ground of the savanna: flat at 10 m, a pool of water a metre deep from x 40 to 50 and z
/// 0 to 10, trees standing seven metres at their trunks' columns.
struct Land {
    trees: Vec<(i32, i32)>,
}

const GROUND: f64 = 10.0;

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

/// The savanna as the agents have it.
struct Savanna {
    land: Land,
    pile: Pile,
    marula: DVec3,
    leopard: Option<DVec3>,
    hour: f32,
    nests: usize,
    alarms: usize,
}

impl Savanna {
    fn new() -> Self {
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
            nests: 0,
            alarms: 0,
        }
    }
}

impl AgentWorld for Savanna {
    fn ground(&self) -> &dyn Ground {
        &self.land
    }

    fn things(&mut self) -> &mut dyn Things {
        &mut self.pile
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

    fn call(&mut self, _at: DVec3, alarm: bool) {
        if alarm {
            self.alarms += 1;
        }
    }

    fn nest(&mut self, _at: DVec3) -> Option<DVec3> {
        self.nests += 1;
        None
    }
}

/// A play day (s) and the steps of play.
const DAY_S: f32 = 2880.0;
const DT: f32 = 0.5;

fn group(world: &mut Savanna) -> Hominins {
    let b = base();
    let mut h = Hominins::new(7);
    let k = b.kinds.index_of("australopithecus").expect("the hominin");
    h.spawn_group(
        &b.kinds,
        &b.graph,
        &b.items,
        world,
        k,
        [4, 3, 2, 1],
        DVec3::new(2.0, GROUND, 2.0),
        0,
    );
    h
}

/// Lives `seconds` of play from the world's hour.
fn live(h: &mut Hominins, world: &mut Savanna, seconds: f32, person: Option<Person>) {
    let b = base();
    let steps = (seconds / DT) as u64;
    for n in 0..steps {
        world.hour = (world.hour + DT * 24.0 / DAY_S) % 24.0;
        let now = Now {
            tick: n,
            hour: world.hour,
        };
        h.step(
            &b.kinds, &b.crafts, &b.content, &b.items, world, person, now, DT,
        );
    }
}

#[test]
fn a_group_lives_its_days() {
    let mut world = Savanna::new();
    // Marula stones under the tree.
    world.pile.add(
        DVec3::new(19.0, GROUND, 1.0),
        Stack::of(&item("marula_stone"), 30),
    );
    let mut h = group(&mut world);
    let mut cracked = 0;
    let mut slept_in_trees = 0;
    for day in 0..3 {
        // The day's waking hours, then the night.
        let mut at_night = 0;
        let steps = (DAY_S / DT) as u64;
        for n in 0..steps {
            world.hour = (world.hour + DT * 24.0 / DAY_S) % 24.0;
            let now = Now {
                tick: day * steps + n,
                hour: world.hour,
            };
            let b = base();
            h.step(
                &b.kinds, &b.crafts, &b.content, &b.items, &mut world, None, now, DT,
            );
            cracked += h
                .done
                .iter()
                .filter(|d| {
                    d.triggers
                        .iter()
                        .any(|t| t.ends_with("crack_marula_stones"))
                })
                .count();
            if world.hour > 22.0 || world.hour < 4.0 {
                at_night =
                    at_night.max(h.agents.iter().filter(|a| a.medium == Medium::Tree).count());
            }
        }
        slept_in_trees = slept_in_trees.max(at_night);
    }
    for a in &h.agents {
        assert!(a.alive(), "agent {} died: {:?}", a.id, a.body.dead);
        let k = &base().kinds.kinds[a.kind];
        let s = a.body.status(&k.body);
        assert!(
            !matches!(
                s.thirst,
                hearth_body::Thirst::Parched | hearth_body::Thirst::Dying
            ),
            "agent {} parched",
            a.id
        );
        assert!(
            !matches!(s.hunger, hearth_body::Hunger::Starving),
            "agent {} starving",
            a.id
        );
    }
    assert!(world.nests > 0, "nests made");
    assert!(
        slept_in_trees >= h.agents.len() / 2,
        "{slept_in_trees} of {} up the trees at night",
        h.agents.len()
    );
    assert!(cracked > 0, "marula stones cracked at the anvil");
}

#[test]
fn a_leopard_sends_them_up_the_trees_with_the_alarm() {
    let mut world = Savanna::new();
    let mut h = group(&mut world);
    live(&mut h, &mut world, 30.0, None);
    // A leopard comes up close.
    world.leopard = Some(DVec3::new(-8.0, GROUND, 25.0));
    live(&mut h, &mut world, 60.0, None);
    assert!(world.alarms > 0, "the alarm called");
    let up = h.agents.iter().filter(|a| a.medium == Medium::Tree).count();
    let mobbing = h
        .agents
        .iter()
        .filter(|a| matches!(a.mind.doing, Doing::Mobbing { .. }))
        .count();
    assert!(up + mobbing > 0, "they took to the trees or faced it");
    // It goes; in time they are down and about again.
    world.leopard = None;
    live(&mut h, &mut world, 400.0, None);
    let still_up = h.agents.iter().filter(|a| a.medium == Medium::Tree).count();
    assert!(still_up < h.agents.len(), "they came down");
}

#[test]
fn a_running_person_is_fled_and_a_calm_one_watched() {
    let mut world = Savanna::new();
    let mut h = group(&mut world);
    live(&mut h, &mut world, 20.0, None);
    // A person standing quietly at 70 m: watched.
    let calm = Person::standing(DVec3::new(-48.0, GROUND, -48.0));
    live(&mut h, &mut world, 10.0, Some(calm));
    let watching = h
        .agents
        .iter()
        .filter(|a| matches!(a.mind.doing, Doing::Watching { .. }))
        .count();
    assert!(watching > 0, "watched");
    // Running at them from 30 m: fled.
    let running = Person {
        running: true,
        ..Person::standing(DVec3::new(-15.0, GROUND, -20.0))
    };
    live(&mut h, &mut world, 10.0, Some(running));
    let fleeing = h
        .agents
        .iter()
        .filter(|a| {
            matches!(a.mind.doing, Doing::Fleeing { .. } | Doing::Mobbing { .. })
                || a.medium == Medium::Tree
        })
        .count();
    assert!(fleeing > 0, "fled");
}

#[test]
fn a_calm_person_comes_to_be_let_be() {
    let mut world = Savanna::new();
    let mut h = group(&mut world);
    live(&mut h, &mut world, 20.0, None);
    // Standing calmly some 70 m off: watched at first.
    let calm = Person::standing(DVec3::new(-48.0, GROUND, -48.0));
    let watching = |h: &Hominins| {
        h.agents
            .iter()
            .filter(|a| matches!(a.mind.doing, Doing::Watching { .. }))
            .count()
    };
    live(&mut h, &mut world, 10.0, Some(calm));
    let at_first = watching(&h);
    assert!(at_first > 0, "watched at first");
    // Two play days of their company: they come to tolerate the person.
    live(&mut h, &mut world, 2.0 * DAY_S, Some(calm));
    let ease = h.groups[0].habituation;
    println!("watched by {at_first} at first; at ease {ease:.2} after two days");
    assert!(ease > 0.4, "at ease {ease:.2}");
    // Running at them undoes it.
    let running = Person {
        running: true,
        ..Person::standing(DVec3::new(-20.0, GROUND, -20.0))
    };
    live(&mut h, &mut world, 20.0, Some(running));
    let after = h.groups[0].habituation;
    assert!(after < ease - 0.2, "at ease {after:.2} after the run");
}

#[test]
fn a_watcher_learns_from_their_knapping() {
    let b = base();
    let mut world = Savanna::new();
    // Cobbles to knap by the anvil, and a hammer more.
    for k in 0..4 {
        world.pile.add(
            DVec3::new(18.4 + 0.3 * k as f64, GROUND, 2.6),
            Stack::of(&item("cobble/basalt"), 1),
        );
    }
    let mut h = group(&mut world);
    // A group at ease with the watcher, who stands some 30 m off, facing the anvil.
    for g in h.groups.iter_mut() {
        g.habituation = 0.9;
    }
    let eye = DVec3::new(18.0, GROUND + 1.6, -30.0);
    let yaw = 0.0;
    let person = Person::standing(DVec3::new(eye.x, GROUND, eye.z));
    let mut watcher = hearth_craft::KnowledgeState::default();
    let mut heard: std::collections::HashMap<String, u64> = Default::default();
    // What is seen heard an hour apart at most, as the game hears it.
    let hour = (DAY_S / 24.0 / DT) as u64;
    let steps = (2.0 * DAY_S / DT) as u64;
    let mut seen_done = 0;
    for n in 0..steps {
        world.hour = (world.hour + DT * 24.0 / DAY_S) % 24.0;
        let now = Now {
            tick: n,
            hour: world.hour,
        };
        h.step(
            &b.kinds,
            &b.crafts,
            &b.content,
            &b.items,
            &mut world,
            Some(person),
            now,
            DT,
        );
        let seen = hearth_agent::seen(&h.done, &b.graph, &b.crafts, eye, yaw);
        seen_done += seen.len();
        for t in seen {
            let fresh = heard.get(&t).is_none_or(|&last| n - last >= hour);
            if fresh {
                heard.insert(t.clone(), n);
                watcher.observe(&b.graph, &t, n, hearth_craft::Mode::Discovery);
            }
        }
    }
    let node = |id: &str| {
        b.graph
            .nodes
            .iter()
            .find(|k| k.id.ends_with(id))
            .map(|k| k.id.clone())
            .expect("the node")
    };
    let toward = |id: &str| {
        let id = node(id);
        if watcher.knows(&id) {
            1.0
        } else {
            watcher.insight.get(&id).copied().unwrap_or(0.0)
        }
    };
    println!(
        "{seen_done} things seen done; heard {:?}; stone as a hammer {:.2}, sharp flakes {:.2}",
        heard.keys().collect::<Vec<_>>(),
        toward("stone_as_hammer"),
        toward("sharp_flake")
    );
    assert!(
        toward("stone_as_hammer") > 0.0,
        "watching gave insight into the stone as a hammer"
    );
    assert!(
        toward("sharp_flake") > 0.0,
        "watching gave insight toward knapping flakes"
    );
}

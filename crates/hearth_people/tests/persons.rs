//! H0 (V2-11 (b) on the new framework): a person is made from its species profile (its body at
//! its size and sex, its coat, its life history, what its bands know and do); a grown one does
//! its band's work by the same processes as the player, through the same engine — cracking marula
//! stones on an anvil with a hammerstone and eating the kernels, striking flakes and leaving them
//! lying — and a suckling, knowing nothing yet, cannot; its mind chooses by its needs and its
//! fears.

use std::sync::OnceLock;

use glam::DVec3;
use hearth_body::{BodyConfig, Rates};
use hearth_content::Content;
use hearth_content::TimeScales;
use hearth_content::schema::humans::{Behavior, BodyPlan, Language, Teaching};
use hearth_craft::engine::{Aimed, Lack, Surroundings};
use hearth_craft::{Crafts, Graph};
use hearth_fauna::live::Stage;
use hearth_items::{Hand, Items, Stack};
use hearth_math::hash::Rng;
use hearth_people::{
    Doing, Intent, Needs, Now, Offer, Person, Pile, Psyche, Situation, Species, SpeciesSet, Things,
    Threat, choose, finish_work, plan_work,
};

struct World {
    content: Content,
    items: Items,
    crafts: Crafts,
    graph: Graph,
    species: SpeciesSet,
    human: BodyConfig,
}

fn world() -> &'static World {
    static W: OnceLock<World> = OnceLock::new();
    W.get_or_init(|| {
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
        World {
            content,
            items,
            crafts,
            graph,
            species,
            human,
        }
    })
}

fn hominin() -> &'static Species {
    world()
        .species
        .get("australopithecus")
        .expect("the hominin")
}

/// The moment: day 0 of a year of 32 days, mid-morning.
const NOW: Now = Now {
    tick: 0,
    hour: 10.0,
    day: 0.0,
    year_days: 32.0,
};

/// A female of an age (years).
fn person(age: f64) -> Person {
    let w = world();
    Person::new(
        1,
        hominin(),
        &w.graph,
        7,
        true,
        -age * NOW.year_days,
        DVec3::ZERO,
        NOW,
        3,
    )
}

/// An item of the content by the end of its id (`cobble/basalt`, a form of a material).
fn item(suffix: &str) -> String {
    let w = world();
    w.items
        .iter()
        .find(|k| k.id.ends_with(suffix))
        .map(|k| k.id.clone())
        .unwrap_or_else(|| panic!("no item {suffix}"))
}

fn recipe(id: &str) -> usize {
    let w = world();
    w.crafts
        .index_of(id)
        .or_else(|| w.crafts.index_of(&format!("hearth:{id}")))
        .unwrap_or_else(|| panic!("no process {id}"))
}

fn daylight() -> Surroundings {
    Surroundings {
        daylight: true,
        air_c: 26.0,
        humidity: 0.5,
        ..Surroundings::default()
    }
}

#[test]
fn a_species_is_made_from_its_profile() {
    let w = world();
    let k = hominin();
    // Small and dimorphic: a grown female about 29 kg and 1.1 m, a male about 43 kg and 1.4 m.
    assert!(
        (26.0..33.0).contains(&k.mass_kg[0]),
        "female {}",
        k.mass_kg[0]
    );
    assert!(
        (38.0..48.0).contains(&k.mass_kg[1]),
        "male {}",
        k.mass_kg[1]
    );
    assert!(k.height_m[0] < k.height_m[1]);
    assert!(
        (1.0..1.5).contains(&k.height_m[1]),
        "height {}",
        k.height_m[1]
    );
    // Its bodies are the player's physiology at their size: a smaller skin, a slower fire.
    for female in [true, false] {
        let b = k.body(female);
        assert!(b.area_m2 < w.human.area_m2 * 0.8);
        assert!(b.bmr_w < w.human.bmr_w * 0.75);
        assert!(b.params.walk_m_s < w.human.params.walk_m_s);
    }
    // A coat of hair over all of it, and a climber's long arms.
    assert!(k.coat.mean_clo() > 0.3);
    assert_eq!(k.plan, BodyPlan::Australopith);
    assert!(k.climbs);
    // No language, shallow plans, little teaching (V2.1 §2).
    assert_eq!(k.cognition.language, Language::Calls);
    assert_eq!(k.cognition.planning_depth, 1);
    assert_eq!(k.cognition.teaching, Teaching::Minimal);
    for b in [
        Behavior::CrackNuts,
        Behavior::TreeNest,
        Behavior::FleeToTrees,
    ] {
        assert!(k.does(b), "it does {b:?}");
    }
    // Its bands' ways: the knowledge and the processes it opens.
    assert!(k.knowledge.iter().any(|n| n.ends_with("nut_cracking")));
    for p in ["crack_marula_stones", "strike_flake"] {
        assert!(
            k.techniques.iter().any(|t| t.ends_with(p)),
            "its techniques {:?} hold {p}",
            k.techniques
        );
    }
    assert!(
        k.population
            .as_deref()
            .is_some_and(|p| p.ends_with("australopithecus"))
    );
    // It grows from a newborn's twentieth to all of it at maturity, through its age classes.
    assert!(k.growth(0.0) < 0.07);
    assert!((k.growth(k.life.maturity_years as f64) - 1.0).abs() < 1e-4);
    assert_eq!(k.stage(0.5), Stage::Young);
    assert_eq!(k.stage(5.0), Stage::Juvenile);
    assert_eq!(k.stage(20.0), Stage::Adult);
}

#[test]
fn a_grown_one_knows_its_bands_ways_and_a_baby_nothing() {
    let grown = person(20.0);
    let baby = person(0.5);
    let k = hominin();
    for n in &k.knowledge {
        assert!(grown.knowledge.knows(n), "a grown one knows {n}");
        assert!(!baby.knowledge.knows(n), "a baby does not know {n}");
    }
    assert!(grown.knowledge.skill("knapping") > 0.8, "practised");
    assert!(grown.mass_kg(k, &NOW) > baby.mass_kg(k, &NOW) * 4.0);
    assert!((grown.age(&NOW) - 20.0).abs() < 1e-9);
    assert_eq!(baby.stage(k, &NOW), Stage::Young);
}

/// One with a cobble in its hand, at an anvil stone with marula stones by it.
fn at_the_anvil(age: f64) -> (Person, Pile, u64) {
    let w = world();
    let mut a = person(age);
    let k = hominin();
    let hammer = Stack::of(&item("cobble/basalt"), 1);
    let mass = a.mass_kg(k, &NOW);
    a.possessions
        .carry
        .hold(&w.items, hammer, Hand::Right, mass)
        .expect("a hammerstone in hand");
    let mut pile = Pile::default();
    let anvil = pile.add(
        DVec3::new(0.5, 0.0, 0.0),
        Stack::of(&item("anvil_stone/basalt"), 1),
    );
    pile.add(
        DVec3::new(0.8, 0.0, 0.3),
        Stack::of(&item("marula_stone"), 10),
    );
    (a, pile, anvil)
}

#[test]
fn a_hominin_cracks_marula_stones_by_the_players_process() {
    let w = world();
    let k = hominin();
    let (mut a, mut pile, anvil) = at_the_anvil(20.0);
    let cfg = k.body(a.life.female);
    // Hungry, as at the end of a morning.
    let mut hungry = a.body.clone();
    for _ in 0..12 {
        hungry.step(
            cfg,
            3600.0 * cfg.scales.factor(hearth_content::schema::TimeScale::Day),
            &hearth_body::Exposure::mild(),
            &k.coat,
            &cfg.activity("walking"),
        );
    }
    a.body = hungry;
    let before = Needs::of(&a.body, cfg, 0.0).hunger;
    assert!(before > 0.1, "hungry: {before}");
    let r = recipe("crack_marula_stones");
    let lying = pile.near(a.place.pos, hearth_people::REACH_M);
    let aimed = Some(Aimed::Thing(anvil));
    let plan = plan_work(
        &w.crafts,
        &w.content,
        &w.items,
        &a,
        r,
        &lying,
        aimed.clone(),
        daylight(),
    )
    .expect("it can crack the stones here");
    let mut rng = Rng::new(3);
    let stones = |p: &Pile| -> u16 {
        p.things
            .iter()
            .filter(|(_, _, s)| s.id.ends_with("marula_stone"))
            .map(|(_, _, s)| s.count)
            .sum()
    };
    let had = stones(&pile);
    let kcal_before = a.body.energy.clone();
    let o = finish_work(
        &w.crafts,
        &w.content,
        &w.items,
        k,
        &mut a,
        &NOW,
        &plan,
        &lying,
        aimed,
        daylight(),
        &mut pile,
        &mut rng,
    );
    assert!(o.done, "{:?}", o.failure);
    assert!(stones(&pile) < had, "the stones were used");
    // The kernels went into it.
    assert_ne!(a.body.energy, kcal_before, "it ate what it cracked");
    // What a watcher sees done.
    assert!(
        o.triggers
            .iter()
            .any(|t| t.ends_with("crack_marula_stones")),
        "{:?}",
        o.triggers
    );
    assert!(o.triggers.iter().any(|t| t.starts_with("strike:")));
}

#[test]
fn a_suckling_knowing_nothing_cannot() {
    let w = world();
    // Two and a half: big enough to hold a stone, not yet weaned, so knowing none of its band's
    // ways.
    let (a, pile, anvil) = at_the_anvil(2.5);
    let lying = pile.near(a.place.pos, hearth_people::REACH_M);
    let r = recipe("crack_marula_stones");
    let plan = plan_work(
        &w.crafts,
        &w.content,
        &w.items,
        &a,
        r,
        &lying,
        Some(Aimed::Thing(anvil)),
        daylight(),
    );
    assert_eq!(plan.err(), Some(Lack::Knowledge));
}

#[test]
fn a_hominin_strikes_flakes_and_leaves_them_lying() {
    let w = world();
    let k = hominin();
    let mut a = person(20.0);
    let mass = a.mass_kg(k, &NOW);
    a.possessions
        .carry
        .hold(
            &w.items,
            Stack::of(&item("cobble/basalt"), 1),
            Hand::Right,
            mass,
        )
        .expect("hammer");
    a.possessions
        .carry
        .hold(
            &w.items,
            Stack::of(&item("core/flint"), 1),
            Hand::Left,
            mass,
        )
        .expect("core");
    let mut pile = Pile::default();
    let r = recipe("strike_flake");
    let mut rng = Rng::new(11);
    let mut flakes = 0;
    for n in 0..12 {
        let lying = pile.near(a.place.pos, hearth_people::REACH_M);
        let Ok(plan) = plan_work(
            &w.crafts,
            &w.content,
            &w.items,
            &a,
            r,
            &lying,
            None,
            daylight(),
        ) else {
            break;
        };
        let now = Now { tick: n, ..NOW };
        let o = finish_work(
            &w.crafts,
            &w.content,
            &w.items,
            k,
            &mut a,
            &now,
            &plan,
            &lying,
            None,
            daylight(),
            &mut pile,
            &mut rng,
        );
        flakes += o
            .made
            .iter()
            .filter(|s| s.id.contains("flake"))
            .map(|s| s.count)
            .sum::<u16>();
    }
    assert!(flakes > 0, "flakes struck");
    // Its hands being full, they lie about it: a scatter.
    let lying_flakes = pile
        .things
        .iter()
        .filter(|(_, _, s)| s.id.contains("flake"))
        .count();
    assert!(lying_flakes > 0, "flakes left lying");
}

fn calm(pos: DVec3) -> Situation {
    Situation {
        pos,
        hour: 10.0,
        flight_m: 60.0,
        grown: true,
        grown_near: 6,
        ..Situation::default()
    }
}

#[test]
fn its_mind_chooses_by_its_needs_and_fears() {
    let k = hominin();
    let fed = Needs::default();
    let hungry = Needs {
        hunger: 0.7,
        ..Needs::default()
    };
    let thirsty = Needs {
        thirst: 0.8,
        hunger: 0.3,
        ..Needs::default()
    };
    let here = DVec3::ZERO;
    let tree = DVec3::new(10.0, 0.0, 0.0);
    // Hungry by an anvil with nuts: it cracks them; with only fruit about, it feeds.
    let mut s = calm(here);
    s.offers.push(Offer {
        recipe: 3,
        at: DVec3::new(1.0, 0.0, 0.0),
        feeds: true,
    });
    s.food = Some(DVec3::new(20.0, 0.0, 0.0));
    assert_eq!(
        choose(k, &hungry, &s, &Psyche::default(), 0.5),
        Doing::Working { recipe: 3 }
    );
    s.offers.clear();
    assert!(matches!(
        choose(k, &hungry, &s, &Psyche::default(), 0.5),
        Doing::Going {
            then: Intent::Feed,
            ..
        }
    ));
    // Thirsty, it goes to the water.
    let mut t = calm(here);
    t.water = Some(DVec3::new(50.0, 0.0, 0.0));
    assert!(matches!(
        choose(k, &thirsty, &t, &Psyche::default(), 0.5),
        Doing::Going {
            then: Intent::Drink,
            ..
        }
    ));
    // A leopard: seen far off, the alarm; near, up the tree; near and faced by the grown ones, mobbed.
    let mut d = calm(here);
    d.tree = Some(tree);
    d.threat = Some(Threat {
        at: DVec3::new(100.0, 0.0, 0.0),
        dist: 100.0,
        hunter: true,
    });
    assert!(matches!(
        choose(k, &fed, &d, &Psyche::default(), 0.5),
        Doing::Alarm { .. }
    ));
    d.threat = Some(Threat {
        at: DVec3::new(30.0, 0.0, 0.0),
        dist: 30.0,
        hunter: true,
    });
    assert!(matches!(
        choose(k, &fed, &d, &Psyche::default(), 0.5),
        Doing::Mobbing { .. }
    ));
    d.grown_near = 1;
    assert_eq!(
        choose(k, &fed, &d, &Psyche::default(), 0.5),
        Doing::Fleeing { to: tree }
    );
    // The player, not too near: watched.
    let mut p = calm(here);
    p.threat = Some(Threat {
        at: DVec3::new(80.0, 0.0, 0.0),
        dist: 80.0,
        hunter: false,
    });
    assert!(matches!(
        choose(k, &fed, &p, &Psyche::default(), 0.5),
        Doing::Watching { .. }
    ));
    // Night: up a tree to make its nest, and asleep in it.
    let mut n = calm(here);
    n.hour = 20.0;
    n.tree = Some(tree);
    assert_eq!(
        choose(k, &fed, &n, &Psyche::default(), 0.5),
        Doing::Going {
            to: tree,
            then: Intent::Nest
        }
    );
    n.in_tree = true;
    assert_eq!(choose(k, &fed, &n, &Psyche::default(), 0.5), Doing::Nesting);
    n.in_nest = true;
    assert_eq!(
        choose(k, &fed, &n, &Psyche::default(), 0.5),
        Doing::Sleeping
    );
}

//! H3 (V2.1 §7): the stages of a life and how the young grow — a newborn a third of grown
//! height, a child's body small with its metabolism, an infant carried by its mother and nursed,
//! a child kept hungry growing shorter.

mod common;

use common::*;
use hearth_people::{Doing, LifeStage};

#[test]
fn stages_and_growth_follow_the_species() {
    let b = base();
    let sp = &b.species.list[b.species.index_of("australopithecus").expect("the hominin")];
    let stage = |age: f64| sp.life_stage(age);
    assert_eq!(stage(1.0), LifeStage::Infant);
    assert_eq!(stage(6.0), LifeStage::Juvenile);
    assert_eq!(stage(10.0), LifeStage::Adolescent);
    assert_eq!(stage(20.0), LifeStage::Adult);
    assert_eq!(stage(40.0), LifeStage::Elder);
    let grown = sp.height_m(true, 30.0, 0.0);
    assert!((sp.height_m(true, 0.0, 0.0) / grown - 0.35).abs() < 0.01);
    assert!(sp.height_m(true, 5.0, 0.0) < sp.height_m(true, 8.0, 0.0));
    // A child's body is its size: less blood, a smaller stomach, a slower metabolism.
    let child = sp.body_at(true, 3.0, 0.0);
    let adult = sp.body_at(true, 30.0, 0.0);
    assert!(child.mass_kg < adult.mass_kg * 0.4);
    assert!(child.bmr_w < adult.bmr_w * 0.6);
    assert!(child.params.blood_l < adult.params.blood_l * 0.5);
}

#[test]
fn an_infant_is_carried_by_its_mother_and_hunger_stunts() {
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    live(&mut p, &mut world, 10.0, &[]);
    let b = base();
    let sp = &b.species.list[b.species.index_of("australopithecus").expect("the hominin")];
    let now = world.now();
    let infant = p
        .full()
        .find(|q| q.life_stage(sp, &now) == LifeStage::Infant)
        .expect("the band's baby");
    let mother = infant.life.mother.expect("a mother");
    assert_eq!(infant.mind.doing, Doing::Carried { by: mother });
    let m = p.get(mother).expect("the mother");
    let apart = infant.place.pos - m.place.pos;
    assert!(
        apart.x.hypot(apart.z) < 0.5,
        "carried {apart:?} from its mother"
    );
    // A child who went hungry while growing is shorter than one who did not.
    let mut hungry = infant.clone();
    hungry.life.undernourished = 0.5;
    assert!(hungry.height_m(sp, &now) < infant.height_m(sp, &now) * 0.99);
}

#[test]
fn an_infant_held_little_carries_stress_more_heavily_within_bounds() {
    // Attachment (V2.1 §18, development): the same hard days weigh more on one who had no one
    // to hold to as an infant, but not beyond the designed half again.
    let b = base();
    let defs = &b.species.psyche;
    let mut held = hearth_people::Psyche::default();
    let mut alone = hearth_people::Psyche {
        insecure: 0.5,
        ..hearth_people::Psyche::default()
    };
    for day in 1..=12 {
        let d = day as f64 * 0.25;
        held.pass(defs, 1.0, d, 0.4);
        alone.pass(defs, 1.0, d, 0.4);
    }
    println!(
        "stress {:.3} held, {:.3} held little",
        held.stress, alone.stress
    );
    assert!(alone.stress > held.stress * 1.05, "it weighs more");
    assert!(alone.stress < held.stress * 1.5, "but within bounds");
}

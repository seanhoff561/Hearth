//! Things as the content makes them, containers that hold them as containers do, and what a
//! person can carry: the tie, a pouch, hands and arms, a load that slows, a log only dragged.

use std::sync::OnceLock;

use hearth_content::Content;
use hearth_items::{Carry, Container, ContainerSpec, Hand, Items, Misfit, Refusal, Stack};

fn items() -> &'static Items {
    static I: OnceLock<Items> = OnceLock::new();
    I.get_or_init(|| Items::from_content(&Content::load_base()))
}

fn id(suffix: &str) -> String {
    items()
        .iter()
        .find(|k| k.id.ends_with(suffix))
        .unwrap_or_else(|| panic!("no item {suffix}"))
        .id
        .clone()
}

fn spec(of: &str) -> ContainerSpec {
    items()
        .get(of)
        .and_then(|k| k.container)
        .expect("a container")
}

const BODY_KG: f32 = 70.0;

#[test]
fn things_weigh_what_they_would() {
    let i = items();
    assert!(i.len() > 100, "{} kinds", i.len());
    for k in i.iter() {
        assert!(
            k.mass_kg > 0.0 && k.mass_kg.is_finite(),
            "{}: {} kg",
            k.id,
            k.mass_kg
        );
        assert!(k.volume_l > 0.0, "{}", k.id);
        assert!(k.footprint.0 >= 1 && k.footprint.1 >= 1, "{}", k.id);
        assert!(k.size_m.iter().all(|s| *s > 0.0), "{}", k.id);
    }
    let flake = i.get(&id("flake/flint")).expect("flake");
    assert!(
        (0.01..0.05).contains(&flake.mass_kg),
        "a flake {}",
        flake.mass_kg
    );
    let axe = i.get(&id("hand_axe/flint")).expect("hand axe");
    assert!(
        (0.3..1.2).contains(&axe.mass_kg),
        "a hand axe {}",
        axe.mass_kg
    );
    let log = i.get(&id("log_section/oak_wood")).expect("log");
    assert!(
        (80.0..110.0).contains(&log.mass_kg),
        "a log section {}",
        log.mass_kg
    );
    // Garments are things, worn on their regions with their attachment points.
    let loincloth = i
        .iter()
        .find(|k| {
            k.wear
                .as_ref()
                .is_some_and(|w| w.garment.ends_with("loincloth"))
        })
        .expect("a loincloth");
    assert_eq!(
        loincloth.wear.as_ref().map(|w| w.attachments.clone()),
        Some(vec!["tie".to_owned()])
    );
    let belt = i
        .iter()
        .find(|k| k.wear.as_ref().is_some_and(|w| w.garment.ends_with("belt")))
        .expect("a belt");
    assert_eq!(belt.wear.as_ref().map(|w| w.attachments.len()), Some(3));
}

#[test]
fn containers_hold_what_fits() {
    let i = items();
    let pouch = id("pouch/rawhide");
    let flake = id("flake/flint");
    let cobble = id("cobble/granite");
    let pouch_spec = spec(&pouch);
    let mut c = Container::default();
    // Twenty flakes fill three of the pouch's four cells (eight to a cell).
    c.put_anywhere(i, &pouch_spec, Stack::of(&flake, 20))
        .expect("flakes go in");
    assert_eq!(c.items.len(), 3);
    assert_eq!(c.items.iter().map(|p| p.stack.count).sum::<u16>(), 20);
    // A cobble is too much for a pouch on top of them.
    let stone = Stack::one(&cobble);
    assert!(stone.mass(i) > 0.8, "a cobble {} kg", stone.mass(i));
    let back = c
        .put_anywhere(i, &pouch_spec, stone)
        .expect_err("too heavy");
    assert_eq!(back.count, 1);
    // A bundle: things don't overlap, a long thing turns to fit, a full pouch goes inside.
    let bundle = id("bundle/rawhide");
    let bundle_spec = spec(&bundle);
    let axe = id("hand_axe/flint");
    let mut b = Container::default();
    b.put(i, &bundle_spec, Stack::one(&axe), 0, 0, false)
        .expect("the axe");
    let (_, why) = b
        .put(i, &bundle_spec, Stack::one(&axe), 0, 1, false)
        .expect_err("overlapping");
    assert_eq!(why, Misfit::Overlaps);
    let (_, why) = b
        .put(i, &bundle_spec, Stack::one(&axe), 0, 2, false)
        .expect_err("hanging out of the bottom");
    assert_eq!(why, Misfit::Outside);
    b.put(i, &bundle_spec, Stack::one(&axe), 1, 2, true)
        .expect("turned along the bottom");
    let mut full = Stack::one(&pouch);
    *full.contents_mut(i).expect("a pouch opens") = c.clone();
    let pouch_mass = full.mass(i);
    b.put_anywhere(i, &bundle_spec, full).expect("the pouch");
    let expected = 2.0 * i.get(&axe).expect("axe").mass_kg + pouch_mass;
    assert!(
        (b.mass(i) - expected).abs() < 1e-4,
        "{} vs {expected}",
        b.mass(i)
    );
    // A water skin holds water, not things.
    let skin = id("water_skin/scraped_hide");
    let (_, why) = Container::default()
        .put(i, &spec(&skin), Stack::one(&flake), 0, 0, false)
        .expect_err("no room");
    assert_eq!(why, Misfit::NoRoom);
    // It all survives a save.
    let json = serde_json::to_string(&b).expect("json");
    let again: Container = serde_json::from_str(&json).expect("back");
    assert_eq!(again, b);
}

#[test]
fn a_new_person_carries_almost_nothing() {
    let i = items();
    let loincloth = i
        .iter()
        .find(|k| {
            k.wear
                .as_ref()
                .is_some_and(|w| w.garment.ends_with("loincloth"))
        })
        .expect("loincloth")
        .id
        .clone();
    let mut c = Carry::dressed(i, [Stack::one(&loincloth)]);
    // The tie takes one small thing: a flake, not a cobble.
    let flake = Stack::one(&id("flake/flint"));
    let cobble = Stack::one(&id("cobble/granite"));
    let (cobble, why) = c
        .hang(i, cobble, 0, 0)
        .expect_err("a cobble is no small thing");
    assert_eq!(why, Refusal::Wrong);
    c.hang(i, flake.clone(), 0, 0).expect("a flake on the tie");
    let (_, why) = c.hang(i, flake, 0, 0).expect_err("one thing");
    assert_eq!(why, Refusal::Taken);
    // Then the hands: one stone each.
    c.hold(i, cobble.clone(), Hand::Left, BODY_KG)
        .expect("left");
    c.hold(i, cobble.clone(), Hand::Right, BODY_KG)
        .expect("right");
    let (_, why) = c.hold(i, cobble, Hand::Right, BODY_KG).expect_err("full");
    assert_eq!(why, Refusal::HandsFull);
    let load = c.load(i, BODY_KG);
    assert!(load.share < 0.05, "{load:?}");
    assert!(load.can_jog() && load.can_sprint());
}

#[test]
fn loads_slow_and_logs_are_dragged() {
    let i = items();
    let log = Stack::one(&id("log_section/oak_wood"));
    let mut c = Carry::default();
    // A log section is beyond carrying: dragged, slowly.
    let (log, why) = c
        .hold(i, log, Hand::Right, BODY_KG)
        .expect_err("too heavy to carry");
    assert_eq!(why, Refusal::DragIt);
    c.drag(i, log, BODY_KG).expect("dragged");
    let load = c.load(i, BODY_KG);
    let on_grass = load.drag_speed(0.45).expect("dragging");
    assert!((0.15..0.4).contains(&on_grass), "{on_grass} m/s on grass");
    let on_snow = load.drag_speed(0.12).expect("dragging");
    assert!(on_snow > 3.0 * on_grass, "snow is easier: {on_snow}");
    assert!(!load.can_jog());
    // Its work at that pace: several hundred watts.
    let w = load.work_w(BODY_KG, on_grass, 0.0, 0.45);
    assert!((300.0..700.0).contains(&w), "{w} W");
    // Carried loads: a fifth of the body's mass barely slows; two fifths slows by a third.
    let stone = |kg: f32| Stack::of(&id("cobble/granite"), (kg / 1.1) as u16);
    let walk = |kg: f32| {
        Carry {
            back: Some(stone(kg)),
            ..Carry::default()
        }
        .load(i, BODY_KG)
    };
    assert!(walk(14.0).walk() > 0.98);
    let heavy = walk(30.0);
    assert!((0.6..0.8).contains(&heavy.walk()), "{}", heavy.walk());
    assert!(!heavy.can_jog());
    // Pandolf: 20 kg at a walk costs a third more work.
    let l = walk(20.0);
    let extra = l.work_w(BODY_KG, 1.4, 0.0, 0.0);
    assert!((60.0..150.0).contains(&extra), "{extra} W for 20 kg");
    // Nothing moves three times the body's mass.
    let mut big = Stack::one(&id("log_section/oak_wood"));
    big.count = 3;
    let (_, why) = Carry::default()
        .drag(i, big, BODY_KG)
        .expect_err("immovable");
    assert_eq!(why, Refusal::Immovable);
}

#[test]
fn moving_things_round_is_all_or_nothing() {
    use hearth_items::{Path, Root, Target};
    let i = items();
    let loincloth = i
        .iter()
        .find(|k| {
            k.wear
                .as_ref()
                .is_some_and(|w| w.garment.ends_with("loincloth"))
        })
        .expect("loincloth")
        .id
        .clone();
    let mut c = Carry::dressed(i, [Stack::one(&loincloth)]);
    // A pouch on the tie, flakes in a hand.
    c.hang(i, Stack::one(&id("pouch/rawhide")), 0, 0)
        .expect("pouch");
    c.hold(i, Stack::of(&id("flake/flint"), 12), Hand::Right, BODY_KG)
        .expect("flakes");
    let hand = Path::at(Root::Hand(Hand::Right));
    let pouch = Path::at(Root::Hung(0, 0));
    let cell = |x: u8, y: u8| Target::Cell {
        container: pouch.clone(),
        x,
        y,
        turned: false,
    };
    // Five of them into the top right of the pouch.
    assert!(c.shift(i, &hand, Some(5), &cell(1, 0), BODY_KG));
    assert_eq!(c.get(&hand).map(|s| s.count), Some(7));
    assert_eq!(c.get(&pouch.inner(0)).map(|s| s.count), Some(5));
    // The rest stowed: into the pouch; the hand empties.
    assert!(c.shift(i, &hand, None, &Target::Stow, BODY_KG));
    assert!(c.right.is_none());
    let in_pouch: u16 = c
        .get(&pouch)
        .and_then(|p| p.contents())
        .map_or(0, |b| b.items.iter().map(|p| p.stack.count).sum());
    assert_eq!(in_pouch, 12);
    // A hand axe will not go into the pouch: it stays in the hand.
    c.hold(i, Stack::one(&id("hand_axe/flint")), Hand::Left, BODY_KG)
        .expect("hand axe");
    let left = Path::at(Root::Hand(Hand::Left));
    assert!(!c.shift(i, &left, None, &cell(0, 1), BODY_KG));
    assert!(c.left.is_some(), "still held");
    // The pouch cannot go into itself; the loincloth cannot come off with the pouch on it.
    assert!(!c.shift(i, &pouch, None, &cell(0, 1), BODY_KG));
    assert!(c.take(i, &Path::at(Root::Worn(0)), None).is_none());
}

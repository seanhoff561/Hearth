//! V2-12 (v2 §11.4–11.6): the Neolithic's other crafts in a running world — flax spun on a
//! spindle and woven on a warp-weighted loom into linen, sewn into a tunic; a log split into
//! planks with wedges; a load that drags hard on the ground rolling easily on a handcart's
//! wheels; a dugout dragged by its owner, to be paddled.

mod common;

use common::{World, temp};
use hearth_items::{Hand, Path, Root, Stack, Target};
use hearth_math::BlockPos;
use hearth_protocol::{AimAt, ToServer};

/// Open ground beside the player (the top solid block of a nearby column).
fn site(w: &World) -> BlockPos {
    let ground = w.ground();
    [(2, 0), (-2, 0), (0, 2), (0, -2), (2, 2), (-2, -2)]
        .iter()
        .find_map(|(dx, dz)| {
            let mut p = BlockPos::new(ground.x + dx, ground.y + 2, ground.z + dz);
            for _ in 0..5 {
                if w.solid(p) {
                    return w.free(p.up()).then_some(p);
                }
                p = p.down();
            }
            None
        })
        .expect("ground to build on")
}

/// Takes something carried into a hand (a tool works only in a hand).
fn hold(w: &mut World, id: &str, hand: Hand) {
    let roots = [
        Root::Hand(Hand::Right),
        Root::Hand(Hand::Left),
        Root::Hung(0, 0),
        Root::Back,
    ];
    let from = roots
        .into_iter()
        .find(|r| w.carry.get(&Path::at(*r)).is_some_and(|s| s.id == id));
    if let Some(from) = from
        && from != Root::Hand(hand)
    {
        w.server.send(ToServer::Shift {
            from: Path::at(from),
            count: None,
            to: Target::Root(Root::Hand(hand)),
        });
        w.run(2);
    }
    assert!(
        w.carry
            .get(&Path::at(Root::Hand(hand)))
            .is_some_and(|s| s.id == id),
        "{id} in hand: {:?}",
        w.carry
    );
}

/// Mild weather to work in through the hours the crafts take.
fn mild(w: &mut World) {
    w.server.send(ToServer::HoldWeather(Some(
        hearth_env::weather::WeatherHold {
            temperature_c: Some(24.0),
            humidity: Some(0.5),
            precip_mm_h: Some(0.0),
            wind_speed_m_s: Some(1.0),
            ..hearth_env::weather::WeatherHold::default()
        },
    )));
    w.run(2);
}

#[test]
fn flax_is_spun_and_woven_into_linen_and_sewn_into_a_tunic() {
    let dir = temp("neolithic-linen");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 11);
    mild(&mut w);
    // A spindle in hand, flax fibre to hand: two skeins of yarn.
    w.give("hearth:spindle/oak_wood", 1);
    hold(&mut w, "hearth:spindle/oak_wood", Hand::Right);
    for _ in 0..2 {
        w.give("hearth:hank/flax_fibre", 12);
        let (done, words) = w.act("spin_flax", AimAt::Nothing);
        assert!(done, "spun: {words}");
    }
    assert!(w.at_hand(|id| id == "hearth:yarn/flax_fibre", 2.5) >= 2);
    // A loom set up: uprights, a beam, the warp hung with weights.
    w.put_down_all();
    let at = site(&w);
    w.give("hearth:stick/oak_wood", 4);
    w.give("hearth:cord/nettle_fibre", 4);
    w.give("hearth:cobble/granite", 8);
    let (done, words) = w.act(
        "build_warp_weighted_loom",
        AimAt::Block { pos: at, top: true },
    );
    assert!(done, "loom: {words}");
    let loom = at.up();
    assert!(w.until(10.0, |w| w.block(loom).as_deref()
        == Some("warp_weighted_loom")));
    // A length of linen from each skein.
    for _ in 0..2 {
        let (done, words) = w.act(
            "weave_linen",
            AimAt::Block {
                pos: loom,
                top: false,
            },
        );
        assert!(done, "woven: {words}");
    }
    assert!(w.at_hand(|id| id == "hearth:length/linen", 2.5) >= 2);
    // Sewn with linen yarn, a bone needle and a flint flake: a tunic.
    w.put_down_all();
    w.give("hearth:needle/bone", 1);
    hold(&mut w, "hearth:needle/bone", Hand::Right);
    w.give("hearth:flake/flint", 1);
    hold(&mut w, "hearth:flake/flint", Hand::Left);
    w.give("hearth:yarn/flax_fibre", 1);
    let (done, words) = w.act("sew_tunic", AimAt::Nothing);
    assert!(done, "sewn: {words}");
    assert!(
        w.at_hand(|id| id == "hearth:tunic/linen", 2.5) >= 1,
        "a tunic"
    );
}

#[test]
fn a_log_is_split_into_planks_with_wedges() {
    let dir = temp("neolithic-planks");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 11);
    mild(&mut w);
    w.give("hearth:wedge/oak_wood", 1);
    hold(&mut w, "hearth:wedge/oak_wood", Hand::Right);
    w.give("hearth:cobble/granite", 1);
    hold(&mut w, "hearth:cobble/granite", Hand::Left);
    // The log too heavy to hold: it lies at the feet.
    w.give("hearth:log_section/oak_wood", 1);
    let (done, words) = w.act("split_planks", AimAt::Nothing);
    assert!(done, "split: {words}");
    let planks = w.at_hand(|id| id == "hearth:plank/oak_wood", 3.0);
    assert!((4..=6).contains(&planks), "planks: {planks}");
}

#[test]
fn wheels_roll_a_load_that_drags_on_the_ground_and_a_dugout_is_paddled() {
    let dir = temp("neolithic-vehicles");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 11);
    let pace = |w: &mut World| {
        w.run(30);
        w.body.as_ref().map(|b| b.ability.walk_m_s).unwrap_or(0.0)
    };
    // A log of oak dragged over the ground (near ninety kilograms).
    w.give("hearth:log_section/oak_wood", 1);
    w.run(4);
    let dragging_log = w.carry.dragging.is_some();
    let log_pace = pace(&mut w);
    w.let_go();
    // A handcart loaded with as much stone, its wheels rolling.
    let mut cart = Stack::of("hearth:handcart/oak_wood", 1);
    {
        let spec = w
            .items
            .get(&cart.id)
            .and_then(|k| k.container)
            .expect("a cart holds things");
        let inside = cart.contents_mut(&w.items).expect("contents");
        for _ in 0..30 {
            inside
                .put_anywhere(&w.items, &spec, Stack::of("hearth:cobble/granite", 1))
                .expect("room in the cart");
        }
    }
    let load_kg = cart.mass(&w.items);
    w.server.send(ToServer::Give(cart));
    w.run(4);
    assert!(w.carry.dragging.is_some(), "the loaded cart pulled");
    let cart_pace = pace(&mut w);
    println!(
        "log dragged ({dragging_log}): {log_pace:.2} m/s; cart of {load_kg:.0} kg: {cart_pace:.2} m/s"
    );
    assert!(dragging_log && log_pace < 0.6, "a log drags: {log_pace}");
    assert!(
        cart_pace > 2.0 * log_pace && cart_pace > 1.0,
        "wheels roll: {cart_pace}"
    );
    w.let_go();
    // A dugout dragged to the water is sat in and paddled.
    w.give("hearth:dugout_canoe/oak_wood", 1);
    w.run(30);
    assert!(w.carry.dragging.is_some(), "the dugout dragged");
    let boat = w.body.as_ref().map_or(0.0, |b| b.ability.boat_m_s);
    assert!(boat > 1.0, "paddled at {boat} m/s");
}

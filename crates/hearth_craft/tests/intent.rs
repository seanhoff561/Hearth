//! Each hand's natural use (Amendment P §5.2): every example of the amendment resolves as it
//! says, a hand holding something puts it away for an empty hand's use, and nothing risky is a
//! default.

use hearth_content::Content;
use hearth_content::schema::Season;
use hearth_craft::intent::{Ask, HandAct, HandUse, resolve};
use hearth_craft::{Aimed, Bench, Crafts, Surroundings};
use hearth_items::{Carry, Hand, Items, Stack};

struct World {
    content: Content,
    items: Items,
    crafts: Crafts,
}

fn world() -> World {
    let content = Content::load_base();
    let items = Items::from_content(&content);
    let crafts = Crafts::from_content(&content, &items);
    World {
        content,
        items,
        crafts,
    }
}

fn carry(w: &World, right: Option<&str>, left: Option<&str>) -> Carry {
    let mut c = Carry::default();
    for (id, hand) in [(right, Hand::Right), (left, Hand::Left)] {
        if let Some(id) = id {
            assert!(w.items.get(id).is_some(), "no item {id}");
            c.hold(&w.items, Stack::one(id), hand, 70.0).expect("hold");
        }
    }
    c
}

fn summer() -> Surroundings {
    Surroundings {
        humidity: 0.5,
        daylight: true,
        air_c: 18.0,
        season: Some(Season::Summer),
        ..Surroundings::default()
    }
}

fn block(name: &str, material: Option<&str>, ground: bool) -> Option<Aimed> {
    Some(Aimed::Block {
        name: name.to_owned(),
        material: material.map(str::to_owned),
        ground,
        room: false,
    })
}

/// What `hand` does, holding what `carry` puts in it, on `aimed`.
fn use_of(w: &World, carry: &Carry, hand: Hand, aimed: Option<Aimed>) -> Option<HandUse> {
    let lying = [(9u64, Stack::one("hearth:cobble/flint"))];
    let bench = Bench::new(
        &w.content,
        &w.items,
        carry,
        lying.iter().map(|(id, s)| (*id, s)),
        aimed,
        summer(),
    )
    .by_hand(Some(hand));
    let held = match hand {
        Hand::Right => carry.right.as_ref(),
        Hand::Left => carry.left.as_ref(),
    };
    let food = |k: &hearth_items::ItemKind, _: &Stack| k.id == "hearth:cut/cooked_meat";
    let ask = Ask {
        may: &|_| true,
        skill: &|_| 0.5,
        known_food: &food,
        prefer: None,
    };
    resolve(&w.content, &w.crafts, &bench, held, &ask)
}

fn process(w: &World, u: &Option<HandUse>) -> String {
    match u.as_ref().map(|u| &u.act) {
        Some(HandAct::Process(i)) => w.crafts.recipes[*i].def.id.clone(),
        other => format!("{other:?}"),
    }
}

#[test]
fn every_example_of_the_hands_resolves_as_the_amendment_says() {
    let w = world();
    let empty = Carry::default();
    // Empty hand + a reachable branch: snap it off.
    let u = use_of(
        &w,
        &empty,
        Hand::Left,
        block("hearth:oak_branch", Some("hearth:oak_wood"), false),
    );
    assert_eq!(
        u.as_ref().map(|u| u.hint.as_str()),
        Some("snap off"),
        "{u:?}"
    );
    // Empty hand + berries: pick them.
    let u = use_of(
        &w,
        &empty,
        Hand::Right,
        block("hearth:raspberry", None, false),
    );
    assert_eq!(process(&w, &u), "hearth:pick_summer_berries");
    // Empty hand + water: cup the hands and drink.
    let u = use_of(&w, &empty, Hand::Right, Some(Aimed::Water));
    assert!(matches!(u.map(|u| u.act), Some(HandAct::Drink)));
    // Empty hand + a loose stone, or a thing lying there: pick it up.
    let u = use_of(
        &w,
        &empty,
        Hand::Left,
        block("hearth:flint_cobbles", None, false),
    );
    assert_eq!(process(&w, &u), "hearth:gather_stones");
    let u = use_of(&w, &empty, Hand::Left, Some(Aimed::Thing(9)));
    assert!(matches!(u.map(|u| u.act), Some(HandAct::PickUp)));
    // Empty hand + soft soil: dig by hand.
    let u = use_of(
        &w,
        &empty,
        Hand::Right,
        block("hearth:grass_block", None, true),
    );
    assert_eq!(process(&w, &u), "hearth:dig_by_hand");
    // A hammerstone + a flint nodule in the other hand: strike a flake (testing the nodule
    // first is the same blow).
    let knapping = carry(
        &w,
        Some("hearth:cobble/granite"),
        Some("hearth:cobble/flint"),
    );
    let u = use_of(&w, &knapping, Hand::Right, None);
    let p = process(&w, &u);
    assert!(
        p == "hearth:test_nodule" || p == "hearth:strike_flake" || p == "hearth:knock_stones",
        "{p}"
    );
    assert_eq!(u.map(|u| u.hint).as_deref(), Some("strike"));
    // A stone axe + a tree: chop.
    let axe = carry(&w, Some("hearth:hand_axe/flint"), None);
    let u = use_of(
        &w,
        &axe,
        Hand::Right,
        block("hearth:oak_log", Some("hearth:oak_wood"), false),
    );
    assert_eq!(process(&w, &u), "hearth:fell_tree");
    // The axe works in the hand that holds it only: the empty left hand does not chop.
    let u = use_of(
        &w,
        &axe,
        Hand::Left,
        block("hearth:oak_log", Some("hearth:oak_wood"), false),
    );
    assert_ne!(process(&w, &u), "hearth:fell_tree");
    // A spear + an animal: thrust.
    let spear = carry(&w, Some("hearth:wooden_spear/oak_wood"), None);
    let deer = Some(Aimed::Animal {
        species: "hearth:red_deer".into(),
        kept: false,
        young: false,
        female: true,
        domesticable: false,
    });
    let u = use_of(&w, &spear, Hand::Right, deer.clone());
    assert!(matches!(u.as_ref().map(|u| &u.act), Some(HandAct::Blow)));
    assert_eq!(u.map(|u| u.hint).as_deref(), Some("thrust"));
    // A water skin + water: fill it.
    let skin = carry(&w, Some("hearth:water_skin/scraped_hide"), None);
    let u = use_of(&w, &skin, Hand::Right, Some(Aimed::Water));
    assert!(matches!(u.map(|u| u.act), Some(HandAct::Fill)));
    // Food in hand + nothing looked at: eat.
    let meat = carry(&w, Some("hearth:cut/cooked_meat"), None);
    let u = use_of(&w, &meat, Hand::Right, None);
    assert!(matches!(u.map(|u| u.act), Some(HandAct::Eat)));

    // A hand holding a stone puts it away to pick berries.
    let stone = carry(&w, None, Some("hearth:cobble/granite"));
    let u = use_of(
        &w,
        &stone,
        Hand::Left,
        block("hearth:raspberry", None, false),
    )
    .expect("pick");
    assert!(u.stow, "the stone is put away first");

    // Nothing risky by default: an empty hand does not punch an animal; food not known to be
    // food is not eaten.
    assert!(use_of(&w, &empty, Hand::Right, deer).is_none());
    let unknown = carry(&w, Some("hearth:cut/meat"), None);
    let u = use_of(&w, &unknown, Hand::Right, None);
    assert!(!matches!(u.map(|u| u.act), Some(HandAct::Eat)));
}

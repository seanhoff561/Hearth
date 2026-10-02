//! The process engine against the game's content: experiments teach, knowledge opens
//! techniques, bulk materials are counted in their units, skill and tools set the pace, and
//! outputs are made of the material in play.

use hearth_content::Content;
use hearth_content::schema::Season;
use hearth_craft::engine::{offers, perform, plan};
use hearth_craft::{Aimed, Bench, Crafts, Event, Graph, KnowledgeState, Mode, Surroundings};
use hearth_items::{Carry, Hand, Items, Stack};
use hearth_math::hash::Rng;

struct World {
    content: Content,
    items: Items,
    crafts: Crafts,
    graph: Graph,
}

fn world() -> World {
    let content = Content::load_base();
    let items = Items::from_content(&content);
    let crafts = Crafts::from_content(&content, &items);
    let graph = Graph::from_content(&content);
    World {
        content,
        items,
        crafts,
        graph,
    }
}

fn kind(w: &World, id: &str) -> String {
    assert!(w.items.get(id).is_some(), "no item {id}");
    id.to_owned()
}

fn holding(w: &World, right: &str, left: Option<&str>) -> Carry {
    let mut c = Carry::default();
    c.hold(&w.items, Stack::one(&kind(w, right)), Hand::Right, 70.0)
        .expect("right");
    if let Some(l) = left {
        c.hold(&w.items, Stack::one(&kind(w, l)), Hand::Left, 70.0)
            .expect("left");
    }
    c
}

fn around() -> Surroundings {
    Surroundings {
        humidity: 0.5,
        daylight: true,
        air_c: 15.0,
        ..Surroundings::default()
    }
}

#[test]
fn knocking_flint_together_teaches_hammering_and_flakes() {
    let w = world();
    let carry = holding(&w, "hearth:cobble/flint", Some("hearth:cobble/granite"));
    let bench = Bench::new(&w.content, &w.items, &carry, [], None, around());
    let knock = w.crafts.index_of("hearth:knock_stones").expect("knock");
    let mut known = KnowledgeState::default();
    let list = offers(
        &w.crafts,
        &bench,
        &|r| known.may_attempt(r.def.knowledge.as_ref().map(|k| k.as_str())),
        &|_| 0.0,
    );
    assert!(
        list.iter().any(|(i, p)| *i == knock && p.is_ok()),
        "anyone can knock stones together"
    );
    // Knowledge-gated work is not offered yet.
    let test = w.crafts.index_of("hearth:test_nodule").expect("test");
    assert!(!list.iter().any(|(i, _)| *i == test));
    let p = plan(&w.crafts, knock, &bench, 0.0).expect("plan");
    let mut rng = Rng::new(7);
    let mut learned = Vec::new();
    let mut flakes = 0;
    for tick in 0..12 {
        let o = perform(&w.crafts, &p, &bench, 0.0, &mut rng);
        assert!(
            o.triggers.iter().any(|t| t == "strike:stone"),
            "{:?}",
            o.triggers
        );
        assert!(o.triggers.iter().any(|t| t == "strike:knappable"));
        flakes += o
            .made
            .iter()
            .filter(|s| s.id == "hearth:flake/flint")
            .count();
        for t in &o.triggers {
            for e in known.observe(&w.graph, t, tick, Mode::Discovery) {
                if let Event::Learned(n) = e {
                    learned.push(n);
                }
            }
        }
    }
    assert!(flakes > 0, "some crude flakes come off");
    assert!(
        learned.contains(&"hearth:stone_as_hammer".to_owned()),
        "{learned:?}"
    );
    assert!(
        learned.contains(&"hearth:sharp_flake".to_owned()),
        "{learned:?}"
    );
    assert!(
        known
            .journal
            .iter()
            .any(|n| n.text.contains("Sharp flakes")),
        "the journal tells of it"
    );
    // Now knapping proper is open.
    assert!(known.may_attempt(Some("hearth:stone_as_hammer")));
}

/// Butchers `carcass` (as given: what is left of it, how far gone) with a flint flake in a
/// season, at full skill: what it gives.
fn butcher(w: &World, carcass: &Stack, season: Season, seed: u64) -> hearth_craft::Outcome {
    let carry = holding(w, "hearth:flake/flint", None);
    let bench = Bench::new(
        &w.content,
        &w.items,
        &carry,
        [(5, carcass)],
        Some(Aimed::Thing(5)),
        Surroundings {
            season: Some(season),
            ..around()
        },
    );
    let process = hearth_content::butchery::butcher_id(&carcass.id);
    let r = w.crafts.index_of(&process).expect("butcher");
    let p = plan(&w.crafts, r, &bench, 0.5).expect("plan");
    assert_eq!(p.uses.len(), 1, "the carcass is used up");
    let mut rng = Rng::new(seed);
    loop {
        let o = perform(&w.crafts, &p, &bench, 1.0, &mut rng);
        if o.done {
            break o;
        }
    }
}

fn count(o: &hearth_craft::Outcome, id: &str) -> u16 {
    o.made.iter().filter(|s| s.id == id).map(|s| s.count).sum()
}

#[test]
fn butchering_gives_meat_hide_bone_in_their_units() {
    let w = world();
    let roe = Stack::one(&kind(&w, "hearth:roe_deer_carcass"));
    let o = butcher(&w, &roe, Season::Summer, 3);
    // A roe doe of 23 kg: about 8–10 kg of meat in quarter-kilo cuts, a hide of a kilo and a
    // half, two or three kilos of bone.
    let meat = count(&o, "hearth:cut/meat");
    assert!(
        (30..=40).contains(&meat),
        "8–10 kg of meat in quarter-kilo cuts: {meat}"
    );
    assert!(
        (1..=2).contains(&count(&o, "hearth:sheet/rawhide")),
        "one hide"
    );
    assert!(count(&o, "hearth:piece/bone") >= 14, "bone pieces");
    assert!(o.triggers.iter().any(|t| t == "cut:carcass"));
    assert!(o.triggers.iter().any(|t| t == "use:flake"));
}

#[test]
fn a_deer_is_fat_in_autumn_and_lean_in_spring() {
    let w = world();
    let hind = Stack::one(&kind(&w, "hearth:red_deer_carcass"));
    let fat = |season: Season| -> u16 {
        (0..4)
            .map(|k| count(&butcher(&w, &hind, season, 10 + k), "hearth:cut/animal_fat"))
            .sum()
    };
    let (autumn, spring) = (fat(Season::Autumn), fat(Season::Spring));
    assert!(
        autumn > spring * 2,
        "{autumn} cuts of fat in autumn, {spring} in spring"
    );
    // A stag's antlers while he carries them; none once cast, none from a hind.
    let stag = Stack::one(&kind(&w, "hearth:red_deer_carcass_male"));
    assert!(
        count(
            &butcher(&w, &stag, Season::Autumn, 1),
            "hearth:piece/antler"
        ) > 0
    );
    assert_eq!(
        count(
            &butcher(&w, &stag, Season::Spring, 1),
            "hearth:piece/antler"
        ),
        0
    );
    assert_eq!(
        count(
            &butcher(&w, &hind, Season::Autumn, 1),
            "hearth:piece/antler"
        ),
        0
    );
}

#[test]
fn a_kill_eaten_from_gives_what_is_left_and_spoiled_meat_stays_spoiled() {
    let w = world();
    let whole = Stack::one(&kind(&w, "hearth:roe_deer_carcass"));
    let mut eaten = whole.clone();
    eaten.condition = 0.5;
    eaten.decay = 0.6;
    let (a, b) = (
        butcher(&w, &whole, Season::Summer, 7),
        butcher(&w, &eaten, Season::Summer, 7),
    );
    let (m0, m1) = (count(&a, "hearth:cut/meat"), count(&b, "hearth:cut/meat"));
    assert!(
        (m1 as f32 / m0 as f32 - 0.5).abs() < 0.1,
        "{m1} cuts against {m0}"
    );
    for s in b.made.iter().filter(|s| s.id == "hearth:cut/meat") {
        assert!(s.decay >= 0.6, "as far gone as the carcass: {}", s.decay);
    }
    assert!(a.made.iter().all(|s| s.decay == 0.0));
}

#[test]
fn skill_and_fine_tools_set_the_pace() {
    let w = world();
    let pole = Stack::one(&kind(&w, "hearth:pole/hazel_wood"));
    let whittle = w.crafts.index_of("hearth:whittle_spear").expect("whittle");
    let hours = |tool: &str, skill: f32| {
        let mut carry = holding(&w, tool, None);
        carry
            .hold(&w.items, pole.clone(), Hand::Left, 70.0)
            .expect("pole");
        let bench = Bench::new(&w.content, &w.items, &carry, [], None, around());
        plan(&w.crafts, whittle, &bench, skill).expect("plan").hours
    };
    let novice = hours("hearth:flake/flint", 0.0);
    let skilled = hours("hearth:flake/flint", 1.0);
    let coarse = hours("hearth:flake/quartzite", 1.0);
    assert!(
        novice > 1.9 * skilled * 0.99,
        "a novice takes twice as long: {novice} vs {skilled}"
    );
    assert!(
        coarse > skilled,
        "a coarse edge is slower: {coarse} vs {skilled}"
    );
    assert!(
        (2.0..3.5).contains(&skilled),
        "about three hours for the skilled: {skilled}"
    );
}

#[test]
fn moccasins_are_sewn_of_the_hide_in_hand() {
    let w = world();
    let mut carry = holding(&w, "hearth:needle/bone", Some("hearth:blade/flint"));
    let mut bundle = Stack::one(&kind(&w, "hearth:bundle/scraped_hide"));
    let spec = bundle
        .kind(&w.items)
        .and_then(|k| k.container)
        .expect("bundle");
    let inside = bundle.contents_mut(&w.items).expect("opens");
    inside
        .put_anywhere(&w.items, &spec, Stack::one("hearth:sheet/scraped_hide"))
        .expect("hide");
    inside
        .put_anywhere(&w.items, &spec, Stack::of("hearth:cord/sinew", 3))
        .expect("thread");
    carry.back = Some(bundle);
    let bench = Bench::new(&w.content, &w.items, &carry, [], None, around());
    let sew = w.crafts.index_of("hearth:sew_moccasins").expect("sew");
    let p = plan(&w.crafts, sew, &bench, 0.2).expect("plan");
    let mut rng = Rng::new(11);
    let o = perform(&w.crafts, &p, &bench, 0.2, &mut rng);
    assert!(o.done);
    assert_eq!(o.made.len(), 1);
    assert_eq!(o.made[0].id, "hearth:moccasins/scraped_hide");
    assert!(
        w.items
            .get(&o.made[0].id)
            .and_then(|k| k.wear.as_ref())
            .is_some(),
        "worn"
    );
}

#[test]
fn every_implemented_process_has_its_content() {
    let w = world();
    for r in &w.crafts.recipes {
        if let Some(k) = &r.def.knowledge {
            assert!(
                w.graph.node(k.as_str()).is_some_and(|n| n.implemented),
                "{} needs `{k}`, which is not implemented",
                r.def.id
            );
        }
    }
    // Stations stand as blocks.
    for st in w.content.workstations.iter() {
        assert!(st.block.is_some(), "{} has no block", st.id);
    }
}

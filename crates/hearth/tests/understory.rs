//! The understory in a running world (V2-6): plants of many species about a temperate spawn,
//! berries that can be picked, a poisonous look-alike that sickens whoever eats it, and the
//! knowledge of telling look-alikes apart, learned the hard way.

mod common;

use common::{World, temp};
use hearth_items::{Hand, Path, Root};
use hearth_protocol::ToServer;

#[test]
fn the_woods_hold_many_plants_and_a_poisonous_berry_is_learned_the_hard_way() {
    let dir = temp("understory");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Discovery, 7);
    // Plants of several species of the understory grow about the spawn.
    let species: std::collections::BTreeSet<String> = w
        .find(44, |n, _| {
            [
                "raspberry",
                "bilberry",
                "lingonberry",
                "dog_rose",
                "juniper",
                "heather",
                "wild_garlic",
                "wood_sorrel",
                "yarrow",
                "ribwort_plantain",
                "dandelion",
                "wild_carrot",
                "hemlock",
                "burdock",
                "wild_strawberry",
                "foxglove",
                "deadly_nightshade",
                "bracken",
                "chanterelle",
                "porcini",
                "field_mushroom",
                "death_cap",
                "fly_agaric",
                "nettle",
                "bramble",
            ]
            .contains(&n)
        })
        .into_iter()
        .filter_map(|p| w.block(p))
        .collect();
    println!("understory about the spawn: {species:?}");
    assert!(species.len() >= 4, "{species:?}");

    // Black berries eaten: they sicken (deadly nightshade), and the eater learns, too late, to
    // tell dark berries apart.
    for _ in 0..3 {
        w.give("hearth:handful/nightshade_berry", 1);
        w.server
            .send(ToServer::Eat(Path::at(Root::Hand(Hand::Right))));
        w.run(4);
    }
    assert!(
        w.knowledge.knows("hearth:telling_dark_berry"),
        "learned: {:?}",
        w.learned
    );
    // The poison takes hold within hours.
    w.wait_hours(6.0);
    let ill = w
        .body
        .as_ref()
        .is_some_and(|b| b.illnesses.iter().any(|i| i.ends_with("poisoning")));
    assert!(ill, "{:?}", w.body.as_ref().map(|b| &b.illnesses));
    drop(w);
    let _ = std::fs::remove_dir_all(&dir);
}

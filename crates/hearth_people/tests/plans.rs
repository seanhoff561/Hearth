//! H2 acceptance (V2.1 §6.1): a person plans many steps ahead on the player's process engine. A
//! woman of *Homo sapiens* who knows how — to test a stone and knap a point from its core, strike
//! a flake, twist cord, whittle a spear and haft a point to it — but carries nothing but a back
//! basket, sets out to have a stone-tipped spear: she gathers flint from a scatter and finds a
//! hammerstone, pulls a dead pine pole, strips nettle fibre and scrapes resin from a pine, makes
//! each part and hafts it, replanning when a knapping fails, and ends with the spear in hand.

mod common;

use common::*;
use glam::DVec3;
use hearth_content::schema::process::Match;
use hearth_craft::engine::Aimed;
use hearth_items::Stack;
use hearth_people::{People, Species, SpeciesSet, Want};

/// The species set with *Homo sapiens* in it (a profile not yet lived in the world, H3).
fn with_sapiens() -> (SpeciesSet, usize) {
    let b = base();
    let mut set = b.species.clone();
    let human = hearth_body::BodyConfig::with_rates(
        &b.content,
        hearth_body::Rates::authentic(),
        hearth_content::TimeScales::defaults(&b.content.time),
    );
    let profile = b
        .content
        .species
        .get("hearth:homo_sapiens")
        .expect("the human profile");
    set.list
        .push(Species::of_profile(profile, &b.graph, &human).with_routine(&b.content));
    let k = set.list.len() - 1;
    (set, k)
}

fn block(name: &str, material: Option<&str>) -> Aimed {
    Aimed::Block {
        name: format!("hearth:{name}"),
        material: material.map(|m| format!("hearth:{m}")),
        ground: true,
        room: false,
    }
}

#[test]
fn a_woman_who_knows_how_makes_a_hafted_spear_from_scratch() {
    let b = base();
    let (species, k) = with_sapiens();
    let mut world = Savanna::new();
    world.hour = 6.5;
    // What the land offers about her: a scatter of flint, a dead pine and a live one, nettles.
    world.targets = vec![
        (
            DVec3::new(30.0, GROUND, -6.0),
            block("flint_cobbles", Some("flint")),
        ),
        (
            DVec3::new(-14.0, GROUND, 12.0),
            block("pine_log", Some("pine_wood")),
        ),
        (
            DVec3::new(-20.0, GROUND, 4.0),
            block("pine_log", Some("pine_wood")),
        ),
        (DVec3::new(8.0, GROUND, 24.0), block("nettle", None)),
        (DVec3::new(10.0, GROUND, 25.0), block("nettle", None)),
    ];
    let mut p = People::new(9);
    let now = world.now();
    p.spawn_band(
        &species,
        &b.graph,
        &b.items,
        &mut world,
        k,
        [1, 0, 0, 0],
        DVec3::new(2.0, GROUND, 2.0),
        now,
    );
    let me = p.persons[0].id;
    {
        let q = &mut p.persons[0];
        // Knowing how, practised.
        for node in [
            "stone_as_hammer",
            "prepared_core",
            "sharp_flake",
            "wooden_spear",
            "cordage_basic",
            "stone_tipped_spear",
        ] {
            q.knowledge.known.insert(
                format!("hearth:{node}"),
                hearth_craft::knowledge::Learned {
                    tick: 0,
                    route: None,
                },
            );
        }
        for skill in ["knapping", "woodworking", "fibrework"] {
            q.knowledge.practice(skill, 200.0, 0);
        }
        // Fed and watered before she sets out.
        let cfg = species.list[k].body(true);
        let meal = hearth_body::Food {
            kcal: 1200.0,
            protein_g: 40.0,
            fat_g: 40.0,
            carb_g: 160.0,
            water_l: 0.6,
            volume_l: 1.0,
            fresh_days: 1.0,
        };
        let _ = q.body.eat(cfg, &meal);
        q.body.drink(cfg, 1.0, 0.0, 0.0);
        // A back basket, and nothing else.
        q.possessions.carry = hearth_items::Carry::default();
        q.possessions.carry.back = Some(Stack::one(&item("back_basket/reed")));
        q.mind.goal = Some(Want::Item {
            what: Match::Form {
                form: hearth_content::IdRef("hearth:stone_tipped_spear".into()),
                materials: None,
            },
            count: 1,
        });
    }
    // Days of the world at most; the steps as she does them.
    let has_spear = |p: &People| {
        p.get(me).is_some_and(|q| {
            hearth_people::plan::has(
                &b.items,
                &b.content,
                q,
                &Want::Item {
                    what: Match::Form {
                        form: hearth_content::IdRef("hearth:stone_tipped_spear".into()),
                        materials: None,
                    },
                    count: 1,
                },
            )
        })
    };
    let mut steps = 0u64;
    while !has_spear(&p) && steps < 40_000 {
        let now = world.now();
        world.advance();
        p.step(
            &species,
            &b.crafts,
            &b.content,
            &b.items,
            &mut world,
            &[],
            now,
            DT,
        );
        steps += 1;
        if steps.is_multiple_of(2000) {
            let q = p.get(me).expect("her");
            println!(
                "{:.1} h: {:?}, plan {:?}",
                steps as f32 * DT / 3600.0 * 24.0 * 3600.0 / DAY_S,
                q.mind.doing,
                q.mind.plan.as_ref().map(|pl| (pl.next, pl.steps.len()))
            );
        }
    }
    println!("done in {steps} steps");
    assert!(has_spear(&p), "no spear after {steps} steps");
    // She sees she has it, and lets the goal go.
    for _ in 0..8 {
        let now = world.now();
        world.advance();
        p.step(
            &species,
            &b.crafts,
            &b.content,
            &b.items,
            &mut world,
            &[],
            now,
            DT,
        );
    }
    let q = p.get(me).expect("her");
    assert!(q.mind.goal.is_none(), "the goal let go once had");
    assert!(q.mind.plan.is_none());
}

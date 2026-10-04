//! H5 (V2.1 §9): every band carries a culture drawn from its people's generator — its values, its
//! social organisation, the division of its work, its rites, its taboos and its ways — the same for
//! the same world; and a band that splits off takes a daughter of it.

mod common;

use common::*;
use glam::DVec3;
use hearth_people::{Now, People};

#[test]
fn bands_carry_cultures_and_those_that_split_off_take_daughters() {
    let b = base();
    let species = &b.species;
    let k = species.index_of("homo_sapiens").expect("our species");
    let g = species.list[k]
        .culture
        .clone()
        .expect("our people have cultures");
    let draw = |seed: u64| {
        let mut world = Savanna::new();
        let mut p = People::new(seed);
        let now = world.now();
        for x in [0.0, 20_000.0] {
            p.spawn_band(
                species,
                &b.graph,
                &b.items,
                &mut world,
                k,
                [4, 4, 2, 0],
                DVec3::new(x, GROUND, 0.0),
                now,
            );
        }
        p
    };
    let p = draw(61);
    for band in &p.bands {
        let c = &band.culture;
        println!(
            "band {}: {:?} {:?} {:?} {:?}, polygyny {:.2}, {:?}, taboos {:?}",
            band.id, c.values, c.residence, c.descent, c.burial, c.polygyny, c.greeting, c.taboos
        );
        assert!(c.drawn() && c.id == band.id, "drawn, the band's own");
        for (v, span) in [
            (c.values.hierarchy, g.hierarchy),
            (c.values.wide, g.wide),
            (c.values.honour, g.honour),
            (c.values.tight, g.tight),
        ] {
            assert!((span.0..=span.1).contains(&v), "{v} within {span:?}");
        }
        assert_eq!(c.labour.len(), g.labour.len());
        assert!(c.women_share("gathering") > c.women_share("hunting"));
        assert!(c.ways.is_some(), "its own ways with strangers");
    }
    assert_ne!(
        p.bands[0].culture.values, p.bands[1].culture.values,
        "two bands, two cultures"
    );
    // The same world draws the same cultures.
    let again = draw(61);
    assert_eq!(p.bands[0].culture, again.bands[0].culture);
    // A band grown past what holds together splits, and the band gone off takes a daughter of
    // its culture.
    let mut world = Savanna::new();
    let mut q = People::new(62);
    let now = world.now();
    q.spawn_band(
        species,
        &b.graph,
        &b.items,
        &mut world,
        k,
        [26, 24, 14, 0],
        DVec3::new(0.0, GROUND, 0.0),
        now,
    );
    let parent = q.bands[0].culture.clone();
    q.live_course(species, &b.items, &world, now);
    q.live_course(
        species,
        &b.items,
        &world,
        Now {
            day: now.day + 2.0,
            ..now
        },
    );
    assert!(q.bands.len() > 1, "the band split");
    let d = &q.bands[1].culture;
    assert_eq!(d.parent, Some(parent.id), "its lineage kept");
    assert_eq!(d.id, q.bands[1].id);
    assert_eq!(d.values, parent.values, "the same ways, for now");
}

#[test]
fn a_culture_shapes_who_works_who_moves_and_what_passes_at_death() {
    use hearth_content::schema::culture::{Burial, Residence};
    use hearth_items::Stack;
    use hearth_people::person::Event;
    use hearth_people::{Doing, Intent, Needs, Offer, Psyche, Situation, choose};
    let b = base();
    let species = &b.species;
    let k = species.index_of("homo_sapiens").expect("our species");
    // Work: a work that is one's own by one's culture's division of labour is taken up; the
    // other sex's, left for rest.
    let sp = &species.list[k];
    let offered = |labour: f32| Situation {
        pos: DVec3::ZERO,
        // Midday, when the day pulls toward rest and not work.
        hour: 12.0,
        flight_m: 60.0,
        grown: true,
        offers: vec![Offer {
            recipe: 3,
            at: DVec3::new(1.0, 0.0, 0.0),
            feeds: false,
            labour,
        }],
        ..Situation::default()
    };
    let works = |labour: f32| {
        matches!(
            choose(
                sp,
                &Needs::default(),
                &offered(labour),
                &Psyche::default(),
                0.5
            ),
            Doing::Working { .. }
                | Doing::Going {
                    then: Intent::Work(_),
                    ..
                }
        )
    };
    assert!(works(1.7), "its own work, done");
    assert!(!works(0.3), "the other's, left");
    // Residence: a woman and a man of two bands pair, and live where her people's culture has
    // a new pair live.
    for (residence, she_goes) in [
        (Residence::Patrilocal, true),
        (Residence::Matrilocal, false),
    ] {
        let mut world = Savanna::new();
        let mut p = People::new(63);
        let now = world.now();
        let mut band = |n: [u16; 4], x: f64| {
            p.spawn_band(
                species,
                &b.graph,
                &b.items,
                &mut world,
                k,
                n,
                DVec3::new(x, GROUND, 0.0),
                now,
            )
        };
        let (hers, his) = (band([1, 0, 0, 0], 0.0), band([0, 1, 0, 0], 1_000.0));
        for q in p.persons.iter_mut() {
            q.life.born = now.day - 22.0 * YEAR_DAYS;
        }
        for x in p.bands.iter_mut().filter(|x| x.id == hers) {
            x.culture.residence = residence;
        }
        let her = p.members(hers).next().expect("her").id;
        let him = p.members(his).next().expect("him").id;
        p.live_course(species, &b.items, &world, now);
        p.live_course(
            species,
            &b.items,
            &world,
            Now {
                day: now.day + YEAR_DAYS / 4.0,
                ..now
            },
        );
        let (w, m) = (p.get(her).expect("her"), p.get(him).expect("him"));
        assert_eq!(w.social.bond, Some(him), "paired");
        assert_eq!(
            w.social.band == his,
            she_goes,
            "{residence:?}: she goes to his"
        );
        assert_eq!(
            m.social.band == hers,
            !she_goes,
            "{residence:?}: he comes to hers"
        );
    }
    // At death: buried with what they carried, or what they carried passes to their heir.
    for (burial, passes) in [(Burial::BuriedWithGoods, false), (Burial::Buried, true)] {
        let mut world = Savanna::new();
        let mut p = People::new(64);
        let now = world.now();
        p.spawn_band(
            species,
            &b.graph,
            &b.items,
            &mut world,
            k,
            [3, 3, 2, 0],
            DVec3::new(2.0, GROUND, 2.0),
            now,
        );
        for x in p.bands.iter_mut() {
            x.culture.burial = burial;
        }
        let mother = p
            .full()
            .find(|q| p.full().any(|c| c.life.mother == Some(q.id)))
            .map(|q| q.id)
            .expect("a mother");
        if let Some(q) = p.persons.iter_mut().find(|q| q.id == mother) {
            q.possessions.carry.right = Some(Stack::of(&item("cobble/basalt"), 1));
            q.body.dead = Some(hearth_body::Death::Injury("a fall".into()));
        }
        step(&mut p, &mut world, &[]);
        let dead = p.get(mother).expect("her");
        assert!(
            dead.life
                .events
                .iter()
                .any(|e| e.event == Event::LaidToRest { how: burial }),
            "laid to rest as her people do"
        );
        let inherited = p.persons.iter().any(|q| {
            q.life
                .events
                .iter()
                .any(|e| e.event == Event::Inherited { from: mother })
        });
        assert_eq!(inherited, passes, "{burial:?}");
    }
}

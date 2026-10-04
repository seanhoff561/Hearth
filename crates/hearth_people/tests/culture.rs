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

#[test]
fn cultures_drift_apart_alone_and_stay_alike_in_contact() {
    use hearth_math::hash::Rng;
    let b = base();
    let species = &b.species;
    let k = species.index_of("homo_sapiens").expect("our species");
    let sp = &species.list[k];
    let g = sp.culture.clone().expect("our people have cultures");
    let mut root = hearth_people::Culture::default();
    root.draw(1, &g, sp.ways.as_ref(), &mut Rng::new(7), 0.0);
    // Two daughters that never meet, and two that meet every year, for five hundred years.
    let (mut a, mut c) = (root.daughter(2, 0.0), root.daughter(3, 0.0));
    let (mut d, mut e) = (root.daughter(4, 0.0), root.daughter(5, 0.0));
    let mut streams: Vec<Rng> = (11..16).map(Rng::new).collect();
    for _ in 0..500 {
        a.drift(&g, &mut streams[0]);
        c.drift(&g, &mut streams[1]);
        d.drift(&g, &mut streams[2]);
        e.drift(&g, &mut streams[3]);
        d.meet(&mut e, &g, 1.0, &mut streams[4]);
    }
    let (apart, together) = (a.value_gap(&c), d.value_gap(&e));
    println!(
        "after five hundred years: apart {apart:.2} ({} customs), in contact {together:.2} ({} customs)",
        a.customs_apart(&c),
        d.customs_apart(&e)
    );
    assert!(apart > 0.3, "those apart drift apart");
    assert!(apart > 2.0 * together, "those in contact stay alike");
}

#[test]
fn the_young_take_in_their_culture_sooner_than_the_grown() {
    use hearth_people::psyche::Value;
    let b = base();
    let species = &b.species;
    let k = species.index_of("homo_sapiens").expect("our species");
    let mut world = Savanna::new();
    let mut p = People::new(65);
    let now = world.now();
    let band = p.spawn_band(
        species,
        &b.graph,
        &b.items,
        &mut world,
        k,
        [3, 3, 6, 0],
        DVec3::new(0.0, GROUND, 0.0),
        now,
    );
    // A people that holds to honour above all.
    for x in p.bands.iter_mut() {
        x.culture.values.honour = 1.0;
    }
    let honour = |p: &People, id: u64| p.get(id).map_or(0.0, |q| q.psyche.value(Value::Honor));
    let young: Vec<u64> = p
        .members(band)
        .filter(|q| q.age(&now) < 14.0)
        .map(|q| q.id)
        .collect();
    let grown: Vec<u64> = p
        .members(band)
        .filter(|q| q.age(&now) >= 20.0)
        .map(|q| q.id)
        .collect();
    let before: Vec<f32> = young
        .iter()
        .chain(&grown)
        .map(|&id| honour(&p, id))
        .collect();
    p.live_course(species, &b.items, &world, now);
    p.live_course(
        species,
        &b.items,
        &world,
        Now {
            day: now.day + YEAR_DAYS,
            ..now
        },
    );
    let rise = |ids: &[u64], from: &[f32]| {
        ids.iter()
            .zip(from)
            .map(|(&id, b)| honour(&p, id) - b)
            .sum::<f32>()
            / ids.len().max(1) as f32
    };
    let (y, g) = (
        rise(&young, &before[..young.len()]),
        rise(&grown, &before[young.len()..]),
    );
    println!(
        "a year on: the young's honour up {y:.3} ({} of them), the grown's {g:.3}",
        young.len()
    );
    assert!(!young.is_empty() && y > 0.05, "the young take it in");
    assert!(y > g, "sooner than the grown");
}

#[test]
fn languages_are_drawn_people_named_and_daughters_stay_related() {
    use hearth_math::hash::Rng;
    use hearth_people::Language;
    let b = base();
    let species = &b.species;
    let k = species.index_of("homo_sapiens").expect("our species");
    let d = species.list[k].language.clone().expect("our people speak");
    let root = Language::draw(1, &d, &mut Rng::new(21));
    assert_eq!(
        root.words.len(),
        d.meanings.len(),
        "a word for every meaning"
    );
    let distinct: std::collections::BTreeSet<&Vec<u8>> =
        root.words.iter().map(|(_, w)| w).collect();
    assert_eq!(distinct.len(), root.words.len(), "no two alike");
    let say = |l: &Language, m: &str| l.say(&d, m).unwrap_or_default();
    println!(
        "water {}, fire {}, mother {}, hello {}, go {}; names {} and {}",
        say(&root, "water"),
        say(&root, "fire"),
        say(&root, "mother"),
        say(&root, "hello"),
        say(&root, "go"),
        root.name(&d, &mut Rng::new(5)),
        root.name(&d, &mut Rng::new(6))
    );
    // Two daughters five hundred years apart, and the tongue of another people altogether.
    let (mut a, mut c) = (root.daughter(2), root.daughter(3));
    let (mut ra, mut rc) = (Rng::new(31), Rng::new(32));
    for _ in 0..500 {
        a.drift(&d, &mut ra);
        c.drift(&d, &mut rc);
    }
    let other = Language::draw(4, &d, &mut Rng::new(41));
    let (related, unrelated) = (a.kinship(&c), a.kinship(&other));
    println!(
        "five hundred years on: water {} / {}, fire {} / {}; {} and {} sound changes; \
         cognates {related:.2}, with a stranger's tongue {unrelated:.2}",
        say(&a, "water"),
        say(&c, "water"),
        say(&a, "fire"),
        say(&c, "fire"),
        a.changed.len(),
        c.changed.len()
    );
    assert!(a.words != c.words, "they changed apart");
    assert!(related > 0.5, "related: many cognates");
    assert!(
        unrelated < related - 0.25,
        "more alike than a stranger's tongue"
    );
    // People are named in their language.
    let mut world = Savanna::new();
    let mut p = People::new(66);
    let now = world.now();
    let band = p.spawn_band(
        species,
        &b.graph,
        &b.items,
        &mut world,
        k,
        [3, 3, 2, 0],
        DVec3::new(0.0, GROUND, 0.0),
        now,
    );
    let names: Vec<String> = p.members(band).map(|q| q.name.clone()).collect();
    println!("{names:?}");
    assert!(names.iter().all(|n| !n.is_empty()), "every one named");
}

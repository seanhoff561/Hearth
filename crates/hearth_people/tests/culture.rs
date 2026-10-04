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

#[test]
fn speech_is_made_out_as_far_as_the_language_is_known() {
    use hearth_math::hash::Rng;
    use hearth_people::speech::{Act, Gesture, Said, Tongue, words};
    let b = base();
    let species = &b.species;
    let k = species.index_of("homo_sapiens").expect("our species");
    let d = species.list[k].language.clone().expect("our people speak");
    let mut world = Savanna::new();
    let mut p = People::new(67);
    let now = world.now();
    let mut found = |x: f64| {
        p.spawn_band(
            species,
            &b.graph,
            &b.items,
            &mut world,
            k,
            [2, 2, 0, 0],
            DVec3::new(x, GROUND, 0.0),
            now,
        )
    };
    let (ours, theirs, kin) = (found(0.0), found(50_000.0), found(100_000.0));
    // The third band speaks a daughter of the first's tongue, three hundred years on.
    let mut daughter = p.bands[0]
        .culture
        .language
        .as_ref()
        .expect("a language")
        .daughter(kin);
    let mut rng = Rng::new(9);
    for _ in 0..300 {
        daughter.drift(&d, &mut rng);
    }
    p.bands[2].culture.language = Some(daughter);
    let listener = p.members(ours).next().expect("a listener").id;
    let mother = p.bands[0].culture.language.as_ref().expect("ours").id;
    for q in p.persons.iter_mut().filter(|q| q.id == listener) {
        q.tongues = vec![Tongue {
            language: mother,
            native: true,
            words: Vec::new(),
        }];
    }
    let greeting = |speaker: u64| Said {
        speaker,
        to: Some(listener),
        act: Act::Greet,
        words: words(&["hello", "friend", "come", "eat"]),
        gesture: Some(Gesture::Beckon),
        at: DVec3::ZERO,
        day: now.day,
    };
    let made_out = |p: &People, speaker: u64| {
        let me = p.get(listener).expect("the listener");
        p.heard(&d, me, &greeting(speaker)).expect("heard")
    };
    // In its mother tongue, all of it; in a stranger's, nothing; in a related one, some.
    let own = made_out(&p, p.members(ours).nth(1).expect("one of ours").id);
    let stranger = p.members(theirs).next().expect("one of theirs").id;
    let strange = made_out(&p, stranger);
    let related = made_out(&p, p.members(kin).next().expect("one of the kin").id);
    println!(
        "own: {} / {}; stranger's: {} / {}; related: {} / {}",
        own.spoken, own.sense, strange.spoken, strange.sense, related.spoken, related.sense
    );
    assert_eq!(own.understood, 1.0);
    assert!(strange.understood < 0.3, "a stranger's tongue is strange");
    assert!(
        related.understood > strange.understood,
        "a related tongue partly made out"
    );
    // Spoken to again and again, with a gesture making it plain, the stranger's words are
    // learned.
    for _ in 0..8 {
        p.learn_from(listener, &greeting(stranger));
    }
    let learned = made_out(&p, stranger);
    println!("after hearing it: {} / {}", learned.spoken, learned.sense);
    assert!(learned.understood > 0.9, "learned");
}

#[test]
fn h5_accepted_cultures_part_with_related_tongues_and_a_newcomer_learns() {
    use hearth_people::speech::{HEARD_M, Tongue};
    let b = base();
    let species = &b.species;
    let k = species.index_of("homo_sapiens").expect("our species");
    let d = species.list[k].language.clone().expect("our people speak");
    // One people, grown past what holds together: it splits, and the band gone off goes far.
    let mut world = Savanna::new();
    let mut p = People::new(68);
    let now = world.now();
    p.spawn_band(
        species,
        &b.graph,
        &b.items,
        &mut world,
        k,
        [26, 24, 14, 0],
        DVec3::new(0.0, GROUND, 0.0),
        now,
    );
    // And another people altogether, far off.
    p.spawn_band(
        species,
        &b.graph,
        &b.items,
        &mut world,
        k,
        [3, 3, 0, 0],
        DVec3::new(300_000.0, GROUND, 0.0),
        now,
    );
    p.live_course(species, &b.items, &world, now);
    p.live_course(
        species,
        &b.items,
        &world,
        Now {
            day: now.day + 2.0,
            ..now
        },
    );
    let a = p.bands[0].id;
    let c = p
        .bands
        .iter()
        .find(|x| x.culture.parent == Some(a))
        .map(|x| x.id)
        .expect("a daughter culture");
    let other = p.bands[1].id;
    for x in p.bands.iter_mut().filter(|x| x.id == c) {
        x.home += glam::DVec2::new(0.0, 150_000.0);
    }
    // Two hundred years of their lives.
    p.live_course(
        species,
        &b.items,
        &world,
        Now {
            day: now.day + 2.0 + 200.0 * YEAR_DAYS,
            ..now
        },
    );
    let band = |id: u64| p.bands.iter().find(|x| x.id == id).expect("the band");
    let (ca, cc, co) = (&band(a).culture, &band(c).culture, &band(other).culture);
    let (la, lc, lo) = (
        ca.language.as_ref().expect("a tongue"),
        cc.language.as_ref().expect("a tongue"),
        co.language.as_ref().expect("a tongue"),
    );
    println!(
        "two hundred years apart: values {:.2} apart, {} customs; cognates {:.2}, with a \
         stranger people's {:.2}; water {} / {} / {}",
        ca.value_gap(cc),
        ca.customs_apart(cc),
        la.kinship(lc),
        la.kinship(lo),
        la.say(&d, "water").unwrap_or_default(),
        lc.say(&d, "water").unwrap_or_default(),
        lo.say(&d, "water").unwrap_or_default()
    );
    assert!(
        ca.value_gap(cc) > 0.0 && la.words != lc.words,
        "they diverged"
    );
    assert!(
        la.kinship(lc) > la.kinship(lo) + 0.25,
        "their tongues are related, and not to a stranger people's"
    );
    // A newcomer from another people learns its hosts' tongue over play: what it makes out of
    // what is said near it, early on and at the end. (One of them has been seen stealing, so
    // there is talk.)
    use hearth_people::memory::Who;
    use hearth_people::repute::{Deed, Seen};
    let mut world = Savanna::new();
    let mut q = People::new(69);
    let now = world.now();
    let mut found = |n: [u16; 4], x: f64| {
        q.spawn_band(
            species,
            &b.graph,
            &b.items,
            &mut world,
            k,
            n,
            DVec3::new(x, GROUND, 2.0),
            now,
        )
    };
    let (hosts, home) = (found([3, 3, 2, 0], 2.0), found([1, 1, 0, 0], 1_000.0));
    let theirs = q.bands[0]
        .culture
        .language
        .as_ref()
        .map(|l| l.id)
        .expect("their tongue");
    let mine = q.bands[1]
        .culture
        .language
        .as_ref()
        .map(|l| l.id)
        .expect("its tongue");
    let newcomer = q.members(home).next().map(|x| x.id).expect("a newcomer");
    let hosts_grown: Vec<u64> = q
        .members(hosts)
        .filter(|x| x.age(&now) >= 18.0)
        .map(|x| x.id)
        .collect();
    let there = q.get(hosts_grown[0]).expect("a host").place.pos;
    // A player's person stays where it is put, among them.
    for x in q.persons.iter_mut().filter(|x| x.id == newcomer) {
        x.player = Some(1);
        x.place.pos = there + DVec3::new(2.0, 0.0, 2.0);
        x.tongues = vec![Tongue {
            language: mine,
            native: true,
            words: Vec::new(),
        }];
    }
    let thief = hosts_grown[0];
    q.deed(
        Seen {
            who: Who::Person(thief),
            deed: Deed::Took {
                from: Who::Person(hosts_grown[1]),
            },
            at: there,
        },
        &species.norms,
        1.0,
        now.day,
    );
    let (mut early, mut late) = ((0.0f32, 0.0f32), (0.0f32, 0.0f32));
    let minutes = 30;
    for minute in 0..minutes {
        for _ in 0..(60.0 / DT) as u64 {
            step(&mut q, &mut world, &[]);
            let me_at = q.get(newcomer).expect("the newcomer").place.pos;
            let near: Vec<hearth_people::Said> = q
                .said
                .iter()
                .filter(|s| s.speaker != newcomer && (s.at - me_at).length() < HEARD_M)
                .filter(|s| q.tongue_of(s.speaker).is_some_and(|l| l.id == theirs))
                .cloned()
                .collect();
            for s in &near {
                let me = q.get(newcomer).expect("the newcomer");
                if let Some(h) = q.heard(&d, me, s) {
                    if minute < 5 {
                        early = (early.0 + h.understood, early.1 + 1.0);
                    } else if minute >= minutes - 5 {
                        late = (late.0 + h.understood, late.1 + 1.0);
                    }
                }
                q.learn_from(newcomer, s);
            }
            q.said.retain(|s| !near.contains(s));
        }
    }
    let (e, l) = (early.0 / early.1.max(1.0), late.0 / late.1.max(1.0));
    println!(
        "the newcomer made out {e:.2} of what was said at first ({} acts), {l:.2} at the end ({} acts)",
        early.1, late.1
    );
    assert!(late.1 > 0.0, "it heard them talk");
    assert!(l > e + 0.2, "it learned their tongue");
}

#[test]
fn a_hard_skill_is_lost_by_a_small_band_alone_and_kept_by_a_large_one() {
    use hearth_craft::knowledge::Learned;
    let b = base();
    let mut species = b.species.clone();
    let k = species.index_of("homo_sapiens").expect("our species");
    // The deepest technique whose groundwork our people know, and is not yet theirs.
    let known = species.list[k].knowledge.clone();
    let (hard, depth) = species
        .lore
        .nodes
        .iter()
        .filter(|f| {
            f.implemented && !known.contains(&f.id) && f.requires.iter().all(|r| known.contains(r))
        })
        .max_by_key(|f| f.depth)
        .map(|f| (f.id.clone(), f.depth))
        .expect("a technique to learn");
    // As hard to pass on as a fine skill: from one knower, once in five hundred years.
    if let Some(t) = species.list[k].learning.as_mut() {
        t.depth_factor = (t.learn_year / 0.002).ln() / depth.max(1) as f32;
    }
    let run = |bands: &[([u16; 4], f64, f64)], seed: u64| -> (usize, bool) {
        let mut world = Savanna::new();
        let mut p = People::new(seed);
        let now = world.now();
        for (n, x, z) in bands {
            p.spawn_band(
                &species,
                &b.graph,
                &b.items,
                &mut world,
                k,
                *n,
                DVec3::new(*x, GROUND, *z),
                now,
            );
        }
        // The first band's grown all know it.
        let first = p.bands[0].id;
        for q in p.persons.iter_mut() {
            if q.social.band == first && q.age(&now) >= 15.0 {
                q.knowledge.known.insert(
                    hard.clone(),
                    Learned {
                        tick: 0,
                        route: None,
                    },
                );
            }
        }
        p.bands[0].culture.knowledge.push(hard.clone());
        p.live_course(&species, &b.items, &world, now);
        p.live_course(
            &species,
            &b.items,
            &world,
            Now {
                day: now.day + 150.0 * YEAR_DAYS,
                ..now
            },
        );
        let knowers = p
            .persons
            .iter()
            .filter(|q| q.alive() && q.knowledge.knows(&hard))
            .count();
        let kept = p.bands.iter().any(|x| x.culture.knowledge.contains(&hard));
        (knowers, kept)
    };
    let (alone, alone_kept) = run(&[([3, 3, 2, 0], 0.0, 0.0)], 70);
    let (many, many_kept) = run(
        &[
            ([16, 16, 12, 0], 0.0, 0.0),
            ([8, 8, 6, 0], 8_000.0, 0.0),
            ([8, 8, 6, 0], 0.0, 8_000.0),
        ],
        71,
    );
    println!(
        "{hard} (depth {depth}) a hundred and fifty years on: {alone} knowers in the small band \
         alone, {many} among the large one and its neighbours"
    );
    assert!(alone == 0 && !alone_kept, "lost by the small band alone");
    assert!(many > 0 && many_kept, "kept by the large connected one");
}

#[test]
fn shown_how_the_player_learns_faster_than_by_watching() {
    use hearth_craft::KnowledgeState;
    let b = base();
    let species = &b.species;
    let k = species.index_of("homo_sapiens").expect("our species");
    let mut world = Savanna::new();
    let mut p = People::new(72);
    let now = world.now();
    let band = p.spawn_band(
        species,
        &b.graph,
        &b.items,
        &mut world,
        k,
        [3, 3, 0, 0],
        DVec3::new(2.0, GROUND, 2.0),
        now,
    );
    // A work of theirs resting on something, its knowledge one the player lacks.
    let techniques = p.bands[0].culture.techniques.clone();
    let (recipe, node) = techniques
        .iter()
        .filter_map(|t| b.crafts.index_of(t))
        .filter_map(|r| {
            let node = b.crafts.recipes[r].def.knowledge.as_ref()?.to_string();
            let depth = species.lore.get(&node)?.depth;
            (1..=3).contains(&depth).then_some((r, node))
        })
        .next()
        .expect("a work to be shown");
    let members: Vec<u64> = p.members(band).map(|q| q.id).collect();
    let (teacher, player) = (members[0], members[1]);
    let at = p.get(teacher).expect("the teacher").place.pos;
    for q in p.persons.iter_mut() {
        if q.id == teacher {
            q.mind.doing = hearth_people::Doing::Working { recipe };
        }
        if q.id == player {
            q.player = Some(1);
            q.place.pos = at + DVec3::new(0.0, 0.0, -2.0);
        }
    }
    let eye = at + DVec3::new(0.0, 1.5, -2.0);
    // Facing +z, toward the teacher.
    let yaw = 0.0;
    let (mut shown, mut watching) = (KnowledgeState::default(), KnowledgeState::default());
    // The player knows what it rests on, as one of the land would: only the work itself is new.
    let mut ground: Vec<String> = species
        .lore
        .get(&node)
        .map(|f| f.requires.clone())
        .unwrap_or_default();
    let mut i = 0;
    while i < ground.len() {
        let more = species
            .lore
            .get(&ground[i])
            .map(|f| f.requires.clone())
            .unwrap_or_default();
        for m in more {
            if !ground.contains(&m) {
                ground.push(m);
            }
        }
        i += 1;
    }
    for g in &ground {
        for k in [&mut shown, &mut watching] {
            k.known.insert(
                g.clone(),
                hearth_craft::knowledge::Learned {
                    tick: 0,
                    route: None,
                },
            );
        }
    }
    let mut learned_in = None;
    for minute in 0..30 {
        for _ in 0..60 {
            for (n, insight) in p.lessons_for(1, &b.crafts, species, eye, yaw, &now, 1.0) {
                shown.taught(&b.graph, &n, insight, now.tick);
            }
        }
        // Watching alone, the player takes in what it sees an hour apart at most: once here.
        if minute == 0 {
            for t in hearth_people::watched(&b.graph, &b.crafts, recipe) {
                watching.observe(
                    &b.graph,
                    &t,
                    now.tick,
                    hearth_craft::knowledge::Mode::Discovery,
                );
            }
        }
        if learned_in.is_none() && shown.knows(&node) {
            learned_in = Some(minute + 1);
        }
    }
    println!(
        "{node}: shown how, learned in {learned_in:?} minutes; by watching, {:.2} of the way",
        watching.insight.get(&node).copied().unwrap_or(0.0)
    );
    assert!(
        learned_in.is_some(),
        "shown how, learned within half an hour"
    );
    assert!(!watching.knows(&node), "not by watching alone");
}

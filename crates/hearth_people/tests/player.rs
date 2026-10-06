//! H9 (V2.1 §16): the player among people. A player's person asks to be shown how and is taught
//! faster than by watching; offers to teach and its pupil comes to know the technique; is known
//! by name only once told; is refused a pair while the other is not fond of it and taken when
//! they are, and has a child by them; a stranger asks to stay with a band and, trusted, is
//! taken in.

mod common;

use common::*;
use glam::DVec3;
use hearth_craft::KnowledgeState;
use hearth_people::player::Ask;
use hearth_people::{Now, People};

/// Two bands of our kind a few hundred metres apart, the player one of the first band's grown.
fn world() -> (Savanna, People, u64, u64, Now) {
    let b = base();
    let species = &b.species;
    let k = species.index_of("homo_sapiens").expect("our species");
    let mut world = Savanna::new();
    let mut p = People::new(91);
    let now = world.now();
    let band = p.spawn_band(
        species,
        &b.graph,
        &b.items,
        &mut world,
        k,
        [8, 5, 0, 0],
        DVec3::new(2.0, GROUND, 2.0),
        now,
    );
    p.spawn_band(
        species,
        &b.graph,
        &b.items,
        &mut world,
        k,
        [3, 3, 0, 0],
        DVec3::new(300.0, GROUND, 2.0),
        now,
    );
    // The player: the band's youngest grown man.
    let me = p
        .members(band)
        .filter(|q| !q.life.female)
        .max_by(|a, b| a.life.born.total_cmp(&b.life.born))
        .map(|q| q.id)
        .expect("a man");
    for q in p.persons.iter_mut().filter(|q| q.id == me) {
        q.player = Some(1);
        q.social.bond = None;
    }
    for q in p.persons.iter_mut() {
        if q.social.bond == Some(me) {
            q.social.bond = None;
        }
    }
    (world, p, band, me, now)
}

fn stand_by(p: &mut People, who: u64, by: u64) -> DVec3 {
    let at = p.get(by).expect("them").place.pos + DVec3::new(0.0, 0.0, -1.5);
    for q in p.persons.iter_mut().filter(|q| q.id == who) {
        q.place.pos = at;
    }
    at
}

#[test]
fn asked_to_show_how_a_band_mate_teaches_the_player() {
    let b = base();
    let (_w, mut p, band, me, now) = world();
    let teacher = p
        .members(band)
        .find(|q| q.id != me && q.knowledge.known.len() > 3)
        .map(|q| q.id)
        .expect("one who knows things");
    let at = stand_by(&mut p, me, teacher);
    let eye = at + DVec3::new(0.0, 1.5, 0.0);
    let mine = KnowledgeState::default();
    let a = p.player_asks(
        1,
        teacher,
        Ask::BeTaught(None),
        &|n| mine.knows(n),
        &b.species,
        &now,
    );
    assert!(a.yes, "{}", a.words);
    let node = a.about.clone().expect("a technique to be shown");
    // Kept near, the player is given insight toward it; walked away, none.
    let mut insight = 0.0;
    for _ in 0..60 {
        for (n, i) in p.lessons_for(1, &b.crafts, &b.species, eye, 0.0, &now, 1.0) {
            if n == node {
                insight += i;
            }
        }
    }
    assert!(insight > 0.05, "a minute beside the teacher gave {insight}");
    let far = eye + DVec3::new(30.0, 0.0, 0.0);
    let away: f32 = p
        .lessons_for(1, &b.crafts, &b.species, far, 0.0, &now, 1.0)
        .into_iter()
        .filter(|(n, _)| *n == node)
        .map(|(_, i)| i)
        .sum();
    assert_eq!(away, 0.0, "nothing taught from afar");
}

#[test]
fn a_stranger_will_not_teach_until_trusted_and_names_are_told() {
    let b = base();
    let (_w, mut p, band, me, now) = world();
    let stranger = p
        .persons
        .iter()
        .find(|q| q.social.band != band && q.alive())
        .map(|q| q.id)
        .expect("a stranger");
    assert!(
        !p.knows_name(me, stranger),
        "a stranger's name is not known"
    );
    let mine = KnowledgeState::default();
    let a = p.player_asks(
        1,
        stranger,
        Ask::BeTaught(None),
        &|n| mine.knows(n),
        &b.species,
        &now,
    );
    assert!(
        !a.yes,
        "a stranger teaches one it does not trust: {}",
        a.words
    );
    let a = p.player_asks(1, stranger, Ask::Join, &|n| mine.knows(n), &b.species, &now);
    assert!(!a.yes, "a band takes in one none of it knows: {}", a.words);
    let told = p.player_asks(
        1,
        stranger,
        Ask::Introduce,
        &|n| mine.knows(n),
        &b.species,
        &now,
    );
    assert!(told.yes);
    assert!(p.knows_name(me, stranger), "told, the name is known");
    let line = p.regard(1, stranger, &b.species, &now);
    let name = p.get(stranger).expect("them").name.clone();
    assert!(line[0].starts_with(&name), "{line:?}");
}

#[test]
fn the_player_teaches_and_its_pupil_learns() {
    let b = base();
    let (_w, mut p, band, me, now) = world();
    let pupil = p
        .members(band)
        .find(|q| q.id != me)
        .map(|q| q.id)
        .expect("a band-mate");
    // A technique the pupil lacks and the player knows, its groundwork known to the pupil.
    let theirs = p.get(pupil).expect("them").knowledge.clone();
    let node = b
        .species
        .lore
        .nodes
        .iter()
        .find(|f| {
            f.implemented && !theirs.knows(&f.id) && f.requires.iter().all(|r| theirs.knows(r))
        })
        .map(|f| f.id.clone())
        .expect("something to teach");
    let n = node.clone();
    stand_by(&mut p, pupil, me);
    let a = p.player_asks(
        1,
        pupil,
        Ask::Teach(Some(node.clone())),
        &move |x| x == n,
        &b.species,
        &now,
    );
    assert!(a.yes, "{}", a.words);
    let eye = p.get(me).expect("me").place.pos + DVec3::new(0.0, 1.5, 0.0);
    for _ in 0..(20 * 60) {
        p.lessons_for(1, &b.crafts, &b.species, eye, 0.0, &now, 1.0);
        if p.get(pupil).expect("them").knowledge.knows(&node) {
            break;
        }
    }
    assert!(
        p.get(pupil).expect("them").knowledge.knows(&node),
        "twenty minutes of being shown teaches it"
    );
}

#[test]
fn a_pair_takes_two_willing_and_a_child_comes() {
    let b = base();
    let (mut world, mut p, band, me, now) = world();
    let her = p
        .members(band)
        .filter(|q| q.life.female && hearth_people::kin::kin_of(&p, me, q.id).is_none())
        .max_by(|a, b| a.life.born.total_cmp(&b.life.born))
        .map(|q| q.id)
        .expect("a woman");
    let age = (now.day - p.get(her).expect("her").life.born) / YEAR_DAYS;
    assert!(age < 38.0, "a woman young enough for children: {age:.0}");
    // Free her of any partner, for the test.
    let was = p.get(her).and_then(|q| q.social.bond);
    for q in p.persons.iter_mut() {
        if q.id == her || Some(q.id) == was {
            q.social.bond = None;
        }
    }
    let mine = KnowledgeState::default();
    let a = p.player_asks(1, her, Ask::Pair, &|n| mine.knows(n), &b.species, &now);
    assert!(
        !a.yes,
        "one not fond of the player pairs with it: {}",
        a.words
    );
    // Time together: she comes to care for the player.
    for q in p.persons.iter_mut().filter(|q| q.id == her) {
        if let Some(t) = q.social.ties.iter_mut().find(|t| t.who == me) {
            t.affection = 0.8;
            t.trust = 0.7;
        }
    }
    let a = p.player_asks(1, her, Ask::Pair, &|n| mine.knows(n), &b.species, &now);
    assert!(a.yes, "{}", a.words);
    assert_eq!(p.get(me).and_then(|q| q.social.bond), Some(her));
    assert_eq!(p.get(her).and_then(|q| q.social.bond), Some(me));
    // Years of the life course: a child of theirs.
    let start = now.day;
    let mut child = None;
    for year in 1..=12 {
        let now = Now {
            day: start + year as f64 * YEAR_DAYS,
            ..now
        };
        p.live_course(&b.species, &b.items, &world, now);
        world.advance();
        child = p
            .persons
            .iter()
            .find(|q| q.life.father == Some(me) && q.life.mother == Some(her))
            .map(|q| q.id);
        if child.is_some() {
            break;
        }
    }
    assert!(child.is_some(), "no child of the pair in twelve years");
}

#[test]
fn a_stranger_trusted_is_taken_in() {
    let b = base();
    let (_w, mut p, band, me, now) = world();
    let other = p
        .persons
        .iter()
        .find(|q| q.social.band != band && q.alive())
        .map(|q| q.social.band)
        .expect("another band");
    let them: Vec<u64> = p.members(other).map(|q| q.id).collect();
    let host = them[0];
    stand_by(&mut p, me, host);
    // They have come to trust the player.
    for q in p.persons.iter_mut().filter(|q| them.contains(&q.id)) {
        let t = hearth_people::ties::Tie {
            who: me,
            affection: 0.6,
            trust: 0.8,
            respect: 0.4,
            fear: 0.0,
            rivalry: 0.0,
            base: 0.05,
            given: 0.0,
            owed: 0.0,
            day: now.day,
            quarrels: 0,
            quarrelled: None,
            named: false,
            courted: None,
        };
        q.social.ties.retain(|x| x.who != me);
        q.social.ties.push(t);
    }
    let mine = KnowledgeState::default();
    let a = p.player_asks(1, host, Ask::Join, &|n| mine.knows(n), &b.species, &now);
    assert!(a.yes, "{}", a.words);
    let bi = p
        .bands
        .iter()
        .position(|x| x.id == other)
        .expect("the band");
    let ways = p.bands[bi]
        .culture
        .ways
        .clone()
        .or_else(|| b.species.get("homo_sapiens").and_then(|s| s.ways.clone()))
        .expect("their ways");
    let later = Now {
        day: now.day + ways.take_in_days + 1.0,
        ..now
    };
    let taken = p.weigh_guests(bi, &ways, &later);
    assert!(taken.contains(&me), "the trusted guest is taken in");
    assert_eq!(p.get(me).map(|q| q.social.band), Some(other));
}

#[test]
fn the_players_children_live_as_others_do() {
    let b = base();
    let (mut world, mut p, band, me, now) = world();
    let her = p
        .members(band)
        .filter(|q| q.life.female && hearth_people::kin::kin_of(&p, me, q.id).is_none())
        .max_by(|a, b| a.life.born.total_cmp(&b.life.born))
        .map(|q| q.id)
        .expect("a woman");
    let was = p.get(her).and_then(|q| q.social.bond);
    for q in p.persons.iter_mut() {
        if q.id == her || Some(q.id) == was {
            q.social.bond = None;
        }
        if q.id == her
            && let Some(t) = q.social.ties.iter_mut().find(|t| t.who == me)
        {
            t.affection = 0.9;
            t.trust = 0.9;
        }
    }
    let mine = KnowledgeState::default();
    let a = p.player_asks(1, her, Ask::Pair, &|n| mine.knows(n), &b.species, &now);
    assert!(a.yes, "{}", a.words);
    let start = now.day;
    for year in 1..=15 {
        let now = Now {
            day: start + year as f64 * YEAR_DAYS,
            ..now
        };
        p.live_course(&b.species, &b.items, &world, now);
        world.advance();
    }
    let (mut ours, mut ours_dead, mut others, mut others_dead) = (0, 0, 0, 0);
    for q in p.persons.iter().filter(|q| q.life.born > start) {
        let dead = !q.alive();
        if q.life.father == Some(me) {
            ours += 1;
            if dead {
                ours_dead += 1;
                println!("ours #{} died: {:?}", q.id, q.life.died);
            }
        } else {
            others += 1;
            if dead {
                others_dead += 1;
            }
        }
    }
    println!("ours {ours_dead}/{ours} dead; others {others_dead}/{others} dead");
    assert!(ours > 0);
    assert!(
        (ours_dead as f64 / ours as f64) < (others_dead as f64 / others.max(1) as f64) + 0.5,
        "the player's children die far more than others'"
    );
}

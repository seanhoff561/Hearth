//! H4 (V2.1 §8.1): kinship read from the pedigree and the bonds, and households — a pair's
//! hearth with the children they raise — kept through decades of the life course.

mod common;

use common::*;
use glam::DVec3;
use hearth_people::kin::{Kin, kin_of};
use hearth_people::{Now, People};

#[test]
fn households_and_kin_hold_through_the_decades() {
    let b = base();
    let species = &b.species;
    let k = species.index_of("homo_sapiens").expect("our species");
    let mut w = Savanna::new();
    let mut p = People::new(21);
    let mut now = w.now();
    for (x, z) in [(0.0, 0.0), (15_000.0, 0.0)] {
        let at = DVec3::new(x, GROUND, z);
        p.spawn_band(
            species,
            &b.graph,
            &b.items,
            &mut w,
            k,
            [8, 7, 9, 3],
            at,
            now,
        );
    }
    let start = now.day;
    for year in 1..=60 {
        now = Now {
            day: start + year as f64 * YEAR_DAYS,
            ..now
        };
        p.live_course(species, &b.items, &w, now);
    }
    let living: Vec<&hearth_people::Person> = p.persons.iter().filter(|q| q.alive()).collect();
    assert!(living.len() > 30, "{} living", living.len());
    // Everyone keeps a hearth; a pair keeps one together; a young child is in its mother's.
    assert!(living.iter().all(|q| q.social.household.is_some()));
    let mut pairs = 0;
    let mut young = 0;
    for q in &living {
        if let Some(r) = q.social.bond.and_then(|r| p.get(r))
            && r.alive()
            && r.social.band == q.social.band
        {
            assert_eq!(q.social.household, r.social.household, "a pair's hearth");
            pairs += 1;
        }
        let age = q.age(&now);
        if age < 10.0
            && let Some(m) = q.life.mother.and_then(|m| p.get(m))
            && m.alive()
            && m.social.band == q.social.band
        {
            assert_eq!(
                q.social.household, m.social.household,
                "a child with its mother"
            );
            young += 1;
        }
    }
    println!(
        "{} living, {pairs} in pairs, {young} young with their mothers",
        living.len()
    );
    assert!(pairs > 4 && young > 4);
    // Kin read rightly: a mother and her child, two of her children, a grandmother.
    let child = living
        .iter()
        .find(|q| {
            q.life
                .mother
                .and_then(|m| p.get(m))
                .and_then(|m| m.life.mother)
                .is_some()
        })
        .expect("a child with a grandmother in the record");
    let mother = child.life.mother.expect("its mother");
    let grandmother = p.get(mother).and_then(|m| m.life.mother).expect("hers");
    assert_eq!(kin_of(&p, child.id, mother), Some(Kin::Parent));
    assert_eq!(kin_of(&p, mother, child.id), Some(Kin::Child));
    assert_eq!(kin_of(&p, child.id, grandmother), Some(Kin::Grandparent));
    assert_eq!(kin_of(&p, grandmother, child.id), Some(Kin::Grandchild));
    if let Some(sib) = p
        .persons
        .iter()
        .find(|q| q.id != child.id && q.life.mother == Some(mother))
    {
        let k = kin_of(&p, child.id, sib.id);
        assert!(matches!(k, Some(Kin::Sibling | Kin::HalfSibling)), "{k:?}");
    }
}

#[test]
fn ties_grow_in_company_kin_begin_closer_and_giving_is_owed() {
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    live(&mut p, &mut world, 300.0, &[]);
    let members: Vec<u64> = p.full().map(|q| q.id).collect();
    // Each knows the others of its band.
    for q in p.full() {
        assert!(
            q.social.ties.len() + 1 >= members.len(),
            "#{} knows {} of {}",
            q.id,
            q.social.ties.len(),
            members.len()
        );
    }
    // A mother is fonder of her child than of one of the band not her kin.
    let tie = |p: &People, a: u64, b: u64| {
        p.get(a)
            .and_then(|q| q.social.ties.iter().find(|t| t.who == b))
            .map(|t| t.affection)
            .unwrap_or(0.0)
    };
    let child = p
        .full()
        .find(|q| living_mother(&p, q).is_some())
        .expect("a child with its mother");
    let mother = living_mother(&p, child).expect("its mother");
    let stranger = p
        .full()
        .find(|q| q.id != mother && kin_of(&p, mother, q.id).is_none())
        .map(|q| q.id)
        .expect("one not her kin");
    assert!(tie(&p, mother, child.id) > tie(&p, mother, stranger) + 0.2);
    // What one gives the other owes, and is the fonder for.
    let before = tie(&p, stranger, mother);
    let day = world.now().day;
    p.give(mother, stranger, 1.0, day);
    let ledger = |p: &People, a: u64, b: u64| {
        p.get(a)
            .and_then(|q| q.social.ties.iter().find(|t| t.who == b))
            .map(|t| (t.given, t.owed))
            .unwrap_or_default()
    };
    assert!(ledger(&p, mother, stranger).0 >= 1.0, "she gave");
    assert!(ledger(&p, stranger, mother).1 >= 1.0, "it is owed her");
    assert!(tie(&p, stranger, mother) > before, "and the fonder");
}

#[test]
fn a_mother_with_food_feeds_her_hungry_child() {
    let b = base();
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    let sp = &b.species.list[b.species.index_of("australopithecus").expect("the hominin")];
    let now = world.now();
    // A weaned child of the band and its mother.
    let child = p
        .full()
        .find(|q| {
            living_mother(&p, q).is_some()
                && q.life_stage(sp, &now) != hearth_people::LifeStage::Infant
        })
        .map(|q| q.id)
        .expect("a weaned child with its mother");
    let mother = p
        .get(child)
        .and_then(|q| living_mother(&p, q))
        .expect("its mother");
    // Something good to eat in her hand; the child hungry.
    let food = b
        .items
        .iter()
        .find(|k| {
            hearth_craft::food::bite_of(&b.content, k, &hearth_items::Stack::of(&k.id, 1))
                .is_some_and(|f| f.kcal > 20.0)
        })
        .map(|k| k.id.clone())
        .expect("a food");
    for q in p.persons.iter_mut() {
        if q.id == mother {
            q.possessions.carry.right = Some(hearth_items::Stack::of(&food, 4));
        }
        if q.id == child {
            q.body.energy.glycogen_kcal = 0.0;
            q.body.energy.fat_kcal *= 0.4;
        }
    }
    live(&mut p, &mut world, 120.0, &[]);
    let gave = p
        .get(mother)
        .and_then(|q| q.social.ties.iter().find(|t| t.who == child))
        .map_or(0.0, |t| t.given);
    println!("she gave {gave:.2} of {food}");
    assert!(gave > 0.0, "she fed her child");
}

#[test]
fn one_hurt_is_tended_by_those_fond_of_them() {
    let b = base();
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    let sp = &b.species.list[b.species.index_of("australopithecus").expect("the hominin")];
    let now = world.now();
    // A mother hurt: a deep cut to the leg.
    let mother = p
        .full()
        .find(|q| p.full().any(|c| c.life.mother == Some(q.id)))
        .map(|q| q.id)
        .expect("a mother");
    for q in p.persons.iter_mut().filter(|q| q.id == mother) {
        let cfg = q.body_config(sp, &now).into_owned();
        q.body
            .injure(
                &cfg,
                "cut",
                hearth_content::schema::body::BodyRegion::LowerLeg,
                hearth_body::Side::Left,
                0.5,
            )
            .expect("a cut");
    }
    live(&mut p, &mut world, 120.0, &[]);
    let tended: f32 = p
        .full()
        .filter(|q| q.id != mother)
        .filter_map(|q| q.social.ties.iter().find(|t| t.who == mother))
        .map(|t| t.given)
        .sum();
    println!("tended her for {tended:.2}");
    assert!(tended > 0.0, "someone stayed by her");
}

#[test]
fn a_theft_is_seen_told_and_sanctioned() {
    use hearth_people::memory::Who;
    use hearth_people::psyche::Feeling;
    use hearth_people::repute::{Deed, Seen};
    let b = base();
    let species = &b.species;
    let mut world = Savanna::new();
    let mut p = People::new(31);
    let k = species.index_of("homo_sapiens").expect("our species");
    let now = world.now();
    p.spawn_band(
        species,
        &b.graph,
        &b.items,
        &mut world,
        k,
        [4, 3, 2, 0],
        DVec3::new(2.0, GROUND, 2.0),
        now,
    );
    live(&mut p, &mut world, 20.0, &[]);
    let grown: Vec<u64> = p
        .full()
        .filter(|q| q.age(&world.now()) >= 18.0)
        .map(|q| q.id)
        .collect();
    let (thief, victim, away) = (grown[0], grown[1], grown[2]);
    // One of them off out of sight when it happens.
    for q in p.persons.iter_mut().filter(|q| q.id == away) {
        q.place.pos += DVec3::new(300.0, 0.0, 0.0);
    }
    let at = p.get(thief).expect("the thief").place.pos;
    // Taken where the thief stands, before whoever is near.
    let seen = |p: &mut People, day: f64| {
        let at = p.get(thief).expect("the thief").place.pos;
        p.deed(
            Seen {
                who: Who::Person(thief),
                deed: Deed::Took {
                    from: Who::Person(victim),
                },
                at,
            },
            &species.norms,
            1.0,
            day,
        );
    };
    seen(&mut p, world.now().day);
    let view = |p: &People, of: u64| {
        p.get(of)
            .and_then(|q| {
                q.social
                    .reputes
                    .iter()
                    .find(|r| r.about == Who::Person(thief))
            })
            .map(|r| (r.honest, r.sure))
            .unwrap_or((0.0, 0.0))
    };
    let (honest, sure) = view(&p, victim);
    assert!(
        honest < -0.4 && sure >= 1.0,
        "the one robbed saw it: {honest:.2} {sure:.2}"
    );
    assert_eq!(view(&p, away), (0.0, 0.0), "the one away did not");
    // Back among them, the one away hears of it in time; the thief is mocked to its face.
    for q in p.persons.iter_mut().filter(|q| q.id == away) {
        q.place.pos = at + DVec3::new(1.0, 0.0, 1.0);
    }
    let mut shamed = 0.0f32;
    for _ in 0..(900.0 / DT) as u64 {
        step(&mut p, &mut world, &[]);
        let s = p
            .get(thief)
            .expect("the thief")
            .psyche
            .feeling(Feeling::Shame);
        shamed = shamed.max(s);
    }
    let (heard, heard_sure) = view(&p, away);
    println!("heard: honest {heard:.2}, sure {heard_sure:.2}; the thief's shame up to {shamed:.2}");
    assert!(heard < -0.05, "the word reached the one who was away");
    assert!(heard_sure < 1.0, "less sure than seeing it");
    assert!(shamed > 0.2, "mocked, and shamed by it");
    // Taken again and again, before them all: cast out at the month's reckoning.
    for _ in 0..3 {
        seen(&mut p, world.now().day);
        live(&mut p, &mut world, 200.0, &[]);
    }
    let band = p.get(thief).expect("the thief").social.band;
    let bi = p.bands.iter().position(|x| x.id == band).expect("its band");
    let now = world.now();
    let v = p.band_view(bi, thief, now.day, now.year_days);
    println!(
        "the band thinks the thief: honest {:.2}, sure {:.2}",
        v.honest, v.sure
    );
    let later = hearth_people::Now {
        day: now.day + now.year_days / 6.0,
        ..now
    };
    p.live_course(species, &b.items, &world, later);
    let after = p.get(thief).expect("the thief");
    assert_ne!(after.social.band, band, "cast out of the band");
    assert!(
        after
            .life
            .events
            .iter()
            .any(|e| matches!(e.event, hearth_people::person::Event::CastOut { .. }))
    );
}

#[test]
fn a_band_debates_and_moves_its_camp() {
    use hearth_people::memory::PlaceKind;
    let b = base();
    let species = &b.species;
    let mut world = Savanna::new();
    let mut p = People::new(41);
    let k = species.index_of("homo_sapiens").expect("our species");
    let now = world.now();
    p.spawn_band(
        species,
        &b.graph,
        &b.items,
        &mut world,
        k,
        [4, 3, 1, 0],
        DVec3::new(2.0, GROUND, 2.0),
        now,
    );
    let bi = 0;
    // The camp's country eaten out; two places remembered away from it: a rich one by water to
    // the east that most know, a thinner one to the north known to three: not yet agreed.
    let east = DVec3::new(600.0, GROUND, 0.0);
    let north = DVec3::new(0.0, GROUND, 500.0);
    let grown: Vec<u64> = p
        .full()
        .filter(|q| q.age(&now) >= 15.0)
        .map(|q| q.id)
        .collect();
    for (k, id) in grown.iter().enumerate() {
        let q = p
            .persons
            .iter_mut()
            .find(|q| q.id == *id)
            .expect("one of them");
        q.memory.places.retain(|r| r.kind != PlaceKind::Food);
        if k < 3 {
            q.memory.remember(PlaceKind::Food, north, now.day, 0.9);
            q.memory.remember(
                PlaceKind::Food,
                north + DVec3::new(20.0, 0.0, 0.0),
                now.day,
                0.9,
            );
        } else {
            q.memory.remember(PlaceKind::Food, east, now.day, 0.9);
            q.memory.remember(
                PlaceKind::Food,
                east + DVec3::new(30.0, 0.0, 10.0),
                now.day,
                0.8,
            );
            q.memory.remember(
                PlaceKind::Water,
                east + DVec3::new(60.0, 0.0, 0.0),
                now.day,
                0.9,
            );
        }
    }
    // Hungry, all of them: they hold council.
    let moved = p.council(bi, now, &|_| true);
    let c = p.bands[bi].council.clone().expect("a council held");
    println!(
        "council: {:?} after {} rounds",
        c.options
            .iter()
            .map(|(at, n)| (at.x.round(), at.z.round(), *n))
            .collect::<Vec<_>>(),
        c.rounds
    );
    assert!(moved, "they moved camp");
    assert!(c.rounds >= 1, "they talked it over");
    let to = c.chosen.expect("they agreed");
    assert!(
        (to - east).length() < 160.0,
        "to the rich place by the water"
    );
    assert_eq!(p.bands[bi].camp, Some(to));
    // Off they go, all of them.
    assert!(p.full().all(|q| matches!(
        q.mind.doing,
        hearth_people::Doing::Going { to: t, .. } if (t - to).length() < 1.0
    )));
}

/// One step of play, the species as given (their ways changed).
fn step_as(p: &mut People, world: &mut Savanna, species: &hearth_people::SpeciesSet) {
    let b = base();
    world.advance();
    let now = world.now();
    p.step(
        species,
        &b.crafts,
        &b.graph,
        &b.content,
        &b.items,
        world,
        &[],
        now,
        DT,
    );
}

/// A band of our kind about a place: its grown (eighteen and over).
fn our_band(p: &mut People, world: &mut Savanna, n: [u16; 4], at: DVec3) -> Vec<u64> {
    let b = base();
    let k = b.species.index_of("homo_sapiens").expect("our species");
    let now = world.now();
    let band = p.spawn_band(&b.species, &b.graph, &b.items, world, k, n, at, now);
    p.members(band)
        .filter(|q| q.age(&now) >= 18.0)
        .map(|q| q.id)
        .collect()
}

#[test]
fn a_grievance_is_had_out_and_eases() {
    use hearth_people::person::Event;
    use hearth_people::psyche::{Feeling, Tendency};
    let mut world = Savanna::new();
    let mut p = People::new(51);
    let grown = our_band(
        &mut p,
        &mut world,
        [3, 3, 0, 0],
        DVec3::new(2.0, GROUND, 2.0),
    );
    live(&mut p, &mut world, 10.0, &[]);
    let (a, wrongdoer) = (grown[0], grown[1]);
    // Wronged by the other, and quick to anger; the two near each other.
    p.wronged(a, wrongdoer, 0.8, world.now().day);
    let at = p.get(wrongdoer).expect("the wrongdoer").place.pos;
    for q in p.persons.iter_mut().filter(|q| q.id == a) {
        q.place.pos = at + DVec3::new(3.0, 0.0, 0.0);
        q.psyche.tendencies[Tendency::AggressionThreshold] = 0.0;
        q.psyche.feelings[Feeling::Anger] = 0.9;
    }
    let rivalry = |p: &People| {
        p.get(a)
            .and_then(|q| q.social.ties.iter().find(|t| t.who == wrongdoer))
            .map_or(0.0, |t| t.rivalry)
    };
    let before = rivalry(&p);
    let (mut began, mut over) = (false, false);
    for _ in 0..(600.0 / DT) as u64 {
        step(&mut p, &mut world, &[]);
        let quarrelling = p.quarrelling(a);
        began |= quarrelling;
        if began && !quarrelling {
            over = true;
            break;
        }
    }
    let q = p.get(a).expect("the wronged");
    let told: Vec<String> = q
        .life
        .events
        .iter()
        .rev()
        .take(4)
        .map(|e| format!("{:?}", e.event))
        .collect();
    println!("rivalry {before:.2} -> {:.2}; {told:?}", rivalry(&p));
    assert!(began, "the grievance was had out");
    assert!(over, "and the quarrel ended");
    assert!(
        q.life
            .events
            .iter()
            .any(|e| matches!(e.event, Event::Quarrelled { with } if with == wrongdoer))
    );
    assert!(rivalry(&p) < before, "the grievance eased");
}

#[test]
fn a_quarrel_is_talked_round_by_one_of_standing() {
    use hearth_people::person::Event;
    use hearth_people::psyche::Tendency;
    let b = base();
    let mut species = b.species.clone();
    let k = species.index_of("homo_sapiens").expect("our species");
    // A long argument, to give one time to step in.
    if let Some(w) = species.list[k].ways.as_mut() {
        w.argue_s = 600.0;
    }
    let mut world = Savanna::new();
    let mut p = People::new(52);
    let grown = our_band(
        &mut p,
        &mut world,
        [3, 3, 0, 0],
        DVec3::new(2.0, GROUND, 2.0),
    );
    for _ in 0..(10.0 / DT) as u64 {
        step_as(&mut p, &mut world, &species);
    }
    let (a, c, elder) = (grown[0], grown[1], grown[2]);
    let day = world.now().day;
    // Two of them, proud and empty-handed, at it; a third, well respected by them all, near.
    for q in p.persons.iter_mut().filter(|q| q.id == a || q.id == c) {
        q.psyche.tendencies[Tendency::Dominance] = 1.0;
        q.possessions.carry = Default::default();
    }
    for &g in grown.iter().filter(|g| **g != elder) {
        p.give(elder, g, 0.0, day);
    }
    let at = p.get(a).expect("one").place.pos;
    for q in p.persons.iter_mut() {
        if let Some(t) = q.social.ties.iter_mut().find(|t| t.who == elder) {
            t.respect = 0.95;
        }
        if q.id == c {
            q.place.pos = at + DVec3::new(2.0, 0.0, 0.0);
        }
        if q.id == elder {
            q.place.pos = at + DVec3::new(0.0, 0.0, 12.0);
            q.psyche.tendencies[Tendency::Cooperativeness] = 1.0;
        }
    }
    assert!(p.quarrel(a, c, hearth_people::Rung::Argument, day));
    let mediated = |p: &People| {
        p.get(elder).is_some_and(|q| {
            q.life
                .events
                .iter()
                .any(|e| matches!(e.event, Event::Mediated { .. }))
        })
    };
    for _ in 0..(300.0 / DT) as u64 {
        step_as(&mut p, &mut world, &species);
        if mediated(&p) {
            break;
        }
    }
    assert!(mediated(&p), "the respected one talked them round");
    assert!(!p.quarrelling(a) && !p.quarrelling(c), "the quarrel over");
}

#[test]
fn a_feud_parts_the_band() {
    use hearth_people::person::Event;
    let b = base();
    let species = &b.species;
    let mut world = Savanna::new();
    let mut p = People::new(53);
    let grown = our_band(
        &mut p,
        &mut world,
        [4, 4, 2, 0],
        DVec3::new(2.0, GROUND, 2.0),
    );
    live(&mut p, &mut world, 10.0, &[]);
    // A day of their lives on: each in a household.
    let now = world.now();
    p.live_course(
        species,
        &b.items,
        &world,
        Now {
            day: now.day + 1.0,
            ..now
        },
    );
    let a = grown[0];
    let home = |p: &People, id: u64| p.get(id).and_then(|q| q.social.household);
    let c = grown
        .iter()
        .copied()
        .find(|&x| x != a && home(&p, x) != home(&p, a))
        .expect("one of another household");
    let band = p.get(a).expect("one").social.band;
    // Quarrel upon quarrel, the grudge deep on both sides.
    let day = world.now().day;
    p.wronged(a, c, 0.95, day);
    p.wronged(c, a, 0.95, day);
    for q in p.persons.iter_mut().filter(|q| q.id == a || q.id == c) {
        for t in q
            .social
            .ties
            .iter_mut()
            .filter(|t| t.who == a || t.who == c)
        {
            t.quarrels = 5;
        }
    }
    let later = Now {
        day: now.day + 1.0 + now.year_days / 6.0,
        ..now
    };
    p.live_course(species, &b.items, &world, later);
    let gone: Vec<&hearth_people::Person> = [a, c]
        .iter()
        .filter_map(|&x| p.get(x))
        .filter(|q| q.social.band != band)
        .collect();
    println!(
        "{} of the two left; bands now {}",
        gone.len(),
        p.bands.len()
    );
    assert_eq!(gone.len(), 1, "one side left");
    let g = gone[0];
    assert!(
        g.life
            .events
            .iter()
            .any(|e| matches!(e.event, Event::Left { from } if from == band))
    );
    // Its household with it.
    if let Some(h) = g.social.household {
        assert!(
            p.persons
                .iter()
                .filter(|q| q.alive() && q.social.household == Some(h))
                .all(|q| q.social.band == g.social.band)
        );
    }
}

#[test]
fn a_stranger_is_greeted_and_taken_in() {
    use hearth_people::person::Event;
    let b = base();
    let species = &b.species;
    let mut world = Savanna::new();
    let mut p = People::new(54);
    let hosts = our_band(
        &mut p,
        &mut world,
        [3, 3, 1, 0],
        DVec3::new(2.0, GROUND, 2.0),
    );
    // One alone, of no band near, coming by.
    our_band(
        &mut p,
        &mut world,
        [0, 1, 0, 0],
        DVec3::new(25.0, GROUND, 2.0),
    );
    let (host, lone) = (p.bands[0].id, p.bands[1].id);
    let stranger = p.members(lone).next().expect("the stranger").id;
    for _ in 0..(300.0 / DT) as u64 {
        step(&mut p, &mut world, &[]);
        if p.guest_of(host, stranger) {
            break;
        }
    }
    assert!(p.guest_of(host, stranger), "met and greeted");
    assert!(
        p.get(stranger)
            .expect("the stranger")
            .life
            .events
            .iter()
            .any(|e| matches!(e.event, Event::Greeted { .. }))
    );
    // Gifts to each of the grown, and days among them: taken in.
    let now = world.now();
    let stone = hearth_items::Stack::of(&item("marula_stone"), 1);
    for &g in &hosts {
        for _ in 0..5 {
            // Each gift put down before the next (hands hold one thing each).
            for q in p.persons.iter_mut().filter(|q| q.id == g) {
                q.possessions.carry = Default::default();
            }
            p.gift(
                stranger,
                g,
                stone.clone(),
                species,
                &b.items,
                &b.content,
                now,
            )
            .expect("taken");
        }
    }
    let k = species.index_of("homo_sapiens").expect("our species");
    let ways = species.list[k].ways.clone().expect("their ways");
    let later = Now {
        day: now.day + ways.take_in_days + 0.1,
        ..now
    };
    let bi = p
        .bands
        .iter()
        .position(|x| x.id == host)
        .expect("the hosts");
    let taken = p.weigh_guests(bi, &ways, &later);
    let trust: Vec<f32> = hosts
        .iter()
        .filter_map(|&g| p.get(g))
        .filter_map(|q| q.social.ties.iter().find(|t| t.who == stranger))
        .map(|t| t.trust)
        .collect();
    println!("the hosts' trust in the stranger: {trust:.2?}");
    assert_eq!(taken, vec![stranger], "taken in");
    let s = p.get(stranger).expect("the stranger");
    assert_eq!(s.social.band, host);
    assert!(
        s.life
            .events
            .iter()
            .any(|e| matches!(e.event, Event::TakenIn { band } if band == host))
    );
}

#[test]
fn an_unwelcome_stranger_is_warned_off() {
    use hearth_people::person::Event;
    let b = base();
    let mut species = b.species.clone();
    let k = species.index_of("homo_sapiens").expect("our species");
    // Their country as crowded as they will bear: strangers are not welcome.
    if let Some(w) = species.list[k].ways.as_mut() {
        w.warn_off_crowding = 0.0;
    }
    let mut world = Savanna::new();
    let mut p = People::new(55);
    let now = world.now();
    for (n, x) in [([3, 3, 1, 0], 2.0), ([0, 1, 0, 0], 40.0)] {
        p.spawn_band(
            &species,
            &b.graph,
            &b.items,
            &mut world,
            k,
            n,
            DVec3::new(x, GROUND, 2.0),
            now,
        );
    }
    let (host, lone) = (p.bands[0].id, p.bands[1].id);
    let stranger = p.members(lone).next().expect("the stranger").id;
    let start = p.get(stranger).expect("the stranger").place.pos;
    let (mut warned, mut went) = (false, 0.0f64);
    for _ in 0..(300.0 / DT) as u64 {
        step_as(&mut p, &mut world, &species);
        let s = p.get(stranger).expect("the stranger");
        warned |= s
            .life
            .events
            .iter()
            .any(|e| matches!(e.event, Event::Unwelcome { .. }));
        if warned {
            went = went.max((s.place.pos - start).length());
        }
    }
    println!("warned off: {warned}; went {went:.0} m");
    assert!(warned, "warned off");
    assert!(!p.guest_of(host, stranger), "not made a guest");
    assert!(went > 20.0, "and it went");
}

#[test]
fn word_of_a_theft_spreads_through_a_band() {
    use hearth_people::memory::Who;
    use hearth_people::repute::{Deed, Seen};
    let b = base();
    let species = &b.species;
    let mut world = Savanna::new();
    let mut p = People::new(56);
    let grown = our_band(
        &mut p,
        &mut world,
        [8, 7, 4, 0],
        DVec3::new(2.0, GROUND, 2.0),
    );
    live(&mut p, &mut world, 10.0, &[]);
    let (thief, victim) = (grown[0], grown[1]);
    // In the dusk, seen only by those within a few metres of it: the one robbed, by the thief;
    // the rest a little way off.
    let at = p.get(thief).expect("the thief").place.pos;
    for q in p.persons.iter_mut() {
        if q.id == victim {
            q.place.pos = at + DVec3::new(1.0, 0.0, 0.0);
        } else if q.id != thief {
            let off = q.place.pos - at;
            let flat = DVec3::new(off.x, 0.0, off.z).normalize_or(DVec3::X);
            if off.length() < 14.0 {
                q.place.pos = at + flat * 14.0;
            }
        }
    }
    let day = world.now().day;
    p.deed(
        Seen {
            who: Who::Person(thief),
            deed: Deed::Took {
                from: Who::Person(victim),
            },
            at,
        },
        &species.norms,
        0.25,
        day,
    );
    // Who of the grown think the thief dishonest, and how sure they are of it on the whole.
    let knowing = |p: &People| -> (Vec<u64>, f32) {
        let views: Vec<(u64, f32)> = grown
            .iter()
            .filter(|&&g| g != thief)
            .filter_map(|&g| {
                p.get(g)?
                    .social
                    .reputes
                    .iter()
                    .find(|r| r.about == Who::Person(thief) && r.honest < -0.05)
                    .map(|r| (g, r.sure))
            })
            .collect();
        let sure = views.iter().map(|v| v.1).sum::<f32>() / views.len().max(1) as f32;
        (views.into_iter().map(|v| v.0).collect(), sure)
    };
    let (saw, _) = knowing(&p);
    live(&mut p, &mut world, 1800.0, &[]);
    let (know, _) = knowing(&p);
    let heard: Vec<u64> = know.iter().copied().filter(|g| !saw.contains(g)).collect();
    let heard_sure = heard
        .iter()
        .filter_map(|&g| {
            p.get(g)?
                .social
                .reputes
                .iter()
                .find(|r| r.about == Who::Person(thief))
                .map(|r| r.sure)
        })
        .fold(0.0f32, f32::max);
    println!(
        "{} grown; {} saw it, {} know of it after half a day ({} by word, sure at most {heard_sure:.2})",
        grown.len(),
        saw.len(),
        know.len(),
        heard.len()
    );
    assert!(saw.len() < grown.len() - 1, "not all saw it");
    assert!(!heard.is_empty(), "word of it spread");
    assert!(
        heard_sure < 1.0,
        "those who heard are less sure than those who saw"
    );
}

//! H10's conversation suite (V2.1 §10.4): with the backend on, conversations never leak unknown
//! facts or anachronisms, and never change the world but through speech acts.
//!
//! A band of our kind lives about the player, with strangers near and talk of a theft; every act
//! said — greetings, warnings, gossip, a case in council, and the answers to what the player says
//! and asks — is taken through the backend's whole path: the context the model
//! would be told, the request built from it, and lines a model might give, held to the filter.
//! The model is a script: lines as a willing model phrases them, and lines that leak — a
//! technique the speaker does not know, a word of a later age, a name not given it, kin it does
//! not have, numbers and markup, words never said, a refusal turned to a yes. The willing lines
//! pass; every leak is refused. Then two worlds of the same seed are lived side by side, the
//! player saying the same in both, in one with the backend phrasing every line and reading the
//! player's typed words, in the other with neither: they end the same, act for act.

#[path = "../../hearth_people/tests/common/mod.rs"]
mod common;

use std::collections::BTreeSet;

use common::*;
use glam::DVec3;
use hearth_ai::{ConversationBackend, Lexicon, Prompts, Rejected, Request, Scripted, read_parse};
use hearth_craft::KnowledgeState;
use hearth_people::converse::{Context, Guard};
use hearth_people::memory::Who;
use hearth_people::player::Ask;
use hearth_people::repute::{Deed, Seen};
use hearth_people::speech::{Act, HEARD_M};
use hearth_people::{People, Said};

/// The player's band, a band of strangers near, the player (one of its band's grown men), and
/// a theft seen, so there is talk.
fn world(seed: u64) -> (Savanna, People, u64, u64) {
    let b = base();
    let k = b.species.index_of("homo_sapiens").expect("our species");
    let mut world = Savanna::new();
    let mut p = People::new(seed);
    let now = world.now();
    let band = p.spawn_band(
        &b.species,
        &b.graph,
        &b.items,
        &mut world,
        k,
        [5, 5, 2, 2],
        DVec3::new(2.0, GROUND, 2.0),
        now,
    );
    p.spawn_band(
        &b.species,
        &b.graph,
        &b.items,
        &mut world,
        k,
        [2, 2, 0, 0],
        DVec3::new(30.0, GROUND, 12.0),
        now,
    );
    let me = p
        .members(band)
        .filter(|q| !q.life.female && q.age(&now) >= 18.0)
        .max_by(|a, b| a.life.born.total_cmp(&b.life.born))
        .map(|q| q.id)
        .expect("a man");
    for q in p.persons.iter_mut().filter(|q| q.id == me) {
        q.player = Some(ONE);
        q.name = "Sean".to_owned();
    }
    let grown: Vec<u64> = p
        .members(band)
        .filter(|q| q.id != me && q.age(&now) >= 18.0)
        .map(|q| q.id)
        .collect();
    let there = p.get(grown[0]).expect("one").place.pos;
    p.deed(
        Seen {
            who: Who::Person(grown[0]),
            deed: Deed::Took {
                from: Who::Person(grown[1]),
            },
            at: there,
        },
        &b.species.norms,
        1.0,
        now.day,
    );
    (world, p, band, me)
}

/// Every act said over a while of play, and the answers to what the player says to its band and
/// to the strangers.
fn talk() -> (People, Vec<Said>, u64, hearth_people::Now) {
    let b = base();
    let (mut world, mut p, band, me) = world(17);
    let mine = KnowledgeState::default();
    let now = world.now();
    let them: Vec<u64> = p
        .members(band)
        .filter(|q| q.id != me)
        .map(|q| q.id)
        .collect();
    let strangers: Vec<u64> = p
        .persons
        .iter()
        .filter(|q| q.social.band != band)
        .map(|q| q.id)
        .collect();
    let mut asks: Vec<(u64, Ask)> = vec![
        (them[0], Ask::Greet),
        (them[1], Ask::BeTaught(None)),
        (them[2], Ask::Insult),
        (them[3], Ask::Pair),
        (strangers[0], Ask::Introduce),
        (strangers[0], Ask::Join),
        (strangers[1], Ask::BeTaught(None)),
    ];
    let mut said: Vec<Said> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut keep = |p: &People, said: &mut Vec<Said>| {
        for s in &p.said {
            let key = format!("{}|{:?}|{:?}|{:?}", s.speaker, s.act, s.words, s.to);
            if s.speaker != me && seen.insert(key) {
                said.push(s.clone());
            }
        }
    };
    for (to, ask) in asks.drain(..) {
        p.player_asks(ONE, to, ask, &|n| mine.knows(n), &b.species, &now);
        keep(&p, &mut said);
    }
    for _ in 0..(8 * 60) {
        live(&mut p, &mut world, 1.0, &[]);
        keep(&p, &mut said);
    }
    let now = world.now();
    (p, said, me, now)
}

/// Lines a willing model gives for an act (`{name}`: a name the act carries).
fn willing(act: Act) -> &'static [&'static str] {
    match act {
        Act::Greet => &[
            "Hello, friend. It is good to see you.",
            "Ah, you are here! Welcome.",
            "Greetings. Come, sit with us.",
        ],
        Act::Introduce => &["They call me {name}.", "I am {name}. And you?"],
        Act::Warn => &[
            "Go away! This place is ours.",
            "Stay back, stranger. Leave us be.",
        ],
        Act::Refuse => &[
            "No. I do not know you well enough.",
            "Not now. Leave me be.",
            "No, I will not.",
        ],
        Act::Accept => &["Yes. Stay here with us.", "Come, you are welcome among us."],
        Act::Teach => &[
            "Watch my hands closely, and I will show you.",
            "Look here. See how I do it?",
        ],
        Act::Thank => &["Thank you. That was kind of you.", "Thanks, friend."],
        Act::Apologise => &["I am sorry. I was wrong.", "Forgive me."],
        Act::Insult => &["You are a fool!", "Bah, you are useless."],
        Act::Threaten => &["Back off, or I will hit you!"],
        Act::Command => &["Stop! That is enough."],
        Act::Offer => &["Here, eat this. You must be hungry."],
        Act::Gossip => &["Listen. {name} takes what is not theirs."],
        Act::ArgueFor | Act::ArgueAgainst => &[
            "We should stay here. There is food near.",
            "Let us go on. There is more to eat over there.",
        ],
        _ => &["Hm. Yes, I see."],
    }
}

/// The names an act carries, as its words have them.
fn named(p: &People, s: &Said) -> Vec<String> {
    s.words
        .iter()
        .filter_map(|w| match w {
            hearth_people::speech::Word::Name(id) => p.get(*id).map(|q| q.name.clone()),
            _ => None,
        })
        .filter(|n| !n.is_empty())
        .collect()
}

/// Lines that leak, for a speaker: each must be refused, and why.
fn leaks(lex: &Lexicon, guard: &Guard, ctx: &Context) -> Vec<(String, &'static str)> {
    let b = base();
    let mut out: Vec<(String, &'static str)> = Vec::new();
    // Techniques it does not know: their own words.
    for n in b
        .graph
        .nodes
        .iter()
        .filter(|n| !guard.known.contains(&n.id))
    {
        for w in lex.words_of_technique(&n.id).into_iter().take(2) {
            if lex.may_say(&w, guard) {
                continue;
            }
            out.push((format!("Let me show you the {w}."), "unknown technique"));
        }
    }
    // Later ages.
    for w in lex.later_words().into_iter().step_by(7) {
        if lex.may_say(&w, guard) {
            continue;
        }
        out.push((format!("Come and see my {w}, friend."), "later age"));
    }
    for line in [
        "I will trade you this meat for a coin.",
        "Shall we farm this field together?",
        "I wrote it down so we would remember.",
        "My people came out of Africa long ago.",
        "We are Homo sapiens, and they are Neanderthals.",
        "Check your phone, it is late.",
        "The king will hear of this.",
        "Okay, dude, see you later.",
    ] {
        out.push((line.to_owned(), "later age"));
    }
    // Names not given it.
    for n in guard.unknown_names.iter().take(6) {
        out.push((format!("{n} told me about you."), "unknown name"));
        out.push((format!("Hello. Have you seen {n}?"), "unknown name"));
    }
    out.push((
        "Greetings from Gronk the Mighty.".to_owned(),
        "unknown name",
    ));
    // Kin it does not have.
    for k in [
        "sister",
        "brother",
        "daughter",
        "son",
        "wife",
        "husband",
        "grandmother",
        "uncle",
    ] {
        if !guard.kin.contains(k) {
            out.push((format!("My {k} will help you."), "kin it lacks"));
        }
    }
    // What a spoken line never holds.
    for line in [
        "I have 3 spears.",
        "*smiles warmly* Hello there.",
        "Hello! :)",
        "<b>Hello</b>",
        "Hello\nfriend",
        "As an AI language model, I cannot say that.",
        "Sure! Here is the line: Hello.",
        "(laughs) You are funny.",
        "Hello, friend. I hope this helps! Let me know if you need anything else, and I will be glad to assist with further requests or questions you may have about this.",
    ] {
        out.push((line.to_owned(), "not a spoken line"));
    }
    for w in ["naked", "torture", "slave"] {
        out.push((format!("You are a {w}."), "forbidden"));
    }
    // Meaning turned about.
    match guard.act {
        Some(Act::Refuse) => {
            out.push(("Yes, of course, gladly.".to_owned(), "opposite meaning"));
        }
        Some(Act::Accept) => out.push(("No. Go away.".to_owned(), "opposite meaning")),
        _ => {}
    }
    // The speaker's own name is given; names it alone knows are not given the model either.
    let _ = ctx;
    out
}

#[test]
fn conversations_never_leak_unknown_facts_or_anachronisms() {
    let b = base();
    let lex = Lexicon::from_content(&b.content, &b.graph);
    assert!(
        lex.clashes().is_empty(),
        "everyday words also listed as never said: {:?}",
        lex.clashes()
    );
    let prompts = Prompts::from_content(&b.content).expect("the prompts");
    let (p, said, me, now) = talk();
    let acts: BTreeSet<String> = said.iter().map(|s| format!("{:?}", s.act)).collect();
    println!("{} acts said: {acts:?}", said.len());
    assert!(said.len() >= 10, "talk enough to try: {}", said.len());
    assert!(acts.len() >= 5, "acts of several kinds: {acts:?}");
    let (mut passed, mut willing_n, mut refused, mut leaks_n) = (0, 0, 0, 0);
    for s in &said {
        let Some((ctx, guard)) = p.context_for(s, &b.graph, &b.species, &now) else {
            continue;
        };
        // What the model is told is the speaker's own: no name not given it, no technique it does
        // not know.
        let req = prompts.phrase(&ctx);
        let told = format!("{}\n{}", req.system, req.user);
        for n in &guard.unknown_names {
            let whole = told
                .split(|c: char| !c.is_alphabetic())
                .any(|w| w == n.as_str());
            assert!(
                !whole,
                "the model is told of {n}, whom the speaker may not name"
            );
        }
        for n in b
            .graph
            .nodes
            .iter()
            .filter(|n| n.implemented && !guard.known.contains(&n.id))
        {
            let listed = ctx.knows.iter().any(|k| *k == n.name);
            assert!(!listed, "the model is told the speaker knows {}", n.name);
        }
        assert!(
            ctx.knows.len() <= guard.known.len(),
            "it is told no more techniques than it knows"
        );
        // A willing model's lines pass.
        let names = named(&p, s);
        for line in willing(s.act) {
            let line = match names.first() {
                Some(n) => line.replace("{name}", n),
                None if line.contains("{name}") => continue,
                None => (*line).to_owned(),
            };
            willing_n += 1;
            match lex.check(&line, &guard) {
                Ok(_) => passed += 1,
                Err(why) => panic!(
                    "a willing line refused ({why}) for {:?} by #{}: {line}",
                    s.act, s.speaker
                ),
            }
        }
        // Every leak is refused.
        for (line, kind) in leaks(&lex, &guard, &ctx) {
            leaks_n += 1;
            match lex.check(&line, &guard) {
                Err(why) => {
                    refused += 1;
                    if kind == "unknown name" {
                        assert!(matches!(why, Rejected::Name(_)), "{line}: {why}");
                    }
                    if kind == "kin it lacks" {
                        assert!(matches!(why, Rejected::Kin(_)), "{line}: {why}");
                    }
                }
                Ok(shown) => panic!(
                    "a leak ({kind}) passed for {:?} by #{} (to #{:?}, the player #{me}): {shown}",
                    s.act, s.speaker, s.to
                ),
            }
        }
    }
    println!("willing lines passed {passed}/{willing_n}; leaks refused {refused}/{leaks_n}");
    assert!(leaks_n > 500, "leaks enough to be sure: {leaks_n}");
    assert_eq!(refused, leaks_n);
}

/// A script standing for the model: phrases each act with a willing line — and, every third
/// time, a leaking one — and reads typed words by what they say.
fn scripted() -> Box<dyn ConversationBackend> {
    let mut n = 0u32;
    Box::new(Scripted(move |r: &Request| {
        n += 1;
        if r.system.contains("single speech act") {
            let typed = r.user.to_lowercase();
            let act = if typed.contains("hello") {
                "greet"
            } else if typed.contains("thank") {
                "thank"
            } else if typed.contains("show me") {
                "be_taught"
            } else {
                "none"
            };
            // A reply trying to do more than an act: it is read as the act alone.
            return Ok(format!(
                "{{\"act\": \"{act}\", \"technique\": null, \"sure\": 0.9, \"or\": [], \"set_trust\": 1.0, \"give\": \"everything\"}}"
            ));
        }
        Ok(if n % 3 == 0 {
            "The king of Rome will bring you iron and bread.".to_owned()
        } else {
            "Hello, friend. Come and sit by the fire.".to_owned()
        })
    }))
}

#[test]
fn the_world_changes_only_through_speech_acts() {
    let b = base();
    let lex = Lexicon::from_content(&b.content, &b.graph);
    let prompts = Prompts::from_content(&b.content).expect("the prompts");
    let mine = KnowledgeState::default();
    // Two worlds of one seed; the player says the same in both: on the wheel in one, typed and
    // read by the backend in the other, which also phrases every line said near the player.
    let (mut wa, mut a, band, me) = world(41);
    let (mut wb, mut c, _, _) = world(41);
    let mut backend = scripted();
    let near: Vec<u64> = a
        .members(band)
        .filter(|q| q.id != me)
        .map(|q| q.id)
        .collect();
    let typed = [
        "hello there!",
        "thank you so much",
        "show me how you do that",
    ];
    let wheel = [Ask::Greet, Ask::Thank, Ask::BeTaught(None)];
    let (mut phrased, mut shown, mut read) = (0, 0, 0);
    let mut done: BTreeSet<String> = BTreeSet::new();
    for minute in 0..6 {
        let to = near[minute % near.len()];
        let now = wa.now();
        // The world without the backend: the wheel's act.
        c.player_asks(
            ONE,
            to,
            wheel[minute % 3].clone(),
            &|n| mine.knows(n),
            &b.species,
            &now,
        );
        // The world with it: the typed words read, and only the act they are read as made.
        let offered: Vec<(String, String)> = b
            .graph
            .nodes
            .iter()
            .filter(|n| n.implemented)
            .map(|n| (n.id.clone(), n.name.clone()))
            .collect();
        let names: Vec<String> = offered.iter().map(|(_, n)| n.clone()).collect();
        let req = prompts.parse(typed[minute % 3], "one of your band", &names);
        let reply = backend.complete(&req).expect("a reply");
        let parsed = read_parse(&reply, &offered).expect("read");
        let ask = parsed.ask.clone().expect("an act");
        assert!(parsed.settled());
        assert_eq!(
            ask,
            wheel[minute % 3],
            "read as the act the wheel would make"
        );
        read += 1;
        a.player_asks(ONE, to, ask, &|n| mine.knows(n), &b.species, &now);
        for _ in 0..60 {
            live(&mut a, &mut wa, 1.0, &[]);
            live(&mut c, &mut wb, 1.0, &[]);
            // Every line said near the player phrased, and held to the filter: shown or not, it
            // touches nothing.
            let at = a.get(me).expect("the player").place.pos;
            let lines: Vec<Said> = a
                .said
                .iter()
                .filter(|s| s.speaker != me && (s.at - at).length() < HEARD_M)
                .filter(|s| {
                    done.insert(format!("{}|{:?}|{:?}|{}", s.speaker, s.act, s.words, s.day))
                })
                .cloned()
                .collect();
            for s in &lines {
                let Some((ctx, guard)) = a.context_for(s, &b.graph, &b.species, &wa.now()) else {
                    continue;
                };
                let line = backend.complete(&prompts.phrase(&ctx)).expect("a line");
                phrased += 1;
                if lex.check(&line, &guard).is_ok() {
                    shown += 1;
                }
            }
        }
    }
    println!("phrased {phrased} lines ({shown} shown), read {read} typed lines");
    assert!(phrased > 0 && shown > 0 && shown < phrased);
    // The same world, act for act.
    assert!(a.said == c.said, "the same said");
    let (sa, sc) = (
        hearth_people::save::to_json(&a.to_save()).expect("saved"),
        hearth_people::save::to_json(&c.to_save()).expect("saved"),
    );
    assert!(sa == sc, "the backend changed the world");
}

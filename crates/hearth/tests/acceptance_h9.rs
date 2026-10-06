//! H9's acceptance (PLAN.md): a scripted player, born into a band of the Upper Paleolithic, asks
//! one of its band to show it how and is taught; courts a woman of the band until she is willing
//! and pairs with her; the years pass and they have children; the player dies, and goes on as
//! their grown child.

mod common;

use common::*;
use hearth_people::player::Ask;
use hearth_physics::Motion;
use hearth_protocol::{Moved, ToClient, ToServer};

/// What the player knows of a person (the regard's lines).
fn regard(w: &mut World, id: u64) -> Vec<String> {
    w.regarded = None;
    w.server.send(ToServer::Regard(Some(id)));
    w.run(20);
    w.until(10.0, |w| w.regarded.as_ref().is_some_and(|r| r.0 == id));
    w.regarded.clone().map_or_else(Vec::new, |r| r.1)
}

/// Goes beside a person and says or does something to them; the answer.
fn ask(w: &mut World, id: u64, what: Ask) -> (bool, String) {
    if let Some(v) = w.people.iter().find(|v| v.id == id) {
        let at = v.pos;
        w.go(at.x + 1.2, at.z);
    }
    let n = w.acted.len();
    w.server.send(ToServer::Speak {
        person: id,
        ask: what,
    });
    w.run(5);
    w.until(10.0, |w| w.acted.len() > n);
    w.acted
        .get(n)
        .map_or((false, String::new()), |a| (a.1, a.2.clone()))
}

/// The children a woman bore, by the inspector's record of her life.
fn children_of(w: &mut World, mother: u64) -> Vec<u64> {
    w.inspected = None;
    w.server.send(ToServer::Inspect(Some(mother)));
    w.run(40);
    w.until(10.0, |w| w.inspected.as_ref().is_some_and(|r| r.id == mother));
    let Some(r) = w.inspected.clone() else {
        return Vec::new();
    };
    w.server.send(ToServer::Inspect(None));
    r.sections
        .iter()
        .filter(|s| s.name == "Life")
        .flat_map(|s| s.lines.iter())
        .flat_map(|l| l.split("; "))
        .filter_map(|e| e.split("bore #").nth(1))
        .filter_map(|n| n.trim().parse::<u64>().ok())
        .collect()
}

#[test]
fn a_player_born_into_a_band_is_taught_forms_a_family_and_lives_on_as_their_grown_child() {
    let dir = temp("acceptance-h9");
    let mut w = World::start_in(
        &dir,
        hearth_save::KnowledgeMode::default(),
        3,
        false,
        "hearth:upper_paleolithic",
        Some(0),
    );
    w.run(200);
    // The grown of the band about the player, by what the player knows of them.
    let mut band: Vec<(u64, Vec<String>)> = Vec::new();
    let near: Vec<u64> = w
        .people
        .iter()
        .filter(|v| !v.dead && format!("{:?}", v.stage) == "Adult")
        .map(|v| v.id)
        .collect();
    for id in near {
        let lines = regard(&mut w, id);
        if lines
            .first()
            .is_some_and(|l| l.contains("your") || l.contains("of your band"))
        {
            band.push((id, lines));
        }
    }
    println!("the grown of the band: {}", band.len());
    assert!(band.len() >= 4, "a band about the player: {band:?}");

    // 1. Taught: one of the band shows the player something it did not know.
    let known = w.knowledge.known.len();
    let mut taught = None;
    for (id, _) in &band {
        let (yes, words) = ask(&mut w, *id, Ask::BeTaught(None));
        println!("asked to be shown: {words}");
        if yes {
            taught = Some(*id);
            break;
        }
    }
    let teacher = taught.expect("one of the band would show the player how");
    let learned = (0..200).any(|_| {
        if let Some(v) = w.people.iter().find(|v| v.id == teacher) {
            let at = v.pos;
            w.go(at.x + 1.0, at.z);
        }
        w.run(40);
        w.knowledge.known.len() > known
    });
    assert!(learned, "kept beside the teacher, the player learns");
    println!(
        "knows now: {:?}",
        w.knowledge.known.keys().collect::<Vec<_>>()
    );

    // 2. A family. The player came of age at sixteen and men of its people pair from twenty:
    // five years pass first, the band about it seen anew.
    for _ in 0..5 {
        w.server.send(ToServer::SkipHours(32.0 * 24.0));
        w.run(40);
    }
    let at = w
        .people
        .iter()
        .filter(|v| !v.dead)
        .map(|v| v.pos)
        .min_by(|a, b| (*a - w.mover.pos).length().total_cmp(&(*b - w.mover.pos).length()));
    if let Some(at) = at {
        w.go(at.x, at.z);
        w.run(200);
    }
    band.clear();
    let near: Vec<u64> = w
        .people
        .iter()
        .filter(|v| !v.dead && format!("{:?}", v.stage) == "Adult")
        .map(|v| v.id)
        .collect();
    for id in near {
        let lines = regard(&mut w, id);
        if lines.first().is_some_and(|l| l.contains("your")) {
            band.push((id, lines));
        }
    }
    // A woman of the band, not kin and not paired, courted until she is willing.
    let woman = band
        .iter()
        .find(|(id, lines)| {
            // Not kin ("…, your sister"); of the band ("…, of your band").
            lines[0].ends_with("of your band")
                && !lines.iter().any(|l| l.contains("Paired"))
                && w.people.iter().any(|v| v.id == *id && v.female)
        })
        .map(|(id, _)| *id)
        .unwrap_or_else(|| {
            let women: Vec<&(u64, Vec<String>)> = band
                .iter()
                .filter(|(id, _)| w.people.iter().any(|v| v.id == *id && v.female))
                .collect();
            panic!("an unpaired woman of the band, not kin: {women:?}")
        });
    let mut paired = false;
    for round in 0..60 {
        let kind = if round % 2 == 0 { Ask::Praise } else { Ask::Thank };
        ask(&mut w, woman, kind);
        w.run(100);
        if round % 5 == 4 {
            let (yes, words) = ask(&mut w, woman, Ask::Pair);
            println!("asked to pair: {words}");
            if yes {
                paired = true;
                break;
            }
        }
    }
    assert!(paired, "she comes to be willing");
    let lines = regard(&mut w, woman);
    assert!(lines.iter().any(|l| l.contains("Your partner")), "{lines:?}");

    // 3. The years pass: their children.
    let mut children = Vec::new();
    for _ in 0..8 {
        w.server.send(ToServer::SkipHours(32.0 * 24.0));
        w.run(40);
        children = children_of(&mut w, woman);
        if !children.is_empty() {
            break;
        }
    }
    println!("their children: {children:?}");
    assert!(!children.is_empty(), "a child of theirs in eight years");
    // Twenty years more: the children grow up.
    for _ in 0..20 {
        w.server.send(ToServer::SkipHours(32.0 * 24.0));
        w.run(40);
    }
    children = children_of(&mut w, woman);

    // 4. The player dies, and goes on as their grown child.
    w.server.send(ToServer::Moved(Moved {
        mover: w.mover,
        landed: None,
        motion: Motion::Swimming,
        speed: 0.0,
        straining: false,
        immersion: 1.0,
        airless_s: 61.0,
        yaw: 0.0,
    }));
    let t0 = std::time::Instant::now();
    let mut story = None;
    while story.is_none() && t0.elapsed().as_secs() < 60 {
        w.server.send(ToServer::Run(1));
        while let Some(m) = w.server.poll() {
            if let ToClient::Story(s) = m {
                story = Some(*s);
            }
        }
    }
    let story = story.expect("the life told");
    println!("{:#?}", story.lines);
    let heir = story
        .others
        .iter()
        .find(|o| children.contains(&o.id) && !o.child)
        .cloned()
        .unwrap_or_else(|| panic!("a grown child to live on as: {:?}", story.others));
    println!("lives on as {}", heir.words);
    w.server.send(ToServer::Inhabit(heir.id));
    let t0 = std::time::Instant::now();
    let mut who = None;
    while who.is_none() && t0.elapsed().as_secs() < 30 {
        w.server.send(ToServer::Run(1));
        while let Some(m) = w.server.poll() {
            if let ToClient::WhoYouAre(lines) = m {
                who = Some(lines);
            }
        }
    }
    let who = who.expect("told who they are");
    println!("{who:#?}");
    assert!(
        who.iter().any(|l| l.contains("father") || l.contains("mother")),
        "the one lived on as knows its parents: {who:?}"
    );
}

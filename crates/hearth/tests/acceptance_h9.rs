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
    go_to_band_of(w, id);
    if !w.people.iter().any(|v| v.id == id) {
        return (false, String::new());
    }
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

/// Goes to where a person's band has its camp (the inspector's record of them), until they are
/// in sight.
fn go_to_band_of(w: &mut World, id: u64) {
    if w.people.iter().any(|v| v.id == id) {
        return;
    }
    w.inspected = None;
    w.server.send(ToServer::Inspect(Some(id)));
    w.run(40);
    w.until(10.0, |w| w.inspected.as_ref().is_some_and(|r| r.id == id));
    w.server.send(ToServer::Inspect(None));
    let report = w.inspected.clone();
    let line = |name: &str| {
        report
            .as_ref()
            .and_then(|r| r.sections.iter().find(|s| s.name == name))
            .map(|s| s.lines.join("; "))
            .unwrap_or_default()
    };
    if line("Life").contains("died day") {
        return;
    }
    let social = line("Social");
    let camp = social.split("camp at ").nth(1).and_then(|at| {
        let mut xz = at.split([',', ';']).map(|n| n.trim().parse::<f64>());
        Some((xz.next()?.ok()?, xz.next()?.ok()?))
    });
    if let Some((x, z)) = camp {
        w.go(x, z);
        w.until(30.0, |w| w.people.iter().any(|v| v.id == id));
    }
    if !w.people.iter().any(|v| v.id == id) {
        println!(
            "  #{id} not in sight at {:?} ({} about); camp {camp:?}; life: {}; social: {}",
            w.mover.pos,
            w.people.len(),
            line("Life").chars().take(300).collect::<String>(),
            social.chars().take(200).collect::<String>()
        );
    }
}

/// Whether a person is living and sixteen or more, by the inspector's record of its life.
fn grown_up(w: &mut World, id: u64) -> bool {
    w.inspected = None;
    w.server.send(ToServer::Inspect(Some(id)));
    w.run(40);
    w.until(10.0, |w| w.inspected.as_ref().is_some_and(|r| r.id == id));
    w.server.send(ToServer::Inspect(None));
    let Some(life) = w.inspected.as_ref().and_then(|r| {
        r.sections
            .iter()
            .find(|s| s.name == "Life")
            .map(|s| s.lines.join("; "))
    }) else {
        return false;
    };
    let born = life
        .split("born day ")
        .nth(1)
        .and_then(|b| b.split(';').next())
        .and_then(|b| b.trim().parse::<f64>().ok());
    let today = w.ticks as f64 / w.ticks_per_day;
    !life.contains("died day") && born.is_some_and(|b| (today - b) / 32.0 >= 16.0)
}

/// The children a woman bore, by the inspector's record of her life.
fn children_of(w: &mut World, mother: u64) -> Vec<u64> {
    w.inspected = None;
    w.server.send(ToServer::Inspect(Some(mother)));
    w.run(40);
    w.until(10.0, |w| {
        w.inspected.as_ref().is_some_and(|r| r.id == mother)
    });
    let Some(r) = w.inspected.clone() else {
        return Vec::new();
    };
    w.server.send(ToServer::Inspect(None));
    for s in r
        .sections
        .iter()
        .filter(|s| s.name == "Life" || s.name == "Social")
    {
        println!("  {}: {}", s.name, s.lines.join("; "));
    }
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

    // 2. A family. A girl or young woman of the band, not kin and not paired: courted now, and
    // the years let pass until both are of an age to pair (men of the player's people pair from
    // twenty, women from seventeen) — she, grown fond of the player, waiting for it.
    let mut girls: Vec<(u64, Vec<String>, bool)> = Vec::new();
    let females: Vec<(u64, bool)> = w
        .people
        .iter()
        .filter(|v| !v.dead && v.female)
        .map(|v| (v.id, format!("{:?}", v.stage) == "Adult"))
        .collect();
    for (id, grown) in females {
        let lines = regard(&mut w, id);
        if lines.first().is_some_and(|l| l.ends_with("of your band"))
            && !lines.iter().any(|l| l.contains("Paired"))
            && (!grown || lines.iter().any(|l| l == "Young."))
        {
            girls.push((id, lines, grown));
        }
    }
    println!("those the player might court: {girls:?}");
    // Young women first, then girls: three of them courted, kind words often each season, as one
    // living in the band would — ties left untended fade back a fifth a season.
    girls.sort_by_key(|g| !g.2);
    let courted: Vec<u64> = girls.iter().take(3).map(|g| g.0).collect();
    assert!(
        !courted.is_empty(),
        "a girl or young woman of the band, not kin"
    );
    let court = |w: &mut World, who: u64, rounds: usize| {
        for round in 0..rounds {
            let kind = if round % 2 == 0 {
                Ask::Praise
            } else {
                Ask::Thank
            };
            ask(w, who, kind);
            w.run(100);
        }
    };
    for &g in &courted {
        court(&mut w, g, 12);
    }
    let mut woman = None;
    'seasons: for season in 1..=40 {
        w.server.send(ToServer::SkipHours(8.0 * 24.0));
        w.run(40);
        for &g in &courted {
            court(&mut w, g, 8);
        }
        // Asked each season: "too young yet", "not yet willing" — the player's intent known,
        // the girl it courts waits for it — until the player is of an age (twenty) and she
        // willing.
        for &g in &courted {
            let (yes, words) = ask(&mut w, g, Ask::Pair);
            println!("season {season}, asked #{g} to pair: {words}");
            if yes {
                woman = Some(g);
                break 'seasons;
            }
        }
    }
    let paired = woman.is_some();
    let woman = woman.unwrap_or(courted[0]);
    assert!(paired, "she comes to be willing");
    let lines = regard(&mut w, woman);
    assert!(
        lines.iter().any(|l| l.contains("Your partner")),
        "{lines:?}"
    );

    // 3. The years pass: their children, until one of them is grown (sixteen; many die young,
    // as foragers' children do).
    let mut children: Vec<u64> = Vec::new();
    let mut grown = None;
    for year in 1..=35 {
        w.server.send(ToServer::SkipHours(32.0 * 24.0));
        w.run(40);
        for c in children_of(&mut w, woman) {
            if !children.contains(&c) {
                children.push(c);
            }
        }
        grown = children.iter().copied().find(|&c| grown_up(&mut w, c));
        println!("year {year}: their children {children:?}, grown {grown:?}");
        if grown.is_some() {
            break;
        }
    }
    assert!(!children.is_empty(), "children of theirs");
    let grown = grown.expect("a child of theirs grown");

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
        .find(|o| o.id == grown && !o.child)
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
        who.iter()
            .any(|l| l.contains("father") || l.contains("mother")),
        "the one lived on as knows its parents: {who:?}"
    );
}

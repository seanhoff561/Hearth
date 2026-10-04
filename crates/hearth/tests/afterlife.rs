//! H3 (V2.1 Addendum B §2, D182): a scripted player, grown in their family, dies; their death is
//! an event of the world and their life is told; they live on as one of their people — a
//! kinsman's body, what he knows and where he stands taken up whole — and are told who they are.

mod common;

use common::*;
use hearth_physics::Motion;
use hearth_protocol::{Moved, ToClient, ToServer};

#[test]
fn a_player_dies_reads_their_story_and_lives_on_as_a_kinsman() {
    let dir = temp("afterlife");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::default(), 7);
    // The family set down about the player.
    let t0 = std::time::Instant::now();
    while w.people.is_empty() && t0.elapsed().as_secs() < 90 {
        w.run(40);
    }
    assert!(!w.people.is_empty(), "the family is about");
    let before = w.appearance.clone().expect("how the player looks");
    // Drowned.
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
    // The life told, and who of their people they could live on as.
    let t0 = std::time::Instant::now();
    let mut story = None;
    while story.is_none() && t0.elapsed().as_secs() < 30 {
        w.server.send(ToServer::Run(1));
        while let Some(m) = w.server.poll() {
            if let ToClient::Story(s) = m {
                story = Some(*s);
            }
        }
    }
    let story = story.expect("the life told");
    println!("{:#?}", story);
    assert!(
        story.lines.iter().any(|l| l.contains("lived")),
        "how long they lived"
    );
    assert!(!story.others.is_empty(), "people to live on as");
    // Live on as the first of the grown among them.
    let first = story
        .others
        .iter()
        .find(|o| !o.child)
        .expect("a grown one")
        .clone();
    let (id, who) = (first.id, first.words);
    w.server.send(ToServer::Inhabit(id));
    let t0 = std::time::Instant::now();
    let mut briefing = None;
    while briefing.is_none() && t0.elapsed().as_secs() < 30 {
        w.server.send(ToServer::Run(1));
        while let Some(m) = w.server.poll() {
            match m {
                ToClient::WhoYouAre(lines) => briefing = Some(lines),
                ToClient::Person(a) => w.appearance = Some(a),
                ToClient::Body(b) => w.body = Some(*b),
                ToClient::Placed(m) => w.mover = m,
                _ => {}
            }
        }
    }
    let briefing = briefing.expect("who they are now");
    println!("as {who}: {briefing:#?}");
    assert!(briefing[0].starts_with("You are a"), "{briefing:?}");
    w.run(4);
    let body = w.body.clone().expect("the body");
    assert!(body.dead.is_none(), "alive again, as another");
    let after = w.appearance.clone().expect("how they look now");
    assert_ne!(after, before, "another body");
}

/// Drowns the player and waits for the life told.
fn drown(w: &mut World) -> hearth_protocol::Story {
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
    loop {
        assert!(t0.elapsed().as_secs() < 60, "the life told");
        w.server.send(ToServer::Run(1));
        while let Some(m) = w.server.poll() {
            match m {
                ToClient::Story(s) => return *s,
                ToClient::Body(b) => w.body = Some(*b),
                _ => {}
            }
        }
    }
}

#[test]
fn in_an_era_a_dead_player_is_born_again_and_lives_on_as_a_child() {
    // H8 (Addendum B §2.2): born again into one of the households offered about the place, and,
    // dead again, living on as a child of the band, whose childhood goes on from its age.
    let dir = temp("afterlife-era");
    let mut w = World::start_in(
        &dir,
        hearth_save::KnowledgeMode::default(),
        3,
        false,
        "hearth:upper_paleolithic",
        Some(0),
    );
    assert!(w.born.is_some(), "born into a household");
    w.run(81);
    w.until(30.0, |w| !w.people.is_empty());
    let first = w.born.take().expect("the first birth");
    // Dead, and born again where they died.
    drown(&mut w);
    w.births.clear();
    w.server.send(ToServer::BornAgain {
        at: None,
        female: Some(true),
    });
    assert!(
        w.until(60.0, |w| !w.births.is_empty()),
        "households offered to be born into"
    );
    println!("offered: {:#?}", w.births);
    w.server.send(ToServer::BeBorn {
        choice: 0,
        female: Some(true),
    });
    assert!(w.until(30.0, |w| w.born.is_some()), "born again");
    let again = w.born.clone().expect("the birth");
    assert_ne!(again.you, first.you, "another body");
    w.run(81);
    assert!(
        w.until(10.0, |w| w.body.as_ref().is_some_and(|b| b.dead.is_none())),
        "alive again"
    );
    // Dead again: among those to live on as, children of the band; live on as one.
    let story = drown(&mut w);
    let child = story
        .others
        .iter()
        .find(|o| o.child && o.group)
        .expect("a child of the band to live on as")
        .clone();
    println!("living on as {}", child.words);
    let tall = w.appearance.as_ref().map_or(0.0, |a| a.height_m);
    w.server.send(ToServer::Inhabit(child.id));
    w.run(41);
    assert!(
        w.until(30.0, |w| w.childhood.is_some()),
        "the child's childhood goes on"
    );
    let now = w.appearance.as_ref().map_or(0.0, |a| a.height_m);
    println!("{tall:.2} m as the grown one, {now:.2} m as the child");
    assert!(now < tall, "a child's body");
    drop(w);
    let _ = std::fs::remove_dir_all(&dir);
}

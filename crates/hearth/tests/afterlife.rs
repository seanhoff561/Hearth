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
    assert!(!story.kin.is_empty(), "kin to live on as");
    // Live on as the first of them.
    let (id, who) = story.kin[0].clone();
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

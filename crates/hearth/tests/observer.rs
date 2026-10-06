//! H9 (V2.1 §15.4, §16): watching the world and speaking with its people, through the server as
//! a client does. The player speaks to one of its family and is answered, and sees what it knows
//! of them; then it watches: put aside (its body still while the world goes on), the world
//! streamed about the eye far off, a person followed and their life read, the chronicle told; and
//! steps back in.

mod common;

use common::*;
use glam::DVec3;
use hearth_people::player::Ask;
use hearth_protocol::ToServer;

#[test]
fn the_player_speaks_with_its_people_then_watches_the_world_and_steps_back_in() {
    let dir = temp("observer");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::default(), 7);
    let t0 = std::time::Instant::now();
    while w.people.is_empty() && t0.elapsed().as_secs() < 90 {
        w.run(40);
    }
    let kin = w
        .people
        .iter()
        .filter(|v| !v.dead)
        .min_by(|a, b| {
            let d = |v: &hearth_people::PersonView| (v.pos - w.mover.pos).length();
            d(a).total_cmp(&d(b))
        })
        .map(|v| (v.id, v.pos))
        .expect("one of the family");
    // Near them: what the player knows of them, and a greeting answered.
    w.go(kin.1.x + 1.5, kin.1.z);
    w.server.send(ToServer::Regard(Some(kin.0)));
    w.run(20);
    assert!(w.until(10.0, |w| w.regarded.is_some()), "told who they are");
    let (_, lines) = w.regarded.clone().expect("what is known of them");
    println!("regarded: {lines:?}");
    assert!(lines[0].contains("your"), "one's own kin: {lines:?}");
    let n = w.acted.len();
    w.server.send(ToServer::Speak {
        person: kin.0,
        ask: Ask::Greet,
    });
    w.run(5);
    assert!(
        w.until(10.0, |w| w.acted.len() > n),
        "a greeting answered"
    );
    println!("answered: {:?}", w.acted[n..].to_vec());

    // Watching, two hundred metres off: the player put aside.
    let body = w.body.clone().expect("the body");
    let eye = w.mover.pos + DVec3::new(200.0, 20.0, 0.0);
    w.server.send(ToServer::Observe(Some(eye)));
    let feet = w.mover.pos;
    w.run(400);
    let still = w.body.clone().expect("the body");
    assert_eq!(w.mover.pos, feet, "the player stays where it was put aside");
    assert!(
        still.status.hunger == body.status.hunger && still.dead.is_none(),
        "its body still: {:?} → {:?}",
        body.status.hunger,
        still.status.hunger
    );
    // The world about the eye streamed.
    let about = hearth_math::BlockPos::containing(DVec3::new(eye.x, 0.0, eye.z));
    assert!(
        w.until(60.0, |w| (-64..128).any(|y| w
            .mirror
            .block(hearth_math::BlockPos::new(about.x, y, about.z))
            .is_some())),
        "the ground about the eye is there"
    );
    // A person followed: their life read.
    w.server.send(ToServer::Follow(Some(kin.0)));
    w.run(40);
    assert!(w.until(10.0, |w| w.life_of.is_some()), "a life told");
    let (_, life) = w.life_of.clone().expect("the life");
    println!("life: {life:?}");
    assert!(!life.is_empty());
    // The chronicle (Wild Earth has no deep time: the living world's only).
    w.server.send(ToServer::Chronicle);
    w.run(5);
    assert!(w.until(10.0, |w| w.chronicle.is_some()), "the chronicle told");
    // Faster: a day and a half of the world, its ticks taken some hundred and fifty at a step.
    let day0 = w.ticks;
    w.server
        .send(ToServer::TimeWarp(w.ticks_per_day / 20.0 - 20.0));
    let t = std::time::Instant::now();
    let ticks = (w.ticks_per_day * 1.5) as u64;
    w.run(ticks);
    let days = (w.ticks - day0) as f64 / w.ticks_per_day;
    println!(
        "watched {days:.1} days in {:.1} s",
        t.elapsed().as_secs_f64()
    );
    assert!(days > 1.0, "time went faster");
    // Stepping back in: the body lives on.
    w.server.send(ToServer::TimeWarp(0.0));
    w.server.send(ToServer::Observe(None));
    w.server.send(ToServer::Follow(None));
    w.run(200);
    assert!(w.body.as_ref().is_some_and(|b| b.dead.is_none()));
}

/// V2.1 §17.2: the Observer's fast-forward passes at least ten years of the world a second, at
/// the demographic tier, on the reference machine — measured here on an era's world in a
/// release build (`cargo test --release -p hearth --test observer -- --ignored`), the eye above
/// the player's band, the years taken half a year at a step (twenty steps a second).
#[test]
#[ignore]
fn the_observers_fast_forward_meets_its_budget() {
    let dir = temp("observer-fast");
    let mut w = World::start_in(
        &dir,
        hearth_save::KnowledgeMode::default(),
        3,
        false,
        "hearth:upper_paleolithic",
        Some(0),
    );
    let eye = w.mover.pos + DVec3::new(0.0, 60.0, 0.0);
    w.server.send(ToServer::Observe(Some(eye)));
    w.run(40);
    let year_ticks = w.ticks_per_day * 32.0;
    // Half a year a step: each tick warped by that many.
    w.server
        .send(ToServer::TimeWarp(year_ticks * 0.5 / 0.05));
    // Settle in a year, then measure ten.
    w.run(year_ticks as u64);
    let t0 = std::time::Instant::now();
    let from = w.ticks;
    w.run((year_ticks * 10.0) as u64);
    let years = (w.ticks - from) as f64 / year_ticks;
    let per_s = years / t0.elapsed().as_secs_f64();
    println!(
        "{years:.1} years in {:.1} s: {per_s:.1} years a second",
        t0.elapsed().as_secs_f64()
    );
    assert!(per_s >= 10.0, "{per_s:.1} years a second");
}

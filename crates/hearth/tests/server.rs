//! The integrated server headless: a world is created and saved, the player lands hard and is
//! hurt, and a restarted server brings back the player, their injuries and the clock.

use std::sync::Arc;
use std::time::{Duration, Instant};

use glam::DVec3;
use hearth::server::{Server, View, WorldSpec};
use hearth_physics::Motion;
use hearth_protocol::{BodyView, Moved, ToClient, ToServer};
use hearth_render::atlas::TextureArray;

fn spec(dir: &std::path::Path) -> WorldSpec {
    WorldSpec {
        name: "test".into(),
        seed: 11,
        planet: hearth_math::PlanetSize::Tiny,
        cache_dir: None,
        saves_dir: Some(dir.to_path_buf()),
        appearance: hearth_character::Appearance::default(),
        death_rules: hearth_save::DeathRules::default(),
    }
}

fn atlas() -> Arc<TextureArray> {
    Arc::new(TextureArray::from_entries(&hearth_texgen::textures_for(
        None,
    )))
}

/// Waits for a message the predicate picks, up to `secs`.
fn wait<T>(s: &Server, secs: f64, mut pick: impl FnMut(ToClient) -> Option<T>) -> T {
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs_f64(secs) {
        match s.poll() {
            Some(m) => {
                if let Some(v) = pick(m) {
                    return v;
                }
            }
            None => std::thread::sleep(Duration::from_millis(2)),
        }
    }
    panic!("timed out");
}

#[test]
fn a_world_lives_saves_and_comes_back() {
    let dir = std::env::temp_dir().join(format!("hearth-server-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let view = View {
        radius: 2,
        vertical: 2,
    };

    let server = Server::start(spec(&dir), atlas(), view);
    let ready = wait(&server, 120.0, |m| match m {
        ToClient::Ready(r) => Some(r),
        _ => None,
    });
    let start = ready.player;
    let tick0 = ready.ticks;
    assert_eq!(tick0, 0, "a new world starts at tick 0");
    // Report a hard landing a little away from the spawn.
    let mut moved = start;
    moved.pos += DVec3::new(2.0, 0.0, 1.0);
    server.send(ToServer::Moved(Moved {
        mover: moved,
        landed: Some(12.0),
        motion: Motion::Still,
        speed: 0.0,
        straining: false,
        immersion: 0.0,
        airless_s: 0.0,
    }));
    let body: BodyView = wait(&server, 10.0, |m| match m {
        ToClient::Body(b) if !b.injuries.is_empty() || b.dead.is_some() => Some(*b),
        _ => None,
    });
    println!(
        "after the landing: {:?}, {:?}",
        body.injuries.iter().map(|i| &i.id).collect::<Vec<_>>(),
        body.dead
    );
    // Let the clock run, then stop (which saves).
    let ticks = wait(&server, 10.0, |m| match m {
        ToClient::Clock(t) if t > 40 => Some(t),
        _ => None,
    });
    drop(server);
    assert!(dir.join("test").join("player.json").exists());

    let server = Server::start(spec(&dir), atlas(), view);
    let ready = wait(&server, 120.0, |m| match m {
        ToClient::Ready(r) => Some(r),
        _ => None,
    });
    assert!(
        ready.ticks >= ticks,
        "the clock came back: {} < {ticks}",
        ready.ticks
    );
    assert!(
        (ready.player.pos - moved.pos).length() < 1e-9,
        "where the player was"
    );
    let again: BodyView = wait(&server, 10.0, |m| match m {
        ToClient::Body(b) => Some(*b),
        _ => None,
    });
    assert_eq!(
        again.injuries.len(),
        body.injuries.len(),
        "the injuries came back"
    );
    assert_eq!(again.dead, body.dead);
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Drowns the player (a report of a minute without air).
fn drown(server: &Server, at: hearth_physics::Mover) {
    server.send(ToServer::Moved(Moved {
        mover: at,
        landed: None,
        motion: Motion::Swimming,
        speed: 0.0,
        straining: false,
        immersion: 1.0,
        airless_s: 61.0,
    }));
}

#[test]
fn death_follows_the_world_rules() {
    let view = View {
        radius: 2,
        vertical: 2,
    };
    // Legacy: someone new arrives near where the player died.
    let dir = std::env::temp_dir().join(format!("hearth-death-legacy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let server = Server::start(spec(&dir), atlas(), view);
    let ready = wait(&server, 120.0, |m| match m {
        ToClient::Ready(r) => Some(r),
        _ => None,
    });
    assert_eq!(ready.death_rules, hearth_save::DeathRules::Legacy);
    let mut there = ready.player;
    there.pos += DVec3::new(30.0, 0.0, 0.0);
    drown(&server, there);
    wait(&server, 10.0, |m| match m {
        ToClient::Body(b) if b.dead.is_some() => Some(()),
        _ => None,
    });
    let next = hearth_character::Appearance::female();
    server.send(ToServer::Respawn(Some(next.clone())));
    let who = wait(&server, 10.0, |m| match m {
        ToClient::Person(a) => Some(a),
        _ => None,
    });
    assert_eq!(who.body, next.body, "the new person is who was chosen");
    let placed = wait(&server, 10.0, |m| match m {
        ToClient::Placed(p) => Some(p),
        _ => None,
    });
    let d = (placed.pos - there.pos).truncate().length();
    assert!(d < 300.0, "near where the player fell: {d:.0} m away");
    let alive = wait(&server, 10.0, |m| match m {
        ToClient::Body(b) => Some(*b),
        _ => None,
    });
    assert!(alive.dead.is_none());
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);

    // Permadeath: the death ends the world with the life's tale, for good.
    let dir = std::env::temp_dir().join(format!("hearth-death-perma-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut s = spec(&dir);
    s.death_rules = hearth_save::DeathRules::Permadeath;
    let server = Server::start(s.clone(), atlas(), view);
    let ready = wait(&server, 120.0, |m| match m {
        ToClient::Ready(r) => Some(r),
        _ => None,
    });
    let mut walked = ready.player;
    walked.pos += DVec3::new(5.0, 0.0, 0.0);
    server.send(ToServer::Moved(Moved {
        mover: walked,
        landed: None,
        motion: Motion::Walking,
        speed: 1.4,
        straining: false,
        immersion: 0.0,
        airless_s: 0.0,
    }));
    drown(&server, walked);
    let tale = wait(&server, 10.0, |m| match m {
        ToClient::Ended(t) => Some(t),
        _ => None,
    });
    assert_eq!(tale.cause, hearth_body::Death::Drowning);
    assert!((tale.walked_km - 0.005).abs() < 0.002, "{}", tale.walked_km);
    // No living on.
    server.send(ToServer::Respawn(None));
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_millis(500) {
        if let Some(ToClient::Placed(_)) = server.poll() {
            panic!("permadeath let the player live on");
        }
    }
    drop(server);
    let server = Server::start(s, atlas(), view);
    let again = wait(&server, 120.0, |m| match m {
        ToClient::Ready(r) => Some(r),
        _ => None,
    });
    assert!(again.ended.is_some(), "the world stays ended");
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}

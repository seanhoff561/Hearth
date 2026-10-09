//! Birds over remains (Amendment T §2.1): the scavengers of a place circle over fresh remains,
//! come down a few at a time to feed, keep off them higher and wider while someone is near, and
//! go when they are sent off — each where its ring over the remains has it, so that the server's
//! birds and the client's far specks agree.

use std::sync::Arc;

use hearth_fauna::ecology::Ecology;
use hearth_fauna::flock::{self, Doing, Flock, LEAVE_S};
use hearth_fauna::habitat::{Uniform, temperate_wood};
use hearth_fauna::live::{Act, Footing, Ground, Live, Medium, Now};
use hearth_fauna::species::{Catalog, Species};

/// Flat ground at y = 64 everywhere within reach.
struct Flat;

impl Ground for Flat {
    fn footing(&self, _x: f64, _z: f64, _y: f64) -> Option<Footing> {
        Some(Footing::dry(64.0))
    }

    fn top(&self, x: f64, z: f64) -> Option<Footing> {
        self.footing(x, z, 64.0)
    }
}

fn wood() -> Ecology {
    let cat = Arc::new(Catalog::new(&hearth_content::Content::load_base()));
    let land = Uniform {
        habitat: temperate_wood(&cat),
        cells_around: 4096,
    };
    Ecology::new(cat, 7, 0.0, &land)
}

fn species<'a>(eco: &'a Ecology, id: &str) -> (u16, &'a Species) {
    let i = eco.catalog.index(id).unwrap_or_else(|| panic!("{id}"));
    (i as u16, &eco.catalog.species[i])
}

/// A flock over remains of `kg` at the origin, on ground at y = 64.
fn flock_of(eco: &Ecology, id: &str, kg: f32, risen: bool) -> Flock {
    let (i, sp) = species(eco, id);
    let key = Flock::key_of([0.0, 0.0], 1234.5);
    Flock {
        key,
        species: i,
        at: [0.0, 64.0, 0.0],
        count: flock::flock_size(sp, kg, key, 40),
        risen,
    }
}

#[test]
fn the_birds_that_gather_over_remains_are_the_scavengers() {
    let eco = wood();
    let names: Vec<&str> = eco
        .catalog
        .species
        .iter()
        .filter(|s| flock::scavenges(s))
        .map(|s| s.id.as_str())
        .collect();
    println!("scavengers: {names:?}");
    for id in [
        "common_raven",
        "carrion_crow",
        "white_backed_vulture",
        "andean_condor",
    ] {
        assert!(flock::scavenges(species(&eco, id).1), "{id} scavenges");
    }
    for id in ["red_deer", "gray_wolf", "european_robin"] {
        assert!(
            !flock::scavenges(species(&eco, id).1),
            "{id} does not gather over remains"
        );
    }
    // A few ravens to a deer; more vultures to an antelope.
    let ravens = flock_of(&eco, "common_raven", 90.0, false).count;
    assert!((2..=8).contains(&ravens), "{ravens} ravens to a deer");
    let vultures = flock_of(&eco, "white_backed_vulture", 120.0, false).count;
    assert!(vultures > ravens, "{vultures} vultures to an antelope");
}

#[test]
fn a_bird_keeps_to_its_ring_at_its_soaring_speed() {
    let eco = wood();
    let f = flock_of(&eco, "common_raven", 90.0, false);
    let (_, sp) = species(&eco, "common_raven");
    let cruise = sp.fly_m_s.unwrap() as f64;
    let kg = sp.mass_kg as f64;
    let dt = 0.05;
    for i in 0..f.count {
        let mut was = f.bird_at(i, 0.0, cruise, kg).0;
        let (mut lo, mut hi) = (f64::MAX, 0.0f64);
        for k in 1..4000 {
            let t = k as f64 * dt;
            let (p, yaw) = f.bird_at(i, t, cruise, kg);
            let v = (p - was) / dt;
            lo = lo.min(v.length());
            hi = hi.max(v.length());
            // It faces the way it goes.
            let way = (v.x.atan2(v.z) as f32 - yaw).rem_euclid(std::f32::consts::TAU);
            let off = way.min(std::f32::consts::TAU - way);
            assert!(
                off < 0.35,
                "bird {i} faces {off:.2} rad off its way at {t:.1} s"
            );
            // Over the remains, above the ground and within sight of them.
            let r = (p.x * p.x + p.z * p.z).sqrt();
            assert!(p.y > 64.0 + 12.0 && r < 120.0, "bird {i} at {p:?}");
            was = p;
        }
        assert!(
            lo > cruise * 0.4 && hi < cruise * 1.2,
            "bird {i} flies {lo:.1}–{hi:.1} m/s; it cruises at {cruise}"
        );
    }
}

/// Steps a flock of live birds for `seconds`, returning the most of them ever on the ground at
/// once and the live birds at the end.
fn watch(eco: &Ecology, f: Flock, seconds: f64, live: &mut Live) -> usize {
    let sp = eco.catalog.species[f.species as usize].clone();
    if live.animals.is_empty() {
        for i in 0..f.count {
            live.attend(&sp, f, i);
        }
    }
    let dt = 0.05f32;
    let mut most = 0;
    for _ in 0..(seconds / dt as f64) as usize {
        live.step(eco, &Flat, None, &Now::day(11.0), dt);
        let down = live
            .animals
            .iter()
            .filter(|a| a.medium == Medium::Ground)
            .count();
        most = most.max(down);
        for a in &live.animals {
            assert!(
                a.pos.y >= 64.0 - 0.01,
                "a bird under the ground: {:?}",
                a.pos
            );
        }
    }
    most
}

#[test]
fn they_come_down_a_few_at_a_time_to_feed() {
    let eco = wood();
    let f = flock_of(&eco, "common_raven", 90.0, false);
    let mut live = Live::new(7);
    let most = watch(&eco, f, 600.0, &mut live);
    println!("{} ravens; at most {most} on the ground at once", f.count);
    assert!(most >= 1, "some come down to the remains in ten minutes");
    assert!(
        most <= flock::feeders(f.count),
        "{most} on the ground of {}, at most {} in their turns",
        f.count,
        flock::feeders(f.count)
    );
    // Those down are at the remains, feeding or looking about.
    for a in live.animals.iter().filter(|a| a.medium == Medium::Ground) {
        assert!(a.pos.x.hypot(a.pos.z) < 3.0, "a feeder off the remains");
        assert!(matches!(a.act, Act::Graze | Act::Alert));
    }
}

#[test]
fn none_come_down_while_someone_is_near_and_those_down_rise() {
    let eco = wood();
    let mut f = flock_of(&eco, "common_raven", 90.0, false);
    let mut live = Live::new(7);
    // Until some are down.
    let mut waited = 0.0;
    while live.animals.iter().all(|a| a.medium != Medium::Ground) {
        watch(&eco, f, 10.0, &mut live);
        waited += 10.0;
        assert!(waited < 900.0, "none came down");
    }
    // Someone comes: the flock rises.
    f.risen = true;
    for a in &mut live.animals {
        if let Some(at) = a.attend.as_mut() {
            at.flock = f;
        }
    }
    watch(&eco, f, 20.0, &mut live);
    let most = watch(&eco, f, 600.0, &mut live);
    assert_eq!(most, 0, "none come down while someone is near");
    for a in &live.animals {
        let at = a.attend.expect("attending");
        assert_eq!(at.doing, Doing::Circling, "back on their rings");
        assert!(a.pos.y > 64.0 + 40.0, "higher, at {:.0} m", a.pos.y - 64.0);
    }
}

#[test]
fn sent_off_they_go_and_are_let_go() {
    let eco = wood();
    let f = flock_of(&eco, "common_raven", 90.0, false);
    let mut live = Live::new(7);
    watch(&eco, f, 120.0, &mut live);
    let n = live.animals.len();
    assert_eq!(n, f.count as usize);
    for a in &mut live.animals {
        flock::leave(a);
    }
    watch(&eco, f, LEAVE_S * 0.5, &mut live);
    assert_eq!(live.animals.len(), n, "still on their way");
    let off = live
        .animals
        .iter()
        .map(|a| a.pos.x.hypot(a.pos.z))
        .fold(f64::MAX, f64::min);
    assert!(off > 100.0, "on their way off, the nearest {off:.0} m off");
    watch(&eco, f, LEAVE_S * 0.6, &mut live);
    assert!(live.animals.is_empty(), "gone");
}

#[test]
fn the_server_and_the_client_place_a_bird_alike() {
    // The client draws a far flock's birds where `bird_at` puts them; a live bird circling is
    // where the same call has it, at the world's clock.
    let eco = wood();
    let f = flock_of(&eco, "common_raven", 90.0, false);
    let (_, sp) = species(&eco, "common_raven");
    let mut live = Live::new(7);
    live.world_s = 5000.0;
    live.attend(sp, f, 0);
    live.step(&eco, &Flat, None, &Now::day(11.0), 0.05);
    let a = &live.animals[0];
    let (p, _) = f.bird_at(
        0,
        live.world_s,
        sp.fly_m_s.unwrap() as f64,
        sp.mass_kg as f64,
    );
    assert!((a.pos - p).length() < 1e-9, "{:?} against {p:?}", a.pos);
    assert_eq!(a.medium, Medium::Air);
}

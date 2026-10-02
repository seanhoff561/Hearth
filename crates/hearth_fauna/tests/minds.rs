//! Minds (V2-7 (f), docs/design/fauna.md "Minds"): approaching from downwind gets a stalker
//! nearer than from upwind; crouched and slow nearer than running; a person standing still is
//! harder to see at night; the herd runs with the first of it to run; a calf follows its mother;
//! a roe deer freezes before it runs; a thirsty deer goes to the water to drink.

use std::sync::Arc;

use glam::{DVec2, DVec3};
use hearth_fauna::ecology::Ecology;
use hearth_fauna::habitat::{Uniform, temperate_wood};
use hearth_fauna::live::{Act, Footing, Ground, Live, Now, Stage};
use hearth_fauna::mind::{Air, Presence};
use hearth_fauna::species::Catalog;

/// Open level ground at 64, and a river across x 40..46 (its surface at 63.8).
struct Meadow;

impl Ground for Meadow {
    fn footing(&self, x: f64, _z: f64, _y: f64) -> Option<Footing> {
        if (40.0..46.0).contains(&x) {
            return Some(Footing {
                y: 62.8,
                water: true,
                depth: 1.0,
            });
        }
        Some(Footing::dry(64.0))
    }

    fn top(&self, x: f64, z: f64) -> Option<Footing> {
        self.footing(x, z, 64.0)
    }
}

fn eco() -> Ecology {
    let cat = Arc::new(Catalog::new(&hearth_content::Content::load_base()));
    let land = Uniform {
        habitat: temperate_wood(&cat),
        cells_around: 4096,
    };
    Ecology::new(cat, 7, 0.0, &land)
}

fn species(eco: &Ecology, id: &str) -> u16 {
    eco.catalog.index(id).expect(id) as u16
}

/// A person coming straight at a grazing red deer from 320 m along +x at `pace` m/s, sensed
/// as `how` of where they are; the distance at which the deer becomes aware of them.
fn noticed_at(how: fn(DVec3) -> Presence, pace: f64, now: Now) -> f64 {
    let eco = eco();
    let deer = species(&eco, "red_deer");
    let mut live = Live::new(11);
    let id = live.place(deer, Stage::Adult, true, DVec3::new(0.5, 64.0, 0.5), 0.0);
    let mut at = DVec3::new(320.5, 64.0, 0.5);
    for _ in 0..20_000 {
        let p = how(at);
        live.step(&eco, &Meadow, Some(&p), &now, 0.05);
        let a = live.animals.iter().find(|a| a.id == id).expect("the deer");
        if a.wary.aware() {
            return (at - a.pos).length();
        }
        at.x -= pace * 0.05;
        if at.x < 1.0 {
            break;
        }
    }
    0.0
}

fn windy(toward: DVec2) -> Now {
    Now {
        air: Air {
            wind: toward,
            wind_speed: 4.0,
            light: 1.0,
        },
        ..Now::day(0.4)
    }
}

#[test]
fn from_downwind_a_stalker_gets_nearer_than_from_upwind() {
    // The wind from the deer toward the stalker (+x) carries the scent away from the deer.
    let downwind = noticed_at(Presence::stalking, 0.5, windy(DVec2::X));
    let upwind = noticed_at(Presence::stalking, 0.5, windy(DVec2::NEG_X));
    println!("noticed at {downwind:.0} m from downwind, {upwind:.0} m from upwind");
    assert!(
        downwind > 0.0 && upwind > downwind * 1.8,
        "{downwind:.0} against {upwind:.0}"
    );
}

#[test]
fn crouched_and_slow_gets_nearer_than_running() {
    let calm = Now::day(0.4);
    let stalking = noticed_at(Presence::stalking, 0.5, calm);
    let running = noticed_at(Presence::running, 5.0, calm);
    println!("noticed at {stalking:.0} m stalking, {running:.0} m running");
    assert!(
        running > stalking * 2.0,
        "{stalking:.0} against {running:.0}"
    );
}

#[test]
fn at_night_a_person_standing_still_is_hard_to_see() {
    let eco = eco();
    let deer = species(&eco, "red_deer");
    let still = |at: DVec3| Presence {
        noise: 0.05,
        plain: 0.45,
        ..Presence::walking(at)
    };
    let aware_after = |light: f32| {
        let mut live = Live::new(12);
        let id = live.place(deer, Stage::Adult, true, DVec3::new(0.5, 64.0, 0.5), 0.0);
        let now = Now {
            air: Air {
                light,
                ..Air::calm_day()
            },
            ..Now::day(0.4)
        };
        let p = still(DVec3::new(0.5, 64.0, 90.5));
        let mut aware = false;
        for _ in 0..600 {
            live.step(&eco, &Meadow, Some(&p), &now, 0.05);
            aware |= live
                .animals
                .iter()
                .find(|a| a.id == id)
                .is_some_and(|a| a.wary.aware());
        }
        aware
    };
    assert!(aware_after(1.0), "seen by day at 90 m");
    assert!(!aware_after(0.04), "unseen in the dark at 90 m");
}

#[test]
fn the_herd_runs_with_the_first_of_it_to_run() {
    let eco = eco();
    let deer = species(&eco, "red_deer");
    let mut live = Live::new(13);
    let ids: Vec<u64> = (0..6)
        .map(|k| {
            live.place(
                deer,
                Stage::Adult,
                true,
                DVec3::new(k as f64 * 12.0 + 0.5, 64.0, 0.5),
                0.0,
            )
        })
        .collect();
    for a in live.animals.iter_mut() {
        a.group = Some(77);
    }
    // Someone runs at the herd's near end from the west.
    let mut at = DVec3::new(-250.0, 64.0, 0.5);
    let mut first = None;
    for t in 0..4000 {
        live.step(
            &eco,
            &Meadow,
            Some(&Presence::running(at)),
            &Now::day(0.4),
            0.05,
        );
        at.x += 5.0 * 0.05;
        let fleeing = live
            .animals
            .iter()
            .filter(|a| ids.contains(&a.id) && a.act == Act::Flee)
            .count();
        if fleeing > 0 && first.is_none() {
            first = Some(t);
        }
        if let Some(f) = first
            && t > f + 30
        {
            // A second and a half after the first ran, all of them are running.
            assert_eq!(fleeing, 6, "{fleeing} of 6 running");
            return;
        }
    }
    panic!("none ran");
}

#[test]
fn a_calf_follows_its_mother() {
    let eco = eco();
    let deer = species(&eco, "red_deer");
    let mut live = Live::new(14);
    let hind = live.place(deer, Stage::Adult, true, DVec3::new(0.5, 64.0, 0.5), 0.0);
    let calf = live.place(deer, Stage::Young, true, DVec3::new(2.5, 64.0, 0.5), 0.0);
    for a in live.animals.iter_mut() {
        if a.id == calf {
            a.mother = Some(hind);
            a.timer = 0.0;
        }
        if a.id == hind {
            // Away along the meadow.
            a.goal = Some(DVec2::new(0.5, 30.5));
            a.act = Act::Walk;
            a.timer = 60.0;
        }
    }
    let mut far = 0.0f64;
    for _ in 0..800 {
        live.step(&eco, &Meadow, None, &Now::day(0.4), 0.05);
        let h = live
            .animals
            .iter()
            .find(|a| a.id == hind)
            .expect("hind")
            .pos;
        let c = live
            .animals
            .iter()
            .find(|a| a.id == calf)
            .expect("calf")
            .pos;
        far = far.max(((h.x - c.x).powi(2) + (h.z - c.z).powi(2)).sqrt());
    }
    let h = live
        .animals
        .iter()
        .find(|a| a.id == hind)
        .expect("hind")
        .pos;
    assert!(h.z > 20.0, "the hind went: {h}");
    assert!(far < 12.0, "the calf kept within {far:.1} m");
}

#[test]
fn a_roe_deer_freezes_before_it_runs() {
    let eco = eco();
    let roe = species(&eco, "roe_deer");
    let mut live = Live::new(15);
    let id = live.place(roe, Stage::Adult, true, DVec3::new(0.5, 64.0, 0.5), 0.0);
    let mut at = DVec3::new(0.5, 64.0, 220.5);
    let (mut froze, mut ran) = (false, false);
    for _ in 0..6000 {
        live.step(
            &eco,
            &Meadow,
            Some(&Presence::walking(at)),
            &Now::day(0.4),
            0.05,
        );
        let a = live.animals.iter().find(|a| a.id == id).expect("the roe");
        if a.wary.aware() && a.act == Act::Alert && a.speed < 0.05 && !ran {
            froze = true;
        }
        if a.act == Act::Flee {
            ran = true;
            break;
        }
        at.z -= 1.3 * 0.05;
    }
    assert!(froze, "it never froze");
    assert!(ran, "it never ran");
}

#[test]
fn a_thirsty_deer_goes_to_drink() {
    let eco = eco();
    let deer = species(&eco, "red_deer");
    let mut live = Live::new(16);
    let id = live.place(deer, Stage::Adult, true, DVec3::new(10.5, 64.0, 0.5), 0.0);
    for a in live.animals.iter_mut() {
        a.thirst = 1.0;
        a.timer = 0.0;
    }
    let mut drank = false;
    for _ in 0..2400 {
        live.step(&eco, &Meadow, None, &Now::day(0.4), 0.05);
        let a = live.animals.iter().find(|a| a.id == id).expect("the deer");
        if a.act == Act::Drink {
            assert!(
                (38.0..41.0).contains(&a.pos.x),
                "drinking at the bank: {}",
                a.pos
            );
            drank = true;
            break;
        }
    }
    assert!(drank, "it never drank");
}

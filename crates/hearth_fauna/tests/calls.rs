//! Calls and the chorus (V2-7 (i), docs/design/fauna.md "Calls and ambient life"): a deer put to
//! flight barks, a hare struck screams; a wolf pack howls by night, the others taking up the
//! first's howl, and not by day; a stag roars in the autumn rut at dusk and not in spring; the
//! small birds of a wood sing most at dawn in spring, less at noon, hardly at all in winter and
//! not at night, when the owls hoot.

use std::sync::Arc;

use glam::DVec3;
use hearth_content::schema::fauna::{CallKind, CallWhen};
use hearth_fauna::ecology::Ecology;
use hearth_fauna::habitat::{Uniform, temperate_wood};
use hearth_fauna::live::{Footing, Ground, Live, Now, Stage};
use hearth_fauna::mind::{Air, Presence};
use hearth_fauna::species::Catalog;
use hearth_fauna::voices::{Called, chorus};
use hearth_fauna::wound::Blow;
use hearth_math::hash::Rng;

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
    let mut eco = Ecology::new(cat, 9, 0.0, &land);
    eco.ensure_region(&land, (3, 0), 0.0);
    eco
}

fn species(eco: &Ecology, id: &str) -> u16 {
    eco.catalog.index(id).expect(id) as u16
}

/// The kind and occasion of a call made.
fn what(eco: &Ecology, c: &Called) -> (CallKind, CallWhen) {
    let call = eco.catalog.species[c.species as usize].calls[c.call as usize];
    (call.kind, call.when)
}

/// A moment: the hour (0–1), the light, the year's fraction.
fn at(hour: f32, light: f32, year_frac: f32) -> Now {
    Now {
        air: Air {
            light,
            ..Air::calm_day()
        },
        year_frac,
        ..Now::day(hour)
    }
}

#[test]
fn a_deer_put_to_flight_barks_and_a_wounded_hare_screams() {
    let eco = wood();
    let deer = species(&eco, "red_deer");
    let mut alarms = 0;
    for seed in 0..10 {
        let mut live = Live::new(seed);
        live.place(deer, Stage::Adult, true, DVec3::new(0.5, 64.0, 0.5), 0.0);
        let person = Presence::walking(DVec3::new(0.5, 64.0, 30.5));
        for _ in 0..60 {
            live.step(&eco, &Flat, Some(&person), &at(0.4, 1.0, 0.3), 0.05);
        }
        alarms += live
            .calls
            .iter()
            .filter(|c| what(&eco, c).1 == CallWhen::Alarm)
            .count();
    }
    assert!(alarms >= 2, "{alarms} alarms from 10 deer put to flight");
    // A hare struck by a thrown stick, not killed outright.
    let hare = species(&eco, "brown_hare");
    let mut live = Live::new(3);
    let id = live.place(hare, Stage::Adult, true, DVec3::new(0.5, 64.0, 0.5), 0.0);
    let path = [DVec3::new(6.5, 64.12, 0.55), DVec3::new(-3.5, 64.12, 0.55)];
    let hit = live.hit_along(&eco.catalog, &path, 0.3).expect("a hit");
    assert_eq!(hit.animal, id);
    live.strike(
        &eco.catalog,
        &hit,
        &Blow {
            energy_j: 4.0,
            piercing: 0.3,
            ..Default::default()
        },
        "stick",
        path[0],
        0.3,
    );
    assert!(
        live.calls
            .iter()
            .any(|c| what(&eco, c) == (CallKind::Scream, CallWhen::Distress)),
        "{:?}",
        live.calls
    );
}

/// Calls made by `who` (placed at the origin as a group) over `hours` as they live them.
fn calls_over(eco: &Ecology, who: &[(u16, bool)], now: &Now, hours: f32) -> Vec<Called> {
    let mut live = Live::new(11);
    for (k, &(s, female)) in who.iter().enumerate() {
        let id = live.place(
            s,
            Stage::Adult,
            female,
            DVec3::new(k as f64 * 4.0 + 0.5, 64.0, 0.5),
            0.0,
        );
        for a in live.animals.iter_mut().filter(|a| a.id == id) {
            a.group = Some(1);
        }
    }
    let steps = (hours * 3600.0 / 0.5) as usize;
    for _ in 0..steps {
        live.step(eco, &Flat, None, now, 0.5);
    }
    live.calls.clone()
}

#[test]
fn a_pack_howls_by_night_together_and_not_by_day() {
    let eco = wood();
    let wolf = species(&eco, "gray_wolf");
    let pack = [(wolf, true), (wolf, false), (wolf, true)];
    let night = calls_over(&eco, &pack, &at(0.05, 0.04, 0.9), 2.0);
    let howls = night
        .iter()
        .filter(|c| what(&eco, c) == (CallKind::Howl, CallWhen::Territory))
        .count();
    println!("{howls} howls in two hours of a winter night");
    assert!(howls >= 3, "{howls}");
    // Taken up together: as many voices as wolves, about the same time.
    assert!(howls % 3 == 0 || howls >= 6, "{howls}");
    let day = calls_over(&eco, &pack, &at(0.5, 1.0, 0.9), 2.0);
    assert!(
        day.iter().all(|c| what(&eco, c).1 != CallWhen::Territory),
        "no howling by day"
    );
}

#[test]
fn a_stag_roars_in_the_autumn_rut_and_not_in_spring() {
    let eco = wood();
    let deer = species(&eco, "red_deer");
    let stag = [(deer, false)];
    let roars = |now: &Now| {
        calls_over(&eco, &stag, now, 0.25)
            .iter()
            .filter(|c| what(&eco, c).0 == CallKind::Roar)
            .count()
    };
    let autumn = roars(&at(0.8, 0.3, 0.55));
    let spring = roars(&at(0.8, 0.3, 0.05));
    println!("{autumn} roars in a quarter hour of an autumn dusk, {spring} in spring");
    assert!(autumn >= 5 && spring == 0);
}

#[test]
fn the_dawn_chorus_swells_in_spring_and_owls_hoot_at_night() {
    let eco = wood();
    let r = eco.regions.values().next().expect("a region");
    let centre = r.cell_centre(REGION_MID);
    let place = DVec3::new(centre[0], 64.0, centre[1]);
    let mut rng = Rng::new(5);
    let mut count = |now: &Now, kind: CallKind, minutes: usize| -> usize {
        let mut n = 0;
        for _ in 0..minutes * 60 {
            n += chorus(&eco, place, now, 1.0, &mut rng)
                .iter()
                .filter(|c| what(&eco, c).0 == kind)
                .count();
        }
        n
    };
    let dawn = count(&at(0.27, 0.6, 0.1), CallKind::Song, 1);
    let noon = count(&at(0.5, 1.0, 0.1), CallKind::Song, 1);
    let night = count(&at(0.0, 0.04, 0.1), CallKind::Song, 1);
    let winter = count(&at(0.3, 0.5, 0.85), CallKind::Song, 1);
    let hoots = count(&at(0.0, 0.04, 0.1), CallKind::Hoot, 60);
    println!(
        "songs a minute within 200 m: spring dawn {dawn}, noon {noon}, night {night}, winter dawn {winter}; owls' hoots in an hour of night {hoots}"
    );
    assert!(dawn > 3 * noon && noon > 0, "{dawn} {noon}");
    assert_eq!(night, 0);
    assert!(winter * 5 < dawn, "{winter} {dawn}");
    assert!(hoots > 0);
}

/// A cell near the middle of a region.
const REGION_MID: usize = 32 * 64 + 32;

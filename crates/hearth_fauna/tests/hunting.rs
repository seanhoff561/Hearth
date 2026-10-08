//! Hunting and wounds (V2-7 (h), docs/design/fauna.md "Hunting and wounds"): a spear behind a
//! deer's shoulder drops it within a minute and a couple of hundred metres; one in the belly
//! lets it run, lie up and die within the hour; a light one only wounds it, and it lives; a
//! wooden point does not reach an aurochs' vitals where a stone one does; a stone kills a hare
//! and only bruises a deer; a wounded boar close by turns on the person; a throw over an
//! animal's back misses it.

use std::sync::Arc;

use glam::DVec3;
use hearth_fauna::danger::Cause;
use hearth_fauna::ecology::Ecology;
use hearth_fauna::habitat::{Uniform, temperate_wood};
use hearth_fauna::live::{Act, Footing, Ground, Live, Now, Stage};
use hearth_fauna::mind::Presence;
use hearth_fauna::rig::Rig;
use hearth_fauna::species::Catalog;
use hearth_fauna::wound::{Blow, Part, wound};

/// Open level ground.
struct Flat;

impl Ground for Flat {
    fn footing(&self, _x: f64, _z: f64, _y: f64) -> Option<Footing> {
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

/// A straight flight from `from` through `to` and a metre beyond, in 20 cm steps.
fn flight(from: DVec3, to: DVec3) -> Vec<DVec3> {
    let d = to - from;
    let n = ((d.length() + 1.0) / 0.2).ceil() as usize;
    (0..=n)
        .map(|i| from + d.normalize() * (i as f64 * 0.2))
        .collect()
}

/// A grown animal of a species standing at the origin facing +z, and where its torso's middle
/// is (its chest a little ahead of that).
fn standing(eco: &Ecology, live: &mut Live, id: &str, female: bool) -> (u64, f64, f64) {
    let s = species(eco, id);
    let sp = &eco.catalog.species[s as usize];
    let rig = Rig::of(sp, !female);
    let a = live.place(s, Stage::Adult, female, DVec3::new(0.5, 64.0, 0.5), 0.0);
    (a, rig.torso_y as f64, rig.torso.z as f64)
}

/// Thrown from 12 m to its side at a point along its torso (`along`: −1 the rump, 1 the chest
/// end), at the torso's height: the blow struck.
fn throw_at(
    eco: &Ecology,
    live: &mut Live,
    id: &str,
    along: f64,
    blow: Blow,
) -> (u64, DVec3, hearth_fauna::live::Struck) {
    let (a, ty, tl) = standing(eco, live, id, true);
    let target = DVec3::new(0.5, 64.0 + ty, 0.5 + along * tl * 0.5);
    let from = target + DVec3::new(12.0, 0.0, 0.0);
    let hit = live
        .hit_along(&eco.catalog, &flight(from, target), 0.4)
        .expect("the throw strikes it");
    assert_eq!(hit.animal, a);
    let s = live
        .strike(&eco.catalog, &hit, &blow, "spear", from, 0.4)
        .expect("struck");
    (a, from, s)
}

/// Lets the world go on for up to `secs` with the person standing at `at`: when the animal
/// died (s) and how far from where it was struck, if it did; whether it lay down first.
fn until_dead(
    eco: &Ecology,
    live: &mut Live,
    a: u64,
    at: DVec3,
    secs: f32,
) -> Option<(f32, f64, bool)> {
    let mut t = 0.0;
    let mut lay = false;
    let dt = 0.1;
    while t < secs {
        live.step(eco, &Flat, Some(&Presence::walking(at)), &Now::day(0.4), dt);
        t += dt;
        let v = live.animals.iter().find(|x| x.id == a).expect("the animal");
        if v.act == Act::Rest {
            lay = true;
        }
        if v.dead {
            let d = (v.pos - DVec3::new(0.5, 64.0, 0.5)).length();
            return Some((t, d, lay));
        }
    }
    None
}

#[test]
fn a_spear_behind_the_shoulder_drops_a_deer_within_a_minute() {
    let eco = eco();
    let mut times = Vec::new();
    for seed in 0..6 {
        let mut live = Live::new(800 + seed);
        let (a, from, s) = throw_at(
            &eco,
            &mut live,
            "red_deer",
            0.6,
            Blow {
                energy_j: 190.0,
                piercing: 0.85,
                ..Default::default()
            },
        );
        assert_eq!(s.part, Part::Chest, "{}", s.words);
        assert!(s.deep, "{}", s.words);
        let (t, d, _) = until_dead(&eco, &mut live, a, from, 120.0).expect("it dies");
        println!("{}: dead after {t:.0} s, {d:.0} m off", s.words);
        assert!(t < 60.0 && (20.0..300.0).contains(&d), "{t} s, {d} m");
        times.push(t);
        let bodies = live.take_bodies(&eco.catalog);
        assert!(bodies[0].by_person && bodies[0].killed_by.is_none());
        assert_eq!(bodies[0].left, 1.0);
    }
}

#[test]
fn a_deer_struck_in_the_belly_lies_up_and_dies_within_the_hour() {
    let eco = eco();
    let mut live = Live::new(900);
    let (a, from, s) = throw_at(
        &eco,
        &mut live,
        "red_deer",
        0.0,
        Blow {
            energy_j: 190.0,
            piercing: 0.85,
            ..Default::default()
        },
    );
    assert_eq!(s.part, Part::Belly, "{}", s.words);
    let (t, d, lay) = until_dead(&eco, &mut live, a, from, 3600.0).expect("it dies");
    println!("gut-struck: dead after {:.0} min, {d:.0} m off", t / 60.0);
    assert!(t > 120.0 && t < 3600.0, "{t} s");
    assert!(lay, "it lay up before it died");
}

#[test]
fn a_light_wound_heals_and_the_deer_lives() {
    let eco = eco();
    let mut live = Live::new(901);
    let (a, from, s) = throw_at(
        &eco,
        &mut live,
        "red_deer",
        -0.7,
        Blow {
            energy_j: 30.0,
            piercing: 0.4,
            ..Default::default()
        },
    );
    assert_eq!(s.part, Part::Haunch, "{}", s.words);
    assert!(!s.deep, "{}", s.words);
    assert!(
        until_dead(&eco, &mut live, a, from, 900.0).is_none(),
        "it lives"
    );
    let v = live.animals.iter().find(|x| x.id == a).expect("deer");
    assert!(v.hurt.lost > 0.02 && v.hurt.lost < 0.35, "{}", v.hurt.lost);
}

#[test]
fn a_wooden_point_does_not_reach_an_aurochs_vitals_where_stone_does() {
    let eco = eco();
    let sp = &eco.catalog.species[species(&eco, "aurochs") as usize];
    let rig = Rig::of(sp, false);
    let mass = rig.mass;
    let wooden = wound(
        &rig,
        1.0,
        mass,
        Part::Chest,
        &Blow {
            energy_j: 190.0,
            piercing: 0.4,
            ..Default::default()
        },
        0.5,
    );
    let stone = wound(
        &rig,
        1.0,
        mass,
        Part::Chest,
        &Blow {
            energy_j: 190.0,
            piercing: 0.85,
            ..Default::default()
        },
        0.5,
    );
    assert!(!wooden.deep && stone.deep);
    // The same wooden point does reach a roe deer's.
    let roe = &eco.catalog.species[species(&eco, "roe_deer") as usize];
    let r = Rig::of(roe, false);
    assert!(
        wound(
            &r,
            1.0,
            r.mass,
            Part::Chest,
            &Blow {
                energy_j: 190.0,
                piercing: 0.4,
                ..Default::default()
            },
            0.5
        )
        .deep
    );
}

#[test]
fn a_stone_kills_a_hare_and_only_bruises_a_deer() {
    let eco = eco();
    let stone = Blow {
        energy_j: 140.0,
        piercing: 0.0,
        ..Default::default()
    };
    let hare = &eco.catalog.species[species(&eco, "brown_hare") as usize];
    let r = Rig::of(hare, false);
    assert!(wound(&r, 1.0, r.mass, Part::Chest, &stone, 0.5).killed);
    let deer = &eco.catalog.species[species(&eco, "red_deer") as usize];
    let r = Rig::of(deer, false);
    let w = wound(&r, 1.0, r.mass, Part::Chest, &stone, 0.5);
    assert!(!w.killed && w.bleeding == 0.0);
}

#[test]
fn a_wounded_boar_close_by_turns_on_the_person() {
    let eco = eco();
    let mut provoked = 0;
    for seed in 0..12 {
        let mut live = Live::new(1000 + seed);
        let (a, _, _) = throw_at(
            &eco,
            &mut live,
            "wild_boar",
            -0.7,
            Blow {
                energy_j: 30.0,
                piercing: 0.4,
                ..Default::default()
            },
        );
        // The person comes up on it, five metres off.
        let at = DVec3::new(5.5, 64.0, 0.5);
        for _ in 0..200 {
            live.step(
                &eco,
                &Flat,
                Some(&Presence::walking(at)),
                &Now::day(0.4),
                0.05,
            );
            if live
                .attacks
                .iter()
                .any(|x| x.animal == a && x.cause == Cause::Provoked)
            {
                provoked += 1;
                break;
            }
        }
    }
    assert!(provoked >= 3, "{provoked} of 12 wounded boars turned");
}

#[test]
fn a_throw_over_its_back_misses_and_one_low_strikes_a_leg() {
    let eco = eco();
    let mut live = Live::new(1100);
    let (_, ty, _) = standing(&eco, &mut live, "red_deer", true);
    let over = DVec3::new(0.5, 64.0 + ty * 2.2, 0.5);
    assert!(
        live.hit_along(
            &eco.catalog,
            &flight(over + DVec3::new(12.0, 0.0, 0.0), over),
            0.4
        )
        .is_none()
    );
    let low = DVec3::new(0.5, 64.3, 0.6);
    let hit = live
        .hit_along(
            &eco.catalog,
            &flight(low + DVec3::new(12.0, 0.0, 0.0), low),
            0.4,
        )
        .expect("a hit");
    assert_eq!(hit.part, Part::Leg);
}

/// Level ground that takes prints as plainly as given (fresh snow 1, grass hardly).
struct Takes(f32);

impl Ground for Takes {
    fn footing(&self, _x: f64, _z: f64, _y: f64) -> Option<Footing> {
        Some(Footing::dry(64.0))
    }

    fn top(&self, x: f64, z: f64) -> Option<Footing> {
        self.footing(x, z, 64.0)
    }

    fn sign_surface(&self, _x: f64, _z: f64, y: f64) -> (f32, f64) {
        (self.0, y)
    }
}

/// A red deer put to flight by a person 30 m off, for `secs`: the signs it left.
fn deer_running(ground: &dyn Ground, wounded: bool, secs: f32) -> Vec<hearth_fauna::live::Sign> {
    let eco = eco();
    let mut live = Live::new(1200);
    let a = live.place(
        species(&eco, "red_deer"),
        Stage::Adult,
        true,
        DVec3::new(0.5, 64.0, 0.5),
        0.0,
    );
    if wounded {
        for x in live.animals.iter_mut().filter(|x| x.id == a) {
            x.hurt.clotting = 0.002;
        }
    }
    let person = Presence::walking(DVec3::new(0.5, 64.0, -29.5));
    let mut t = 0.0;
    while t < secs {
        live.step(&eco, ground, Some(&person), &Now::day(0.4), 0.05);
        t += 0.05;
    }
    live.signs.clone()
}

#[test]
fn a_deer_leaves_prints_in_snow_none_in_grass_and_a_wounded_one_blood() {
    use hearth_fauna::live::SignKind;
    let prints =
        |s: &[hearth_fauna::live::Sign]| s.iter().filter(|x| x.kind == SignKind::Print).count();
    let snow = deer_running(&Takes(1.0), false, 8.0);
    println!("{} prints in snow over 8 s", prints(&snow));
    assert!(prints(&snow) >= 6, "{}", prints(&snow));
    // In order along its way, a stride apart, on either side of its line.
    assert!(snow.windows(2).all(|w| w[1].t >= w[0].t));
    assert!(prints(&deer_running(&Takes(0.2), false, 8.0)) == 0);
    let blood = deer_running(&Takes(0.2), true, 8.0)
        .iter()
        .filter(|x| x.kind == SignKind::Blood)
        .count();
    assert!(blood >= 10, "{blood} drops of blood");
}

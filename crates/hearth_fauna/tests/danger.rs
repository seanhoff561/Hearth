//! Danger (V2-7 (g), docs/design/fauna.md "Danger"): animals turn on a person only for their
//! reasons — a bear whose cub the person comes near charges (more often a bluff when faced
//! down); wolves stalk a person who seems small and alone at night at the hungry end of winter,
//! but not one by a fire; a boar come upon too close goes for them; an adder stepped near bites
//! with its venom; a lynx ambushes a roe deer; in a tranquil world they seldom turn — and every
//! attack says why.

use std::sync::Arc;

use glam::DVec3;
use hearth_fauna::danger::Cause;
use hearth_fauna::ecology::Ecology;
use hearth_fauna::habitat::{Uniform, temperate_wood};
use hearth_fauna::live::{Act, Footing, Ground, Live, Now, Stage};
use hearth_fauna::mind::{Air, Presence};
use hearth_fauna::species::Catalog;

/// Level ground grown over with plants up to two metres.
struct Thicket;

impl Ground for Thicket {
    fn footing(&self, _x: f64, _z: f64, _y: f64) -> Option<Footing> {
        Some(Footing::dry(64.0))
    }

    fn top(&self, x: f64, z: f64) -> Option<Footing> {
        self.footing(x, z, 64.0)
    }

    fn cell(&self, _x: i32, y: i32, _z: i32) -> Option<hearth_fauna::live::Cell> {
        use hearth_fauna::live::Cell;
        Some(match y {
            ..64 => Cell::Solid,
            64..66 => Cell::Plant,
            _ => Cell::Open,
        })
    }
}

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

/// What a bear with her cub does as a person comes upon them sixteen metres off (facing them,
/// and shouting if `face_down`): the number of charges that closed and that stopped short,
/// over many encounters.
fn bear_encounters(face_down: bool, aggression: f32) -> (usize, usize) {
    let eco = eco();
    let bear = species(&eco, "brown_bear");
    let (mut closed, mut bluffs) = (0, 0);
    for seed in 0..40 {
        let mut live = Live::new(100 + seed);
        live.aggression = aggression;
        let sow = live.place(bear, Stage::Adult, true, DVec3::new(0.5, 64.0, 0.5), 0.0);
        let cub = live.place(bear, Stage::Young, true, DVec3::new(2.5, 64.0, 0.5), 0.0);
        for a in live.animals.iter_mut() {
            if a.id == cub {
                a.mother = Some(sow);
            }
            a.timer = 30.0;
        }
        // Come upon them round a bend, sixteen metres off.
        let mut at = DVec3::new(0.5, 64.0, 16.5);
        for _ in 0..1200 {
            let mut p = Presence::walking(at);
            // Facing the bears (toward -z).
            p.facing = std::f32::consts::PI;
            p.shouting = face_down;
            live.step(&eco, &Flat, Some(&p), &Now::day(0.4), 0.05);
            for a in &live.attacks {
                assert_eq!(a.cause, Cause::DefendingYoung, "{a:?}");
                assert!(
                    a.words.contains(Cause::DefendingYoung.words()),
                    "{}",
                    a.words
                );
                if a.injury.is_empty() {
                    bluffs += 1;
                } else {
                    closed += 1;
                }
            }
            if !live.attacks.is_empty() {
                break;
            }
            // Coming on a step or two, and standing there.
            if at.z > 14.0 {
                at.z -= 1.3 * 0.05;
            }
        }
    }
    (closed, bluffs)
}

#[test]
fn a_bear_defends_her_cub_and_faced_down_she_bluffs_more() {
    let (closed, bluffs) = bear_encounters(false, 1.0);
    let (closed_faced, bluffs_faced) = bear_encounters(true, 1.0);
    println!(
        "coming on: {closed} closed, {bluffs} stopped short; facing her down: {closed_faced} closed, {bluffs_faced} stopped short"
    );
    assert!(closed + bluffs >= 20, "she charged {}", closed + bluffs);
    assert!(bluffs > 0 && closed > 0);
    // Faced down, she more often turns away: fewer of her charges close.
    assert!(closed_faced < closed, "{closed_faced} against {closed}");
    // In a tranquil world she seldom does.
    let (calm_closed, calm_bluffs) = bear_encounters(false, 0.15);
    assert!(
        (calm_closed + calm_bluffs) * 3 < closed + bluffs,
        "{} against {}",
        calm_closed + calm_bluffs,
        closed + bluffs
    );
}

/// Late winter, a dark night: wolves and a person crouched among them at 25 m.
fn wolf_night(by_fire: bool) -> Vec<Cause> {
    let eco = eco();
    let wolf = species(&eco, "gray_wolf");
    let mut causes = Vec::new();
    for seed in 0..12 {
        let mut live = Live::new(200 + seed);
        for k in 0..3 {
            live.place(
                wolf,
                Stage::Adult,
                k % 2 == 0,
                DVec3::new(k as f64 * 3.0 + 0.5, 64.0, 0.5),
                0.0,
            );
        }
        let now = Now {
            air: Air {
                light: 0.04,
                ..Air::calm_day()
            },
            year_frac: 0.97,
            ..Now::day(0.95)
        };
        let mut p = Presence::stalking(DVec3::new(0.5, 64.0, 25.5));
        p.vulnerable = 0.85;
        p.by_fire = by_fire;
        for _ in 0..2400 {
            live.step(&eco, &Flat, Some(&p), &now, 0.05);
            causes.extend(live.attacks.iter().map(|a| a.cause));
        }
    }
    causes
}

#[test]
fn hungry_wolves_go_for_a_lone_small_person_at_night_but_not_by_a_fire() {
    let dark = wolf_night(false);
    assert!(!dark.is_empty(), "no attack");
    assert!(dark.iter().all(|c| *c == Cause::Hunger), "{dark:?}");
    let fire = wolf_night(true);
    assert!(
        fire.iter().all(|c| *c != Cause::Hunger),
        "by a fire: {fire:?}"
    );
}

#[test]
fn a_boar_come_upon_close_goes_for_the_person() {
    let eco = eco();
    let boar = species(&eco, "wild_boar");
    let mut causes = Vec::new();
    for seed in 0..20 {
        let mut live = Live::new(300 + seed);
        live.place(boar, Stage::Adult, false, DVec3::new(0.5, 64.0, 0.5), 0.0);
        // Out of nowhere, two metres off.
        let p = Presence::walking(DVec3::new(0.5, 64.0, 2.5));
        for _ in 0..100 {
            live.step(&eco, &Flat, Some(&p), &Now::day(0.4), 0.05);
            causes.extend(
                live.attacks
                    .iter()
                    .filter(|a| !a.injury.is_empty())
                    .map(|a| a.cause),
            );
        }
    }
    assert!(causes.len() >= 5, "{} of 20 attacked", causes.len());
    assert!(causes.iter().all(|c| *c == Cause::Surprise), "{causes:?}");
}

#[test]
fn an_adder_stepped_near_bites() {
    let eco = eco();
    let adder = species(&eco, "common_european_adder");
    let mut bites = Vec::new();
    for seed in 0..10 {
        let mut live = Live::new(400 + seed);
        live.place(adder, Stage::Adult, true, DVec3::new(0.5, 64.0, 0.5), 0.0);
        // Walking along over it.
        let mut at = DVec3::new(0.5, 64.0, 3.5);
        for _ in 0..120 {
            live.step(
                &eco,
                &Flat,
                Some(&Presence::walking(at)),
                &Now::day(0.4),
                0.05,
            );
            bites.extend(
                live.attacks
                    .iter()
                    .filter(|a| !a.injury.is_empty())
                    .cloned(),
            );
            at.z -= 1.3 * 0.05;
        }
    }
    assert!(!bites.is_empty(), "no bite");
    for b in &bites {
        assert_eq!(b.cause, Cause::SteppedNear);
        assert!(b.venom, "venomous");
        assert_eq!(b.injury, "bite");
    }
}

#[test]
fn a_lynx_ambushes_a_roe_deer() {
    let eco = eco();
    let lynx = species(&eco, "eurasian_lynx");
    let roe = species(&eco, "roe_deer");
    let mut kills = 0;
    for seed in 0..10 {
        let mut live = Live::new(500 + seed);
        let cat = live.place(lynx, Stage::Adult, true, DVec3::new(0.5, 64.0, -30.5), 0.0);
        live.place(roe, Stage::Adult, true, DVec3::new(0.5, 64.0, 0.5), 0.0);
        // Hunting it, from behind the roe, downwind of it.
        for a in live.animals.iter_mut() {
            if a.id == cat {
                a.timer = 0.0;
            }
        }
        let now = Now {
            air: Air {
                wind: glam::DVec2::new(0.0, -1.0),
                wind_speed: 3.0,
                light: 1.0,
            },
            ..Now::day(0.3)
        };
        for _ in 0..2400 {
            live.step(&eco, &Thicket, None, &now, 0.05);
            for k in &live.kills {
                assert_eq!(k.cause, Cause::Hunger);
                assert_eq!(k.predator, lynx);
                kills += 1;
            }
            if live.animals.iter().any(|a| a.dead) {
                break;
            }
        }
    }
    assert!(kills >= 2, "{kills} kills in 10 ambushes");
}

#[test]
fn a_charge_ends_when_the_person_backs_off() {
    let eco = eco();
    let bear = species(&eco, "brown_bear");
    let mut live = Live::new(600);
    let sow = live.place(bear, Stage::Adult, true, DVec3::new(0.5, 64.0, 0.5), 0.0);
    let cub = live.place(bear, Stage::Young, true, DVec3::new(2.5, 64.0, 0.5), 0.0);
    for a in live.animals.iter_mut() {
        if a.id == cub {
            a.mother = Some(sow);
        }
    }
    let mut at = DVec3::new(0.5, 64.0, 20.5);
    let mut charged = false;
    for _ in 0..2000 {
        live.step(
            &eco,
            &Flat,
            Some(&Presence::walking(at)),
            &Now::day(0.4),
            0.05,
        );
        let s = live.animals.iter().find(|a| a.id == sow).expect("the sow");
        if s.hostile.is_some() {
            charged = true;
            // Backing away fast.
            at.z += 6.0 * 0.05;
        }
        if charged && s.hostile.is_none() {
            assert_ne!(s.act, Act::Attack);
            return;
        }
    }
    assert!(!charged, "a charge that never ended");
}

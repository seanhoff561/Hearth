//! A body moving through small worlds: falls injure, sprints tire, injuries slow, water drowns
//! and chills.

use std::sync::OnceLock;

use glam::{DVec2, DVec3};
use hearth_body::{BodyConfig, Death, Exposure, Rates, Side, Worn};
use hearth_content::Content;
use hearth_content::schema::body::BodyRegion;
use hearth_content::time::TimeScales;
use hearth_physics::testing::{Cell, Grid};
use hearth_physics::{Gait, Intent, Motion};
use hearth_player::Player;

fn config() -> BodyConfig {
    static C: OnceLock<Content> = OnceLock::new();
    let c = C.get_or_init(Content::load_base);
    BodyConfig::with_rates(c, Rates::authentic(), TimeScales::defaults(&c.time))
}

const DT: f64 = 0.05;

fn east(gait: Gait) -> Intent {
    Intent {
        wish: DVec2::new(1.0, 0.0),
        gait,
        ..Intent::default()
    }
}

fn live(
    p: &mut Player,
    cfg: &BodyConfig,
    g: &Grid,
    i: &Intent,
    secs: f64,
) -> Vec<hearth_physics::Report> {
    let mild = Exposure {
        air_c: 22.0,
        ..Exposure::mild()
    };
    (0..(secs / DT) as usize)
        .map(|_| p.tick(cfg, g, i, &mild, &Worn::naked(), DT))
        .collect()
}

#[test]
fn walking_off_a_cliff_hurts_and_a_high_one_kills() {
    let cfg = config();
    let outcome = |height: i32, seed: u64| {
        let mut g = Grid::default();
        g.fill((-10, -1, -10), (2, -1, 10), Cell::Solid);
        g.fill((3, -1 - height, -10), (30, -1 - height, 10), Cell::Solid);
        let mut p = Player::new(&cfg, DVec3::new(0.5, 0.0, 0.5), seed);
        live(&mut p, &cfg, &g, &east(Gait::Walk), 8.0);
        p
    };
    let mut hurt = 0;
    for seed in 0..40 {
        let p = outcome(4, seed);
        assert!(p.mover.pos.y < -3.9, "fell: {:?}", p.mover.pos);
        if !p.body.injuries.is_empty() {
            hurt += 1;
        }
    }
    assert!(hurt > 25, "a 4 m fall hurts most: {hurt} of 40");
    let dead = (0..40)
        .filter(|&s| outcome(30, s).body.dead == Some(Death::Injury("fall".into())))
        .count();
    assert_eq!(dead, 40, "30 m kills");
}

#[test]
fn a_sprint_spends_stamina_and_drops_to_a_jog() {
    let cfg = config();
    let g = Grid::floor(200);
    let mut p = Player::new(&cfg, DVec3::new(0.5, 0.0, 0.5), 1);
    let r = live(&mut p, &cfg, &g, &east(Gait::Sprint), 30.0);
    let sprinting = r.iter().filter(|r| r.motion == Motion::Sprinting).count() as f64 * DT;
    println!("sprinted {sprinting:.1} s, stamina {:.2}", p.body.stamina);
    assert!((10.0..=25.0).contains(&sprinting), "{sprinting:.1} s");
    assert_eq!(r.last().expect("steps").motion, Motion::Jogging);
    // Rest brings it back.
    live(&mut p, &cfg, &g, &Intent::default(), 40.0);
    assert!(p.body.stamina > 0.9);
}

#[test]
fn a_broken_leg_slows_and_grounds() {
    let cfg = config();
    let g = Grid::floor(100);
    let mut p = Player::new(&cfg, DVec3::new(0.5, 0.0, 0.5), 2);
    p.body
        .injure(&cfg, "fracture", BodyRegion::LowerLeg, Side::Left, 0.6)
        .expect("leg");
    let r = live(&mut p, &cfg, &g, &east(Gait::Sprint), 5.0);
    let speed = r.last().expect("steps").speed;
    println!("with a broken leg: {speed:.2} m/s");
    assert!(speed < 0.8, "{speed}");
    let jump = Intent {
        jump: true,
        ..Intent::default()
    };
    let y0 = p.mover.pos.y;
    live(&mut p, &cfg, &g, &jump, 1.0);
    assert!((p.mover.pos.y - y0).abs() < 1e-6, "no jumping");
}

#[test]
fn a_diver_who_stays_down_drowns() {
    let cfg = config();
    let mut g = Grid::floor(20);
    g.fill((-20, -10, -20), (20, -10, 20), Cell::Solid);
    g.fill((-20, -9, -20), (20, -1, 20), Cell::Water);
    let mut p = Player::new(&cfg, DVec3::new(0.5, -1.0, 0.5), 3);
    let dive = Intent {
        descend: true,
        ..Intent::default()
    };
    let r = live(&mut p, &cfg, &g, &dive, 150.0);
    let dead_at = r.iter().position(|r| r.airless_s >= hearth_player::DROWN_S);
    println!("drowned after {:.0} s", dead_at.unwrap_or(0) as f64 * DT);
    assert_eq!(p.body.dead, Some(Death::Drowning));
    assert!(dead_at.is_some_and(|i| (90.0..130.0).contains(&(i as f64 * DT))));
}

#[test]
fn swimming_in_cold_water_chills() {
    let cfg = config();
    let mut g = Grid::floor(20);
    g.fill((-20, -6, -20), (20, -6, 20), Cell::Solid);
    g.fill((-20, -5, -20), (20, -1, 20), Cell::Water);
    let mut p = Player::new(&cfg, DVec3::new(0.5, -1.0, 0.5), 4);
    let sea = Exposure {
        air_c: 10.0,
        water_c: 8.0,
        ..Exposure::mild()
    };
    // Ten minutes of play is five hours of the body's time: swimming in 8 °C water.
    for _ in 0..(600.0 / DT) as usize {
        p.tick(&cfg, &g, &Intent::default(), &sea, &Worn::naked(), DT);
        if p.body.dead.is_some() {
            break;
        }
    }
    println!("core {:.2} °C, {:?}", p.body.thermal.core_c, p.body.dead);
    assert!(p.body.thermal.core_c < 35.0 || p.body.dead == Some(Death::Hypothermia));
}

#[test]
fn a_tired_body_sleeps_till_rested_and_the_cold_wakes_it() {
    let cfg = config();
    let sleeping = cfg.activity("sleeping");
    // Lying naked in a warm night (about the warmth a bare body needs at rest).
    let night = |air_c: f32| Exposure {
        air_c,
        humidity: 0.5,
        wind_m_s: 0.2,
        sky_c_offset: 0.0,
        local_hour: 23.0,
        ..Exposure::mild()
    };
    let sleep = |air_c: f32| {
        let e = night(air_c);
        let mut p = Player::new(&cfg, DVec3::ZERO, 1);
        p.body.sleep.pressure = 0.7;
        p.lying = true;
        for _ in 0..(6.0 / DT) as usize {
            assert!(p.rest(&cfg, &e, 23.0, DT).is_none());
        }
        assert!(p.asleep, "a tired body at ease drops off within seconds");
        // Asleep: a second of play is half a minute of the body's night.
        for s in 0..2400 {
            let hour = (23.0 + s as f64 * 30.0 / 3600.0) % 24.0;
            let e = Exposure {
                local_hour: hour as f32,
                ..e
            };
            p.body.step(&cfg, 1.0, &e, &Worn::naked(), &sleeping);
            if let Some(why) = p.rest(&cfg, &e, hour, 1.0) {
                return (why, s as f64 * 30.0 / 3600.0, p);
            }
        }
        panic!("never woke");
    };
    let (why, hours, p) = sleep(29.0);
    println!(
        "warm night: woke {why:?} after {hours:.1} h, pressure {:.2}",
        p.body.sleep.pressure
    );
    assert_eq!(why, hearth_body::Wake::Rested);
    assert!((5.0..11.0).contains(&hours), "slept {hours:.1} h");
    assert!(!p.lying, "up on waking");
    let (why, hours, _) = sleep(4.0);
    println!("cold night: woke {why:?} after {hours:.1} h");
    assert_eq!(why, hearth_body::Wake::Cold);
    assert!(
        hours < 2.0,
        "the cold wakes a bare body within the hour or two"
    );
}

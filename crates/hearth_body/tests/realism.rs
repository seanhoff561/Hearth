//! The body against known human physiology: the thermoneutral zone, cold water, a day's energy,
//! weeks of starvation, sweating in the heat, bleeding, infection, seawater, sleep, frostbite.

mod common;

use common::*;
use hearth_body::{Body, Death, Exposure, Food, RegionCover, Side, Tiredness, Wake, Worn};
use hearth_content::schema::body::BodyRegion;

#[test]
fn a_naked_body_at_rest_holds_its_temperature_in_warm_still_air() {
    let cfg = config();
    let mut body = Body::new(&cfg, 10);
    let warm = Exposure {
        air_c: 29.0,
        humidity: 0.5,
        wind_m_s: 0.1,
        ..Exposure::mild()
    };
    let rest = cfg.activity("resting");
    let (mut lo, mut hi, mut most_shiver, mut most_sweat) = (40.0f64, 30.0f64, 0.0f64, 0.0f64);
    run(
        &mut body,
        &cfg,
        12.0,
        |b, t| {
            if t > 1.0 {
                lo = lo.min(b.thermal.core_c);
                hi = hi.max(b.thermal.core_c);
                most_shiver = most_shiver.max(b.last.shivering_w);
                most_sweat = most_sweat.max(b.last.sweat_kg_s * 3600.0);
            }
            (warm, Worn::naked(), rest)
        },
        |_| false,
    );
    println!(
        "core {lo:.2}–{hi:.2} °C, shivering ≤ {most_shiver:.0} W, sweat ≤ {most_sweat:.2} l/h"
    );
    assert!(lo > 36.4 && hi < 37.3);
    assert!(most_shiver < 20.0 && most_sweat < 0.1);
}

#[test]
fn ordinary_clothes_are_comfortable_indoors() {
    let cfg = config();
    let mut body = Body::new(&cfg, 11);
    // About one clo over everything but the head and hands, as a suit does.
    let mut worn = Worn::naked();
    for (r, c) in hearth_body::REGIONS.iter().zip(worn.regions.iter_mut()) {
        if !matches!(r, BodyRegion::Head | BodyRegion::Hand) {
            *c = RegionCover {
                clo: 1.1,
                wind: 0.5,
                water: 0.2,
            };
        }
    }
    let room = Exposure {
        air_c: 22.0,
        humidity: 0.45,
        wind_m_s: 0.1,
        ..Exposure::mild()
    };
    let sit = hearth_body::Activity {
        met: 1.2,
        ..cfg.activity("resting")
    };
    run(&mut body, &cfg, 8.0, |_, _| (room, worn, sit), |_| false);
    let s = body.status(&cfg);
    println!(
        "core {:.2} °C, skin {:.1} °C, {:?}",
        s.core_c, s.skin_c, s.warmth
    );
    assert!(s.core_c > 36.5 && s.core_c < 37.2);
    assert!(s.skin_c > 30.5 && s.skin_c < 35.0, "{}", s.skin_c);
}

#[test]
fn cold_water_kills_within_hours() {
    let cfg = config();
    let mut body = Body::new(&cfg, 12);
    let sea = Exposure {
        air_c: 5.0,
        immersion: 1.0,
        water_c: 5.0,
        humidity: 0.9,
        ..Exposure::mild()
    };
    let floating = cfg.activity("resting");
    let mut hypothermic_at = None;
    let hours = run(
        &mut body,
        &cfg,
        8.0,
        |b, t| {
            if hypothermic_at.is_none() && b.thermal.core_c < 35.0 {
                hypothermic_at = Some(t);
            }
            (sea, Worn::naked(), floating)
        },
        |b| b.dead.is_some(),
    );
    let hyp = hypothermic_at.expect("hypothermic");
    println!(
        "5 °C water: hypothermic after {:.0} min, dead after {hours:.2} h",
        hyp * 60.0
    );
    assert!((0.2..=1.2).contains(&hyp), "{hyp:.2} h");
    assert_eq!(body.dead, Some(Death::Hypothermia));
    // Expected survival in 5 °C water is one to three hours, longer for some.
    assert!((1.0..=5.0).contains(&hours), "{hours:.2} h");
}

fn stores_kcal(b: &Body) -> f64 {
    b.energy.glycogen_kcal + b.energy.fat_kcal + b.energy.stomach.kcal
}

#[test]
fn a_day_costs_what_people_burn() {
    let cfg = config();
    let worn = wearing(&["loincloth"]);
    // Warm enough that a nearly naked body neither shivers nor sweats at rest.
    let mild = Exposure {
        air_c: 27.0,
        ..Exposure::mild()
    };
    // A day of rest: about the basal rate.
    let mut body = Body::new(&cfg, 13);
    let before = stores_kcal(&body);
    let rest = cfg.activity("resting");
    run(&mut body, &cfg, 24.0, |_, _| (mild, worn, rest), |_| false);
    let resting_kcal = before - stores_kcal(&body);
    // An active day: eight hours of walking and foraging.
    let mut body = Body::new(&cfg, 14);
    let before = stores_kcal(&body);
    run(
        &mut body,
        &cfg,
        24.0,
        |_, t| {
            let a = if t < 8.0 {
                cfg.activity("walking")
            } else if t < 16.0 {
                cfg.activity("standing")
            } else {
                cfg.activity("sleeping")
            };
            (mild, worn, a)
        },
        |_| false,
    );
    let active_kcal = before - stores_kcal(&body);
    println!("a day at rest {resting_kcal:.0} kcal, an active day {active_kcal:.0} kcal");
    assert!((1600.0..=2300.0).contains(&resting_kcal));
    assert!((2400.0..=3600.0).contains(&active_kcal));
}

#[test]
fn a_hungry_body_shivers_on_its_fat_and_a_wasted_one_cannot() {
    let cfg = config();
    let worn = wearing(&["loincloth", "hide_cape", "moccasins"]);
    let rest = cfg.activity("resting");
    // A dry, breezy autumn night, nothing to burn.
    let night = Exposure {
        air_c: 6.0,
        wind_m_s: 2.0,
        ..Exposure::mild()
    };
    let core_after = |glycogen: f64, fat_share: f64| {
        let mut body = Body::new(&cfg, 21);
        body.energy.glycogen_kcal = hearth_body::energy::glycogen_max(cfg.mass_kg) * glycogen;
        body.energy.fat_kcal = fat_share * cfg.mass_kg * hearth_body::energy::KCAL_PER_KG_FAT;
        run(&mut body, &cfg, 4.0, |_, _| (night, worn, rest), |_| false);
        body.thermal.core_c
    };
    let fed = core_after(1.0, 0.2);
    let hungry = core_after(0.0, 0.18);
    let wasted = core_after(0.0, 0.02);
    println!(
        "core after four hours: fed {fed:.2} °C, its glycogen spent {hungry:.2}, wasted {wasted:.2}"
    );
    // Glycogen spent, the fat still shivers nearly as warm.
    assert!(fed - hungry < 0.8, "fed {fed:.2}, hungry {hungry:.2}");
    assert!(hungry > 34.0, "hungry {hungry:.2}");
    // Wasted to the last fat, it cannot keep warm.
    assert!(
        wasted < hungry - 0.5,
        "hungry {hungry:.2}, wasted {wasted:.2}"
    );
}

#[test]
fn starving_with_water_takes_weeks() {
    let cfg = config();
    let worn = wearing(&["loincloth"]);
    let mut body = Body::new(&cfg, 15);
    let rest = cfg.activity("resting");
    let warm = Exposure {
        air_c: 26.0,
        ..Exposure::mild()
    };
    let mut hungry_at = None;
    let hours = run(
        &mut body,
        &cfg,
        120.0 * 24.0,
        |b, t| {
            if b.water.deficit_l > 0.5 {
                b.drink(&cfg, 1.0, 0.0, 0.0);
            }
            if hungry_at.is_none() && b.status(&cfg).hunger >= hearth_body::Hunger::Hungry {
                hungry_at = Some(t);
            }
            (warm, worn, rest)
        },
        |b| b.dead.is_some(),
    );
    let days = hours / 24.0;
    println!(
        "hungry after {:.1} h, dead of starvation after {days:.0} days",
        hungry_at.unwrap_or(0.0)
    );
    assert_eq!(body.dead, Some(Death::Starvation));
    assert!((35.0..=80.0).contains(&days), "{days:.0} days");
    assert!(hungry_at.is_some_and(|h| h < 24.0));
}

#[test]
fn heat_brings_sweat_that_drinking_replaces() {
    let cfg = config();
    let worn = wearing(&["loincloth"]);
    let mut body = Body::new(&cfg, 16);
    let desert = Exposure {
        air_c: 38.0,
        humidity: 0.2,
        wind_m_s: 1.5,
        radiant_w_m2: 150.0,
        ..Exposure::mild()
    };
    let walk = cfg.activity("walking");
    let mut sweat_l = 0.0;
    let mut hottest: f64 = 0.0;
    run(
        &mut body,
        &cfg,
        3.0,
        |b, _| {
            sweat_l += b.last.sweat_kg_s * 30.0;
            hottest = hottest.max(b.thermal.core_c);
            if b.water.deficit_l > 0.3 {
                b.drink(&cfg, 0.3, 0.0, 0.0);
            }
            (desert, worn, walk)
        },
        |_| false,
    );
    println!("walking 3 h at 38 °C: {sweat_l:.2} l of sweat, hottest core {hottest:.2} °C");
    assert!((1.2..=4.5).contains(&sweat_l), "{sweat_l:.2} l");
    // A working core in the heat runs warm; heat exhaustion starts near 39–40 °C.
    assert!(hottest < 39.0);
    assert!(body.dead.is_none());
}

#[test]
fn a_deep_wound_bleeds_out_unless_pressed_and_bound() {
    let cfg = config();
    let worn = wearing(&["loincloth"]);
    let rest = cfg.activity("resting");
    let mut open = Body::new(&cfg, 17);
    open.injure(&cfg, "deep_wound", BodyRegion::UpperLeg, Side::Left, 0.8)
        .expect("thigh");
    let hours = run(
        &mut open,
        &cfg,
        6.0,
        |_, _| (Exposure::mild(), worn, rest),
        |b| b.dead.is_some(),
    );
    println!("untreated: dead after {:.0} min", hours * 60.0);
    assert_eq!(open.dead, Some(Death::BloodLoss));
    assert!(hours < 1.0);

    let mut bound = Body::new(&cfg, 18);
    let i = bound
        .injure(&cfg, "deep_wound", BodyRegion::UpperLeg, Side::Left, 0.8)
        .expect("thigh");
    bound.treat_injury(i, "pressure");
    bound.treat_injury(i, "bandage");
    run(
        &mut bound,
        &cfg,
        6.0,
        |_, _| (Exposure::mild(), worn, rest),
        |_| false,
    );
    let lost = bound.status(&cfg).blood_lost;
    println!("pressed and bound: {:.0} % of the blood lost", lost * 100.0);
    assert!(bound.dead.is_none() && lost < 0.25);
}

#[test]
fn uncleaned_wounds_get_infected_at_their_rate() {
    let cfg = config();
    let worn = wearing(&["loincloth"]);
    let rest = cfg.activity("resting");
    let kind = cfg.injury("puncture").expect("puncture").clone();
    let n = 400;
    let mut infected = 0;
    let mut cleaned_infected = 0;
    for seed in 0..n {
        for clean in [false, true] {
            let mut b = Body::new(&cfg, 1000 + seed);
            let i = b
                .injure(&cfg, "puncture", BodyRegion::Foot, Side::Right, 0.5)
                .expect("foot");
            if clean {
                b.treat_injury(i, "clean_water");
            }
            // Past the six hours in which cleaning prevents infection.
            run(
                &mut b,
                &cfg,
                7.0,
                |_, _| (Exposure::mild(), worn, rest),
                |_| false,
            );
            let got = b.injuries.iter().any(|i| i.infected);
            if got && clean {
                cleaned_infected += 1;
            } else if got {
                infected += 1;
            }
        }
    }
    let rate = infected as f64 / n as f64;
    let expected = kind.infection_risk as f64 * (0.5 + 0.5);
    println!("uncleaned punctures infected: {rate:.3} (expected {expected:.3})");
    assert!((rate - expected).abs() < 0.07);
    assert_eq!(cleaned_infected, 0);
}

#[test]
fn seawater_makes_thirst_worse() {
    let cfg = config();
    let mut body = Body::new(&cfg, 19);
    body.water.deficit_l = 2.0;
    body.drink(&cfg, 1.0, 35.0, 0.0);
    // Once absorbed, the salt has cost more water than the drink brought.
    let worn = wearing(&["loincloth"]);
    run(
        &mut body,
        &cfg,
        2.0,
        |_, _| (Exposure::mild(), worn, cfg.activity("resting")),
        |_| false,
    );
    println!(
        "deficit after a litre of seawater: {:.2} l",
        body.water.deficit_l
    );
    assert!(body.water.deficit_l > 2.4);
}

#[test]
fn a_day_awake_tires_and_a_warm_night_restores() {
    let cfg = config();
    let worn = wearing(&["loincloth"]);
    let mut body = Body::new(&cfg, 20);
    run(
        &mut body,
        &cfg,
        16.0,
        |b, t| {
            if t.fract() < 0.01 && b.water.deficit_l > 0.5 {
                b.drink(&cfg, 0.5, 0.0, 0.0);
            }
            let e = Exposure {
                air_c: 24.0,
                local_hour: (7.0 + t) as f32,
                ..Exposure::mild()
            };
            (e, worn, cfg.activity("foraging"))
        },
        |_| false,
    );
    assert!(body.status(&cfg).tiredness >= Tiredness::Tired);
    // Supper, then a warm bed of boughs and furs.
    body.eat(&cfg, &meal()).expect("room for supper");
    body.drink(&cfg, 0.5, 0.0, 0.0);
    let night = Exposure {
        air_c: 24.0,
        ground_clo: 1.0,
        local_hour: 23.0,
        ..Exposure::mild()
    };
    let mut woke = None;
    run(
        &mut body,
        &cfg,
        12.0,
        |_, _| (night, worn, cfg.activity("sleeping")),
        |b| {
            if let Some(w) = b.wakes(&cfg, &night) {
                return w == Wake::Rested;
            }
            false
        },
    );
    if let Some(w) = body.wakes(&cfg, &night) {
        woke = Some(w);
    }
    println!("woke: {woke:?}, {:?}", body.status(&cfg).tiredness);
    assert_eq!(woke, Some(Wake::Rested));
    assert!(body.status(&cfg).tiredness <= Tiredness::Awake);

    // Sleeping naked on cold ground wakes the sleeper with cold.
    let mut cold = Body::new(&cfg, 21);
    let frosty = Exposure {
        air_c: 2.0,
        sky_c_offset: -8.0,
        local_hour: 1.0,
        ..Exposure::mild()
    };
    run(
        &mut cold,
        &cfg,
        3.0,
        |_, _| (frosty, worn, cfg.activity("sleeping")),
        |b| b.wakes(&cfg, &frosty).is_some(),
    );
    assert_eq!(cold.wakes(&cfg, &frosty), Some(Wake::Cold));
}

#[test]
fn bare_hands_freeze_in_the_arctic_and_mittens_save_them() {
    let cfg = config();
    let arctic = Exposure {
        air_c: -20.0,
        humidity: 0.7,
        wind_m_s: 5.0,
        ..Exposure::mild()
    };
    let walk = cfg.activity("walking");
    for (mittens, expect) in [(false, true), (true, false)] {
        let mut gear = vec!["loincloth", "sewn_fur_parka", "fur_leggings", "moccasins"];
        if mittens {
            gear.push("fur_mittens");
        }
        let worn = wearing(&gear);
        let mut b = Body::new(&cfg, 22);
        run(&mut b, &cfg, 1.0, |_, _| (arctic, worn, walk), |_| false);
        let bitten = b
            .injuries
            .iter()
            .any(|i| i.id.ends_with("frostbite") && i.region == BodyRegion::Hand);
        let hand = b.thermal.regions_c[hearth_body::clothing::region_index(BodyRegion::Hand)];
        println!(
            "mittens {mittens}: hands {hand:.1} °C, frostbite {bitten}, core {:.2}",
            b.thermal.core_c
        );
        assert_eq!(bitten, expect);
    }
}

#[test]
fn a_meal_ends_hunger_and_a_full_stomach_refuses_more() {
    let cfg = config();
    let mut body = Body::new(&cfg, 23);
    body.energy.glycogen_kcal = 300.0;
    assert!(body.status(&cfg).hunger >= hearth_body::Hunger::Hungry);
    assert!(body.eat(&cfg, &meal()).is_ok());
    let big = Food {
        volume_l: 1.5,
        kcal: 1500.0,
        ..Food::default()
    };
    assert_eq!(body.eat(&cfg, &big), Err(hearth_body::Refusal::Full));
}

#[test]
fn falls_hurt_by_their_height() {
    let cfg = config();
    let v = |h: f64| (2.0 * 9.81 * h).sqrt();
    let outcome = |h: f64| {
        let (mut hurt, mut broken, mut dead) = (0, 0, 0);
        for seed in 0..1000 {
            let mut b = Body::new(&cfg, 5000 + seed);
            b.land(&cfg, v(h));
            if b.dead.is_some() {
                dead += 1;
            } else if !b.injuries.is_empty() {
                hurt += 1;
                if b.injuries.iter().any(|i| i.id.ends_with("fracture")) {
                    broken += 1;
                }
            }
        }
        (
            hurt as f64 / 1000.0,
            broken as f64 / 1000.0,
            dead as f64 / 1000.0,
        )
    };
    for h in [1.0, 2.0, 3.0, 6.0, 10.0, 15.0, 25.0] {
        let (hurt, broken, dead) = outcome(h);
        println!("{h:>4} m: hurt {hurt:.2}, fractures {broken:.2}, dead {dead:.2}");
    }
    assert_eq!(outcome(1.0), (0.0, 0.0, 0.0));
    let (hurt, broken, dead) = outcome(3.0);
    assert!(hurt > 0.5 && (0.1..0.35).contains(&broken) && dead == 0.0);
    let (_, broken, dead) = outcome(6.0);
    assert!(broken > 0.6 && dead < 0.05);
    let (_, _, dead) = outcome(12.0);
    assert!((0.3..0.7).contains(&dead));
    let (_, _, dead) = outcome(25.0);
    assert!(dead > 0.95);
}

#[test]
fn yarrow_stems_bleeding_and_willow_bark_eases_pain() {
    let cfg = config();
    let lost = |treat: bool| {
        let mut b = Body::new(&cfg, 77);
        let i = b
            .injure(
                &cfg,
                "deep_wound",
                hearth_content::schema::body::BodyRegion::UpperLeg,
                hearth_body::Side::Left,
                0.6,
            )
            .expect("a wound");
        if treat {
            let n = b.untreated("yarrow").expect("untreated");
            assert_eq!(n, i);
            b.treat_injury(n, "yarrow");
        }
        let before = b.blood_l;
        run(
            &mut b,
            &cfg,
            0.25,
            |_, _| {
                (
                    hearth_body::Exposure::mild(),
                    wearing(&["loincloth"]),
                    cfg.activity("resting"),
                )
            },
            |_| false,
        );
        before - b.blood_l
    };
    let (bare, dressed) = (lost(false), lost(true));
    println!("blood lost in a quarter hour: {bare:.3} l bare, {dressed:.3} l under yarrow");
    assert!(
        dressed < bare * 0.6 && dressed > bare * 0.2,
        "{dressed} vs {bare}"
    );

    // Pain eased by a third for some hours, then back.
    let mut b = Body::new(&cfg, 78);
    b.injure(
        &cfg,
        "fracture",
        hearth_content::schema::body::BodyRegion::LowerArm,
        hearth_body::Side::Right,
        0.7,
    );
    let pain = |b: &Body| b.effects(&cfg).pain;
    let raw = pain(&b);
    b.take_medicine("analgesic", 0.3, 4.0);
    let eased = pain(&b);
    assert!(
        raw > 0.1 && (eased - raw * 0.7).abs() < 0.02,
        "{raw} → {eased}"
    );
    run(
        &mut b,
        &cfg,
        5.0,
        |_, _| {
            (
                hearth_body::Exposure::mild(),
                wearing(&["loincloth"]),
                cfg.activity("resting"),
            )
        },
        |_| false,
    );
    assert!(b.medicines.is_empty(), "worn off");
}

/// H9: a body grown at a stroke (a year passed at once, a child sized to its new age) keeps the
/// share of its blood: growing is no wound.
#[test]
fn a_body_grown_at_a_stroke_has_lost_no_blood() {
    let mut small = config();
    small.params.blood_l = 0.5;
    small.mass_kg = 12.0;
    let big = config();
    let mut body = Body::new(&small, 3);
    let rest = small.activity("resting");
    body.step(&small, 1.0, &Exposure::mild(), &Worn::naked(), &rest);
    body.step(&big, 1.0, &Exposure::mild(), &Worn::naked(), &rest);
    let lost = body.status(&big).blood_lost;
    assert!(
        lost < 0.01,
        "grown, it has lost {:.0} % of its blood",
        lost * 100.0
    );
    // Its stores grown with it: no hungrier for it.
    assert_eq!(
        body.status(&big).hunger,
        Body::new(&big, 3).status(&big).hunger
    );
    assert!(body.dead.is_none());
}

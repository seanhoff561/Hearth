//! V2-3's acceptance tests of the body (v2 milestone list): a naked body in 5 °C rain becomes
//! hypothermic within hours, one in furs by a fire does not; without water a body dies after
//! about three days; a sprain heals in days and a fracture in weeks.

mod common;

use common::*;
use hearth_body::{Body, Death, Exposure, Side, Warmth};
use hearth_content::schema::body::BodyRegion;

fn cold_rain() -> Exposure {
    Exposure {
        air_c: 5.0,
        humidity: 0.95,
        wind_m_s: 3.0,
        rain_mm_h: 3.0,
        water_c: 5.0,
        ..Exposure::mild()
    }
}

#[test]
fn naked_in_cold_rain_becomes_hypothermic_within_hours() {
    let cfg = config();
    let mut body = Body::new(&cfg, 1);
    let worn = wearing(&["loincloth"]);
    let standing = cfg.activity("standing");
    let hours = run(
        &mut body,
        &cfg,
        8.0,
        |_, _| (cold_rain(), worn, standing),
        |b| b.thermal.core_c < 35.0,
    );
    println!(
        "core below 35 °C after {hours:.2} h, skin {:.1} °C",
        body.thermal.skin_c
    );
    assert!(
        body.thermal.core_c < 35.0,
        "still {:.2} °C",
        body.thermal.core_c
    );
    assert!(
        (0.5..=4.0).contains(&hours),
        "mild hypothermia after {hours:.2} h: people take one to a few hours"
    );
    assert_eq!(body.status(&cfg).warmth, Warmth::Hypothermic);
}

#[test]
fn furs_and_a_fire_keep_a_body_warm_in_the_same_rain() {
    let cfg = config();
    let mut body = Body::new(&cfg, 2);
    let worn = wearing(&[
        "loincloth",
        "sewn_fur_parka",
        "fur_leggings",
        "fur_mittens",
        "moccasins",
    ]);
    let sitting = cfg.activity("resting");
    // Sitting about two metres from a good campfire: ≈ 60 W/m² absorbed over the body.
    let by_fire = Exposure {
        radiant_w_m2: 60.0,
        ..cold_rain()
    };
    let mut coldest: f64 = 37.0;
    run(
        &mut body,
        &cfg,
        12.0,
        |b, _| {
            coldest = coldest.min(b.thermal.core_c);
            (by_fire, worn, sitting)
        },
        |_| false,
    );
    println!(
        "coldest core {coldest:.2} °C, now {:.2} °C, skin {:.1} °C",
        body.thermal.core_c, body.thermal.skin_c
    );
    assert!(coldest > 35.5, "fell to {coldest:.2} °C");
    assert!(body.dead.is_none());
}

/// A hot, active day: up at six, walking in the sun from nine to three, foraging morning and
/// evening, asleep from ten at night; 32 °C by day, 22 °C at night, dry air.
fn active_day(
    cfg: &hearth_body::BodyConfig,
    hour_of_day: f64,
) -> (Exposure, hearth_body::Activity) {
    let h = hour_of_day % 24.0;
    let day = (6.0..22.0).contains(&h);
    let e = Exposure {
        air_c: if day { 32.0 } else { 22.0 },
        humidity: 0.3,
        wind_m_s: 1.0,
        radiant_w_m2: if (9.0..16.0).contains(&h) { 100.0 } else { 0.0 },
        sky_c_offset: if day { 0.0 } else { -6.0 },
        ground_clo: 0.4,
        local_hour: h as f32,
        ..Exposure::mild()
    };
    let a = if !day {
        cfg.activity("sleeping")
    } else if (9.0..15.0).contains(&h) {
        cfg.activity("walking")
    } else if (15.0..21.0).contains(&h) || (6.0..9.0).contains(&h) {
        cfg.activity("foraging")
    } else {
        cfg.activity("resting")
    };
    (e, a)
}

#[test]
fn without_water_a_body_dies_after_about_three_days() {
    let cfg = config();
    let worn = wearing(&["loincloth"]);
    // Hot, active days: sweat takes most of the water. How long a body lasts depends on heat
    // and work, as it does: about three days here, a week or more resting in the shade.
    let mut body = Body::new(&cfg, 3);
    let hours = run(
        &mut body,
        &cfg,
        10.0 * 24.0,
        |_, t| {
            let (e, a) = active_day(&cfg, 6.0 + t);
            (e, worn, a)
        },
        |b| b.dead.is_some(),
    );
    let days = hours / 24.0;
    println!(
        "hot and active, no water: dead after {days:.2} days ({:?})",
        body.dead
    );
    assert_eq!(body.dead, Some(Death::Dehydration));
    assert!((2.3..=4.0).contains(&days), "{days:.2} days");

    // Resting in the shade at 20 °C lasts longer.
    let mut resting = Body::new(&cfg, 4);
    let rest = cfg.activity("resting");
    let hours = run(
        &mut resting,
        &cfg,
        14.0 * 24.0,
        |_, t| {
            let e = Exposure {
                local_hour: ((6.0 + t) % 24.0) as f32,
                ..Exposure::mild()
            };
            (e, worn, rest)
        },
        |b| b.dead.is_some(),
    );
    let rest_days = hours / 24.0;
    println!("resting, no water: dead after {rest_days:.2} days");
    assert_eq!(resting.dead, Some(Death::Dehydration));
    assert!(
        rest_days > days + 2.0 && rest_days < 12.0,
        "{rest_days:.2} days"
    );
}

#[test]
fn a_sprain_heals_in_days_and_a_fracture_in_weeks() {
    let cfg = config();
    let worn = wearing(&["loincloth"]);
    let mut body = Body::new(&cfg, 5);
    let sprain = body
        .injure(&cfg, "sprain", BodyRegion::Foot, Side::Left, 0.5)
        .expect("a foot can be sprained");
    let fracture = body
        .injure(&cfg, "fracture", BodyRegion::LowerLeg, Side::Right, 0.5)
        .expect("a shin can break");
    body.treat_injury(fracture, "splint");
    assert_ne!(sprain, fracture);
    let fx = body.effects(&cfg);
    assert!(!fx.jump && !fx.sprint && fx.walk < 0.5, "{fx:?}");

    // Fed and watered three times a day, living ordinary days.
    let mut healed_at = [None, None];
    let mut last_meal = -10.0;
    let end = run(
        &mut body,
        &cfg,
        60.0 * 24.0,
        |b, t| {
            if t - last_meal >= 6.0 {
                last_meal = t;
                let _ = b.eat(&cfg, &meal());
                b.drink(&cfg, 1.0, 0.0, 0.0);
            }
            let h = (6.0 + t) % 24.0;
            let asleep = !(6.0..22.0).contains(&h);
            let a = cfg.activity(if asleep { "sleeping" } else { "resting" });
            let e = Exposure {
                local_hour: h as f32,
                ground_clo: 0.5,
                ..Exposure::mild()
            };
            for (k, id) in ["sprain", "fracture"].iter().enumerate() {
                if healed_at[k].is_none() && !b.injuries.iter().any(|i| i.id.ends_with(id)) {
                    healed_at[k] = Some(t);
                }
            }
            (e, worn, a)
        },
        |b| b.injuries.is_empty(),
    );
    // The run stops as the last one heals.
    let sprain_h = healed_at[0].unwrap_or(end);
    let fracture_h = healed_at[1].unwrap_or(end);
    println!(
        "sprain healed after {sprain_h:.0} h, fracture after {:.1} days",
        fracture_h / 24.0
    );
    assert!(body.injuries.is_empty(), "both healed: {:?}", body.injuries);
    // A moderate sprain: about four days; a splinted fracture: about six weeks.
    assert!((70.0..=110.0).contains(&sprain_h), "{sprain_h:.0} h");
    assert!(
        (30.0..=48.0).contains(&(fracture_h / 24.0)),
        "{:.1} days",
        fracture_h / 24.0
    );
    let fx = body.effects(&cfg);
    assert!(fx.jump && fx.sprint && fx.walk > 0.95, "{fx:?}");
}

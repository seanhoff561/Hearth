//! Shared set-up of the body tests: the base content's body at the default time scales.

#![allow(dead_code)]

use std::sync::OnceLock;

use hearth_body::{Activity, Body, BodyConfig, Exposure, Food, Rates, Worn};
use hearth_content::Content;
use hearth_content::schema::TimeScale;
use hearth_content::time::TimeScales;

pub fn content() -> &'static Content {
    static C: OnceLock<Content> = OnceLock::new();
    C.get_or_init(Content::load_base)
}

pub fn config() -> BodyConfig {
    let c = content();
    BodyConfig::with_rates(c, Rates::authentic(), TimeScales::defaults(&c.time))
}

/// Seconds of play per real hour of body time (the day scale).
pub fn play_per_hour(cfg: &BodyConfig) -> f64 {
    cfg.play_seconds(1.0, TimeScale::Day)
}

/// What a set of garments (content ids without the namespace) covers.
pub fn wearing(ids: &[&str]) -> Worn {
    let c = content();
    Worn::of(ids.iter().map(|id| {
        c.garments
            .get(&format!("hearth:{id}"))
            .unwrap_or_else(|| panic!("garment {id}"))
    }))
}

/// Runs the body for `hours` of body time in steps of one second of play, calling `each` with
/// the body time (hours since the start) before every step; stops early when `stop` holds.
pub fn run(
    body: &mut Body,
    cfg: &BodyConfig,
    hours: f64,
    mut each: impl FnMut(&mut Body, f64) -> (Exposure, Worn, Activity),
    stop: impl Fn(&Body) -> bool,
) -> f64 {
    let dt = 1.0;
    let step_h = dt / play_per_hour(cfg);
    let mut t = 0.0;
    while t < hours {
        let (e, w, a) = each(body, t);
        body.step(cfg, dt, &e, &w, &a);
        t += step_h;
        if stop(body) {
            break;
        }
    }
    t
}

/// A plain day's meal: 900 kcal of mixed food with half a litre of water.
pub fn meal() -> Food {
    Food {
        kcal: 900.0,
        protein_g: 35.0,
        fat_g: 35.0,
        carb_g: 110.0,
        water_l: 0.5,
        volume_l: 0.9,
        fresh_days: 1.0,
    }
}

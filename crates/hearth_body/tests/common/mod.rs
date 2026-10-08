//! Shared set-up of the body tests: the base content's body, at real rates.

#![allow(dead_code)]

use std::sync::OnceLock;

use hearth_body::{Activity, Body, BodyConfig, Exposure, Food, Rates, Worn};
use hearth_content::Content;

pub fn content() -> &'static Content {
    static C: OnceLock<Content> = OnceLock::new();
    C.get_or_init(Content::load_base)
}

pub fn config() -> BodyConfig {
    let c = content();
    BodyConfig::with_rates(c, Rates::authentic())
}

/// The body tests' step (s).
pub const STEP_S: f64 = 30.0;

/// What a set of garments (content ids without the namespace) covers.
pub fn wearing(ids: &[&str]) -> Worn {
    let c = content();
    Worn::of(ids.iter().map(|id| {
        c.garments
            .get(&format!("hearth:{id}"))
            .unwrap_or_else(|| panic!("garment {id}"))
    }))
}

/// Runs the body for `hours` in steps of [`STEP_S`], calling `each` with the time (hours since
/// the start) before every step; stops early when `stop` holds.
pub fn run(
    body: &mut Body,
    cfg: &BodyConfig,
    hours: f64,
    mut each: impl FnMut(&mut Body, f64) -> (Exposure, Worn, Activity),
    stop: impl Fn(&Body) -> bool,
) -> f64 {
    let dt = STEP_S;
    let step_h = dt / 3600.0;
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

//! Fields (v2 Era 3; V2-12): plots of tilled soil, what is sown in them, how it grows through the
//! year, and what reaping it gives — and how a crop changes over the generations it is sown and
//! reaped. A plot is a square metre: the tilled block, its crop drawn on it.
//!
//! * **When** — a winter cereal sown in autumn ripens the next early summer, sown in spring later
//!   that summer for less; sown in summer or the depth of winter it fails. Where winters are hard
//!   an autumn sowing is mostly killed. Times run on the calendar: the year as the world has it.
//! * **How much** — a square metre gives its crop's best yield (a wild stand's, rising toward a
//!   domestic crop's as its grain grows plumper), less for a thin stand (seed lying dormant),
//!   for a climate the crop does not like, for too little water (rain in its season, or
//!   irrigation from water within a few metres), for poor soil (its nitrogen, drawn down by each
//!   crop, restored by fallow and dung), for the weeds left among it, for what birds and mice
//!   take, and as the year is good or bad.
//! * **The wild's ways** — ripe, a wild stand's ears shatter: reaped at ripeness only some two
//!   thirds of their grain is still held, and less with each day after, while ears with a tough
//!   rachis hold it all. What is reaped is richer in tough lines than what was sown, so sowing
//!   from the reaping and reaping again selects them, as Hillman and Davies (1990) showed for
//!   sickle harvests; a lot's dormant seed is never reaped, so dormancy falls; keeping the
//!   plumpest third of the grain for seed raises its weight by the breeder's equation (a
//!   tenth's spread, half of it heritable). Reaped late, more is lost and the selection is the
//!   stronger.

use hearth_content::Content;
use hearth_env::Moment;
use hearth_env::climate::Normals;
use hearth_items::Lot;
use hearth_math::BlockPos;
use hearth_math::hash::Rng;
use serde::{Deserialize, Serialize};

/// A crop as the fields grow it (from its plant's `crop`).
#[derive(Debug, Clone, PartialEq)]
pub struct CropDef {
    pub plant: String,
    pub name: String,
    /// The grain's material and the crop block.
    pub grain: String,
    pub block: String,
    pub fibre: Option<String>,
    pub ripe_autumn: f32,
    pub ripe_spring: Option<f32>,
    pub spring_yield: f32,
    pub grain_mg: (f32, f32),
    pub yield_kg_m2: (f32, f32),
    pub water_mm: f32,
    pub draws: f32,
    pub wild: Lot,
    pub temp_c: Option<(f32, f32)>,
    pub precip_mm: Option<(f32, f32)>,
}

/// Every crop there is.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Crops {
    pub list: Vec<CropDef>,
}

impl Crops {
    pub fn from_content(c: &Content) -> Self {
        let list = c
            .plants
            .iter()
            .filter_map(|p| {
                let cr = p.crop.as_ref()?;
                Some(CropDef {
                    plant: p.id.clone(),
                    name: p.name.clone(),
                    grain: cr.grain.to_string(),
                    block: cr.block.to_string(),
                    fibre: cr.fibre.as_ref().map(|f| f.to_string()),
                    ripe_autumn: cr.ripe_autumn_sown,
                    ripe_spring: cr.ripe_spring_sown,
                    spring_yield: cr.spring_yield,
                    grain_mg: cr.grain_mg,
                    yield_kg_m2: cr.yield_kg_m2,
                    water_mm: cr.water_mm,
                    draws: cr.draws,
                    wild: Lot {
                        tough: cr.wild_tough,
                        grain_mg: cr.grain_mg.0,
                        dormant: cr.wild_dormant,
                        generations: 0,
                    },
                    temp_c: p.climate.temp_c,
                    precip_mm: p.climate.precip_mm,
                })
            })
            .collect();
        Self { list }
    }

    /// The crop whose grain is a material.
    pub fn by_grain(&self, material: &str) -> Option<&CropDef> {
        self.list.iter().find(|c| same(&c.grain, material))
    }

    /// The crop standing as a block.
    pub fn by_block(&self, block: &str) -> Option<&CropDef> {
        self.list.iter().find(|c| same(&c.block, block))
    }

    pub fn get(&self, plant: &str) -> Option<&CropDef> {
        self.list.iter().find(|c| same(&c.plant, plant))
    }
}

/// Whether two ids name the same thing (with or without their namespace).
fn same(a: &str, b: &str) -> bool {
    let k = |s: &str| s.rsplit(':').next().unwrap_or(s).to_owned();
    k(a) == k(b)
}

/// The grain a sheaf holds (kg): a reaper's handful of ears, bound.
pub const SHEAF_GRAIN_KG: f32 = 0.02;
/// A lot's tough lines never fall below this: the mutation keeps arising.
pub const LEAST_TOUGH: f32 = 0.001;
/// Selecting the plumpest third for seed raises a lot's grain weight by this share (the
/// breeder's equation: a third's selection intensity 1.09, a spread of a tenth, half heritable).
pub const SELECTED_GAIN: f32 = 1.09 * 0.1 * 0.5;
/// Nitrogen dung adds to a plot, and what a year's rest restores.
pub const DUNG_N: f32 = 0.35;
pub const FALLOW_N_YEAR: f32 = 0.25;
/// Years a tilled plot left unsown goes back to grass.
pub const ABANDONED_YEARS: f64 = 2.0;
/// Days after ripeness a crop left standing lodges and is lost.
pub const LOST_DAYS: f64 = 20.0;

/// A crop growing in a plot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Growing {
    /// The crop's plant.
    pub crop: String,
    pub lot: Lot,
    /// World days it was sown and ripens.
    pub sown: f64,
    pub ripe: f64,
    /// Sown in spring.
    pub spring: bool,
    /// 0–1 of the sown seed standing (the dormant lie in the ground; hard winters kill).
    pub stand: f32,
    /// The weeds over its season (their cover, summed by the share of the season).
    pub weed_load: f32,
    /// 0–1 taken by birds, mice and insects.
    pub eaten: f32,
    /// The year as it went (0.75 a bad one … 1.15 a good one).
    pub year: f32,
}

/// A plot of tilled soil.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plot {
    pub pos: BlockPos,
    /// Its soil's nitrogen (1: a fertile soil's).
    pub n: f32,
    /// What its soil holds of nitrogen at rest.
    pub base_n: f32,
    /// 0–1 weed cover.
    pub weeds: f32,
    /// Years in a row it has been cropped (weeds build up in the soil).
    pub cropped: u8,
    pub growing: Option<Growing>,
    /// The world day it was last tilled or reaped (its rest since).
    pub since: f64,
    /// The world day it was last brought up to date.
    pub day: f64,
}

/// A crop's stage as it is drawn: 0 sown, 1 shoots, 2 leafy, 3 eared, 4 ripe.
pub fn stage(g: &Growing, day: f64) -> u8 {
    let f = (day - g.sown) / (g.ripe - g.sown).max(1e-3);
    match f {
        f if f >= 1.0 => 4,
        f if f >= 0.72 => 3,
        f if f >= 0.38 => 2,
        f if f >= 0.1 => 1,
        _ => 0,
    }
}

/// The share of the year (from the local spring equinox) a moment is at.
pub fn local_year(m: &Moment, southern: bool) -> f64 {
    if southern {
        (m.year_frac + 0.5).fract()
    } else {
        m.year_frac
    }
}

/// Why a crop cannot be sown now, or when it will ripen (world days from now) and whether as a
/// spring sowing.
pub fn sowing(c: &CropDef, local: f64, days_per_year: f64) -> Result<(f64, bool), &'static str> {
    if (0.5..0.78).contains(&local) {
        // Autumn (and the first days of winter): it ripens the next year.
        return Ok(((c.ripe_autumn as f64 + 1.0 - local) * days_per_year, false));
    }
    if local < 0.18 {
        return match c.ripe_spring {
            Some(r) if (r as f64) > local + 0.1 => Ok(((r as f64 - local) * days_per_year, true)),
            _ => Err("It is too late in the year to sow this: it is sown in autumn."),
        };
    }
    if local >= 0.78 {
        return Err("The ground is too cold: sow in autumn, or in early spring.");
    }
    Err("It is no time for sowing: seed sown now dies in the summer's heat and drought.")
}

/// How well a place's climate suits a crop (0.2–1).
pub fn climate_fit(c: &CropDef, normals: &Normals) -> f32 {
    let mut f = 1.0f32;
    if let Some((lo, hi)) = c.temp_c {
        let t = normals.t_mean as f32;
        let off = (lo - t).max(t - hi).max(0.0);
        f *= (1.0 - off / 6.0).clamp(0.2, 1.0);
    }
    if let Some((lo, hi)) = c.precip_mm {
        let p = normals.precip as f32;
        let off = ((lo - p) / lo.max(1.0))
            .max((p - hi) / hi.max(1.0))
            .max(0.0);
        f *= (1.0 - off * 1.5).clamp(0.2, 1.0);
    }
    f
}

/// How well a crop is watered (0.1–1): the rain of its season (some three fifths of the year's
/// fall) and irrigation.
pub fn watered(c: &CropDef, normals: &Normals, irrigated: bool) -> f32 {
    let supply = normals.precip as f32 * 0.6 + if irrigated { 400.0 } else { 0.0 };
    (supply / c.water_mm.max(1.0)).min(1.0).powf(0.6).max(0.1)
}

/// What a soil's nitrogen gives (1 at a fertile soil's, less on poor or spent ground).
pub fn nitrogen(n: f32) -> f32 {
    (n * 1.3 / (n.max(0.0) + 0.3)).min(1.15)
}

/// How far a lot has come toward its crop's domestic grain (0 wild … 1).
pub fn domestic(c: &CropDef, lot: &Lot) -> f32 {
    ((lot.grain_mg - c.grain_mg.0) / (c.grain_mg.1 - c.grain_mg.0).max(1e-3)).clamp(0.0, 1.0)
}

/// The grain a ripe square metre bears before reaping's losses (kg).
pub fn bears(c: &CropDef, g: &Growing, plot: &Plot, fit: f32, water: f32) -> f32 {
    let d = domestic(c, &g.lot);
    let best = c.yield_kg_m2.0 + (c.yield_kg_m2.1 - c.yield_kg_m2.0) * d;
    let season = (g.ripe - g.sown).max(1e-3) as f32;
    let weeds = (g.weed_load / season).clamp(0.0, 1.0);
    let spring = if g.spring { c.spring_yield } else { 1.0 };
    best * g.stand.sqrt()
        * fit
        * water
        * nitrogen(plot.n)
        * (1.0 - 0.5 * weeds)
        * spring
        * g.year
        * (1.0 - g.eaten).max(0.0)
}

/// The share of their grain a lot's ears still hold when reaped `after` days past ripeness
/// (before it, the grain not yet filled is the loss instead).
pub fn held(lot: &Lot, after: f64) -> f32 {
    let brittle = 0.05 + 0.6 * (-(after.max(0.0) as f32) / 2.0).exp();
    lot.tough * 0.98 + (1.0 - lot.tough) * brittle
}

/// What reaping a ripe plot `after` days past ripeness gives: the grain (kg) and the lot it is,
/// richer in tough lines than the lot sown, its dormant seed never reaped.
pub fn reaped(c: &CropDef, g: &Growing, bears_kg: f32, after: f64) -> (f32, Lot) {
    // Green, the grain is not yet filled; it does not yet shatter.
    let (fill, keep) = if after < 0.0 {
        let f = ((g.ripe + after - g.sown) / (g.ripe - g.sown).max(1e-3)) as f32;
        (((f - 0.75) / 0.25).clamp(0.0, 1.0), 0.95)
    } else {
        (1.0, held(&g.lot, after))
    };
    let grain = bears_kg * fill * keep;
    let tough_kept = if after < 0.0 {
        g.lot.tough
    } else {
        (g.lot.tough * 0.98 / keep.max(1e-4)).min(1.0)
    };
    let lot = Lot {
        tough: tough_kept.max(LEAST_TOUGH),
        // Sown deep among other plants, the bigger seedlings do a little better.
        grain_mg: (g.lot.grain_mg * 1.004).min(c.grain_mg.1),
        dormant: (g.lot.dormant * 0.7).max(0.02),
        generations: g.lot.generations.saturating_add(1),
    };
    (grain, lot)
}

/// The plumpest third of a lot, picked out for seed, and the rest.
pub fn selected(c: &CropDef, lot: &Lot) -> (Lot, Lot) {
    let up = Lot {
        grain_mg: (lot.grain_mg * (1.0 + SELECTED_GAIN)).min(c.grain_mg.1),
        ..*lot
    };
    let down = Lot {
        grain_mg: lot.grain_mg * (1.0 - SELECTED_GAIN / 2.0),
        ..*lot
    };
    (up, down)
}

impl Plot {
    /// A plot newly tilled from soil of nitrogen `n`.
    pub fn tilled(pos: BlockPos, n: f32, day: f64) -> Self {
        Self {
            pos,
            n,
            base_n: n,
            weeds: 0.0,
            cropped: 0,
            growing: None,
            since: day,
            day,
        }
    }

    /// Sows a lot of a crop now (world day `day`), to ripen in `ripe_in` days.
    pub fn sow(
        &mut self,
        c: &CropDef,
        lot: Lot,
        day: f64,
        ripe_in: f64,
        spring: bool,
        hard_winter: bool,
        rng: &mut Rng,
    ) {
        let winter = if !spring && hard_winter { 0.3 } else { 1.0 };
        // A year's luck: rain at the right time, a late frost, a plague of mice.
        let year = 0.8 + 0.35 * rng.next_f32();
        let pests = 0.08 * rng.next_f32();
        self.growing = Some(Growing {
            crop: c.plant.clone(),
            lot,
            sown: day,
            ripe: day + ripe_in,
            spring,
            stand: (1.0 - lot.dormant).clamp(0.0, 1.0) * 0.9 * winter,
            weed_load: 0.0,
            eaten: pests,
            year,
        });
        self.weeds = self.weeds.min(0.1);
    }

    /// The plot brought up to day `day` (days per year `dpy`): weeds grow among a crop and on
    /// bare tilled ground, faster the more years it has been cropped; birds and mice take from a
    /// ripe crop left standing; fallow ground recovers its nitrogen. True if its crop is lost
    /// (lodged and spoiled, left too long).
    pub fn advance(&mut self, day: f64, dpy: f64) -> bool {
        let dt = (day - self.day).max(0.0);
        self.day = day;
        if dt <= 0.0 {
            return false;
        }
        let years = (dt / dpy.max(1.0)) as f32;
        let rate = 1.6 * (1.0 + 0.2 * self.cropped as f32);
        self.weeds = (self.weeds + rate * years * (1.0 - self.weeds)).min(1.0);
        match &mut self.growing {
            Some(g) => {
                g.weed_load += self.weeds * dt as f32;
                let after = (day - g.ripe) as f32;
                if after > 0.0 {
                    let days = after.min(dt as f32);
                    g.eaten = (g.eaten + 0.04 * days).min(1.0);
                }
                if day - g.ripe > LOST_DAYS {
                    self.growing = None;
                    self.since = day;
                    return true;
                }
            }
            None => {
                // At rest, the soil recovers.
                let cap = (self.base_n * 1.2).max(self.n);
                self.n = (self.n + FALLOW_N_YEAR * years).min(cap.max(self.base_n));
            }
        }
        false
    }

    /// Reaped: its crop gone, its soil drawn on by what it bore.
    pub fn reap_done(&mut self, c: &CropDef, bore_kg: f32, day: f64) {
        let full = c.yield_kg_m2.1.max(1e-3);
        self.n = (self.n - c.draws * (bore_kg / full).min(1.5)).max(0.05);
        self.cropped = self.cropped.saturating_add(1);
        self.growing = None;
        self.since = day;
    }
}

/// The nitrogen of a soil as its block's material has it (1 a fertile soil's).
pub fn soil_n(c: &Content, material: Option<&str>) -> f32 {
    let Some(m) = material.and_then(|m| c.materials.get(m)) else {
        return 0.7;
    };
    if m.tags.iter().any(|t| t == "fertile") {
        1.0
    } else if m.tags.iter().any(|t| t == "acidic" || t == "sand") {
        0.45
    } else {
        0.7
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn einkorn() -> CropDef {
        CropDef {
            plant: "hearth:wild_einkorn".into(),
            name: "Wild einkorn".into(),
            grain: "hearth:einkorn".into(),
            block: "hearth:einkorn_crop".into(),
            fibre: None,
            ripe_autumn: 0.28,
            ripe_spring: Some(0.40),
            spring_yield: 0.75,
            grain_mg: (12.0, 28.0),
            yield_kg_m2: (0.1, 0.25),
            water_mm: 300.0,
            draws: 0.25,
            wild: Lot {
                tough: 0.003,
                grain_mg: 12.0,
                dormant: 0.5,
                generations: 0,
            },
            temp_c: Some((4.0, 20.0)),
            precip_mm: Some((250.0, 900.0)),
        }
    }

    #[test]
    fn sowing_times_follow_the_year() {
        let c = einkorn();
        let (days, spring) = sowing(&c, 0.6, 32.0).expect("autumn");
        assert!(!spring && (days - (0.68 * 32.0)).abs() < 1e-6, "{days}");
        let (days, spring) = sowing(&c, 0.05, 32.0).expect("spring");
        assert!(spring && (days - 0.35 * 32.0).abs() < 1e-4, "{days}");
        assert!(sowing(&c, 0.35, 32.0).is_err(), "not in summer");
        assert!(sowing(&c, 0.85, 32.0).is_err(), "not in deep winter");
    }

    /// Sown, reaped at ripeness and resown, the best third kept each time: the wild lot turns
    /// domestic over some twenty generations, and its yield rises severalfold.
    #[test]
    fn a_wild_lot_resown_and_reaped_turns_domestic() {
        let c = einkorn();
        let mut lot = c.wild;
        let mut first = 0.0;
        let mut last = 0.0;
        for g_n in 0..25 {
            let mut plot = Plot::tilled(BlockPos::new(0, 0, 0), 1.0, 0.0);
            let mut rng = Rng::new(7 + g_n);
            plot.sow(&c, lot, 0.0, 20.0, false, false, &mut rng);
            let g = plot.growing.clone().expect("sown");
            let g = Growing {
                year: 1.0,
                eaten: 0.0,
                weed_load: 0.0,
                ..g
            };
            let bears_kg = bears(&c, &g, &plot, 1.0, 1.0);
            let (kg, next) = reaped(&c, &g, bears_kg, 0.0);
            if g_n == 0 {
                first = kg;
            }
            last = kg;
            lot = selected(&c, &next).0;
        }
        println!("after 25 generations: {lot:?}; {first:.3} → {last:.3} kg a square metre");
        assert!(lot.tough > 0.9, "tough-eared: {lot:?}");
        assert!(lot.dormant < 0.05);
        assert!(lot.grain_mg > 25.0);
        assert!(last > 3.0 * first, "{first} → {last}");
    }

    #[test]
    fn reaped_late_the_wild_loses_more_and_selects_harder() {
        let c = einkorn();
        let g = Growing {
            crop: c.plant.clone(),
            lot: c.wild,
            sown: 0.0,
            ripe: 20.0,
            spring: false,
            stand: 0.45,
            weed_load: 0.0,
            eaten: 0.0,
            year: 1.0,
        };
        let (on_time, a) = reaped(&c, &g, 0.1, 0.0);
        let (late, b) = reaped(&c, &g, 0.1, 4.0);
        assert!(late < on_time * 0.5, "{on_time} {late}");
        assert!(b.tough > a.tough);
        // Green: no shattering, and no selection either.
        let (green, d) = reaped(&c, &g, 0.1, -1.0);
        assert!(green < 0.1 && (d.tough - c.wild.tough).abs() < 1e-6);
    }

    #[test]
    fn fallow_and_dung_restore_a_spent_soil() {
        let c = einkorn();
        let mut plot = Plot::tilled(BlockPos::new(0, 0, 0), 1.0, 0.0);
        plot.reap_done(&c, 0.25, 0.0);
        plot.reap_done(&c, 0.25, 0.0);
        let spent = plot.n;
        assert!(spent < 0.6, "{spent}");
        plot.advance(64.0, 32.0);
        assert!(plot.n > spent + 0.4, "two years' rest: {}", plot.n);
        assert!(plot.weeds > 0.5, "and the weeds come");
    }
}

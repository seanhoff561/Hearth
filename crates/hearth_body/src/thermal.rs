//! Heat balance (v2 §9.4): a core and a shell of eleven skin regions, after the two-node model
//! of Gagge, Stolwijk and Nishi (1971) with the shell split by region as in multi-segment
//! models. The core makes the metabolic heat (and the shivering) and passes it to each region of
//! skin through the tissue and the blood flowing there; each region loses it to the
//! surroundings through what covers it — by convection and radiation, by evaporating sweat and
//! rain, by warming the rain, to water when immersed and to the ground when lying — and gains the
//! sun's and fires' radiation through its cover. The skin of a region settles within minutes, so
//! it is solved as a balance at every step; the core carries the body's heat store. Warm and
//! cold signals (core, and the mean skin against their set points) drive the skin's blood flow,
//! sweating and shivering; in the cold the hands and feet are shut off most, so they grow cold
//! while the trunk stays warm, as they do.
//!
//! Gagge's skin-layer conductance (5.28 W/m²K) is replaced by the shell's tissue insulation at
//! full vasoconstriction (≈ 0.1 m²K/W), so a naked body in cold rain cools as people do — mildly
//! hypothermic in an hour or two despite shivering — rather than holding its core warm.

use serde::{Deserialize, Serialize};

use crate::clothing::{REGIONS, Worn, region_area};
use crate::{Exposure, Posture};
use hearth_content::schema::body::BodyRegion;

/// Specific heat of body tissue (J/kg/K).
const C_TISSUE: f64 = 3490.0;
/// Share of the body's mass that stores the core's heat.
const CORE_MASS: f64 = 0.9;
/// Set points of the core and the mean skin (°C).
pub const CORE_SET: f64 = 36.8;
pub const SKIN_SET: f64 = 33.7;
/// Conductance of the shell's tissue at full vasoconstriction (W/m²K).
const K_TISSUE: f64 = 10.0;
/// Latent heat of evaporation of water at skin temperature (J/kg).
pub const LATENT: f64 = 2.43e6;
/// Water skin and clothing hold before it runs off (kg/m²).
pub const WET_HOLD: f64 = 0.12;
/// Radiative heat transfer coefficient (W/m²K).
const H_RAD: f64 = 4.7;
/// Heat transfer coefficient to still water (W/m²K).
const H_WATER: f64 = 150.0;
/// Conductance of bare skin to the ground when lying (W/m²K).
const H_GROUND: f64 = 15.0;
/// Share of the skin against the ground when lying.
const GROUND_SHARE: f64 = 0.25;
/// Vapour resistance of the skin itself (m²kPa/W): water diffuses through dry skin at about
/// 9 g/m²h in room air, some 0.35 l a day (insensible perspiration; the breath adds as much).
const R_SKIN_VAPOUR: f64 = 0.6;
/// Longest step the explicit integration of the core takes (s).
const MAX_STEP: f64 = 30.0;

/// Saturation vapour pressure of water (kPa) at `t` °C (Tetens).
pub fn p_sat(t: f64) -> f64 {
    0.6108 * (17.27 * t / (t + 237.3)).exp()
}

/// The core's temperature, the skin's by region and the water on skin and clothing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Thermal {
    pub core_c: f64,
    /// Area-weighted mean of the skin.
    pub skin_c: f64,
    /// Skin temperature of each region (°C), in the order of [`REGIONS`].
    #[serde(default = "warm_skin")]
    pub regions_c: [f32; 11],
    /// Rain or swimming water held by skin and clothing (kg/m²).
    pub wet_kg_m2: f64,
}

fn warm_skin() -> [f32; 11] {
    [33.5; 11]
}

impl Default for Thermal {
    fn default() -> Self {
        Self {
            core_c: CORE_SET,
            skin_c: 33.5,
            regions_c: warm_skin(),
            wet_kg_m2: 0.0,
        }
    }
}

/// What the rest of the body asks of the heat balance in one step.
#[derive(Debug, Clone, Copy)]
pub struct Drive {
    pub area_m2: f64,
    pub mass_kg: f64,
    /// Metabolic heat apart from shivering (W).
    pub metabolic_w: f64,
    /// Basal metabolic rate (W): exercise above it opens the muscles' blood flow.
    pub basal_w: f64,
    /// The most shivering can add (W), lower when the glycogen that fuels it runs low.
    pub shiver_max_w: f64,
    /// Raise of the core's set point by fever (°C).
    pub fever_c: f64,
    /// 0–1: sweating the body's water allows (dehydrated bodies sweat less).
    pub sweat_capacity: f64,
    /// Balance multipliers on heat lost in the cold and gained in the heat.
    pub cold_stress: f64,
    pub heat_stress: f64,
    /// Air speed over the body (wind and the body's own movement, m/s).
    pub air_speed: f64,
    pub posture: Posture,
}

/// What a step did, as averages over it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Flows {
    /// Heat from shivering (W).
    pub shivering_w: f64,
    /// Sweat produced, evaporated or dripping (kg/s).
    pub sweat_kg_s: f64,
    /// Body water evaporated through the skin and in the breath, apart from sweat (kg/s).
    pub insensible_kg_s: f64,
    /// 0–1 share of the skin that is wet (sweat or rain).
    pub skin_wetness: f64,
    /// Net heat lost to the surroundings (W; negative when gaining).
    pub loss_w: f64,
}

/// How much of the core's blood a region's skin gets, relative to the trunk's, for a
/// vasodilation of 0 (cold) to 1 (hot): hands and feet are shut off most in the cold and opened
/// most in the heat.
fn perfusion(r: BodyRegion, dilation: f64) -> f64 {
    match r {
        BodyRegion::Hand | BodyRegion::Foot => 0.3 + 0.9 * dilation,
        BodyRegion::LowerArm | BodyRegion::LowerLeg => 0.65 + 0.35 * dilation,
        BodyRegion::UpperArm | BodyRegion::UpperLeg => 0.85 + 0.15 * dilation,
        _ => 1.0,
    }
}

/// One region's surroundings, per m² of its skin.
struct Surface {
    /// Conductance of dry heat loss (W/m²K) to the operative temperature.
    u_dry: f64,
    /// Evaporative conductance from wet skin (W/m²kPa), and through dry skin.
    u_evap: f64,
    u_diffuse: f64,
    /// Conductance to the ground (lying) and to the water (immersed).
    u_ground: f64,
    u_water: f64,
    /// Radiant heat absorbed and reaching the skin (W/m²).
    radiant: f64,
    /// Rain reaching the skin (kg/m²s), and the share of the region's skin it keeps wet.
    rain: f64,
    w_rain: f64,
}

impl Thermal {
    /// Advances the heat balance by `dt` real seconds.
    pub fn step(&mut self, d: &Drive, e: &Exposure, worn: &Worn, dt: f64) -> Flows {
        let n = (dt / MAX_STEP).ceil().max(1.0) as usize;
        let h = dt / n as f64;
        let mut sum = Flows::default();
        for _ in 0..n {
            let f = self.substep(d, e, worn, h);
            sum.shivering_w += f.shivering_w / n as f64;
            sum.sweat_kg_s += f.sweat_kg_s / n as f64;
            sum.insensible_kg_s += f.insensible_kg_s / n as f64;
            sum.skin_wetness += f.skin_wetness / n as f64;
            sum.loss_w += f.loss_w / n as f64;
        }
        sum
    }

    fn substep(&mut self, d: &Drive, e: &Exposure, worn: &Worn, dt: f64) -> Flows {
        let a = d.area_m2;
        let t_a = e.air_c as f64;
        let v = d.air_speed.max(0.0);
        let h_c = (8.3 * v.powf(0.6)).max(3.1);
        let h = h_c + H_RAD;
        let h_e = 16.5 * h_c;
        let t_mrt = t_a + e.sky_c_offset as f64;
        let t_o = (H_RAD * t_mrt + h_c * t_a) / h;
        let t_w = e.water_c as f64;
        let p_a = (e.humidity as f64).clamp(0.0, 1.0) * p_sat(t_a);
        let wet_frac = (self.wet_kg_m2 / WET_HOLD).clamp(0.0, 1.0);
        let immersed = (e.immersion as f64).clamp(0.0, 1.0);
        let ground = if d.posture == Posture::Lying {
            GROUND_SHARE
        } else {
            0.0
        };
        let in_air = (1.0 - immersed) * (1.0 - ground);
        let rain_fall = e.rain_mm_h.max(0.0) as f64 / 3600.0; // kg per m² of ground per s
        let intercept = rain_fall * (0.08 + 0.03 * v.min(10.0)) * (1.0 - immersed); // per m² of skin

        // Signals and the body's responses.
        let set = CORE_SET + d.fever_c;
        let t_cr = self.core_c;
        let warm_c = (t_cr - set).max(0.0);
        let cold_c = (set - t_cr).max(0.0);
        let warm_s = (self.skin_c - SKIN_SET).max(0.0);
        let cold_s = (SKIN_SET - self.skin_c).max(0.0);
        let skin_blood = ((2.5 + 200.0 * warm_c) / (1.0 + 0.5 * cold_s)).clamp(0.5, 90.0);
        // Working muscles carry blood close to the skin: exercise thins the shell's insulation
        // (swimming in cold water cools faster than floating still).
        let exercise = (d.metabolic_w / d.basal_w.max(1.0) - 1.0).max(0.0);
        let k = K_TISSUE * (1.0 + 0.15 * exercise) + 1.163 * skin_blood;
        // Blood to the hands and feet: wide open in the heat, kept up while the body is warm
        // and its skin not cold, shut off when the body has heat to save.
        let heat_open = ((skin_blood - 0.5) / 20.0).clamp(0.0, 1.0);
        let warm_body =
            ((t_cr - 36.0) / 0.8).clamp(0.0, 1.0) * (1.0 - cold_s / 8.0).clamp(0.0, 1.0);
        let dilation = heat_open.max(0.5 * warm_body);
        // Shivering fails as the core cools past 32 °C.
        let failing = ((t_cr - 30.0) / 2.0).clamp(0.0, 1.0);
        let shiver = (19.4 * cold_s * cold_c * a).min(d.shiver_max_w) * failing;
        let m = (d.metabolic_w + shiver) / a;
        let res_dry = 0.0014 * m * (34.0 - t_a);
        let res_latent = 0.0173 * m * (5.87 - p_a).max(0.0);
        let sweat_g_m2_h =
            (170.0 * warm_c * (warm_s / 10.7).exp()).min(500.0) * d.sweat_capacity.clamp(0.0, 1.0);
        let e_rsw = 0.68 * sweat_g_m2_h;

        // Each region's skin settles where the heat from the core meets the losses.
        let mut q_core = 0.0; // W/m² of body
        let mut env_sum = 0.0;
        let (mut e_dif_sum, mut e_rain_sum, mut wet_sum, mut skin_mean) = (0.0, 0.0, 0.0, 0.0);
        for (i, (&r, c)) in REGIONS.iter().zip(&worn.regions).enumerate() {
            let f = region_area(r);
            let wind_loss = 0.4 * (1.0 - c.wind as f64) * (v / 8.0).min(1.0);
            let through = 1.0 - c.water as f64;
            let soaked = 0.5 * wet_frac * through;
            let clo = c.clo as f64 * (1.0 - wind_loss) * (1.0 - soaked);
            let f_cl = 1.0 + 0.15 * clo;
            let r_cl = 0.155 * clo;
            let r_air = 1.0 / (f_cl * h);
            let s = Surface {
                u_dry: in_air / (r_cl + r_air),
                u_evap: in_air / (0.0209 * clo + 1.0 / (f_cl * h_e)),
                u_diffuse: in_air / (R_SKIN_VAPOUR + 0.0209 * clo + 1.0 / (f_cl * h_e)),
                u_ground: (1.0 - immersed) * ground
                    / (0.155 * e.ground_clo as f64 + 1.0 / H_GROUND),
                u_water: immersed / (0.155 * 0.3 * clo + 1.0 / H_WATER),
                radiant: e.radiant_w_m2.max(0.0) as f64 * in_air * r_air / (r_cl + r_air),
                rain: intercept * through,
                w_rain: (wet_frac * through).max(immersed),
            };
            let k_r = k * perfusion(r, dilation);
            let loss = |t: f64| -> (f64, f64, f64, f64) {
                // Evaporation: sweat first, then rain water, and diffusion through the dry rest.
                let dp = (p_sat(t) - p_a).max(0.0);
                let e_max = s.u_evap * dp;
                let w_rsw = if e_max > 1e-9 {
                    (e_rsw / e_max).min(1.0)
                } else {
                    1.0
                };
                let w_rain = s.w_rain.min(1.0 - w_rsw);
                let e_dif = (1.0 - w_rsw - w_rain).max(0.0) * s.u_diffuse * dp;
                let total = s.u_dry * (t - t_o)
                    + s.u_ground * (t - t_a)
                    + s.u_water * (t - t_w)
                    + (w_rsw + w_rain) * e_max
                    + e_dif
                    + s.rain * 4186.0 * (t - t_a).max(0.0)
                    - s.radiant;
                let w = w_rsw + w_rain + if e_max > 1e-9 { e_dif / e_max } else { 0.0 };
                (total, w_rain * e_max, e_dif, w)
            };
            // The skin temperature where the core's heat equals the loss (bisection: the loss
            // grows with the skin's temperature).
            let (mut lo, mut hi) = (t_o.min(t_w).min(t_a) - 30.0, t_cr.max(t_o).max(t_a) + 10.0);
            for _ in 0..30 {
                let mid = 0.5 * (lo + hi);
                if k_r * (t_cr - mid) > loss(mid).0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let t = 0.5 * (lo + hi);
            let (total, e_rain, e_dif, w) = loss(t);
            q_core += f * k_r * (t_cr - t);
            env_sum += f * total;
            e_rain_sum += f * e_rain;
            e_dif_sum += f * e_dif;
            wet_sum += f * w;
            skin_mean += f * t;
            self.regions_c[i] = t as f32;
        }
        self.skin_c = skin_mean;
        // Balance multipliers act on the heat the surroundings take or give.
        let stress = if env_sum > 0.0 {
            d.cold_stress
        } else {
            d.heat_stress
        };
        let q_core = q_core * stress;

        let core_cap = C_TISSUE * d.mass_kg * CORE_MASS;
        self.core_c += (m - res_dry - res_latent - q_core) * a * dt / core_cap;

        // Water held by skin and clothing: rain and swimming wet it, evaporation dries it.
        let soak = if immersed > 0.5 { WET_HOLD } else { 0.0 };
        self.wet_kg_m2 = (self.wet_kg_m2 + (intercept - e_rain_sum / LATENT) * dt)
            .max(soak)
            .clamp(0.0, WET_HOLD);

        Flows {
            shivering_w: shiver,
            sweat_kg_s: sweat_g_m2_h / 3.6e6 * a,
            insensible_kg_s: (e_dif_sum + res_latent) / LATENT * a,
            skin_wetness: wet_sum.min(1.0),
            loss_w: (env_sum * stress + res_dry + res_latent) * a,
        }
    }
}

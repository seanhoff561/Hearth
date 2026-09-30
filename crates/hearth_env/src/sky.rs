//! Radiometry of the sky on the CPU: the same atmosphere, lookup tables and integration as
//! `hearth_render`'s `shaders/atmosphere.wgsl` (after Hillaire 2020), so the lighting and
//! exposure computed here agree with the sky drawn on the GPU (the `sky_consistency` test
//! checks this). Gives the direct sun and moon light after the atmosphere and the sky's diffuse
//! irradiance, in lux per RGB channel; used for lighting uniforms and exposure, and later for
//! the sun's warmth on the body.

use std::borrow::Cow;
use std::f64::consts::PI;
use std::sync::OnceLock;

use glam::DVec3;

const R_GROUND: f64 = 6_360_000.0;
const R_TOP: f64 = 6_460_000.0;
const RAYLEIGH: Rgb = [5.802e-6, 13.558e-6, 33.1e-6];
const RAYLEIGH_H: f64 = 8_000.0;
const MIE_SCATTER: f64 = 3.996e-5;
const MIE_EXTINCT: f64 = 4.44e-5;
const MIE_H: f64 = 1_200.0;
const MIE_G: f64 = 0.8;
const OZONE: Rgb = [0.650e-6, 1.881e-6, 0.085e-6];
const GROUND_ALBEDO: f64 = 0.3;

/// Solar illuminance at the top of the atmosphere (lux).
pub const SUN_ILLUMINANCE: f64 = 128_000.0;
/// Full-moon illuminance at the top of the atmosphere (lux).
pub const FULL_MOON_ILLUMINANCE: f64 = 0.3;
/// Starlight and airglow on a moonless night (lux).
pub const NIGHT_SKY_ILLUMINANCE: f64 = 0.002;

type Rgb = [f64; 3];

struct Medium {
    rayleigh: Rgb,
    mie: f64,
    scatter: Rgb,
    extinct: Rgb,
}

fn medium(h: f64, haze: f64) -> Medium {
    let rd = (-h / RAYLEIGH_H).exp();
    let md = (-h / MIE_H).exp() * haze;
    let od = (1.0 - (h - 25_000.0).abs() / 15_000.0).max(0.0);
    let rayleigh = RAYLEIGH.map(|b| b * rd);
    let mie = MIE_SCATTER * md;
    Medium {
        rayleigh,
        mie,
        scatter: rayleigh.map(|r| r + mie),
        extinct: [0, 1, 2].map(|i| rayleigh[i] + MIE_EXTINCT * md + OZONE[i] * od),
    }
}

/// Distance along a ray from radius `r` with direction cosine `mu` to a sphere (the nearest
/// crossing ahead), if any.
fn ray_sphere(r: f64, mu: f64, radius: f64) -> Option<f64> {
    let b = r * mu;
    let c = r * r - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let s = disc.sqrt();
    let (t0, t1) = (-b - s, -b + s);
    if t0 > 0.0 {
        Some(t0)
    } else if t1 > 0.0 {
        Some(t1)
    } else {
        None
    }
}

/// 1 where a light in direction cosine `mu` is above the planet's horizon at radius `r`, 0 in
/// its shadow, blended across the sun's angular radius.
fn planet_shadow(r: f64, mu: f64) -> f64 {
    let s = R_GROUND / r;
    let mu_h = -(1.0 - s * s).max(0.0).sqrt();
    smoothstep(mu_h - 0.0047, mu_h + 0.0047, mu)
}

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mie_phase(c: f64) -> f64 {
    let g = MIE_G;
    let k = 3.0 / (8.0 * PI) * (1.0 - g * g) / (2.0 + g * g);
    k * (1.0 + c * c) / (1.0 + g * g - 2.0 * g * c).powf(1.5)
}

fn rayleigh_phase(c: f64) -> f64 {
    3.0 / (16.0 * PI) * (1.0 + c * c)
}

// ------------------------------------------------------------------ lookup tables

const TRANS_SIZE: (usize, usize) = (64, 32);
const MS_SIZE: (usize, usize) = (16, 16);

/// Bruneton's transmittance parameterisation (rays that miss the ground).
fn transmittance_uv(r: f64, mu: f64) -> (f64, f64) {
    let h = (R_TOP * R_TOP - R_GROUND * R_GROUND).sqrt();
    let rho = (r * r - R_GROUND * R_GROUND).max(0.0).sqrt();
    let disc = r * r * (mu * mu - 1.0) + R_TOP * R_TOP;
    let d = (-r * mu + disc.max(0.0).sqrt()).max(0.0);
    let (d_min, d_max) = (R_TOP - r, rho + h);
    ((d - d_min) / (d_max - d_min).max(1e-3), rho / h)
}

fn transmittance_rmu(u: f64, v: f64) -> (f64, f64) {
    let h = (R_TOP * R_TOP - R_GROUND * R_GROUND).sqrt();
    let rho = h * v;
    let r = (rho * rho + R_GROUND * R_GROUND).sqrt();
    let (d_min, d_max) = (R_TOP - r, rho + h);
    let d = d_min + u * (d_max - d_min);
    let mu = if d > 0.0 {
        ((h * h - rho * rho - d * d) / (2.0 * r * d)).clamp(-1.0, 1.0)
    } else {
        1.0
    };
    (r, mu)
}

/// Bilinear lookup with texel centres at (i + 0.5) / size and clamping at the edges, like a
/// GPU sampler.
fn sample(table: &[Rgb], size: (usize, usize), u: f64, v: f64) -> Rgb {
    let fx = (u * size.0 as f64 - 0.5).clamp(0.0, (size.0 - 1) as f64);
    let fy = (v * size.1 as f64 - 0.5).clamp(0.0, (size.1 - 1) as f64);
    let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(size.0 - 1), (y0 + 1).min(size.1 - 1));
    let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
    let at = |x: usize, y: usize| table[y * size.0 + x];
    let (a, b, c, d) = (at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1));
    [0, 1, 2].map(|k| {
        (a[k] * (1.0 - tx) + b[k] * tx) * (1.0 - ty) + (c[k] * (1.0 - tx) + d[k] * tx) * ty
    })
}

/// Transmittance and multiple-scattering tables for one aerosol density. The multiple
/// scattering is stored as a logarithm: in twilight it falls tenfold every degree or two, and
/// interpolating it linearly would smear daylight into the dusk.
#[derive(Clone)]
struct Tables {
    trans: Vec<Rgb>,
    ms_log: Vec<Rgb>,
}

/// Smallest value the logarithmic tables hold.
const LOG_FLOOR: f64 = 1e-30;

impl Tables {
    fn build(haze: f64) -> Self {
        let (tw, th) = TRANS_SIZE;
        let mut trans = vec![[0.0; 3]; tw * th];
        for j in 0..th {
            for i in 0..tw {
                let (r, mu) =
                    transmittance_rmu((i as f64 + 0.5) / tw as f64, (j as f64 + 0.5) / th as f64);
                let t_max = ray_sphere(r, mu, R_TOP).unwrap_or(0.0).max(0.0);
                let steps = 40;
                let dt = t_max / steps as f64;
                let mut depth = [0.0; 3];
                for s in 0..steps {
                    let t = (s as f64 + 0.5) * dt;
                    let h = (r * r + t * t + 2.0 * r * mu * t).sqrt() - R_GROUND;
                    let e = medium(h, haze).extinct;
                    for k in 0..3 {
                        depth[k] += e[k] * dt;
                    }
                }
                trans[j * tw + i] = depth.map(|d| (-d).exp());
            }
        }
        let mut tables = Self {
            trans,
            ms_log: Vec::new(),
        };
        let (mw, mh) = MS_SIZE;
        let mut ms = vec![[0.0; 3]; mw * mh];
        for j in 0..mh {
            for i in 0..mw {
                let (u, v) = ((i as f64 + 0.5) / mw as f64, (j as f64 + 0.5) / mh as f64);
                let psi = tables.multiscatter_texel(
                    u * 2.0 - 1.0,
                    R_GROUND + v * (R_TOP - R_GROUND) + 1.0,
                    haze,
                );
                ms[j * mw + i] = psi.map(|x| x.max(LOG_FLOOR).ln());
            }
        }
        tables.ms_log = ms;
        tables
    }

    /// Second-order scattered light from all directions and the transfer factor (Hillaire's
    /// psi_ms = L2 / (1 - f_ms)), as `multiscatter_main` computes it.
    fn multiscatter_texel(&self, sun_mu: f64, r: f64, haze: f64) -> Rgb {
        let sun = DVec3::new((1.0 - sun_mu * sun_mu).max(0.0).sqrt(), sun_mu, 0.0);
        let n_dir = 8;
        let mut lum = [0.0; 3];
        let mut fms = [0.0; 3];
        for a in 0..n_dir {
            for b in 0..n_dir {
                let theta = PI * (a as f64 + 0.5) / n_dir as f64;
                let phi = 2.0 * PI * (b as f64 + 0.5) / n_dir as f64;
                let dir = DVec3::new(
                    theta.sin() * phi.cos(),
                    theta.cos(),
                    theta.sin() * phi.sin(),
                );
                let t_ground = ray_sphere(r, dir.y, R_GROUND);
                let t_max = t_ground
                    .or_else(|| ray_sphere(r, dir.y, R_TOP))
                    .unwrap_or(0.0)
                    .max(0.0);
                let steps = 20;
                let dt = t_max / steps as f64;
                let mut trans = [1.0; 3];
                let mut l = [0.0; 3];
                let mut f = [0.0; 3];
                for s in 0..steps {
                    let t = (s as f64 + 0.5) * dt;
                    let p = DVec3::new(0.0, r, 0.0) + dir * t;
                    let pr = p.length();
                    let m = medium(pr - R_GROUND, haze);
                    let lmu = (p / pr).dot(sun);
                    let sun_t = self.transmittance(pr, lmu);
                    let shadow = planet_shadow(pr, lmu);
                    for k in 0..3 {
                        let step_t = (-m.extinct[k] * dt).exp();
                        let ext = m.extinct[k].max(1e-9);
                        let s_k = m.scatter[k] / (4.0 * PI) * sun_t[k] * shadow;
                        l[k] += trans[k] * (s_k - s_k * step_t) / ext;
                        f[k] += trans[k] * (m.scatter[k] - m.scatter[k] * step_t) / ext;
                        trans[k] *= step_t;
                    }
                }
                if let Some(tg) = t_ground {
                    let p = DVec3::new(0.0, r, 0.0) + dir * tg;
                    let up = p.normalize();
                    let cos = up.dot(sun);
                    let gt = self.transmittance(p.length(), cos);
                    for k in 0..3 {
                        l[k] += trans[k] * gt[k] * cos.max(0.0) * GROUND_ALBEDO / PI;
                    }
                }
                let w = theta.sin() * (PI / n_dir as f64) * (2.0 * PI / n_dir as f64) / (4.0 * PI);
                for k in 0..3 {
                    lum[k] += l[k] * w;
                    fms[k] += f[k] * w;
                }
            }
        }
        [0, 1, 2].map(|k| lum[k] / (1.0 - fms[k]))
    }

    fn transmittance(&self, r: f64, mu: f64) -> Rgb {
        let (u, v) = transmittance_uv(r, mu);
        sample(&self.trans, TRANS_SIZE, u, v)
    }
}

/// Aerosol densities the tables are built for; others blend between neighbours.
const HAZE_LEVELS: [f64; 5] = [0.5, 1.0, 2.0, 4.0, 8.0];

fn level_tables(i: usize) -> &'static Tables {
    static LEVELS: [OnceLock<Tables>; 5] = [const { OnceLock::new() }; 5];
    LEVELS[i].get_or_init(|| Tables::build(HAZE_LEVELS[i]))
}

/// The haze level at or below `haze` and the weight of the next one.
fn haze_bracket(haze: f64) -> (usize, f64) {
    let h = haze.clamp(HAZE_LEVELS[0], HAZE_LEVELS[HAZE_LEVELS.len() - 1]);
    let i = HAZE_LEVELS
        .windows(2)
        .position(|w| h <= w[1])
        .unwrap_or(HAZE_LEVELS.len() - 2);
    let (a, b) = (HAZE_LEVELS[i], HAZE_LEVELS[i + 1]);
    (i, (h - a) / (b - a))
}

/// The atmosphere for one aerosol density (`haze` 1 = clear air).
pub struct Atmosphere {
    haze: f64,
    tables: Cow<'static, Tables>,
}

impl Atmosphere {
    /// The atmosphere at one of the precomputed haze levels (no tables to blend).
    fn level(i: usize) -> Self {
        Self {
            haze: HAZE_LEVELS[i],
            tables: Cow::Borrowed(level_tables(i)),
        }
    }

    pub fn new(haze: f64) -> Self {
        let h = haze.clamp(HAZE_LEVELS[0], HAZE_LEVELS[HAZE_LEVELS.len() - 1]);
        let (i, w) = haze_bracket(h);
        let tables = if w < 1e-6 {
            Cow::Borrowed(level_tables(i))
        } else if w > 1.0 - 1e-6 {
            Cow::Borrowed(level_tables(i + 1))
        } else {
            // Optical depth is linear in the aerosol density: blend transmittance in log space.
            let (ta, tb) = (level_tables(i), level_tables(i + 1));
            let blend_log = |x: &Rgb, y: &Rgb| {
                [0, 1, 2]
                    .map(|k| (x[k].max(1e-30).ln() * (1.0 - w) + y[k].max(1e-30).ln() * w).exp())
            };
            let blend = |x: &Rgb, y: &Rgb| [0, 1, 2].map(|k| x[k] * (1.0 - w) + y[k] * w);
            Cow::Owned(Tables {
                trans: ta
                    .trans
                    .iter()
                    .zip(&tb.trans)
                    .map(|(x, y)| blend_log(x, y))
                    .collect(),
                ms_log: ta
                    .ms_log
                    .iter()
                    .zip(&tb.ms_log)
                    .map(|(x, y)| blend(x, y))
                    .collect(),
            })
        };
        Self { haze: h, tables }
    }

    /// Transmittance from radius `r` toward direction cosine `mu` to space, 0 behind the planet.
    pub fn transmittance(&self, r: f64, mu: f64) -> Rgb {
        let shadow = planet_shadow(r, mu);
        if shadow <= 0.0 {
            return [0.0; 3];
        }
        self.tables.transmittance(r, mu).map(|t| t * shadow)
    }

    fn multiscatter(&self, r: f64, sun_mu: f64) -> Rgb {
        let log = sample(
            &self.tables.ms_log,
            MS_SIZE,
            sun_mu * 0.5 + 0.5,
            (r - R_GROUND) / (R_TOP - R_GROUND),
        );
        log.map(f64::exp)
    }

    /// Sky radiance (per unit illuminance of the light) in direction `dir` from altitude `alt`,
    /// single plus multiple scattering, as `scatter_ray` on the GPU.
    pub fn sky_radiance(&self, alt: f64, dir: DVec3, light: DVec3, steps: usize) -> Rgb {
        let r = R_GROUND + alt.max(1.0);
        let t_max = ray_sphere(r, dir.y, R_GROUND)
            .or_else(|| ray_sphere(r, dir.y, R_TOP))
            .unwrap_or(0.0);
        if t_max <= 0.0 {
            return [0.0; 3];
        }
        let c = dir.dot(light);
        let (ph_r, ph_m) = (rayleigh_phase(c), mie_phase(c));
        let mut trans = [1.0; 3];
        let mut out = [0.0; 3];
        let mut t_prev = 0.0;
        for i in 0..steps {
            let f = (i as f64 + 0.5) / steps as f64;
            let t = t_max * f * f;
            let dt = (t - t_prev).max(1.0);
            t_prev = t;
            let p = DVec3::new(0.0, r, 0.0) + dir * t;
            let pr = p.length();
            let m = medium(pr - R_GROUND, self.haze);
            let light_mu = (p / pr).dot(light);
            let lt = self.transmittance(pr, light_mu);
            let ms = self.multiscatter(pr, light_mu);
            for k in 0..3 {
                let single = (m.rayleigh[k] * ph_r + m.mie * ph_m) * lt[k];
                let s = single + m.scatter[k] * ms[k];
                let step_t = (-m.extinct[k] * dt).exp();
                out[k] += trans[k] * (s - s * step_t) / m.extinct[k].max(1e-9);
                trans[k] *= step_t;
            }
        }
        out
    }

    /// Diffuse irradiance on a horizontal surface from the sky lit by one light (per unit
    /// illuminance of the light). Depends only on the light's elevation, so the sky is
    /// integrated in a frame with the light at azimuth 0 and mirrored.
    pub fn sky_irradiance(&self, alt: f64, light: DVec3) -> Rgb {
        self.sky_irradiance_with(alt, light, 10, 8, 16)
    }

    /// `sky_irradiance` with a chosen resolution: `n_v` elevations, `n_az` azimuths over half
    /// the sky and `steps` samples per ray.
    pub fn sky_irradiance_with(
        &self,
        alt: f64,
        light: DVec3,
        n_v: usize,
        n_az: usize,
        steps: usize,
    ) -> Rgb {
        let mu = light.y.clamp(-1.0, 1.0);
        let light = DVec3::new((1.0 - mu * mu).max(0.0).sqrt(), mu, 0.0);
        // Elevation spacing as the GPU's sky-view table: denser toward the horizon.
        let mut sum = [0.0; 3];
        for i in 0..n_v {
            let v = (i as f64 + 0.5) / n_v as f64;
            let el = v * v * PI / 2.0;
            let d_el = v * PI / n_v as f64;
            for j in 0..n_az {
                // Half the circle, counted twice (the sky is symmetric about the light's plane).
                let az = (j as f64 + 0.5) / n_az as f64 * PI;
                let dir = DVec3::new(az.cos() * el.cos(), el.sin(), az.sin() * el.cos());
                let l = self.sky_radiance(alt, dir, light, steps);
                let w = el.sin() * el.cos() * d_el * (2.0 * PI / n_az as f64);
                for k in 0..3 {
                    sum[k] += l[k] * w;
                }
            }
        }
        sum
    }
}

/// Steps of the sky-irradiance nodes: light elevation (cosine) and altitude.
const NODE_MU: f64 = 0.01;
const NODE_ALT: f64 = 50.0;
/// The whole sky is in the planet's shadow once a light is ~20 degrees below the horizon.
const SKY_DARK_MU: f64 = -0.35;

/// `SkyLight` for a frame loop. The sky's irradiance — an integral over the whole sky and the
/// costly part (about 0.13 ms per light) — depends only on the light's elevation, the altitude
/// and the haze, so it is computed at nodes (steps of 0.01 in the elevation's cosine and of 50
/// in altitude, at the atmosphere's haze levels) and interpolated between them: a frame
/// integrates only when a light or the camera reaches a new node. The atmosphere for the
/// direct light is kept while the haze stays within half a percent.
#[derive(Default)]
pub struct SkyLightCache {
    nodes: rustc_hash::FxHashMap<(i32, i32, u8), Rgb>,
    atmosphere: Option<Atmosphere>,
}

impl SkyLightCache {
    fn node(&mut self, mu_i: i32, alt_i: i32, level: usize) -> Rgb {
        if self.nodes.len() > 1 << 16 {
            self.nodes.clear();
        }
        *self
            .nodes
            .entry((mu_i, alt_i, level as u8))
            .or_insert_with(|| {
                let mu = (mu_i as f64 * NODE_MU).clamp(-1.0, 1.0);
                if mu < SKY_DARK_MU {
                    return [0.0; 3];
                }
                let light = DVec3::new((1.0 - mu * mu).max(0.0).sqrt(), mu, 0.0);
                Atmosphere::level(level).sky_irradiance(alt_i as f64 * NODE_ALT, light)
            })
    }

    /// Diffuse irradiance on a horizontal surface from the sky lit by a light whose elevation
    /// has cosine `mu` (per unit illuminance of the light), as `sky_irradiance`.
    pub fn sky_irradiance(&mut self, alt: f64, mu: f64, haze: f64) -> Rgb {
        let (level, lw) = haze_bracket(haze);
        let fm = mu.clamp(-1.0, 1.0) / NODE_MU;
        let fa = alt.max(0.0) / NODE_ALT;
        let (m0, a0) = (fm.floor(), fa.floor());
        let (tm, ta) = (fm - m0, fa - a0);
        // In log space: through twilight the sky's light falls off exponentially with the
        // light's elevation, and linear interpolation would overestimate it.
        let mut log = [0.0; 3];
        for (dl, wl) in [(0, 1.0 - lw), (1, lw)] {
            for (dm, wm) in [(0, 1.0 - tm), (1, tm)] {
                for (da, wa) in [(0, 1.0 - ta), (1, ta)] {
                    let w = wl * wm * wa;
                    if w <= 0.0 {
                        continue;
                    }
                    let v = self.node(m0 as i32 + dm, a0 as i32 + da, level + dl);
                    for k in 0..3 {
                        log[k] += v[k].max(1e-12).ln() * w;
                    }
                }
            }
        }
        log.map(|l| if l < -25.0 { 0.0 } else { l.exp() })
    }

    /// The atmosphere for `haze`, rebuilt only when the haze has moved by half a percent.
    fn atmosphere(&mut self, haze: f64) -> &Atmosphere {
        let stale = self
            .atmosphere
            .as_ref()
            .is_none_or(|a| (a.haze - haze).abs() > 0.005 * haze);
        if stale {
            self.atmosphere = Some(Atmosphere::new(haze));
        }
        self.atmosphere.as_ref().expect("set above")
    }
}

/// Transmittance from radius `r` toward a direction with cosine `mu` to space (0 if the
/// planet is in the way).
pub fn transmittance(r: f64, mu: f64, haze: f64) -> Rgb {
    Atmosphere::new(haze).transmittance(r, mu)
}

/// Diffuse irradiance on a horizontal surface from the sky lit by one light (per unit
/// illuminance of the light).
pub fn sky_irradiance(alt: f64, light: DVec3, haze: f64) -> Rgb {
    Atmosphere::new(haze).sky_irradiance(alt, light)
}

/// Moon illuminance (lux) for a phase (0 new, 0.5 full), with the opposition surge.
pub fn moon_illuminance(phase: f64) -> f64 {
    let angle = ((phase - 0.5).abs() * 360.0).min(180.0);
    if angle >= 170.0 {
        return 0.0;
    }
    FULL_MOON_ILLUMINANCE * 10f64.powf(-0.4 * (0.026 * angle + 4e-9 * angle.powi(4)))
}

/// Lighting environment at a place and moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkyLight {
    /// Direct sun illuminance on a surface facing the sun (lux).
    pub sun: Rgb,
    /// Direct moon illuminance on a surface facing the moon (lux).
    pub moon: Rgb,
    /// Diffuse sky irradiance on a horizontal surface (lux), sun + moon + night sky.
    pub sky: Rgb,
}

impl SkyLight {
    /// `cloud_cover` 0..1 dims direct light and turns part of it into diffuse light.
    pub fn compute(
        alt: f64,
        sun_dir: DVec3,
        moon_dir: DVec3,
        moon_phase: f64,
        cloud_cover: f64,
        haze: f64,
    ) -> Self {
        let atmosphere = Atmosphere::new(haze);
        let irradiance = |dir: DVec3| {
            if dir.y < SKY_DARK_MU {
                [0.0; 3]
            } else {
                atmosphere.sky_irradiance(alt, dir)
            }
        };
        let s_irr = irradiance(sun_dir);
        let m_irr = if moon_illuminance(moon_phase) > 0.0 {
            irradiance(moon_dir)
        } else {
            [0.0; 3]
        };
        Self::assemble(
            &atmosphere,
            alt,
            (sun_dir, s_irr),
            (moon_dir, m_irr),
            moon_phase,
            cloud_cover,
        )
    }

    /// As `compute`, with the sky's irradiance interpolated from `cache` (for a frame loop).
    pub fn compute_cached(
        cache: &mut SkyLightCache,
        alt: f64,
        sun_dir: DVec3,
        moon_dir: DVec3,
        moon_phase: f64,
        cloud_cover: f64,
        haze: f64,
    ) -> Self {
        let s_irr = cache.sky_irradiance(alt, sun_dir.y, haze);
        let m_irr = if moon_illuminance(moon_phase) > 0.0 {
            cache.sky_irradiance(alt, moon_dir.y, haze)
        } else {
            [0.0; 3]
        };
        let atmosphere = cache.atmosphere(haze);
        Self::assemble(
            atmosphere,
            alt,
            (sun_dir, s_irr),
            (moon_dir, m_irr),
            moon_phase,
            cloud_cover,
        )
    }

    /// Direct light through the atmosphere, the sky's irradiance by each light, and clouds.
    fn assemble(
        atmosphere: &Atmosphere,
        alt: f64,
        (sun_dir, s_irr): (DVec3, Rgb),
        (moon_dir, m_irr): (DVec3, Rgb),
        moon_phase: f64,
        cloud_cover: f64,
    ) -> Self {
        let r = R_GROUND + alt.max(1.0);
        let st = atmosphere.transmittance(r, sun_dir.y);
        let sun = st.map(|t| t * SUN_ILLUMINANCE);
        let m_illum = moon_illuminance(moon_phase);
        let mt = atmosphere.transmittance(r, moon_dir.y);
        let moon = mt.map(|t| t * m_illum);
        let mut sky = [0.0; 3];
        for k in 0..3 {
            sky[k] = s_irr[k] * SUN_ILLUMINANCE + m_irr[k] * m_illum + NIGHT_SKY_ILLUMINANCE;
        }
        let cc = cloud_cover.clamp(0.0, 1.0);
        let direct_keep = 1.0 - 0.92 * cc.powf(1.5);
        let horizontal = |d: Rgb, y: f64| d.map(|v| v * y.max(0.0));
        let sun_h = horizontal(sun, sun_dir.y);
        let moon_h = horizontal(moon, moon_dir.y);
        for k in 0..3 {
            // Overcast: the lost direct light partly comes through as diffuse grey light.
            let lost = (sun_h[k] + moon_h[k]) * (1.0 - direct_keep);
            let grey = (sun_h[1] + moon_h[1]) * (1.0 - direct_keep);
            sky[k] = sky[k] * (1.0 - 0.6 * cc) + 0.35 * (0.5 * lost + 0.5 * grey);
        }
        Self {
            sun: sun.map(|v| v * direct_keep),
            moon: moon.map(|v| v * direct_keep),
            sky,
        }
    }

    /// Illuminance on a horizontal surface (lux, green channel).
    pub fn horizontal_lux(&self, sun_dir: DVec3, moon_dir: DVec3) -> f64 {
        self.sun[1] * sun_dir.y.max(0.0) + self.moon[1] * moon_dir.y.max(0.0) + self.sky[1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_sky_light_matches_the_exact_integral() {
        let mut cache = SkyLightCache::default();
        let mut worst: f64 = 0.0;
        for &haze in &[1.0, 1.37, 2.6] {
            for &alt in &[0.0, 23.0, 180.0, 420.0] {
                for k in 0..24 {
                    let mu = -0.3 + k as f64 * 0.055;
                    let sun = DVec3::new((1.0 - mu * mu).sqrt(), mu, 0.0);
                    let moon = -sun;
                    let exact = SkyLight::compute(alt, sun, moon, 0.5, 0.3, haze);
                    let cached =
                        SkyLight::compute_cached(&mut cache, alt, sun, moon, 0.5, 0.3, haze);
                    let e = exact.sky[1].max(1e-3);
                    worst = worst.max(((cached.sky[1] - exact.sky[1]) / e).abs());
                    assert_eq!(cached.sun, exact.sun, "direct light is not interpolated");
                }
            }
        }
        // Within a couple of percent of the sky light, twilight included.
        assert!(worst < 0.025, "sky light off by {:.2} %", worst * 100.0);
    }

    fn up_at(elev_deg: f64) -> DVec3 {
        let e = elev_deg.to_radians();
        DVec3::new(0.0, e.sin(), e.cos())
    }

    #[test]
    fn daylight_levels_are_realistic() {
        // Clear noon: about 100 klx in all, the sky giving 10-20 % (CIE clear sky: about
        // 15 klx diffuse with the sun at 60 degrees).
        let noon = SkyLight::compute(0.0, up_at(60.0), up_at(-60.0), 0.0, 0.0, 1.0);
        let lux = noon.horizontal_lux(up_at(60.0), up_at(-60.0));
        assert!((85_000.0..120_000.0).contains(&lux), "clear noon {lux}");
        assert!(
            (8_000.0..20_000.0).contains(&noon.sky[1]),
            "diffuse {:?}",
            noon.sky
        );
        assert!(noon.sky[2] > noon.sky[0], "the sky is blue");
        // Haze brightens the sky and dims the sun.
        let hazy = SkyLight::compute(0.0, up_at(60.0), up_at(-60.0), 0.0, 0.0, 3.0);
        assert!(hazy.sky[1] > 1.3 * noon.sky[1] && hazy.sun[1] < noon.sun[1]);
        // Low sun is reddened.
        let low = SkyLight::compute(0.0, up_at(3.0), up_at(-60.0), 0.0, 0.0, 1.0);
        assert!(
            low.sun[0] > 1.5 * low.sun[2],
            "sunset light is red {:?}",
            low.sun
        );
        // Overcast noon is several times darker.
        let overcast = SkyLight::compute(0.0, up_at(60.0), up_at(-60.0), 0.0, 1.0, 1.0);
        let lux_o = overcast.horizontal_lux(up_at(60.0), up_at(-60.0));
        assert!(lux_o < lux * 0.45 && lux_o > lux * 0.05, "overcast {lux_o}");
    }

    #[test]
    fn twilight_follows_measured_illuminance() {
        // Clear-sky horizontal illuminance against the sun's elevation (published tables:
        // about 750 lx at sunset, 3.4 lx at the end of civil twilight, 0.008 lx at the end of
        // nautical twilight). The model is allowed a factor of about three.
        let at = |elev: f64| {
            let l = SkyLight::compute(0.0, up_at(elev), up_at(-60.0), 0.0, 0.0, 1.0);
            l.horizontal_lux(up_at(elev), up_at(-60.0))
        };
        let sunset = at(0.0);
        assert!((300.0..1_500.0).contains(&sunset), "sunset {sunset}");
        let civil = at(-6.0);
        assert!((1.5..10.0).contains(&civil), "civil twilight {civil}");
        let deep = at(-8.0);
        assert!((0.1..1.5).contains(&deep), "sun 8 degrees down {deep}");
        let nautical = at(-12.0);
        assert!(nautical < 0.03, "nautical twilight {nautical}");
        // Monotonic through the evening.
        let mut prev = f64::INFINITY;
        for e in (-20..=10).rev() {
            let v = at(e as f64);
            assert!(v <= prev * 1.001, "brightens at {e} degrees");
            prev = v;
        }
    }

    #[test]
    fn night() {
        let night = SkyLight::compute(0.0, up_at(-30.0), up_at(-60.0), 0.0, 0.0, 1.0);
        let lux_n = night.horizontal_lux(up_at(-30.0), up_at(-60.0));
        assert!(lux_n < 0.01, "moonless night {lux_n}");
        let moonlit = SkyLight::compute(0.0, up_at(-30.0), up_at(50.0), 0.5, 0.0, 1.0);
        let lux_m = moonlit.horizontal_lux(up_at(-30.0), up_at(50.0));
        assert!((0.1..0.4).contains(&lux_m), "full moon {lux_m}");
        assert!(
            moon_illuminance(0.25) < 0.1 * moon_illuminance(0.5),
            "a quarter moon is much dimmer"
        );
        assert_eq!(moon_illuminance(0.0), 0.0);
    }

    #[test]
    fn haze_levels_blend_smoothly() {
        let sun = up_at(20.0);
        let e = |h: f64| sky_irradiance(0.0, sun, h)[1];
        let (a, b, c) = (e(1.0), e(1.5), e(2.0));
        assert!(a < b && b < c, "{a} {b} {c}");
        let t = |h: f64| transmittance(R_GROUND + 1.0, 0.5, h)[1];
        assert!(t(1.0) > t(1.5) && t(1.5) > t(2.0));
    }
}

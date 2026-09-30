//! The environment at the camera: sun, moon and stars for the date and place, the weather,
//! and the resulting lighting (`hearth_env`) in the form the renderer takes.

use std::sync::Arc;

use glam::{DVec3, Mat3, Vec2, Vec3};
use hearth_env::climate::Normals;
use hearth_env::sky::{SkyLight, SkyLightCache};
use hearth_env::weather::Precip;
use hearth_env::{Calendar, Moment, WeatherModel, WeatherState, astro};
use hearth_math::Planet;
use hearth_render::precip::Precipitation;
use hearth_render::scene::Environment;
use hearth_worldgen::PlanetGrid;

/// Samples everything time- and place-dependent for rendering.
pub struct EnvSampler {
    pub planet: Planet,
    pub grid: Arc<PlanetGrid>,
    pub weather: WeatherModel,
    pub calendar: Calendar,
    /// The sky's irradiance, integrated at nodes and interpolated from frame to frame.
    sky: std::cell::RefCell<SkyLightCache>,
}

/// Optional overrides for tests and screenshots.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnvOverrides {
    pub cloud_cover: Option<f64>,
    /// Precipitation type and rate (mm/h).
    pub precipitation: Option<(Precip, f64)>,
}

/// Particles for the weather's precipitation: a heavy shower (8 mm/h) or a heavy snowfall
/// (2 mm/h of water) fills the air.
pub fn precipitation(w: &WeatherState) -> Precipitation {
    let (rain, full_rate) = match w.precip {
        Precip::None => (1.0, f64::INFINITY),
        Precip::Rain => (1.0, 8.0),
        Precip::Sleet => (0.5, 4.0),
        Precip::Snow => (0.0, 2.0),
    };
    let (s, c) = w.wind_dir.sin_cos();
    Precipitation {
        intensity: (w.precip_mm_h / full_rate).clamp(0.0, 1.0) as f32,
        rain,
        wind: Vec3::new(s as f32, 0.0, -c as f32) * w.wind_speed_m_s as f32,
    }
}

impl EnvSampler {
    pub fn new(grid: Arc<PlanetGrid>, calendar: Calendar) -> Self {
        let planet = *grid.planet();
        Self {
            weather: WeatherModel::new(grid.seed, planet),
            planet,
            grid,
            calendar,
            sky: Default::default(),
        }
    }

    /// Local solar time at world X for a moment.
    pub fn local_time(&self, m: &Moment, x: f64) -> f64 {
        m.local_time(self.planet.solar_time_offset(x))
    }

    pub fn sample(
        &self,
        m: &Moment,
        cam: DVec3,
        block_light_at_camera: f32,
        o: EnvOverrides,
    ) -> (Environment, WeatherState) {
        let lat = self.planet.latitude_deg(cam.z);
        let local = self.local_time(m, cam.x);
        let tilt = self.calendar.axial_tilt_deg;
        let sun = astro::sun(lat, m.year_frac, local, tilt);
        let moon = astro::moon(lat, m.year_frac, local, m.moon_phase, tilt);
        let normals = Normals::sample(&self.grid, cam.x, cam.z);
        let mut w = self
            .weather
            .sample(&normals, cam.x, cam.z, m.days, m.year_frac, local);
        if let Some(c) = o.cloud_cover {
            w.cloud_cover = c.clamp(0.0, 1.0);
        }
        if let Some((kind, rate)) = o.precipitation {
            w.precip = kind;
            w.precip_mm_h = if kind == Precip::None {
                0.0
            } else {
                rate.max(0.0)
            };
        }
        let haze = 1.0 + 1.5 * (w.humidity - 0.5).max(0.0) + (w.precip_mm_h / 4.0).min(1.5);
        let light = SkyLight::compute_cached(
            &mut self.sky.borrow_mut(),
            cam.y.max(0.0),
            sun.dir,
            moon.dir,
            m.moon_phase,
            w.cloud_cover,
            haze,
        );
        let rot = astro::sky_rotation(lat, m.year_frac, local);
        let v3 = |a: [f64; 3]| Vec3::new(a[0] as f32, a[1] as f32, a[2] as f32);
        let star_rotation = Mat3::from_cols(
            rot.x_axis.as_vec3(),
            rot.y_axis.as_vec3(),
            rot.z_axis.as_vec3(),
        );
        // Clouds drift with the wind (time-lapsed like the rest of the day scale).
        let drift = (m.days * 86_400.0 * 0.02) as f32 * w.wind_speed_m_s as f32;
        let cloud_offset = Vec2::new((w.wind_dir.sin()) as f32, -(w.wind_dir.cos()) as f32) * drift;
        let env = Environment {
            sun_dir: sun.dir.as_vec3(),
            moon_dir: moon.dir.as_vec3(),
            moon_phase: m.moon_phase as f32,
            sun_lux: v3(light.sun),
            moon_lux: v3(light.moon),
            sky_lux: v3(light.sky),
            year_frac: m.year_frac as f32,
            seconds: (m.days * self.calendar.day_length_s) as f32 % 100_000.0,
            wind: (w.wind_speed_m_s / 6.0).clamp(0.3, 3.0) as f32,
            star_rotation,
            cloud_cover: w.cloud_cover as f32,
            cloud_base: (600.0 + 700.0 * (1.0 - w.humidity)) as f32,
            cloud_offset,
            haze: haze as f32,
            block_light_at_camera,
            aerial_perspective: true,
            exposure_bias: 1.0,
            precipitation: precipitation(&w),
        };
        (env, w)
    }
}

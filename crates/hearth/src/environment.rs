//! The environment at the camera: sun, moon and stars for the date and place, the weather,
//! and the resulting lighting (`hearth_env`) in the form the renderer takes.

use std::sync::Arc;

use glam::{DVec2, DVec3, Mat3, Vec2, Vec3};
use hearth_env::calendar::DAY_S;
use hearth_env::climate::Normals;
use hearth_env::sky::{SkyLight, SkyLightCache};
use hearth_env::weather::Precip;
use hearth_env::{Calendar, Moment, WeatherModel, WeatherState};
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
    /// Weather held as given (tests and bots).
    pub hold: Option<hearth_env::weather::WeatherHold>,
    /// The sky's irradiance, integrated at nodes and interpolated from frame to frame.
    sky: std::cell::RefCell<SkyLightCache>,
    /// What moves in the sky at real speeds, carried from frame to frame.
    motion: std::cell::RefCell<Motion>,
}

/// How far one sample may carry the sky's motion (real seconds): beyond, time has jumped (a
/// skip, a sleep, a load), and the clouds and the wind's turn take only this step rather than
/// sweeping across the sky in a frame.
const MOTION_STEP_S: f64 = 2.0;

/// How quickly the wind's direction, as the clouds and waves show it, follows the weather's
/// (seconds): a turn of the wind turns them over minutes, not at a stroke.
const WIND_TURN_S: f64 = 120.0;

/// How fast the clouds' smaller shapes (some 300 m across) slide against their larger (m/s): a
/// cumulus lives some ten or twenty minutes, so a cloud is another in about ten.
pub const CLOUD_CHURN_M_S: f64 = 0.5;

/// What moves in the sky at real speeds, carried from frame to frame (Amendment P §8).
#[derive(Debug, Clone, Copy, Default)]
struct Motion {
    /// The real seconds of the last sample.
    at: Option<f64>,
    /// How far the clouds have drifted (m).
    clouds: DVec2,
    /// How far the clouds' smaller shapes have slid against their larger (m).
    churn: f64,
    /// How far the air near the ground has carried falling rain and snow (m).
    air: DVec2,
    /// The way the wind blows, as the clouds and waves follow it (radians, eased).
    wind_dir: Option<f64>,
}

/// The wind at the cloud base from the wind at ten metres: the wind's power law (exponent 1/7
/// over open ground), about twice as strong at 1.5 km.
pub fn wind_aloft(surface_m_s: f64, base_m: f64) -> f64 {
    surface_m_s * (base_m.max(10.0) / 10.0).powf(1.0 / 7.0)
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
        drift: DVec2::ZERO,
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
            hold: None,
            sky: Default::default(),
            motion: Default::default(),
        }
    }

    /// Local solar time at world X for a moment.
    pub fn local_time(&self, m: &Moment, x: f64) -> f64 {
        m.local_time(self.planet.solar_time_offset(x))
    }

    /// The climate's normals at a place (its year's mean temperature and rain).
    pub fn normals(&self, at: DVec3) -> hearth_env::Normals {
        hearth_env::Normals::sample(&self.grid, at.x, at.z)
    }

    /// The weather at a place (without the sky's light).
    pub fn weather_at(&self, m: &Moment, at: DVec3) -> WeatherState {
        let local = self.local_time(m, at.x);
        let normals = Normals::sample(&self.grid, at.x, at.z);
        let mut w = self
            .weather
            .sample(&normals, at.x, at.z, m.days, m.year_frac, local);
        if let Some(h) = &self.hold {
            h.apply(&mut w);
        }
        w
    }

    /// How light it is at a place to see by (0 a dark night … 1 day): the sun's height through
    /// dusk and dawn, a little light of the moon and stars at night.
    pub fn daylight(&self, m: &Moment, at: DVec3) -> f32 {
        let sun = self.sun_seen(m, at);
        let s = ((sun.dir.y + 0.1) / 0.2).clamp(0.0, 1.0);
        (s * s * (3.0 - 2.0 * s)).max(0.04) as f32
    }

    /// The air as the animals sense by it at a place: the way the wind blows, how hard, and the
    /// light.
    pub fn air_at(&self, m: &Moment, at: DVec3) -> hearth_fauna::mind::Air {
        let w = self.weather_at(m, at);
        // The wind blows toward `wind_dir` (0 north, which is -z; a quarter turn east, +x).
        let wind = glam::DVec2::new(w.wind_dir.sin(), -w.wind_dir.cos());
        hearth_fauna::mind::Air {
            wind,
            wind_speed: w.wind_speed_m_s as f32,
            light: self.daylight(m, at),
        }
    }

    /// The sun as seen from a place.
    pub fn sun_seen(&self, m: &Moment, at: DVec3) -> hearth_env::astro::SunPosition {
        m.sun_seen(
            self.planet.latitude_deg(at.z),
            self.planet.solar_time_offset(at.x),
        )
    }

    /// Whether the sun is up at a place.
    pub fn sun_up(&self, m: &Moment, at: DVec3) -> bool {
        self.sun_seen(m, at).dir.y > 0.0
    }

    pub fn sample(
        &self,
        m: &Moment,
        cam: DVec3,
        block_light_at_camera: f32,
        o: EnvOverrides,
    ) -> (Environment, WeatherState) {
        let lat = self.planet.latitude_deg(cam.z);
        let offset = self.planet.solar_time_offset(cam.x);
        let local = m.local_time(offset);
        let sun = m.sun_seen(lat, offset);
        let moon = m.moon_seen(lat, offset);
        let normals = Normals::sample(&self.grid, cam.x, cam.z);
        let mut w = self
            .weather
            .sample(&normals, cam.x, cam.z, m.days, m.year_frac, local);
        if let Some(h) = &self.hold {
            h.apply(&mut w);
        }
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
        let rot = m.sky_rotation(lat, offset);
        let v3 = |a: [f64; 3]| Vec3::new(a[0] as f32, a[1] as f32, a[2] as f32);
        let star_rotation = Mat3::from_cols(
            rot.x_axis.as_vec3(),
            rot.y_axis.as_vec3(),
            rot.z_axis.as_vec3(),
        );
        // Clouds drift with the wind at their height (Amendment P §8), the drift carried from
        // frame to frame, so a change in the wind changes how fast they go, never where they are.
        let real_s = m.days * DAY_S;
        // Cumulus form where rising air cools to its dew point, the lifting condensation level,
        // above the ground (Espy: some 125 m per degree between air and dew point, about 25 m
        // per percent of relative humidity short of saturation): over the region's ground, so
        // mountains stand into the clouds rather than through a sheet at a fixed height.
        let (gx, gz) = self
            .grid
            .geom
            .grid_coords(self.planet.wrap_xf(cam.x), cam.z);
        let ground =
            (self.grid.elevation.bilinear(gx, gz).max(0.0) as f64) * self.grid.vertical_scale;
        let cloud_base = ground + (2500.0 * (1.0 - w.humidity)).clamp(600.0, 3000.0);
        let (cloud_offset, air, wind_dir, cloud_churn) = {
            let mut mo = self.motion.borrow_mut();
            let dt = mo
                .at
                .map_or(0.0, |t| (real_s - t).clamp(0.0, MOTION_STEP_S));
            mo.at = Some(real_s);
            let dir = match mo.wind_dir {
                Some(d) => {
                    let turn = (w.wind_dir - d + std::f64::consts::PI)
                        .rem_euclid(std::f64::consts::TAU)
                        - std::f64::consts::PI;
                    d + turn * (1.0 - (-dt / WIND_TURN_S).exp())
                }
                None => w.wind_dir,
            };
            mo.wind_dir = Some(dir);
            let toward = DVec2::new(dir.sin(), -dir.cos());
            mo.clouds += toward * wind_aloft(w.wind_speed_m_s, cloud_base) * dt;
            mo.air += toward * w.wind_speed_m_s * dt;
            mo.churn += CLOUD_CHURN_M_S * dt;
            (mo.clouds.as_vec2(), mo.air, dir, mo.churn as f32)
        };
        let toward = Vec2::new(wind_dir.sin() as f32, -(wind_dir.cos()) as f32);
        let env = Environment {
            sun_dir: sun.dir.as_vec3(),
            moon_dir: moon.dir.as_vec3(),
            moon_phase: m.moon_phase as f32,
            sun_lux: v3(light.sun),
            moon_lux: v3(light.moon),
            sky_lux: v3(light.sky),
            year_frac: m.year_frac as f32,
            seconds: real_s.rem_euclid(100_000.0) as f32,
            real_seconds: real_s,
            wind: (w.wind_speed_m_s / 6.0).clamp(0.3, 3.0) as f32,
            wind_dir: toward,
            wind_speed_m_s: w.wind_speed_m_s as f32,
            star_rotation,
            cloud_cover: w.cloud_cover as f32,
            cloud_base: cloud_base as f32,
            cloud_offset,
            cloud_churn,
            haze: haze as f32,
            block_light_at_camera,
            aerial_perspective: true,
            exposure_bias: 1.0,
            precipitation: Precipitation {
                wind: Vec3::new(toward.x, 0.0, toward.y) * w.wind_speed_m_s as f32,
                drift: air,
                ..precipitation(&w)
            },
        };
        (env, w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hearth_worldgen::WorldGenSettings;

    /// One real minute of a long-lived world, a tick at a time: the clouds drift smoothly at the
    /// wind's speed at their height (Amendment P §8). They raced when the drift was the world's
    /// whole age times the wind of the moment, every change of wind sweeping them across the sky.
    #[test]
    fn clouds_drift_at_the_wind_aloft_in_real_seconds() {
        let settings = WorldGenSettings {
            seed: 5,
            grid_resolution: 64,
            ..WorldGenSettings::default()
        }
        .sanitized();
        let grid = Arc::new(PlanetGrid::build(&settings, &|_, _| {}));
        let calendar = Calendar::default();
        let env = EnvSampler::new(grid, calendar);
        let at = DVec3::new(300.0, 80.0, -2000.0);
        let start = (400.0 * calendar.ticks_per_day()) as u64;
        let per_s = hearth_env::calendar::TICKS_PER_SECOND;
        let mut last: Option<Vec2> = None;
        let (mut path, mut aloft, mut worst) = (0.0f64, 0.0f64, 0.0f32);
        let seconds = 60;
        for tick in 0..seconds * per_s {
            let m = calendar.at(start + tick);
            let (e, w) = env.sample(&m, at, 0.0, EnvOverrides::default());
            if let Some(prev) = last {
                let step = (e.cloud_offset - prev).length();
                worst = worst.max(step);
                path += step as f64;
                aloft += wind_aloft(w.wind_speed_m_s, e.cloud_base as f64) / per_s as f64;
            }
            last = Some(e.cloud_offset);
        }
        let speed = path / seconds as f64;
        let expected = aloft / seconds as f64;
        assert!(
            (speed - expected).abs() < 0.05 * expected.max(1.0),
            "the clouds go {speed:.2} m/s with the wind aloft at {expected:.2}"
        );
        assert!(
            speed > 1.0 && speed < 60.0,
            "a real wind's speed: {speed:.2} m/s"
        );
        // Smooth: no step much beyond a tick's worth of the strongest wind.
        let tick_s = 1.0 / per_s as f32;
        assert!(worst < 60.0 * tick_s, "a jump of {worst:.1} m in a tick");
    }

    /// Fast-forward (P §8): with the world going 1,000 times faster than play, the sky
    /// time-lapses smoothly, each frame carrying the clouds and their change of shape a capped
    /// step and never sweeping across the sky; at normal speed the shapes change at their rate.
    #[test]
    fn the_sky_time_lapses_smoothly_when_time_runs_fast() {
        let settings = WorldGenSettings {
            seed: 5,
            grid_resolution: 64,
            ..WorldGenSettings::default()
        }
        .sanitized();
        let grid = Arc::new(PlanetGrid::build(&settings, &|_, _| {}));
        let calendar = Calendar::default();
        let env = EnvSampler::new(grid, calendar);
        let at = DVec3::new(300.0, 80.0, -2000.0);
        let per_s = hearth_env::calendar::TICKS_PER_SECOND;
        let start = (400.0 * calendar.ticks_per_day()) as u64;
        // Normal speed, a frame a tick: the shapes change at their rate.
        let (e0, _) = env.sample(&calendar.at(start), at, 0.0, EnvOverrides::default());
        let mut e1 = e0;
        for tick in 1..=60 * per_s {
            e1 = env
                .sample(&calendar.at(start + tick), at, 0.0, EnvOverrides::default())
                .0;
        }
        let rate = (e1.cloud_churn - e0.cloud_churn) as f64 / 60.0;
        assert!(
            (rate - CLOUD_CHURN_M_S).abs() < 0.01,
            "the shapes change at {rate} m/s"
        );
        // A thousand times faster at 60 frames a second: some 17 s of the world a frame.
        let step = 1000 * per_s / 60;
        let mut last = e1;
        let mut t = start + 60 * per_s;
        for _ in 0..120 {
            t += step;
            let (e, w) = env.sample(&calendar.at(t), at, 0.0, EnvOverrides::default());
            let drift = (e.cloud_offset - last.cloud_offset).length() as f64;
            let most = wind_aloft(w.wind_speed_m_s, e.cloud_base as f64) * MOTION_STEP_S;
            assert!(
                drift <= most * 1.05 + 0.01,
                "the clouds jumped {drift} m in a frame (at most {most})"
            );
            let churn = (e.cloud_churn - last.cloud_churn) as f64;
            assert!(churn <= CLOUD_CHURN_M_S * MOTION_STEP_S + 1e-3, "{churn}");
            last = e;
        }
    }
}

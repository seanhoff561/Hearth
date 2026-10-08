//! Clear view (Amendment P §3.2): Creative's way of seeing exactly what is going on — daylight
//! whatever the hour, no haze or aerial perspective, no clouds, rain or snow, the eye not adapting
//! and the body's senses not colouring the image. Each part may be left out; F4 turns it on and
//! off. Only in Creative and Developer mode.

use glam::Vec3;
use hearth_render::post::Senses;
use hearth_render::scene::Environment;

/// Which parts of the clear view are on, and whether it is on at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClearView {
    pub on: bool,
    /// Daylight whatever the hour or the cloud (the sun's light at midday, a clear sky's).
    pub light: bool,
    /// No haze or aerial perspective.
    pub air: bool,
    /// No clouds, rain or snow.
    pub weather: bool,
    /// The image as it is: no adapting eye, nothing of the body's senses.
    pub plain: bool,
}

impl Default for ClearView {
    fn default() -> Self {
        Self {
            on: false,
            light: true,
            air: true,
            weather: true,
            plain: true,
        }
    }
}

/// A clear day's light: the sun's direct light on a facing surface and the sky's on a level one
/// (lux, RGB), as `Environment::default` has them.
const SUN_LUX: Vec3 = Vec3::new(95_000.0, 90_000.0, 80_000.0);
const SKY_LUX: Vec3 = Vec3::new(12_000.0, 16_000.0, 24_000.0);

impl ClearView {
    /// The frame's environment as the clear view shows it.
    pub fn environment(&self, e: &Environment) -> Environment {
        let mut e = *e;
        if !self.on {
            return e;
        }
        if self.light {
            // The sun stands where it stands, but no lower than well up the sky.
            let mut sun = e.sun_dir;
            if sun.y < 0.6 {
                let flat = Vec3::new(sun.x, 0.0, sun.z).normalize_or(Vec3::X);
                sun = (flat * 0.8 + Vec3::Y * 0.6).normalize();
            }
            e.sun_dir = sun;
            e.sun_lux = SUN_LUX;
            e.sky_lux = SKY_LUX;
            e.moon_lux = Vec3::ZERO;
        }
        if self.air {
            e.aerial_perspective = false;
            e.haze = 0.0;
        }
        if self.weather {
            e.cloud_cover = 0.0;
            e.precipitation = Default::default();
        }
        if self.plain {
            e.exposure_bias = 1.0;
            e.block_light_at_camera = 0.0;
        }
        e
    }

    /// The body's senses on the image, as the clear view leaves them.
    pub fn senses(&self, s: Senses) -> Senses {
        if self.on && self.plain {
            Senses::default()
        } else {
            s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn night_and_storm_are_seen_as_a_clear_day() {
        let night = Environment {
            sun_dir: Vec3::new(0.0, -0.5, 0.86).normalize(),
            sun_lux: Vec3::ZERO,
            sky_lux: Vec3::splat(0.01),
            cloud_cover: 1.0,
            haze: 3.0,
            ..Default::default()
        };
        let off = ClearView::default();
        assert_eq!(
            off.environment(&night).sky_lux,
            night.sky_lux,
            "off: as it is"
        );
        let on = ClearView {
            on: true,
            ..Default::default()
        };
        let e = on.environment(&night);
        assert!(e.sun_dir.y >= 0.59 && e.horizontal_lux() > 50_000.0);
        assert_eq!(
            (e.cloud_cover, e.haze, e.aerial_perspective),
            (0.0, 0.0, false)
        );
        // A part left out stays as it is.
        let lit_only = ClearView {
            on: true,
            weather: false,
            ..Default::default()
        };
        assert_eq!(lit_only.environment(&night).cloud_cover, 1.0);
        let s = Senses {
            desaturate: 0.5,
            ..Default::default()
        };
        assert_eq!(on.senses(s), Senses::default());
        assert_eq!(off.senses(s), s);
    }
}

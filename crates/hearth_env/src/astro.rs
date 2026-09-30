//! Sun, moon and stars (v2 §4.1): solar declination from axial tilt and the year fraction, sun
//! position by latitude and local solar time, day length with atmospheric refraction, polar day
//! and night, the moon along the ecliptic, and the sidereal rotation of the sky.
//!
//! World directions: +X east, −Z north, +Y up.

use std::f64::consts::{PI, TAU};

use glam::DVec3;

/// Altitude of the sun's centre at sunrise/sunset, including refraction and the disc radius.
pub const SUNRISE_ALTITUDE_DEG: f64 = -0.833;

/// Solar declination (radians) for a year fraction (0 = March equinox) and axial tilt.
pub fn declination(year_frac: f64, axial_tilt_deg: f64) -> f64 {
    (axial_tilt_deg.to_radians().sin() * (TAU * year_frac).sin()).asin()
}

/// Direction to a body with declination `decl` and hour angle `hour_angle` (radians, positive
/// after local noon) seen from latitude `lat` (radians).
pub fn direction(lat: f64, decl: f64, hour_angle: f64) -> DVec3 {
    let (sd, cd) = decl.sin_cos();
    let (sl, cl) = lat.sin_cos();
    let (sh, ch) = hour_angle.sin_cos();
    let east = -cd * sh;
    let north = sd * cl - cd * ch * sl;
    let up = sd * sl + cd * ch * cl;
    DVec3::new(east, up, -north)
}

/// Where the sun is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SunPosition {
    /// Unit vector toward the sun (world axes).
    pub dir: DVec3,
    /// Elevation above the horizon (radians).
    pub elevation: f64,
    /// Azimuth from north, clockwise toward east (radians).
    pub azimuth: f64,
}

impl SunPosition {
    fn from_dir(dir: DVec3) -> Self {
        let elevation = dir.y.clamp(-1.0, 1.0).asin();
        let azimuth = dir.x.atan2(-dir.z).rem_euclid(TAU);
        Self {
            dir,
            elevation,
            azimuth,
        }
    }
}

/// Sun position at latitude `lat_deg`, year fraction and local solar time (0.5 = noon).
pub fn sun(lat_deg: f64, year_frac: f64, local_time: f64, axial_tilt_deg: f64) -> SunPosition {
    let decl = declination(year_frac, axial_tilt_deg);
    let hour_angle = TAU * (local_time - 0.5);
    SunPosition::from_dir(direction(lat_deg.to_radians(), decl, hour_angle))
}

/// Hours of daylight (sunrise to sunset, with refraction) at a latitude and year fraction.
pub fn day_length_hours(lat_deg: f64, year_frac: f64, axial_tilt_deg: f64) -> f64 {
    let decl = declination(year_frac, axial_tilt_deg);
    let lat = lat_deg.to_radians();
    let h0 = SUNRISE_ALTITUDE_DEG.to_radians();
    let denom = lat.cos() * decl.cos();
    if denom.abs() < 1e-12 {
        return if lat * decl > 0.0 { 24.0 } else { 0.0 };
    }
    let c = (h0.sin() - lat.sin() * decl.sin()) / denom;
    if c <= -1.0 {
        24.0
    } else if c >= 1.0 {
        0.0
    } else {
        2.0 * c.acos() / TAU * 24.0
    }
}

/// Local solar times of sunrise and sunset, or `None` during polar day or night.
pub fn sunrise_sunset(lat_deg: f64, year_frac: f64, axial_tilt_deg: f64) -> Option<(f64, f64)> {
    let len = day_length_hours(lat_deg, year_frac, axial_tilt_deg);
    if len <= 0.0 || len >= 24.0 {
        return None;
    }
    let half = len / 48.0;
    Some((0.5 - half, 0.5 + half))
}

/// The moon's direction. It follows the ecliptic, `phase` of a cycle ahead of the sun (0 new,
/// 0.5 full), so a full moon rises as the sun sets and stands opposite it in declination.
pub fn moon(
    lat_deg: f64,
    year_frac: f64,
    local_time: f64,
    phase: f64,
    axial_tilt_deg: f64,
) -> SunPosition {
    // Ecliptic longitude of the moon relative to the sun advances with the phase.
    let decl = declination(year_frac + phase, axial_tilt_deg);
    let hour_angle = TAU * (local_time - 0.5) - TAU * phase;
    SunPosition::from_dir(direction(lat_deg.to_radians(), decl, hour_angle))
}

/// Rotation of the star field: returns the matrix taking celestial (equatorial) directions to
/// local world directions. Stars turn once per sidereal day around the celestial pole, which
/// stands at the observer's latitude above the north (or south) horizon.
pub fn sky_rotation(lat_deg: f64, year_frac: f64, local_time: f64) -> glam::DMat3 {
    // Local sidereal angle: solar time plus one extra turn per year.
    let sidereal = TAU * (local_time + year_frac - 0.5);
    let lat = lat_deg.to_radians();
    // Celestial frame: z = north celestial pole, x = direction of hour angle 0 at sidereal 0.
    let col = |ra: f64, dec: f64| direction(lat, dec, sidereal - ra);
    // Build the matrix from the images of the celestial basis vectors.
    let x = col(0.0, 0.0);
    let y = col(PI / 2.0, 0.0);
    let z = col(0.0, PI / 2.0);
    glam::DMat3::from_cols(x, y, z)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TILT: f64 = 23.44;

    #[test]
    fn declination_follows_the_seasons() {
        assert!(declination(0.0, TILT).abs() < 1e-12, "equinox");
        assert!(
            (declination(0.25, TILT).to_degrees() - TILT).abs() < 1e-9,
            "June solstice"
        );
        assert!(
            (declination(0.75, TILT).to_degrees() + TILT).abs() < 1e-9,
            "December solstice"
        );
    }

    #[test]
    fn day_lengths_match_known_values() {
        // Equator: about 12 h 7 min all year (refraction adds a few minutes).
        for f in [0.0, 0.25, 0.5, 0.75] {
            let h = day_length_hours(0.0, f, TILT);
            assert!((h - 12.12).abs() < 0.05, "equator {f}: {h}");
        }
        // 60°N: ~18.8 h at the June solstice, ~5.9 h in December (published tables).
        assert!((day_length_hours(60.0, 0.25, TILT) - 18.8).abs() < 0.2);
        assert!((day_length_hours(60.0, 0.75, TILT) - 5.9).abs() < 0.2);
        // 40°N: ~15.0 h and ~9.3 h.
        assert!((day_length_hours(40.0, 0.25, TILT) - 15.0).abs() < 0.15);
        assert!((day_length_hours(40.0, 0.75, TILT) - 9.3).abs() < 0.15);
        // Polar day and night beyond the Arctic circle; mirrored in the south.
        assert_eq!(day_length_hours(75.0, 0.25, TILT), 24.0);
        assert_eq!(day_length_hours(75.0, 0.75, TILT), 0.0);
        assert_eq!(day_length_hours(-75.0, 0.75, TILT), 24.0);
        assert!(sunrise_sunset(75.0, 0.25, TILT).is_none());
        // Equinox: ~12 h everywhere outside the poles.
        assert!((day_length_hours(55.0, 0.0, TILT) - 12.2).abs() < 0.15);
        // No tilt: no seasons.
        assert!(
            (day_length_hours(60.0, 0.25, 0.0) - day_length_hours(60.0, 0.75, 0.0)).abs() < 1e-9
        );
    }

    #[test]
    fn sun_positions() {
        // Noon at 40°N on the equinox: 50° high, due south.
        let s = sun(40.0, 0.0, 0.5, TILT);
        assert!((s.elevation.to_degrees() - 50.0).abs() < 1e-6);
        assert!((s.azimuth.to_degrees() - 180.0).abs() < 1e-6);
        assert!(s.dir.z > 0.0, "south is +Z");
        // In the southern hemisphere the noon sun is to the north.
        let s = sun(-40.0, 0.0, 0.5, TILT);
        assert!(
            s.azimuth.to_degrees().abs() < 1e-6 || (s.azimuth.to_degrees() - 360.0).abs() < 1e-6
        );
        // Morning sun is in the east (+X), evening sun in the west.
        assert!(sun(40.0, 0.0, 0.3, TILT).dir.x > 0.0);
        assert!(sun(40.0, 0.0, 0.7, TILT).dir.x < 0.0);
        // Midnight sun at 75°N in June; below the horizon at noon in December.
        assert!(sun(75.0, 0.25, 0.0, TILT).elevation > 0.0);
        assert!(sun(75.0, 0.75, 0.5, TILT).elevation < 0.0);
        // Summer noon at 60°N: 90 − 60 + 23.44.
        let e = sun(60.0, 0.25, 0.5, TILT).elevation.to_degrees();
        assert!((e - 53.44).abs() < 1e-6);
    }

    #[test]
    fn full_moon_opposes_the_sun() {
        let sun_dir = sun(45.0, 0.1, 0.8, TILT).dir;
        let moon_dir = moon(45.0, 0.1, 0.8, 0.5, TILT).dir;
        assert!(
            sun_dir.dot(moon_dir) < -0.8,
            "full moon sits opposite the sun"
        );
        let new_moon = moon(45.0, 0.1, 0.8, 0.0, TILT).dir;
        assert!(sun_dir.dot(new_moon) > 0.99, "new moon is next to the sun");
    }

    #[test]
    fn celestial_pole_stands_at_the_latitude() {
        for lat in [0.0, 30.0, 60.0, -45.0] {
            let m = sky_rotation(lat, 0.3, 0.7);
            let pole = m * DVec3::Z;
            let elevation = pole.y.asin().to_degrees();
            assert!((elevation - lat).abs() < 1e-6, "{lat}: {elevation}");
        }
    }
}

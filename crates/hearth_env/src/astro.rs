//! Sun, moon and stars as they really stand (E §4.1): where they are on a date at a place, from
//! days since J2000.0. The sun follows the U.S. Naval Observatory's approximate solar
//! coordinates (about a minute of arc from 1950 to 2050), with the equation of time that moves
//! solar noon by up to about a quarter hour through the year; the moon a low-precision lunar
//! series (its chief inequality, about half a degree), so its phases and rising fall on their
//! real days; the stars turn once a sidereal day by Greenwich mean sidereal time.
//!
//! World directions: +X east, −Z north, +Y up.

use std::f64::consts::{PI, TAU};

use glam::DVec3;

/// Altitude of the sun's centre at sunrise/sunset, including refraction and the disc radius.
pub const SUNRISE_ALTITUDE_DEG: f64 = -0.833;

/// Days since J2000.0 (2000-01-01 12:00 UTC) of a Unix time (seconds).
pub fn j2000_days(unix_s: f64) -> f64 {
    unix_s / 86_400.0 - 10_957.5
}

/// The obliquity of the ecliptic (radians) on a day since J2000.0.
pub fn obliquity(d: f64) -> f64 {
    (23.439 - 0.000_000_36 * d).to_radians()
}

/// Where the sun is among the stars: right ascension and declination (radians), its ecliptic
/// longitude (0 at the March equinox, radians), and the equation of time (apparent minus mean
/// solar time, days).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SunCoords {
    pub ra: f64,
    pub decl: f64,
    pub lon: f64,
    pub eot: f64,
}

/// The sun on a day since J2000.0 (USNO's approximate solar coordinates).
pub fn sun_coords(d: f64) -> SunCoords {
    let g = (357.529 + 0.985_600_28 * d).to_radians();
    let q = 280.459 + 0.985_647_36 * d;
    let lon = (q + 1.915 * g.sin() + 0.020 * (2.0 * g).sin()).to_radians();
    let e = obliquity(d);
    let ra = (e.cos() * lon.sin()).atan2(lon.cos());
    let decl = (e.sin() * lon.sin()).asin();
    // The mean sun's right ascension less the true sun's, in hours, wrapped to ±12.
    let eot_h = (q / 15.0 - ra.to_degrees() / 15.0 + 12.0).rem_euclid(24.0) - 12.0;
    SunCoords {
        ra,
        decl,
        lon: lon.rem_euclid(TAU),
        eot: eot_h / 24.0,
    }
}

/// Where the moon is: right ascension and declination (radians), and its phase (its
/// elongation from the sun as a share of a turn: 0 new, 0.5 full).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoonCoords {
    pub ra: f64,
    pub decl: f64,
    pub phase: f64,
}

/// The moon on a day since J2000.0, given the sun's ecliptic longitude then.
pub fn moon_coords(d: f64, sun_lon: f64) -> MoonCoords {
    let mean_lon = 218.316 + 13.176_396 * d;
    let anomaly = (134.963 + 13.064_993 * d).to_radians();
    let node = (93.272 + 13.229_350 * d).to_radians();
    let lon = (mean_lon + 6.289 * anomaly.sin()).to_radians();
    let lat = (5.128 * node.sin()).to_radians();
    let e = obliquity(d);
    let (sl, cl) = lon.sin_cos();
    let (sb, cb) = lat.sin_cos();
    let x = cb * cl;
    let y = cb * sl * e.cos() - sb * e.sin();
    let z = cb * sl * e.sin() + sb * e.cos();
    MoonCoords {
        ra: y.atan2(x),
        decl: z.clamp(-1.0, 1.0).asin(),
        phase: ((lon - sun_lon) / TAU).rem_euclid(1.0),
    }
}

/// Greenwich mean sidereal time (radians) on a day since J2000.0.
pub fn gmst(d: f64) -> f64 {
    (18.697_374_558 + 24.065_709_824_419_08 * d).rem_euclid(24.0) / 24.0 * TAU
}

/// Direction to a body with declination `decl` and hour angle `hour_angle` (radians, positive
/// after it crosses the meridian) seen from latitude `lat` (radians).
pub fn direction(lat: f64, decl: f64, hour_angle: f64) -> DVec3 {
    let (sd, cd) = decl.sin_cos();
    let (sl, cl) = lat.sin_cos();
    let (sh, ch) = hour_angle.sin_cos();
    let east = -cd * sh;
    let north = sd * cl - cd * ch * sl;
    let up = sd * sl + cd * ch * cl;
    DVec3::new(east, up, -north)
}

/// Where a body is in the sky.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SunPosition {
    /// Unit vector toward it (world axes).
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

/// The local sidereal angle (radians) at `lon_frac` (east longitude as a share of a turn) on
/// a day since J2000.0.
pub fn local_sidereal(d: f64, lon_frac: f64) -> f64 {
    gmst(d) + TAU * lon_frac
}

/// A body of right ascension `ra` and declination `decl` seen from `lat_deg` at `lon_frac`
/// on a day since J2000.0.
pub fn seen(d: f64, lat_deg: f64, lon_frac: f64, ra: f64, decl: f64) -> SunPosition {
    let hour_angle = local_sidereal(d, lon_frac) - ra;
    SunPosition::from_dir(direction(lat_deg.to_radians(), decl, hour_angle))
}

/// The sun seen from `lat_deg` at `lon_frac` on a day since J2000.0.
pub fn sun(d: f64, lat_deg: f64, lon_frac: f64) -> SunPosition {
    let s = sun_coords(d);
    seen(d, lat_deg, lon_frac, s.ra, s.decl)
}

/// The moon seen from `lat_deg` at `lon_frac` on a day since J2000.0.
pub fn moon(d: f64, lat_deg: f64, lon_frac: f64) -> SunPosition {
    let m = moon_coords(d, sun_coords(d).lon);
    seen(d, lat_deg, lon_frac, m.ra, m.decl)
}

/// Hours of daylight (sunrise to sunset, with refraction) at a latitude with the sun at
/// declination `decl` (radians).
pub fn day_length_hours(lat_deg: f64, decl: f64) -> f64 {
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

/// Sunrise and sunset on the UTC day containing `d` (days since J2000.0) at `lat_deg` and
/// `lon_frac`, as days since J2000.0; `None` in polar day or night. Solved by stepping the sun's
/// own motion (two passes, a fraction of a minute).
pub fn sunrise_sunset(d: f64, lat_deg: f64, lon_frac: f64) -> Option<(f64, f64)> {
    // The UTC midnight beginning the day.
    let midnight = (d + 0.5).floor() - 0.5;
    let mut noon = midnight + 0.5 - lon_frac;
    let mut half = 0.0;
    for _ in 0..3 {
        let s = sun_coords(noon);
        // Solar noon: when the hour angle is zero.
        let lst = local_sidereal(noon, lon_frac);
        let ha = (lst - s.ra + PI).rem_euclid(TAU) - PI;
        noon -= ha / TAU * 0.997_269_566;
        let len = day_length_hours(lat_deg, s.decl);
        if len <= 0.0 || len >= 24.0 {
            return None;
        }
        half = len / 48.0;
    }
    Some((noon - half, noon + half))
}

/// The star field's turn: the matrix taking celestial (equatorial) directions to local world
/// directions on a day since J2000.0 at `lat_deg` and `lon_frac`. The celestial pole stands at
/// the latitude above the north (or south) horizon; the sky turns once a sidereal day.
pub fn sky_rotation(d: f64, lat_deg: f64, lon_frac: f64) -> glam::DMat3 {
    let lst = local_sidereal(d, lon_frac);
    let lat = lat_deg.to_radians();
    // Celestial frame: z = north celestial pole, x = the March equinox (right ascension 0).
    let col = |ra: f64, dec: f64| direction(lat, dec, lst - ra);
    glam::DMat3::from_cols(col(0.0, 0.0), col(PI / 2.0, 0.0), col(0.0, PI / 2.0))
}

/// The civil (Gregorian) date of a day since J2000.0 (UTC): year, month (1–12), day (1–31).
pub fn civil_date(d: f64) -> (i64, u32, u32) {
    // Days since 1970-01-01, then Howard Hinnant's civil_from_days.
    let z = (d + 10_957.5).floor() as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Days since J2000.0 of a civil date at a UTC hour.
pub fn j2000_of(year: i64, month: u32, day: u32, hour_utc: f64) -> f64 {
    // Hinnant's days_from_civil.
    let y = year - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = month as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let since_1970 = era * 146_097 + doe - 719_468;
    since_1970 as f64 - 10_957.5 + hour_utc / 24.0
}

/// The moment (days since J2000.0) of the equinox that begins spring in a hemisphere in a
/// year: the March equinox in the north, the September one in the south.
pub fn spring_equinox(year: i64, southern: bool) -> f64 {
    let (month, target) = if southern { (9, PI) } else { (3, 0.0) };
    let mut d = j2000_of(year, month, 20, 12.0);
    // The sun moves about a degree a day: a few Newton steps on its longitude.
    for _ in 0..4 {
        let off = (sun_coords(d).lon - target + PI).rem_euclid(TAU) - PI;
        d -= off.to_degrees() / 0.985_647_36;
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Days since J2000.0 of a UTC date and time.
    fn at(year: i64, month: u32, day: u32, h: u32, m: u32) -> f64 {
        j2000_of(year, month, day, h as f64 + m as f64 / 60.0)
    }

    /// Minutes between two moments.
    fn minutes(a: f64, b: f64) -> f64 {
        (a - b) * 1_440.0
    }

    #[test]
    fn dates_and_days_agree_with_leap_years() {
        assert_eq!(j2000_of(2000, 1, 1, 12.0), 0.0);
        assert_eq!(civil_date(0.0), (2000, 1, 1));
        assert_eq!(civil_date(at(2024, 2, 29, 23, 0)), (2024, 2, 29));
        assert_eq!(civil_date(at(2024, 3, 1, 0, 30)), (2024, 3, 1));
        assert_eq!(
            at(2025, 1, 1, 0, 0) - at(2024, 1, 1, 0, 0),
            366.0,
            "a leap year"
        );
        assert_eq!(at(2026, 1, 1, 0, 0) - at(2025, 1, 1, 0, 0), 365.0);
        assert_eq!(
            at(2100, 3, 1, 0, 0) - at(2100, 2, 28, 0, 0),
            1.0,
            "2100 is no leap year"
        );
        // The Unix epoch.
        assert_eq!(j2000_days(0.0), at(1970, 1, 1, 0, 0));
    }

    #[test]
    fn the_year_is_the_tropical_year() {
        // From one March equinox to the next, averaged over decades: 365.2422 days.
        let a = spring_equinox(2000, false);
        let b = spring_equinox(2040, false);
        let year = (b - a) / 40.0;
        assert!((year - 365.2422).abs() < 0.001, "{year}");
        // The 2024 March equinox fell at 03:06 UTC on the 20th; the September one at 12:44 UTC
        // on the 22nd.
        assert!(minutes(spring_equinox(2024, false), at(2024, 3, 20, 3, 6)).abs() < 20.0);
        assert!(minutes(spring_equinox(2024, true), at(2024, 9, 22, 12, 44)).abs() < 20.0);
    }

    #[test]
    fn the_equation_of_time_runs_its_real_course() {
        // About −14.2 minutes in mid-February, +16.4 at the start of November, near zero in
        // mid-April, mid-June, the start of September and late December.
        let eot = |m: u32, d: u32| sun_coords(at(2024, m, d, 12, 0)).eot * 1_440.0;
        assert!((eot(2, 11) + 14.2).abs() < 0.5, "{}", eot(2, 11));
        assert!((eot(11, 3) - 16.4).abs() < 0.5, "{}", eot(11, 3));
        for (m, d) in [(4, 15), (6, 13), (9, 1), (12, 25)] {
            assert!(eot(m, d).abs() < 1.0, "{m}/{d}: {}", eot(m, d));
        }
    }

    #[test]
    fn sunrise_and_sunset_where_and_when_they_really_are() {
        // (place, latitude, longitude °E, date, sunrise and sunset UTC) from published tables.
        let cases: [(&str, f64, f64, (u32, u32), (u32, u32), (u32, u32)); 4] = [
            (
                "London, June solstice",
                51.5074,
                -0.1278,
                (6, 21),
                (3, 43),
                (20, 21),
            ),
            (
                "London, December solstice",
                51.5074,
                -0.1278,
                (12, 21),
                (8, 4),
                (15, 53),
            ),
            (
                "New York, December solstice",
                40.7128,
                -74.006,
                (12, 21),
                (12, 16),
                (21, 32),
            ),
            (
                "Sydney, June solstice",
                -33.8688,
                151.2093,
                (6, 21),
                (21, 0),
                (6, 54),
            ),
        ];
        for (name, lat, lon, (mo, da), (rh, rm), (sh, sm)) in cases {
            let d = at(2024, mo, da, 12, 0) - lon / 360.0;
            let (rise, set) = sunrise_sunset(d, lat, lon / 360.0).expect(name);
            let day = (rise + 0.5).floor() - 0.5;
            let rise_min = minutes(rise, day).rem_euclid(1_440.0);
            let set_min = minutes(set, day).rem_euclid(1_440.0);
            let want_rise = (rh * 60 + rm) as f64;
            let want_set = (sh * 60 + sm) as f64;
            let off = |a: f64, b: f64| ((a - b + 720.0).rem_euclid(1_440.0) - 720.0).abs();
            assert!(
                off(rise_min, want_rise) < 4.0,
                "{name} sunrise {rise_min:.0} min"
            );
            assert!(
                off(set_min, want_set) < 4.0,
                "{name} sunset {set_min:.0} min"
            );
        }
        // Tromsø (69.65°N): polar night at midwinter, midnight sun at midsummer.
        assert!(sunrise_sunset(at(2024, 12, 21, 12, 0), 69.65, 18.96 / 360.0).is_none());
        assert!(sunrise_sunset(at(2024, 6, 21, 12, 0), 69.65, 18.96 / 360.0).is_none());
        let noon = sun(at(2024, 12, 21, 11, 0), 69.65, 18.96 / 360.0);
        assert!(noon.elevation < 0.0, "the sun stays down at noon");
        let midnight = sun(at(2024, 6, 21, 23, 0), 69.65, 18.96 / 360.0);
        assert!(midnight.elevation > 0.0, "and up at midnight");
    }

    #[test]
    fn solar_noon_drifts_with_the_equation_of_time() {
        // At longitude 0 the sun crosses the meridian near 12:14 UTC in February and 11:44 UTC
        // at the start of November.
        let noon_at = |m: u32, dd: u32| {
            let (rise, set) = sunrise_sunset(at(2024, m, dd, 12, 0), 0.0, 0.0).expect("day");
            minutes((rise + set) / 2.0, at(2024, m, dd, 0, 0))
        };
        assert!((noon_at(2, 11) - (12.0 * 60.0 + 14.2)).abs() < 1.0);
        assert!((noon_at(11, 3) - (11.0 * 60.0 + 43.6)).abs() < 1.0);
        // Noon at 40°N on an equinox stands about 50° high and due south.
        let d = spring_equinox(2024, false);
        let (rise, set) = sunrise_sunset(d, 40.0, 0.0).expect("day");
        let high = sun((rise + set) / 2.0, 40.0, 0.0);
        assert!((high.elevation.to_degrees() - 50.0).abs() < 0.5);
        assert!((high.azimuth.to_degrees() - 180.0).abs() < 0.5, "due south");
        assert!(high.dir.z > 0.0, "south is +Z");
    }

    #[test]
    fn the_moons_phases_fall_on_their_days() {
        // Full moon 2024-01-25 17:54 UTC, new moon 2024-01-11 11:57 UTC.
        let phase = |d: f64| moon_coords(d, sun_coords(d).lon).phase;
        let full = at(2024, 1, 25, 17, 54);
        let new = at(2024, 1, 11, 11, 57);
        assert!((phase(full) - 0.5).abs() < 0.02, "{}", phase(full));
        let p = phase(new);
        assert!(!(0.02..=0.98).contains(&p), "{p}");
        // A synodic month later it is full again.
        let next = full + 29.530_589;
        assert!((phase(next) - 0.5).abs() < 0.03, "{}", phase(next));
        // The full moon stands opposite the sun.
        let s = sun(full, 45.0, 0.0);
        let m = moon(full, 45.0, 0.0);
        assert!(s.dir.dot(m.dir) < -0.95);
    }

    #[test]
    fn the_celestial_pole_stands_at_the_latitude_and_the_sky_turns_sidereally() {
        for lat in [0.0, 30.0, 60.0, -45.0] {
            let m = sky_rotation(1234.5, lat, 0.2);
            let pole = m * DVec3::Z;
            let elevation = pole.y.asin().to_degrees();
            assert!((elevation - lat).abs() < 1e-6, "{lat}: {elevation}");
        }
        // After one sidereal day the stars stand where they stood.
        let a = sky_rotation(100.0, 50.0, 0.0) * DVec3::X;
        let b = sky_rotation(100.0 + 0.997_269_566, 50.0, 0.0) * DVec3::X;
        assert!((a - b).length() < 1e-4);
        // After a solar day they have moved on about a degree.
        let c = sky_rotation(101.0, 50.0, 0.0) * DVec3::X;
        let turned = a.angle_between(c).to_degrees();
        assert!((turned - 0.986).abs() < 0.2, "{turned}");
    }
}

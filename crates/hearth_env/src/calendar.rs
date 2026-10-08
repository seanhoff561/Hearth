//! The world calendar: Earth's (E §4.1). A game second is a real second, a day 86,400 of them;
//! tick 0 is a moment of real time (days since J2000.0, UTC), so a world has real dates, leap
//! years and all. The year's fraction is the sun's ecliptic longitude (0 at the March
//! equinox), so the seasons are the astronomical ones, opposite in the southern hemisphere; the
//! moon's phase is its elongation from the sun; local solar time is the sun's own, with the
//! equation of time. The sun, moon and stars stand where they really do (`astro`).
//!
//! The time of day at longitude 0 is UTC; a place's offset is its east longitude as a share of a
//! turn (`Planet::solar_time_offset`), so it is noon on one side of the planet while it is
//! midnight on the other.

use std::f64::consts::TAU;

use hearth_content::schema::Season;
use serde::{Deserialize, Serialize};

use crate::astro::{self, MoonCoords, SunCoords, SunPosition};

/// Simulation ticks per second of play (the game's one rate, `hearth_core::TICKS_PER_SECOND`).
pub const TICKS_PER_SECOND: u64 = hearth_content::time::TICKS_PER_SECOND as u64;

/// A mean solar day (s).
pub const DAY_S: f64 = hearth_content::time::DAY_S;
/// The tropical year (days).
pub const YEAR_DAYS: f64 = 365.242_2;
/// The synodic month (days).
pub const MONTH_DAYS: f64 = 29.530_589;
/// The local solar time a spring morning begins at (seven o'clock).
const MORNING: f64 = 7.0 / 24.0;
/// How far into spring a spring morning falls: twenty days after the equinox that begins it (a
/// share of the year).
const INTO_SPRING: f64 = 20.0 / YEAR_DAYS;
/// The year of a world made in no year (tests, tools, a world not saved): 2000's, at J2000.0,
/// where the ephemeris is closest.
pub const UNDATED_YEAR: i64 = 2000;

/// A world's calendar: the moment its clock started.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Calendar {
    /// The moment of tick 0 (days since J2000.0, UTC).
    pub epoch: f64,
}

impl Default for Calendar {
    /// A spring morning of an undated year at longitude 0 (until a world's own is known).
    fn default() -> Self {
        Self::spring_morning(UNDATED_YEAR, false, 0.0)
    }
}

impl Calendar {
    /// A calendar starting at a moment (days since J2000.0, UTC).
    pub fn at_j2000(epoch: f64) -> Self {
        Self { epoch }
    }

    /// A spring morning of `year` where the first life is: twenty days after the equinox that
    /// begins spring in its hemisphere, at seven by the sun at `solar_offset` (east longitude as
    /// a share of a turn).
    pub fn spring_morning(year: i64, southern: bool, solar_offset: f64) -> Self {
        let spring = if southern { 0.5 } else { 0.0 };
        Self::when(year, spring + INTO_SPRING, MORNING, solar_offset)
    }

    /// The moment of the year that begins at `year`'s March equinox when the sun stands
    /// `year_frac` of its course from it (0.25 the June solstice), at `local` by the sun (0
    /// midnight, 0.5 noon) at `solar_offset`: tools' and tests' dates by season and hour.
    pub fn when(year: i64, year_frac: f64, local: f64, solar_offset: f64) -> Self {
        let f = year_frac.rem_euclid(1.0);
        let mut day = astro::spring_equinox(year, false) + f * YEAR_DAYS;
        // The sun's course runs unevenly by up to two days: steps onto it.
        for _ in 0..3 {
            let at = astro::sun_coords(day).lon / TAU;
            day += ((f - at + 0.5).rem_euclid(1.0) - 0.5) * YEAR_DAYS;
        }
        let midnight = (day + 0.5).floor() - 0.5;
        // The UTC moment of that day that is `local` by the sun there (the equation of time
        // taken at that moment).
        let mut epoch = day;
        for _ in 0..2 {
            let eot = astro::sun_coords(epoch).eot;
            epoch = midnight + (local - solar_offset - eot).rem_euclid(1.0);
        }
        Self { epoch }
    }

    /// The real date and time of a Unix moment (seconds).
    pub fn from_unix(unix_s: f64) -> Self {
        Self {
            epoch: astro::j2000_days(unix_s),
        }
    }

    pub fn ticks_per_day(&self) -> f64 {
        DAY_S * TICKS_PER_SECOND as f64
    }

    pub fn days_per_year(&self) -> f64 {
        YEAR_DAYS
    }

    /// Years since the world began at a tick (tropical years).
    pub fn years(&self, ticks: u64) -> f64 {
        ticks as f64 / self.ticks_per_day() / YEAR_DAYS
    }

    /// The year's share at tick 0 (0 at the March equinox): with [`Calendar::years`], the
    /// season's place on the sun's mean course.
    pub fn year_offset(&self) -> f64 {
        self.at(0).year_frac
    }

    /// The year's share where a season begins in a hemisphere.
    pub fn season_start(season: Season, southern: bool) -> f64 {
        let north: f64 = match season {
            Season::Spring => 0.0,
            Season::Summer => 0.25,
            Season::Autumn => 0.5,
            Season::Winter => 0.75,
        };
        if southern {
            (north + 0.5).fract()
        } else {
            north
        }
    }

    /// Days since J2000.0 (UTC) at a tick.
    pub fn days(&self, ticks: u64) -> f64 {
        self.epoch + ticks as f64 / self.ticks_per_day()
    }

    /// The tick of a moment (days since J2000.0), or 0 if it is before the world began.
    pub fn ticks_at(&self, days: f64) -> u64 {
        ((days - self.epoch) * self.ticks_per_day())
            .max(0.0)
            .round() as u64
    }

    /// The moment described by `ticks`.
    pub fn at(&self, ticks: u64) -> Moment {
        Moment::of(self.days(ticks))
    }
}

/// A point in world time, with where the sun and the moon stand among the stars then.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moment {
    /// Days since J2000.0 (UTC).
    pub days: f64,
    /// The UTC day: days since 2000-01-01.
    pub day: i64,
    /// The time of day at longitude 0 (UTC): 0 midnight, 0.5 noon.
    pub time_of_day: f64,
    /// The sun's ecliptic longitude as a share of a turn: 0 at the March equinox.
    pub year_frac: f64,
    /// The civil year.
    pub year: i64,
    /// 0 new moon, 0.5 full moon.
    pub moon_phase: f64,
    /// The equation of time: apparent minus mean solar time (days).
    pub eot: f64,
    pub sun: SunCoords,
    pub moon: MoonCoords,
}

impl Moment {
    /// The moment `days` since J2000.0.
    pub fn of(days: f64) -> Self {
        let sun = astro::sun_coords(days);
        let moon = astro::moon_coords(days, sun.lon);
        Self {
            days,
            day: (days + 0.5).floor() as i64,
            time_of_day: (days + 0.5).rem_euclid(1.0),
            year_frac: sun.lon / TAU,
            year: astro::civil_date(days).0,
            moon_phase: moon.phase,
            eot: sun.eot,
            sun,
            moon,
        }
    }

    /// Local solar time (0 midnight, 0.5 when the sun crosses the meridian) at a place
    /// `solar_offset` east of longitude 0 (a share of a turn): the sun's own time, the
    /// equation of time included.
    pub fn local_time(&self, solar_offset: f64) -> f64 {
        (self.time_of_day + solar_offset + self.eot).rem_euclid(1.0)
    }

    /// Local mean time there: what a clock kept by the mean sun reads.
    pub fn local_mean_time(&self, solar_offset: f64) -> f64 {
        (self.time_of_day + solar_offset).rem_euclid(1.0)
    }

    /// The civil date (year, month, day) at a place `solar_offset` east of longitude 0, by its
    /// local mean time.
    pub fn date(&self, solar_offset: f64) -> (i64, u32, u32) {
        astro::civil_date(self.days + solar_offset)
    }

    /// The sun seen from `lat_deg` at `solar_offset`.
    pub fn sun_seen(&self, lat_deg: f64, solar_offset: f64) -> SunPosition {
        astro::seen(self.days, lat_deg, solar_offset, self.sun.ra, self.sun.decl)
    }

    /// The moon seen from `lat_deg` at `solar_offset`.
    pub fn moon_seen(&self, lat_deg: f64, solar_offset: f64) -> SunPosition {
        astro::seen(
            self.days,
            lat_deg,
            solar_offset,
            self.moon.ra,
            self.moon.decl,
        )
    }

    /// The star field's turn seen from `lat_deg` at `solar_offset`.
    pub fn sky_rotation(&self, lat_deg: f64, solar_offset: f64) -> glam::DMat3 {
        astro::sky_rotation(self.days, lat_deg, solar_offset)
    }

    /// Hours of daylight at a latitude on this day.
    pub fn day_length_hours(&self, lat_deg: f64) -> f64 {
        astro::day_length_hours(lat_deg, self.sun.decl)
    }

    /// Astronomical season in a hemisphere.
    pub fn season(&self, southern: bool) -> Season {
        let f = if southern {
            (self.year_frac + 0.5).fract()
        } else {
            self.year_frac
        };
        match (f * 4.0) as u32 {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Autumn,
            _ => Season::Winter,
        }
    }

    /// Fraction of the current season elapsed (0..1).
    pub fn season_progress(&self) -> f64 {
        (self.year_frac * 4.0).fract()
    }

    /// Illuminated fraction of the moon's disc.
    pub fn moon_illumination(&self) -> f64 {
        0.5 * (1.0 - (self.moon_phase * TAU).cos())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_day_is_a_real_day_and_the_year_a_real_year() {
        let c = Calendar::spring_morning(2026, false, 0.0);
        assert_eq!(c.ticks_per_day(), 86_400.0 * 20.0);
        assert_eq!(c.days_per_year(), 365.2422);
        let start = c.at(0);
        let a_day = c.at(c.ticks_per_day() as u64);
        assert!((a_day.days - start.days - 1.0).abs() < 1e-9);
        // A year on, the sun is back where it was (to a few minutes of its course).
        let a_year = c.at((365.2422 * c.ticks_per_day()) as u64);
        assert!((a_year.year_frac - start.year_frac).abs() < 1e-4);
        assert_eq!(a_year.year, start.year + 1);
        assert_eq!(c.ticks_at(a_day.days), c.ticks_per_day() as u64);
    }

    #[test]
    fn a_spring_morning_in_either_hemisphere() {
        for (southern, offset) in [(false, 0.0), (false, 0.3), (true, 0.7), (true, 0.05)] {
            let c = Calendar::spring_morning(2026, southern, offset);
            let m = c.at(0);
            assert_eq!(m.season(southern), Season::Spring, "{southern} {offset}");
            assert!(
                m.season_progress() > 0.15 && m.season_progress() < 0.3,
                "three weeks in"
            );
            // Seven by the sun there.
            assert!((m.local_time(offset) - 7.0 / 24.0).abs() < 1e-5);
            assert_eq!(m.season(!southern), Season::Autumn, "opposite seasons");
        }
        // In the north it is April; in the south, October.
        assert_eq!(
            Calendar::spring_morning(2026, false, 0.0).at(0).date(0.0).1,
            4
        );
        assert_eq!(
            Calendar::spring_morning(2026, true, 0.0).at(0).date(0.0).1,
            10
        );
    }

    #[test]
    fn a_date_by_the_suns_course_and_the_hour() {
        for (f, hour, offset) in [
            (0.0, 12.0, 0.0),
            (0.25, 6.0, 0.4),
            (0.62, 21.5, 0.9),
            (0.9, 0.5, 0.1),
        ] {
            let m = Calendar::when(2026, f, hour / 24.0, offset).at(0);
            let d = (m.year_frac - f + 0.5).rem_euclid(1.0) - 0.5;
            assert!(
                d.abs() < 1.0 / YEAR_DAYS,
                "the sun's place {f} → {}",
                m.year_frac
            );
            assert!(
                (m.local_time(offset) * 24.0 - hour).abs() < 1e-3,
                "{hour} h"
            );
        }
        // The June solstice falls on the 20th or 21st; the year's share 0.9 is next February.
        assert_eq!(Calendar::when(2026, 0.25, 0.5, 0.0).at(0).date(0.0).1, 6);
        assert_eq!(Calendar::when(2026, 0.9, 0.5, 0.0).at(0).date(0.0).0, 2027);
        // Season starts: opposite in the south.
        assert_eq!(Calendar::season_start(Season::Summer, false), 0.25);
        assert_eq!(Calendar::season_start(Season::Summer, true), 0.75);
        // Years since the world began, offset onto the sun's course.
        let c = Calendar::spring_morning(2026, false, 0.0);
        let t = (100.0 * c.ticks_per_day()) as u64;
        let mean = (c.years(t) + c.year_offset()).fract();
        assert!((mean - c.at(t).year_frac).abs() < 0.01);
    }

    #[test]
    fn now_is_now() {
        // 2026-10-08 12:00 UTC.
        let c = Calendar::from_unix(1_791_460_800.0);
        assert_eq!(c.at(0).date(0.0), (2026, 10, 8));
        assert!((c.at(0).time_of_day - 0.5).abs() < 1e-9);
        assert_eq!(c.at(0).season(false), Season::Autumn);
    }

    #[test]
    fn the_moon_waxes_and_wanes_in_a_synodic_month() {
        let c = Calendar::spring_morning(2026, false, 0.0);
        let a = c.at(0).moon_phase;
        let b = c.at((MONTH_DAYS * c.ticks_per_day()) as u64).moon_phase;
        let d = ((b - a + 0.5).rem_euclid(1.0) - 0.5).abs();
        assert!(d < 0.03, "{a} → {b}");
        let full = Moment {
            moon_phase: 0.5,
            ..c.at(0)
        };
        assert!((full.moon_illumination() - 1.0).abs() < 1e-12);
    }
}

//! The world calendar: ticks → days, time of day, year fraction and seasons (v2 §4.1).
//!
//! * The global time of day is 0 at midnight at X = 0; local solar time adds the planet's solar
//!   offset (`Planet::solar_time_offset`), so it is noon on one side of the planet while it is
//!   midnight on the other.
//! * The year fraction is 0 at the March equinox (start of northern spring). Seasons are the
//!   astronomical ones and are opposite in the southern hemisphere.

use hearth_content::schema::Season;
use hearth_content::schema::config::TimeConfig;
use serde::{Deserialize, Serialize};

/// Simulation ticks per second of play (the game's one rate, `hearth_core::TICKS_PER_SECOND`).
pub const TICKS_PER_SECOND: u64 = hearth_content::time::TICKS_PER_SECOND as u64;

/// Calendar settings of a world.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Calendar {
    /// Seconds of play per game day.
    pub day_length_s: f64,
    pub days_per_season: u32,
    /// Year fraction at tick 0 (so a world can start in any season).
    pub year_offset: f64,
    /// Time of day (global, 0 = midnight at X = 0) at tick 0.
    pub day_offset: f64,
    /// Synodic months per year (12.37 on Earth); the moon cycle scales with the year.
    pub months_per_year: f64,
    /// Axial tilt in degrees.
    pub axial_tilt_deg: f64,
}

impl Calendar {
    pub fn new(day_length_min: u32, days_per_season: u32, axial_tilt_deg: f64) -> Self {
        Self {
            day_length_s: day_length_min.max(1) as f64 * 60.0,
            days_per_season: days_per_season.max(1),
            year_offset: 0.0,
            day_offset: 0.25,
            months_per_year: 365.2422 / 29.530589,
            axial_tilt_deg,
        }
    }

    /// The default calendar of a world from `time.ron` (day length, season length, tilt and
    /// the moon's month relative to the year).
    pub fn from_config(cfg: &TimeConfig) -> Self {
        let mut c = Self::new(
            cfg.day_length_min.default,
            cfg.days_per_season.default,
            cfg.axial_tilt_deg.default,
        );
        if cfg.synodic_month_days > 0.0 {
            c.months_per_year = cfg.real_year_days / cfg.synodic_month_days;
        }
        c
    }

    /// Year fraction at which a season starts in the given hemisphere.
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

    /// Sets the clock so tick 0 falls at the start of `season` (in the hemisphere of the
    /// starting location) at the given local solar time.
    pub fn start_at(
        mut self,
        season: Season,
        southern: bool,
        local_time: f64,
        solar_offset: f64,
    ) -> Self {
        self.year_offset = Self::season_start(season, southern);
        self.day_offset = (local_time - solar_offset).rem_euclid(1.0);
        self
    }

    pub fn ticks_per_day(&self) -> f64 {
        self.day_length_s * TICKS_PER_SECOND as f64
    }

    pub fn days_per_year(&self) -> f64 {
        self.days_per_season as f64 * 4.0
    }

    /// Game days elapsed since tick 0 (fractional), counted from the start offset.
    pub fn days(&self, ticks: u64) -> f64 {
        ticks as f64 / self.ticks_per_day() + self.day_offset
    }

    /// The moment described by `ticks`.
    pub fn at(&self, ticks: u64) -> Moment {
        let days = self.days(ticks);
        Moment {
            days,
            day: days.floor() as i64,
            time_of_day: days.fract(),
            year_frac: (days / self.days_per_year() + self.year_offset).rem_euclid(1.0),
            year: ((days / self.days_per_year() + self.year_offset).floor()) as i64,
            moon_phase: (days * self.months_per_year / self.days_per_year()).rem_euclid(1.0),
        }
    }
}

/// A point in world time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moment {
    /// Fractional game days since the calendar's origin.
    pub days: f64,
    pub day: i64,
    /// Global time of day, 0 = midnight at X = 0.
    pub time_of_day: f64,
    /// 0 at the March equinox.
    pub year_frac: f64,
    pub year: i64,
    /// 0 new moon, 0.5 full moon.
    pub moon_phase: f64,
}

impl Moment {
    /// Local solar time (0 = midnight, 0.5 = noon) at a place with the given solar offset.
    pub fn local_time(&self, solar_offset: f64) -> f64 {
        (self.time_of_day + solar_offset).rem_euclid(1.0)
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
        0.5 * (1.0 - (self.moon_phase * std::f64::consts::TAU).cos())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_from_the_data_pack_defaults() {
        let cfg: TimeConfig =
            ron::from_str(include_str!("../../../data/hearth/time.ron")).expect("time.ron parses");
        let c = Calendar::from_config(&cfg);
        assert_eq!(c.day_length_s, cfg.day_length_min.default as f64 * 60.0);
        assert_eq!(c.days_per_season, cfg.days_per_season.default);
        assert!((c.months_per_year - 12.37).abs() < 0.01);
    }

    #[test]
    fn days_seasons_and_offsets() {
        let c = Calendar::new(48, 8, 23.44);
        assert_eq!(c.ticks_per_day(), 48.0 * 60.0 * 20.0);
        assert_eq!(c.days_per_year(), 32.0);
        let start = c.at(0);
        assert!(
            (start.time_of_day - 0.25).abs() < 1e-12,
            "worlds start at dawn by default"
        );
        assert_eq!(start.season(false), Season::Spring);
        assert_eq!(start.season(true), Season::Autumn, "opposite seasons");
        let summer = c.at((9.0 * c.ticks_per_day()) as u64);
        assert_eq!(summer.season(false), Season::Summer);
        let next_year = c.at((32.0 * c.ticks_per_day()) as u64);
        assert_eq!(next_year.year, 1);
        assert!((next_year.year_frac - start.year_frac).abs() < 1e-9);
        // A southern-hemisphere start in spring begins at the September equinox.
        let s = Calendar::new(48, 8, 23.44).start_at(Season::Spring, true, 0.3, 0.1);
        let m = s.at(0);
        assert_eq!(m.season(true), Season::Spring);
        assert!((m.local_time(0.1) - 0.3).abs() < 1e-12);
    }

    #[test]
    fn moon_cycles_scale_with_the_year() {
        let c = Calendar::new(48, 8, 23.44);
        let per_year = c.months_per_year;
        let day_ticks = c.ticks_per_day();
        let a = c.at(0).moon_phase;
        let b = c.at((c.days_per_year() * day_ticks) as u64).moon_phase;
        // After one year the phase advanced by the fractional part of 12.37 cycles.
        let expect = (a + per_year).fract();
        assert!((b - expect).abs() < 1e-6);
        let full = Moment {
            moon_phase: 0.5,
            ..c.at(0)
        };
        assert!((full.moon_illumination() - 1.0).abs() < 1e-12);
    }
}

//! The two time scales of v2 §4.2, in one place.
//!
//! * **Day scale** (body & action time): one game day stands for one real day, so a real
//!   duration is compressed by `day_length / 24 h`. A 15-minute task takes 30 s of play at the
//!   default 48-minute day.
//! * **Year scale** (life-cycle & calendar time): one game year stands for one real year, so a
//!   real duration is compressed by `game_year / real_year`. Six weeks of bone healing take
//!   about 3.7 game days with 8-day seasons.

use crate::schema::TimeScale;
use crate::schema::config::TimeConfig;

/// Simulation ticks per second of play.
pub const TICKS_PER_SECOND: f64 = 20.0;

/// Compression factors for a world's calendar settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeScales {
    /// Seconds of play per game day.
    pub day_length_s: f64,
    /// Game days per game year (4 seasons).
    pub days_per_year: f64,
    /// Seconds in a real day.
    pub real_day_s: f64,
    /// Seconds in a real (tropical) year.
    pub real_year_s: f64,
}

impl TimeScales {
    pub fn new(day_length_min: u32, days_per_season: u32, cfg: &TimeConfig) -> Self {
        Self {
            day_length_s: day_length_min.max(1) as f64 * 60.0,
            days_per_year: days_per_season.max(1) as f64 * 4.0,
            real_day_s: cfg.real_day_s,
            real_year_s: cfg.real_year_days * cfg.real_day_s,
        }
    }

    /// The defaults from `time.ron`.
    pub fn defaults(cfg: &TimeConfig) -> Self {
        Self::new(cfg.day_length_min.default, cfg.days_per_season.default, cfg)
    }

    /// Seconds of play per real second on `scale`.
    pub fn factor(&self, scale: TimeScale) -> f64 {
        match scale {
            TimeScale::Day => self.day_length_s / self.real_day_s,
            TimeScale::Year => self.days_per_year * self.day_length_s / self.real_year_s,
        }
    }

    /// Seconds of play for a real-world duration.
    pub fn play_seconds(&self, real_hours: f64, scale: TimeScale) -> f64 {
        real_hours * 3600.0 * self.factor(scale)
    }

    /// Game days for a real-world duration.
    pub fn game_days(&self, real_hours: f64, scale: TimeScale) -> f64 {
        self.play_seconds(real_hours, scale) / self.day_length_s
    }

    /// Simulation ticks for a real-world duration (at least one).
    pub fn ticks(&self, real_hours: f64, scale: TimeScale) -> u64 {
        (self.play_seconds(real_hours, scale) * TICKS_PER_SECOND)
            .round()
            .max(1.0) as u64
    }

    /// Seconds of play in one game year.
    pub fn year_length_s(&self) -> f64 {
        self.days_per_year * self.day_length_s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Season;
    use crate::schema::config::{FloatSetting, IntSetting, SleepAcceleration};

    fn cfg() -> TimeConfig {
        TimeConfig {
            schema: 1,
            day_length_min: IntSetting {
                default: 48,
                min: 20,
                max: 120,
            },
            days_per_season: IntSetting {
                default: 8,
                min: 3,
                max: 91,
            },
            axial_tilt_deg: FloatSetting {
                default: 23.44,
                min: 0.0,
                max: 45.0,
            },
            starting_season: Season::Spring,
            real_day_s: 86_400.0,
            real_year_days: 365.2422,
            synodic_month_days: 29.530_589,
            sleep: SleepAcceleration {
                max_factor: 100.0,
                ramp_s: 3.0,
            },
        }
    }

    #[test]
    fn day_scale_matches_the_spec_example() {
        let t = TimeScales::defaults(&cfg());
        // A 15-minute real task takes 30 real seconds at the default 48-minute day.
        assert!((t.play_seconds(0.25, TimeScale::Day) - 30.0).abs() < 1e-9);
        assert!(
            (t.game_days(24.0, TimeScale::Day) - 1.0).abs() < 1e-9,
            "a real day is a game day"
        );
        assert_eq!(t.ticks(0.25, TimeScale::Day), 600);
    }

    #[test]
    fn year_scale_maps_a_real_year_to_a_game_year() {
        let t = TimeScales::defaults(&cfg());
        let year_h = 365.2422 * 24.0;
        assert!((t.game_days(year_h, TimeScale::Year) - 32.0).abs() < 1e-6);
        // Six weeks of bone healing ≈ 3.7 game days with 8-day seasons.
        let d = t.game_days(42.0 * 24.0, TimeScale::Year);
        assert!((d - 3.68).abs() < 0.02, "{d}");
        // With a real-length year (91-day seasons) the year scale equals the day scale.
        let real = TimeScales::new(48, 91, &cfg());
        let ratio = real.factor(TimeScale::Year) / real.factor(TimeScale::Day);
        assert!((ratio - 364.0 / 365.2422).abs() < 1e-9);
    }
}

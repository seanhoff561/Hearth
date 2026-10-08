//! One clock, Earth's (E §4.1): a game second is a real second, so a duration in data (real
//! hours) is played as long as it really takes.

/// Simulation ticks per second of play (the game's one rate, `hearth_core::TICKS_PER_SECOND`).
pub const TICKS_PER_SECOND: f64 = hearth_core::TICKS_PER_SECOND as f64;

/// A mean solar day (s).
pub const DAY_S: f64 = 86_400.0;

/// Simulation ticks for a duration in hours (at least one).
pub fn ticks(hours: f64) -> u64 {
    (hours * 3600.0 * TICKS_PER_SECOND).round().max(1.0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quarter_hour_is_a_quarter_hour() {
        assert_eq!(ticks(0.25), 900 * 20);
        assert_eq!(ticks(0.0), 1, "at least a tick");
    }
}

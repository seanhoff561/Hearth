//! The ice ages as deep time feels them (D191): the sea's level and the air's warmth against
//! today's, years ago — stylised cycles in the far past, the record's own outline over the last
//! glacial cycle.

use hearth_content::schema::history::{Cycles, HistorySettings};

/// The climate's swings, from the history settings.
#[derive(Debug, Clone, PartialEq)]
pub struct Climate {
    cycles: Vec<Cycles>,
    curve: Vec<(f64, f32)>,
    degrees_per_metre: f32,
    rain_per_degree: f32,
}

impl Climate {
    pub fn new(s: &HistorySettings) -> Self {
        let mut curve = s.curve.clone();
        curve.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut cycles = s.cycles.clone();
        cycles.sort_by(|a, b| b.from_ya.total_cmp(&a.from_ya));
        Self {
            cycles,
            curve,
            degrees_per_metre: s.degrees_per_metre,
            rain_per_degree: s.rain_per_degree,
        }
    }

    /// Today's climate throughout (tests).
    pub fn today() -> Self {
        Self {
            cycles: Vec::new(),
            curve: Vec::new(),
            degrees_per_metre: 0.05,
            rain_per_degree: 0.04,
        }
    }

    /// The sea against today's (m), `ya` years ago.
    pub fn sea_m(&self, ya: f64) -> f32 {
        let ya = ya.max(0.0);
        // Within the record's outline: the straight line between its points (oldest first).
        if let (Some(first), Some(last)) = (self.curve.first(), self.curve.last())
            && ya <= first.0
        {
            if ya <= last.0 {
                return last.1;
            }
            for w in self.curve.windows(2) {
                let ((a0, m0), (a1, m1)) = (w[0], w[1]);
                if ya <= a0 && ya >= a1 {
                    let t = if a0 > a1 { (a0 - ya) / (a0 - a1) } else { 1.0 };
                    return m0 + (m1 - m0) * t as f32;
                }
            }
            return last.1;
        }
        // Before it: the stylised cycles of the stretch it falls in.
        let Some(c) = self
            .cycles
            .iter()
            .find(|c| ya <= c.from_ya)
            .or(self.cycles.last())
        else {
            return 0.0;
        };
        if c.period_years >= 90_000.0 {
            // The 100,000-year cycles: a slow fall through nine tenths of the cycle into the
            // glacial, a quick rise out of it — an interglacial at each multiple of the period.
            let since = (c.period_years - ya.rem_euclid(c.period_years)) / c.period_years;
            let since = since.rem_euclid(1.0);
            let depth = if since < 0.9 {
                since / 0.9
            } else {
                1.0 - (since - 0.9) / 0.1
            };
            c.low_m * depth as f32
        } else {
            // The 41,000-year cycles: even swings.
            let phase = (ya / c.period_years) * std::f64::consts::TAU;
            c.low_m * (0.5 - 0.5 * phase.cos()) as f32
        }
    }

    /// The air against today's (°C), `ya` years ago.
    pub fn degrees(&self, ya: f64) -> f32 {
        self.sea_m(ya) * self.degrees_per_metre
    }

    /// The rain's share of today's, `ya` years ago.
    pub fn rain(&self, ya: f64) -> f32 {
        (1.0 + self.rain_per_degree * self.degrees(ya)).clamp(0.3, 1.2)
    }
}

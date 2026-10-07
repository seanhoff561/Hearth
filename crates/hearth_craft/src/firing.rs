//! Firing (v2 §11.4): what heat makes of clay. Dried clay turns to pottery only when it is
//! brought into its firing range and held there: below it the clay's water of crystallisation is
//! never driven off, and what comes out of the fire is still clay — whole, but it softens and
//! slumps back to mud in the rain or in a pot of water, and can be fired again. Fired at the
//! bottom of the range, or held there too short a while, a pot is soft and porous; fired well up
//! the range and held, it rings hard. Hotter than the range, earthenware bloats and slumps.

/// How far above the top of its range clay may go before it slumps and bloats (°C).
pub const SLUMP_MARGIN_C: f32 = 50.0;
/// The share of a firing's hold a pot must have had to come out fired at all.
pub const LEAST_HOLD: f32 = 0.25;

/// What a firing made of the clay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fired {
    /// Fired through, with this quality (0–1).
    Fired(f32),
    /// Never hot enough, or not held long enough: still clay.
    Under,
    /// Hotter than it can bear: slumped and bloated.
    Over,
}

/// The firing of clay with firing range `range` (°C) that reached `peak_c` and was held at or
/// above the range's bottom for `hot_h` hours, against the `hold_h` the work asks.
pub fn fired(range: (f32, f32), peak_c: f32, hot_h: f32, hold_h: f32) -> Fired {
    let (lo, hi) = (range.0.min(range.1), range.0.max(range.1));
    if peak_c > hi + SLUMP_MARGIN_C {
        return Fired::Over;
    }
    if peak_c < lo || hot_h < LEAST_HOLD * hold_h {
        return Fired::Under;
    }
    let heat = ((peak_c - lo) / (hi - lo).max(1.0)).clamp(0.0, 1.0);
    let soak = (hot_h / hold_h.max(1e-3)).clamp(0.0, 1.0);
    Fired::Fired((0.2 + 0.45 * heat + 0.3 * soak).clamp(0.05, 0.95))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EARTHENWARE: (f32, f32) = (600.0, 1100.0);

    #[test]
    fn clay_fires_only_in_its_range_and_held() {
        // A low fire: never clay's change.
        assert_eq!(fired(EARTHENWARE, 450.0, 3.0, 1.0), Fired::Under);
        // Hot enough, but only a moment of it.
        assert_eq!(fired(EARTHENWARE, 700.0, 0.1, 1.0), Fired::Under);
        // A bonfire's firing: soft but pottery.
        let Fired::Fired(soft) = fired(EARTHENWARE, 680.0, 1.0, 1.0) else {
            panic!("fired");
        };
        // A kiln's: harder.
        let Fired::Fired(hard) = fired(EARTHENWARE, 980.0, 2.0, 1.0) else {
            panic!("fired");
        };
        assert!(hard > soft + 0.2, "{soft} {hard}");
        // Too hot for earthenware.
        assert_eq!(fired(EARTHENWARE, 1200.0, 2.0, 1.0), Fired::Over);
    }
}

//! Watching the world (Creative's spectating, Amendment P §3.3): the speeds time may be watched
//! at. (The Observer's chronicle, overlays and following of persons went with the people,
//! Amendment E §2.2.)

/// The speeds the eye watches time at: game seconds per real second, by name (as lived: the
/// world's own pace).
pub const SPEEDS: [(f64, &str); 9] = [
    (0.0, "stopped"),
    (1.0, "as lived"),
    (60.0, "a minute a second"),
    (3600.0, "an hour a second"),
    (86_400.0, "a day a second"),
    (30.0 * 86_400.0, "a month a second"),
    (365.0 * 86_400.0, "a year a second"),
    (3_650.0 * 86_400.0, "ten years a second"),
    (36_500.0 * 86_400.0, "a hundred years a second"),
];

/// The speed that is the world's own pace.
pub const AS_LIVED: usize = 1;

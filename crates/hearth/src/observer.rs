//! The Observer (V2.1 §15.4; H9): what watching the world needs of the server — the chronicle
//! of deep time and of the living world, the globe's overlays of who lives where, whose culture,
//! what is known and how the people look, and the speeds time may be watched at.

use glam::DVec3;
use hearth_math::Planet;
use hearth_people::history::History;
use hearth_protocol::{ChronicleEntry, OverlayKind, OverlayMap};
use hearth_worldgen::Terrain;

/// The overlay's size (pixels): a coarse equirectangular picture over the globe's map.
pub const OVERLAY_W: usize = 256;
pub const OVERLAY_H: usize = 128;
/// The most lines of the chronicle told.
const CHRONICLE_LINES: usize = 120;

/// The speeds the Observer watches time at: game seconds per real second, by name (as lived:
/// the world's own pace). The faster ones pass the years as the household and demographic
/// tiers live them.
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

/// A history cell's centre in the world (x, z).
fn cell_centre(h: &History, cell: u32) -> (f64, f64) {
    let c = h.cell_m * h.n as f64;
    (
        ((cell as usize % h.n) as f64 + 0.5) * h.cell_m,
        -c * 0.5 + ((cell as usize / h.n) as f64 + 0.5) * h.cell_m,
    )
}

/// The history cell a world column falls in.
fn cell_of(h: &History, x: f64, z: f64) -> Option<usize> {
    let c = h.cell_m * h.n as f64;
    let i = (x.rem_euclid(c) / h.cell_m) as usize;
    let j = ((z + c * 0.5) / h.cell_m).floor();
    (j >= 0.0 && (j as usize) < h.n).then(|| j as usize * h.n + i.min(h.n - 1))
}

/// A point to go to: on the ground there, a little above.
fn ground(terrain: &Terrain, x: f64, z: f64) -> DVec3 {
    let s = terrain.sample(x.floor() as i32, z.floor() as i32);
    DVec3::new(x, s.height.max(s.water) as f64 + 2.0, z)
}

/// The chronicle, newest first: the living world's notable events (by the days since), then
/// deep time's (by the years ago), each where it happened.
pub fn chronicle(
    notable: &[hearth_people::notable::Notable],
    history: Option<&History>,
    terrain: &Terrain,
    day: f64,
    year_days: f64,
) -> Vec<ChronicleEntry> {
    let mut out: Vec<ChronicleEntry> = notable
        .iter()
        .rev()
        .map(|n| {
            let ago = day - n.day;
            let when = if ago < 1.0 {
                "today".to_owned()
            } else if ago < year_days {
                format!("{ago:.0} days ago")
            } else {
                format!("{:.0} years ago", ago / year_days)
            };
            ChronicleEntry {
                when,
                text: n.text.clone(),
                at: Some(ground(terrain, n.at.x, n.at.z)),
            }
        })
        .collect();
    if let Some(h) = history {
        out.extend(h.chronicle.iter().rev().map(|e| {
            let (x, z) = cell_centre(h, e.cell);
            ChronicleEntry {
                when: format!("{:.0} years ago", e.ya),
                text: crate::history_cli::tell(h, e, None),
                at: Some(ground(terrain, x, z)),
            }
        }));
    }
    out.truncate(CHRONICLE_LINES);
    out
}

/// What each history cell holds, summed over its demes: its people, its largest people's
/// lineage, what any of its peoples knows, and their looks' sunlight weighted by numbers.
struct CellSum {
    people: f32,
    lineage: Option<(u32, f32)>,
    knows: u128,
    sun: f32,
}

/// A colour for a lineage (its culture and language), steady from its id.
fn lineage_colour(id: u32) -> [u8; 3] {
    let h = hearth_math::hash::hash2(0x0c01_0ab5, id as u64);
    let hue = (h % 360) as f32;
    let (s, v) = (0.65, 0.95);
    let c = v * s;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let (r, g, b) = match (hue / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    ]
}

/// A map overlay (V2.1 §15.4) from deep time's peoples as they stand, the living bands about the
/// player counted where they are; none where there is no history and no band.
pub fn overlay(
    kind: OverlayKind,
    history: Option<&History>,
    bands: &[(DVec3, usize)],
    planet: &Planet,
) -> Option<OverlayMap> {
    let h = history?;
    let cells = h.n * h.n;
    let mut sums: Vec<CellSum> = (0..cells)
        .map(|_| CellSum {
            people: 0.0,
            lineage: None,
            knows: 0,
            sun: 0.0,
        })
        .collect();
    for d in &h.demes {
        let Some(c) = sums.get_mut(d.cell as usize) else {
            continue;
        };
        c.people += d.people;
        c.knows |= d.knows();
        c.sun += d.pool.sun * d.people;
        if c.lineage.is_none_or(|(_, n)| d.people > n) {
            c.lineage = Some((d.lineage, d.people));
        }
    }
    for &(at, n) in bands {
        if let Some(c) = cell_of(h, at.x, at.z).and_then(|i| sums.get_mut(i)) {
            c.people = c.people.max(n as f32);
        }
    }
    let area_km2 = (h.cell_m / 1000.0).powi(2) as f32;
    let most = sums
        .iter()
        .map(|c| c.people / area_km2)
        .fold(0.0f32, f32::max)
        .max(1e-6);
    let mut rgba = vec![[0u8; 4]; OVERLAY_W * OVERLAY_H];
    for py in 0..OVERLAY_H {
        let lat = std::f32::consts::FRAC_PI_2
            - (py as f32 + 0.5) / OVERLAY_H as f32 * std::f32::consts::PI;
        for px in 0..OVERLAY_W {
            let lon = -std::f32::consts::PI
                + (px as f32 + 0.5) / OVERLAY_W as f32 * std::f32::consts::TAU;
            let (x, z) = crate::globe::world_xz(planet, lat, lon);
            let Some(c) = cell_of(h, x as f64, z as f64).and_then(|i| sums.get(i)) else {
                continue;
            };
            if c.people < 0.5 {
                continue;
            }
            rgba[py * OVERLAY_W + px] = match kind {
                OverlayKind::People => {
                    // Thin to thick: yellow to deep red, on a log scale.
                    let t = ((c.people / area_km2).ln() - (most * 1e-3).ln()) / (1e3f32).ln();
                    let t = t.clamp(0.05, 1.0);
                    [
                        255,
                        (230.0 * (1.0 - t)) as u8,
                        40,
                        (110.0 + 120.0 * t) as u8,
                    ]
                }
                OverlayKind::Cultures => {
                    let [r, g, b] = c
                        .lineage
                        .map_or([200, 200, 200], |(l, _)| lineage_colour(l));
                    [r, g, b, 190]
                }
                OverlayKind::Knowledge(k) => {
                    if k < 128 && c.knows & (1u128 << k) != 0 {
                        [255, 150, 30, 210]
                    } else {
                        [90, 100, 120, 150]
                    }
                }
                OverlayKind::Looks => {
                    let t = (c.sun / c.people.max(1e-6)).clamp(0.0, 1.0);
                    let lerp = |a: f32, b: f32| (a + (b - a) * t) as u8;
                    [lerp(238.0, 92.0), lerp(212.0, 58.0), lerp(190.0, 38.0), 210]
                }
            };
        }
    }
    let legend = match kind {
        OverlayKind::People => "Where people live: thin (yellow) to thick (red)".to_owned(),
        OverlayKind::Cultures => "Peoples: each culture and its language its own colour".to_owned(),
        OverlayKind::Knowledge(k) => format!(
            "Where {} is known (orange)",
            h.techniques
                .get(k)
                .map_or("?", |t| t.rsplit(':').next().unwrap_or(t))
                .replace('_', " ")
        ),
        OverlayKind::Looks => "How the peoples look: paler to darker".to_owned(),
    };
    Some(OverlayMap {
        kind,
        width: OVERLAY_W as u32,
        height: OVERLAY_H as u32,
        rgba,
        legend,
    })
}

/// The number of techniques the world's deep time follows (the knowledge overlays to cycle).
pub fn techniques(history: Option<&History>) -> usize {
    history.map_or(0, |h| h.techniques.len().min(128))
}

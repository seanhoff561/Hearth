//! The motion timing check (Amendment P §8, P5): every visual that moves with a clock is listed
//! in `docs/design/motion-timing.md` with where it moves, and the speeds the shaders give are the
//! speeds the document states. The speeds summed on the CPU have their own checks: the clouds'
//! drift and change of shape and the sky in fast-forward (`hearth::environment`), the waves
//! (`water::phase_speeds`), the bodies' feet (`hearth_character` and `hearth_fauna` tests).

use std::f32::consts::TAU;

const SHADERS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/shaders");

/// Each clock a shader reads, by file: the world's seconds and ticks, the sky's own seconds, the
/// sums carried on the CPU (the clouds' drift and change of shape, the waves' phases, the air's
/// carry), the particles' and the smoke's clocks, the heat shimmer's, a frame's seconds.
const CLOCKS: &[(&str, &str)] = &[
    ("terrain.wgsl", "g.params.x"),
    ("common.wgsl", "g.params.x"),
    ("common.wgsl", "g.params.y"),
    ("sky.wgsl", "P.camera.w"),
    ("sky.wgsl", "P.clouds.zw"),
    ("sky.wgsl", "P.ambient.w"),
    ("water.wgsl", "water.phase"),
    ("precip.wgsl", "P.cam_mod.w"),
    ("precip.wgsl", "P.drift"),
    ("smoke.wgsl", "P.wind.w"),
    ("post.wgsl", "P.senses2.y"),
    ("meter.wgsl", "P.p.x"),
];

/// Shaders that declare a clock only for others to read (none now: `common.wgsl` declares the
/// globals and reads them itself, in the sway and the animated textures).
const DECLARES: &[&str] = &[];

fn doc() -> String {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/design/motion-timing.md"
    );
    std::fs::read_to_string(p).expect("motion-timing.md")
}

/// The table's rows: (what, clock, speed, where).
fn rows(doc: &str) -> Vec<[String; 4]> {
    doc.lines()
        .filter(|l| l.starts_with("| ") && !l.starts_with("| What") && !l.starts_with("|---"))
        .filter_map(|l| {
            let c: Vec<String> = l
                .split(" | ")
                .map(|c| c.trim_matches('|').trim().to_owned())
                .collect();
            (c.len() >= 4).then(|| [c[0].clone(), c[1].clone(), c[2].clone(), c[3].clone()])
        })
        .collect()
}

fn shader(file: &str) -> String {
    std::fs::read_to_string(format!("{SHADERS}/{file}")).expect(file)
}

/// The function a line of a shader is in.
fn enclosing(src: &str, line: usize) -> String {
    src.lines()
        .take(line + 1)
        .filter_map(|l| l.strip_prefix("fn "))
        .last()
        .map(|l| l.split('(').next().unwrap_or("").to_owned())
        .unwrap_or_default()
}

/// A function's body.
fn body(src: &str, name: &str) -> String {
    let start = src.find(&format!("fn {name}(")).expect(name);
    let rest = &src[start..];
    let end = rest.find("\n}").expect("end of function");
    rest[..end].to_owned()
}

/// The numbers after each `pat` in `s`, in order (not where `pat` ends a longer name).
fn after(s: &str, pat: &str) -> Vec<f32> {
    s.match_indices(pat)
        .filter(|(i, _)| {
            !s[..*i]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.')
        })
        .filter_map(|(i, _)| {
            let n: String = s[i + pat.len()..]
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            n.parse().ok()
        })
        .collect()
}

/// The speed columns of the rows whose places name `file` and `func`.
fn speed_of(rows: &[[String; 4]], file: &str, func: &str) -> String {
    let s: Vec<&str> = rows
        .iter()
        .filter(|r| r[3].contains(file) && r[3].contains(&format!("`{func}`")))
        .map(|r| r[2].as_str())
        .collect();
    assert!(!s.is_empty(), "no row for {file} ({func})");
    s.join("; ")
}

fn says(speed: &str, n: f32, decimals: usize) -> bool {
    speed.contains(&format!("{n:.decimals$}"))
}

#[test]
fn every_animated_visual_is_listed() {
    let doc = doc();
    let rows = rows(&doc);
    assert!(rows.len() > 15, "the table: {} rows", rows.len());
    let mut missing = Vec::new();
    for (file, clock) in CLOCKS {
        let src = shader(file);
        let mut seen = false;
        for (i, l) in src.lines().enumerate() {
            if l.trim_start().starts_with("//") || !l.contains(clock) {
                continue;
            }
            seen = true;
            let f = enclosing(&src, i);
            let listed = rows
                .iter()
                .any(|r| r[3].contains(file) && r[3].contains(&format!("`{f}`")));
            if !listed {
                missing.push(format!("{file} ({f}) reads {clock}"));
            }
        }
        assert!(seen, "{file} no longer reads {clock}: update the check");
    }
    // A shader reading a clock not named here is one this check does not know of.
    for e in std::fs::read_dir(SHADERS).expect("shaders") {
        let path = e.expect("entry").path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let src = std::fs::read_to_string(&path).expect("shader");
        for l in src.lines().filter(|l| l.trim_start().starts_with("//")) {
            let l = l.to_lowercase();
            let names_a_clock =
                (l.contains("seconds") || l.contains(" ticks")) && !l.contains("per second");
            if names_a_clock
                && !CLOCKS.iter().any(|(f, _)| *f == name)
                && !DECLARES.contains(&name.as_str())
            {
                missing.push(format!("{name}: a clock in \"{}\"", l.trim()));
            }
        }
    }
    assert!(missing.is_empty(), "not in motion-timing.md: {missing:#?}");
    // Every row says its clock and its speed.
    for r in &rows {
        assert!(!r[1].is_empty() && !r[2].is_empty(), "{r:?}");
    }
}

#[test]
fn the_shaders_move_at_the_documented_speeds() {
    let rows = rows(&doc());
    let terrain = shader("terrain.wgsl");

    // Plants' sway: the periods of its two waves (in `common.wgsl`, as the highlight's mask
    // sways with the plant it outlines).
    let w = body(&shader("common.wgsl"), "wind");
    let periods: Vec<f32> = after(&w, "t * ").iter().map(|a| TAU / a).collect();
    let s = speed_of(&rows, "common.wgsl", "wind");
    assert_eq!(periods.len(), 2, "{w}");
    for p in &periods {
        assert!(says(&s, *p, 1), "sway period {p:.1} s not in \"{s}\"");
    }
    // Real gusts sway grass and leaves over seconds, not faster than a hertz.
    assert!(
        periods.iter().all(|p| (1.0..6.0).contains(p)),
        "{periods:?}"
    );

    // Caustics: two layers, each drifting at |v| × the map's scale (its repeat in blocks, as
    // `wrap_freq` snaps it to whole repeats over WRAP blocks: within a thousandth).
    let c = body(&terrain, "caustic_light");
    let v = after(&c, "t * ");
    let repeats = after(&c, "wrap_freq(1.0 / ");
    assert_eq!(repeats.len(), 2, "{c}");
    let scale = [repeats[0], repeats[1]];
    let speeds = [
        (v[0].hypot(v[1]) * scale[0] * 100.0).round() / 100.0,
        (v[2].hypot(v[3]) * scale[1] * 100.0).round() / 100.0,
    ];
    let s = speed_of(&rows, "terrain.wgsl", "caustic_light");
    for sp in speeds {
        assert!(says(&s, sp, 2), "caustics at {sp} m/s not in \"{s}\"");
    }

    // The heat shimmer's two waves (Hz).
    let post = shader("post.wgsl");
    let g = body(&post, "graded");
    let hz: Vec<f32> = after(&g, "t * ").iter().map(|a| a / TAU).collect();
    let s = speed_of(&rows, "post.wgsl", "graded");
    let (lo, hi) = (
        hz.iter().cloned().fold(f32::MAX, f32::min),
        hz.iter().cloned().fold(0.0, f32::max),
    );
    assert!(
        says(&s, lo, 1) && says(&s, hi, 1),
        "shimmer {lo:.1}–{hi:.1} Hz not in \"{s}\""
    );

    // Snow falls at 0.8–1.3 m/s; rain at 8.5 m/s × 0.85–1.15.
    let precip = shader("precip.wgsl");
    let p = body(&precip, "vs_main");
    let snow = after(&p, "whole_speed(");
    assert!(
        !snow.is_empty() && p.contains("+ 0.5 * fract(r * 3.7)"),
        "{p}"
    );
    let s = speed_of(&rows, "precip.wgsl", "vs_main");
    assert!(
        says(&s, snow[0], 1) && says(&s, snow[0] + 0.5, 1),
        "snow {}–{} m/s not in \"{s}\"",
        snow[0],
        snow[0] + 0.5
    );
    let rain = include_str!("../src/precip.rs");
    let fall = after(rain, "Vec3::new(p.wind.x, -");
    assert_eq!(fall.len(), 1);
    assert!(
        says(&s, fall[0] * 0.85, 1) && says(&s, fall[0] * 1.15, 1),
        "rain {:.1}–{:.1} m/s not in \"{s}\"",
        fall[0] * 0.85,
        fall[0] * 1.15
    );

    // Smoke: a puff's life near a fire, its first rise, and carried at the wind's speed by the
    // end (d/dt of wind × a² × life × k is 2k × wind at a = 1).
    let smoke = shader("smoke.wgsl");
    let m = body(&smoke, "vs_main");
    let life = after(&m, "let life = select(")[0];
    let k = after(&m, "a * a * life * ")[0];
    let top = after(&m, "let top = select(")[0];
    let s = speed_of(&rows, "smoke.wgsl", "vs_main");
    assert!(says(&s, life, 0), "a puff's life {life} s not in \"{s}\"");
    assert!(
        (2.0 * k - 1.0).abs() < 1e-3,
        "smoke drifts at {} of the wind",
        2.0 * k
    );
    // h = top (1 - (1 - a)²): its first rise 2 top / life, top at 0.6–1 of its full.
    let (r0, r1) = (2.0 * top * 0.6 / life, 2.0 * top / life);
    assert!(
        says(&s, r0, 1) && says(&s, r1, 1),
        "smoke rises {r0:.1}–{r1:.1} m/s, not in \"{s}\""
    );

    // The eye's highlight meter adapts with a time constant of 1 / k s.
    let meter = shader("meter.wgsl");
    let k = after(&meter, "exp(-P.p.x * ")[0];
    let s = speed_of(&rows, "meter.wgsl", "meter_main");
    assert!(
        says(&s, 1.0 / k, 2),
        "the meter's {:.2} s not in \"{s}\"",
        1.0 / k
    );
}

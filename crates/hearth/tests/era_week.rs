//! H8's era reviews (V2.1 §15.5; `docs/review/era-*.md`): a sample week of each Paleolithic
//! era's people about a life born among them, logged to `bench-out/h8/<era>_week.md` — how many
//! are about and of what ages, what they do through the day, what is said and how much of it the
//! newborn-grown player makes out. The people's works are guarded against anachronism by a debug
//! assertion (H6), so a week without a panic is a week of only what they know. Slow (minutes an
//! era): `cargo test -p hearth --test era_week -- --ignored --nocapture`.

mod common;

use std::collections::BTreeMap;
use std::fmt::Write as _;

use common::*;

/// What a person is about, in a word.
fn about(d: &hearth_people::Doing) -> String {
    let s = format!("{d:?}");
    s.split([' ', '{', '(']).next().unwrap_or("").to_owned()
}

#[test]
#[ignore]
fn a_sample_week_of_each_era() {
    let days: u64 = std::env::var("DAYS")
        .ok()
        .and_then(|d| d.parse().ok())
        .unwrap_or(7);
    let only = std::env::var("ERA").ok();
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench-out/h8");
    std::fs::create_dir_all(&out).expect("bench-out/h8");
    for era in [
        "hearth:lower_paleolithic",
        "hearth:middle_paleolithic",
        "hearth:upper_paleolithic",
    ] {
        if only.as_deref().is_some_and(|o| !era.ends_with(o)) {
            continue;
        }
        let key = era.rsplit(':').next().unwrap_or(era);
        let dir = temp(&format!("week-{key}"));
        let t0 = std::time::Instant::now();
        let mut w = World::start_in(
            &dir,
            hearth_save::KnowledgeMode::default(),
            3,
            false,
            era,
            Some(0),
        );
        let born = w.born.clone().expect("born into a household");
        let mut log = String::new();
        let _ = writeln!(log, "# A sample week: {key}\n");
        let _ = writeln!(
            log,
            "Born into a household: a mother of {:.0}, a father of {:.0}, {} brothers and sisters; the \
             player starting grown at {:.0}.\n",
            born.ages[0],
            born.ages[2],
            born.siblings.len(),
            born.ages[1]
        );
        // Every half hour of the world's day for the week: who is about (within 150 m), and what
        // each is doing, by the hour of the day.
        let half_hour = (w.ticks_per_day / 48.0).round().max(1.0) as u64;
        let mut by_hour: BTreeMap<u32, BTreeMap<String, u32>> = BTreeMap::new();
        let mut seen: BTreeMap<u64, (bool, String, f32)> = BTreeMap::new();
        let mut near_counts: Vec<usize> = Vec::new();
        for _ in 0..days * 48 {
            w.run(half_hour);
            let me = w.mover.pos;
            let hour = (w.calendar.at(w.ticks).time_of_day * 24.0) as u32;
            let near: Vec<_> = w
                .people
                .iter()
                .filter(|v| !v.dead && (v.pos - me).length() < 150.0)
                .collect();
            near_counts.push(near.len());
            for v in near {
                *by_hour
                    .entry(hour / 3 * 3)
                    .or_default()
                    .entry(about(&v.doing))
                    .or_default() += 1;
                seen.insert(v.id, (v.female, format!("{:?}", v.stage), v.height_m));
            }
        }
        let mean = near_counts.iter().sum::<usize>() as f64 / near_counts.len().max(1) as f64;
        let most = near_counts.iter().copied().max().unwrap_or(0);
        let _ = writeln!(
            log,
            "## Who is about\n\n{} people seen within 150 m over {days} days; {mean:.1} about at a \
             time on average, at most {most}.\n",
            seen.len()
        );
        let mut stages: BTreeMap<String, (u32, u32)> = BTreeMap::new();
        for (female, stage, _) in seen.values() {
            let e = stages.entry(stage.clone()).or_default();
            if *female {
                e.0 += 1;
            } else {
                e.1 += 1;
            }
        }
        let _ = writeln!(
            log,
            "| Stage | Women and girls | Men and boys |\n|---|---|---|"
        );
        for (stage, (f, m)) in &stages {
            let _ = writeln!(log, "| {stage} | {f} | {m} |");
        }
        let _ = writeln!(
            log,
            "\n## What they do through the day\n\nPeople-half-hours by what they were about, in \
             three-hour blocks of the day (the world's hour):\n"
        );
        let kinds: Vec<String> = {
            let mut k: Vec<String> = by_hour.values().flat_map(|m| m.keys().cloned()).collect();
            k.sort();
            k.dedup();
            k
        };
        let _ = writeln!(log, "| Hours | {} |", kinds.join(" | "));
        let _ = writeln!(log, "|---|{}", "---|".repeat(kinds.len()));
        for (h, m) in &by_hour {
            let cells: Vec<String> = kinds
                .iter()
                .map(|k| m.get(k).copied().unwrap_or(0).to_string())
                .collect();
            let _ = writeln!(log, "| {h:02}–{:02} | {} |", h + 3, cells.join(" | "));
        }
        let _ = writeln!(log, "\n## What was said\n");
        let understood = if w.heard.is_empty() {
            0.0
        } else {
            w.heard.iter().map(|h| h.understood).sum::<f32>() / w.heard.len() as f32
        };
        let _ = writeln!(
            log,
            "{} lines heard, {:.0} % of them made out on average. The first of them:\n",
            w.heard.len(),
            understood * 100.0
        );
        for h in w.heard.iter().take(12) {
            let _ = writeln!(
                log,
                "- {}: \"{}\" ({}){}",
                h.speaker,
                h.spoken,
                h.sense,
                h.gesture
                    .as_ref()
                    .map(|g| format!(", {g}"))
                    .unwrap_or_default()
            );
        }
        let _ = writeln!(
            log,
            "\nThe week took {:.0} s to run.",
            t0.elapsed().as_secs_f64()
        );
        let path = out.join(format!("{key}_week.md"));
        std::fs::write(&path, &log).expect("the week's log");
        println!("{log}");
        assert!(!seen.is_empty(), "people about in {era}");
        drop(w);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

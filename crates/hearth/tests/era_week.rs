//! H8's era reviews (V2.1 §15.5; `docs/review/era-*.md`): a sample week of each Paleolithic
//! era's people about a life born among them, logged to `bench-out/h8/<era>_week.md` — how many
//! are about and of what ages, what they do through the day (by the place's own hour), who works
//! at what, what is said and how much of it the newborn-grown player makes out, their camp, and
//! the inspector's record of two of them at the week's end. The people's works are guarded
//! against anachronism by a debug assertion (H6), so a week without a panic is a week of only
//! what they know. Slow (many minutes an era): `cargo test -p hearth --test era_week --
//! --ignored --nocapture` (`ERA=upper` for one era, `DAYS=3` for a shorter week).

mod common;

use std::collections::BTreeMap;
use std::fmt::Write as _;

use common::*;
use hearth_people::Doing;
use hearth_protocol::ToServer;

/// What a person is about, in a word.
fn about(d: &Doing) -> String {
    let s = format!("{d:?}");
    s.split([' ', '{', '(']).next().unwrap_or("").to_owned()
}

/// A person's sex and stage, in words.
fn who(female: bool, stage: &str) -> String {
    match (stage, female) {
        ("Adult", true) => "women",
        ("Adult", false) => "men",
        (_, true) => "girls",
        (_, false) => "boys",
    }
    .to_owned()
}

/// A table of counts, rows by columns.
fn table(log: &mut String, corner: &str, rows: &BTreeMap<String, BTreeMap<String, u32>>) {
    let mut cols: Vec<&String> = rows.values().flat_map(|m| m.keys()).collect();
    cols.sort();
    cols.dedup();
    let names: Vec<&str> = cols.iter().map(|c| c.as_str()).collect();
    let _ = writeln!(log, "| {corner} | {} |", names.join(" | "));
    let _ = writeln!(log, "|---|{}", "---|".repeat(cols.len()));
    for (r, m) in rows {
        let cells: Vec<String> = cols
            .iter()
            .map(|c| m.get(*c).copied().unwrap_or(0).to_string())
            .collect();
        let _ = writeln!(log, "| {r} | {} |", cells.join(" | "));
    }
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
        if only.as_deref().is_some_and(|o| !era.contains(o)) {
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
        let crafts = hearth_craft::Crafts::from_content(&w.content, &w.items);
        let planet = *w.mirror.planet();
        let mut log = String::new();
        let _ = writeln!(log, "# A sample week: {key}\n");
        let _ = writeln!(
            log,
            "Born into a household at latitude {:.1}°: a mother of {:.0}, a father of {:.0}, {} \
             brothers and sisters; the player starting grown at {:.0}. The recent past took {:.0} \
             s to live.\n",
            born.latitude_deg,
            born.ages[0],
            born.ages[2],
            born.siblings.len(),
            born.ages[1],
            t0.elapsed().as_secs_f64()
        );
        // Every half hour of the world's day for the week: who is about (within 150 m), and what
        // each is doing, by the place's own hour. The band the player was born into — those about
        // in the first half hour, as soon as they are drawn out — is followed through the week as
        // a child of theirs would follow them: the player is put down where most of them are (a
        // debug move) when they have gone on more than 40 m.
        let half_hour = (w.ticks_per_day / 48.0).round().max(1.0) as u64;
        let start = w.mover.pos;
        let mut band: std::collections::BTreeSet<u64> = Default::default();
        let (mut walked, mut farthest, mut lost) = (0.0f64, 0.0f64, 0u32);
        let mut last: Option<glam::DVec3> = None;
        let mut died: Option<String> = None;
        let t_start = w.ticks as f64;
        let mut by_hour: BTreeMap<String, BTreeMap<String, u32>> = BTreeMap::new();
        let mut work: BTreeMap<String, BTreeMap<String, u32>> = BTreeMap::new();
        let mut seen: BTreeMap<u64, (bool, String, f32)> = BTreeMap::new();
        let mut near_counts: Vec<usize> = Vec::new();
        let mut asleep_by_day = 0u32;
        for step in 0..days * 48 {
            // The half hour a few minutes at a time, so the band is not lost between looks.
            let mut seen_them = false;
            for _ in 0..6 {
                w.run((half_hour / 6).max(1));
                if band.is_empty() {
                    let me = w.mover.pos;
                    band = w
                        .people
                        .iter()
                        .filter(|v| !v.dead && (v.pos - me).length() < 150.0)
                        .map(|v| v.id)
                        .collect();
                }
                let ours: Vec<glam::DVec3> = w
                    .people
                    .iter()
                    .filter(|v| !v.dead && band.contains(&v.id))
                    .map(|v| v.pos)
                    .collect();
                if ours.is_empty() {
                    continue;
                }
                seen_them = true;
                let mid = ours.iter().copied().sum::<glam::DVec3>() / ours.len() as f64;
                let flat = |d: glam::DVec3| d.x.hypot(d.z);
                if let Some(l) = last {
                    walked += flat(mid - l);
                }
                last = Some(mid);
                farthest = farthest.max(flat(mid - start));
                if flat(mid - w.mover.pos) > 40.0 {
                    let before = w.mover.pos;
                    w.server.send(ToServer::Place(mid));
                    w.until(10.0, |w| w.mover.pos != before);
                }
            }
            if !seen_them {
                lost += 1;
            }
            if died.is_none()
                && let Some(d) = w.body.as_ref().and_then(|b| b.dead.as_ref())
            {
                died = Some(format!(
                    "{:.1} days in: {d:?}",
                    (w.ticks as f64 - t_start) / w.ticks_per_day
                ));
                // The week goes on with the player living on as one of the grown of its band
                // (life after death, H8), so the band is watched to its end.
                if let Some(id) = w
                    .people
                    .iter()
                    .filter(|v| !v.dead && band.contains(&v.id))
                    .filter(|v| format!("{:?}", v.stage) == "Adult")
                    .map(|v| v.id)
                    .min()
                {
                    w.server.send(ToServer::Inhabit(id));
                    w.until(10.0, |w| w.body.as_ref().is_some_and(|b| b.dead.is_none()));
                }
            }
            // The player drinks, every six hours, at the nearest open water within 400 m (it
            // stands about, and would otherwise die of thirst in the week).
            if step % 12 == 11 {
                let back = w.mover.pos;
                let t = w.generator.terrain.clone();
                let water = (1..=40).find_map(|ring| {
                    let r = ring as f64 * 10.0;
                    let n = (r * std::f64::consts::TAU / 8.0).ceil() as usize;
                    (0..n).find_map(|k| {
                        let a = k as f64 / n as f64 * std::f64::consts::TAU;
                        let (x, z) = (back.x + a.cos() * r, back.z + a.sin() * r);
                        let c = t.sample(x.floor() as i32, z.floor() as i32);
                        (c.water.is_finite() && c.water > c.height + 0.3 && !c.ocean)
                            .then_some((x, z))
                    })
                });
                if let Some((x, z)) = water {
                    w.go(x, z);
                    w.until(10.0, |w| !w.find(6, |n, _| n == "water").is_empty());
                    if let Some(p) = w.find(6, |n, _| n == "water").into_iter().next() {
                        w.go_to_block(p);
                        w.server
                            .send(ToServer::Drink(hearth_protocol::DrinkFrom::Water(
                                hearth_protocol::AimAt::Block { pos: p, top: true },
                            )));
                        w.run(2);
                    }
                    w.go(back.x, back.z);
                }
            }
            let me = w.mover.pos;
            let local = w
                .calendar
                .at(w.ticks)
                .local_time(planet.solar_time_offset(me.x));
            let hour = (local * 24.0) as u32 / 3 * 3;
            let near: Vec<_> = w
                .people
                .iter()
                .filter(|v| !v.dead && (v.pos - me).length() < 150.0)
                .collect();
            near_counts.push(near.len());
            for v in near {
                let stage = format!("{:?}", v.stage);
                let doing = about(&v.doing);
                if doing == "Sleeping" && (9..15).contains(&hour) {
                    asleep_by_day += 1;
                }
                *by_hour
                    .entry(format!("{hour:02}–{:02}", hour + 3))
                    .or_default()
                    .entry(doing)
                    .or_default() += 1;
                let task = match &v.doing {
                    Doing::Working { recipe } | Doing::Imitating { recipe, .. } => {
                        crafts.recipes.get(*recipe).map(|r| {
                            let watching = matches!(v.doing, Doing::Imitating { .. });
                            format!(
                                "{}{}",
                                r.def.name,
                                if watching { " (watching)" } else { "" }
                            )
                        })
                    }
                    Doing::Sharing { .. } => Some("sharing food".to_owned()),
                    Doing::Tending { .. } => Some("tending the hurt".to_owned()),
                    Doing::Taking { .. } => Some("taking up things".to_owned()),
                    Doing::Feeding => Some("feeding".to_owned()),
                    Doing::Drinking => Some("drinking".to_owned()),
                    _ => None,
                };
                if let Some(task) = task {
                    *work
                        .entry(task)
                        .or_default()
                        .entry(who(v.female, &stage))
                        .or_default() += 1;
                }
                seen.insert(v.id, (v.female, stage, v.height_m));
            }
        }
        let mean = near_counts.iter().sum::<usize>() as f64 / near_counts.len().max(1) as f64;
        let most = near_counts.iter().copied().max().unwrap_or(0);
        let least = near_counts.iter().copied().min().unwrap_or(0);
        let _ = writeln!(
            log,
            "## Who is about\n\n{} people seen within 150 m over {days} days; {mean:.1} about at a \
             time on average, {least} to {most}. The band of {} (those about at the start) went \
             {:.1} km in the week, its middle at most {:.0} m from where the life began; it was \
             out of sight {lost} half-hours of {}.\n",
            seen.len(),
            band.len(),
            walked / 1000.0,
            farthest,
            days * 48
        );
        let mut stages: BTreeMap<String, BTreeMap<String, u32>> = BTreeMap::new();
        for (female, stage, _) in seen.values() {
            *stages
                .entry(stage.clone())
                .or_default()
                .entry(if *female { "female" } else { "male" }.to_owned())
                .or_default() += 1;
        }
        table(&mut log, "Stage", &stages);
        let _ = writeln!(
            log,
            "\n## What they do through the day\n\nPeople-half-hours by what they were about, in \
             three-hour blocks of the place's own (solar) hour; {asleep_by_day} people-half-hours \
             asleep between 9 and 15:\n"
        );
        table(&mut log, "Hours", &by_hour);
        let _ = writeln!(
            log,
            "\n## Who works at what\n\nPeople-half-hours at each work, by who did it:\n"
        );
        table(&mut log, "Work", &work);
        let _ = writeln!(log, "\n## What was said\n");
        let understood = if w.heard.is_empty() {
            0.0
        } else {
            w.heard.iter().map(|h| h.understood).sum::<f32>() / w.heard.len() as f32
        };
        let mut said: BTreeMap<String, (u32, String)> = BTreeMap::new();
        for h in &w.heard {
            let e = said.entry(h.sense.clone()).or_insert((
                0,
                format!(
                    "\"{}\" ({}){}",
                    h.spoken,
                    h.sense,
                    h.gesture
                        .as_ref()
                        .map(|g| format!(", {g}"))
                        .unwrap_or_default()
                ),
            ));
            e.0 += 1;
        }
        let mut said: Vec<(u32, String)> = said.into_values().collect();
        said.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let _ = writeln!(
            log,
            "{} lines heard, {} different, {:.0} % of them made out on average; the most said:\n",
            w.heard.len(),
            said.len(),
            understood * 100.0
        );
        for (n, line) in said.iter().take(16) {
            let _ = writeln!(log, "- {line} × {n}");
        }
        // The camp about the player: its fire and its beds.
        let me = w.mover.pos;
        let (mut fires, mut beds) = (Vec::new(), 0);
        for dx in -60..=60 {
            for dz in -60..=60 {
                for dy in -6..=6 {
                    let p = hearth_math::BlockPos::new(
                        me.x.floor() as i32 + dx,
                        me.y.floor() as i32 + dy,
                        me.z.floor() as i32 + dz,
                    );
                    let Some(s) = w.mirror.block(p) else {
                        continue;
                    };
                    let name = w.reg.state_string(s);
                    if name.starts_with("hearth:campfire") {
                        fires.push(name);
                    } else if name.ends_with("_bed") {
                        beds += 1;
                    }
                }
            }
        }
        let _ = writeln!(
            log,
            "\n## The camp\n\nWithin 60 m of the player at the week's end: {} ({}), {beds} beds.",
            if fires.is_empty() {
                "no fire".to_owned()
            } else {
                format!("{} fire", fires.len())
            },
            fires.join(", ")
        );
        // Those of the band no longer about at the week's end: how each went, by the inspector.
        let about_now: std::collections::BTreeSet<u64> =
            w.people.iter().filter(|v| !v.dead).map(|v| v.id).collect();
        let gone: Vec<u64> = band
            .iter()
            .copied()
            .filter(|id| !about_now.contains(id))
            .collect();
        let _ = writeln!(log, "\n## Gone from the band\n");
        if gone.is_empty() {
            let _ = writeln!(log, "None: all {} are about at the week's end.", band.len());
        }
        for id in gone {
            w.inspected = None;
            w.server.send(ToServer::Inspect(Some(id)));
            w.run(40);
            w.until(5.0, |w| w.inspected.as_ref().is_some_and(|r| r.id == id));
            let Some(r) = w.inspected.clone() else {
                let _ = writeln!(log, "- #{id}: no record");
                continue;
            };
            let life = r
                .sections
                .iter()
                .find(|s| s.name == "Life")
                .and_then(|s| s.lines.first().cloned())
                .unwrap_or_default();
            let _ = writeln!(log, "- {}: {life}", r.title);
        }
        // The inspector's record of two of them: a grown woman and a grown man of the band.
        let mut grown: Vec<(bool, u64)> = Vec::new();
        for female in [true, false] {
            if let Some((id, _)) = seen
                .iter()
                .filter(|(id, (f, stage, _))| band.contains(id) && *f == female && stage == "Adult")
                .min_by_key(|(id, _)| **id)
            {
                grown.push((female, *id));
            }
        }
        for (female, id) in grown {
            w.inspected = None;
            w.server.send(ToServer::Inspect(Some(id)));
            w.run(40);
            w.until(5.0, |w| w.inspected.as_ref().is_some_and(|r| r.id == id));
            let Some(r) = w.inspected.clone() else {
                continue;
            };
            let _ = writeln!(
                log,
                "\n## The inspector on {} \n\n**{}**\n",
                if female { "a woman" } else { "a man" },
                r.title
            );
            for s in r.sections.iter().filter(|s| {
                [
                    "Life",
                    "Body",
                    "Memory",
                    "Mind",
                    "Knowledge",
                    "Social",
                    "Culture",
                ]
                .contains(&s.name.as_str())
            }) {
                let _ = writeln!(log, "- *{}*: {}", s.name, s.lines.join("; "));
            }
        }
        w.server.send(ToServer::Inspect(None));
        let _ = writeln!(
            log,
            "\nThe player {}.",
            died.map_or("lived the week".to_owned(), |d| format!("died {d}"))
        );
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

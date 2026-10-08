//! The worlds on disk and what the Worlds screen does with them (Amendment P §4.2): each world
//! told by its name, era, play time, when it was last played and who its player is; renamed,
//! duplicated, backed up as a dated copy, and deleted into a trash it can be restored from for
//! thirty days.
//!
//! ```text
//! saves/<world>/            a world (level.json and the rest)
//! saves/trash/<world>.<t>/  a world deleted at unix time t, kept thirty days
//! saves/backups/<world>-<date>/  a dated copy
//! ```

use std::path::{Path, PathBuf};

use serde_json::Value;

/// How long a deleted world is kept (seconds).
pub const TRASH_KEPT_S: u64 = 30 * 86_400;
/// The folders of the saves that are not worlds.
pub const TRASH: &str = "trash";
pub const BACKUPS: &str = "backups";

/// A world on disk, as the Worlds screen tells it.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldInfo {
    /// Its folder in the saves (the name the server opens).
    pub folder: String,
    pub name: String,
    /// Its era's id.
    pub era: String,
    pub created_unix: u64,
    pub last_played_unix: u64,
    /// Seconds of play (the world's ticks at 20 a second).
    pub played_s: u64,
    /// The player's person: their name and age (years), when there is one.
    pub character: Option<(String, f64)>,
    /// Ended with its character's death (permadeath).
    pub ended: bool,
    /// Its game mode's id (none: a world from before the modes), and whether it was ever played
    /// in Creative (Amendment P §2).
    pub mode: Option<String>,
    pub played_in_creative: bool,
}

/// A world in the trash.
#[derive(Debug, Clone, PartialEq)]
pub struct Trashed {
    /// Its folder in the trash.
    pub entry: String,
    /// The folder it had.
    pub folder: String,
    pub name: String,
    pub deleted_unix: u64,
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// What a world's folder says of it, if it holds a world.
pub fn info(dir: &Path) -> Option<WorldInfo> {
    let folder = dir.file_name()?.to_str()?.to_owned();
    let level = read_json(&dir.join("level.json"))?;
    let num = |v: &Value, k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0);
    let ticks = level
        .pointer("/clock/ticks")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let life = level.pointer("/settings/life");
    let life_num = |k: &str, d: f64| {
        life.and_then(|l| l.get(k))
            .and_then(Value::as_f64)
            .unwrap_or(d)
    };
    // The player: their name, and their age now — their age when their life began and the
    // days lived since, by the world's calendar.
    let character = read_json(&dir.join("player.json")).and_then(|p| {
        let name = p.pointer("/appearance/name")?.as_str()?.trim().to_owned();
        let born_tick = p
            .pointer("/player/life/born_tick")
            .and_then(Value::as_u64)?;
        let start_age = p
            .pointer("/household/age")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let ticks_per_day = life_num("day_length_min", 48.0) * 60.0 * 20.0;
        let days_per_year = life_num("days_per_season", 8.0) * 4.0;
        let lived = ticks.saturating_sub(born_tick) as f64 / ticks_per_day.max(1.0);
        Some((name, start_age + lived / days_per_year.max(1.0)))
    });
    Some(WorldInfo {
        folder,
        name: level
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        era: level
            .pointer("/settings/era")
            .and_then(Value::as_str)
            .unwrap_or(crate::eras::WILD_EARTH)
            .to_owned(),
        created_unix: num(&level, "created_unix"),
        last_played_unix: num(&level, "last_played_unix"),
        played_s: ticks / 20,
        character,
        ended: level.get("ended").and_then(Value::as_bool).unwrap_or(false),
        mode: level
            .pointer("/settings/mode")
            .and_then(Value::as_str)
            .map(str::to_owned),
        played_in_creative: level
            .pointer("/settings/played_in_creative")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// Changes a world's game mode (its Edit, Amendment P §2): only toward a less strict one; its
/// rules set by the new mode, and a world once in Creative marked so for good. Whether it was
/// changed.
pub fn set_mode(
    saves: &Path,
    folder: &str,
    from: Option<&crate::modes::Rules>,
    to: &crate::modes::Rules,
) -> std::io::Result<bool> {
    if from.is_some_and(|f| !crate::modes::may_change(f, to)) {
        return Ok(false);
    }
    let path = saves.join(folder).join("level.json");
    let mut level = read_json(&path).ok_or_else(|| std::io::Error::other("no level.json"))?;
    let settings = level
        .get_mut("settings")
        .ok_or_else(|| std::io::Error::other("no settings"))?;
    let mut life: hearth_save::LifeSettings = settings
        .get("life")
        .cloned()
        .and_then(|l| serde_json::from_value(l).ok())
        .ok_or_else(|| std::io::Error::other("unreadable life settings"))?;
    crate::modes::apply(to, &mut life);
    settings["life"] = serde_json::to_value(&life).map_err(std::io::Error::other)?;
    settings["mode"] = Value::String(to.id.clone());
    if to.creative {
        settings["played_in_creative"] = Value::Bool(true);
    }
    let text = serde_json::to_string_pretty(&level).map_err(std::io::Error::other)?;
    std::fs::write(&path, text)?;
    Ok(true)
}

/// The worlds in a saves folder, most recently played first.
pub fn list(saves: &Path) -> Vec<WorldInfo> {
    let mut out: Vec<WorldInfo> = std::fs::read_dir(saves)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter_map(|e| info(&e.path()))
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|w| std::cmp::Reverse(w.last_played_unix));
    out
}

/// A folder in `dir` like `want`, free: `want`, else `want (2)`, `want (3)`…
fn free_folder(dir: &Path, want: &str) -> String {
    if !dir.join(want).exists() && want != TRASH && want != BACKUPS {
        return want.to_owned();
    }
    (2..)
        .map(|n| format!("{want} ({n})"))
        .find(|f| !dir.join(f).exists())
        .expect("a free folder")
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let target = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &target)?;
        } else {
            std::fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}

fn set_name(dir: &Path, name: &str) -> std::io::Result<()> {
    let path = dir.join("level.json");
    let mut level = read_json(&path)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "no level.json"))?;
    level["name"] = Value::String(name.to_owned());
    std::fs::write(&path, serde_json::to_vec_pretty(&level)?)
}

/// Gives a world another name (its folder stays).
pub fn rename(saves: &Path, folder: &str, name: &str) -> std::io::Result<()> {
    set_name(&saves.join(folder), name.trim())
}

/// A copy of a world beside it, named as a copy; returns its folder.
pub fn duplicate(saves: &Path, folder: &str) -> std::io::Result<String> {
    let from = saves.join(folder);
    let name = info(&from).map_or_else(|| folder.to_owned(), |w| w.name);
    let to = free_folder(saves, &format!("{folder} (copy)"));
    copy_dir(&from, &saves.join(&to))?;
    set_name(&saves.join(&to), &format!("{name} (copy)"))?;
    Ok(to)
}

/// A dated copy of a world in the backups; returns where it went.
pub fn back_up(saves: &Path, folder: &str, now_unix: u64) -> std::io::Result<PathBuf> {
    let dir = saves.join(BACKUPS);
    std::fs::create_dir_all(&dir)?;
    let (y, m, d, hh, mm) = civil(now_unix);
    let to = dir.join(free_folder(
        &dir,
        &format!("{folder}-{y:04}-{m:02}-{d:02}-{hh:02}{mm:02}"),
    ));
    copy_dir(&saves.join(folder), &to)?;
    Ok(to)
}

/// Moves a world to the trash.
pub fn delete(saves: &Path, folder: &str, now_unix: u64) -> std::io::Result<()> {
    let dir = saves.join(TRASH);
    std::fs::create_dir_all(&dir)?;
    let to = dir.join(free_folder(&dir, &format!("{folder}.{now_unix}")));
    std::fs::rename(saves.join(folder), to)
}

/// The worlds in the trash, the latest deleted first.
pub fn trashed(saves: &Path) -> Vec<Trashed> {
    let mut out: Vec<Trashed> = std::fs::read_dir(saves.join(TRASH))
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| {
                    let entry = e.file_name().to_str()?.to_owned();
                    let (folder, t) = entry.rsplit_once('.')?;
                    let deleted_unix = t.split(' ').next()?.parse().ok()?;
                    let name = info(&e.path()).map_or_else(|| folder.to_owned(), |w| w.name);
                    Some(Trashed {
                        folder: folder.to_owned(),
                        entry,
                        name,
                        deleted_unix,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|t| std::cmp::Reverse(t.deleted_unix));
    out
}

/// Brings a world back from the trash (beside a world that took its folder since, if one did);
/// returns its folder.
pub fn restore(saves: &Path, entry: &str) -> std::io::Result<String> {
    let t = trashed(saves)
        .into_iter()
        .find(|t| t.entry == entry)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "not in the trash"))?;
    let to = free_folder(saves, &t.folder);
    std::fs::rename(saves.join(TRASH).join(entry), saves.join(&to))?;
    Ok(to)
}

/// Removes the worlds deleted more than thirty days before `now_unix` (all of them with
/// `everything`); returns how many went.
pub fn empty_trash(saves: &Path, now_unix: u64, everything: bool) -> usize {
    let mut n = 0;
    for t in trashed(saves) {
        if (everything || now_unix.saturating_sub(t.deleted_unix) > TRASH_KEPT_S)
            && std::fs::remove_dir_all(saves.join(TRASH).join(&t.entry)).is_ok()
        {
            n += 1;
        }
    }
    n
}

/// Opens a folder in the system's file browser.
pub fn open_folder(path: &Path) -> std::io::Result<()> {
    let program = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(path)
        .spawn()
        .map(|_| ())
}

/// A unix time as a calendar date and time (UTC): year, month, day, hour, minute.
pub fn civil(unix: u64) -> (i64, u32, u32, u32, u32) {
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;
    // Days to a proleptic Gregorian date (the era-based method).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d, (secs / 3600) as u32, (secs / 60 % 60) as u32)
}

/// How long a world was played, in words ("3 h 20 min", "12 min").
pub fn played_words(s: u64) -> String {
    let (h, m) = (s / 3600, s / 60 % 60);
    if h > 0 {
        format!("{h} h {m} min")
    } else {
        format!("{m} min")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saves(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("hearth-worlds-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn world(saves: &Path, folder: &str, name: &str, played: u64) {
        let dir = saves.join(folder);
        std::fs::create_dir_all(dir.join("region")).unwrap();
        let level = serde_json::json!({
            "name": name, "created_unix": 100, "last_played_unix": played,
            "clock": {"ticks": 20 * 3600 * 2},
            "settings": {"era": "hearth:upper_paleolithic",
                         "life": {"day_length_min": 48.0, "days_per_season": 8}},
        });
        std::fs::write(dir.join("level.json"), serde_json::to_vec(&level).unwrap()).unwrap();
        std::fs::write(dir.join("region").join("r.0.0.0.hrg"), b"cubes").unwrap();
        // Born as a baby at the world's start: 2 h of play is 2.5 days of 48 minutes.
        let player = serde_json::json!({
            "appearance": {"name": "Ama"}, "player": {"life": {"born_tick": 0}},
            "household": {"age": 0.0},
        });
        std::fs::write(
            dir.join("player.json"),
            serde_json::to_vec(&player).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn a_world_s_mode_changes_only_toward_less_strict_and_creative_marks_it() {
        let content = hearth_content::Content::load_base();
        let r = |id: &str| crate::modes::rules(crate::modes::find(&content, id).unwrap());
        let s = saves("mode");
        let dir = s.join("w");
        std::fs::create_dir_all(&dir).unwrap();
        let level = serde_json::json!({
            "name": "W", "settings": {"era": "hearth:wild_earth", "mode": "hearth:realistic",
            "life": serde_json::to_value(hearth_save::LifeSettings::default()).unwrap()},
        });
        std::fs::write(dir.join("level.json"), serde_json::to_vec(&level).unwrap()).unwrap();
        let mode = |s: &Path| info(&s.join("w")).unwrap();
        // Realistic to Easy: Easy's rules, not yet played in Creative.
        assert!(set_mode(&s, "w", Some(&r("realistic")), &r("easy")).unwrap());
        let w = mode(&s);
        assert_eq!(w.mode.as_deref(), Some("hearth:easy"));
        assert!(!w.played_in_creative);
        let life: hearth_save::LifeSettings = serde_json::from_value(
            read_json(&s.join("w").join("level.json")).unwrap()["settings"]["life"].clone(),
        )
        .unwrap();
        assert_eq!(life.realism.preset, "hardy");
        // Never back to stricter.
        assert!(!set_mode(&s, "w", Some(&r("easy")), &r("realistic")).unwrap());
        assert_eq!(mode(&s).mode.as_deref(), Some("hearth:easy"));
        // To Creative: marked so for good.
        assert!(set_mode(&s, "w", Some(&r("easy")), &r("creative")).unwrap());
        assert!(mode(&s).played_in_creative);
        let _ = std::fs::remove_dir_all(&s);
    }

    #[test]
    fn worlds_are_told_by_name_era_play_and_player() {
        let s = saves("list");
        world(&s, "a", "Older", 10);
        world(&s, "b", "Newer", 20);
        std::fs::create_dir_all(s.join(TRASH)).unwrap();
        let l = list(&s);
        assert_eq!(
            l.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(),
            ["Newer", "Older"]
        );
        let w = &l[0];
        assert_eq!(w.era, "hearth:upper_paleolithic");
        assert_eq!(w.played_s, 7200);
        assert_eq!(played_words(w.played_s), "2 h 0 min");
        let (name, age) = w.character.clone().unwrap();
        assert_eq!(name, "Ama");
        assert!((age - 2.5 / 32.0).abs() < 1e-9, "{age}");
        let _ = std::fs::remove_dir_all(&s);
    }

    /// Deleted to the trash and restored whole; renamed, duplicated and backed up.
    #[test]
    fn a_world_deleted_is_kept_thirty_days_and_comes_back() {
        let s = saves("trash");
        world(&s, "valley", "The Valley", 10);
        delete(&s, "valley", 1_000).unwrap();
        assert!(list(&s).is_empty(), "gone from the list");
        let t = trashed(&s);
        assert_eq!(t.len(), 1);
        assert_eq!(
            (t[0].folder.as_str(), t[0].name.as_str()),
            ("valley", "The Valley")
        );
        // A month on, it is still there; past thirty days the trash lets it go.
        assert_eq!(empty_trash(&s, 1_000 + TRASH_KEPT_S - 10, false), 0);
        assert_eq!(restore(&s, &t[0].entry).unwrap(), "valley");
        assert_eq!(list(&s)[0].name, "The Valley");
        assert_eq!(
            std::fs::read(s.join("valley/region/r.0.0.0.hrg")).unwrap(),
            b"cubes",
            "whole"
        );
        rename(&s, "valley", " Green Valley ").unwrap();
        assert_eq!(list(&s)[0].name, "Green Valley");
        let copy = duplicate(&s, "valley").unwrap();
        assert_eq!(copy, "valley (copy)");
        assert_eq!(info(&s.join(&copy)).unwrap().name, "Green Valley (copy)");
        let b = back_up(&s, "valley", 1_760_000_000).unwrap();
        assert!(b.ends_with("backups/valley-2025-10-09-0853"), "{b:?}");
        assert!(b.join("level.json").exists());
        assert_eq!(list(&s).len(), 2, "backups and trash are not worlds");
        delete(&s, "valley", 2_000).unwrap();
        assert_eq!(empty_trash(&s, 2_000 + TRASH_KEPT_S + 1, false), 1);
        assert!(trashed(&s).is_empty());
        let _ = std::fs::remove_dir_all(&s);
    }

    #[test]
    fn dates_are_civil() {
        assert_eq!(civil(0), (1970, 1, 1, 0, 0));
        assert_eq!(civil(951_782_400), (2000, 2, 29, 0, 0));
        assert_eq!(civil(1_760_000_000), (2025, 10, 9, 8, 53));
    }
}

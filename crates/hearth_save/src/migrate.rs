//! Save migrations. Each step upgrades the raw `level.json` value by one format version, so a
//! world from any supported older version is walked forward step by step to [`FORMAT`].

use serde_json::{Map, Value, json};

use crate::SaveError;
use crate::meta::{FORMAT, OLDEST_SUPPORTED};

/// What happened while opening a world.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationReport {
    pub from: u32,
    pub to: u32,
    pub steps: Vec<String>,
}

impl MigrationReport {
    pub fn migrated(&self) -> bool {
        self.from != self.to
    }
}

/// Upgrades `v` in place to the current format.
pub fn migrate(v: &mut Value) -> Result<MigrationReport, SaveError> {
    let format = v
        .get("format")
        .and_then(Value::as_u64)
        .ok_or_else(|| SaveError::Corrupt("level.json has no format version".into()))?
        as u32;
    if format < OLDEST_SUPPORTED {
        return Err(SaveError::Incompatible(format!(
            "This world was created by an earlier, incompatible version of {} (save format {format}). \
             Worlds from before the \"Earth, Transposed\" update can't be opened; create a new world.",
            hearth_core::GAME_NAME
        )));
    }
    if format > FORMAT {
        return Err(SaveError::Incompatible(format!(
            "This world was saved by a newer version of {} (save format {format}; this version reads up to {FORMAT}).",
            hearth_core::GAME_NAME
        )));
    }
    let mut report = MigrationReport {
        from: format,
        to: format,
        steps: Vec::new(),
    };
    while report.to < FORMAT {
        let obj = v
            .as_object_mut()
            .ok_or_else(|| SaveError::Corrupt("level.json is not an object".into()))?;
        let step = match report.to {
            2 => v2_to_v3(obj)?,
            3 => v3_to_v4(obj),
            other => {
                return Err(SaveError::Corrupt(format!(
                    "no migration from format {other}"
                )));
            }
        };
        report.to += 1;
        obj.insert("format".into(), json!(report.to));
        report.steps.push(step);
    }
    Ok(report)
}

fn take(obj: &mut Map<String, Value>, key: &str) -> Result<Value, SaveError> {
    obj.remove(key)
        .ok_or_else(|| SaveError::Corrupt(format!("level.json (format 2) lacks `{key}`")))
}

/// Format 2 → 3: world-generation fields move under `settings.planet` (format 2 kept the seed
/// at the top level next to `planet`), life & time settings and the era are added with their
/// defaults, and `time` becomes `clock`.
fn v2_to_v3(obj: &mut Map<String, Value>) -> Result<String, SaveError> {
    let seed = take(obj, "seed")?;
    let planet = take(obj, "planet")?;
    let mut planet = match planet {
        Value::Object(m) => m,
        _ => return Err(SaveError::Corrupt("`planet` is not an object".into())),
    };
    planet.insert("seed".into(), seed);
    let life = serde_json::to_value(crate::settings::LifeSettings::default())
        .map_err(|e| SaveError::Corrupt(e.to_string()))?;
    obj.insert(
        "settings".into(),
        json!({ "planet": Value::Object(planet), "life": life, "era": "hearth:wild_earth" }),
    );
    let time = take(obj, "time")?;
    obj.insert("clock".into(), time);
    Ok("2→3: grouped world settings, added life & time settings (defaults) and the era, renamed time → clock".into())
}

/// Format 3 → 4: v2's death rules (D167) become the settings that replaced them — Legacy a head
/// start on what earlier lives knew, Hardy all of it kept, Permadeath no one to live on as and
/// no being born again.
fn v3_to_v4(obj: &mut Map<String, Value>) -> String {
    let life = obj
        .get_mut("settings")
        .and_then(|s| s.get_mut("life"))
        .and_then(Value::as_object_mut);
    let Some(life) = life else {
        return "3→4: no life settings to carry over".into();
    };
    let rule = life
        .remove("death_rules")
        .and_then(|r| r.as_str().map(str::to_owned))
        .unwrap_or_else(|| "legacy".into());
    let (after, born_again) = match rule.as_str() {
        "hardy" => ("keep_everything", true),
        "permadeath" => ("theirs_only", false),
        _ => ("head_start", true),
    };
    if rule == "permadeath" {
        life.insert("inhabit".into(), json!("none"));
    }
    life.insert("after_death".into(), json!(after));
    life.insert("born_again".into(), json!(born_again));
    format!(
        "3→4: the death rule {rule} became knowledge after death {after}, born again {born_again}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v2() -> Value {
        json!({
            "format": 2,
            "name": "Old world",
            "game_version": "0.1.0",
            "created_unix": 1,
            "last_played_unix": 2,
            "seed": 42,
            "planet": {
                "planet_size": "Standard",
                "vertical_scale_factor": 1.0,
                "rarity": "rare",
                "land_fraction": 0.3,
                "spawn_climate": "temperate",
                "grid_resolution": 2048
            },
            "time": { "ticks": 1234 },
            "block_states": ["hearth:air", "hearth:stone"]
        })
    }

    #[test]
    fn format_2_migrates_to_current() {
        let mut v = v2();
        let r = migrate(&mut v).unwrap();
        assert_eq!((r.from, r.to), (2, FORMAT));
        assert!(r.migrated());
        let meta: crate::meta::WorldMeta = serde_json::from_value(v).unwrap();
        assert_eq!(meta.settings.planet.seed, 42);
        assert_eq!(meta.clock.ticks, 1234);
        assert_eq!(meta.settings.era, "hearth:wild_earth");
        assert_eq!(meta.settings.life.day_length_min, 48);
    }

    #[test]
    fn the_old_death_rules_become_their_presets() {
        for (rule, preset) in [
            ("legacy", crate::DeathPreset::Legacy),
            ("hardy", crate::DeathPreset::Hardy),
            ("permadeath", crate::DeathPreset::Permadeath),
        ] {
            let mut v = v2();
            migrate(&mut v).unwrap();
            v["format"] = json!(3);
            let life = v["settings"]["life"].as_object_mut().unwrap();
            life.remove("after_death");
            life.remove("born_again");
            life.insert("death_rules".into(), json!(rule));
            let r = migrate(&mut v).unwrap();
            assert_eq!((r.from, r.to), (3, FORMAT));
            let meta: crate::meta::WorldMeta = serde_json::from_value(v).unwrap();
            let l = &meta.settings.life;
            assert_eq!(
                crate::DeathPreset::of(l.inhabit, l.after_death, l.born_again),
                Some(preset),
                "{rule}"
            );
        }
    }

    #[test]
    fn old_and_future_formats_are_refused_clearly() {
        let mut v1 = json!({ "format": 1 });
        match migrate(&mut v1) {
            Err(SaveError::Incompatible(msg)) => {
                assert!(msg.contains("earlier, incompatible"), "{msg}")
            }
            other => panic!("expected refusal, got {other:?}"),
        }
        let mut future = json!({ "format": FORMAT + 1 });
        match migrate(&mut future) {
            Err(SaveError::Incompatible(msg)) => assert!(msg.contains("newer version"), "{msg}"),
            other => panic!("expected refusal, got {other:?}"),
        }
    }
}

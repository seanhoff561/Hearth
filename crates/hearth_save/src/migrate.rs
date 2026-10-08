//! Save migrations. Each step upgrades the raw `level.json` value by one format version, so a
//! world from any supported older version is walked forward step by step to [`FORMAT`]. Worlds
//! before format 6 had people in them and are refused.

use serde_json::{Value, json};

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
            "This world was made with an earlier version that had people in it (save format \
             {format}); start a new world."
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
    if report.to == 6 {
        // Earth's clock (E3): the compressed day's and season's lengths go; the world's clock is
        // set to a spring morning, its ticks counted in real seconds from there.
        if let Some(life) = v
            .pointer_mut("/settings/life")
            .and_then(Value::as_object_mut)
        {
            for k in [
                "day_length_min",
                "days_per_season",
                "starting_season",
                "axial_tilt_deg",
            ] {
                life.remove(k);
            }
            life.insert("start".into(), json!("spring_morning"));
        }
        report.to = 7;
        report
            .steps
            .push("6 → 7: the clock is Earth's; the world wakes to a spring morning".into());
    }
    v["format"] = json!(report.to);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_and_future_formats_are_refused_clearly() {
        for old in 1..OLDEST_SUPPORTED {
            let mut v = json!({ "format": old });
            match migrate(&mut v) {
                Err(SaveError::Incompatible(msg)) => {
                    assert!(msg.contains("had people in it"), "{msg}")
                }
                other => panic!("expected refusal, got {other:?}"),
            }
        }
        let mut future = json!({ "format": FORMAT + 1 });
        match migrate(&mut future) {
            Err(SaveError::Incompatible(msg)) => assert!(msg.contains("newer version"), "{msg}"),
            other => panic!("expected refusal, got {other:?}"),
        }
        let mut now = json!({ "format": FORMAT });
        assert!(!migrate(&mut now).unwrap().migrated());
    }

    #[test]
    fn a_world_on_the_old_clock_wakes_to_a_spring_morning() {
        let mut v = json!({
            "format": 6,
            "settings": {"life": {"day_length_min": 48, "days_per_season": 8,
                                  "starting_season": "Spring", "axial_tilt_deg": 23.44,
                                  "realism": {"preset": "authentic"}}},
        });
        let r = migrate(&mut v).unwrap();
        assert_eq!((r.from, r.to), (6, 7));
        let life = &v["settings"]["life"];
        assert_eq!(life["start"], "spring_morning");
        assert!(life.get("day_length_min").is_none() && life.get("axial_tilt_deg").is_none());
        assert_eq!(life["realism"]["preset"], "authentic", "the rest kept");
        let back: crate::LifeSettings = serde_json::from_value(life.clone()).unwrap();
        assert_eq!(back.start, crate::Start::SpringMorning);
    }
}

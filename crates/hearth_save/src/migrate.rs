//! Save migrations. Each step upgrades the raw `level.json` value by one format version, so a
//! world from any supported older version is walked forward step by step to [`FORMAT`]. Format 6
//! (Amendment E) has no steps yet: every older world had people in it and is refused.

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
    let report = MigrationReport {
        from: format,
        to: format,
        steps: Vec::new(),
    };
    v["format"] = json!(report.to);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_and_future_formats_are_refused_clearly() {
        for old in 1..FORMAT {
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
}

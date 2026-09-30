//! The balance layer (v2 §3.4): every tuning multiplier goes through here. Presets are data
//! (`balance/presets`); Custom is a preset plus per-key overrides from the world settings.

use std::collections::BTreeMap;

use rustc_hash::FxHashMap;

use crate::content::Content;

/// Resolved multipliers for one world.
#[derive(Debug, Clone, PartialEq)]
pub struct Balance {
    values: FxHashMap<String, f32>,
}

impl Balance {
    /// Values of `preset` (a balance preset id), then `overrides` on top, clamped to each key's
    /// range. Unknown preset → key defaults.
    pub fn resolve(content: &Content, preset: &str, overrides: &BTreeMap<String, f32>) -> Balance {
        let preset = content.balance_presets.get(preset);
        let mut values = FxHashMap::default();
        for key in content.balance_keys.iter() {
            let short = key.id.split_once(':').map_or(key.id.as_str(), |(_, p)| p);
            let lookup =
                |m: &BTreeMap<String, f32>| m.get(short).or_else(|| m.get(&key.id)).copied();
            let v = overrides
                .get(short)
                .or_else(|| overrides.get(&key.id))
                .copied()
                .or_else(|| preset.and_then(|p| lookup(&p.values)))
                .unwrap_or(key.default);
            values.insert(short.to_owned(), v.clamp(key.min, key.max));
        }
        Balance { values }
    }

    /// The multiplier for `key` (its short name, e.g. `hunger_rate`).
    pub fn get(&self, key: &str) -> f32 {
        match self.values.get(key) {
            Some(v) => *v,
            None => {
                debug_assert!(false, "unknown balance key `{key}`");
                1.0
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, f32)> {
        self.values.iter().map(|(k, v)| (k.as_str(), *v))
    }
}

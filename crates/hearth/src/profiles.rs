//! The player's people (v2 §9.1): appearances kept as profiles in `characters.json`, one of
//! them chosen for new worlds.

use std::path::Path;

use hearth_character::Appearance;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profiles {
    pub list: Vec<Appearance>,
    pub selected: usize,
}

impl Default for Profiles {
    fn default() -> Self {
        Self {
            list: vec![Appearance::default()],
            selected: 0,
        }
    }
}

impl Profiles {
    /// The profiles saved at `path`, or a first one.
    pub fn load(path: &Path) -> Self {
        let mut p: Profiles = match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                log::warn!("{} unreadable ({e}): starting a new one", path.display());
                Profiles::default()
            }),
            Err(_) => Profiles::default(),
        };
        p.list = p.list.into_iter().map(Appearance::sanitized).collect();
        if p.list.is_empty() {
            p.list.push(Appearance::default());
        }
        p.selected = p.selected.min(p.list.len() - 1);
        p
    }

    /// Writes them (through a temporary file, so a crash leaves the old ones).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }

    pub fn current(&self) -> &Appearance {
        &self.list[self.selected.min(self.list.len() - 1)]
    }

    pub fn current_mut(&mut self) -> &mut Appearance {
        let i = self.selected.min(self.list.len() - 1);
        &mut self.list[i]
    }

    /// A profile's name, or "Person N" (`unnamed` is the words with `{n}`).
    pub fn name(&self, i: usize, unnamed: &str) -> String {
        match self.list.get(i) {
            Some(a) if !a.name.trim().is_empty() => a.name.clone(),
            _ => unnamed.replace("{n}", &(i + 1).to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_save_and_load() {
        let dir = std::env::temp_dir().join(format!("hearth-profiles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("characters.json");
        let mut p = Profiles::default();
        p.list.push(Appearance {
            name: "Ash".into(),
            ..Appearance::female()
        });
        p.selected = 1;
        p.save(&path).expect("save");
        let back = Profiles::load(&path);
        assert_eq!(back, p);
        assert_eq!(back.name(0, "Person {n}"), "Person 1");
        assert_eq!(back.name(1, "Person {n}"), "Ash");
        // Nothing there, or rubbish: one default person.
        std::fs::write(&path, "not json").expect("write");
        assert_eq!(Profiles::load(&path).list.len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}

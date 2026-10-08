//! The player's people (Amendment E §6.2): appearances made in the character creator, kept as
//! profiles in `characters.json`, one of them chosen for the next life. (The wishes kept in
//! `birth.json` before the creator came back give a first profile their name, body and
//! loincloth once.)

use std::path::Path;

use hearth_character::{Appearance, BodyType, Loincloth};
use serde::{Deserialize, Serialize};

/// Where the profiles are kept, in the game's folder.
pub const FILE: &str = "characters.json";

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

/// The wishes of before: a name, a body (or chance), a loincloth.
#[derive(Default, Deserialize)]
#[serde(default)]
struct Wishes {
    name: String,
    born: String,
    loincloth: Loincloth,
}

impl Profiles {
    /// The profiles saved at `path`, or a first one.
    pub fn load(path: &Path) -> Self {
        let p: Profiles = match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                log::warn!("{} unreadable ({e}): starting a new one", path.display());
                Profiles::default()
            }),
            Err(_) => Profiles::default(),
        };
        p.sanitized()
    }

    /// The profiles in the game's folder; where there are none yet, one from the wishes of
    /// before.
    pub fn load_or_migrate(dir: &Path) -> Self {
        let path = dir.join(FILE);
        if path.exists() {
            return Self::load(&path);
        }
        let Some(w) = std::fs::read_to_string(dir.join("birth.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Wishes>(&t).ok())
        else {
            return Profiles::default();
        };
        let mut a = if w.born == "Daughter" {
            Appearance::female()
        } else {
            Appearance::default()
        };
        a.name = w.name;
        a.loincloth = w.loincloth;
        Profiles {
            list: vec![a],
            selected: 0,
        }
        .sanitized()
    }

    fn sanitized(mut self) -> Self {
        self.list = self.list.into_iter().map(Appearance::sanitized).collect();
        if self.list.is_empty() {
            self.list.push(Appearance::default());
        }
        self.selected = self.selected.min(self.list.len() - 1);
        self
    }

    /// Writes them (through a temporary file, so a crash leaves the old ones).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }

    /// The one chosen for the next life.
    pub fn current(&self) -> &Appearance {
        &self.list[self.selected.min(self.list.len() - 1)]
    }

    pub fn current_mut(&mut self) -> &mut Appearance {
        let i = self.selected.min(self.list.len() - 1);
        &mut self.list[i]
    }

    /// Adds a new person (of the other body to the last), chosen.
    pub fn add(&mut self) {
        let female = self.list.last().is_some_and(|a| a.body == BodyType::Male);
        self.list.push(if female {
            Appearance::female()
        } else {
            Appearance::default()
        });
        self.selected = self.list.len() - 1;
    }

    /// Removes the chosen person, keeping at least one.
    pub fn remove(&mut self) {
        if self.list.len() > 1 {
            self.list.remove(self.selected);
            self.selected = self.selected.min(self.list.len() - 1);
        }
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
    fn profiles_save_load_and_come_from_the_wishes_of_before() {
        let dir = std::env::temp_dir().join(format!("hearth-profiles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        // The wishes of before: a first profile with their name, body and loincloth.
        let old = r#"{"name":"Ash","born":"Daughter","loincloth":"PlantFibre"}"#;
        std::fs::write(dir.join("birth.json"), old).expect("write");
        let p = Profiles::load_or_migrate(&dir);
        assert_eq!(p.list.len(), 1);
        assert_eq!(p.current().name, "Ash");
        assert_eq!(p.current().body, BodyType::Female);
        assert_eq!(p.current().loincloth, Loincloth::PlantFibre);
        // Made in the creator and saved, they are what is loaded.
        let mut changed = p.clone();
        changed.add();
        *changed.current_mut() = changed.current().randomized(9);
        changed.save(&dir.join(FILE)).expect("save");
        let back = Profiles::load_or_migrate(&dir);
        assert_eq!(back, changed);
        assert_eq!(back.selected, 1);
        assert_eq!(back.name(1, "Person {n}"), "Person 2");
        assert_eq!(back.name(0, "Person {n}"), "Ash");
        // Removing keeps one at least.
        let mut one = back.clone();
        one.remove();
        one.remove();
        assert_eq!(one.list.len(), 1);
        // Rubbish: one default person.
        std::fs::write(dir.join(FILE), "not json").expect("write");
        assert_eq!(Profiles::load_or_migrate(&dir), Profiles::default());
        std::fs::remove_dir_all(&dir).ok();
    }
}

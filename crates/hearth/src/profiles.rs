//! What the player wishes of a birth (V2.1 Addendum A): a name, whether to be born a daughter or
//! a son or leave it to chance, and the loincloth they first wear — kept in `birth.json` for the
//! next world. Nothing of their looks: those come from their parents' genes. (The character
//! profiles of before, `characters.json`, give their chosen one's name, sex and loincloth once.)

use std::path::Path;

use hearth_character::{Appearance, BodyType, Loincloth};
use serde::{Deserialize, Serialize};

/// Where the wishes are kept, in the game's folder.
pub const FILE: &str = "birth.json";

/// Born a daughter, a son, or as chance has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Born {
    #[default]
    Chance,
    Daughter,
    Son,
}

impl Born {
    pub const ALL: [Born; 3] = [Born::Chance, Born::Daughter, Born::Son];

    /// The words for it.
    pub fn key(self) -> &'static str {
        match self {
            Born::Chance => "birth.chance",
            Born::Daughter => "birth.daughter",
            Born::Son => "birth.son",
        }
    }

    /// Female, male, or for the father's gamete to decide.
    pub fn female(self) -> Option<bool> {
        match self {
            Born::Chance => None,
            Born::Daughter => Some(true),
            Born::Son => Some(false),
        }
    }
}

/// The player's wishes for their next birth.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Profiles {
    pub name: String,
    pub born: Born,
    pub loincloth: Loincloth,
}

/// The character profiles of before: people the player made, one chosen.
#[derive(Default, Deserialize)]
#[serde(default)]
struct Characters {
    list: Vec<Appearance>,
    selected: usize,
}

impl Profiles {
    /// The wishes saved at `path`, or none yet.
    pub fn load(path: &Path) -> Self {
        let mut p: Profiles = match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                log::warn!("{} unreadable ({e}): starting anew", path.display());
                Profiles::default()
            }),
            Err(_) => Profiles::default(),
        };
        p.name = p.name.chars().take(32).collect();
        p
    }

    /// The wishes in the game's folder; where there are none yet, the name, sex and loincloth of
    /// the character chosen in the profiles of before.
    pub fn load_or_migrate(dir: &Path) -> Self {
        let path = dir.join(FILE);
        if path.exists() {
            return Self::load(&path);
        }
        let old: Characters = std::fs::read_to_string(dir.join("characters.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        old.list
            .get(old.selected.min(old.list.len().saturating_sub(1)))
            .map_or_else(Profiles::default, |a| Profiles {
                name: a.name.chars().take(32).collect(),
                born: match a.body {
                    BodyType::Female => Born::Daughter,
                    BodyType::Male => Born::Son,
                },
                loincloth: a.loincloth,
            })
    }

    /// Writes them (through a temporary file, so a crash leaves the old ones).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }

    /// The wish as the server takes it.
    pub fn wish(&self) -> hearth_protocol::Wish {
        hearth_protocol::Wish {
            name: self.name.trim().to_owned(),
            female: self.born.female(),
            loincloth: self.loincloth,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wishes_save_load_and_come_from_the_characters_of_before() {
        let dir = std::env::temp_dir().join(format!("hearth-birth-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        // The characters of before: the chosen one's name, sex and loincloth.
        let old = r#"{"list":[{"name":"Bo"},{"name":"Ash","body":"Female","loincloth":"PlantFibre"}],"selected":1}"#;
        std::fs::write(dir.join("characters.json"), old).expect("write");
        let p = Profiles::load_or_migrate(&dir);
        assert_eq!(
            p,
            Profiles {
                name: "Ash".into(),
                born: Born::Daughter,
                loincloth: Loincloth::PlantFibre,
            }
        );
        assert_eq!(p.wish().female, Some(true));
        // Saved, they are what is loaded.
        let changed = Profiles {
            born: Born::Chance,
            ..p
        };
        changed.save(&dir.join(FILE)).expect("save");
        assert_eq!(Profiles::load_or_migrate(&dir), changed);
        assert_eq!(changed.wish().female, None);
        // Rubbish: chance, no name.
        std::fs::write(dir.join(FILE), "not json").expect("write");
        assert_eq!(Profiles::load_or_migrate(&dir), Profiles::default());
        std::fs::remove_dir_all(&dir).ok();
    }
}

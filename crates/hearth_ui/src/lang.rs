//! The words of the interface in the player's language: `lang/<code>.json` of every data pack
//! (later packs override earlier ones), with English under every language and the key itself
//! when a word is missing everywhere.

use std::path::Path;

use rustc_hash::FxHashMap;

/// The fallback language.
pub const DEFAULT: &str = "en_us";

#[derive(Debug, Clone, Default)]
pub struct Lang {
    pub code: String,
    words: FxHashMap<String, String>,
}

impl Lang {
    /// Loads `code` from the packs (each a directory holding `lang/`), English first.
    pub fn load(packs: &[impl AsRef<Path>], code: &str) -> Self {
        let mut words = FxHashMap::default();
        let mut codes = vec![DEFAULT];
        if code != DEFAULT {
            codes.push(code);
        }
        for c in codes {
            for pack in packs {
                let path = pack.as_ref().join("lang").join(format!("{c}.json"));
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                match serde_json::from_str::<FxHashMap<String, String>>(&text) {
                    Ok(map) => words.extend(map),
                    Err(e) => log::warn!("{}: {e}", path.display()),
                }
            }
        }
        Self {
            code: code.to_owned(),
            words,
        }
    }

    /// From pairs (for tests and tools).
    pub fn from_pairs(pairs: &[(&str, &str)]) -> Self {
        Self {
            code: DEFAULT.into(),
            words: pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        }
    }

    /// The words for `key`, or the key itself.
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.words.get(key).map_or(key, String::as_str)
    }

    pub fn has(&self, key: &str) -> bool {
        self.words.contains_key(key)
    }

    /// The words for `key` with `{name}` placeholders filled in.
    pub fn format(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut s = self.get(key).to_owned();
        for (name, value) in args {
            s = s.replace(&format!("{{{name}}}"), value);
        }
        s
    }

    pub fn len(&self) -> usize {
        self.words.len()
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_words_show_their_key() {
        let l = Lang::from_pairs(&[
            ("body.hunger.hungry", "hungry"),
            ("hello", "Hello, {name}!"),
        ]);
        assert_eq!(l.get("body.hunger.hungry"), "hungry");
        assert_eq!(l.get("no.such.key"), "no.such.key");
        assert_eq!(l.format("hello", &[("name", "Ada")]), "Hello, Ada!");
    }
}

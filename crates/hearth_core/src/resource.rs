//! Namespaced resource identifiers (`namespace:path`).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::GAME_NAMESPACE;

/// Errors produced when parsing a [`ResourceLocation`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResourceLocationError {
    #[error("resource location is empty")]
    Empty,
    #[error("invalid character {ch:?} in namespace of {input:?}")]
    BadNamespaceChar { input: String, ch: char },
    #[error("invalid character {ch:?} in path of {input:?}")]
    BadPathChar { input: String, ch: char },
    #[error("resource location {0:?} has an empty namespace or path")]
    EmptyPart(String),
}

/// A namespaced identifier such as `hearth:stone` or `mymod:blocks/ruby_ore`.
///
/// Namespaces may contain `[a-z0-9_.-]`, paths additionally `/`. A missing namespace defaults
/// to the base game namespace, mirroring how data files are usually written.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResourceLocation {
    // Stored as one string with the separator index to keep lookups and hashing cheap.
    full: Box<str>,
    split: u16,
}

fn valid_namespace_char(c: char) -> bool {
    matches!(c, 'a'..='z' | '0'..='9' | '_' | '.' | '-')
}

fn valid_path_char(c: char) -> bool {
    valid_namespace_char(c) || c == '/'
}

impl ResourceLocation {
    /// Builds a location from its parts, validating both.
    pub fn new(namespace: &str, path: &str) -> Result<Self, ResourceLocationError> {
        let input = format!("{namespace}:{path}");
        if namespace.is_empty() || path.is_empty() {
            return Err(ResourceLocationError::EmptyPart(input));
        }
        if let Some(ch) = namespace.chars().find(|c| !valid_namespace_char(*c)) {
            return Err(ResourceLocationError::BadNamespaceChar { input, ch });
        }
        if let Some(ch) = path.chars().find(|c| !valid_path_char(*c)) {
            return Err(ResourceLocationError::BadPathChar { input, ch });
        }
        Ok(Self {
            split: namespace.len() as u16,
            full: input.into_boxed_str(),
        })
    }

    /// A location in the base game namespace. Panics on invalid paths, so only use it with
    /// literals.
    pub fn game(path: &str) -> Self {
        Self::new(GAME_NAMESPACE, path).expect("invalid built-in resource path")
    }

    /// Parses `namespace:path`, or `path` alone (base game namespace).
    pub fn parse(s: &str) -> Result<Self, ResourceLocationError> {
        if s.is_empty() {
            return Err(ResourceLocationError::Empty);
        }
        match s.split_once(':') {
            Some((ns, path)) => Self::new(ns, path),
            None => Self::new(GAME_NAMESPACE, s),
        }
    }

    pub fn namespace(&self) -> &str {
        &self.full[..self.split as usize]
    }

    pub fn path(&self) -> &str {
        &self.full[self.split as usize + 1..]
    }

    /// The full `namespace:path` string.
    pub fn as_str(&self) -> &str {
        &self.full
    }

    /// Returns a new location with the same namespace and `prefix` prepended to the path,
    /// e.g. `hearth:stone` → `hearth:block/stone`.
    pub fn with_path_prefix(&self, prefix: &str) -> Self {
        Self::new(self.namespace(), &format!("{prefix}{}", self.path()))
            .expect("prefix produced invalid path")
    }
}

impl fmt::Display for ResourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.full)
    }
}

impl fmt::Debug for ResourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ResourceLocation({})", self.full)
    }
}

impl FromStr for ResourceLocation {
    type Err = ResourceLocationError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Serialize for ResourceLocation {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.full)
    }
}

impl<'de> Deserialize<'de> for ResourceLocation {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_with_and_without_namespace() {
        let a = ResourceLocation::parse("mymod:blocks/ruby").unwrap();
        assert_eq!(a.namespace(), "mymod");
        assert_eq!(a.path(), "blocks/ruby");
        let b = ResourceLocation::parse("stone").unwrap();
        assert_eq!(b.namespace(), GAME_NAMESPACE);
        assert_eq!(b.path(), "stone");
        assert_eq!(b.to_string(), format!("{GAME_NAMESPACE}:stone"));
    }

    #[test]
    fn rejects_invalid() {
        assert!(ResourceLocation::parse("").is_err());
        assert!(ResourceLocation::parse("Mod:stone").is_err());
        assert!(ResourceLocation::parse("mod:St one").is_err());
        assert!(ResourceLocation::parse("mod:").is_err());
        assert!(ResourceLocation::parse(":x").is_err());
        assert!(ResourceLocation::parse("a/b:c").is_err());
    }

    #[test]
    fn prefix() {
        let a = ResourceLocation::game("stone");
        assert_eq!(a.with_path_prefix("block/").path(), "block/stone");
    }
}

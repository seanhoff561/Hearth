//! Content ids. Data files refer to entries by `namespace:path` or just `path`; a bare path means
//! the namespace of the pack the file belongs to, resolved while the file is parsed.

use std::cell::RefCell;
use std::fmt;

use hearth_core::ResourceLocation;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

thread_local! {
    static NAMESPACE: RefCell<String> = RefCell::new(hearth_core::GAME_NAMESPACE.to_owned());
}

/// Runs `f` with bare ids resolving into `namespace`.
pub fn with_namespace<R>(namespace: &str, f: impl FnOnce() -> R) -> R {
    let prev = NAMESPACE.with(|n| std::mem::replace(&mut *n.borrow_mut(), namespace.to_owned()));
    let out = f();
    NAMESPACE.with(|n| *n.borrow_mut() = prev);
    out
}

fn current_namespace() -> String {
    NAMESPACE.with(|n| n.borrow().clone())
}

/// A reference to another content entry (always fully qualified after parsing).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdRef(pub String);

impl IdRef {
    /// Qualifies `s` with the current namespace if it has none.
    pub fn qualify(s: &str) -> IdRef {
        if s.contains(':') {
            IdRef(s.to_owned())
        } else {
            IdRef(format!("{}:{s}", current_namespace()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn location(&self) -> Option<ResourceLocation> {
        ResourceLocation::parse(&self.0).ok()
    }

    /// The part after the namespace.
    pub fn path(&self) -> &str {
        self.0.split_once(':').map_or(&self.0, |(_, p)| p)
    }
}

impl fmt::Display for IdRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for IdRef {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(IdRef::qualify(&s))
    }
}

impl Serialize for IdRef {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_ids_take_the_file_namespace() {
        assert_eq!(IdRef::qualify("flint").0, "hearth:flint");
        with_namespace("mymod", || {
            assert_eq!(IdRef::qualify("flint").0, "mymod:flint");
            assert_eq!(IdRef::qualify("hearth:flint").0, "hearth:flint");
        });
        assert_eq!(IdRef::qualify("flint").path(), "flint");
    }
}

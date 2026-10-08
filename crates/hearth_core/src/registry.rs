//! Registries map namespaced ids to dense numeric ids assigned at load time. Saves store names,
//! not numeric ids, so adding or removing content never corrupts a world.

use rustc_hash::FxHashMap;

use crate::resource::ResourceLocation;

/// Dense numeric id inside one registry.
pub type RawId = u32;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    #[error("{registry}: {id} is already registered")]
    Duplicate {
        registry: &'static str,
        id: ResourceLocation,
    },
    #[error("{registry}: registry is frozen; cannot register {id}")]
    Frozen {
        registry: &'static str,
        id: ResourceLocation,
    },
    #[error("{registry}: {id} is not registered")]
    Unknown {
        registry: &'static str,
        id: ResourceLocation,
    },
    #[error("{registry}: registry is full")]
    Full { registry: &'static str },
}

/// An ordered registry of `T` keyed by [`ResourceLocation`].
#[derive(Debug, Clone)]
pub struct Registry<T> {
    kind: &'static str,
    names: Vec<ResourceLocation>,
    values: Vec<T>,
    by_name: FxHashMap<ResourceLocation, RawId>,
    frozen: bool,
    max_len: usize,
}

impl<T> Registry<T> {
    /// A registry for entries of `kind` (e.g. `"block"`) with room for `max_len` entries.
    pub fn new(kind: &'static str, max_len: usize) -> Self {
        Self {
            kind,
            names: Vec::new(),
            values: Vec::new(),
            by_name: FxHashMap::default(),
            frozen: false,
            max_len,
        }
    }

    pub fn kind(&self) -> &'static str {
        self.kind
    }

    pub fn register(&mut self, id: ResourceLocation, value: T) -> Result<RawId, RegistryError> {
        if self.frozen {
            return Err(RegistryError::Frozen {
                registry: self.kind,
                id,
            });
        }
        if self.by_name.contains_key(&id) {
            return Err(RegistryError::Duplicate {
                registry: self.kind,
                id,
            });
        }
        if self.values.len() >= self.max_len {
            return Err(RegistryError::Full {
                registry: self.kind,
            });
        }
        let raw = self.values.len() as RawId;
        self.by_name.insert(id.clone(), raw);
        self.names.push(id);
        self.values.push(value);
        Ok(raw)
    }

    /// Replaces an existing entry (data-pack override), keeping its numeric id.
    pub fn replace(&mut self, id: &ResourceLocation, value: T) -> Result<RawId, RegistryError> {
        let raw = self.id_of(id).ok_or_else(|| RegistryError::Unknown {
            registry: self.kind,
            id: id.clone(),
        })?;
        self.values[raw as usize] = value;
        Ok(raw)
    }

    /// Registers or overrides.
    pub fn upsert(&mut self, id: ResourceLocation, value: T) -> Result<RawId, RegistryError> {
        if self.by_name.contains_key(&id) {
            self.replace(&id, value)
        } else {
            self.register(id, value)
        }
    }

    pub fn freeze(&mut self) {
        self.frozen = true;
    }

    #[inline]
    pub fn get(&self, raw: RawId) -> Option<&T> {
        self.values.get(raw as usize)
    }

    #[inline]
    pub fn get_mut(&mut self, raw: RawId) -> Option<&mut T> {
        self.values.get_mut(raw as usize)
    }

    #[inline]
    pub fn id_of(&self, name: &ResourceLocation) -> Option<RawId> {
        self.by_name.get(name).copied()
    }

    /// Looks up by string (`"stone"` or `"ns:path"`).
    pub fn id_of_str(&self, name: &str) -> Option<RawId> {
        ResourceLocation::parse(name)
            .ok()
            .and_then(|n| self.id_of(&n))
    }

    pub fn by_name(&self, name: &ResourceLocation) -> Option<&T> {
        self.id_of(name).and_then(|i| self.get(i))
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (RawId, &ResourceLocation, &T)> {
        self.names
            .iter()
            .zip(self.values.iter())
            .enumerate()
            .map(|(i, (n, v))| (i as RawId, n, v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rl(s: &str) -> ResourceLocation {
        ResourceLocation::parse(s).unwrap()
    }

    #[test]
    fn register_lookup_freeze() {
        let mut r: Registry<u32> = Registry::new("thing", 16);
        let a = r.register(rl("a"), 10).unwrap();
        let b = r.register(rl("mod:b"), 20).unwrap();
        assert_eq!((a, b), (0, 1));
        assert_eq!(r.get(b), Some(&20));
        assert_eq!(r.id_of_str("hearth:a"), Some(0));
        assert!(matches!(
            r.register(rl("a"), 1),
            Err(RegistryError::Duplicate { .. })
        ));
        r.replace(&rl("a"), 11).unwrap();
        assert_eq!(r.get(0), Some(&11));
        r.freeze();
        assert!(matches!(
            r.register(rl("c"), 1),
            Err(RegistryError::Frozen { .. })
        ));
    }

    #[test]
    fn capacity() {
        let mut r: Registry<()> = Registry::new("tiny", 1);
        r.register(rl("a"), ()).unwrap();
        assert!(matches!(
            r.register(rl("b"), ()),
            Err(RegistryError::Full { .. })
        ));
    }
}

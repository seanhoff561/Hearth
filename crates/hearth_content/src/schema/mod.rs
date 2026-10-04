//! Content schemas: one module per domain of `data/<namespace>/`. The Rust structs are the
//! schemas; `schema` numbers in files are checked against `Entry::SCHEMA`.
//!
//! Every entry has an `id`, a `status` (`Implemented` when a game system uses it, `Planned` for
//! data authored ahead of its system, hidden in game), optional `notes` and `realism_source`,
//! and `uncertain: true` for values a realism review should look at. Quantities are SI with the
//! unit in the field name (`_kg`, `_m`, `_c`, `_mpa`, ...).

pub mod body;
pub mod config;
pub mod ecosystem;
pub mod era;
pub mod fauna;
pub mod flora;
pub mod geology;
pub mod humans;
pub mod item;
pub mod knowledge;
pub mod life;
pub mod material;
pub mod mind;
pub mod process;
pub mod psyche;
pub mod station;

use std::fmt;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Whether a game system uses an entry yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Status {
    Implemented,
    #[default]
    Planned,
}

/// Common behaviour of domain entries.
pub trait Entry: DeserializeOwned + Clone + fmt::Debug + Send + Sync + 'static {
    /// Directory under `data/<namespace>/`.
    const DOMAIN: &'static str;
    /// Current schema version of the domain.
    const SCHEMA: u32;
    fn id(&self) -> &str;
    fn set_id(&mut self, id: String);
    fn status(&self) -> Status;
    fn uncertain(&self) -> bool;
    fn display_name(&self) -> &str;
}

/// Declares an entry struct with the standard fields appended.
macro_rules! entry {
    (
        $(#[$m:meta])*
        pub struct $name:ident in $domain:literal, schema $schema:literal, name $namefield:ident {
            $($(#[$fm:meta])* pub $f:ident : $t:ty),* $(,)?
        }
    ) => {
        $(#[$m])*
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct $name {
            /// `path` or `namespace:path`; bare ids take the file's namespace.
            pub id: String,
            $($(#[$fm])* pub $f: $t,)*
            #[serde(default)]
            pub status: $crate::schema::Status,
            #[serde(default)]
            pub notes: Option<String>,
            /// Where the values come from, e.g. "approximate adult mass from general zoological
            /// references; review".
            #[serde(default)]
            pub realism_source: Option<String>,
            /// Values that a realism review should check.
            #[serde(default)]
            pub uncertain: bool,
        }

        impl $crate::schema::Entry for $name {
            const DOMAIN: &'static str = $domain;
            const SCHEMA: u32 = $schema;
            fn id(&self) -> &str {
                &self.id
            }
            fn set_id(&mut self, id: String) {
                self.id = id;
            }
            fn status(&self) -> $crate::schema::Status {
                self.status
            }
            fn uncertain(&self) -> bool {
                self.uncertain
            }
            fn display_name(&self) -> &str {
                &self.$namefield
            }
        }
    };
}
pub(crate) use entry;

/// sRGB colour written as `"#rrggbb"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color(pub [u8; 3]);

impl Color {
    pub fn parse(s: &str) -> Option<Color> {
        let h = s.strip_prefix('#')?;
        if h.len() != 6 {
            return None;
        }
        let v = u32::from_str_radix(h, 16).ok()?;
        Some(Color([(v >> 16) as u8, (v >> 8) as u8, v as u8]))
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Color::parse(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("expected \"#rrggbb\", got {s:?}")))
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let [r, g, b] = self.0;
        s.serialize_str(&format!("#{r:02x}{g:02x}{b:02x}"))
    }
}

/// A closed range `(min, max)`.
pub type Range = (f32, f32);

/// The four seasons (of the hemisphere the thing lives in).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

/// Which time scale a duration runs on (v2 §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TimeScale {
    /// Body & action time: one game day stands for one real day.
    #[default]
    Day,
    /// Life-cycle & calendar time: one game year stands for one real year.
    Year,
}

/// A real-world duration and the scale it is compressed on.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Duration {
    /// Real-world duration in hours.
    pub hours: f32,
    #[serde(default)]
    pub scale: TimeScale,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_round_trip() {
        let c = Color::parse("#8a8175").unwrap();
        assert_eq!(c.0, [0x8a, 0x81, 0x75]);
        assert!(Color::parse("8a8175").is_none());
        assert!(Color::parse("#8a81").is_none());
        let s = serde_json::to_string(&c).unwrap();
        assert_eq!(s, "\"#8a8175\"");
    }
}

//! Eras (`eras/`), v2 §17, reduced by Amendment E §6.4 to the era list: Wild Earth playable,
//! the others "Coming soon" until Phase F brings their people.

use super::entry;

entry! {
    /// A historical setting a world can be created in.
    pub struct Era in "eras", schema 1, name name {
        pub name: String,
        /// One line, as the era selector tells it.
        pub description: String,
        /// Order in the era selector.
        pub order: u8,
        /// Playable now; the others are shown as "Coming soon".
        pub available: bool,
    }
}

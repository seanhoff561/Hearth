//! Languages (`humans/language/`), V2.1 §10.1; H5: what a people's languages are drawn from — the
//! sounds human languages use and how many of them have each, the shapes of their syllables, their
//! word orders, how fast a language changes once its speakers part — and the meanings every
//! language has a word for.

use serde::{Deserialize, Serialize};

use super::entry;
use crate::IdRef;

/// A syllable's shape beyond the consonant and vowel every language has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shape {
    /// A vowel alone.
    V,
    /// Closed by a consonant.
    Cvc,
    /// Two consonants before the vowel.
    Ccv,
    /// Two before and one after.
    Ccvc,
}

/// A language's dominant word order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Order {
    #[default]
    Sov,
    Svo,
    Vso,
    Vos,
    Ovs,
}

/// Where in a word a sound change takes hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Context {
    Anywhere,
    BetweenVowels,
    WordStart,
    WordEnd,
    /// Before a front vowel (i, e).
    BeforeFront,
}

/// A regular sound change: a sound become another (or lost) where it stands in its context,
/// everywhere at once, with its chance a century.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoundChange {
    pub from: String,
    pub to: String,
    pub context: Context,
    pub chance: f32,
}

/// What kind of word a meaning takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WordKind {
    Noun,
    Verb,
    Quality,
    Kin,
    Pronoun,
    Particle,
}

entry! {
    /// What a people's languages are drawn from (V2.1 §10.1).
    pub struct LanguageGenerator in "humans/language/generators", schema 1, name name {
        pub name: String,
        pub species: IdRef,
        /// The sounds, each with the share of the world's languages that have it.
        pub consonants: Vec<(String, f32)>,
        pub vowels: Vec<(String, f32)>,
        /// How many consonants and vowels a language has.
        pub consonant_count: (u8, u8),
        pub vowel_count: (u8, u8),
        /// Syllable shapes beyond CV, each with the share of languages allowing it.
        pub shapes: Vec<(Shape, f32)>,
        pub orders: Vec<(Order, f32)>,
        /// The share of languages putting adjectives after the noun, marking a plural, a past.
        pub adjective_after: f32,
        pub plural: f32,
        pub past: f32,
        /// A word's chance a century to be replaced; a name's syllables.
        pub replacement: f32,
        pub name_syllables: (u8, u8),
        pub changes: Vec<SoundChange>,
    }
}

entry! {
    /// A meaning every language has a word for (V2.1 §10.1); its name is the gloss the player
    /// reads.
    pub struct Meaning in "humans/language/meanings", schema 1, name name {
        pub name: String,
        pub kind: WordKind,
        /// How many syllables its word tends to have.
        pub syllables: u8,
    }
}

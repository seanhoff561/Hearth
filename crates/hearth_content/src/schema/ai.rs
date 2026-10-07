//! The conversation backend's data (`ai/`), V2.1 §10.4; H10: the words a person may say when a
//! language model phrases its speech acts — everyday words any forager has, the words of each
//! technique (theirs alone who know it), the words of later ages no one here may say — the
//! prompts the model is asked with, and the words that point to a speech act in what a player
//! types.

use serde::{Deserialize, Serialize};

use super::entry;
use crate::IdRef;

/// The speech acts a player's words may be taken as (`Cue::act`, and the backend's answer).
pub const ACTS: &[&str] = &[
    "greet",
    "introduce",
    "thank",
    "apologise",
    "praise",
    "joke",
    "insult",
    "be_taught",
    "teach",
    "join",
    "pair",
];

/// The gestures, as `gesture:<kind>` names them.
pub const GESTURES: &[&str] = &[
    "point",
    "show",
    "offer",
    "beckon",
    "shoo",
    "threat_display",
    "submission",
    "embrace",
    "hands",
];

/// Whether an act's name is one a player's words may be taken as.
pub fn is_act(act: &str) -> bool {
    ACTS.contains(&act)
        || act
            .strip_prefix("gesture:")
            .is_some_and(|g| GESTURES.contains(&g))
}

/// What a list of words is to the filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WordsKind {
    /// Words anyone may say (or, with `known_with`, anyone who knows that technique).
    #[default]
    Everyday,
    /// Kin words: said of one's own kin only as one has them ("my sister" from one who has a
    /// sister).
    Kin,
    /// Words of later ages: never said, whatever the line (labelled as anachronisms).
    LaterAges,
    /// Words no line may hold (V2.1 §1's ground rules): labelled as forbidden.
    Forbidden,
}

entry! {
    /// A list of words for the conversation filter (V2.1 §10.4): every word of a phrased line
    /// must be on a list its speaker may say.
    pub struct WordList in "ai/words", schema 1, name name {
        pub name: String,
        #[serde(default)]
        pub kind: WordsKind,
        /// A technique (a knowledge node): only those who know it may say these words, and they
        /// are an anachronism in the mouths of others.
        #[serde(default)]
        pub known_with: Option<IdRef>,
        /// The words, lower case, in their plain forms ("go", not "went"; irregular forms are
        /// listed as words of their own).
        pub words: Vec<String>,
    }
}

entry! {
    /// A prompt the backend is asked with: its instructions and its request, with `{slots}`
    /// filled from what the speaker knows.
    pub struct Prompt in "ai/prompts", schema 1, name name {
        pub name: String,
        /// The instructions (the system prompt).
        pub system: String,
        /// The request, with its slots.
        pub user: String,
        /// The longest reply asked for (tokens).
        pub max_tokens: u32,
    }
}

entry! {
    /// Words pointing to a speech act in what a player types: the guesses offered when the
    /// backend cannot say which act was meant.
    pub struct Cue in "ai/cues", schema 1, name name {
        pub name: String,
        /// The act: `greet`, `introduce`, `thank`, `apologise`, `praise`, `joke`, `insult`,
        /// `be_taught`, `teach`, `join`, `pair`, or `gesture:<kind>`.
        pub act: String,
        /// Words or short phrases, lower case.
        pub words: Vec<String>,
    }
}

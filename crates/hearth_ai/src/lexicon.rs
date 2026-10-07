//! The knowledge and anachronism filter (V2.1 §10.4; H10). A line a model phrases for a person is
//! shown only if every word of it is one that person may say: an everyday word any forager has,
//! a gloss of its language's words, a word of a technique it knows, a name it knows — and kin
//! words only of the kin it has. A word of a technique it does not know, of a later age, a name
//! it does not know, anything else outside its words, a character no spoken line holds, a line
//! too long, or one saying no where the act says yes: the line is refused, and the templated
//! line stands. The lists are data (`data/hearth/ai/words/`); this reads them and holds lines to
//! them.

use std::collections::{HashMap, HashSet};

use hearth_content::Content;
use hearth_content::schema::ai::WordsKind;
use hearth_craft::knowledge::Graph;
use hearth_people::converse::Guard;
use hearth_people::speech::Act;

/// The most words a phrased line may have, and characters.
pub const MAX_WORDS: usize = 30;
pub const MAX_CHARS: usize = 240;

/// Words that say no.
const NEGATIONS: &[&str] = &[
    "no", "not", "never", "nothing", "none", "nobody", "nowhere", "neither", "nor", "away",
];

/// Words before a kin word that make it any kin, not one's own ("a mother", "like a brother").
const GENERIC: &[&str] = &[
    "a", "an", "any", "every", "each", "no", "like", "some", "such", "as",
];

/// Punctuation a spoken line may hold.
const PUNCTUATION: &str = ".,!?;:'’‘\"“”-–—…";

/// Why a line was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejected {
    /// Nothing said.
    Empty,
    /// Longer than a line (its words).
    TooLong(usize),
    /// A character no spoken line holds (a digit, markup, a line break).
    Character(char),
    /// A word no line may hold.
    Forbidden(String),
    /// A word of a later age.
    LaterAge(String),
    /// A word of a technique the speaker does not know (the word, the technique).
    Technique(String, String),
    /// A name the speaker does not know.
    Name(String),
    /// Kin the speaker does not have.
    Kin(String),
    /// A word outside the speaker's words.
    Word(String),
    /// The line says the opposite of the act (no for yes, yes for no).
    Meaning,
}

impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rejected::Empty => write!(f, "nothing said"),
            Rejected::TooLong(n) => write!(f, "too long ({n} words)"),
            Rejected::Character(c) => write!(f, "a character no line holds ({c:?})"),
            Rejected::Forbidden(w) => write!(f, "a word never said ({w})"),
            Rejected::LaterAge(w) => write!(f, "an anachronism ({w})"),
            Rejected::Technique(w, t) => write!(f, "a technique it does not know ({w}: {t})"),
            Rejected::Name(n) => write!(f, "a name not given it ({n})"),
            Rejected::Kin(w) => write!(f, "kin it does not have ({w})"),
            Rejected::Word(w) => write!(f, "a word outside its words ({w})"),
            Rejected::Meaning => write!(f, "the line says the opposite of the act"),
        }
    }
}

/// The words people may say, by list.
#[derive(Debug, Clone, Default)]
pub struct Lexicon {
    everyday: HashSet<String>,
    kin: HashSet<String>,
    later: HashSet<String>,
    forbidden: HashSet<String>,
    /// A word → the techniques whose knowers alone may say it.
    gated: HashMap<String, Vec<String>>,
    /// A technique's name, by its id.
    names: HashMap<String, String>,
}

/// The words of a name or phrase, lower case: each whole, and a hyphened one's parts too.
fn words_of(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw in s.split(|c: char| !(c.is_alphabetic() || c == '-' || c == '\'' || c == '’')) {
        let w = raw
            .to_lowercase()
            .replace('’', "'")
            .trim_matches(|c| c == '\'' || c == '-')
            .trim_end_matches("'s")
            .to_owned();
        if w.is_empty() {
            continue;
        }
        if w.contains('-') {
            out.extend(w.split('-').filter(|p| !p.is_empty()).map(str::to_owned));
        }
        out.push(w);
    }
    out
}

/// A word's plain forms as written: itself, and without a plural's ending.
fn exact_forms(w: &str) -> Vec<String> {
    let mut out = vec![w.to_owned()];
    if let Some(b) = w.strip_suffix("ies")
        && b.len() >= 2
    {
        out.push(format!("{b}y"));
    }
    if let Some(b) = w.strip_suffix("es")
        && b.len() >= 2
    {
        out.push(b.to_owned());
    }
    if let Some(b) = w.strip_suffix('s')
        && b.len() >= 2
        && !b.ends_with('s')
    {
        out.push(b.to_owned());
    }
    out
}

/// A consonant doubled at a word's end ("stopp" → "stop").
fn undouble(b: &str) -> Option<String> {
    let c: Vec<char> = b.chars().collect();
    let n = c.len();
    (n >= 3 && c[n - 1] == c[n - 2] && !"aeiouls".contains(c[n - 1]))
        .then(|| c[..n - 1].iter().collect())
}

/// The plain words a word may be formed from: endings taken off (-ed, -ing, -er, -est, -ly,
/// -ness, -ful, -less), a doubled consonant undone, a dropped e restored, "un-" taken off.
fn stems(w: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let push = |s: String, out: &mut Vec<String>| {
        if s.len() >= 2 && !out.contains(&s) {
            out.push(s);
        }
    };
    for base in exact_forms(w) {
        for (suffix, to) in [
            ("ied", "y"),
            ("ier", "y"),
            ("iest", "y"),
            ("ily", "y"),
            ("iness", "y"),
            ("ying", "ie"),
        ] {
            if let Some(b) = base.strip_suffix(suffix) {
                push(format!("{b}{to}"), &mut out);
            }
        }
        for suffix in [
            "ed", "ing", "er", "est", "ly", "ness", "ful", "less", "en", "d", "y",
        ] {
            let Some(b) = base.strip_suffix(suffix) else {
                continue;
            };
            if b.len() < 2 {
                continue;
            }
            push(b.to_owned(), &mut out);
            push(format!("{b}e"), &mut out);
            if suffix == "ly" {
                push(format!("{b}le"), &mut out);
            }
            if let Some(u) = undouble(b) {
                push(u, &mut out);
            }
        }
        if let Some(b) = base.strip_prefix("un")
            && b.len() >= 3
        {
            push(b.to_owned(), &mut out);
        }
    }
    out
}

/// A word with its contraction undone ("don't" → do not; "we'll" → we will; "Tana's" → Tana).
fn expand(w: &str) -> Vec<String> {
    let w = w.trim_matches(|c| c == '\'' || c == '-');
    if w.is_empty() {
        return Vec::new();
    }
    if w == "cannot" {
        return vec!["can".to_owned(), "not".to_owned()];
    }
    if w == "let's" {
        return vec!["let".to_owned(), "us".to_owned()];
    }
    if let Some(base) = w.strip_suffix("n't") {
        let base = match base {
            "wo" => "will",
            "ca" => "can",
            "sha" => "shall",
            b => b,
        };
        return vec![base.to_owned(), "not".to_owned()];
    }
    for (suffix, full) in [
        ("'ll", "will"),
        ("'re", "are"),
        ("'ve", "have"),
        ("'d", "would"),
        ("'m", "am"),
    ] {
        if let Some(base) = w.strip_suffix(suffix) {
            return vec![base.to_owned(), full.to_owned()];
        }
    }
    if let Some(base) = w.strip_suffix("'s") {
        return vec![base.to_owned()];
    }
    if let Some(base) = w.strip_suffix("s'") {
        return vec![format!("{base}s")];
    }
    vec![w.to_owned()]
}

/// A word of a line: as written (apostrophes made plain), and whether it begins a sentence.
struct Token {
    text: String,
    starts: bool,
}

fn tokens(line: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut starts = true;
    let mut next_starts = true;
    for c in line.chars().chain(std::iter::once(' ')) {
        if c.is_alphabetic() || c == '\'' || c == '’' || (c == '-' && !word.is_empty()) {
            if word.is_empty() {
                starts = next_starts;
            }
            word.push(if c == '’' { '\'' } else { c });
            continue;
        }
        if !word.is_empty() {
            let w = std::mem::take(&mut word);
            let w = w.trim_end_matches('-').to_owned();
            if !w.trim_matches('\'').is_empty() {
                out.push(Token { text: w, starts });
            }
            next_starts = false;
        }
        if ".!?:…".contains(c) {
            next_starts = true;
        }
    }
    out
}

impl Lexicon {
    /// The lists of the content, and the words of each technique's name.
    pub fn from_content(c: &Content, graph: &Graph) -> Self {
        let mut l = Lexicon::default();
        for list in c.word_lists.iter() {
            let words = list.words.iter().map(|w| w.to_lowercase());
            match (list.kind, &list.known_with) {
                (WordsKind::Everyday, None) => l.everyday.extend(words),
                (WordsKind::Everyday, Some(node)) => {
                    for w in words {
                        l.gated.entry(w).or_default().push(node.to_string());
                    }
                }
                (WordsKind::Kin, _) => l.kin.extend(words),
                (WordsKind::LaterAges, _) => l.later.extend(words),
                (WordsKind::Forbidden, _) => l.forbidden.extend(words),
            }
        }
        // Every language has a word for each meaning: their glosses are anyone's words.
        for m in c.meanings.iter() {
            l.everyday.extend(words_of(&m.name));
        }
        // A technique's name: its words that are no one's everyday words are its knowers'.
        for n in &graph.nodes {
            l.names.insert(n.id.clone(), n.name.clone());
            for w in words_of(&n.name) {
                let plain = exact_forms(&w)
                    .iter()
                    .chain(stems(&w).iter())
                    .any(|x| l.everyday.contains(x) || l.kin.contains(x));
                if !plain {
                    let nodes = l.gated.entry(w).or_default();
                    if !nodes.contains(&n.id) {
                        nodes.push(n.id.clone());
                    }
                }
            }
        }
        l
    }

    /// Whether a word is on an everyday list.
    pub fn is_everyday(&self, w: &str) -> bool {
        self.everyday.contains(w)
    }

    /// Words on both an everyday list and a list of words never said (a fault of the data).
    pub fn clashes(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .everyday
            .iter()
            .filter(|w| self.later.contains(*w) || self.forbidden.contains(*w))
            .cloned()
            .collect();
        out.sort();
        out
    }

    /// The words of a technique's name a speaker who does not know it may not say.
    pub fn words_of_technique(&self, id: &str) -> Vec<String> {
        let mut out: Vec<String> = self
            .gated
            .iter()
            .filter(|(_, nodes)| nodes.iter().any(|n| n == id))
            .map(|(w, _)| w.clone())
            .collect();
        out.sort();
        out
    }

    /// The words of later ages.
    pub fn later_words(&self) -> Vec<String> {
        let mut out: Vec<String> = self.later.iter().cloned().collect();
        out.sort();
        out
    }

    /// Whether a speaker may say a word as written (a name given it, an everyday word, a word of
    /// a technique it knows).
    pub fn may_say(&self, w: &str, guard: &Guard) -> bool {
        let names: HashSet<String> = guard.names.iter().map(|n| n.to_lowercase()).collect();
        exact_forms(w)
            .iter()
            .any(|x| self.allowed(x, guard, &names))
    }

    /// Whether a speaker may say a word as written.
    fn allowed(&self, w: &str, guard: &Guard, names: &HashSet<String>) -> bool {
        self.everyday.contains(w)
            || names.contains(w)
            || self
                .gated
                .get(w)
                .is_some_and(|nodes| nodes.iter().any(|n| guard.known.contains(n)))
    }

    /// Why a speaker may not say a word, if it may not.
    fn word(&self, w: &str, guard: &Guard, names: &HashSet<String>) -> Option<Rejected> {
        let exact = exact_forms(w);
        if exact.iter().any(|x| self.forbidden.contains(x)) {
            return Some(Rejected::Forbidden(w.to_owned()));
        }
        if exact.iter().any(|x| self.allowed(x, guard, names)) {
            return None;
        }
        if let Some(r) = self.why_not(&exact, w) {
            return Some(r);
        }
        let stems = stems(w);
        if stems.iter().any(|x| self.forbidden.contains(x)) {
            return Some(Rejected::Forbidden(w.to_owned()));
        }
        if stems.iter().any(|x| self.allowed(x, guard, names)) {
            return None;
        }
        Some(
            self.why_not(&stems, w)
                .unwrap_or(Rejected::Word(w.to_owned())),
        )
    }

    /// Which list keeps a word's forms from a speaker: a later age's, or a technique's.
    fn why_not(&self, forms: &[String], w: &str) -> Option<Rejected> {
        if forms.iter().any(|x| self.later.contains(x)) {
            return Some(Rejected::LaterAge(w.to_owned()));
        }
        forms.iter().find_map(|x| {
            self.gated.get(x).map(|nodes| {
                let name = nodes
                    .first()
                    .and_then(|n| self.names.get(n))
                    .cloned()
                    .unwrap_or_default();
                Rejected::Technique(w.to_owned(), name)
            })
        })
    }

    /// A line as its speaker may say it — trimmed, its wrapping quotes taken off — or why it may
    /// not be said.
    pub fn check(&self, line: &str, guard: &Guard) -> Result<String, Rejected> {
        let mut line = line.trim().to_owned();
        if let Some(c) = line
            .chars()
            .find(|c| *c == '\n' || *c == '\r' || *c == '\t')
        {
            return Err(Rejected::Character(c));
        }
        // Wrapping quotes, and a leading "Name:" the model put before the line.
        loop {
            let t = line.trim();
            let mut cs = t.chars();
            let (first, last) = (cs.next(), t.chars().last());
            let wrapped = matches!(
                (first, last),
                (Some('"'), Some('"')) | (Some('“'), Some('”')) | (Some('\''), Some('\''))
            ) && t.chars().count() >= 2;
            if wrapped {
                let inner: String = t.chars().skip(1).take(t.chars().count() - 2).collect();
                line = inner.trim().to_owned();
                continue;
            }
            if let Some((who, rest)) = t.split_once(':')
                && guard.names.contains(who.trim())
            {
                line = rest.trim().to_owned();
                continue;
            }
            line = t.to_owned();
            break;
        }
        let line: String = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.chars().count() > MAX_CHARS {
            return Err(Rejected::TooLong(line.split_whitespace().count()));
        }
        if let Some(c) = line
            .chars()
            .find(|c| !(c.is_alphabetic() || *c == ' ' || PUNCTUATION.contains(*c)))
        {
            return Err(Rejected::Character(c));
        }
        let tokens = tokens(&line);
        if tokens.is_empty() {
            return Err(Rejected::Empty);
        }
        if tokens.len() > MAX_WORDS {
            return Err(Rejected::TooLong(tokens.len()));
        }
        let names: HashSet<String> = guard.names.iter().map(|n| n.to_lowercase()).collect();
        let mut said: Vec<String> = Vec::new();
        for t in &tokens {
            // Names first: one it does not know is never said; a capital mid-sentence is a name.
            let bare = t.text.trim_end_matches("'s").trim_end_matches('\'');
            if guard.unknown_names.contains(bare) && !guard.names.contains(bare) {
                return Err(Rejected::Name(bare.to_owned()));
            }
            if guard.names.contains(bare) {
                said.push(bare.to_lowercase());
                continue;
            }
            let capital = t.text.chars().next().is_some_and(char::is_uppercase);
            let me = t.text == "I" || t.text.starts_with("I'");
            if capital && !t.starts && !me {
                return Err(Rejected::Name(bare.to_owned()));
            }
            for w in expand(&t.text.to_lowercase()) {
                // A hyphened word on a list as a whole, else each part.
                let parts: Vec<String> = if w.contains('-')
                    && !(self.kin.contains(&w)
                        || self.everyday.contains(&w)
                        || self.gated.contains_key(&w))
                {
                    w.split('-')
                        .filter(|p| !p.is_empty())
                        .map(str::to_owned)
                        .collect()
                } else {
                    vec![w]
                };
                for w in parts {
                    let kin = exact_forms(&w).into_iter().find(|x| self.kin.contains(x));
                    if let Some(k) = kin {
                        let generic = said.last().is_some_and(|p| GENERIC.contains(&p.as_str()));
                        if !generic && !guard.kin.contains(&w) && !guard.kin.contains(&k) {
                            return Err(Rejected::Kin(w));
                        }
                    } else if let Some(r) = self.word(&w, guard, &names) {
                        return Err(r);
                    }
                    said.push(w);
                }
            }
        }
        // The act's sense kept: a refusal says no, a yes does not begin with no.
        let negated = said.iter().any(|w| NEGATIONS.contains(&w.as_str()));
        match guard.act {
            Some(Act::Refuse) if !negated => return Err(Rejected::Meaning),
            Some(Act::Accept)
                if said
                    .first()
                    .is_some_and(|w| NEGATIONS.contains(&w.as_str()) && w != "away") =>
            {
                return Err(Rejected::Meaning);
            }
            _ => {}
        }
        Ok(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_taken_apart_as_written() {
        assert_eq!(expand("don't"), vec!["do", "not"]);
        assert_eq!(expand("won't"), vec!["will", "not"]);
        assert_eq!(expand("we'll"), vec!["we", "will"]);
        assert_eq!(expand("tana's"), vec!["tana"]);
        assert!(stems("making").contains(&"make".to_owned()));
        assert!(stems("running").contains(&"run".to_owned()));
        assert!(stems("happily").contains(&"happy".to_owned()));
        assert!(stems("gently").contains(&"gentle".to_owned()));
        assert!(stems("kindness").contains(&"kind".to_owned()));
        assert!(stems("dying").contains(&"die".to_owned()));
        assert!(stems("unhappy").contains(&"happy".to_owned()));
        assert!(exact_forms("berries").contains(&"berry".to_owned()));
        let t = tokens("Look, Tana! We go there.");
        let starts: Vec<(&str, bool)> = t.iter().map(|t| (t.text.as_str(), t.starts)).collect();
        assert_eq!(
            starts,
            vec![
                ("Look", true),
                ("Tana", false),
                ("We", true),
                ("go", false),
                ("there", false)
            ]
        );
    }
}

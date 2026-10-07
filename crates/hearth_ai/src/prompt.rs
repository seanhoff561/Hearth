//! The prompts (V2.1 §10.4; H10): templates kept as data (`data/hearth/ai/prompts.ron`), their
//! `{slots}` filled from what a speaker knows of itself — never from what it does not know — or
//! from what the player typed and the techniques it may mean.

use hearth_content::Content;
use hearth_people::converse::Context;

use crate::Request;
use crate::lexicon::MAX_WORDS;

/// The slots a phrasing prompt may hold, and a parsing prompt.
pub const PHRASE_SLOTS: &[&str] = &[
    "self",
    "temperament",
    "feelings",
    "values",
    "knows",
    "kin",
    "recent",
    "listener",
    "names",
    "act",
    "meaning",
    "gesture",
    "words",
];
pub const PARSE_SLOTS: &[&str] = &["techniques", "listener", "text"];

/// The words a phrased line is asked to keep within (the filter allows a little more).
pub const ASKED_WORDS: usize = 20;

/// A prompt: its instructions and request, with slots.
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    pub system: String,
    pub user: String,
    pub max_tokens: u32,
}

/// The two prompts the game asks with.
#[derive(Debug, Clone, PartialEq)]
pub struct Prompts {
    pub phrase: Template,
    pub parse: Template,
}

/// The names in braces a template holds (`{self}`; braces about anything else are left be).
pub fn slots(template: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(i) = rest.find('{') {
        let after = &rest[i + 1..];
        if let Some(j) = after.find('}') {
            let name = &after[..j];
            if !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
                out.push(name.to_owned());
            }
        }
        rest = after;
    }
    out
}

/// A template with its slots filled.
pub fn fill(template: &str, values: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(template.len() + 256);
    let mut rest = template;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let slot = after.find('}').and_then(|j| {
            values
                .iter()
                .find(|(k, _)| *k == &after[..j])
                .map(|v| (j, v))
        });
        match slot {
            Some((j, (_, v))) => {
                out.push_str(v);
                rest = &after[j + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// A list in words, or "none".
fn listed(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_owned()
    } else {
        items.join("; ")
    }
}

impl Prompts {
    /// The content's prompts; none if either is missing or holds a slot the game cannot fill.
    pub fn from_content(c: &Content) -> Option<Self> {
        let get = |id: &str, known: &[&str]| -> Option<Template> {
            let p = c.prompts.get(&format!("hearth:{id}"))?;
            let unknown: Vec<String> = slots(&p.system)
                .into_iter()
                .chain(slots(&p.user))
                .filter(|s| !known.contains(&s.as_str()))
                .collect();
            if !unknown.is_empty() {
                log::warn!("the {id} prompt has slots the game cannot fill: {unknown:?}");
                return None;
            }
            Some(Template {
                system: p.system.clone(),
                user: p.user.clone(),
                max_tokens: p.max_tokens,
            })
        };
        Some(Self {
            phrase: get("phrase", PHRASE_SLOTS)?,
            parse: get("parse", PARSE_SLOTS)?,
        })
    }

    /// The request phrasing a speech act from what its speaker knows of itself.
    pub fn phrase(&self, c: &Context) -> Request {
        let values = [
            ("self", c.who.clone()),
            ("temperament", listed(&c.temperament)),
            ("feelings", listed(&c.feelings)),
            ("values", listed(&c.values)),
            ("knows", listed(&c.knows)),
            ("kin", listed(&c.kin)),
            ("recent", listed(&c.recent)),
            ("listener", c.listener.clone()),
            ("names", listed(&c.names)),
            ("act", c.act.clone()),
            ("meaning", c.meaning.clone()),
            (
                "gesture",
                c.gesture
                    .as_ref()
                    .map_or_else(String::new, |g| format!(", {g}")),
            ),
            ("words", ASKED_WORDS.min(MAX_WORDS).to_string()),
        ];
        Request {
            system: fill(&self.phrase.system, &values),
            user: fill(&self.phrase.user, &values),
            max_tokens: self.phrase.max_tokens,
            temperature: 0.7,
        }
    }

    /// The request reading what a player typed to someone, with the techniques it may mean.
    pub fn parse(&self, text: &str, listener: &str, techniques: &[String]) -> Request {
        let values = [
            ("techniques", listed(techniques)),
            ("listener", listener.to_owned()),
            // Its quotes made plain, so the typed words cannot close the prompt's own.
            ("text", text.replace(['"', '“', '”'], "'")),
        ];
        Request {
            system: fill(&self.parse.system, &values),
            user: fill(&self.parse.user, &values),
            max_tokens: self.parse.max_tokens,
            temperature: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_are_filled_and_other_braces_left_be() {
        let t = "Hello {self}; reply {\"act\": \"x\"} {words} {unknown}";
        assert_eq!(slots(t), vec!["self", "words", "unknown"]);
        let out = fill(t, &[("self", "Tuka".into()), ("words", "20".into())]);
        assert_eq!(out, "Hello Tuka; reply {\"act\": \"x\"} 20 {unknown}");
    }
}

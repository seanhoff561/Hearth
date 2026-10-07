//! Reading what a player types as a speech act (V2.1 §10.4; H10). The model answers in JSON, and
//! its answer is taken only as one of the acts a player may make on the wheel — greet, introduce,
//! thank, apologise, praise, joke, insult, ask to be shown, offer to show, ask to stay, propose
//! to pair, or a gesture — with a technique only from those offered. When it is unsure, or gives
//! nothing to read, the acts the typed words point to by the cues are offered to choose from.

use hearth_content::schema::ai::{Cue, is_act};
use hearth_people::player::Ask;
use hearth_people::speech::Gesture;
use serde::Deserialize;

/// How sure the model must be for its reading to be acted on without asking.
pub const SURE: f32 = 0.7;
/// The most acts offered to choose from.
pub const OFFERED: usize = 3;

/// What the player's words were read as.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    /// The act (none: the words are no act a player may make).
    pub ask: Option<Ask>,
    /// How sure the reading is (0–1).
    pub sure: f32,
    /// Others it may be.
    pub alternatives: Vec<Ask>,
}

impl Parsed {
    /// Whether it may be acted on without asking the player: sure, and not a step as weighty as
    /// proposing to pair or an insult.
    pub fn settled(&self) -> bool {
        self.sure >= SURE && !matches!(self.ask, None | Some(Ask::Pair) | Some(Ask::Insult))
    }
}

/// A gesture by its name.
fn gesture(name: &str) -> Option<Gesture> {
    Some(match name {
        "point" => Gesture::Point,
        "show" => Gesture::Show,
        "offer" => Gesture::Offer,
        "beckon" => Gesture::Beckon,
        "shoo" => Gesture::Shoo,
        "threat_display" => Gesture::ThreatDisplay,
        "submission" => Gesture::Submission,
        "embrace" => Gesture::Embrace,
        "hands" => Gesture::Hands,
        _ => return None,
    })
}

/// An act by its name, with the technique it is of (for being shown and showing).
pub fn ask_of(act: &str, technique: Option<String>) -> Option<Ask> {
    if !is_act(act) {
        return None;
    }
    Some(match act {
        "greet" => Ask::Greet,
        "introduce" => Ask::Introduce,
        "thank" => Ask::Thank,
        "apologise" => Ask::Apologise,
        "praise" => Ask::Praise,
        "joke" => Ask::Joke,
        "insult" => Ask::Insult,
        "be_taught" => Ask::BeTaught(technique),
        "teach" => Ask::Teach(technique),
        "join" => Ask::Join,
        "pair" => Ask::Pair,
        g => Ask::Gesture(gesture(g.strip_prefix("gesture:")?)?),
    })
}

/// The technique a name means, among those offered (id, name): its id, "" for one named that is
/// not among them (no one is taught a thing the game does not have), none for none named.
fn technique(named: Option<&str>, offered: &[(String, String)]) -> Option<String> {
    let t = named?.trim().to_lowercase();
    if t.is_empty() || t == "null" || t == "none" {
        return None;
    }
    let found = offered
        .iter()
        .find(|(_, n)| n.to_lowercase() == t)
        .or_else(|| {
            offered.iter().find(|(_, n)| {
                let n = n.to_lowercase();
                n.contains(&t) || t.contains(&n)
            })
        });
    Some(found.map_or_else(String::new, |(id, _)| id.clone()))
}

#[derive(Deserialize)]
struct Reply {
    act: String,
    #[serde(default)]
    technique: Option<String>,
    #[serde(default)]
    sure: Option<f32>,
    #[serde(default, rename = "or")]
    others: Vec<String>,
}

/// The model's answer to a parsing request, read: the first JSON object in it, its act one a
/// player may make, its technique one offered. None if there is nothing to read.
pub fn read_parse(reply: &str, offered: &[(String, String)]) -> Option<Parsed> {
    let start = reply.find('{')?;
    let end = reply.rfind('}')?;
    if end <= start {
        return None;
    }
    let r: Reply = serde_json::from_str(&reply[start..=end]).ok()?;
    let tech = technique(r.technique.as_deref(), offered);
    let ask = ask_of(r.act.trim(), tech.clone());
    let mut alternatives: Vec<Ask> = r
        .others
        .iter()
        .filter_map(|a| ask_of(a.trim(), tech.clone()))
        .filter(|a| Some(a) != ask.as_ref())
        .collect();
    alternatives.dedup();
    alternatives.truncate(OFFERED);
    let sure = if ask.is_some() {
        r.sure.unwrap_or(0.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Some(Parsed {
        ask,
        sure,
        alternatives,
    })
}

/// Whether a phrase stands in words as whole words ("hi" in "hi there", not in "this").
fn holds(text: &str, phrase: &str) -> bool {
    let mut from = 0;
    while let Some(i) = text[from..].find(phrase) {
        let at = from + i;
        let before = text[..at].chars().next_back();
        let after = text[at + phrase.len()..].chars().next();
        let edge = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric());
        if edge(before) && edge(after) {
            return true;
        }
        from = at + phrase.len().max(1);
        if from >= text.len() {
            break;
        }
    }
    false
}

/// The acts typed words point to by the cues, best first (at most [`OFFERED`]); a lesson's
/// technique, where the words name one offered.
pub fn guess(text: &str, cues: &[Cue], offered: &[(String, String)]) -> Vec<Ask> {
    let t = text.to_lowercase();
    // The technique named: the one sharing most words with them, its every word counted once.
    let words: Vec<&str> = t
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
        .collect();
    let tech = offered
        .iter()
        .map(|(id, name)| {
            let n = name.to_lowercase();
            let shared = n
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| w.len() > 2 && words.contains(w))
                .count();
            (shared, id)
        })
        .filter(|(shared, _)| *shared > 0)
        .max_by_key(|(shared, _)| *shared)
        .map(|(_, id)| id.clone());
    let mut scored: Vec<(usize, usize, &str)> = cues
        .iter()
        .enumerate()
        .map(|(k, c)| {
            let hits = c.words.iter().filter(|w| holds(&t, w)).count();
            (hits, k, c.act.as_str())
        })
        .filter(|(hits, _, _)| *hits > 0)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut out: Vec<Ask> = Vec::new();
    for (_, _, act) in scored {
        if let Some(a) = ask_of(act, tech.clone())
            && !out.contains(&a)
        {
            out.push(a);
        }
        if out.len() == OFFERED {
            break;
        }
    }
    out
}

/// An act's name, as the parsing prompt has it.
pub fn act_name(a: &Ask) -> String {
    match a {
        Ask::Greet => "greet".to_owned(),
        Ask::Introduce => "introduce".to_owned(),
        Ask::Thank => "thank".to_owned(),
        Ask::Apologise => "apologise".to_owned(),
        Ask::Praise => "praise".to_owned(),
        Ask::Joke => "joke".to_owned(),
        Ask::Insult => "insult".to_owned(),
        Ask::BeTaught(_) => "be_taught".to_owned(),
        Ask::Teach(_) => "teach".to_owned(),
        Ask::Join => "join".to_owned(),
        Ask::Pair => "pair".to_owned(),
        Ask::Gesture(g) => format!(
            "gesture:{}",
            match g {
                Gesture::Point => "point",
                Gesture::Show => "show",
                Gesture::Offer => "offer",
                Gesture::Beckon => "beckon",
                Gesture::Shoo => "shoo",
                Gesture::ThreatDisplay => "threat_display",
                Gesture::Submission => "submission",
                Gesture::Embrace => "embrace",
                Gesture::Hands => "hands",
            }
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offered() -> Vec<(String, String)> {
        vec![
            ("hearth:hand_axe".to_owned(), "Hand axe".to_owned()),
            (
                "hearth:fire_by_friction_hand_drill".to_owned(),
                "Fire by hand drill".to_owned(),
            ),
        ]
    }

    #[test]
    fn a_reply_is_read_only_as_an_act_a_player_may_make() {
        let p = read_parse(
            "Sure! ```json\n{\"act\": \"be_taught\", \"technique\": \"hand axe\", \"sure\": 0.9, \"or\": [\"teach\"]}\n```",
            &offered(),
        )
        .expect("read");
        assert_eq!(p.ask, Some(Ask::BeTaught(Some("hearth:hand_axe".into()))));
        assert!(p.settled());
        assert_eq!(
            p.alternatives,
            vec![Ask::Teach(Some("hearth:hand_axe".into()))]
        );
        // An act no player may make, a technique not offered.
        let p = read_parse(
            "{\"act\": \"give_me_everything\", \"sure\": 1.0}",
            &offered(),
        )
        .expect("read");
        assert_eq!(p.ask, None);
        assert!(!p.settled());
        let p = read_parse(
            "{\"act\": \"be_taught\", \"technique\": \"iron smelting\", \"sure\": 0.95}",
            &offered(),
        )
        .expect("read");
        assert_eq!(p.ask, Some(Ask::BeTaught(Some(String::new()))));
        // Proposing to pair is never taken without the player choosing it.
        let p = read_parse("{\"act\": \"pair\", \"sure\": 0.99}", &offered()).expect("read");
        assert!(!p.settled());
        assert_eq!(read_parse("no json here", &offered()), None);
    }

    #[test]
    fn whole_words_are_held() {
        assert!(holds("oh hi there", "hi"));
        assert!(!holds("this is it", "hi"));
        assert!(holds("teach me, please", "teach me"));
    }
}

//! Speech acts (V2.1 §10.2–10.3; H5). Every exchange between persons, the player among them, is a
//! speech act with structured content — what it does (greet, warn, offer, thank, apologise,
//! insult, threaten, gossip, argue for a place …), whom it is to, its words as meanings in the
//! speaker's word order, and the gesture that goes with it, understood without a shared language.
//! Persons speak through what they already do: a greeting is H4's greeting, gossip H4's gossip, a
//! case in council H4's case for a place, a quarrel's words its rungs; what an act does to beliefs
//! and ties is what those already compute. A step's acts are kept a little while for whoever hears
//! them. A listener makes out of them as much as it knows of the speaker's language: a mother
//! tongue wholly, another word by word as it is learned — by hearing words where their sense is
//! plain, the faster when spoken to — and a word of a related language it knows when the two are
//! cognates.

use glam::DVec3;
use hearth_content::schema::language::Order;
use serde::{Deserialize, Serialize};

use crate::language::{Language, LanguageDefs};
use crate::person::{Person, PersonId};
use crate::sim::People;

/// What a speech act does (V2.1 §10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Act {
    Greet,
    Introduce,
    Ask,
    Inform,
    Warn,
    Request,
    Offer,
    Accept,
    Refuse,
    Thank,
    Apologise,
    Praise,
    Insult,
    Joke,
    Threaten,
    Command,
    Teach,
    TellStory,
    Gossip,
    Propose,
    ArgueFor,
    ArgueAgainst,
}

/// A gesture, understood without a shared language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Gesture {
    Point,
    Show,
    Offer,
    Beckon,
    Shoo,
    ThreatDisplay,
    Submission,
    Embrace,
    Hands,
}

/// A word of a speech act: a meaning (its id), or a person's name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Word {
    Meaning(String),
    Name(PersonId),
}

/// A speech act said: who said it, to whom, what it does, its words in the speaker's order and
/// the gesture with it, where and when.
#[derive(Debug, Clone, PartialEq)]
pub struct Said {
    pub speaker: PersonId,
    pub to: Option<PersonId>,
    pub act: Act,
    pub words: Vec<Word>,
    pub gesture: Option<Gesture>,
    pub at: DVec3,
    pub day: f64,
}

/// What one knows of a language (V2.1 §10.3): its mother tongue wholly, or each word as far as
/// it has learned it (0–1).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Tongue {
    pub language: u64,
    pub native: bool,
    pub words: Vec<(String, f32)>,
}

impl Tongue {
    /// How well it knows a word.
    pub fn knows(&self, meaning: &str) -> f32 {
        if self.native {
            1.0
        } else {
            self.words
                .iter()
                .find(|(m, _)| m == meaning)
                .map_or(0.0, |(_, f)| *f)
        }
    }
}

/// A speech act as a listener makes it out (V2.1 §10.3).
#[derive(Debug, Clone, PartialEq)]
pub struct Heard {
    /// Who spoke (its name, or "someone"), and whether to the listener.
    pub speaker: String,
    pub to_you: bool,
    /// The speaker's words as they sound.
    pub spoken: String,
    /// What the listener makes of them: words it knows in its own tongue, half-known ones with a
    /// doubt, the rest as dots.
    pub sense: String,
    /// The share of the words it made out.
    pub understood: f32,
    pub gesture: Option<Gesture>,
}

/// How far speech carries to be heard and made out (m).
pub const HEARD_M: f64 = 20.0;
/// How long a step's acts are kept for those who hear them (days: about half a minute).
const KEPT_DAYS: f64 = 0.01;
/// How much nearer to knowing a word one comes, hearing it spoken to one, or overheard (a share
/// of the way left; twice with a gesture making its sense plain).
const LEARN_TO: f32 = 0.1;
const LEARN_OVER: f32 = 0.04;

/// A clause's words by their roles — who does it, the doing, to whom or what — put in a
/// language's order, the rest after.
pub fn clause(
    order: Order,
    subject: Option<&str>,
    verb: Option<&str>,
    object: Option<&str>,
    rest: &[&str],
) -> Vec<Word> {
    let w = |m: Option<&str>| m.map(|m| Word::Meaning(m.to_owned()));
    let (s, v, o) = (w(subject), w(verb), w(object));
    let mut out: Vec<Word> = match order {
        Order::Sov => [s, o, v],
        Order::Svo => [s, v, o],
        Order::Vso => [v, s, o],
        Order::Vos => [v, o, s],
        Order::Ovs => [o, v, s],
    }
    .into_iter()
    .flatten()
    .collect();
    out.extend(rest.iter().map(|m| Word::Meaning((*m).to_owned())));
    out
}

/// Words as meanings, in the order given.
pub fn words(ms: &[&str]) -> Vec<Word> {
    ms.iter().map(|m| Word::Meaning((*m).to_owned())).collect()
}

impl People {
    /// A person says something: kept a little while for whoever hears it. Those of a people
    /// without language say nothing (their calls are the animals' kind).
    pub(crate) fn say(
        &mut self,
        speaker: PersonId,
        to: Option<PersonId>,
        act: Act,
        words: Vec<Word>,
        gesture: Option<Gesture>,
        day: f64,
    ) {
        let Some(i) = self.index_of_person(speaker) else {
            return;
        };
        if self.persons[i].player.is_some() || self.tongue_of(speaker).is_none() {
            return;
        }
        let at = self.persons[i].place.pos;
        self.said.push(Said {
            speaker,
            to,
            act,
            words,
            gesture,
            at,
            day,
        });
    }

    /// Lets go of what was said longer ago than anyone still hears.
    pub(crate) fn forget_said(&mut self, day: f64) {
        self.said.retain(|s| day - s.day < KEPT_DAYS);
    }

    /// The language a person speaks: its band's.
    pub fn tongue_of(&self, who: PersonId) -> Option<&Language> {
        let p = self.get(who)?;
        self.bands
            .iter()
            .find(|b| b.id == p.social.band)
            .and_then(|b| b.culture.language.as_ref())
    }

    /// The word order a person speaks in (verb in the middle for one without a language).
    pub(crate) fn order_of(&self, who: PersonId) -> Order {
        self.tongue_of(who).map_or(Order::Svo, |l| l.order)
    }

    /// A language by its id, as some band speaks it now.
    pub fn language(&self, id: u64) -> Option<&Language> {
        self.bands
            .iter()
            .filter_map(|b| b.culture.language.as_ref())
            .find(|l| l.id == id)
    }

    /// How well a listener knows a word of a language: as it has learned it, or — in a language
    /// related to one it knows — six tenths of how well it knows the cognate.
    pub fn familiarity(&self, listener: &Person, l: &Language, meaning: &str) -> f32 {
        let mut best = 0.0f32;
        for t in &listener.tongues {
            if t.language == l.id {
                best = best.max(t.knows(meaning));
            } else if let Some(k) = self.language(t.language)
                && l.cognate(k, meaning)
            {
                best = best.max(0.6 * t.knows(meaning));
            }
        }
        best
    }

    /// A speech act as a listener makes it out (see the module).
    pub fn heard(&self, d: &LanguageDefs, listener: &Person, s: &Said) -> Option<Heard> {
        let l = self.tongue_of(s.speaker)?;
        let (mut spoken, mut sense) = (Vec::new(), Vec::new());
        let (mut made_out, mut n) = (0.0, 0.0);
        for w in &s.words {
            match w {
                Word::Meaning(m) => {
                    let Some(word) = l.say(d, m) else {
                        continue;
                    };
                    spoken.push(word);
                    n += 1.0;
                    let f = self.familiarity(listener, l, m);
                    let gloss = d.gloss(m).unwrap_or(m);
                    if f >= 0.7 {
                        sense.push(gloss.to_owned());
                        made_out += 1.0;
                    } else if f >= 0.3 {
                        sense.push(format!("{gloss}?"));
                        made_out += 0.5;
                    } else {
                        sense.push("…".to_owned());
                    }
                }
                Word::Name(id) => {
                    let name = self.get(*id).map_or(String::new(), |p| p.name.clone());
                    if !name.is_empty() {
                        spoken.push(name.clone());
                        sense.push(name);
                    }
                }
            }
        }
        let speaker = self
            .get(s.speaker)
            .map(|p| p.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Someone".to_owned());
        Some(Heard {
            speaker,
            to_you: s.to == Some(listener.id),
            spoken: spoken.join(" "),
            sense: sense.join(" "),
            understood: if n > 0.0 { made_out / n } else { 1.0 },
            gesture: s.gesture,
        })
    }

    /// A listener hears a speech act and learns from it: each of its words a little more known —
    /// the more when spoken to it, twice when a gesture makes its sense plain.
    pub fn learn_from(&mut self, listener: PersonId, s: &Said) {
        let Some(lang) = self.tongue_of(s.speaker).map(|l| l.id) else {
            return;
        };
        let Some(i) = self.index_of_person(listener) else {
            return;
        };
        let rate = if s.to == Some(listener) {
            LEARN_TO
        } else {
            LEARN_OVER
        } * if s.gesture.is_some() { 2.0 } else { 1.0 };
        let p = &mut self.persons[i];
        let k = match p.tongues.iter().position(|t| t.language == lang) {
            Some(k) => k,
            None => {
                p.tongues.push(Tongue {
                    language: lang,
                    native: false,
                    words: Vec::new(),
                });
                p.tongues.len() - 1
            }
        };
        let t = &mut p.tongues[k];
        if t.native {
            return;
        }
        for w in &s.words {
            let Word::Meaning(m) = w else {
                continue;
            };
            match t.words.iter_mut().find(|(x, _)| x == m) {
                Some((_, f)) => *f += (1.0 - *f) * rate,
                None => t.words.push((m.clone(), rate)),
            }
        }
    }
}

//! The player among people (V2.1 §16; H9): what a player says and does to a person, and how the
//! person answers as anyone would — greetings and introductions, thanks, apologies, praise,
//! jokes and insults, gestures; asking to be taught and offering to teach; asking to stay among
//! a band; proposing to pair, which takes the other's own willingness (ground rule 3: mutual
//! only). Nothing here is the player's alone: the ties, trust and views it moves are those every
//! person keeps, and a band takes a player in as it takes in any guest (H4).

use hearth_content::schema::knowledge::Route;

use crate::kin::{Kin, kin_of};
use crate::learning::{SHOW_M, TAUGHT_PER_MIN, learn};
use hearth_fauna::live::Stage;

use crate::person::{Event, PersonId, Tier};
use crate::psyche::Feeling;
use crate::sim::People;
use crate::species::SpeciesSet;
use crate::speech::{Act, Gesture, words};
use crate::strangers::Guest;
use crate::world::Now;

/// What a player says or does to a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    Greet,
    /// Tells its name, and asks theirs.
    Introduce,
    Thank,
    Apologise,
    Praise,
    Joke,
    Insult,
    /// A gesture alone.
    Gesture(Gesture),
    /// Asks to be taught: a technique of theirs (`None`: whatever they would show).
    BeTaught(Option<String>),
    /// Offers to teach them a technique the player knows (`None`: one they do not).
    Teach(Option<String>),
    /// Asks to stay among their band.
    Join,
    /// Proposes to pair with them.
    Pair,
}

/// How the person took it: words for the player, and whether it was a yes.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub words: String,
    pub yes: bool,
    /// The technique a lesson agreed on is of.
    pub about: Option<String>,
}

impl Answer {
    fn yes(words: impl Into<String>) -> Self {
        Self {
            words: words.into(),
            yes: true,
            about: None,
        }
    }

    fn no(words: impl Into<String>) -> Self {
        Self {
            words: words.into(),
            yes: false,
            about: None,
        }
    }
}

/// A lesson asked for or offered: a teacher showing a pupil a technique while they keep near,
/// until a day (not saved: a lesson is a matter of an hour or two).
#[derive(Debug, Clone, PartialEq)]
pub struct Lesson {
    pub teacher: PersonId,
    pub pupil: PersonId,
    pub node: String,
    pub until: f64,
}

/// How long a lesson lasts (days: about an hour and a half).
pub const LESSON_DAYS: f64 = 0.06;
/// Asked to, a teacher teaches this many times faster than one merely shown at another's work
/// (V2.1 §11.2: being taught is much faster than finding out).
const ASKED: f32 = 2.0;
/// The trust in the player a person not of its band needs to teach it, or to learn from it.
const TEACH_TRUST: f32 = 0.3;
/// The trust a person needs in the player to speak for its staying among them.
const JOIN_TRUST: f32 = 0.4;
/// How fond and trusting of the player a person must be to pair with it.
const PAIR_FOND: f32 = 0.6;
const PAIR_TRUST: f32 = 0.5;

impl People {
    /// Whether a person's name is known to another: kin, of one band, or told it.
    pub fn knows_name(&self, me: PersonId, other: PersonId) -> bool {
        let (Some(a), Some(b)) = (self.get(me), self.get(other)) else {
            return false;
        };
        a.social.band == b.social.band
            || kin_of(self, me, other).is_some()
            || a.social.ties.iter().any(|t| t.who == other && t.named)
    }

    /// A player says or does something to a person (V2.1 §16): the person answers as its ties to
    /// the player, its people's ways and the player's standing with its band have it. `knows`
    /// says whether the player knows a technique (the player's knowledge is the player's own).
    pub fn player_asks(
        &mut self,
        player: u64,
        to: PersonId,
        ask: Ask,
        knows: &dyn Fn(&str) -> bool,
        species: &SpeciesSet,
        now: &Now,
    ) -> Answer {
        let Some(me) = self.player_person(player).map(|p| p.id) else {
            return Answer::no("You are no one here.");
        };
        let (Some(a), Some(b)) = (self.index_of_person(me), self.index_of_person(to)) else {
            return Answer::no("There is no one there.");
        };
        if a == b || !self.persons[b].alive() || self.persons[b].player.is_some() {
            return Answer::no("They do not answer.");
        }
        let day = now.day;
        let k = self.tie_index(b, me, day);
        let same_band = self.persons[a].social.band == self.persons[b].social.band;
        let them = |s: &People| {
            if s.knows_name(me, to) && !s.persons[b].name.is_empty() {
                s.persons[b].name.clone()
            } else {
                "They".to_owned()
            }
        };
        match ask {
            Ask::Greet => {
                let t = &mut self.persons[b].social.ties[k];
                t.affection += (1.0 - t.affection) * 0.03;
                t.day = day;
                self.say(to, Some(me), Act::Greet, words(&["hello"]), None, day);
                Answer::yes(format!("{} greet you back.", them(self)))
            }
            Ask::Introduce => {
                // Each tells the other its name.
                let t = &mut self.persons[b].social.ties[k];
                t.named = true;
                t.day = day;
                let j = self.tie_index(a, to, day);
                self.persons[a].social.ties[j].named = true;
                self.say(
                    to,
                    Some(me),
                    Act::Introduce,
                    vec![crate::speech::Word::Name(to)],
                    Some(Gesture::Hands),
                    day,
                );
                let name = &self.persons[b].name;
                if name.is_empty() {
                    Answer::yes("They tell you their name with a gesture.")
                } else {
                    Answer::yes(format!("They are called {name}."))
                }
            }
            Ask::Thank | Ask::Praise => {
                let t = &mut self.persons[b].social.ties[k];
                t.affection += (1.0 - t.affection) * 0.05;
                t.respect += (1.0 - t.respect) * 0.02;
                t.day = day;
                self.persons[b].psyche.feel(Feeling::Joy, 0.2);
                self.persons[b].psyche.feel(Feeling::Pride, 0.1);
                Answer::yes(format!("{} are pleased.", them(self)))
            }
            Ask::Apologise => {
                let t = &mut self.persons[b].social.ties[k];
                t.rivalry *= 0.6;
                t.trust += (1.0 - t.trust) * 0.03;
                t.day = day;
                Answer::yes(format!("{} let it go.", them(self)))
            }
            Ask::Joke => {
                // A joke lands with those fond of one; with others it falls flat.
                let fond = self.persons[b].social.ties[k].affection >= 0.4;
                let t = &mut self.persons[b].social.ties[k];
                if fond {
                    t.affection += (1.0 - t.affection) * 0.04;
                    t.day = day;
                    self.persons[b].psyche.feel(Feeling::Joy, 0.3);
                    Answer::yes(format!("{} laugh.", them(self)))
                } else {
                    Answer::no(format!("{} do not laugh.", them(self)))
                }
            }
            Ask::Insult => {
                let t = &mut self.persons[b].social.ties[k];
                t.affection = (t.affection - 0.2).max(0.0);
                t.trust = (t.trust - 0.1).max(0.0);
                t.rivalry = (t.rivalry + 0.15).min(1.0);
                t.day = day;
                self.persons[b].psyche.feel(Feeling::Anger, 0.4);
                self.say(
                    to,
                    Some(me),
                    Act::Insult,
                    words(&["you", "bad"]),
                    Some(Gesture::ThreatDisplay),
                    day,
                );
                Answer::no(format!("{} take it badly.", them(self)))
            }
            Ask::Gesture(g) => {
                let words = match g {
                    Gesture::Embrace | Gesture::Hands | Gesture::Offer => {
                        let t = &mut self.persons[b].social.ties[k];
                        t.affection += (1.0 - t.affection) * 0.02;
                        "They see it, and their face softens."
                    }
                    Gesture::ThreatDisplay | Gesture::Shoo => {
                        let t = &mut self.persons[b].social.ties[k];
                        t.fear = (t.fear + 0.1).min(1.0);
                        t.trust = (t.trust - 0.05).max(0.0);
                        "They draw back, watching you."
                    }
                    Gesture::Submission => {
                        let t = &mut self.persons[b].social.ties[k];
                        t.rivalry *= 0.8;
                        "They relax a little."
                    }
                    Gesture::Beckon | Gesture::Point | Gesture::Show => "They look where you show.",
                };
                Answer::yes(words)
            }
            Ask::BeTaught(node) => {
                let trust = self.persons[b].social.ties[k].trust;
                let teach = species
                    .get(&self.persons[b].species)
                    .and_then(|sp| sp.learning.as_ref())
                    .map_or(1.0, |t| t.teach);
                if teach <= 1.0 {
                    return Answer::no("They do not understand what you want.");
                }
                if !same_band && trust < TEACH_TRUST {
                    self.say(to, Some(me), Act::Refuse, words(&["no"]), None, day);
                    return Answer::no(format!(
                        "{} do not trust you enough to show you.",
                        them(self)
                    ));
                }
                // What they know that the player does not: the one asked for, else the first.
                let theirs = &self.persons[b].knowledge;
                let node = match node {
                    Some(n) if theirs.knows(&n) && !knows(&n) => Some(n),
                    Some(_) => None,
                    None => theirs.known.keys().find(|n| !knows(n)).cloned(),
                };
                let Some(node) = node else {
                    return Answer::no(format!("{} know nothing of that to show you.", them(self)));
                };
                self.lessons.retain(|l| l.pupil != me);
                self.lessons.push(Lesson {
                    teacher: to,
                    pupil: me,
                    node: node.clone(),
                    until: day + LESSON_DAYS,
                });
                self.say(
                    to,
                    Some(me),
                    Act::Teach,
                    words(&["see", "this"]),
                    Some(Gesture::Show),
                    day,
                );
                Answer {
                    about: Some(node),
                    ..Answer::yes(format!("{} agree to show you; keep near them.", them(self)))
                }
            }
            Ask::Teach(node) => {
                let Some(node) = node else {
                    return Answer::no("You know nothing they do not.");
                };
                if !knows(&node) {
                    return Answer::no("You do not know that well enough to teach it.");
                }
                if self.persons[b].knowledge.knows(&node) {
                    return Answer::no(format!("{} know it already.", them(self)));
                }
                let child = matches!(
                    kin_of(self, me, to),
                    Some(Kin::Child) | Some(Kin::Grandchild)
                );
                let trust = self.persons[b].social.ties[k].trust;
                if !same_band && !child && trust < TEACH_TRUST {
                    return Answer::no(format!("{} will not learn from a stranger.", them(self)));
                }
                self.lessons.retain(|l| l.pupil != to);
                self.lessons.push(Lesson {
                    teacher: me,
                    pupil: to,
                    node: node.clone(),
                    until: day + LESSON_DAYS,
                });
                Answer {
                    about: Some(node),
                    ..Answer::yes(format!("{} watch you closely.", them(self)))
                }
            }
            Ask::Join => {
                if same_band {
                    return Answer::no("You are one of them already.");
                }
                let bi = self.band_index_of(self.persons[b].social.band);
                let trust = self.persons[b].social.ties[k].trust;
                let Some(bi) = bi else {
                    return Answer::no("They have no band to take you into.");
                };
                if trust < JOIN_TRUST {
                    self.say(
                        to,
                        Some(me),
                        Act::Refuse,
                        words(&["no"]),
                        Some(Gesture::Shoo),
                        day,
                    );
                    return Answer::no(format!(
                        "{} wave you off: they do not know you well enough yet.",
                        them(self)
                    ));
                }
                if !self.bands[bi].guests.iter().any(|g| g.who == me) {
                    self.bands[bi].guests.push(Guest {
                        who: me,
                        since: day,
                        by: to,
                    });
                }
                let tolerance = self.bands[bi].tolerance_of(player).max(0.6);
                self.bands[bi].set_tolerance(player, tolerance);
                self.say(
                    to,
                    Some(me),
                    Act::Accept,
                    words(&["stay", "here"]),
                    Some(Gesture::Beckon),
                    day,
                );
                Answer::yes(format!(
                    "{} let you stay among them; if they come to trust you, they will take you in.",
                    them(self)
                ))
            }
            Ask::Pair => self.proposed(a, b, k, me, to, species, now),
        }
    }

    /// A proposal to pair (V2.1 §7.1, §16; ground rule 3): taken only by a grown person of the
    /// player's band, unpaired, not its kin, who is fond enough of the player and trusts it.
    #[allow(clippy::too_many_arguments)]
    fn proposed(
        &mut self,
        a: usize,
        b: usize,
        k: usize,
        me: PersonId,
        to: PersonId,
        species: &SpeciesSet,
        now: &Now,
    ) -> Answer {
        let day = now.day;
        let (p, q) = (&self.persons[a], &self.persons[b]);
        let grown = |i: usize| {
            species
                .get(&self.persons[i].species)
                .is_some_and(|sp| self.persons[i].stage(sp, now) == Stage::Adult)
        };
        if !grown(a) || !grown(b) {
            return Answer::no("Neither of you is of an age for that.");
        }
        if p.social.band != q.social.band {
            return Answer::no("They would pair only with one of their own band.");
        }
        if p.life.female == q.life.female {
            return Answer::no("They do not take it so.");
        }
        if p.social.bond.is_some() || q.social.bond.is_some() {
            return Answer::no("One of you is paired already.");
        }
        let in_law = matches!(
            kin_of(self, me, to),
            Some(Kin::ParentInLaw) | Some(Kin::ChildInLaw) | Some(Kin::SiblingInLaw)
        );
        if kin_of(self, me, to).is_some() && !in_law {
            return Answer::no("You are kin: it is not done.");
        }
        let t = &self.persons[b].social.ties[k];
        if t.affection < PAIR_FOND || t.trust < PAIR_TRUST {
            self.say(to, Some(me), Act::Refuse, words(&["no"]), None, day);
            return Answer::no(
                "They are not willing. Perhaps in time, if they come to care for you.",
            );
        }
        self.persons[a].social.bond = Some(to);
        self.persons[b].social.bond = Some(me);
        self.persons[a].record(day, Event::Paired { with: to });
        self.persons[b].record(day, Event::Paired { with: me });
        self.persons[b].psyche.feel(Feeling::Joy, 0.6);
        let bi = self.band_index_of(self.persons[b].social.band);
        if let Some(bi) = bi {
            self.settle_households(bi);
        }
        self.say(
            to,
            Some(me),
            Act::Accept,
            words(&["yes"]),
            Some(Gesture::Embrace),
            day,
        );
        Answer::yes("They are willing. You are a pair now.")
    }

    /// The lessons under way, one step: a lesson's time run out ends it; a person the player
    /// teaches, kept near the player, comes to know the technique as the player's people teach
    /// (H9). What the player is taught is given out by [`People::lessons_for`].
    pub(crate) fn teach_step(&mut self, species: &SpeciesSet, now: &Now, dt: f32) {
        self.lessons.retain(|l| l.until > now.day);
        let lessons = self.lessons.clone();
        for l in lessons {
            let (Some(t), Some(p)) = (
                self.index_of_person(l.teacher),
                self.index_of_person(l.pupil),
            ) else {
                continue;
            };
            if self.persons[p].player.is_some() || !self.persons[p].alive() {
                continue;
            }
            let near = (self.persons[t].place.pos - self.persons[p].place.pos)
                .with_y(0.0)
                .length()
                <= SHOW_M;
            if !near || self.persons[p].tier != Tier::Full {
                continue;
            }
            let teach = species
                .get(&self.persons[p].species)
                .and_then(|sp| sp.learning.as_ref())
                .map_or(1.0, |x| x.teach);
            let gain = TAUGHT_PER_MIN * teach.max(1.0) * ASKED * dt / 60.0;
            let q = &mut self.persons[p];
            let had = q.knowledge.insight.get(&l.node).copied().unwrap_or(0.0);
            if had + gain >= 1.0 {
                learn(q, &species.lore, &l.node, Route::Taught, now.tick);
                let pupil = q.id;
                self.lessons
                    .retain(|x| !(x.pupil == pupil && x.node == l.node));
                self.say(
                    pupil,
                    Some(l.teacher),
                    Act::Thank,
                    words(&["thanks"]),
                    Some(Gesture::Hands),
                    now.day,
                );
            } else {
                q.knowledge.insight.insert(l.node.clone(), had + gain);
            }
        }
    }

    /// The insight a lesson the player asked for gives it this step, while it keeps near its
    /// teacher: the technique and the insight.
    pub(crate) fn asked_lessons(
        &self,
        me: PersonId,
        eye: glam::DVec3,
        species: &SpeciesSet,
        now: &Now,
        dt: f32,
    ) -> Vec<(String, f32)> {
        self.lessons
            .iter()
            .filter(|l| l.pupil == me && l.until > now.day)
            .filter_map(|l| {
                let t = self.get(l.teacher)?;
                if !t.alive() || (t.place.pos - eye).with_y(0.0).length() > SHOW_M {
                    return None;
                }
                let teach = species
                    .get(&t.species)
                    .and_then(|sp| sp.learning.as_ref())
                    .map_or(1.0, |x| x.teach);
                Some((
                    l.node.clone(),
                    TAUGHT_PER_MIN * teach.max(1.0) * ASKED * dt / 60.0,
                ))
            })
            .collect()
    }

    /// What a player knows of a person it looks at (V2.1 §16: no stat sheets) — its name if it
    /// has learned it, else what it can see; their kinship, band and pairing; how they seem to
    /// take the player; what the player has heard of them.
    pub fn regard(
        &self,
        player: u64,
        id: PersonId,
        species: &SpeciesSet,
        now: &Now,
    ) -> Vec<String> {
        let (Some(me), Some(q)) = (self.player_person(player), self.get(id)) else {
            return Vec::new();
        };
        if !q.alive() {
            return vec!["Dead.".to_owned()];
        }
        let grown = species
            .get(&q.species)
            .map(|sp| q.stage(sp, now))
            .unwrap_or(Stage::Adult);
        let seen = match (grown, q.life.female) {
            (Stage::Adult, true) => "a woman",
            (Stage::Adult, false) => "a man",
            (_, true) => "a girl",
            (_, false) => "a boy",
        };
        let mut lines = Vec::new();
        let kin = kin_of(self, me.id, id);
        let named = self.knows_name(me.id, id) && !q.name.is_empty();
        let mut first = if named { q.name.clone() } else { capital(seen) };
        if let Some(k) = kin {
            first = format!("{first}, your {}", k.word(q.life.female));
        } else if q.social.band == me.social.band {
            first = format!("{first}, of your band");
        } else {
            first = format!("{first}, a stranger");
        }
        lines.push(first);
        if q.social.bond == Some(me.id) {
            lines.push("Your partner.".to_owned());
        } else if q.social.bond.is_some() && grown == Stage::Adult {
            lines.push("Paired.".to_owned());
        }
        if let Some(t) = q.social.ties.iter().find(|t| t.who == me.id) {
            let mood = if t.rivalry > 0.4 || t.affection < 0.1 {
                "They seem at odds with you."
            } else if t.affection >= 0.6 {
                "They seem fond of you."
            } else if t.trust < 0.2 {
                "They seem wary of you."
            } else {
                "They seem at ease with you."
            };
            lines.push(mood.to_owned());
        }
        // The ledger between them, as the player keeps it (V2.1 §8.3): what is owed.
        if let Some(t) = me.social.ties.iter().find(|t| t.who == id) {
            if t.owed >= 0.3 {
                lines.push("You owe them for what they have given you.".to_owned());
            } else if t.given >= 0.3 {
                lines.push("They owe you for what you have given them.".to_owned());
            }
        }
        if let Some(r) = me
            .social
            .reputes
            .iter()
            .find(|r| r.about == crate::memory::Who::Person(id) && r.sure > 0.2)
        {
            if r.generous >= 0.3 {
                lines.push("You have heard they are generous.".to_owned());
            } else if r.generous <= -0.3 {
                lines.push("You have heard they do not share.".to_owned());
            }
            if r.honest <= -0.3 {
                lines.push("You have heard they take what is not theirs.".to_owned());
            }
        }
        lines
    }

    /// A band's index by its id.
    pub(crate) fn band_index_of(&self, id: u64) -> Option<usize> {
        self.bands.iter().position(|b| b.id == id)
    }
}

/// A phrase with its first letter a capital.
fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

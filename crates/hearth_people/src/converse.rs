//! What a language model may be told of a person who speaks, and what its line is held to (V2.1
//! §10.4; H10). When the optional conversation backend phrases a speech act, it is told the
//! speaker's own state and nothing else: who it is and how old, its nature, what it feels now,
//! what it and its people hold to, the techniques it knows, its kin, what has befallen it lately,
//! whom it speaks to and how it stands with them, the names it may say — and the act, in its
//! words. Alongside goes a guard the line is checked against before anyone sees it: the
//! techniques the speaker knows, the names it may say and those of everyone it does not know, the
//! kin it has. Nothing here changes the world: it is read from the people and handed out.

use std::collections::BTreeSet;

use hearth_craft::knowledge::Graph;
use hearth_fauna::live::Stage;

use crate::kin::{Kin, kin_of};
use crate::memory::Happened;
use crate::person::{Person, PersonId};
use crate::psyche::{Feeling, Tendency, Value};
use crate::sim::People;
use crate::species::SpeciesSet;
use crate::speech::{Act, Gesture, Said, Word};
use crate::world::Now;

/// What a model is told of a speaker as it phrases one of its speech acts: its own state, in
/// words.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Context {
    pub speaker: PersonId,
    /// Its name, its sex and age as one sees it ("Tuka, a woman in the prime of life").
    pub who: String,
    /// Its nature: its strongest tendencies in words.
    pub temperament: Vec<String>,
    /// What it feels strongly now.
    pub feelings: Vec<String>,
    /// What it and its people hold to.
    pub values: Vec<String>,
    /// The techniques it knows, by name.
    pub knows: Vec<String>,
    /// Its kin it knows of ("your mother, Tana").
    pub kin: Vec<String>,
    /// What has befallen it lately.
    pub recent: Vec<String>,
    /// Whom it speaks to and how it stands with them (none: no one in particular).
    pub listener: String,
    /// The names it may say.
    pub names: Vec<String>,
    /// The act, in words ("a greeting").
    pub act: String,
    /// The act's words as the speaker means them, in English glosses ("hello friend").
    pub meaning: String,
    /// The gesture with it, in words ("holding it out").
    pub gesture: Option<String>,
}

/// What a phrased line is held to (never shown to the model).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Guard {
    /// The speech act phrased.
    pub act: Option<Act>,
    /// The knowledge nodes the speaker knows (their ids).
    pub known: BTreeSet<String>,
    /// The names it may say, as written.
    pub names: BTreeSet<String>,
    /// The names of everyone else in the world, as written: never said.
    pub unknown_names: BTreeSet<String>,
    /// The kin words it may say of kin it has ("sister" from one with a sister).
    pub kin: BTreeSet<String>,
}

/// How strongly a feeling must be felt to be told (0–1).
const FELT: f32 = 0.25;
/// How far from the middle a tendency or value must lie to be told.
const MARKED: f32 = 0.2;
/// The most techniques told (the rest are known, and allowed, but not listed).
const KNOWS_TOLD: usize = 40;
/// The most happenings told, and how long ago they may be (days).
const RECENT_TOLD: usize = 3;
const RECENT_DAYS: f64 = 64.0;

/// A speech act in words.
pub fn act_words(act: Act) -> &'static str {
    match act {
        Act::Greet => "a greeting",
        Act::Introduce => "telling your name",
        Act::Ask => "a question",
        Act::Inform => "telling them something",
        Act::Warn => "a warning",
        Act::Request => "asking for something",
        Act::Offer => "an offer",
        Act::Accept => "saying yes",
        Act::Refuse => "saying no",
        Act::Thank => "thanks",
        Act::Apologise => "saying sorry",
        Act::Praise => "praise",
        Act::Insult => "an insult",
        Act::Joke => "a joke",
        Act::Threaten => "a threat",
        Act::Command => "telling them what to do",
        Act::Teach => "showing them how",
        Act::TellStory => "telling a story",
        Act::Gossip => "talk of someone",
        Act::Propose => "a proposal",
        Act::ArgueFor => "arguing for a choice",
        Act::ArgueAgainst => "arguing against a choice",
    }
}

/// A gesture in words.
pub fn gesture_words(g: Gesture) -> &'static str {
    match g {
        Gesture::Point => "pointing",
        Gesture::Show => "showing it",
        Gesture::Offer => "holding it out",
        Gesture::Beckon => "beckoning",
        Gesture::Shoo => "waving them off",
        Gesture::ThreatDisplay => "drawing yourself up to threaten",
        Gesture::Submission => "with your head bowed",
        Gesture::Embrace => "with open arms",
        Gesture::Hands => "with open hands",
    }
}

fn tendency_words(t: Tendency, high: bool) -> Option<&'static str> {
    Some(match (t, high) {
        (Tendency::RiskTolerance, true) => "bold",
        (Tendency::RiskTolerance, false) => "careful",
        (Tendency::Curiosity, true) => "curious",
        (Tendency::Curiosity, false) => "content with what you know",
        (Tendency::Patience, true) => "patient",
        (Tendency::Patience, false) => "impatient",
        (Tendency::Cooperativeness, true) => "helpful",
        (Tendency::Cooperativeness, false) => "one who keeps to yourself",
        (Tendency::TrustStrangers, true) => "trusting of strangers",
        (Tendency::TrustStrangers, false) => "wary of strangers",
        (Tendency::Conformity, true) => "one who keeps to custom",
        (Tendency::Conformity, false) => "one who goes your own way",
        (Tendency::Reactivity, true) => "quick to feel",
        (Tendency::Reactivity, false) => "calm",
        (Tendency::AggressionThreshold, true) => "slow to anger",
        (Tendency::AggressionThreshold, false) => "quick to anger",
        (Tendency::Forgiveness, true) => "forgiving",
        (Tendency::Forgiveness, false) => "slow to forgive",
        (Tendency::Diligence, true) => "hard working",
        (Tendency::Diligence, false) => "easy going",
        (Tendency::Sociability, true) => "fond of company",
        (Tendency::Sociability, false) => "quiet",
        (Tendency::Dominance, true) => "forceful",
        (Tendency::Dominance, false) => "mild",
        (Tendency::PrestigeBias, _) => return None,
    })
}

fn feeling_words(f: Feeling) -> &'static str {
    match f {
        Feeling::Fear => "afraid",
        Feeling::Anger => "angry",
        Feeling::Grief => "grieving",
        Feeling::Shame => "ashamed",
        Feeling::Guilt => "guilty",
        Feeling::Indignation => "wronged",
        Feeling::Joy => "glad",
        Feeling::Pride => "proud",
        Feeling::Disgust => "sickened",
        Feeling::Affection => "warm toward those near you",
    }
}

fn value_words(v: Value) -> &'static str {
    match v {
        Value::KinLoyalty => "kin come first",
        Value::Generosity => "food and help are to be shared",
        Value::Honor => "a wrong must be answered",
        Value::Autonomy => "each is free to choose",
        Value::Deference => "elders are to be heeded",
        Value::Piety => "the spirits are to be respected",
        Value::Fairness => "all are to be dealt with fairly",
        Value::Courage => "one must be brave",
    }
}

/// One's age as it is plain to see.
fn seen_as(p: &Person, species: &SpeciesSet, now: &Now) -> &'static str {
    let grown = species
        .get(&p.species)
        .map(|sp| p.stage(sp, now))
        .unwrap_or(Stage::Adult);
    match (grown, p.life.female) {
        (Stage::Adult, female) => {
            let age = p.age(now);
            match (age, female) {
                (a, true) if a < 25.0 => "a young woman",
                (a, false) if a < 25.0 => "a young man",
                (a, true) if a < 45.0 => "a woman in the prime of life",
                (a, false) if a < 45.0 => "a man in the prime of life",
                (a, true) if a < 60.0 => "a woman getting on in years",
                (a, false) if a < 60.0 => "a man getting on in years",
                (_, true) => "an old woman",
                (_, false) => "an old man",
            }
        }
        (_, true) => "a girl",
        (_, false) => "a boy",
    }
}

/// The plural of a kin word ("sons").
fn kin_plural(w: &str) -> String {
    match w {
        "wife" => "wives".to_owned(),
        "grandchild" => "grandchildren".to_owned(),
        w if w.contains("-in-law") => w.replacen("-in-law", "s-in-law", 1),
        w => format!("{w}s"),
    }
}

impl People {
    /// What a model may be told of a speaker as it phrases `said` (V2.1 §10.4), and the guard its
    /// line is held to; none for one gone, or a people without words.
    pub fn context_for(
        &self,
        said: &Said,
        graph: &Graph,
        species: &SpeciesSet,
        now: &Now,
    ) -> Option<(Context, Guard)> {
        let p = self.get(said.speaker)?;
        let defs = species.get(&p.species)?.language.as_ref()?;
        let mut names: BTreeSet<String> = BTreeSet::new();
        let name = |id: PersonId, names: &mut BTreeSet<String>| -> Option<String> {
            let q = self.get(id)?;
            (!q.name.is_empty() && (id == p.id || self.knows_name(p.id, id))).then(|| {
                names.insert(q.name.clone());
                q.name.clone()
            })
        };
        let me = name(p.id, &mut names);
        let seen = seen_as(p, species, now);
        let who = me.map_or_else(|| seen.to_owned(), |n| format!("{n}, {seen}"));

        let ps = &p.psyche;
        let mut temperament = Vec::new();
        for &t in Tendency::ALL {
            let v = ps.tendency(t);
            if (v - 0.5).abs() >= MARKED
                && let Some(w) = tendency_words(t, v > 0.5)
            {
                temperament.push(w.to_owned());
            }
        }
        let mut feelings: Vec<(f32, &str)> = Feeling::ALL
            .iter()
            .map(|&f| (ps.feeling(f), feeling_words(f)))
            .filter(|(v, _)| *v >= FELT)
            .collect();
        feelings.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut feelings: Vec<String> = feelings.into_iter().map(|(_, w)| w.to_owned()).collect();
        if feelings.is_empty() {
            feelings.push(
                if ps.mood > 0.3 {
                    "at ease"
                } else if ps.mood < -0.3 {
                    "low"
                } else {
                    "nothing much"
                }
                .to_owned(),
            );
        }
        let mut values: Vec<String> = Value::ALL
            .iter()
            .filter(|&&v| ps.value(v) >= 0.5 + MARKED)
            .map(|&v| value_words(v).to_owned())
            .collect();
        if let Some(c) = self
            .bands
            .iter()
            .find(|b| b.id == p.social.band)
            .map(|b| &b.culture)
        {
            let v = &c.values;
            let mut hold = |cond: bool, w: &str| {
                if cond {
                    values.push(w.to_owned());
                }
            };
            hold(v.hierarchy >= 0.6, "some lead and the rest follow");
            hold(v.hierarchy <= 0.25, "no one stands above the rest");
            hold(v.wide >= 0.65, "help is owed beyond one's kin");
            hold(v.wide <= 0.3, "kin come before all others");
            hold(v.honour >= 0.6, "a slight is not let go");
            hold(v.honour <= 0.25, "quarrels are best mended");
            hold(v.tight >= 0.65, "the old ways are to be kept");
            hold(v.tight <= 0.3, "each may go their own way");
        }

        // What it knows.
        let known: BTreeSet<String> = p.knowledge.known.keys().cloned().collect();
        let mut knows: Vec<String> = known
            .iter()
            .filter_map(|id| graph.node(id))
            .filter(|n| n.implemented)
            .map(|n| n.name.clone())
            .collect();
        knows.truncate(KNOWS_TOLD);

        // Its kin, and the words it may say of them.
        let mut kin_words: BTreeSet<String> = BTreeSet::new();
        let mut kin: Vec<(u8, String)> = Vec::new();
        for q in &self.persons {
            let Some(k) = kin_of(self, p.id, q.id) else {
                continue;
            };
            let w = k.word(q.life.female);
            kin_words.insert(w.to_owned());
            kin_words.insert(kin_plural(w));
            if matches!(k, Kin::Partner) {
                kin_words.insert(if q.life.female { "wife" } else { "husband" }.to_owned());
                kin_words.insert("mate".to_owned());
            }
            if matches!(k, Kin::Parent) {
                kin_words.insert("parent".to_owned());
                kin_words.insert("parents".to_owned());
            }
            if matches!(k, Kin::HalfSibling) {
                kin_words.insert(if q.life.female { "sister" } else { "brother" }.to_owned());
            }
            if matches!(k, Kin::Grandchild) {
                kin_words.insert("grandchild".to_owned());
                kin_words.insert("grandchildren".to_owned());
            }
            let alive = if q.alive() { "" } else { ", dead now" };
            let line = match name(q.id, &mut names) {
                Some(n) => format!("your {w}, {n}{alive}"),
                None => format!("your {w}{alive}"),
            };
            kin.push((k.nearness(), line));
        }
        kin.sort();
        let kin: Vec<String> = kin.into_iter().take(12).map(|(_, l)| l).collect();

        // Lately.
        let mut episodes: Vec<_> = p
            .memory
            .episodes
            .iter()
            .filter(|e| now.day - e.day <= RECENT_DAYS && e.weight >= 0.2)
            .collect();
        episodes.sort_by(|a, b| b.day.total_cmp(&a.day));
        let mut recent = Vec::new();
        for e in episodes.into_iter().take(RECENT_TOLD) {
            let when = match now.day - e.day {
                d if d < 1.0 => "today",
                d if d < 8.0 => "a few days ago",
                _ => "this season",
            };
            let what = match e.what {
                Happened::Threat => "you saw a hunting beast near".to_owned(),
                Happened::Hurt => "you were hurt".to_owned(),
                Happened::Loss { who } => {
                    let k = kin_of(self, p.id, who)
                        .zip(self.get(who))
                        .map(|(k, q)| k.word(q.life.female));
                    match (k, name(who, &mut names)) {
                        (Some(k), Some(n)) => format!("your {k} {n} died"),
                        (Some(k), None) => format!("your {k} died"),
                        _ => "one of your kin died".to_owned(),
                    }
                }
                Happened::Met { .. } => "you met a stranger".to_owned(),
                Happened::Feast => "you ate your fill".to_owned(),
            };
            recent.push(format!("{what} {when}"));
        }

        // Whom it speaks to.
        let listener = match said.to.and_then(|id| self.get(id)) {
            None => "no one in particular: those about you".to_owned(),
            Some(q) => {
                let seen = seen_as(q, species, now);
                let named = name(q.id, &mut names);
                let mut s = match kin_of(self, p.id, q.id) {
                    Some(k) => {
                        let w = k.word(q.life.female);
                        match &named {
                            Some(n) => format!("{n}, your {w}"),
                            None => format!("your {w}"),
                        }
                    }
                    None if q.social.band == p.social.band => match &named {
                        Some(n) => format!("{n}, {seen} of your band"),
                        None => format!("{seen} of your band"),
                    },
                    None => match &named {
                        Some(n) => format!("{n}, {seen} not of your band"),
                        None => format!("{seen}, a stranger"),
                    },
                };
                if let Some(t) = p.social.ties.iter().find(|t| t.who == q.id) {
                    let mut how = Vec::new();
                    if t.affection >= 0.6 {
                        how.push("you are fond of them");
                    } else if t.affection < 0.1 {
                        how.push("you do not like them");
                    }
                    if t.trust >= 0.6 {
                        how.push("you trust them");
                    } else if t.trust < 0.2 {
                        how.push("you are wary of them");
                    }
                    if t.rivalry > 0.4 {
                        how.push("you are at odds with them");
                    }
                    if t.fear > 0.4 {
                        how.push("you fear them");
                    }
                    if t.respect >= 0.6 {
                        how.push("you respect them");
                    }
                    if t.owed >= 0.3 {
                        how.push("you owe them for what they gave you");
                    } else if t.given >= 0.3 {
                        how.push("they owe you for what you gave them");
                    }
                    if !how.is_empty() {
                        s = format!("{s}; {}", how.join(", "));
                    }
                }
                s
            }
        };

        // The act, in its words.
        let mut meaning = Vec::new();
        for w in &said.words {
            match w {
                Word::Meaning(m) => meaning.push(defs.gloss(m).unwrap_or(m).to_owned()),
                Word::Name(id) => {
                    if let Some(n) = self.get(*id).map(|q| q.name.clone())
                        && !n.is_empty()
                    {
                        names.insert(n.clone());
                        meaning.push(n);
                    }
                }
            }
        }

        let unknown_names: BTreeSet<String> = self
            .persons
            .iter()
            .filter(|q| !q.name.is_empty() && !names.contains(&q.name))
            .map(|q| q.name.clone())
            .collect();
        let context = Context {
            speaker: p.id,
            who,
            temperament,
            feelings,
            values,
            knows,
            kin,
            recent,
            listener,
            names: names.iter().cloned().collect(),
            act: act_words(said.act).to_owned(),
            meaning: meaning.join(" "),
            gesture: said.gesture.map(|g| gesture_words(g).to_owned()),
        };
        let guard = Guard {
            act: Some(said.act),
            known,
            names,
            unknown_names,
            kin: kin_words,
        };
        Some((context, guard))
    }
}

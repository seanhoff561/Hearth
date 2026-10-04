//! The psyche (V2.1 §5; H2): a person's tendencies — personality as it shows in what they
//! choose — their feelings and mood, their stress and their values. Tendencies and values are
//! reckoned from the phenotype's temperament by the content's curves (development shifts them in
//! H3, culture in H5); feelings are felt as events are appraised against what the person wants
//! and holds dear, fade each at its own pace, are caught from those seen feeling them, and show
//! on the body; the mood follows the feelings and the body's state through the day; stress builds
//! from what threatens or wears on one for long, and eases slowly.

use std::collections::BTreeMap;

use hearth_content::Content;
use hearth_content::schema::psyche::{Display, Fade, Lean};
use serde::{Deserialize, Serialize};

use crate::genome::Phenotype;

/// A closed set of kinds the minds know, each with its content id, and one value for each —
/// saved as a map by id, so kinds added later read as nothing from older saves.
macro_rules! kinds {
    ($(#[$km:meta])* $kind:ident, $(#[$vm:meta])* $values:ident { $($v:ident => $key:literal),+ $(,)? }) => {
        $(#[$km])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub enum $kind {
            $($v),+
        }

        impl $kind {
            pub const ALL: &'static [$kind] = &[$($kind::$v),+];

            /// Its content id (without the namespace).
            pub fn key(self) -> &'static str {
                match self {
                    $($kind::$v => $key),+
                }
            }

            /// The kind of a content id (`namespace:key` or `key`).
            pub fn of_id(id: &str) -> Option<Self> {
                let key = id.rsplit(':').next().unwrap_or(id);
                Self::ALL.iter().copied().find(|k| k.key() == key)
            }
        }

        $(#[$vm])*
        #[derive(Debug, Clone, Copy, PartialEq, Default)]
        pub struct $values(pub [f32; $kind::ALL.len()]);

        impl std::ops::Index<$kind> for $values {
            type Output = f32;
            fn index(&self, k: $kind) -> &f32 {
                &self.0[k as usize]
            }
        }

        impl std::ops::IndexMut<$kind> for $values {
            fn index_mut(&mut self, k: $kind) -> &mut f32 {
                &mut self.0[k as usize]
            }
        }

        impl $values {
            /// Every kind with its value.
            pub fn iter(&self) -> impl Iterator<Item = ($kind, f32)> + '_ {
                $kind::ALL.iter().map(move |k| (*k, self[*k]))
            }
        }

        impl Serialize for $values {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                let map: BTreeMap<&str, f32> = self.iter().map(|(k, v)| (k.key(), v)).collect();
                map.serialize(s)
            }
        }

        impl<'de> Deserialize<'de> for $values {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let map: BTreeMap<String, f32> = BTreeMap::deserialize(d)?;
                let mut out = $values::default();
                for (k, v) in map {
                    if let Some(kind) = $kind::of_id(&k) {
                        out[kind] = v;
                    }
                }
                Ok(out)
            }
        }
    };
}

kinds! {
    /// A behaviour tendency (V2.1 §5.1).
    Tendency,
    /// A person's tendencies, 0–1 each.
    Tendencies {
        RiskTolerance => "risk_tolerance",
        Curiosity => "curiosity",
        Patience => "patience",
        Cooperativeness => "cooperativeness",
        TrustStrangers => "trust_strangers",
        Conformity => "conformity",
        PrestigeBias => "prestige_bias",
        Reactivity => "reactivity",
        AggressionThreshold => "aggression_threshold",
        Forgiveness => "forgiveness",
        Diligence => "diligence",
        Sociability => "sociability",
        Dominance => "dominance",
    }
}

kinds! {
    /// A feeling (V2.1 §5.2).
    Feeling,
    /// How strongly each feeling is felt, 0–1 (or, as gains, how strongly it is felt to a
    /// middling person's 1).
    Feelings {
        Fear => "fear",
        Anger => "anger",
        Grief => "grief",
        Shame => "shame",
        Guilt => "guilt",
        Indignation => "indignation",
        Joy => "joy",
        Pride => "pride",
        Disgust => "disgust",
        Affection => "affection",
    }
}

kinds! {
    /// A value (V2.1 §5.3).
    Value,
    /// The weight a person gives each value, 0–1.
    Values {
        KinLoyalty => "kin_loyalty",
        Generosity => "generosity",
        Honor => "honor",
        Autonomy => "autonomy",
        Deference => "deference",
        Piety => "piety",
        Fairness => "fairness",
        Courage => "courage",
    }
}

/// A curve from temperament: a middle, a range, and how far it moves per standard deviation of
/// the traits it leans on.
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    pub middle: f32,
    pub range: (f32, f32),
    pub spread: f32,
    pub from: Vec<(String, f32)>,
}

impl Default for Curve {
    fn default() -> Self {
        Self {
            middle: 0.5,
            range: (0.0, 1.0),
            spread: 0.0,
            from: Vec::new(),
        }
    }
}

fn leans(from: &[Lean]) -> Vec<(String, f32)> {
    from.iter().map(|l| (l.of.to_string(), l.weight)).collect()
}

impl Curve {
    /// Its value for a phenotype (the middle without one).
    pub fn of(&self, ph: Option<&Phenotype>) -> f32 {
        let lean: f32 = ph.map_or(0.0, |ph| self.from.iter().map(|(id, w)| w * ph.z(id)).sum());
        (self.middle + self.spread * lean).clamp(self.range.0, self.range.1)
    }
}

/// A feeling as the content has it.
#[derive(Debug, Clone, PartialEq)]
pub struct FeelingDef {
    pub valence: f32,
    pub fade: Fade,
    pub contagion: f32,
    pub display: Display,
    pub reactivity: Vec<(String, f32)>,
}

impl Default for FeelingDef {
    fn default() -> Self {
        Self {
            valence: 0.0,
            fade: Fade::Seconds(60.0),
            contagion: 0.0,
            display: Display::None,
            reactivity: Vec::new(),
        }
    }
}

/// The content's psyche: the curves of the tendencies and values and the feelings' ways.
#[derive(Debug, Clone, PartialEq)]
pub struct PsycheDefs {
    pub tendencies: Vec<Curve>,
    pub feelings: Vec<FeelingDef>,
    pub values: Vec<Curve>,
}

impl Default for PsycheDefs {
    fn default() -> Self {
        Self {
            tendencies: vec![Curve::default(); Tendency::ALL.len()],
            feelings: vec![FeelingDef::default(); Feeling::ALL.len()],
            values: vec![Curve::default(); Value::ALL.len()],
        }
    }
}

impl PsycheDefs {
    /// The content's tendencies, feelings and values (a kind the content lacks keeps a neutral
    /// one).
    pub fn from_content(c: &Content) -> Self {
        let mut d = PsycheDefs::default();
        for t in c.tendencies.iter() {
            if let Some(k) = Tendency::of_id(&t.id) {
                d.tendencies[k as usize] = Curve {
                    middle: t.middle,
                    range: t.range,
                    spread: t.spread,
                    from: leans(&t.from),
                };
            }
        }
        for f in c.feelings.iter() {
            if let Some(k) = Feeling::of_id(&f.id) {
                d.feelings[k as usize] = FeelingDef {
                    valence: f.valence,
                    fade: f.half_life,
                    contagion: f.contagion,
                    display: f.display,
                    reactivity: leans(&f.reactivity),
                };
            }
        }
        for v in c.values.iter() {
            if let Some(k) = Value::of_id(&v.id) {
                d.values[k as usize] = Curve {
                    middle: v.default,
                    range: (0.0, 1.0),
                    spread: v.spread,
                    from: leans(&v.from),
                };
            }
        }
        d
    }

    pub fn feeling(&self, f: Feeling) -> &FeelingDef {
        &self.feelings[f as usize]
    }
}

/// The weakest a feeling shows on the body.
pub const SHOWS: f32 = 0.2;
/// The most of a feeling seen that is caught from it.
const CAUGHT: f32 = 0.5;
/// How long the mood takes to follow (days of the world, to two thirds of the way).
const MOOD_DAYS: f64 = 0.3;
/// How long stress takes to build toward its load, and to ease (days).
const STRESS_BUILD_DAYS: f64 = 2.0;
const STRESS_EASE_DAYS: f64 = 5.0;

/// A person's psyche.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Psyche {
    /// Reckoned from the phenotype (a record from before psyches is reckoned when next drawn out).
    pub formed: bool,
    pub tendencies: Tendencies,
    pub values: Values,
    /// How strongly each feeling is felt, to a middling person's 1.
    pub gains: Feelings,
    /// Each feeling now, 0–1.
    pub feelings: Feelings,
    /// −1 low … 1 high: the feelings and the body's state over the last while.
    pub mood: f32,
    /// 0–1: what has threatened or worn on it for long.
    pub stress: f32,
    /// 0–1: how little it had a carer to hold to as an infant (its mother away or dead): it
    /// carries stress more heavily and longer all its life.
    pub insecure: f32,
    /// The day of the world it was last reckoned at.
    pub day: f64,
}

impl Default for Psyche {
    fn default() -> Self {
        Self {
            formed: false,
            tendencies: Tendencies([0.5; Tendency::ALL.len()]),
            values: Values([0.5; Value::ALL.len()]),
            gains: Feelings([1.0; Feeling::ALL.len()]),
            feelings: Feelings::default(),
            mood: 0.0,
            stress: 0.0,
            insecure: 0.0,
            day: 0.0,
        }
    }
}

impl Psyche {
    /// One's psyche from their phenotype's temperament (a middling one without a phenotype).
    pub fn form(defs: &PsycheDefs, ph: Option<&Phenotype>, day: f64) -> Self {
        let mut p = Psyche {
            formed: true,
            day,
            ..Psyche::default()
        };
        for &t in Tendency::ALL {
            p.tendencies[t] = defs.tendencies[t as usize].of(ph);
        }
        for &v in Value::ALL {
            p.values[v] = defs.values[v as usize].of(ph);
        }
        for &f in Feeling::ALL {
            let lean: f32 = ph.map_or(0.0, |ph| {
                defs.feeling(f)
                    .reactivity
                    .iter()
                    .map(|(id, w)| w * ph.z(id))
                    .sum()
            });
            p.gains[f] = (1.0 + lean).clamp(0.4, 1.8);
        }
        p
    }

    pub fn tendency(&self, t: Tendency) -> f32 {
        self.tendencies[t]
    }

    pub fn value(&self, v: Value) -> f32 {
        self.values[v]
    }

    pub fn feeling(&self, f: Feeling) -> f32 {
        self.feelings[f]
    }

    /// A feeling stirred: `strength` (0–1) is what the event would stir in one of middling
    /// temperament; it rises to that, as strongly as this person feels it.
    pub fn feel(&mut self, f: Feeling, strength: f32) {
        let felt = (strength * self.gains[f]).clamp(0.0, 1.0);
        if felt > self.feelings[f] {
            self.feelings[f] = felt;
        }
    }

    /// Another seen feeling `f` at `seen` (0–1, less the farther they are): some of it is caught,
    /// the more by one who feels that feeling strongly, over `dt` seconds of play — at most
    /// half as much as is seen, so a feeling passed on weakens with each passing and dies away
    /// without its cause.
    pub fn catch(&mut self, defs: &PsycheDefs, f: Feeling, seen: f32, dt: f32) {
        let gap = CAUGHT * seen * self.gains[f] - self.feelings[f];
        if gap <= 0.0 {
            return;
        }
        let rate = (defs.feeling(f).contagion * dt).min(1.0);
        self.feelings[f] = (self.feelings[f] + gap * rate).clamp(0.0, 1.0);
    }

    /// Time passing — `dt` seconds of play, and the world's day now: the feelings fade, the mood
    /// follows them and the body's strain (0–1: hunger, thirst, weariness, pain), stress builds
    /// while fear, grief or strain last and eases when they are gone.
    pub fn pass(&mut self, defs: &PsycheDefs, dt: f32, day: f64, strain: f32) {
        let days = (day - self.day).clamp(0.0, 30.0);
        self.day = day;
        for &f in Feeling::ALL {
            let keep = match defs.feeling(f).fade {
                Fade::Seconds(h) => 0.5f32.powf(dt / h.max(0.01)),
                Fade::Days(h) => 0.5f64.powf(days / h.max(1e-4) as f64) as f32,
            };
            self.feelings[f] *= keep;
            if self.feelings[f] < 1e-3 {
                self.feelings[f] = 0.0;
            }
        }
        let felt: f32 = Feeling::ALL
            .iter()
            .map(|&f| defs.feeling(f).valence * self.feelings[f])
            .sum();
        let toward = (felt - strain).clamp(-1.0, 1.0);
        let follow = 1.0 - (-days / MOOD_DAYS).exp() as f32;
        self.mood += (toward - self.mood) * follow;
        let load = ((self.feelings[Feeling::Fear] + self.feelings[Feeling::Grief] + strain)
            * (1.0 + 0.5 * self.insecure))
            .min(1.0);
        let k = if load > self.stress {
            STRESS_BUILD_DAYS
        } else {
            STRESS_EASE_DAYS * (1.0 + self.insecure as f64)
        };
        self.stress += (load - self.stress) * (1.0 - (-days / k).exp() as f32);
    }

    /// The feeling it shows, and how strongly: the strongest that shows on the body at all.
    pub fn shown(&self, defs: &PsycheDefs) -> Option<(Feeling, f32)> {
        self.feelings
            .iter()
            .filter(|(f, v)| *v >= SHOWS && defs.feeling(*f).display != Display::None)
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_save_by_name_and_read_what_they_know() {
        let mut f = Feelings::default();
        f[Feeling::Fear] = 0.5;
        f[Feeling::Joy] = 0.25;
        let json = serde_json::to_string(&f).expect("json");
        assert!(json.contains("\"fear\":0.5"), "{json}");
        let back: Feelings = serde_json::from_str(&json).expect("back");
        assert_eq!(back, f);
        // A kind it does not know is passed over.
        let odd: Feelings = serde_json::from_str(r#"{"fear":0.3,"wonder":0.9}"#).expect("odd");
        assert_eq!(odd[Feeling::Fear], 0.3);
        assert_eq!(Feeling::of_id("hearth:grief"), Some(Feeling::Grief));
    }

    #[test]
    fn feelings_rise_fade_and_spread() {
        let mut defs = PsycheDefs::default();
        defs.feelings[Feeling::Fear as usize] = FeelingDef {
            valence: -0.8,
            fade: Fade::Seconds(10.0),
            contagion: 0.5,
            display: Display::Cower,
            reactivity: Vec::new(),
        };
        defs.feelings[Feeling::Grief as usize] = FeelingDef {
            valence: -0.9,
            fade: Fade::Days(2.0),
            ..FeelingDef::default()
        };
        let mut p = Psyche::form(&defs, None, 0.0);
        p.feel(Feeling::Fear, 0.8);
        assert_eq!(p.shown(&defs), Some((Feeling::Fear, 0.8)));
        p.pass(&defs, 10.0, 0.0, 0.0);
        assert!(
            (p.feeling(Feeling::Fear) - 0.4).abs() < 1e-4,
            "half in its half-life"
        );
        // Grief keeps for days.
        p.feel(Feeling::Grief, 0.9);
        p.pass(&defs, 1.0, 2.0, 0.0);
        assert!((p.feeling(Feeling::Grief) - 0.45).abs() < 1e-3);
        assert!(p.mood < -0.2, "grief lowers the mood: {}", p.mood);
        // Another's fear, seen, is caught.
        let mut q = Psyche::form(&defs, None, 0.0);
        for _ in 0..10 {
            q.catch(&defs, Feeling::Fear, 0.8, 0.5);
        }
        assert!(
            q.feeling(Feeling::Fear) > 0.3,
            "{}",
            q.feeling(Feeling::Fear)
        );
        assert!(
            q.feeling(Feeling::Fear) <= 0.4,
            "at most half of what is seen"
        );
    }
}

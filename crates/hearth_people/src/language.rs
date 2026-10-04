//! Languages (V2.1 §10.1; H5). Every culture of a people with language speaks one, drawn from its
//! people's generator (`humans/language/generators.ron`): its sounds — the commonest of the
//! world's languages the likeliest — the syllables it allows, its word order and affixes, and a
//! word for each of the meanings every language has (`humans/language/meanings.ron`), the
//! commonest meanings the shortest. A daughter culture speaks a daughter of its language, and from
//! then on each changes on its own: now and then a regular sound change runs through every word
//! at once, so related languages keep regular correspondences, and now and then a word gives way
//! to a new one; neighbours lend each other words. People are named in their language.

use hearth_content::Content;
use hearth_content::schema::language::{Context, Order, Shape, WordKind};
use hearth_math::hash::Rng;
use serde::{Deserialize, Serialize};

/// A sound of a people's inventory: how it is written, whether a vowel, and the share of the
/// world's languages that have it.
#[derive(Debug, Clone, PartialEq)]
pub struct Sound {
    pub written: String,
    pub vowel: bool,
    pub share: f32,
}

/// A meaning every language has a word for, resolved: its id, the gloss the player reads, its
/// kind and how many syllables its word tends to have.
#[derive(Debug, Clone, PartialEq)]
pub struct MeaningDef {
    pub id: String,
    pub gloss: String,
    pub kind: WordKind,
    pub syllables: u8,
}

/// What a people's languages are drawn from, resolved, with the meanings.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageDefs {
    pub species: String,
    /// The inventory: consonants first, then vowels.
    pub sounds: Vec<Sound>,
    pub consonant_count: (u8, u8),
    pub vowel_count: (u8, u8),
    pub shapes: Vec<(Shape, f32)>,
    pub orders: Vec<(Order, f32)>,
    pub adjective_after: f32,
    pub plural: f32,
    pub past: f32,
    pub replacement: f32,
    pub name_syllables: (u8, u8),
    /// The sound changes: a sound (by its index) become another or lost, where, and the chance
    /// a century.
    pub changes: Vec<(u8, Option<u8>, Context, f32)>,
    pub meanings: Vec<MeaningDef>,
}

fn key(id: &str) -> &str {
    id.rsplit(':').next().unwrap_or(id)
}

impl LanguageDefs {
    /// Every people's language generator in the content, with its meanings.
    pub fn from_content(c: &Content) -> Vec<Self> {
        let meanings: Vec<MeaningDef> = c
            .meanings
            .iter()
            .map(|m| MeaningDef {
                id: key(&m.id).to_owned(),
                gloss: m.name.clone(),
                kind: m.kind,
                syllables: m.syllables.max(1),
            })
            .collect();
        c.languages
            .iter()
            .map(|g| {
                let mut sounds: Vec<Sound> = Vec::new();
                for (written, share) in &g.consonants {
                    sounds.push(Sound {
                        written: written.clone(),
                        vowel: false,
                        share: *share,
                    });
                }
                for (written, share) in &g.vowels {
                    sounds.push(Sound {
                        written: written.clone(),
                        vowel: true,
                        share: *share,
                    });
                }
                let index = |w: &str| sounds.iter().position(|s| s.written == w).map(|i| i as u8);
                let changes = g
                    .changes
                    .iter()
                    .filter_map(|ch| {
                        let to = if ch.to.is_empty() {
                            None
                        } else {
                            Some(index(&ch.to)?)
                        };
                        Some((index(&ch.from)?, to, ch.context, ch.chance))
                    })
                    .collect();
                LanguageDefs {
                    species: g.species.to_string(),
                    sounds,
                    consonant_count: g.consonant_count,
                    vowel_count: g.vowel_count,
                    shapes: g.shapes.clone(),
                    orders: g.orders.clone(),
                    adjective_after: g.adjective_after,
                    plural: g.plural,
                    past: g.past,
                    replacement: g.replacement,
                    name_syllables: g.name_syllables,
                    changes,
                    // A proto-language's words are of its kinds of meaning only.
                    meanings: meanings
                        .iter()
                        .filter(|m| g.kinds.is_empty() || g.kinds.contains(&m.kind))
                        .cloned()
                        .collect(),
                }
            })
            .collect()
    }

    /// The generator of a species, among a content's.
    pub fn of<'a>(list: &'a [Self], species: &str) -> Option<&'a Self> {
        list.iter().find(|d| key(&d.species) == key(species))
    }

    fn vowel(&self, s: u8) -> bool {
        self.sounds.get(s as usize).is_some_and(|x| x.vowel)
    }

    fn front(&self, s: u8) -> bool {
        self.sounds
            .get(s as usize)
            .is_some_and(|x| matches!(x.written.as_str(), "i" | "e" | "è"))
    }

    /// How a word is written.
    pub fn spell(&self, word: &[u8]) -> String {
        word.iter()
            .filter_map(|&s| self.sounds.get(s as usize))
            .map(|s| s.written.as_str())
            .collect()
    }

    /// A meaning's gloss.
    pub fn gloss(&self, meaning: &str) -> Option<&str> {
        self.meanings
            .iter()
            .find(|m| m.id == meaning)
            .map(|m| m.gloss.as_str())
    }
}

/// A language (V2.1 §10.1).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Language {
    /// Its id (the band that first spoke it), and the language it came from.
    pub id: u64,
    pub parent: Option<u64>,
    /// Its sounds (indices into its people's inventory).
    pub sounds: Vec<u8>,
    /// The syllable shapes it allows beyond CV.
    pub shapes: Vec<Shape>,
    pub order: Order,
    pub adjective_after: bool,
    /// Its plural and past suffixes (empty where it marks neither).
    pub plural: Vec<u8>,
    pub past: Vec<u8>,
    /// Its words: each meaning's id, and its sounds.
    pub words: Vec<(String, Vec<u8>)>,
    /// The sound changes it has been through (indices into its people's), oldest first.
    pub changed: Vec<u16>,
}

/// One of a weighted list.
fn pick<T: Copy>(options: &[(T, f32)], rng: &mut Rng) -> Option<T> {
    let total: f32 = options.iter().map(|(_, w)| w.max(0.0)).sum();
    if total <= 0.0 {
        return options.first().map(|(t, _)| *t);
    }
    let mut r = rng.next_f32() * total;
    for (t, w) in options {
        r -= w.max(0.0);
        if r <= 0.0 {
            return Some(*t);
        }
    }
    options.last().map(|(t, _)| *t)
}

/// A whole number in a range, both ends in.
fn between(range: (u8, u8), rng: &mut Rng) -> usize {
    let (a, b) = (range.0 as usize, range.1.max(range.0) as usize);
    (a + (rng.next_f32() * (b - a + 1) as f32) as usize).min(b)
}

/// How many sounds apart two words are (the edit distance).
fn apart(a: &[u8], b: &[u8]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.iter().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, y) in b.iter().enumerate() {
            let here = row[j + 1];
            row[j + 1] = (prev + usize::from(x != y))
                .min(row[j] + 1)
                .min(row[j + 1] + 1);
            prev = here;
        }
    }
    row[b.len()]
}

/// A sound change run through a word (in its context, all at once): whether it changed it. A
/// word is never lost whole.
fn apply(word: &mut Vec<u8>, from: u8, to: Option<u8>, context: Context, d: &LanguageDefs) -> bool {
    let n = word.len();
    let mut out = Vec::with_capacity(n);
    let mut changed = false;
    for k in 0..n {
        let s = word[k];
        let hit = s == from
            && match context {
                Context::Anywhere => true,
                Context::WordStart => k == 0,
                Context::WordEnd => k + 1 == n,
                Context::BetweenVowels => {
                    k > 0 && k + 1 < n && d.vowel(word[k - 1]) && d.vowel(word[k + 1])
                }
                Context::BeforeFront => k + 1 < n && d.front(word[k + 1]),
            };
        if hit {
            changed = true;
            if let Some(t) = to {
                out.push(t);
            }
        } else {
            out.push(s);
        }
    }
    if changed && !out.is_empty() {
        *word = out;
        true
    } else {
        false
    }
}

impl Language {
    /// A language drawn from its people's generator, as language `id`.
    pub fn draw(id: u64, d: &LanguageDefs, rng: &mut Rng) -> Self {
        // Its sounds: each drawn by how many of the world's languages have it, until it has as
        // many as it will.
        let mut sounds = Vec::new();
        for (vowel, count) in [(false, d.consonant_count), (true, d.vowel_count)] {
            let n = between(count, rng);
            let mut pool: Vec<(u8, f32)> = d
                .sounds
                .iter()
                .enumerate()
                .filter(|(_, s)| s.vowel == vowel)
                .map(|(i, s)| (i as u8, s.share.max(0.001)))
                .collect();
            let mut got = 0;
            while got < n && !pool.is_empty() {
                let total: f32 = pool.iter().map(|p| p.1).sum();
                let mut r = rng.next_f32() * total;
                let mut k = pool.len() - 1;
                for (j, p) in pool.iter().enumerate() {
                    r -= p.1;
                    if r <= 0.0 {
                        k = j;
                        break;
                    }
                }
                sounds.push(pool.swap_remove(k).0);
                got += 1;
            }
        }
        sounds.sort_unstable();
        let mut l = Language {
            id,
            parent: None,
            sounds,
            shapes: d
                .shapes
                .iter()
                .filter_map(|(s, share)| (rng.next_f32() < *share).then_some(*s))
                .collect(),
            order: pick(&d.orders, rng).unwrap_or_default(),
            adjective_after: rng.next_f32() < d.adjective_after,
            plural: Vec::new(),
            past: Vec::new(),
            words: Vec::new(),
            changed: Vec::new(),
        };
        if rng.next_f32() < d.plural {
            l.plural = l.syllable(d, rng);
        }
        if rng.next_f32() < d.past {
            l.past = l.syllable(d, rng);
        }
        // A word for each meaning, no two alike.
        for m in &d.meanings {
            let mut w = l.word(d, m.syllables, rng);
            for _ in 0..6 {
                if !l.words.iter().any(|(_, x)| *x == w) {
                    break;
                }
                w = l.word(d, m.syllables + 1, rng);
            }
            l.words.push((m.id.clone(), w));
        }
        l
    }

    /// A syllable of its shapes: a consonant and a vowel mostly, the others as it allows.
    fn syllable(&self, d: &LanguageDefs, rng: &mut Rng) -> Vec<u8> {
        let cons: Vec<u8> = self
            .sounds
            .iter()
            .copied()
            .filter(|&s| !d.vowel(s))
            .collect();
        let vows: Vec<u8> = self
            .sounds
            .iter()
            .copied()
            .filter(|&s| d.vowel(s))
            .collect();
        let one = |list: &[u8], rng: &mut Rng| -> Option<u8> {
            (!list.is_empty())
                .then(|| list[((rng.next_f32() * list.len() as f32) as usize).min(list.len() - 1)])
        };
        let shape = if self.shapes.is_empty() || rng.next_f32() < 0.6 {
            None
        } else {
            let k =
                ((rng.next_f32() * self.shapes.len() as f32) as usize).min(self.shapes.len() - 1);
            Some(self.shapes[k])
        };
        let (onset, coda) = match shape {
            None => (1, 0),
            Some(Shape::V) => (0, 0),
            Some(Shape::Cvc) => (1, 1),
            Some(Shape::Ccv) => (2, 0),
            Some(Shape::Ccvc) => (2, 1),
        };
        let mut s = Vec::new();
        for _ in 0..onset {
            s.extend(one(&cons, rng));
        }
        s.extend(one(&vows, rng));
        for _ in 0..coda {
            s.extend(one(&cons, rng));
        }
        s
    }

    /// A new word of about so many syllables (one more now and then).
    fn word(&self, d: &LanguageDefs, syllables: u8, rng: &mut Rng) -> Vec<u8> {
        let n = syllables.max(1) + u8::from(rng.next_f32() < 0.25);
        let mut w = Vec::new();
        for _ in 0..n {
            w.extend(self.syllable(d, rng));
        }
        w
    }

    /// Its word for a meaning, written.
    pub fn say(&self, d: &LanguageDefs, meaning: &str) -> Option<String> {
        self.words
            .iter()
            .find(|(m, _)| m == meaning)
            .map(|(_, w)| d.spell(w))
    }

    /// A name in it: two or three syllables, its first letter raised.
    pub fn name(&self, d: &LanguageDefs, rng: &mut Rng) -> String {
        let n = between(d.name_syllables, rng).max(1);
        let mut w = Vec::new();
        for _ in 0..n {
            w.extend(self.syllable(d, rng));
        }
        let spelt = d.spell(&w).replace('\'', "");
        let mut chars = spelt.chars();
        match chars.next() {
            Some(c) => c.to_uppercase().chain(chars).collect(),
            None => "Ana".to_owned(),
        }
    }

    /// A daughter of it, for a people gone off on its own: the same words, its own lineage.
    pub fn daughter(&self, id: u64) -> Self {
        let mut l = self.clone();
        if self.id != 0 {
            l.id = id;
            l.parent = Some(self.id);
        }
        l
    }

    /// A year of change (V2.1 §10.1): now and then a regular sound change runs through every
    /// word at once (a hundredth a year of its chance a century), and now and then a word gives
    /// way to a new one.
    pub fn drift(&mut self, d: &LanguageDefs, rng: &mut Rng) {
        for (k, &(from, to, context, chance)) in d.changes.iter().enumerate() {
            if !self.sounds.contains(&from) || rng.next_f32() >= chance / 100.0 {
                continue;
            }
            let mut any = false;
            for (_, w) in &mut self.words {
                any |= apply(w, from, to, context, d);
            }
            apply(&mut self.plural, from, to, context, d);
            apply(&mut self.past, from, to, context, d);
            if !any {
                continue;
            }
            self.changed.push(k as u16);
            if let Some(t) = to
                && !self.sounds.contains(&t)
            {
                self.sounds.push(t);
                self.sounds.sort_unstable();
            }
            if !self.words.iter().any(|(_, w)| w.contains(&from)) {
                self.sounds.retain(|s| *s != from);
            }
        }
        let p = d.replacement / 100.0;
        for k in 0..self.words.len() {
            if rng.next_f32() < p {
                let syllables = d
                    .meanings
                    .iter()
                    .find(|m| m.id == self.words[k].0)
                    .map_or(2, |m| m.syllables);
                let w = self.word(d, syllables, rng);
                self.words[k].1 = w;
            }
        }
    }

    /// A word lent: one takes the other's word for a meaning (neighbours' contact).
    pub fn lend(&mut self, other: &mut Self, rng: &mut Rng) {
        if self.words.is_empty() || other.words.is_empty() {
            return;
        }
        let k = ((rng.next_f32() * self.words.len() as f32) as usize).min(self.words.len() - 1);
        let meaning = self.words[k].0.clone();
        let Some(j) = other.words.iter().position(|(m, _)| *m == meaning) else {
            return;
        };
        if rng.next_f32() < 0.5 {
            self.words[k].1 = other.words[j].1.clone();
        } else {
            other.words[j].1 = self.words[k].1.clone();
        }
    }

    /// How alike two languages' words are: the share of meanings whose words are cognates.
    pub fn kinship(&self, other: &Self) -> f32 {
        let (mut n, mut alike) = (0, 0);
        for (m, _) in &self.words {
            if other.words.iter().any(|(o, _)| o == m) {
                n += 1;
                if self.cognate(other, m) {
                    alike += 1;
                }
            }
        }
        if n == 0 { 0.0 } else { alike as f32 / n as f32 }
    }

    /// Whether its word for a meaning and another language's are cognates: no more than a third
    /// of their sounds apart.
    pub fn cognate(&self, other: &Self, meaning: &str) -> bool {
        let w = self.words.iter().find(|(m, _)| m == meaning);
        let v = other.words.iter().find(|(m, _)| m == meaning);
        matches!(
            (w, v),
            (Some((_, w)), Some((_, v))) if apart(w, v) * 3 <= w.len().max(v.len())
        )
    }
}

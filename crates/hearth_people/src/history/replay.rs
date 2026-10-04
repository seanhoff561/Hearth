//! A lineage's culture and language, made when a band of it is first met (D193): drawn at its
//! root from its people's generators on the root's own stream, then down its branch — a daughter
//! at each split, a year's drift for each year of the branch (only the last
//! `branch_years_max` years count: nothing older would be left to tell) — and customs and words
//! taken from the lineages it had most contact with.

use hearth_math::hash::{Rng, hash2};

use super::History;
use crate::band::Culture;
use crate::species::Species;

/// Lineage cultures' ids, apart from bands' (H5's cultures carry the id of the band that first
/// carried them).
pub const LINEAGE_IDS: u64 = 3_000_000_000;
/// The salt of the streams lineages are replayed on.
const STREAM: u64 = 0x11e_a6e5;
/// The most meetings a contact is reckoned as.
const MOST_MEETINGS: f32 = 40.0;

/// The culture id of a lineage.
pub fn culture_id(lineage: u32) -> u64 {
    LINEAGE_IDS + lineage as u64
}

/// A lineage's culture and language as they stand at the era's date, its contacts' borrowings
/// taken in when `contacts`.
pub fn culture_of(
    h: &History,
    lineage: u32,
    sp: &Species,
    max_years: f64,
    contacts: bool,
) -> Option<Culture> {
    let g = sp.culture.as_ref()?;
    // The chain from the root down to it.
    let mut chain = Vec::new();
    let mut at = Some(lineage);
    while let Some(id) = at {
        let l = h.lineage(id)?;
        chain.push(l);
        at = l.parent.filter(|p| *p != id && chain.len() < 10_000);
    }
    chain.reverse();
    let root = chain[0];
    let mut rng = Rng::new(hash2(h.seed ^ STREAM, root.id as u64));
    let mut c = Culture::default();
    c.draw(culture_id(root.id), g, sp.ways.as_ref(), &mut rng, 0.0);
    c.language = sp
        .language
        .as_ref()
        .map(|d| crate::language::Language::draw(culture_id(root.id), d, &mut rng));
    // Down the branch: drift through the years that still tell, a daughter at each split.
    let from = h.years_ago + max_years;
    for (i, l) in chain.iter().enumerate() {
        if i > 0 {
            c = c.daughter(culture_id(l.id), 0.0);
        }
        let start = l.since_ya.min(from);
        let end = chain
            .get(i + 1)
            .map_or(h.years_ago, |n| n.since_ya.max(h.years_ago));
        let years = (start - end).max(0.0).round() as u64;
        let mut rng = Rng::new(hash2(h.seed ^ STREAM ^ 0x0d, l.id as u64));
        for _ in 0..years {
            c.drift(g, &mut rng);
            if let (Some(lang), Some(d)) = (c.language.as_mut(), sp.language.as_ref()) {
                lang.drift(d, &mut rng);
            }
        }
    }
    // Its neighbours' customs and words.
    if contacts {
        let me = h.lineage(lineage)?;
        let mut rng = Rng::new(hash2(h.seed ^ STREAM ^ 0xc0, lineage as u64));
        for &(other, much) in me.contacts.iter().take(3) {
            let Some(ol) = h.lineage(other) else {
                continue;
            };
            if ol.species != me.species {
                continue;
            }
            let Some(mut theirs) = culture_of(h, other, sp, max_years, false) else {
                continue;
            };
            // A meeting a generation of a hundred people together, at most so many.
            let meetings = (much / 2500.0).min(MOST_MEETINGS) as u32;
            for _ in 0..meetings {
                c.meet(&mut theirs, g, 0.5, &mut rng);
                if let (Some(a), Some(b)) = (c.language.as_mut(), theirs.language.as_mut())
                    && rng.next_f32() < g.drift.borrow
                {
                    a.lend(b, &mut rng);
                }
            }
        }
    }
    Some(c)
}

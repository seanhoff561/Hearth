//! What is worth telling of the living world (V2.1 §15.4; H9): a band's camp moved, a band
//! split in two, a technique found out, a stranger taken in, one cast out, a fight — kept a
//! while for the Observer's chronicle, beside deep time's.

use glam::DVec3;

use crate::person::PersonId;
use crate::sim::People;

/// A notable event of the living world: when (the world's day), what, where.
#[derive(Debug, Clone, PartialEq)]
pub struct Notable {
    pub day: f64,
    pub text: String,
    pub at: DVec3,
}

/// The most kept (the oldest let go).
const KEPT: usize = 400;

impl People {
    /// Tells of something notable.
    pub(crate) fn note(&mut self, day: f64, text: String, at: DVec3) {
        if self.notable.len() >= KEPT {
            self.notable.remove(0);
        }
        self.notable.push(Notable { day, text, at });
    }

    /// A person as the chronicle names them: by name, else as someone.
    pub(crate) fn called(&self, id: PersonId) -> String {
        self.get(id)
            .map(|p| p.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Someone".to_owned())
    }
}

/// A technique's id as words.
pub(crate) fn technique(id: &str) -> String {
    id.rsplit(':').next().unwrap_or(id).replace('_', " ")
}

impl People {
    /// A person's life as the Observer reads it (V2.1 §15.4): who they are, their parents and
    /// partner, and what befell them, oldest first — told in years of the world's calendar.
    pub fn life_of(&self, id: PersonId, now: &crate::world::Now) -> Vec<String> {
        use crate::person::Event as E;
        let Some(p) = self.get(id) else {
            return Vec::new();
        };
        let years = |d: f64| (d / now.year_days.max(1.0)).floor();
        let age = years(p.life.died.as_ref().map_or(now.day, |d| d.day) - p.life.born);
        let mut lines = vec![format!(
            "{}, {} of {age:.0}{}",
            self.called(id),
            if p.life.female { "a woman" } else { "a man" },
            if p.alive() { "" } else { ", dead" }
        )];
        let parent = |x: Option<PersonId>| x.map(|m| self.called(m));
        match (parent(p.life.mother), parent(p.life.father)) {
            (Some(m), Some(f)) => lines.push(format!("Child of {m} and {f}.")),
            (Some(m), None) => lines.push(format!("Child of {m}.")),
            _ => {}
        }
        if let Some(b) = p.social.bond {
            lines.push(format!("Paired with {}.", self.called(b)));
        }
        let ago = |d: f64| {
            let y = years(now.day - d);
            if y < 1.0 {
                "This year".to_owned()
            } else {
                format!("{y:.0} years ago")
            }
        };
        for e in &p.life.events {
            let what = match &e.event {
                E::Paired { with } => format!("paired with {}", self.called(*with)),
                E::Bore { child } => format!("bore {}", self.called(*child)),
                E::Discovered { node } => format!("found out {}", technique(node)),
                E::MovedCamp { .. } => "moved camp with the band".to_owned(),
                E::Fought { with } => format!("came to blows with {}", self.called(*with)),
                E::TakenIn { .. } => "was taken into a band".to_owned(),
                E::CastOut { .. } => "was cast out of the band".to_owned(),
                E::Left { .. } => "left the band after a feud".to_owned(),
                E::Died { cause } => format!("died ({cause:?})"),
                _ => continue,
            };
            lines.push(format!("{}: {what}.", ago(e.day)));
        }
        // The latest dozen.
        if lines.len() > 15 {
            let head: Vec<String> = lines[..3].to_vec();
            let tail: Vec<String> = lines[lines.len() - 12..].to_vec();
            lines = head.into_iter().chain(tail).collect();
        }
        lines
    }
}

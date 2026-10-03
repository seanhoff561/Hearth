//! The developer's inspector (V2.1 §16, H0): a person's record as text, a section for each of its
//! components. Each milestone adds its own (the genome and the phenotype's chain in H1, the
//! psyche in H2, …).

use hearth_craft::Graph;
use serde::{Deserialize, Serialize};

use crate::mind::Needs;
use crate::person::{Event, PersonId, Tier};
use crate::sim::People;
use crate::species::SpeciesSet;
use crate::world::Now;

/// A component's lines.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub name: String,
    pub lines: Vec<String>,
}

/// A person's record, for the inspector (a message's payload: serializable, D166).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub id: PersonId,
    pub title: String,
    pub sections: Vec<Section>,
}

fn section(name: &str, lines: Vec<String>) -> Section {
    Section {
        name: name.to_owned(),
        lines,
    }
}

/// The record of person `id` at a moment.
pub fn report(
    people: &People,
    species: &SpeciesSet,
    graph: &Graph,
    id: PersonId,
    now: &Now,
) -> Option<Report> {
    let p = people.get(id)?;
    let sp = species.get(&p.species)?;
    let age = p.age(now);
    let sex = if p.life.female { "female" } else { "male" };
    let title = format!("{} #{} — {sex}, {age:.1} years", sp.name, p.id);
    let mut sections = Vec::new();

    // Life history.
    let mut life = vec![
        format!(
            "{:?}, {}; born day {:.1}{}",
            p.stage(sp, now),
            match p.tier {
                Tier::Full => "lived in full",
                Tier::Dormant => "dormant",
            },
            p.life.born,
            p.life.died.as_ref().map_or_else(String::new, |d| format!(
                "; died day {:.1}: {:?}",
                d.day, d.cause
            ))
        ),
        format!(
            "mother {}, father {}",
            p.life
                .mother
                .map_or("unknown".to_owned(), |m| format!("#{m}")),
            p.life
                .father
                .map_or("unknown".to_owned(), |m| format!("#{m}")),
        ),
    ];
    for e in p.life.events.iter().rev().take(5) {
        let what = match &e.event {
            Event::Born { band } => format!("born into band {band}"),
            Event::Found { band } => format!("first seen, in band {band}"),
            Event::Joined { band } => format!("joined band {band}"),
            Event::Died { cause } => format!("died: {cause:?}"),
        };
        life.push(format!("day {:.1}: {what}", e.day));
    }
    sections.push(section("Life", life));

    // Body.
    let cfg = sp.body(p.life.female);
    let s = p.body.status(cfg);
    sections.push(section(
        "Body",
        vec![
            format!(
                "{:.0} kg, {:.2} m; core {:.1} °C, skin {:.1} °C",
                p.mass_kg(sp, now),
                p.height_m(sp, now),
                s.core_c,
                s.skin_c
            ),
            format!(
                "{:?}, {:?}, {:?}, {:?}; stamina {:.2}",
                s.hunger, s.thirst, s.warmth, s.tiredness, s.stamina
            ),
            format!(
                "injuries {}, illnesses {}{}",
                p.body.injuries.len(),
                p.body.illnesses.len(),
                if s.bleeding { ", bleeding" } else { "" }
            ),
        ],
    ));

    // Mind.
    let needs = Needs::of(&p.body, cfg, p.mind.fear);
    sections.push(section(
        "Mind",
        vec![
            format!("{:?} ({:.0} s more)", p.mind.doing, p.mind.timer.max(0.0)),
            format!(
                "hunger {:.2}, thirst {:.2}, tiredness {:.2}, fear {:.2}",
                needs.hunger, needs.thirst, needs.tiredness, needs.fear
            ),
        ],
    ));

    // Knowledge.
    let mut known: Vec<String> = p
        .knowledge
        .known
        .keys()
        .map(|k| graph.node(k).map_or_else(|| k.clone(), |n| n.name.clone()))
        .collect();
    known.sort();
    let skills: Vec<String> = p
        .knowledge
        .skills
        .keys()
        .map(|k| format!("{k} {:.2}", p.knowledge.skill(k)))
        .collect();
    let mut knowledge = vec![if known.is_empty() {
        "knows nothing yet".to_owned()
    } else {
        known.join(", ")
    }];
    if !skills.is_empty() {
        knowledge.push(format!("skills: {}", skills.join(", ")));
    }
    sections.push(section("Knowledge", knowledge));

    // Social.
    let band = people.bands.iter().find(|b| b.id == p.social.band);
    let living = people.members(p.social.band).filter(|q| q.alive()).count();
    let mut social = vec![format!("band {} of {living}", p.social.band)];
    if let Some(b) = band {
        for (player, t) in &b.tolerance {
            social.push(format!("at ease with player {player}: {t:.2}"));
        }
    }
    sections.push(section("Social", social));

    // Possessions.
    let carry = &p.possessions.carry;
    let hand = |s: &Option<hearth_items::Stack>| {
        s.as_ref()
            .map_or("empty".to_owned(), |s| format!("{} ×{}", s.id, s.count))
    };
    sections.push(section(
        "Possessions",
        vec![format!(
            "right hand {}, left hand {}",
            hand(&carry.right),
            hand(&carry.left)
        )],
    ));

    // Place.
    sections.push(section(
        "Place",
        vec![format!(
            "{:.1}, {:.1}, {:.1}, {:?}, {:.1} m/s",
            p.place.pos.x, p.place.pos.y, p.place.pos.z, p.place.medium, p.place.speed
        )],
    ));
    Some(Report {
        id,
        title,
        sections,
    })
}

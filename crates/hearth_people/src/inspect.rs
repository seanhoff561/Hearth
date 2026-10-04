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
            p.life_stage(sp, now),
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
    if age < sp.life.maturity_years as f64 {
        let (h, m) = sp.life.grown_share(age);
        life.push(format!(
            "growing: {:.0} % of grown height, {:.0} % of its mass; underfed {:.2}",
            h * 100.0,
            m * 100.0,
            p.life.undernourished
        ));
    }
    if let Some(due) = &p.life.pregnant {
        life.push(format!("with child, due day {:.1}", due.due));
    }
    for e in p.life.events.iter().rev().take(5) {
        let what = match &e.event {
            Event::Born { band } => format!("born into band {band}"),
            Event::Found { band } => format!("first seen, in band {band}"),
            Event::Joined { band } => format!("joined band {band}"),
            Event::Died { cause } => format!("died: {cause:?}"),
            Event::Met { player } => format!("first met player {player}"),
            Event::Paired { with } => format!("paired with #{with}"),
            Event::Bore { child } => format!("bore #{child}"),
            Event::Inherited { from } => format!("took up what #{from} had carried"),
            Event::Breached { breach } => format!("seen breaking a norm: {breach:?}"),
            Event::CastOut { from } => format!("cast out of band {from}"),
            Event::Hurt => "badly hurt".to_owned(),
            Event::Mourned { who } => format!("mourned #{who}"),
        };
        life.push(format!("day {:.1}: {what}", e.day));
    }
    sections.push(section("Life", life));

    // Body.
    let sized = p.body_config(sp, now);
    let cfg: &hearth_body::BodyConfig = &sized;
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

    // Genome and phenotype: what the genes give, chance and development, and what shows.
    if let (Some(g), Some(ph)) = (&p.genome, &p.phenotype) {
        let het = g
            .maternal
            .iter()
            .zip(&g.paternal)
            .filter(|(a, b)| a != b && **b != crate::genome::NONE)
            .count();
        let inbred = crate::lineage::Kinship::new(people).inbreeding(p.id);
        sections.push(section(
            "Genome",
            vec![
                format!(
                    "{} loci, heterozygous at {het}; inbreeding {inbred:.3}; recessive conditions {}; immune diversity {:.2}",
                    g.maternal.len(),
                    ph.conditions,
                    ph.immune_diversity
                ),
                ph.named
                    .iter()
                    .map(|(k, [a, b])| format!("{}: {a}/{b}", k.rsplit(':').next().unwrap_or(k)))
                    .collect::<Vec<_>>()
                    .join(", "),
            ],
        ));
        let chain = |id: &str| {
            ph.traits
                .iter()
                .find(|(k, _)| k.as_str() == id || k.ends_with(&format!(":{id}")))
                .map_or_else(String::new, |(_, v)| {
                    format!(
                        "{id} {:+.2} = genes {:+.2} chance {:+.2} development {:+.2}",
                        v.z(),
                        v.genetic,
                        v.chance,
                        v.development
                    )
                })
        };
        let mut lines: Vec<String> = ["stature", "skin_pigment", "hair_darkness", "build"]
            .iter()
            .map(|id| chain(id))
            .collect();
        let hexaco = [
            ("H", "honesty_humility"),
            ("E", "emotionality"),
            ("X", "extraversion"),
            ("A", "agreeableness"),
            ("C", "conscientiousness"),
            ("O", "openness"),
        ];
        lines.push(format!(
            "HEXACO {}",
            hexaco
                .iter()
                .map(|(k, id)| format!("{k} {:+.1}", ph.z(id)))
                .collect::<Vec<_>>()
                .join(" ")
        ));
        sections.push(section("Phenotype", lines));
    }

    // Psyche: the mood and stress, the feelings now, the tendencies and values.
    let ps = &p.psyche;
    let felt: Vec<String> = ps
        .feelings
        .iter()
        .filter(|(_, v)| *v >= 0.02)
        .map(|(f, v)| format!("{} {v:.2}", f.key()))
        .collect();
    let tendencies: Vec<String> = ps
        .tendencies
        .iter()
        .map(|(t, v)| format!("{} {v:.2}", t.key()))
        .collect();
    let values: Vec<String> = ps
        .values
        .iter()
        .map(|(v, w)| format!("{} {w:.2}", v.key()))
        .collect();
    sections.push(section(
        "Psyche",
        vec![
            format!(
                "mood {:+.2}, stress {:.2}{}; feeling {}",
                ps.mood,
                ps.stress,
                if ps.insecure > 0.05 {
                    format!(" (held little as an infant: {:.2})", ps.insecure)
                } else {
                    String::new()
                },
                if felt.is_empty() {
                    "nothing much".to_owned()
                } else {
                    felt.join(", ")
                }
            ),
            tendencies.join(", "),
            values.join(", "),
        ],
    ));

    // Memory: the mental map, the people known, what happened.
    let m = &p.memory;
    let count = |k: crate::memory::PlaceKind| m.all(k).count();
    use crate::memory::PlaceKind as K;
    let well = m.known.iter().filter(|k| k.familiarity > 0.5).count();
    let mut memory = vec![format!(
        "places: water {}, sleep {}, food {}, anvils {}, danger {}; knows {} ({well} well)",
        count(K::Water),
        count(K::Sleep),
        count(K::Food),
        count(K::Anvil),
        count(K::Danger),
        m.known.len()
    )];
    let mut episodes: Vec<&crate::memory::Episode> = m.episodes.iter().collect();
    episodes.sort_by(|a, b| b.weight.total_cmp(&a.weight));
    for e in episodes.iter().take(4) {
        memory.push(format!("day {:.1}: {:?} ({:.2})", e.day, e.what, e.weight));
    }
    sections.push(section("Memory", memory));

    // Mind.
    let needs = Needs::of(&p.body, cfg, p.psyche.feeling(crate::psyche::Feeling::Fear));
    sections.push(section(
        "Mind",
        vec![
            format!("{:?} ({:.0} s more)", p.mind.doing, p.mind.timer.max(0.0)),
            format!(
                "hunger {:.2}, thirst {:.2}, tiredness {:.2}, fear {:.2}",
                needs.hunger, needs.thirst, needs.tiredness, needs.fear
            ),
            match (&p.mind.goal, &p.mind.plan) {
                (None, _) => "no goal".to_owned(),
                (Some(g), None) => format!("goal {g:?}, unplanned ({} failed)", p.mind.failures),
                (Some(g), Some(pl)) => format!(
                    "goal {g:?}: step {} of {}, {:?}",
                    pl.next + 1,
                    pl.steps.len(),
                    pl.step()
                ),
            },
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
    if let Some(with) = p.social.bond {
        social.push(format!("paired with #{with}"));
    }
    if let Some(h) = p.social.household {
        let members = people.household_of(h);
        social.push(format!("household #{h} of {}", members.len()));
    }
    // Its kin among the living, nearest first.
    let mut kin: Vec<(u8, String)> = people
        .persons
        .iter()
        .filter(|q| q.alive() && q.id != p.id)
        .filter_map(|q| {
            crate::kin::kin_of(people, p.id, q.id)
                .map(|k| (k.nearness(), format!("{} #{}", k.word(q.life.female), q.id)))
        })
        .collect();
    kin.sort();
    if !kin.is_empty() {
        let words: Vec<String> = kin.into_iter().take(10).map(|(_, w)| w).collect();
        social.push(format!("kin: {}", words.join(", ")));
    }
    // Its closest ties, and what it owes and is owed.
    let mut ties: Vec<&crate::ties::Tie> = p.social.ties.iter().collect();
    ties.sort_by(|a, b| b.strength().total_cmp(&a.strength()));
    if !ties.is_empty() {
        let words: Vec<String> = ties
            .iter()
            .take(5)
            .map(|t| {
                format!(
                    "#{} (fond {:.2}, trust {:.2}{})",
                    t.who,
                    t.affection,
                    t.trust,
                    if t.given + t.owed > 0.05 {
                        format!(", gave {:.1}, owes {:.1}", t.given, t.owed)
                    } else {
                        String::new()
                    }
                )
            })
            .collect();
        social.push(format!(
            "knows {} — closest: {}",
            p.social.ties.len(),
            words.join("; ")
        ));
    }
    // What it thinks of others, and what its band thinks of it.
    let mut views: Vec<&crate::repute::Repute> = p
        .social
        .reputes
        .iter()
        .filter(|r| r.badness() < -0.05 || r.generous > 0.2)
        .collect();
    views.sort_by(|a, b| a.badness().total_cmp(&b.badness()));
    if !views.is_empty() {
        let words: Vec<String> = views
            .iter()
            .take(4)
            .map(|r| {
                format!(
                    "{:?} generous {:+.2} honest {:+.2} (sure {:.2})",
                    r.about, r.generous, r.honest, r.sure
                )
            })
            .collect();
        social.push(format!("thinks: {}", words.join("; ")));
    }
    if let Some(bi) = people.bands.iter().position(|b| b.id == p.social.band) {
        let r = people.band_view(bi, p.id, now.day, now.year_days);
        if r.sure > 0.01 {
            social.push(format!(
                "its band thinks it generous {:+.2}, honest {:+.2}",
                r.generous, r.honest
            ));
        }
    }
    let children: Vec<String> = people
        .persons
        .iter()
        .filter(|c| c.life.mother == Some(p.id) || c.life.father == Some(p.id))
        .map(|c| format!("#{}{}", c.id, if c.alive() { "" } else { " (dead)" }))
        .collect();
    if !children.is_empty() {
        social.push(format!("children {}", children.join(", ")));
    }
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

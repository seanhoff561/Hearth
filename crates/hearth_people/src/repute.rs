//! Reputation, gossip, norms and sanctions (V2.1 §8.4; H4). What people believe of one another
//! — how generous, how honest, and how sure they are of it — comes from deeds they see (taking
//! another's things, keeping food from the hungry, sharing it, staying by the hurt) and from
//! talk: in company, now and then, one tells another what it thinks of a third, and the hearer
//! comes round to it as far as it trusts the teller, less sure than one who saw, the telling a
//! little changed. A breach of the people's norms (data) is felt as indignation by those who
//! see it, and as its doer's name worsens among them they apply the norm's sanctions in turn:
//! mockery that shames, keeping away, food withheld, and at last casting out of the band.

use glam::DVec3;
use hearth_content::Content;
use hearth_content::schema::social::{Breach, Sanction};
use serde::{Deserialize, Serialize};

use crate::memory::Who;
use crate::person::{Event, PersonId, Tier};
use crate::psyche::Feeling;
use crate::sim::People;

/// What one believes of another person (or a player).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Repute {
    pub about: Who,
    /// −1 stingy … 1 generous.
    pub generous: f32,
    /// −1 a thief … 1 to be trusted with one's things.
    pub honest: f32,
    /// 1 seen with its own eyes … less, heard of.
    pub sure: f32,
    pub day: f64,
}

impl Repute {
    fn new(about: Who, day: f64) -> Self {
        Self {
            about,
            generous: 0.0,
            honest: 0.0,
            sure: 0.0,
            day,
        }
    }

    /// How bad a name it is, −1 … 0 (0 none).
    pub fn badness(&self) -> f32 {
        self.generous.min(self.honest).min(0.0)
    }

    /// How much there is to tell of it.
    fn worth_telling(&self) -> f32 {
        (self.generous.abs().max(self.honest.abs())) * self.sure
    }
}

/// A deed others may see.
#[derive(Debug, Clone, PartialEq)]
pub enum Deed {
    /// Took a thing that was another's.
    Took { from: Who },
    /// Kept food from one hungry near.
    Withheld { from: PersonId },
    /// Shared food with one hungry.
    Shared { with: PersonId },
    /// Stayed by one hurt.
    Tended { who: PersonId },
}

/// A deed done somewhere, by someone.
#[derive(Debug, Clone, PartialEq)]
pub struct Seen {
    pub who: Who,
    pub deed: Deed,
    pub at: DVec3,
}

/// A people's norms, resolved: how heavily each breach weighs and the sanctions it brings (each
/// from how bad a name).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Norms {
    pub list: Vec<(Breach, f32, Vec<(Sanction, f32)>)>,
}

impl Norms {
    pub fn from_content(c: &Content) -> Self {
        Self {
            list: c
                .norms
                .iter()
                .map(|n| (n.breach, n.severity, n.sanctions.clone()))
                .collect(),
        }
    }

    /// How heavily a breach weighs (0 where the people have no such norm).
    pub fn severity(&self, b: Breach) -> f32 {
        self.list
            .iter()
            .find(|(x, ..)| *x == b)
            .map_or(0.0, |(_, s, _)| *s)
    }

    /// Whether a name as bad as `badness` (−1 … 0) on a breach brings a sanction.
    pub fn brings(&self, b: Breach, badness: f32, s: Sanction) -> bool {
        self.list
            .iter()
            .filter(|(x, ..)| *x == b)
            .flat_map(|(_, _, list)| list.iter())
            .any(|(k, from)| *k == s && badness <= *from)
    }

    /// Whether a name brings a sanction on either breach (honesty for taking, generosity for
    /// withholding).
    pub fn sanctions(&self, r: &Repute, s: Sanction) -> bool {
        self.brings(Breach::Taking, r.honest, s) || self.brings(Breach::Withholding, r.generous, s)
    }
}

/// How far off a deed is seen (m), by day.
const WITNESS_M: f64 = 40.0;
/// The most others a person holds a view of.
const VIEWS: usize = 60;
/// The chance a second, in close company, that one tells another what it thinks of a third.
const GOSSIP_A_SECOND: f32 = 0.03;
/// How near two must be to talk (m).
const TALK_M: f64 = 5.0;
/// How sure a hearer is of what it hears, to the teller's sureness.
const HEARD: f32 = 0.6;
/// The age (years) from which one's view counts in what a band thinks.
const GROWN: f64 = 15.0;
/// What views fade by, a month of the life course, toward nothing.
const FADE_A_MONTH: f32 = 0.05;

impl People {
    /// What a person believes of another, made if it has no view yet.
    pub(crate) fn repute_index(&mut self, i: usize, about: Who, day: f64) -> usize {
        let views = &mut self.persons[i].social.reputes;
        if let Some(k) = views.iter().position(|r| r.about == about) {
            return k;
        }
        if views.len() >= VIEWS
            && let Some(least) = (0..views.len()).min_by(|&a, &b| {
                views[a]
                    .worth_telling()
                    .total_cmp(&views[b].worth_telling())
                    .then(views[a].day.total_cmp(&views[b].day))
            })
        {
            views.swap_remove(least);
        }
        views.push(Repute::new(about, day));
        views.len() - 1
    }

    /// A deed done: those who see it (lived in full, within sight of it, by daylight) think the
    /// better or the worse of its doer, sure of it; a breach angers its victim and stirs
    /// indignation in the rest, the more the heavier the norm.
    pub fn deed(&mut self, seen: Seen, norms: &Norms, light: f32, day: f64) {
        let (breach, moves): (Option<Breach>, (f32, f32)) = match &seen.deed {
            Deed::Took { .. } => {
                let s = norms.severity(Breach::Taking);
                (Some(Breach::Taking), (0.0, -s))
            }
            Deed::Withheld { .. } => {
                let s = norms.severity(Breach::Withholding);
                (Some(Breach::Withholding), (-s, 0.0))
            }
            Deed::Shared { .. } => (None, (0.15, 0.0)),
            Deed::Tended { .. } => (None, (0.05, 0.0)),
        };
        let victim = match &seen.deed {
            Deed::Took { from } => Some(*from),
            Deed::Withheld { from } => Some(Who::Person(*from)),
            _ => None,
        };
        let sight = WITNESS_M * light.clamp(0.25, 1.0) as f64;
        let witnesses: Vec<usize> = (0..self.persons.len())
            .filter(|&i| {
                let p = &self.persons[i];
                p.tier == Tier::Full
                    && p.alive()
                    && Who::Person(p.id) != seen.who
                    && (p.place.pos - seen.at).length() <= sight
            })
            .collect();
        let mut wronged: Option<usize> = None;
        for i in witnesses {
            let k = self.repute_index(i, seen.who, day);
            let p = &mut self.persons[i];
            let r = &mut p.social.reputes[k];
            r.generous = (r.generous + moves.0).clamp(-1.0, 1.0);
            r.honest = (r.honest + moves.1).clamp(-1.0, 1.0);
            r.sure = 1.0;
            r.day = day;
            if let Some(b) = breach {
                let weight = norms.severity(b);
                if victim == Some(Who::Person(p.id)) {
                    p.psyche.feel(Feeling::Anger, 0.4 + 0.5 * weight);
                    wronged = Some(i);
                } else {
                    p.psyche.feel(Feeling::Indignation, 0.2 + 0.6 * weight);
                }
                // Less trust in a person seen breaking a norm.
                if let Who::Person(doer) = seen.who
                    && let Some(t) = p.social.ties.iter_mut().find(|t| t.who == doer)
                {
                    t.trust = (t.trust - 0.3 * weight).max(0.0);
                }
            }
        }
        // The wronged holds it against the doer (V2.1 §8.6).
        if let (Some(i), Who::Person(doer), Some(b)) = (wronged, seen.who, breach) {
            self.aggrieve(i, doer, 0.2 + 0.5 * norms.severity(b), day);
        }
        if let (Who::Person(doer), Some(b)) = (seen.who, breach)
            && let Ok(d) = self.persons.binary_search_by_key(&doer, |p| p.id)
        {
            let p = &mut self.persons[d];
            p.record(day, Event::Breached { breach: b });
        }
    }

    /// Talk in close company: `teller` tells `hearer` what it thinks of someone, the thing most
    /// worth telling; the hearer comes round to it as far as it trusts the teller, less sure, the
    /// telling changed a little in the passing.
    pub(crate) fn gossip(&mut self, teller: usize, hearer: usize, day: f64) {
        let hearer_id = self.persons[hearer].id;
        let Some(told) = self.persons[teller]
            .social
            .reputes
            .iter()
            .filter(|r| r.about != Who::Person(hearer_id) && r.worth_telling() > 0.05)
            .max_by(|a, b| a.worth_telling().total_cmp(&b.worth_telling()))
            .cloned()
        else {
            return;
        };
        let teller_id = self.persons[teller].id;
        let trust = self.persons[hearer]
            .social
            .ties
            .iter()
            .find(|t| t.who == teller_id)
            .map_or(0.2, |t| t.trust);
        let twist = (self.persons[hearer].rng.next_f32() - 0.5) * 0.1;
        let k = self.repute_index(hearer, told.about, day);
        let r = &mut self.persons[hearer].social.reputes[k];
        let take = 0.5 * trust * (1.0 - 0.5 * r.sure);
        r.generous = (r.generous + (told.generous + twist - r.generous) * take).clamp(-1.0, 1.0);
        r.honest = (r.honest + (told.honest + twist - r.honest) * take).clamp(-1.0, 1.0);
        r.sure = r.sure.max(told.sure * HEARD);
        r.day = day;
    }

    /// Those in close company talk now and then (`dt` seconds of play).
    pub(crate) fn talk(&mut self, dt: f32, day: f64) {
        let close: Vec<(usize, u64, DVec3)> = self
            .persons
            .iter()
            .enumerate()
            .filter(|(_, p)| p.tier == Tier::Full && p.alive() && p.player.is_none())
            .map(|(i, p)| (i, p.social.band, p.place.pos))
            .collect();
        for &(a, band, pos) in &close {
            if self.persons[a].rng.next_f32() >= GOSSIP_A_SECOND * dt {
                continue;
            }
            // The nearest of its band within talking distance hears it.
            let Some(&(b, _, _)) = close
                .iter()
                .filter(|(b, band_b, pos_b)| {
                    *b != a && *band_b == band && (*pos_b - pos).length() < TALK_M
                })
                .min_by(|x, y| (x.2 - pos).length().total_cmp(&(y.2 - pos).length()))
            else {
                continue;
            };
            self.gossip(a, b, day);
        }
    }

    /// A month of a band's life for its members' views: they fade toward nothing, the heard
    /// sooner than the seen.
    pub(crate) fn fade_reputes(&mut self, bi: usize) {
        for i in self.band_members(bi) {
            for r in &mut self.persons[i].social.reputes {
                r.generous *= 1.0 - FADE_A_MONTH;
                r.honest *= 1.0 - FADE_A_MONTH;
                r.sure *= 1.0 - FADE_A_MONTH * 0.5;
            }
            self.persons[i]
                .social
                .reputes
                .retain(|r| r.worth_telling() > 0.005 || r.sure > 0.05);
        }
    }

    /// What a band's grown members (fifteen and over on `day`) think of one of them, on the
    /// whole: the mean of their views (none held counted as nothing).
    pub fn band_view(&self, bi: usize, about: PersonId, day: f64, year_days: f64) -> Repute {
        let mut out = Repute::new(Who::Person(about), 0.0);
        let members = self.band_members(bi);
        let mut n = 0.0;
        for i in members {
            let p = &self.persons[i];
            if p.id == about || (day - p.life.born) / year_days.max(1.0) < GROWN {
                continue;
            }
            n += 1.0;
            if let Some(r) = p
                .social
                .reputes
                .iter()
                .find(|r| r.about == Who::Person(about))
            {
                out.generous += r.generous;
                out.honest += r.honest;
                out.sure += r.sure;
            }
        }
        if n > 0.0 {
            out.generous /= n;
            out.honest /= n;
            out.sure /= n;
        }
        out
    }

    /// The band's worst named, if its name among them brings casting out, leaves: a band of its
    /// own some way off.
    pub(crate) fn cast_out(&mut self, bi: usize, norms: &Norms, day: f64, year_days: f64) {
        let members = self.band_members(bi);
        let worst = members
            .iter()
            .map(|&i| (i, self.band_view(bi, self.persons[i].id, day, year_days)))
            .filter(|(i, r)| {
                self.persons[*i].player.is_none() && norms.sanctions(r, Sanction::Ostracism)
            })
            .min_by(|a, b| a.1.badness().total_cmp(&b.1.badness()));
        let Some((i, _)) = worst else {
            return;
        };
        let id = 2_000_000_000 + self.take_id();
        let from = self.bands[bi].id;
        let rng = &mut self.bands[bi].rng;
        let a = rng.next_f64() * std::f64::consts::TAU;
        let d = 6_000.0 + 6_000.0 * rng.next_f64();
        let mut band = self.bands[bi].clone();
        band.id = id;
        band.home += glam::DVec2::new(a.cos(), a.sin()) * d;
        band.population_group = None;
        band.lived_to = Some(day);
        band.rng = crate::sim::band_stream(self.seed, id);
        band.council = None;
        band.weighed = 0.0;
        band.guests = Vec::new();
        let y = self.persons[i].place.pos.y;
        band.camp = Some(DVec3::new(band.home.x, y, band.home.y));
        let pid = self.persons[i].id;
        band.members = vec![pid];
        self.bands[bi].members.retain(|m| *m != pid);
        let household = self.take_id();
        let p = &mut self.persons[i];
        p.social.band = id;
        p.social.household = Some(household);
        p.psyche.feel(Feeling::Shame, 0.8);
        p.psyche.feel(Feeling::Grief, 0.5);
        p.record(day, Event::CastOut { from });
        self.bands.push(band);
    }
}

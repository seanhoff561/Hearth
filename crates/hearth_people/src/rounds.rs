//! Camps, seasonal rounds and gatherings (V2.1 §15.3; H8, D199): how an era's peoples move
//! through the year. Each season of its people's round a band keeps camp where the round goes
//! then — by a river, a lake or the shore; up on the open uplands; down in the shelter of the
//! woods — within its range, and moves when the season turns. Where the era has it, once a year
//! the bands of one people within a few days' walk gather at one camp for some weeks, each band
//! at its own fire a little apart: they meet one another, marry across bands, and learn what the
//! others know; then they go back to their rounds. The world says where such places are
//! ([`Country`]); a world that does not (a test's) keeps its camps where they are, though its
//! bands still gather.

use std::sync::Arc;

use glam::{DVec2, DVec3};
use hearth_content::schema::Season;
use hearth_content::schema::era::{Aggregation, EraPeople, Toward};
use serde::{Deserialize, Serialize};

use crate::person::Tier;
use crate::sim::People;
use crate::world::Now;

/// Where the land offers a camp (the game answers from its terrain).
pub trait Country: Send + Sync {
    /// The best place for a camp of the kind `toward` within `within_m` of `from`, away from the
    /// camps of other bands (`taken`): its ground.
    fn camp_toward(
        &self,
        toward: Toward,
        from: DVec2,
        within_m: f64,
        taken: &[DVec2],
    ) -> Option<DVec3>;

    /// The dry ground nearest a place within `within_m` (a camp is not made in the water), at
    /// least `apart_m` from other bands' camps (`taken`): its ground. None where the world does
    /// not say.
    fn dry_ground(
        &self,
        _near: DVec2,
        _within_m: f64,
        _taken: &[DVec2],
        _apart_m: f64,
    ) -> Option<DVec3> {
        None
    }
}

/// A band lived in full moves camp at most this far (m).
const FULL_MOVE_M: f64 = 1_500.0;

/// Bands keep their camps at least this far apart, but at a gathering (m).
pub const CAMPS_APART_M: f64 = 800.0;

/// How an era's peoples live through the year (set by the world; none outside the eras).
#[derive(Clone, Default)]
pub struct EraWays {
    pub peoples: Vec<EraPeople>,
    /// The calendar's year fraction at day 0 (0 at the March equinox).
    pub year_offset: f64,
    pub country: Option<Arc<dyn Country>>,
}

impl std::fmt::Debug for EraWays {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EraWays")
            .field("peoples", &self.peoples.len())
            .field("year_offset", &self.year_offset)
            .field("country", &self.country.is_some())
            .finish()
    }
}

impl PartialEq for EraWays {
    fn eq(&self, other: &Self) -> bool {
        self.peoples == other.peoples && self.year_offset == other.year_offset
    }
}

fn key(id: &str) -> &str {
    id.rsplit(':').next().unwrap_or(id)
}

impl EraWays {
    /// How a species' people live in the era.
    pub fn of(&self, species: &str) -> Option<&EraPeople> {
        self.peoples
            .iter()
            .find(|p| key(p.species.as_str()) == key(species))
    }

    /// The season on a day, in a hemisphere.
    pub fn season(&self, day: f64, year_days: f64, southern: bool) -> Season {
        let mut f = (day / year_days.max(1.0) + self.year_offset).rem_euclid(1.0);
        if southern {
            f = (f + 0.5).fract();
        }
        match (f * 4.0) as u32 {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Autumn,
            _ => Season::Winter,
        }
    }
}

/// A band's way through the year: the season whose camp of its round it keeps and where that
/// is, the day it last moved camp, and the gathering of its people it was last at.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Round {
    pub season: Option<Season>,
    pub toward: Option<Toward>,
    pub since: f64,
    pub gathering: Option<Gathering>,
    /// The camp it left for its last gathering.
    pub left: Option<DVec3>,
}

/// A gathering of a people's bands (V2.1 §15.3): where, the band that held it there, when it
/// began and when it ends, and how many bands came.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Gathering {
    pub at: DVec3,
    pub host: u64,
    pub from: f64,
    pub until: f64,
    pub bands: u16,
}

/// Bands at a gathering keep their fires this far apart (m), in a ring about the host's.
const APART_M: f64 = 30.0;
/// Each one of a gathering meets as many of the other bands' people.
const MEETS: usize = 8;
/// A gathered band's people learn from the others' as from their nearest neighbours.
pub const GATHERED_WEIGHT: f32 = 0.5;

impl People {
    /// Whether a band is at a gathering of its people on a day.
    pub fn gathered(&self, bi: usize, day: f64) -> Option<Gathering> {
        self.bands[bi]
            .round
            .gathering
            .filter(|g| (g.from..g.until).contains(&day))
    }

    /// Whether two bands came to the same gathering within the last year.
    pub(crate) fn gathered_together(&self, a: usize, b: usize, day: f64, year_days: f64) -> bool {
        match (self.bands[a].round.gathering, self.bands[b].round.gathering) {
            (Some(x), Some(y)) => {
                x.host == y.host && (x.from - y.from).abs() < 1.0 && day - x.from < year_days
            }
            _ => false,
        }
    }

    /// A step of a band's year (its life course's): to a gathering of its people when the
    /// season for it comes, home from one when it ends, and to its round's camp for the season.
    pub(crate) fn round_step(&mut self, bi: usize, day: f64, southern: bool, now: &Now) {
        let Some(ep) = self.era.of(&self.bands[bi].species).cloned() else {
            return;
        };
        if self.bands[bi].members.is_empty() || self.gathered(bi, day).is_some() {
            return;
        }
        let year_days = now.year_days.max(1.0);
        let season = self.era.season(day, year_days, southern);
        // The season of its people's gathering: to it, once a year.
        if let Some(a) = ep.aggregation
            && a.season == season
            && self.bands[bi]
                .round
                .gathering
                .is_none_or(|g| day - g.from > year_days * 0.5)
        {
            self.gather(bi, &a, day, year_days);
            if self.gathered(bi, day).is_some() {
                return;
            }
        }
        // Home from a gathering just ended (not moved since): back on its round.
        let ended = self.bands[bi]
            .round
            .gathering
            .filter(|g| g.bands > 1 && day >= g.until && self.bands[bi].round.since <= g.from);
        if ended.is_some() {
            self.bands[bi].round.season = None;
        }
        if let Some(stop) = ep.round.iter().find(|r| r.season == season).copied()
            && self.bands[bi].round.season != Some(season)
        {
            let (home, range) = (self.bands[bi].home, self.bands[bi].range_m);
            let taken = self.camps_near(bi, home, range + CAMPS_APART_M);
            let r = &mut self.bands[bi].round;
            r.season = Some(season);
            r.toward = Some(stop.toward);
            let at = self
                .era
                .country
                .as_ref()
                .and_then(|c| c.camp_toward(stop.toward, home, range, &taken));
            // A band lived in full walks to its new camp: not further than a short day's walk
            // over the land as the full tier walks it (else it stays, D203).
            let near = |at: &DVec3| {
                self.bands[bi].tier != Tier::Full
                    || self.bands[bi]
                        .camp
                        .is_none_or(|c| DVec2::new(c.x - at.x, c.z - at.z).length() <= FULL_MOVE_M)
            };
            if let Some(at) = at.filter(near) {
                self.move_camp(bi, at, day);
            }
        }
        // No camp of its round to go to: back to the one it left for the gathering.
        if let Some(g) = ended
            && self.bands[bi].round.since <= g.from
            && let Some(left) = self.bands[bi].round.left
        {
            self.move_camp(bi, left, day);
        }
    }

    /// The camps other bands than `bi` keep within `within_m` of a place.
    pub(crate) fn camps_near(&self, bi: usize, at: DVec2, within_m: f64) -> Vec<DVec2> {
        self.bands
            .iter()
            .enumerate()
            .filter(|(b, x)| {
                *b != bi && matches!(x.tier, Tier::Full | Tier::Household) && !x.members.is_empty()
            })
            .filter_map(|(_, x)| x.camp.map(|c| DVec2::new(c.x, c.z)))
            .filter(|c| (*c - at).length() <= within_m)
            .collect()
    }

    /// A band makes camp at a place: its people of the household tier live there now, those
    /// lived in full go to it.
    pub(crate) fn move_camp(&mut self, bi: usize, at: DVec3, day: f64) {
        self.bands[bi].camp = Some(at);
        self.bands[bi].round.since = day;
        if self.bands[bi].tier == Tier::Household {
            for i in self.band_members(bi) {
                let p = &mut self.persons[i];
                p.place.pos.x = at.x;
                p.place.pos.z = at.z;
            }
        }
    }

    /// The nearest bands of a band's people (its species and lineage) within the gathering's
    /// reach of it, as many as gather in one camp, gather there — by the water near the largest
    /// band's home — each at its own fire, for the gathering's part of the year; its people meet
    /// the others'. None to gather with: it stays on its round.
    fn gather(&mut self, bi: usize, a: &Aggregation, day: f64, year_days: f64) {
        let (home, kind) = (self.bands[bi].home, self.bands[bi].species.clone());
        let lineage = self.bands[bi].deep.as_ref().map(|d| d.lineage);
        let reach = a.reach_km as f64 * 1000.0;
        let mut come: Vec<usize> = (0..self.bands.len())
            .filter(|&b| {
                let x = &self.bands[b];
                matches!(x.tier, Tier::Full | Tier::Household)
                    && x.species == kind
                    && x.deep.as_ref().map(|d| d.lineage) == lineage
                    && !x.members.is_empty()
                    && (x.home - home).length() <= reach
                    && x.round
                        .gathering
                        .is_none_or(|g| day - g.from > 0.5 * year_days)
            })
            .collect();
        // The nearest come (the band itself first), as many as gather in one camp.
        come.sort_by(|&x, &y| {
            let d = |b: usize| (self.bands[b].home - home).length();
            d(x).total_cmp(&d(y))
                .then(self.bands[x].id.cmp(&self.bands[y].id))
        });
        come.truncate(a.bands.max(2) as usize);
        if come.len() < 2 {
            // Marked as gathered (alone), so it is not looked for again this season.
            self.bands[bi].round.gathering = Some(Gathering {
                at: self.bands[bi]
                    .camp
                    .unwrap_or(DVec3::new(home.x, 0.0, home.y)),
                host: self.bands[bi].id,
                from: day,
                until: day,
                bands: 1,
            });
            return;
        }
        // The host: the band of the most people (the earliest founded among equals).
        come.sort_by(|&x, &y| {
            self.bands[y]
                .members
                .len()
                .cmp(&self.bands[x].members.len())
                .then(self.bands[x].id.cmp(&self.bands[y].id))
        });
        let host = come[0];
        let (hhome, hrange) = (self.bands[host].home, self.bands[host].range_m);
        let at = self
            .era
            .country
            .as_ref()
            .and_then(|c| c.camp_toward(Toward::Water, hhome, hrange, &[]))
            .or(self.bands[host].camp)
            .unwrap_or(DVec3::new(hhome.x, 0.0, hhome.y));
        // Those lived in full come only from a short day's walk (D203).
        come.retain(|&b| {
            let x = &self.bands[b];
            x.tier != Tier::Full
                || x.camp
                    .is_none_or(|c| DVec2::new(c.x - at.x, c.z - at.z).length() <= FULL_MOVE_M)
        });
        if come.len() < 2 {
            self.bands[bi].round.gathering = Some(Gathering {
                at: self.bands[bi]
                    .camp
                    .unwrap_or(DVec3::new(home.x, 0.0, home.y)),
                host: self.bands[bi].id,
                from: day,
                until: day,
                bands: 1,
            });
            return;
        }
        // Its days are a real year's: as long a part of the game's year.
        let g = Gathering {
            at,
            host: self.bands[host].id,
            from: day,
            until: day + a.days as f64 / 365.0 * year_days,
            bands: come.len() as u16,
        };
        // The host's fire, the others' in a ring about it, each its own distance from the next,
        // on dry ground.
        let ring = (APART_M * (come.len() - 1) as f64 / std::f64::consts::TAU).max(APART_M);
        let mut fires: Vec<DVec2> = Vec::new();
        for (k, &b) in come.iter().enumerate() {
            let angle =
                k.saturating_sub(1) as f64 / (come.len() - 1) as f64 * std::f64::consts::TAU;
            let mut spot = if k == 0 {
                at
            } else {
                at + DVec3::new(angle.cos() * ring, 0.0, angle.sin() * ring)
            };
            if let Some(dry) =
                self.era.country.as_ref().and_then(|c| {
                    c.dry_ground(DVec2::new(spot.x, spot.z), ring, &fires, APART_M * 0.8)
                })
            {
                spot = dry;
            }
            fires.push(DVec2::new(spot.x, spot.z));
            self.bands[b].round.gathering = Some(g);
            self.bands[b].round.left = self.bands[b].camp;
            self.move_camp(b, spot, day);
        }
        self.meet_at(&come, day);
    }

    /// The people of the bands at a gathering meet: each grown one comes to know a few of the
    /// other bands' people.
    fn meet_at(&mut self, bands: &[usize], day: f64) {
        let mut all: Vec<(usize, u64)> = Vec::new();
        for &b in bands {
            for i in self.band_members(b) {
                all.push((i, self.bands[b].id));
            }
        }
        if all.len() < 2 {
            return;
        }
        let host = bands[0];
        for k in 0..all.len() {
            let (i, band) = all[k];
            if !self.persons[i].alive() {
                continue;
            }
            let others: Vec<usize> = all
                .iter()
                .filter(|(_, b)| *b != band)
                .map(|(j, _)| *j)
                .collect();
            if others.is_empty() {
                continue;
            }
            for _ in 0..MEETS.min(others.len()) {
                let pick = (self.bands[host].rng.next_f32() * others.len() as f32) as usize;
                let j = others[pick.min(others.len() - 1)];
                let other = self.persons[j].id;
                self.tie_index(i, other, day);
                let me = self.persons[i].id;
                self.tie_index(j, me, day);
            }
        }
    }
}

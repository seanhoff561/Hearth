//! What stands on a tree site over the years (V2-6, docs/design/flora.md). A site's trees live
//! out their lives and others take their place; felling, clearing and fire end them early, and
//! opened land grows back through the pioneers to the trees of the old forest.
//!
//! Everything is drawn from the site's hash, so a site at a year is the same wherever and
//! whenever it is asked for.

use hearth_flora::{Stage, Turn, VARIANTS};
use hearth_math::hash::{mix64, unit_f32};
use smallvec::SmallVec;

use crate::trees::{Forest, PlaceClimate};
use crate::vegetation::{DisturbanceKind, Vegetation};

/// What of a tree is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remains {
    /// A living tree, all of it.
    Living,
    /// Dead of age and standing: the trunk and limbs of an old tree, no foliage.
    Snag,
    /// Killed by fire and standing: trunk and limbs charred, foliage and twigs burned away.
    Charred,
    /// Cut down: the stump.
    Stump,
}

/// A tree as a site holds it at some year.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SiteTree {
    pub species: usize,
    pub stage: Stage,
    pub variant: u8,
    pub turn: Turn,
    pub remains: Remains,
    /// A young tree of the understory where the canopy tree does not stand.
    pub understory: bool,
}

/// What a site holds at the vegetation's year, and the year that next changes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SiteNow {
    pub tree: Option<SiteTree>,
    pub next_change: f64,
}

/// The years a snag of an old tree stands (at most; the draw takes a share).
const SNAG_YEARS: f64 = 14.0;
/// The years a charred trunk stands.
const CHARRED_YEARS: f64 = 15.0;
/// How many trees felled in each other's gaps make an opening the pioneers take.
const OPENING: usize = 3;

/// A site to ask about.
pub struct SiteInput<'a> {
    pub forest: &'a Forest,
    pub climate: &'a PlaceClimate,
    pub veg: &'a Vegetation,
    /// The site's hash (its draws).
    pub h: u64,
    /// The age of the stand about it when the world began.
    pub stand: f32,
    /// Its foot.
    pub x: i32,
    pub z: i32,
    /// The ground one site stands for (m²): a crown covering more is shared with neighbours.
    pub cell_area: f32,
}

/// One generation of trees on a site.
#[derive(Debug, Clone, Copy)]
struct Generation {
    species: usize,
    /// The year it took root.
    born: f64,
    /// The year it dies of age.
    dies: f64,
    /// Its draws.
    hash: u64,
    /// The year of the opening its succession began with: the shade-tolerant come up under
    /// it a third of its life later.
    opened: Option<f64>,
    /// A young stand on opened ground thins as the stand grows, whatever each tree's own pace:
    /// by the crowns of the place's usual species at the stand's age (species, year the stand
    /// began). Elsewhere a tree stands as often as its own crown leaves room.
    thin_by: Option<(usize, f64)>,
}

impl SiteInput<'_> {
    /// What stands on the site now; None where no species fits the place (the old shapes stand
    /// there).
    pub fn now(&self) -> Option<SiteNow> {
        let forest = self.forest;
        let first = forest.choose(self.climate, 0.0, unit_f32(self.h.rotate_left(40)))?;
        let sp = &forest.templates.species[first];
        let age = (self.stand * (0.5 + 0.8 * unit_f32(self.h.rotate_left(52))))
            .min(sp.lifespan_years * 0.97) as f64;
        let life =
            sp.lifespan_years as f64 * (0.97 + 0.15 * unit_f32(self.h.rotate_left(5)) as f64);
        let mut g = Generation {
            species: first,
            born: -age,
            dies: -age + life,
            hash: self.h,
            opened: None,
            thin_by: None,
        };
        let year = self.veg.year;
        // What has happened here: fires and clearings that took the ground, trees cut at the
        // foot (an opening when others were cut about it before).
        let mut events: SmallVec<[(f64, DisturbanceKind, bool); 4]> = SmallVec::new();
        let here = self.veg.at(self.x, self.z);
        for e in &here {
            if e.kind != DisturbanceKind::Felled {
                events.push((e.year, e.kind, true));
            }
        }
        for e in self.veg.felled(self.x, self.z) {
            let gaps = here
                .iter()
                .filter(|o| o.kind == DisturbanceKind::Felled && o.year <= e.year)
                .count();
            events.push((e.year, DisturbanceKind::Felled, gaps >= OPENING));
        }
        events.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut remains: Option<(SiteTree, f64)> = None;
        let mut next_event = 0;
        for _ in 0..12 {
            if let Some(&(t, kind, open)) = events.get(next_event).filter(|e| e.0 <= year)
                && t < g.dies
            {
                next_event += 1;
                let standing_remains = remains.filter(|(_, until)| t < *until);
                match kind {
                    DisturbanceKind::Felled => {
                        if standing_remains.is_some() {
                            // A snag or a stump cut away.
                            remains = None;
                            continue;
                        }
                        let Some(stood) = self.standing(&g, t) else {
                            continue;
                        };
                        let next = self.following(&g, t, open, first);
                        remains = Some((
                            SiteTree {
                                remains: Remains::Stump,
                                ..stood
                            },
                            next.born,
                        ));
                        g = next;
                    }
                    DisturbanceKind::Cleared => {
                        remains = None;
                        g = self.following(&g, t, true, first);
                    }
                    DisturbanceKind::Burned => {
                        let stood = standing_remains
                            .map(|(r, _)| r)
                            .or_else(|| self.standing(&g, t));
                        let next = self.following(&g, t, true, first);
                        let stands = unit_f32(next.hash.rotate_left(7)) as f64;
                        remains = stood
                            .filter(|s| s.stage >= Stage::Pole && s.remains != Remains::Stump)
                            .map(|s| {
                                (
                                    SiteTree {
                                        remains: Remains::Charred,
                                        ..s
                                    },
                                    t + CHARRED_YEARS * (0.2 + 0.8 * stands),
                                )
                            });
                        g = next;
                    }
                }
                continue;
            }
            if g.dies <= year {
                // Dead of age: an old canopy tree stands a while as a snag.
                let t = g.dies;
                let stood = remains
                    .filter(|(_, until)| t < *until)
                    .map(|(r, _)| r)
                    .or_else(|| self.standing(&g, t - 0.01));
                let next = self.following(&g, t, false, first);
                let stands = unit_f32(next.hash.rotate_left(7)) as f64;
                remains = stood
                    .filter(|s| {
                        s.remains == Remains::Living && !s.understory && s.stage >= Stage::Young
                    })
                    .map(|s| {
                        (
                            SiteTree {
                                stage: Stage::Snag,
                                remains: Remains::Snag,
                                ..s
                            },
                            t + SNAG_YEARS * (0.3 + 0.7 * stands),
                        )
                    })
                    .or(remains);
                g = next;
                continue;
            }
            break;
        }
        // What shows: the remains while they stand, else the living generation.
        let mut next_change = f64::INFINITY;
        let tree = match remains.filter(|(_, until)| year < *until) {
            Some((r, until)) => {
                next_change = until;
                Some(r)
            }
            None => self.standing(&g, year),
        };
        let sp = &forest.templates.species[g.species];
        if year < g.born {
            next_change = next_change.min(g.born);
        } else {
            let age = (year - g.born) as f32;
            let mut edge = Stage::next_age(sp, age);
            if edge.is_some_and(|a| g.born + a as f64 <= year) {
                edge = Stage::next_age(sp, age + 0.01);
            }
            if let Some(a) = edge {
                next_change = next_change.min(g.born + a as f64);
            }
            next_change = next_change.min(g.dies);
        }
        if let Some(e) = g.opened {
            let under = e + self.onset(&g);
            if year < under {
                next_change = next_change.min(under);
            }
        }
        Some(SiteNow { tree, next_change })
    }

    /// The years after the opening when the shade-tolerant come up under a generation: a third
    /// of its life.
    fn onset(&self, g: &Generation) -> f64 {
        self.forest.templates.species[g.species].lifespan_years as f64 / 3.0
    }

    /// What of a generation stands at a year: its tree where the canopy has room for it, else
    /// a young tree of the shade-tolerant understory (in the old forest always, under pioneers
    /// once they have grown a third of their lives), else nothing.
    fn standing(&self, g: &Generation, t: f64) -> Option<SiteTree> {
        let forest = self.forest;
        let variant = ((g.hash >> 56) as u8) % VARIANTS;
        let turn = Turn::from_hash(g.hash.rotate_left(13));
        if t >= g.born && t < g.dies {
            let sp = &forest.templates.species[g.species];
            let stage = Stage::of(sp, (t - g.born) as f32);
            // Crowns fill the canopy without heaping on each other: a tree stands on its site
            // as often as the site is a share of a crown (its own, or in a young stand on
            // opened ground the stand's).
            let r = match g.thin_by {
                Some((usual, began)) => {
                    let age = (t - began).max(0.0) as f32;
                    let stage = Stage::of(&forest.templates.species[usual], age);
                    forest.templates.get(usual, stage, variant).reach()
                }
                None => forest.templates.get(g.species, stage, variant).reach(),
            }
            .max(1) as f32;
            let p = (self.cell_area / (std::f32::consts::PI * r * r * 0.5)).min(1.0);
            if unit_f32(g.hash.rotate_left(17)) < p {
                return Some(SiteTree {
                    species: g.species,
                    stage,
                    variant,
                    turn,
                    remains: Remains::Living,
                    understory: false,
                });
            }
        }
        if g.opened.is_some_and(|e| t < e + self.onset(g)) {
            return None;
        }
        // The world as it began has its own understory; later ones are the gap-takers'.
        let roll = unit_f32(g.hash.rotate_left(29));
        let under = if g.hash == self.h {
            forest.choose(self.climate, 0.85, roll)
        } else {
            forest.choose_gap(self.climate, roll)
        }?;
        if unit_f32(g.hash.rotate_left(26)) >= 0.35 {
            return None;
        }
        let stage = if unit_f32(g.hash.rotate_left(33)) < 0.5 {
            Stage::Sapling
        } else {
            Stage::Pole
        };
        Some(SiteTree {
            species: under,
            stage,
            variant,
            turn,
            remains: Remains::Living,
            understory: true,
        })
    }

    /// The generation that takes a site after one ends at `t`: on opened ground the
    /// light-demanding first (the pioneers within a few years, others later), in a gap of the
    /// old forest the shade-tolerant.
    fn following(&self, g: &Generation, t: f64, open: bool, usual: usize) -> Generation {
        let forest = self.forest;
        let hash = mix64(g.hash ^ 0x9e37_79b9_7f4a_7c15);
        let roll = unit_f32(hash.rotate_left(40));
        let species = if open {
            forest.choose_open(self.climate, roll)
        } else {
            forest.choose_gap(self.climate, roll)
        }
        .unwrap_or(g.species);
        let sp = &forest.templates.species[species];
        let u = unit_f32(hash.rotate_left(3)) as f64;
        let pioneer = forest.niches[species].shade_tolerance < 0.3;
        let lag = match (open, pioneer) {
            (true, true) => 1.0 + 3.0 * u,
            (true, false) => 3.0 + 8.0 * u,
            (false, _) => 1.0 + 4.0 * u,
        };
        let born = t + lag;
        let life = sp.lifespan_years as f64 * (0.75 + 0.35 * unit_f32(hash.rotate_left(5)) as f64);
        Generation {
            species,
            born,
            dies: born + life,
            hash,
            opened: open.then_some(t),
            thin_by: open.then_some((usual, t + 2.0)),
        }
    }
}

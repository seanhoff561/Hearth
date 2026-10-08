//! The state of the vegetation (V2-6, docs/design/flora.md): the year the trees have grown to,
//! and what has disturbed the land (trees felled, ground cleared, land burned). The generator
//! grows the land from it (`cubegen::succession`); the server keeps it (`vegetation.json`) and
//! rebuilds the terrain it changes.

use std::sync::Arc;

use hearth_math::hash::{hash_2d, mix64, unit_f32};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// The side (m) of an ecological cell: the grid fire spreads over far from the player.
pub const ECO_CELL: i32 = 256;
/// The buckets disturbances are found by (it divides every planet's circumference).
const BUCKET: i32 = 16;
/// How far past its radius a disturbance's ragged edge may reach.
pub const EDGE: f32 = 1.15;
/// The side (m) of the squares a disturbance's exact shape is kept in (a fire's burned ground).
pub const PATCH: i32 = 4;
/// Years burned ground lies bare before the first herbs come.
pub const BARE_YEARS: f32 = 0.4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisturbanceKind {
    /// One tree cut down: gone but for its stump, with a gap in the canopy about it.
    Felled,
    /// Ground cleared of its trees and plants.
    Cleared,
    /// Burned: the trees killed stand charred and the ground lies bare a while.
    Burned,
}

/// One disturbance of the vegetation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Disturbance {
    pub kind: DisturbanceKind,
    /// When: years since the world began.
    pub year: f64,
    /// Where: its centre (a felled tree's foot).
    pub x: i32,
    pub z: i32,
    /// How far it reached (m); its edge is ragged.
    pub radius: f32,
    /// How much of the ground inside it took, 0–1 (a fire leaves unburned patches).
    #[serde(default = "whole")]
    pub severity: f32,
    /// Its exact shape where it has one (the ground a fire burned): the `PATCH`-metre squares
    /// it took, as (x, z) ÷ `PATCH`. Empty: the circle, ragged at the edge.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patches: Vec<[i32; 2]>,
}

fn whole() -> f32 {
    1.0
}

impl Disturbance {
    /// The draws of its ragged edge and spared patches.
    fn seed(&self) -> u64 {
        mix64(hash_2d(0x7e9e_d157, self.x, self.z) ^ self.year.to_bits())
    }
}

/// The vegetation as saved (`vegetation.json`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VegetationSave {
    #[serde(default)]
    pub disturbances: Vec<Disturbance>,
}

/// Smooth value noise in 0–1 with features about `scale` blocks across.
fn patchy(seed: u64, x: i32, z: i32, scale: f32) -> f32 {
    let (fx, fz) = (x as f32 / scale, z as f32 / scale);
    let (x0, z0) = (fx.floor(), fz.floor());
    let (tx, tz) = (fx - x0, fz - z0);
    let (i, k) = (x0 as i32, z0 as i32);
    let c = |a: i32, b: i32| unit_f32(hash_2d(seed, a, b));
    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sz = tz * tz * (3.0 - 2.0 * tz);
    let a = c(i, k) + (c(i + 1, k) - c(i, k)) * sx;
    let b = c(i, k + 1) + (c(i + 1, k + 1) - c(i, k + 1)) * sx;
    a + (b - a) * sz
}

/// The disturbances, in the order they happened, found by place.
#[derive(Debug, Default, Clone)]
pub struct Disturbances {
    list: Vec<Disturbance>,
    /// The squares of each one that has an exact shape.
    masks: Vec<Option<FxHashSet<(i32, i32)>>>,
    buckets: FxHashMap<(i32, i32), SmallVec<[u32; 4]>>,
    /// The planet's circumference (0: no wrapping).
    circumference: i32,
}

impl Disturbances {
    fn new(list: Vec<Disturbance>, circumference: i32) -> Self {
        let mut d = Self {
            list: Vec::with_capacity(list.len()),
            masks: Vec::with_capacity(list.len()),
            buckets: FxHashMap::default(),
            circumference,
        };
        for e in list {
            d.push(e);
        }
        d
    }

    fn wrap(&self, x: i32) -> i32 {
        if self.circumference > 0 {
            x.rem_euclid(self.circumference)
        } else {
            x
        }
    }

    fn push(&mut self, mut e: Disturbance) {
        e.x = self.wrap(e.x);
        let i = self.list.len() as u32;
        if e.patches.is_empty() {
            let r = (e.radius.max(0.0) * EDGE).ceil() as i32 + 1;
            for bz in (e.z - r).div_euclid(BUCKET)..=(e.z + r).div_euclid(BUCKET) {
                for bx in (e.x - r).div_euclid(BUCKET)..=(e.x + r).div_euclid(BUCKET) {
                    let key = (self.wrap(bx * BUCKET).div_euclid(BUCKET), bz);
                    let ids = self.buckets.entry(key).or_default();
                    if ids.last() != Some(&i) {
                        ids.push(i);
                    }
                }
            }
            self.masks.push(None);
        } else {
            let mut mask = FxHashSet::default();
            for &[px, pz] in &e.patches {
                let wx = self.wrap(px * PATCH);
                mask.insert((wx.div_euclid(PATCH), pz));
                let key = (wx.div_euclid(BUCKET), (pz * PATCH).div_euclid(BUCKET));
                let ids = self.buckets.entry(key).or_default();
                if !ids.contains(&i) {
                    ids.push(i);
                }
            }
            self.masks.push(Some(mask));
        }
        self.list.push(e);
    }

    /// Horizontal distance (m) from a disturbance's centre, across the seam where the planet
    /// wraps.
    fn distance(&self, e: &Disturbance, x: i32, z: i32) -> f32 {
        let mut dx = (self.wrap(x) - e.x) as f32;
        if self.circumference > 0 {
            let c = self.circumference as f32;
            if dx > c / 2.0 {
                dx -= c;
            } else if dx < -c / 2.0 {
                dx += c;
            }
        }
        let dz = (z - e.z) as f32;
        (dx * dx + dz * dz).sqrt()
    }

    /// Whether the `i`th disturbance took a place: in its exact shape, or inside its ragged
    /// edge and not in a patch it spared.
    fn takes(&self, i: usize, x: i32, z: i32) -> bool {
        let e = &self.list[i];
        if let Some(mask) = &self.masks[i] {
            return mask.contains(&(self.wrap(x).div_euclid(PATCH), z.div_euclid(PATCH)));
        }
        let d = self.distance(e, x, z);
        if d > e.radius * EDGE {
            return false;
        }
        let seed = e.seed();
        let wx = self.wrap(x);
        if d > e.radius * (0.85 + 0.3 * patchy(seed, wx, z, 7.0)) {
            return false;
        }
        e.severity >= 1.0 || patchy(seed ^ 0x5eed, wx, z, 11.0) < e.severity
    }

    /// The disturbances that may reach a place, oldest first, with their index.
    fn near(&self, x: i32, z: i32) -> impl Iterator<Item = (usize, &Disturbance)> {
        let key = (self.wrap(x).div_euclid(BUCKET), z.div_euclid(BUCKET));
        self.buckets
            .get(&key)
            .into_iter()
            .flatten()
            .map(|&i| (i as usize, &self.list[i as usize]))
    }
}

/// The vegetation the generator grows: the year the trees have grown to and the disturbances
/// so far. Cheap to clone; a snapshot.
#[derive(Debug, Clone, Default)]
pub struct Vegetation {
    /// Years since the world began.
    pub year: f64,
    disturbances: Arc<Disturbances>,
}

impl Vegetation {
    /// From a save, on a planet of this circumference, at a year.
    pub fn new(save: &VegetationSave, circumference: i32, year: f64) -> Self {
        let mut list = save.disturbances.clone();
        list.sort_by(|a, b| a.year.total_cmp(&b.year));
        Self {
            year,
            disturbances: Arc::new(Disturbances::new(list, circumference)),
        }
    }

    pub fn save(&self) -> VegetationSave {
        VegetationSave {
            disturbances: self.disturbances.list.clone(),
        }
    }

    /// The same disturbances, grown to another year.
    pub fn at_year(&self, year: f64) -> Self {
        Self {
            year,
            disturbances: self.disturbances.clone(),
        }
    }

    /// With one more disturbance (the newest).
    pub fn with(&self, e: Disturbance) -> Self {
        let mut d = (*self.disturbances).clone();
        d.push(e);
        Self {
            year: self.year,
            disturbances: Arc::new(d),
        }
    }

    /// Every disturbance, oldest first.
    pub fn disturbances(&self) -> &[Disturbance] {
        &self.disturbances.list
    }

    /// Whether two snapshots hold the same disturbances (whatever their years).
    pub fn same_disturbances(&self, other: &Vegetation) -> bool {
        Arc::ptr_eq(&self.disturbances, &other.disturbances)
    }

    /// The disturbances added since an older snapshot this one grew from.
    pub fn added_since(&self, older: &Vegetation) -> &[Disturbance] {
        if self.same_disturbances(older) {
            return &[];
        }
        let n = older
            .disturbances
            .list
            .len()
            .min(self.disturbances.list.len());
        &self.disturbances.list[n..]
    }

    /// Horizontal distance (m) between a disturbance and a place, across the seam.
    pub fn distance(&self, e: &Disturbance, x: i32, z: i32) -> f32 {
        self.disturbances.distance(e, x, z)
    }

    /// What has happened at a place by this year, oldest first: the disturbances that took
    /// its ground (a felled tree's: the gap about it).
    pub fn at(&self, x: i32, z: i32) -> SmallVec<[&Disturbance; 4]> {
        let d = &*self.disturbances;
        if d.list.is_empty() {
            return SmallVec::new();
        }
        d.near(x, z)
            .filter(|(i, e)| e.year <= self.year && d.takes(*i, x, z))
            .map(|(_, e)| e)
            .collect()
    }

    /// The trees felled with their foot at a place by this year, oldest first.
    pub fn felled(&self, x: i32, z: i32) -> SmallVec<[&Disturbance; 2]> {
        let d = &*self.disturbances;
        if d.list.is_empty() {
            return SmallVec::new();
        }
        let wx = d.wrap(x);
        d.near(x, z)
            .map(|(_, e)| e)
            .filter(|e| {
                e.kind == DisturbanceKind::Felled && e.x == wx && e.z == z && e.year <= self.year
            })
            .collect()
    }

    /// The newest disturbance of the ground at a place and how long ago it was (years).
    pub fn ground(&self, x: i32, z: i32) -> Option<(DisturbanceKind, f32)> {
        self.at(x, z)
            .last()
            .map(|e| (e.kind, (self.year - e.year).max(0.0) as f32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn burn(x: i32, z: i32, radius: f32, severity: f32, year: f64) -> Disturbance {
        Disturbance {
            kind: DisturbanceKind::Burned,
            year,
            x,
            z,
            radius,
            severity,
            patches: Vec::new(),
        }
    }

    #[test]
    fn a_fire_keeps_the_exact_ground_it_burned() {
        // Burned squares in an L about (40, 40), across the seam on a small planet.
        let c = 1024;
        let patches = vec![[-1, 10], [0, 10], [1, 10], [1, 11], [1, 12]];
        let v = Vegetation::new(
            &VegetationSave {
                disturbances: vec![Disturbance {
                    kind: DisturbanceKind::Burned,
                    year: 1.0,
                    x: 2,
                    z: 44,
                    radius: 12.0,
                    severity: 1.0,
                    patches,
                }],
            },
            c,
            2.0,
        );
        assert!(!v.at(1, 41).is_empty(), "in the square (0, 10)");
        assert!(
            !v.at(c - 2, 43).is_empty(),
            "in the square (-1, 10), over the seam"
        );
        assert!(!v.at(5, 49).is_empty(), "in the square (1, 12)");
        assert!(v.at(1, 49).is_empty(), "not in the L");
        assert!(v.at(20, 41).is_empty());
        let saved = serde_json::to_string(&v.save()).expect("json");
        let back: VegetationSave = serde_json::from_str(&saved).expect("json");
        assert_eq!(back, v.save());
    }

    #[test]
    fn a_disturbance_takes_its_ground_from_its_year_with_a_ragged_edge() {
        let v = Vegetation::new(
            &VegetationSave {
                disturbances: vec![burn(100, -40, 30.0, 1.0, 2.0)],
            },
            65_536,
            1.5,
        );
        // Not yet.
        assert!(v.at(100, -40).is_empty());
        let v = v.at_year(3.0);
        assert_eq!(
            v.ground(100, -40),
            Some((DisturbanceKind::Burned, 1.0)),
            "the centre burned a year ago"
        );
        assert!(v.at(100 + 20, -40).len() == 1, "well inside");
        assert!(v.at(100 + 40, -40).is_empty(), "outside");
        // The edge wanders: some places at 30 m are taken and some are not.
        let edge: Vec<bool> = (0..64)
            .map(|k| {
                let a = k as f32 / 64.0 * std::f32::consts::TAU;
                let (x, z) = (100 + (30.0 * a.cos()) as i32, -40 + (30.0 * a.sin()) as i32);
                !v.at(x, z).is_empty()
            })
            .collect();
        assert!(
            edge.iter().any(|t| *t) && edge.iter().any(|t| !*t),
            "{edge:?}"
        );
    }

    #[test]
    fn a_light_fire_spares_patches_and_the_seam_is_crossed() {
        let c = 16_384;
        let v = Vegetation::default()
            .with(burn(5, 0, 60.0, 0.5, 0.0))
            .at_year(1.0);
        let v = Vegetation::new(&v.save(), c, 1.0);
        let mut taken = 0;
        let mut n = 0;
        for z in (-30..30).step_by(3) {
            for x in (-30..30).step_by(3) {
                n += 1;
                // West of the seam, the place is at the far end of the planet's x.
                let wx = if x < 0 { x + c } else { x };
                if !v.at(wx, z).is_empty() {
                    taken += 1;
                }
                assert_eq!(v.at(wx, z).len(), v.at(x, z).len());
            }
        }
        let share = taken as f32 / n as f32;
        assert!((0.25..0.75).contains(&share), "{share}");
    }

    #[test]
    fn felled_trees_are_found_by_their_foot_and_new_ones_are_added_after() {
        let v = Vegetation::new(&VegetationSave::default(), 65_536, 4.0);
        let one = v.with(Disturbance {
            kind: DisturbanceKind::Felled,
            year: 4.0,
            x: -7,
            z: 12,
            radius: 3.0,
            severity: 1.0,
            patches: Vec::new(),
        });
        assert_eq!(one.felled(-7, 12).len(), 1);
        assert_eq!(one.felled(65_536 - 7, 12).len(), 1, "the same place");
        assert!(one.felled(-6, 12).is_empty());
        assert_eq!(one.added_since(&v).len(), 1);
        assert!(one.added_since(&one).is_empty());
        // The gap about it is disturbed ground.
        assert_eq!(
            one.ground(-6, 13).map(|g| g.0),
            Some(DisturbanceKind::Felled)
        );
        let saved = serde_json::to_string(&one.save()).expect("json");
        let back: VegetationSave = serde_json::from_str(&saved).expect("json");
        assert_eq!(back, one.save());
    }
}

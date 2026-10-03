//! Finding the way (V2-7 (e), v2 §7.3): paths over the ground for walkers, wading where it is
//! shallow and swimming where it is deep for those that swim; through the water for fish;
//! flights over the trees for birds, to a perch or to the ground; trunks for climbers.
//!
//! Ways are searched on the loaded blocks a column at a time (A*: a body steps to a neighbouring
//! column if the ground there is within the step it climbs or drops, and not in water deeper
//! than it wades unless it swims; diagonal steps only where both sides are open, so that it does
//! not cut between trunks), the search bounded so that a herd turning at once to flee costs a
//! frame; then drawn straight wherever the ground between allows. A search that cannot reach
//! the goal in its bound goes as near as it found.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use glam::{DVec2, DVec3};
use hearth_content::schema::fauna::BodyPlan;
use rustc_hash::FxHashMap;

use crate::live::{Cell, Footing, Ground};
use crate::species::Species;

/// How a body goes over the ground.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Walker {
    /// The highest step it takes up, and the deepest down (m).
    pub climb: f64,
    pub drop: f64,
    /// The deepest water it wades (m); deeper, it swims if it can.
    pub wade: f64,
    pub swims: bool,
    /// A fish: only in water.
    pub fish: bool,
}

/// The water a fish needs under it (m).
pub const FISH_DEPTH: f64 = 0.4;

impl Walker {
    pub fn of(sp: &Species) -> Self {
        let climb = sp.climb_m() as f64;
        let fish = matches!(
            sp.plan,
            BodyPlan::FishFusiform | BodyPlan::FishFlat | BodyPlan::Eel
        );
        Self {
            climb,
            drop: (climb * 2.5).max(1.2),
            wade: (sp.shoulder_m as f64 * 0.6).max(0.04),
            swims: sp.swim_m_s.is_some() || sp.aquatic || sp.marine,
            fish,
        }
    }

    /// The level a body moves at over a footing: the water's surface over water (all one level
    /// to a swimmer, and the bank is stepped onto from it), the ground otherwise.
    pub fn level(f: &Footing) -> f64 {
        if f.water { f.level() } else { f.y }
    }

    /// Whether a body at `level` may step onto a footing; the cost of the step against level
    /// ground if so.
    pub fn step(&self, level: f64, to: &Footing) -> Option<f64> {
        if self.fish {
            return (to.water && to.depth >= FISH_DEPTH).then_some(1.0);
        }
        let deep = to.water && to.depth > self.wade;
        if deep && !self.swims {
            return None;
        }
        let dy = Self::level(to) - level;
        if dy > self.climb || -dy > self.drop {
            return None;
        }
        let wet = if deep {
            3.0
        } else if to.water {
            1.4
        } else {
            1.0
        };
        Some((1.0 + 0.6 * dy.abs()) * wet)
    }
}

/// A way found: its points (to the first step and on to the end), and whether it reaches the
/// goal rather than only as near as the search went.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Way {
    pub points: Vec<DVec3>,
    pub reaches: bool,
}

#[derive(Debug, Clone, Copy)]
struct Node {
    g: f64,
    level: f64,
    y: f64,
    parent: (i32, i32),
    closed: bool,
}

const DIRS: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

/// The way from `from` to the column of `to` for a walker, searching at most `budget` columns.
pub fn find_way(ground: &dyn Ground, from: DVec3, to: DVec2, w: &Walker, budget: usize) -> Way {
    let key = |x: f64, z: f64| (x.floor() as i32, z.floor() as i32);
    let start = key(from.x, from.z);
    let goal = key(to.x, to.y);
    let h =
        |k: (i32, i32)| (((k.0 - goal.0) as f64).powi(2) + ((k.1 - goal.1) as f64).powi(2)).sqrt();
    let start_level = ground
        .footing(from.x, from.z, from.y)
        .map_or(from.y, |f| Walker::level(&f));
    let mut nodes: FxHashMap<(i32, i32), Node> = FxHashMap::default();
    let mut feet: FxHashMap<(i32, i32), Option<Footing>> = FxHashMap::default();
    let mut heap = BinaryHeap::new();
    nodes.insert(
        start,
        Node {
            g: 0.0,
            level: start_level,
            y: from.y,
            parent: start,
            closed: false,
        },
    );
    let cost_key = |f: f64| (f * 1000.0) as u64;
    heap.push(Reverse((cost_key(h(start)), start)));
    let (mut best, mut best_h) = (start, h(start));
    let mut reaches = false;
    let mut expanded = 0;
    while let Some(Reverse((_, k))) = heap.pop() {
        let n = nodes[&k];
        if n.closed {
            continue;
        }
        if let Some(m) = nodes.get_mut(&k) {
            m.closed = true;
        }
        let hk = h(k);
        if hk < best_h {
            (best, best_h) = (k, hk);
        }
        if k == goal {
            (best, reaches) = (k, true);
            break;
        }
        expanded += 1;
        if expanded >= budget {
            break;
        }
        // The footing of a column (the first one asked for it: near the level searched from).
        let mut foot = |c: (i32, i32), level: f64| -> Option<Footing> {
            *feet
                .entry(c)
                .or_insert_with(|| ground.footing(c.0 as f64 + 0.5, c.1 as f64 + 0.5, level))
        };
        for (dx, dz) in DIRS {
            let c = (k.0 + dx, k.1 + dz);
            let Some(f) = foot(c, n.level) else {
                continue;
            };
            let Some(factor) = w.step(n.level, &f) else {
                continue;
            };
            if dx != 0 && dz != 0 {
                // Not between two trunks or around a corner.
                let side =
                    |c: (i32, i32), foot: &mut dyn FnMut((i32, i32), f64) -> Option<Footing>| {
                        foot(c, n.level).and_then(|f| w.step(n.level, &f)).is_some()
                    };
                if !side((k.0 + dx, k.1), &mut foot) || !side((k.0, k.1 + dz), &mut foot) {
                    continue;
                }
            }
            let len = if dx != 0 && dz != 0 {
                std::f64::consts::SQRT_2
            } else {
                1.0
            };
            let g = n.g + len * factor;
            if let Some(m) = nodes.get(&c)
                && (m.closed || m.g <= g)
            {
                continue;
            }
            nodes.insert(
                c,
                Node {
                    g,
                    level: Walker::level(&f),
                    y: if w.fish {
                        f.level() - f.depth * 0.5
                    } else {
                        f.y
                    },
                    parent: k,
                    closed: false,
                },
            );
            heap.push(Reverse((cost_key(g + h(c)), c)));
        }
    }
    // Back from where it got to.
    let mut points = Vec::new();
    let mut k = best;
    while k != start {
        let n = nodes[&k];
        points.push(DVec3::new(k.0 as f64 + 0.5, n.y, k.1 as f64 + 0.5));
        k = n.parent;
    }
    points.reverse();
    Way {
        points: smooth(ground, from, &points, w),
        reaches,
    }
}

/// Whether a walker goes straight from `a` to `b` (the ground every half metre within its step,
/// the water within its rules).
pub fn straight(ground: &dyn Ground, a: DVec3, b: DVec3, w: &Walker) -> bool {
    let d = DVec2::new(b.x - a.x, b.z - a.z);
    let n = (d.length() / 0.5).ceil().max(1.0) as usize;
    let mut level = ground
        .footing(a.x, a.z, a.y)
        .map_or(a.y, |f| Walker::level(&f));
    for i in 1..=n {
        let t = i as f64 / n as f64;
        let (x, z) = (a.x + d.x * t, a.z + d.y * t);
        let Some(f) = ground.footing(x, z, level) else {
            return false;
        };
        if w.step(level, &f).is_none() {
            return false;
        }
        level = Walker::level(&f);
    }
    true
}

/// A path drawn straight wherever the ground allows: from each point, on to the farthest of
/// the next few that it goes straight to.
fn smooth(ground: &dyn Ground, from: DVec3, points: &[DVec3], w: &Walker) -> Vec<DVec3> {
    let mut out = Vec::new();
    let mut at = from;
    let mut i = 0;
    while i < points.len() {
        let last = (i + 12).min(points.len() - 1);
        let mut j = last;
        while j > i && !straight(ground, at, points[j], w) {
            j -= 1;
        }
        out.push(points[j]);
        at = points[j];
        i = j + 1;
    }
    out
}

/// The top of what stands in a column: the highest trunk, limb, leaf or solid block within
/// forty metres over the ground (the ground itself where nothing stands).
pub fn canopy(ground: &dyn Ground, x: f64, z: f64) -> Option<f64> {
    let f = ground.top(x, z)?;
    let (ix, iz) = (x.floor() as i32, z.floor() as i32);
    let base = f.level().floor() as i32;
    for y in (base..base + 40).rev() {
        match ground.cell(ix, y, iz) {
            Some(Cell::Open | Cell::Plant | Cell::Water) | None => continue,
            Some(_) => return Some(y as f64 + 1.0),
        }
    }
    Some(f.level())
}

/// A flight from one place to another over what stands between: up, level a few metres over
/// the highest trees on the way, down to the end; climbing and coming down as steeply as the
/// trees near either end ask.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flight {
    pub from: DVec3,
    pub to: DVec3,
    pub cruise: f64,
    /// Rise over run climbing, and coming down.
    pub climb: f64,
    pub descent: f64,
}

/// How steeply a bird climbs and comes down at the least, and at the most (rise over run).
const CLIMB: f64 = 0.8;
const DESCENT: f64 = 0.6;
const STEEPEST: f64 = 4.0;

impl Flight {
    pub fn plan(ground: &dyn Ground, from: DVec3, to: DVec3) -> Self {
        let d = DVec2::new(to.x - from.x, to.z - from.z);
        let len = d.length();
        let n = (len / 1.5).ceil().max(1.0) as usize;
        let mut top = from.y.max(to.y);
        let (mut climb, mut descent) = (CLIMB, DESCENT);
        for k in 0..=n {
            let t = k as f64 / n as f64;
            let Some(c) = canopy(ground, from.x + d.x * t, from.z + d.y * t) else {
                continue;
            };
            top = top.max(c);
            // Clear by a metre and a half what stands near either end (not the perch itself).
            let s = len * t;
            if s > 1.5 {
                climb = climb.max((c + 1.5 - from.y) / s);
            }
            if len - s > 1.5 {
                descent = descent.max((c + 1.5 - to.y) / (len - s));
            }
        }
        Self {
            from,
            to,
            cruise: top + 4.0,
            climb: climb.min(STEEPEST),
            descent: descent.min(STEEPEST),
        }
    }

    /// Its length over the ground.
    pub fn length(&self) -> f64 {
        DVec2::new(self.to.x - self.from.x, self.to.z - self.from.z).length()
    }

    /// Where the bird is `s` metres along.
    pub fn at(&self, s: f64) -> DVec3 {
        let len = self.length();
        let t = if len > 1e-6 {
            (s / len).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let s = t * len;
        let up = self.from.y + s * self.climb;
        let down = self.to.y + (len - s) * self.descent;
        let y = up
            .min(down)
            .min(self.cruise.max(self.from.y).max(self.to.y));
        DVec3::new(
            self.from.x + (self.to.x - self.from.x) * t,
            y,
            self.from.z + (self.to.z - self.from.z) * t,
        )
    }
}

/// Somewhere to perch near a point: the top of a tree's foliage or a limb with air over it;
/// of those within `radius` (looked for every two metres), the nearest to the point.
pub fn perch_near(ground: &dyn Ground, at: DVec2, radius: f64) -> Option<DVec3> {
    let r = radius.ceil() as i32;
    let mut best: Option<(f64, DVec3)> = None;
    for dz in (-r..=r).step_by(2) {
        for dx in (-r..=r).step_by(2) {
            let d2 = (dx * dx + dz * dz) as f64;
            if d2 > radius * radius || best.is_some_and(|b| b.0 <= d2) {
                continue;
            }
            let (x, z) = (at.x + dx as f64, at.y + dz as f64);
            let Some(f) = ground.top(x, z) else {
                continue;
            };
            let (ix, iz) = (x.floor() as i32, z.floor() as i32);
            let g = f.level().floor() as i32;
            for y in (g + 2..g + 30).rev() {
                match ground.cell(ix, y, iz) {
                    Some(Cell::Open | Cell::Plant) | None => continue,
                    Some(Cell::Leaves | Cell::Limb) => {
                        if matches!(ground.cell(ix, y + 1, iz), Some(Cell::Open | Cell::Plant)) {
                            let p = DVec3::new(ix as f64 + 0.5, y as f64 + 1.0, iz as f64 + 0.5);
                            best = Some((d2, p));
                        }
                        break;
                    }
                    _ => break,
                }
            }
        }
    }
    best.map(|b| b.1)
}

/// Water to drink near a place: the nearest water within `radius` (looked for every two
/// metres, ring by ring), and the dry ground at its edge toward the place.
pub fn water_near(ground: &dyn Ground, at: DVec3, radius: f64) -> Option<(DVec3, DVec3)> {
    let mut r = 2.0;
    while r <= radius {
        let n = ((r * std::f64::consts::TAU / 2.0).ceil() as usize).max(6);
        for k in 0..n {
            let a = k as f64 / n as f64 * std::f64::consts::TAU;
            let (x, z) = (at.x + a.cos() * r, at.z + a.sin() * r);
            let Some(f) = ground.footing(x, z, at.y) else {
                continue;
            };
            if !f.water || f.depth < 0.1 {
                continue;
            }
            // Back toward the place to dry ground.
            let back = DVec2::new(at.x - x, at.z - z).normalize_or(DVec2::X);
            let mut d = 0.5;
            while d < r {
                let (bx, bz) = (x + back.x * d, z + back.y * d);
                match ground.footing(bx, bz, at.y) {
                    Some(b) if !b.water => {
                        return Some((DVec3::new(bx, b.y, bz), DVec3::new(x, f.level(), z)));
                    }
                    Some(_) => d += 0.5,
                    None => break,
                }
            }
        }
        r += 2.0;
    }
    None
}

/// A tree to climb near a place: the middle of its trunk's column where it meets the ground,
/// and how high the trunk goes; the nearest within `radius`.
pub fn trunk_near(ground: &dyn Ground, at: DVec3, radius: f64) -> Option<(DVec3, f64)> {
    let r = radius.ceil() as i32;
    let (cx, cz) = (at.x.floor() as i32, at.z.floor() as i32);
    let y0 = at.y.floor() as i32;
    let mut best: Option<(i32, DVec3, f64)> = None;
    for dz in -r..=r {
        for dx in -r..=r {
            let d2 = dx * dx + dz * dz;
            if d2 as f64 > radius * radius || best.is_some_and(|b| b.0 <= d2) {
                continue;
            }
            let (x, z) = (cx + dx, cz + dz);
            // The trunk's foot near the ground the climber stands on.
            let Some(base) = (y0 - 1..=y0 + 2).find(|y| ground.cell(x, *y, z) == Some(Cell::Trunk))
            else {
                continue;
            };
            let mut top = base;
            while top < base + 40 && ground.cell(x, top + 1, z) == Some(Cell::Trunk) {
                top += 1;
            }
            let height = (top + 1 - base) as f64;
            if height >= 3.0 {
                best = Some((
                    d2,
                    DVec3::new(x as f64 + 0.5, base as f64, z as f64 + 0.5),
                    height,
                ));
            }
        }
    }
    best.map(|b| (b.1, b.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Level ground at 10 with a wall along x = 5 (z from -10 to 10) and a river across x from
    /// 12 to 15, 1.5 m deep (its surface at 9.8).
    struct Field;

    impl Ground for Field {
        fn footing(&self, x: f64, z: f64, _y: f64) -> Option<Footing> {
            let (ix, iz) = (x.floor() as i32, z.floor() as i32);
            if ix == 5 && (-10..=10).contains(&iz) {
                return None;
            }
            if (12..15).contains(&ix) {
                return Some(Footing {
                    y: 8.3,
                    water: true,
                    depth: 1.5,
                });
            }
            Some(Footing::dry(10.0))
        }

        fn top(&self, x: f64, z: f64) -> Option<Footing> {
            self.footing(x, z, 10.0)
        }
    }

    fn deer() -> Walker {
        Walker {
            climb: 1.1,
            drop: 2.7,
            wade: 0.7,
            swims: true,
            fish: false,
        }
    }

    #[test]
    fn a_way_goes_round_a_wall() {
        let w = Walker {
            swims: false,
            ..deer()
        };
        let way = find_way(
            &Field,
            DVec3::new(0.5, 10.0, 0.5),
            DVec2::new(9.5, 0.5),
            &w,
            4000,
        );
        assert!(way.reaches, "{way:?}");
        // Round the wall's end.
        assert!(
            way.points.iter().any(|p| p.z.abs() > 10.0),
            "{:?}",
            way.points
        );
        // Every leg is walkable.
        let mut at = DVec3::new(0.5, 10.0, 0.5);
        for p in &way.points {
            assert!(straight(&Field, at, *p, &w), "{at} to {p}");
            at = *p;
        }
    }

    #[test]
    fn a_swimmer_crosses_the_river_and_a_non_swimmer_does_not() {
        let from = DVec3::new(8.5, 10.0, 0.5);
        let to = DVec2::new(18.5, 0.5);
        let swims = find_way(&Field, from, to, &deer(), 4000);
        assert!(swims.reaches);
        let dry = Walker {
            swims: false,
            ..deer()
        };
        let stays = find_way(&Field, from, to, &dry, 4000);
        assert!(!stays.reaches);
        assert!(
            stays.points.iter().all(|p| p.x < 12.0),
            "{:?}",
            stays.points
        );
    }

    #[test]
    fn a_fish_keeps_to_the_water() {
        let fish = Walker {
            fish: true,
            ..deer()
        };
        let way = find_way(
            &Field,
            DVec3::new(12.5, 9.0, 0.5),
            DVec2::new(14.5, 30.5),
            &fish,
            4000,
        );
        assert!(way.reaches);
        assert!(way.points.iter().all(|p| (12.0..15.0).contains(&p.x)));
        // Out of the water it does not go.
        let out = find_way(
            &Field,
            DVec3::new(12.5, 9.0, 0.5),
            DVec2::new(20.5, 0.5),
            &fish,
            4000,
        );
        assert!(!out.reaches);
    }

    #[test]
    fn a_flight_rises_over_the_trees_and_comes_down_at_its_end() {
        // A tree's crown at x 10..13 up to 25 m.
        struct Wood;
        impl Ground for Wood {
            fn footing(&self, _x: f64, _z: f64, _y: f64) -> Option<Footing> {
                Some(Footing::dry(10.0))
            }
            fn top(&self, x: f64, z: f64) -> Option<Footing> {
                self.footing(x, z, 10.0)
            }
            fn cell(&self, x: i32, y: i32, _z: i32) -> Option<Cell> {
                Some(if y < 10 {
                    Cell::Solid
                } else if (10..13).contains(&x) && y < 25 {
                    Cell::Leaves
                } else {
                    Cell::Open
                })
            }
        }
        let f = Flight::plan(
            &Wood,
            DVec3::new(0.0, 10.0, 0.0),
            DVec3::new(30.0, 10.0, 0.0),
        );
        assert!(f.cruise >= 29.0);
        let mid = f.at(11.5);
        assert!(mid.y > 25.0, "over the crown: {mid}");
        assert!((f.at(0.0).y - 10.0).abs() < 1e-6);
        assert!((f.at(f.length()).y - 10.0).abs() < 1e-6);
        // A perch on the crown.
        let p = perch_near(&Wood, DVec2::new(14.0, 0.0), 6.0).expect("a perch");
        assert!(
            (p.y - 25.0).abs() < 1e-6 && (10.0..13.0).contains(&p.x),
            "{p}"
        );
    }
}

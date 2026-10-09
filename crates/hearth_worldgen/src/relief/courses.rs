//! The parent's rivers as a tile of a level reads them: each cell met along their courses read
//! once, the water each flows on to, and the bed each lies at — at or above every bed in the next
//! stretch of its course, so every tile lays the same bed whatever part of the river it holds.

use rustc_hash::FxHashMap;

use super::{Cell, Relief, VALLEY};

/// Cells followed down a course for a bed ([`Courses::bed`]): some tiles' width at the parent's
/// cells, past any tile that holds a part of the river.
const AHEAD: usize = 32;
/// Cells followed down a course for the water it flows on to.
const TO_WATER: usize = 100;

/// A river runs on its lake's surface where it crosses one.
fn top(c: &Cell) -> f32 {
    if c.lake.is_finite() {
        c.h.max(c.lake)
    } else {
        c.h
    }
}

/// The parent's courses about a tile of `level`.
pub(super) struct Courses<'a> {
    relief: &'a Relief,
    level: usize,
    cells: FxHashMap<(i64, i64), Cell>,
    floors: FxHashMap<(i64, i64), f32>,
    own: FxHashMap<(i64, i64, bool), f32>,
    beds: FxHashMap<(i64, i64, bool), f32>,
}

impl<'a> Courses<'a> {
    pub(super) fn new(relief: &'a Relief, level: usize) -> Self {
        Self {
            relief,
            level,
            cells: FxHashMap::default(),
            floors: FxHashMap::default(),
            own: FxHashMap::default(),
            beds: FxHashMap::default(),
        }
    }

    /// The parent's cell at (i, j).
    pub(super) fn cell(&mut self, i: i64, j: i64) -> Cell {
        let (relief, level) = (self.relief, self.level);
        *self
            .cells
            .entry((i, j))
            .or_insert_with(|| relief.cell(level - 1, i, j))
    }

    /// The cell whose river is the main stream running into (i, j): of its neighbours draining
    /// to it with at least `min_q`, the one with the most water (the first found of equals).
    pub(super) fn upstream(&mut self, i: i64, j: i64, min_q: f32) -> Option<(i64, i64)> {
        let count = self.relief.count(self.level - 1);
        let mut best: Option<((i64, i64), f32)> = None;
        for dj in -1..=1 {
            for di in -1..=1 {
                if (di, dj) == (0, 0) {
                    continue;
                }
                let c = self.cell(i + di, j + dj);
                let Some((ri, rj)) = c.receiver else {
                    continue;
                };
                if rj == j
                    && (ri - i).rem_euclid(count) == 0
                    && c.q >= min_q
                    && best.is_none_or(|(_, q)| c.q > q)
                {
                    best = Some(((i + di, j + dj), c.q));
                }
            }
        }
        best.map(|(at, _)| at)
    }

    /// The level of the water a cell's river flows on to: the first lake's surface or the sea's
    /// level (0) down its course, followed up to [`TO_WATER`] cells; −∞ if none comes.
    fn floor(&mut self, i: i64, j: i64) -> f32 {
        if let Some(&f) = self.floors.get(&(i, j)) {
            return f;
        }
        let (mut ci, mut cj) = (i, j);
        let mut floor = f32::NEG_INFINITY;
        for _ in 0..TO_WATER {
            let c = self.cell(ci, cj);
            if c.sea {
                floor = 0.0;
                break;
            }
            if c.lake.is_finite() {
                floor = c.lake;
                break;
            }
            match c.receiver {
                Some(r) => (ci, cj) = r,
                None => break,
            }
        }
        self.floors.insert((i, j), floor);
        floor
    }

    /// A river's bed in a cell by itself: below the parent's surface by a share of the relief
    /// this level adds there at the parent's cells' scale (keeping half its height above the sea, so lowland rivers still
    /// fall to it), but never below the water it flows on to: a river on land falls to the sea's
    /// level, not to the sea floor, and to a lake's surface. A lake's outflow leaves at its
    /// surface, and an inflow ends there.
    fn own_bed(&mut self, i: i64, j: i64, land: bool) -> f32 {
        if let Some(&b) = self.own.get(&(i, j, land)) {
            return b;
        }
        let c = self.cell(i, j);
        let b = if c.lake.is_finite() {
            top(&c)
        } else {
            let floor = self.floor(i, j);
            let r = self.relief;
            let at = r.node(self.level - 1, i, j);
            let (gx, gz) = r.grid.geom.grid_coords(at.0.rem_euclid(r.c), at.1);
            let rough = r.roughness_at(gx, gz);
            // The valley the level cuts at the parent's cells' scale.
            let mut low = VALLEY * rough.at(r.cell_size(self.level - 1) / r.v) * r.v;
            if land {
                low = low.min(0.5 * (top(&c) as f64).max(0.0));
            }
            let b = ((top(&c) as f64 - low) as f32).max(floor.min(top(&c)));
            if land { b.max(0.0) } else { b }
        };
        self.own.insert((i, j, land), b);
        b
    }

    /// The bed a river lies at in a cell: its own, or the highest of the next [`AHEAD`] cells'
    /// down its course should one lie higher (held up by the water it flows on to, or in a
    /// shallower valley), so that it never rises downstream within that stretch and every tile
    /// lays it the same.
    pub(super) fn bed(&mut self, i: i64, j: i64, land: bool) -> f32 {
        if let Some(&b) = self.beds.get(&(i, j, land)) {
            return b;
        }
        let mut bed = f32::NEG_INFINITY;
        let (mut ci, mut cj) = (i, j);
        for _ in 0..AHEAD {
            bed = bed.max(self.own_bed(ci, cj, land));
            let c = self.cell(ci, cj);
            if c.sea || c.lake.is_finite() {
                break;
            }
            match c.receiver {
                Some(r) => (ci, cj) = r,
                None => break,
            }
        }
        self.beds.insert((i, j, land), bed);
        bed
    }
}

//! Block-resolution rivers refined from the planet grid's drainage network.
//!
//! Every river cell of the grid becomes a node (its centre jittered so channels don't follow
//! grid lines) with a segment to its receiver. A query point is domain-warped by smooth noise
//! before measuring the distance to the segments, which makes channels meander while staying
//! continuous. Water levels interpolate between nodes, and node levels never increase
//! downstream, so rivers never flow uphill.

use hearth_math::hash::{hash2, unit_f64};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::planet::PlanetGrid;

/// A river node (one grid cell's reach).
#[derive(Debug, Clone, Copy)]
pub struct RiverNode {
    /// World position (x unwrapped around the cell centre, z).
    pub x: f64,
    pub z: f64,
    /// Water surface in blocks.
    pub level: f32,
    /// Channel width in blocks.
    pub width: f32,
    /// Channel depth in blocks.
    pub depth: f32,
    /// Receiver cell index (u32::MAX at the network's end).
    pub receiver: u32,
}

/// A segment between two nodes, in world coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Segment {
    /// Grid cell of the upstream node: the reach carries that cell's discharge.
    pub cell: u32,
    pub ax: f64,
    pub az: f64,
    pub bx: f64,
    pub bz: f64,
    pub level_a: f32,
    pub level_b: f32,
    pub width_a: f32,
    pub width_b: f32,
    pub depth_a: f32,
    pub depth_b: f32,
}

/// Result of a river query.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiverHit {
    /// Grid cell whose reach this is (a key of `PlanetGrid::rivers`).
    pub cell: u32,
    /// Distance from the (warped) point to the channel centreline, in blocks.
    pub distance: f32,
    pub level: f32,
    pub width: f32,
    pub depth: f32,
    /// Height of the land here before the river shaped it (blocks): what lies around the
    /// river's banks.
    pub plain: f32,
}

/// River width in blocks from discharge (square degrees × m/yr).
pub fn width_for(discharge: f32) -> f32 {
    (1.7 * discharge.max(0.0).sqrt()).clamp(3.0, 56.0)
}

/// River depth in blocks from width.
pub fn depth_for(width: f32) -> f32 {
    (1.4 + width * 0.12).clamp(1.5, 9.0)
}

/// Banks stand this far above a river's water level (blocks).
pub const BANK_HEIGHT: f32 = 1.2;

/// Width of the banks and floodplain on each side of a channel (blocks), which the terrain
/// shapes from the bank height down to the surrounding land.
pub fn bank_width(width: f32) -> f32 {
    3.0 + width * 0.9
}

/// The river network.
#[derive(Debug, Clone, Default)]
pub struct RiverNet {
    nodes: FxHashMap<u32, RiverNode>,
    /// Grid cell size in blocks.
    cell: f64,
    n: usize,
    circumference: f64,
}

impl RiverNet {
    pub fn build(grid: &PlanetGrid) -> Self {
        let n = grid.n();
        let v = grid.vertical_scale as f32;
        let mut nodes = FxHashMap::default();
        let jitter = |idx: u32| -> (f64, f64) {
            let h = hash2(grid.seed ^ 0x51_7e4, idx as u64);
            (
                (unit_f64(h) - 0.5) * 0.6,
                (unit_f64(h.rotate_left(23)) - 0.5) * 0.6,
            )
        };
        let mut cells: Vec<(u32, crate::planet::RiverCell)> =
            grid.rivers.iter().map(|(k, v)| (*k, *v)).collect();
        cells.sort_by_key(|c| c.0);
        for (idx, cell) in cells {
            let idx = idx as usize;
            let (i, j) = grid.geom.ij(idx);
            let (jx, jz) = jitter(idx as u32);
            let (x, z) = grid.geom.world_xz(i, j);
            let width = width_for(cell.discharge);
            let r = cell.receiver;
            let level = grid.water.data[idx] * v;
            nodes.insert(
                idx as u32,
                RiverNode {
                    x: x + jx * grid.geom.cell,
                    z: z + jz * grid.geom.cell,
                    level,
                    width,
                    depth: depth_for(width),
                    receiver: if r as usize == idx { u32::MAX } else { r },
                },
            );
        }
        // Terminal nodes: where a river flows into the sea or a lake, add an end node at the
        // receiver so the channel reaches it (at the receiver's water level).
        let mut ends = Vec::new();
        for node in nodes.values() {
            let r = node.receiver;
            if r == u32::MAX || nodes.contains_key(&r) {
                continue;
            }
            let (i, j) = grid.geom.ij(r as usize);
            let (x, z) = grid.geom.world_xz(i, j);
            let water = grid.water.data[r as usize];
            let level = if water.is_nan() {
                grid.elevation.data[r as usize] * v
            } else {
                water * v
            };
            ends.push((
                r,
                RiverNode {
                    x,
                    z,
                    level: level.min(node.level),
                    width: node.width,
                    depth: node.depth,
                    receiver: u32::MAX,
                },
            ));
        }
        for (r, node) in ends {
            nodes.entry(r).or_insert(node);
        }
        // Enforce monotone levels downstream (guards interpolation and rounding): propagate
        // minima along the receiver chains until nothing changes.
        let mut order: Vec<u32> = nodes.keys().copied().collect();
        order.sort_unstable();
        for _ in 0..64 {
            let mut changed = false;
            for &idx in &order {
                let node = nodes[&idx];
                if node.receiver != u32::MAX
                    && let Some(recv) = nodes.get_mut(&node.receiver)
                    && recv.level > node.level
                {
                    recv.level = node.level;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        Self {
            nodes,
            cell: grid.geom.cell,
            n,
            circumference: grid.geom.c,
        }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn nodes(&self) -> impl Iterator<Item = (&u32, &RiverNode)> {
        self.nodes.iter()
    }

    /// All segments whose nodes lie in grid cells overlapping the world rectangle expanded by
    /// `margin` blocks. X coordinates are unwrapped to be near `x0`.
    pub fn segments_near(
        &self,
        x0: f64,
        z0: f64,
        x1: f64,
        z1: f64,
        margin: f64,
    ) -> SmallVec<[Segment; 16]> {
        let mut out = SmallVec::new();
        if self.nodes.is_empty() {
            return out;
        }
        let cell = self.cell;
        let n = self.n as i64;
        let reach = margin + cell * 1.5;
        let gi0 = ((x0 - reach) / cell).floor() as i64;
        let gi1 = ((x1 + reach) / cell).floor() as i64;
        let half = self.circumference * 0.5;
        let gj0 = (((z0 - reach) + half) / cell).floor().max(0.0) as i64;
        let gj1 = ((((z1 + reach) + half) / cell).floor() as i64).min(n - 1);
        let cx = (x0 + x1) * 0.5;
        let unwrap = |x: f64| -> f64 {
            let c = self.circumference;
            let mut d = (x - cx).rem_euclid(c);
            if d > c * 0.5 {
                d -= c;
            }
            cx + d
        };
        for gj in gj0..=gj1 {
            for gi in gi0..=gi1 {
                let idx = (gj * n + gi.rem_euclid(n)) as u32;
                let Some(a) = self.nodes.get(&idx) else {
                    continue;
                };
                if a.receiver == u32::MAX {
                    continue;
                }
                let Some(b) = self.nodes.get(&a.receiver) else {
                    continue;
                };
                out.push(Segment {
                    cell: idx,
                    ax: unwrap(a.x),
                    az: a.z,
                    bx: unwrap(b.x),
                    bz: b.z,
                    level_a: a.level,
                    level_b: b.level,
                    width_a: a.width,
                    width_b: b.width.max(a.width),
                    depth_a: a.depth,
                    depth_b: b.depth.max(a.depth),
                });
            }
        }
        out
    }

    /// Closest channel to a point among `segments` (point already warped and unwrapped near
    /// the segments' frame).
    pub fn closest(x: f64, z: f64, segments: &[Segment]) -> Option<RiverHit> {
        let mut best: Option<RiverHit> = None;
        for s in segments {
            let (dx, dz) = (s.bx - s.ax, s.bz - s.az);
            let len2 = dx * dx + dz * dz;
            let t = if len2 > 0.0 {
                (((x - s.ax) * dx + (z - s.az) * dz) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let px = s.ax + dx * t;
            let pz = s.az + dz * t;
            let d = ((x - px).powi(2) + (z - pz).powi(2)).sqrt() as f32;
            let t = t as f32;
            let width = s.width_a + (s.width_b - s.width_a) * t;
            // Compare by distance relative to the channel's half width, so a wide river wins
            // over a nearby creek.
            let score = d - width * 0.5;
            if best.is_none_or(|b| score < b.distance - b.width * 0.5) {
                best = Some(RiverHit {
                    cell: s.cell,
                    distance: d,
                    level: s.level_a + (s.level_b - s.level_a) * t,
                    width,
                    depth: s.depth_a + (s.depth_b - s.depth_a) * t,
                    plain: f32::INFINITY,
                });
            }
        }
        best
    }

    /// Checks that no segment flows uphill. Returns the worst rise found (≤ 0 when valid).
    pub fn max_uphill_step(&self) -> f32 {
        let mut worst = f32::NEG_INFINITY;
        for node in self.nodes.values() {
            if let Some(r) = self.nodes.get(&node.receiver) {
                worst = worst.max(r.level - node.level);
            }
        }
        worst
    }
}

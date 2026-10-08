//! The ground as a field (Amendment S §8.3–8.4): where a look meets it to the centimetre, digging
//! that removes the volume a stroke moves and piling that adds it back, and loose ground slumping
//! to its angle of repose. Volume is conserved: what is dug is what the ground loses, and what is
//! piled is what it gains.
//!
//! A voxel's **occupancy**, the share of it that is ground, follows from its fill: half full at
//! the surface, full a half voxel inside it, empty a half voxel out (`occupancy`). After an edit
//! the fill about it is set again from the occupancies (`refill`), so the surface stays where the
//! volume says.

use glam::DVec3;
use hearth_math::BlockPos;

use crate::block::{BlockRegistry, BlockStateId, StateFlags};
use crate::fill::{Fill, RANGE};
use crate::storage::CubeMap;

/// The least change of a voxel's share the fill can hold (m³ of a 1 m voxel: some 12 litres).
pub const STEP: f32 = crate::fill::RANGE / crate::fill::FULL as f32;

/// Below this share a voxel is empty (it is air again), above one less it is full.
const GONE: f32 = STEP * 0.5;

/// The share of a voxel that is ground, from its fill (to the fill's step).
pub fn occupancy(q: i8) -> f32 {
    let occ = (0.5 + Fill::depth(q)).clamp(0.0, 1.0);
    if occ > 1.0 - GONE {
        1.0
    } else if occ < GONE {
        0.0
    } else {
        occ
    }
}

/// The ground's depth (voxels, positive inside) at a point: the fill at the voxels' centres,
/// trilinear between them; unloaded ground is outside.
pub fn field(map: &CubeMap, reg: &BlockRegistry, p: DVec3) -> f32 {
    let q = p - DVec3::splat(0.5);
    let b = q.floor();
    let f = q - b;
    let (bx, by, bz) = (b.x as i32, b.y as i32, b.z as i32);
    let mut sum = 0.0;
    for k in 0..8 {
        let (dx, dy, dz) = (k & 1, (k >> 1) & 1, (k >> 2) & 1);
        let w = if dx == 1 { f.x } else { 1.0 - f.x }
            * if dy == 1 { f.y } else { 1.0 - f.y }
            * if dz == 1 { f.z } else { 1.0 - f.z };
        let d = map
            .fill(BlockPos::new(bx + dx, by + dy, bz + dz), reg)
            .map_or(-RANGE, Fill::depth);
        sum += w * d as f64;
    }
    sum as f32
}

/// The field's gradient at a point (central differences over a tenth of a voxel).
pub fn gradient(map: &CubeMap, reg: &BlockRegistry, p: DVec3) -> DVec3 {
    let h = 0.1;
    let d = |o: DVec3| field(map, reg, p + o) as f64 - field(map, reg, p - o) as f64;
    DVec3::new(d(DVec3::X * h), d(DVec3::Y * h), d(DVec3::Z * h)) / (2.0 * h)
}

/// Where a look meets the ground.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundHit {
    /// The point on the surface.
    pub at: DVec3,
    /// The surface's outward normal there.
    pub normal: DVec3,
    /// The voxel of ground just under the point.
    pub voxel: BlockPos,
    /// Its state (its material is the block's).
    pub state: BlockStateId,
    pub distance: f64,
}

/// The first point along a look (from `from` along unit `dir`, within `reach`) where the ground's
/// field turns from outside to inside, found to a millimetre or so.
pub fn raycast(
    map: &CubeMap,
    reg: &BlockRegistry,
    from: DVec3,
    dir: DVec3,
    reach: f64,
) -> Option<GroundHit> {
    let step = 0.1;
    let mut t0 = 0.0;
    let mut f0 = field(map, reg, from);
    if f0 > 0.0 {
        return None;
    }
    let mut t = step;
    while t <= reach + step {
        let t1 = t.min(reach);
        let f1 = field(map, reg, from + dir * t1);
        if f1 > 0.0 {
            // Bisect to the crossing.
            let (mut a, mut b) = (t0, t1);
            for _ in 0..12 {
                let m = 0.5 * (a + b);
                if field(map, reg, from + dir * m) > 0.0 {
                    b = m;
                } else {
                    a = m;
                }
            }
            let at = from + dir * b;
            let g = gradient(map, reg, at);
            let normal = if g.length_squared() > 1e-12 {
                -g.normalize()
            } else {
                -dir
            };
            let voxel = BlockPos::containing(at - normal * 0.05);
            let state = map.block(voxel).unwrap_or(BlockStateId::AIR);
            return Some(GroundHit {
                at,
                normal,
                voxel,
                state,
                distance: b,
            });
        }
        if t1 >= reach {
            break;
        }
        t0 = t1;
        f0 = f1;
        t += step;
    }
    let _ = f0;
    None
}

/// What a brush took from the ground: volume (m³) by the state it was.
pub type Taken = Vec<(BlockStateId, f32)>;

fn occupancy_at(map: &CubeMap, reg: &BlockRegistry, p: BlockPos) -> f32 {
    match map.block(p) {
        Some(s) if reg.has(s, StateFlags::NATURAL) => {
            occupancy(map.fill(p, reg).unwrap_or(crate::fill::FULL))
        }
        _ => 0.0,
    }
}

/// Sets a voxel's occupancy (its state when it comes to hold ground or empties).
fn set_occupancy(map: &mut CubeMap, reg: &BlockRegistry, p: BlockPos, occ: f32, s: BlockStateId) {
    if occ < GONE {
        map.set_block(p, BlockStateId::AIR, reg);
        map.set_fill(p, Fill::quantize(-0.5), reg);
    } else {
        if map.block(p) != Some(s) {
            map.set_block(p, s, reg);
        }
        map.set_fill(p, Fill::quantize(occ - 0.5), reg);
    }
}

/// Sets the fill about the voxels in `lo..=hi` (and a voxel beyond) from their occupancies: a
/// voxel part full holds its surface where its share puts it, a full one is as deep as the
/// emptiest of its six neighbours leaves it, an empty one as far out as the fullest.
pub fn refill(map: &mut CubeMap, reg: &BlockRegistry, lo: BlockPos, hi: BlockPos) {
    let mut set = Vec::new();
    for y in lo.y - 1..=hi.y + 1 {
        for z in lo.z - 1..=hi.z + 1 {
            for x in lo.x - 1..=hi.x + 1 {
                let p = BlockPos::new(x, y, z);
                let Some(s) = map.block(p) else {
                    continue;
                };
                let natural = reg.has(s, StateFlags::NATURAL);
                let occ = occupancy_at(map, reg, p);
                let n = [
                    p.up(),
                    p.down(),
                    BlockPos::new(x + 1, y, z),
                    BlockPos::new(x - 1, y, z),
                    BlockPos::new(x, y, z + 1),
                    BlockPos::new(x, y, z - 1),
                ]
                .map(|q| occupancy_at(map, reg, q));
                let depth = if natural && occ >= 1.0 {
                    0.5 + n.iter().cloned().fold(1.0f32, f32::min)
                } else if natural {
                    occ - 0.5
                } else {
                    -0.5 - (1.0 - n.iter().cloned().fold(0.0f32, f32::max))
                };
                set.push((p, Fill::read(Fill::quantize(depth), natural)));
            }
        }
    }
    for (p, q) in set {
        map.set_fill(p, q, reg);
    }
}

/// Digs `volume` m³ of natural ground about `at` (a surface point), from a bowl of `radius` m
/// sunk along `-normal`: the voxels nearest the bowl's middle give most. Returns what was taken,
/// to the fill's step (`STEP`, some 12 litres): a caller digging less at a stroke carries the
/// rest to the next. `diggable` says which states the tool can move.
pub fn dig(
    map: &mut CubeMap,
    reg: &BlockRegistry,
    at: DVec3,
    normal: DVec3,
    radius: f64,
    volume: f32,
    diggable: &dyn Fn(BlockStateId) -> bool,
) -> Taken {
    let centre = at - normal * (radius * 0.35);
    let r = radius.max(0.6);
    let (lo, hi) = bounds(centre, r);
    // Weight each voxel by nearness to the bowl's middle and what ground it holds.
    let mut cells = Vec::new();
    let mut total = 0.0f32;
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let p = BlockPos::new(x, y, z);
                let Some(s) = map.block(p) else { continue };
                if !reg.has(s, StateFlags::NATURAL) || !diggable(s) {
                    continue;
                }
                let d =
                    (DVec3::new(x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5) - centre).length();
                if d > r {
                    continue;
                }
                let occ = occupancy_at(map, reg, p);
                let w = (1.0 - d / r) as f32 * occ;
                if w > 0.0 {
                    cells.push((p, s, occ, w));
                    total += w;
                }
            }
        }
    }
    let mut taken: Taken = Vec::new();
    if total <= 0.0 {
        return taken;
    }
    // Whole steps of the fill: each voxel its share by weight, rounded down, then the steps
    // left to the voxels nearest a further step, until the volume is taken to half a step.
    let mut steps = (volume / STEP).round() as i64;
    let have: Vec<i64> = cells.iter().map(|c| (c.2 / STEP).round() as i64).collect();
    let mut give: Vec<i64> = vec![0; cells.len()];
    let mut part: Vec<(f32, usize)> = Vec::new();
    for (k, c) in cells.iter().enumerate() {
        let want = c.3 / total * steps as f32;
        give[k] = (want.floor() as i64).min(have[k]);
        part.push((want - give[k] as f32, k));
    }
    steps -= give.iter().sum::<i64>();
    part.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    while steps > 0 {
        let mut any = false;
        for &(_, k) in &part {
            if steps == 0 {
                break;
            }
            if give[k] < have[k] {
                give[k] += 1;
                steps -= 1;
                any = true;
            }
        }
        if !any {
            break;
        }
    }
    for (k, c) in cells.iter().enumerate() {
        if give[k] == 0 {
            continue;
        }
        let before = occupancy_at(map, reg, c.0);
        set_occupancy(
            map,
            reg,
            c.0,
            (before - give[k] as f32 * STEP).max(0.0),
            c.1,
        );
        add(&mut taken, c.1, before - occupancy_at(map, reg, c.0));
    }
    refill(map, reg, lo, hi);
    taken
}

/// Piles `volume` m³ of ground of state `s` about `at`: into the lowest open voxels within
/// `radius` m, filling each from below. Returns the volume that found room.
pub fn pile(
    map: &mut CubeMap,
    reg: &BlockRegistry,
    at: DVec3,
    radius: f64,
    s: BlockStateId,
    volume: f32,
) -> f32 {
    let r = radius.max(0.5);
    let (lo, mut hi) = bounds(at, r + 1.0);
    // A heap may stand as tall as what is piled.
    hi.y = at.y.floor() as i32 + 2 + volume.ceil() as i32;
    let mut left = volume;
    for _ in 0..64 {
        if left <= 1e-5 {
            break;
        }
        // The lowest voxel within reach with room, nearest the middle first.
        let mut best: Option<(i32, f64, BlockPos)> = None;
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let d = DVec3::new(x as f64 + 0.5 - at.x, 0.0, z as f64 + 0.5 - at.z).length();
                if d > r {
                    continue;
                }
                for y in lo.y..=hi.y {
                    let p = BlockPos::new(x, y, z);
                    let Some(here) = map.block(p) else { continue };
                    let occ = occupancy_at(map, reg, p);
                    let open = here.is_air() || (reg.has(here, StateFlags::NATURAL) && occ < 1.0);
                    let on_ground = occupancy_at(map, reg, p.down()) >= 1.0;
                    if open && on_ground {
                        if best.is_none_or(|(by, bd, _)| (y, d) < (by, bd)) {
                            best = Some((y, d, p));
                        }
                        break;
                    }
                }
            }
        }
        let Some((_, _, p)) = best else { break };
        let here = map.block(p).unwrap_or(BlockStateId::AIR);
        let before = occupancy_at(map, reg, p);
        let into = if here.is_air() { s } else { here };
        let after = (before + left).min(1.0);
        set_occupancy(map, reg, p, after, into);
        let got = occupancy_at(map, reg, p) - before;
        if got <= 0.0 {
            break;
        }
        left -= got;
    }
    refill(map, reg, lo, hi);
    volume - left
}

/// Lets loose ground about `at` (within `radius` blocks) slump until no slope of it stands
/// steeper than its angle of repose: ground passes from a column's top to a lower neighbour's
/// until the step between them is within the repose, conserving its volume. `repose` gives a
/// state's angle (degrees), none for ground that does not flow (rock). At most `budget` moves.
pub fn settle(
    map: &mut CubeMap,
    reg: &BlockRegistry,
    at: BlockPos,
    radius: i32,
    repose: &dyn Fn(BlockStateId) -> Option<f32>,
    budget: usize,
) -> usize {
    let (y0, y1) = (at.y - 12, at.y + 12);
    // A column's surface: its top voxel of ground, and its height (that voxel's floor plus its
    // share).
    let top = |map: &CubeMap, x: i32, z: i32| -> Option<(BlockPos, f32)> {
        for y in (y0..=y1).rev() {
            let p = BlockPos::new(x, y, z);
            let occ = occupancy_at(map, reg, p);
            if occ > GONE {
                return Some((p, y as f32 + occ));
            }
            if map
                .block(p)
                .is_some_and(|s| !s.is_air() && !reg.has(s, StateFlags::NATURAL))
                && reg.has(map.block(p).unwrap_or_default(), StateFlags::HAS_COLLISION)
            {
                // Something built or a stone stands there: not ground to slump onto.
                return None;
            }
        }
        None
    };
    let mut moves = 0;
    let mut changed = true;
    while changed && moves < budget {
        changed = false;
        for z in at.z - radius..=at.z + radius {
            for x in at.x - radius..=at.x + radius {
                let Some((p, h)) = top(map, x, z) else {
                    continue;
                };
                let s = map.block(p).unwrap_or_default();
                let Some(angle) = repose(s) else { continue };
                let rise = (angle as f64).to_radians().tan() as f32;
                for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let Some((q, hq)) = top(map, x + dx, z + dz) else {
                        continue;
                    };
                    let step = h - hq;
                    if step <= rise + 0.02 {
                        continue;
                    }
                    // Half the excess passes, no more than the top voxel holds.
                    let occ = occupancy_at(map, reg, p);
                    let amount = ((step - rise) * 0.5).min(occ).min(0.5);
                    if amount <= 1e-3 {
                        continue;
                    }
                    set_occupancy(map, reg, p, occ - amount, s);
                    let given = occ - occupancy_at(map, reg, p);
                    let mut left = given;
                    // Onto the neighbour's top, filling it and then the voxel above.
                    let mut r = q;
                    for _ in 0..3 {
                        if left <= 1e-5 {
                            break;
                        }
                        let here = map.block(r).unwrap_or_default();
                        let before = occupancy_at(map, reg, r);
                        let into = if here.is_air() { s } else { here };
                        if !here.is_air() && !reg.has(here, StateFlags::NATURAL) {
                            break;
                        }
                        set_occupancy(map, reg, r, (before + left).min(1.0), into);
                        left -= occupancy_at(map, reg, r) - before;
                        r = r.up();
                    }
                    if left > 1e-5 {
                        // No room: what did not pass goes back.
                        let back = occupancy_at(map, reg, p);
                        set_occupancy(map, reg, p, (back + left).min(1.0), s);
                    }
                    moves += 1;
                    changed = true;
                    break;
                }
            }
        }
    }
    let r = radius + 1;
    refill(
        map,
        reg,
        BlockPos::new(at.x - r, y0, at.z - r),
        BlockPos::new(at.x + r, y1, at.z + r),
    );
    moves
}

/// The ground's volume (m³) in the box `lo..=hi`, by state.
pub fn volume(map: &CubeMap, reg: &BlockRegistry, lo: BlockPos, hi: BlockPos) -> Taken {
    let mut v: Taken = Vec::new();
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let p = BlockPos::new(x, y, z);
                let occ = occupancy_at(map, reg, p);
                if occ > 0.0 {
                    add(&mut v, map.block(p).unwrap_or_default(), occ);
                }
            }
        }
    }
    v
}

fn add(t: &mut Taken, s: BlockStateId, v: f32) {
    if v <= 0.0 {
        return;
    }
    match t.iter_mut().find(|(k, _)| *k == s) {
        Some(e) => e.1 += v,
        None => t.push((s, v)),
    }
}

fn bounds(c: DVec3, r: f64) -> (BlockPos, BlockPos) {
    (
        BlockPos::containing(c - DVec3::splat(r)),
        BlockPos::containing(c + DVec3::splat(r)),
    )
}

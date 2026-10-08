//! Trees as blocks: a skeleton's wood and foliage as blocks relative to the tree's foot, with
//! a per-column summary of the crown for the distant terrain, grown once per species, stage
//! and variant and cached; each tree in the world is one turned a quarter turn and mirrored as
//! its position's hash says.

use std::sync::{Arc, RwLock};

use glam::Vec3;
use hearth_math::hash::{hash_3d, unit_f32};
use rustc_hash::FxHashMap;

use crate::growth::{Species, Stage};
use crate::skeleton::{self, Seg, Skeleton};

/// Faces a limb joins (bits of `Part::Branch::joins`).
pub const DOWN: u8 = 1;
pub const UP: u8 = 2;
pub const NORTH: u8 = 4;
pub const SOUTH: u8 = 8;
pub const WEST: u8 = 16;
pub const EAST: u8 = 32;

/// Variants grown per species and stage.
pub const VARIANTS: u8 = 8;

/// Wood this thick (m) and thicker fills whole blocks.
pub const LOG_DIAMETER_M: f32 = 0.9;
/// Thinner wood is not drawn (it is lost in the foliage).
const MIN_BRANCH_DIAMETER_M: f32 = 0.08;

/// What a block of a tree is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Part {
    /// Wood filling the block, along `axis` (0 x, 1 y, 2 z).
    Log {
        axis: u8,
    },
    /// A limb `thickness` px through (2, 4, 8 or 12), joined toward the faces in `joins`.
    Branch {
        thickness: u8,
        joins: u8,
    },
    Leaves,
}

/// Block thickness (px) of wood of a diameter (m).
pub fn thickness_px(diameter_m: f32) -> u8 {
    if diameter_m >= 0.6 {
        12
    } else if diameter_m >= 0.35 {
        8
    } else if diameter_m >= 0.15 {
        4
    } else {
        2
    }
}

/// A tree's blocks relative to its foot (the block over the ground the trunk stands in).
#[derive(Debug, Clone, PartialEq)]
pub struct TreeTemplate {
    pub blocks: Vec<([i16; 3], Part)>,
    /// Inclusive bounds of `blocks`.
    pub min: [i32; 3],
    pub max: [i32; 3],
    /// Per column with foliage: (x, z, lowest leaf, highest leaf).
    pub crowns: Vec<[i16; 4]>,
    /// Per column with upright trunk: (x, z, lowest, highest).
    pub trunks: Vec<[i16; 4]>,
    /// The trunk's axis is at a block corner (a trunk two blocks across) rather than a centre.
    pub corner: bool,
    pub height_m: f32,
    pub diameter_m: f32,
}

impl TreeTemplate {
    /// Horizontal reach from the foot (blocks).
    pub fn reach(&self) -> i32 {
        [
            self.min[0].abs(),
            self.max[0].abs(),
            self.min[2].abs(),
            self.max[2].abs(),
        ]
        .into_iter()
        .max()
        .unwrap_or(0)
            + 1
    }
}

/// How a tree's template is turned and mirrored where it stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Turn {
    /// Quarter turns about the vertical.
    pub quarter: u8,
    /// Mirrored east–west first.
    pub mirror: bool,
}

impl Turn {
    pub fn from_hash(h: u64) -> Self {
        Self {
            quarter: (h & 3) as u8,
            mirror: (h >> 2) & 1 == 1,
        }
    }

    /// Where a template's column lands (relative to the foot). `corner` trunks turn about the
    /// foot block's corner, others about its centre.
    #[inline]
    pub fn apply(self, x: i32, z: i32, corner: bool) -> (i32, i32) {
        let c = corner as i32;
        let (mut x, mut z) = (if self.mirror { -x - c } else { x }, z);
        for _ in 0..self.quarter {
            (x, z) = (-z - c, x);
        }
        (x, z)
    }

    /// Where a point of a template's skeleton (metres; x and z) lands relative to the foot
    /// block's minimum corner: on the block's middle (or, for a `corner` trunk, its corner),
    /// then turned and mirrored as its blocks are (`apply`), so the point stays in the block
    /// its template put it in.
    pub fn apply_f(self, x: f32, z: f32, corner: bool) -> (f32, f32) {
        let (c, off) = if corner { (1.0, 0.0) } else { (0.0, 0.5) };
        let (mut x, mut z) = (x + off, z + off);
        if self.mirror {
            x = (1.0 - c) - x;
        }
        for _ in 0..self.quarter {
            (x, z) = ((1.0 - c) - z, x);
        }
        (x, z)
    }

    /// A part turned with its tree.
    #[inline]
    pub fn part(self, p: Part) -> Part {
        match p {
            Part::Leaves => p,
            Part::Log { axis } => Part::Log {
                axis: if axis != 1 && self.quarter % 2 == 1 {
                    2 - axis
                } else {
                    axis
                },
            },
            Part::Branch { thickness, joins } => {
                let mut j = joins;
                if self.mirror {
                    j = swap_bits(j, WEST, EAST);
                }
                for _ in 0..self.quarter {
                    // A quarter turn (x, z) → (−z, x): east → south → west → north → east.
                    let mut n = j & (UP | DOWN);
                    if j & EAST != 0 {
                        n |= SOUTH;
                    }
                    if j & SOUTH != 0 {
                        n |= WEST;
                    }
                    if j & WEST != 0 {
                        n |= NORTH;
                    }
                    if j & NORTH != 0 {
                        n |= EAST;
                    }
                    j = n;
                }
                Part::Branch {
                    thickness,
                    joins: j,
                }
            }
        }
    }
}

fn swap_bits(j: u8, a: u8, b: u8) -> u8 {
    let (ha, hb) = (j & a != 0, j & b != 0);
    let mut j = j & !(a | b);
    if ha {
        j |= b;
    }
    if hb {
        j |= a;
    }
    j
}

/// The bit for a step of `sign` along `axis`.
fn face(axis: usize, sign: i32) -> u8 {
    match (axis, sign > 0) {
        (0, true) => EAST,
        (0, false) => WEST,
        (1, true) => UP,
        (1, false) => DOWN,
        (2, true) => SOUTH,
        _ => NORTH,
    }
}

/// Visits the blocks a segment passes through in order (Amanatides and Woo), with the step
/// taken into each (axis, sign) and the share of the segment there.
fn traverse(a: Vec3, b: Vec3, mut f: impl FnMut([i32; 3], Option<(usize, i32)>, f32)) {
    let cell = |p: Vec3| [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
    let d = b - a;
    let mut c = cell(a);
    let end = cell(b);
    f(c, None, 0.0);
    if d.length_squared() < 1e-10 {
        return;
    }
    let mut step = [0i32; 3];
    let mut t_max = [f32::INFINITY; 3];
    let mut t_delta = [f32::INFINITY; 3];
    for k in 0..3 {
        if d[k] > 0.0 {
            step[k] = 1;
            t_max[k] = ((c[k] + 1) as f32 - a[k]) / d[k];
            t_delta[k] = 1.0 / d[k];
        } else if d[k] < 0.0 {
            step[k] = -1;
            t_max[k] = (c[k] as f32 - a[k]) / d[k];
            t_delta[k] = -1.0 / d[k];
        }
    }
    for _ in 0..4096 {
        if c == end {
            break;
        }
        let k = if t_max[0] < t_max[1] {
            if t_max[0] < t_max[2] { 0 } else { 2 }
        } else if t_max[1] < t_max[2] {
            1
        } else {
            2
        };
        if t_max[k] > 1.0 {
            break;
        }
        c[k] += step[k];
        let t = t_max[k];
        t_max[k] += t_delta[k];
        f(c, Some((k, step[k])), t);
    }
}

/// Distance from a point to a segment, and the share along it of the closest point.
fn to_segment(p: Vec3, a: Vec3, b: Vec3) -> (f32, f32) {
    let ab = b - a;
    let l2 = ab.length_squared();
    let s = if l2 < 1e-10 {
        0.0
    } else {
        ((p - a).dot(ab) / l2).clamp(0.0, 1.0)
    };
    ((a + ab * s - p).length(), s)
}

/// Where the trunk's axis stands: at the foot block's centre, or (for trunks about two blocks
/// across) at its corner, whichever fills the closest number of blocks to its section.
fn trunk_on_corner(diameter: f32) -> bool {
    let r = diameter / 2.0;
    if diameter < LOG_DIAMETER_M {
        return false;
    }
    let count = |cx: f32, cz: f32| {
        let mut n = 0;
        for i in -4..4 {
            for j in -4..4 {
                let (x, z) = (i as f32 + 0.5 - cx, j as f32 + 0.5 - cz);
                if (x * x + z * z).sqrt() <= r {
                    n += 1;
                }
            }
        }
        n as f32
    };
    let area = std::f32::consts::PI * r * r;
    (count(0.0, 0.0) - area).abs() < (count(0.5, 0.5) - area).abs()
}

/// Turns a skeleton into blocks.
pub fn voxelize(sk: &Skeleton, foliage_density: f32, seed: u64) -> TreeTemplate {
    let corner = trunk_on_corner(sk.diameter);
    let off = if corner {
        Vec3::ZERO
    } else {
        Vec3::new(0.5, 0.0, 0.5)
    };
    let mut cells: FxHashMap<[i32; 3], Part> = FxHashMap::default();
    let thick = |s: &Seg| 2.0 * s.ra.max(s.rb) >= LOG_DIAMETER_M;
    // Wood that fills blocks: every block whose middle is inside the tapered cylinder.
    for s in sk.wood.iter().filter(|s| thick(s)) {
        let (a, b) = (s.a + off, s.b + off);
        let d = b - a;
        let axis = if d.y.abs() >= d.x.abs() && d.y.abs() >= d.z.abs() {
            1
        } else if d.x.abs() >= d.z.abs() {
            0
        } else {
            2
        };
        let r = s.ra.max(s.rb);
        let lo = a.min(b) - Vec3::splat(r + 1.0);
        let hi = a.max(b) + Vec3::splat(r + 1.0);
        for y in lo.y.floor() as i32..=hi.y.floor() as i32 {
            for z in lo.z.floor() as i32..=hi.z.floor() as i32 {
                for x in lo.x.floor() as i32..=hi.x.floor() as i32 {
                    let p = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                    let (dist, t) = to_segment(p, a, b);
                    if dist <= s.ra + (s.rb - s.ra) * t {
                        cells.insert([x, y, z], Part::Log { axis });
                    }
                }
            }
        }
        traverse(a, b, |c, _, _| {
            cells.insert(c, Part::Log { axis });
        });
    }
    // Limbs: the blocks each passes through, joined to the next along it and to the log it
    // grows from.
    for s in sk.wood.iter().filter(|s| !thick(s)) {
        if s.order != 0 && 2.0 * s.ra.max(s.rb) < MIN_BRANCH_DIAMETER_M {
            continue;
        }
        let mut prev: Option<[i32; 3]> = None;
        traverse(s.a + off, s.b + off, |c, step, t| {
            let px = thickness_px(2.0 * (s.ra + (s.rb - s.ra) * t));
            let is_log = matches!(cells.get(&c), Some(Part::Log { .. }));
            if !is_log {
                let e = cells.entry(c).or_insert(Part::Branch {
                    thickness: px,
                    joins: 0,
                });
                if let Part::Branch { thickness, .. } = e {
                    *thickness = (*thickness).max(px);
                } else {
                    *e = Part::Branch {
                        thickness: px,
                        joins: 0,
                    };
                }
            }
            if let (Some(p), Some((axis, sign))) = (prev, step) {
                // Join the two blocks across the face between them.
                if let Some(Part::Branch { joins, .. }) = cells.get_mut(&p) {
                    *joins |= face(axis, sign);
                }
                if let Some(Part::Branch { joins, .. }) = cells.get_mut(&c) {
                    *joins |= face(axis, -sign);
                }
            }
            prev = Some(c);
        });
    }
    // Foliage: blocks in the clusters, thinning toward their edges, where there is no wood.
    for (k, blob) in sk.foliage.iter().enumerate() {
        let (a, b) = (blob.a + off, blob.b + off);
        let lo = a.min(b) - Vec3::splat(blob.r + 1.0);
        let hi = a.max(b) + Vec3::splat(blob.r + 1.0);
        // Every cluster has foliage at its heart, beside the wood if the wood runs through it.
        let mid = (a + b) * 0.5;
        let heart = [
            mid.x.floor() as i32,
            mid.y.floor() as i32,
            mid.z.floor() as i32,
        ];
        let free = std::iter::once([0, 0, 0])
            .chain([
                [0, 1, 0],
                [1, 0, 0],
                [-1, 0, 0],
                [0, 0, 1],
                [0, 0, -1],
                [0, -1, 0],
            ])
            .map(|d| [heart[0] + d[0], heart[1] + d[1], heart[2] + d[2]])
            .find(|c| !cells.contains_key(c));
        if let Some(c) = free {
            cells.insert(c, Part::Leaves);
        }
        for y in lo.y.floor() as i32..=hi.y.floor() as i32 {
            for z in lo.z.floor() as i32..=hi.z.floor() as i32 {
                for x in lo.x.floor() as i32..=hi.x.floor() as i32 {
                    let p = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                    let (dist, _) = to_segment(p, a, b);
                    if dist > blob.r {
                        continue;
                    }
                    let edge = dist / blob.r.max(0.01);
                    let keep = (0.25 + 0.85 * foliage_density) * (1.0 - 0.55 * edge * edge);
                    let roll = unit_f32(hash_3d(seed ^ (k as u64).wrapping_mul(0x9e37), x, y, z));
                    if edge < 0.35 || roll < keep {
                        cells.entry([x, y, z]).or_insert(Part::Leaves);
                    }
                }
            }
        }
    }
    // Nothing below the ground but the roots' logs.
    cells.retain(|c, p| c[1] >= 0 || matches!(p, Part::Log { .. }) && c[1] >= -1);
    let mut blocks: Vec<([i16; 3], Part)> = cells
        .into_iter()
        .map(|(c, p)| ([c[0] as i16, c[1] as i16, c[2] as i16], p))
        .collect();
    blocks.sort_by_key(|(c, _)| (c[1], c[2], c[0]));
    let mut min = [i32::MAX; 3];
    let mut max = [i32::MIN; 3];
    let mut crowns: FxHashMap<(i16, i16), (i16, i16)> = FxHashMap::default();
    let mut trunks: FxHashMap<(i16, i16), (i16, i16)> = FxHashMap::default();
    for (c, p) in &blocks {
        for k in 0..3 {
            min[k] = min[k].min(c[k] as i32);
            max[k] = max[k].max(c[k] as i32);
        }
        let column = match p {
            Part::Leaves => Some(&mut crowns),
            Part::Log { axis: 1 } => Some(&mut trunks),
            _ => None,
        };
        if let Some(m) = column {
            let e = m.entry((c[0], c[2])).or_insert((c[1], c[1]));
            e.0 = e.0.min(c[1]);
            e.1 = e.1.max(c[1]);
        }
    }
    if blocks.is_empty() {
        min = [0; 3];
        max = [0; 3];
    }
    let flatten = |m: FxHashMap<(i16, i16), (i16, i16)>| {
        let mut v: Vec<[i16; 4]> = m
            .into_iter()
            .map(|((x, z), (lo, hi))| [x, z, lo, hi])
            .collect();
        v.sort_unstable();
        v
    };
    TreeTemplate {
        blocks,
        min,
        max,
        crowns: flatten(crowns),
        trunks: flatten(trunks),
        corner,
        height_m: sk.height,
        diameter_m: sk.diameter,
    }
}

/// A variant's skeleton as it stands in its template: grown, turned about its foot to the
/// variant's own angle and set off its block's middle by up to 0.4 m, so trees face every way
/// and stand off the grid's points (P §11.1). Its blocks and its mesh are both made from it.
pub fn variant_skeleton(sp: &Species, stage: Stage, variant: u8) -> Skeleton {
    let h = hearth_math::hash::derive_seed(variant as u64 * 0x51 + stage.index() as u64, &sp.id)
        ^ 0x9a3c_11d7;
    let yaw = unit_f32(h) * std::f32::consts::TAU;
    let offset = Vec3::new(
        (unit_f32(h.rotate_left(21)) - 0.5) * 0.8,
        0.0,
        (unit_f32(h.rotate_left(42)) - 0.5) * 0.8,
    );
    skeleton::grow(sp, stage, variant as u32).placed(yaw, offset)
}

/// Grows a template.
pub fn template(sp: &Species, stage: Stage, variant: u8) -> TreeTemplate {
    let sk = variant_skeleton(sp, stage, variant);
    if stage == Stage::Seedling {
        // Under 0.6 m: a leafy tuft in one block.
        return TreeTemplate {
            blocks: vec![([0, 0, 0], Part::Leaves)],
            min: [0; 3],
            max: [0; 3],
            crowns: vec![[0, 0, 0, 0]],
            trunks: Vec::new(),
            corner: false,
            height_m: sk.height,
            diameter_m: sk.diameter,
        };
    }
    let seed = hearth_math::hash::derive_seed(variant as u64 * 31 + stage.index() as u64, &sp.id);
    voxelize(&sk, sp.form.foliage_density, seed)
}

/// Templates of a set of species, grown as asked for and kept.
#[derive(Debug, Default)]
pub struct Templates {
    pub species: Vec<Species>,
    cache: Vec<RwLock<FxHashMap<(Stage, u8), Arc<TreeTemplate>>>>,
}

impl Templates {
    pub fn new(species: Vec<Species>) -> Self {
        let cache = species.iter().map(|_| RwLock::default()).collect();
        Self { species, cache }
    }

    /// The species of every plant in the content with a growth form, in content order.
    pub fn from_content(c: &hearth_content::Content) -> Self {
        Self::new(c.plants.iter().filter_map(Species::from_plant).collect())
    }

    /// The index of a species, by its id with or without the namespace.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        let path = |s: &str| s.rsplit(':').next().unwrap_or(s).to_owned();
        let want = path(id);
        self.species.iter().position(|s| path(&s.id) == want)
    }

    /// The template of a species' stage and variant, grown on first use.
    pub fn get(&self, species: usize, stage: Stage, variant: u8) -> Arc<TreeTemplate> {
        let key = (stage, variant % VARIANTS);
        if let Some(t) = self.cache[species]
            .read()
            .expect("template cache")
            .get(&key)
        {
            return t.clone();
        }
        let t = Arc::new(template(&self.species[species], stage, key.1));
        self.cache[species]
            .write()
            .expect("template cache")
            .entry(key)
            .or_insert(t)
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_moves_joins_with_the_columns() {
        // A limb going east from the foot, turned a quarter, goes south.
        let t = Turn {
            quarter: 1,
            mirror: false,
        };
        assert_eq!(t.apply(1, 0, false), (0, 1));
        assert_eq!(
            t.part(Part::Branch {
                thickness: 4,
                joins: EAST | UP
            }),
            Part::Branch {
                thickness: 4,
                joins: SOUTH | UP
            }
        );
        assert_eq!(t.part(Part::Log { axis: 0 }), Part::Log { axis: 2 });
        // Four quarters and two mirrors are no turn at all, about a corner too.
        for corner in [false, true] {
            let mut p = (3, -2);
            for _ in 0..4 {
                p = t.apply(p.0, p.1, corner);
            }
            assert_eq!(p, (3, -2));
            let m = Turn {
                quarter: 0,
                mirror: true,
            };
            let q = m.apply(3, -2, corner);
            assert_eq!(m.apply(q.0, q.1, corner), (3, -2));
        }
        // About a corner, the four blocks around the foot's corner go round among themselves.
        let mut seen = std::collections::BTreeSet::new();
        let mut p = (0, 0);
        for _ in 0..4 {
            p = t.apply(p.0, p.1, true);
            seen.insert(p);
        }
        assert_eq!(
            seen,
            [(-1, -1), (-1, 0), (0, -1), (0, 0)].into_iter().collect()
        );
    }

    #[test]
    fn a_turned_point_stays_in_its_turned_block() {
        for corner in [false, true] {
            let off = if corner { 0.0 } else { 0.5 };
            for quarter in 0..4 {
                for mirror in [false, true] {
                    let t = Turn { quarter, mirror };
                    for (x, z) in [(0.13f32, 0.71f32), (-2.4, 1.05), (3.9, -0.6)] {
                        // The block the template puts the point in (after its offset).
                        let (bx, bz) = ((x + off).floor() as i32, (z + off).floor() as i32);
                        let (tx, tz) = t.apply(bx, bz, corner);
                        let (fx, fz) = t.apply_f(x, z, corner);
                        assert_eq!(
                            (fx.floor() as i32, fz.floor() as i32),
                            (tx, tz),
                            "{corner} {quarter} {mirror}: ({x}, {z}) to ({fx}, {fz})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn variants_face_every_way_off_the_blocks_middle() {
        let sp = crate::growth::tests::oak();
        let feet: Vec<Vec3> = (0..VARIANTS)
            .map(|v| variant_skeleton(&sp, Stage::Young, v).wood[0].a)
            .collect();
        // The foot stays within its block, never on its middle.
        for f in &feet {
            assert!(f.x.abs() <= 0.4 && f.z.abs() <= 0.4, "{f}");
            assert!(f.x.abs() > 1e-4 || f.z.abs() > 1e-4, "{f}");
        }
        let distinct: std::collections::BTreeSet<i32> =
            feet.iter().map(|f| (f.x * 1000.0) as i32).collect();
        assert_eq!(distinct.len(), VARIANTS as usize);
    }

    #[test]
    fn a_traversal_steps_through_neighbouring_blocks() {
        let mut cells = Vec::new();
        traverse(
            Vec3::new(0.5, 0.5, 0.5),
            Vec3::new(3.2, 2.7, 0.4),
            |c, _, _| cells.push(c),
        );
        assert_eq!(cells.first(), Some(&[0, 0, 0]));
        assert_eq!(cells.last(), Some(&[3, 2, 0]));
        for w in cells.windows(2) {
            let d: i32 = (0..3).map(|k| (w[1][k] - w[0][k]).abs()).sum();
            assert_eq!(d, 1, "{w:?}");
        }
    }
}

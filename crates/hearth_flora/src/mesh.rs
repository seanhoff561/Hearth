//! Trees as meshes (Amendment S §7.1–7.2): a skeleton's wood as generalized cylinders (each run
//! of joined segments one tube, tapering, its rings carried along it by parallel transport,
//! capped where it is cut or broken) and its foliage as cards of leaves about each cluster, by
//! the species' leaf kind (broad-leaved clusters, needle sprays, scale-leaved sprays, palm
//! fronds). Positions are metres in the skeleton's frame, so a tree's mesh and its blocks
//! (`template::voxelize`, from the same skeleton) agree.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use hearth_content::schema::flora::LeafKind;
use hearth_math::hash::{Rng, mix64};

use crate::skeleton::{Blob, Seg, Skeleton};

/// What a vertex is (`TreeVertex::normal[3]`).
pub const WOOD: i8 = 0;
pub const LEAF: i8 = 1;
/// The cut end of wood (a stump's top, a broken limb): end grain.
pub const END_GRAIN: i8 = 2;

/// How the leaves on a card are drawn (`TreeVertex::extra[0]`).
pub const LEAF_BROAD: u8 = 0;
pub const LEAF_NEEDLE: u8 = 1;
pub const LEAF_SCALE: u8 = 2;
pub const LEAF_FROND: u8 = 3;

/// One vertex of a tree's mesh (32 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct TreeVertex {
    /// Metres in the skeleton's frame.
    pub pos: [f32; 3],
    /// Wood: around the stem (whole turns of the bark's pattern) and along it (m). Leaves: the
    /// card's corner (0..1).
    pub uv: [f32; 2],
    /// Normal (snorm) and what the vertex is (`WOOD`, `LEAF`, `END_GRAIN`).
    pub normal: [i8; 4],
    /// How far it sways in the wind (0..255), its sway's phase, the leaf card's own number
    /// (which leaves fall first in autumn), and the wood's order (0 stem, 1 branch, 2 twig,
    /// 3 root).
    pub sway: [u8; 4],
    /// The leaf kind (`LEAF_*`), the wood's radius (cm, at most 255), unused.
    pub extra: [u8; 4],
}

/// How much of a tree a mesh draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Detail {
    /// Every piece of wood and some twenty cards about each cluster: close by.
    Full,
    /// The twigs left out, fewer and larger cards: some tens of metres off.
    Reduced,
    /// Stems and limbs as few-sided tubes, three crossed cards a cluster: far off.
    Low,
}

/// What of a tree is meshed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    pub detail: Detail,
    /// Its foliage (a dead or burned tree has none).
    pub leaves: bool,
    /// Wood thinner than this (m) is left out (burned twigs).
    pub min_diameter: f32,
    /// Wood above this height (m) is cut away and the cut capped (a stump).
    pub cut: Option<f32>,
    pub leaf: LeafKind,
    /// Palm fronds rather than clusters.
    pub fronds: bool,
    /// The species' foliage density (0..1).
    pub density: f32,
}

impl Options {
    pub fn new(detail: Detail, leaf: LeafKind, fronds: bool, density: f32) -> Self {
        Self {
            detail,
            leaves: true,
            min_diameter: 0.0,
            cut: None,
            leaf,
            fronds,
            density,
        }
    }
}

/// A tree's mesh: its wood's triangles first (`wood` indices), then its leaves'.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TreeMesh {
    pub vertices: Vec<TreeVertex>,
    pub indices: Vec<u32>,
    pub wood: u32,
    /// The bounds of its vertices (m).
    pub min: Vec3,
    pub max: Vec3,
}

impl TreeMesh {
    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn bytes(&self) -> usize {
        self.vertices.len() * std::mem::size_of::<TreeVertex>() + self.indices.len() * 4
    }
}

fn snorm(n: Vec3) -> [i8; 3] {
    let n = n.normalize_or(Vec3::Y);
    [
        (n.x * 127.0).round() as i8,
        (n.y * 127.0).round() as i8,
        (n.z * 127.0).round() as i8,
    ]
}

fn byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A vector at right angles to `d`.
fn perpendicular(d: Vec3) -> Vec3 {
    let other = if d.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    d.cross(other).normalize_or(Vec3::X)
}

/// Sides of a tube of radius `r` at a detail.
fn sides(r: f32, detail: Detail) -> usize {
    match detail {
        Detail::Full => {
            if r >= 0.3 {
                12
            } else if r >= 0.12 {
                8
            } else if r >= 0.05 {
                5
            } else {
                3
            }
        }
        Detail::Reduced => {
            if r >= 0.2 {
                6
            } else if r >= 0.08 {
                4
            } else {
                3
            }
        }
        Detail::Low => {
            if r >= 0.12 {
                4
            } else {
                3
            }
        }
    }
}

/// Wood thinner than this (m radius) is left out at a detail.
fn min_radius(detail: Detail) -> f32 {
    match detail {
        Detail::Full => 0.008,
        Detail::Reduced => 0.045,
        Detail::Low => 0.1,
    }
}

/// A run's rings are kept where it bends more than this (radians), or every so far (m), at a
/// detail; the rest are left out and the tube runs straight between those kept.
fn ring_spacing(detail: Detail) -> (f32, f32) {
    match detail {
        Detail::Full => (0.09, 2.5),
        Detail::Reduced => (0.22, 5.0),
        Detail::Low => (0.45, 10.0),
    }
}

/// Foliage clusters are gathered into cells of this size (m) at a detail, and covered with
/// cards about this large.
fn leaf_cells(detail: Detail) -> (f32, f32) {
    match detail {
        Detail::Full => (1.6, 1.0),
        Detail::Reduced => (3.6, 2.3),
        Detail::Low => (6.5, 0.0),
    }
}

/// The rings of a run worth keeping: its ends, and where it bends or has run far enough.
fn simplify(rings: &[(Vec3, f32)], detail: Detail) -> Vec<(Vec3, f32)> {
    if rings.len() <= 2 {
        return rings.to_vec();
    }
    let (bend, far) = ring_spacing(detail);
    let mut out = vec![rings[0]];
    let mut from = 0;
    for k in 1..rings.len() - 1 {
        let kept = out[out.len() - 1].0;
        let d0 = (rings[k].0 - kept).normalize_or(Vec3::Y);
        let d1 = (rings[k + 1].0 - rings[k].0).normalize_or(Vec3::Y);
        let run = rings[from..=k]
            .windows(2)
            .map(|w| w[0].0.distance(w[1].0))
            .sum::<f32>();
        if d0.angle_between(d1) > bend || run >= far {
            out.push(rings[k]);
            from = k;
        }
    }
    out.push(rings[rings.len() - 1]);
    out
}

struct Builder<'a> {
    out: TreeMesh,
    sk: &'a Skeleton,
    opts: Options,
    rng: Rng,
}

impl Builder<'_> {
    fn push(&mut self, v: TreeVertex) -> u32 {
        let p = Vec3::from(v.pos);
        self.out.min = self.out.min.min(p);
        self.out.max = self.out.max.max(p);
        self.out.vertices.push(v);
        (self.out.vertices.len() - 1) as u32
    }

    /// How far wood or leaves at a height sway (0..1): the stem hardly, the crown's top and the
    /// branches' ends most.
    fn sway(&self, y: f32, order: u8, along: f32) -> u8 {
        let h = (y / self.sk.height.max(0.5)).clamp(0.0, 1.0);
        let reach = match order {
            0 => 0.0,
            1 => 0.3 * along,
            2 => 0.35 + 0.3 * along,
            _ => 0.0,
        };
        byte(0.25 * h * h + reach)
    }

    /// One tube along a run of joined segments.
    fn tube(&mut self, run: &[Seg], phase: u8) {
        let min_r = min_radius(self.opts.detail).max(self.opts.min_diameter * 0.5);
        // The run as rings (point, radius), cut where it thins below what is drawn or rises
        // above the cut.
        let mut rings: Vec<(Vec3, f32)> = vec![(run[0].a, run[0].ra)];
        let mut capped = false;
        for s in run {
            if s.order != 0 && s.ra < min_r {
                break;
            }
            if let Some(cut) = self.opts.cut {
                if s.a.y >= cut {
                    capped = true;
                    break;
                }
                if s.b.y > cut {
                    let t = (cut - s.a.y) / (s.b.y - s.a.y).max(1e-5);
                    rings.push((s.a.lerp(s.b, t), s.ra + (s.rb - s.ra) * t));
                    capped = true;
                    break;
                }
            }
            if s.rb < min_r && s.order != 0 {
                // Its thin end: drawn to where it reaches the least radius drawn.
                let t = ((s.ra - min_r) / (s.ra - s.rb).max(1e-5)).clamp(0.0, 1.0);
                if t > 0.05 {
                    rings.push((s.a.lerp(s.b, t), min_r));
                }
                break;
            }
            rings.push((s.b, s.rb));
        }
        if rings.len() < 2 {
            return;
        }
        let rings = simplify(&rings, self.opts.detail);
        let order = run[0].order;
        let n = sides(rings[0].1, self.opts.detail);
        // The bark's pattern repeats round the stem about every 0.4 m of its girth.
        let turns = (std::f32::consts::TAU * rings[0].1 / 0.4).round().max(1.0);
        let total: f32 = rings.windows(2).map(|w| w[0].0.distance(w[1].0)).sum();
        let mut normal = Vec3::ZERO;
        let mut along = 0.0;
        let mut prev_dir = Vec3::Y;
        let mut base: Vec<u32> = Vec::with_capacity(rings.len());
        for k in 0..rings.len() {
            let (p, r) = rings[k];
            let into = if k > 0 {
                (p - rings[k - 1].0).normalize_or(prev_dir)
            } else {
                (rings[1].0 - p).normalize_or(Vec3::Y)
            };
            let out = if k + 1 < rings.len() {
                (rings[k + 1].0 - p).normalize_or(into)
            } else {
                into
            };
            let t = (into + out).normalize_or(out);
            // Parallel transport of the ring's frame.
            normal = if k == 0 {
                perpendicular(t)
            } else {
                (normal - t * normal.dot(t)).normalize_or(perpendicular(t))
            };
            let binormal = t.cross(normal);
            if k > 0 {
                along += p.distance(rings[k - 1].0);
            }
            // The surface leans in as the tube tapers.
            let (r0, r1, l) = if k + 1 < rings.len() {
                (r, rings[k + 1].1, p.distance(rings[k + 1].0))
            } else {
                (rings[k - 1].1, r, p.distance(rings[k - 1].0))
            };
            let slope = (r0 - r1) / l.max(1e-4);
            let sway = self.sway(p.y, order, along / total.max(1e-4));
            let first = self.out.vertices.len() as u32;
            for s in 0..=n {
                let a = s as f32 / n as f32 * std::f32::consts::TAU;
                let radial = normal * a.cos() + binormal * a.sin();
                let nrm = (radial + t * slope).normalize_or(radial);
                let [nx, ny, nz] = snorm(nrm);
                self.push(TreeVertex {
                    pos: (p + radial * r).to_array(),
                    uv: [s as f32 / n as f32 * turns, along],
                    normal: [nx, ny, nz, WOOD],
                    sway: [sway, phase, 0, order],
                    extra: [0, (r * 100.0).min(255.0) as u8, 0, 0],
                });
            }
            base.push(first);
            prev_dir = t;
        }
        for w in base.windows(2) {
            for s in 0..n as u32 {
                let (a, b) = (w[0] + s, w[0] + s + 1);
                let (c, d) = (w[1] + s, w[1] + s + 1);
                self.out.indices.extend_from_slice(&[a, b, c, b, d, c]);
            }
        }
        // A cut or broken end of some thickness shows its end grain.
        let (end, r_end) = rings[rings.len() - 1];
        let thick_end = r_end > 0.03 && (capped || self.opts.cut.is_some() || r_end > 0.06);
        if thick_end {
            let t = (end - rings[rings.len() - 2].0).normalize_or(Vec3::Y);
            let [nx, ny, nz] = snorm(t);
            let sway = self.sway(end.y, order, 1.0);
            let last = base[base.len() - 1];
            let centre = self.push(TreeVertex {
                pos: end.to_array(),
                uv: [0.0, 0.0],
                normal: [nx, ny, nz, END_GRAIN],
                sway: [sway, phase, 0, order],
                extra: [0, (r_end * 100.0).min(255.0) as u8, 0, 0],
            });
            let mut rim = Vec::with_capacity(n);
            for s in 0..n as u32 {
                let v = self.out.vertices[(last + s) as usize];
                let rel = (Vec3::from(v.pos) - end) / r_end.max(1e-4);
                rim.push(self.push(TreeVertex {
                    uv: [rel.x, rel.z],
                    normal: [nx, ny, nz, END_GRAIN],
                    ..v
                }));
            }
            for s in 0..n {
                let (a, b) = (rim[s], rim[(s + 1) % n]);
                self.out.indices.extend_from_slice(&[centre, b, a]);
            }
        }
    }

    fn wood(&mut self) {
        let wood = &self.sk.wood;
        let mut i = 0;
        let mut run_no = 0u64;
        while i < wood.len() {
            let mut j = i + 1;
            while j < wood.len() && wood[j].a == wood[j - 1].b && wood[j].order == wood[j - 1].order
            {
                j += 1;
            }
            let phase = (mix64(run_no ^ 0x51ed) & 255) as u8;
            let run: Vec<Seg> = wood[i..j].to_vec();
            self.tube(&run, phase);
            run_no += 1;
            i = j;
        }
    }

    /// One card of leaves: centre, its plane's two half-extents, the normal its light takes.
    #[allow(clippy::too_many_arguments)]
    fn card(&mut self, c: Vec3, u: Vec3, v: Vec3, light: Vec3, kind: u8, phase: u8) {
        let [nx, ny, nz] = snorm(light);
        let sway = (self.sway(c.y, 2, 1.0) as f32 * 1.1).min(255.0) as u8;
        let id = (self.rng.next_u64() & 255) as u8;
        let first = self.out.vertices.len() as u32;
        for (cu, cv) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            let p = c + u * (cu * 2.0 - 1.0) + v * (cv * 2.0 - 1.0);
            self.push(TreeVertex {
                pos: p.to_array(),
                uv: [cu, cv],
                normal: [nx, ny, nz, LEAF],
                sway: [sway, phase, id, 2],
                extra: [kind, 0, 0, 0],
            });
        }
        self.out.indices.extend_from_slice(&[
            first,
            first + 1,
            first + 2,
            first,
            first + 2,
            first + 3,
        ]);
    }

    fn unit(&mut self) -> Vec3 {
        loop {
            let v = Vec3::new(
                self.rng.range_f32(-1.0, 1.0),
                self.rng.range_f32(-1.0, 1.0),
                self.rng.range_f32(-1.0, 1.0),
            );
            let l = v.length_squared();
            if l > 1e-3 && l <= 1.0 {
                return v / l.sqrt();
            }
        }
    }

    fn leaves(&mut self) {
        let foliage = &self.sk.foliage;
        if foliage.is_empty() {
            return;
        }
        // The crown's middle: leaves take their light as if the crown were one soft body.
        let centre = foliage.iter().map(|b| (b.a + b.b) * 0.5).sum::<Vec3>() / foliage.len() as f32;
        let reach = foliage
            .iter()
            .map(|b| ((b.a + b.b) * 0.5).distance(centre) + b.r)
            .fold(0.5f32, f32::max);
        let kind = match (self.opts.fronds, self.opts.leaf) {
            (true, _) => LEAF_FROND,
            (_, LeafKind::Broad) => LEAF_BROAD,
            (_, LeafKind::Needle) => LEAF_NEEDLE,
            (_, LeafKind::Scale) => LEAF_SCALE,
        };
        let light_of = move |p: Vec3, own: Vec3| {
            let crown = (p - centre) / reach;
            (own * 0.45 + crown.normalize_or(Vec3::Y) * 0.55 + Vec3::Y * 0.15).normalize_or(Vec3::Y)
        };
        if kind == LEAF_FROND {
            let blobs = foliage.clone();
            for (k, blob) in blobs.iter().enumerate() {
                let phase = (mix64(k as u64 ^ 0x1eaf) & 255) as u8;
                self.frond(blob, kind, phase, &light_of);
            }
            return;
        }
        // The clusters gathered into cells: each cell's foliage one body, covered with cards
        // on its outer part.
        let (cell, _) = leaf_cells(self.opts.detail);
        let mut cells: rustc_hash::FxHashMap<[i32; 3], (Vec3, Vec3, f32, f32, Vec3)> =
            rustc_hash::FxHashMap::default();
        for b in foliage {
            let mid = (b.a + b.b) * 0.5;
            let key = (mid / cell).floor().as_ivec3().to_array();
            let e = cells
                .entry(key)
                .or_insert((Vec3::ZERO, Vec3::ZERO, 0.0, 0.0, Vec3::ZERO));
            let w = b.r * b.r * (b.r + b.a.distance(b.b));
            e.0 += mid * w;
            e.2 += w;
            e.3 = e.3.max(b.r);
            e.4 += (b.b - b.a).abs();
            e.1 = e.1.max(mid);
        }
        let mut gathered: Vec<([i32; 3], Vec3, f32, Vec3)> = Vec::with_capacity(cells.len());
        for b in foliage {
            let mid = (b.a + b.b) * 0.5;
            let key = (mid / cell).floor().as_ivec3().to_array();
            let (sum, _, w, _, axis) = cells[&key];
            let c = sum / w.max(1e-6);
            if let Some(g) = gathered.iter_mut().find(|g| g.0 == key) {
                g.2 = g.2.max(c.distance(mid) + b.r);
            } else {
                gathered.push((key, c, c.distance(mid) + b.r, axis));
            }
        }
        gathered.sort_by_key(|g| g.0);
        for (k, (_, c, r, axis)) in gathered.into_iter().enumerate() {
            let phase = (mix64(k as u64 ^ 0x1eaf) & 255) as u8;
            // Needles and scales grow along their shoots: the cell's sprays lie along them.
            let along = axis.normalize_or(Vec3::Y);
            self.cluster(c, r.min(cell), along, kind, phase, &light_of);
        }
    }

    /// Cards about a body of foliage (centre, radius): on its outer part, facing out of it;
    /// sprays (needles, scales) lie along `along`.
    #[allow(clippy::too_many_arguments)]
    fn cluster(
        &mut self,
        c: Vec3,
        r: f32,
        along: Vec3,
        kind: u8,
        phase: u8,
        light_of: &dyn Fn(Vec3, Vec3) -> Vec3,
    ) {
        let r = r.max(0.25);
        let (_, card) = leaf_cells(self.opts.detail);
        if self.opts.detail == Detail::Low {
            // Three cards crossed through its middle.
            let spin = self.rng.range_f32(0.0, std::f32::consts::TAU);
            for k in 0..3 {
                let a = spin + k as f32 * std::f32::consts::TAU / 3.0;
                let u = Vec3::new(a.cos(), 0.0, a.sin()) * r;
                let v = Vec3::Y * r * 0.85;
                let own = Vec3::new(-a.sin(), 0.3, a.cos());
                self.card(c, u, v, light_of(c, own), kind, phase);
            }
            return;
        }
        let size = card * if kind == LEAF_SCALE { 0.85 } else { 1.0 };
        let area = 4.0 * std::f32::consts::PI * r * r;
        let n = ((area / (size * size)) * 0.7 * (0.55 + 0.6 * self.opts.density))
            .round()
            .clamp(2.0, 28.0) as usize;
        let sprays = matches!(kind, LEAF_NEEDLE | LEAF_SCALE);
        for _ in 0..n {
            let out = self.unit();
            let depth = 0.45 + 0.55 * self.rng.range_f32(0.0, 1.0).cbrt();
            let p = c + out * r * depth;
            let half = size * 0.5 * self.rng.range_f32(0.8, 1.2);
            let (u, v) = if sprays {
                // A spray: long along its shoot, turned about it at random.
                let side = perpendicular(along);
                let a = self.rng.range_f32(0.0, std::f32::consts::TAU);
                let w = side * a.cos() + along.cross(side) * a.sin();
                let d = (along + self.unit() * 0.35).normalize_or(along);
                (w * half * 0.55, d * half * 1.3)
            } else {
                // The card faces out of the cluster, tipped and turned at random.
                let facing = (out + self.unit() * 0.55).normalize_or(out);
                let u0 = perpendicular(facing);
                let v0 = facing.cross(u0);
                let spin = self.rng.range_f32(0.0, std::f32::consts::TAU);
                let (sn, cs) = spin.sin_cos();
                ((u0 * cs + v0 * sn) * half, (v0 * cs - u0 * sn) * half)
            };
            self.card(p, u, v, light_of(p, out), kind, phase);
        }
    }

    /// A palm frond's piece: two strips crossed along it, its leaflets painted on them.
    fn frond(&mut self, blob: &Blob, kind: u8, phase: u8, light_of: &dyn Fn(Vec3, Vec3) -> Vec3) {
        let axis = blob.b - blob.a;
        let len = axis.length();
        if len < 0.05 {
            // The crown's heart.
            self.cluster(blob.a, blob.r, Vec3::Y, LEAF_BROAD, phase, light_of);
            return;
        }
        let d = axis / len;
        let mid = (blob.a + blob.b) * 0.5;
        let flat = d.cross(Vec3::Y).normalize_or(perpendicular(d));
        let up = flat.cross(d).normalize_or(Vec3::Y);
        let width = blob.r * 1.6;
        for side in [flat, (flat * 0.5 - up * 0.85).normalize_or(flat)] {
            let out = side.cross(d).normalize_or(Vec3::Y);
            self.card(
                mid,
                side * width,
                d * (len * 0.5 + 0.05),
                light_of(mid, out),
                kind,
                phase,
            );
        }
    }
}

/// The mesh of a skeleton.
pub fn mesh(sk: &Skeleton, opts: Options, seed: u64) -> TreeMesh {
    let mut b = Builder {
        out: TreeMesh {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
            ..TreeMesh::default()
        },
        sk,
        opts,
        rng: Rng::new(mix64(seed ^ 0x7ee5_e5)),
    };
    b.wood();
    b.out.wood = b.out.indices.len() as u32;
    if opts.leaves && opts.cut.is_none() {
        b.leaves();
    }
    let mut out = b.out;
    if out.vertices.is_empty() {
        out.min = Vec3::ZERO;
        out.max = Vec3::ZERO;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::growth::Stage;
    use crate::skeleton::grow;

    fn oak_mesh(stage: Stage, detail: Detail) -> (Skeleton, TreeMesh) {
        let sp = crate::growth::tests::oak();
        let sk = grow(&sp, stage, 2);
        let m = mesh(
            &sk,
            Options::new(detail, LeafKind::Broad, false, sp.form.foliage_density),
            7,
        );
        (sk, m)
    }

    #[test]
    fn a_trees_mesh_follows_its_skeleton() {
        let (sk, m) = oak_mesh(Stage::Mature, Detail::Full);
        assert!(
            m.wood > 0 && (m.indices.len() as u32) > m.wood,
            "wood and leaves"
        );
        assert_eq!(m.indices.len() % 3, 0);
        assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
        // As tall as the tree, as wide as its crown, standing on its foot.
        assert!(
            (m.max.y - sk.height).abs() < sk.height * 0.15,
            "{} against {}",
            m.max.y,
            sk.height
        );
        assert!(m.min.y > -1.5, "{}", m.min.y);
        // Every wood vertex lies on the surface of a piece of wood: within its radius of the
        // skeleton's axis.
        let near_wood = |p: Vec3| {
            sk.wood.iter().any(|s| {
                let ab = s.b - s.a;
                let t = ((p - s.a).dot(ab) / ab.length_squared().max(1e-8)).clamp(0.0, 1.0);
                let r = s.ra + (s.rb - s.ra) * t;
                (s.a + ab * t).distance(p) <= r * 1.05 + 0.02
            })
        };
        let wood: Vec<&TreeVertex> = m.vertices.iter().filter(|v| v.normal[3] == WOOD).collect();
        assert!(wood.iter().all(|v| near_wood(Vec3::from(v.pos))));
        // Every card lies in a foliage cluster's reach.
        let near_leaves = |p: Vec3| {
            sk.foliage.iter().any(|b| {
                let ab = b.b - b.a;
                let t = ((p - b.a).dot(ab) / ab.length_squared().max(1e-8)).clamp(0.0, 1.0);
                (b.a + ab * t).distance(p) <= b.r + 2.4
            })
        };
        let leaves: Vec<&TreeVertex> = m.vertices.iter().filter(|v| v.normal[3] == LEAF).collect();
        assert!(!leaves.is_empty());
        assert!(leaves.iter().all(|v| near_leaves(Vec3::from(v.pos))));
        // Normals are unit length.
        for v in &m.vertices {
            let n = Vec3::new(v.normal[0] as f32, v.normal[1] as f32, v.normal[2] as f32) / 127.0;
            assert!((n.length() - 1.0).abs() < 0.03, "{n}");
        }
    }

    #[test]
    fn detail_falls_with_distance_and_the_trunk_stays() {
        let (_, full) = oak_mesh(Stage::Mature, Detail::Full);
        let (_, reduced) = oak_mesh(Stage::Mature, Detail::Reduced);
        let (_, low) = oak_mesh(Stage::Mature, Detail::Low);
        eprintln!(
            "an oak: {} / {} / {} triangles, {} KiB at full detail",
            full.triangles(),
            reduced.triangles(),
            low.triangles(),
            full.bytes() / 1024
        );
        assert!(reduced.triangles() * 2 < full.triangles());
        assert!(low.triangles() * 3 < reduced.triangles());
        assert!(full.triangles() < 70_000, "{}", full.triangles());
        assert!(reduced.triangles() < 15_000, "{}", reduced.triangles());
        assert!(low.triangles() < 3_000, "{}", low.triangles());
        // All three stand as tall.
        for m in [&reduced, &low] {
            assert!(
                (m.max.y - full.max.y).abs() < 2.5,
                "{} {}",
                m.max.y,
                full.max.y
            );
        }
    }

    #[test]
    fn a_stump_is_capped_and_a_snag_bare() {
        let sp = crate::growth::tests::oak();
        let sk = grow(&sp, Stage::Mature, 1);
        let mut o = Options::new(Detail::Full, LeafKind::Broad, false, 0.6);
        o.cut = Some(0.8);
        let stump = mesh(&sk, o, 1);
        assert!(stump.max.y <= 0.85, "{}", stump.max.y);
        assert_eq!(
            stump.indices.len() as u32,
            stump.wood,
            "no leaves on a stump"
        );
        assert!(stump.vertices.iter().any(|v| v.normal[3] == END_GRAIN));
        let mut o = Options::new(Detail::Full, LeafKind::Broad, false, 0.6);
        o.leaves = false;
        o.min_diameter = 0.15;
        let charred = mesh(&sk, o, 1);
        assert_eq!(charred.indices.len() as u32, charred.wood);
        let thin = charred
            .vertices
            .iter()
            .filter(|v| v.normal[3] == WOOD && v.sway[3] != 0)
            .all(|v| v.extra[1] >= 7);
        assert!(thin, "burned twigs are gone");
    }

    #[test]
    fn needles_grow_in_sprays_and_palms_in_fronds() {
        let mut sp = crate::growth::tests::oak();
        sp.form.leaf = LeafKind::Needle;
        sp.form.whorled = true;
        sp.form.crown = hearth_content::schema::flora::Crown::Conical;
        let sk = grow(&sp, Stage::Mature, 4);
        let m = mesh(
            &sk,
            Options::new(Detail::Full, LeafKind::Needle, false, 0.7),
            3,
        );
        let sprays = m
            .vertices
            .iter()
            .filter(|v| v.normal[3] == LEAF && v.extra[0] == LEAF_NEEDLE)
            .count();
        assert!(sprays > 200, "{sprays}");
        sp.form.crown = hearth_content::schema::flora::Crown::Palm;
        sp.form.leaf = LeafKind::Broad;
        let sk = grow(&sp, Stage::Mature, 4);
        let m = mesh(
            &sk,
            Options::new(Detail::Full, LeafKind::Broad, true, 0.7),
            3,
        );
        assert!(
            m.vertices
                .iter()
                .any(|v| v.normal[3] == LEAF && v.extra[0] == LEAF_FROND)
        );
    }

    #[test]
    fn the_same_tree_meshes_the_same() {
        let (_, a) = oak_mesh(Stage::Young, Detail::Full);
        let (_, b) = oak_mesh(Stage::Young, Detail::Full);
        assert_eq!(a, b);
    }
}

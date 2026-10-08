//! The dual meshers (S0's prototypes; `docs/design/smooth-terrain.md`). A cell is the cube
//! between eight neighbouring samples; every cell the surface crosses gets one vertex, and every
//! grid edge whose two samples lie on either side of the surface gets a quad joining the
//! vertices of the four cells around it. The methods differ only in where a cell's vertex goes.
//!
//! All of a vertex's arithmetic is done in its cell's own coordinates from the samples around
//! it, so a cell meshed as part of any block, with its apron, comes out bit for bit the same:
//! blocks meet without cracks.

use glam::{IVec3, Vec3};

use crate::field::{Field, dequantize, solid};
use crate::mesh::Mesh;
use crate::qef::Qef;

/// Samples a block needs beyond its region on every side: the cells straddling its faces reach
/// one sample out, and their corners' gradients (and the relaxation's neighbours) one more.
pub const APRON: usize = 2;

/// Materials a vertex blends, at most (S §3.2).
pub const MAX_BLEND: usize = 4;

/// How far the one relaxation pass moves a Surface Nets vertex toward the mean of its linked
/// neighbours (Gibson's constrained elastic net, one step).
const RELAX: f32 = 0.5;

/// The feature solve's pull toward the crossings' mean, against planes of unit weight: small
/// enough to leave a crease where planes meet, large enough to hold a vertex in place along a
/// flat or gently curved surface where the planes don't pin it.
const LAMBDA: f32 = 0.05;

/// Extra weight of a cell's upper corners in its vertex's materials, so the material on top
/// shows (grass over soil, snow over rock).
const TOP_BIAS: f32 = 1.0;

/// How far the crossings' planes may miss the feature solve (mean squared distance, in cells²)
/// and still be trusted to meet at a crease or corner. Beyond the upper bound the cell holds
/// more surface than one vertex can show (a notch or ledge narrower than the grid) and the solve
/// would only scatter vertices into a jagged edge; Surface Nets with sharp features then falls
/// back to the smooth mean.
const TRUST: (f32, f32) = (0.004, 0.025);

/// How far inside its cell a solved vertex is kept, so neighbouring cells' vertices clamped to
/// their shared face never coincide (a triangle of no area).
const INSET: f32 = 0.02;

/// Corner `i` of a cell is at offset (i & 1, i >> 1 & 1, i >> 2 & 1).
const CORNERS: [[usize; 3]; 8] = [
    [0, 0, 0],
    [1, 0, 0],
    [0, 1, 0],
    [1, 1, 0],
    [0, 0, 1],
    [1, 0, 1],
    [0, 1, 1],
    [1, 1, 1],
];

/// A cell's twelve edges: lower corner, upper corner, axis.
const EDGES: [(usize, usize, usize); 12] = [
    (0, 1, 0),
    (2, 3, 0),
    (4, 5, 0),
    (6, 7, 0),
    (0, 2, 1),
    (1, 3, 1),
    (4, 6, 1),
    (5, 7, 1),
    (0, 4, 2),
    (1, 5, 2),
    (2, 6, 2),
    (3, 7, 2),
];

/// The corners of a cell's faces: −x, +x, −y, +y, −z, +z.
const FACES: [[usize; 4]; 6] = [
    [0, 2, 4, 6],
    [1, 3, 5, 7],
    [0, 1, 4, 5],
    [2, 3, 6, 7],
    [0, 1, 2, 3],
    [4, 5, 6, 7],
];

const NONE: u32 = u32::MAX;

/// Where a cell's vertex goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Method {
    /// At the mean of the cell's edge crossings, relaxed once toward its linked neighbours':
    /// smooth everywhere, creases rounded.
    SurfaceNets,
    /// Between the crossings' mean and the feature solve, by the materials' sharpness: soft
    /// ground smooth, rock with its creases. No relaxation: it would shrink soft forms (a
    /// spoil heap's top, a dune's crest) toward the blobby.
    SharpNets,
    /// At the feature solve of the crossings' planes everywhere (Dual Contouring).
    DualContouring,
}

impl Method {
    pub const ALL: [Method; 3] = [
        Method::SurfaceNets,
        Method::SharpNets,
        Method::DualContouring,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Method::SurfaceNets => "Surface Nets",
            Method::SharpNets => "Surface Nets, sharp features",
            Method::DualContouring => "Dual Contouring",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Method::SurfaceNets => "sn",
            Method::SharpNets => "sharp",
            Method::DualContouring => "dc",
        }
    }
}

/// What the mesher needs to know of materials.
pub trait Materials {
    /// How crisp the material's surface is: 0 follows the smoothed field, 1 keeps the creases
    /// the field turns (S §2.3).
    fn sharpness(&self, material: u16) -> f32;
}

impl<F: Fn(u16) -> f32> Materials for F {
    fn sharpness(&self, material: u16) -> f32 {
        self(material)
    }
}

/// The grid edges a mesh covers, in the field's sample coordinates: every edge whose lower
/// sample lies in `lo..hi` on each axis. Neighbouring regions share no edge, so their meshes
/// share no triangle; together they cover exactly what their union would.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub lo: [usize; 3],
    pub hi: [usize; 3],
}

impl Region {
    /// Everything in a field that has its apron.
    pub fn interior(field: &Field) -> Region {
        let size = field.size();
        Region {
            lo: [APRON; 3],
            hi: size.map(|s| s.saturating_sub(APRON).max(APRON)),
        }
    }

    /// The region shrunk to what `field` can mesh.
    fn within(self, field: &Field) -> Region {
        let inner = Region::interior(field);
        let lo = [0, 1, 2].map(|a| self.lo[a].max(inner.lo[a]));
        let hi = [0, 1, 2].map(|a| self.hi[a].min(inner.hi[a]).max(lo[a]));
        Region { lo, hi }
    }
}

/// Meshes the surface through `region` of `field` (shrunk to keep [`APRON`] samples of the
/// field around it).
pub fn mesh(field: &Field, region: Region, method: Method, materials: &dyn Materials) -> Mesh {
    let region = region.within(field);
    let mut mesher = Mesher::new(field, region, method, materials);
    mesher.run();
    mesher.out
}

/// A cell's corner samples and edge crossings, in the cell's coordinates.
struct Cell {
    distance: [f32; 8],
    /// Bit `i` set when corner `i` is inside.
    inside: u8,
    points: [Vec3; 12],
    edges: [u8; 12],
    crossings: usize,
}

impl Cell {
    fn mean(&self) -> Vec3 {
        let mut sum = Vec3::ZERO;
        for p in &self.points[..self.crossings] {
            sum += *p;
        }
        sum / self.crossings as f32
    }

    fn corner_inside(&self, i: usize) -> bool {
        self.inside >> i & 1 == 1
    }
}

struct Mesher<'a> {
    field: &'a Field,
    region: Region,
    method: Method,
    materials: &'a dyn Materials,
    /// The cells a vertex or a relaxation can need: lowest sample in `lo − 2 ..= hi`.
    base: [usize; 3],
    dims: [usize; 3],
    vertex: Vec<u32>,
    /// Per cell: 0 not looked at, 1 crossed (its mean in `mass`), 2 not crossed.
    state: Vec<u8>,
    mass: Vec<Vec3>,
    /// Each vertex's position in its own cell's coordinates.
    local: Vec<Vec3>,
    out: Mesh,
}

impl<'a> Mesher<'a> {
    fn new(field: &'a Field, region: Region, method: Method, materials: &'a dyn Materials) -> Self {
        let base = region.lo.map(|l| l - 2);
        let dims = [0, 1, 2].map(|a| region.hi[a] + 1 - base[a]);
        let n = dims[0] * dims[1] * dims[2];
        Self {
            field,
            region,
            method,
            materials,
            base,
            dims,
            vertex: vec![NONE; n],
            state: vec![0; n],
            mass: vec![Vec3::ZERO; n],
            local: Vec::new(),
            out: Mesh::default(),
        }
    }

    fn key(&self, c: [usize; 3]) -> usize {
        let [x, y, z] = [0, 1, 2].map(|a| c[a] - self.base[a]);
        (y * self.dims[2] + z) * self.dims[0] + x
    }

    fn run(&mut self) {
        let Region { lo, hi } = self.region;
        for y in lo[1]..hi[1] {
            for z in lo[2]..hi[2] {
                for x in lo[0]..hi[0] {
                    let inside = solid(self.field.fill(x, y, z));
                    for axis in 0..3 {
                        let mut q = [x, y, z];
                        q[axis] += 1;
                        if solid(self.field.fill(q[0], q[1], q[2])) != inside {
                            self.quad([x, y, z], axis, inside);
                        }
                    }
                }
            }
        }
    }

    /// The quad around the edge from sample `p` along `axis`, facing away from the inside end.
    fn quad(&mut self, p: [usize; 3], axis: usize, inside_low: bool) {
        let [x, y, z] = p;
        // The four cells around the edge, counter-clockwise seen from the edge's upper end.
        let mut cells = match axis {
            0 => [[x, y - 1, z - 1], [x, y, z - 1], [x, y, z], [x, y - 1, z]],
            1 => [[x - 1, y, z - 1], [x - 1, y, z], [x, y, z], [x, y, z - 1]],
            _ => [[x - 1, y - 1, z], [x, y - 1, z], [x, y, z], [x - 1, y, z]],
        };
        if !inside_low {
            cells.swap(1, 3);
        }
        let v = cells.map(|c| self.vertex(c));
        // Corners relative to `p`'s cell, to judge the split without leaving local coordinates.
        let at = |k: usize| {
            let off = Vec3::new(
                cells[k][0] as f32 - x as f32,
                cells[k][1] as f32 - y as f32,
                cells[k][2] as f32 - z as f32,
            );
            off + self.local[v[k] as usize]
        };
        let corners = [at(0), at(1), at(2), at(3)];
        let tris = if self.split_02(p, corners) {
            [v[0], v[1], v[2], v[0], v[2], v[3]]
        } else {
            [v[0], v[1], v[3], v[1], v[2], v[3]]
        };
        self.out.indices.extend_from_slice(&tris);
    }

    /// Whether to cut the quad along its 0–2 diagonal: the one whose midpoint lies nearer the
    /// surface, so a ridge or a valley runs along triangle edges instead of being notched
    /// across; failing that the shorter.
    fn split_02(&self, p: [usize; 3], c: [Vec3; 4]) -> bool {
        let d02 = self.distance_near(p, (c[0] + c[2]) * 0.5).abs();
        let d13 = self.distance_near(p, (c[1] + c[3]) * 0.5).abs();
        if (d02 - d13).abs() > 1e-4 {
            return d02 < d13;
        }
        let (l02, l13) = (c[0].distance_squared(c[2]), c[1].distance_squared(c[3]));
        if (l02 - l13).abs() > 1e-6 {
            return l02 < l13;
        }
        true
    }

    /// The distance at `local`, a point relative to the cell whose lowest sample is `cell`.
    fn distance_near(&self, cell: [usize; 3], local: Vec3) -> f32 {
        let f = local.floor();
        let c = [0, 1, 2].map(|a| (cell[a] as f32 + f[a]) as usize);
        self.field.trilinear(c, local - f)
    }

    fn cell(&self, c: [usize; 3]) -> Cell {
        let mut cell = Cell {
            distance: [0.0; 8],
            inside: 0,
            points: [Vec3::ZERO; 12],
            edges: [0; 12],
            crossings: 0,
        };
        for (i, o) in CORNERS.iter().enumerate() {
            let q = self.field.fill(c[0] + o[0], c[1] + o[1], c[2] + o[2]);
            cell.distance[i] = dequantize(q);
            if solid(q) {
                cell.inside |= 1 << i;
            }
        }
        if cell.inside == 0 || cell.inside == 0xff {
            return cell;
        }
        for (e, &(a, b, axis)) in EDGES.iter().enumerate() {
            if cell.corner_inside(a) != cell.corner_inside(b) {
                let (da, db) = (cell.distance[a], cell.distance[b]);
                let mut p = corner(a);
                p[axis] = da / (da - db);
                cell.points[cell.crossings] = p;
                cell.edges[cell.crossings] = e as u8;
                cell.crossings += 1;
            }
        }
        cell
    }

    /// The mean of a cell's crossings (in its coordinates), or `None` where the surface misses it.
    fn mass(&mut self, c: [usize; 3]) -> Option<Vec3> {
        let k = self.key(c);
        match self.state[k] {
            1 => Some(self.mass[k]),
            2 => None,
            _ => {
                let cell = self.cell(c);
                if cell.crossings == 0 {
                    self.state[k] = 2;
                    None
                } else {
                    let m = cell.mean();
                    self.state[k] = 1;
                    self.mass[k] = m;
                    Some(m)
                }
            }
        }
    }

    fn vertex(&mut self, c: [usize; 3]) -> u32 {
        let k = self.key(c);
        if self.vertex[k] != NONE {
            return self.vertex[k];
        }
        let cell = self.cell(c);
        let gradients = CORNERS.map(|o| self.field.gradient(c[0] + o[0], c[1] + o[1], c[2] + o[2]));
        let sharp_gradients = || {
            CORNERS.map(|o| {
                self.field
                    .gradient_sharp(c[0] + o[0], c[1] + o[1], c[2] + o[2])
            })
        };
        let mean = cell.mean();
        let local = match self.method {
            Method::SurfaceNets => self.relaxed(c, &cell, mean),
            Method::SharpNets => {
                let s = self.cell_sharpness(c, &cell);
                if s > 0.0 {
                    let (at, trust) = feature(&cell, &sharp_gradients(), mean);
                    mean.lerp(at, (s * trust).min(1.0))
                } else {
                    mean
                }
            }
            Method::DualContouring => feature(&cell, &sharp_gradients(), mean).0,
        };
        let normal = normal_at(&cell, &gradients, local);
        let (materials, weights) = self.blend(c, &cell, local);
        let sharpness = materials
            .iter()
            .zip(weights)
            .map(|(&m, w)| w * self.materials.sharpness(m))
            .sum();
        let at = self.field.origin() + IVec3::new(c[0] as i32, c[1] as i32, c[2] as i32);
        let index = self.out.positions.len() as u32;
        self.out.positions.push(at.as_vec3() + local);
        self.out.normals.push(normal);
        self.out.materials.push(materials);
        self.out.weights.push(weights);
        self.out.sharpness.push(sharpness);
        self.out.cells.push(at);
        self.local.push(local);
        self.vertex[k] = index;
        index
    }

    /// The mean moved once toward the mean of the linked neighbours' means (the cells across the
    /// faces the surface crosses), kept in the cell.
    fn relaxed(&mut self, c: [usize; 3], cell: &Cell, mean: Vec3) -> Vec3 {
        let mut sum = Vec3::ZERO;
        let mut linked = 0u32;
        for (f, face) in FACES.iter().enumerate() {
            let inside = face.iter().filter(|&&i| cell.corner_inside(i)).count();
            if inside == 0 || inside == 4 {
                continue;
            }
            let (axis, up) = (f / 2, f % 2 == 1);
            let mut n = c;
            if up {
                n[axis] += 1;
            } else {
                n[axis] -= 1;
            }
            if let Some(mut m) = self.mass(n) {
                m[axis] += if up { 1.0 } else { -1.0 };
                sum += m;
                linked += 1;
            }
        }
        if linked == 0 {
            return mean;
        }
        let target = sum / linked as f32;
        (mean + (target - mean) * RELAX).clamp(Vec3::ZERO, Vec3::ONE)
    }

    /// The sharpness of a cell's inside corners' materials, upper corners counting more.
    fn cell_sharpness(&self, c: [usize; 3], cell: &Cell) -> f32 {
        let (mut sum, mut weight) = (0.0, 0.0);
        for (i, o) in CORNERS.iter().enumerate() {
            if cell.corner_inside(i) {
                let w = 1.0 + TOP_BIAS * o[1] as f32;
                let m = self.field.material(c[0] + o[0], c[1] + o[1], c[2] + o[2]);
                sum += w * self.materials.sharpness(m);
                weight += w;
            }
        }
        if weight > 0.0 { sum / weight } else { 0.0 }
    }

    /// The vertex's materials and weights: the inside corners' materials, each by its nearness
    /// to the vertex with upper corners counting more, the heaviest [`MAX_BLEND`] kept.
    fn blend(&self, c: [usize; 3], cell: &Cell, at: Vec3) -> ([u16; MAX_BLEND], [f32; MAX_BLEND]) {
        let mut ids = [0u16; 8];
        let mut ws = [0.0f32; 8];
        let mut n = 0;
        for (i, o) in CORNERS.iter().enumerate() {
            if !cell.corner_inside(i) {
                continue;
            }
            let near = [0, 1, 2]
                .map(|a| if o[a] == 1 { at[a] } else { 1.0 - at[a] })
                .iter()
                .product::<f32>()
                + 1e-3;
            let w = near * (1.0 + TOP_BIAS * o[1] as f32);
            let m = self.field.material(c[0] + o[0], c[1] + o[1], c[2] + o[2]);
            match ids[..n].iter().position(|&id| id == m) {
                Some(j) => ws[j] += w,
                None => {
                    ids[n] = m;
                    ws[n] = w;
                    n += 1;
                }
            }
        }
        // Heaviest first; equal weights by material id, so the order never depends on corners.
        let mut order = [0usize, 1, 2, 3, 4, 5, 6, 7];
        order[..n].sort_by(|&a, &b| ws[b].total_cmp(&ws[a]).then(ids[a].cmp(&ids[b])));
        let kept = n.min(MAX_BLEND);
        let total: f32 = order[..kept].iter().map(|&j| ws[j]).sum();
        let mut materials = [ids[order[0]]; MAX_BLEND];
        let mut weights = [0.0; MAX_BLEND];
        for (slot, &j) in order[..kept].iter().enumerate() {
            materials[slot] = ids[j];
            weights[slot] = ws[j] / total;
        }
        (materials, weights)
    }
}

/// Corner `i`'s position in its cell.
fn corner(i: usize) -> Vec3 {
    let o = CORNERS[i];
    Vec3::new(o[0] as f32, o[1] as f32, o[2] as f32)
}

/// The feature solve of a cell's crossings, each a plane through the crossing with the surface's
/// normal there, held toward the crossings' mean and kept in the cell, and how far to trust it
/// (1 where the planes meet cleanly, 0 where they disagree; see [`TRUST`]). `gradients` are the
/// corners' crease-keeping gradients; a crossing between two corners on different faces of a
/// crease (their gradients far apart) takes the nearer corner's rather than a blend of both.
fn feature(cell: &Cell, gradients: &[Vec3; 8], mean: Vec3) -> (Vec3, f32) {
    let mut qef = Qef::default();
    for k in 0..cell.crossings {
        let (a, b, axis) = EDGES[cell.edges[k] as usize];
        let p = cell.points[k];
        let (na, nb) = (
            (-gradients[a]).normalize_or_zero(),
            (-gradients[b]).normalize_or_zero(),
        );
        let n = if na.dot(nb) > 0.95 {
            na.lerp(nb, p[axis]).normalize_or_zero()
        } else if p[axis] < 0.5 {
            na
        } else {
            nb
        };
        if n != Vec3::ZERO {
            qef.add(p, n);
        }
    }
    if qef.count() == 0 {
        return (mean, 0.0);
    }
    let at = qef
        .solve(mean, LAMBDA)
        .clamp(Vec3::splat(INSET), Vec3::splat(1.0 - INSET));
    let miss = qef.error(at) / qef.count() as f32;
    let t = ((miss - TRUST.0) / (TRUST.1 - TRUST.0)).clamp(0.0, 1.0);
    (at, 1.0 - t * t * (3.0 - 2.0 * t))
}

/// The outward normal at `at` in a cell: the opposite of the trilinear blend of its corners'
/// gradients; failing that (a field flat to the samples' precision) the crossings' mean normal.
fn normal_at(cell: &Cell, gradients: &[Vec3; 8], at: Vec3) -> Vec3 {
    let g = |i: usize| gradients[i];
    let c00 = g(0).lerp(g(1), at.x);
    let c10 = g(2).lerp(g(3), at.x);
    let c01 = g(4).lerp(g(5), at.x);
    let c11 = g(6).lerp(g(7), at.x);
    let n = (-c00.lerp(c10, at.y).lerp(c01.lerp(c11, at.y), at.z)).normalize_or_zero();
    if n != Vec3::ZERO {
        return n;
    }
    // Inside corners toward outside ones: the way out of the ground.
    let mut out = Vec3::ZERO;
    for &(a, b, axis) in &EDGES {
        if cell.corner_inside(a) != cell.corner_inside(b) {
            out[axis] += if cell.corner_inside(a) { 1.0 } else { -1.0 };
        }
    }
    let out = out.normalize_or_zero();
    if out == Vec3::ZERO { Vec3::Y } else { out }
}

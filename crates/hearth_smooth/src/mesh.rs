//! The meshers' output and the checks run on it.

use std::collections::HashMap;

use glam::{IVec3, Vec3};

use crate::mesher::MAX_BLEND;

/// An indexed triangle mesh of the surface. Positions are in grid units with each voxel's centre
/// at its integer coordinates (a voxel's world box is its position ± 0.5).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub positions: Vec<Vec3>,
    /// Outward unit normals from the fill's gradient.
    pub normals: Vec<Vec3>,
    /// Up to [`MAX_BLEND`] materials a vertex, heaviest first; unused slots repeat the first
    /// with weight 0.
    pub materials: Vec<[u16; MAX_BLEND]>,
    /// The materials' weights, summing to one.
    pub weights: Vec<[f32; MAX_BLEND]>,
    /// The blended materials' sharpness: how much shading should follow the faces (crisp rock)
    /// rather than the smooth normals.
    pub sharpness: Vec<f32>,
    /// The grid cell each vertex stands for (the cell's lowest sample, as a grid position): the
    /// key that joins meshes of neighbouring blocks, and the debug views' cell bounds.
    pub cells: Vec<IVec3>,
    /// Counter-clockwise triangles seen from outside.
    pub indices: Vec<u32>,
}

/// What [`Mesh::check`] finds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Check {
    pub triangles: usize,
    /// Edges of one triangle only: the mesh's open border (where its region ends) or a crack.
    pub boundary_edges: usize,
    /// Edges shared by more than two triangles.
    pub non_manifold_edges: usize,
    /// Edges two triangles both run the same way: one of them faces the wrong way.
    pub misoriented_edges: usize,
    /// Triangles with (almost) no area.
    pub degenerate: usize,
    /// Triangles facing against their vertices' normals: a fold in the surface.
    pub folded: usize,
}

impl Check {
    /// No defect but open borders.
    pub fn sound(&self) -> bool {
        self.non_manifold_edges == 0
            && self.misoriented_edges == 0
            && self.degenerate == 0
            && self.folded == 0
    }
}

impl Mesh {
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// The corners of triangle `t`.
    pub fn triangle(&self, t: usize) -> [Vec3; 3] {
        let i = &self.indices[t * 3..t * 3 + 3];
        [
            self.positions[i[0] as usize],
            self.positions[i[1] as usize],
            self.positions[i[2] as usize],
        ]
    }

    /// Appends another mesh's vertices and triangles.
    pub fn append(&mut self, other: &Mesh) {
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&other.positions);
        self.normals.extend_from_slice(&other.normals);
        self.materials.extend_from_slice(&other.materials);
        self.weights.extend_from_slice(&other.weights);
        self.sharpness.extend_from_slice(&other.sharpness);
        self.cells.extend_from_slice(&other.cells);
        self.indices.extend(other.indices.iter().map(|i| i + base));
    }

    /// The mesh with vertices of the same cell merged (the first kept): meshes of neighbouring
    /// blocks appended together become one surface.
    pub fn welded(&self) -> Mesh {
        let mut out = Mesh::default();
        let mut seen: HashMap<IVec3, u32> = HashMap::new();
        let mut remap = Vec::with_capacity(self.positions.len());
        for v in 0..self.positions.len() {
            let next = out.positions.len() as u32;
            let idx = *seen.entry(self.cells[v]).or_insert(next);
            if idx == next {
                out.positions.push(self.positions[v]);
                out.normals.push(self.normals[v]);
                out.materials.push(self.materials[v]);
                out.weights.push(self.weights[v]);
                out.sharpness.push(self.sharpness[v]);
                out.cells.push(self.cells[v]);
            }
            remap.push(idx);
        }
        out.indices = self.indices.iter().map(|&i| remap[i as usize]).collect();
        out
    }

    /// Checks the mesh's topology and triangles.
    pub fn check(&self) -> Check {
        let mut edges: HashMap<(u32, u32), (u32, u32)> = HashMap::new();
        let mut check = Check {
            triangles: self.triangle_count(),
            ..Check::default()
        };
        for t in 0..self.triangle_count() {
            let i = [
                self.indices[t * 3],
                self.indices[t * 3 + 1],
                self.indices[t * 3 + 2],
            ];
            for k in 0..3 {
                let (a, b) = (i[k], i[(k + 1) % 3]);
                let e = edges.entry((a.min(b), a.max(b))).or_insert((0, 0));
                if a < b {
                    e.0 += 1;
                } else {
                    e.1 += 1;
                }
            }
            let [p0, p1, p2] = self.triangle(t);
            let n = (p1 - p0).cross(p2 - p0);
            if n.length_squared() < 1e-12 {
                check.degenerate += 1;
            } else {
                let smooth = self.normals[i[0] as usize]
                    + self.normals[i[1] as usize]
                    + self.normals[i[2] as usize];
                if n.dot(smooth) < 0.0 {
                    check.folded += 1;
                }
            }
        }
        for &(forward, backward) in edges.values() {
            match forward + backward {
                1 => check.boundary_edges += 1,
                2 if forward != 1 => check.misoriented_edges += 1,
                2 => {}
                _ => check.non_manifold_edges += 1,
            }
        }
        check
    }

    /// The triangles as sorted lists of their corners' cells, each triangle rotated to start at
    /// its smallest cell (orientation kept): two meshes of the same surface give the same list.
    pub fn canonical_triangles(&self) -> Vec<[IVec3; 3]> {
        let key = |c: IVec3| (c.x, c.y, c.z);
        let mut out: Vec<[IVec3; 3]> = (0..self.triangle_count())
            .map(|t| {
                let c = [0, 1, 2].map(|k| self.cells[self.indices[t * 3 + k] as usize]);
                let first = (0..3).min_by_key(|&k| key(c[k])).unwrap_or(0);
                [c[first], c[(first + 1) % 3], c[(first + 2) % 3]]
            })
            .collect();
        out.sort_by_key(|t| t.map(key));
        out
    }
}

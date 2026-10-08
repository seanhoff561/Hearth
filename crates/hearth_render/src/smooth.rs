//! The smooth ground's meshes (Amendment S §3.2): each cube's natural ground meshed through its
//! fill by Surface Nets with sharp features (D222, `hearth_smooth`), with two voxels of apron so
//! neighbouring cubes meet without a crack, into compact vertices: position, normal, up to four
//! blended materials, light, ambient occlusion and the place's climate (for grass's colour) in
//! 24 bytes.

use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Vec3};
use hearth_math::{BlockPos, CubePos};
use hearth_smooth::{APRON, Field, MAX_BLEND, Method, Region};
use hearth_world::{BlockRegistry, BlockStateId, CubeMap};

/// Samples a window takes on each axis: the cube and its apron.
pub const WINDOW: usize = 16 + 2 * APRON;

/// A material slot that is no ground.
pub const NO_SLOT: u8 = u8::MAX;

/// The natural blocks as the ground's materials: each a slot, with its sharpness (from its
/// ground family) and its colour.
#[derive(Debug, Clone, Default)]
pub struct GroundMaterials {
    /// Each state's slot, or [`NO_SLOT`].
    slot_of: Vec<u8>,
    pub slots: Vec<GroundSlot>,
}

/// One of the ground's materials.
#[derive(Debug, Clone, PartialEq)]
pub struct GroundSlot {
    /// The natural block it is (`hearth:loam`, `hearth:grass_block`).
    pub block: String,
    /// Its physical material, if the block names one.
    pub material: Option<String>,
    pub sharpness: f32,
}

impl GroundMaterials {
    /// The natural blocks of `reg`, in the registry's order; `sharpness` gives a material's
    /// (none: a block that names no material).
    pub fn new(reg: &BlockRegistry, sharpness: &dyn Fn(Option<&str>) -> f32) -> Self {
        let (slot_of, ids) = reg.ground_slots();
        let slots = ids
            .into_iter()
            .map(|id| {
                let b = reg.block(id);
                GroundSlot {
                    block: b.name.to_string(),
                    material: b.def.material.clone(),
                    sharpness: sharpness(b.def.material.as_deref()).clamp(0.0, 1.0),
                }
            })
            .collect();
        Self { slot_of, slots }
    }

    #[inline]
    pub fn slot(&self, s: BlockStateId) -> u8 {
        self.slot_of.get(s.0 as usize).copied().unwrap_or(NO_SLOT)
    }

    pub fn sharpness(&self, slot: u16) -> f32 {
        self.slots.get(slot as usize).map_or(0.5, |s| s.sharpness)
    }
}

/// A vertex of the smooth ground (24 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub struct SmoothVertex {
    /// Position in the cube, (metres + 1) × 2048: half a millimetre over −1..31 m.
    pub pos: [u16; 3],
    /// Outward normal, octahedral, snorm.
    pub normal: [i8; 2],
    /// Up to four material slots, heaviest first.
    pub materials: [u8; 4],
    /// Their weights, unorm, summing to about 255.
    pub weights: [u8; 4],
    /// Sky light << 4 | block light.
    pub light: u8,
    /// Ambient occlusion, unorm (255 open).
    pub ao: u8,
    /// How far shading follows the faces (crisp rock) rather than the smooth normal, unorm.
    pub sharpness: u8,
    /// How much of the ground about it lies under snow, unorm (until S7 makes snow fill).
    pub snow: u8,
    /// The column's climate code (`mesh::ColumnTints::climate`): grass's colour by place and
    /// season.
    pub climate: u32,
}

/// A cube's smooth ground: vertices and counter-clockwise triangles.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SmoothMesh {
    pub vertices: Vec<SmoothVertex>,
    pub indices: Vec<u16>,
}

impl SmoothMesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn gpu_bytes(&self) -> usize {
        self.vertices.len() * std::mem::size_of::<SmoothVertex>() + self.indices.len() * 2
    }

    /// A vertex's position in the cube (m).
    pub fn position(v: &SmoothVertex) -> Vec3 {
        Vec3::new(v.pos[0] as f32, v.pos[1] as f32, v.pos[2] as f32) / 2048.0 - Vec3::ONE
    }
}

/// The fill of a cube and its apron as a field in the window's own coordinates (sample (0, 0, 0)
/// the cube's lowest voxel less the apron, so positions stay small and exact far from the
/// world's origin); materials are ground slots. Cubes not loaded are empty.
pub fn window(map: &CubeMap, reg: &BlockRegistry, ground: &GroundMaterials, pos: CubePos) -> Field {
    let o = pos.min_block();
    let corner = IVec3::new(o.x, o.y, o.z) - IVec3::splat(APRON as i32);
    let n = WINDOW * WINDOW * WINDOW;
    let mut fill = vec![-127i8; n];
    let mut material = vec![0u16; n];
    for y in 0..WINDOW {
        for z in 0..WINDOW {
            for x in 0..WINDOW {
                let p = BlockPos::new(
                    corner.x + x as i32,
                    corner.y + y as i32,
                    corner.z + z as i32,
                );
                let i = (y * WINDOW + z) * WINDOW + x;
                if let Some(q) = map.fill(p, reg) {
                    fill[i] = q;
                    let s = map.block(p).unwrap_or_default();
                    let slot = ground.slot(s);
                    material[i] = if slot == NO_SLOT { 0 } else { slot as u16 };
                }
            }
        }
    }
    Field::from_parts(IVec3::ZERO, [WINDOW; 3], fill, material).expect("a window's arrays")
}

/// What the mesher reads of the cube about it, in the cube's coordinates (−1..=16).
pub struct Surroundings<'a> {
    /// `sky << 4 | block` at a voxel, and whether it is open (not solid ground).
    pub light: &'a dyn Fn(IVec3) -> (u8, bool),
    /// A column's climate code (0..16 each).
    pub climate: &'a dyn Fn(usize, usize) -> u32,
    /// Whether snow lies in a voxel (a layer of it, or snowy ground).
    pub snow: &'a dyn Fn(IVec3) -> bool,
}

/// Meshes a cube's smooth ground from its window.
pub fn mesh_cube(field: &Field, ground: &GroundMaterials, about: &Surroundings) -> SmoothMesh {
    let region = Region {
        lo: [APRON; 3],
        hi: [APRON + 16; 3],
    };
    let sharp = |m: u16| ground.sharpness(m);
    let mesh = hearth_smooth::mesh(field, region, Method::SharpNets, &sharp);
    let mut out = SmoothMesh {
        vertices: Vec::with_capacity(mesh.positions.len()),
        indices: Vec::with_capacity(mesh.indices.len()),
    };
    if mesh.positions.len() > u16::MAX as usize {
        log::warn!(
            "a cube's ground has {} vertices; not drawn",
            mesh.positions.len()
        );
        return out;
    }
    for (k, p) in mesh.positions.iter().enumerate() {
        // The window's grid units, a voxel's centre at its integer sample, into the cube's
        // metres: the cell's integer corner and the place in the cell rounded apart, so the same
        // vertex meshed in two cubes comes out the same.
        let cell = mesh.cells[k];
        let within = ((*p - cell.as_vec3()) * 2048.0).round().as_ivec3();
        let q = ((cell - IVec3::splat(APRON as i32)) * 2048 + IVec3::splat(3072) + within)
            .clamp(IVec3::ZERO, IVec3::splat(65535));
        let local = q.as_vec3() / 2048.0 - Vec3::ONE;
        let n = mesh.normals[k];
        // Slivers under 8 % are dropped (they show as nothing) and the rest ordered by slot, so
        // corners holding the same materials name them alike.
        let mut blend: Vec<(u8, f32)> = (0..MAX_BLEND.min(4))
            .filter(|&b| mesh.weights[k][b] >= 0.08)
            .map(|b| (mesh.materials[k][b].min(254) as u8, mesh.weights[k][b]))
            .collect();
        if blend.is_empty() {
            blend.push((mesh.materials[k][0].min(254) as u8, 1.0));
        }
        blend.sort_by_key(|b| b.0);
        let sum: f32 = blend.iter().map(|b| b.1).sum();
        let mut materials = [blend[0].0; 4];
        let mut weights = [0u8; 4];
        for (b, (m, w)) in blend.iter().enumerate() {
            materials[b] = *m;
            weights[b] = (w / sum * 255.0).round() as u8;
        }
        out.vertices.push(SmoothVertex {
            pos: [q.x as u16, q.y as u16, q.z as u16],
            normal: octahedral(n),
            materials,
            weights,
            light: light_at(local, n, about.light),
            ao: (occlusion(field, *p, n) * 255.0).round() as u8,
            sharpness: (mesh.sharpness[k] * 255.0).round() as u8,
            snow: (snow_at(local, n, about.snow) * 255.0).round() as u8,
            climate: (about.climate)(
                local.x.floor().clamp(0.0, 15.0) as usize,
                local.z.floor().clamp(0.0, 15.0) as usize,
            ),
        });
    }
    out.indices.extend(mesh.indices.iter().map(|&i| i as u16));
    unify_materials(&mut out);
    out
}

/// A triangle's three corners must name the same materials in the same order, so the shader can
/// take them from one corner and blend by the weights the three carry. Where they differ (at a
/// change of material), the triangle gets corners of its own carrying the union of its corners'
/// materials (the four heaviest), each corner weighting them as it did; elsewhere corners stay
/// shared.
fn unify_materials(m: &mut SmoothMesh) {
    let n = m.indices.len() / 3;
    for t in 0..n {
        let c = [0, 1, 2].map(|k| m.indices[t * 3 + k] as usize);
        let v = c.map(|i| m.vertices[i]);
        let same = |a: &SmoothVertex, b: &SmoothVertex| {
            (0..4).all(|k| {
                a.materials[k] == b.materials[k] || (a.weights[k] == 0 && b.weights[k] == 0)
            })
        };
        if same(&v[0], &v[1]) && same(&v[0], &v[2]) {
            continue;
        }
        // The union, heaviest by the corners' summed weights.
        let mut total: Vec<(u8, u32)> = Vec::new();
        for x in &v {
            for k in 0..4 {
                if x.weights[k] == 0 {
                    continue;
                }
                match total.iter_mut().find(|(m, _)| *m == x.materials[k]) {
                    Some(e) => e.1 += x.weights[k] as u32,
                    None => total.push((x.materials[k], x.weights[k] as u32)),
                }
            }
        }
        total.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        total.truncate(4);
        total.sort_by_key(|t| t.0);
        let mut list = [total[0].0; 4];
        for (k, (mat, _)) in total.iter().enumerate() {
            list[k] = *mat;
        }
        if m.vertices.len() + 3 > u16::MAX as usize {
            return;
        }
        for (k, x) in v.iter().enumerate() {
            let mut w = [0u32; 4];
            for (slot, mat) in list.iter().enumerate().take(total.len()) {
                w[slot] = (0..4)
                    .filter(|&j| x.weights[j] > 0 && x.materials[j] == *mat)
                    .map(|j| x.weights[j] as u32)
                    .sum();
            }
            let sum: u32 = w.iter().sum::<u32>().max(1);
            let mut nv = *x;
            nv.materials = list;
            nv.weights = w.map(|wk| ((wk * 255 + sum / 2) / sum) as u8);
            m.indices[t * 3 + k] = m.vertices.len() as u16;
            m.vertices.push(nv);
        }
    }
}

/// A unit normal octahedral-encoded into two snorm bytes.
pub fn octahedral(n: Vec3) -> [i8; 2] {
    let n = n / (n.x.abs() + n.y.abs() + n.z.abs()).max(1e-6);
    let (mut x, mut z) = (n.x, n.z);
    if n.y < 0.0 {
        let (ox, oz) = (x, z);
        x = (1.0 - oz.abs()) * ox.signum();
        z = (1.0 - ox.abs()) * oz.signum();
    }
    [
        (x.clamp(-1.0, 1.0) * 127.0).round() as i8,
        (z.clamp(-1.0, 1.0) * 127.0).round() as i8,
    ]
}

/// The octahedral normal decoded (the shader's `oct_decode`).
pub fn octahedral_decode(e: [i8; 2]) -> Vec3 {
    let (x, z) = (e[0] as f32 / 127.0, e[1] as f32 / 127.0);
    let y = 1.0 - x.abs() - z.abs();
    let (mut x, mut z) = (x, z);
    if y < 0.0 {
        let (ox, oz) = (x, z);
        x = (1.0 - oz.abs()) * ox.signum();
        z = (1.0 - ox.abs()) * oz.signum();
    }
    Vec3::new(x, y, z).normalize_or(Vec3::Y)
}

/// The light at a point of the surface: the open voxels about it, by nearness, taken half a voxel
/// out along the normal so the ground's own darkness does not shade it.
fn light_at(local: Vec3, n: Vec3, light: &dyn Fn(IVec3) -> (u8, bool)) -> u8 {
    let p = local + n * 0.5 - Vec3::splat(0.5);
    let b = p.floor();
    let f = p - b;
    let (mut sky, mut blk, mut w) = (0.0f32, 0.0f32, 0.0f32);
    for k in 0..8 {
        let d = IVec3::new(k & 1, (k >> 1) & 1, (k >> 2) & 1);
        let v = (b.as_ivec3() + d).clamp(IVec3::splat(-1), IVec3::splat(16));
        let (l, open) = light(v);
        if !open {
            continue;
        }
        let wk = if d.x == 1 { f.x } else { 1.0 - f.x }
            * if d.y == 1 { f.y } else { 1.0 - f.y }
            * if d.z == 1 { f.z } else { 1.0 - f.z };
        sky += wk * (l >> 4) as f32;
        blk += wk * (l & 15) as f32;
        w += wk;
    }
    if w <= 1e-4 {
        return 0;
    }
    let sky = (sky / w).round().min(15.0) as u8;
    let blk = (blk / w).round().min(15.0) as u8;
    (sky << 4) | blk
}

/// The share of the voxels about a point of the surface (and just above it) that hold snow.
fn snow_at(local: Vec3, n: Vec3, snow: &dyn Fn(IVec3) -> bool) -> f32 {
    let p = local + n * 0.3 - Vec3::splat(0.5);
    let b = p.floor().as_ivec3();
    let mut hits = 0;
    for k in 0..8 {
        let v = (b + IVec3::new(k & 1, (k >> 1) & 1, (k >> 2) & 1))
            .clamp(IVec3::splat(-1), IVec3::splat(16));
        hits += snow(v) as u32;
    }
    hits as f32 / 8.0
}

/// Ambient occlusion from the fill (S §4.3): the ground found along a few short rays into the
/// hemisphere about the normal, 1 open … 0 enclosed.
fn occlusion(field: &Field, p: Vec3, n: Vec3) -> f32 {
    let t = if n.y.abs() < 0.9 {
        n.cross(Vec3::Y).normalize()
    } else {
        n.cross(Vec3::X).normalize()
    };
    let b = n.cross(t);
    let dirs = [
        n,
        (n + t).normalize(),
        (n - t).normalize(),
        (n + b).normalize(),
        (n - b).normalize(),
    ];
    let mut blocked = 0.0;
    for d in dirs {
        for (dist, w) in [(0.8f32, 0.6f32), (1.8, 0.4)] {
            // Inside the ground along the ray: how far (a voxel and more is fully dark).
            let inside = field.sample(p + d * dist).clamp(0.0, 1.0);
            blocked += w * inside;
        }
    }
    (1.0 - blocked / dirs.len() as f32).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normals_survive_their_two_bytes() {
        for i in 0..200 {
            let a = i as f32 * 0.37;
            let n = Vec3::new(a.cos() * (a * 0.5).sin(), (a * 1.3).cos(), a.sin()).normalize();
            let back = octahedral_decode(octahedral(n));
            assert!(back.dot(n) > 0.995, "{n:?} → {back:?}");
        }
        assert!(octahedral_decode(octahedral(Vec3::NEG_Y)).y < -0.99);
    }

    #[test]
    fn a_vertex_is_twenty_four_bytes() {
        assert_eq!(std::mem::size_of::<SmoothVertex>(), 24);
    }
}

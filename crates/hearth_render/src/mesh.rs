//! Cube meshing.
//!
//! Input is a padded 18³ neighbourhood (the cube plus a one-block border from its neighbours)
//! of block states and light. Output:
//! * [`PackedQuad`] — 16-byte quads for full-cube faces, greedily merged where the face is
//!   uniform (same texture, tint, light and AO at all corners), grouped by render layer and face
//!   direction (for per-direction culling);
//! * [`GeneralQuad`] — 64-byte quads for models, fluids and translucent surfaces;
//! * a face-connectivity bitset for cave culling.

use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};
use hearth_math::{CUBE_SIZE, CubePos, Direction, LocalPos};
use hearth_world::{BlockRegistry, BlockStateId, CubeMap, RenderLayer, StateFlags};

use crate::models::{BlockModels, FaceTex, ModelQuad, NO_OVERLAY, StateModel, Tint};

pub const PAD: usize = 18;
const PAD3: usize = PAD * PAD * PAD;

#[inline]
fn pidx(x: i32, y: i32, z: i32) -> usize {
    // x, y, z in -1..=16
    (((y + 1) as usize) * PAD + (z + 1) as usize) * PAD + (x + 1) as usize
}

/// Per-column tint inputs (index `z * 16 + x`): the 24-bit climate code that the shader turns
/// into seasonal grass and leaf colours (layout in `hearth_env::tint`), and baked water colours.
#[derive(Debug, Clone)]
pub struct ColumnTints {
    pub climate: [u32; 256],
    pub water: [[u8; 3]; 256],
}

/// Climate code of a mild temperate place (12 °C mean, 16 °C range, 900 mm).
pub const DEFAULT_CLIMATE: u32 = 155 | (8 << 8) | (99 << 13);

/// Tint kinds understood by the shader (low 2 bits; the next 2 bits are a variant).
pub const TINT_RGB: u32 = 0;
pub const TINT_GRASS: u32 = 1;
pub const TINT_DECIDUOUS: u32 = 2;
pub const TINT_EVERGREEN: u32 = 3;
/// Deciduous variant with yellow autumn colour (birch).
pub const VARIANT_BIRCH: u32 = 1 << 2;
/// Deciduous variant with red autumn colour (maples, cherry, hawthorn).
pub const VARIANT_RED: u32 = 2 << 2;
/// Dry-season type 3 (arid) in the climate code: grass that is mostly cured.
const ARID_BITS: u32 = 3 << 21;

impl Default for ColumnTints {
    fn default() -> Self {
        Self {
            climate: [DEFAULT_CLIMATE; 256],
            water: [[63, 118, 228]; 256],
        }
    }
}

impl ColumnTints {
    /// Shader tint kind (4 bits) and 24-bit value (RGB or climate code) for a face.
    #[inline]
    pub fn get(&self, tint: Tint, lx: usize, lz: usize) -> (u32, u32) {
        let i = (lz.min(15)) * 16 + lx.min(15);
        let rgb = |c: [u8; 3]| (c[0] as u32) | ((c[1] as u32) << 8) | ((c[2] as u32) << 16);
        match tint {
            Tint::None => (TINT_RGB, 0x00ff_ffff),
            Tint::Water => (TINT_RGB, rgb(self.water[i])),
            Tint::Grass => (TINT_GRASS, self.climate[i]),
            Tint::DryGrass => (TINT_GRASS, (self.climate[i] & !(3 << 21)) | ARID_BITS),
            Tint::Foliage => (TINT_DECIDUOUS, self.climate[i]),
            Tint::Birch => (TINT_DECIDUOUS | VARIANT_BIRCH, self.climate[i]),
            Tint::FoliageRed => (TINT_DECIDUOUS | VARIANT_RED, self.climate[i]),
            Tint::Spruce => (TINT_EVERGREEN, self.climate[i]),
        }
    }
}

/// Padded neighbourhood of one cube.
pub struct MeshInput {
    pub pos: CubePos,
    pub blocks: Box<[BlockStateId; PAD3]>,
    /// `sky << 4 | block` per block.
    pub light: Box<[u8; PAD3]>,
    pub tints: ColumnTints,
    /// The smooth ground's fill about the cube and its materials (Amendment S): when given,
    /// natural ground is meshed smooth instead of as faces.
    pub ground: Option<(
        hearth_smooth::Field,
        std::sync::Arc<crate::smooth::GroundMaterials>,
    )>,
}

impl MeshInput {
    /// Gathers the neighbourhood from the world. Missing neighbours count as open air (sky 15).
    pub fn gather(map: &CubeMap, pos: CubePos, tints: ColumnTints) -> Self {
        let mut blocks = Box::new([BlockStateId::AIR; PAD3]);
        let mut light = Box::new([0xF0u8; PAD3]);
        let planet = *map.planet();
        for dy in -1..=1 {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let np = planet.cube_offset(pos, glam::IVec3::new(dx, dy, dz));
                    let Some(cube) = map.cube(np) else { continue };
                    // Range of local coordinates of this neighbour inside the padded window.
                    let range = |d: i32| -> std::ops::Range<i32> {
                        match d {
                            -1 => 15..16,
                            0 => 0..16,
                            _ => 0..1,
                        }
                    };
                    for ly in range(dy) {
                        for lz in range(dz) {
                            for lx in range(dx) {
                                let lp = LocalPos::new(lx as u8, ly as u8, lz as u8);
                                let (px, py, pz) = (lx + dx * 16, ly + dy * 16, lz + dz * 16);
                                let i = pidx(px, py, pz);
                                blocks[i] = cube.get(lp);
                                light[i] = (cube.sky_light(lp) << 4) | cube.block_light(lp);
                            }
                        }
                    }
                }
            }
        }
        Self {
            pos,
            blocks,
            light,
            tints,
            ground: None,
        }
    }

    /// With the smooth ground's fill about the cube, so its natural ground is meshed smooth.
    pub fn with_ground(
        mut self,
        map: &CubeMap,
        reg: &BlockRegistry,
        ground: &std::sync::Arc<crate::smooth::GroundMaterials>,
    ) -> Self {
        let field = crate::smooth::window(map, reg, ground, self.pos);
        self.ground = Some((field, ground.clone()));
        self
    }

    #[inline]
    fn at(&self, x: i32, y: i32, z: i32) -> BlockStateId {
        self.blocks[pidx(x, y, z)]
    }

    #[inline]
    fn light_at(&self, x: i32, y: i32, z: i32) -> u8 {
        self.light[pidx(x, y, z)]
    }
}

/// 16-byte greedy quad for full-cube faces. Layout (little-endian u32s):
/// * `a`: x:4 y:4 z:4 | w−1:4 h−1:4 | face:3 | flip:1 | fluid:1 | waving:1 | rot:2 | frames−1:4
/// * `b`: layer:12 | overlay:12 (4095 = none) | frame_time−1:4 | tint kind:4 (see `TINT_*`)
/// * `c`: per-corner light, 8 bits each (sky:4 | block:4), corner 0 in the low byte
/// * `d`: tint value (24: RGB or climate code) | ao per corner 2 bits each (8)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub struct PackedQuad {
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
}

/// 64-byte general quad (models, fluids, translucent surfaces). Per corner: position in 1/256
/// block units relative to the cube origin (x | y<<16, then z | light<<16 | ao<<24) and texel
/// UV×256 (u | v<<16).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub struct GeneralQuad {
    pub corners: [[u32; 3]; 4],
    /// layer:12 | frames−1:4 | frame_time−1:4 | shade dir:3 (7 = none) | waving:1 | fluid:1
    pub layer: u32,
    /// tint value (24: RGB or climate code) | tint kind (4) | spare (4)
    pub tint: u32,
    /// Overlay layer (4095 = none).
    pub overlay: u32,
    pub _pad: u32,
}

/// Render groups inside one cube mesh.
pub const GROUP_DIRS: usize = 6;

/// Output of meshing one cube.
#[derive(Debug, Clone, Default)]
pub struct CubeMesh {
    pub pos: CubePos,
    /// Packed quads, ordered by (layer: opaque, cutout) then face direction.
    pub quads: Vec<PackedQuad>,
    /// Quad count per [layer][direction].
    pub quad_counts: [[u32; GROUP_DIRS]; 2],
    /// General quads, opaque then cutout.
    pub models: Vec<GeneralQuad>,
    pub model_counts: [u32; 2],
    /// Translucent general quads (water, ice, glass-like).
    pub translucent: Vec<GeneralQuad>,
    /// Face connectivity: bit `a * 6 + b` set when faces a and b are connected through
    /// non-opaque blocks.
    pub visibility: u64,
    /// The smooth natural ground (Amendment S).
    pub smooth: crate::smooth::SmoothMesh,
}

impl CubeMesh {
    pub fn is_empty(&self) -> bool {
        self.quads.is_empty()
            && self.models.is_empty()
            && self.translucent.is_empty()
            && self.smooth.is_empty()
    }

    /// Bytes of GPU memory the mesh needs.
    pub fn gpu_bytes(&self) -> usize {
        self.quads.len() * 16
            + (self.models.len() + self.translucent.len()) * 64
            + self.smooth.gpu_bytes()
    }
}

#[inline]
fn ao_value(s1: bool, s2: bool, c: bool) -> u8 {
    if s1 && s2 {
        0
    } else {
        3 - (s1 as u8 + s2 as u8 + c as u8)
    }
}

/// Face axes: (w axis, h axis) as offsets, and the corner offsets in (w, h) units per the
/// shared corner convention (see the shader): corners (0,h),(w,h),(w,0),(0,0) in UV.
fn face_axes(d: Direction) -> (glam::IVec3, glam::IVec3) {
    use glam::IVec3 as V;
    match d {
        Direction::Down | Direction::Up => (V::X, V::Z),
        Direction::North | Direction::South => (V::X, V::Y),
        Direction::West | Direction::East => (V::Z, V::Y),
    }
}

/// Corner positions of a unit face of block (x, y, z), in the same order the shader uses.
fn face_corner_offsets(d: Direction) -> [glam::IVec3; 4] {
    use glam::IVec3 as V;
    match d {
        Direction::Down => [
            V::new(0, 0, 0),
            V::new(1, 0, 0),
            V::new(1, 0, 1),
            V::new(0, 0, 1),
        ],
        Direction::Up => [
            V::new(0, 1, 1),
            V::new(1, 1, 1),
            V::new(1, 1, 0),
            V::new(0, 1, 0),
        ],
        Direction::North => [
            V::new(1, 0, 0),
            V::new(0, 0, 0),
            V::new(0, 1, 0),
            V::new(1, 1, 0),
        ],
        Direction::South => [
            V::new(0, 0, 1),
            V::new(1, 0, 1),
            V::new(1, 1, 1),
            V::new(0, 1, 1),
        ],
        Direction::West => [
            V::new(0, 0, 0),
            V::new(0, 0, 1),
            V::new(0, 1, 1),
            V::new(0, 1, 0),
        ],
        Direction::East => [
            V::new(1, 0, 1),
            V::new(1, 0, 0),
            V::new(1, 1, 0),
            V::new(1, 1, 1),
        ],
    }
}

/// Meshing options.
#[derive(Debug, Clone, Copy)]
pub struct MeshOptions {
    pub smooth_lighting: bool,
    /// Render faces between two leaf blocks (fancy leaves).
    pub fancy_leaves: bool,
}

impl Default for MeshOptions {
    fn default() -> Self {
        Self {
            smooth_lighting: true,
            fancy_leaves: true,
        }
    }
}

/// The mesher (stateless apart from scratch buffers).
pub struct Mesher<'a> {
    pub reg: &'a BlockRegistry,
    pub models: &'a BlockModels,
    pub opts: MeshOptions,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct FaceKey {
    layer_bits: u32,
    b: u32,
    light: u32,
    tint_ao: u32,
    uniform: bool,
}

impl Mesher<'_> {
    #[inline]
    fn opaque(&self, s: BlockStateId) -> bool {
        self.reg.has(s, StateFlags::OPAQUE)
    }

    /// Whether a block that is not a full cube still stops all light (a roof, a wall of a
    /// building): no light is kept inside it, so its own faces take theirs from about it.
    fn shuts_light(&self, s: BlockStateId) -> bool {
        !self.opaque(s) && self.reg.light_opacity(s) >= 15
    }

    /// The light a block's own faces are lit by: its own, or for a block that stops all light,
    /// the brightest about it.
    fn own_light(&self, inp: &MeshInput, x: i32, y: i32, z: i32) -> u8 {
        if !self.shuts_light(inp.at(x, y, z)) {
            return inp.light_at(x, y, z);
        }
        let mut best = (0u8, 0u8);
        for d in Direction::ALL {
            let o = d.offset();
            let l = inp.light_at(x + o.x, y + o.y, z + o.z);
            best = (best.0.max(l >> 4), best.1.max(l & 15));
        }
        (best.0 << 4) | best.1
    }

    /// True if the face of `s` toward `d` must be drawn given neighbour `n`.
    #[inline]
    fn face_visible(&self, s: BlockStateId, n: BlockStateId, d: Direction) -> bool {
        if self.models.occludes(n, d.opposite()) {
            return false;
        }
        if s == n {
            // Glass, ice and water hide faces between equal blocks; leaves only in fast mode.
            let leaves = matches!(self.models.get(s), StateModel::Cube(c) if c.waving);
            if !leaves || !self.opts.fancy_leaves {
                return false;
            }
        }
        true
    }

    /// Light and AO at the four corners of the face of block (x, y, z) toward `d`.
    fn corner_light(
        &self,
        inp: &MeshInput,
        x: i32,
        y: i32,
        z: i32,
        d: Direction,
    ) -> ([u8; 4], [u8; 4]) {
        let n = d.offset();
        let (px, py, pz) = (x + n.x, y + n.y, z + n.z);
        let center = inp.light_at(px, py, pz);
        if !self.opts.smooth_lighting {
            return ([center; 4], [3; 4]);
        }
        let (ta, tb) = face_axes(d);
        let offsets = face_corner_offsets(d);
        let mut lights = [0u8; 4];
        let mut aos = [3u8; 4];
        for (c, off) in offsets.iter().enumerate() {
            // Tangent signs toward this corner.
            let sa = if (off.dot(ta)) > 0 { 1 } else { -1 };
            let sb = if (off.dot(tb)) > 0 { 1 } else { -1 };
            let a = ta * sa;
            let b = tb * sb;
            let s1p = (px + a.x, py + a.y, pz + a.z);
            let s2p = (px + b.x, py + b.y, pz + b.z);
            let cp = (px + a.x + b.x, py + a.y + b.y, pz + a.z + b.z);
            let s1 = self.opaque(inp.at(s1p.0, s1p.1, s1p.2));
            let s2 = self.opaque(inp.at(s2p.0, s2p.1, s2p.2));
            let cc = self.opaque(inp.at(cp.0, cp.1, cp.2));
            aos[c] = ao_value(s1, s2, cc);
            // Smooth light: average of the non-opaque samples among the four.
            let mut sky = (center >> 4) as u32;
            let mut blk = (center & 15) as u32;
            let mut count = 1;
            let mut add = |p: (i32, i32, i32), opaque: bool| {
                if !opaque {
                    let l = inp.light_at(p.0, p.1, p.2);
                    sky += (l >> 4) as u32;
                    blk += (l & 15) as u32;
                    count += 1;
                }
            };
            add(s1p, s1);
            add(s2p, s2);
            if !(s1 && s2) {
                add(cp, cc);
            }
            let sky = ((sky + count / 2) / count).min(15) as u8;
            let blk = ((blk + count / 2) / count).min(15) as u8;
            lights[c] = (sky << 4) | blk;
        }
        (lights, aos)
    }

    /// Whether a state is a layer of snow.
    fn snow_layer(&self, s: BlockStateId) -> bool {
        !s.is_air() && self.reg.block_of(s).name.path() == "snow"
    }

    /// Meshes one cube.
    pub fn mesh(&self, inp: &MeshInput) -> CubeMesh {
        let mut out = CubeMesh {
            pos: inp.pos,
            ..CubeMesh::default()
        };
        // Per-layer, per-direction quad lists.
        let mut groups: [[Vec<PackedQuad>; 6]; 2] = Default::default();
        let mut models: [Vec<GeneralQuad>; 2] = Default::default();
        let mut translucent: Vec<GeneralQuad> = Vec::new();
        let mut mask: [Option<FaceKey>; 256] = [None; 256];
        let mut all_air = true;
        for d in Direction::ALL {
            let (ta, tb) = face_axes(d);
            let n = d.offset();
            // Slices along the face normal.
            for slice in 0..CUBE_SIZE {
                mask.fill(None);
                let mut any = false;
                for hb in 0..16 {
                    for wa in 0..16 {
                        let p = normal_axis(d) * slice + ta * wa + tb * hb;
                        let (x, y, z) = (p.x, p.y, p.z);
                        let s = inp.at(x, y, z);
                        if s.is_air() {
                            continue;
                        }
                        all_air = false;
                        // Natural ground is the smooth mesh's.
                        if inp.ground.is_some() && self.reg.has(s, StateFlags::NATURAL) {
                            continue;
                        }
                        let StateModel::Cube(cube) = self.models.get(s) else {
                            continue;
                        };
                        let nb = inp.at(x + n.x, y + n.y, z + n.z);
                        if !self.face_visible(s, nb, d) {
                            continue;
                        }
                        // Between two blocks of the same foliage, a face is seen only through
                        // the leaf beside it: where another leaf lies beyond that one, it is
                        // behind two layers of foliage and is not drawn (the first layer in,
                        // seen through the gaps at a crown's surface, is).
                        if s == nb && cube.waving {
                            let (fx, fy, fz) = (x + 2 * n.x, y + 2 * n.y, z + 2 * n.z);
                            let inside = |v: i32| (-1..=16).contains(&v);
                            if inside(fx) && inside(fy) && inside(fz) && inp.at(fx, fy, fz) == s {
                                continue;
                            }
                        }
                        let ft = cube.faces[d.index()];
                        let (lights, aos) = self.corner_light(inp, x, y, z, d);
                        let (tint_kind, tint) = inp.tints.get(ft.tint, x as usize, z as usize);
                        let key = FaceKey {
                            layer_bits: layer_bits(&ft, cube.waving, false),
                            b: b_bits(&ft) | (tint_kind << 28),
                            light: u32::from_le_bytes(lights),
                            tint_ao: tint_ao(tint, aos),
                            uniform: lights.iter().all(|l| *l == lights[0])
                                && aos.iter().all(|a| *a == aos[0]),
                        };
                        mask[(hb * 16 + wa) as usize] = Some(key);
                        any = true;
                        let _ = cube;
                    }
                }
                if !any {
                    continue;
                }
                // Greedy merge of uniform faces.
                for hb in 0..16usize {
                    let mut wa = 0usize;
                    while wa < 16 {
                        let Some(key) = mask[hb * 16 + wa] else {
                            wa += 1;
                            continue;
                        };
                        let mut w = 1;
                        let mut h = 1;
                        if key.uniform {
                            while wa + w < 16 && mask[hb * 16 + wa + w] == Some(key) {
                                w += 1;
                            }
                            'grow: while hb + h < 16 {
                                for k in 0..w {
                                    if mask[(hb + h) * 16 + wa + k] != Some(key) {
                                        break 'grow;
                                    }
                                }
                                h += 1;
                            }
                        }
                        for dh in 0..h {
                            for dw in 0..w {
                                mask[(hb + dh) * 16 + wa + dw] = None;
                            }
                        }
                        let origin = normal_axis(d) * slice + ta * wa as i32 + tb * hb as i32;
                        let s = inp.at(origin.x, origin.y, origin.z);
                        let layer = match self.models.get(s) {
                            StateModel::Cube(c) => c.layer,
                            _ => RenderLayer::Opaque,
                        };
                        let li = if layer == RenderLayer::Opaque { 0 } else { 1 };
                        let aos = unpack_ao(key.tint_ao);
                        let flip =
                            (aos[0] as u32 + aos[2] as u32) < (aos[1] as u32 + aos[3] as u32);
                        let a = (origin.x as u32)
                            | ((origin.y as u32) << 4)
                            | ((origin.z as u32) << 8)
                            | (((w - 1) as u32) << 12)
                            | (((h - 1) as u32) << 16)
                            | ((d.index() as u32) << 20)
                            | ((flip as u32) << 23)
                            | key.layer_bits;
                        let quad = PackedQuad {
                            a,
                            b: key.b,
                            c: key.light,
                            d: key.tint_ao,
                        };
                        if layer == RenderLayer::Translucent {
                            // Translucent cubes (ice, glass-like) go through the sorted path.
                            translucent.push(packed_to_general(&quad, d));
                        } else {
                            groups[li][d.index()].push(quad);
                        }
                        wa += w;
                    }
                }
            }
        }
        // Models and fluids.
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let s = inp.at(x, y, z);
                    // Snow lying on the smooth ground is drawn on it (its cover), not as a
                    // layer of blocks (until S7 makes it fill).
                    if inp.ground.is_some()
                        && self.snow_layer(s)
                        && self.reg.has(inp.at(x, y - 1, z), StateFlags::NATURAL)
                    {
                        all_air = false;
                        continue;
                    }
                    match self.models.get(s) {
                        StateModel::Quads(quads, layer) => {
                            all_air = false;
                            let li = if *layer == RenderLayer::Opaque { 0 } else { 1 };
                            let limb = self.models.is_limb(s);
                            let px = self.models.limb_thickness(s);
                            for q in quads.iter() {
                                if let Some(c) = q.cull {
                                    let o = c.offset();
                                    let nb = inp.at(x + o.x, y + o.y, z + o.z);
                                    if self.models.occludes(nb, c.opposite()) {
                                        continue;
                                    }
                                }
                                // A limb's faces toward foliage are inside the crown, and its end
                                // against a limb at least as thick joined to it inside that limb.
                                if limb && let Some(d) = q.dir {
                                    let o = d.offset();
                                    let nb = inp.at(x + o.x, y + o.y, z + o.z);
                                    if self.models.is_foliage(nb)
                                        || (q.cull == Some(d)
                                            && self.models.covers_limb_end(nb, d.opposite(), px))
                                    {
                                        continue;
                                    }
                                }
                                let g = self.general(inp, x, y, z, q);
                                if *layer == RenderLayer::Translucent {
                                    translucent.push(g);
                                } else {
                                    models[li].push(g);
                                }
                            }
                            // A construction piece: the members of the pieces beside it that
                            // reach it carried on to meet it.
                            if let Some(own) = self.models.member(s) {
                                self.joints(inp, x, y, z, &own.boxes, &mut models);
                            }
                        }
                        StateModel::Fluid { still, flow } => {
                            all_air = false;
                            self.fluid(inp, x, y, z, s, still, flow, &mut translucent);
                        }
                        _ => {}
                    }
                    // Waterlogged non-fluid blocks also show water.
                    if !matches!(self.models.get(s), StateModel::Fluid { .. })
                        && self.reg.has(s, StateFlags::WATER)
                        && let Some(water) = self.water_model()
                    {
                        let (still, flow) = water;
                        self.fluid(
                            inp,
                            x,
                            y,
                            z,
                            self.reg.water_source(),
                            &still,
                            &flow,
                            &mut translucent,
                        );
                    }
                }
            }
        }
        // Built pieces on the smooth ground (S §6): a buried skirt half a metre down below each
        // one standing on natural ground, so no gap shows where the surface dips under its
        // base (the ground covers the skirt where it reaches higher).
        if inp.ground.is_some() {
            let mut quads = Vec::new();
            for y in 0..CUBE_SIZE {
                for z in 0..CUBE_SIZE {
                    for x in 0..CUBE_SIZE {
                        let s = inp.at(x, y, z);
                        if s.is_air() || self.reg.has(s, StateFlags::NATURAL) {
                            continue;
                        }
                        let StateModel::Cube(cube) = self.models.get(s) else {
                            continue;
                        };
                        if cube.waving || !self.reg.has(inp.at(x, y - 1, z), StateFlags::NATURAL) {
                            continue;
                        }
                        let li = if cube.layer == RenderLayer::Opaque {
                            0
                        } else {
                            1
                        };
                        quads.clear();
                        crate::models::box_quads(
                            Vec3::new(0.0, -0.5, 0.0),
                            Vec3::new(1.0, 0.0, 1.0),
                            |d| cube.faces[d.index()],
                            true,
                            &mut quads,
                        );
                        for q in &quads {
                            // The sides only, and not against another piece beside it.
                            let Some(d) = q.dir.filter(|d| d.axis() != hearth_math::Axis::Y) else {
                                continue;
                            };
                            let o = d.offset();
                            let beside = inp.at(x + o.x, y, z + o.z);
                            if self.opaque(beside) && !self.reg.has(beside, StateFlags::NATURAL) {
                                continue;
                            }
                            let q = crate::models::ModelQuad { cull: None, ..*q };
                            models[li].push(self.general(inp, x, y, z, &q));
                        }
                    }
                }
            }
        }
        for (li, dirs) in groups.iter_mut().enumerate() {
            for (di, g) in dirs.iter_mut().enumerate() {
                out.quad_counts[li][di] = g.len() as u32;
                out.quads.append(g);
            }
        }
        for (li, m) in models.iter_mut().enumerate() {
            out.model_counts[li] = m.len() as u32;
            out.models.append(m);
        }
        out.translucent = translucent;
        out.visibility = if all_air {
            u64::MAX >> 28
        } else {
            self.visibility(inp)
        };
        if let Some((field, ground)) = &inp.ground {
            let light = |p: glam::IVec3| {
                let s = inp.at(p.x, p.y, p.z);
                (
                    inp.light_at(p.x, p.y, p.z),
                    !self.reg.has(s, StateFlags::NATURAL),
                )
            };
            let climate = |x: usize, z: usize| inp.tints.climate[z * 16 + x];
            let snow = |p: glam::IVec3| self.snow_layer(inp.at(p.x, p.y, p.z));
            let about = crate::smooth::Surroundings {
                light: &light,
                climate: &climate,
                snow: &snow,
            };
            out.smooth = crate::smooth::mesh_cube(field, ground, &about);
        }
        out
    }

    /// Draws into block (x, y, z), whose piece has boxes `own`, the ends of its neighbours'
    /// members carried on to meet it (`crate::joints`), each as its own piece is drawn.
    fn joints(
        &self,
        inp: &MeshInput,
        x: i32,
        y: i32,
        z: i32,
        own: &[crate::joints::Box6],
        models: &mut [Vec<GeneralQuad>; 2],
    ) {
        let mut quads = Vec::new();
        for d in Direction::ALL {
            let o = d.offset();
            let Some(nb) = self.models.member(inp.at(x + o.x, y + o.y, z + o.z)) else {
                continue;
            };
            let li = if nb.layer == RenderLayer::Opaque {
                0
            } else {
                1
            };
            let carried = crate::joints::carried(own, &nb.boxes, d);
            // A roof carried on along its length (over a gable, to a post) goes on as its smooth
            // slab, where all of it goes the same way.
            if let Some((facing, t)) = nb.roof
                && !carried.is_empty()
                && matches!(
                    (facing.axis(), d.axis()),
                    (hearth_math::Axis::Z, hearth_math::Axis::X)
                        | (hearth_math::Axis::X, hearth_math::Axis::Z)
                )
            {
                let (n, far) = match d {
                    Direction::West => (0, false),
                    Direction::East => (0, true),
                    Direction::North => (2, false),
                    _ => (2, true),
                };
                let depth = |c: &crate::joints::Carried| c.b[n + 3] - c.b[n];
                let first = depth(&carried[0]);
                if carried.iter().all(|c| (depth(c) - first).abs() < 1e-4) {
                    let met = carried.iter().all(|c| c.met);
                    let (lo, hi) = if far {
                        (1.0 - first, 1.0)
                    } else {
                        (0.0, first)
                    };
                    // Along the slab's length in its own (rising north) frame.
                    let (x0, x1) = match facing {
                        Direction::South | Direction::West => (1.0 - hi, 1.0 - lo),
                        _ => (lo, hi),
                    };
                    for q in crate::models::roof_slab(t, x0, x1, facing, nb.tex) {
                        if q.dir == Some(d) || (met && q.dir == Some(d.opposite())) {
                            continue;
                        }
                        models[li].push(self.general(inp, x, y, z, &q));
                    }
                    continue;
                }
            }
            for c in carried {
                quads.clear();
                let [x0, y0, z0, x1, y1, z1] = c.b;
                crate::models::box_quads(
                    Vec3::new(x0, y0, z0),
                    Vec3::new(x1, y1, z1),
                    |_| nb.tex,
                    true,
                    &mut quads,
                );
                for q in &quads {
                    // Not the end against the neighbour's member, nor the end against this
                    // piece where it meets it.
                    if q.dir == Some(d) || (c.met && q.dir == Some(d.opposite())) {
                        continue;
                    }
                    if let Some(cd) = q.cull {
                        let co = cd.offset();
                        if self
                            .models
                            .occludes(inp.at(x + co.x, y + co.y, z + co.z), cd.opposite())
                        {
                            continue;
                        }
                    }
                    models[li].push(self.general(inp, x, y, z, q));
                }
            }
        }
    }

    fn water_model(&self) -> Option<(FaceTex, FaceTex)> {
        match self.models.get(self.reg.water_source()) {
            StateModel::Fluid { still, flow } => Some((*still, *flow)),
            _ => None,
        }
    }

    /// A general quad from a model quad at block (x, y, z).
    fn general(&self, inp: &MeshInput, x: i32, y: i32, z: i32, q: &ModelQuad) -> GeneralQuad {
        let base = Vec3::new(x as f32, y as f32, z as f32);
        let own = self.own_light(inp, x, y, z);
        let (tint_kind, tint) = inp.tints.get(q.tex.tint, x as usize, z as usize);
        let mut corners = [[0u32; 3]; 4];
        for (c, corner) in corners.iter_mut().enumerate() {
            let p = base + q.pos[c];
            let (light, ao) = if q.shade && self.opts.smooth_lighting {
                self.vertex_light(inp, p, own)
            } else {
                (own, 3)
            };
            *corner = pack_corner(p, q.uv[c], light, ao);
        }
        let dir = q.dir.map_or(7, |d| d.index() as u32);
        GeneralQuad {
            corners,
            layer: tex_bits(&q.tex) | (dir << 20) | ((q.waving as u32) << 23),
            tint: tint | (tint_kind << 24),
            overlay: if q.tex.overlay == NO_OVERLAY {
                4095
            } else {
                q.tex.overlay as u32
            },
            _pad: 0,
        }
    }

    /// Smooth light at an arbitrary vertex: average of the non-opaque blocks around it.
    fn vertex_light(&self, inp: &MeshInput, p: Vec3, fallback: u8) -> (u8, u8) {
        let bx = p.x.round() as i32;
        let by = p.y.round() as i32;
        let bz = p.z.round() as i32;
        let (mut sky, mut blk, mut count, mut solid) = (0u32, 0u32, 0u32, 0u32);
        for dy in -1..=0 {
            for dz in -1..=0 {
                for dx in -1..=0 {
                    let (x, y, z) = (
                        (bx + dx).clamp(-1, 16),
                        (by + dy).clamp(-1, 16),
                        (bz + dz).clamp(-1, 16),
                    );
                    let s = inp.at(x, y, z);
                    if self.opaque(s) {
                        solid += 1;
                        continue;
                    }
                    // A roof or a wall keeps no light inside it to give.
                    if self.shuts_light(s) {
                        continue;
                    }
                    let l = inp.light_at(x, y, z);
                    sky += (l >> 4) as u32;
                    blk += (l & 15) as u32;
                    count += 1;
                }
            }
        }
        if count == 0 {
            return (fallback, 0);
        }
        let l = ((((sky + count / 2) / count) as u8) << 4) | ((blk + count / 2) / count) as u8;
        let ao = 3u8.saturating_sub((solid / 2) as u8);
        (l, ao)
    }

    #[allow(clippy::too_many_arguments)]
    fn fluid(
        &self,
        inp: &MeshInput,
        x: i32,
        y: i32,
        z: i32,
        s: BlockStateId,
        still: &FaceTex,
        flow: &FaceTex,
        out: &mut Vec<GeneralQuad>,
    ) {
        let is_water = |st: BlockStateId| self.reg.has(st, StateFlags::WATER);
        let above = inp.at(x, y + 1, z);
        let height = |xx: i32, zz: i32| -> Option<f32> {
            let st = inp.at(xx, y, zz);
            if !is_water(st) {
                return None;
            }
            if is_water(inp.at(xx, y + 1, zz)) {
                return Some(1.0);
            }
            let amount = self.reg.fluid_amount(st).max(1) as f32;
            Some(amount / 9.0)
        };
        // Corner heights: average of the up-to-four water columns sharing the corner.
        let corner_h = |cx: i32, cz: i32| -> f32 {
            let mut sum = 0.0;
            let mut n = 0.0;
            for (dx, dz) in [(-1, -1), (0, -1), (-1, 0), (0, 0)] {
                let (xx, zz) = (x + cx + dx, z + cz + dz);
                if is_water(inp.at(xx, y + 1, zz)) {
                    return 1.0;
                }
                if let Some(hh) = height(xx, zz) {
                    sum += hh;
                    n += 1.0;
                }
            }
            if n > 0.0 { sum / n } else { 8.0 / 9.0 }
        };
        let full = is_water(above);
        let (h00, h10, h11, h01) = if full {
            (1.0, 1.0, 1.0, 1.0)
        } else {
            (
                corner_h(0, 0),
                corner_h(1, 0),
                corner_h(1, 1),
                corner_h(0, 1),
            )
        };
        let own = inp.light_at(x, y, z);
        let top_light = if full {
            own
        } else {
            inp.light_at(x, y + 1, z).max(own)
        };
        let (tint_kind, tint) = inp.tints.get(Tint::Water, x as usize, z as usize);
        let base = Vec3::new(x as f32, y as f32, z as f32);
        let flat = (h00 - h10).abs() < 1e-3 && (h00 - h11).abs() < 1e-3 && (h00 - h01).abs() < 1e-3;
        let push = |out: &mut Vec<GeneralQuad>,
                    pts: [Vec3; 4],
                    uv: [Vec2; 4],
                    tex: &FaceTex,
                    light: u8,
                    dir: u32| {
            let mut corners = [[0u32; 3]; 4];
            for c in 0..4 {
                corners[c] = pack_corner(base + pts[c], uv[c], light, 3);
            }
            out.push(GeneralQuad {
                corners,
                layer: tex_bits(tex) | (dir << 20) | (1 << 24),
                tint: tint | (tint_kind << 24),
                overlay: 4095,
                _pad: 0,
            });
        };
        let full_uv = [
            Vec2::new(0.0, 16.0),
            Vec2::new(16.0, 16.0),
            Vec2::new(16.0, 0.0),
            Vec2::new(0.0, 0.0),
        ];
        // Top surface.
        if !full || !is_water(above) {
            let above_occludes = self.models.occludes(above, Direction::Down);
            if !full && !above_occludes {
                let tex = if flat { still } else { flow };
                let top = [
                    Vec3::new(0.0, h01, 1.0),
                    Vec3::new(1.0, h11, 1.0),
                    Vec3::new(1.0, h10, 0.0),
                    Vec3::new(0.0, h00, 0.0),
                ];
                push(
                    out,
                    top,
                    full_uv,
                    tex,
                    top_light,
                    Direction::Up.index() as u32,
                );
            }
        }
        let _ = s;
        // Sides and bottom.
        for d in [
            Direction::North,
            Direction::South,
            Direction::West,
            Direction::East,
            Direction::Down,
        ] {
            let o = d.offset();
            let nb = inp.at(x + o.x, y + o.y, z + o.z);
            if is_water(nb) || self.models.occludes(nb, d.opposite()) {
                continue;
            }
            let nlight = inp.light_at(x + o.x, y + o.y, z + o.z).max(own);
            let (pts, uv) = match d {
                Direction::Down => {
                    crate::models::face_corners(Vec3::ZERO, Vec3::new(1.0, 0.0, 1.0), d)
                }
                _ => {
                    let (mut p, uv) = crate::models::face_corners(Vec3::ZERO, Vec3::ONE, d);
                    // Lower the top edge to the corner heights.
                    for v in &mut p {
                        if v.y > 0.5 {
                            v.y = match (v.x > 0.5, v.z > 0.5) {
                                (false, false) => h00,
                                (true, false) => h10,
                                (true, true) => h11,
                                (false, true) => h01,
                            };
                        }
                    }
                    (p, uv)
                }
            };
            push(out, pts, uv, still, nlight, d.index() as u32);
        }
    }

    /// Which faces of the cube are connected through non-opaque blocks (flood fill).
    fn visibility(&self, inp: &MeshInput) -> u64 {
        let mut seen = [false; 4096];
        let mut stack: Vec<u16> = Vec::with_capacity(512);
        let mut bits = 0u64;
        for start in 0..4096usize {
            if seen[start] {
                continue;
            }
            let lp = LocalPos::from_index(start);
            if self.opaque(inp.at(lp.x as i32, lp.y as i32, lp.z as i32)) {
                seen[start] = true;
                continue;
            }
            let mut faces = 0u8;
            seen[start] = true;
            stack.push(start as u16);
            while let Some(i) = stack.pop() {
                let p = LocalPos::from_index(i as usize);
                let (x, y, z) = (p.x as i32, p.y as i32, p.z as i32);
                if y == 0 {
                    faces |= 1 << Direction::Down.index();
                }
                if y == 15 {
                    faces |= 1 << Direction::Up.index();
                }
                if z == 0 {
                    faces |= 1 << Direction::North.index();
                }
                if z == 15 {
                    faces |= 1 << Direction::South.index();
                }
                if x == 0 {
                    faces |= 1 << Direction::West.index();
                }
                if x == 15 {
                    faces |= 1 << Direction::East.index();
                }
                for d in Direction::ALL {
                    let o = d.offset();
                    let (nx, ny, nz) = (x + o.x, y + o.y, z + o.z);
                    if !(0..16).contains(&nx) || !(0..16).contains(&ny) || !(0..16).contains(&nz) {
                        continue;
                    }
                    let ni = LocalPos::new(nx as u8, ny as u8, nz as u8).index();
                    if seen[ni] {
                        continue;
                    }
                    seen[ni] = true;
                    if !self.opaque(inp.at(nx, ny, nz)) {
                        stack.push(ni as u16);
                    }
                }
            }
            for a in 0..6 {
                if faces & (1 << a) == 0 {
                    continue;
                }
                for b in 0..6 {
                    if faces & (1 << b) != 0 {
                        bits |= 1 << (a * 6 + b);
                    }
                }
            }
        }
        bits
    }
}

#[inline]
fn normal_axis(d: Direction) -> glam::IVec3 {
    match d.axis() {
        hearth_math::Axis::X => glam::IVec3::X,
        hearth_math::Axis::Y => glam::IVec3::Y,
        hearth_math::Axis::Z => glam::IVec3::Z,
    }
}

/// Texture bits shared by both quad formats: layer:12 | frames−1:4 | frame_time−1:4.
#[inline]
fn tex_bits(t: &FaceTex) -> u32 {
    (t.tex.layer as u32 & 0xfff)
        | (((t.tex.frames.max(1) - 1) as u32 & 15) << 12)
        | (((t.tex.frame_time.max(1) - 1) as u32 & 15) << 16)
}

/// High bits of the packed quad's first word: fluid:1 waving:1 rot:2 frames−1:4.
#[inline]
fn layer_bits(t: &FaceTex, waving: bool, fluid: bool) -> u32 {
    ((fluid as u32) << 24)
        | ((waving as u32) << 25)
        | ((t.rot as u32 & 3) << 26)
        | (((t.tex.frames.max(1) - 1) as u32 & 15) << 28)
}

#[inline]
fn b_bits(t: &FaceTex) -> u32 {
    let overlay = if t.overlay == NO_OVERLAY {
        4095
    } else {
        t.overlay as u32 & 0xfff
    };
    (t.tex.layer as u32 & 0xfff)
        | (overlay << 12)
        | (((t.tex.frame_time.max(1) - 1) as u32 & 15) << 24)
}

#[inline]
fn tint_ao(tint: u32, aos: [u8; 4]) -> u32 {
    (tint & 0x00ff_ffff)
        | ((aos[0] as u32) << 24)
        | ((aos[1] as u32) << 26)
        | ((aos[2] as u32) << 28)
        | ((aos[3] as u32) << 30)
}

#[inline]
fn unpack_ao(v: u32) -> [u8; 4] {
    [
        ((v >> 24) & 3) as u8,
        ((v >> 26) & 3) as u8,
        ((v >> 28) & 3) as u8,
        ((v >> 30) & 3) as u8,
    ]
}

#[inline]
fn pack_corner(p: Vec3, uv: Vec2, light: u8, ao: u8) -> [u32; 3] {
    let q = |v: f32| ((v * 256.0).round() as i32).clamp(-32768, 32767) as i16 as u16 as u32;
    let t = |v: f32| ((v * 256.0).round() as i32).clamp(0, 65535) as u32;
    [
        q(p.x) | (q(p.y) << 16),
        q(p.z) | ((light as u32) << 16) | ((ao as u32 & 3) << 24),
        t(uv.x) | (t(uv.y) << 16),
    ]
}

/// The face of a full cube toward `d` as the packed path draws it alone: its corners in the
/// shader's order and its texels as the shader maps them, turned by the face's rotation (a block's
/// own shape, for the aim and its highlight, T1.3).
pub fn cube_face(d: Direction, tex: FaceTex, waving: bool) -> ModelQuad {
    let uv = [
        Vec2::new(0.0, 16.0),
        Vec2::new(16.0, 16.0),
        Vec2::new(16.0, 0.0),
        Vec2::new(0.0, 0.0),
    ];
    let turn = |t: Vec2| match tex.rot & 3 {
        1 => Vec2::new(t.y, -t.x),
        2 => -t,
        3 => Vec2::new(-t.y, t.x),
        _ => t,
    };
    ModelQuad {
        pos: face_corner_offsets(d).map(|o| o.as_vec3()),
        uv: uv.map(turn),
        tex,
        dir: Some(d),
        cull: Some(d),
        shade: true,
        waving,
    }
}

/// Converts a packed cube face (1×1..16×16) to a general quad for the translucent path.
fn packed_to_general(q: &PackedQuad, d: Direction) -> GeneralQuad {
    let x = (q.a & 15) as i32;
    let y = ((q.a >> 4) & 15) as i32;
    let z = ((q.a >> 8) & 15) as i32;
    let w = ((q.a >> 12) & 15) as i32 + 1;
    let h = ((q.a >> 16) & 15) as i32 + 1;
    let (ta, tb) = face_axes(d);
    let offs = face_corner_offsets(d);
    let lights = q.c.to_le_bytes();
    let aos = unpack_ao(q.d);
    let mut corners = [[0u32; 3]; 4];
    let uvs = [
        Vec2::new(0.0, h as f32 * 16.0),
        Vec2::new(w as f32 * 16.0, h as f32 * 16.0),
        Vec2::new(w as f32 * 16.0, 0.0),
        Vec2::new(0.0, 0.0),
    ];
    for c in 0..4 {
        let o = offs[c];
        // Stretch the unit corner along the face axes by (w, h).
        let sa = if o.dot(ta) > 0 { w } else { 0 };
        let sb = if o.dot(tb) > 0 { h } else { 0 };
        let n = o - ta * o.dot(ta) - tb * o.dot(tb);
        let p = glam::IVec3::new(x, y, z) + n + ta * sa + tb * sb;
        corners[c] = pack_corner(p.as_vec3(), uvs[c], lights[c], aos[c]);
    }
    let layer = q.b & 0xfff;
    let frames = (q.a >> 28) & 15;
    let frame_time = (q.b >> 24) & 15;
    GeneralQuad {
        corners,
        layer: layer | (frames << 12) | (frame_time << 16) | ((d.index() as u32) << 20),
        tint: (q.d & 0x00ff_ffff) | ((q.b >> 28) << 24),
        overlay: (q.b >> 12) & 0xfff,
        _pad: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas::TextureArray;
    use hearth_math::{Planet, PlanetSize};
    use hearth_world::{Cube, LightEngine};
    use std::sync::Arc;

    fn setup() -> (BlockRegistry, BlockModels) {
        let reg = hearth_world::datapack::load_test_registry().unwrap();
        let atlas = TextureArray::from_entries(&hearth_texgen::textures_for(Some(
            &hearth_content::Content::load_base(),
        )));
        let models = BlockModels::build(&reg, &atlas);
        (reg, models)
    }

    fn world_with(reg: &BlockRegistry, fill: impl Fn(i32, i32, i32) -> BlockStateId) -> CubeMap {
        let mut map = CubeMap::new(Planet::from_size(PlanetSize::Tiny).unwrap());
        let mut cubes = Vec::new();
        for cy in -1..=1 {
            for cz in -1..=1 {
                for cx in -1..=1 {
                    let mut c = Cube::filled(BlockStateId::AIR);
                    for i in 0..4096 {
                        let lp = LocalPos::from_index(i);
                        let s = fill(
                            cx * 16 + lp.x as i32,
                            cy * 16 + lp.y as i32,
                            cz * 16 + lp.z as i32,
                        );
                        c.set(lp, s);
                    }
                    let p = CubePos::new(cx, cy, cz);
                    map.insert_cube(p, Arc::new(c), reg);
                    cubes.push(p);
                }
            }
        }
        LightEngine::new().light_new_cubes(&mut map, reg, &cubes);
        map
    }

    #[test]
    fn flat_ground_merges_into_one_quad_per_face_run() {
        let (reg, models) = setup();
        let stone = reg.default_state("stone");
        let map = world_with(
            &reg,
            |_, y, _| if y < 8 { stone } else { BlockStateId::AIR },
        );
        let mesher = Mesher {
            reg: &reg,
            models: &models,
            opts: MeshOptions::default(),
        };
        let mesh = mesher.mesh(&MeshInput::gather(
            &map,
            CubePos::new(0, 0, 0),
            ColumnTints::default(),
        ));
        let up = mesh.quad_counts[0][Direction::Up.index()];
        assert_eq!(up, 1, "a sunlit flat top merges into one 16×16 quad");
        assert_eq!(
            mesh.quad_counts[0][Direction::Down.index()],
            0,
            "buried faces culled"
        );
        let total: u32 = mesh.quad_counts.iter().flatten().sum();
        assert_eq!(total, 1);
        // Everything but the air above connects; the solid half blocks nothing on the top side.
        assert!(
            mesh.visibility & (1 << (Direction::Up.index() * 6 + Direction::North.index())) != 0
        );
    }

    #[test]
    fn deep_foliage_is_not_drawn_but_the_first_layer_in_is() {
        let (reg, models) = setup();
        let leaves = reg.default_state("oak_leaves");
        // A 6×6×6 block of foliage: its outer faces, and inside the faces seen through one leaf
        // only (each slab's faces toward a neighbour on the surface).
        let map = world_with(&reg, |x, y, z| {
            if (4..10).contains(&x) && (4..10).contains(&y) && (4..10).contains(&z) {
                leaves
            } else {
                BlockStateId::AIR
            }
        });
        let mesher = Mesher {
            reg: &reg,
            models: &models,
            opts: MeshOptions::default(),
        };
        let mesh = mesher.mesh(&MeshInput::gather(
            &map,
            CubePos::new(0, 0, 0),
            ColumnTints::default(),
        ));
        // Per direction, faces are drawn on the 6×6 blocks of the slab at the surface and those
        // just behind it: at most 2 slabs of 36 faces (fewer quads where they merge), where all 6
        // slabs were drawn before.
        let faces: u32 = mesh.quad_counts.iter().flatten().sum();
        let east: u32 = mesh
            .quad_counts
            .iter()
            .map(|layer| layer[Direction::East.index()])
            .sum();
        assert!(east > 0, "the crown's east face");
        assert!(
            faces <= 6 * 2 * 36,
            "{faces} quads for a 6×6×6 block of foliage"
        );
    }

    #[test]
    fn a_limb_in_the_crown_hides_its_faces_toward_the_foliage() {
        let (reg, models) = setup();
        let limb = reg
            .parse_state("hearth:oak_branch[thickness=4,east=true,west=true]")
            .unwrap();
        let leaves = reg.default_state("oak_leaves");
        // A limb along x through y = z = 6; with `crown`, foliage above, below and to both sides.
        let count = |crown: bool| {
            let map = world_with(&reg, |x, y, z| {
                if !(2..12).contains(&x) {
                    BlockStateId::AIR
                } else if (y, z) == (6, 6) {
                    limb
                } else if crown && (5..8).contains(&y) && (5..8).contains(&z) {
                    leaves
                } else {
                    BlockStateId::AIR
                }
            });
            let mesher = Mesher {
                reg: &reg,
                models: &models,
                opts: MeshOptions::default(),
            };
            let mesh = mesher.mesh(&MeshInput::gather(
                &map,
                CubePos::new(0, 0, 0),
                ColumnTints::default(),
            ));
            mesh.model_counts.iter().sum::<u32>()
        };
        let open = count(false);
        let covered = count(true);
        assert!(open >= 10 * 4, "{open} quads for a bare limb of ten blocks");
        // Its four sides in each of the ten blocks face foliage.
        assert!(
            covered + 10 * 4 <= open,
            "{covered} quads for the limb in the crown, {open} bare"
        );
    }

    #[test]
    fn a_limb_ends_inside_a_limb_at_least_as_thick() {
        let (reg, models) = setup();
        let state = |px: u32| {
            reg.parse_state(&format!(
                "hearth:oak_branch[thickness={px},east=true,west=true]"
            ))
            .unwrap()
        };
        // Along x through y = z = 6, from x = 2: `a` blocks of one thickness, then `b` of another.
        let count = |(a, pa): (i32, u32), (b, pb): (i32, u32)| {
            let (sa, sb) = (state(pa), state(pb));
            let map = world_with(&reg, |x, y, z| {
                if (y, z) != (6, 6) {
                    BlockStateId::AIR
                } else if (2..2 + a).contains(&x) {
                    sa
                } else if (2 + a..2 + a + b).contains(&x) {
                    sb
                } else {
                    BlockStateId::AIR
                }
            });
            let mesher = Mesher {
                reg: &reg,
                models: &models,
                opts: MeshOptions::default(),
            };
            let mesh = mesher.mesh(&MeshInput::gather(
                &map,
                CubePos::new(0, 0, 0),
                ColumnTints::default(),
            ));
            mesh.model_counts.iter().sum::<u32>()
        };
        // Ten blocks of one limb: four sides each and the two far ends.
        assert_eq!(count((10, 4), (0, 4)), 10 * 4 + 2);
        // A thick limb thinning: the thin one's end lies inside the thick, the thick one's shows
        // about it.
        assert_eq!(count((5, 8), (5, 4)), 10 * 4 + 2 + 1);
    }

    #[test]
    fn single_block_has_six_faces_with_ao_neighbours() {
        let (reg, models) = setup();
        let stone = reg.default_state("stone");
        let map = world_with(&reg, |x, y, z| {
            if (x, y, z) == (5, 5, 5) {
                stone
            } else {
                BlockStateId::AIR
            }
        });
        let mesher = Mesher {
            reg: &reg,
            models: &models,
            opts: MeshOptions::default(),
        };
        let mesh = mesher.mesh(&MeshInput::gather(
            &map,
            CubePos::new(0, 0, 0),
            ColumnTints::default(),
        ));
        let total: u32 = mesh.quad_counts.iter().flatten().sum();
        assert_eq!(total, 6);
        for q in &mesh.quads {
            assert_eq!(unpack_ao(q.d), [3, 3, 3, 3]);
            assert_eq!(q.c, 0xF0F0_F0F0, "sky 15 at every corner");
        }
    }

    #[test]
    fn plants_water_and_translucency() {
        let (reg, models) = setup();
        let grass = reg.default_state("grass_block");
        let plant = reg.default_state("short_grass");
        let water = reg.water_source();
        let map = world_with(&reg, |x, y, _| match y {
            y if y < 4 => grass,
            4 if x < 8 => plant,
            4 => water,
            _ => BlockStateId::AIR,
        });
        let mesher = Mesher {
            reg: &reg,
            models: &models,
            opts: MeshOptions::default(),
        };
        let mesh = mesher.mesh(&MeshInput::gather(
            &map,
            CubePos::new(0, 0, 0),
            ColumnTints::default(),
        ));
        assert!(
            mesh.model_counts[1] >= 8 * 16 * 2,
            "two cross quads per plant"
        );
        assert!(!mesh.translucent.is_empty(), "water surface");
        // The grass top has a tint and the side an overlay.
        let side = mesh
            .quads
            .iter()
            .find(|q| (q.a >> 20) & 7 == Direction::East.index() as u32);
        if let Some(q) = side {
            assert_ne!((q.b >> 12) & 0xfff, 4095, "grass side overlay");
        }
    }

    #[test]
    fn caves_split_visibility() {
        let (reg, models) = setup();
        let stone = reg.default_state("stone");
        // Solid cube with a tunnel along X only.
        let map = world_with(&reg, |_, y, z| {
            if (6..9).contains(&y) && (6..9).contains(&z) {
                BlockStateId::AIR
            } else {
                stone
            }
        });
        let mesher = Mesher {
            reg: &reg,
            models: &models,
            opts: MeshOptions::default(),
        };
        let mesh = mesher.mesh(&MeshInput::gather(
            &map,
            CubePos::new(0, 0, 0),
            ColumnTints::default(),
        ));
        let w = Direction::West.index();
        let e = Direction::East.index();
        let up = Direction::Up.index();
        assert!(
            mesh.visibility & (1 << (w * 6 + e)) != 0,
            "tunnel connects west and east"
        );
        assert_eq!(mesh.visibility & (1 << (w * 6 + up)), 0, "but not the top");
    }
}

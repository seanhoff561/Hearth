//! Distant terrain (v1 §8): what lies beyond the full-detail cubes, out to the LOD distance.
//!
//! * **Tiles.** A quadtree over the wrapped planet: a tile of level `L` holds 32×32 LOD columns
//!   of `2^L` blocks (level-7 tiles are 4,096 blocks, and every planet circumference is a
//!   multiple of that). Tiles are refined toward the camera so a column stays a few pixels wide
//!   on screen at any distance.
//! * **Columns** are sampled straight from the world generator's surface model at the tile's
//!   resolution, never by generating cubes: the ground and its top block (from the soil and the
//!   rock) or water, and the crown of the trees over it. On the finer levels (columns of up to
//!   8 blocks, out to about a kilometre) the tile's real trees are grown by the generator's own
//!   tree code into a map of the canopy, so every tree of the cubes stands in the distance too:
//!   crowns float over the ground where leaves cover a quarter of a column or more, and trunks
//!   stand at their own blocks. Coarser levels, whose columns are wider than a crown, raise a
//!   crown where the place's trees are expected to close over the ground and darken the ground
//!   under sparser ones.
//! * **Meshes** are flat-topped columns with the sides that show and skirts along the tile
//!   edges, which hide cracks against neighbours of another level; crowns are boxes with the
//!   faces no neighbour crown hides, and the ground under a crown is lit as shade. Every vertex carries the
//!   colour of its block's texture, the block's tint kind and the column's climate code, so the
//!   shader colours grass and leaves by season exactly as it does the full-detail terrain.

use bytemuck::{Pod, Zeroable};
use hearth_math::Planet;
use hearth_math::hash::hash_2d;
use hearth_texgen::TexEntry;
use hearth_world::{BlockRegistry, BlockStateId, TintKind};
use hearth_worldgen::cubegen::features::TreeSink;
use hearth_worldgen::region::biome::{Biome, water_color};
use hearth_worldgen::{ColumnSample, WorldGenerator};
use rustc_hash::{FxHashMap, FxHashSet};

/// Columns along a tile side.
pub const TILE: i32 = 32;
/// Coarsest level: tiles of 4,096 blocks.
pub const MAX_LEVEL: u8 = 7;
/// A tile is split while the camera is within this many tile sizes of it.
pub const SPLIT: f64 = 4.0;
/// Width of the band where the full-detail terrain hands over to the LOD (blocks; as in the
/// terrain shader).
pub const HANDOFF_BAND: f64 = 8.0;
/// Earth's mean radius (m); times the vertical scale, the radius of the planet's curvature.
pub const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// Distance to the horizon from a camera `camera_y` blocks above sea level (blocks): the LOD
/// reaches at least this far, so the land never stops short of it.
pub fn horizon_distance(camera_y: f64, vertical_scale: f64) -> f64 {
    (2.0 * EARTH_RADIUS_M * vertical_scale * camera_y.max(1.0)).sqrt()
}

/// How far to draw LOD terrain: the LOD distance setting (chunks), or the horizon when that is
/// farther (plus the height of hills beyond it).
pub fn draw_distance(lod_chunks: u32, camera_y: f64, vertical_scale: f64) -> f64 {
    if lod_chunks == 0 {
        return 0.0;
    }
    (lod_chunks as f64 * 16.0).max(horizon_distance(camera_y, vertical_scale) * 1.15)
}

/// A tile of the quadtree: level, and position in tiles of its size (X around the planet).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileKey {
    pub level: u8,
    pub x: i32,
    pub z: i32,
}

impl TileKey {
    /// Side in blocks.
    pub fn size(self) -> i32 {
        TILE << self.level
    }

    /// Side of one LOD column in blocks.
    pub fn column(self) -> i32 {
        1 << self.level
    }

    /// World block of the tile's minimum corner (X canonical).
    pub fn min_block(self) -> (i32, i32) {
        (self.x * self.size(), self.z * self.size())
    }

    /// A compact id for maps outside this crate.
    pub fn id(self) -> u64 {
        ((self.level as u64) << 60)
            | (((self.x as u32 as u64) & 0x3fff_ffff) << 30)
            | ((self.z as u32 as u64) & 0x3fff_ffff)
    }

    fn children(self) -> [TileKey; 4] {
        let (l, x, z) = (self.level - 1, self.x * 2, self.z * 2);
        [
            TileKey { level: l, x, z },
            TileKey {
                level: l,
                x: x + 1,
                z,
            },
            TileKey {
                level: l,
                x,
                z: z + 1,
            },
            TileKey {
                level: l,
                x: x + 1,
                z: z + 1,
            },
        ]
    }
}

/// Tiles around the planet at a level.
fn tiles_around(planet: &Planet, level: u8) -> i32 {
    (planet.circumference() / (TILE << level)).max(1)
}

/// The tiles to draw for a camera at (x, z): a quadtree refined toward the camera, out to
/// `distance` blocks (capped at half the circumference, so no tile is drawn twice), leaving out
/// tiles wholly inside the full-detail area `near` (min x, min z, max x, max z in world blocks).
pub fn select(
    planet: &Planet,
    cam_x: f64,
    cam_z: f64,
    distance: f64,
    near: Option<[f64; 4]>,
) -> Vec<TileKey> {
    let c = planet.circumference() as f64;
    let distance = distance.min(c * 0.5);
    let root = (TILE << MAX_LEVEL) as f64;
    let around = tiles_around(planet, MAX_LEVEL);
    let (zmin, zmax) = (
        (-c * 0.5 / root).floor() as i32,
        (c * 0.5 / root).ceil() as i32 - 1,
    );
    // The full-detail area relative to the camera, less the handoff band.
    let near_rel = near.map(|[x0, z0, x1, z1]| {
        let dx0 = planet.delta_x(cam_x, x0);
        [
            dx0 + HANDOFF_BAND,
            z0 - cam_z + HANDOFF_BAND,
            dx0 + (x1 - x0) - HANDOFF_BAND,
            z1 - cam_z - HANDOFF_BAND,
        ]
    });
    let mut out = Vec::new();
    let mut seen = FxHashSet::default();
    let mut stack = Vec::new();
    let (z0, z1) = (
        ((cam_z - distance) / root).floor() as i32,
        ((cam_z + distance) / root).floor() as i32,
    );
    let (x0, x1) = (
        ((cam_x - distance) / root).floor() as i32,
        ((cam_x + distance) / root).floor() as i32,
    );
    for tz in z0.max(zmin)..=z1.min(zmax) {
        for tx in x0..=x1 {
            let key = TileKey {
                level: MAX_LEVEL,
                x: tx.rem_euclid(around),
                z: tz,
            };
            if seen.insert(key) {
                stack.push(key);
            }
        }
    }
    while let Some(key) = stack.pop() {
        let size = key.size() as f64;
        let (mx, mz) = key.min_block();
        // The tile relative to the camera (X by the shortest way around).
        let rx0 = planet.delta_x(cam_x, mx as f64 + size * 0.5) - size * 0.5;
        let rz0 = mz as f64 - cam_z;
        let dx = (rx0.max(0.0)).max(-(rx0 + size)).max(0.0);
        let dz = (rz0.max(0.0)).max(-(rz0 + size)).max(0.0);
        let d = (dx * dx + dz * dz).sqrt();
        if d > distance {
            continue;
        }
        if let Some([nx0, nz0, nx1, nz1]) = near_rel
            && rx0 >= nx0
            && rx0 + size <= nx1
            && rz0 >= nz0
            && rz0 + size <= nz1
        {
            continue;
        }
        if key.level > 0 && d < SPLIT * size {
            stack.extend(key.children());
        } else {
            out.push(key);
        }
    }
    out.sort_by_key(|k| (std::cmp::Reverse(k.level), k.z, k.x));
    out
}

/// A vertex of a tile mesh (16 bytes): position in blocks relative to the tile's minimum corner
/// (X and Z packed as u16, Y absolute), colour word (sRGB texture colour, face, tint kind, water
/// flag) and climate code.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub struct LodVertex {
    pub xz: u32,
    pub y: i32,
    pub color: u32,
    pub climate: u32,
}

/// The mesh of one tile.
#[derive(Debug, Clone)]
pub struct TileMesh {
    pub key: TileKey,
    /// World block of the minimum corner (X canonical).
    pub origin: [i32; 2],
    /// Four vertices per quad.
    pub vertices: Vec<LodVertex>,
    pub min_y: i32,
    pub max_y: i32,
}

impl TileMesh {
    pub fn quads(&self) -> u32 {
        (self.vertices.len() / 4) as u32
    }
}

/// Tint kinds understood by the shader (as `hearth_render::mesh`).
const TINT_RGB: u8 = 0;
const TINT_GRASS: u8 = 1;
const TINT_DECIDUOUS: u8 = 2;
const TINT_EVERGREEN: u8 = 3;
const VARIANT_BIRCH: u8 = 1 << 2;

/// Colour (sRGB, packed) and tint kind of every block state's top face: the average of its
/// texture, which the shader tints like the full-detail face.
pub struct BlockColors {
    per_state: Vec<(u32, u8)>,
    /// Colour of each state's side (a log's bark).
    sides: Vec<u32>,
}

fn pack(c: [u8; 3]) -> u32 {
    c[0] as u32 | (c[1] as u32) << 8 | (c[2] as u32) << 16
}

fn to_linear(v: u8) -> f32 {
    (v as f32 / 255.0).powf(2.2)
}

fn to_srgb(v: f32) -> u8 {
    (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0).round() as u8
}

/// Average colour of the first frame of a texture (opaque texels, averaged in linear light).
fn average(t: &hearth_texgen::Tex) -> [u8; 3] {
    let side = t.w.min(t.h) as usize;
    let mut sum = [0.0f32; 3];
    let mut n = 0.0;
    for y in 0..side {
        for x in 0..side {
            let p = t.px[y * t.w as usize + x];
            if p[3] >= 128 {
                for k in 0..3 {
                    sum[k] += to_linear(p[k]);
                }
                n += 1.0;
            }
        }
    }
    if n == 0.0 {
        return [0, 0, 0];
    }
    [
        to_srgb(sum[0] / n),
        to_srgb(sum[1] / n),
        to_srgb(sum[2] / n),
    ]
}

impl BlockColors {
    pub fn new(reg: &BlockRegistry, textures: &[TexEntry]) -> Self {
        let by_name: rustc_hash::FxHashMap<&str, &hearth_texgen::Tex> =
            textures.iter().map(|e| (e.name.as_str(), &e.tex)).collect();
        let mut per_state = vec![(0u32, TINT_RGB); reg.state_count()];
        let mut sides = vec![0u32; reg.state_count()];
        for block in reg.blocks() {
            let path = block.name.path();
            let candidates = [
                format!("block/{path}_top"),
                format!("block/{path}"),
                format!("block/{}", path.replace("_block", "")),
            ];
            let color = candidates
                .iter()
                .find_map(|n| by_name.get(n.as_str()))
                .map_or(block.map_color, |t| average(t));
            let side = [format!("block/{path}"), format!("block/{path}_side")]
                .iter()
                .find_map(|n| by_name.get(n.as_str()))
                .map_or(color, |t| average(t));
            let first = block.first_state.0 as usize;
            sides[first..first + block.state_count as usize].fill(pack(side));
            let kind = match block.def.tint {
                TintKind::Grass | TintKind::DryGrass => TINT_GRASS,
                TintKind::Foliage => TINT_DECIDUOUS,
                TintKind::Birch => TINT_DECIDUOUS | VARIANT_BIRCH,
                TintKind::Spruce => TINT_EVERGREEN,
                TintKind::None | TintKind::Water => TINT_RGB,
            };
            for entry in &mut per_state[first..first + block.state_count as usize] {
                *entry = (pack(color), kind);
            }
        }
        Self { per_state, sides }
    }

    /// The side colour of a state (sRGB, packed): a log's bark.
    pub fn side(&self, s: BlockStateId) -> u32 {
        self.sides.get(s.0 as usize).copied().unwrap_or(0x808080)
    }

    pub fn get(&self, s: BlockStateId) -> (u32, u8) {
        self.per_state
            .get(s.0 as usize)
            .copied()
            .unwrap_or((0x808080, TINT_RGB))
    }
}

/// A tree crown over an LOD column: the leaves' lowest and top (first above) blocks, colour and
/// tint kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Crown {
    bottom: i32,
    top: i32,
    rgb: u32,
    kind: u8,
}

/// One LOD column: the ground or water surface, and the crown of the trees over it.
#[derive(Debug, Clone, Copy)]
struct Col {
    /// First block above the ground or water.
    top: i32,
    rgb: u32,
    kind: u8,
    water: bool,
    climate: u32,
    crown: Option<Crown>,
}

/// A trunk drawn at its own block (fine levels): column (x, z) in blocks relative to the tile,
/// from `y0` up to `y1`, in the bark's colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Trunk {
    x: i32,
    z: i32,
    y0: i32,
    y1: i32,
    rgb: u32,
}

/// Levels up to this one grow the tile's real trees (the ones the cubes will have); coarser
/// levels, whose columns are wider than a crown, estimate the canopy from the tree density.
pub const EXACT_TREES_MAX_LEVEL: u8 = 3;
/// Levels up to this one draw each trunk at its own block.
const TRUNK_MAX_LEVEL: u8 = 2;
/// A column shows a crown when its share of leaf cover beats a threshold drawn for the column
/// between these: on average crowns then cover as much ground as the leaves do, whatever the
/// column size (a lone tree does not grow to fill a whole column).
const CROWN_COVER_MIN: f32 = 0.1;
const CROWN_COVER_MAX: f32 = 0.9;
/// Coarse levels: a crown where the trees are expected to cover at least this share of the
/// ground; sparser trees darken the ground instead.
const FAR_CROWN_COVER: f32 = 0.5;

/// What a block state is to the canopy map.
const OTHER: u8 = 0;
const LEAVES: u8 = 1;
/// A vertical log (a trunk).
const TRUNK: u8 = 2;

/// The canopy of a tile at block resolution, filled by growing the tile's trees with the world
/// generator's own code.
struct CanopyMap<'a> {
    x0: i32,
    z0: i32,
    w: i32,
    d: i32,
    /// First block above the highest leaf (`i32::MIN`: no leaves), lowest leaf, and the leaves
    /// at the top, per block column.
    top: Vec<i32>,
    bottom: Vec<i32>,
    leaf: Vec<BlockStateId>,
    /// Trunks by block column: lowest and first-above log, and the log.
    trunks: FxHashMap<(i32, i32), (i32, i32, BlockStateId)>,
    class: &'a [u8],
}

impl<'a> CanopyMap<'a> {
    fn new(x0: i32, z0: i32, w: i32, d: i32, class: &'a [u8]) -> Self {
        let n = (w * d) as usize;
        Self {
            x0,
            z0,
            w,
            d,
            top: vec![i32::MIN; n],
            bottom: vec![i32::MAX; n],
            leaf: vec![BlockStateId::AIR; n],
            trunks: FxHashMap::default(),
            class,
        }
    }

    /// The crown over a `cs`-block square column at (x, z) (its minimum corner), if leaves
    /// cover enough of it; the ground's top keeps it above.
    fn crown(&self, x: i32, z: i32, cs: i32, ground: i32, colors: &BlockColors) -> Option<Crown> {
        let (mut covered, mut top, mut bottom, mut leaf) = (0, i32::MIN, i32::MAX, None);
        for dz in 0..cs {
            for dx in 0..cs {
                let (lx, lz) = (x + dx - self.x0, z + dz - self.z0);
                if lx < 0 || lz < 0 || lx >= self.w || lz >= self.d {
                    continue;
                }
                let i = (lz * self.w + lx) as usize;
                if self.top[i] == i32::MIN || self.top[i] <= ground {
                    continue;
                }
                covered += 1;
                if self.top[i] > top {
                    top = self.top[i];
                    leaf = Some(self.leaf[i]);
                }
                bottom = bottom.min(self.bottom[i]);
            }
        }
        let draw = (hash_2d(0xc40e ^ cs as u64, x, z) & 0xffff) as f32 / 65535.0;
        let need = CROWN_COVER_MIN + (CROWN_COVER_MAX - CROWN_COVER_MIN) * draw;
        if covered == 0 || (covered as f32) < need * (cs * cs) as f32 {
            return None;
        }
        let (rgb, kind) = colors.get(leaf?);
        Some(Crown {
            bottom: bottom.max(ground),
            top,
            rgb,
            kind,
        })
    }
}

impl TreeSink for CanopyMap<'_> {
    fn bounds(&self) -> ([i32; 3], [i32; 3]) {
        (
            [self.x0, -4096, self.z0],
            [self.x0 + self.w - 1, 8192, self.z0 + self.d - 1],
        )
    }

    fn get(&self, _x: i32, _y: i32, _z: i32) -> Option<BlockStateId> {
        None
    }

    fn put(&mut self, x: i32, y: i32, z: i32, s: BlockStateId) {
        let (lx, lz) = (x - self.x0, z - self.z0);
        if lx < 0 || lz < 0 || lx >= self.w || lz >= self.d {
            return;
        }
        match self.class.get(s.0 as usize).copied().unwrap_or(OTHER) {
            LEAVES => {
                let i = (lz * self.w + lx) as usize;
                if y + 1 > self.top[i] {
                    self.top[i] = y + 1;
                    self.leaf[i] = s;
                }
                self.bottom[i] = self.bottom[i].min(y);
            }
            TRUNK => {
                let e = self.trunks.entry((x, z)).or_insert((y, y + 1, s));
                e.0 = e.0.min(y);
                e.1 = e.1.max(y + 1);
            }
            _ => {}
        }
    }
}

/// Builds tile meshes from a world generator.
pub struct LodGen {
    colors: BlockColors,
    /// `OTHER`, `LEAVES` or `TRUNK` per block state.
    class: Vec<u8>,
    /// Leaves of the usual trees: oak, birch, spruce, mangrove.
    leaves: [BlockStateId; 4],
    /// Average colour of the water texture (linear).
    water_tex: [f32; 3],
}

impl LodGen {
    pub fn new(reg: &BlockRegistry, textures: &[TexEntry]) -> Self {
        let leaf = |w: &str| {
            reg.parse_state(&format!(
                "{w}_leaves[distance=1,persistent=false,waterlogged=false]"
            ))
            .unwrap_or(BlockStateId::AIR)
        };
        let water_tex = textures
            .iter()
            .find(|e| e.name == "block/water_still")
            .map_or([0.6; 3], |e| average(&e.tex).map(to_linear));
        let mut class = vec![OTHER; reg.state_count()];
        for block in reg.blocks() {
            let path = block.name.path();
            let first = block.first_state.0 as usize;
            for (k, c) in class[first..first + block.state_count as usize]
                .iter_mut()
                .enumerate()
            {
                let s = BlockStateId((first + k) as u16);
                *c = if path.ends_with("_leaves") {
                    LEAVES
                } else if path.ends_with("_log") && reg.get(s, "axis").is_none_or(|a| a == "y") {
                    TRUNK
                } else {
                    OTHER
                };
            }
        }
        Self {
            colors: BlockColors::new(reg, textures),
            class,
            leaves: [leaf("oak"), leaf("birch"), leaf("spruce"), leaf("mangrove")],
            water_tex,
        }
    }

    /// Samples and meshes one tile.
    pub fn build(&self, wg: &WorldGenerator, key: TileKey) -> TileMesh {
        let planet = wg.planet();
        let cs = key.column();
        let (mx, mz) = key.min_block();
        let half = key.size() / 2;
        let normals = hearth_env::climate::Normals::sample(
            &wg.terrain.grid,
            (mx + half) as f64,
            (mz + half) as f64,
        );
        let southern = planet.latitude((mz + half) as f64) < 0.0;
        // Fine levels: the tile's real trees, grown into a map of the canopy (over the tile and
        // its ring of neighbour columns).
        let canopy = (key.level <= EXACT_TREES_MAX_LEVEL).then(|| {
            let size = key.size();
            let mut map =
                CanopyMap::new(mx - cs, mz - cs, size + 2 * cs, size + 2 * cs, &self.class);
            wg.features().grow_trees(&mut map, wg);
            map
        });
        // Columns with a ring of neighbours around the tile.
        let n = (TILE + 2) as usize;
        let mut cols = Vec::with_capacity(n * n);
        for j in -1..=TILE {
            for i in -1..=TILE {
                let (bx, bz) = (mx + i * cs, mz + j * cs);
                let x = planet.wrap_x(bx + cs / 2);
                let z = bz + cs / 2;
                let s = wg.terrain.sample(x, z);
                let mut col = self.column(wg, &s, x, z, &normals, southern);
                col.crown = match &canopy {
                    Some(map) => map.crown(bx, bz, cs, col.top, &self.colors),
                    None => self.expected_crown(&s, &mut col, x, z),
                };
                cols.push(col);
            }
        }
        // Trunks at their own blocks, up to their crown.
        let mut trunks = Vec::new();
        if let Some(map) = canopy.as_ref().filter(|_| key.level <= TRUNK_MAX_LEVEL) {
            for (&(x, z), &(y0, y1, log)) in &map.trunks {
                let (rx, rz) = (x - mx, z - mz);
                if rx < 0 || rz < 0 || rx >= key.size() || rz >= key.size() {
                    continue;
                }
                let c = &cols[((rz / cs + 1) as usize) * n + (rx / cs + 1) as usize];
                // Up to the crown, and down to the column's ground wherever that lies lower.
                let y1 = c.crown.map_or(y1, |cr| y1.min(cr.bottom));
                let y0 = y0.min(c.top);
                if y1 > y0 {
                    trunks.push(Trunk {
                        x: rx,
                        z: rz,
                        y0,
                        y1,
                        rgb: self.colors.side(log),
                    });
                }
            }
            trunks.sort_by_key(|t| (t.z, t.x));
        }
        mesh(key, &cols, &trunks)
    }

    /// Coarse levels: a crown where the trees are expected to close over the ground, at the
    /// usual height of the place's trees; sparser trees darken the ground.
    fn expected_crown(&self, s: &ColumnSample, col: &mut Col, x: i32, z: i32) -> Option<Crown> {
        if s.tree_density <= 0.0 || (col.water && s.biome != Biome::Mangrove) {
            return None;
        }
        let (leaves, height, area) = canopy(s.biome);
        let cover = (s.tree_density * 0.95 * area / 25.0).min(1.0);
        if cover < FAR_CROWN_COVER {
            let dim = 1.0 - 0.35 * cover / FAR_CROWN_COVER;
            let c = col.rgb;
            let ch = |k: u32| ((((c >> k) & 255) as f32 * dim) as u32) << k;
            col.rgb = ch(0) | ch(8) | ch(16);
            return None;
        }
        let (rgb, kind) = self.colors.get(self.leaves[leaves]);
        let jitter = 0.85 + 0.3 * (hash_2d(0x7ee5, x, z) & 1023) as f32 / 1023.0;
        let h = (height as f32 * (0.7 + 0.3 * s.tree_density.min(1.0)) * jitter).round() as i32;
        Some(Crown {
            bottom: col.top,
            top: col.top + h.max(2),
            rgb,
            kind,
        })
    }

    fn column(
        &self,
        wg: &WorldGenerator,
        s: &ColumnSample,
        x: i32,
        z: i32,
        normals: &hearth_env::climate::Normals,
        southern: bool,
    ) -> Col {
        use hearth_worldgen::planet::climate::ClimateClass as C;
        let arid = matches!(
            s.climate,
            C::HotDesert | C::HotSteppe | C::ColdDesert | C::ColdSteppe
        );
        let dry = hearth_env::tint::DryType::of(normals, arid);
        let range = (2.0 * (s.t_warm - s.temperature)).max(0.0) as f64;
        let climate = hearth_env::tint::encode(
            s.temperature as f64,
            range,
            s.precipitation as f64,
            dry,
            southern,
        );
        if s.is_underwater() {
            // Water: its tint by warmth and depth over the texture, with the bed showing through
            // the shallows (dimmed by the water above it) as through the full-detail water.
            let depth = (s.water_i() - s.height_i()).max(0) as f32;
            let w = water_color(s.sea_temperature, depth);
            let rock = wg.geology.column(x, z);
            let bed_block = wg
                .soils
                .profile(
                    s,
                    rock.rock_at(s.height_i() - 1 - s.soil_depth as i32),
                    x,
                    z,
                )
                .first()
                .copied()
                .unwrap_or_else(|| rock.rock_at(s.height_i() - 1));
            let bed = self.colors.get(bed_block).0;
            let bed = [bed & 255, (bed >> 8) & 255, (bed >> 16) & 255].map(|c| to_linear(c as u8));
            let through = 0.25 * (-depth / 2.5).exp();
            let rgb = [0, 1, 2].map(|k| {
                to_srgb(
                    to_linear(w[k]) * self.water_tex[k] * (1.0 - through) + bed[k] * 0.5 * through,
                )
            });
            return Col {
                top: s.water_i(),
                rgb: pack(rgb),
                kind: TINT_RGB,
                water: true,
                climate,
                crown: None,
            };
        }
        let ground = s.height_i();
        let rock = wg.geology.column(x, z);
        let parent = rock.rock_at(ground - 1 - s.soil_depth as i32);
        let profile = wg.soils.profile(s, parent, x, z);
        let block = profile
            .first()
            .copied()
            .unwrap_or_else(|| rock.rock_at(ground - 1));
        let (rgb, kind) = self.colors.get(block);
        Col {
            top: ground,
            rgb,
            kind,
            water: false,
            climate,
            crown: None,
        }
    }
}

/// The usual trees of a biome for the coarse levels: leaves (oak, birch, spruce, mangrove),
/// crown top above the ground, and crown area (blocks²).
fn canopy(b: Biome) -> (usize, i32, f32) {
    match b {
        Biome::TropicalRainforest => (0, 18, 60.0),
        Biome::TemperateRainforest => (2, 16, 30.0),
        Biome::BorealForest | Biome::SnowyTaiga | Biome::MontaneForest => (2, 11, 22.0),
        Biome::Krummholz | Biome::AlpineMeadow | Biome::Tundra => (2, 3, 9.0),
        Biome::BirchForest => (1, 8, 15.0),
        Biome::Mangrove => (3, 8, 28.0),
        Biome::Savanna => (0, 7, 55.0),
        Biome::MediterraneanScrub => (0, 5, 16.0),
        _ => (0, 8, 22.0),
    }
}

/// Face codes as in the terrain shader.
const DOWN: u32 = 0;
const UP: u32 = 1;
const NORTH: u32 = 2;
const SOUTH: u32 = 3;
const WEST: u32 = 4;
const EAST: u32 = 5;

fn vertex(
    x: i32,
    y: i32,
    z: i32,
    rgb: u32,
    kind: u8,
    water: bool,
    climate: u32,
    face: u32,
) -> LodVertex {
    LodVertex {
        xz: (x as u32 & 0xffff) | ((z as u32 & 0xffff) << 16),
        y,
        color: rgb | (face << 24) | ((kind as u32 & 15) << 27) | (u32::from(water) << 31),
        climate,
    }
}

/// The four corners of a side face of the box `x0..x1 × z0..z1` from `lo` to `hi`, wound to
/// face outward.
fn side(face: u32, x0: i32, x1: i32, z0: i32, z1: i32, lo: i32, hi: i32) -> [(i32, i32, i32); 4] {
    match face {
        NORTH => [(x1, lo, z0), (x0, lo, z0), (x0, hi, z0), (x1, hi, z0)],
        SOUTH => [(x0, lo, z1), (x1, lo, z1), (x1, hi, z1), (x0, hi, z1)],
        WEST => [(x0, lo, z0), (x0, lo, z1), (x0, hi, z1), (x0, hi, z0)],
        _ => [(x1, lo, z1), (x1, lo, z0), (x1, hi, z0), (x1, hi, z1)],
    }
}

/// Meshes a tile's columns (with their ring of neighbours) and trunks: row-merged ground tops,
/// the sides that show and skirts along the tile edges; tree crowns as boxes floating over the
/// ground (tops, bottoms and the sides no neighbour crown hides); trunks as thin boxes.
fn mesh(key: TileKey, cols: &[Col], trunks: &[Trunk]) -> TileMesh {
    let cs = key.column();
    let n = TILE + 2;
    let at = |i: i32, j: i32| &cols[((j + 1) * n + (i + 1)) as usize];
    let mut v = Vec::new();
    let (mut min_y, mut max_y) = (i32::MAX, i32::MIN);
    let mut quad = |v: &mut Vec<LodVertex>,
                    p: [(i32, i32, i32); 4],
                    rgb: u32,
                    kind: u8,
                    water: bool,
                    climate: u32,
                    face: u32| {
        for (x, y, z) in p {
            min_y = min_y.min(y);
            max_y = max_y.max(y);
            v.push(vertex(x, y, z, rgb, kind, water, climate, face));
        }
    };
    // Ground under a crown is in its shade: lit as from below.
    let ground_face = |c: &Col| if c.crown.is_some() { DOWN } else { UP };
    let same = |a: &Col, b: &Col| {
        a.top == b.top
            && a.rgb == b.rgb
            && a.kind == b.kind
            && a.water == b.water
            && ground_face(a) == ground_face(b)
    };
    let same_crown = |a: &Col, b: &Col, top: bool| match (a.crown, b.crown) {
        (Some(x), Some(y)) => {
            x.rgb == y.rgb
                && x.kind == y.kind
                && if top {
                    x.top == y.top
                } else {
                    x.bottom == y.bottom && x.bottom > a.top && y.bottom > b.top
                }
        }
        _ => false,
    };
    for j in 0..TILE {
        let (z0, z1) = (j * cs, (j + 1) * cs);
        // Ground tops, merged along the row.
        let mut i = 0;
        while i < TILE {
            let c = at(i, j);
            let mut end = i;
            while end + 1 < TILE && same(at(end + 1, j), c) {
                end += 1;
            }
            let (x0, x1) = (i * cs, (end + 1) * cs);
            quad(
                &mut v,
                [
                    (x0, c.top, z1),
                    (x1, c.top, z1),
                    (x1, c.top, z0),
                    (x0, c.top, z0),
                ],
                c.rgb,
                c.kind,
                c.water,
                c.climate,
                ground_face(c),
            );
            i = end + 1;
        }
        // Crown tops and bottoms, merged along the row.
        for top in [true, false] {
            let mut i = 0;
            while i < TILE {
                let c = at(i, j);
                let Some(cr) = c.crown.filter(|cr| top || cr.bottom > c.top) else {
                    i += 1;
                    continue;
                };
                let mut end = i;
                while end + 1 < TILE && same_crown(at(end + 1, j), c, top) {
                    end += 1;
                }
                let (x0, x1) = (i * cs, (end + 1) * cs);
                let (face, p) = if top {
                    let y = cr.top;
                    (UP, [(x0, y, z1), (x1, y, z1), (x1, y, z0), (x0, y, z0)])
                } else {
                    let y = cr.bottom;
                    (DOWN, [(x0, y, z0), (x1, y, z0), (x1, y, z1), (x0, y, z1)])
                };
                quad(&mut v, p, cr.rgb, cr.kind, false, c.climate, face);
                i = end + 1;
            }
        }
        // Sides toward lower neighbours; along the tile edge, skirts reaching below both. Crown
        // sides where no neighbour crown hides them.
        for i in 0..TILE {
            let c = at(i, j);
            let (x0, x1) = (i * cs, (i + 1) * cs);
            for (di, dj, face) in [(0, -1, NORTH), (0, 1, SOUTH), (-1, 0, WEST), (1, 0, EAST)] {
                let (ni, nj) = (i + di, j + dj);
                let nb = at(ni, nj);
                let edge = !(0..TILE).contains(&ni) || !(0..TILE).contains(&nj);
                let low = if edge {
                    c.top.min(nb.top) - 2 * cs
                } else {
                    nb.top
                };
                if low < c.top {
                    let p = side(face, x0, x1, z0, z1, low, c.top);
                    quad(&mut v, p, c.rgb, c.kind, c.water, c.climate, face);
                }
                let Some(cr) = c.crown else {
                    continue;
                };
                // The crown's side from its bottom (or the neighbour's ground) to its top, less
                // what the neighbour's crown covers.
                let lo = cr.bottom.max(nb.top);
                let spans: [(i32, i32); 2] = match nb.crown {
                    Some(o) => [(lo, cr.top.min(o.bottom)), (lo.max(o.top), cr.top)],
                    None => [(lo, cr.top), (0, 0)],
                };
                for (a, b) in spans {
                    if b > a {
                        let p = side(face, x0, x1, z0, z1, a, b);
                        quad(&mut v, p, cr.rgb, cr.kind, false, c.climate, face);
                    }
                }
            }
        }
    }
    // Trunks: the four sides of a one-block box.
    for t in trunks {
        for face in [NORTH, SOUTH, WEST, EAST] {
            let p = side(face, t.x, t.x + 1, t.z, t.z + 1, t.y0, t.y1);
            quad(&mut v, p, t.rgb, TINT_RGB, false, 0, face);
        }
    }
    let (mx, mz) = key.min_block();
    TileMesh {
        key,
        origin: [mx, mz],
        vertices: v,
        min_y: if min_y == i32::MAX { 0 } else { min_y },
        max_y: if max_y == i32::MIN { 0 } else { max_y },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn planet() -> Planet {
        Planet::new(hearth_math::PlanetSize::Standard.circumference()).expect("standard planet")
    }

    #[test]
    fn selection_refines_toward_the_camera_and_covers_without_overlap() {
        let p = planet();
        let tiles = select(&p, 1000.0, 500.0, 8192.0, None);
        assert!(!tiles.is_empty());
        // Fine near, coarse far.
        let near = tiles
            .iter()
            .filter(|t| {
                let (x, z) = t.min_block();
                (x as f64 - 1000.0).abs() < 64.0 && (z as f64 - 500.0).abs() < 64.0
            })
            .map(|t| t.level)
            .max();
        assert_eq!(near, Some(0), "full resolution at the camera");
        assert!(tiles.iter().any(|t| t.level >= 5), "coarse tiles far away");
        // No two tiles overlap: every block is covered by at most one tile.
        for (a_i, a) in tiles.iter().enumerate() {
            for b in &tiles[a_i + 1..] {
                let (ax, az) = a.min_block();
                let (bx, bz) = b.min_block();
                let dx = p.delta_block_x(ax, bx);
                let overlap_x = dx < a.size() && -dx < b.size();
                let overlap_z = bz < az + a.size() && az < bz + b.size();
                assert!(!(overlap_x && overlap_z), "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn selection_skips_the_full_detail_area_and_wraps() {
        let p = planet();
        let near = [900.0, 400.0, 1100.0, 600.0];
        let tiles = select(&p, 1000.0, 500.0, 4096.0, Some(near));
        for t in &tiles {
            let (x, z) = t.min_block();
            let (x1, z1) = (x + t.size(), z + t.size());
            let inside = x as f64 >= near[0] + HANDOFF_BAND
                && (x1 as f64) <= near[2] - HANDOFF_BAND
                && z as f64 >= near[1] + HANDOFF_BAND
                && (z1 as f64) <= near[3] - HANDOFF_BAND;
            assert!(!inside, "{t:?} lies wholly inside the full-detail area");
        }
        // Near the seam, tiles on both sides of X = 0 are chosen, with canonical positions.
        let c = p.circumference();
        let seam = select(&p, 10.0, 0.0, 1000.0, None);
        assert!(seam.iter().any(|t| t.min_block().0 + t.size() > c - 600));
        assert!(
            seam.iter()
                .all(|t| t.min_block().0 >= 0 && t.min_block().0 < c)
        );
    }

    fn ground(top: i32) -> Col {
        Col {
            top,
            rgb: 0x406080,
            kind: TINT_RGB,
            water: false,
            climate: 0,
            crown: None,
        }
    }

    fn faces(m: &TileMesh, face: u32) -> usize {
        m.vertices
            .chunks(4)
            .filter(|q| (q[0].color >> 24) & 7 == face)
            .count()
    }

    #[test]
    fn meshes_have_tops_sides_and_skirts() {
        let key = TileKey {
            level: 1,
            x: 3,
            z: -2,
        };
        let n = (TILE + 2) as usize;
        let mut cols = vec![ground(10); n * n];
        // A single raised column in the middle of the tile.
        cols[(17 * n) + 17].top = 14;
        let m = mesh(key, &cols, &[]);
        // Flat rows merge into one top each (the raised column splits its row into three).
        assert_eq!(faces(&m, UP), TILE as usize + 2);
        // Four sides of the raised column, and skirts along all four tile edges.
        assert_eq!(m.vertices.len() / 4 - faces(&m, UP), 4 + 4 * TILE as usize);
        assert_eq!(m.max_y, 14);
        assert_eq!(m.min_y, 10 - 2 * key.column());
        assert!(
            m.vertices
                .iter()
                .all(|v| (v.xz & 0xffff) as i32 <= key.size() && (v.xz >> 16) as i32 <= key.size())
        );
    }

    fn tiny_world() -> (WorldGenerator, BlockRegistry, Vec<TexEntry>) {
        let settings = hearth_worldgen::WorldGenSettings {
            seed: 7,
            planet_size: hearth_math::PlanetSize::Tiny,
            grid_resolution: 256,
            ..Default::default()
        }
        .sanitized();
        let grid = std::sync::Arc::new(hearth_worldgen::PlanetGrid::build(&settings, &|_, _| {}));
        let terrain = std::sync::Arc::new(hearth_worldgen::Terrain::new(grid));
        let content = hearth_content::Content::load_base();
        let reg = hearth_world::datapack::load_builtin_registry().expect("registry");
        let wg = WorldGenerator::new(terrain, &reg, &content).expect("generator");
        let tex = hearth_texgen::textures_for(Some(&content));
        (wg, reg, tex)
    }

    /// A patch of flat, dense forest.
    fn forest(wg: &WorldGenerator) -> (i32, i32) {
        let c = wg.planet().circumference();
        for z in (-3000..3000).step_by(97) {
            for x in (0..c).step_by(89) {
                let s = wg.terrain.sample(x, z);
                if s.tree_density > 0.7 && !s.is_underwater() && s.slope < 0.2 {
                    return (x, z);
                }
            }
        }
        panic!("no forest");
    }

    #[test]
    fn distant_trees_are_the_trees_the_cubes_grow() {
        let (wg, reg, tex) = tiny_world();
        let lod = LodGen::new(&reg, &tex);
        let (x0, z0) = forest(&wg);
        let size = 48;
        let mut map = CanopyMap::new(x0, z0, size, size, &lod.class);
        wg.features().grow_trees(&mut map, &wg);
        // The highest leaves of each column in the generated cubes.
        let mut cubes: FxHashMap<hearth_math::CubePos, hearth_world::Cube> = FxHashMap::default();
        let (mut both, mut agree, mut only_one) = (0, 0, 0);
        for dz in 0..size {
            for dx in 0..size {
                let (x, z) = (x0 + dx, z0 + dz);
                let ground = wg.terrain.sample(x, z).height_i();
                let mut cube_top = i32::MIN;
                for y in (ground..ground + 40).rev() {
                    let p = hearth_math::BlockPos::new(x, y, z);
                    let cube = cubes
                        .entry(p.cube())
                        .or_insert_with(|| wg.generate_cube(p.cube()));
                    if lod.class[cube.get(p.local()).0 as usize] == LEAVES {
                        cube_top = y + 1;
                        break;
                    }
                }
                let map_top = map.top[(dz * size + dx) as usize];
                match (cube_top > i32::MIN, map_top > i32::MIN) {
                    (true, true) => {
                        both += 1;
                        agree += usize::from(cube_top == map_top);
                    }
                    (false, false) => {}
                    _ => only_one += 1,
                }
            }
        }
        assert!(both > 300, "a forest canopy: {both} columns");
        // Leaves the cubes lose to logs or the ground where trees meet are the only differences.
        assert!(
            agree as f64 >= 0.95 * both as f64 && only_one as f64 <= 0.03 * both as f64,
            "{agree} of {both} columns agree on the canopy top, {only_one} disagree on leaves"
        );
    }

    #[test]
    fn coarse_levels_close_the_canopy_over_forests_only() {
        let (wg, reg, tex) = tiny_world();
        let lod = LodGen::new(&reg, &tex);
        let (x, z) = forest(&wg);
        let key = |level: u8| TileKey {
            level,
            x: x.div_euclid(TILE << level),
            z: z.div_euclid(TILE << level),
        };
        for level in [2, 5] {
            let m = lod.build(&wg, key(level));
            let crowns = m
                .vertices
                .chunks(4)
                .filter(|q| (q[0].color >> 24) & 7 == UP && ((q[0].color >> 27) & 3) >= 2)
                .count();
            assert!(crowns > 0, "level {level}: crowns over the forest");
        }
    }

    #[test]
    fn crowns_float_over_shaded_ground_on_their_trunks() {
        let key = TileKey {
            level: 1,
            x: 0,
            z: 0,
        };
        let n = (TILE + 2) as usize;
        let mut cols = vec![ground(10); n * n];
        let crown = Crown {
            bottom: 14,
            top: 19,
            rgb: 0x205020,
            kind: TINT_DECIDUOUS,
        };
        // Two crowns side by side: their facing sides hide each other.
        cols[(6 * n) + 6].crown = Some(crown);
        cols[(6 * n) + 7].crown = Some(crown);
        let trunk = Trunk {
            x: 10,
            z: 10,
            y0: 10,
            y1: 14,
            rgb: 0x403020,
        };
        let m = mesh(key, &cols, &[trunk]);
        let crown_quads = |face: u32| {
            m.vertices
                .chunks(4)
                .filter(|q| (q[0].color & 0xff_ffff) == 0x205020 && (q[0].color >> 24) & 7 == face)
                .count()
        };
        // One merged top and one merged bottom; north, south and one side each way.
        assert_eq!(crown_quads(UP), 1);
        assert_eq!(crown_quads(DOWN), 1);
        assert_eq!(
            crown_quads(NORTH) + crown_quads(SOUTH) + crown_quads(WEST) + crown_quads(EAST),
            6
        );
        // The crown hangs above the ground: its bottom at 14, over ground at 10.
        assert!(m.vertices.iter().any(|v| v.y == 14) && m.max_y == 19);
        // The ground under the crowns is shaded (drawn as seen from below), in one piece.
        let shaded_ground = m
            .vertices
            .chunks(4)
            .filter(|q| (q[0].color & 0xff_ffff) == 0x406080 && (q[0].color >> 24) & 7 == DOWN)
            .count();
        assert_eq!(shaded_ground, 1);
        // The trunk: four sides of a one-block box.
        let bark = m
            .vertices
            .chunks(4)
            .filter(|q| (q[0].color & 0xff_ffff) == 0x403020)
            .count();
        assert_eq!(bark, 4);
    }
}

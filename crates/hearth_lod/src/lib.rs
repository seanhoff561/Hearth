//! Distant terrain (v1 §8): what lies beyond the full-detail cubes, out to the LOD distance.
//!
//! * **Tiles.** A quadtree over the wrapped planet: a tile of level `L` holds 32×32 LOD columns
//!   of `2^L` blocks (level-7 tiles are 4,096 blocks, and every planet circumference is a
//!   multiple of that). Tiles are refined toward the camera so a column stays a few pixels wide
//!   on screen at any distance.
//! * **Columns** are sampled straight from the world generator's surface model at the tile's
//!   resolution, never by generating cubes: the ground and its top block (from the soil and the
//!   rock), water, and forest canopy as a raised roof in the colour of the leaves over a shaded
//!   forest floor (what shows under the roof's edge where the full-detail trees end).
//! * **Meshes** are flat-topped columns with the sides that show and skirts along the tile
//!   edges, which hide cracks against neighbours of another level. Every vertex carries the
//!   colour of its block's texture, the block's tint kind and the column's climate code, so the
//!   shader colours grass and leaves by season exactly as it does the full-detail terrain.

use bytemuck::{Pod, Zeroable};
use hearth_math::Planet;
use hearth_texgen::TexEntry;
use hearth_world::{BlockRegistry, BlockStateId, TintKind};
use hearth_worldgen::region::biome::{Biome, water_color};
use hearth_worldgen::{ColumnSample, WorldGenerator};
use rustc_hash::FxHashSet;

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
            let kind = match block.def.tint {
                TintKind::Grass | TintKind::DryGrass => TINT_GRASS,
                TintKind::Foliage => TINT_DECIDUOUS,
                TintKind::Birch => TINT_DECIDUOUS | VARIANT_BIRCH,
                TintKind::Spruce => TINT_EVERGREEN,
                TintKind::None | TintKind::Water => TINT_RGB,
            };
            let first = block.first_state.0 as usize;
            for entry in &mut per_state[first..first + block.state_count as usize] {
                *entry = (pack(color), kind);
            }
        }
        Self { per_state }
    }

    pub fn get(&self, s: BlockStateId) -> (u32, u8) {
        self.per_state
            .get(s.0 as usize)
            .copied()
            .unwrap_or((0x808080, TINT_RGB))
    }
}

/// One LOD column.
#[derive(Debug, Clone, Copy)]
struct Col {
    /// First block above the surface (ground, canopy or water).
    top: i32,
    rgb: u32,
    kind: u8,
    water: bool,
    climate: u32,
    /// Under a canopy: the forest floor's height, colour and tint kind.
    floor: Option<(i32, u32, u8)>,
}

/// Builds tile meshes from a world generator.
pub struct LodGen {
    colors: BlockColors,
    /// Leaves of the forest canopy: oak, birch, spruce, mangrove.
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
        Self {
            colors: BlockColors::new(reg, textures),
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
        // Columns with a ring of neighbours around the tile.
        let n = (TILE + 2) as usize;
        let mut cols = Vec::with_capacity(n * n);
        for j in -1..=TILE {
            for i in -1..=TILE {
                let x = planet.wrap_x(mx + i * cs + cs / 2);
                let z = mz + j * cs + cs / 2;
                let s = wg.terrain.sample(x, z);
                cols.push(self.column(wg, &s, x, z, &normals, southern));
            }
        }
        mesh(key, &cols)
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
                floor: None,
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
        // Forest: a canopy of the region's usual trees over the forest floor.
        if s.tree_density > 0.25 {
            let (leaves, height) = canopy(s.biome);
            let (leaf_rgb, leaf_kind) = self.colors.get(self.leaves[leaves]);
            let h = (height as f32 * (0.6 + 0.4 * s.tree_density.min(1.0))).round() as i32;
            return Col {
                top: ground + h,
                rgb: leaf_rgb,
                kind: leaf_kind,
                water: false,
                climate,
                floor: Some((ground, rgb, kind)),
            };
        }
        Col {
            top: ground,
            rgb,
            kind,
            water: false,
            climate,
            floor: None,
        }
    }
}

/// The usual canopy of a forest biome: leaves (oak, birch, spruce, mangrove) and height.
fn canopy(b: Biome) -> (usize, i32) {
    match b {
        Biome::TropicalRainforest => (0, 16),
        Biome::TemperateRainforest => (2, 14),
        Biome::BorealForest | Biome::SnowyTaiga | Biome::MontaneForest => (2, 11),
        Biome::Krummholz => (2, 3),
        Biome::BirchForest => (1, 8),
        Biome::Mangrove => (3, 8),
        Biome::Savanna => (0, 6),
        _ => (0, 8),
    }
}

/// Face codes as in the terrain shader.
const DOWN_UP: [u32; 2] = [0, 1];
const NORTH: u32 = 2;
const SOUTH: u32 = 3;
const WEST: u32 = 4;
const EAST: u32 = 5;

fn meshed_vertex(x: i32, y: i32, z: i32, c: &Col, face: u32) -> LodVertex {
    LodVertex {
        xz: (x as u32 & 0xffff) | ((z as u32 & 0xffff) << 16),
        y,
        color: c.rgb | (face << 24) | ((c.kind as u32 & 15) << 27) | (u32::from(c.water) << 31),
        climate: c.climate,
    }
}

/// Meshes a tile's columns (with their ring of neighbours): row-merged tops, the sides that
/// show, and skirts along the tile edges.
fn mesh(key: TileKey, cols: &[Col]) -> TileMesh {
    let cs = key.column();
    let n = TILE + 2;
    let at = |i: i32, j: i32| &cols[((j + 1) * n + (i + 1)) as usize];
    let mut v = Vec::new();
    let (mut min_y, mut max_y) = (i32::MAX, i32::MIN);
    let quad = |v: &mut Vec<LodVertex>, p: [(i32, i32, i32); 4], c: &Col, face: u32| {
        for (x, y, z) in p {
            v.push(meshed_vertex(x, y, z, c, face));
        }
    };
    let same = |a: &Col, b: &Col| {
        a.top == b.top && a.rgb == b.rgb && a.kind == b.kind && a.water == b.water
    };
    let same_floor = |a: &Col, b: &Col| match (a.floor, b.floor) {
        (Some(fa), Some(fb)) => fa == fb,
        _ => false,
    };
    for j in 0..TILE {
        // Forest floors under the canopy, merged along the row; lit from below (face 0) so they
        // take only the dim light of the shade.
        let mut i = 0;
        while i < TILE {
            let c = at(i, j);
            let Some((ground, rgb, kind)) = c.floor else {
                i += 1;
                continue;
            };
            let mut end = i;
            while end + 1 < TILE && same_floor(at(end + 1, j), c) {
                end += 1;
            }
            let floor = Col {
                top: ground,
                rgb,
                kind,
                water: false,
                climate: c.climate,
                floor: None,
            };
            let (x0, x1, z0, z1) = (i * cs, (end + 1) * cs, j * cs, (j + 1) * cs);
            quad(
                &mut v,
                [
                    (x0, ground, z1),
                    (x1, ground, z1),
                    (x1, ground, z0),
                    (x0, ground, z0),
                ],
                &floor,
                DOWN_UP[0],
            );
            min_y = min_y.min(ground);
            i = end + 1;
        }
        // Tops, merged along the row.
        let mut i = 0;
        while i < TILE {
            let c = at(i, j);
            let mut end = i;
            while end + 1 < TILE && same(at(end + 1, j), c) {
                end += 1;
            }
            let (x0, x1, z0, z1) = (i * cs, (end + 1) * cs, j * cs, (j + 1) * cs);
            quad(
                &mut v,
                [
                    (x0, c.top, z1),
                    (x1, c.top, z1),
                    (x1, c.top, z0),
                    (x0, c.top, z0),
                ],
                c,
                DOWN_UP[1],
            );
            max_y = max_y.max(c.top);
            min_y = min_y.min(c.top);
            i = end + 1;
        }
        // Sides toward lower neighbours; along the tile edge, skirts reaching below both.
        for i in 0..TILE {
            let c = at(i, j);
            let (x0, x1, z0, z1) = (i * cs, (i + 1) * cs, j * cs, (j + 1) * cs);
            for (di, dj, face) in [(0, -1, NORTH), (0, 1, SOUTH), (-1, 0, WEST), (1, 0, EAST)] {
                let (ni, nj) = (i + di, j + dj);
                let nb = at(ni, nj);
                let edge = !(0..TILE).contains(&ni) || !(0..TILE).contains(&nj);
                let low = if edge {
                    c.top.min(nb.top) - 2 * cs
                } else {
                    nb.top
                };
                if low >= c.top {
                    continue;
                }
                min_y = min_y.min(low);
                let p = match face {
                    NORTH => [
                        (x1, low, z0),
                        (x0, low, z0),
                        (x0, c.top, z0),
                        (x1, c.top, z0),
                    ],
                    SOUTH => [
                        (x0, low, z1),
                        (x1, low, z1),
                        (x1, c.top, z1),
                        (x0, c.top, z1),
                    ],
                    WEST => [
                        (x0, low, z0),
                        (x0, low, z1),
                        (x0, c.top, z1),
                        (x0, c.top, z0),
                    ],
                    _ => [
                        (x1, low, z1),
                        (x1, low, z0),
                        (x1, c.top, z0),
                        (x1, c.top, z1),
                    ],
                };
                quad(&mut v, p, c, face);
            }
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

    #[test]
    fn meshes_have_tops_sides_and_skirts() {
        let key = TileKey {
            level: 1,
            x: 3,
            z: -2,
        };
        let n = (TILE + 2) as usize;
        let mut cols = vec![
            Col {
                top: 10,
                rgb: 0x406080,
                kind: TINT_RGB,
                water: false,
                climate: 0,
                floor: None,
            };
            n * n
        ];
        // A single raised column in the middle of the tile.
        cols[(17 * n) + 17].top = 14;
        let m = mesh(key, &cols);
        // Flat rows merge into one top each (the raised column splits its row into three).
        let tops = m
            .vertices
            .chunks(4)
            .filter(|q| (q[0].color >> 24) & 7 == 1)
            .count();
        assert_eq!(tops, TILE as usize + 2);
        // A forest column keeps its floor under the canopy.
        let mut forest = cols.clone();
        forest[(5 * n) + 5] = Col {
            top: 20,
            floor: Some((10, 0x204020, TINT_RGB)),
            ..forest[(5 * n) + 5]
        };
        let f = mesh(key, &forest);
        let floors = f
            .vertices
            .chunks(4)
            .filter(|q| (q[0].color >> 24) & 7 == 0)
            .count();
        assert_eq!(floors, 1);
        // Four sides of the raised column, and skirts along all four tile edges.
        let sides = m
            .vertices
            .chunks(4)
            .filter(|q| (q[0].color >> 24) & 7 != 1)
            .count();
        assert_eq!(sides, 4 + 4 * TILE as usize);
        assert_eq!(m.max_y, 14);
        assert_eq!(m.min_y, 10 - 2 * key.column());
        assert!(
            m.vertices
                .iter()
                .all(|v| (v.xz & 0xffff) as i32 <= key.size() && (v.xz >> 16) as i32 <= key.size())
        );
    }
}

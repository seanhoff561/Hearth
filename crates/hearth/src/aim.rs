//! What the eyes rest on (Amendment T §2.3): the thing itself, not the cell it is in. A look
//! meets a block by its own drawn shape — a plant's sprite only where it is not clear (its fruit
//! and flowers told from its leaves by their texels' alpha, `hearth_texgen::PART_ALPHA`), a limb
//! or a piece built by its quads — the smooth ground to the millimetre, where it marks the patch
//! a dig there would take, and water at its surface; things lying and animals by the boxes they
//! are drawn as, or their bodies' parts (`client.rs`). The highlight follows the same shapes
//! (`hearth_render::outline`).

use std::sync::Arc;

use glam::DVec3;
use hearth_character::FigureInstance;
use hearth_math::{BlockPos, Direction};
use hearth_render::atlas::TextureArray;
use hearth_render::mesh::ColumnTints;
use hearth_render::models::{BlockModels, ModelQuad, Tint};
use hearth_render::outline::{Highlight, MaskQuad};
use hearth_world::{BlockRegistry, BlockStateId, CubeMap, RenderLayer, StateFlags};

/// What the eyes rest on within reach.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Aim {
    /// A thing lying in the world.
    Item(u64),
    /// An animal within reach of what is in hand.
    Animal(u64),
    /// A block (a plant, a limb, a piece built, a station, the ground, water): where, whether
    /// its top face is looked at (and which face is), the point the eyes rest on and the part
    /// of it.
    Block {
        pos: BlockPos,
        top: bool,
        face: Direction,
        at: DVec3,
        part: Part,
    },
}

impl Aim {
    /// The point the eyes rest on, on a block (the ground a dig takes from).
    pub fn point(&self) -> Option<DVec3> {
        match self {
            Aim::Block { at, .. } => Some(*at),
            _ => None,
        }
    }
}

/// Which of a block the eyes rest on: what its highlight follows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Part {
    /// All of it as drawn: a plant, a limb, a piece built.
    Whole,
    /// A plant's fruit or flowers: its sprite's texels of them.
    Fruit,
    /// The ground: the patch a dig there takes, about the point; the surface's normal there.
    Ground(DVec3),
    /// The water's surface at the point.
    Water,
}

/// The blocks as they are drawn, for a look to meet them: the baked models and their textures'
/// alpha.
pub struct Shapes {
    pub models: BlockModels,
    atlas: Arc<TextureArray>,
}

impl Shapes {
    pub fn new(reg: &BlockRegistry, atlas: Arc<TextureArray>) -> Self {
        Self {
            models: BlockModels::build(reg, &atlas),
            atlas,
        }
    }

    /// With models already baked.
    pub fn with_models(models: BlockModels, atlas: Arc<TextureArray>) -> Self {
        Self { models, atlas }
    }

    /// The middles (in the world) of the texels of a block's cutout quads whose alpha `want`
    /// takes: where its fruit is drawn, its leaves, its gaps.
    pub fn texels(&self, s: BlockStateId, corner: DVec3, want: impl Fn(u8) -> bool) -> Vec<DVec3> {
        let mut out = Vec::new();
        self.models.each_quad(s, |q, layer| {
            if layer != RenderLayer::Cutout {
                return;
            }
            let Some(texels) = self.atlas.layers.get(q.tex.tex.layer as usize) else {
                return;
            };
            // Corners 0, 1 and 3 span the quad, its texels running across them.
            let p = q.pos.map(|c| c.as_dvec3());
            let (u0, u1, v0, v3) = (q.uv[0].x, q.uv[1].x, q.uv[0].y, q.uv[3].y);
            if (u1 - u0).abs() < 1e-3 || (v3 - v0).abs() < 1e-3 {
                return;
            }
            let n = self.atlas.size as usize;
            for (i, t) in texels.iter().enumerate() {
                if !want(t[3]) {
                    continue;
                }
                let (tx, ty) = ((i % n) as f32 + 0.5, (i / n) as f32 + 0.5);
                let su = ((tx - u0) / (u1 - u0)) as f64;
                let sv = ((ty - v0) / (v3 - v0)) as f64;
                if (0.0..=1.0).contains(&su) && (0.0..=1.0).contains(&sv) {
                    out.push(corner + p[0] + (p[1] - p[0]) * su + (p[3] - p[0]) * sv);
                }
            }
        });
        out
    }

    /// The alpha of a layer's texel at texel coordinates (`u`, `v`), wrapping as the textures
    /// do where a face's texture is turned.
    fn alpha(&self, layer: u16, u: f64, v: f64) -> u8 {
        let n = self.atlas.size as i64;
        let texel = |c: f64| {
            if (0.0..=n as f64).contains(&c) {
                (c as i64).min(n - 1)
            } else {
                (c.floor() as i64).rem_euclid(n)
            }
        };
        self.atlas
            .layers
            .get(layer as usize)
            .map_or(255, |l| l[(texel(v) * n + texel(u)) as usize][3])
    }

    /// Where a look from `eye` along unit `dir` meets the drawn shape of block `s` with its
    /// corner at `corner`: its distance, the part, and the face of the quad met where it faces a
    /// way. A cutout's texels are met only where they are seen: not where clear, nor where its
    /// foliage's leaves have fallen (`leaf` its cover, as the terrain draws it, asked once at
    /// most).
    pub fn hit(
        &self,
        s: BlockStateId,
        corner: DVec3,
        (eye, dir): (DVec3, DVec3),
        leaf: &dyn Fn() -> f64,
    ) -> Option<(f64, Part, Option<Direction>)> {
        let from = eye - corner;
        let cover = std::cell::OnceCell::new();
        let mut best: Option<(f64, Part, Option<Direction>)> = None;
        self.models.each_quad(s, |q, layer| {
            let Some((t, u, v)) = ray_quad(from, dir, q) else {
                return;
            };
            if best.is_some_and(|b| b.0 <= t) {
                return;
            }
            let part = if layer == RenderLayer::Cutout {
                let part = match self.alpha(q.tex.tex.layer, u, v) {
                    a if a < 128 => return,
                    hearth_texgen::PART_ALPHA => Part::Fruit,
                    _ => Part::Whole,
                };
                let normal = q.dir.map_or(DVec3::ZERO, |d| d.offset().as_dvec3());
                if deciduous(q.tex.tint)
                    && leaf_fall(eye + dir * t, normal, *cover.get_or_init(leaf)) == 2
                {
                    return;
                }
                part
            } else {
                Part::Whole
            };
            best = Some((t, part, q.dir));
        });
        best
    }
}

/// Whether a face's tint is a deciduous tree's (its leaves fall).
fn deciduous(t: Tint) -> bool {
    matches!(t, Tint::Foliage | Tint::Birch | Tint::FoliageRed)
}

/// The leaf cover of deciduous foliage in a column (its climate code) at a time of year, as
/// the terrain's shader reckons it (`common.wgsl`'s `leaf_cover`, after `hearth_env::phenology`).
pub fn leaf_cover(climate: u32, year_frac: f64) -> f64 {
    let d = hearth_env::tint::decode(climate);
    hearth_env::phenology::state(
        hearth_env::phenology::PlantType::DeciduousBroadleaf,
        &d.normals(),
        year_frac,
    )
    .leaf
}

/// `common.wgsl`'s `hash_texel`: a texel's place (texels of 1/16 block, wrapped at 4096
/// blocks) to 0..1.
fn hash_texel(t: [i64; 3]) -> f64 {
    let q = t.map(|v| (v as u32) & 0xffff);
    let mut h = q[0].wrapping_mul(0x8da6_b343)
        ^ q[1].wrapping_mul(0xd816_3841)
        ^ q[2].wrapping_mul(0xcb1a_b31f);
    h = (h ^ (h >> 16)).wrapping_mul(0x7feb_352d);
    h = (h ^ (h >> 15)).wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    h as f64 / 4_294_967_296.0
}

/// `common.wgsl`'s `leaf_fall` at a point of foliage in the world facing `normal`, its cover
/// `leaf`: 0 the leaf stays, 1 a twig shows, 2 nothing (the look goes on through).
fn leaf_fall(at: DVec3, normal: DVec3, leaf: f64) -> u8 {
    if leaf >= 0.999 {
        return 0;
    }
    let p = ((at - normal * 0.03) * 16.0).floor();
    let t = [p.x as i64, p.y as i64, p.z as i64];
    if hash_texel(t) < leaf {
        return 0;
    }
    if hash_texel([t[0] + 19, t[1] + 7, t[2] + 3]) > 0.16 {
        return 2;
    }
    1
}

/// Where a ray (`from` along `dir`) meets a model quad, both sides: its distance (in `dir`s)
/// and the texel coordinates there.
fn ray_quad(from: DVec3, dir: DVec3, q: &ModelQuad) -> Option<(f64, f64, f64)> {
    let p = q.pos.map(|v| v.as_dvec3());
    let uv = q.uv.map(|v| v.as_dvec2());
    for (a, b, c) in [(0, 1, 2), (0, 2, 3)] {
        if let Some((t, s, r)) = ray_triangle(from, dir, p[a], p[b], p[c]) {
            let at = uv[a] * (1.0 - s - r) + uv[b] * s + uv[c] * r;
            return Some((t, at.x, at.y));
        }
    }
    None
}

/// Möller–Trumbore, either side: the distance and the barycentric weights of `b` and `c`. A
/// look along the triangle's plane (within a microradian: a quad seen edge on is not drawn)
/// does not meet it.
fn ray_triangle(from: DVec3, dir: DVec3, a: DVec3, b: DVec3, c: DVec3) -> Option<(f64, f64, f64)> {
    let (e1, e2) = (b - a, c - a);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-6 * e1.cross(e2).length() {
        return None;
    }
    let inv = 1.0 / det;
    let s = from - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t >= 0.0).then_some((t, u, v))
}

/// Where a ray (`from` along `dir`, camera-relative as the box is) enters a box drawn by an
/// instance (the unit cube placed): its distance in `dir`s.
pub fn ray_instance(i: &FigureInstance, from: DVec3, dir: DVec3) -> Option<f64> {
    let r = &i.rows;
    let m = glam::DMat3::from_cols(
        DVec3::new(r[0][0] as f64, r[1][0] as f64, r[2][0] as f64),
        DVec3::new(r[0][1] as f64, r[1][1] as f64, r[2][1] as f64),
        DVec3::new(r[0][2] as f64, r[1][2] as f64, r[2][2] as f64),
    );
    if m.determinant().abs() < 1e-12 {
        return None;
    }
    let inv = m.inverse();
    let t = DVec3::new(r[0][3] as f64, r[1][3] as f64, r[2][3] as f64);
    ray_box(
        inv * (from - t),
        inv * dir,
        DVec3::splat(-0.5),
        DVec3::splat(0.5),
    )
}

/// Where a ray from `from` along `dir` enters the box `lo`–`hi` (its distance in `dir`s), if
/// it does; zero when it starts inside.
pub fn ray_box(from: DVec3, dir: DVec3, lo: DVec3, hi: DVec3) -> Option<f64> {
    let mut near = 0.0f64;
    let mut far = f64::INFINITY;
    for a in 0..3 {
        let (o, d, l, h) = (from[a], dir[a], lo[a], hi[a]);
        if d.abs() < 1e-12 {
            if o < l || o > h {
                return None;
            }
            continue;
        }
        let (t0, t1) = ((l - o) / d, (h - o) / d);
        near = near.max(t0.min(t1));
        far = far.min(t0.max(t1));
        if near > far {
            return None;
        }
    }
    Some(near)
}

/// The face of a block a look enters it by, stepping along `axis` the way of `step`.
fn entered_by(axis: usize, step: i32) -> Direction {
    match (axis, step > 0) {
        (0, true) => Direction::West,
        (0, false) => Direction::East,
        (1, true) => Direction::Down,
        (1, false) => Direction::Up,
        (2, true) => Direction::North,
        _ => Direction::South,
    }
}

/// Where a look meets water in block `cell` (state `s`), having entered the block at distance
/// `entered` by face `by`: its surface (the block's water height, or its top under more water)
/// from above, or the face it came in by when it enters below the surface.
fn water_hit(
    map: &CubeMap,
    reg: &BlockRegistry,
    cell: BlockPos,
    s: BlockStateId,
    (eye, dir): (DVec3, DVec3),
    entered: f64,
    by: Direction,
) -> Option<(f64, DVec3, Direction)> {
    let deep = map
        .block(cell.up())
        .is_some_and(|a| reg.has(a, StateFlags::WATER));
    let h = if deep {
        1.0
    } else {
        reg.fluid_amount(s).max(1) as f64 / 9.0
    };
    let top = cell.y as f64 + h;
    let p = eye + dir * entered;
    if p.y <= top + 1e-9 {
        return Some((entered, p, by));
    }
    if dir.y >= 0.0 {
        return None;
    }
    let t = (top - eye.y) / dir.y;
    let q = eye + dir * t;
    let within = |v: f64, lo: i32| v >= lo as f64 - 1e-6 && v <= lo as f64 + 1.0 + 1e-6;
    (within(q.x, cell.x) && within(q.z, cell.z)).then_some((t, q, Direction::Up))
}

/// The first block a look from `eye` along unit `dir` meets within `reach` (its distance and
/// what is met), by the blocks' drawn shapes; natural ground met to the millimetre beyond them.
/// `leaf` gives the leaf cover of deciduous foliage at a block (1 in full leaf).
pub fn pick_block(
    map: &CubeMap,
    reg: &BlockRegistry,
    shapes: &Shapes,
    (eye, dir): (DVec3, DVec3),
    reach: f64,
    leaf: &dyn Fn(BlockPos) -> f64,
) -> Option<(f64, Aim)> {
    // The smooth ground (Amendment S): where the look meets its surface. Blocks are looked for
    // only before it, and natural ground is not a block to them.
    let ground = hearth_world::ground::raycast(map, reg, eye, dir, reach);
    let reach = ground.map_or(reach, |g| g.distance);
    // From within water, its surface is not met: the look goes on through it.
    let in_water = map
        .block(BlockPos::containing(eye))
        .is_some_and(|s| reg.has(s, StateFlags::WATER));
    // Through the blocks along the look, one by one (Amanatides and Woo).
    let mut cell = BlockPos::containing(eye);
    let mut c = [cell.x, cell.y, cell.z];
    let step: [i32; 3] = std::array::from_fn(|a| {
        if dir[a] > 0.0 {
            1
        } else if dir[a] < 0.0 {
            -1
        } else {
            0
        }
    });
    let span: [f64; 3] = std::array::from_fn(|a| {
        if step[a] == 0 {
            f64::INFINITY
        } else {
            1.0 / dir[a].abs()
        }
    });
    let mut next: [f64; 3] = std::array::from_fn(|a| match step[a] {
        0 => f64::INFINITY,
        1 => (c[a] as f64 + 1.0 - eye[a]) * span[a],
        _ => (eye[a] - c[a] as f64) * span[a],
    });
    let (mut entered, mut by) = (0.0f64, Direction::Up);
    while entered <= reach {
        if let Some(s) = map.block(cell)
            && !s.is_air()
            && !reg.has(s, StateFlags::NATURAL)
        {
            let corner = DVec3::new(cell.x as f64, cell.y as f64, cell.z as f64);
            let mut best: Option<(f64, Aim)> = None;
            if let Some((t, part, face)) = shapes.hit(s, corner, (eye, dir), &|| leaf(cell))
                && t <= reach
            {
                let face = face.unwrap_or(by);
                best = Some((
                    t,
                    Aim::Block {
                        pos: cell,
                        top: face == Direction::Up,
                        face,
                        at: eye + dir * t,
                        part,
                    },
                ));
            }
            if !in_water
                && reg.has(s, StateFlags::WATER)
                && let Some((t, at, face)) = water_hit(map, reg, cell, s, (eye, dir), entered, by)
                && t <= reach
                && best.as_ref().is_none_or(|b| t < b.0)
            {
                best = Some((
                    t,
                    Aim::Block {
                        pos: cell,
                        top: face == Direction::Up,
                        face,
                        at,
                        part: Part::Water,
                    },
                ));
            }
            if best.is_some() {
                return best;
            }
        }
        // On to the next block along the look.
        let a = (0..3)
            .min_by(|&i, &j| next[i].total_cmp(&next[j]))
            .unwrap_or(0);
        if !next[a].is_finite() {
            break;
        }
        entered = next[a];
        next[a] += span[a];
        c[a] += step[a];
        by = entered_by(a, step[a]);
        cell = BlockPos::new(c[0], c[1], c[2]);
    }
    ground.map(|g| {
        use hearth_math::Direction as D;
        let n = g.normal;
        let face = if n.y.abs() >= n.x.abs().max(n.z.abs()) {
            if n.y > 0.0 { D::Up } else { D::Down }
        } else if n.x.abs() >= n.z.abs() {
            if n.x > 0.0 { D::East } else { D::West }
        } else if n.z > 0.0 {
            D::South
        } else {
            D::North
        };
        (
            g.distance,
            Aim::Block {
                pos: g.voxel,
                top: face == D::Up,
                face,
                at: g.at,
                part: Part::Ground(g.normal),
            },
        )
    })
}

/// The highlight of a block looked at (T §2.3), camera-relative to `view`: the patch of ground
/// a dig there takes, the water about the point, or the block's own quads (only its fruit and
/// flowers when those are what the eyes rest on); `tints` its column's.
pub fn block_highlight(
    shapes: &Shapes,
    map: &CubeMap,
    tints: &ColumnTints,
    (pos, at, part): (BlockPos, DVec3, Part),
    view: DVec3,
) -> Option<Highlight> {
    let rel = |p: DVec3| (p - view).as_vec3();
    match part {
        Part::Ground(normal) => {
            let (centre, r) =
                hearth_world::ground::bowl(at, normal, hearth_world::ground::DIG_RADIUS_M);
            Some(Highlight::Ground {
                centre: rel(centre),
                radius: r as f32,
            })
        }
        // A cupped hand's breadth of the surface about the point.
        Part::Water => Some(Highlight::Water {
            at: rel(at),
            radius: 0.3,
        }),
        Part::Whole | Part::Fruit => {
            let s = map.block(pos)?;
            let (lx, lz) = ((pos.x & 15) as usize, (pos.z & 15) as usize);
            let cube = matches!(
                shapes.models.get(s),
                hearth_render::models::StateModel::Cube(_)
            );
            let mut quads = Vec::new();
            shapes.models.each_quad(s, |q, layer| {
                let kind = tints.get(q.tex.tint, lx, lz).0;
                quads.push(MaskQuad::new(q, layer, kind, cube));
            });
            Some(Highlight::Block {
                quads,
                origin: rel(DVec3::new(pos.x as f64, pos.y as f64, pos.z as f64)),
                fruit: part == Part::Fruit,
                climate: tints.climate[lz * 16 + lx],
            })
        }
    }
}

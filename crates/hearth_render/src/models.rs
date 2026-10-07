//! Baked block models: what to draw for every block state.
//!
//! Default models are derived from the block definitions and texture naming conventions (a
//! resource pack can override them later with standard blockstate/model files). Full cubes are
//! kept in a compact per-face form for the greedy mesher; everything else becomes quads.

use std::sync::Arc;

use glam::{Vec2, Vec3};
use hearth_math::Direction;
use hearth_world::{
    Block, BlockRegistry, BlockStateId, RenderKind, RenderLayer, StateFlags, TintKind,
};

use crate::atlas::{TexInfo, TextureArray};

/// Tint applied to a face, resolved per column from climate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Tint {
    #[default]
    None = 0,
    Grass = 1,
    Foliage = 2,
    Water = 3,
    Birch = 4,
    Spruce = 5,
    DryGrass = 6,
    FoliageRed = 7,
}

impl From<TintKind> for Tint {
    fn from(t: TintKind) -> Self {
        match t {
            TintKind::None => Tint::None,
            TintKind::Grass => Tint::Grass,
            TintKind::Foliage => Tint::Foliage,
            TintKind::Water => Tint::Water,
            TintKind::Birch => Tint::Birch,
            TintKind::Spruce => Tint::Spruce,
            TintKind::DryGrass => Tint::DryGrass,
            TintKind::FoliageRed => Tint::FoliageRed,
        }
    }
}

/// Sentinel for "no overlay".
pub const NO_OVERLAY: u16 = u16::MAX;

/// Texture of one face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FaceTex {
    pub tex: TexInfo,
    /// Optional second (tinted) layer drawn over the base, e.g. the grass side fringe.
    pub overlay: u16,
    /// Tint of the base (or of the overlay when present).
    pub tint: Tint,
    /// UV rotation in quarter turns.
    pub rot: u8,
}

/// A full-cube model: one texture per face (indexed by `Direction::index`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CubeModel {
    pub faces: [FaceTex; 6],
    pub layer: RenderLayer,
    /// Sways in the wind (leaves).
    pub waving: bool,
}

/// An arbitrary quad in block-local space (0..1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelQuad {
    /// Corners in counter-clockwise order seen from the front.
    pub pos: [Vec3; 4],
    /// Texture coordinates in texels (0..16) per corner.
    pub uv: [Vec2; 4],
    pub tex: FaceTex,
    /// Face direction for shading, if axis-aligned.
    pub dir: Option<Direction>,
    /// The quad is hidden when the neighbour on this side occludes it.
    pub cull: Option<Direction>,
    /// Apply ambient occlusion and directional shading.
    pub shade: bool,
    pub waving: bool,
}

/// What to draw for one state.
#[derive(Debug, Clone)]
pub enum StateModel {
    Invisible,
    Cube(CubeModel),
    Quads(Arc<[ModelQuad]>, RenderLayer),
    Fluid { still: FaceTex, flow: FaceTex },
}

/// Models for every state plus occlusion data.
#[derive(Debug, Clone)]
pub struct BlockModels {
    models: Vec<StateModel>,
    /// Bit `d` set: the state's face toward `d` is a full opaque face that hides neighbours.
    occludes: Vec<u8>,
    /// Per state: 1 a tree's limb, 2 foliage, 0 anything else.
    tree_part: Vec<u8>,
    /// Per limb state: its thickness in pixels (bits 8..16) and the sides it joins (bits 0..6).
    limbs: Vec<u16>,
    /// Per state: 1 + the index of its member in `members` if it is a construction piece.
    member_of: Vec<u32>,
    members: Vec<Member>,
    /// Average (linear-ish sRGB) colour for LOD and particles.
    pub colors: Vec<[u8; 4]>,
}

/// A construction piece as its neighbours join it (`crate::joints`): its boxes and how it is
/// drawn.
#[derive(Debug, Clone)]
pub struct Member {
    pub boxes: smallvec::SmallVec<[crate::joints::Box6; 8]>,
    pub tex: FaceTex,
    pub layer: RenderLayer,
    /// A roof: the way it rises and its slab's thickness (sixteenths, upright), drawn smooth.
    pub roof: Option<(Direction, f32)>,
}

impl BlockModels {
    #[inline]
    pub fn get(&self, s: BlockStateId) -> &StateModel {
        &self.models[s.0 as usize]
    }

    /// True if `s` hides the neighbour face that looks at it from direction `d` (i.e. the face of
    /// `s` pointing toward `d` is full and opaque).
    #[inline]
    pub fn occludes(&self, s: BlockStateId, d: Direction) -> bool {
        self.occludes[s.0 as usize] & (1 << d.index()) != 0
    }

    /// A tree's limb (its faces toward foliage are hidden in the crown).
    #[inline]
    pub fn is_limb(&self, s: BlockStateId) -> bool {
        self.tree_part[s.0 as usize] == 1
    }

    /// Foliage.
    #[inline]
    pub fn is_foliage(&self, s: BlockStateId) -> bool {
        self.tree_part[s.0 as usize] == 2
    }

    /// The construction piece a state is, as its neighbours join it.
    #[inline]
    pub fn member(&self, s: BlockStateId) -> Option<&Member> {
        match self.member_of[s.0 as usize] {
            0 => None,
            i => Some(&self.members[i as usize - 1]),
        }
    }

    /// A limb's thickness in pixels (0 for anything else).
    #[inline]
    pub fn limb_thickness(&self, s: BlockStateId) -> u8 {
        (self.limbs[s.0 as usize] >> 8) as u8
    }

    /// Whether `s` is a limb joined toward `d` at least `px` thick: the end of a limb `px` thick
    /// that meets it from that side lies inside it.
    #[inline]
    pub fn covers_limb_end(&self, s: BlockStateId, d: Direction, px: u8) -> bool {
        let l = self.limbs[s.0 as usize];
        l & (1 << d.index()) != 0 && (l >> 8) as u8 >= px
    }

    pub fn len(&self) -> usize {
        self.models.len()
    }

    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }

    /// Bakes every state of the registry.
    pub fn build(reg: &BlockRegistry, atlas: &TextureArray) -> Self {
        let n = reg.state_count();
        let mut models = Vec::with_capacity(n);
        let mut occludes = Vec::with_capacity(n);
        let mut tree_part = Vec::with_capacity(n);
        let mut limbs = Vec::with_capacity(n);
        let mut colors = Vec::with_capacity(n);
        let mut member_of = Vec::with_capacity(n);
        let mut members = Vec::new();
        for i in 0..n {
            let s = BlockStateId(i as u16);
            let block = reg.block_of(s);
            let model = bake(block, s, reg, atlas);
            // A construction piece: its boxes and look, for the joints its neighbours make.
            member_of.push(match piece_look(atlas, block.name.path()) {
                Some((tex, layer)) => {
                    let boxes = reg
                        .outline_shape(s)
                        .boxes
                        .iter()
                        .map(|b| {
                            let (a, c) = (b.min.as_vec3(), b.max.as_vec3());
                            [a.x, a.y, a.z, c.x, c.y, c.z]
                        })
                        .collect();
                    let roof = roof_of(reg, s);
                    members.push(Member {
                        boxes,
                        tex,
                        layer,
                        roof,
                    });
                    members.len() as u32
                }
                None => 0,
            });
            let mut occ = 0u8;
            if block.def.layer == RenderLayer::Opaque && !reg.has(s, StateFlags::INVISIBLE) {
                let shape = reg.outline_shape(s);
                for d in Direction::ALL {
                    if block.def.opaque || (!shape.is_empty() && shape.covers_face(d)) {
                        occ |= 1 << d.index();
                    }
                }
            }
            colors.push(average_color(&model, atlas, block));
            let path = block.name.path();
            tree_part.push(if path.ends_with("_branch") {
                1
            } else if matches!(&model, StateModel::Cube(c) if c.waving) {
                2
            } else {
                0
            });
            limbs.push(if path.ends_with("_branch") {
                let px = prop(reg, s, "thickness")
                    .and_then(|v| v.parse::<u16>().ok())
                    .unwrap_or(4);
                Direction::ALL
                    .iter()
                    .filter(|d| prop(reg, s, d.name()) == Some("true"))
                    .fold(px << 8, |m, d| m | (1 << d.index()))
            } else {
                0
            });
            models.push(model);
            occludes.push(occ);
        }
        Self {
            models,
            occludes,
            tree_part,
            limbs,
            member_of,
            members,
            colors,
        }
    }
}

/// A roof piece (its shape steps up across the block, V2-8): the way it rises and the thickness
/// its smooth slab is drawn (sixteenths, upright): its steps' and two more, so that the steps,
/// which bear and are walked on, and the ends of what meets them lie inside it.
fn roof_of(reg: &BlockRegistry, s: BlockStateId) -> Option<(Direction, f32)> {
    let shape = reg.outline_shape(s);
    if shape.boxes.len() < 4 {
        return None;
    }
    let facing = prop(reg, s, "facing")
        .and_then(Direction::from_name)
        .unwrap_or(Direction::North);
    let b = shape
        .boxes
        .iter()
        .min_by(|a, b| a.min.y.total_cmp(&b.min.y))?;
    Some((facing, ((b.max.y - b.min.y) * 16.0) as f32 + 2.0))
}

/// The smooth slab of a roof rising toward the north (V2-9): the band between y = s − 2 and
/// y = s − 2 + `t` (sixteenths; s the way from the low, south side) over the block's column,
/// laid along x from `x0` to `x1` (0..1); then turned to rise toward `facing`. A thick slab
/// passes a little above its block at the high side and below it at the low: so the next
/// piece up or down the slope, which draws the band over its own column, carries it on
/// without a notch. Its faces toward the ends of its length are its gables.
pub(crate) fn roof_slab(
    t: f32,
    x0: f32,
    x1: f32,
    facing: Direction,
    tex: FaceTex,
) -> Vec<ModelQuad> {
    // The band in (s, y) over the column, counter-clockwise.
    let poly = [
        Vec2::new(0.0, -2.0),
        Vec2::new(16.0, 14.0),
        Vec2::new(16.0, 14.0 + t),
        Vec2::new(0.0, t - 2.0),
    ];
    // (s, y) in sixteenths at x (0..1) to the block's space, rising toward the north.
    let at = |p: Vec2, x: f32| Vec3::new(x, p.y / 16.0, (16.0 - p.x) / 16.0);
    let mut out = Vec::new();
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (b - a).length() < 1e-4 {
            continue;
        }
        // The side's outward way in (s, y), and what it is: a cut at the block's side, or the
        // slope above or below.
        let out_sy = Vec2::new(b.y - a.y, -(b.x - a.x)).normalize();
        let on = |c: f32, v: f32| (c - v).abs() < 1e-4;
        let (dir, cull) = if on(a.x, 0.0) && on(b.x, 0.0) {
            (Direction::South, true)
        } else if on(a.x, 16.0) && on(b.x, 16.0) {
            (Direction::North, true)
        } else if out_sy.y > 0.0 {
            (Direction::Up, false)
        } else {
            (Direction::Down, false)
        };
        let outward = Vec3::new(0.0, out_sy.y, -out_sy.x);
        // Texels down the face (a texture's width on, to keep them positive: they repeat).
        let v = |p: Vec2| 32.0 - p.y;
        let corners = [at(a, x0), at(a, x1), at(b, x1), at(b, x0)];
        let uv = [
            Vec2::new(x0 * 16.0, v(a)),
            Vec2::new(x1 * 16.0, v(a)),
            Vec2::new(x1 * 16.0, v(b)),
            Vec2::new(x0 * 16.0, v(b)),
        ];
        out.push(facing_out(
            corners,
            uv,
            outward,
            tex,
            Some(dir),
            cull.then_some(dir),
        ));
    }
    // The gables: the band's outline at each end, in triangles.
    for (x, outward) in [(x0, -1.0f32), (x1, 1.0)] {
        let dir = if outward < 0.0 {
            Direction::West
        } else {
            Direction::East
        };
        let cull = (outward < 0.0 && x <= 1e-4) || (outward > 0.0 && x >= 1.0 - 1e-4);
        for i in 1..n - 1 {
            let tri = [poly[0], poly[i], poly[i + 1], poly[i + 1]];
            let corners = tri.map(|p| at(p, x));
            let uv = tri.map(|p| Vec2::new(16.0 - p.x, 32.0 - p.y));
            out.push(facing_out(
                corners,
                uv,
                Vec3::new(outward, 0.0, 0.0),
                tex,
                Some(dir),
                cull.then_some(dir),
            ));
        }
    }
    turned(out, facing)
}

/// A quad wound so that it faces `outward` (counter-clockwise seen from there).
fn facing_out(
    mut pos: [Vec3; 4],
    mut uv: [Vec2; 4],
    outward: Vec3,
    tex: FaceTex,
    dir: Option<Direction>,
    cull: Option<Direction>,
) -> ModelQuad {
    let mut n = (pos[1] - pos[0]).cross(pos[2] - pos[0]);
    if n.length_squared() < 1e-12 {
        n = (pos[2] - pos[0]).cross(pos[3] - pos[0]);
    }
    if n.dot(outward) < 0.0 {
        pos.reverse();
        uv.reverse();
    }
    ModelQuad {
        pos,
        uv,
        tex,
        dir,
        cull,
        shade: true,
        waving: false,
    }
}

/// Quads made rising toward the north, turned to rise toward `facing` (as the block registry
/// turns a facing block's shape).
fn turned(mut quads: Vec<ModelQuad>, facing: Direction) -> Vec<ModelQuad> {
    let rot = |p: Vec3| match facing {
        Direction::South => Vec3::new(1.0 - p.x, p.y, 1.0 - p.z),
        Direction::East => Vec3::new(1.0 - p.z, p.y, p.x),
        Direction::West => Vec3::new(p.z, p.y, 1.0 - p.x),
        _ => p,
    };
    let way = |d: Direction| {
        let n = d.normal();
        let v = rot(n + Vec3::splat(0.5)) - rot(Vec3::splat(0.5));
        Direction::nearest(v.as_dvec3())
    };
    for q in &mut quads {
        q.pos = q.pos.map(rot);
        q.dir = q.dir.map(way);
        q.cull = q.cull.map(way);
    }
    quads
}

/// How a construction piece (`brush_wall/oak_wood`) is drawn: a look of its own if it has one
/// (brush: twigs with gaps, a cutout), else its material's texture.
fn piece_look(atlas: &TextureArray, name: &str) -> Option<(FaceTex, RenderLayer)> {
    let (_, m) = name.split_once('/')?;
    let own = format!("piece/{name}");
    let ctx = Ctx {
        atlas,
        tint: Tint::None,
    };
    Some(if atlas.contains(&format!("block/{own}")) {
        (ctx.tex(&own), RenderLayer::Cutout)
    } else {
        (ctx.tex(&format!("material/{m}")), RenderLayer::Opaque)
    })
}

fn average_color(model: &StateModel, atlas: &TextureArray, block: &Block) -> [u8; 4] {
    let name = |t: &FaceTex| -> Option<[u8; 4]> { layer_avg(atlas, t.tex.layer) };
    let c = match model {
        StateModel::Invisible => None,
        StateModel::Cube(c) => name(&c.faces[Direction::Up.index()]),
        StateModel::Quads(q, _) => q.first().and_then(|q| name(&q.tex)),
        StateModel::Fluid { still, .. } => name(still),
    };
    c.unwrap_or([
        block.map_color[0],
        block.map_color[1],
        block.map_color[2],
        255,
    ])
}

fn layer_avg(atlas: &TextureArray, layer: u16) -> Option<[u8; 4]> {
    let px = atlas.layers.get(layer as usize)?;
    let (mut r, mut g, mut b, mut a) = (0u32, 0u32, 0u32, 0u32);
    for p in px {
        let w = p[3] as u32;
        r += p[0] as u32 * w;
        g += p[1] as u32 * w;
        b += p[2] as u32 * w;
        a += w;
    }
    if a == 0 {
        return None;
    }
    Some([
        (r / a) as u8,
        (g / a) as u8,
        (b / a) as u8,
        (a / px.len() as u32) as u8,
    ])
}

// ------------------------------------------------------------------ baking

struct Ctx<'a> {
    atlas: &'a TextureArray,
    tint: Tint,
}

impl Ctx<'_> {
    fn tex(&self, name: &str) -> FaceTex {
        self.tex_tint(name, Tint::None)
    }

    fn tex_tint(&self, name: &str, tint: Tint) -> FaceTex {
        FaceTex {
            tex: self.atlas.get(&format!("block/{name}")),
            overlay: NO_OVERLAY,
            tint,
            rot: 0,
        }
    }

    /// The block's own texture with its tint.
    fn own(&self, name: &str) -> FaceTex {
        self.tex_tint(name, self.tint)
    }
}

fn prop<'a>(reg: &'a BlockRegistry, s: BlockStateId, p: &str) -> Option<&'a str> {
    reg.get(s, p)
}

/// Base material texture for slabs and stairs (`oak_slab` → `oak_planks`).
fn material(name: &str) -> String {
    let base = name.trim_end_matches("_slab").trim_end_matches("_stairs");
    match base {
        "oak" | "birch" | "spruce" => format!("{base}_planks"),
        other => other.into(),
    }
}

fn bake(block: &Block, s: BlockStateId, reg: &BlockRegistry, atlas: &TextureArray) -> StateModel {
    let name = block.name.path();
    let ctx = Ctx {
        atlas,
        tint: block.def.tint.into(),
    };
    match block.def.render {
        RenderKind::Invisible => StateModel::Invisible,
        RenderKind::Fluid => StateModel::Fluid {
            still: ctx.tex_tint("water_still", Tint::Water),
            flow: ctx.tex_tint("water_flow", Tint::Water),
        },
        RenderKind::Cross => {
            let tex = ctx.own(&cross_texture(name, s, reg));
            StateModel::Quads(
                cross(tex, block.def.tint != TintKind::None).into(),
                RenderLayer::Cutout,
            )
        }
        RenderKind::Cube => StateModel::Cube(cube_model(name, s, reg, &ctx, block)),
        RenderKind::Model => bake_model(name, s, reg, &ctx, block),
    }
}

fn cross_texture(name: &str, s: BlockStateId, reg: &BlockRegistry) -> String {
    if let Some(half) = prop(reg, s, "half") {
        return format!("{name}_{}", if half == "upper" { "top" } else { "bottom" });
    }
    // A crop as it grows (V2-12): its texture by its stage.
    if let Some(stage) = prop(reg, s, "stage") {
        return format!("{name}_{stage}");
    }
    name.to_owned()
}

/// Two diagonal quads.
fn cross(tex: FaceTex, waving: bool) -> Vec<ModelQuad> {
    let (a, b) = (0.15, 0.85);
    let quad = |x0: f32, z0: f32, x1: f32, z1: f32| ModelQuad {
        pos: [
            Vec3::new(x0, 0.0, z0),
            Vec3::new(x1, 0.0, z1),
            Vec3::new(x1, 1.0, z1),
            Vec3::new(x0, 1.0, z0),
        ],
        uv: [
            Vec2::new(0.0, 16.0),
            Vec2::new(16.0, 16.0),
            Vec2::new(16.0, 0.0),
            Vec2::new(0.0, 0.0),
        ],
        tex,
        dir: None,
        cull: None,
        shade: false,
        waving,
    };
    vec![quad(a, a, b, b), quad(a, b, b, a)]
}

/// Two diagonal quads filling the box from `min` to `max`, the whole texture stretched over
/// each (flames in a hearth, a lamp's flame).
fn cross_box(tex: FaceTex, min: Vec3, max: Vec3) -> Vec<ModelQuad> {
    let quad = |x0: f32, z0: f32, x1: f32, z1: f32| ModelQuad {
        pos: [
            Vec3::new(x0, min.y, z0),
            Vec3::new(x1, min.y, z1),
            Vec3::new(x1, max.y, z1),
            Vec3::new(x0, max.y, z0),
        ],
        uv: [
            Vec2::new(0.0, 16.0),
            Vec2::new(16.0, 16.0),
            Vec2::new(16.0, 0.0),
            Vec2::new(0.0, 0.0),
        ],
        tex,
        dir: None,
        cull: None,
        shade: false,
        waving: false,
    };
    vec![
        quad(min.x, min.z, max.x, max.z),
        quad(min.x, max.z, max.x, min.z),
    ]
}

fn cube_model(
    name: &str,
    s: BlockStateId,
    reg: &BlockRegistry,
    ctx: &Ctx<'_>,
    block: &Block,
) -> CubeModel {
    let all = |f: FaceTex| [f; 6];
    let up = Direction::Up.index();
    let down = Direction::Down.index();
    let axis_faces = |side: FaceTex, end: FaceTex, axis: &str| -> [FaceTex; 6] {
        let rotated = FaceTex { rot: 1, ..side };
        match axis {
            "x" => {
                let mut f = [rotated; 6];
                f[Direction::West.index()] = end;
                f[Direction::East.index()] = end;
                f
            }
            "z" => {
                let mut f = [side; 6];
                f[Direction::North.index()] = end;
                f[Direction::South.index()] = end;
                f[Direction::West.index()] = rotated;
                f[Direction::East.index()] = rotated;
                f
            }
            _ => {
                let mut f = [side; 6];
                f[up] = end;
                f[down] = end;
                f
            }
        }
    };
    let faces = if name == "grass_block" {
        if prop(reg, s, "snowy") == Some("true") {
            let mut f = all(ctx.tex("grass_block_snow"));
            f[up] = ctx.tex("snow");
            f[down] = ctx.tex("loam");
            f
        } else {
            let side = FaceTex {
                overlay: ctx.atlas.get("block/grass_block_side_overlay").layer,
                tint: Tint::Grass,
                ..ctx.tex("grass_block_side")
            };
            let mut f = all(side);
            f[up] = ctx.tex_tint("grass_block_top", Tint::Grass);
            f[down] = ctx.tex("loam");
            f
        }
    } else if name == "podzol" {
        let mut f = all(ctx.tex("podzol_side"));
        f[up] = if prop(reg, s, "snowy") == Some("true") {
            ctx.tex("snow")
        } else {
            ctx.tex("podzol_top")
        };
        f[down] = ctx.tex("loam");
        f
    } else if name == "tilled_soil" {
        let mut f = all(ctx.tex("tilled_soil"));
        f[up] = ctx.tex("tilled_soil_top");
        f
    } else if name == "burnt_ground" {
        let mut f = all(ctx.tex("burnt_ground_side"));
        f[up] = ctx.tex("burnt_ground_top");
        f[down] = ctx.tex("loam");
        f
    } else if name.ends_with("_log") {
        let axis = prop(reg, s, "axis").unwrap_or("y");
        axis_faces(ctx.tex(name), ctx.tex(&format!("{name}_top")), axis)
    } else if name.ends_with("_branch") {
        let bark = ctx.tex(&name.replace("_branch", "_log"));
        all(bark)
    } else if name.ends_with("_wood") {
        let log = name.replace("_wood", "_log");
        let axis = prop(reg, s, "axis").unwrap_or("y");
        axis_faces(ctx.tex(&log), ctx.tex(&log), axis)
    } else if name == "snow_block" {
        all(ctx.tex("snow"))
    } else {
        all(ctx.own(name))
    };
    CubeModel {
        faces,
        layer: block.def.layer,
        waving: name.ends_with("_leaves"),
    }
}

/// A tree's limb `thickness` px through: a bar along each axis it runs straight through,
/// arms to the faces it joins on one side only (one of them reaching across the middle when no
/// bar does), and no face drawn where another part covers it.
fn branch_quads(
    thickness: f32,
    joined: impl Fn(Direction) -> bool,
    tex: FaceTex,
    out: &mut Vec<ModelQuad>,
) {
    let (a, b) = (0.5 - thickness / 32.0, 0.5 + thickness / 32.0);
    // Per axis (x, y, z): the directions toward −1 and +1.
    let axes = [
        (Direction::West, Direction::East),
        (Direction::Down, Direction::Up),
        (Direction::North, Direction::South),
    ];
    let span = |axis: usize, lo: f32, hi: f32| {
        let mut mn = Vec3::splat(a);
        let mut mx = Vec3::splat(b);
        mn[axis] = lo;
        mx[axis] = hi;
        (mn, mx)
    };
    let mut boxes: Vec<(Vec3, Vec3)> = Vec::with_capacity(4);
    for (axis, (neg, pos)) in axes.iter().enumerate() {
        if joined(*neg) && joined(*pos) {
            boxes.push(span(axis, 0.0, 1.0));
        }
    }
    let mut middle = !boxes.is_empty();
    for (axis, (neg, pos)) in axes.iter().enumerate() {
        let (n, p) = (joined(*neg), joined(*pos));
        if n && !p {
            boxes.push(span(axis, 0.0, if middle { a } else { b }));
            middle = true;
        } else if p && !n {
            boxes.push(span(axis, if middle { b } else { a }, 1.0));
            middle = true;
        }
    }
    if !middle {
        boxes.push((Vec3::splat(a), Vec3::splat(b)));
    }
    let eps = 1e-4;
    for (i, (mn, mx)) in boxes.iter().enumerate() {
        for d in Direction::ALL {
            let (p, uv) = face_corners(*mn, *mx, d);
            // Hidden when the face lies wholly against another part.
            let n = d.normal();
            let covered = boxes.iter().enumerate().any(|(j, (on, ox))| {
                j != i
                    && p.iter().all(|c| {
                        let q = *c + n * eps;
                        (0..3).all(|k| q[k] >= on[k] - eps && q[k] <= ox[k] + eps)
                    })
            });
            if covered {
                continue;
            }
            let on_boundary = match d {
                Direction::Down => mn.y <= 0.0,
                Direction::Up => mx.y >= 1.0,
                Direction::North => mn.z <= 0.0,
                Direction::South => mx.z >= 1.0,
                Direction::West => mn.x <= 0.0,
                Direction::East => mx.x >= 1.0,
            };
            out.push(ModelQuad {
                pos: p,
                uv,
                tex,
                dir: Some(d),
                cull: on_boundary.then_some(d),
                shade: true,
                waving: false,
            });
        }
    }
}

/// Emits the six faces of an axis-aligned box (block-local 0..1 coordinates).
pub(crate) fn box_quads(
    min: Vec3,
    max: Vec3,
    tex: impl Fn(Direction) -> FaceTex,
    shade: bool,
    out: &mut Vec<ModelQuad>,
) {
    for d in Direction::ALL {
        let (p, uv) = face_corners(min, max, d);
        let on_boundary = match d {
            Direction::Down => min.y <= 0.0,
            Direction::Up => max.y >= 1.0,
            Direction::North => min.z <= 0.0,
            Direction::South => max.z >= 1.0,
            Direction::West => min.x <= 0.0,
            Direction::East => max.x >= 1.0,
        };
        out.push(ModelQuad {
            pos: p,
            uv,
            tex: tex(d),
            dir: Some(d),
            cull: on_boundary.then_some(d),
            shade,
            waving: false,
        });
    }
}

/// Corners (CCW from outside) and texel UVs of a box face, projected like the reference game's
/// automatic UVs.
pub fn face_corners(min: Vec3, max: Vec3, d: Direction) -> ([Vec3; 4], [Vec2; 4]) {
    let s = 16.0;
    let (a, b) = (min, max);
    match d {
        Direction::Up => (
            [
                Vec3::new(a.x, b.y, b.z),
                Vec3::new(b.x, b.y, b.z),
                Vec3::new(b.x, b.y, a.z),
                Vec3::new(a.x, b.y, a.z),
            ],
            [
                Vec2::new(a.x * s, b.z * s),
                Vec2::new(b.x * s, b.z * s),
                Vec2::new(b.x * s, a.z * s),
                Vec2::new(a.x * s, a.z * s),
            ],
        ),
        Direction::Down => (
            [
                Vec3::new(a.x, a.y, a.z),
                Vec3::new(b.x, a.y, a.z),
                Vec3::new(b.x, a.y, b.z),
                Vec3::new(a.x, a.y, b.z),
            ],
            [
                Vec2::new(a.x * s, (1.0 - a.z) * s),
                Vec2::new(b.x * s, (1.0 - a.z) * s),
                Vec2::new(b.x * s, (1.0 - b.z) * s),
                Vec2::new(a.x * s, (1.0 - b.z) * s),
            ],
        ),
        Direction::North => (
            [
                Vec3::new(b.x, a.y, a.z),
                Vec3::new(a.x, a.y, a.z),
                Vec3::new(a.x, b.y, a.z),
                Vec3::new(b.x, b.y, a.z),
            ],
            [
                Vec2::new((1.0 - b.x) * s, (1.0 - a.y) * s),
                Vec2::new((1.0 - a.x) * s, (1.0 - a.y) * s),
                Vec2::new((1.0 - a.x) * s, (1.0 - b.y) * s),
                Vec2::new((1.0 - b.x) * s, (1.0 - b.y) * s),
            ],
        ),
        Direction::South => (
            [
                Vec3::new(a.x, a.y, b.z),
                Vec3::new(b.x, a.y, b.z),
                Vec3::new(b.x, b.y, b.z),
                Vec3::new(a.x, b.y, b.z),
            ],
            [
                Vec2::new(a.x * s, (1.0 - a.y) * s),
                Vec2::new(b.x * s, (1.0 - a.y) * s),
                Vec2::new(b.x * s, (1.0 - b.y) * s),
                Vec2::new(a.x * s, (1.0 - b.y) * s),
            ],
        ),
        Direction::West => (
            [
                Vec3::new(a.x, a.y, a.z),
                Vec3::new(a.x, a.y, b.z),
                Vec3::new(a.x, b.y, b.z),
                Vec3::new(a.x, b.y, a.z),
            ],
            [
                Vec2::new(a.z * s, (1.0 - a.y) * s),
                Vec2::new(b.z * s, (1.0 - a.y) * s),
                Vec2::new(b.z * s, (1.0 - b.y) * s),
                Vec2::new(a.z * s, (1.0 - b.y) * s),
            ],
        ),
        Direction::East => (
            [
                Vec3::new(b.x, a.y, b.z),
                Vec3::new(b.x, a.y, a.z),
                Vec3::new(b.x, b.y, a.z),
                Vec3::new(b.x, b.y, b.z),
            ],
            [
                Vec2::new((1.0 - b.z) * s, (1.0 - a.y) * s),
                Vec2::new((1.0 - a.z) * s, (1.0 - a.y) * s),
                Vec2::new((1.0 - a.z) * s, (1.0 - b.y) * s),
                Vec2::new((1.0 - b.z) * s, (1.0 - b.y) * s),
            ],
        ),
    }
}

fn bake_model(
    name: &str,
    s: BlockStateId,
    reg: &BlockRegistry,
    ctx: &Ctx<'_>,
    block: &Block,
) -> StateModel {
    let mut quads = Vec::new();
    let mut layer = block.def.layer;
    let side_top_bottom = |side: FaceTex, top: FaceTex, bottom: FaceTex| {
        move |d: Direction| match d {
            Direction::Up => top,
            Direction::Down => bottom,
            _ => side,
        }
    };
    match name {
        "vine" => {
            let tex = ctx.tex_tint("vine", Tint::Foliage);
            let e = 0.8 / 16.0;
            for d in [
                Direction::North,
                Direction::East,
                Direction::South,
                Direction::West,
                Direction::Up,
            ] {
                if prop(reg, s, d.name()) != Some("true") {
                    continue;
                }
                // A plane just inside the face the vine clings to, visible from both sides.
                let (min, max) = match d {
                    Direction::North => (Vec3::new(0.0, 0.0, e), Vec3::new(1.0, 1.0, e)),
                    Direction::South => {
                        (Vec3::new(0.0, 0.0, 1.0 - e), Vec3::new(1.0, 1.0, 1.0 - e))
                    }
                    Direction::West => (Vec3::new(e, 0.0, 0.0), Vec3::new(e, 1.0, 1.0)),
                    Direction::East => (Vec3::new(1.0 - e, 0.0, 0.0), Vec3::new(1.0 - e, 1.0, 1.0)),
                    _ => (Vec3::new(0.0, 1.0 - e, 0.0), Vec3::new(1.0, 1.0 - e, 1.0)),
                };
                let face = if d == Direction::Up {
                    Direction::Down
                } else {
                    d.opposite()
                };
                let (p, uv) = face_corners(min, max, face);
                quads.push(ModelQuad {
                    pos: p,
                    uv,
                    tex,
                    dir: None,
                    cull: None,
                    shade: false,
                    waving: true,
                });
            }
            layer = RenderLayer::Cutout;
        }
        "lily_pad" => {
            let tex = ctx.tex_tint("lily_pad", Tint::Foliage);
            let (p, uv) = face_corners(
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.06, 1.0),
                Direction::Up,
            );
            quads.push(ModelQuad {
                pos: p,
                uv,
                tex,
                dir: Some(Direction::Up),
                cull: None,
                shade: true,
                waving: false,
            });
            layer = RenderLayer::Cutout;
        }
        // Leaves afloat on the water, in the block's own colours (a water lily, a lotus).
        _ if matches!(block.def.shape, hearth_world::ShapeKind::LilyPad) => {
            let tex = ctx.tex(block.name.path());
            let (p, uv) = face_corners(
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.06, 1.0),
                Direction::Up,
            );
            quads.push(ModelQuad {
                pos: p,
                uv,
                tex,
                dir: Some(Direction::Up),
                cull: None,
                shade: true,
                waving: false,
            });
            layer = RenderLayer::Cutout;
        }
        "sugar_cane" => {
            let tex = ctx.own(&cross_texture(name, s, reg));
            quads = cross(tex, true);
            layer = RenderLayer::Cutout;
        }
        n if n.ends_with("_branch") => {
            let t = prop(reg, s, "thickness")
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(4.0);
            let joined = |d: Direction| prop(reg, s, d.name()) == Some("true");
            let bark = ctx.tex(&n.replace("_branch", "_log"));
            branch_quads(t, joined, bark, &mut quads);
            layer = RenderLayer::Cutout;
        }
        "campfire" => {
            // A ring of stones; logs laid in it, or a mound of ash with coals; flames by how
            // well it burns.
            let px = |v: f32| v / 16.0;
            let stones = ctx.tex("campfire_stones");
            for [x0, z0, x1, z1, h] in [
                [1.0, 6.0, 4.0, 10.0, 3.0],
                [12.0, 6.0, 15.0, 10.0, 3.0],
                [6.0, 1.0, 10.0, 4.0, 3.0],
                [6.0, 12.0, 10.0, 15.0, 3.0],
                [2.0, 2.0, 5.0, 5.0, 2.0],
                [11.0, 11.0, 14.0, 14.0, 2.0],
                [11.0, 2.0, 14.0, 5.0, 2.0],
                [2.0, 11.0, 5.0, 14.0, 2.0],
            ] {
                box_quads(
                    Vec3::new(px(x0), 0.0, px(z0)),
                    Vec3::new(px(x1), px(h), px(z1)),
                    |_| stones,
                    true,
                    &mut quads,
                );
            }
            let fire = prop(reg, s, "fire").unwrap_or("out");
            if matches!(fire, "out" | "low" | "high") {
                let logs = ctx.tex("campfire_logs");
                for (a, b) in [
                    (
                        Vec3::new(px(4.0), px(0.5), px(7.0)),
                        Vec3::new(px(12.0), px(2.5), px(9.0)),
                    ),
                    (
                        Vec3::new(px(7.0), px(1.5), px(4.0)),
                        Vec3::new(px(9.0), px(3.5), px(12.0)),
                    ),
                ] {
                    box_quads(a, b, |_| logs, true, &mut quads);
                }
            } else {
                let ash = ctx.tex("campfire_ash");
                let top = if fire == "embers" {
                    ctx.tex("embers")
                } else {
                    ash
                };
                box_quads(
                    Vec3::new(px(5.0), 0.0, px(5.0)),
                    Vec3::new(px(11.0), px(1.5), px(11.0)),
                    move |d| if d == Direction::Up { top } else { ash },
                    true,
                    &mut quads,
                );
            }
            let flame_h = match fire {
                "low" => 0.55,
                "high" => 0.95,
                _ => 0.0,
            };
            if flame_h > 0.0 {
                quads.extend(cross_box(
                    ctx.tex("flames"),
                    Vec3::new(0.2, px(1.5), 0.2),
                    Vec3::new(0.8, px(1.5) + flame_h, 0.8),
                ));
            }
            layer = RenderLayer::Cutout;
        }
        "firing_pit" => {
            // A shallow pit ringed with dug earth: pots bedded in fuel, or ash and coals once it
            // has burned; flames by how it burns.
            let px = |v: f32| v / 16.0;
            let earth = ctx.tex("firing_pit_earth");
            for [x0, z0, x1, z1, h] in [
                [0.0, 0.0, 16.0, 3.0, 4.0],
                [0.0, 13.0, 16.0, 16.0, 4.0],
                [0.0, 3.0, 3.0, 13.0, 3.5],
                [13.0, 3.0, 16.0, 13.0, 3.5],
            ] {
                box_quads(
                    Vec3::new(px(x0), 0.0, px(z0)),
                    Vec3::new(px(x1), px(h), px(z1)),
                    |_| earth,
                    true,
                    &mut quads,
                );
            }
            let fire = prop(reg, s, "fire").unwrap_or("out");
            let pot = ctx.tex("earthenware");
            for [x0, z0] in [[4.0, 4.5], [8.5, 7.5]] {
                box_quads(
                    Vec3::new(px(x0), 0.0, px(z0)),
                    Vec3::new(px(x0 + 3.5), px(3.0), px(z0 + 3.5)),
                    |_| pot,
                    true,
                    &mut quads,
                );
            }
            if matches!(fire, "out" | "low" | "high") {
                let logs = ctx.tex("campfire_logs");
                for (a, b) in [
                    (
                        Vec3::new(px(3.0), px(2.5), px(5.0)),
                        Vec3::new(px(13.0), px(4.0), px(6.5)),
                    ),
                    (
                        Vec3::new(px(3.0), px(2.5), px(10.0)),
                        Vec3::new(px(13.0), px(4.0), px(11.5)),
                    ),
                ] {
                    box_quads(a, b, |_| logs, true, &mut quads);
                }
            } else {
                let ash = ctx.tex("campfire_ash");
                let top = if fire == "embers" {
                    ctx.tex("embers")
                } else {
                    ash
                };
                box_quads(
                    Vec3::new(px(3.0), 0.0, px(3.0)),
                    Vec3::new(px(13.0), px(2.0), px(13.0)),
                    move |d| if d == Direction::Up { top } else { ash },
                    true,
                    &mut quads,
                );
            }
            let flame_h = match fire {
                "low" => 0.5,
                "high" => 0.9,
                _ => 0.0,
            };
            if flame_h > 0.0 {
                quads.extend(cross_box(
                    ctx.tex("flames"),
                    Vec3::new(0.15, px(2.0), 0.15),
                    Vec3::new(0.85, px(2.0) + flame_h, 0.85),
                ));
            }
            layer = RenderLayer::Cutout;
        }
        "updraft_kiln" => {
            // A squat clay chamber narrowing to its flue, the firing mouth at its foot glowing
            // when it burns.
            let px = |v: f32| v / 16.0;
            let wall = ctx.tex("kiln_clay");
            for [x0, y0, z0, x1, y1, z1] in [
                [2.0, 0.0, 2.0, 14.0, 9.0, 14.0],
                [3.0, 9.0, 3.0, 13.0, 12.0, 13.0],
                [5.0, 12.0, 5.0, 11.0, 14.0, 11.0],
            ] {
                box_quads(
                    Vec3::new(px(x0), px(y0), px(z0)),
                    Vec3::new(px(x1), px(y1), px(z1)),
                    |_| wall,
                    true,
                    &mut quads,
                );
            }
            let fire = prop(reg, s, "fire").unwrap_or("out");
            let mouth = match fire {
                "low" | "high" | "embers" => ctx.tex("embers"),
                _ => ctx.tex("campfire_ash"),
            };
            box_quads(
                Vec3::new(px(6.0), px(0.5), px(1.6)),
                Vec3::new(px(10.0), px(4.0), px(2.0)),
                move |_| mouth,
                false,
                &mut quads,
            );
            if fire == "high" {
                quads.extend(cross_box(
                    ctx.tex("flames"),
                    Vec3::new(px(6.0), px(14.0), px(6.0)),
                    Vec3::new(px(10.0), px(18.0), px(10.0)),
                ));
            }
            layer = RenderLayer::Cutout;
        }
        "saddle_quern" => {
            // A long slab hollowed like a saddle, its rubbing stone lying in the hollow.
            let px = |v: f32| v / 16.0;
            let slab = ctx.tex("campfire_stones");
            for [x0, x1, h] in [[1.0, 4.0, 4.0], [4.0, 12.0, 2.5], [12.0, 15.0, 4.0]] {
                box_quads(
                    Vec3::new(px(x0), 0.0, px(3.0)),
                    Vec3::new(px(x1), px(h), px(13.0)),
                    |_| slab,
                    true,
                    &mut quads,
                );
            }
            box_quads(
                Vec3::new(px(6.0), px(2.5), px(5.0)),
                Vec3::new(px(9.5), px(5.0), px(11.0)),
                |_| slab,
                true,
                &mut quads,
            );
            layer = RenderLayer::Cutout;
        }
        "warp_weighted_loom" => {
            // Two uprights and the beam across their tops, the warp hanging from it in front,
            // a row of stone weights at its foot.
            let px = |v: f32| v / 16.0;
            let wood = ctx.tex("drying_rack");
            let warp = ctx.tex("warp_weighted_loom");
            let stone = ctx.tex("campfire_stones");
            for [x0, x1] in [[0.5, 2.0], [14.0, 15.5]] {
                box_quads(
                    Vec3::new(px(x0), 0.0, px(8.5)),
                    Vec3::new(px(x1), px(16.0), px(10.0)),
                    |_| wood,
                    true,
                    &mut quads,
                );
            }
            box_quads(
                Vec3::new(px(0.0), px(14.0), px(7.5)),
                Vec3::new(px(16.0), px(15.5), px(9.0)),
                |_| wood,
                true,
                &mut quads,
            );
            // The warp: a sheet of threads, and the cloth woven at its top.
            box_quads(
                Vec3::new(px(2.0), px(3.0), px(7.9)),
                Vec3::new(px(14.0), px(14.0), px(8.1)),
                |_| warp,
                false,
                &mut quads,
            );
            for k in 0..6 {
                let x = 2.5 + k as f32 * 2.0;
                box_quads(
                    Vec3::new(px(x), px(1.0), px(7.0)),
                    Vec3::new(px(x + 1.5), px(3.0), px(9.0)),
                    |_| stone,
                    true,
                    &mut quads,
                );
            }
            layer = RenderLayer::Cutout;
        }
        "potters_wheel" => {
            // A pivot stone, the wooden disc on it, a lump of clay centred on top.
            let px = |v: f32| v / 16.0;
            let stone = ctx.tex("campfire_stones");
            let wood = ctx.tex("drying_rack");
            let clay = ctx.tex("potters_wheel");
            box_quads(
                Vec3::new(px(5.0), 0.0, px(5.0)),
                Vec3::new(px(11.0), px(4.0), px(11.0)),
                |_| stone,
                true,
                &mut quads,
            );
            box_quads(
                Vec3::new(px(2.0), px(4.0), px(2.0)),
                Vec3::new(px(14.0), px(5.5), px(14.0)),
                |_| wood,
                true,
                &mut quads,
            );
            box_quads(
                Vec3::new(px(6.0), px(5.5), px(6.0)),
                Vec3::new(px(10.0), px(7.0), px(10.0)),
                |_| clay,
                true,
                &mut quads,
            );
            layer = RenderLayer::Cutout;
        }
        "tether_stake" => {
            // A stake driven into the ground, the tether's loop round its foot.
            let px = |v: f32| v / 16.0;
            let wood = ctx.tex("drying_rack");
            let rope = ctx.tex("tether_stake");
            box_quads(
                Vec3::new(px(7.0), 0.0, px(7.0)),
                Vec3::new(px(9.0), px(10.0), px(9.0)),
                |_| wood,
                true,
                &mut quads,
            );
            box_quads(
                Vec3::new(px(6.5), px(1.0), px(6.5)),
                Vec3::new(px(9.5), px(2.0), px(9.5)),
                |_| rope,
                true,
                &mut quads,
            );
            layer = RenderLayer::Cutout;
        }
        "storage_pit" => {
            // A pit's sealed mouth: a low dome of clay daubed over straw.
            let px = |v: f32| v / 16.0;
            let clay = ctx.tex("kiln_clay");
            box_quads(
                Vec3::new(px(1.0), 0.0, px(1.0)),
                Vec3::new(px(15.0), px(1.0), px(15.0)),
                |_| clay,
                true,
                &mut quads,
            );
            box_quads(
                Vec3::new(px(4.0), px(1.0), px(4.0)),
                Vec3::new(px(12.0), px(2.0), px(12.0)),
                |_| clay,
                true,
                &mut quads,
            );
            layer = RenderLayer::Cutout;
        }
        "fat_lamp" => {
            let px = |v: f32| v / 16.0;
            let stone = ctx.tex("fat_lamp");
            let side = ctx.tex("campfire_stones");
            box_quads(
                Vec3::new(px(5.0), 0.0, px(5.0)),
                Vec3::new(px(11.0), px(3.0), px(11.0)),
                move |d| if d == Direction::Up { stone } else { side },
                true,
                &mut quads,
            );
            if prop(reg, s, "lit") == Some("true") {
                quads.extend(cross_box(
                    ctx.tex("flames"),
                    Vec3::new(px(6.5), px(3.0), px(6.5)),
                    Vec3::new(px(9.5), px(8.0), px(9.5)),
                ));
            }
            layer = RenderLayer::Cutout;
        }
        _ if piece_look(ctx.atlas, name).is_some() && roof_of(reg, s).is_some() => {
            // A roof: the smooth slab its steps stand for.
            let (tex, look) = piece_look(ctx.atlas, name).expect("a piece");
            let (facing, t) = roof_of(reg, s).expect("a roof");
            layer = look;
            quads.extend(roof_slab(t, 0.0, 1.0, facing, tex));
        }
        _ => {
            // Box models from the state's outline shape.
            let faces: Box<dyn Fn(Direction) -> FaceTex> =
                if let Some((t, look)) = piece_look(ctx.atlas, name) {
                    // A construction piece wears its material's texture, or a look of its own.
                    layer = look;
                    Box::new(move |_| t)
                } else if name.ends_with("_slab") || name.ends_with("_stairs") {
                    let m = material(name);
                    if m == "sandstone" || m == "red_sandstone" {
                        Box::new(side_top_bottom(
                            ctx.tex(&m),
                            ctx.tex(&format!("{m}_top")),
                            ctx.tex(&format!("{m}_bottom")),
                        ))
                    } else {
                        let t = ctx.tex(&m);
                        Box::new(move |_| t)
                    }
                } else if name.ends_with("_fence") || name.ends_with("_fence_gate") {
                    let wood = name.split('_').next().unwrap_or("oak");
                    let t = ctx.tex(&format!("{wood}_planks"));
                    Box::new(move |_| t)
                } else if name.ends_with("_door") {
                    let half = if prop(reg, s, "half") == Some("upper") {
                        "top"
                    } else {
                        "bottom"
                    };
                    let t = ctx.tex(&format!("{name}_{half}"));
                    layer = RenderLayer::Cutout;
                    Box::new(move |_| t)
                } else if name.ends_with("_trapdoor") {
                    let t = ctx.tex(name);
                    layer = RenderLayer::Cutout;
                    Box::new(move |_| t)
                } else if name == "snow" {
                    let t = ctx.tex("snow");
                    Box::new(move |_| t)
                } else if name == "moss_carpet" {
                    let t = ctx.tex("moss_carpet");
                    Box::new(move |_| t)
                } else if name == "salt_crust" {
                    let t = ctx.tex("rock_salt");
                    Box::new(move |_| t)
                } else if name.ends_with("_branch") {
                    let t = ctx.tex(&name.replace("_branch", "_log"));
                    Box::new(move |_| t)
                } else if name == "cactus" {
                    layer = RenderLayer::Cutout;
                    Box::new(side_top_bottom(
                        ctx.tex("cactus_side"),
                        ctx.tex("cactus_top"),
                        ctx.tex("cactus_bottom"),
                    ))
                } else {
                    let t = ctx.own(name);
                    Box::new(move |_| t)
                };
            let shape = reg.outline_shape(s);
            for b in &shape.boxes {
                box_quads(b.min.as_vec3(), b.max.as_vec3(), &faces, true, &mut quads);
            }
        }
    }
    StateModel::Quads(quads.into(), layer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (BlockRegistry, TextureArray, BlockModels) {
        let reg = hearth_world::datapack::load_test_registry().unwrap();
        let atlas = TextureArray::from_entries(&hearth_texgen::textures_for(Some(
            &hearth_content::Content::load_base(),
        )));
        let models = BlockModels::build(&reg, &atlas);
        (reg, atlas, models)
    }

    #[test]
    fn every_state_has_a_model_with_known_textures() {
        // Game content only: the engine test pack's blocks have no textures on purpose.
        let reg = hearth_world::datapack::load_builtin_registry().unwrap();
        let atlas = TextureArray::from_entries(&hearth_texgen::textures_for(Some(
            &hearth_content::Content::load_base(),
        )));
        let models = BlockModels::build(&reg, &atlas);
        assert_eq!(models.len(), reg.state_count());
        let missing = atlas.get("block/missing").layer;
        let mut bad = Vec::new();
        for i in 0..reg.state_count() {
            let s = BlockStateId(i as u16);
            let name = reg.block_of(s).name.path().to_owned();
            if name == "unknown" {
                continue;
            }
            let uses_missing = match models.get(s) {
                StateModel::Cube(c) => c.faces.iter().any(|f| f.tex.layer == missing),
                StateModel::Quads(q, _) => q.iter().any(|q| q.tex.tex.layer == missing),
                _ => false,
            };
            if uses_missing {
                bad.push(reg.state_string(s));
            }
        }
        bad.dedup_by(|a, b| a.split('[').next() == b.split('[').next());
        assert!(bad.is_empty(), "states with missing textures: {bad:?}");
    }

    #[test]
    fn occlusion_and_shapes() {
        let (reg, _, models) = setup();
        let stone = reg.default_state("stone");
        assert!(Direction::ALL.iter().all(|d| models.occludes(stone, *d)));
        let glass = reg.default_state("glass");
        assert!(!models.occludes(glass, Direction::Up));
        let slab = reg.parse_state("oak_slab[type=bottom]").unwrap();
        assert!(models.occludes(slab, Direction::Down));
        assert!(!models.occludes(slab, Direction::Up));
        let log = reg.parse_state("oak_log[axis=x]").unwrap();
        match models.get(log) {
            StateModel::Cube(c) => {
                assert_eq!(
                    c.faces[Direction::East.index()].tex,
                    c.faces[Direction::West.index()].tex
                );
                assert_ne!(
                    c.faces[Direction::East.index()].tex,
                    c.faces[Direction::Up.index()].tex
                );
                assert_eq!(c.faces[Direction::Up.index()].rot, 1);
            }
            other => panic!("log should be a cube: {other:?}"),
        }
        assert!(matches!(
            models.get(reg.default_state("short_grass")),
            StateModel::Quads(..)
        ));
        assert!(matches!(
            models.get(reg.water_source()),
            StateModel::Fluid { .. }
        ));
        assert!(matches!(
            models.get(BlockStateId::AIR),
            StateModel::Invisible
        ));
    }
}

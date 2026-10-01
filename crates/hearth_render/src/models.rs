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
    /// Average (linear-ish sRGB) colour for LOD and particles.
    pub colors: Vec<[u8; 4]>,
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
        let mut colors = Vec::with_capacity(n);
        for i in 0..n {
            let s = BlockStateId(i as u16);
            let block = reg.block_of(s);
            let model = bake(block, s, reg, atlas);
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
            models.push(model);
            occludes.push(occ);
        }
        Self {
            models,
            occludes,
            colors,
        }
    }
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
    } else if name.ends_with("_log") {
        let axis = prop(reg, s, "axis").unwrap_or("y");
        axis_faces(ctx.tex(name), ctx.tex(&format!("{name}_top")), axis)
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

/// Emits the six faces of an axis-aligned box (block-local 0..1 coordinates).
fn box_quads(
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
        "sugar_cane" => {
            let tex = ctx.own(&cross_texture(name, s, reg));
            quads = cross(tex, true);
            layer = RenderLayer::Cutout;
        }
        _ => {
            // Box models from the state's outline shape.
            let faces: Box<dyn Fn(Direction) -> FaceTex> =
                if name.ends_with("_slab") || name.ends_with("_stairs") {
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

//! Building in the world (V2-8, docs/design/building.md), as client and server both see it:
//! where a piece put up goes — beside the face looked at, or in the place of what gives way to it
//! (grass, lying snow) — whether there is room for it there, and whether it rests on what is
//! about it (the stages: a frame before what hangs on it); and its ghost, drawn where it will go.

use glam::{Affine3A, DVec3, Vec3};
use hearth_character::FigureInstance;
use hearth_content::Content;
use hearth_content::building::{Bearing, piece_of, rests};
use hearth_content::schema::station::PieceShape;
use hearth_math::{BlockPos, Direction};
use hearth_protocol::AimAt;
use hearth_world::{BlockRegistry, BlockStateId, CubeMap};

/// Whether a block gives way to a piece put up in its place.
fn gives_way(reg: &BlockRegistry, s: BlockStateId) -> bool {
    s.is_air() || {
        let d = &reg.block_of(s).def;
        d.replaceable && d.fluid.is_none()
    }
}

/// Where a piece put up at an aim goes: in the place of what is looked at when it gives way,
/// else beside the face looked at (above, when it is the top).
pub fn spot(map: &CubeMap, reg: &BlockRegistry, aim: AimAt) -> Option<BlockPos> {
    let (pos, face) = match aim {
        AimAt::Block { pos, top: true } => (pos, Direction::Up),
        AimAt::Beside { pos, face } => (pos, face),
        _ => return None,
    };
    if gives_way(reg, map.block(pos)?) {
        return Some(pos);
    }
    Some(pos.offset(face))
}

/// Whether a piece can go at a place: nothing there but what gives way, and no one standing in
/// it (a body 0.6 m wide and 1.8 m tall at `feet`).
pub fn room(map: &CubeMap, reg: &BlockRegistry, at: BlockPos, feet: DVec3) -> bool {
    let free = map.block(at).is_some_and(|s| gives_way(reg, s));
    let (lo, hi) = (
        DVec3::new(at.x as f64, at.y as f64, at.z as f64),
        DVec3::new(at.x as f64 + 1.0, at.y as f64 + 1.0, at.z as f64 + 1.0),
    );
    let body = (
        feet - DVec3::new(0.3, 0.0, 0.3),
        feet + DVec3::new(0.3, 1.8, 0.3),
    );
    let inside = body.0.cmplt(hi).all() && body.1.cmpgt(lo).all();
    free && !inside
}

/// The piece a block is, if it is one.
pub fn piece_shape(content: &Content, reg: &BlockRegistry, s: BlockStateId) -> Option<PieceShape> {
    let name = &reg.block_of(s).name;
    let (piece, _) = piece_of(name.path())?;
    content
        .construction
        .get(&format!("{}:{piece}", name.namespace()))
        .map(|p| p.shape)
}

/// What is at a place, as a piece put up beside it bears on it.
pub fn bearing(map: &CubeMap, reg: &BlockRegistry, content: &Content, p: BlockPos) -> Bearing {
    let Some(s) = map.block(p).filter(|s| !s.is_air()) else {
        return Bearing::Nothing;
    };
    if let Some(shape) = piece_shape(content, reg, s) {
        return Bearing::Piece(shape);
    }
    if reg.collision_shape(s).is_empty() {
        Bearing::Nothing
    } else {
        Bearing::Ground
    }
}

/// Whether a piece of `shape` put up at `at` rests on what is about it.
pub fn rests_at(
    map: &CubeMap,
    reg: &BlockRegistry,
    content: &Content,
    shape: PieceShape,
    at: BlockPos,
) -> bool {
    let below = bearing(map, reg, content, at.down());
    let beside = [
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ]
    .map(|d| bearing(map, reg, content, at.offset(d)));
    rests(shape, below, beside)
}

/// The block a piece of a material is, facing the way the builder faces (radians: 0 toward +z,
/// turning toward +x).
pub fn piece_state(
    reg: &BlockRegistry,
    piece: &str,
    material: &str,
    facing: f32,
) -> Option<BlockStateId> {
    let id = hearth_content::building::piece_block_id(piece, material);
    let state = hearth_core::ResourceLocation::parse(&id)
        .ok()
        .and_then(|r| reg.default_state_of(&r))?;
    Some(
        reg.with(state, "facing", crate::workshop::facing_name(facing))
            .unwrap_or(state),
    )
}

/// The ghost of a piece where it will go: the edges of its boxes, pale where it would rest and
/// red where it would not, relative to the eye at `view`.
pub fn ghost(
    reg: &BlockRegistry,
    state: BlockStateId,
    at: BlockPos,
    rests: bool,
    view: DVec3,
) -> Vec<FigureInstance> {
    let color = if rests {
        [226, 232, 238]
    } else {
        [228, 84, 64]
    };
    let origin = DVec3::new(at.x as f64, at.y as f64, at.z as f64) - view;
    let thin = 0.012_f32;
    let mut out = Vec::new();
    for b in &reg.outline_shape(state).boxes {
        let (lo, hi) = (
            (origin + b.min).as_vec3() - Vec3::splat(thin * 0.5),
            (origin + b.max).as_vec3() + Vec3::splat(thin * 0.5),
        );
        let size = hi - lo;
        // The four edges along each axis.
        for axis in 0..3 {
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            for (su, sv) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                let mut c = lo;
                let mut s = Vec3::splat(thin);
                c[axis] = lo[axis] + size[axis] * 0.5;
                s[axis] = size[axis];
                c[u] = lo[u] + thin * 0.5 + su * (size[u] - thin);
                c[v] = lo[v] + thin * 0.5 + sv * (size[v] - thin);
                let place = Affine3A::from_scale_rotation_translation(
                    s,
                    glam::Quat::IDENTITY,
                    c - Vec3::Y * (s.y * 0.5),
                );
                out.push(hearth_character::solid(place, color, (15, 15)));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ghost_has_twelve_edges_to_a_box() {
        let reg = BlockRegistry::build(vec![(
            hearth_core::ResourceLocation::game("stone"),
            hearth_world::BlockDef::default(),
        )])
        .unwrap();
        let stone = reg.default_state("hearth:stone");
        let g = ghost(&reg, stone, BlockPos::new(0, 0, 0), true, DVec3::ZERO);
        assert_eq!(g.len(), 12);
    }
}

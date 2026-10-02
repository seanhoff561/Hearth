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

/// The share of the rain that drips through a covering laid flatter than it sheds rain at.
pub const LEAK: f32 = 0.4;

/// The share of the rain that comes through a tree's crown, however deep: leaves hold back a
/// fifth to a third of a steady rain and drip the rest (interception, Crockford & Richardson
/// 2000).
pub const CANOPY: f32 = 0.7;

/// What covers a place from the sky (V2-8 (d)): whether anything does (shade; the night sky
/// shut out) and the share of the rain that comes through — none through ground, a whole block
/// or a wall, or a roof at least as steep as its covering needs; [`LEAK`] through a roof laid
/// flatter than that or a flat covering, which drip (a good roof over a leaking one keeps the
/// rain off both); [`CANOPY`] through a tree's crown.
pub fn cover(
    map: &CubeMap,
    reg: &BlockRegistry,
    content: &Content,
    x: i32,
    z: i32,
    from_y: f64,
) -> (bool, f32) {
    let Some(top) = map.sky_top(x, z) else {
        return (false, 1.0);
    };
    if (top as f64) < from_y {
        return (false, 1.0);
    }
    let mut through = 1.0_f32;
    let mut covered = false;
    let mut crown = false;
    for y in (from_y.floor() as i32).max(top - 255)..=top {
        let Some(s) = map.block(BlockPos::new(x, y, z)).filter(|s| !s.is_air()) else {
            continue;
        };
        let piece = {
            let name = &reg.block_of(s).name;
            piece_of(name.path()).and_then(|(p, _)| {
                content
                    .construction
                    .get(&format!("{}:{p}", name.namespace()))
            })
        };
        match piece {
            Some(p) => match p.shape {
                PieceShape::Roof => {
                    covered = true;
                    let sheds = p
                        .sheds_rain_min_pitch_deg
                        .is_some_and(|min| p.pitch_deg.unwrap_or(45.0) >= min);
                    through *= if sheds { 0.0 } else { LEAK };
                }
                PieceShape::Layer => {
                    covered = true;
                    through *= LEAK;
                }
                PieceShape::Wall | PieceShape::Block => {
                    covered = true;
                    through = 0.0;
                }
                PieceShape::Post | PieceShape::Beam | PieceShape::Panel => {}
            },
            // Leaves shade, and drip most of a rain through (once for the crown).
            None if reg.block_of(s).name.path().ends_with("_leaves") => {
                covered = true;
                if !crown {
                    crown = true;
                    through *= CANOPY;
                }
            }
            None if reg.light_opacity(s) > 0 => {
                covered = true;
                through = 0.0;
            }
            None => {}
        }
        if through <= 0.0 {
            break;
        }
    }
    (covered, through)
}

/// How a place is sheltered (V2-8 (e)): how closed about it is on its sides and above (0 open
/// to 1 shut in), how closed its sides are (what the wind meets), and how readily heat goes
/// out through what closes it (W/m²·K, from the pieces' insulation; earth and rock hold heat
/// in well). The ground under it is not counted: it is under everyone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shelter {
    pub enclosure: f32,
    pub sides: f32,
    pub u_w_m2k: f32,
}

impl Shelter {
    /// The surface of a small hut's inside (m²).
    const AREA_M2: f32 = 40.0;

    /// How much warmer than outside fires of `fire_kw` within it keep the air (°C): their heat
    /// over what the walls and the openings let out (an opening loses as the warm air goes
    /// out of it, two hundred watts a square metre for each degree), at most twenty-five.
    pub fn warming_c(&self, fire_kw: f32) -> f32 {
        let e = self.enclosure.clamp(0.0, 1.0);
        let ua = Self::AREA_M2 * ((1.0 - e) * 200.0 + e * self.u_w_m2k);
        (fire_kw * 1000.0 / ua.max(1.0)).min(25.0)
    }

    /// The share of the wind that reaches a body inside: what its sides let through.
    pub fn wind_share(&self) -> f32 {
        1.0 - 0.9 * self.sides.clamp(0.0, 1.0)
    }
}

/// How sheltered the place at `eye` is: rays out in the seventeen directions level and up to
/// four metres,
/// each stopped (wholly or partly) by ground, a wall, a panel, a roof — brush lets the air
/// through, hides and wattle do not — or leaves (a little).
pub fn shelter(map: &CubeMap, reg: &BlockRegistry, content: &Content, eye: DVec3) -> Shelter {
    let (mut closed, mut sides, mut loss) = (0.0_f32, 0.0_f32, 0.0_f32);
    for dy in 0..=1 {
        for dz in -1..=1 {
            for dx in -1..=1 {
                if (dx, dy, dz) == (0, 0, 0) {
                    continue;
                }
                let dir = DVec3::new(dx as f64, dy as f64, dz as f64).normalize();
                let start = BlockPos::containing(eye);
                let mut hit: Option<(f32, f32)> = None;
                for k in 1..=16 {
                    let p = BlockPos::containing(eye + dir * (k as f64 * 0.25));
                    if p == start {
                        continue;
                    }
                    let Some(s) = map.block(p).filter(|s| !s.is_air()) else {
                        continue;
                    };
                    let name = &reg.block_of(s).name;
                    let piece = piece_of(name.path()).and_then(|(pc, _)| {
                        content
                            .construction
                            .get(&format!("{}:{pc}", name.namespace()))
                    });
                    hit = match piece {
                        Some(pc) => match pc.shape {
                            PieceShape::Post | PieceShape::Beam => None,
                            _ => Some((
                                (pc.fill * 3.0).clamp(0.2, 1.0),
                                pc.insulation_r.unwrap_or(0.1).max(0.02),
                            )),
                        },
                        None => {
                            let def = &reg.block_of(s).def;
                            if def.fluid.is_some() || reg.collision_shape(s).is_empty() {
                                None
                            } else if def.opaque {
                                // Earth and rock about: a metre of it holds heat well.
                                Some((1.0, 1.0))
                            } else {
                                Some((0.3, 0.05))
                            }
                        }
                    };
                    if hit.is_some() {
                        break;
                    }
                }
                if let Some((tight, r)) = hit {
                    closed += tight;
                    loss += tight / r;
                    if dy == 0 {
                        sides += tight;
                    }
                }
            }
        }
    }
    Shelter {
        enclosure: closed / 17.0,
        sides: sides / 8.0,
        u_w_m2k: if closed > 0.0 { loss / closed } else { 10.0 },
    }
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

/// The ghost's colour: pale where the piece would rest and stand, amber where it would stand
/// but hard pressed, red where it would rest on nothing or something would give way.
pub fn ghost_color(rests: bool, worst: f32) -> [u8; 3] {
    if !rests || worst > 1.0 {
        [228, 84, 64]
    } else if worst > 0.7 {
        [236, 186, 72]
    } else {
        [226, 232, 238]
    }
}

/// The builder's view's colour for a piece pressed `stress` (V2-8 (f)): blue at ease, through
/// green and yellow, to red at what it can bear.
pub fn stress_color(stress: f32) -> [u8; 3] {
    const STOPS: [(f32, [f32; 3]); 4] = [
        (0.0, [80.0, 140.0, 255.0]),
        (0.5, [100.0, 220.0, 110.0]),
        (0.8, [245.0, 215.0, 70.0]),
        (1.0, [235.0, 60.0, 45.0]),
    ];
    let s = stress.clamp(0.0, 1.0);
    let k = STOPS
        .iter()
        .rposition(|(at, _)| s >= *at)
        .unwrap_or(0)
        .min(2);
    let ((a, ca), (b, cb)) = (STOPS[k], STOPS[k + 1]);
    let t = ((s - a) / (b - a)).clamp(0.0, 1.0);
    [0, 1, 2].map(|i| (ca[i] + (cb[i] - ca[i]) * t) as u8)
}

/// The ghost of a piece where it will go: the edges of its boxes in `color`, relative to the
/// eye at `view`.
pub fn ghost(
    reg: &BlockRegistry,
    state: BlockStateId,
    at: BlockPos,
    color: [u8; 3],
    view: DVec3,
) -> Vec<FigureInstance> {
    edges(&reg.outline_shape(state).boxes, at, color, view, 0.012)
}

/// A piece's outline in the builder's view: the edges of the box about all of it.
pub fn outline(
    reg: &BlockRegistry,
    state: BlockStateId,
    at: BlockPos,
    color: [u8; 3],
    view: DVec3,
) -> Vec<FigureInstance> {
    let boxes = &reg.outline_shape(state).boxes;
    let Some(all) = boxes.iter().copied().reduce(|a, b| hearth_math::Aabb {
        min: a.min.min(b.min),
        max: a.max.max(b.max),
    }) else {
        return Vec::new();
    };
    edges(&[all], at, color, view, 0.03)
}

/// The edges of boxes (block-local) at a place, as bars `thin` thick (m).
fn edges(
    boxes: &[hearth_math::Aabb],
    at: BlockPos,
    color: [u8; 3],
    view: DVec3,
    thin: f32,
) -> Vec<FigureInstance> {
    let origin = DVec3::new(at.x as f64, at.y as f64, at.z as f64) - view;
    let mut out = Vec::new();
    for b in boxes {
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
    fn a_pitched_roof_sheds_the_rain_and_a_flat_covering_drips() {
        use std::sync::Arc;
        let reg = hearth_world::datapack::load_builtin_registry().expect("registry");
        let content = Content::load_base();
        let mut map = CubeMap::new(
            hearth_math::Planet::from_size(hearth_math::PlanetSize::Tiny).expect("planet"),
        );
        map.insert_cube(
            hearth_math::CubePos::new(0, 0, 0),
            Arc::new(hearth_world::Cube::filled(BlockStateId::AIR)),
            &reg,
        );
        let put = |map: &mut CubeMap, x, y, z, s: &str| {
            map.set_block(BlockPos::new(x, y, z), reg.parse_state(s).expect(s), &reg);
        };
        put(
            &mut map,
            2,
            5,
            2,
            "hearth:bark_roof/birch_bark[facing=north]",
        );
        put(&mut map, 6, 5, 6, "hearth:bark_cover/birch_bark");
        put(&mut map, 9, 5, 9, "hearth:post/hazel_wood");
        // A tree's crown, three leaves deep: shade, and most of the rain dripping through.
        for y in 5..8 {
            put(&mut map, 4, y, 12, "hearth:birch_leaves");
        }
        // A roof under the crown keeps it all off.
        put(
            &mut map,
            4,
            4,
            12,
            "hearth:bark_roof/birch_bark[facing=north]",
        );
        put(&mut map, 7, 7, 12, "hearth:birch_leaves");
        assert_eq!(cover(&map, &reg, &content, 2, 2, 3.0), (true, 0.0));
        assert_eq!(cover(&map, &reg, &content, 6, 6, 3.0), (true, LEAK));
        assert_eq!(cover(&map, &reg, &content, 9, 9, 3.0), (false, 1.0));
        assert_eq!(cover(&map, &reg, &content, 12, 12, 3.0), (false, 1.0));
        assert_eq!(cover(&map, &reg, &content, 7, 12, 3.0), (true, CANOPY));
        assert_eq!(cover(&map, &reg, &content, 4, 12, 3.0), (true, 0.0));
    }

    #[test]
    fn a_closed_hut_with_a_fire_is_warm_and_still() {
        use std::sync::Arc;
        let reg = hearth_world::datapack::load_builtin_registry().expect("registry");
        let content = Content::load_base();
        let mut map = CubeMap::new(
            hearth_math::Planet::from_size(hearth_math::PlanetSize::Tiny).expect("planet"),
        );
        map.insert_cube(
            hearth_math::CubePos::new(0, 0, 0),
            Arc::new(hearth_world::Cube::filled(BlockStateId::AIR)),
            &reg,
        );
        let eye = DVec3::new(8.5, 5.5, 8.5);
        let open = shelter(&map, &reg, &content, eye);
        assert!(
            open.enclosure < 0.01 && open.wind_share() > 0.99,
            "{open:?}"
        );
        assert!(open.warming_c(5.0) < 1.0);
        // Hides hung all round two blocks out, a bark roof over.
        let hide = |f: &str| format!("hearth:hide_wall/scraped_hide[facing={f}]");
        for y in 4..7 {
            for k in 6..=10 {
                for (x, z, f) in [
                    (k, 6, "south"),
                    (k, 10, "north"),
                    (6, k, "east"),
                    (10, k, "west"),
                ] {
                    let s = reg.parse_state(&hide(f)).expect("hide wall");
                    map.set_block(BlockPos::new(x, y, z), s, &reg);
                }
            }
        }
        let roof = reg
            .parse_state("hearth:bark_cover/birch_bark")
            .expect("roof");
        for z in 6..=10 {
            for x in 6..=10 {
                map.set_block(BlockPos::new(x, 7, z), roof, &reg);
            }
        }
        let hut = shelter(&map, &reg, &content, eye);
        assert!(hut.enclosure > 0.9 && hut.wind_share() < 0.2, "{hut:?}");
        // A small fire keeps it ten degrees and more above the cold outside.
        let warm = hut.warming_c(5.0);
        assert!((8.0..25.0).contains(&warm), "{warm} °C");
    }

    #[test]
    fn a_ghost_has_twelve_edges_to_a_box() {
        let reg = BlockRegistry::build(vec![(
            hearth_core::ResourceLocation::game("stone"),
            hearth_world::BlockDef::default(),
        )])
        .unwrap();
        let stone = reg.default_state("hearth:stone");
        let g = ghost(&reg, stone, BlockPos::new(0, 0, 0), [255; 3], DVec3::ZERO);
        assert_eq!(g.len(), 12);
        assert_eq!(stress_color(0.0), [80, 140, 255]);
        assert_eq!(stress_color(1.5), [235, 60, 45]);
    }
}

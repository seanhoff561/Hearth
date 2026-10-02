//! Structures standing and falling, on the server (V2-8 (b), (c), docs/design/building.md
//! "Stability", "Excavation"): the pieces about every change are reckoned
//! (`hearth_world::structure`), a bounded number a tick; what fails breaks and falls, half of
//! what it was made of lying where it fell; what it held is reckoned again, so a collapse
//! spreads as it would. Natural ground over an opening holds while the opening is no wider
//! than the ground roofs over (loose earth hardly at all, rock far) or while timbers or walls
//! under it take its weight; else it falls in, and the ground over it is over an opening next.

use std::collections::{BTreeMap, BTreeSet};

use hearth_content::Content;
use hearth_content::building::{Member, piece_of, self_span, take_down_id};
use hearth_content::schema::process::Match;
use hearth_math::{BlockPos, Direction};
use hearth_world::structure::{Cell, connected, reckon};
use hearth_world::{BlockRegistry, BlockStateId, CubeMap};

/// The most pieces reckoned in a tick (the rest wait for the next): a few milliseconds at
/// most; a hut is a hundred or two.
pub const BUDGET: usize = 8_192;

/// How far an opening is followed to find its width (blocks).
const WIDEST: i32 = 32;

/// How many blocks of the ground over a timbered opening bear on the timbers at most: the
/// ground arches over the rest.
const ARCHED: i32 = 8;

/// What fell this tick.
#[derive(Debug, Default)]
pub struct Fell {
    /// Pieces that gave way.
    pub pieces: Vec<BlockPos>,
    /// Natural ground that fell in.
    pub ground: Vec<BlockPos>,
}

/// The structures of the loaded world, as far as they have been reckoned.
pub struct Structures {
    /// The member each block (by id) is, if it is a piece.
    member_of: Vec<Option<u32>>,
    members: Vec<Member>,
    /// How wide an opening each natural block roofs over (m), and its weight (N), by id; none
    /// for what is not reckoned as ground (wood, leaves, plants).
    ground_of: Vec<Option<(f32, f32)>>,
    /// Places changed and not yet reckoned about.
    dirty: BTreeSet<BlockPos>,
    /// How hard each piece reckoned is pressed (over one, it fails).
    pub stress: BTreeMap<BlockPos, f32>,
}

impl Structures {
    pub fn new(reg: &BlockRegistry, content: &Content) -> Self {
        let mut member_of = vec![None; reg.block_count()];
        let mut members = Vec::new();
        for p in hearth_content::building::piece_blocks(
            &content.construction,
            &content.materials,
            &content.forms,
        ) {
            if let Some(id) = reg.block_id(&p.id) {
                member_of[id.0 as usize] = Some(members.len() as u32);
                members.push(p.member);
            }
        }
        let ground_of = reg
            .blocks()
            .map(|b| {
                if !b.def.collision || b.def.fluid.is_some() {
                    return None;
                }
                match b.def.material.as_deref() {
                    Some(m) => {
                        let m = content.materials.get(m)?;
                        Some((self_span(m)?, m.density_kg_m3 * 9.81))
                    }
                    // The surface's own blocks (turf, forest floor, burnt ground) say no
                    // material: earth bound by roots, a metre across; bare earth less. (Whole
                    // blocks only: leaves sound as grass too.)
                    None if !b.def.opaque => None,
                    None => match b.def.sound.as_str() {
                        "grass" | "moss" => Some((1.0, 1400.0 * 9.81)),
                        "soil" | "mud" => Some((0.5, 1400.0 * 9.81)),
                        "gravel" | "sand" => Some((0.0, 1700.0 * 9.81)),
                        _ => None,
                    },
                }
            })
            .collect();
        Structures {
            member_of,
            members,
            ground_of,
            dirty: BTreeSet::new(),
            stress: BTreeMap::new(),
        }
    }

    /// What the reckoning sees at a place: a piece, the ground that holds (or land not
    /// loaded), or nothing that does.
    pub fn cell(&self, map: &CubeMap, reg: &BlockRegistry, p: BlockPos) -> Cell {
        cell(&self.member_of, map, reg, p)
    }

    /// Whether a block is a piece.
    pub fn is_piece(&self, reg: &BlockRegistry, s: BlockStateId) -> bool {
        self.member_of
            .get(reg.block_id_of(s).0 as usize)
            .is_some_and(|m| m.is_some())
    }

    /// Notes blocks changed: the structures and the ground about them are reckoned again.
    pub fn changed(&mut self, at: &[BlockPos]) {
        self.dirty.extend(at.iter().copied());
    }

    /// Whether changes wait to be reckoned.
    pub fn busy(&self) -> bool {
        !self.dirty.is_empty()
    }

    /// The natural ground at a place, if it is reckoned as ground: how wide it roofs over (m)
    /// and its weight (N).
    fn ground(&self, map: &CubeMap, reg: &BlockRegistry, p: BlockPos) -> Option<(f32, f32)> {
        let s = map.block(p)?;
        if s.is_air() {
            return None;
        }
        *self.ground_of.get(reg.block_id_of(s).0 as usize)?
    }

    /// How wide the opening at `under` is: along each way the open places from it until
    /// ground or a piece (or land not loaded), the narrower of the two.
    fn width(&self, map: &CubeMap, reg: &BlockRegistry, under: BlockPos) -> i32 {
        let open = |p: BlockPos| cell(&self.member_of, map, reg, p) == Cell::Open;
        let run = |dx: i32, dz: i32| {
            let mut n = 1;
            for sign in [-1, 1] {
                let mut p = under;
                for _ in 0..WIDEST {
                    p = BlockPos::new(p.x + dx * sign, p.y, p.z + dz * sign);
                    if !open(p) {
                        break;
                    }
                    n += 1;
                }
            }
            n
        };
        run(1, 0).min(run(0, 1))
    }

    /// The weight of the ground bearing down on a piece at `p` from over it (N): the ground over
    /// it as deep as the opening is wide (at most [`ARCHED`] blocks), the rest arching over.
    fn pressing(&self, map: &CubeMap, reg: &BlockRegistry, p: BlockPos) -> f32 {
        // How wide the opening the piece stands in is, the pieces in it counting as open.
        let member_of = &self.member_of;
        let open = |q: BlockPos| cell(member_of, map, reg, q) != Cell::Ground;
        let run = |dx: i32, dz: i32| {
            let mut n = 1;
            for sign in [-1, 1] {
                let mut q = p;
                for _ in 0..WIDEST {
                    q = BlockPos::new(q.x + dx * sign, q.y, q.z + dz * sign);
                    if !open(q) {
                        break;
                    }
                    n += 1;
                }
            }
            n
        };
        let deep = run(1, 0).min(run(0, 1)).clamp(1, ARCHED);
        let mut w = 0.0;
        let mut q = p.up();
        for _ in 0..deep {
            match self.ground(map, reg, q) {
                Some((_, weight)) => w += weight,
                None => break,
            }
            q = q.up();
        }
        w
    }

    /// The natural ground that falls in over an opening at `p`: over the open places along
    /// each way through it, each ground block whose opening is wider than it roofs over.
    fn ground_falls(&self, map: &CubeMap, reg: &BlockRegistry, p: BlockPos) -> Vec<BlockPos> {
        let open = |q: BlockPos| cell(&self.member_of, map, reg, q) == Cell::Open;
        if !open(p) {
            return Vec::new();
        }
        let mut under = vec![p];
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let mut q = p;
            for _ in 0..WIDEST {
                q = BlockPos::new(q.x + dx, q.y, q.z + dz);
                if !open(q) {
                    break;
                }
                under.push(q);
            }
        }
        let mut out = Vec::new();
        for u in under {
            let roof = u.up();
            if let Some((span, _)) = self.ground(map, reg, roof)
                && self.width(map, reg, u) as f32 > span + 1e-3
            {
                out.push(roof);
            }
        }
        out
    }

    /// Reckons the structures and the ground about what changed, within the budget: what
    /// falls.
    pub fn tick(&mut self, map: &CubeMap, reg: &BlockRegistry) -> Fell {
        let mut dirty = std::mem::take(&mut self.dirty);
        let member_of = &self.member_of;
        let look = |p: BlockPos| cell(member_of, map, reg, p);
        let pressing = |p: BlockPos| self.pressing(map, reg, p);
        let mut spent = 0;
        let mut fell = Fell::default();
        let mut done: BTreeSet<BlockPos> = BTreeSet::new();
        let mut stress = Vec::new();
        let mut gone = Vec::new();
        while spent < BUDGET {
            let Some(seed) = dirty.pop_first() else {
                break;
            };
            // The ground over any opening the change made or widened.
            let falls = self.ground_falls(map, reg, seed);
            spent += 1 + falls.len();
            fell.ground.extend(falls);
            let pieces = connected(&look, &self.members, &[seed]);
            if pieces.iter().all(|p| done.contains(p)) {
                // Gone (taken down, fallen): it bears nothing now.
                gone.push(seed);
                continue;
            }
            spent += pieces.len().max(1);
            let r = reckon(&look, &pressing, &self.members, &pieces);
            stress.extend(r.stress);
            fell.pieces.extend(r.failed);
            done.extend(pieces);
        }
        self.dirty = dirty;
        for p in gone {
            self.stress.remove(&p);
        }
        self.stress.extend(stress);
        // What is no longer a piece is pressed by nothing.
        let member_of = &self.member_of;
        self.stress
            .retain(|p, _| matches!(cell(member_of, map, reg, *p), Cell::Piece(..)));
        for v in [&mut fell.pieces, &mut fell.ground] {
            v.sort();
            v.dedup();
        }
        fell
    }
}

fn cell(member_of: &[Option<u32>], map: &CubeMap, reg: &BlockRegistry, p: BlockPos) -> Cell {
    let Some(s) = map.block(p) else {
        return Cell::Ground;
    };
    if s.is_air() {
        return Cell::Open;
    }
    let id = reg.block_id_of(s);
    if let Some(Some(m)) = member_of.get(id.0 as usize) {
        return Cell::Piece(*m, reg.get_dir(s, "facing").unwrap_or(Direction::North));
    }
    let def = &reg.block_of(s).def;
    if def.fluid.is_some() || reg.collision_shape(s).is_empty() {
        Cell::Open
    } else {
        Cell::Ground
    }
}

/// What lies where a piece fell: half of what taking it down would give, in its material.
pub fn debris(content: &Content, block: &str, material: Option<&str>) -> Vec<(String, u16)> {
    let Some((piece, _)) = piece_of(block) else {
        return Vec::new();
    };
    let ns = block.split_once(':').map_or("hearth", |(n, _)| n);
    let Some(p) = content
        .processes
        .get(&take_down_id(&format!("{ns}:{piece}")))
    else {
        return Vec::new();
    };
    let material = material.map(|m| m.split_once(':').map_or(m, |(_, p)| p));
    let mut out = Vec::new();
    for o in &p.outputs {
        let n = (o.amount.0 * 0.5).round() as u16;
        if n == 0 {
            continue;
        }
        let id = match (&o.item, material) {
            (Match::Form { form, .. }, Some(m)) => {
                let (fns, f) = form
                    .as_str()
                    .split_once(':')
                    .unwrap_or(("hearth", form.as_str()));
                format!("{fns}:{f}/{m}")
            }
            (Match::Item(i), _) => i.as_str().to_owned(),
            _ => continue,
        };
        out.push((id, n));
    }
    out
}

/// What natural ground becomes when it falls in: rock breaks to its loose stones where it has
/// them, everything else to loose earth.
pub fn fallen(reg: &BlockRegistry, s: BlockStateId) -> BlockStateId {
    let b = reg.block_of(s);
    let cobbles = format!("{}:{}_cobbles", b.name.namespace(), b.name.path());
    let loose = reg.block_of(s).def.material.as_deref().and_then(|m| {
        let path = m.split_once(':').map_or(m, |(_, p)| p);
        reg.default_state_of(
            &hearth_core::ResourceLocation::parse(&format!("hearth:{path}_cobbles")).ok()?,
        )
    });
    hearth_core::ResourceLocation::parse(&cobbles)
        .ok()
        .and_then(|r| reg.default_state_of(&r))
        .or(loose)
        .or_else(|| reg.parse_state("hearth:spoil").ok())
        .unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use hearth_math::{CubePos, PlanetSize};
    use hearth_world::Cube;

    use super::*;

    struct Fixture {
        reg: BlockRegistry,
        map: CubeMap,
        s: Structures,
    }

    /// A cube of `ground` below y = 12 with air over it.
    fn fixture(ground: &str) -> Fixture {
        let reg = hearth_world::datapack::load_builtin_registry().expect("registry");
        let content = Content::load_base();
        let s = Structures::new(&reg, &content);
        let mut map =
            CubeMap::new(hearth_math::Planet::from_size(PlanetSize::Tiny).expect("planet"));
        let g = reg.parse_state(ground).expect(ground);
        let mut cube = Cube::filled(g);
        for y in 12..16 {
            for z in 0..16 {
                for x in 0..16 {
                    cube.set(hearth_math::LocalPos::new(x, y, z), BlockStateId::AIR);
                }
            }
        }
        map.insert_cube(CubePos::new(0, 0, 0), Arc::new(cube), &reg);
        Fixture { reg, map, s }
    }

    impl Fixture {
        fn set(&mut self, x: i32, y: i32, z: i32, state: &str) {
            let s = self.reg.parse_state(state).expect(state);
            let p = BlockPos::new(x, y, z);
            self.map.set_block(p, s, &self.reg);
            self.s.changed(&[p]);
        }
        /// A tunnel along x from 3 to 12, `wide` blocks across from z = 6, two high from y = 6.
        fn tunnel(&mut self, wide: i32) {
            for x in 3..13 {
                for z in 6..6 + wide {
                    for y in 6..8 {
                        self.set(x, y, z, "hearth:air");
                    }
                }
            }
        }
        fn tick(&mut self) -> Fell {
            self.s.tick(&self.map, &self.reg)
        }
    }

    #[test]
    fn clay_roofs_a_narrow_tunnel_but_not_a_wide_one() {
        let mut f = fixture("hearth:earthenware_clay");
        f.tunnel(1);
        assert!(f.tick().ground.is_empty());
        let mut f = fixture("hearth:earthenware_clay");
        f.tunnel(2);
        let fell = f.tick();
        assert!(
            fell.ground.contains(&BlockPos::new(5, 8, 6)),
            "{:?}",
            fell.ground
        );
        assert!(fell.ground.iter().all(|p| p.y == 8), "{:?}", fell.ground);
    }

    #[test]
    fn granite_roofs_a_hall_and_loose_earth_nothing() {
        let mut f = fixture("hearth:granite");
        f.tunnel(6);
        assert!(f.tick().ground.is_empty());
        let mut f = fixture("hearth:loam");
        f.tunnel(1);
        assert!(!f.tick().ground.is_empty());
    }

    #[test]
    fn posts_under_a_roof_bear_the_ground_over_it() {
        // Three wide in clay, with hazel posts in the side rows: they hold the roof over them
        // but buckle under three blocks of clay each.
        let mut f = fixture("hearth:earthenware_clay");
        f.tunnel(3);
        for x in [4, 8] {
            for z in [6, 8] {
                for y in 6..8 {
                    f.set(x, y, z, "hearth:post/hazel_wood");
                }
            }
        }
        let fell = f.tick();
        assert!(
            fell.pieces.contains(&BlockPos::new(4, 7, 6)),
            "{:?}",
            fell.pieces
        );
    }
}

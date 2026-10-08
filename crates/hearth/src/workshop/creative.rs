//! Creative's acts on the world (Amendment P §3.1): what its inventory takes, places, plants,
//! summons, builds and teaches, and what its remove tool takes away — at once, where the player
//! looks (or just before them). Persons are summoned and removed by the people's side
//! (`PeopleNear::summons`, `PeopleNear::remove`), never a child.

use glam::DVec3;
use hearth_items::Stack;
use hearth_math::BlockPos;
use hearth_protocol::{AimAt, CreativeAct};
use hearth_world::BlockStateId;

use super::{Here, Workshop, acted};

/// How far before the player a thing goes when nothing is looked at (m).
const AHEAD_M: f64 = 2.0;

impl Workshop {
    /// Does what Creative's inventory asks. Persons are the people's (see the module's words).
    pub(crate) fn creative(&mut self, h: &mut Here, act: &CreativeAct, aim: AimAt) {
        match act {
            CreativeAct::Take { item, count } => self.creative_take(h, item, *count),
            CreativeAct::Place { block } => {
                let Ok(state) = h.lw.reg.parse_state(block) else {
                    return;
                };
                if let Some(at) = self.creative_spot(h, aim) {
                    self.set_block(h, at, state);
                }
            }
            CreativeAct::Plant { species, young } => {
                if let Some(at) = self.creative_spot(h, aim) {
                    self.creative_plant(h, species, *young, at);
                }
            }
            CreativeAct::Summon {
                species,
                female,
                young,
                count,
            } => {
                let centre = creative_ahead(h, aim);
                for k in 0..(*count).clamp(1, 32) {
                    // A herd stands about the place, a pace apart.
                    let a = k as f64 * 2.4;
                    let r = if k == 0 { 0.0 } else { 1.2 + 0.35 * k as f64 };
                    let at = centre + DVec3::new(a.cos() * r, 0.5, a.sin() * r);
                    let at = crate::server::rest_on(h.lw, at);
                    h.fauna.bring(species, *young, *female, at);
                }
            }
            CreativeAct::Build { piece } => {
                let Some(at) = self.creative_spot(h, aim) else {
                    return;
                };
                if h.lw.content.workstations.get(piece.as_str()).is_some() {
                    self.build(h, piece, at, &[]);
                } else if let Some((p, m)) = hearth_content::building::piece_of(piece) {
                    let (ns, _) = piece.split_once(':').unwrap_or(("hearth", piece));
                    let state =
                        crate::building::piece_state(&h.lw.reg, &format!("{ns}:{p}"), m, h.facing);
                    if let Some(state) = state {
                        self.set_block(h, at, state);
                    }
                }
            }
            CreativeAct::Learn { node, known } => {
                if self.graph.nodes.iter().all(|n| n.id != *node) {
                    return;
                }
                if *known {
                    h.player.knowledge.known.insert(
                        node.clone(),
                        hearth_craft::knowledge::Learned {
                            tick: h.ticks,
                            route: None,
                        },
                    );
                } else {
                    h.player.knowledge.known.remove(node);
                }
                self.knowledge_changed = true;
            }
        }
    }

    /// Items of a kind into the hands, as many as asked; what they cannot hold beside the
    /// player.
    fn creative_take(&mut self, h: &mut Here, item: &str, count: u32) {
        let Some(kind) = h.items.get(item) else {
            return;
        };
        let body_kg = h.cfg.mass_kg as f32;
        let stack = Stack::of(item, count.clamp(1, 64) as u16);
        let rest = match h.player.carry.stow(h.items, stack, body_kg) {
            Ok(()) => None,
            Err(s) => h.player.carry.drag(h.items, s, body_kg).err(),
        };
        if let Some((s, _)) = rest {
            let at = crate::server::rest_on(h.lw, h.player.mover.pos);
            h.world_items.add(s, at.to_array(), 0.0);
            *h.items_changed = true;
        }
        h.out.push(acted(
            "",
            false,
            format!("You take {}.", kind.name.to_lowercase()),
        ));
    }

    /// The block a thing placed goes in: beside what is looked at, or on the ground just before
    /// the player; none where something solid already is or the player stands.
    fn creative_spot(&self, h: &Here, aim: AimAt) -> Option<BlockPos> {
        let at = crate::building::spot(&h.lw.map, &h.lw.reg, aim).or_else(|| {
            let p = creative_ahead(h, aim);
            Some(BlockPos::new(
                p.x.floor() as i32,
                (p.y + 0.01).floor() as i32,
                p.z.floor() as i32,
            ))
        })?;
        // Creative puts its thing in place of anything a body walks through (a plant too), not
        // where something solid stands or the player is.
        let walk_through =
            h.lw.map
                .block(at)
                .is_some_and(|s| !h.lw.reg.has(s, hearth_world::StateFlags::HAS_COLLISION));
        let body = h.player.mover.bounds();
        let cell_lo = DVec3::new(at.x as f64, at.y as f64, at.z as f64);
        let in_body = body.min.cmplt(cell_lo + DVec3::ONE).all() && body.max.cmpgt(cell_lo).all();
        (walk_through && !in_body).then_some(at)
    }

    /// A plant of a species where `at` is (its foot): a crop sown or ripe, an herb or a shrub
    /// of the understory, or a tree young (a sapling) or grown.
    fn creative_plant(&mut self, h: &mut Here, species: &str, young: bool, at: BlockPos) {
        let wg = h.lw.generator.clone();
        let bare = |id: &str| id.rsplit(':').next().unwrap_or(id).to_owned();
        if let Some(crop) = self.crops.get(species).cloned() {
            // Sown, or at the last stage its block has.
            let state = if young {
                h.lw.reg
                    .parse_state(&format!("{}[stage=0]", crop.block))
                    .ok()
            } else {
                (0..16).rev().find_map(|st| {
                    h.lw.reg
                        .parse_state(&format!("{}[stage={st}]", crop.block))
                        .ok()
                })
            };
            if let Some(state) = state {
                self.set_block(h, at, state);
            }
            return;
        }
        if let Some(u) = wg
            .forest
            .understory
            .iter()
            .find(|u| bare(&u.id) == bare(species))
        {
            self.set_block(h, at, u.lower);
            if let Some(upper) = u.upper
                && Self::open(h, at.up())
            {
                self.set_block(h, at.up(), upper);
            }
            return;
        }
        let Some(si) = wg.forest.templates.index_of(species) else {
            return;
        };
        let stage = if young {
            hearth_flora::Stage::Sapling
        } else {
            hearth_flora::Stage::Mature
        };
        let seed = (at.x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (at.z as u64);
        let tree = hearth_worldgen::cubegen::features::PlacedTree {
            species: si,
            stage,
            template: wg.forest.templates.get(si, stage, (seed % 4) as u8),
            turn: hearth_flora::Turn::from_hash(seed),
            foot: [at.x, at.y, at.z],
            remains: hearth_worldgen::cubegen::features::Remains::Living,
            understory: false,
        };
        let blocks: Vec<(BlockPos, BlockStateId)> = tree
            .blocks()
            .map(|(p, part)| (p, tree.state(&wg.forest, part)))
            .collect();
        for (p, s) in blocks {
            if Self::open(h, p) {
                self.set_block(h, p, s);
            }
        }
    }

    /// Creative's remove tool on what is looked at: a thing lying, an animal, or a block — a
    /// standing tree whole, a plant two blocks tall both halves. Whether anything was taken.
    pub(crate) fn creative_remove(&mut self, h: &mut Here, aim: AimAt) -> bool {
        match aim {
            AimAt::Thing(id) => {
                let taken = h.world_items.take(id).is_some();
                *h.items_changed |= taken;
                taken
            }
            AimAt::Animal(id) => {
                let before = h.fauna.live.animals.len();
                h.fauna.live.animals.retain(|a| a.id != id);
                h.fauna.live.animals.len() < before
            }
            AimAt::Block { pos, .. } | AimAt::Beside { pos, .. } => {
                if let Some(t) = self.standing_tree(h, pos) {
                    let all: Vec<BlockPos> = t.blocks().map(|(p, _)| p).collect();
                    for p in all {
                        self.set_block(h, p, BlockStateId::AIR);
                    }
                    return true;
                }
                let Some(s) = h.lw.map.block(pos) else {
                    return false;
                };
                if s.is_air() {
                    return false;
                }
                // The other half of a tall plant goes with it.
                let block = h.lw.reg.block_of(s).name.clone();
                for q in [pos.up(), pos.down()] {
                    if h.lw
                        .map
                        .block(q)
                        .is_some_and(|o| !o.is_air() && h.lw.reg.block_of(o).name == block)
                        && !h.lw.reg.has(s, hearth_world::StateFlags::HAS_COLLISION)
                    {
                        self.set_block(h, q, BlockStateId::AIR);
                    }
                }
                self.set_block(h, pos, BlockStateId::AIR);
                true
            }
            AimAt::Nothing => false,
        }
    }
}

/// Where a thing summoned or placed goes when nothing is looked at: on the ground a couple of
/// paces before the player; where a block is looked at, over it.
fn creative_ahead(h: &Here, aim: AimAt) -> DVec3 {
    if let Some(p) = aim.block() {
        return DVec3::new(p.x as f64 + 0.5, p.y as f64 + 1.0, p.z as f64 + 0.5);
    }
    let f = h.facing as f64;
    let at = h.player.mover.pos + DVec3::new(f.sin(), 0.0, f.cos()) * AHEAD_M;
    crate::server::rest_on(h.lw, at + DVec3::new(0.0, 1.0, 0.0))
}

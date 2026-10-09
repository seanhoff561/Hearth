//! Digging the smooth ground (Amendment S §8.3–8.4): a dig takes a cubic metre stroke by stroke
//! from where the player looks at it, the spoil piles beside the hole and loose ground slumps to
//! its angle of repose, dry or wet; the volume dug is the volume piled. Every voxel changed is
//! kept as the player's change, its block and its fill.

use glam::DVec3;
use hearth_math::BlockPos;
use hearth_world::ground;
use hearth_world::{BlockRegistry, BlockStateId, CubeMap, StateFlags};

use super::{Here, Workshop};

/// What a dig process moves in all (m³): the block its time is for.
pub(super) const DIG_M3: f64 = 1.0;

/// Whether a natural state is loose ground: dug earth (spoil), or a material that falls (sand,
/// gravel, ash) or snow.
fn loose(content: &hearth_content::Content, reg: &BlockRegistry, s: BlockStateId) -> bool {
    let b = reg.block_of(s);
    b.name.path() == "spoil"
        || b.def
            .material
            .as_deref()
            .and_then(|m| content.materials.get(m))
            .is_some_and(|m| {
                m.tags.iter().any(|t| t == "falls")
                    || m.category == hearth_content::schema::material::MaterialCategory::Snow
            })
}

/// The ground family of a natural state: its material's, or soil's for a ground block that
/// names none (grass, podzol, moss over earth).
fn family<'a>(
    content: &'a hearth_content::Content,
    reg: &BlockRegistry,
    s: BlockStateId,
) -> Option<&'a hearth_content::schema::material::GroundFamily> {
    if !reg.has(s, StateFlags::NATURAL) {
        return None;
    }
    match reg.block_of(s).def.material.as_deref() {
        Some(m) => content.ground_of(m),
        None => content.reference.ground.iter().find(|g| g.id == "soil"),
    }
}

impl Workshop {
    /// Changes the ground by `edit` within `lo..=hi`, keeping each voxel it changed as the
    /// player's change and for meshing.
    fn ground_edit<R>(
        &mut self,
        h: &mut Here,
        lo: BlockPos,
        hi: BlockPos,
        edit: impl FnOnce(&mut CubeMap, &BlockRegistry) -> R,
    ) -> R {
        let reg = h.lw.reg.clone();
        let snap = |map: &CubeMap| {
            let mut v = Vec::new();
            for y in lo.y..=hi.y {
                for z in lo.z..=hi.z {
                    for x in lo.x..=hi.x {
                        let p = BlockPos::new(x, y, z);
                        v.push((p, map.block(p), map.fill(p, &reg)));
                    }
                }
            }
            v
        };
        let before = snap(&h.lw.map);
        let out = edit(&mut h.lw.map, &reg);
        for (p, s0, f0) in before {
            let (s1, f1) = (h.lw.map.block(p), h.lw.map.fill(p, &reg));
            if s1 != s0
                && let Some(s) = s1
            {
                h.lw.edits.set(p, s);
                if s.is_air() {
                    self.stations.retain(|st| st.pos != p);
                }
            }
            if f1 != f0
                && let Some(q) = f1
            {
                h.lw.edits.set_fill(p, q);
            }
            if s1 != s0 || f1 != f0 {
                h.changed.push(p);
            }
        }
        out
    }

    /// Digs `volume` m³ of the ground at `pos` (the block looked at), where the player's look
    /// meets it (at the point the look rested on when the dig began, its preview's patch, T
    /// §2.3); the spoil goes on a heap beside the hole, away from the player (snow packs away),
    /// and loose ground about slumps. Returns the volume dug.
    pub(super) fn dig_ground(&mut self, h: &mut Here, pos: BlockPos, volume: f64) -> f64 {
        if volume <= 0.0 {
            return 0.0;
        }
        let reg = h.lw.reg.clone();
        let eye = h.player.mover.pos + DVec3::Y * 1.6;
        // The point told, if it is on the block aimed at (or about it); else its middle.
        let aim = self
            .dig_point
            .filter(|p| (*p - super::center(pos)).length() < 1.5)
            .unwrap_or_else(|| super::center(pos));
        let look = (aim - eye).normalize_or(DVec3::NEG_Y);
        let hit = ground::raycast(&h.lw.map, &reg, eye, look, 6.0)
            .or_else(|| ground::raycast(&h.lw.map, &reg, aim + DVec3::Y * 2.0, DVec3::NEG_Y, 4.0));
        let Some(hit) = hit else {
            return 0.0;
        };
        let content = h.lw.content.clone();
        let diggable = |s: BlockStateId| family(&content, &reg, s).is_some_and(|g| g.slumps());
        let r = ground::DIG_RADIUS_M;
        let lo = BlockPos::containing(hit.at - DVec3::splat(2.0));
        let hi = BlockPos::containing(hit.at + DVec3::splat(2.0));
        let taken = self.ground_edit(h, lo, hi, |map, reg| {
            ground::dig(map, reg, hit.at, hit.normal, r, volume as f32, &diggable)
        });
        let dug: f32 = taken.iter().map(|(_, v)| v).sum();
        // The spoil: loose ground as it was, the rest as spoil; snow is packed away.
        let spoil = reg.parse_state("hearth:spoil").ok();
        let feet = h.player.mover.pos;
        let mut away = DVec3::new(hit.at.x - feet.x, 0.0, hit.at.z - feet.z);
        if away.length_squared() < 1e-6 {
            away = DVec3::X;
        }
        // Thrown a stride clear of the hole's lip, so it does not run back in.
        let heap = hit.at + away.normalize() * 2.0;
        for (s, v) in &taken {
            if family(&content, &reg, *s).is_some_and(|g| g.id == "snow") {
                continue;
            }
            let into = if loose(&content, &reg, *s) {
                Some(*s)
            } else {
                spoil
            };
            if let Some(into) = into {
                // As far as a pile may reach (`ground::pile`), and a voxel beyond.
                let lo = BlockPos::containing(heap - DVec3::new(7.0, 7.0, 7.0));
                let hi = BlockPos::containing(heap + DVec3::new(7.0, 4.0 + *v as f64, 7.0));
                let put = self.ground_edit(h, lo, hi, |map, reg| {
                    ground::pile(map, reg, heap, 0.8, into, *v)
                });
                if put < *v - 1e-3 {
                    log::warn!("{} m³ of spoil found no room about {heap:?}", v - put);
                }
            }
        }
        self.settle_ground(h, BlockPos::containing(hit.at), 4);
        dug as f64
    }

    /// Levels the ground for building (S §6): the three-metre square about the column aimed at
    /// cut down to its middle height and its hollows filled from what is cut; what is left over
    /// goes on a heap beside it, away from the player. Returns the volume moved (m³).
    pub(super) fn level_ground(&mut self, h: &mut Here, pos: BlockPos) -> f64 {
        let reg = h.lw.reg.clone();
        let content = h.lw.content.clone();
        let diggable = |s: BlockStateId| family(&content, &reg, s).is_some_and(|g| g.slumps());
        // The surface over the square's columns, and their middle height.
        let mut heights = Vec::new();
        for dz in -1..=1 {
            for dx in -1..=1 {
                let top = DVec3::new(
                    (pos.x + dx) as f64 + 0.5,
                    pos.y as f64 + 4.0,
                    (pos.z + dz) as f64 + 0.5,
                );
                if let Some(hit) = ground::raycast(&h.lw.map, &reg, top, DVec3::NEG_Y, 9.0) {
                    heights.push(hit.at.y);
                }
            }
        }
        if heights.is_empty() {
            return 0.0;
        }
        let level = heights.iter().sum::<f64>() / heights.len() as f64;
        let before = ground::volume(
            &h.lw.map,
            &reg,
            BlockPos::new(pos.x - 1, level.floor() as i32 - 4, pos.z - 1),
            BlockPos::new(pos.x + 1, level.ceil() as i32 + 4, pos.z + 1),
        );
        let lo = BlockPos::new(pos.x - 1, level.floor() as i32 - 4, pos.z - 1);
        let hi = BlockPos::new(pos.x + 1, level.ceil() as i32 + 4, pos.z + 1);
        let left = self.ground_edit(h, lo, hi, |map, reg| {
            ground::level(
                map,
                reg,
                (pos.x - 1, pos.z - 1),
                (pos.x + 1, pos.z + 1),
                level,
                &diggable,
            )
        });
        // The rest on a heap a stride beyond the square, away from the player.
        let middle = DVec3::new(pos.x as f64 + 0.5, level, pos.z as f64 + 0.5);
        let feet = h.player.mover.pos;
        let mut away = DVec3::new(middle.x - feet.x, 0.0, middle.z - feet.z);
        if away.length_squared() < 1e-6 {
            away = DVec3::X;
        }
        let heap = middle + away.normalize() * 3.0;
        for (s, v) in &left {
            let lo = BlockPos::containing(heap - DVec3::new(7.0, 7.0, 7.0));
            let hi = BlockPos::containing(heap + DVec3::new(7.0, 4.0 + *v as f64, 7.0));
            self.ground_edit(h, lo, hi, |map, reg| {
                ground::pile(map, reg, heap, 0.8, *s, *v)
            });
        }
        self.settle_ground(h, BlockPos::containing(heap), 3);
        let after = ground::volume(&h.lw.map, &reg, lo, hi);
        let total = |t: &ground::Taken| t.iter().map(|(_, v)| *v as f64).sum::<f64>();
        (total(&before) - total(&after)).abs() + total(&left)
    }

    /// Lets loose ground about `at` slump to its angle of repose, wet in rain.
    pub(super) fn settle_ground(&mut self, h: &mut Here, at: BlockPos, radius: i32) {
        let reg = h.lw.reg.clone();
        let content = h.lw.content.clone();
        let wet = h.env.weather_at(&h.moment, super::center(at)).precip_mm_h > 0.1;
        // Only loose ground flows: spoil, sand, gravel, ash, snow. Intact earth stands in a pit's
        // wall (it is cohesive and rooted) until it is dug.
        let repose = |s: BlockStateId| {
            if !loose(&content, &reg, s) {
                return None;
            }
            let (dry, rain) = family(&content, &reg, s)?.repose_deg?;
            Some(if wet { rain } else { dry })
        };
        let lo = BlockPos::new(at.x - radius - 1, at.y - 13, at.z - radius - 1);
        let hi = BlockPos::new(at.x + radius + 1, at.y + 13, at.z + radius + 1);
        self.ground_edit(h, lo, hi, |map, reg| {
            ground::settle(map, reg, at, radius, &repose, 4000)
        });
    }
}

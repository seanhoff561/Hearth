//! Ore veins and rock-variety blobs. Veins are seeded per cube and may extend a few blocks into
//! neighbours; each cube evaluates the veins of its 27-neighbourhood, so results are identical
//! regardless of generation order.

use hearth_math::hash::{Rng, derive_seed, hash_3d};
use hearth_math::{CUBE_SIZE, CubePos};
use hearth_world::BlockStateId;

use super::CubeBuf;
use super::blocks::GenBlocks;
use crate::planet::province;
use crate::region::Terrain;

/// What a vein places.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VeinKind {
    Coal,
    Iron,
    Gold,
    Diamond,
    Granite,
    Diorite,
    Andesite,
    Tuff,
    Dirt,
    Gravel,
    Calcite,
}

/// Maximum reach of a vein from its seeding cube's boundary.
const MAX_VEIN_REACH: i32 = 6;

/// Ore generation for one world.
#[derive(Debug, Clone)]
pub struct OreGen {
    seed: u64,
    /// Scale of the Y bands relative to the Standard planet (vertical scale / 0.25, clamped).
    scale: f32,
}

#[inline]
fn tri(y: f32, lo: f32, peak: f32, hi: f32) -> f32 {
    if y <= lo || y >= hi {
        0.0
    } else if y <= peak {
        (y - lo) / (peak - lo)
    } else {
        (hi - y) / (hi - peak)
    }
}

impl OreGen {
    pub fn new(seed: u64, vertical_scale: f32) -> Self {
        Self {
            seed: derive_seed(seed, "ores"),
            scale: (vertical_scale / 0.25).clamp(0.5, 2.0),
        }
    }

    /// Expected veins per cube of each kind at world Y (cube centre), with province bias.
    fn frequencies(&self, y: f32, depth: f32, prov: u8) -> [(VeinKind, f32, (u32, u32)); 11] {
        let s = self.scale;
        let yy = y / s; // Standard-equivalent Y
        let shield = prov == province::SHIELD;
        let orogen = prov == province::OROGEN || prov == province::OLD_OROGEN;
        let arc = prov == province::ARC;
        let basin = prov == province::BASIN;
        let coal = if yy > -200.0 { 1.0 } else { 0.0 }
            * (1.0 - ((-yy - 120.0) / 80.0).clamp(0.0, 1.0))
            * 1.6
            * if basin || prov == province::OLD_OROGEN {
                1.4
            } else {
                1.0
            };
        let iron = (tri(yy, -300.0, -120.0, 20.0) * 1.1 + if yy > 200.0 { 0.8 } else { 0.0 })
            * if shield { 1.4 } else { 1.0 };
        let gold = tri(yy, -600.0, -300.0, -60.0) * 0.55 * if orogen || arc { 1.8 } else { 1.0 };
        let diamond = if yy < -300.0 {
            (0.03 + 0.25 * ((-yy - 300.0) / 900.0).clamp(0.0, 1.0)) * if shield { 1.3 } else { 1.0 }
        } else {
            0.0
        };
        let near_surface = if depth < 80.0 { 1.0 } else { 0.25 };
        [
            (VeinKind::Coal, coal, (7, 17)),
            (VeinKind::Iron, iron, (4, 10)),
            (VeinKind::Gold, gold, (4, 9)),
            (VeinKind::Diamond, diamond, (2, 7)),
            (
                VeinKind::Granite,
                0.28 * if shield || orogen { 2.0 } else { 1.0 },
                (24, 48),
            ),
            (VeinKind::Diorite, 0.22, (24, 44)),
            (
                VeinKind::Andesite,
                0.26 * if arc { 2.2 } else { 1.0 },
                (24, 48),
            ),
            (
                VeinKind::Tuff,
                if yy < 0.0 { 0.12 } else { 0.03 } * if arc { 4.0 } else { 1.0 },
                (20, 40),
            ),
            (VeinKind::Dirt, 0.3 * near_surface, (16, 32)),
            (VeinKind::Gravel, 0.22 * near_surface, (16, 32)),
            (VeinKind::Calcite, 0.03, (10, 20)),
        ]
    }

    /// Places all veins intersecting the cube in `buf`.
    pub fn apply(&self, buf: &mut CubeBuf, pos: CubePos, terrain: &Terrain, b: &GenBlocks) {
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let seed_cube = CubePos::new(pos.x + dx, pos.y + dy, pos.z + dz);
                    self.veins_of(seed_cube, terrain, &mut |kind, cx, cy, cz, size, rng| {
                        self.place_vein(buf, kind, cx, cy, cz, size, rng, b);
                    });
                }
            }
        }
    }

    /// Emits the veins seeded in `cube`. Depends only on the cube's own position (and the pure
    /// planet model), so every neighbour sees the same veins.
    fn veins_of(
        &self,
        cube: CubePos,
        terrain: &Terrain,
        emit: &mut dyn FnMut(VeinKind, f32, f32, f32, u32, &mut Rng),
    ) {
        let min = cube.min_block();
        let grid = &terrain.grid;
        let wx = terrain.planet().wrap_xf(min.x as f64 + 8.0);
        let wz = min.z as f64 + 8.0;
        let prov = grid.province[grid.cell_at(wx, wz)];
        let (gx, gz) = grid.geom.grid_coords(wx, wz);
        let surface = grid.elevation.bilinear(gx, gz) * terrain.vertical_scale();
        let y_mid = (min.y + 8) as f32;
        let depth = surface - y_mid;
        let mut rng = Rng::new(hash_3d(self.seed, cube.x, cube.y, cube.z));
        for (kind, freq, (smin, smax)) in self.frequencies(y_mid, depth, prov) {
            if freq <= 0.0 {
                // Keep the RNG stream stable across kinds.
                rng.next_u64();
                continue;
            }
            let whole = freq.floor() as u32;
            let count = whole + u32::from(rng.next_f32() < freq - whole as f32);
            for _ in 0..count {
                let cx = min.x as f32 + rng.range_f32(0.0, CUBE_SIZE as f32);
                let cy = min.y as f32 + rng.range_f32(0.0, CUBE_SIZE as f32);
                let cz = min.z as f32 + rng.range_f32(0.0, CUBE_SIZE as f32);
                let size = rng.range_i32(smin as i32, smax as i32) as u32;
                let mut vein_rng = Rng::new(rng.next_u64());
                emit(kind, cx, cy, cz, size, &mut vein_rng);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_vein(
        &self,
        buf: &mut CubeBuf,
        kind: VeinKind,
        cx: f32,
        cy: f32,
        cz: f32,
        size: u32,
        rng: &mut Rng,
        b: &GenBlocks,
    ) {
        // Quick reject: the vein's reach doesn't touch the cube.
        let o = buf.origin;
        let reach = MAX_VEIN_REACH as f32;
        if cx + reach < o.x as f32
            || cx - reach > (o.x + 16) as f32
            || cy + reach < o.y as f32
            || cy - reach > (o.y + 16) as f32
            || cz + reach < o.z as f32
            || cz - reach > (o.z + 16) as f32
        {
            return;
        }
        // A vein is a short line of overlapping blobs (like the reference game's ore veins).
        let len = (size as f32 / 5.0).clamp(1.0, 4.0);
        let yaw = rng.range_f32(0.0, std::f32::consts::TAU);
        let pitch = rng.range_f32(-0.6, 0.6);
        let (dx, dy, dz) = (
            yaw.cos() * pitch.cos(),
            pitch.sin(),
            yaw.sin() * pitch.cos(),
        );
        let radius = (0.55 + (size as f32).cbrt() * 0.45).min(3.0);
        let steps = 3;
        let deep = |y: i32, current: BlockStateId| -> usize {
            usize::from(current == b.deepslate || y < super::DEEPSLATE_FLOOR)
        };
        for k in 0..steps {
            let t = (k as f32 / (steps - 1) as f32 - 0.5) * len;
            let px = cx + dx * t;
            let py = cy + dy * t;
            let pz = cz + dz * t;
            let r = radius * (0.8 + 0.4 * rng.next_f32());
            let r2 = r * r;
            let (x0, x1) = ((px - r).floor() as i32, (px + r).ceil() as i32);
            let (y0, y1) = ((py - r).floor() as i32, (py + r).ceil() as i32);
            let (z0, z1) = ((pz - r).floor() as i32, (pz + r).ceil() as i32);
            for y in y0..=y1 {
                for z in z0..=z1 {
                    for x in x0..=x1 {
                        let (fx, fy, fz) = (
                            x as f32 + 0.5 - px,
                            y as f32 + 0.5 - py,
                            z as f32 + 0.5 - pz,
                        );
                        if fx * fx + fy * fy + fz * fz > r2 {
                            continue;
                        }
                        let Some(i) = buf.idx(x, y, z) else { continue };
                        let cur = buf.states[i];
                        if !b.is_base_rock(cur) {
                            continue;
                        }
                        let d = deep(y, cur);
                        let new = match kind {
                            VeinKind::Coal => b.coal_ore[d],
                            VeinKind::Iron => b.iron_ore[d],
                            VeinKind::Gold => b.gold_ore[d],
                            VeinKind::Diamond => b.diamond_ore[d],
                            VeinKind::Granite => b.granite,
                            VeinKind::Diorite => b.diorite,
                            VeinKind::Andesite => b.andesite,
                            VeinKind::Tuff => b.tuff,
                            VeinKind::Dirt => b.dirt,
                            VeinKind::Gravel => b.gravel,
                            VeinKind::Calcite => b.calcite,
                        };
                        // Rock blobs don't overwrite ores already placed and stay out of the
                        // deepslate (except tuff, which lives down there).
                        let is_ore = matches!(
                            kind,
                            VeinKind::Coal | VeinKind::Iron | VeinKind::Gold | VeinKind::Diamond
                        );
                        if !is_ore && d == 1 && kind != VeinKind::Tuff {
                            continue;
                        }
                        buf.states[i] = new;
                    }
                }
            }
        }
    }
}

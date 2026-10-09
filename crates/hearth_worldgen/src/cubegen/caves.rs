//! Caves with explicit bounding volumes, so cubes far from any cave skip them entirely:
//! * worm cave systems (winding tunnels and small chambers) on a 128³ region grid — sparser
//!   than the reference game, richer in humid (karst) regions and fractured mountain roots,
//!   continuing far below Y −500 with decreasing frequency;
//! * rare ravines (tall narrow worms), which open to the sky as slot canyons in arid regions;
//! * rare giant caverns on a 1024-block grid, with pillars, underground lakes and sometimes a
//!   sinkhole to the surface.
//!
//! Systems are pure functions of their region and cached. A region is keyed by its place around
//! the planet (the same region from either side of the seam: every planet's circumference is a
//! multiple of both grids) and its caves are laid out about its first block in x and z, exact in
//! f32 anywhere on the planet (E4.1 §4.5).

use std::sync::Arc;

use hearth_math::hash::{Rng, derive_seed, hash_2d, hash_3d};
use hearth_math::{CUBE_SIZE, CubePos};

use super::blocks::GenBlocks;
use super::cache::Cache;
use super::{ColumnData, CubeBuf, WorldGenerator};
use crate::noise::Perlin;
use crate::planet::province;

const REGION: i32 = 128;
const CAVERN_REGION: i32 = 1024;
/// Worms never travel farther than this from their region's bounds.
const WORM_REACH: i32 = 112;

/// A sphere of a worm: x and z about its region's first block, y as it is.
#[derive(Debug, Clone, Copy)]
struct Sphere {
    x: f32,
    y: f32,
    z: f32,
    /// Horizontal radius.
    r: f32,
    /// Vertical radius.
    ry: f32,
}

/// A worm: its spheres and their bounds (as theirs: x and z about the region's first block).
#[derive(Debug, Clone, Default)]
struct Worm {
    spheres: Vec<Sphere>,
    min: [i32; 3],
    max: [i32; 3],
}

impl Worm {
    fn finish(&mut self) {
        let mut min = [i32::MAX; 3];
        let mut max = [i32::MIN; 3];
        for s in &self.spheres {
            let r = s.r.max(s.ry).ceil() as i32 + 1;
            min[0] = min[0].min(s.x as i32 - r);
            min[1] = min[1].min(s.y as i32 - (s.ry.ceil() as i32 + 1));
            min[2] = min[2].min(s.z as i32 - r);
            max[0] = max[0].max(s.x as i32 + r);
            max[1] = max[1].max(s.y as i32 + s.ry.ceil() as i32 + 1);
            max[2] = max[2].max(s.z as i32 + r);
        }
        self.min = min;
        self.max = max;
    }
}

/// A connected cave system.
#[derive(Debug, Clone, Default)]
pub struct CaveSystem {
    worms: Vec<Worm>,
    /// Carved blocks below this Y become water: the water table where the system starts.
    flood_level: Option<i32>,
}

/// A giant cavern: x and z (its centre, sinkhole and bounds) about its region's first block, y
/// as it is.
#[derive(Debug, Clone)]
pub struct Cavern {
    cx: f32,
    cy: f32,
    cz: f32,
    rx: f32,
    ry: f32,
    rz: f32,
    lake_level: i32,
    sinkhole: Option<(f32, f32, f32, i32)>, // x, z, radius, top y
    min: [i32; 3],
    max: [i32; 3],
    seed: u64,
}

/// Cave generator.
pub struct CaveGen {
    seed: u64,
    /// The planet's circumference (blocks).
    c: i32,
    systems: Cache<(i32, i32, i32), Vec<CaveSystem>>,
    caverns: Cache<(i32, i32), Option<Cavern>>,
    noise: Perlin,
    cavern_frequency: f64,
    vertical_scale: f32,
}

/// A noise's frequency (per block) for about `wavelength` blocks, a whole number of waves around
/// a planet of circumference `c`, and that number (its period in lattice cells).
fn wave(c: i32, wavelength: f64) -> (f64, i64) {
    let cells = ((c as f64 / wavelength).round() as i64).max(1);
    (cells as f64 / c as f64, cells)
}

impl CaveGen {
    /// The caves of a planet of circumference `c` (blocks).
    pub fn new(seed: u64, vertical_scale: f32, c: i32) -> Self {
        Self {
            seed: derive_seed(seed, "caves"),
            c,
            systems: Cache::new(4096),
            caverns: Cache::new(1024),
            noise: Perlin::new(derive_seed(seed, "cavern-noise")),
            cavern_frequency: 1.0,
            vertical_scale,
        }
    }

    /// Multiplier on giant-cavern frequency (Rare = 1).
    pub fn set_cavern_frequency(&mut self, f: f64) {
        self.cavern_frequency = f;
    }

    /// The systems of a region (its index around the planet).
    fn region_systems(
        &self,
        rx: i32,
        ry: i32,
        rz: i32,
        wg: &WorldGenerator,
    ) -> Arc<Vec<CaveSystem>> {
        self.systems
            .get_or_insert_with((rx, ry, rz), || self.generate_region(rx, ry, rz, wg))
    }

    fn generate_region(&self, rx: i32, ry: i32, rz: i32, wg: &WorldGenerator) -> Vec<CaveSystem> {
        let terrain = &*wg.terrain;
        let mut rng = Rng::new(hash_3d(self.seed, rx, ry, rz));
        // The region's first block in x and z (about which its caves are laid out).
        let (x0, z0) = (rx * REGION, rz * REGION);
        let cx = x0 + REGION / 2;
        let cz = z0 + REGION / 2;
        let cy = ry * REGION + REGION / 2;
        // Local conditions from the pure surface model at the region centre.
        let s = terrain.sample(cx, cz);
        let surface = s.height;
        let depth = surface - cy as f32;
        if depth < -40.0 {
            return Vec::new(); // entirely above ground
        }
        let humid = (s.precipitation / 1100.0).clamp(0.5, 1.6);
        let fractured = if s.province == province::OROGEN || s.province == province::OLD_OROGEN {
            1.4
        } else {
            1.0
        };
        let underwater = s.is_underwater();
        let depth_factor = if depth > 500.0 {
            (-(depth - 500.0) / 1500.0).exp().max(0.12)
        } else {
            1.0
        };
        let p = 0.34 * humid * fractured * depth_factor * if underwater { 0.5 } else { 1.0 };
        let count = u32::from(rng.chance(p as f64)) + u32::from(rng.chance((p * 0.35) as f64));
        let mut systems = Vec::new();
        for _ in 0..count {
            let mut sys = CaveSystem::default();
            let start = (
                rng.range_f32(8.0, (REGION - 8) as f32),
                (ry * REGION) as f32 + rng.range_f32(8.0, (REGION - 8) as f32),
                rng.range_f32(8.0, (REGION - 8) as f32),
            );
            let worms = 1 + rng.below(3);
            for _ in 0..worms {
                let mut w = Worm::default();
                let ravine = rng.chance(0.035);
                self.worm(&mut rng, start, ravine, &mut w, 0);
                w.finish();
                if !w.spheres.is_empty() {
                    sys.worms.push(w);
                }
            }
            // Below the water table every void is full of water.
            let table = wg
                .hydro
                .water_table(wg, x0 + start.0 as i32, z0 + start.2 as i32);
            sys.flood_level = Some(table.floor() as i32);
            systems.push(sys);
        }
        // Slot canyons: arid regions get rare ravines starting at the surface.
        if s.precipitation < 450.0
            && !underwater
            && (surface as i32).div_euclid(REGION) == ry
            && rng.chance(0.05)
        {
            let mut w = Worm::default();
            let start = ((REGION / 2) as f32, surface - 6.0, (REGION / 2) as f32);
            self.worm(&mut rng, start, true, &mut w, 0);
            w.finish();
            let table = wg.hydro.water_table(wg, cx, cz);
            systems.push(CaveSystem {
                worms: vec![w],
                flood_level: Some(table.floor() as i32),
            });
        }
        systems
    }

    /// Traces one worm (and occasional branches), appending spheres.
    fn worm(
        &self,
        rng: &mut Rng,
        start: (f32, f32, f32),
        ravine: bool,
        out: &mut Worm,
        depth: u32,
    ) {
        let (mut x, mut y, mut z) = start;
        let (ox, oy, oz) = start;
        let mut yaw = rng.range_f32(0.0, std::f32::consts::TAU);
        let mut pitch = rng.range_f32(-0.4, 0.4);
        let mut dyaw = 0.0f32;
        let mut dpitch = 0.0f32;
        let len = if ravine {
            rng.range_i32(90, 150)
        } else {
            rng.range_i32(60, 190)
        };
        let base_r = if ravine {
            rng.range_f32(1.8, 3.5)
        } else {
            rng.range_f32(1.3, 3.0)
        };
        let ravine_h = rng.range_f32(4.0, 8.0);
        // Chambers at the start of some worms.
        if !ravine && depth == 0 && rng.chance(0.25) {
            let r = rng.range_f32(4.0, 8.0);
            out.spheres.push(Sphere {
                x,
                y,
                z,
                r,
                ry: r * 0.6,
            });
        }
        let branch_at = if depth == 0 && rng.chance(0.4) {
            rng.range_i32(len / 4, len * 3 / 4)
        } else {
            -1
        };
        for i in 0..len {
            let t = i as f32 / len as f32;
            let r = base_r * (0.6 + 0.8 * (t * std::f32::consts::PI).sin());
            let (sy, cy) = pitch.sin_cos();
            x += yaw.cos() * cy;
            y += sy;
            z += yaw.sin() * cy;
            if (x - ox).abs() > WORM_REACH as f32
                || (y - oy).abs() > WORM_REACH as f32
                || (z - oz).abs() > WORM_REACH as f32
            {
                break;
            }
            if ravine {
                pitch *= 0.7;
                dyaw = dyaw * 0.8 + (rng.next_f32() - rng.next_f32()) * 0.05;
            } else {
                pitch *= 0.9;
                dyaw = dyaw * 0.75 + (rng.next_f32() - rng.next_f32()) * rng.next_f32() * 0.35;
                dpitch = dpitch * 0.9 + (rng.next_f32() - rng.next_f32()) * rng.next_f32() * 0.12;
            }
            yaw += dyaw;
            pitch = (pitch + dpitch).clamp(-1.0, 1.0);
            // Only every other step is stored (spheres overlap heavily anyway).
            if i % 2 == 0 {
                out.spheres.push(Sphere {
                    x,
                    y,
                    z,
                    r,
                    ry: if ravine { r * ravine_h } else { r * 0.85 },
                });
            }
            if i == branch_at {
                self.worm(rng, (x, y, z), false, out, depth + 1);
            }
        }
    }

    /// True if the 1024-block cavern region (rx, rz) holds a giant cavern.
    pub fn has_cavern(&self, rx: i32, rz: i32, wg: &WorldGenerator) -> bool {
        self.cavern(rx, rz, wg).is_some()
    }

    /// The cavern of a cavern region (its index around the planet, as `rx` may be from either
    /// side of the seam).
    fn cavern(&self, rx: i32, rz: i32, wg: &WorldGenerator) -> Arc<Option<Cavern>> {
        let rx = rx.rem_euclid((self.c / CAVERN_REGION).max(1));
        self.caverns
            .get_or_insert_with((rx, rz), || self.generate_cavern(rx, rz, wg))
    }

    fn generate_cavern(&self, rx: i32, rz: i32, wg: &WorldGenerator) -> Option<Cavern> {
        let terrain = &*wg.terrain;
        let h = hash_2d(self.seed ^ 0xca7e, rx, rz);
        let mut rng = Rng::new(h);
        // Roughly one per several square kilometres at Rare.
        if !rng.chance((0.28 * self.cavern_frequency).min(0.95)) {
            return None;
        }
        let margin = 260.0;
        // About the region's first block in x and z.
        let (x0, z0) = (rx * CAVERN_REGION, rz * CAVERN_REGION);
        let cx = rng.range_f32(margin, CAVERN_REGION as f32 - margin);
        let cz = rng.range_f32(margin, CAVERN_REGION as f32 - margin);
        let rxr = rng.range_f32(80.0, 210.0);
        let rzr = rng.range_f32(80.0, 210.0);
        let ryr = rng.range_f32(50.0, 140.0);
        let s = terrain.sample(x0 + cx as i32, z0 + cz as i32);
        let underwater = s.is_underwater();
        let cover = if underwater {
            rng.range_f32(80.0, 200.0)
        } else {
            rng.range_f32(30.0, 160.0)
        };
        let cy = s.height - cover - ryr;
        // An underground lake, or the water table if that stands higher.
        let lake_level = ((cy - ryr * rng.range_f32(0.45, 0.75)) as i32).max(
            wg.hydro
                .water_table(wg, x0 + cx as i32, z0 + cz as i32)
                .floor() as i32,
        );
        let sinkhole = if !underwater && rng.chance(0.35) {
            let a = rng.range_f32(0.0, std::f32::consts::TAU);
            let d = rng.range_f32(0.0, 0.5);
            let sx = cx + a.cos() * rxr * d;
            let sz = cz + a.sin() * rzr * d;
            let top = terrain.sample(x0 + sx as i32, z0 + sz as i32).height as i32 + 6;
            Some((sx, sz, rng.range_f32(7.0, 15.0), top))
        } else {
            None
        };
        let pad = 1.35;
        let mut min = [
            (cx - rxr * pad) as i32,
            (cy - ryr * pad) as i32,
            (cz - rzr * pad) as i32,
        ];
        let mut max = [
            (cx + rxr * pad) as i32,
            (cy + ryr * pad) as i32,
            (cz + rzr * pad) as i32,
        ];
        if let Some((sx, sz, r, top)) = sinkhole {
            min[0] = min[0].min((sx - r * 1.5) as i32);
            min[2] = min[2].min((sz - r * 1.5) as i32);
            max[0] = max[0].max((sx + r * 1.5) as i32);
            max[2] = max[2].max((sz + r * 1.5) as i32);
            max[1] = max[1].max(top);
        }
        Some(Cavern {
            cx,
            cy,
            cz,
            rx: rxr,
            ry: ryr,
            rz: rzr,
            lake_level,
            sinkhole,
            min,
            max,
            seed: h,
        })
    }

    /// Carves every cave volume intersecting the cube.
    pub fn carve(
        &self,
        buf: &mut CubeBuf,
        pos: CubePos,
        col: &ColumnData,
        wg: &WorldGenerator,
        b: &GenBlocks,
    ) {
        let o = buf.origin;
        let cmin = [o.x, o.y, o.z];
        let cmax = [
            o.x + CUBE_SIZE - 1,
            o.y + CUBE_SIZE - 1,
            o.z + CUBE_SIZE - 1,
        ];
        // Worm systems from the surrounding regions (as the cube sees them: x unwrapped).
        let r0 = |v: i32| (v - WORM_REACH - REGION).div_euclid(REGION);
        let r1 = |v: i32| (v + WORM_REACH + REGION).div_euclid(REGION);
        let _ = pos;
        let around = (self.c / REGION).max(1);
        for rz in r0(cmin[2])..=r1(cmax[2]) {
            for ry in r0(cmin[1])..=r1(cmax[1]) {
                for rx in r0(cmin[0])..=r1(cmax[0]) {
                    // Regions entirely out of reach are skipped without generating them.
                    let reg_min = [
                        rx * REGION - WORM_REACH,
                        ry * REGION - WORM_REACH,
                        rz * REGION - WORM_REACH,
                    ];
                    let reg_max = [
                        (rx + 1) * REGION + WORM_REACH,
                        (ry + 1) * REGION + WORM_REACH,
                        (rz + 1) * REGION + WORM_REACH,
                    ];
                    if !overlaps(reg_min, reg_max, cmin, cmax) {
                        continue;
                    }
                    let systems = self.region_systems(rx.rem_euclid(around), ry, rz, wg);
                    // Its first block in x and z where the cube sees it.
                    let (x0, z0) = (rx * REGION, rz * REGION);
                    for sys in systems.iter() {
                        for w in &sys.worms {
                            let wmin = [w.min[0] + x0, w.min[1], w.min[2] + z0];
                            let wmax = [w.max[0] + x0, w.max[1], w.max[2] + z0];
                            if !overlaps(wmin, wmax, cmin, cmax) {
                                continue;
                            }
                            for s in &w.spheres {
                                self.carve_sphere(buf, s, (x0, z0), sys.flood_level, col, b);
                            }
                        }
                    }
                }
            }
        }
        // Giant caverns.
        let c0 = |v: i32| (v - 500).div_euclid(CAVERN_REGION);
        let c1 = |v: i32| (v + 500).div_euclid(CAVERN_REGION);
        for rz in c0(cmin[2])..=c1(cmax[2]) {
            for rx in c0(cmin[0])..=c1(cmax[0]) {
                let cav = self.cavern(rx, rz, wg);
                let (x0, z0) = (rx * CAVERN_REGION, rz * CAVERN_REGION);
                if let Some(c) = cav.as_ref()
                    && overlaps(
                        [c.min[0] + x0, c.min[1], c.min[2] + z0],
                        [c.max[0] + x0, c.max[1], c.max[2] + z0],
                        cmin,
                        cmax,
                    )
                {
                    self.carve_cavern(buf, c, (x0, z0), col, b);
                }
            }
        }
    }

    /// Carves a sphere of a worm whose region's first block is `(rx0, rz0)` where the cube sees
    /// it.
    fn carve_sphere(
        &self,
        buf: &mut CubeBuf,
        s: &Sphere,
        (rx0, rz0): (i32, i32),
        flood: Option<i32>,
        col: &ColumnData,
        b: &GenBlocks,
    ) {
        let o = buf.origin;
        // The walls' fill reaches a voxel and a half beyond the space.
        let m = hearth_world::fill::RANGE;
        let x0 = (rx0 + (s.x - s.r - m).floor() as i32).max(o.x);
        let x1 = (rx0 + (s.x + s.r + m).ceil() as i32).min(o.x + 15);
        let y0 = ((s.y - s.ry - m).floor() as i32).max(o.y);
        let y1 = ((s.y + s.ry + m).ceil() as i32).min(o.y + 15);
        let z0 = (rz0 + (s.z - s.r - m).floor() as i32).max(o.z);
        let z1 = (rz0 + (s.z + s.r + m).ceil() as i32).min(o.z + 15);
        let short = s.r.min(s.ry);
        if x0 > x1 || y0 > y1 || z0 > z1 {
            return;
        }
        let inv_r2 = 1.0 / (s.r * s.r);
        let inv_ry2 = 1.0 / (s.ry * s.ry);
        for z in z0..=z1 {
            for x in x0..=x1 {
                let colm = col.at((x - o.x) as usize, (z - o.z) as usize);
                // Never breach the bed of the sea, a lake or a river.
                let protect_above = if colm.is_underwater() {
                    colm.height_i() - 6
                } else {
                    i32::MAX
                };
                for y in y0..=y1 {
                    if y >= protect_above {
                        break;
                    }
                    let dx = (x - rx0) as f32 + 0.5 - s.x;
                    let dy = y as f32 + 0.5 - s.y;
                    let dz = (z - rz0) as f32 + 0.5 - s.z;
                    let e = dx * dx * inv_r2 + dz * dz * inv_r2 + dy * dy * inv_ry2;
                    // Out from the wall, in voxels (about: the ellipsoid's shorter radius).
                    let outside = (e.sqrt() - 1.0) * short;
                    if outside > m {
                        continue;
                    }
                    let i =
                        ((y - o.y) as usize) << 8 | ((z - o.z) as usize) << 4 | (x - o.x) as usize;
                    if b.is_carvable(buf.states[i]) {
                        buf.cut(i, outside);
                    }
                    if e >= 1.0 {
                        continue;
                    }
                    if b.is_carvable(buf.states[i]) {
                        buf.states[i] = if flood.is_some_and(|f| y < f) {
                            b.water
                        } else {
                            b.air
                        };
                    }
                }
            }
        }
    }

    /// Carves a cavern whose region's first block is `(rx0, rz0)` where the cube sees it.
    fn carve_cavern(
        &self,
        buf: &mut CubeBuf,
        c: &Cavern,
        (rx0, rz0): (i32, i32),
        col: &ColumnData,
        b: &GenBlocks,
    ) {
        let o = buf.origin;
        let pillar_seed = (c.seed >> 7) as f64 * 1e-6;
        let short = c.rx.min(c.ry).min(c.rz);
        // The noises periodic around the planet (the walls whole at the seam).
        let (pillars, wide, fine, wobbles) = (
            wave(self.c, 38.0),
            wave(self.c, 44.0),
            wave(self.c, 13.0),
            wave(self.c, 9.0),
        );
        for lz in 0..16 {
            for lx in 0..16 {
                let x = o.x + lx;
                let z = o.z + lz;
                let colm = col.at(lx as usize, lz as usize);
                let protect_above = if colm.is_underwater() {
                    colm.height_i() - 8
                } else {
                    i32::MAX
                };
                // About the region's first block (exact in f32).
                let (ux, uz) = ((x - rx0) as f32 + 0.5, (z - rz0) as f32 + 0.5);
                let fx = (ux - c.cx) / c.rx;
                let fz = (uz - c.cz) / c.rz;
                // Pillars: columns of rock where a 2D noise peaks (not near the walls).
                let (f, cells) = pillars;
                let pn = self
                    .noise
                    .noise2(x as f64 * f + pillar_seed, z as f64 * f, cells);
                let pillar = pn > 0.55 && fx * fx + fz * fz < 0.6;
                let sink = c.sinkhole.map(|(sx, sz, r, top)| {
                    let d2 = (ux - sx).powi(2) + (uz - sz).powi(2);
                    (d2, r, top)
                });
                for ly in 0..16 {
                    let y = o.y + ly;
                    if y >= protect_above {
                        break;
                    }
                    let fy = (y as f32 + 0.5 - c.cy) / c.ry;
                    let mut d = (fx * fx + fy * fy + fz * fz).sqrt();
                    let mut open = false;
                    if d < 1.4 {
                        let ((fw, cw), (ff, cf)) = (wide, fine);
                        let n = self
                            .noise
                            .noise3(x as f64 * fw, y as f64 * fw, z as f64 * fw, cw)
                            as f32
                            * 0.24
                            + self
                                .noise
                                .noise3(x as f64 * ff, y as f64 * ff, z as f64 * ff, cf)
                                as f32
                                * 0.07;
                        d += n;
                        open = d < 1.0 && !pillar;
                    }
                    // Out from the wall, in voxels (about: the cavern's shortest radius).
                    let mut outside = if d < 1.4 && !pillar {
                        (d - 1.0) * short
                    } else {
                        f32::INFINITY
                    };
                    if !open && let Some((d2, r, top)) = sink {
                        // Shaft from the cavern roof up to the surface.
                        let (f, cells) = wobbles;
                        let wobble =
                            self.noise.noise2(x as f64 * f, y as f64 * f, cells) as f32 * 2.5;
                        let shaft = y as f32 > c.cy && y <= top;
                        open = shaft && d2 < (r + wobble).powi(2);
                        if shaft {
                            outside = outside.min(d2.sqrt() - (r + wobble));
                        }
                    }
                    let i = (ly as usize) << 8 | (lz as usize) << 4 | lx as usize;
                    if outside <= hearth_world::fill::RANGE && b.is_carvable(buf.states[i]) {
                        buf.cut(
                            i,
                            if open {
                                outside.min(-0.01)
                            } else {
                                outside.max(0.01)
                            },
                        );
                    }
                    if !open {
                        continue;
                    }
                    if b.is_carvable(buf.states[i]) {
                        buf.states[i] = if y < c.lake_level { b.water } else { b.air };
                    }
                }
            }
        }
        let _ = self.vertical_scale;
    }
}

#[inline]
fn overlaps(amin: [i32; 3], amax: [i32; 3], bmin: [i32; 3], bmax: [i32; 3]) -> bool {
    amin[0] <= bmax[0]
        && amax[0] >= bmin[0]
        && amin[1] <= bmax[1]
        && amax[1] >= bmin[1]
        && amin[2] <= bmax[2]
        && amax[2] >= bmin[2]
}

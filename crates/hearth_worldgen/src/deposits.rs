//! Mineral deposits (v2 §13.2): real bodies of every resource in its geological setting —
//! veins, seams, nodule bands, stream placers, disseminated ore bodies, crusts, pipes, bog ores,
//! evaporite lenses and lava flows — from the content's deposit models, with the surface
//! indicators (stains, gossans, float) that lead a prospector to shallow ones.
//!
//! Bodies are placed per 256-block cell: a Poisson number of candidates per model (its
//! frequency per km²), each kept only where its province, conditions and host rock hold. A
//! cell's bodies are computed once and cached; each cube draws the bodies that reach it, so
//! the result is independent of generation order.

use hearth_content::Content;
use hearth_content::generate::{body_block, float_block, placer_block, resource_block};
use hearth_content::schema::geology::{DepositGeometry, IndicatorKind};
use hearth_math::hash::{derive_seed, hash_2d, hash2, unit_f32};
use hearth_math::{ColumnPos, CubePos};
use hearth_world::{BlockRegistry, BlockStateId};
use smallvec::SmallVec;

use crate::cubegen::blocks::MissingBlock;
use crate::cubegen::cache::Cache;
use crate::cubegen::{CubeBuf, WorldGenerator};
use crate::planet::flags;
use crate::region::ColumnSample;
use crate::region::biome::Biome;

/// Side of a placement cell (blocks).
pub const CELL: i32 = 256;
const CELL_KM2: f32 = (CELL as f32 / 1000.0) * (CELL as f32 / 1000.0);

mod cond {
    pub const RIVER: u16 = 1;
    pub const LAKE: u16 = 2;
    pub const WETLAND: u16 = 4;
    pub const PLACES: u16 = RIVER | LAKE | WETLAND;
    pub const ARID: u16 = 8;
    pub const HUMID: u16 = 16;
    pub const WARM: u16 = 32;
    pub const COLD: u16 = 64;
    pub const VOLCANO: u16 = 128;
    pub const GRANITE: u16 = 256;

    pub fn parse(s: &str) -> Option<u16> {
        Some(match s {
            "river" => RIVER,
            "lake" => LAKE,
            "wetland" => WETLAND,
            "arid" => ARID,
            "humid" => HUMID,
            "warm" => WARM,
            "cold" => COLD,
            "volcano" => VOLCANO,
            "granite" => GRANITE,
            _ => return None,
        })
    }
}

/// A deposit model resolved to block states.
#[derive(Debug, Clone)]
pub struct DepositModel {
    pub id: String,
    /// The resource (rock, mineral or material id).
    pub resource: String,
    pub geometry: DepositGeometry,
    pub era: u8,
    /// Mass fraction of the resource in a body.
    grade: (f32, f32),
    /// What washing gravel near a body leaves in the pan: the resource itself (placers) or an
    /// indicator mineral, with its grade in the gravel.
    pan: Option<(String, (f32, f32))>,
    provinces: Vec<u8>,
    hosts: Vec<BlockStateId>,
    conditions: u16,
    depth: (f32, f32),
    size: (f32, f32),
    thickness: (f32, f32),
    extent: (f32, f32),
    per_cell: f32,
    pub block: BlockStateId,
    placer: Option<BlockStateId>,
    /// The placer lies on the bed (cobbles) rather than replacing it (placer gravel).
    placer_on_top: bool,
    stains: SmallVec<[BlockStateId; 2]>,
    float: Option<BlockStateId>,
}

/// One body in the world.
#[derive(Debug, Clone)]
pub struct Body {
    pub model: u16,
    pub center: [i32; 3],
    /// Surface height at the centre (first air block).
    pub surface: i32,
    size: f32,
    thickness: f32,
    /// Mass fraction of the resource.
    pub grade: f32,
    /// Vein plane normal, strike and in-plane dip axis.
    normal: [f32; 3],
    strike: [f32; 3],
    dip: [f32; 3],
    seed: u64,
    min: [i32; 3],
    max: [i32; 3],
    /// Horizontal reach of the body and its indicators around the centre (blocks).
    reach: i32,
    /// Top close enough to the surface to show indicators.
    shallow: bool,
    /// Lowest and highest ground within its reach (shallow bodies; the centre's otherwise).
    ground: (i32, i32),
}

/// What a pan of washed gravel holds (v2 §13.3).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PanResult {
    /// Heavy grains caught in the pan: resource or mineral id and grams per pan, largest first.
    pub grains: Vec<(String, f32)>,
}

/// Gravel in one pan (kg).
pub const PAN_KG: f32 = 8.0;

/// Deposits of one world.
pub struct Deposits {
    pub models: Vec<DepositModel>,
    cells: Cache<(i32, i32), Vec<Body>>,
    cells_around: i32,
    /// The farthest any body or its indicators reach from its centre (blocks).
    max_reach: i32,
    granite: Option<BlockStateId>,
    seed: u64,
}

impl Body {
    /// Blocks of ground over the top of the body (0 for bodies at the surface).
    pub fn top_depth(&self) -> i32 {
        (self.surface - 1 - self.max[1]).max(0)
    }

    /// Size across (blocks; the length of the reach for placers).
    pub fn size(&self) -> f32 {
        self.size
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

impl Deposits {
    pub fn new(
        content: &Content,
        reg: &BlockRegistry,
        province_ids: &[String],
        seed: u64,
        circumference: i32,
        vertical_scale: f64,
    ) -> Result<Self, MissingBlock> {
        use hearth_content::schema::Entry;
        let state = |id: &str| -> Result<BlockStateId, MissingBlock> {
            let path = id
                .split_once(':')
                .map_or(id, |(ns, p)| if ns == "hearth" { p } else { id });
            reg.parse_state(path)
                .map_err(|_| MissingBlock(id.to_owned()))
        };
        let v = vertical_scale as f32;
        let mut models = Vec::new();
        for d in content.deposits.iter() {
            let resource = d.resource.as_str();
            let (block, _) = body_block(content, resource, d.geometry)
                .ok_or_else(|| MissingBlock(format!("deposit {} resource {resource}", d.id())))?;
            let block = state(&block)?;
            let mut conditions = 0;
            for c in &d.conditions {
                match cond::parse(c) {
                    Some(b) => conditions |= b,
                    None => log::warn!("deposit {}: unknown condition {c:?}", d.id()),
                }
            }
            let provinces = d
                .provinces
                .iter()
                .filter_map(|p| {
                    province_ids
                        .iter()
                        .position(|id| id == p.as_str())
                        .map(|i| i as u8)
                })
                .collect();
            let hosts = d
                .host_rocks
                .iter()
                .map(|r| state(r.as_str()))
                .collect::<Result<_, _>>()?;
            // Deep bodies scale with the world's vertical scale, like the strata; surface-bound
            // bodies stay at the block scale.
            let surface_bound = matches!(
                d.geometry,
                DepositGeometry::Placer
                    | DepositGeometry::Crust
                    | DepositGeometry::Flow
                    | DepositGeometry::Bog
            );
            let depth = if surface_bound {
                (d.depth_m.0, d.depth_m.1)
            } else {
                (d.depth_m.0 * v, d.depth_m.1 * v)
            };
            let (placer, placer_on_top) = match placer_block(content, resource) {
                Some((id, kind)) if d.geometry == DepositGeometry::Placer => (
                    Some(state(&id)?),
                    kind == hearth_content::generate::NaturalKind::Cobbles,
                ),
                _ => (None, false),
            };
            let mut stains = SmallVec::new();
            let mut float = None;
            let mut pan = None;
            for ind in &d.indicators {
                let shown = ind.mineral.as_ref().map_or(resource, |m| m.as_str());
                if ind.kind == IndicatorKind::PanConcentrate {
                    // Placers leave their own grains; other bodies shed a trace of an indicator
                    // mineral into the streams below them.
                    let grade = if d.geometry == DepositGeometry::Placer {
                        (d.grade.0, d.grade.1)
                    } else {
                        (0.00002, 0.0002)
                    };
                    pan = Some((shown.to_owned(), grade));
                }
                match ind.kind {
                    IndicatorKind::Stain | IndicatorKind::Gossan => {
                        if let Some((id, _)) = resource_block(content, shown) {
                            stains.push(state(&id)?);
                        }
                    }
                    IndicatorKind::Float => {
                        if let Some(id) = float_block(content, shown) {
                            float = Some(state(&id)?);
                        }
                    }
                    _ => {}
                }
            }
            models.push(DepositModel {
                id: d.id().to_owned(),
                resource: resource.to_owned(),
                geometry: d.geometry,
                era: d.era,
                grade: (d.grade.0, d.grade.1),
                pan,
                provinces,
                hosts,
                conditions,
                depth,
                size: (d.size_m.0.max(0.5), d.size_m.1.clamp(1.0, 150.0)),
                thickness: d
                    .thickness_m
                    .map_or((1.0, 1.0), |t| (t.0.max(1.0), t.1.max(1.0))),
                extent: d.extent_m.map_or((60.0, 200.0), |e| (e.0, e.1.min(800.0))),
                per_cell: d.frequency_per_km2 * CELL_KM2,
                block,
                placer,
                placer_on_top,
                stains,
                float,
            });
        }
        let max_reach = models
            .iter()
            .map(|m| Self::reach_of(m, m.size.1, m.extent.1).ceil() as i32)
            .max()
            .unwrap_or(0);
        Ok(Self {
            models,
            cells: Cache::new(512),
            cells_around: (circumference / CELL).max(1),
            max_reach,
            granite: reg.parse_state("granite").ok(),
            seed: derive_seed(seed, "deposits"),
        })
    }

    /// How far a body of this size reaches, indicators and float included.
    fn reach_of(m: &DepositModel, size: f32, extent: f32) -> f32 {
        let body = match m.geometry {
            DepositGeometry::Placer | DepositGeometry::Nodules => extent * 0.6,
            _ => size * 0.6,
        };
        let float = if m.float.is_some() {
            (size * 1.5).max(24.0)
        } else {
            0.0
        };
        body.max(float) + 2.0
    }

    /// Whether any of the place conditions (river, lake, wetland) listed in `c` holds.
    fn place_holds(c: u16, s: &ColumnSample) -> bool {
        if c & cond::PLACES == 0 {
            return true;
        }
        let river = s.river.is_some_and(|r| r.distance < r.width + 24.0);
        let lake = s.lake || s.biome == Biome::Lake;
        let wetland = s.biome == Biome::Wetland;
        (c & cond::RIVER != 0 && river)
            || (c & cond::LAKE != 0 && lake)
            || (c & cond::WETLAND != 0 && wetland)
    }

    /// Whether a model's conditions hold at a place.
    fn conditions_hold(
        &self,
        m: &DepositModel,
        s: &ColumnSample,
        wg: &WorldGenerator,
        x: i32,
        z: i32,
    ) -> bool {
        let c = m.conditions;
        if !Self::place_holds(c, s) {
            return false;
        }
        if c & cond::ARID != 0 && s.precipitation >= 450.0 {
            return false;
        }
        if c & cond::HUMID != 0 && s.precipitation <= 750.0 {
            return false;
        }
        if c & cond::WARM != 0 && s.temperature <= 15.0 {
            return false;
        }
        if c & cond::COLD != 0 && s.temperature >= 3.0 {
            return false;
        }
        if c & cond::VOLCANO != 0 && !Self::near_volcano(wg, x, z) {
            return false;
        }
        // Placers wash down from their source: the granite may lie some way upstream.
        let reach = if m.geometry == DepositGeometry::Placer {
            500
        } else {
            150
        };
        if c & cond::GRANITE != 0 && !self.near_granite(wg, x, z, reach) {
            return false;
        }
        true
    }

    fn near_volcano(wg: &WorldGenerator, x: i32, z: i32) -> bool {
        let grid = &wg.terrain.grid;
        let idx = grid.cell_at(x as f64, z as f64);
        if grid.flags[idx] & flags::VOLCANIC != 0 {
            return true;
        }
        let planet = wg.planet();
        grid.volcanoes.iter().any(|v| {
            let dx = planet.delta_x(x as f64, v.x);
            let dz = v.z - z as f64;
            dx * dx + dz * dz < 1200.0 * 1200.0
        })
    }

    /// A granite body close by: a pluton under the place or within `r` blocks of it.
    fn near_granite(&self, wg: &WorldGenerator, x: i32, z: i32, r: i32) -> bool {
        let Some(granite) = self.granite else {
            return false;
        };
        let h = r / 2;
        [
            (0, 0),
            (r, 0),
            (-r, 0),
            (0, r),
            (0, -r),
            (h, h),
            (-h, h),
            (h, -h),
            (-h, -h),
        ]
        .iter()
        .take(if r > 150 { 9 } else { 5 })
        .any(|(dx, dz)| {
            wg.geology
                .column(x + dx, z + dz)
                .intrusion()
                .is_some_and(|(rock, _)| rock == granite)
        })
    }

    /// Bodies of a placement cell.
    pub fn cell(&self, wg: &WorldGenerator, cx: i32, cz: i32) -> std::sync::Arc<Vec<Body>> {
        let cx = cx.rem_euclid(self.cells_around);
        self.cells
            .get_or_insert_with((cx, cz), || self.compute_cell(wg, cx, cz))
    }

    /// Placement cells around the planet (and from pole edge to pole edge).
    pub fn cells_around(&self) -> i32 {
        self.cells_around
    }

    /// Whether a placement cell has land in or just beside it (on the planet grid).
    pub fn has_land(&self, wg: &WorldGenerator, cx: i32, cz: i32) -> bool {
        let grid = &wg.terrain.grid;
        [-0.25, 0.25, 0.75, 1.25].iter().any(|fz| {
            [-0.25, 0.25, 0.75, 1.25].iter().any(|fx| {
                let x = (cx as f64 + fx) * CELL as f64;
                let z = (cz as f64 + fz) * CELL as f64;
                grid.elevation.data[grid.cell_at(x, z)] > 0.0
            })
        })
    }

    /// Every body on the planet, from every placement cell with land in or beside it (bypassing
    /// the cache; cells are computed in parallel). Rows of cells run from `-n/2` to `n - n/2`.
    pub fn census(&self, wg: &WorldGenerator) -> Vec<Body> {
        use rayon::prelude::*;
        let n = self.cells_around;
        (-n / 2..n - n / 2)
            .into_par_iter()
            .flat_map_iter(|cz| {
                (0..n)
                    .filter(move |&cx| self.has_land(wg, cx, cz))
                    .flat_map(move |cx| self.compute_cell(wg, cx, cz))
            })
            .collect()
    }

    /// Whether a body lies in one of its model's provinces.
    pub fn in_province(&self, b: &Body, province: u8) -> bool {
        let m = &self.models[b.model as usize];
        m.provinces.is_empty() || m.provinces.contains(&province)
    }

    /// The rocks a model's bodies may replace (empty: any rock).
    pub fn hosts(&self, model: u16) -> &[BlockStateId] {
        &self.models[model as usize].hosts
    }

    fn compute_cell(&self, wg: &WorldGenerator, cx: i32, cz: i32) -> Vec<Body> {
        let mut out = Vec::new();
        for (mi, m) in self.models.iter().enumerate() {
            let h0 = hash2(hash_2d(self.seed, cx, cz), mi as u64);
            // Poisson count by inversion (small means).
            let mut n = 0u32;
            let (mut p, l) = (1.0f64, (-(m.per_cell as f64)).exp());
            loop {
                p *= unit_f32(hash2(h0, n as u64 + 100)) as f64;
                if p < l || n > 64 {
                    break;
                }
                n += 1;
            }
            // Deposits tied to a place (a river, lake or wetland) look for one in the cell:
            // their frequency is per km² of ground that has such places.
            let tries = if m.conditions & cond::PLACES != 0 {
                16
            } else {
                1
            };
            for k in 0..n {
                let h = hash2(h0, k as u64);
                for t in 0..tries {
                    let ht = hash2(h, 1000 + t);
                    let x = cx * CELL + (unit_f32(hash2(ht, 1)) * CELL as f32) as i32;
                    let z = cz * CELL + (unit_f32(hash2(ht, 2)) * CELL as f32) as i32;
                    let s = wg.terrain.sample(x, z);
                    if tries > 1 && !Self::place_holds(m.conditions, &s) {
                        continue;
                    }
                    if let Some(b) = self.try_body(wg, mi, m, x, z, &s, h) {
                        out.push(b);
                    }
                    break;
                }
            }
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn try_body(
        &self,
        wg: &WorldGenerator,
        mi: usize,
        m: &DepositModel,
        x: i32,
        z: i32,
        s: &ColumnSample,
        h: u64,
    ) -> Option<Body> {
        let province = wg.geology.province_index(x as f64 + 0.5, z as f64 + 0.5);
        if !m.provinces.is_empty() && !m.provinces.contains(&province) {
            return None;
        }
        if !self.conditions_hold(m, s, wg, x, z) {
            return None;
        }
        let surface = s.height_i();
        let surface_bound = matches!(
            m.geometry,
            DepositGeometry::Placer
                | DepositGeometry::Crust
                | DepositGeometry::Flow
                | DepositGeometry::Bog
        );
        if surface_bound && s.is_underwater() && m.geometry != DepositGeometry::Placer {
            return None;
        }
        let r = |k: u64| unit_f32(hash2(h, k));
        let lerp = |a: (f32, f32), t: f32| a.0 + (a.1 - a.0) * t;
        let size = lerp(m.size, r(3));
        let thickness = lerp(m.thickness, r(4)).round().max(1.0);
        // Depth: pick one where the host rock is (a few tries), else give up.
        let mut y = surface - 1;
        if !surface_bound {
            let mut found = false;
            let rock_col = wg.geology.column(x, z);
            for t in 0..6u64 {
                let depth = lerp(m.depth, r(10 + t));
                let yy = surface - 1 - depth.round() as i32;
                if m.hosts.is_empty() || m.hosts.contains(&rock_col.rock_at(yy)) {
                    y = yy;
                    found = true;
                    break;
                }
            }
            if !found {
                return None;
            }
        }
        // Orientation of veins: random strike, steep dip.
        let strike_a = r(20) * std::f32::consts::PI;
        let dip_a = (55.0 + 35.0 * r(21)).to_radians();
        let strike = [strike_a.cos(), 0.0, strike_a.sin()];
        let normal = [
            strike_a.sin() * dip_a.sin(),
            dip_a.cos(),
            -strike_a.cos() * dip_a.sin(),
        ];
        let dip = [
            normal[1] * strike[2] - normal[2] * strike[1],
            normal[2] * strike[0] - normal[0] * strike[2],
            normal[0] * strike[1] - normal[1] * strike[0],
        ];
        let extent = lerp(m.extent, r(5));
        // Bounding box by geometry.
        let (hr, top_up, down) = match m.geometry {
            DepositGeometry::Vein => (size * 0.5 + thickness, size * 0.35, size * 0.35),
            DepositGeometry::Seam | DepositGeometry::Evaporite => {
                (size * 0.6, thickness + 3.0, thickness + 3.0)
            }
            DepositGeometry::Nodules => (extent * 0.6, 8.0, 8.0),
            DepositGeometry::Disseminated => (size * 0.6, size * 0.35, size * 0.35),
            DepositGeometry::Pipe => (size * 0.6, (surface - y) as f32 + 2.0, 4.0),
            DepositGeometry::Crust | DepositGeometry::Flow | DepositGeometry::Bog => {
                (size * 0.6, 2.0, thickness + 3.0)
            }
            DepositGeometry::Placer => (extent * 0.55, 3.0, 4.0),
        };
        let (hr, top_up, down) = (hr.ceil() as i32, top_up.ceil() as i32, down.ceil() as i32);
        let body_top = y + top_up;
        let reach = Self::reach_of(m, size, extent).ceil() as i32;
        let shallow = surface - body_top < 16;
        // Ground heights under the reach of shallow bodies, so the cubes their indicators and
        // surface layers fall in are known on slopes too.
        let mut ground = (surface, surface);
        if shallow {
            for k in 0..8 {
                let (sin, cos) = (k as f32 * std::f32::consts::FRAC_PI_4).sin_cos();
                for f in [0.5, 1.0] {
                    let px = x + (cos * reach as f32 * f) as i32;
                    let pz = z + (sin * reach as f32 * f) as i32;
                    let h = wg.terrain.sample(px, pz).height_i();
                    ground = (ground.0.min(h), ground.1.max(h));
                }
            }
        }
        Some(Body {
            model: mi as u16,
            center: [x, y, z],
            surface,
            size: if m.geometry == DepositGeometry::Placer {
                extent
            } else {
                size
            },
            thickness,
            grade: lerp(m.grade, r(6)),
            normal,
            strike,
            dip,
            seed: h,
            min: [x - hr, y - down, z - hr],
            max: [x + hr, y + top_up, z + hr],
            reach,
            shallow,
            ground,
        })
    }

    /// Draws every body that reaches a cube into it, and their indicators.
    pub fn apply(&self, buf: &mut CubeBuf, pos: CubePos, wg: &WorldGenerator) {
        let o = buf.origin;
        let (x0, y0, z0) = (o.x, o.y, o.z);
        let (x1, y1, z1) = (x0 + 15, y0 + 15, z0 + 15);
        let _ = pos;
        let margin = self.max_reach + 8;
        let (ca, cb) = (
            (x0 - margin).div_euclid(CELL),
            (x1 + margin).div_euclid(CELL),
        );
        let (za, zb) = (
            (z0 - margin).div_euclid(CELL),
            (z1 + margin).div_euclid(CELL),
        );
        for cz in za..=zb {
            for cx in ca..=cb {
                let bodies = self.cell(wg, cx, cz);
                for b in bodies.iter() {
                    // The cell's bodies are in canonical X; shift to this cube's side of the seam.
                    let shift = wg.planet().delta_block_x(b.center[0], x0) - (x0 - b.center[0]);
                    let bx = b.center[0] - shift;
                    let bz = b.center[2];
                    if bx + b.reach < x0
                        || bx - b.reach > x1
                        || bz + b.reach < z0
                        || bz - b.reach > z1
                    {
                        continue;
                    }
                    let indicator_band = b.shallow && b.ground.1 + 2 >= y0 && b.ground.0 - 6 <= y1;
                    if b.max[1] < y0 || b.min[1] > y1 {
                        if indicator_band {
                            self.indicators(buf, wg, b, shift);
                        }
                        continue;
                    }
                    self.draw(buf, wg, b, shift);
                    if indicator_band {
                        self.indicators(buf, wg, b, shift);
                    }
                }
            }
        }
    }

    fn draw(&self, buf: &mut CubeBuf, wg: &WorldGenerator, b: &Body, shift: i32) {
        let m = &self.models[b.model as usize];
        let o = buf.origin;
        let cx = b.center[0] - shift;
        let [_, cy, cz] = b.center;
        let replaceable = |s: BlockStateId| -> bool {
            if m.hosts.is_empty() {
                wg.blocks.is_base_rock(s) || wg.blocks.is_carvable(s)
            } else {
                m.hosts.contains(&s)
            }
        };
        let hash =
            |x: i32, y: i32, z: i32, k: u64| unit_f32(hash2(hash_2d(b.seed ^ k, x, z), y as u64));
        for ly in 0..16 {
            let y = o.y + ly;
            if y < b.min[1] || y > b.max[1] {
                continue;
            }
            for lz in 0..16 {
                let z = o.z + lz;
                if z < b.min[2] || z > b.max[2] {
                    continue;
                }
                for lx in 0..16 {
                    let x = o.x + lx;
                    if x < cx - (b.center[0] - b.min[0]) || x > cx + (b.max[0] - b.center[0]) {
                        continue;
                    }
                    let d = [(x - cx) as f32, (y - cy) as f32, (z - cz) as f32];
                    let wobble = (hash(x >> 2, y >> 2, z >> 2, 7) - 0.5) * 0.8;
                    let inside = match m.geometry {
                        DepositGeometry::Vein => {
                            dot(d, b.normal).abs() < b.thickness * 0.5 + wobble.max(0.0)
                                && dot(d, b.strike).abs() < b.size * 0.5
                                && dot(d, b.dip).abs() < b.size * 0.3
                        }
                        DepositGeometry::Seam | DepositGeometry::Evaporite => {
                            let r = (d[0] * d[0] + d[2] * d[2]).sqrt();
                            let undulation = ((x as f32 * 0.02 + b.seed as f32 % 7.0).sin()
                                + (z as f32 * 0.017).cos())
                                * 1.2;
                            r < b.size * 0.5 * (1.0 + 0.15 * wobble)
                                && (d[1] - undulation).abs() < b.thickness * 0.5
                        }
                        DepositGeometry::Nodules => {
                            let r = (d[0] * d[0] + d[2] * d[2]).sqrt();
                            // Three bands a few blocks apart, nodules scattered along each.
                            let band = (d[1] as i32).rem_euclid(3) == 0 && d[1].abs() <= 6.0;
                            r < b.size * 0.5 && band && hash(x, y, z, 11) < 0.22
                        }
                        DepositGeometry::Disseminated => {
                            let e = (d[0] / (b.size * 0.5)).powi(2)
                                + (d[1] / (b.size * 0.3)).powi(2)
                                + (d[2] / (b.size * 0.5)).powi(2);
                            e < 1.0 + 0.2 * wobble && hash(x, y, z, 12) < 0.45
                        }
                        DepositGeometry::Pipe => {
                            let r = (d[0] * d[0] + d[2] * d[2]).sqrt();
                            r < b.size * 0.5 * (1.0 + 0.1 * wobble)
                                && y >= b.min[1]
                                && y < b.surface
                        }
                        // Surface-bound bodies follow the ground in `indicators`-like passes.
                        DepositGeometry::Crust
                        | DepositGeometry::Flow
                        | DepositGeometry::Bog
                        | DepositGeometry::Placer => false,
                    };
                    if !inside {
                        continue;
                    }
                    if let Some(s) = buf.get(x, y, z)
                        && replaceable(s)
                    {
                        buf.set(x, y, z, m.block);
                    }
                }
            }
        }
    }

    /// Surface-bound bodies and the signs of shallow ones: crusts, flows, bog ore and placers
    /// laid along the ground; stains and gossans on the ground above a body; float scattered
    /// around it.
    fn indicators(&self, buf: &mut CubeBuf, wg: &WorldGenerator, b: &Body, shift: i32) {
        let m = &self.models[b.model as usize];
        let o = buf.origin;
        let cx = b.center[0] - shift;
        let cz = b.center[2];
        let hash = |x: i32, z: i32, k: u64| unit_f32(hash_2d(b.seed ^ k, x, z));
        let radius = match m.geometry {
            DepositGeometry::Placer => b.size * 0.5,
            _ => (b.size * 0.5).max(3.0),
        };
        let float_radius = (b.size * 1.5).max(24.0);
        for lz in 0..16 {
            let z = o.z + lz;
            for lx in 0..16 {
                let x = o.x + lx;
                let (dx, dz) = ((x - cx) as f32, (z - cz) as f32);
                let r = (dx * dx + dz * dz).sqrt();
                if r > radius.max(float_radius) {
                    continue;
                }
                let col = wg.column(ColumnPos::new(x >> 4, z >> 4));
                let s = col.at((x & 15) as usize, (z & 15) as usize);
                let top = s.height_i() - 1;
                if top < o.y - 4 || top > o.y + 15 {
                    continue;
                }
                let ground = |buf: &CubeBuf, y: i32| {
                    buf.get(x, y, z)
                        .is_some_and(|g| wg.blocks.is_base_rock(g) || wg.blocks.is_carvable(g))
                };
                let noisy_r = r * (1.0 + 0.25 * (hash(x >> 2, z >> 2, 3) - 0.5));
                match m.geometry {
                    DepositGeometry::Crust | DepositGeometry::Flow | DepositGeometry::Bog => {
                        // Crusts break into blotches toward their edges; flows are solid.
                        let edge = noisy_r / radius;
                        let blotch = 0.6 * hash(x >> 2, z >> 2, 5) + 0.4 * hash(x >> 1, z >> 1, 6);
                        let patchy =
                            m.geometry == DepositGeometry::Crust && blotch < edge * edge * 0.9;
                        if noisy_r < radius && !patchy && !s.is_underwater() {
                            let fill = if m.geometry == DepositGeometry::Bog {
                                0.4
                            } else {
                                1.0
                            };
                            let skip = if m.geometry == DepositGeometry::Bog {
                                1
                            } else {
                                0
                            };
                            for k in skip..(b.thickness as i32 + skip) {
                                let y = top - k;
                                if ground(buf, y) && hash(x, z, 40 + k as u64) < fill {
                                    buf.set(x, y, z, m.block);
                                }
                            }
                        }
                    }
                    DepositGeometry::Placer => {
                        let at_river = s.river.is_some_and(|rv| rv.distance < rv.width * 0.5 + 6.0);
                        if noisy_r < radius && at_river && hash(x, z, 41) < 0.35 {
                            match (m.placer, m.placer_on_top) {
                                // Cobbles on dry bars and banks.
                                (Some(c), true)
                                    if !s.is_underwater()
                                        && buf.get(x, top + 1, z).is_some_and(|a| a.is_air())
                                        && ground(buf, top) =>
                                {
                                    buf.set(x, top + 1, z, c);
                                }
                                // Placer gravel in the bed.
                                (Some(p), false) if ground(buf, top) => buf.set(x, top, z, p),
                                _ => {}
                            }
                        }
                    }
                    _ => {
                        // Stains and gossans on the ground above the body.
                        if !m.stains.is_empty()
                            && noisy_r < radius
                            && !s.is_underwater()
                            && hash(x, z, 42) < 0.35
                            && ground(buf, top)
                        {
                            let pick = m.stains[(hash(x, z, 43) * m.stains.len() as f32) as usize
                                % m.stains.len()];
                            buf.set(x, top, z, pick);
                        }
                    }
                }
                // Float: pieces scattered around shallow bodies (and along placer reaches).
                if let Some(f) = m.float
                    && r < float_radius
                    && !s.is_underwater()
                    && hash(x, z, 44) < 0.025
                    && buf.get(x, top + 1, z).is_some_and(|a| a.is_air())
                    && ground(buf, top)
                {
                    buf.set(x, top + 1, z, f);
                }
            }
        }
    }
}

impl Deposits {
    /// Washing a pan of gravel at a river bed or bank: the heavy grains of the placers it lies in
    /// (richest near the middle of a placer reach) and the trace indicator grains shed by bodies
    /// upstream within a few hundred blocks. Away from rivers nothing is left but mud.
    pub fn pan(&self, wg: &WorldGenerator, x: i32, z: i32) -> PanResult {
        let s = wg.terrain.sample(x, z);
        if !s.river.is_some_and(|r| r.distance < r.width * 0.5 + 6.0) {
            return PanResult::default();
        }
        const SHED: f32 = 300.0;
        let mut grains: Vec<(String, f32)> = Vec::new();
        let margin = self.max_reach.max(SHED as i32) + 8;
        let (ca, cb) = ((x - margin).div_euclid(CELL), (x + margin).div_euclid(CELL));
        let (za, zb) = ((z - margin).div_euclid(CELL), (z + margin).div_euclid(CELL));
        for cz in za..=zb {
            for cx in ca..=cb {
                for b in self.cell(wg, cx, cz).iter() {
                    let m = &self.models[b.model as usize];
                    let Some((mineral, grade)) = &m.pan else {
                        continue;
                    };
                    let dx = wg.planet().delta_block_x(b.center[0], x) as f32;
                    let dz = (z - b.center[2]) as f32;
                    let r = (dx * dx + dz * dz).sqrt();
                    let (radius, grade) = if m.geometry == DepositGeometry::Placer {
                        (b.size * 0.5, b.grade)
                    } else {
                        let t = unit_f32(hash2(b.seed, 77));
                        (SHED, grade.0 + (grade.1 - grade.0) * t)
                    };
                    if r >= radius {
                        continue;
                    }
                    let grams = grade * PAN_KG * 1000.0 * (1.0 - r / radius);
                    match grains.iter_mut().find(|(id, _)| id == mineral) {
                        Some(g) => g.1 += grams,
                        None => grains.push((mineral.clone(), grams)),
                    }
                }
            }
        }
        grains.sort_by(|a, b| b.1.total_cmp(&a.1));
        PanResult { grains }
    }
}

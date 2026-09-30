//! Geology: the rock at every block (v2 §5.1–5.2).
//!
//! * Every planet grid cell gets a geological province from the content's province table. The
//!   cell's tectonic setting (from the planet model's history) selects the candidates, the
//!   provinces' conditions (arid, humid, warm, cold, coastal, inland, old, young) filter them,
//!   and a weighted hash of a warped region picks one, so provinces cover coherent regions
//!   with irregular borders.
//! * A province's stratigraphic sequence lies with its top a little above the regional surface
//!   (the planet's elevation, smoothed) plus the thickness erosion has stripped off where the
//!   land was uplifted: mountain cores expose the deeper layers and the basement, basins keep
//!   their youngest rocks on top. Layer thicknesses vary across a region and layers pinch out
//!   where they are absent. Collision belts fold the layers into anticlines and synclines;
//!   elsewhere they warp into gentle domes and basins. The present terrain cuts through them,
//!   so deeper layers crop out in valleys, cliffs and canyons.
//! * Plutons (granite, diorite, gabbro) rise into the sequence as domes; the crystalline
//!   basement lies under everything. Rock warms with depth (geothermal gradient).
//! * Thicknesses are real metres times the world's vertical scale, like relief; structures
//!   (fold wavelengths, dips) are at the block scale, where a block is a metre.

use hearth_content::Content;
use hearth_content::schema::geology::{Province, TectonicSetting};
use hearth_math::hash::{derive_seed, hash_2d, hash2, unit_f32};
use hearth_world::{BlockRegistry, BlockStateId};
use rayon::prelude::*;
use smallvec::SmallVec;

use crate::cubegen::blocks::MissingBlock;
use crate::noise::BlockFbm;
use crate::planet::grid::Field;
use crate::planet::{PlanetGrid, flags, province as code};

/// Geothermal gradient (°C per real metre of depth).
pub const GEOTHERMAL_C_PER_M: f64 = 0.025;
/// The top of a sequence sits this far above the regional surface (blocks).
const DATUM_OFFSET: f32 = 8.0;
/// Share of the uplift that erosion has removed from above today's surface.
const EXHUMATION: f64 = 0.6;
/// Spacing of the pluton grid (blocks).
const PLUTON_CELL: i32 = 1600;
/// Weight a province keeps where its climate conditions do not hold: today's climate is only a
/// weak guide to where rocks formed (continents drift, climates change), so coal measures and
/// evaporites favour today's humid and dry lands without being confined to them.
const CLIMATE_MISMATCH: f32 = 0.15;

/// Province conditions as bits.
mod cond {
    pub const ARID: u8 = 1;
    pub const HUMID: u8 = 2;
    pub const WARM: u8 = 4;
    pub const COLD: u8 = 8;
    pub const COASTAL: u8 = 16;
    pub const INLAND: u8 = 32;
    pub const OLD: u8 = 64;
    pub const YOUNG: u8 = 128;

    /// Conditions on today's climate (the others are structural).
    pub const CLIMATE: u8 = ARID | HUMID | WARM | COLD;

    pub fn parse(s: &str) -> Option<u8> {
        Some(match s {
            "arid" => ARID,
            "humid" => HUMID,
            "warm" => WARM,
            "cold" => COLD,
            "coastal" => COASTAL,
            "inland" => INLAND,
            "old" => OLD,
            "young" => YOUNG,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
struct LayerModel {
    rock: BlockStateId,
    /// Thickness range in blocks.
    thickness: (f32, f32),
    presence: f32,
}

/// A province resolved to block states.
#[derive(Debug, Clone)]
pub struct ProvinceModel {
    pub id: String,
    pub name: String,
    pub setting: TectonicSetting,
    layers: Vec<LayerModel>,
    pub basement: BlockStateId,
    intrusions: Vec<BlockStateId>,
    /// Share of pluton sites with an intrusion, and the depth range of their roofs (blocks).
    intrusion_share: f32,
    intrusion_roof: (f32, f32),
    /// Folded (collision belts) or domed and basined.
    folded: bool,
    /// Fold or warp amplitude (blocks), wavelength (blocks) and axis direction (radians).
    amplitude: f32,
    wavelength: f32,
    axis: f32,
    conditions: u8,
    weight: f32,
}

/// The rock column at one (x, z).
#[derive(Debug, Clone)]
pub struct RockColumn {
    pub province: u8,
    /// Y of the top of the sequence (datum plus structure).
    pub top: f32,
    /// Depth below `top` of each layer's base (blocks), top down.
    bases: SmallVec<[(f32, BlockStateId); 8]>,
    basement: BlockStateId,
    /// An intrusion under this column: its rock and the Y of its roof.
    intrusion: Option<(BlockStateId, f32)>,
}

impl RockColumn {
    /// The intrusion under this column, if any: its rock and the Y of its roof.
    pub fn intrusion(&self) -> Option<(BlockStateId, f32)> {
        self.intrusion
    }

    /// The rock at world Y.
    #[inline]
    pub fn rock_at(&self, y: i32) -> BlockStateId {
        let yf = y as f32 + 0.5;
        if let Some((rock, roof)) = self.intrusion
            && yf < roof
        {
            return rock;
        }
        let depth = self.top - yf;
        for &(base, rock) in &self.bases {
            if depth < base {
                return rock;
            }
        }
        self.basement
    }
}

/// Geology of one world.
pub struct Geology {
    provinces: Vec<ProvinceModel>,
    /// Province index per half-resolution grid cell.
    cells: Vec<u8>,
    /// Half-resolution grid size and blocks per half cell.
    n: usize,
    cell: f64,
    circumference: f64,
    /// Smoothed planet elevation (blocks) for the stratigraphic datum.
    datum: Field<f32>,
    full_n: usize,
    warp_x: BlockFbm,
    warp_z: BlockFbm,
    phase: BlockFbm,
    thickness: BlockFbm,
    presence: BlockFbm,
    pluton_shape: BlockFbm,
    vertical_scale: f64,
    seed: u64,
}

fn setting_of(grid: &PlanetGrid, idx: usize, coastal: bool) -> TectonicSetting {
    let volcanic = grid.flags[idx] & flags::VOLCANIC != 0;
    match grid.province[idx] {
        code::OCEANIC => TectonicSetting::OceanFloor,
        code::ARC => TectonicSetting::VolcanicArc,
        _ if volcanic => TectonicSetting::Hotspot,
        code::SHIELD => TectonicSetting::Craton,
        code::OROGEN | code::OLD_OROGEN => TectonicSetting::FoldBelt,
        code::RIFT => TectonicSetting::Rift,
        _ if coastal => TectonicSetting::PassiveMargin,
        _ => TectonicSetting::SedimentaryBasin,
    }
}

impl Geology {
    /// Resolves the content's provinces and assigns them to the planet.
    pub fn new(
        grid: &PlanetGrid,
        content: &Content,
        reg: &BlockRegistry,
    ) -> Result<Self, MissingBlock> {
        let v = grid.vertical_scale;
        let state = |id: &str| -> Result<BlockStateId, MissingBlock> {
            let path = id
                .split_once(':')
                .map_or(id, |(ns, p)| if ns == "hearth" { p } else { id });
            reg.parse_state(path)
                .map_err(|_| MissingBlock(id.to_owned()))
        };
        let seed = derive_seed(grid.seed, "geology");
        let mut provinces = Vec::new();
        for p in content.provinces.iter() {
            provinces.push(Self::resolve(p, &state, v, seed)?);
        }
        if provinces.is_empty() || provinces.len() > 255 {
            return Err(MissingBlock(format!(
                "{} geological provinces (need 1–255)",
                provinces.len()
            )));
        }
        let geom = &grid.geom;
        let full_n = geom.n;
        let n = (full_n / 2).max(1);
        let c = geom.c;
        let circ = c.round() as i64;
        // Region choice: regions of 1/64 of the circumference, warped so borders wander.
        let region = (n / 64).max(4);
        let region_warp = BlockFbm::new(
            derive_seed(seed, "region warp"),
            n as i64,
            region as f64 * 1.5,
            3,
            0.5,
        );
        let cells: Vec<u8> = (0..n * n)
            .into_par_iter()
            .map(|h| {
                let (i, j) = (h % n, h / n);
                let (fi, fj) = (2 * i + 1, (2 * j + 1).min(full_n - 1));
                let idx = geom.idx(fi.min(full_n - 1), fj);
                let (gx, gz) = (fi as f64, fj as f64);
                let precip = grid.precipitation.bilinear(gx, gz);
                let temp = grid.sea_level_temperature.bilinear(gx, gz);
                let coast = grid.coast.bilinear(gx, gz);
                let coastal = coast.abs() < 0.035;
                let mut flags_here = 0u8;
                if precip < 450.0 {
                    flags_here |= cond::ARID;
                }
                if precip > 750.0 {
                    flags_here |= cond::HUMID;
                }
                if temp > 18.0 {
                    flags_here |= cond::WARM;
                }
                if temp < 4.0 {
                    flags_here |= cond::COLD;
                }
                flags_here |= if coastal { cond::COASTAL } else { cond::INLAND };
                match grid.province[idx] {
                    code::OLD_OROGEN => flags_here |= cond::OLD,
                    code::OROGEN => flags_here |= cond::YOUNG,
                    _ => {}
                }
                let mut setting = setting_of(grid, idx, coastal);
                let wi = i as f64 + region_warp.sample2(i as f64, j as f64) * region as f64 * 0.4;
                let wj = j as f64
                    + region_warp.sample2(j as f64 + 5000.0, i as f64) * region as f64 * 0.4;
                // Large igneous provinces: some stable interiors were flooded by basalt from
                // mantle plumes (about a twelfth of cratons and basins, in regions twice the
                // usual size).
                if matches!(
                    setting,
                    TectonicSetting::Craton | TectonicSetting::SedimentaryBasin
                ) && !coastal
                {
                    let big = (region * 2) as f64;
                    let lip = hash_2d(
                        seed ^ 0x1195,
                        (wi / big).floor().rem_euclid((n as f64 / big).max(1.0)) as i32,
                        (wj / big).floor() as i32,
                    );
                    if unit_f32(lip) < 0.08 {
                        setting = TectonicSetting::Hotspot;
                    }
                }
                let key = hash_2d(
                    seed ^ setting as u64,
                    (wi / region as f64)
                        .floor()
                        .rem_euclid((n / region).max(1) as f64) as i32,
                    (wj / region as f64).floor() as i32,
                );
                Self::pick(&provinces, setting, flags_here, key)
            })
            .collect();
        // Datum: the planet's elevation at quarter resolution, box-blurred, in blocks, raised by
        // the thickness eroded off uplifted land.
        let elev_blocks = Field {
            n: grid.elevation.n,
            data: grid
                .elevation
                .data
                .iter()
                .map(|e| (*e as f64 * v) as f32)
                .collect(),
            scale: grid.elevation.scale,
        };
        let mut quarter = elev_blocks.downsample2().downsample2();
        let qn = quarter.n;
        for j in 0..qn {
            for i in 0..qn {
                let (gx, gz) = ((i * 4 + 2) as f64, (j * 4 + 2) as f64);
                let uplift = grid.uplift.bilinear(gx, gz).max(0.0) as f64;
                let e = quarter.get(i, j) + (uplift * v * EXHUMATION) as f32;
                quarter.set(i, j, e);
            }
        }
        let datum = blur3(&quarter);
        let wl = |x: f64| x.min(c / 4.0);
        Ok(Self {
            provinces,
            cells,
            n,
            cell: c / n as f64,
            circumference: c,
            datum,
            full_n,
            warp_x: BlockFbm::new(derive_seed(seed, "warp x"), circ, wl(220.0), 3, 0.5),
            warp_z: BlockFbm::new(derive_seed(seed, "warp z"), circ, wl(220.0), 3, 0.5),
            phase: BlockFbm::new(derive_seed(seed, "phase"), circ, wl(1600.0), 2, 0.5),
            thickness: BlockFbm::new(derive_seed(seed, "thickness"), circ, wl(4000.0), 2, 0.5),
            presence: BlockFbm::new(derive_seed(seed, "presence"), circ, wl(2600.0), 2, 0.5),
            pluton_shape: BlockFbm::new(derive_seed(seed, "pluton"), circ, wl(300.0), 2, 0.5),
            vertical_scale: v,
            seed,
        })
    }

    fn resolve(
        p: &Province,
        state: &impl Fn(&str) -> Result<BlockStateId, MissingBlock>,
        v: f64,
        seed: u64,
    ) -> Result<ProvinceModel, MissingBlock> {
        use hearth_content::schema::Entry;
        let blocks = |m: f32| (m as f64 * v) as f32;
        let layers = p
            .sequence
            .iter()
            .map(|l| {
                Ok(LayerModel {
                    rock: state(l.rock.as_str())?,
                    thickness: (
                        blocks(l.thickness_m.0).max(1.0),
                        blocks(l.thickness_m.1).max(1.0),
                    ),
                    presence: l.presence.clamp(0.0, 1.0),
                })
            })
            .collect::<Result<Vec<_>, MissingBlock>>()?;
        let intrusions = p
            .intrusions
            .iter()
            .map(|r| state(r.as_str()))
            .collect::<Result<Vec<_>, MissingBlock>>()?;
        let mut conditions = 0u8;
        for c in &p.conditions {
            match cond::parse(c) {
                Some(b) => conditions |= b,
                None => log::warn!("province {}: unknown condition {c:?}", p.id()),
            }
        }
        // Structure at the block scale: folds of a few hundred blocks in collision belts, broad
        // warps elsewhere, with the typical dip of the province on their flanks.
        let h = hash2(seed, hearth_math::hash::derive_seed(0, p.id()));
        let dip = (p.dip_deg.0 + p.dip_deg.1) * 0.5;
        let wavelength = if p.folded {
            300.0 + 600.0 * unit_f32(h)
        } else {
            2500.0 + 3500.0 * unit_f32(h)
        };
        let amplitude = (dip.to_radians().tan() * wavelength / std::f32::consts::TAU).min(400.0);
        Ok(ProvinceModel {
            id: p.id().to_owned(),
            name: p.name.clone(),
            setting: p.setting,
            layers,
            basement: state(p.basement.as_str())?,
            intrusions,
            intrusion_share: p.intrusion_share.clamp(0.0, 1.0),
            intrusion_roof: (blocks(p.intrusion_roof_m.0), blocks(p.intrusion_roof_m.1)),
            folded: p.folded,
            amplitude,
            wavelength,
            axis: unit_f32(hash2(h, 1)) * std::f32::consts::PI,
            conditions,
            weight: p.weight.max(0.0),
        })
    }

    /// Weighted choice among the provinces of a setting whose structural conditions hold;
    /// those whose climate conditions do not hold keep a small share of their weight.
    fn pick(provinces: &[ProvinceModel], setting: TectonicSetting, here: u8, key: u64) -> u8 {
        let eligible = |strict: bool| {
            provinces
                .iter()
                .enumerate()
                .filter(move |(_, p)| {
                    let structural = p.conditions & !cond::CLIMATE;
                    p.setting == setting
                        && (if strict {
                            structural & here == structural
                        } else {
                            structural == 0
                        })
                })
                .map(move |(i, p)| {
                    let climate = p.conditions & cond::CLIMATE;
                    let w = if climate & here == climate {
                        p.weight
                    } else {
                        p.weight * CLIMATE_MISMATCH
                    };
                    (i, w)
                })
        };
        let mut candidates: SmallVec<[(usize, f32); 8]> = eligible(true).collect();
        if candidates.is_empty() {
            candidates = eligible(false).collect();
        }
        if candidates.is_empty() {
            candidates = provinces
                .iter()
                .enumerate()
                .filter(|(_, p)| p.setting == setting)
                .map(|(i, p)| (i, p.weight))
                .collect();
        }
        let total: f32 = candidates.iter().map(|c| c.1).sum();
        if candidates.is_empty() || total <= 0.0 {
            return candidates.first().map_or(0, |c| c.0 as u8);
        }
        let mut r = unit_f32(key) * total;
        for (i, w) in &candidates {
            if r < *w {
                return *i as u8;
            }
            r -= w;
        }
        candidates[candidates.len() - 1].0 as u8
    }

    pub fn provinces(&self) -> &[ProvinceModel] {
        &self.provinces
    }

    /// The province under a world position: the province of the nearest of the cells' jittered
    /// sites, after a warp, so borders are irregular rather than following the grid.
    pub fn province_index(&self, x: f64, z: f64) -> u8 {
        let wx = x + self.warp_x.sample2(x, z) * self.cell * 1.2;
        let wz = z + self.warp_z.sample2(x, z) * self.cell * 1.2;
        let gx = wx.rem_euclid(self.circumference) / self.cell;
        let gz = ((wz + self.circumference * 0.5) / self.cell).max(0.0);
        let (ci, cj) = (gx.floor() as i64, gz.floor() as i64);
        let n = self.n as i64;
        let mut best = (f64::MAX, 0u8);
        for dj in -1..=1 {
            let j = cj + dj;
            if j < 0 || j >= n {
                continue;
            }
            for di in -1..=1 {
                let i = ci + di;
                let wrapped = i.rem_euclid(n);
                let h = hash_2d(self.seed ^ 0x517e, wrapped as i32, j as i32);
                let sx = i as f64 + 0.05 + 0.9 * unit_f32(h) as f64;
                let sz = j as f64 + 0.05 + 0.9 * unit_f32(hash2(h, 1)) as f64;
                let d = (gx - sx).powi(2) + (gz - sz).powi(2);
                if d < best.0 {
                    best = (d, self.cells[j as usize * self.n + wrapped as usize]);
                }
            }
        }
        best.1
    }

    pub fn province(&self, index: u8) -> &ProvinceModel {
        &self.provinces[index as usize]
    }

    /// The rock column at a block column.
    pub fn column(&self, x: i32, z: i32) -> RockColumn {
        let (xf, zf) = (x as f64 + 0.5, z as f64 + 0.5);
        let pi = self.province_index(xf, zf);
        let p = &self.provinces[pi as usize];
        // Datum: the regional surface.
        let (gx, gz) = (
            xf / (self.circumference / self.full_n as f64) - 0.5,
            (zf + self.circumference * 0.5) / (self.circumference / self.full_n as f64) - 0.5,
        );
        let datum = self.datum.bilinear(gx, gz) + DATUM_OFFSET;
        // Structure: folds across the province's axis (bent by the phase noise) in collision
        // belts; elsewhere irregular domes and basins.
        let structure = if p.folded {
            let u = xf as f32 * p.axis.cos() + zf as f32 * p.axis.sin();
            let phase = self.phase.sample2(xf, zf) as f32 * 3.0;
            let k = std::f32::consts::TAU / p.wavelength;
            p.amplitude * ((u * k + phase).sin() + 0.3 * (u * k * 2.3 + phase * 1.7).sin()) / 1.3
        } else {
            let s = p.wavelength as f64 / 1600.0;
            p.amplitude * self.phase.sample2(xf / s, zf / s + 911.0) as f32 * 1.6
        };
        let top = datum + structure;
        let mut bases: SmallVec<[(f32, BlockStateId); 8]> = SmallVec::new();
        let mut depth = 0.0f32;
        for (li, l) in p.layers.iter().enumerate() {
            let off = li as f64 * 7919.0;
            let pres = self.presence.sample2(xf + off, zf - off) as f32 * 0.5 + 0.5;
            // Layers taper out at the edge of where they are present.
            let taper = ((l.presence - pres) / 0.08).clamp(0.0, 1.0);
            if taper <= 0.0 {
                continue;
            }
            let t = self.thickness.sample2(xf - off, zf + off) as f32 * 0.5 + 0.5;
            let thickness = (l.thickness.0 + (l.thickness.1 - l.thickness.0) * t) * taper;
            if thickness < 0.5 {
                continue;
            }
            depth += thickness;
            bases.push((depth, l.rock));
        }
        RockColumn {
            province: pi,
            top,
            bases,
            basement: p.basement,
            intrusion: self.intrusion_at(xf, zf, datum),
        }
    }

    /// The highest pluton roof under a column, if any pluton's footprint covers it.
    fn intrusion_at(&self, x: f64, z: f64, datum: f32) -> Option<(BlockStateId, f32)> {
        let cx = (x / PLUTON_CELL as f64).floor() as i32;
        let cz = (z / PLUTON_CELL as f64).floor() as i32;
        let cells_around = (self.circumference / PLUTON_CELL as f64).round().max(1.0) as i32;
        let mut best: Option<(BlockStateId, f32)> = None;
        for dz in -1..=1 {
            for dx in -1..=1 {
                let (px, pz) = ((cx + dx).rem_euclid(cells_around), cz + dz);
                let h = hash_2d(self.seed ^ 0x9107, px, pz);
                // Centre, size and depth of the pluton.
                let ox = (cx + dx) as f64 * PLUTON_CELL as f64
                    + unit_f32(hash2(h, 1)) as f64 * PLUTON_CELL as f64;
                let oz = pz as f64 * PLUTON_CELL as f64
                    + unit_f32(hash2(h, 2)) as f64 * PLUTON_CELL as f64;
                let radius = 120.0 + 380.0 * unit_f32(hash2(h, 3)) as f64;
                let (ddx, ddz) = (x - ox, z - oz);
                let d = (ddx * ddx + ddz * ddz).sqrt() / radius;
                let d = d * (1.0 + 0.25 * self.pluton_shape.sample2(x, z));
                if d >= 1.0 {
                    continue;
                }
                let host = &self.provinces[self.province_index(ox, oz) as usize];
                if host.intrusions.is_empty() || unit_f32(h) > host.intrusion_share {
                    continue;
                }
                let rock = host.intrusions[(hash2(h, 4) % host.intrusions.len() as u64) as usize];
                let (lo, hi) = host.intrusion_roof;
                let roof_depth = lo + (hi - lo) * unit_f32(hash2(h, 5));
                // A flat roof with steep walls, a few hundred metres of relief at the edge.
                let wall = (d * d * d * d) as f32 * 600.0 * self.vertical_scale as f32;
                let roof = datum - roof_depth - wall;
                if best.is_none_or(|(_, r)| roof > r) {
                    best = Some((rock, roof));
                }
            }
        }
        best
    }

    /// Rock temperature (°C) at a depth below the surface, given the mean surface temperature.
    pub fn rock_temperature_c(&self, depth_blocks: f64, surface_mean_c: f64) -> f64 {
        surface_mean_c + GEOTHERMAL_C_PER_M * depth_blocks.max(0.0) / self.vertical_scale
    }
}

/// 3×3 box blur (X wraps, Z clamps).
fn blur3(f: &Field<f32>) -> Field<f32> {
    let n = f.n;
    let mut out = f.clone();
    for j in 0..n {
        for i in 0..n {
            let mut sum = 0.0;
            for dj in -1isize..=1 {
                for di in -1isize..=1 {
                    let ii = (i as isize + di).rem_euclid(n as isize) as usize;
                    let jj = (j as isize + dj).clamp(0, n as isize - 1) as usize;
                    sum += f.get(ii, jj);
                }
            }
            out.set(i, j, sum / 9.0);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, OnceLock};

    use hearth_math::PlanetSize;

    use super::*;
    use crate::settings::WorldGenSettings;

    struct Fixture {
        grid: Arc<PlanetGrid>,
        reg: BlockRegistry,
        geology: Geology,
    }

    fn fixture() -> &'static Fixture {
        static F: OnceLock<Fixture> = OnceLock::new();
        F.get_or_init(|| {
            let s = WorldGenSettings {
                seed: 3,
                planet_size: PlanetSize::Small,
                grid_resolution: 256,
                ..WorldGenSettings::default()
            };
            let grid = Arc::new(PlanetGrid::build(&s, &|_, _| {}));
            let content = Content::load_base();
            let reg = hearth_world::datapack::load_builtin_registry().expect("registry");
            let geology = Geology::new(&grid, &content, &reg).expect("geology");
            Fixture { grid, reg, geology }
        })
    }

    fn name(f: &Fixture, s: BlockStateId) -> String {
        f.reg.block_of(s).name.path().to_owned()
    }

    #[test]
    fn provinces_follow_the_tectonic_settings() {
        let f = fixture();
        let g = &f.grid;
        let full = g.geom.n;
        let mut seen = std::collections::BTreeSet::new();
        for j in 0..f.geology.n {
            for i in 0..f.geology.n {
                // The full-resolution cell each half cell was classified from.
                let (fi, fj) = ((2 * i + 1).min(full - 1), (2 * j + 1).min(full - 1));
                let idx = g.geom.idx(fi, fj);
                let p = f.geology.province(f.geology.cells[j * f.geology.n + i]);
                seen.insert(p.id.clone());
                if g.province[idx] == code::OCEANIC {
                    assert_eq!(
                        p.setting,
                        TectonicSetting::OceanFloor,
                        "deep sea at cell {fi},{fj}"
                    );
                } else {
                    assert_ne!(
                        p.setting,
                        TectonicSetting::OceanFloor,
                        "ocean floor off the deep sea at {fi},{fj}"
                    );
                }
            }
        }
        assert!(seen.len() >= 6, "a varied planet: {seen:?}");
    }

    /// Structural conditions are rules; climate conditions strong preferences (D45).
    #[test]
    fn conditioned_provinces_meet_their_conditions() {
        let f = fixture();
        let g = &f.grid;
        let (mut checked, mut arid_ok) = (0, 0);
        for j in 0..f.geology.n {
            for i in 0..f.geology.n {
                let p = f.geology.province(f.geology.cells[j * f.geology.n + i]);
                let (fi, fj) = ((2 * i + 1).min(g.n() - 1), (2 * j + 1).min(g.n() - 1));
                let here = g.province[g.geom.idx(fi, fj)];
                if p.conditions & cond::OLD != 0 {
                    assert_eq!(here, code::OLD_OROGEN, "{} only on old orogens", p.id);
                }
                if p.conditions & cond::YOUNG != 0 {
                    assert_eq!(here, code::OROGEN, "{} only on young orogens", p.id);
                }
                if p.conditions & cond::ARID != 0 {
                    checked += 1;
                    let precip = g
                        .precipitation
                        .bilinear((2 * i + 1) as f64, (2 * j + 1) as f64);
                    if precip < 450.0 {
                        arid_ok += 1;
                    }
                }
            }
        }
        assert!(checked > 0, "some arid basins");
        assert!(
            arid_ok as f64 >= 0.7 * checked as f64,
            "red beds and evaporites mostly where it is arid: {arid_ok} of {checked}"
        );
    }

    #[test]
    fn layers_come_in_order_down_to_the_basement() {
        let f = fixture();
        let mut rng = hearth_math::hash::Rng::new(7);
        let c = f.grid.planet().circumference();
        for _ in 0..200 {
            let x = rng.range_i32(0, c);
            let z = rng.range_i32(-c / 3, c / 3);
            let col = f.geology.column(x, z);
            let p = f.geology.province(col.province);
            // Walking down from the top of the sequence visits its layers in order and ends
            // in the basement (outside intrusions).
            if col.intrusion.is_some() {
                continue;
            }
            let order: Vec<BlockStateId> = p.layers.iter().map(|l| l.rock).collect();
            let mut last_index = 0usize;
            let mut y = col.top.floor() as i32;
            let bottom = y - 20_000;
            while y > bottom {
                let r = col.rock_at(y);
                if r == p.basement && !order[last_index..].contains(&r) {
                    break;
                }
                let k = order[last_index..]
                    .iter()
                    .position(|l| *l == r)
                    .expect("a layer of the sequence")
                    + last_index;
                assert!(
                    k >= last_index,
                    "{} above {}",
                    name(f, r),
                    name(f, order[last_index])
                );
                last_index = k;
                y -= 7;
            }
            assert_eq!(col.rock_at(bottom), p.basement, "basement under {}", p.id);
        }
    }

    #[test]
    fn every_documented_condition_is_understood() {
        for word in hearth_content::schema::geology::PROVINCE_CONDITIONS {
            assert!(cond::parse(word).is_some(), "{word}");
        }
    }

    #[test]
    fn deep_rock_is_hot() {
        let f = fixture();
        let v = f.grid.vertical_scale;
        // 1 km of real depth adds 25 °C.
        let t = f.geology.rock_temperature_c(1000.0 * v, 10.0);
        assert!((t - 35.0).abs() < 1e-9, "{t}");
    }
}

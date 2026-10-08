//! Regional sampling: pure, deterministic, thread-safe functions of (seed, x, z) giving the
//! block-resolution surface — height, water, rivers, lakes, biome, surface material, tree
//! density, snow and tree lines. Cube generation and the LOD system both build on these.

pub mod biome;
pub mod rivers;

use std::sync::Arc;

use glam::DVec3;

use hearth_math::Planet;
use hearth_math::hash::derive_seed;

use smallvec::SmallVec;

use crate::noise::BlockFbm;
use crate::planet::climate::{ClimateClass, LAPSE_RATE};
use crate::planet::{PlanetGrid, flags, province};
use crate::relief::{Patch, Reach, Relief};
use biome::{Biome, BiomeInputs};
use rivers::{BANK_HEIGHT, RiverHit, RiverNet, Segment, bank_width};

/// Top-layer material of a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Surface {
    Grass = 0,
    Podzol = 1,
    CoarseDirt = 2,
    Dirt = 3,
    Sand = 4,
    RedSand = 5,
    Gravel = 6,
    Stone = 7,
    Snow = 8,
    Ice = 9,
    Mud = 10,
    Clay = 11,
    Calcite = 12,
    Moss = 13,
    Sandstone = 14,
    RedSandstone = 15,
    Tuff = 16,
    SnowGrass = 17,
    /// Living reef over reef rock.
    Coral = 18,
}

/// Everything known about one block column's surface.
#[derive(Debug, Clone, Copy)]
pub struct ColumnSample {
    /// Terrain surface height in blocks (continuous). Blocks with `y < height_i()` are ground.
    pub height: f32,
    /// Water surface in blocks, or `NEG_INFINITY` when dry.
    pub water: f32,
    pub biome: Biome,
    pub climate: ClimateClass,
    /// Mean annual temperature at the surface (°C).
    pub temperature: f32,
    /// Warmest-month temperature at the surface (°C).
    pub t_warm: f32,
    pub precipitation: f32,
    /// Sea-surface temperature (for water colour and ocean biomes).
    pub sea_temperature: f32,
    /// Local slope (blocks per block).
    pub slope: f32,
    pub surface: Surface,
    /// Depth of the soil/filler layer under the top block.
    pub soil_depth: u8,
    /// Filler material under the top block.
    pub filler: Surface,
    /// 0..1 tree density.
    pub tree_density: f32,
    /// Snow line and tree line in blocks.
    pub snow_line: f32,
    pub tree_line: f32,
    /// River channel info if a river is near.
    pub river: Option<RiverHit>,
    pub ocean: bool,
    pub lake: bool,
    /// Geology province of the underlying rock.
    pub province: u8,
    /// 0..1 strength of 3D overhang/cliff shaping in this column.
    pub cliffiness: f32,
    /// The biogeographic realm.
    pub realm: crate::realms::Realm,
}

impl ColumnSample {
    /// First non-ground block (ground is `y < height_i`).
    #[inline]
    pub fn height_i(&self) -> i32 {
        self.height.round() as i32
    }

    /// First block above the water (water fills `height_i..water_i`).
    #[inline]
    pub fn water_i(&self) -> i32 {
        if self.water.is_finite() {
            self.water.round() as i32
        } else {
            i32::MIN
        }
    }

    pub fn is_underwater(&self) -> bool {
        self.water_i() > self.height_i()
    }
}

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Block-space detail noises.
#[derive(Debug, Clone)]
struct DetailNoise {
    hills: BlockFbm,
    mountains: BlockFbm,
    rough: BlockFbm,
    warp_x: BlockFbm,
    warp_z: BlockFbm,
    variation: BlockFbm,
    variation2: BlockFbm,
    soil: BlockFbm,
    patch: BlockFbm,
    cliff: BlockFbm,
    /// Relief below a refinement level's cells (wavelengths of 96 blocks and less).
    detail: BlockFbm,
}

impl DetailNoise {
    fn new(seed: u64, c: i64) -> Self {
        let d = |p: &str| derive_seed(seed, p);
        Self {
            hills: BlockFbm::new(d("hills"), c, 192.0, 4, 0.5),
            mountains: BlockFbm::new(d("mtn"), c, 256.0, 5, 0.5),
            rough: BlockFbm::new(d("rough"), c, 24.0, 2, 0.5),
            warp_x: BlockFbm::new(d("warpx"), c, 384.0, 3, 0.5),
            warp_z: BlockFbm::new(d("warpz"), c, 384.0, 3, 0.5),
            variation: BlockFbm::new(d("var"), c, 1200.0, 3, 0.5),
            variation2: BlockFbm::new(d("var2"), c, 700.0, 3, 0.5),
            soil: BlockFbm::new(d("soil"), c, 48.0, 2, 0.5),
            patch: BlockFbm::new(d("patch"), c, 40.0, 2, 0.5),
            cliff: BlockFbm::new(d("cliff"), c, 300.0, 2, 0.5),
            detail: BlockFbm::new(d("detail"), c, DETAIL_M, 4, 0.5),
        }
    }
}

/// The world generator's surface model for one world.
#[derive(Debug, Clone)]
pub struct Terrain {
    pub grid: Arc<PlanetGrid>,
    /// The biogeographic realms of the planet's landmasses.
    pub realms: Arc<crate::realms::Realms>,
    planet: Planet,
    /// Blocks per real metre.
    v: f32,
    rivers: RiverNet,
    noise: DetailNoise,
    seed: u64,
    /// The refinement levels between the grid and the blocks (an Earth-sized planet's; a small
    /// test planet's grid is fine enough without them).
    relief: Option<Arc<Relief>>,
    /// Neighbourhoods read for single samples on a refined planet, by square of
    /// [`NEAR_SQUARE`] blocks (trees and other features sample many columns close together).
    near: Arc<NearCache>,
}

/// Side (blocks) of the squares whose neighbourhoods single samples share.
const NEAR_SQUARE: i32 = 64;

/// Neighbourhoods kept for single samples.
struct NearCache(crate::cubegen::cache::Cache<(i32, i32), Nearby>);

impl std::fmt::Debug for NearCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NearCache")
            .field("len", &self.0.len())
            .finish()
    }
}

/// What the sampler reads about a rectangle of columns, once for all of them: the rivers near
/// it and, on a planet with refinement levels, one level's cells over it.
#[derive(Debug, Clone, Default)]
pub struct Nearby {
    pub segments: SmallVec<[Segment; 16]>,
    patch: Option<Patch>,
}

/// The surface of a column before the climate and the living things: its ground, its water
/// and the river by it.
struct Shape {
    h: f32,
    slope: f32,
    water: f32,
    lake: bool,
    ocean_near: bool,
    river: Option<RiverHit>,
}

/// The longest wavelength (m) of the relief the blocks add below a refinement level's cells.
const DETAIL_M: f64 = 96.0;

/// Latitude where the flat polar ice plateau starts blending in.
const POLAR_BLEND_START: f64 = 80.5;

/// Minimum local water fraction for sea/lake water to fill low ground.
const FLOOD_FRACTION: f32 = 0.08;

/// Cells of a refinement level read beyond a rectangle for its rivers: a river is taken from
/// the finest level whose margin holds its banks, a wider one from the level above.
const RIVER_MARGIN: i64 = 6;

/// A river's width (m) from its discharge (m³/s): w = 4 Q^0.5 (Leopold and Maddock 1953's
/// hydraulic geometry; a brook of a cubic metre a second some 4 m across, the lower
/// Mississippi's 17,000 about half a kilometre).
pub fn river_width_m(q: f32) -> f32 {
    4.0 * q.max(0.0).sqrt()
}

/// A river's mean depth (m) from its discharge (m³/s): d = 0.27 Q^0.39 (the same).
pub fn river_depth_m(q: f32) -> f32 {
    0.27 * q.max(0.0).powf(0.39)
}

/// The discharge (m³/s) of a river `w` metres wide.
fn river_q_for_width(w: f32) -> f32 {
    (w.max(0.0) / 4.0).powi(2)
}

impl Terrain {
    pub fn new(grid: Arc<PlanetGrid>) -> Self {
        let planet = *grid.planet();
        let rivers = RiverNet::build(&grid);
        let noise = DetailNoise::new(grid.seed, planet.circumference() as i64);
        let relief = Relief::new(grid.clone());
        Self {
            v: grid.vertical_scale as f32,
            planet,
            rivers,
            noise,
            seed: grid.seed,
            realms: Arc::new(crate::realms::Realms::new(&grid)),
            relief: (!relief.levels().is_empty()).then(|| Arc::new(relief)),
            near: Arc::new(NearCache(crate::cubegen::cache::Cache::new(1024))),
            grid,
        }
    }

    /// The refinement levels, if the planet has them.
    pub fn relief(&self) -> Option<&Relief> {
        self.relief.as_deref()
    }

    pub fn planet(&self) -> &Planet {
        &self.planet
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn rivers(&self) -> &RiverNet {
        &self.rivers
    }

    /// Blocks per real metre of relief.
    pub fn vertical_scale(&self) -> f32 {
        self.v
    }

    /// What the sampler reads about a rectangle of block columns (for batch sampling).
    pub fn nearby(&self, x0: i32, z0: i32, x1: i32, z1: i32) -> Nearby {
        self.nearby_scaled(x0, z0, x1, z1, 1.0)
    }

    /// As [`Self::nearby`], for columns `scale` blocks apart (a far tile's): the refinement
    /// level whose cells suit them, and the rivers wide enough to show at that spacing.
    pub fn nearby_scaled(&self, x0: i32, z0: i32, x1: i32, z1: i32, scale: f64) -> Nearby {
        let Some(relief) = &self.relief else {
            return Nearby {
                segments: self
                    .rivers
                    .segments_near(x0 as f64, z0 as f64, x1 as f64, z1 as f64, 90.0),
                patch: None,
            };
        };
        let (lo, hi) = ((x0 as f64, z0 as f64), (x1 as f64 + 1.0, z1 as f64 + 1.0));
        let finest = relief.level_for(scale);
        let patch = relief.patch(finest, lo, hi, RIVER_MARGIN);
        // The narrowest river that shows between columns this far apart (and no brook
        // narrower than a metre and a half).
        let mut min_q = river_q_for_width((0.5 * scale as f32 / self.v).max(1.5));
        let mut segments = SmallVec::new();
        // Each level gives the rivers whose banks its margin holds; wider ones come from the
        // level above, down to the grid's.
        let mut level = finest;
        let mut above: Option<Patch> = None;
        loop {
            let cells = above.as_ref().unwrap_or(&patch);
            let reach = (RIVER_MARGIN as f64 * cells.cell_size()) as f32 / self.v;
            // A river's banks reach 1.4 widths and 3 m from its middle.
            let max_q = if level == 0 {
                f32::INFINITY
            } else {
                river_q_for_width((reach - 3.0) / 1.4)
            };
            if max_q > min_q {
                for r in relief.reaches(cells, min_q) {
                    if r.q_a < max_q {
                        segments.push(self.segment(&r));
                    }
                }
            }
            if level == 0 || max_q.is_infinite() {
                break;
            }
            min_q = min_q.max(max_q);
            level -= 1;
            above = Some(relief.patch(level, lo, hi, RIVER_MARGIN));
        }
        Nearby {
            segments,
            patch: Some(patch),
        }
    }

    /// A refinement level's reach as a channel the sampler carves.
    fn segment(&self, r: &Reach) -> Segment {
        let v = self.v;
        Segment {
            cell: r.grid,
            ax: r.a.0,
            az: r.a.1,
            bx: r.b.0,
            bz: r.b.1,
            level_a: r.level_a,
            level_b: r.level_b,
            width_a: river_width_m(r.q_a) * v,
            width_b: river_width_m(r.q_b) * v,
            depth_a: river_depth_m(r.q_a) * v,
            depth_b: river_depth_m(r.q_b) * v,
        }
    }

    /// Samples one column (convenience; batch callers should read the neighbourhood once). On
    /// a refined planet the neighbourhood is its square's, kept for the columns about it.
    pub fn sample(&self, x: i32, z: i32) -> ColumnSample {
        if self.relief.is_some() {
            let key = (x.div_euclid(NEAR_SQUARE), z.div_euclid(NEAR_SQUARE));
            let near = self.near.0.get_or_insert_with(key, || {
                let (x0, z0) = (key.0 * NEAR_SQUARE, key.1 * NEAR_SQUARE);
                self.nearby(x0, z0, x0 + NEAR_SQUARE - 1, z0 + NEAR_SQUARE - 1)
            });
            return self.sample_with(x, z, &near);
        }
        let near = self.nearby(x, z, x, z);
        self.sample_with(x, z, &near)
    }

    /// Samples one column of a far tile, its columns `scale` blocks apart.
    pub fn sample_scaled(&self, x: i32, z: i32, scale: f64) -> ColumnSample {
        let near = self.nearby_scaled(x, z, x, z, scale);
        self.sample_with(x, z, &near)
    }

    /// Water level at a column (NEG_INFINITY when dry).
    pub fn water_level(&self, x: i32, z: i32) -> f32 {
        self.sample(x, z).water
    }

    /// A column as a survey of the whole planet sees it: on a refined planet, at the grid's
    /// own scale (so a search across the world reads no finer cells), else as it is.
    pub fn survey(&self, x: i32, z: i32) -> ColumnSample {
        if self.relief.is_some() {
            self.sample_scaled(x, z, self.grid.geom.cell)
        } else {
            self.sample(x, z)
        }
    }

    /// Macro height (grid bicubic + large detail) without rivers or fine detail: used for
    /// slope estimates.
    fn base_height(&self, x: f64, z: f64, gx: f64, gz: f64) -> (f32, f32) {
        let g = &self.grid;
        let e_m = g.elevation.bicubic(gx, gz);
        let uplift = g.uplift.bilinear(gx, gz).max(0.0);
        let mut h = e_m * self.v;
        let land_w = smoothstep(-8.0, 6.0, h);
        let mtn = smoothstep(150.0, 1400.0, uplift);
        let relief = (uplift * self.v * 0.07).clamp(3.0, 48.0);
        let hills = self.noise.hills.sample2(x, z) as f32;
        let ridges = self.noise.mountains.ridged2(x, z) as f32 - 0.42;
        // Hills in the lowlands, ridged crags in mountains, gentle undulation on the sea floor.
        // Near sea level the detail is small so coastlines follow the planet model.
        let lowland_amp = 1.2 + 1.3 * smoothstep(1.0, 8.0, h) + 5.5 * smoothstep(20.0, 160.0, h);
        h += hills * (lowland_amp * (1.0 - mtn) * land_w + 2.0 * (1.0 - land_w));
        h += ridges * relief * mtn * land_w * 2.0;
        (h, mtn)
    }

    /// The column's surface from the grid and the block-scale noises (a planet without
    /// refinement levels).
    fn grid_shape(&self, xf: f64, zf: f64, gx: f64, gz: f64, segs: &[Segment]) -> Shape {
        let g = &*self.grid;
        let (mut h, mtn) = self.base_height(xf, zf, gx, gz);
        // Slope from a small finite difference of the macro surface.
        let step = 4.0;
        let (hx, _) = self.base_height(xf + step, zf, gx + step / g.geom.cell, gz);
        let (hz, _) = self.base_height(xf, zf + step, gx, gz + step / g.geom.cell);
        let slope = (((hx - h) / step as f32).powi(2) + ((hz - h) / step as f32).powi(2)).sqrt();
        h += self.noise.rough.sample2(xf, zf) as f32 * (0.35 + 1.2 * mtn);
        let crater_lake = self.craters(xf, zf, &mut h);

        let (ocean_frac, lake_frac, lake_level) = self.nearby_water(gx, gz);
        let ocean_near = ocean_frac > FLOOD_FRACTION;
        let mut water = f32::NEG_INFINITY;
        let mut lake = false;
        if ocean_near {
            if h < 0.0 {
                water = 0.0;
            }
        } else if ocean_frac > 0.0 && h < 0.6 {
            // Berm between the sea and low land that is not connected to it.
            h = 0.6;
        }
        if let Some(level) = lake_level {
            if lake_frac > FLOOD_FRACTION {
                if h < level {
                    water = water.max(level);
                    lake = true;
                }
            } else if h < level + 0.6 {
                h = level + 0.6;
            }
        }
        if h < crater_lake {
            water = water.max(crater_lake);
            lake = true;
        }
        let river = self.carve_river(xf, zf, g.geom.cell * 0.35, segs, &mut h, &mut water);
        Shape {
            h,
            slope,
            water,
            lake,
            ocean_near,
            river,
        }
    }

    /// The column's surface from a refinement level's cells: their surface with the blocks'
    /// own roughness on it, the level's sea and lakes, and its rivers carved at their width.
    fn refined_shape(
        &self,
        xf: f64,
        zf: f64,
        gx: f64,
        gz: f64,
        patch: &Patch,
        segs: &[Segment],
    ) -> Shape {
        let g = &*self.grid;
        // Below the level's cells, the blocks' own relief, as rough as the place is.
        let rough = self.relief.as_deref().map_or(0.0, |r| r.relief_at(gx, gz));
        let amp = (crate::relief::amplitude(DETAIL_M) * rough) as f32 * self.v;
        let surface =
            |x: f64, z: f64| patch.height(x, z) + self.noise.detail.sample2(x, z) as f32 * amp;
        let mut h = surface(xf, zf);
        let step = 4.0;
        let hx = surface(xf + step, zf);
        let hz = surface(xf, zf + step);
        let slope = (((hx - h) / step as f32).powi(2) + ((hz - h) / step as f32).powi(2)).sqrt();
        let mtn = smoothstep(150.0, 1400.0, g.uplift.bilinear(gx, gz).max(0.0));
        h += self.noise.rough.sample2(xf, zf) as f32 * (0.35 + 1.2 * mtn);
        let crater_lake = self.craters(xf, zf, &mut h);

        // The sea over low ground about the level's sea; a lake to its surface about its cells.
        let (sea, lakes, lake_level) = patch.water(xf, zf);
        let ocean_near = sea > 0.0;
        let mut water = f32::NEG_INFINITY;
        let mut lake = false;
        if ocean_near && h < 0.0 {
            water = 0.0;
        }
        if lakes > 0.0
            && let Some(level) = lake_level
            && h < level
        {
            water = water.max(level);
            lake = true;
        }
        if h < crater_lake {
            water = water.max(crater_lake);
            lake = true;
        }
        let river = self.carve_river(xf, zf, patch.cell_size() * 0.35, segs, &mut h, &mut water);
        Shape {
            h,
            slope,
            water,
            lake,
            ocean_near,
            river,
        }
    }

    /// Volcano craters: a bowl carved into the summit about a point; the surface of the crater
    /// lake there, if one stands in it (else −∞).
    fn craters(&self, xf: f64, zf: f64, h: &mut f32) -> f32 {
        let mut crater_lake = f32::NEG_INFINITY;
        for vol in &self.grid.volcanoes {
            let dx = self.planet.delta_x(vol.x, xf) as f32;
            let dz = (zf - vol.z) as f32;
            let r = vol.crater_radius * 1.4;
            if dx.abs() >= r || dz.abs() >= r {
                continue;
            }
            let d = (dx * dx + dz * dz).sqrt() / vol.crater_radius;
            let rim = vol.summit * self.v;
            if d >= 1.4 || *h < rim - vol.crater_depth * 3.0 {
                continue;
            }
            let bowl = rim - vol.crater_depth * (1.0 - d.min(1.0).powi(2));
            let t = smoothstep(1.4, 0.9, d);
            *h += (h.min(bowl) - *h) * t;
            if vol.crater_lake && d < 0.8 {
                crater_lake = crater_lake.max(rim - vol.crater_depth * 0.45);
            }
        }
        crater_lake
    }

    /// Rivers: the channel nearest a point (warped by up to `warp` blocks, so channels wander
    /// between their nodes) carved to its depth, its banks and floodplain shaped about it.
    fn carve_river(
        &self,
        xf: f64,
        zf: f64,
        warp: f64,
        segs: &[Segment],
        h: &mut f32,
        water: &mut f32,
    ) -> Option<RiverHit> {
        if segs.is_empty() {
            return None;
        }
        let plain = *h;
        let wx = xf + self.noise.warp_x.sample2(xf, zf) * warp;
        let wz = zf + self.noise.warp_z.sample2(xf, zf) * warp;
        let seg_cx = (segs[0].ax + segs[0].bx) * 0.5;
        let wx = seg_cx + self.planet.delta_x(seg_cx, wx);
        let hit = RiverNet::closest(wx, wz, segs)?;
        let half = hit.width * 0.5;
        let bank_w = bank_width(hit.width);
        if hit.distance >= half + bank_w {
            return None;
        }
        let bank = hit.level + BANK_HEIGHT;
        if hit.distance < half {
            let t = hit.distance / half;
            let bed = hit.level - hit.depth * (1.0 - t * t) - 0.3;
            *h = h.min(bed);
            *water = water.max(hit.level);
        } else {
            let t = (hit.distance - half) / bank_w;
            let w = 1.0 - smoothstep(0.0, 1.0, t);
            *h += (bank - *h) * w;
        }
        Some(RiverHit { plain, ..hit })
    }

    /// Samples one column with what was read about its neighbourhood.
    pub fn sample_with(&self, x: i32, z: i32, near: &Nearby) -> ColumnSample {
        let g = &*self.grid;
        let xf = x as f64 + 0.5;
        let zf = z as f64 + 0.5;
        let (gx, gz) = g.geom.grid_coords(self.planet.wrap_xf(xf), zf);
        let lat = self.planet.latitude_deg(zf);
        let polar = ((lat.abs() - POLAR_BLEND_START) / 2.5).clamp(0.0, 1.0) as f32;
        let idx = g.cell_at(self.planet.wrap_xf(xf), zf);
        let cell_flags = g.flags[idx];
        let Shape {
            mut h,
            slope,
            mut water,
            lake,
            ocean_near,
            river: river_hit,
        } = match &near.patch {
            Some(patch) => self.refined_shape(xf, zf, gx, gz, patch, &near.segments),
            None => self.grid_shape(xf, zf, gx, gz, &near.segments),
        };

        // Polar plateau: flat and featureless.
        if polar > 0.0 {
            let cap = g.elevation.bilinear(gx, gz) * self.v;
            h += (cap - h) * polar;
        }

        // ------------------------------------------------------------ climate at the column
        // Gentle regional perturbations so climate borders wander naturally.
        let wobble = self.noise.variation.sample2(xf * 0.37, zf * 0.37) as f32;
        let wobble2 = self.noise.variation2.sample2(zf * 0.41, xf * 0.41) as f32;
        let sea_t = g.sea_level_temperature.bilinear(gx, gz) + wobble * 1.4;
        let range = g.temp_range.bilinear(gx, gz);
        let precip = g.precipitation.bilinear(gx, gz) * (1.0 + wobble2 * 0.18);
        let alt_m = (h.max(0.0) / self.v) as f64;
        let temperature = sea_t - (LAPSE_RATE * alt_m) as f32;
        let t_warm = temperature + range * 0.5;
        let t_warm_sl = sea_t + range * 0.5;
        // Tree line where the warmest month reaches ~10.5 °C; permanent snow where it stays
        // below ~0.5 °C, lower on wet slopes.
        let tree_line = ((t_warm_sl - 10.5) / LAPSE_RATE as f32) * self.v;
        let wet = ((precip - 900.0) / 2500.0).clamp(0.0, 0.35);
        let snow_line = ((t_warm_sl - 0.5) / LAPSE_RATE as f32) * self.v * (1.0 - wet);
        let climate = crate::planet::climate::classify(
            temperature as f64,
            range as f64,
            precip as f64,
            crate::planet::climate::dry_season::from_strengths(
                g.winter_dry.bilinear(gx, gz),
                g.summer_dry.bilinear(gx, gz),
            ),
        );

        // ------------------------------------------------------------ coasts
        // Sheltered stretches of coast (bays, estuaries, lagoons): long, slow noise, and every
        // river mouth.
        let (ci, cj) = g.geom.ij(idx);
        let delta_near = (-1..=1).any(|dj| {
            (-1..=1).any(|di| {
                g.geom
                    .neighbor(ci, cj, di, dj)
                    .is_some_and(|k| g.flags[k] & flags::DELTA != 0)
            })
        });
        let shelter_n = self.noise.variation.sample2(xf * 0.45 + 7_000.0, zf * 0.45) as f32;
        let mut shelter = if delta_near {
            1.0
        } else {
            smoothstep(0.05, 0.3, shelter_n)
        };
        // Coral reefs grow up to just below the surface in warm, clear, shallow sea: fringing
        // reefs along the shore, barrier reefs some way out with a lagoon behind, and atolls
        // around the drowned hotspot islands.
        let mut reef = false;
        if ocean_near && !lake && water == 0.0 && h < -0.5 && sea_t > 21.0 && !delta_near {
            let depth_m = -h / self.v;
            if depth_m < 60.0 {
                let n1 = (self.noise.variation2.sample2(xf * 1.7, zf * 1.7) as f32 * 0.5 + 0.5)
                    .clamp(0.0, 1.0);
                let n2 = (self.noise.variation.sample2(zf * 1.3 + 900.0, xf * 1.3) as f32 * 0.5
                    + 0.5)
                    .clamp(0.0, 1.0);
                let blocks_per_radian =
                    (self.planet.circumference_f64() / std::f64::consts::TAU) as f32;
                let off = -g.coast.bilinear(gx, gz) * blocks_per_radian;
                // Fringing reefs need clear water: not in muddy sheltered bays.
                let fringing = depth_m < 25.0 && off < 50.0 + 110.0 * n1 && shelter < 0.5;
                let barrier =
                    depth_m < 45.0 && (off - (320.0 + 280.0 * n2)).abs() < 10.0 + 18.0 * n1;
                let atoll = depth_m < 60.0
                    && g.volcanoes.iter().any(|vol| {
                        if vol.summit >= 0.0 {
                            return false;
                        }
                        let dx = self.planet.delta_x(vol.x, xf) as f32;
                        let dz = (zf - vol.z) as f32;
                        let ring = 70.0 + vol.crater_radius * 2.0;
                        ((dx * dx + dz * dz).sqrt() - ring).abs() < 8.0 + 6.0 * n1
                    });
                if fringing || barrier || atoll {
                    reef = true;
                    // Coral heads reach the surface in places, spurs and grooves lie deeper.
                    let heads = self.noise.rough.sample2(xf * 0.7, zf * 0.7) as f32 * 0.5 + 0.5;
                    h = h.max(-1.0 - 2.5 * (1.0 - heads).clamp(0.0, 1.0));
                    // The crest takes the waves.
                    shelter = 0.0;
                }
            }
        }

        // ------------------------------------------------------------ biome
        let variation = (self.noise.variation.sample2(xf, zf) as f32 * 0.5 + 0.5).clamp(0.0, 1.0);
        let variation2 = (self.noise.variation2.sample2(xf, zf) as f32 * 0.5 + 0.5).clamp(0.0, 1.0);
        let ocean = ocean_near && water.is_finite() && h < 0.0 && !lake;
        let near_ocean = ocean_near && h < 6.0;
        let inputs = BiomeInputs {
            climate,
            temperature,
            t_warm,
            precipitation: precip,
            height: h,
            water,
            slope,
            above_tree_line: h - tree_line,
            above_snow_line: h - snow_line,
            ocean,
            sea_temperature: sea_t,
            near_ocean,
            river: river_hit.is_some_and(|r| r.distance < r.width * 0.5),
            lake,
            salt_flat: cell_flags & flags::SALT_FLAT != 0,
            volcanic: cell_flags & flags::VOLCANIC != 0,
            variation,
            variation2,
            vertical_scale: self.v,
            shelter,
            bank: river_hit.is_some_and(|r| r.distance >= r.width * 0.5),
        };
        let biome = if polar > 0.5 {
            if h < 1.0 {
                Biome::PolarSea
            } else {
                Biome::IceSheet
            }
        } else {
            biome::select(&inputs)
        };

        // Pools in the wetlands: hollows of standing water a block or two deep among the sedges
        // and the reeds.
        if biome == Biome::Wetland && !(water.is_finite() && h < water - 0.5) {
            let pool = self.noise.patch.sample2(xf + 5_000.0, zf - 3_000.0) as f32;
            if pool > 0.2 {
                let ground = h.round();
                water = water.max(ground);
                h = ground - if pool > 0.5 { 2.0 } else { 1.0 };
            }
        }

        // ------------------------------------------------------------ materials
        let (surface, filler, soil_depth) = if reef {
            (Surface::Coral, Surface::Coral, 2)
        } else {
            self.materials(biome, h, water, slope, xf, zf, temperature, precip, shelter)
        };
        let tree_density = self.tree_density(
            biome,
            temperature,
            t_warm,
            precip,
            slope,
            h - tree_line,
            variation,
        );
        let cliffiness = smoothstep(1.1, 2.2, slope)
            * (0.5 + 0.5 * self.noise.cliff.sample2(xf, zf) as f32).clamp(0.0, 1.0);
        if polar > 0.9 && h < 0.5 {
            // Sea ice over the polar sea.
            water = water.max(0.0);
        }
        ColumnSample {
            height: h,
            water,
            biome,
            climate,
            temperature,
            t_warm,
            precipitation: precip,
            sea_temperature: sea_t,
            slope,
            surface,
            soil_depth,
            filler,
            tree_density,
            snow_line,
            tree_line,
            river: river_hit,
            ocean,
            lake,
            province: g.province[idx],
            cliffiness,
            realm: self.realms.realm_at(self.planet.wrap_xf(xf), zf),
        }
    }

    /// Smooth ocean and lake fractions around a point (tent-weighted over the 4×4 nearest
    /// cells, so flooding boundaries are smooth curves rather than grid squares), plus the
    /// level of the nearest lake.
    fn nearby_water(&self, gx: f64, gz: f64) -> (f32, f32, Option<f32>) {
        let g = &*self.grid;
        let n = g.n() as isize;
        let i0 = gx.floor() as isize;
        let j0 = gz.floor() as isize;
        let (mut ocean, mut lakes, mut total) = (0.0f32, 0.0f32, 0.0f32);
        let mut lake: Option<(f64, f32)> = None;
        for dj in -1..=2 {
            let j = (j0 + dj).clamp(0, n - 1);
            for di in -1..=2 {
                let i = (i0 + di).rem_euclid(n);
                let idx = (j * n + i) as usize;
                let f = g.flags[idx];
                let dx = gx - (i0 + di) as f64;
                let dz = gz - (j0 + dj) as f64;
                // Tent weight reaching 1.8 cells.
                let w = ((1.0 - dx.abs() / 1.8).max(0.0) * (1.0 - dz.abs() / 1.8).max(0.0)) as f32;
                total += w;
                if f & flags::OCEAN != 0 {
                    ocean += w;
                }
                if f & flags::LAKE != 0 {
                    lakes += w;
                    let d2 = dx * dx + dz * dz;
                    let level = g.water.data[idx] * self.v;
                    if lake.is_none_or(|(bd, _)| d2 < bd) {
                        lake = Some((d2, level));
                    }
                }
            }
        }
        let t = total.max(1e-6);
        (ocean / t, lakes / t, lake.map(|l| l.1))
    }

    #[allow(clippy::too_many_arguments)]
    fn materials(
        &self,
        biome: Biome,
        h: f32,
        water: f32,
        slope: f32,
        x: f64,
        z: f64,
        temperature: f32,
        precip: f32,
        shelter: f32,
    ) -> (Surface, Surface, u8) {
        let soil_n = self.noise.soil.sample2(x, z) as f32;
        let patch = self.noise.patch.sample2(x, z) as f32;
        let underwater = water.is_finite() && h < water - 0.5;
        // Soil depth: thick in valleys and wet climates, thin on ridges, none on cliffs.
        let wetness = (precip / 1500.0).clamp(0.2, 1.3);
        let mut depth = (3.0 * wetness + soil_n * 1.5 - slope * 2.2).clamp(0.0, 6.0);
        if slope > 1.0 {
            depth = 0.0;
        }
        let d = depth.round() as u8;
        use Surface::*;
        if underwater {
            let depth_below = water - h;
            // Tidal mud in the shallow water of sheltered coasts.
            let mudflat = shelter > 0.5
                && depth_below < 1.6
                && matches!(
                    biome,
                    Biome::WarmShallows | Biome::TemperateSea | Biome::ColdSea | Biome::Mangrove
                );
            let s = match biome {
                _ if mudflat => Dirt,
                Biome::WarmShallows => Sand,
                Biome::River => {
                    if patch > 0.2 {
                        Gravel
                    } else if patch < -0.3 {
                        Clay
                    } else {
                        Sand
                    }
                }
                Biome::Lake => {
                    if patch > 0.3 {
                        Gravel
                    } else if temperature > 5.0 && patch < -0.2 {
                        Clay
                    } else {
                        Dirt
                    }
                }
                Biome::DeepOcean | Biome::Trench => {
                    if patch > 0.3 {
                        Gravel
                    } else {
                        Clay
                    }
                }
                Biome::ColdSea | Biome::PolarSea => {
                    if depth_below < 30.0 && patch < 0.0 {
                        Sand
                    } else {
                        Gravel
                    }
                }
                _ => {
                    if depth_below > 60.0 {
                        if patch > 0.2 { Gravel } else { Clay }
                    } else if patch > 0.45 {
                        Gravel
                    } else {
                        Sand
                    }
                }
            };
            return (s, s, 3);
        }
        if slope > 1.35 && !matches!(biome, Biome::Glacier | Biome::IceSheet) {
            return (Stone, Stone, 0);
        }
        let (top, fill) = match biome {
            Biome::SaltMarsh | Biome::Mangrove => (Dirt, Dirt),
            Biome::Beach => (Sand, Sand),
            Biome::StonyShore => (if patch > 0.0 { Gravel } else { Stone }, Stone),
            Biome::DuneSea => (if patch > 0.55 { RedSand } else { Sand }, Sand),
            Biome::HotDesert => {
                if patch > 0.35 {
                    (Gravel, Gravel)
                } else if patch < -0.45 {
                    (RedSand, RedSand)
                } else {
                    (Sand, Sand)
                }
            }
            Biome::Mesa => (RedSand, RedSandstone),
            Biome::Oasis => (Grass, Dirt),
            Biome::SaltFlat => (Calcite, Sand),
            Biome::ColdDesert => (if patch > 0.1 { Gravel } else { CoarseDirt }, Dirt),
            Biome::Steppe | Biome::Savanna => (if patch > 0.55 { CoarseDirt } else { Grass }, Dirt),
            Biome::MediterraneanScrub => (if patch > 0.5 { CoarseDirt } else { Grass }, Dirt),
            Biome::BorealForest => (if patch > 0.1 { Podzol } else { Grass }, Dirt),
            Biome::SnowyTaiga => (SnowGrass, Dirt),
            Biome::Tundra => {
                if patch > 0.45 {
                    (CoarseDirt, Dirt)
                } else if temperature < -8.0 {
                    (SnowGrass, Dirt)
                } else {
                    (Grass, Dirt)
                }
            }
            Biome::IceSheet | Biome::Glacier => (Snow, Ice),
            Biome::AlpineRock => (if patch > 0.2 { Gravel } else { Stone }, Stone),
            Biome::AlpineMeadow => (Grass, Dirt),
            // The bogs' sphagnum over their peat where the summers are cool; the marshes' and
            // the swamps' mud and grass.
            Biome::Wetland if temperature < 8.0 => (if patch > -0.4 { Moss } else { Grass }, Mud),
            Biome::Wetland => (if patch > 0.25 { Mud } else { Grass }, Mud),
            Biome::TropicalRainforest => (if patch > 0.6 { Moss } else { Grass }, Dirt),
            Biome::TemperateRainforest => (if patch > 0.35 { Moss } else { Grass }, Dirt),
            Biome::Volcanic => (if patch > 0.0 { Tuff } else { Stone }, Stone),
            _ => (Grass, Dirt),
        };
        let depth = match biome {
            Biome::DuneSea => d.max(4) + 4,
            Biome::HotDesert | Biome::Beach => d.max(3),
            Biome::AlpineRock => 0,
            Biome::IceSheet | Biome::Glacier => d.max(3) + 6,
            _ => d,
        };
        (top, fill, depth)
    }

    #[allow(clippy::too_many_arguments)]
    fn tree_density(
        &self,
        biome: Biome,
        temperature: f32,
        t_warm: f32,
        precip: f32,
        slope: f32,
        above_tree_line: f32,
        variation: f32,
    ) -> f32 {
        // Climate capacity: moisture minus evaporative demand, limited by summer warmth.
        let moisture = smoothstep(250.0, 1100.0, precip - 12.0 * temperature.max(0.0));
        let warmth = smoothstep(10.0, 15.0, t_warm);
        let capacity = moisture * warmth;
        let base = match biome {
            Biome::TropicalRainforest => 1.0,
            Biome::TemperateRainforest => 0.9,
            Biome::BroadleafForest | Biome::BirchForest | Biome::MixedForest => 0.75,
            Biome::BorealForest | Biome::MontaneForest => 0.7,
            Biome::SnowyTaiga => 0.5,
            Biome::Krummholz => 0.35,
            Biome::Wetland => 0.25,
            Biome::Oasis => 0.35,
            Biome::Mangrove => 0.9,
            // Palms along the tropical shore, pines on cooler dunes, here and there.
            Biome::Beach => 0.08,
            Biome::MediterraneanScrub => 0.14,
            Biome::Savanna => 0.05,
            Biome::TemperatePlains => 0.035,
            Biome::AlpineMeadow => 0.01,
            Biome::Steppe => 0.004,
            _ => 0.0,
        };
        // Thin toward the tree line and on steep slopes; forest cores denser than edges.
        let alpine = smoothstep(0.0, -60.0, above_tree_line);
        let steep = 1.0 - smoothstep(0.7, 1.3, slope);
        let core = 0.7 + 0.6 * variation;
        (base * capacity.max(0.25) * alpine * steep * core).clamp(0.0, 1.0)
    }

    /// Finds a spawn point: warm-temperate lowland (subtropical, mediterranean or oceanic
    /// climate, 25–45° from the equator) near a coast or river, where a body in a loincloth can
    /// live through the first nights (or anywhere on land for `random`). Deterministic.
    pub fn find_spawn(&self, random: bool) -> (i32, i32) {
        let g = &*self.grid;
        let n = g.n();
        let mut rng = hearth_math::hash::Rng::new(derive_seed(self.seed, "spawn"));
        let mut best: Option<(f32, i32, i32)> = None;
        for attempt in 0..4000 {
            let i = rng.below(n as u32) as usize;
            let j = rng.below(n as u32) as usize;
            let idx = g.geom.idx(i, j);
            let lat = g.geom.lat[j].to_degrees().abs();
            let e = g.elevation.data[idx];
            if e <= 2.0 || e > 600.0 {
                continue;
            }
            let class = g.climate_at(idx);
            let warm = matches!(
                class,
                ClimateClass::HumidSubtropical | ClimateClass::Mediterranean
            );
            let temperate = warm || class == ClimateClass::Oceanic;
            if !random && (!(25.0..=45.0).contains(&lat) || !temperate) {
                continue;
            }
            let coast = g.field_at(&g.coast, idx);
            let river = g.flags[idx] & flags::RIVER != 0;
            let near_water = coast < 0.02 || river;
            let score = if near_water { 2.0 } else { 1.0 } + if warm { 0.5 } else { 0.0 }
                - e / 1000.0
                + if g.province[idx] == province::OROGEN {
                    -1.0
                } else {
                    0.0
                };
            if best.is_none_or(|b| score > b.0) {
                let (x, z) = g.geom.world_xz(i, j);
                best = Some((score, x as i32, z as i32));
            }
            if random && attempt > 50 && best.is_some() {
                break;
            }
        }
        let (x, z) = best.map(|b| (b.1, b.2)).unwrap_or((0, 0));
        self.settle(x, z)
    }

    /// The heart of a biome (tests, screenshots and benchmarks of a kind of land): of the points
    /// on a lattice of the planet whose column is `biome` and passes `also`, the one with the
    /// most of the same biome about it — on rings out to `radius` blocks — and, of those alike,
    /// the first. Deterministic; none if the planet has no such land.
    pub fn find_biome(
        &self,
        biome: biome::Biome,
        radius: f64,
        also: impl Fn(&ColumnSample) -> bool + Sync,
    ) -> Option<(i32, i32)> {
        use rayon::prelude::*;
        let c = self.planet.circumference();
        let step = (c / 128).max(64);
        let zs: Vec<i32> = (-c / 2 + step..c / 2 - step)
            .step_by(step as usize)
            .collect();
        let best = zs
            .par_iter()
            .flat_map_iter(|&z| (0..c).step_by(step as usize).map(move |x| (x, z)))
            .filter_map(|(x, z)| {
                let s = self.survey(x, z);
                if s.biome != biome || !also(&s) {
                    return None;
                }
                // Rings of eight at a quarter, half and the whole of the radius.
                let mut same = 0;
                for k in 1..=3 {
                    let r = radius * [0.25, 0.5, 1.0][k - 1];
                    for a in 0..8 {
                        let t = a as f64 * std::f64::consts::FRAC_PI_4 + k as f64 * 0.3;
                        let (dx, dz) = ((t.cos() * r) as i32, (t.sin() * r) as i32);
                        if self.survey(x + dx, z + dz).biome == biome {
                            same += 1;
                        }
                    }
                }
                Some((same, x, z))
            })
            .max_by(|a, b| a.0.cmp(&b.0).then(b.2.cmp(&a.2)).then(b.1.cmp(&a.1)))?;
        Some(self.settle(best.1, best.2))
    }

    /// A place to start at near (x, z) (v2 §16): there if it is dry, gentle land, else the
    /// nearest such column; from the sea or a lake, the nearest land on the planet (the coast).
    pub fn spawn_near(&self, x: i32, z: i32) -> (i32, i32) {
        if !self.sample(x, z).is_underwater() {
            return self.settle(x, z);
        }
        // The nearest land cell by distance on the sphere.
        let g = &*self.grid;
        let n = g.n();
        let target = self.planet.sphere_point(x as f64, z as f64);
        let mut best: Option<(f64, usize, usize)> = None;
        for j in 0..n {
            let (sl, cl) = (g.geom.sin_lat[j], g.geom.cos_lat[j]);
            for i in 0..n {
                let idx = g.geom.idx(i, j);
                if g.elevation.data[idx] <= 0.0 || g.flags[idx] & flags::LAKE != 0 {
                    continue;
                }
                let p = DVec3::new(cl * g.geom.cos_lon[i], sl, cl * g.geom.sin_lon[i]);
                let near = p.dot(target);
                if best.is_none_or(|b| near > b.0) {
                    best = Some((near, i, j));
                }
            }
        }
        let Some((_, i, j)) = best else {
            return (self.planet.wrap_x(x), z);
        };
        // From the land cell's centre toward the clicked point, the last dry column before
        // the water: the coast itself.
        let (cx, cz) = g.geom.world_xz(i, j);
        let (cx, cz) = (cx as i32, cz as i32);
        let dx = self.planet.delta_block_x(cx, x) as f64;
        let dz = (z - cz) as f64;
        let len = dx.hypot(dz);
        let steps = (len / 8.0).ceil().max(1.0) as i32;
        let mut shore = (cx, cz);
        for k in 1..=steps {
            let t = k as f64 / steps as f64;
            let (px, pz) = (cx + (dx * t) as i32, cz + (dz * t) as i32);
            if self.sample(px, pz).is_underwater() {
                break;
            }
            shore = (px, pz);
        }
        self.settle(shore.0, shore.1)
    }

    /// The nearest dry, gentle column to (x, z), searched on widening rings (up to 256
    /// blocks away; (x, z) itself if none is).
    fn settle(&self, x: i32, z: i32) -> (i32, i32) {
        // On a refined planet the neighbourhood is read once for the whole search.
        let near = self
            .relief
            .is_some()
            .then(|| self.nearby(x - 260, z - 260, x + 260, z + 260));
        let good = |x: i32, z: i32| {
            let s = match &near {
                Some(near) => self.sample_with(x, z, near),
                None => self.sample(x, z),
            };
            !s.is_underwater() && s.height > 1.0 && s.slope < 0.6
        };
        if good(x, z) {
            return (self.planet.wrap_x(x), z);
        }
        for r in (4..=256).step_by(4) {
            // A column every few blocks around the ring.
            let k = (r as f64 * std::f64::consts::TAU / 6.0).ceil() as usize;
            for i in 0..k {
                let a = i as f64 / k as f64 * std::f64::consts::TAU;
                let px = x + (a.cos() * r as f64).round() as i32;
                let pz = z + (a.sin() * r as f64).round() as i32;
                if good(px, pz) {
                    return (self.planet.wrap_x(px), pz);
                }
            }
        }
        (self.planet.wrap_x(x), z)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::settings::WorldGenSettings;
    use hearth_math::PlanetSize;
    use std::sync::OnceLock;

    pub fn test_terrain() -> &'static Terrain {
        static T: OnceLock<Terrain> = OnceLock::new();
        T.get_or_init(|| {
            let s = WorldGenSettings {
                seed: 3,
                planet_size: PlanetSize::Standard,
                grid_resolution: 256,
            };
            Terrain::new(Arc::new(PlanetGrid::build(&s, &|_, _| {})))
        })
    }

    /// An Earth-sized planet on a coarse grid: refinement levels down to tens of metres.
    fn earth_terrain() -> &'static Terrain {
        static T: OnceLock<Terrain> = OnceLock::new();
        T.get_or_init(|| {
            let s = WorldGenSettings {
                seed: 5,
                planet_size: PlanetSize::Earth,
                grid_resolution: 256,
            };
            Terrain::new(Arc::new(PlanetGrid::build(&s.sanitized(), &|_, _| {})))
        })
    }

    /// Land columns of a planet away from the poles, one per `step` grid cells.
    fn land_columns(t: &Terrain, step: usize) -> Vec<(i32, i32)> {
        let g = &*t.grid;
        let n = g.n();
        (n / 4..n * 3 / 4)
            .step_by(step)
            .flat_map(|j| (0..n).step_by(step).map(move |i| (i, j)))
            .filter(|&(i, j)| g.elevation.data[g.geom.idx(i, j)] > 20.0)
            .map(|(i, j)| {
                let (x, z) = g.geom.world_xz(i, j);
                (x as i32, z as i32)
            })
            .collect()
    }

    #[test]
    fn a_refined_planet_reads_the_same_however_it_is_sampled() {
        let t = earth_terrain();
        assert!(t.relief().is_some(), "an Earth-sized planet is refined");
        let c = t.planet().circumference();
        for (x, z) in land_columns(t, 37).into_iter().take(12) {
            let one = t.sample(x, z);
            // In a batch of columns about it, across the planet's seam, and again.
            let near = t.nearby(x - 9, z - 5, x + 6, z + 10);
            let batch = t.sample_with(x, z, &near);
            let round = t.sample(x + c, z);
            for other in [batch, round, t.sample(x, z)] {
                assert_eq!(one.height, other.height, "height at {x},{z}");
                assert_eq!(one.water, other.water, "water at {x},{z}");
                assert_eq!(one.biome, other.biome, "biome at {x},{z}");
            }
        }
    }

    #[test]
    fn the_blocks_rivers_run_down_and_far_tiles_read_coarse_cells() {
        let t = earth_terrain();
        let relief = t.relief().expect("refined");
        let finest = relief.levels().len();
        assert_eq!(relief.level_for(1.0), finest);
        assert_eq!(relief.level_for(8.0), finest);
        assert!(relief.level_for(512.0) < finest);
        assert_eq!(relief.level_for(1.0e6), 0);
        let mut reaches = 0;
        for (x, z) in land_columns(t, 11) {
            let near = t.nearby(x, z, x + 63, z + 63);
            for s in &near.segments {
                assert!(
                    s.level_b <= s.level_a + 1e-3,
                    "a reach by {x},{z} runs up from {} to {}",
                    s.level_a,
                    s.level_b
                );
                assert!(s.width_a > 0.0 && s.depth_a > 0.0);
                reaches += 1;
            }
        }
        assert!(reaches > 50, "rivers about the land: {reaches}");
    }

    #[test]
    fn sampling_is_deterministic_and_wraps() {
        let t = test_terrain();
        let c = t.planet().circumference();
        for (x, z) in [(0, 0), (1234, -5000), (c - 1, 777), (40_000, 12_000)] {
            let a = t.sample(x, z);
            let b = t.sample(x + c, z);
            assert_eq!(a.height, b.height, "seam at {x},{z}");
            assert_eq!(a.biome, b.biome);
            let again = t.sample(x, z);
            assert_eq!(a.height, again.height);
        }
    }

    #[test]
    fn rivers_never_flow_uphill() {
        let t = test_terrain();
        assert!(t.rivers().node_count() > 50, "network exists");
        assert!(t.rivers().max_uphill_step() <= 0.0);
    }

    #[test]
    fn spawning_at_sea_lands_on_the_nearest_coast() {
        let t = test_terrain();
        let planet = *t.planet();
        // A point a few hundred blocks out to sea from the default spawn's coast, and one in
        // mid-ocean.
        let (sx, sz) = t.find_spawn(false);
        let mut at_sea = None;
        'search: for r in (64..4096).step_by(64) {
            for k in 0..16 {
                let a = k as f64 * std::f64::consts::TAU / 16.0;
                let (x, z) = (
                    sx + (a.cos() * r as f64) as i32,
                    sz + (a.sin() * r as f64) as i32,
                );
                if t.sample(x, z).is_underwater() && t.sample(x, z).biome != Biome::River {
                    at_sea = Some((x, z));
                    break 'search;
                }
            }
        }
        let (x, z) = at_sea.expect("water near the spawn");
        let (lx, lz) = t.spawn_near(x, z);
        let s = t.sample(lx, lz);
        assert!(!s.is_underwater() && s.height > 1.0, "{s:?}");
        let d = planet.great_circle_distance((x as f64, z as f64), (lx as f64, lz as f64));
        let to_spawn = planet.great_circle_distance((x as f64, z as f64), (sx as f64, sz as f64));
        assert!(
            d <= to_spawn + 400.0,
            "landed {d:.0} blocks away, land {to_spawn:.0} away"
        );
        // On dry land: there or close by.
        let (px, pz) = t.spawn_near(sx, sz);
        assert!(
            planet.great_circle_distance((sx as f64, sz as f64), (px as f64, pz as f64)) < 800.0
        );
        // Mid-ocean (the deepest cell): still some coast.
        let g = &*t.grid;
        let deepest = (0..g.geom.len())
            .min_by(|&a, &b| g.elevation.data[a].total_cmp(&g.elevation.data[b]))
            .expect("cells");
        let (i, j) = g.geom.ij(deepest);
        let (ox, oz) = g.geom.world_xz(i, j);
        let (lx, lz) = t.spawn_near(ox as i32, oz as i32);
        assert!(!t.sample(lx, lz).is_underwater());
    }

    #[test]
    fn spawn_is_on_warm_temperate_land() {
        let t = test_terrain();
        let (x, z) = t.find_spawn(false);
        let s = t.sample(x, z);
        assert!(!s.is_underwater() && s.height > 0.0, "{s:?}");
        let lat = t.planet().latitude_deg(z as f64).abs();
        assert!((23.0..=47.0).contains(&lat), "spawn latitude {lat}");
        // A spring morning there is no deadly cold for a body in a loincloth.
        assert!(s.temperature > 10.0, "mean temperature {}", s.temperature);
    }
}

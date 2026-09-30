//! Regional sampling: pure, deterministic, thread-safe functions of (seed, x, z) giving the
//! block-resolution surface — height, water, rivers, lakes, biome, surface material, tree
//! density, snow and tree lines. Cube generation and the LOD system both build on these.

pub mod biome;
pub mod rivers;

use std::sync::Arc;

use hearth_math::Planet;
use hearth_math::hash::derive_seed;

use crate::noise::BlockFbm;
use crate::planet::climate::{ClimateClass, LAPSE_RATE};
use crate::planet::{PlanetGrid, flags, province};
use biome::{Biome, BiomeInputs};
use rivers::{RiverHit, RiverNet, Segment};

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
        }
    }
}

/// The world generator's surface model for one world.
#[derive(Debug, Clone)]
pub struct Terrain {
    pub grid: Arc<PlanetGrid>,
    planet: Planet,
    /// Blocks per real metre.
    v: f32,
    rivers: RiverNet,
    noise: DetailNoise,
    seed: u64,
}

/// Latitude where the flat polar ice plateau starts blending in.
const POLAR_BLEND_START: f64 = 80.5;

/// Minimum local water fraction for sea/lake water to fill low ground.
const FLOOD_FRACTION: f32 = 0.08;

impl Terrain {
    pub fn new(grid: Arc<PlanetGrid>) -> Self {
        let planet = *grid.planet();
        let rivers = RiverNet::build(&grid);
        let noise = DetailNoise::new(grid.seed, planet.circumference() as i64);
        Self {
            v: grid.vertical_scale as f32,
            planet,
            rivers,
            noise,
            seed: grid.seed,
            grid,
        }
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

    /// Segments relevant to a rectangle of columns (for batch sampling).
    pub fn river_segments(
        &self,
        x0: i32,
        z0: i32,
        x1: i32,
        z1: i32,
    ) -> smallvec::SmallVec<[Segment; 16]> {
        self.rivers
            .segments_near(x0 as f64, z0 as f64, x1 as f64, z1 as f64, 90.0)
    }

    /// Samples one column (convenience; batch callers should pass precomputed segments).
    pub fn sample(&self, x: i32, z: i32) -> ColumnSample {
        let segs = self.river_segments(x, z, x, z);
        self.sample_with(x, z, &segs)
    }

    /// Height only (fast path for LOD and spawn searches).
    pub fn surface_height(&self, x: i32, z: i32) -> f32 {
        self.sample(x, z).height
    }

    /// Water level at a column (NEG_INFINITY when dry).
    pub fn water_level(&self, x: i32, z: i32) -> f32 {
        self.sample(x, z).water
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

    /// Samples one column with precomputed river segments.
    pub fn sample_with(&self, x: i32, z: i32, segs: &[Segment]) -> ColumnSample {
        let g = &*self.grid;
        let xf = x as f64 + 0.5;
        let zf = z as f64 + 0.5;
        let (gx, gz) = g.geom.grid_coords(self.planet.wrap_xf(xf), zf);
        let lat = self.planet.latitude_deg(zf);
        let polar = ((lat.abs() - POLAR_BLEND_START) / 2.5).clamp(0.0, 1.0) as f32;

        // ------------------------------------------------------------ height
        let (mut h, mtn) = self.base_height(xf, zf, gx, gz);
        // Slope from a small finite difference of the macro surface.
        let step = 4.0;
        let (hx, _) = self.base_height(xf + step, zf, gx + step / g.geom.cell, gz);
        let (hz, _) = self.base_height(xf, zf + step, gx, gz + step / g.geom.cell);
        let slope = (((hx - h) / step as f32).powi(2) + ((hz - h) / step as f32).powi(2)).sqrt();
        h += self.noise.rough.sample2(xf, zf) as f32 * (0.35 + 1.2 * mtn);

        // Volcano craters: a bowl carved into the summit, sometimes holding a crater lake.
        let mut crater_lake = f32::NEG_INFINITY;
        for vol in &g.volcanoes {
            let dx = self.planet.delta_x(vol.x, xf) as f32;
            let dz = (zf - vol.z) as f32;
            let r = vol.crater_radius * 1.4;
            if dx.abs() >= r || dz.abs() >= r {
                continue;
            }
            let d = (dx * dx + dz * dz).sqrt() / vol.crater_radius;
            let rim = vol.summit * self.v;
            if d >= 1.4 || h < rim - vol.crater_depth * 3.0 {
                continue;
            }
            let bowl = rim - vol.crater_depth * (1.0 - d.min(1.0).powi(2));
            let t = smoothstep(1.4, 0.9, d);
            h += (h.min(bowl) - h) * t;
            if vol.crater_lake && d < 0.8 {
                crater_lake = crater_lake.max(rim - vol.crater_depth * 0.45);
            }
        }

        // ------------------------------------------------------------ water
        let idx = g.cell_at(self.planet.wrap_xf(xf), zf);
        let cell_flags = g.flags[idx];
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
        // Rivers: carve the channel and shape a floodplain.
        let mut river_hit = None;
        if !segs.is_empty() {
            let wx = xf + self.noise.warp_x.sample2(xf, zf) * g.geom.cell * 0.35;
            let wz = zf + self.noise.warp_z.sample2(xf, zf) * g.geom.cell * 0.35;
            let seg_cx = (segs[0].ax + segs[0].bx) * 0.5;
            let wx = seg_cx + self.planet.delta_x(seg_cx, wx);
            if let Some(hit) = RiverNet::closest(wx, wz, segs) {
                let half = hit.width * 0.5;
                let bank_w = 3.0 + hit.width * 0.9;
                if hit.distance < half + bank_w {
                    let bank = hit.level + 1.2;
                    if hit.distance < half {
                        let t = hit.distance / half;
                        let bed = hit.level - hit.depth * (1.0 - t * t) - 0.3;
                        h = h.min(bed);
                        water = water.max(hit.level);
                    } else {
                        let t = (hit.distance - half) / bank_w;
                        let w = 1.0 - smoothstep(0.0, 1.0, t);
                        h += (bank - h) * w;
                    }
                    river_hit = Some(hit);
                }
            }
        }

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

        // ------------------------------------------------------------ materials
        let (surface, filler, soil_depth) =
            self.materials(biome, h, water, slope, xf, zf, temperature, precip);
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
            let s = match biome {
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
            Biome::Wetland => (if patch > 0.0 { Mud } else { Grass }, Mud),
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

    /// Finds a spawn point: temperate mid-latitude lowland near a coast or river (or anywhere on
    /// land for `random`). Deterministic.
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
            let temperate = matches!(
                class,
                ClimateClass::Oceanic
                    | ClimateClass::HumidContinental
                    | ClimateClass::HumidSubtropical
                    | ClimateClass::Mediterranean
            );
            if !random && (!(35.0..=55.0).contains(&lat) || !temperate) {
                continue;
            }
            let coast = g.field_at(&g.coast, idx);
            let river = g.flags[idx] & flags::RIVER != 0;
            let near_water = coast < 0.02 || river;
            let score = if near_water { 2.0 } else { 1.0 } - e / 1000.0
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
        let (mut x, mut z) = best.map(|b| (b.1, b.2)).unwrap_or((0, 0));
        // Refine at block level: nearest dry, gentle column.
        for r in 0..64 {
            let s = self.sample(x, z);
            if !s.is_underwater() && s.height > 1.0 && s.slope < 0.6 {
                break;
            }
            let a = r as f64 * 2.4;
            x += (a.cos() * 12.0) as i32;
            z += (a.sin() * 12.0) as i32;
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
                ..WorldGenSettings::default()
            };
            Terrain::new(Arc::new(PlanetGrid::build(&s, &|_, _| {})))
        })
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
    fn spawn_is_on_temperate_land() {
        let t = test_terrain();
        let (x, z) = t.find_spawn(false);
        let s = t.sample(x, z);
        assert!(!s.is_underwater() && s.height > 0.0, "{s:?}");
        let lat = t.planet().latitude_deg(z as f64).abs();
        assert!((33.0..=57.0).contains(&lat), "spawn latitude {lat}");
    }
}

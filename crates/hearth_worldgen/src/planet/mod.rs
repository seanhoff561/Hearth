//! The planet model: a deterministic simulation of tectonics → elevation → erosion & drainage
//! → climate, computed once per world on the planet grid and saved with the world.

pub mod climate;
pub mod elevation;
pub mod fields;
pub mod grid;
pub mod hydrology;
pub mod io;
#[cfg(test)]
mod stats;
pub mod tectonics;

use glam::DVec3;
use hearth_math::Planet;
use hearth_math::hash::{Rng, derive_seed};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::noise::SphereFbm;
use crate::settings::{LAND_FRACTION, WorldGenSettings};
use climate::ClimateClass;
use elevation::{BoundaryKind, CellInput, ElevationNoise, ElevationParams, Scale};
use fields::{NONE, class_edges, distance_to, nearest_seed, weighted_quantile};
use grid::{Field, GridGeom};
use hydrology::ErosionParams;
use tectonics::TectonicLayout;

/// Per-cell flags.
pub mod flags {
    /// Water connected to the world ocean.
    pub const OCEAN: u8 = 1;
    /// Standing fresh water (lake surface above the ground).
    pub const LAKE: u8 = 2;
    /// A river flows through this cell.
    pub const RIVER: u8 = 4;
    /// Closed basin without outlet (salt flat or saline lake).
    pub const ENDORHEIC: u8 = 8;
    /// Dry salt flat (endorheic basin that holds no permanent water).
    pub const SALT_FLAT: u8 = 16;
    /// Volcanic terrain (arcs, hotspots).
    pub const VOLCANIC: u8 = 32;
    /// River mouth delta.
    pub const DELTA: u8 = 64;
}

/// Rock provinces used by underground generation (stone variety, ore bias).
pub mod province {
    pub const OCEANIC: u8 = 0;
    pub const SHIELD: u8 = 1;
    pub const OROGEN: u8 = 2;
    pub const ARC: u8 = 3;
    pub const BASIN: u8 = 4;
    pub const RIFT: u8 = 5;
    pub const OLD_OROGEN: u8 = 6;
}

/// A volcano: cone with a summit crater, applied at block level.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Volcano {
    pub x: f64,
    pub z: f64,
    /// Summit elevation (metres).
    pub summit: f32,
    /// Crater radius in blocks.
    pub crater_radius: f32,
    /// Crater depth in blocks.
    pub crater_depth: f32,
    pub crater_lake: bool,
}

/// The eight neighbours a cell may drain to, as (Δi, Δj); `flow` codes index it.
pub const FLOW_D8: [(i64, i64); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];
/// The `flow` code of a cell that drains to no neighbour.
pub const NO_FLOW: u8 = 8;

/// The flow code from a cell to its receiver (wrapping in i).
fn flow_code(geom: &GridGeom, idx: usize, receiver: usize) -> u8 {
    if receiver == idx {
        return NO_FLOW;
    }
    let n = geom.n as i64;
    let (i, j) = ((idx % geom.n) as i64, (idx / geom.n) as i64);
    let (ri, rj) = ((receiver % geom.n) as i64, (receiver / geom.n) as i64);
    let mut di = ri - i;
    if di > 1 {
        di -= n;
    } else if di < -1 {
        di += n;
    }
    FLOW_D8
        .iter()
        .position(|&o| o == (di, rj - j))
        .map_or(NO_FLOW, |c| c as u8)
}

/// A river cell of the drainage network.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RiverCell {
    /// Downstream cell index.
    pub receiver: u32,
    /// Discharge (square degrees of catchment × metres of rain per year).
    pub discharge: f32,
}

/// The finished planet model.
///
/// Elevation and water are stored at full resolution; smooth fields (climate, uplift, coast
/// distance) at half resolution; the river network sparsely.
#[derive(Debug, Clone)]
pub struct PlanetGrid {
    pub geom: GridGeom,
    pub seed: u64,
    pub vertical_scale: f64,
    /// Settings the planet was built with.
    pub settings: WorldGenSettings,
    /// Terrain elevation in real metres (sea level 0).
    pub elevation: Field<f32>,
    /// Water surface in metres where there is water (ocean 0, lakes, rivers); NaN when dry.
    pub water: Field<f32>,
    /// River cells (cell index → receiver and discharge).
    pub rivers: rustc_hash::FxHashMap<u32, RiverCell>,
    /// Every cell's drainage: the neighbour it flows to (a code into [`FLOW_D8`]; [`NO_FLOW`]
    /// for the sea, a basin's floor or a lake that does not spill to a neighbour) and its
    /// discharge (square degrees of catchment × metres of rain a year, as the rivers').
    pub flow: Vec<u8>,
    pub discharge: Vec<f32>,
    pub flags: Vec<u8>,
    pub plate: Vec<u8>,
    pub province: Vec<u8>,
    /// Mountain uplift (metres) — local relief driver.
    pub uplift: Field<f32>,
    /// Signed distance to the coast in radians (positive on land).
    pub coast: Field<f32>,
    pub temperature: Field<f32>,
    pub sea_level_temperature: Field<f32>,
    pub temp_range: Field<f32>,
    pub precipitation: Field<f32>,
    pub dry_season: Vec<u8>,
    pub winter_dry: Field<f32>,
    pub summer_dry: Field<f32>,
    pub climate: Vec<u8>,
    pub current: Field<f32>,
    pub volcanoes: Vec<Volcano>,
    pub layout: TectonicLayout,
}

/// Progress callback: (fraction 0..1, stage name).
pub type Progress<'a> = &'a (dyn Fn(f32, &str) + Sync);

/// The planet generator's version, part of the name a planet is cached under: bumped whenever
/// the planet made for the same settings changes, so that no machine goes on using a planet an
/// older build made (`tests/planet_pinned.rs` fails until it is bumped and the new planets are
/// pinned). A world keeps the planet it was made on in its own folder, whatever the version.
pub const GENERATOR: u32 = 1;

/// Discharge below this is not a river. Discharge is measured in square degrees of catchment ×
/// metres of rain per year, so it is independent of grid resolution and planet size.
pub const RIVER_MIN_DISCHARGE: f32 = 2.0;

impl PlanetGrid {
    /// A fingerprint of what the refinement levels are made from (the surface, its waters and
    /// their drainage): tells this planet's tiles kept on disk from another's, or from an older
    /// build's of the same seed.
    pub fn fingerprint(&self) -> u64 {
        use std::hash::Hasher;
        let mut h = rustc_hash::FxHasher::default();
        h.write_u64(self.seed);
        h.write_usize(self.n());
        h.write_u64(self.vertical_scale.to_bits());
        h.write_u64(self.geom.c.to_bits());
        // Every seventh cell: a change to how a planet is made changes nearly all of them.
        for i in (0..self.flow.len()).step_by(7) {
            h.write_u32(self.elevation.data[i].to_bits());
            h.write_u32(self.water.data[i].to_bits());
            h.write_u32(self.discharge[i].to_bits());
            h.write_u8(self.flow[i]);
            h.write_u8(self.flags[i]);
        }
        h.finish()
    }

    /// The memory its fields take (bytes).
    pub fn bytes(&self) -> u64 {
        fn field<T>(f: &Field<T>) -> u64 {
            (f.data.len() * std::mem::size_of::<T>()) as u64
        }
        let f32s = [
            &self.elevation,
            &self.water,
            &self.uplift,
            &self.coast,
            &self.temperature,
            &self.sea_level_temperature,
            &self.temp_range,
            &self.precipitation,
            &self.winter_dry,
            &self.summer_dry,
            &self.current,
        ];
        let bytes: [&Vec<u8>; 6] = [
            &self.flow,
            &self.flags,
            &self.plate,
            &self.province,
            &self.dry_season,
            &self.climate,
        ];
        f32s.iter().map(|f| field(f)).sum::<u64>()
            + bytes.iter().map(|v| v.len() as u64).sum::<u64>()
            + (self.discharge.len() * 4) as u64
            + (self.rivers.len() * std::mem::size_of::<(u32, RiverCell)>()) as u64
    }
}

impl PlanetGrid {
    /// Builds the planet for `settings`. Deterministic for a given seed and settings.
    pub fn build(settings: &WorldGenSettings, progress: Progress<'_>) -> Self {
        let settings = settings.clone().sanitized();
        let planet: Planet = settings.planet().expect("valid planet size");
        let seed = settings.seed;
        let n = settings.grid_resolution;
        let geom = GridGeom::new(planet, n);
        let v = settings.vertical_scale();
        let scale = Scale {
            radius_blocks: planet.radius(),
            vertical: v,
            horizontal: settings.planet_size.horizontal_scale(),
        };
        progress(0.0, "Arranging tectonic plates");
        // Continental plates lose their margins to the sea, so they must cover more than the
        // target land fraction; the threshold below trims them to the exact target.
        let layout = TectonicLayout::generate(seed, (LAND_FRACTION * 1.6).min(0.8));
        let len = geom.len();

        // ------------------------------------------------------------ plates
        let hits: Vec<tectonics::PlateHit> = (0..len)
            .into_par_iter()
            .map(|idx| {
                let (i, j) = geom.ij(idx);
                layout.locate(geom.sphere(i, j))
            })
            .collect();
        let plate: Vec<u8> = hits.iter().map(|h| h.plate).collect();
        progress(0.08, "Finding plate boundaries");
        let edges = class_edges(&geom, &plate);
        // Boundary info for edge cells: (other plate, convergence, kind).
        let binfo: Vec<(u8, f32, BoundaryKind)> = (0..len)
            .into_par_iter()
            .map(|idx| {
                if !edges[idx] {
                    return (plate[idx], 0.0, BoundaryKind::Inactive);
                }
                let (i, j) = geom.ij(idx);
                let own = plate[idx];
                let other = [(-1isize, 0isize), (1, 0), (0, -1), (0, 1)]
                    .iter()
                    .filter_map(|&(di, dj)| geom.neighbor(i, j, di, dj))
                    .map(|nb| plate[nb])
                    .find(|p| *p != own)
                    .unwrap_or(own);
                let p = geom.sphere(i, j);
                let (conv, shear) = layout.boundary_motion(own, other, p);
                let kind = if conv > 0.12 {
                    BoundaryKind::Convergent
                } else if conv < -0.12 {
                    BoundaryKind::Divergent
                } else if shear > 0.2 {
                    BoundaryKind::Transform
                } else {
                    BoundaryKind::Inactive
                };
                (other, conv as f32, kind)
            })
            .collect();
        let near_b = nearest_seed(&geom, &edges);
        let dist_b = distance_to(&geom, &near_b);
        // Boundaries between a continental and an oceanic plate define coastlines.
        let is_cont = |p: u8| layout.plates[p as usize].continental;
        let co_edges: Vec<bool> = (0..len)
            .map(|idx| edges[idx] && is_cont(plate[idx]) != is_cont(binfo[idx].0))
            .collect();
        let near_co = nearest_seed(&geom, &co_edges);
        let dist_co = distance_to(&geom, &near_co);

        // ------------------------------------------------------------ continents
        progress(0.18, "Growing continents");
        let coast_big = SphereFbm::new(derive_seed(seed, "coast-big"), 6.0, 3, 0.5, 2.0);
        let coast_mid = SphereFbm::new(derive_seed(seed, "coast-mid"), 16.0, 3, 0.5, 2.0);
        let coast_fine = SphereFbm::new(derive_seed(seed, "coast-fine"), 40.0, 4, 0.55, 2.1);
        let micro = SphereFbm::new(derive_seed(seed, "micro"), 4.0, 3, 0.5, 2.0);
        let margin_noise = SphereFbm::new(derive_seed(seed, "margin"), 5.0, 2, 0.5, 2.0);
        let mut inland: Vec<f32> = (0..len)
            .into_par_iter()
            .map(|idx| {
                let (i, j) = geom.ij(idx);
                let p = geom.sphere(i, j);
                let own = plate[idx];
                let shape = coast_big.sample(p) * 0.09
                    + coast_mid.sample(p) * 0.035
                    + coast_fine.sample(p) * 0.016;
                if is_cont(own) {
                    let b = near_co[idx];
                    let (margin, d) = if b == NONE {
                        (0.0, 1.0)
                    } else {
                        let (_, conv, kind) = binfo[b as usize];
                        let m01 = 0.5 + 0.5 * margin_noise.sample(p);
                        let m = match kind {
                            BoundaryKind::Convergent if conv > 0.0 => 0.004 + 0.012 * m01,
                            BoundaryKind::Divergent => 0.10 + 0.16 * m01,
                            _ => 0.04 + 0.07 * m01,
                        };
                        (m, dist_co[idx] as f64)
                    };
                    (d - margin + shape) as f32
                } else {
                    let d = if near_co[idx] == NONE {
                        1.0
                    } else {
                        dist_co[idx] as f64
                    };
                    // Oceanic plates: ocean, except rare microcontinents (independent of the plate
                    // boundaries so they never form strips along them).
                    let _ = d;
                    (-0.45 + (micro.sample(p) - 0.72).max(0.0) * 3.0 + shape * 0.3) as f32
                }
            })
            .collect();
        fields::blur(&geom, &mut inland, 1, 1);
        // Ancient sutures inside continents (old eroded ranges).
        let sutures = TectonicLayout::generate(derive_seed(seed, "ancient"), 0.0);
        let old_plate: Vec<u8> = (0..len)
            .into_par_iter()
            .map(|idx| {
                let (i, j) = geom.ij(idx);
                sutures.locate(geom.sphere(i, j) * 1.0).plate
            })
            .collect();
        let old_edges = class_edges(&geom, &old_plate);
        let near_old = nearest_seed(&geom, &old_edges);
        let dist_old = distance_to(&geom, &near_old);

        // ------------------------------------------------------------ hotspots
        progress(0.34, "Tracing hotspot chains");
        let chain = hotspot_chain(&layout, &scale, seed);

        let params = ElevationParams { scale };
        let noise = ElevationNoise::new(seed, &scale);
        // Everything from the land threshold to the composed elevation, as a function of the
        // target land fraction. Coastal ranges and island arcs add some land beyond the
        // threshold, so the target is corrected once from the measured result.
        let stage = |target: f64| -> ContinentStage {
            let threshold = weighted_quantile(&geom, &inland, 1.0 - target);
            let land: Vec<bool> = inland.iter().map(|v| *v > threshold).collect();

            // ------------------------------------------------------------ coasts, margins, age
            progress(0.26, "Measuring coastlines");
            let coast_edges = class_edges(&geom, &land);
            let near_coast = nearest_seed(&geom, &coast_edges);
            let dist_coast = distance_to(&geom, &near_coast);
            let coast: Vec<f32> = (0..len)
                .map(|idx| {
                    let d = dist_coast[idx].min(3.0);
                    if land[idx] { d } else { -d }
                })
                .collect();
            // Margin activity of coast cells: subduction margins are narrow and steep.
            let activity_at: Vec<f32> = (0..len)
                .into_par_iter()
                .map(|idx| {
                    let b = near_co[idx];
                    if b == NONE {
                        return 0.0;
                    }
                    let (_, conv, kind) = binfo[b as usize];
                    if kind == BoundaryKind::Convergent && conv > 0.0 {
                        (1.0 - elevation::smoothstep(0.02, 0.09, dist_co[idx] as f64)) as f32
                    } else {
                        0.0
                    }
                })
                .collect();
            // Per-cell margin activity (ocean cells inherit their nearest coast's), smoothed so
            // shelf widths change gradually along the coast.
            let mut margin_activity: Vec<f32> = (0..len)
                .map(|idx| {
                    if land[idx] {
                        activity_at[idx]
                    } else {
                        let c = near_coast[idx];
                        if c == NONE {
                            0.0
                        } else {
                            activity_at[c as usize]
                        }
                    }
                })
                .collect();
            fields::blur(&geom, &mut margin_activity, (n / 90).max(1), 2);
            let ridge_seeds: Vec<bool> = (0..len)
                .map(|idx| edges[idx] && binfo[idx].2 == BoundaryKind::Divergent && !land[idx])
                .collect();
            let near_ridge = nearest_seed(&geom, &ridge_seeds);
            let dist_ridge = distance_to(&geom, &near_ridge);

            // ------------------------------------------------------------ elevation
            let composed: Vec<elevation::CellElevation> = (0..len)
                .into_par_iter()
                .map(|idx| {
                    let (i, j) = geom.ij(idx);
                    let p = geom.sphere(i, j);
                    let own = plate[idx];
                    let b = near_b[idx];
                    let (other, conv, kind) = if b == NONE {
                        (own, 0.0, BoundaryKind::Inactive)
                    } else {
                        let (bo, conv, kind) = binfo[b as usize];
                        let bp = plate[b as usize];
                        (if bp == own { bo } else { bp }, conv, kind)
                    };
                    let own_c = is_cont(own);
                    let other_c = is_cont(other);
                    let overriding = match (own_c, other_c) {
                        (true, false) => true,
                        (false, true) => false,
                        _ => {
                            layout.plates[own as usize].density
                                < layout.plates[other as usize].density
                        }
                    };
                    let mut hot = 0.0f64;
                    for v in &chain {
                        let d = p.dot(v.0).clamp(-1.0, 1.0).acos();
                        if d < v.2 {
                            let t = 1.0 - d / v.2;
                            hot = hot.max(v.1 * t.powf(1.6));
                        }
                    }
                    let crust_age = if near_ridge[idx] == NONE {
                        1.0
                    } else {
                        elevation::smoothstep(0.0, 0.55, dist_ridge[idx] as f64)
                    };
                    let margin_activity = margin_activity[idx] as f64;
                    let input = CellInput {
                        p,
                        coast: coast[idx] as f64,
                        boundary_dist: dist_b[idx] as f64,
                        boundary: kind,
                        convergence: (conv as f64).max(0.0),
                        own_continental: own_c,
                        other_continental: other_c,
                        overriding,
                        crust_age,
                        margin_activity,
                        suture_dist: if near_old[idx] == NONE {
                            1.0
                        } else {
                            dist_old[idx] as f64
                        },
                        hotspot: hot,
                    };
                    elevation::compose(&input, &params, &noise)
                })
                .collect();
            // Smooth the boundary-driven components so no seams remain where the nearest plate
            // boundary switches segment or type.
            let smooth_r = (n / 340).max(1);
            let mut tectonic: Vec<f32> = composed.iter().map(|c| c.tectonic as f32).collect();
            let mut uplift: Vec<f32> = composed.iter().map(|c| c.uplift as f32).collect();
            let mut plateau: Vec<f32> = composed.iter().map(|c| c.plateau as f32).collect();
            fields::blur(&geom, &mut tectonic, smooth_r, 2);
            fields::blur(&geom, &mut uplift, smooth_r, 2);
            fields::blur(&geom, &mut plateau, smooth_r, 2);
            let composed: Vec<elevation::CellElevation> = composed
                .iter()
                .enumerate()
                .map(|(idx, c)| elevation::CellElevation {
                    tectonic: tectonic[idx] as f64,
                    uplift: uplift[idx] as f64,
                    plateau: plateau[idx] as f64,
                    ..*c
                })
                .collect();
            let elev: Vec<f32> = (0..len)
                .into_par_iter()
                .map(|idx| {
                    let (i, j) = geom.ij(idx);
                    elevation::finish(&composed[idx], geom.sphere(i, j), &noise) as f32
                })
                .collect();

            ContinentStage {
                land,
                coast,
                composed,
                elev,
            }
        };
        progress(0.40, "Raising mountains");
        let first = stage(LAND_FRACTION);
        let measured = area_fraction(&geom, &first.elev, |e| e > 0.0);
        let corrected = (2.0 * LAND_FRACTION - measured).clamp(0.05, 0.9);
        let ContinentStage {
            land,
            coast,
            composed,
            mut elev,
        } = if (measured - LAND_FRACTION).abs() > 0.01 {
            stage(corrected)
        } else {
            first
        };

        let uplift: Vec<f32> = composed.iter().map(|c| c.uplift as f32).collect();

        // Provinces for underground generation.
        let province: Vec<u8> = (0..len)
            .map(|idx| {
                let c = &composed[idx];
                if !land[idx] && elev[idx] < -1000.0 {
                    province::OCEANIC
                } else if c.rift > 0.2 {
                    province::RIFT
                } else if c.uplift > 1500.0 {
                    let b = near_b[idx];
                    if b != NONE && !is_cont(binfo[b as usize].0) {
                        province::ARC
                    } else {
                        province::OROGEN
                    }
                } else if c.uplift > 300.0 {
                    province::OLD_OROGEN
                } else if coast[idx] > 0.12 {
                    province::SHIELD
                } else {
                    province::BASIN
                }
            })
            .collect();

        // ------------------------------------------------------------ erosion
        progress(0.52, "Carving valleys");
        polar_caps(&geom, &mut elev);
        let ocean0 = ocean_mask(&geom, &elev);
        let h_orig: Vec<f32> = elev.iter().map(|e| e * v as f32).collect();
        // The polar ice caps are drainage base levels like the sea (meltwater leaves along the
        // ice margin), so they never dam up basins.
        let polar = polar_mask(&geom);
        let base0: Vec<bool> = (0..len).map(|i| ocean0[i] || polar[i]).collect();
        let filled = hydrology::fill_depressions(&geom, &h_orig, &base0, 0.002);
        let pre_climate = climate::compute(&geom, &elev, &ocean0);
        // Tectonic and glacial basins (rifts, scoured shields) and deep arid basins are real
        // closed basins: they stay fixed base levels during erosion. Other depressions fill
        // with sediment and drain (their floors become plains).
        let lake_basin = protected_basins(
            &geom,
            &h_orig,
            &filled,
            &base0,
            &composed,
            &pre_climate.precipitation,
            &pre_climate.temperature,
            v,
        );
        let erosion_base: Vec<bool> = (0..len).map(|i| base0[i] || lake_basin[i]).collect();
        // Sediment plains are not perfectly flat: a gentle undulation lets rivers meander
        // across them instead of running along the fill order in straight lines.
        let undulate = SphereFbm::new(
            derive_seed(seed, "plains"),
            geom.radius() / 90.0,
            3,
            0.5,
            2.0,
        );
        let h_start: Vec<f32> = (0..len)
            .into_par_iter()
            .map(|i| {
                if lake_basin[i] {
                    h_orig[i]
                } else if filled[i] > h_orig[i] + 0.01 {
                    let (ci, cj) = geom.ij(i);
                    filled[i] + (undulate.sample(geom.sphere(ci, cj)) as f32 + 1.0) * 1.5
                } else {
                    filled[i]
                }
            })
            .collect();
        let mut h_blocks = hydrology::fill_depressions(&geom, &h_start, &erosion_base, 0.002);
        let rain: Vec<f32> = pre_climate
            .precipitation
            .iter()
            .map(|p| (p / 1000.0).clamp(0.05, 3.0))
            .collect();
        let debug = std::env::var_os("HEARTH_DEBUG_PLANET").is_some();
        let count_pits = |h: &[f32], base: &[bool]| -> usize {
            let f = hydrology::fill_depressions(&geom, h, base, 0.0);
            (0..len).filter(|&i| f[i] - h[i] > 0.5 * v as f32).count()
        };
        if debug {
            eprintln!(
                "protected cells {}, pits before erosion {}",
                lake_basin.iter().filter(|b| **b).count(),
                count_pits(&h_blocks, &base0)
            );
        }
        hydrology::stream_power(
            &geom,
            &mut h_blocks,
            &erosion_base,
            &rain,
            &ErosionParams::default(),
        );
        if debug {
            eprintln!("pits after erosion {}", count_pits(&h_blocks, &base0));
        }
        // Write back every non-ocean cell (including inland basins below sea level, which
        // either stay closed — protected — or were filled with sediment).
        for (idx, (e, hb)) in elev.iter_mut().zip(&h_blocks).enumerate() {
            if ocean0[idx] || polar[idx] {
                continue;
            }
            let eroded = hb / v as f32;
            *e = if *e > 0.0 || !lake_basin[idx] {
                eroded.max(2.0)
            } else {
                eroded
            };
        }

        // ------------------------------------------------------------ climate
        progress(0.68, "Simulating climate");
        let ocean = ocean_mask(&geom, &elev);
        let cl = climate::compute(&geom, &elev, &ocean);

        // ------------------------------------------------------------ rivers and lakes
        progress(0.80, "Filling lakes and rivers");
        let hydro = finish_hydrology(&geom, &elev, &ocean, &polar, &cl, v);

        progress(0.94, "Placing volcanoes");
        let mut flags = hydro.flags;
        let volcanoes = find_volcanoes(
            &geom, &elev, &uplift, &chain, &mut flags, &province, seed, v,
        );

        progress(1.0, "Done");
        let rivers = (0..len)
            .filter(|&idx| flags[idx] & flags::RIVER != 0)
            .map(|idx| {
                (
                    idx as u32,
                    RiverCell {
                        receiver: hydro.receiver[idx],
                        discharge: hydro.discharge[idx],
                    },
                )
            })
            .collect();
        let flow: Vec<u8> = (0..len)
            .map(|idx| flow_code(&geom, idx, hydro.receiver[idx] as usize))
            .collect();
        let half = |data: Vec<f32>| Field::from_vec(n, data).downsample2();
        PlanetGrid {
            geom,
            seed,
            vertical_scale: v,
            settings,
            elevation: Field::from_vec(n, elev),
            water: Field::from_vec(n, hydro.water),
            rivers,
            flow,
            discharge: hydro.discharge,
            flags,
            plate,
            province,
            uplift: half(uplift),
            coast: half(coast),
            temperature: half(cl.temperature),
            sea_level_temperature: half(cl.sea_level_temperature),
            temp_range: half(cl.temp_range),
            precipitation: half(cl.precipitation),
            dry_season: cl.dry_season,
            winter_dry: half(cl.winter_dry),
            summer_dry: half(cl.summer_dry),
            climate: cl.class,
            current: half(cl.current),
            volcanoes,
            layout,
        }
    }

    pub fn n(&self) -> usize {
        self.geom.n
    }

    /// Value of a (possibly half-resolution) field at a full-grid cell index.
    #[inline]
    pub fn field_at(&self, f: &Field<f32>, idx: usize) -> f32 {
        f.at_index(idx, self.geom.n)
    }

    /// Discharge of a river cell (0 elsewhere).
    pub fn discharge_at(&self, idx: usize) -> f32 {
        self.rivers.get(&(idx as u32)).map_or(0.0, |r| r.discharge)
    }

    pub fn planet(&self) -> &Planet {
        self.geom.planet()
    }

    /// Climate class of a cell.
    pub fn climate_at(&self, idx: usize) -> ClimateClass {
        ClimateClass::from_u8(self.climate[idx])
    }

    /// Cell index containing a world position.
    pub fn cell_at(&self, x: f64, z: f64) -> usize {
        let (gx, gz) = self.geom.grid_coords(x, z);
        let n = self.geom.n as isize;
        let i = (gx.round() as isize).rem_euclid(n) as usize;
        let j = (gz.round() as isize).clamp(0, n - 1) as usize;
        self.geom.idx(i, j)
    }

    /// Area-weighted fraction of the planet that is land (elevation > 0).
    pub fn land_fraction(&self) -> f64 {
        area_fraction(&self.geom, &self.elevation.data, |e| e > 0.0)
    }
}

/// Depressions that are real lake basins rather than noise pits: rift grabens, glacially
/// scoured shields, and any basin that is both deep and large (intermontane basins). They stay
/// fixed during erosion; the final hydrology turns them into lakes or, when arid, into closed
/// basins.
fn protected_basins(
    geom: &GridGeom,
    h: &[f32],
    filled: &[f32],
    ocean: &[bool],
    composed: &[elevation::CellElevation],
    precipitation: &[f32],
    temperature: &[f32],
    v: f64,
) -> Vec<bool> {
    let len = geom.len();
    let depression: Vec<bool> = (0..len)
        .map(|i| !ocean[i] && filled[i] - h[i] > 0.5)
        .collect();
    let (label, count) = hydrology::label_components(geom, &depression);
    let mut depth = vec![0f32; count as usize];
    let mut cells = vec![0u32; count as usize];
    let mut rift = vec![false; count as usize];
    let mut scour = vec![false; count as usize];
    let mut rain = vec![0f32; count as usize];
    let mut pet = vec![0f32; count as usize];
    for idx in 0..len {
        let l = label[idx];
        if l == u32::MAX {
            continue;
        }
        let l = l as usize;
        depth[l] = depth[l].max(filled[idx] - h[idx]);
        cells[l] += 1;
        rain[l] += precipitation[idx];
        pet[l] += potential_evaporation_mm(temperature[idx]);
        if composed[idx].rift > 0.02 {
            rift[l] = true;
        }
        if scoured(geom, idx, &composed[idx]) {
            scour[l] = true;
        }
    }
    // Thresholds in blocks: ~60 m deep and a few cells wide.
    let deep = (60.0 * v) as f32;
    (0..len)
        .map(|idx| {
            let l = label[idx];
            if l == u32::MAX {
                return false;
            }
            let l = l as usize;
            // Water balance, not raw rainfall: cold tundra is wet, hot steppe is dry.
            let arid = rain[l] < pet[l] * 0.5;
            // Scour lakes are small and shallow; rift lakes long and deep.
            let scour_lake = scour[l] && depth[l] < (70.0 * v) as f32 && cells[l] <= 40;
            (rift[l] && depth[l] > (8.0 * v) as f32)
                || scour_lake
                || (arid && depth[l] > deep && cells[l] >= 4)
        })
        .collect()
}

/// Potential evaporation in mm/yr from mean annual temperature.
pub fn potential_evaporation_mm(t: f32) -> f32 {
    (250.0 + 75.0 * t).clamp(100.0, 2400.0)
}

/// Cells of the polar ice caps (beyond 80.5 degrees).
fn polar_mask(geom: &GridGeom) -> Vec<bool> {
    let n = geom.n;
    (0..geom.len())
        .map(|idx| geom.lat[idx / n].to_degrees().abs() > 80.5)
        .collect()
}

/// True where glacial scouring created the depression (cold shields).
fn scoured(geom: &GridGeom, idx: usize, c: &elevation::CellElevation) -> bool {
    let j = idx / geom.n;
    geom.lat[j].to_degrees().abs() > 45.0 && c.uplift < 300.0 && c.local < -3.0
}

/// Flattens the last degrees before each pole edge into a featureless ice plateau: a high ice
/// sheet if the polar region is mostly land, sea ice (sea level) otherwise.
fn polar_caps(geom: &GridGeom, elev: &mut [f32]) {
    let n = geom.n;
    for north in [true, false] {
        // Land share of the 74-81 degree band decides the cap type.
        let (mut land, mut total) = (0usize, 0usize);
        for j in 0..n {
            let lat = geom.lat[j].to_degrees();
            let in_band = if north {
                (74.0..81.0).contains(&lat)
            } else {
                (-81.0..-74.0).contains(&lat)
            };
            if in_band {
                for i in 0..n {
                    total += 1;
                    if elev[j * n + i] > 0.0 {
                        land += 1;
                    }
                }
            }
        }
        let sheet = total > 0 && land * 10 > total * 7;
        let cap = if sheet { 2400.0f32 } else { -30.0 };
        for j in 0..n {
            let lat = geom.lat[j].to_degrees();
            let a = if north { lat } else { -lat };
            if a <= 80.0 {
                continue;
            }
            let t = elevation::smoothstep(80.0, 83.5, a) as f32;
            for i in 0..n {
                let e = &mut elev[j * n + i];
                *e += (cap - *e) * t;
            }
        }
    }
}

/// Intermediate result of the continent/elevation stage.
struct ContinentStage {
    land: Vec<bool>,
    coast: Vec<f32>,
    composed: Vec<elevation::CellElevation>,
    elev: Vec<f32>,
}

/// Area-weighted fraction of cells whose value satisfies `pred`.
fn area_fraction(geom: &GridGeom, values: &[f32], pred: impl Fn(f32) -> bool) -> f64 {
    let w = fields::row_area_weights(geom);
    let n = geom.n;
    let (mut hit, mut total) = (0.0, 0.0);
    for (idx, v) in values.iter().enumerate() {
        let wt = w[idx / n];
        total += wt;
        if pred(*v) {
            hit += wt;
        }
    }
    hit / total
}

/// Cells at or below sea level that are connected to the largest body of such water.
pub fn ocean_mask(geom: &GridGeom, elev: &[f32]) -> Vec<bool> {
    let below: Vec<bool> = elev.iter().map(|e| *e <= 0.0).collect();
    let (label, count) = hydrology::label_components(geom, &below);
    if count == 0 {
        return below;
    }
    // The world ocean is the component with the largest area.
    let w = fields::row_area_weights(geom);
    let mut area = vec![0.0f64; count as usize];
    for (idx, l) in label.iter().enumerate() {
        if *l != u32::MAX {
            area[*l as usize] += w[idx / geom.n];
        }
    }
    let biggest = area
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(i, _)| i as u32)
        .unwrap_or(0);
    // Other large below-sea-level bodies connected to the sea by narrow straits also count as
    // ocean if they're big (inland seas).
    let total: f64 = (0..geom.len()).map(|idx| w[idx / geom.n]).sum();
    // Large seas (≥ 0.05 % of the sphere) count as ocean even if cut off at this resolution.
    let big_enough = (area[biggest as usize] * 0.02).min(total * 0.0005);
    // Polar seas (bodies touching the pole-edge rows) are open ocean too.
    let n = geom.n;
    let mut polar = vec![false; count as usize];
    for i in 0..n {
        for j in [0, n - 1] {
            let l = label[j * n + i];
            if l != u32::MAX {
                polar[l as usize] = true;
            }
        }
    }
    label
        .iter()
        .map(|l| {
            *l != u32::MAX
                && (*l == biggest || area[*l as usize] > big_enough || polar[*l as usize])
        })
        .collect()
}

/// A hotspot volcano: (position, edifice height above the sea floor in metres, radius rad).
type ChainVolcano = (DVec3, f64, f64);

fn hotspot_chain(layout: &TectonicLayout, scale: &Scale, seed: u64) -> Vec<ChainVolcano> {
    let mut out = Vec::new();
    let mut rng = Rng::new(derive_seed(seed, "hotspot-chain"));
    for hs in &layout.hotspots {
        let plate = &layout.plates[layout.locate(hs.pos).plate as usize];
        let axis = plate.omega.normalize_or_zero();
        if axis == DVec3::ZERO {
            continue;
        }
        let h0 = 6200.0 * hs.strength;
        let r0 = scale.footprint(35.0, h0, 0.9);
        let mut angle = 0.0f64;
        for k in 0..12 {
            let h = h0 * (-0.28 * k as f64).exp() - 180.0 * k as f64;
            if h < 900.0 {
                break;
            }
            let r = scale.footprint(35.0, h, 0.9);
            let q = glam::DQuat::from_axis_angle(axis, -angle);
            let pos = (q * hs.pos).normalize();
            out.push((pos, h, r));
            angle += (r0 + r) * rng.range_f64(0.55, 0.8);
        }
    }
    out
}

struct Hydro {
    water: Vec<f32>,
    discharge: Vec<f32>,
    receiver: Vec<u32>,
    flags: Vec<u8>,
}

/// Final drainage: fills depressions, decides which basins hold lakes and which are
/// endorheic (arid, no outflow), accumulates discharge with transmission losses, and marks
/// rivers.
fn finish_hydrology(
    geom: &GridGeom,
    elev: &[f32],
    ocean: &[bool],
    polar: &[bool],
    cl: &climate::ClimateFields,
    v: f64,
) -> Hydro {
    let n = geom.n;
    let len = geom.len();
    let base: Vec<bool> = (0..len).map(|i| ocean[i] || polar[i]).collect();
    let filled = hydrology::fill_depressions(geom, elev, &base, 0.01);
    // Depression cells (would hold water if the basin filled to its spill point).
    let depression: Vec<bool> = (0..len)
        .map(|idx| !base[idx] && filled[idx] - elev[idx] > 0.5)
        .collect();
    let (label, count) = hydrology::label_components(geom, &depression);
    let mut rec = hydrology::receivers(geom, &filled, &base);
    let order = hydrology::stack_order(&rec);
    let rain: Vec<f32> = (0..len)
        .map(|idx| {
            let j = idx / n;
            hydrology::cell_sqdeg(geom, j) as f32 * cl.precipitation[idx] / 1000.0
        })
        .collect();
    let cell_area = |idx: usize| hydrology::cell_sqdeg(geom, idx / n) as f32;
    // Potential evaporation (m/yr) from temperature and aridity.
    let evap: Vec<f32> = (0..len)
        .map(|idx| {
            let t = cl.temperature[idx].max(-5.0);
            (0.25 + 0.075 * t).clamp(0.1, 2.4)
        })
        .collect();
    let catchment = hydrology::accumulate(&rec, &order, &rain, None);

    // Per basin: inflow (max catchment inside = water arriving at the outlet), area, evaporation.
    let mut basin_inflow = vec![0f32; count as usize];
    let mut basin_area = vec![0f32; count as usize];
    let mut basin_evap = vec![0f32; count as usize];
    for idx in 0..len {
        let l = label[idx];
        if l == u32::MAX {
            continue;
        }
        let l = l as usize;
        basin_inflow[l] = basin_inflow[l].max(catchment[idx]);
        let a = cell_area(idx);
        basin_area[l] += a;
        basin_evap[l] += a * evap[idx];
    }
    let endorheic: Vec<bool> = (0..count as usize)
        .map(|l| basin_inflow[l] < basin_evap[l] * 0.9)
        .collect();

    let mut flags = vec![0u8; len];
    let mut water = vec![f32::NAN; len];
    for idx in 0..len {
        if ocean[idx] {
            flags[idx] |= flags::OCEAN;
            water[idx] = 0.0;
        }
    }
    // Lakes: open basins fill to their spill point; closed basins shrink to their water balance.
    let mut basin_cells: Vec<Vec<u32>> = vec![Vec::new(); count as usize];
    for (idx, l) in label.iter().enumerate() {
        if *l != u32::MAX {
            basin_cells[*l as usize].push(idx as u32);
        }
    }
    for (l, cells) in basin_cells.iter_mut().enumerate() {
        if cells.is_empty() {
            continue;
        }
        if !endorheic[l] {
            let level = cells
                .iter()
                .map(|c| filled[*c as usize])
                .fold(f32::INFINITY, f32::min);
            for &c in cells.iter() {
                let c = c as usize;
                if elev[c] < level {
                    flags[c] |= flags::LAKE;
                    water[c] = level;
                }
            }
        } else {
            // Water balance: the lake grows from the lowest point until its evaporation
            // matches the inflow.
            cells.sort_by(|a, b| elev[*a as usize].total_cmp(&elev[*b as usize]));
            let inflow = basin_inflow[l];
            let mut area = 0.0f32;
            let mut level = f32::NAN;
            for &c in cells.iter() {
                let c = c as usize;
                let a = cell_area(c);
                if (area + a) * evap[c] > inflow {
                    break;
                }
                area += a;
                level = elev[c];
            }
            let lake_cells = cells
                .iter()
                .take_while(|c| !level.is_nan() && elev[**c as usize] <= level)
                .count();
            for (k, &c) in cells.iter().enumerate() {
                let c = c as usize;
                flags[c] |= flags::ENDORHEIC;
                if k < lake_cells && lake_cells >= 2 {
                    flags[c] |= flags::LAKE;
                    water[c] = level + 0.5;
                } else if k < 3.max(cells.len() / 6) {
                    flags[c] |= flags::SALT_FLAT;
                }
            }
            // Endorheic basins keep their water: flow terminates at the basin floor.
            if let Some(&low) = cells.first() {
                for &c in cells.iter() {
                    rec[c as usize] = low;
                }
                rec[low as usize] = low;
            }
        }
    }
    // Discharge with transmission losses in dry climates (small rivers vanish in deserts,
    // great rivers cross them).
    let order = hydrology::stack_order(&rec);
    let loss: Vec<f32> = (0..len)
        .map(|idx| {
            let p = cl.precipitation[idx];
            if ocean[idx] || flags[idx] & flags::LAKE != 0 {
                0.0
            } else {
                (0.02 * (1.0 - p / 600.0)).clamp(0.0, 0.02)
            }
        })
        .collect();
    let discharge = hydrology::accumulate(&rec, &order, &rain, Some(&loss));
    let _ = v;
    for idx in 0..len {
        if !ocean[idx] && flags[idx] & flags::LAKE == 0 && discharge[idx] > RIVER_MIN_DISCHARGE {
            flags[idx] |= flags::RIVER;
            water[idx] = filled[idx];
        }
    }
    // Deltas: large rivers entering the sea on gentle coasts.
    for idx in 0..len {
        if flags[idx] & flags::RIVER != 0 && discharge[idx] > RIVER_MIN_DISCHARGE * 60.0 {
            let r = rec[idx] as usize;
            if ocean[r] {
                flags[idx] |= flags::DELTA;
            }
        }
    }
    Hydro {
        water,
        discharge,
        receiver: rec,
        flags,
    }
}

/// Picks volcano summits: hotspot chain members plus local maxima of volcanic-arc terrain.
fn find_volcanoes(
    geom: &GridGeom,
    elev: &[f32],
    uplift: &[f32],
    chain: &[ChainVolcano],
    flags: &mut [u8],
    province: &[u8],
    seed: u64,
    v: f64,
) -> Vec<Volcano> {
    let n = geom.n;
    let planet = *geom.planet();
    let mut rng = Rng::new(derive_seed(seed, "volcano"));
    let mut out = Vec::new();
    for &(pos, _, _) in chain {
        let (x, z) = planet.world_xz_of_sphere_point(pos);
        let idx = {
            let (gx, gz) = geom.grid_coords(x, z);
            let i = (gx.round() as isize).rem_euclid(n as isize) as usize;
            let j = (gz.round() as isize).clamp(0, n as isize - 1) as usize;
            geom.idx(i, j)
        };
        flags[idx] |= flags::VOLCANIC;
        out.push(make_volcano(x, z, elev[idx], &mut rng, v));
    }
    // Arc volcanoes: local maxima in arc provinces.
    for j in 2..n - 2 {
        for i in 0..n {
            let idx = j * n + i;
            if province[idx] != province::ARC || uplift[idx] < 1000.0 {
                continue;
            }
            let e = elev[idx];
            let is_max = (-2isize..=2).all(|dj| {
                (-2isize..=2).all(|di| {
                    (di == 0 && dj == 0)
                        || geom.neighbor(i, j, di, dj).is_none_or(|nb| elev[nb] < e)
                })
            });
            if is_max && rng.chance(0.35) {
                flags[idx] |= flags::VOLCANIC;
                let (x, z) = geom.world_xz(i, j);
                out.push(make_volcano(x, z, e, &mut rng, v));
            }
        }
    }
    out
}

fn make_volcano(x: f64, z: f64, summit: f32, rng: &mut Rng, v: f64) -> Volcano {
    let scale = (v / 0.25).sqrt() as f32;
    Volcano {
        x,
        z,
        summit,
        crater_radius: rng.range_f32(14.0, 40.0) * scale,
        crater_depth: rng.range_f32(8.0, 26.0) * scale,
        crater_lake: rng.chance(0.35),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hearth_math::PlanetSize;

    pub fn small_settings(seed: u64) -> WorldGenSettings {
        WorldGenSettings {
            seed,
            planet_size: PlanetSize::Standard,
            grid_resolution: 256,
        }
    }

    #[test]
    fn builds_deterministically_with_target_land_fraction() {
        let s = small_settings(11);
        let a = PlanetGrid::build(&s, &|_, _| {});
        let b = PlanetGrid::build(&s, &|_, _| {});
        assert_eq!(a.elevation.data, b.elevation.data);
        assert_eq!(a.climate, b.climate);
        let lf = a.land_fraction();
        assert!((0.2..0.42).contains(&lf), "land fraction {lf}");
    }
}

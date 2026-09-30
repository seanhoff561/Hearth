//! Seasonal river regimes (v2 §5): how much water each river reach carries through the year,
//! relative to its mean.
//!
//! A reach carries the runoff of its whole upstream basin, each part following its own climate
//! (the year-scale water balance of `SeasonalCover`: rain, snowmelt, evaporation, slow
//! drainage through the ground) and arriving after the flood wave's travel time down the
//! network. So a small river follows the seasons of its own valley (a spring flood where snow
//! melts, a dry-season low in a savanna), while a great river crossing a desert floods with the
//! rains of distant highlands, weeks later. Each part's weight is the water it adds: the reach's
//! own gain in discharge on the planet grid, times the share of its rain that runs off.
//!
//! The regimes add up in the frequency domain, where a travel delay is an exact phase shift
//! (all harmonics of the 73-step year are kept); each reach then keeps its year as a series,
//! smoothed over about one step (Lanczos σ factors) so that sharp floods shifted by part of a
//! step do not ring.

use std::f64::consts::TAU;

use hearth_worldgen::PlanetGrid;
use rayon::prelude::*;
use rustc_hash::FxHashMap;

#[cfg(test)]
use crate::climate::flow_from_harmonics;
use crate::climate::{Normals, STEPS, SeasonalCover, flow_at_step};

/// Harmonics of a year series of `STEPS` samples: all of them.
pub const HARMONICS: usize = STEPS / 2;
/// A year series in the frequency domain: (re, im) of harmonics 1..=HARMONICS of a flow
/// relative to its mean (`SeasonalCover::flow_harmonics`).
pub type Spectrum = [(f64, f64); HARMONICS];
/// Speed of a flood wave down a river (m/s).
const WAVE_SPEED_M_S: f64 = 1.5;
/// Earth's circumference (m). Travel times use real distances: the planet's rivers stand for
/// Earth-sized ones, as its climate does.
const EARTH_CIRCUMFERENCE_M: f64 = 40_075_000.0;
/// Share of a reach's water that runs off even where the year's balance leaves none (storms,
/// springs): keeps rivers of arid basins from having no regime at all.
const MIN_RUNOFF_RATIO: f64 = 0.02;
const SECONDS_PER_YEAR: f64 = 365.2422 * 86_400.0;
/// Stored flows are thousandths of the mean.
const FLOW_UNIT: f64 = 1000.0;

/// One reach of a river network, for accumulation.
#[derive(Debug, Clone, Copy)]
pub struct Reach {
    /// Downstream reach (index into the same slice), if the river goes on as a river.
    pub receiver: Option<usize>,
    /// Water the reach adds itself (any unit, the same for all reaches).
    pub gain: f64,
    /// Regime of the water it adds.
    pub local: Spectrum,
    /// Travel time of a flood wave to the receiver (years).
    pub travel_years: f64,
}

/// Regimes of all reaches: each the gain-weighted mix of everything upstream, delayed by the
/// travel times on the way. Receivers must form a forest (no cycles).
pub fn accumulate(reaches: &[Reach]) -> Vec<Spectrum> {
    let m = reaches.len();
    let mut pending = vec![0u32; m];
    for r in reaches {
        if let Some(t) = r.receiver {
            pending[t] += 1;
        }
    }
    let mut weight: Vec<f64> = reaches.iter().map(|r| r.gain.max(0.0)).collect();
    let mut sum: Vec<Spectrum> = reaches
        .iter()
        .map(|r| {
            let g = r.gain.max(0.0);
            r.local.map(|(re, im)| (re * g, im * g))
        })
        .collect();
    let mut ready: Vec<usize> = (0..m).filter(|&k| pending[k] == 0).collect();
    while let Some(k) = ready.pop() {
        let Some(t) = reaches[k].receiver else {
            continue;
        };
        // Water leaving k at time s reaches t at s + travel: a phase lag per harmonic.
        let lag = reaches[k].travel_years;
        let from = sum[k];
        for (h, ((re, im), to)) in from.iter().zip(sum[t].iter_mut()).enumerate() {
            let (s, c) = (TAU * (h + 1) as f64 * lag).sin_cos();
            to.0 += re * c + im * s;
            to.1 += im * c - re * s;
        }
        weight[t] += weight[k];
        pending[t] -= 1;
        if pending[t] == 0 {
            ready.push(t);
        }
    }
    sum.iter()
        .zip(&weight)
        .map(|(s, w)| {
            if *w > 0.0 {
                s.map(|(re, im)| (re / w, im / w))
            } else {
                [(0.0, 0.0); HARMONICS]
            }
        })
        .collect()
}

/// The seasonal flow regime of every river reach of a planet.
#[derive(Debug, Default)]
pub struct RiverRegimes {
    /// River cell → offset of its year in `flows`.
    index: FxHashMap<u32, u32>,
    /// `STEPS` flows per reach, in thousandths of its mean.
    flows: Vec<u16>,
}

impl RiverRegimes {
    /// Regimes of all river cells of the planet grid (tens of milliseconds on a Standard
    /// planet: one year-scale water balance per river cell).
    pub fn build(grid: &PlanetGrid) -> Self {
        let geom = &grid.geom;
        let mut cells: Vec<u32> = grid.rivers.keys().copied().collect();
        cells.sort_unstable();
        let index: FxHashMap<u32, usize> = cells.iter().enumerate().map(|(k, c)| (*c, k)).collect();
        // Each reach's own climate: its runoff regime and the share of its rain that runs off.
        let local: Vec<(Spectrum, f64)> = cells
            .par_iter()
            .map(|&c| {
                let (i, j) = geom.ij(c as usize);
                let (x, z) = geom.world_xz(i, j);
                let n = Normals::sample(grid, x, z);
                let cover = SeasonalCover::compute(&n);
                let ratio = if n.precip > 0.0 {
                    cover.runoff_mm as f64 / n.precip
                } else {
                    0.0
                };
                (
                    cover.flow_harmonics::<HARMONICS>(),
                    ratio.max(MIN_RUNOFF_RATIO),
                )
            })
            .collect();
        // Discharge arriving from upstream reaches, to find what each reach adds itself.
        let mut inflow = vec![0f64; cells.len()];
        for c in &cells {
            let r = grid.rivers[c];
            if let Some(&t) = index.get(&r.receiver)
                && r.receiver != *c
            {
                inflow[t] += r.discharge as f64;
            }
        }
        let metres_per_block = EARTH_CIRCUMFERENCE_M / geom.c;
        let n = geom.n as i64;
        let reaches: Vec<Reach> = cells
            .iter()
            .enumerate()
            .map(|(k, &c)| {
                let r = grid.rivers[&c];
                let receiver = index.get(&r.receiver).copied().filter(|&t| t != k);
                let (i, j) = geom.ij(c as usize);
                let (ri, rj) = geom.ij(r.receiver as usize);
                let di = (ri as i64 - i as i64 + n / 2).rem_euclid(n) - n / 2;
                let dj = rj as i64 - j as i64;
                // Mercator: the map scale shrinks with cos(latitude) in both directions.
                let blocks = geom.phys_cell(j) * ((di * di + dj * dj) as f64).sqrt();
                let (local, ratio) = local[k];
                Reach {
                    receiver,
                    gain: (r.discharge as f64 - inflow[k]).max(0.0) * ratio,
                    local,
                    travel_years: blocks * metres_per_block / WAVE_SPEED_M_S / SECONDS_PER_YEAR,
                }
            })
            .collect();
        let spectra = accumulate(&reaches);
        let sigma: Vec<f64> = (1..=HARMONICS)
            .map(|h| {
                let a = std::f64::consts::PI * h as f64 / (HARMONICS + 1) as f64;
                a.sin() / a
            })
            .collect();
        let years: Vec<[u16; STEPS]> = spectra
            .par_iter()
            .map(|h| {
                let mut h = *h;
                for (c, s) in h.iter_mut().zip(&sigma) {
                    *c = (c.0 * s, c.1 * s);
                }
                let h = &h;
                let mut year = [0u16; STEPS];
                for (k, f) in year.iter_mut().enumerate() {
                    *f = (flow_at_step(h, k) * FLOW_UNIT)
                        .round()
                        .min(u16::MAX as f64) as u16;
                }
                year
            })
            .collect();
        Self {
            index: cells
                .iter()
                .enumerate()
                .map(|(k, c)| (*c, (k * STEPS) as u32))
                .collect(),
            flows: years.into_iter().flatten().collect(),
        }
    }

    /// Number of reaches with a regime.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Flow of a river cell's reach relative to its mean at a year fraction (1 for cells with
    /// no regime).
    pub fn flow(&self, cell: u32, year_frac: f64) -> f64 {
        let Some(&o) = self.index.get(&cell) else {
            return 1.0;
        };
        let year = &self.flows[o as usize..o as usize + STEPS];
        let x = year_frac.rem_euclid(1.0) * STEPS as f64 - 0.5;
        let i0 = x.floor().rem_euclid(STEPS as f64) as usize;
        let i1 = (i0 + 1) % STEPS;
        let t = x - x.floor();
        (year[i0] as f64 * (1.0 - t) + year[i1] as f64 * t) / FLOW_UNIT
    }

    /// The year fraction of a reach's highest flow.
    pub fn peak(&self, cell: u32) -> Option<f64> {
        let o = *self.index.get(&cell)? as usize;
        let year = &self.flows[o..o + STEPS];
        let k = (0..STEPS).max_by_key(|&k| year[k])?;
        Some((k as f64 + 0.5) / STEPS as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regime(n: &Normals) -> Spectrum {
        SeasonalCover::compute(n).flow_harmonics::<HARMONICS>()
    }

    fn peak(h: &Spectrum) -> f64 {
        (0..365)
            .map(|d| d as f64 / 365.0)
            .fold((0.0, f64::MIN), |best, t| {
                let f = flow_from_harmonics(h, t);
                if f > best.1 { (t, f) } else { best }
            })
            .0
    }

    #[test]
    fn a_desert_reach_floods_with_distant_rains_later() {
        // A monsoon highland (wet summer) feeds a long river through a desert that adds nothing.
        let monsoon = regime(&Normals::new(10.0, 22.0, 5.0, 1400.0, 0.9, 0.0));
        let desert = regime(&Normals::new(24.0, 27.0, 14.0, 20.0, 0.0, 0.0));
        let reaches = [
            Reach {
                receiver: Some(1),
                gain: 100.0,
                local: monsoon,
                travel_years: 0.04,
            },
            Reach {
                receiver: None,
                gain: 0.0,
                local: desert,
                travel_years: 0.0,
            },
        ];
        let out = accumulate(&reaches);
        let (up, down) = (peak(&out[0]), peak(&out[1]));
        let lag = (down - up).rem_euclid(1.0);
        assert!(
            (0.03..0.05).contains(&lag),
            "flood arrives two weeks later: {lag}"
        );
        let hi = flow_from_harmonics(&out[1], down);
        let lo = flow_from_harmonics(&out[1], (down + 0.5).fract());
        assert!(hi > 1.5 && lo < 0.6, "the desert reach floods: {hi} / {lo}");
    }

    #[test]
    fn confluences_mix_by_the_water_they_bring() {
        // A snowmelt tributary (spring flood) and a mediterranean one (winter rains) meet.
        let melt = regime(&Normals::new(50.0, 2.0, 24.0, 700.0, 0.0, 0.0));
        let winter_rain = regime(&Normals::new(38.0, 16.0, 16.0, 700.0, 0.0, 0.9));
        let reach = |receiver, gain, local| Reach {
            receiver,
            gain,
            local,
            travel_years: 0.0,
        };
        let out = accumulate(&[
            reach(Some(2), 300.0, melt),
            reach(Some(2), 100.0, winter_rain),
            reach(None, 0.0, [(0.0, 0.0); HARMONICS]),
        ]);
        for (k, h) in out[2].iter().enumerate() {
            let want = (
                0.75 * melt[k].0 + 0.25 * winter_rain[k].0,
                0.75 * melt[k].1 + 0.25 * winter_rain[k].1,
            );
            assert!((h.0 - want.0).abs() < 1e-9 && (h.1 - want.1).abs() < 1e-9);
        }
        // The larger tributary sets the season of the flood.
        assert!((peak(&out[2]) - peak(&out[0])).abs() < 0.08);
    }
}

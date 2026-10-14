//! The walking level (G1b): the finest refinement level on Earth, its cells some metres across
//! (the walking scale's), made only about the players.
//!
//! Its tiles are built as every level's are (`Relief::build`); what is its own is here:
//! - a second pass of its drainage over tiles offset half a tile from the first's, which cuts
//!   the dams the first pass's blend leaves between tiles (`Relief::drained_tile`);
//! - its tiles made side by side about a place before its first cubes want them
//!   ([`Relief::prefetch_walking`]);
//! - its stand-in for every other read ([`Relief::walking_proxy`]): its octaves over its
//!   parent's surface, without the drainage its tiles work on them, for distant tiles and
//!   placement, so that nothing but the blocks about the players makes its tiles.

use std::sync::{Arc, OnceLock};

use super::drainage::fill;
use super::{MARGIN, Relief, Run, SPAN, TILE, TileKey, WALKING_CELL, bare, smoothstep};

impl Relief {
    /// The walking scale's level (cells of some metres), if the planet has one: its finest.
    pub fn walking_level(&self) -> Option<usize> {
        let level = self.levels.len();
        self.walking(level).then_some(level)
    }

    /// Whether a level is the walking scale's (cells of some metres).
    pub(super) fn walking(&self, level: usize) -> bool {
        level >= 1 && level == self.levels.len() && self.levels[level - 1].cell < WALKING_CELL
    }

    /// Makes the walking level's tiles about world (x, z), `radius` blocks round, side by side:
    /// the first pass's, then the second's. A world's first cubes so find them made, rather than
    /// each making the ones it needs one after another (E4.1 §4.4).
    pub fn prefetch_walking(&self, x: f64, z: f64, radius: f64) {
        use rayon::prelude::*;
        let level = self.levels.len();
        if !self.walking(level) {
            return;
        }
        let s = self.cell_size(level);
        let count = self.count(level);
        let round = count.div_euclid(TILE).max(1);
        let half = self.c * 0.5;
        let (i0, i1) = (
            ((x - radius) / s).floor() as i64,
            ((x + radius) / s).ceil() as i64,
        );
        let (j0, j1) = (
            ((z - radius + half) / s).floor() as i64,
            ((z + radius + half) / s).ceil() as i64,
        );
        let tiles = |lo: i64, hi: i64, shift: i64, pad: i64| {
            (lo - shift - pad).div_euclid(TILE)..=(hi - shift + pad).div_euclid(TILE)
        };
        // The second pass's tiles about the place, and every first-pass tile their spans read.
        let second: Vec<(i64, i64)> = tiles(i0, i1, TILE / 2, MARGIN)
            .flat_map(|tx| tiles(j0, j1, TILE / 2, MARGIN).map(move |tz| (tx, tz)))
            .filter(|&(_, tz)| tz * TILE + TILE / 2 < count)
            .collect();
        let first: Vec<(i64, i64)> = tiles(i0, i1, 0, 2 * MARGIN + TILE / 2)
            .flat_map(|tx| tiles(j0, j1, 0, 2 * MARGIN + TILE / 2).map(move |tz| (tx, tz)))
            .filter(|&(_, tz)| tz >= 0 && tz * TILE < count)
            .collect();
        first.par_iter().for_each(|&(tx, tz)| {
            self.tile(level, tx.rem_euclid(round), tz);
        });
        second.par_iter().for_each(|&(tx, tz)| {
            self.drained_tile(level, tx.rem_euclid(round), tz);
        });
    }

    /// The walking level's surface at a cell after its second pass: the second pass's tiles,
    /// offset half a tile from the first's, blended as the first's are.
    pub(super) fn drained_h_in(
        &self,
        level: usize,
        i: i64,
        j: i64,
        run: &mut Run<Vec<f32>>,
    ) -> f32 {
        let count = self.count(level);
        let i = i.rem_euclid(count);
        let j = j.clamp(0, count - 1);
        let tiles_round = count.div_euclid(TILE).max(1);
        // Cells counted from the second pass's tiling, half a tile on.
        let (si, sj) = (i - TILE / 2, j - TILE / 2);
        let (tx, tz) = (si.div_euclid(TILE), sj.div_euclid(TILE));
        let (ci, cj) = (si - tx * TILE, sj - tz * TILE);
        let at = |t: &[f32], dx: i64, dz: i64| {
            t[((cj + MARGIN - dz * TILE) * SPAN + ci + MARGIN - dx * TILE) as usize] as f64
        };
        let own_x = tx.rem_euclid(tiles_round);
        let own = run.at(own_x, tz, || self.drained_tile(level, own_x, tz));
        let (mut sum, mut weight) = (at(own, 0, 0), 1.0);
        for dz in -1..=1i64 {
            for dx in -1..=1i64 {
                if dx == 0 && dz == 0 {
                    continue;
                }
                let ex = match dx {
                    -1 => ci as f64 + 0.5,
                    1 => (TILE - ci) as f64 - 0.5,
                    _ => 0.0,
                };
                let ez = match dz {
                    -1 => cj as f64 + 0.5,
                    1 => (TILE - cj) as f64 - 0.5,
                    _ => 0.0,
                };
                if ex >= MARGIN as f64 || ez >= MARGIN as f64 {
                    continue;
                }
                let wgt = 1.0 - smoothstep(0.0, MARGIN as f64, (ex * ex + ez * ez).sqrt());
                if wgt <= 0.0 {
                    continue;
                }
                let (ntx, ntz) = ((tx + dx).rem_euclid(tiles_round), tz + dz);
                let t = run.at(ntx, ntz, || self.drained_tile(level, ntx, ntz));
                sum += wgt * at(t, dx, dz);
                weight += wgt;
            }
        }
        (sum / weight) as f32
    }

    /// The walking level's second pass over a tile of its own tiling (offset half a tile from the
    /// first's): the first pass's blended surface, drained again. Each first-pass tile drains its
    /// own span, but two tiles may lay the same hollow's way out differently in their overlap,
    /// and their blend there can dam it. Every such overlap lies inside a second-pass tile's core,
    /// which cuts the dam (or fills what cannot be cut, the sea's level being the lowest); and
    /// where second-pass tiles overlap, the first pass is one tile's own drained surface, which
    /// they leave as it is, so their blend dams nothing.
    fn drained_tile(&self, level: usize, tx: i64, tz: i64) -> Arc<OnceLock<Vec<f32>>> {
        let key = TileKey {
            level: level as u8,
            tx,
            tz,
        };
        let tile = self.drained.get_or_insert_with(key, OnceLock::new);
        tile.get_or_init(|| {
            let _zone = hearth_core::prof::Zone::new("relief.drain");
            let n = SPAN as usize;
            let count = self.count(level);
            let (i0, j0) = (tx * TILE + TILE / 2 - MARGIN, tz * TILE + TILE / 2 - MARGIN);
            let mut ground = vec![0.0f32; n * n];
            let mut wet = vec![0.0f32; n * n];
            let mut base = vec![false; n * n];
            let mut run = Run::default();
            for w in 0..n {
                for u in 0..n {
                    let k = w * n + u;
                    let j = j0 + w as i64;
                    let edge = u == 0 || w == 0 || u == n - 1 || w == n - 1;
                    if j < 0 || j >= count {
                        base[k] = true;
                        continue;
                    }
                    let c = self.blended_in(level, i0 + u as i64, j, &mut run);
                    ground[k] = c.h;
                    wet[k] = if c.lake.is_finite() {
                        c.lake.max(c.h)
                    } else {
                        c.h
                    };
                    base[k] = edge || c.sea || c.channel || c.lake.is_finite();
                }
            }
            fill(&mut wet, &base, n, 1e-3, &vec![f32::MAX; n * n]);
            (0..n * n)
                .map(|k| if base[k] { ground[k] } else { wet[k] })
                .collect()
        });
        tile
    }

    /// The walking level's relief as a coarser read stands in for it (a distant tile, the view
    /// from a hill): its octaves no shorter than `min_wl` (blocks; finer would not show between
    /// the columns) over its parent's surface, whose slope is `slope`, without the drainage its
    /// tiles work on it. Nought where the planet has no walking level.
    pub fn walking_proxy(&self, x: f64, z: f64, slope: (f64, f64), min_wl: f64) -> f64 {
        let level = self.levels.len();
        // None where the planet has no walking level, nor where even its longest octave is
        // finer than the columns.
        if level < 2 || self.levels[level - 1].cell >= WALKING_CELL || {
            let noise = &self.noise[level - 1];
            noise.dissect.first().is_none_or(|o| o.2 < min_wl)
        } {
            return 0.0;
        }
        let g = &*self.grid;
        let (gx, gz) = g.geom.grid_coords(x.rem_euclid(self.c), z);
        let rough = self.roughness_at(gx, gz);
        let p = g.precipitation.bilinear(gx, gz) as f64;
        let t = g.temperature.bilinear(gx, gz) as f64;
        let mut a = [0.0; 16];
        self.amplitudes(level, &rough, bare(p, t), &mut a);
        self.octaves_at(level, x, z, &a, slope, min_wl)
    }
}

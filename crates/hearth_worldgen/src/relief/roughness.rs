//! How much relief a place takes at the refinement levels (G1a): by its kind of land and its
//! height above base level, not by uplift alone.
//!
//! Real relief grows with how far the land stands above the water it drains to (the sea, a
//! lake, the trunk river): rivers cut down towards that level and the hillslopes follow them
//! (Ahnert 1970; Montgomery and Brandon 2002). How much of that height a landscape has turned
//! into relief, and at what spacing its valleys lie, depends on the kind of land: humid hills are
//! cut through to their streams at a few hundred metres (Perron et al. 2009), dry plains keep
//! broad flat interfluves, a glaciated shield is a low knobbly surface, mountains are relief
//! throughout.
//!
//! A place's relief is an amplitude at its valleys' spacing, falling away below it (smooth
//! hillslopes; amplitude ∝ λ^0.95, as real ground's roughness grows with the lag) and growing
//! slowly beyond (∝ λ^0.2: the height to cut bounds it); the spacing is as far as hillslopes of
//! the place's gradient take to rise through the relief. Calibrated to `bench realism global`
//! and `terrain` (`docs/review/realism/measures/`, D299).

use crate::planet::grid::Field;
use crate::planet::{FLOW_D8, NO_FLOW, PlanetGrid, flags, province};

/// Cells about a cell whose graded profiles count for its base level (some 40 km on the Earth's
/// grid): a plateau's rim stands over the valley beside it.
const BASE_AROUND: i64 = 2;
/// The amplitude's growth with wavelength below and beyond the valleys' spacing.
const SHORT: f64 = 0.95;
const LONG: f64 = 0.2;
/// The gradient of the graded profile rivers cut to (m per m): a large river's.
const GRADE: f64 = 0.5e-3;

/// The relief a place takes at the levels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Roughness {
    /// Amplitude (m, ±) at the valleys' spacing.
    pub amp: f64,
    /// The valleys' spacing (m).
    pub spacing: f64,
    /// How much of the levels' relief the ground lying low among its neighbours loses (0 … 1):
    /// dry land's basins buried in what its ranges shed, their floors flat between the ranges.
    pub fill: f64,
}

impl Roughness {
    /// The relief's amplitude (m, ±) at a wavelength (m): ∝ λ^0.95 below the valleys' spacing,
    /// ∝ λ^0.2 beyond, joined smoothly.
    pub fn at(&self, wavelength_m: f64) -> f64 {
        let x = wavelength_m / self.spacing;
        2.0 * self.amp * x.powf(SHORT) / (1.0 + x.powf(SHORT - LONG))
    }
}

#[inline]
fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Each grid cell's height above its base level (m): above the graded profile its rivers could
/// cut to, the lowest, over its course downstream, of the ground or water there with a gentle
/// gradient ([`GRADE`]) added for the way to it (0 at sea), or the lower such profile of
/// a cell within [`BASE_AROUND`]; blurred over a cell, so that relief changes smoothly. A plain
/// sloping to the sea more gently than the grade stands at its base level; a plateau far inland
/// stands above it by its height less the grade's rise.
fn above_base(g: &PlanetGrid) -> Field<f32> {
    use rayon::prelude::*;
    let n = g.n();
    let len = n * n;
    let grade = GRADE / g.vertical_scale;
    let e = &g.elevation.data;
    let sea = |k: usize| g.flags[k] & flags::OCEAN != 0;
    // The ground, or a lake's surface over it.
    let own = |k: usize| {
        if g.flags[k] & flags::LAKE != 0 && g.water.data[k].is_finite() {
            g.water.data[k].max(0.0)
        } else {
            e[k].max(0.0)
        }
    };
    let receiver = |k: usize| -> Option<(usize, f64)> {
        let code = g.flow[k];
        if code == NO_FLOW {
            return None;
        }
        let (i, j) = ((k % n) as i64, (k / n) as i64);
        let (di, dj) = FLOW_D8[code as usize];
        let nj = j + dj;
        if nj < 0 || nj >= n as i64 {
            return None;
        }
        let step =
            g.geom.phys_cell(j as usize) * if di != 0 && dj != 0 { 2f64.sqrt() } else { 1.0 };
        Some((
            (nj * n as i64 + (i + di).rem_euclid(n as i64)) as usize,
            step,
        ))
    };
    // The graded profile under each cell, each course followed down to a cell already known (or
    // the sea, or its end) and then back up.
    const UNKNOWN: f32 = f32::NAN;
    let mut graded = vec![UNKNOWN; len];
    let mut on_path = vec![false; len];
    let mut path: Vec<(usize, f64)> = Vec::new();
    for start in 0..len {
        if !graded[start].is_nan() {
            continue;
        }
        path.clear();
        let mut k = start;
        let mut below: Option<f32> = loop {
            if !graded[k].is_nan() {
                break Some(graded[k]);
            }
            if sea(k) {
                graded[k] = 0.0;
                break Some(0.0);
            }
            if on_path[k] {
                // A ring (the drainage should have none): it ends here.
                break None;
            }
            on_path[k] = true;
            match receiver(k) {
                Some((r, step)) => {
                    path.push((k, step));
                    k = r;
                }
                None => {
                    path.push((k, 0.0));
                    break None;
                }
            }
        };
        for &(c, step) in path.iter().rev() {
            let mut v = own(c);
            if let Some(b) = below {
                v = v.min(b + (grade * step) as f32);
            }
            graded[c] = v;
            on_path[c] = false;
            below = Some(v);
        }
    }
    let mut out: Vec<f32> = (0..len)
        .into_par_iter()
        .map(|k| {
            if sea(k) || e[k] <= 0.0 {
                return 0.0;
            }
            let (i, j) = ((k % n) as i64, (k / n) as i64);
            let mut low = graded[k];
            for dj in -BASE_AROUND..=BASE_AROUND {
                let jj = j + dj;
                if jj < 0 || jj >= n as i64 {
                    continue;
                }
                let dx = g.geom.phys_cell(jj as usize);
                for di in -BASE_AROUND..=BASE_AROUND {
                    let kk = (jj * n as i64 + (i + di).rem_euclid(n as i64)) as usize;
                    let d = dx * ((di * di + dj * dj) as f64).sqrt();
                    low = low.min(graded[kk] + (grade * d) as f32);
                }
            }
            (e[k] - low.max(0.0)).max(0.0)
        })
        .collect();
    crate::planet::fields::blur(&g.geom, &mut out, 1, 1);
    Field::from_vec(n, out)
}

/// What the grid says of its land's relief, made once with the levels.
pub struct Land {
    /// Each cell's height above its base level (m, [`above_base`]).
    pub above: Field<f32>,
    /// The relief of the range about each cell (m): its highest ground less its lowest within
    /// [`BASE_AROUND`] cells (the sea's level at the lowest), blurred over a cell. Ranges of
    /// great relief are cut fast and their hillslopes stand at the threshold of sliding
    /// (Montgomery and Brandon 2002).
    pub range: Field<f32>,
}

impl Land {
    pub fn new(g: &PlanetGrid) -> Self {
        use rayon::prelude::*;
        let n = g.n();
        let e = &g.elevation.data;
        let mut range: Vec<f32> = (0..n * n)
            .into_par_iter()
            .map(|k| {
                let (i, j) = ((k % n) as i64, (k / n) as i64);
                let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                for dj in -BASE_AROUND..=BASE_AROUND {
                    let jj = (j + dj).clamp(0, n as i64 - 1);
                    for di in -BASE_AROUND..=BASE_AROUND {
                        let v =
                            e[(jj * n as i64 + (i + di).rem_euclid(n as i64)) as usize].max(0.0);
                        lo = lo.min(v);
                        hi = hi.max(v);
                    }
                }
                hi - lo
            })
            .collect();
        crate::planet::fields::blur(&g.geom, &mut range, 1, 1);
        Self {
            above: above_base(g),
            range: Field::from_vec(n, range),
        }
    }

    /// Nothing (a planet without levels).
    pub fn none() -> Self {
        Self {
            above: Field::new(0, 0.0),
            range: Field::new(0, 0.0),
        }
    }
}

/// The relief a place takes (at grid coordinates): its amplitude from its height above base
/// level, its height and its kind; the valleys' spacing from how steep its hillslopes stand.
pub fn roughness(g: &PlanetGrid, land: &Land, gx: f64, gz: f64) -> Roughness {
    let e = g.elevation.bilinear(gx, gz) as f64;
    if e < 0.0 {
        // The shelf smooth, the slope and the deep floor in abyssal hills.
        return Roughness {
            amp: 1.5 + 40.0 * smoothstep(-150.0, -2500.0, e),
            spacing: 4000.0,
            fill: 0.0,
        };
    }
    let hb = land.above.bilinear(gx, gz).max(0.0) as f64;
    let range = land.range.bilinear(gx, gz).max(0.0) as f64;
    let uplift = g.uplift.bilinear(gx, gz).max(0.0) as f64;
    let p = g.precipitation.bilinear(gx, gz) as f64;
    let t = g.temperature.bilinear(gx, gz) as f64;
    let n = g.n();
    let (ci, cj) = (
        (gx.round() as i64).rem_euclid(n as i64) as usize,
        (gz.round().max(0.0) as usize).min(n - 1),
    );
    let prov = g.province[g.geom.idx(ci, cj)];
    // Mountains where the rock is uplifted.
    let mtn = smoothstep(500.0, 3500.0, uplift).powf(0.8);
    // Rain cuts: humid land through to its streams, the wettest to its base level; dry land
    // keeps broad interfluves.
    let humid = smoothstep(300.0, 1600.0, p);
    let drenched = smoothstep(1400.0, 2200.0, p);
    let dry = 1.0 - smoothstep(150.0, 500.0, p);
    let dissect = lerp(0.09, 0.20, humid) + 0.35 * drenched;
    // The height above base level counts less as it grows: a plateau's interior stays whole.
    let cut = dissect * hb / (1.0 + hb / 1500.0);
    // High land is being cut into as it rises: relief grows with height above some hundreds of
    // metres too, however gently the land falls to the sea (up to 2 km; above, the mountains').
    let upland = 0.35 * (e - 480.0).clamp(0.0, 1550.0);
    let mut amp = 3.0 + cut + upland;
    // The hillslopes' gradient by the climate: steeper as rain cuts; gentler on dry land, whose
    // basins fill with what the ranges shed, and where ice scoured and drift buried the land
    // (cold lowland; where it is drenched, rivers and ice cut it steep); steeper as the relief
    // grows.
    let mut grad = lerp(0.12, 0.35, humid) + 0.75 * drenched;
    grad *= lerp(1.0, 0.4, smoothstep(11.0, 2.0, t) * (1.0 - drenched));
    grad *= amp / (amp + 20.0);
    grad *= lerp(1.0, 0.5, dry);
    match prov {
        // An old mountain belt: ridges of hard rock between cut valleys.
        province::OLD_OROGEN => {
            amp *= 1.25;
            grad *= 1.3;
        }
        // Rifts and their basins: ranges standing over filled basins.
        province::RIFT => amp *= 1.5,
        _ => {}
    }
    // Mountains: relief throughout; their hillslopes at the threshold of sliding where the range
    // is steepest, and where ice carved it.
    let mountain = 0.18 * hb + 20.0;
    let ice = smoothstep(2.0, -6.0, t);
    let threshold = lerp(0.25, 0.6, smoothstep(800.0, 3500.0, range)) * lerp(1.0, 1.2, ice);
    amp = lerp(amp, mountain.max(amp), mtn);
    grad = lerp(grad, threshold, mtn);
    // The valleys as far apart as hillslopes of that gradient take to rise through the relief.
    let spacing = (4.0 * amp / grad.max(1e-3)).clamp(150.0, 30_000.0);
    Roughness {
        amp,
        spacing,
        fill: dry,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WorldGenSettings;
    use hearth_math::PlanetSize;

    fn earth() -> PlanetGrid {
        let s = WorldGenSettings {
            seed: 3,
            planet_size: PlanetSize::Earth,
            grid_resolution: 256,
        };
        PlanetGrid::build(&s.sanitized(), &|_, _| {})
    }

    fn median(mut v: Vec<f64>) -> f64 {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    }

    /// Every land cell of a grid: its rain, its height above base level and its relief.
    fn land_cells(g: &PlanetGrid, land: &Land) -> Vec<(f64, f64, f64, Roughness)> {
        let n = g.n();
        (0..n * n)
            .filter(|&k| g.flags[k] & flags::OCEAN == 0 && g.elevation.data[k] > 0.0)
            .map(|k| {
                let (i, j) = ((k % n) as f64, (k / n) as f64);
                (
                    g.precipitation.bilinear(i, j) as f64,
                    land.above.data[k] as f64,
                    g.uplift.bilinear(i, j) as f64,
                    roughness(g, land, i, j),
                )
            })
            .collect()
    }

    #[test]
    fn relief_grows_with_height_above_base_level() {
        let g = earth();
        let land = Land::new(&g);
        let cells = land_cells(&g, &land);
        let amp = |lo: f64, hi: f64| {
            median(
                cells
                    .iter()
                    .filter(|c| (lo..hi).contains(&c.1))
                    .map(|c| c.3.amp)
                    .collect(),
            )
        };
        let (low, high) = (amp(0.0, 50.0), amp(600.0, f64::MAX));
        assert!(
            high > 4.0 * low,
            "{low} m near base level, {high} m far above it"
        );
    }

    #[test]
    fn rain_steepens_hillslopes_and_dry_land_fills_its_basins() {
        let g = earth();
        let land = Land::new(&g);
        let cells = land_cells(&g, &land);
        // The hillslopes' gradient, out of the mountains, in land of some relief.
        let grad = |lo: f64, hi: f64| {
            median(
                cells
                    .iter()
                    .filter(|c| (lo..hi).contains(&c.0) && c.2 < 300.0 && c.3.amp > 20.0)
                    .map(|c| 4.0 * c.3.amp / c.3.spacing)
                    .collect(),
            )
        };
        let (dry, wet) = (grad(300.0, 600.0), grad(2200.0, f64::MAX));
        assert!(
            wet > 2.0 * dry,
            "{dry} on dry grassland, {wet} on drenched hills"
        );
        for (rain, _, _, r) in &cells {
            if *rain < 150.0 {
                assert!(r.fill > 0.99, "{rain} mm: {r:?}");
            } else if *rain > 500.0 {
                assert_eq!(r.fill, 0.0, "{rain} mm: {r:?}");
            }
        }
    }

    #[test]
    fn relief_falls_away_below_the_valleys_spacing_and_grows_slowly_beyond() {
        let r = Roughness {
            amp: 50.0,
            spacing: 1000.0,
            fill: 0.0,
        };
        assert!((r.at(1000.0) - 50.0).abs() < 1e-9);
        // Halving the wavelength below: some 60 % left; doubling beyond: a quarter more.
        let below = r.at(250.0) / r.at(500.0);
        let beyond = r.at(16000.0) / r.at(8000.0);
        assert!((0.58..0.64).contains(&below), "{below}");
        assert!((1.2..1.28).contains(&beyond), "{beyond}");
    }
}

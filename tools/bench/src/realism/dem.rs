//! Height grids from real elevation models and from the generator, and pictures of them.

use std::path::Path;

use hearth_worldgen::Terrain;
use rayon::prelude::*;

use super::metrics::Dem;
use crate::image::Image;

/// An SRTM-style tile (`N43W124.hgt`: 3601 × 3601 big-endian heights, one arc-second apart,
/// row 0 at the north edge; −32768 where unknown).
pub struct Hgt {
    side: usize,
    /// The tile's south-west corner (degrees).
    lat: f64,
    lon: f64,
    h: Vec<i16>,
}

impl Hgt {
    pub fn load(path: &Path) -> anyhow::Result<Hgt> {
        let bytes = std::fs::read(path)?;
        let side = ((bytes.len() / 2) as f64).sqrt() as usize;
        anyhow::ensure!(
            side * side * 2 == bytes.len(),
            "{}: not a square tile",
            path.display()
        );
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("tile name"))?;
        let (ns, rest) = name.split_at(1);
        let lat: f64 = rest[..2].parse()?;
        let (ew, lon) = rest[2..].split_at(1);
        let lon: f64 = lon.parse()?;
        let lat = if ns == "S" { -lat } else { lat };
        let lon = if ew == "W" { -lon } else { lon };
        let h = bytes
            .chunks_exact(2)
            .map(|b| i16::from_be_bytes([b[0], b[1]]))
            .collect();
        Ok(Hgt { side, lat, lon, h })
    }

    /// The height (m) at a latitude and longitude inside the tile, bilinear; NaN where unknown
    /// or at the sea's level and below (the sea and the voids).
    pub fn at(&self, lat: f64, lon: f64) -> f32 {
        let s = (self.side - 1) as f64;
        let x = (lon - self.lon) * s;
        let y = (self.lat + 1.0 - lat) * s;
        if x < 0.0 || y < 0.0 || x >= s || y >= s {
            return f32::NAN;
        }
        let (i, j) = (x as usize, y as usize);
        let (fx, fy) = ((x - i as f64) as f32, (y - j as f64) as f32);
        let g = |i: usize, j: usize| {
            let v = self.h[j * self.side + i];
            if v == i16::MIN || v <= 0 {
                f32::NAN
            } else {
                v as f32
            }
        };
        let top = g(i, j) * (1.0 - fx) + g(i + 1, j) * fx;
        let bottom = g(i, j + 1) * (1.0 - fx) + g(i + 1, j + 1) * fx;
        top * (1.0 - fy) + bottom * fy
    }

    /// `n` × `n` heights `dx` metres apart about a point, north up.
    pub fn window(&self, lat: f64, lon: f64, n: usize, dx: f64) -> Dem {
        let m_per_deg = 111_320.0;
        let h = (0..n * n)
            .into_par_iter()
            .map(|k| {
                let (i, j) = (k % n, k / n);
                let east = (i as f64 - n as f64 * 0.5) * dx;
                let north = (n as f64 * 0.5 - j as f64) * dx;
                self.at(
                    lat + north / m_per_deg,
                    lon + east / (m_per_deg * lat.to_radians().cos()),
                )
            })
            .collect();
        Dem { n, dx, h }
    }
}

/// A one-metre lidar model (USGS 3DEP's GeoTIFFs: 32-bit heights, row 0 at the north edge,
/// −999999 where unknown).
pub struct Lidar {
    pub w: usize,
    pub h: usize,
    pub z: Vec<f32>,
}

impl Lidar {
    pub fn load(path: &Path) -> anyhow::Result<Lidar> {
        use tiff::decoder::{Decoder, DecodingResult, Limits};
        let file = std::io::BufReader::new(std::fs::File::open(path)?);
        let mut dec = Decoder::new(file)?.with_limits(Limits::unlimited());
        let (w, h) = dec.dimensions()?;
        let z = match dec.read_image()? {
            DecodingResult::F32(v) => v,
            DecodingResult::F64(v) => v.into_iter().map(|x| x as f32).collect(),
            _ => anyhow::bail!("{}: not floating-point heights", path.display()),
        };
        let z = z
            .into_iter()
            .map(|v| if v < -1000.0 { f32::NAN } else { v })
            .collect();
        Ok(Lidar {
            w: w as usize,
            h: h as usize,
            z,
        })
    }

    /// The `n` × `n` window with its top-left corner at (x, y).
    pub fn window(&self, x: usize, y: usize, n: usize) -> Dem {
        let mut h = Vec::with_capacity(n * n);
        for j in 0..n {
            h.extend_from_slice(&self.z[(y + j) * self.w + x..(y + j) * self.w + x + n]);
        }
        Dem { n, dx: 1.0, h }
    }
}

/// The generator's surface, `n` × `n` columns `step` blocks apart about (x, z), in blocks (the
/// metres the player walks in, whatever the planet's vertical scale): the water's surface where
/// water stands over the ground (as lidar sees a lake), unknown at sea.
pub fn generated(terrain: &Terrain, x: i32, z: i32, n: usize, step: i32) -> Dem {
    let surface = |s: hearth_worldgen::ColumnSample| {
        if s.ocean && s.water > s.height {
            f32::NAN
        } else {
            s.height.max(s.water)
        }
    };
    let at = |i: usize, j: usize| {
        (
            x + (i as i32 - n as i32 / 2) * step,
            z + (j as i32 - n as i32 / 2) * step,
        )
    };
    let mut h = vec![f32::NAN; n * n];
    if step < 8 {
        // Columns close together share the sampler's own neighbourhoods.
        h.par_iter_mut().enumerate().for_each(|(k, v)| {
            let (cx, cz) = at(k % n, k / n);
            *v = surface(terrain.sample(cx, cz));
        });
    } else {
        // Columns far apart: the neighbourhood read once for each block of 16 × 16 of them.
        const B: usize = 16;
        let blocks: Vec<(usize, usize)> = (0..n.div_ceil(B))
            .flat_map(|bj| (0..n.div_ceil(B)).map(move |bi| (bi, bj)))
            .collect();
        let done: Vec<(usize, Vec<f32>)> = blocks
            .par_iter()
            .map(|&(bi, bj)| {
                let (i0, j0) = (bi * B, bj * B);
                let (i1, j1) = ((i0 + B).min(n) - 1, (j0 + B).min(n) - 1);
                let (xa, za) = at(i0, j0);
                let (xb, zb) = at(i1, j1);
                let near = terrain.nearby(xa, za, xb, zb);
                let mut v = Vec::with_capacity(B * B);
                for j in j0..=j1 {
                    for i in i0..=i1 {
                        let (cx, cz) = at(i, j);
                        v.push(surface(terrain.sample_with(cx, cz, &near)));
                    }
                }
                (bj * n.div_ceil(B) + bi, v)
            })
            .collect();
        for (b, v) in done {
            let (bi, bj) = (b % n.div_ceil(B), b / n.div_ceil(B));
            let (i0, j0) = (bi * B, bj * B);
            let w = (i0 + B).min(n) - i0;
            for (k, val) in v.into_iter().enumerate() {
                h[(j0 + k / w) * n + i0 + k % w] = val;
            }
        }
    }
    Dem {
        n,
        dx: step as f64,
        h,
    }
}

/// A picture of a grid lit from the north-west (azimuth 315°, 45° up), the same for every
/// grid so that real and generated ground can be laid side by side; unknown ground blue.
pub fn hillshade(d: &Dem) -> Image {
    let n = d.n;
    let mut img = Image::new(n, n);
    // Toward the light: north-west, 45° up (east, north, up).
    let c = 45f64.to_radians().cos() * std::f64::consts::FRAC_1_SQRT_2;
    let light = [-c, c, 45f64.to_radians().sin()];
    for j in 0..n {
        for i in 0..n {
            let c = d.get(i, j);
            let at = |a: usize, b: usize| {
                let v = d.get(a.min(n - 1), b.min(n - 1));
                if v.is_finite() { v } else { c }
            };
            if !c.is_finite() {
                img.set(i, j, [70, 100, 150]);
                continue;
            }
            let gx = (at(i + 1, j) - at(i.saturating_sub(1), j)) as f64 / (2.0 * d.dx);
            // Rows run north to south: the gradient northward is minus the rows'.
            let gy = -(at(i, j + 1) - at(i, j.saturating_sub(1))) as f64 / (2.0 * d.dx);
            let l = (-gx * light[0] - gy * light[1] + light[2]) / (gx * gx + gy * gy + 1.0).sqrt();
            let g = (l.clamp(0.0, 1.0) * 235.0 + 15.0) as u8;
            img.set(i, j, [g, g, g]);
        }
    }
    img
}

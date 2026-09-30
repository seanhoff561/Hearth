//! Geometry of the planet analysis grid: an N×N grid over the Mercator world square (X wraps,
//! rows run from the north pole edge at z = −C/2 to the south pole edge at z = +C/2), with
//! sphere-correct metrics.

use glam::DVec3;
use hearth_math::Planet;

/// Geometry and per-row/per-column trigonometry of the grid.
#[derive(Debug, Clone)]
pub struct GridGeom {
    pub n: usize,
    /// Circumference in blocks.
    pub c: f64,
    /// Blocks per cell (C / N).
    pub cell: f64,
    /// Latitude per row (radians, north positive).
    pub lat: Vec<f64>,
    pub sin_lat: Vec<f64>,
    pub cos_lat: Vec<f64>,
    pub sin_lon: Vec<f64>,
    pub cos_lon: Vec<f64>,
    planet: Planet,
}

impl GridGeom {
    pub fn new(planet: Planet, n: usize) -> Self {
        let c = planet.circumference() as f64;
        let cell = c / n as f64;
        let mut lat = Vec::with_capacity(n);
        for j in 0..n {
            let z = -c * 0.5 + (j as f64 + 0.5) * cell;
            lat.push(planet.latitude(z));
        }
        let sin_lat = lat.iter().map(|l| l.sin()).collect();
        let cos_lat = lat.iter().map(|l| l.cos()).collect();
        let mut sin_lon = Vec::with_capacity(n);
        let mut cos_lon = Vec::with_capacity(n);
        for i in 0..n {
            let lon = (i as f64 + 0.5) / n as f64 * std::f64::consts::TAU;
            sin_lon.push(lon.sin());
            cos_lon.push(lon.cos());
        }
        Self {
            n,
            c,
            cell,
            lat,
            sin_lat,
            cos_lat,
            sin_lon,
            cos_lon,
            planet,
        }
    }

    pub fn planet(&self) -> &Planet {
        &self.planet
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.n * self.n
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    #[inline]
    pub fn idx(&self, i: usize, j: usize) -> usize {
        j * self.n + i
    }

    #[inline]
    pub fn ij(&self, idx: usize) -> (usize, usize) {
        (idx % self.n, idx / self.n)
    }

    #[inline]
    pub fn wrap_i(&self, i: isize) -> usize {
        i.rem_euclid(self.n as isize) as usize
    }

    /// Neighbour index with X wrap; `None` past the pole rows.
    #[inline]
    pub fn neighbor(&self, i: usize, j: usize, di: isize, dj: isize) -> Option<usize> {
        let jj = j as isize + dj;
        if jj < 0 || jj >= self.n as isize {
            return None;
        }
        Some(self.idx(self.wrap_i(i as isize + di), jj as usize))
    }

    /// World (x, z) of a cell centre.
    #[inline]
    pub fn world_xz(&self, i: usize, j: usize) -> (f64, f64) {
        (
            (i as f64 + 0.5) * self.cell,
            -self.c * 0.5 + (j as f64 + 0.5) * self.cell,
        )
    }

    /// Unit sphere point of a cell centre.
    #[inline]
    pub fn sphere(&self, i: usize, j: usize) -> DVec3 {
        let cl = self.cos_lat[j];
        DVec3::new(cl * self.cos_lon[i], self.sin_lat[j], cl * self.sin_lon[i])
    }

    /// Physical size of a cell in row `j`, in equatorial blocks (cell × cos φ).
    #[inline]
    pub fn phys_cell(&self, j: usize) -> f64 {
        self.cell * self.cos_lat[j]
    }

    /// Continuous grid coordinates (cell units, cell centres at integers) of a world position.
    #[inline]
    pub fn grid_coords(&self, x: f64, z: f64) -> (f64, f64) {
        (x / self.cell - 0.5, (z + self.c * 0.5) / self.cell - 0.5)
    }

    /// Radius of the planet sphere in blocks.
    #[inline]
    pub fn radius(&self) -> f64 {
        self.planet.radius()
    }

    /// The eight neighbour offsets.
    pub const OFFSETS8: [(isize, isize); 8] = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];
}

/// A scalar field on the grid. Smooth fields may be stored at a coarser resolution
/// (`scale` grid cells per field cell); sampling takes full-grid coordinates either way.
#[derive(Debug, Clone)]
pub struct Field<T> {
    /// Field resolution (n × n).
    pub n: usize,
    pub data: Vec<T>,
    /// Grid cells per field cell (1 = full resolution).
    pub scale: usize,
}

impl<T: Copy> Field<T> {
    pub fn new(n: usize, fill: T) -> Self {
        Self {
            n,
            data: vec![fill; n * n],
            scale: 1,
        }
    }

    /// Wraps full-resolution data.
    pub fn from_vec(n: usize, data: Vec<T>) -> Self {
        debug_assert_eq!(data.len(), n * n);
        Self { n, data, scale: 1 }
    }

    /// Value at full-grid cell (i, j).
    #[inline]
    pub fn cell(&self, i: usize, j: usize) -> T {
        self.data[(j / self.scale) * self.n + i / self.scale]
    }

    /// Value at a full-grid cell index given the full grid size.
    #[inline]
    pub fn at_index(&self, idx: usize, full_n: usize) -> T {
        self.cell(idx % full_n, idx / full_n)
    }

    #[inline]
    pub fn get(&self, i: usize, j: usize) -> T {
        self.data[j * self.n + i]
    }

    #[inline]
    pub fn set(&mut self, i: usize, j: usize, v: T) {
        self.data[j * self.n + i] = v;
    }
}

impl Field<f32> {
    /// Halves the resolution by averaging 2×2 blocks (for smooth fields).
    pub fn downsample2(&self) -> Field<f32> {
        let m = self.n / 2;
        let mut data = vec![0f32; m * m];
        for j in 0..m {
            for i in 0..m {
                let a = self.get(2 * i, 2 * j);
                let b = self.get(2 * i + 1, 2 * j);
                let c = self.get(2 * i, 2 * j + 1);
                let d = self.get(2 * i + 1, 2 * j + 1);
                data[j * m + i] = (a + b + c + d) * 0.25;
            }
        }
        Field {
            n: m,
            data,
            scale: self.scale * 2,
        }
    }

    #[inline]
    fn local(&self, g: f64) -> f64 {
        if self.scale == 1 {
            g
        } else {
            (g + 0.5) / self.scale as f64 - 0.5
        }
    }

    /// Bilinear sample at continuous (full-)grid coordinates (X wraps, Z clamps).
    #[inline]
    pub fn bilinear(&self, gx: f64, gz: f64) -> f32 {
        let (gx, gz) = (self.local(gx), self.local(gz));
        let n = self.n as isize;
        let x0 = gx.floor();
        let z0 = gz.floor();
        let fx = (gx - x0) as f32;
        let fz = (gz - z0) as f32;
        let i0 = (x0 as isize).rem_euclid(n) as usize;
        let i1 = (x0 as isize + 1).rem_euclid(n) as usize;
        let j0 = (z0 as isize).clamp(0, n - 1) as usize;
        let j1 = (z0 as isize + 1).clamp(0, n - 1) as usize;
        let a = self.get(i0, j0) + (self.get(i1, j0) - self.get(i0, j0)) * fx;
        let b = self.get(i0, j1) + (self.get(i1, j1) - self.get(i0, j1)) * fx;
        a + (b - a) * fz
    }

    /// Catmull-Rom bicubic sample (X wraps, Z clamps). Smooth first derivatives, so terrain
    /// built on it has no grid creases.
    #[inline]
    pub fn bicubic(&self, gx: f64, gz: f64) -> f32 {
        let (gx, gz) = (self.local(gx), self.local(gz));
        let n = self.n as isize;
        let x0 = gx.floor();
        let z0 = gz.floor();
        let tx = gx - x0;
        let tz = gz - z0;
        let xi = x0 as isize;
        let zi = z0 as isize;
        let wx = catmull_weights(tx);
        let wz = catmull_weights(tz);
        let mut sum = 0.0f64;
        for (dz, wzv) in wz.iter().enumerate() {
            let j = (zi + dz as isize - 1).clamp(0, n - 1) as usize;
            let row = &self.data[j * self.n..(j + 1) * self.n];
            let mut rs = 0.0f64;
            for (dx, wxv) in wx.iter().enumerate() {
                let i = (xi + dx as isize - 1).rem_euclid(n) as usize;
                rs += row[i] as f64 * wxv;
            }
            sum += rs * wzv;
        }
        sum as f32
    }
}

#[inline]
fn catmull_weights(t: f64) -> [f64; 4] {
    let t2 = t * t;
    let t3 = t2 * t;
    [
        -0.5 * t3 + t2 - 0.5 * t,
        1.5 * t3 - 2.5 * t2 + 1.0,
        -1.5 * t3 + 2.0 * t2 + 0.5 * t,
        0.5 * t3 - 0.5 * t2,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use hearth_math::PlanetSize;

    #[test]
    fn geometry_round_trips() {
        let p = Planet::from_size(PlanetSize::Tiny).unwrap();
        let g = GridGeom::new(p, 64);
        let (x, z) = g.world_xz(10, 20);
        let (gx, gz) = g.grid_coords(x, z);
        assert!((gx - 10.0).abs() < 1e-9 && (gz - 20.0).abs() < 1e-9);
        let s = g.sphere(10, 20);
        let s2 = p.sphere_point(x, z);
        assert!((s - s2).length() < 1e-9);
        // Row 0 is the northern edge (positive latitude).
        assert!(g.lat[0] > 1.4 && g.lat[63] < -1.4);
        assert_eq!(g.neighbor(0, 5, -1, 0), Some(g.idx(63, 5)));
        assert_eq!(g.neighbor(0, 0, 0, -1), None);
    }

    #[test]
    fn bicubic_reproduces_linear_and_wraps() {
        let mut f = Field::new(16, 0.0f32);
        for j in 0..16 {
            for i in 0..16 {
                f.set(i, j, j as f32 * 2.0);
            }
        }
        assert!((f.bicubic(3.3, 7.25) - 14.5).abs() < 1e-4);
        assert!((f.bilinear(3.3, 7.25) - 14.5).abs() < 1e-4);
        let mut g = Field::new(16, 0.0f32);
        g.set(0, 5, 1.0);
        assert!((g.bicubic(16.0, 5.0) - 1.0).abs() < 1e-6, "x wraps");
        assert!((g.bicubic(-16.0, 5.0) - 1.0).abs() < 1e-6);
    }
}

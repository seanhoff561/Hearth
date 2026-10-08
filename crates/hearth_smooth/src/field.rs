//! Fill samples on the voxel grid: one signed 8-bit value per voxel, the distance from the voxel's
//! centre to the surface, positive inside the ground (S §2.1).

use glam::{IVec3, Vec3};

/// How far from the surface a fill value reaches, in voxels: a sample's fill is its centre's
/// signed distance to the surface clamped to ±`FILL_RANGE`.
pub const FILL_RANGE: f32 = 1.5;

/// Quantization steps each way across [`FILL_RANGE`]: a step is 1.5 m / 127 ≈ 1.2 cm.
const STEPS: f32 = 127.0;

/// The fill value of a signed distance (voxels, positive inside), clamped to the range. Values
/// run −127..=127; −128 is never written (it reads back as −127).
#[inline]
pub fn quantize(distance: f32) -> i8 {
    if distance.is_nan() {
        return -127;
    }
    (distance * (STEPS / FILL_RANGE))
        .round()
        .clamp(-STEPS, STEPS) as i8
}

/// The signed distance (voxels) a fill value stands for.
#[inline]
pub fn dequantize(fill: i8) -> f32 {
    f32::from(fill.max(-127)) * (FILL_RANGE / STEPS)
}

/// Whether a fill value is inside the ground. A sample exactly on the surface (fill 0) is
/// outside, so the solid voxels are exactly those of positive fill.
#[inline]
pub const fn solid(fill: i8) -> bool {
    fill > 0
}

/// A box of samples on the grid: the fill of each voxel and its material. Sample `(x, y, z)` is the
/// voxel at `origin + (x, y, z)`; positions inside the field are in sample units with each
/// sample at its integer coordinates (a voxel's centre).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    origin: IVec3,
    size: [usize; 3],
    fill: Vec<i8>,
    material: Vec<u16>,
}

impl Field {
    /// A field of `size` samples, all empty (fill −127, material 0).
    pub fn new(origin: IVec3, size: [usize; 3]) -> Self {
        let n = size[0] * size[1] * size[2];
        Self {
            origin,
            size,
            fill: vec![-127; n],
            material: vec![0; n],
        }
    }

    /// A field sampled from a function of each voxel's grid position giving its signed distance
    /// (voxels, positive inside) and material.
    pub fn from_fn(
        origin: IVec3,
        size: [usize; 3],
        mut f: impl FnMut(IVec3) -> (f32, u16),
    ) -> Self {
        let mut field = Self::new(origin, size);
        for y in 0..size[1] {
            for z in 0..size[2] {
                for x in 0..size[0] {
                    let (d, m) = f(origin + IVec3::new(x as i32, y as i32, z as i32));
                    let i = field.index(x, y, z);
                    field.fill[i] = quantize(d);
                    field.material[i] = m;
                }
            }
        }
        field
    }

    /// A field from its sample arrays in [`Field::index`] order, or `None` if their lengths
    /// don't match the size.
    pub fn from_parts(
        origin: IVec3,
        size: [usize; 3],
        fill: Vec<i8>,
        material: Vec<u16>,
    ) -> Option<Self> {
        let n = size[0] * size[1] * size[2];
        (fill.len() == n && material.len() == n).then_some(Self {
            origin,
            size,
            fill,
            material,
        })
    }

    /// The grid position of sample (0, 0, 0).
    pub fn origin(&self) -> IVec3 {
        self.origin
    }

    /// Samples along x, y and z.
    pub fn size(&self) -> [usize; 3] {
        self.size
    }

    /// Where sample (x, y, z) is in the arrays: x fastest, then z, then y (as the cubes'
    /// `local_index`).
    #[inline]
    pub fn index(&self, x: usize, y: usize, z: usize) -> usize {
        (y * self.size[2] + z) * self.size[0] + x
    }

    #[inline]
    pub fn fill(&self, x: usize, y: usize, z: usize) -> i8 {
        self.fill[self.index(x, y, z)]
    }

    #[inline]
    pub fn material(&self, x: usize, y: usize, z: usize) -> u16 {
        self.material[self.index(x, y, z)]
    }

    /// The signed distance (voxels) at a sample.
    #[inline]
    pub fn distance(&self, x: usize, y: usize, z: usize) -> f32 {
        dequantize(self.fill(x, y, z))
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, fill: i8, material: u16) {
        let i = self.index(x, y, z);
        self.fill[i] = fill;
        self.material[i] = material;
    }

    /// The fill values in [`Field::index`] order.
    pub fn fills(&self) -> &[i8] {
        &self.fill
    }

    /// The materials in [`Field::index`] order.
    pub fn materials(&self) -> &[u16] {
        &self.material
    }

    /// A copy of the `size` samples from `lo` (a block and its apron, to mesh on its own).
    pub fn sub(&self, lo: [usize; 3], size: [usize; 3]) -> Field {
        let mut out = Field::new(
            self.origin + IVec3::new(lo[0] as i32, lo[1] as i32, lo[2] as i32),
            size,
        );
        for y in 0..size[1] {
            for z in 0..size[2] {
                let from = self.index(lo[0], lo[1] + y, lo[2] + z);
                let to = out.index(0, y, z);
                out.fill[to..to + size[0]].copy_from_slice(&self.fill[from..from + size[0]]);
                out.material[to..to + size[0]]
                    .copy_from_slice(&self.material[from..from + size[0]]);
            }
        }
        out
    }

    /// The distance's gradient at a sample by central differences (one-sided on the field's
    /// faces). It points into the ground; the surface's outward normal is its opposite.
    pub fn gradient(&self, x: usize, y: usize, z: usize) -> Vec3 {
        let p = [x, y, z];
        let mut g = [0.0f32; 3];
        for (axis, g) in g.iter_mut().enumerate() {
            let (mut lo, mut hi) = (p, p);
            let mut span = 0.0;
            if p[axis] > 0 {
                lo[axis] -= 1;
                span += 1.0;
            }
            if p[axis] + 1 < self.size[axis] {
                hi[axis] += 1;
                span += 1.0;
            }
            if span > 0.0 {
                *g = (self.distance(hi[0], hi[1], hi[2]) - self.distance(lo[0], lo[1], lo[2]))
                    / span;
            }
        }
        Vec3::from(g)
    }

    /// The gradient at a sample with creases kept. Central differences blur a crease (a cliff's
    /// edge, a ridge's crest) over the samples beside it, mixing its two faces' slopes; here, on
    /// each axis where the forward and backward differences disagree (a crease or the fill's
    /// clamp lies between the sample and a neighbour) the one-sided differences are combined so
    /// the gradient's length comes nearest one, as a distance field's is: the sample's own
    /// face wins over the mix across the crease. Elsewhere it is the central difference.
    pub fn gradient_sharp(&self, x: usize, y: usize, z: usize) -> Vec3 {
        /// Disagreement between the one-sided differences that marks a crease: well above the
        /// quantization's noise (two steps), well below a right-angled crease's (about 0.5).
        const CREASE: f32 = 0.1;
        let p = [x, y, z];
        let d0 = self.distance(x, y, z);
        let mut options = [[0.0f32; 2]; 3];
        let mut count = [1usize; 3];
        for axis in 0..3 {
            let forward = (p[axis] + 1 < self.size[axis]).then(|| {
                let mut q = p;
                q[axis] += 1;
                self.distance(q[0], q[1], q[2]) - d0
            });
            let backward = (p[axis] > 0).then(|| {
                let mut q = p;
                q[axis] -= 1;
                d0 - self.distance(q[0], q[1], q[2])
            });
            match (forward, backward) {
                (Some(f), Some(b)) if (f - b).abs() <= CREASE => options[axis][0] = (f + b) * 0.5,
                (Some(f), Some(b)) => {
                    options[axis] = [f, b];
                    count[axis] = 2;
                }
                (Some(g), None) | (None, Some(g)) => options[axis][0] = g,
                (None, None) => {}
            }
        }
        let mut best = Vec3::ZERO;
        let mut best_score = f32::INFINITY;
        for i in 0..count[0] {
            for j in 0..count[1] {
                for k in 0..count[2] {
                    let g = Vec3::new(options[0][i], options[1][j], options[2][k]);
                    let score = (g.length_squared() - 1.0).abs();
                    if score < best_score {
                        best = g;
                        best_score = score;
                    }
                }
            }
        }
        best
    }

    /// The distance at `t` (each component 0..=1) inside the cell whose lowest sample is
    /// `cell`, by trilinear interpolation.
    pub fn trilinear(&self, cell: [usize; 3], t: Vec3) -> f32 {
        let [x, y, z] = cell;
        let d = |dx, dy, dz| self.distance(x + dx, y + dy, z + dz);
        let c00 = d(0, 0, 0) * (1.0 - t.x) + d(1, 0, 0) * t.x;
        let c10 = d(0, 1, 0) * (1.0 - t.x) + d(1, 1, 0) * t.x;
        let c01 = d(0, 0, 1) * (1.0 - t.x) + d(1, 0, 1) * t.x;
        let c11 = d(0, 1, 1) * (1.0 - t.x) + d(1, 1, 1) * t.x;
        let c0 = c00 * (1.0 - t.y) + c10 * t.y;
        let c1 = c01 * (1.0 - t.y) + c11 * t.y;
        c0 * (1.0 - t.z) + c1 * t.z
    }

    /// The distance at a point in sample units, by trilinear interpolation (clamped to the
    /// field).
    pub fn sample(&self, p: Vec3) -> f32 {
        let (cell, t) = self.locate(p);
        self.trilinear(cell, t)
    }

    /// The gradient at a point in sample units: the trilinear blend of its cell's corner
    /// gradients, so it varies smoothly from cell to cell.
    pub fn gradient_at(&self, p: Vec3) -> Vec3 {
        let (cell, t) = self.locate(p);
        let [x, y, z] = cell;
        let g = |dx, dy, dz| self.gradient(x + dx, y + dy, z + dz);
        let c00 = g(0, 0, 0).lerp(g(1, 0, 0), t.x);
        let c10 = g(0, 1, 0).lerp(g(1, 1, 0), t.x);
        let c01 = g(0, 0, 1).lerp(g(1, 0, 1), t.x);
        let c11 = g(0, 1, 1).lerp(g(1, 1, 1), t.x);
        c00.lerp(c10, t.y).lerp(c01.lerp(c11, t.y), t.z)
    }

    /// The cell holding a point (clamped to the field) and the point's place in it.
    fn locate(&self, p: Vec3) -> ([usize; 3], Vec3) {
        let mut cell = [0usize; 3];
        let mut t = [0.0f32; 3];
        for a in 0..3 {
            let top = self.size[a].saturating_sub(1) as f32;
            let v = p[a].clamp(0.0, top);
            let c = (v.floor() as usize).min(self.size[a].saturating_sub(2));
            cell[a] = c;
            t[a] = v - c as f32;
        }
        (cell, Vec3::from(t))
    }

    /// Samples whose fill isn't saturated: those within [`FILL_RANGE`] of the surface, the
    /// ones a surface cube has to store.
    pub fn unsaturated(&self) -> usize {
        self.fill.iter().filter(|&&q| q > -127 && q < 127).count()
    }
}

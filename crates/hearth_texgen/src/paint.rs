//! A tiny pixel-art painting toolkit: RGBA images, palettes, tileable noise and shapes.

use hearth_math::hash::{hash_2d, hash2, mix64};

pub type Rgb = [u8; 3];
pub type Rgba = [u8; 4];

/// The alpha of a texel of fruit or flowers in a plant's sprite: one step short of full, the
/// same to the eye and to the alpha test, read exactly where the aim and its highlight look for
/// them (Amendment T §2.3).
pub const PART_ALPHA: u8 = 254;

/// An RGBA8 image.
#[derive(Debug, Clone, PartialEq)]
pub struct Tex {
    pub w: u32,
    pub h: u32,
    pub px: Vec<Rgba>,
}

impl Tex {
    /// Fully transparent image.
    pub fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            px: vec![[0, 0, 0, 0]; (w * h) as usize],
        }
    }

    pub fn filled(w: u32, h: u32, c: Rgb) -> Self {
        Self {
            w,
            h,
            px: vec![[c[0], c[1], c[2], 255]; (w * h) as usize],
        }
    }

    #[inline]
    fn idx(&self, x: i32, y: i32) -> usize {
        let x = x.rem_euclid(self.w as i32) as u32;
        let y = y.rem_euclid(self.h as i32) as u32;
        (y * self.w + x) as usize
    }

    /// Pixel with wrapping coordinates (textures tile).
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Rgba {
        self.px[self.idx(x, y)]
    }

    #[inline]
    pub fn set(&mut self, x: i32, y: i32, c: Rgb) {
        let i = self.idx(x, y);
        self.px[i] = [c[0], c[1], c[2], 255];
    }

    #[inline]
    pub fn set_rgba(&mut self, x: i32, y: i32, c: Rgba) {
        let i = self.idx(x, y);
        self.px[i] = c;
    }

    /// Sets a texel of a plant's fruit or flowers: opaque as any, told from the leaves by its
    /// alpha (`PART_ALPHA`), so the eyes can rest on the berries themselves (Amendment T §2.3).
    #[inline]
    pub fn set_part(&mut self, x: i32, y: i32, c: Rgb) {
        self.set_rgba(x, y, [c[0], c[1], c[2], PART_ALPHA]);
    }

    /// Sets a pixel only if inside the image (no wrapping), for sprites.
    #[inline]
    pub fn put(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && (x as u32) < self.w && (y as u32) < self.h {
            self.set(x, y, c);
        }
    }

    /// `set_part` only if inside the image (no wrapping), for sprites.
    #[inline]
    pub fn put_part(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && (x as u32) < self.w && (y as u32) < self.h {
            self.set_part(x, y, c);
        }
    }

    #[inline]
    pub fn clear(&mut self, x: i32, y: i32) {
        let i = self.idx(x, y);
        self.px[i] = [0, 0, 0, 0];
    }

    /// Alpha-blends `c` over the pixel.
    pub fn blend(&mut self, x: i32, y: i32, c: Rgb, a: f32) {
        let i = self.idx(x, y);
        let p = self.px[i];
        let a = a.clamp(0.0, 1.0);
        let mix = |d: u8, s: u8| (d as f32 * (1.0 - a) + s as f32 * a).round() as u8;
        let alpha = if p[3] == 0 {
            (a * 255.0) as u8
        } else {
            p[3].max((a * 255.0) as u8)
        };
        self.px[i] = if p[3] == 0 {
            [c[0], c[1], c[2], alpha]
        } else {
            [mix(p[0], c[0]), mix(p[1], c[1]), mix(p[2], c[2]), alpha]
        };
    }

    /// Multiplies RGB by `f` (keeping alpha).
    pub fn shade(&mut self, x: i32, y: i32, f: f32) {
        let i = self.idx(x, y);
        let p = self.px[i];
        let m = |v: u8| (v as f32 * f).clamp(0.0, 255.0) as u8;
        self.px[i] = [m(p[0]), m(p[1]), m(p[2]), p[3]];
    }

    /// Draws `other` over this image (alpha compositing), with an offset.
    pub fn overlay(&mut self, other: &Tex, ox: i32, oy: i32) {
        for y in 0..other.h as i32 {
            for x in 0..other.w as i32 {
                let p = other.get(x, y);
                if p[3] > 0 {
                    self.blend(x + ox, y + oy, [p[0], p[1], p[2]], p[3] as f32 / 255.0);
                }
            }
        }
    }

    /// Converts to grayscale luminance (for tinted textures), preserving alpha.
    pub fn to_gray(&mut self, lo: f32, hi: f32) {
        for p in &mut self.px {
            let l = (0.3 * p[0] as f32 + 0.59 * p[1] as f32 + 0.11 * p[2] as f32) / 255.0;
            let v = ((lo + (hi - lo) * l) * 255.0).clamp(0.0, 255.0) as u8;
            *p = [v, v, v, p[3]];
        }
    }

    /// Writes the image as PNG.
    pub fn save_png(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let file = std::fs::File::create(path)?;
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), self.w, self.h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().map_err(std::io::Error::other)?;
        let data: Vec<u8> = self.px.iter().flatten().copied().collect();
        w.write_image_data(&data).map_err(std::io::Error::other)?;
        Ok(())
    }

    /// Vertically stacks frames into an animation strip.
    pub fn strip(frames: &[Tex]) -> Tex {
        let w = frames[0].w;
        let h: u32 = frames.iter().map(|f| f.h).sum();
        let mut out = Tex::new(w, h);
        let mut y0 = 0;
        for f in frames {
            for y in 0..f.h {
                for x in 0..w {
                    out.px[((y0 + y) * w + x) as usize] = f.px[(y * w + x) as usize];
                }
            }
            y0 += f.h;
        }
        out
    }
}

/// A shade ramp from dark to light.
#[derive(Debug, Clone)]
pub struct Palette(pub Vec<Rgb>);

impl Palette {
    /// Picks a colour for t in 0..1.
    #[inline]
    pub fn pick(&self, t: f32) -> Rgb {
        let n = self.0.len();
        let i = ((t.clamp(0.0, 0.9999)) * n as f32) as usize;
        self.0[i.min(n - 1)]
    }

    /// Builds a ramp around a base colour: darker and lighter shades.
    pub fn around(base: Rgb, steps: usize, spread: f32) -> Palette {
        let mut v = Vec::with_capacity(steps);
        for i in 0..steps {
            let t = if steps == 1 {
                0.5
            } else {
                i as f32 / (steps - 1) as f32
            };
            let f = 1.0 - spread + 2.0 * spread * t;
            v.push(scale(base, f));
        }
        Palette(v)
    }
}

pub fn scale(c: Rgb, f: f32) -> Rgb {
    [
        (c[0] as f32 * f).clamp(0.0, 255.0) as u8,
        (c[1] as f32 * f).clamp(0.0, 255.0) as u8,
        (c[2] as f32 * f).clamp(0.0, 255.0) as u8,
    ]
}

pub fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
    ]
}

/// Deterministic per-pixel random in 0..1.
#[inline]
pub fn rand01(seed: u64, x: i32, y: i32) -> f32 {
    (hash_2d(seed, x, y) >> 40) as f32 / (1u64 << 24) as f32
}

/// Tileable value noise with lattice spacing `cell` on a `size`-periodic torus. 0..1.
pub fn value_noise(seed: u64, x: f32, y: f32, cell: f32, size: f32) -> f32 {
    let period = (size / cell).round().max(1.0) as i32;
    let gx = x / cell;
    let gy = y / cell;
    let x0 = gx.floor();
    let y0 = gy.floor();
    let fx = gx - x0;
    let fy = gy - y0;
    let (xi, yi) = (x0 as i32, y0 as i32);
    let v = |i: i32, j: i32| rand01(seed, i.rem_euclid(period), j.rem_euclid(period));
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (s(fx), s(fy));
    let a = v(xi, yi) + (v(xi + 1, yi) - v(xi, yi)) * sx;
    let b = v(xi, yi + 1) + (v(xi + 1, yi + 1) - v(xi, yi + 1)) * sx;
    a + (b - a) * sy
}

/// Tileable fractal noise (cells 8, 4, 2) mixed with per-pixel grain. 0..1.
pub fn fbm(seed: u64, x: i32, y: i32, size: u32, grain: f32) -> f32 {
    let (fx, fy, s) = (x as f32 + 0.5, y as f32 + 0.5, size as f32);
    let n = value_noise(seed, fx, fy, 8.0, s) * 0.5
        + value_noise(seed ^ 0x51, fx, fy, 4.0, s) * 0.3
        + value_noise(seed ^ 0xa3, fx, fy, 2.0, s) * 0.2;
    n * (1.0 - grain) + rand01(seed ^ 0x77, x, y) * grain
}

/// Tileable Voronoi: returns (distance to nearest, distance to second nearest, cell id) for a
/// point in a `size`-periodic domain with roughly `count` sites.
pub fn voronoi(seed: u64, x: f32, y: f32, size: f32, count: u32) -> (f32, f32, u64) {
    let mut best = (f32::MAX, f32::MAX, 0u64);
    for i in 0..count {
        let h = hash2(seed, i as u64);
        let sx = (h & 0xffff) as f32 / 65535.0 * size;
        let sy = ((h >> 16) & 0xffff) as f32 / 65535.0 * size;
        for oy in [-size, 0.0, size] {
            for ox in [-size, 0.0, size] {
                let dx = x - (sx + ox);
                let dy = y - (sy + oy);
                let d = (dx * dx + dy * dy).sqrt();
                if d < best.0 {
                    best = (d, best.0, mix64(h));
                } else if d < best.1 {
                    best.1 = d;
                }
            }
        }
    }
    best
}

/// Fills an image by a function of the pixel.
pub fn paint(w: u32, h: u32, mut f: impl FnMut(i32, i32) -> Rgb) -> Tex {
    let mut t = Tex::new(w, h);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            t.set(x, y, f(x, y));
        }
    }
    t
}

/// Draws a line with Bresenham's algorithm (no wrapping).
pub fn line(t: &mut Tex, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
    let (mut x, mut y) = (x0, y0);
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        t.put(x, y, c);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

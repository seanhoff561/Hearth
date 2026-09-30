//! Minimal RGB image buffer with PNG output.

use std::path::Path;

pub struct Image {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[u8; 3]>,
}

impl Image {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            px: vec![[0, 0, 0]; w * h],
        }
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, c: [u8; 3]) {
        if x < self.w && y < self.h {
            self.px[y * self.w + x] = c;
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let file = std::fs::File::create(path)?;
        let mut enc =
            png::Encoder::new(std::io::BufWriter::new(file), self.w as u32, self.h as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header()?;
        let mut data = Vec::with_capacity(self.px.len() * 3);
        for p in &self.px {
            data.extend_from_slice(p);
        }
        writer.write_image_data(&data)?;
        Ok(())
    }
}

pub fn lerp_color(a: [u8; 3], b: [u8; 3], t: f64) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f64 + (b[0] as f64 - a[0] as f64) * t) as u8,
        (a[1] as f64 + (b[1] as f64 - a[1] as f64) * t) as u8,
        (a[2] as f64 + (b[2] as f64 - a[2] as f64) * t) as u8,
    ]
}

/// Piecewise-linear colour ramp.
pub fn ramp(stops: &[(f64, [u8; 3])], v: f64) -> [u8; 3] {
    if v <= stops[0].0 {
        return stops[0].1;
    }
    for w in stops.windows(2) {
        if v <= w[1].0 {
            return lerp_color(w[0].1, w[1].1, (v - w[0].0) / (w[1].0 - w[0].0));
        }
    }
    stops[stops.len() - 1].1
}

pub fn shade(c: [u8; 3], f: f64) -> [u8; 3] {
    [
        (c[0] as f64 * f).clamp(0.0, 255.0) as u8,
        (c[1] as f64 * f).clamp(0.0, 255.0) as u8,
        (c[2] as f64 * f).clamp(0.0, 255.0) as u8,
    ]
}

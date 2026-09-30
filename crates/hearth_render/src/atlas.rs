//! The block texture array: every texture (and every animation frame) is one layer of a 2D
//! texture array. Names map to layers and animation info.

use rustc_hash::FxHashMap;

/// Animation info for a texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TexInfo {
    /// First layer in the array.
    pub layer: u16,
    /// Number of frames (1 = still).
    pub frames: u8,
    /// Ticks per frame.
    pub frame_time: u8,
}

/// CPU-side texture array contents.
#[derive(Debug, Clone)]
pub struct TextureArray {
    /// Edge length of every layer in pixels.
    pub size: u32,
    /// RGBA8 pixels of every layer, layer-major.
    pub layers: Vec<Vec<[u8; 4]>>,
    names: FxHashMap<String, TexInfo>,
    /// Average colour of each named texture (for LOD colours, particles, maps).
    pub average: FxHashMap<String, [u8; 4]>,
}

impl TextureArray {
    /// Builds the array from the generated pack. Layer 0 is always the missing texture.
    pub fn from_entries(entries: &[hearth_texgen::TexEntry]) -> Self {
        let size = 16;
        let mut arr = Self {
            size,
            layers: Vec::new(),
            names: FxHashMap::default(),
            average: FxHashMap::default(),
        };
        // Missing texture first so layer 0 is always safe.
        if let Some(m) = entries.iter().find(|e| e.name == "block/missing") {
            arr.push(m);
        }
        for e in entries {
            if e.name != "block/missing" {
                arr.push(e);
            }
        }
        arr
    }

    fn push(&mut self, e: &hearth_texgen::TexEntry) {
        let first = self.layers.len() as u16;
        let fh = e.tex.h / e.frames.max(1);
        for f in 0..e.frames.max(1) {
            let mut layer = Vec::with_capacity((self.size * self.size) as usize);
            for y in 0..self.size {
                for x in 0..self.size {
                    // Nearest resample if a pack texture is a different size.
                    let sx = x * e.tex.w / self.size;
                    let sy = f * fh + y * fh / self.size;
                    layer.push(e.tex.px[(sy * e.tex.w + sx) as usize]);
                }
            }
            self.layers.push(layer);
        }
        // Average colour of the first frame (alpha-weighted).
        let (mut r, mut g, mut b, mut a) = (0u64, 0u64, 0u64, 0u64);
        for p in &self.layers[first as usize] {
            let w = p[3] as u64;
            r += p[0] as u64 * w;
            g += p[1] as u64 * w;
            b += p[2] as u64 * w;
            a += w;
        }
        let n = (self.size * self.size) as u64;
        let avg = match (r.checked_div(a), g.checked_div(a), b.checked_div(a)) {
            (Some(r), Some(g), Some(b)) => [r as u8, g as u8, b as u8, (a / n) as u8],
            _ => [0, 0, 0, 0],
        };
        self.average.insert(e.name.clone(), avg);
        self.names.insert(
            e.name.clone(),
            TexInfo {
                layer: first,
                frames: e.frames.max(1) as u8,
                frame_time: e.frame_time.clamp(1, 255) as u8,
            },
        );
    }

    /// Looks up a texture by name (`block/stone`); unknown names give the missing texture.
    pub fn get(&self, name: &str) -> TexInfo {
        self.names.get(name).copied().unwrap_or(TexInfo {
            layer: 0,
            frames: 1,
            frame_time: 1,
        })
    }

    pub fn contains(&self, name: &str) -> bool {
        self.names.contains_key(name)
    }

    pub fn layer_count(&self) -> u32 {
        self.layers.len() as u32
    }

    /// Full mip chain of one layer. Cutout textures use alpha-weighted averaging and keep
    /// coverage (so leaves don't vanish in the distance).
    pub fn mips(&self, layer: usize, levels: u32) -> Vec<Vec<[u8; 4]>> {
        let mut out = vec![self.layers[layer].clone()];
        let mut size = self.size;
        let base_coverage = coverage(&self.layers[layer], 0.5);
        for _ in 1..levels {
            let prev = out.last().expect("level 0 exists");
            let half = (size / 2).max(1);
            let mut next = Vec::with_capacity((half * half) as usize);
            for y in 0..half {
                for x in 0..half {
                    let mut acc = [0f32; 4];
                    let mut wsum = 0f32;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let p = prev[((y * 2 + dy) * size + x * 2 + dx) as usize];
                        let w = p[3] as f32 / 255.0;
                        acc[0] += srgb_to_linear(p[0]) * w;
                        acc[1] += srgb_to_linear(p[1]) * w;
                        acc[2] += srgb_to_linear(p[2]) * w;
                        acc[3] += p[3] as f32;
                        wsum += w;
                    }
                    let px = if wsum > 0.0 {
                        [
                            linear_to_srgb(acc[0] / wsum),
                            linear_to_srgb(acc[1] / wsum),
                            linear_to_srgb(acc[2] / wsum),
                            (acc[3] / 4.0) as u8,
                        ]
                    } else {
                        [0, 0, 0, 0]
                    };
                    next.push(px);
                }
            }
            // Preserve alpha-test coverage: scale alpha so the same fraction passes 0.5.
            if base_coverage > 0.0 && base_coverage < 1.0 {
                preserve_coverage(&mut next, base_coverage);
            }
            out.push(next);
            size = half;
        }
        out
    }
}

fn coverage(px: &[[u8; 4]], threshold: f32) -> f32 {
    px.iter()
        .filter(|p| p[3] as f32 / 255.0 >= threshold)
        .count() as f32
        / px.len() as f32
}

/// Scales alpha so that the fraction of texels passing the 0.5 alpha test matches `target`.
fn preserve_coverage(px: &mut [[u8; 4]], target: f32) {
    let (mut lo, mut hi) = (0.0f32, 4.0f32);
    for _ in 0..12 {
        let mid = (lo + hi) * 0.5;
        let c = px
            .iter()
            .filter(|p| (p[3] as f32 / 255.0 * mid) >= 0.5)
            .count() as f32
            / px.len() as f32;
        if c < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let s = (lo + hi) * 0.5;
    for p in px.iter_mut() {
        p[3] = (p[3] as f32 * s).clamp(0.0, 255.0) as u8;
    }
}

fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_names_and_mips() {
        let arr = TextureArray::from_entries(&hearth_texgen::default_textures());
        assert_eq!(arr.get("block/missing").layer, 0);
        let water = arr.get("block/water_still");
        assert_eq!(water.frames, 16);
        let stone = arr.get("block/stone");
        assert_ne!(stone.layer, 0);
        assert_eq!(arr.get("block/definitely_not_there").layer, 0);
        let mips = arr.mips(arr.get("block/oak_leaves").layer as usize, 5);
        assert_eq!(mips.len(), 5);
        assert_eq!(mips[4].len(), 1);
        // Coverage is roughly preserved at level 2.
        let c0 = coverage(&mips[0], 0.5);
        let c2 = coverage(&mips[2], 0.5);
        assert!((c0 - c2).abs() < 0.2, "{c0} vs {c2}");
    }
}

//! Procedurally generated default resource pack.
//!
//! All textures are original 16×16 pixel art produced from code, so the game ships without any
//! third-party assets. The generator is deterministic: every build produces identical images.

pub mod blocks;
pub mod paint;

pub use paint::Tex;

/// A texture in the pack. Animated textures are vertical strips of `frames` frames.
#[derive(Debug, Clone)]
pub struct TexEntry {
    /// Path without namespace or extension, e.g. `block/stone`.
    pub name: String,
    pub tex: Tex,
    pub frames: u32,
    /// Ticks per frame for animated textures.
    pub frame_time: u32,
}

impl TexEntry {
    pub fn still(name: &str, tex: Tex) -> Self {
        Self {
            name: name.to_owned(),
            tex,
            frames: 1,
            frame_time: 1,
        }
    }
}

/// The whole default texture pack.
pub fn default_textures() -> Vec<TexEntry> {
    blocks::textures()
}

/// Writes the pack to `<dir>/assets/hearth/textures/<name>.png` (plus `.mcmeta` animation
/// files for animated textures) in the standard resource-pack layout.
pub fn write_pack(dir: &std::path::Path) -> std::io::Result<usize> {
    let tex = default_textures();
    for t in &tex {
        let path = dir
            .join("assets/hearth/textures")
            .join(format!("{}.png", t.name));
        t.tex.save_png(&path)?;
        if t.frames > 1 {
            let meta = format!(
                "{{\n  \"animation\": {{ \"frametime\": {} }}\n}}\n",
                t.frame_time
            );
            std::fs::write(path.with_extension("png.mcmeta"), meta)?;
        }
    }
    Ok(tex.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_is_deterministic_and_well_formed() {
        let a = default_textures();
        let b = default_textures();
        assert_eq!(a.len(), b.len());
        let mut names = std::collections::BTreeSet::new();
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x.name, y.name);
            assert_eq!(x.tex, y.tex, "{}", x.name);
            assert_eq!(x.tex.w, 16);
            assert_eq!(x.tex.h, 16 * x.frames, "{}", x.name);
            assert!(names.insert(x.name.clone()), "duplicate {}", x.name);
        }
        assert!(a.len() > 80, "{} textures", a.len());
    }
}

//! The pixel font as an atlas and its metrics: every glyph of [`crate::glyphs`] laid into one
//! single-channel texture, with a solid texel for filled rectangles.

use rustc_hash::FxHashMap;

use crate::glyphs::GLYPHS;

/// Side of the atlas (texels).
pub const ATLAS: u32 = 256;
/// Rows of a glyph cell: seven above the baseline, two below.
pub const CELL: u32 = 9;
/// Distance between lines (interface pixels).
pub const LINE: u32 = 11;
/// Advance of a space.
const SPACE: u32 = 3;

/// Where a glyph is in the atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    /// Its width and one pixel of space after it.
    pub advance: u32,
}

/// The font: its atlas and glyphs.
#[derive(Debug, Clone)]
pub struct Font {
    /// `ATLAS`² coverage bytes (255 ink); texel (0, 0) is ink, for filled rectangles.
    pub pixels: Vec<u8>,
    glyphs: FxHashMap<char, Glyph>,
}

impl Default for Font {
    fn default() -> Self {
        Self::new()
    }
}

impl Font {
    pub fn new() -> Self {
        let mut pixels = vec![0u8; (ATLAS * ATLAS) as usize];
        pixels[0] = 255;
        let mut glyphs = FxHashMap::default();
        // Glyphs left to right in rows, a texel apart, after the solid texel.
        let (mut x, mut y) = (2u32, 0u32);
        for &(c, rows) in GLYPHS {
            let width = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as u32;
            if x + width + 1 > ATLAS {
                x = 0;
                y += CELL + 1;
            }
            assert!(y + CELL <= ATLAS, "the font atlas is full");
            for (row, line) in rows.iter().enumerate() {
                for (col, ch) in line.chars().enumerate() {
                    if ch == '#' {
                        let i = (y + row as u32) * ATLAS + x + col as u32;
                        pixels[i as usize] = 255;
                    }
                }
            }
            glyphs.insert(
                c,
                Glyph {
                    x,
                    y,
                    width,
                    advance: width + 1,
                },
            );
            x += width + 1;
        }
        Self { pixels, glyphs }
    }

    /// A character's glyph; `None` for spaces and characters the font lacks (drawn as `?`).
    pub fn glyph(&self, c: char) -> Option<&Glyph> {
        self.glyphs.get(&c).or_else(|| {
            if c.is_whitespace() {
                None
            } else {
                self.glyphs.get(&'?')
            }
        })
    }

    /// How far a character moves the pen (interface pixels).
    pub fn advance(&self, c: char) -> u32 {
        if c == ' ' {
            SPACE
        } else if c == '\t' {
            SPACE * 4
        } else {
            self.glyph(c).map_or(SPACE, |g| g.advance)
        }
    }

    /// Width of a line of text (interface pixels), without the space after the last glyph.
    pub fn width(&self, text: &str) -> u32 {
        let w: u32 = text.chars().map(|c| self.advance(c)).sum();
        w.saturating_sub(1)
    }

    /// Breaks text into lines no wider than `max` (at spaces where possible; explicit newlines
    /// kept).
    pub fn wrap(&self, text: &str, max: u32) -> Vec<String> {
        let mut out = Vec::new();
        for para in text.split('\n') {
            let mut line = String::new();
            for word in para.split(' ') {
                let candidate = if line.is_empty() {
                    word.to_owned()
                } else {
                    format!("{line} {word}")
                };
                if self.width(&candidate) <= max || line.is_empty() {
                    line = candidate;
                } else {
                    out.push(std::mem::take(&mut line));
                    line = word.to_owned();
                }
            }
            out.push(line);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_fits_and_is_distinct() {
        let f = Font::new();
        assert!(f.glyph('A').is_some() && f.glyph('°').is_some());
        assert_eq!(
            f.glyph('\u{2603}'),
            f.glyph('?'),
            "missing characters show as ?"
        );
        // No two glyphs share their ink.
        let mut seen = std::collections::HashSet::new();
        for &(c, rows) in GLYPHS {
            assert!(rows.len() <= CELL as usize, "{c} too tall");
            let w = rows[0].chars().count();
            assert!(
                rows.iter().all(|r| r.chars().count() == w),
                "{c}: ragged rows"
            );
            assert!(seen.insert(rows.join("|")), "{c} duplicates another glyph");
        }
        assert_eq!(f.width("Hi"), 5 + 1 + 1);
    }

    #[test]
    fn wrapping_keeps_lines_within_width() {
        let f = Font::new();
        let lines = f.wrap("the quick brown fox jumps over the lazy dog", 60);
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| f.width(l) <= 60));
        assert_eq!(
            lines.join(" "),
            "the quick brown fox jumps over the lazy dog"
        );
    }
}

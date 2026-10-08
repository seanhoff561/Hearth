//! The interface's typefaces (Amendment Q §6): Source Sans 3 for the interface and Source Serif
//! 4 for the journal (both SIL Open Font License 1.1, in `data/hearth/fonts/`). Their glyphs are
//! kept as signed distance fields in one single-channel atlas, so text is crisp at any size and
//! interface scale: the shader turns distance into coverage. Text is laid out in interface
//! pixels: a line's box is [`CELL`] tall with the baseline [`BASELINE`] below its top, and lines
//! are [`LINE`] apart.

use ab_glyph::{Font as _, FontRef, GlyphId, PxScale, point};
use rustc_hash::FxHashMap;

const SANS: &[u8] = include_bytes!("../../../data/hearth/fonts/SourceSans3-Regular.ttf");
const SERIF: &[u8] = include_bytes!("../../../data/hearth/fonts/SourceSerif4-Regular.ttf");

/// Side of the atlas (texels).
pub const ATLAS: u32 = 1024;
/// Height of a line's box (interface pixels).
pub const CELL: u32 = 9;
/// Distance between lines (interface pixels).
pub const LINE: u32 = 11;
/// From the top of a line's box to its baseline (interface pixels).
pub const BASELINE: f32 = 7.0;
/// How tall capitals stand (interface pixels).
const CAP: f32 = 6.5;
/// Atlas texels per interface pixel: glyphs are drawn this much finer than they are laid out.
const RASTER: f32 = 4.0;
/// How far the distance field reaches each side of an outline (texels).
const SPREAD: f32 = 5.0;
/// The solid block for filled rectangles (texels from the atlas's corner).
const SOLID: u32 = 4;

/// Which typeface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    /// The interface's.
    Sans,
    /// The journal's.
    Serif,
}

/// The characters the interface draws: ASCII, Latin-1 and the punctuation, arrows and signs the
/// words of the interface use. Others are drawn as `?`.
fn charset() -> impl Iterator<Item = char> {
    let ranges: [(u32, u32); 8] = [
        (0x21, 0x7e),
        (0xa1, 0xff),
        (0x2013, 0x2014),
        (0x2018, 0x201d),
        (0x2022, 0x2022),
        (0x2026, 0x2026),
        (0x2190, 0x2193),
        (0x2212, 0x2212),
    ];
    ranges
        .into_iter()
        .flat_map(|(a, b)| a..=b)
        .filter_map(char::from_u32)
        .chain(['≈', '≤', '≥'])
}

/// Where a glyph is in the atlas and how it sits on the line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glyph {
    /// Its rectangle in the atlas (texels).
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Its quad from the pen at the top of the line's box (interface pixels).
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    /// How far it moves the pen (interface pixels).
    pub advance: f32,
}

/// The typefaces as an atlas and their metrics.
#[derive(Debug, Clone)]
pub struct Font {
    /// `ATLAS`² distance bytes (128 on an outline, more inside); a solid block at the corner
    /// for filled rectangles.
    pub pixels: Vec<u8>,
    glyphs: FxHashMap<(Face, char), Glyph>,
    space: [f32; 2],
}

impl Default for Font {
    fn default() -> Self {
        Self::new()
    }
}

impl Font {
    pub fn new() -> Self {
        let mut pixels = vec![0u8; (ATLAS * ATLAS) as usize];
        for y in 0..SOLID {
            for x in 0..SOLID {
                pixels[(y * ATLAS + x) as usize] = 255;
            }
        }
        let mut glyphs = FxHashMap::default();
        let mut shelf = Shelf::new();
        let mut space = [0.0; 2];
        for (face, bytes) in [(Face::Sans, SANS), (Face::Serif, SERIF)] {
            let font = FontRef::try_from_slice(bytes).expect("a built-in typeface");
            // Interface pixels per font unit, from the capitals' height (an outline's bounds
            // run top to bottom).
            let cap_units = font
                .outline(font.glyph_id('H'))
                .map_or(660.0, |o| (o.bounds.max.y - o.bounds.min.y).abs());
            let k = CAP / cap_units;
            let scale = PxScale::from(k * RASTER * font.height_unscaled());
            space[face as usize] = font.h_advance_unscaled(font.glyph_id(' ')) * k;
            for c in charset() {
                let id = font.glyph_id(c);
                if id == GlyphId(0) {
                    continue;
                }
                let advance = font.h_advance_unscaled(id) * k;
                let Some(outlined) =
                    font.outline_glyph(id.with_scale_and_position(scale, point(0.0, 0.0)))
                else {
                    continue;
                };
                let b = outlined.px_bounds();
                let (gw, gh) = (b.width() as usize, b.height() as usize);
                let mut cover = vec![0f32; gw * gh];
                outlined.draw(|x, y, v| {
                    if let Some(c) = cover.get_mut(y as usize * gw + x as usize) {
                        *c = v.min(1.0);
                    }
                });
                let pad = SPREAD.ceil() as usize;
                let (w, h) = (gw + 2 * pad, gh + 2 * pad);
                let field = distance_field(&cover, gw, gh, pad);
                let (ax, ay) = shelf.place(w as u32, h as u32);
                for row in 0..h {
                    let at = (ay as usize + row) * ATLAS as usize + ax as usize;
                    pixels[at..at + w].copy_from_slice(&field[row * w..row * w + w]);
                }
                glyphs.insert(
                    (face, c),
                    Glyph {
                        x: ax,
                        y: ay,
                        w: w as u32,
                        h: h as u32,
                        left: (b.min.x - pad as f32) / RASTER,
                        top: BASELINE + (b.min.y - pad as f32) / RASTER,
                        width: w as f32 / RASTER,
                        height: h as f32 / RASTER,
                        advance,
                    },
                );
            }
        }
        Self {
            pixels,
            glyphs,
            space,
        }
    }

    /// A character's glyph in the interface's typeface; `None` for spaces (characters the
    /// typeface lacks are drawn as `?`).
    pub fn glyph(&self, c: char) -> Option<&Glyph> {
        self.glyph_in(Face::Sans, c)
    }

    /// A character's glyph in a typeface.
    pub fn glyph_in(&self, face: Face, c: char) -> Option<&Glyph> {
        if c.is_whitespace() {
            return None;
        }
        self.glyphs
            .get(&(face, c))
            .or_else(|| self.glyphs.get(&(face, '?')))
    }

    /// How far a character moves the pen (interface pixels).
    pub fn advance_in(&self, face: Face, c: char) -> f32 {
        let space = self.space[face as usize];
        match c {
            ' ' => space,
            '\t' => space * 4.0,
            _ => self.glyph_in(face, c).map_or(space, |g| g.advance),
        }
    }

    /// Width of a line of text in the interface's typeface, rounded up to whole interface
    /// pixels.
    pub fn width(&self, text: &str) -> u32 {
        self.width_in(Face::Sans, text).ceil() as u32
    }

    /// Width of a line of text in a typeface (interface pixels).
    pub fn width_in(&self, face: Face, text: &str) -> f32 {
        text.chars().map(|c| self.advance_in(face, c)).sum()
    }

    /// Breaks text into lines no wider than `max` (at spaces where possible; explicit newlines
    /// kept).
    pub fn wrap(&self, text: &str, max: u32) -> Vec<String> {
        self.wrap_in(Face::Sans, text, max as f32)
    }

    /// As [`Font::wrap`], in a typeface.
    pub fn wrap_in(&self, face: Face, text: &str, max: f32) -> Vec<String> {
        let mut out = Vec::new();
        for para in text.split('\n') {
            let mut line = String::new();
            for word in para.split(' ') {
                let candidate = if line.is_empty() {
                    word.to_owned()
                } else {
                    format!("{line} {word}")
                };
                if self.width_in(face, &candidate).ceil() <= max || line.is_empty() {
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

/// Rows of glyphs laid left to right, a new row when one is full.
struct Shelf {
    x: u32,
    y: u32,
    row: u32,
}

impl Shelf {
    fn new() -> Self {
        Self {
            x: SOLID + 2,
            y: 0,
            row: SOLID,
        }
    }

    fn place(&mut self, w: u32, h: u32) -> (u32, u32) {
        if self.x + w > ATLAS {
            self.x = 0;
            self.y += self.row + 1;
            self.row = 0;
        }
        assert!(self.y + h <= ATLAS, "the interface atlas is full");
        let at = (self.x, self.y);
        self.x += w + 1;
        self.row = self.row.max(h);
        at
    }
}

/// A glyph's coverage (`gw`×`gh`, 0–1) as a signed distance field padded by `pad` texels: 128
/// on the outline, rising inside and falling to 0 [`SPREAD`] texels outside.
fn distance_field(cover: &[f32], gw: usize, gh: usize, pad: usize) -> Vec<u8> {
    let (w, h) = (gw + 2 * pad, gh + 2 * pad);
    let ink = |x: usize, y: usize| {
        (pad..pad + gw).contains(&x)
            && (pad..pad + gh).contains(&y)
            && cover[(y - pad) * gw + x - pad] >= 0.5
    };
    const FAR: f32 = 1e20;
    let mut to_ink = vec![FAR; w * h];
    let mut to_paper = vec![FAR; w * h];
    for y in 0..h {
        for x in 0..w {
            if ink(x, y) {
                to_ink[y * w + x] = 0.0;
            } else {
                to_paper[y * w + x] = 0.0;
            }
        }
    }
    edt(&mut to_ink, w, h);
    edt(&mut to_paper, w, h);
    to_ink
        .iter()
        .zip(&to_paper)
        .map(|(&a, &b)| {
            // Positive outside; the outline lies half a texel from the ink's centres.
            let d = if a > 0.0 {
                a.sqrt() - 0.5
            } else {
                0.5 - b.sqrt()
            };
            ((0.5 - d / (2.0 * SPREAD)).clamp(0.0, 1.0) * 255.0).round() as u8
        })
        .collect()
}

/// Squared Euclidean distance transform in place (Felzenszwalb and Huttenlocher): each cell's
/// squared distance to the nearest cell that was 0.
fn edt(grid: &mut [f32], w: usize, h: usize) {
    let n = w.max(h);
    let mut f = vec![0f32; n];
    let mut d = vec![0f32; n];
    let mut v = vec![0usize; n];
    let mut z = vec![0f32; n + 1];
    for x in 0..w {
        for y in 0..h {
            f[y] = grid[y * w + x];
        }
        edt_1d(&f[..h], &mut d[..h], &mut v, &mut z);
        for y in 0..h {
            grid[y * w + x] = d[y];
        }
    }
    for y in 0..h {
        f[..w].copy_from_slice(&grid[y * w..y * w + w]);
        edt_1d(&f[..w], &mut d[..w], &mut v, &mut z);
        grid[y * w..y * w + w].copy_from_slice(&d[..w]);
    }
}

/// The 1-D squared distance transform of `f` (the lower envelope of parabolas).
fn edt_1d(f: &[f32], d: &mut [f32], v: &mut [usize], z: &mut [f32]) {
    let n = f.len();
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    for q in 1..n {
        let mut s;
        loop {
            let p = v[k];
            s = ((f[q] + (q * q) as f32) - (f[p] + (p * p) as f32)) / (2.0 * (q - p) as f32);
            if s <= z[k] && k > 0 {
                k -= 1;
            } else {
                break;
            }
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f32::INFINITY;
    }
    k = 0;
    for (q, out) in d.iter_mut().enumerate() {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let p = v[k];
        *out = (q as f32 - p as f32).powi(2) + f[p];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_interface_s_characters_are_in_both_typefaces() {
        let f = Font::new();
        let lang = include_str!("../../../data/hearth/lang/en_us.json");
        for c in lang
            .chars()
            .filter(|c| !c.is_whitespace() && !c.is_control())
        {
            for face in [Face::Sans, Face::Serif] {
                assert!(
                    f.glyphs.contains_key(&(face, c)),
                    "{c:?} ({:x}) is not in {face:?}",
                    c as u32
                );
            }
        }
        assert_eq!(
            f.glyph('\u{2603}'),
            f.glyph('?'),
            "missing characters show as ?"
        );
        // Capitals stand on the baseline, as tall as they should.
        let h = f.glyph('H').unwrap();
        let pad = SPREAD.ceil() / RASTER;
        let cap = (h.height - 2.0 * pad, BASELINE - (h.top + pad));
        assert!(
            (cap.0 - CAP).abs() < 0.3 && (cap.1 - CAP).abs() < 0.3,
            "{cap:?}"
        );
        assert!((5..=9).contains(&f.width("Hi")), "{}", f.width("Hi"));
    }

    #[test]
    fn the_field_is_half_on_the_outline_full_inside_and_empty_far_out() {
        let cover = vec![1.0f32; 16];
        let f = distance_field(&cover, 4, 4, 6);
        let at = |x: usize, y: usize| f[y * 16 + x] as i32;
        assert!(at(7, 7) > 150, "inside {}", at(7, 7));
        assert_eq!(at(0, 0), 0, "far outside");
        // The outline runs between texel 5 (paper) and 6 (ink).
        assert!((at(5, 7) - 128).abs() < 30 && (at(6, 7) - 128).abs() < 30);
        assert!(at(5, 7) < 128 && at(6, 7) > 128);
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

//! What the interface draws in a frame: filled rectangles and text as textured triangles in
//! interface pixels, scaled to screen pixels by the interface scale.

use bytemuck::{Pod, Zeroable};

use crate::font::{ATLAS, CELL, Font};

/// A colour (sRGB, straight alpha).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
#[repr(C)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub const WHITE: Rgba = Rgba([255, 255, 255, 255]);
    pub const BLACK: Rgba = Rgba([0, 0, 0, 255]);
    /// The shadow under text.
    pub const SHADOW: Rgba = Rgba([20, 20, 24, 200]);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Rgba([r, g, b, 255])
    }

    pub const fn with_alpha(self, a: u8) -> Self {
        Rgba([self.0[0], self.0[1], self.0[2], a])
    }
}

/// A vertex of the interface: screen pixels, atlas coordinates (0–1), colour.
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct UiVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: Rgba,
}

/// The interface's triangles for a frame.
#[derive(Debug, Clone, Default)]
pub struct DrawList {
    pub vertices: Vec<UiVertex>,
    /// Screen pixels per interface pixel.
    pub scale: f32,
    /// Where drawing shows (interface pixels: left, top, right, bottom); what falls outside is
    /// cut away (a scrolled area's rows past its edges).
    clip: Option<[f32; 4]>,
}

impl DrawList {
    pub fn new(scale: u32) -> Self {
        Self {
            vertices: Vec::new(),
            scale: scale.max(1) as f32,
            clip: None,
        }
    }

    pub fn clear(&mut self, scale: u32) {
        self.vertices.clear();
        self.scale = scale.max(1) as f32;
        self.clip = None;
    }

    /// Sets where drawing shows (interface pixels: left, top, right, bottom; `None` the whole
    /// frame), returning what it was.
    pub fn set_clip(&mut self, clip: Option<[f32; 4]>) -> Option<[f32; 4]> {
        std::mem::replace(&mut self.clip, clip)
    }

    /// Where drawing shows now.
    pub fn clip(&self) -> Option<[f32; 4]> {
        self.clip
    }

    fn quad(&mut self, x: f32, y: f32, w: f32, h: f32, uv: [f32; 4], color: Rgba) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (x, y, x + w, y + h);
        let mut uv = uv;
        if let Some([cl, ct, cr, cb]) = self.clip {
            if cr <= cl || cb <= ct || x1 <= cl || x0 >= cr || y1 <= ct || y0 >= cb {
                return;
            }
            // Cut to the clip, the texture staying where it was.
            let (du, dv) = ((uv[2] - uv[0]) / w, (uv[3] - uv[1]) / h);
            if x0 < cl {
                uv[0] += (cl - x0) * du;
                x0 = cl;
            }
            if x1 > cr {
                uv[2] -= (x1 - cr) * du;
                x1 = cr;
            }
            if y0 < ct {
                uv[1] += (ct - y0) * dv;
                y0 = ct;
            }
            if y1 > cb {
                uv[3] -= (y1 - cb) * dv;
                y1 = cb;
            }
        }
        let s = self.scale;
        let (x0, y0, x1, y1) = (x0 * s, y0 * s, x1 * s, y1 * s);
        let v = |px: f32, py: f32, u: f32, t: f32| UiVertex {
            pos: [px, py],
            uv: [u, t],
            color,
        };
        self.vertices.extend_from_slice(&[
            v(x0, y0, uv[0], uv[1]),
            v(x1, y0, uv[2], uv[1]),
            v(x1, y1, uv[2], uv[3]),
            v(x0, y0, uv[0], uv[1]),
            v(x1, y1, uv[2], uv[3]),
            v(x0, y1, uv[0], uv[3]),
        ]);
    }

    /// A filled rectangle (interface pixels).
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) {
        let t = 0.5 / ATLAS as f32;
        self.quad(x, y, w, h, [t, t, t, t], color);
    }

    /// A line of text with its top-left at (x, y) (interface pixels); returns its width.
    pub fn text(&mut self, font: &Font, text: &str, x: f32, y: f32, color: Rgba) -> f32 {
        let a = ATLAS as f32;
        let mut pen = x;
        for c in text.chars() {
            if let Some(g) = font.glyph(c) {
                let uv = [
                    g.x as f32 / a,
                    g.y as f32 / a,
                    (g.x + g.width) as f32 / a,
                    (g.y + CELL) as f32 / a,
                ];
                self.quad(pen, y, g.width as f32, CELL as f32, uv, color);
            }
            pen += font.advance(c) as f32;
        }
        (pen - x - 1.0).max(0.0)
    }

    /// Text `size` times as large (whole numbers keep the pixels crisp), with a shadow.
    pub fn text_sized(&mut self, font: &Font, text: &str, x: f32, y: f32, size: f32, color: Rgba) {
        let a = ATLAS as f32;
        for (dx, c) in [(size, Rgba::SHADOW), (0.0, color)] {
            let mut pen = x;
            for ch in text.chars() {
                if let Some(g) = font.glyph(ch) {
                    let uv = [
                        g.x as f32 / a,
                        g.y as f32 / a,
                        (g.x + g.width) as f32 / a,
                        (g.y + CELL) as f32 / a,
                    ];
                    self.quad(
                        pen + dx,
                        y + dx,
                        g.width as f32 * size,
                        CELL as f32 * size,
                        uv,
                        c,
                    );
                }
                pen += font.advance(ch) as f32 * size;
            }
        }
    }

    /// Text with a shadow one pixel down and right, readable over the world.
    pub fn text_shadowed(&mut self, font: &Font, text: &str, x: f32, y: f32, color: Rgba) -> f32 {
        self.text(
            font,
            text,
            x + 1.0,
            y + 1.0,
            Rgba::SHADOW.with_alpha(color.0[3].min(200)),
        );
        self.text(font, text, x, y, color)
    }
}

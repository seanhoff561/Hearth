//! The interface's per-window state: the font, the words, this frame's drawing and the input
//! gathered for the widgets since the last frame, and the renderer that draws it all over the
//! frame.

use hearth_render::GpuContext;
use hearth_render::ui::UiRenderer;
use hearth_ui::{DrawList, Font, Lang, NavKey, Ui, UiInput, UiState};

pub struct Interface {
    pub font: Font,
    pub lang: Lang,
    pub state: UiState,
    draw: DrawList,
    renderer: Option<UiRenderer>,
    /// Input gathered since the last frame.
    pub input: UiInput,
    /// The pointer in window pixels (kept across frames).
    pointer_px: Option<(f32, f32)>,
    /// Screen pixels per interface pixel this frame.
    pub scale: u32,
}

impl Interface {
    pub fn new(language: &str) -> Self {
        Self {
            font: Font::new(),
            lang: load_lang(language),
            state: UiState::default(),
            draw: DrawList::new(1),
            renderer: None,
            input: UiInput::default(),
            pointer_px: None,
            scale: 1,
        }
    }

    pub fn set_language(&mut self, language: &str) {
        self.lang = load_lang(language);
    }

    /// The languages the packs have words for.
    pub fn languages() -> Vec<String> {
        let dir = crate::scene::data_pack_dir().join("hearth").join("lang");
        let mut out: Vec<String> = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter_map(|e| {
                        let p = e.path();
                        (p.extension()? == "json")
                            .then(|| p.file_stem()?.to_str().map(str::to_owned))?
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        if out.is_empty() {
            out.push(hearth_ui::lang::DEFAULT.to_owned());
        }
        out
    }

    pub fn pointer_moved(&mut self, x: f32, y: f32) {
        self.pointer_px = Some((x, y));
    }

    pub fn pointer_left(&mut self) {
        self.pointer_px = None;
    }

    pub fn button(&mut self, pressed: bool) {
        if pressed {
            self.input.pressed = true;
            self.input.down = true;
        } else {
            self.input.released = true;
            self.input.down = false;
        }
    }

    pub fn scroll(&mut self, steps: f32) {
        self.input.scroll += steps;
    }

    pub fn typed(&mut self, text: &str) {
        self.input.text.push_str(text);
    }

    pub fn key(&mut self, k: NavKey) {
        self.input.keys.push(k);
    }

    /// Runs one frame of the interface: `build` adds widgets and drawing; then it is drawn over
    /// `target`.
    pub fn frame(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        size: (u32, u32),
        gui_scale: u32,
        build: impl FnOnce(&mut Ui<'_>),
    ) {
        self.scale = hearth_ui::gui_scale(gui_scale, size.1);
        let s = self.scale as f32;
        self.input.pointer = self.pointer_px.map(|(x, y)| (x / s, y / s));
        self.draw.clear(self.scale);
        {
            let mut ui = Ui::begin(
                &self.font,
                &self.lang,
                &mut self.draw,
                &self.input,
                &mut self.state,
                (size.0 as f32 / s, size.1 as f32 / s),
            );
            build(&mut ui);
        }
        // The frame's edges are used; the held button stays held.
        let down = self.input.down;
        self.input = UiInput {
            down,
            ..UiInput::default()
        };
        if self.draw.vertices.is_empty() {
            return;
        }
        let font = &self.font;
        let renderer = self
            .renderer
            .get_or_insert_with(|| UiRenderer::new(ctx, format, &font.pixels));
        renderer.draw(ctx, enc, target, size, &self.draw);
    }
}

fn load_lang(code: &str) -> Lang {
    Lang::load(&[crate::scene::data_pack_dir().join("hearth")], code)
}

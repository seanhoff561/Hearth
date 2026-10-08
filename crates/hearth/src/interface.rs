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
        let lang = load_lang(language);
        set_current(&lang);
        Self {
            font: Font::new(),
            lang,
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
        set_current(&self.lang);
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

    /// The secondary (right) button went down.
    pub fn alt_button(&mut self) {
        self.input.alt_pressed = true;
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

/// The words of the language in use, for what is named away from the interface (the names
/// look-alikes go by); English until the interface sets one.
static CURRENT: std::sync::RwLock<Option<std::sync::Arc<Lang>>> = std::sync::RwLock::new(None);

fn set_current(l: &Lang) {
    if let Ok(mut c) = CURRENT.write() {
        *c = Some(std::sync::Arc::new(l.clone()));
    }
}

pub fn lang() -> std::sync::Arc<Lang> {
    if let Some(l) = CURRENT.read().ok().and_then(|c| c.clone()) {
        return l;
    }
    let l = std::sync::Arc::new(load_lang("en_us"));
    if let Ok(mut c) = CURRENT.write() {
        *c = Some(l.clone());
    }
    l
}

/// A sample of the interface for screenshots (`ui=` in a shot): a panel of the widgets in their
/// states, the words of a page, figures and signs, and a journal's page in the serif face.
pub fn specimen(ui: &mut Ui<'_>) {
    use hearth_ui::widgets::theme;
    use hearth_ui::{Column, Face, Rect, Rgba};
    let (w, h) = ui.size;
    let panel = Rect::new(12.0, 12.0, (w * 0.5 - 18.0).min(260.0), h - 24.0);
    ui.panel(&panel);
    ui.draw.text_sized(
        ui.font,
        "Hearth",
        panel.x + 10.0,
        panel.y + 8.0,
        2.0,
        theme::TEXT,
    );
    let mut c = Column::new(panel.x + 10.0, panel.y + 36.0, panel.w - 20.0);
    let line = hearth_ui::font::LINE as f32;
    for l in ui.font.wrap(
        "A wild Earth without people. Survive, learn and build from nothing.",
        c.w as u32,
    ) {
        ui.label(c.x, c.y, &l, theme::DIM);
        c.space(line);
    }
    c.space(4.0);
    ui.button(c.row(18.0), "Begin a new life here");
    let mut on = true;
    ui.toggle(c.row(18.0), "Guided body bars", &mut on);
    let mut v = 0.6;
    ui.slider(c.row(18.0), "Master volume", &mut v, 0.0, 1.0, "60%");
    let mut i = 1;
    let names = ["Low".to_owned(), "Medium".to_owned(), "High".to_owned()];
    ui.cycle(c.row(18.0), "Graphics", &names, &mut i);
    ui.button_enabled(c.row(18.0), "Play", false);
    ui.label(c.x, c.y, "−4 °C · 12.5 kg × 3 — “dry” …", theme::TEXT);
    c.space(line);
    ui.label(c.x, c.y, "The fire has gone out.", theme::WARN);
    c.space(line);
    // A journal's page: paper, the serif face, a sketch's frame.
    let page = Rect::new(w * 0.5 + 6.0, 12.0, (w * 0.5 - 18.0).min(260.0), h * 0.6);
    let paper = Rgba([226, 214, 188, 245]);
    ui.draw.rect(page.x, page.y, page.w, page.h, paper);
    let ink = Rgba([52, 44, 36, 255]);
    let mut y = page.y + 10.0;
    ui.draw.text_in(
        ui.font,
        Face::Serif,
        "Flint knapping",
        page.x + 10.0,
        y,
        ink,
    );
    y += line * 1.5;
    for l in ui.font.wrap_in(
        Face::Serif,
        "Struck near its edge with a round hammerstone, flint gives a flake as sharp as glass. \
         The angle of the blow matters more than its strength.",
        page.w - 20.0,
    ) {
        ui.draw
            .text_in(ui.font, Face::Serif, &l, page.x + 10.0, y, ink);
        y += line;
    }
}

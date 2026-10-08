//! Immediate-mode widgets: each frame the screens call `button`, `slider`, `toggle`, `cycle`,
//! `text_field` and friends with where they go and what they edit; the widgets draw themselves
//! into the frame's draw list and say what the player did. What persists between frames (which
//! widget is held, the text cursor, scroll offsets) lives in [`UiState`].

use crate::draw::{DrawList, Rgba};
use crate::font::{CELL, Font};
use crate::lang::Lang;

/// A rectangle in interface pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, p: (f32, f32)) -> bool {
        p.0 >= self.x && p.0 < self.x + self.w && p.1 >= self.y && p.1 < self.y + self.h
    }

    /// Where two rectangles overlap, if they do.
    pub fn intersect(&self, o: &Rect) -> Option<Rect> {
        let (x0, y0) = (self.x.max(o.x), self.y.max(o.y));
        let (x1, y1) = (
            (self.x + self.w).min(o.x + o.w),
            (self.y + self.h).min(o.y + o.h),
        );
        (x1 > x0 && y1 > y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
    }

    /// Whether `o` lies wholly within this one.
    pub fn holds(&self, o: &Rect) -> bool {
        o.x >= self.x - 0.01
            && o.y >= self.y - 0.01
            && o.x + o.w <= self.x + self.w + 0.01
            && o.y + o.h <= self.y + self.h + 0.01
    }

    /// Split into a left part `w` wide and the rest (with `gap` between).
    pub fn split_left(&self, w: f32, gap: f32) -> (Rect, Rect) {
        (
            Rect::new(self.x, self.y, w, self.h),
            Rect::new(
                self.x + w + gap,
                self.y,
                (self.w - w - gap).max(0.0),
                self.h,
            ),
        )
    }

    /// Centred `w` × `h` in a frame `size`.
    pub fn centred(size: (f32, f32), w: f32, h: f32) -> Rect {
        Rect::new(
            ((size.0 - w) / 2.0).round(),
            ((size.1 - h) / 2.0).round(),
            w,
            h,
        )
    }
}

/// Rows down a column.
#[derive(Debug, Clone, Copy)]
pub struct Column {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub gap: f32,
}

impl Column {
    pub fn new(x: f32, y: f32, w: f32) -> Self {
        Self { x, y, w, gap: 4.0 }
    }

    /// The next row `h` high.
    pub fn row(&mut self, h: f32) -> Rect {
        let r = Rect::new(self.x, self.y, self.w, h);
        self.y += h + self.gap;
        r
    }

    pub fn space(&mut self, h: f32) {
        self.y += h;
    }
}

/// Keys the interface understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavKey {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Back,
    Tab,
    Backspace,
    Delete,
    Home,
    End,
}

/// The player's input for the interface this frame.
#[derive(Debug, Clone, Default)]
pub struct UiInput {
    /// The pointer (interface pixels), if over the window.
    pub pointer: Option<(f32, f32)>,
    pub down: bool,
    pub pressed: bool,
    pub released: bool,
    /// The secondary (right) button pressed this frame.
    pub alt_pressed: bool,
    /// Wheel steps (positive away from the player).
    pub scroll: f32,
    /// Text typed.
    pub text: String,
    pub keys: Vec<NavKey>,
}

impl UiInput {
    pub fn key(&self, k: NavKey) -> bool {
        self.keys.contains(&k)
    }
}

/// An interactive widget as it was laid out this frame (kept when [`UiState::layout`] is on, for
/// the layout test).
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    /// What it shows.
    pub text: String,
    /// Where it is (interface pixels; in a scrolled area, where the scroll puts it now).
    pub rect: Rect,
    /// The scrolled area it is in, if any, and how far the area's rows run (interface pixels).
    pub area: Option<(Rect, f32)>,
    /// How wide its text is drawn.
    pub text_w: f32,
}

/// What persists between frames.
#[derive(Debug, Clone, Default)]
pub struct UiState {
    /// The widget the pointer went down on (held), and the focused one (keys).
    pub active: Option<u64>,
    pub focus: Option<u64>,
    /// Focusable widgets this frame, in order (for Tab and arrows), and last frame's.
    order: Vec<u64>,
    last_order: Vec<u64>,
    /// Text cursor (characters) of the focused field.
    pub cursor: usize,
    /// Scroll offsets by list id (interface pixels).
    pub scroll: rustc_hash::FxHashMap<u64, f32>,
    /// How far each scrolled area's rows ran last frame (interface pixels).
    extent: rustc_hash::FxHashMap<u64, f32>,
    /// Where the pointer took hold of a scroll bar (interface pixels below the bar's top).
    grab: f32,
    /// The widgets laid out this frame, when kept (the layout test turns it on).
    pub layout: Option<Vec<Placed>>,
    /// Presses since the owner last took them (for the click's sound).
    pub clicks: u32,
}

impl UiState {
    /// A state that keeps where each frame's widgets were laid out (for the layout test).
    pub fn recording() -> Self {
        Self {
            layout: Some(Vec::new()),
            ..Self::default()
        }
    }
}

/// Colours of the interface.
pub mod theme {
    use crate::draw::Rgba;
    pub const PANEL: Rgba = Rgba([16, 18, 22, 210]);
    pub const BUTTON: Rgba = Rgba([58, 62, 70, 235]);
    pub const BUTTON_HOT: Rgba = Rgba([88, 96, 108, 245]);
    pub const BUTTON_DOWN: Rgba = Rgba([44, 48, 54, 245]);
    pub const EDGE: Rgba = Rgba([150, 158, 170, 255]);
    pub const FOCUS: Rgba = Rgba([230, 200, 120, 255]);
    pub const TEXT: Rgba = Rgba([236, 236, 230, 255]);
    pub const DIM: Rgba = Rgba([160, 160, 156, 255]);
    pub const FIELD: Rgba = Rgba([10, 10, 12, 230]);
    pub const FILL: Rgba = Rgba([120, 150, 110, 255]);
    pub const WARN: Rgba = Rgba([230, 120, 90, 255]);
}

/// Widgets for one frame.
pub struct Ui<'a> {
    pub font: &'a Font,
    pub lang: &'a Lang,
    pub draw: &'a mut DrawList,
    pub input: &'a UiInput,
    pub state: &'a mut UiState,
    /// The frame's size (interface pixels).
    pub size: (f32, f32),
    /// Clicks are taken by an open popup this frame.
    pub blocked: bool,
    /// The scrolled area being laid out, if any, and how far its rows ran last frame: the
    /// pointer counts only within it.
    area: Option<(Rect, f32)>,
    /// Where the focused widget lies, when it is in the scrolled area being laid out.
    focused_at: Option<Rect>,
}

/// A scrolled area's id: its label and where it begins (not its height, which follows the
/// window).
fn id_label_id(label: &str, area: &Rect) -> u64 {
    id_of(label, &Rect::new(area.x, area.y, 0.0, 0.0))
}

/// A widget's id from a seed (screens mix the label and the place).
pub fn id_of(label: &str, r: &Rect) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in label.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x1b3);
    }
    for v in [r.x, r.y] {
        h = (h ^ v.to_bits() as u64).wrapping_mul(0x1b3);
    }
    h
}

impl<'a> Ui<'a> {
    /// Starts a frame: keyboard navigation moves the focus along last frame's widgets.
    pub fn begin(
        font: &'a Font,
        lang: &'a Lang,
        draw: &'a mut DrawList,
        input: &'a UiInput,
        state: &'a mut UiState,
        size: (f32, f32),
    ) -> Self {
        state.last_order = std::mem::take(&mut state.order);
        let order = &state.last_order;
        if !order.is_empty() {
            let at = state.focus.and_then(|f| order.iter().position(|&o| o == f));
            let step = |forward: bool| match at {
                Some(i) if forward => order[(i + 1) % order.len()],
                Some(i) => order[(i + order.len() - 1) % order.len()],
                None if forward => order[0],
                None => order[order.len() - 1],
            };
            if input.key(NavKey::Tab) || input.key(NavKey::Down) {
                state.focus = Some(step(true));
                state.cursor = usize::MAX;
            } else if input.key(NavKey::Up) {
                state.focus = Some(step(false));
                state.cursor = usize::MAX;
            }
        }
        if input.released {
            state.active = None;
        }
        if let Some(l) = state.layout.as_mut() {
            l.clear();
        }
        Self {
            font,
            lang,
            draw,
            input,
            state,
            size,
            blocked: false,
            area: None,
            focused_at: None,
        }
    }

    /// The words for a key.
    pub fn t(&self, key: &str) -> String {
        self.lang.get(key).to_owned()
    }

    fn hot(&self, r: &Rect) -> bool {
        !self.blocked
            && self
                .input
                .pointer
                .is_some_and(|p| r.contains(p) && self.area.is_none_or(|(a, _)| a.contains(p)))
    }

    fn focusable(&mut self, id: u64, r: &Rect) -> bool {
        self.state.order.push(id);
        let focused = self.state.focus == Some(id);
        if focused && self.area.is_some() {
            self.focused_at = Some(*r);
        }
        focused
    }

    /// Notes an interactive widget and what it shows, when the layout is kept.
    fn placed(&mut self, r: &Rect, text: &str) {
        let area = self.area;
        let text_w = self.font.width(text) as f32;
        if let Some(l) = self.state.layout.as_mut() {
            l.push(Placed {
                text: text.to_owned(),
                rect: *r,
                area,
                text_w,
            });
        }
    }

    /// How far the rows of the scrolled area `id_label` at `area` ran last frame, if it was laid
    /// out (its size aside: a window's change of height keeps its scroll).
    pub fn scroll_ran(&self, area: &Rect, id_label: &str) -> Option<f32> {
        self.state.extent.get(&id_label_id(id_label, area)).copied()
    }

    /// A row of tabs across `r`, the chosen one marked; true when another is chosen.
    pub fn tabs(&mut self, r: Rect, labels: &[String], index: &mut usize) -> bool {
        if labels.is_empty() {
            return false;
        }
        let n = labels.len() as f32;
        let w = ((r.w - 2.0 * (n - 1.0)) / n).floor();
        let mut changed = false;
        let open = *index;
        for (i, label) in labels.iter().enumerate() {
            let t = Rect::new(r.x + i as f32 * (w + 2.0), r.y, w, r.h);
            if self.button(t, label) && i != *index {
                *index = i;
                changed = true;
            }
            if i == open {
                // The open tab, underlined.
                self.draw
                    .rect(t.x + 1.0, t.y + t.h - 3.0, t.w - 2.0, 2.0, theme::FOCUS);
            }
        }
        changed
    }

    /// A column of rows in `area` that scrolls when they run past it: `body` lays its rows down
    /// the column it is given (from the area's top, moved by the scroll). Rows past the area's
    /// edges are cut away; the wheel over the area, the bar at its right dragged, and the keys
    /// moving the focus to a row out of sight all scroll it. Returns whether it scrolls.
    pub fn scroll(
        &mut self,
        area: Rect,
        id_label: &str,
        gap: f32,
        body: impl FnOnce(&mut Ui<'_>, &mut Column),
    ) -> bool {
        const BAR: f32 = 4.0;
        let id = id_label_id(id_label, &area);
        let bar_id = id ^ 0x5c40_11ba;
        let ran = self.state.extent.get(&id).copied().unwrap_or(0.0);
        let max = (ran - area.h).max(0.0);
        let mut off = self.state.scroll.get(&id).copied().unwrap_or(0.0);
        if max > 0.0 {
            if self.hot(&area) && self.input.scroll != 0.0 {
                off -= self.input.scroll * 24.0;
            }
            let bar_h = (area.h * area.h / ran).max(8.0);
            let track = Rect::new(area.x + area.w - BAR, area.y, BAR, area.h);
            if self.hot(&track) && self.input.pressed {
                let bar_y = area.y + (area.h - bar_h) * off.clamp(0.0, max) / max;
                let y = self.input.pointer.map_or(0.0, |p| p.1);
                self.state.grab = if (bar_y..bar_y + bar_h).contains(&y) {
                    y - bar_y
                } else {
                    bar_h / 2.0
                };
                self.state.active = Some(bar_id);
            }
            if self.state.active == Some(bar_id)
                && self.input.down
                && let Some(p) = self.input.pointer
            {
                let t = (p.1 - self.state.grab - area.y) / (area.h - bar_h).max(1.0);
                off = t * max;
            }
        }
        off = off.clamp(0.0, max);
        // The rows, cut to the area (and to any area it is within).
        let outer = self.draw.clip();
        let inner = [area.x, area.y, area.x + area.w, area.y + area.h];
        let clip = match outer {
            Some(o) => [
                o[0].max(inner[0]),
                o[1].max(inner[1]),
                o[2].min(inner[2]),
                o[3].min(inner[3]),
            ],
            None => inner,
        };
        self.draw.set_clip(Some(clip));
        let outer_area = self.area.replace((area, ran));
        let outer_focus = self.focused_at.take();
        let mut col = Column::new(
            area.x,
            area.y - off,
            area.w - if max > 0.0 { BAR + 2.0 } else { 0.0 },
        );
        col.gap = gap;
        let top = col.y;
        body(self, &mut col);
        let ran = (col.y - top - gap).max(0.0);
        // The focused row kept in sight.
        if let Some(f) = self.focused_at.take() {
            if f.y < area.y {
                off -= area.y - f.y;
            } else if f.y + f.h > area.y + area.h {
                off += f.y + f.h - (area.y + area.h);
            }
        }
        self.area = outer_area;
        self.focused_at = outer_focus;
        self.draw.set_clip(outer);
        let max = (ran - area.h).max(0.0);
        self.state.extent.insert(id, ran);
        self.state.scroll.insert(id, off.clamp(0.0, max));
        if max > 0.0 {
            let off = off.clamp(0.0, max);
            let bar_h = (area.h * area.h / ran).max(8.0);
            let bar_y = area.y + (area.h - bar_h) * off / max;
            let x = area.x + area.w - BAR;
            self.draw
                .rect(x, area.y, BAR, area.h, theme::FIELD.with_alpha(120));
            let held = self.state.active == Some(bar_id);
            self.draw.rect(
                x,
                bar_y,
                BAR,
                bar_h,
                theme::EDGE.with_alpha(if held { 230 } else { 160 }),
            );
        }
        max > 0.0
    }

    fn frame(&mut self, r: &Rect, fill: Rgba, focused: bool) {
        self.draw.rect(r.x, r.y, r.w, r.h, fill);
        let edge = if focused {
            theme::FOCUS
        } else {
            theme::EDGE.with_alpha(90)
        };
        self.draw.rect(r.x, r.y, r.w, 1.0, edge);
        self.draw.rect(r.x, r.y + r.h - 1.0, r.w, 1.0, edge);
        self.draw.rect(r.x, r.y, 1.0, r.h, edge);
        self.draw.rect(r.x + r.w - 1.0, r.y, 1.0, r.h, edge);
    }

    /// Text centred in a rectangle.
    pub fn text_centred(&mut self, r: &Rect, text: &str, color: Rgba) {
        let w = self.font.width(text) as f32;
        let x = (r.x + (r.w - w) / 2.0).round();
        let y = (r.y + (r.h - 7.0) / 2.0).round();
        self.draw.text_shadowed(self.font, text, x, y, color);
    }

    /// Text at a place.
    pub fn label(&mut self, x: f32, y: f32, text: &str, color: Rgba) {
        self.draw
            .text_shadowed(self.font, text, x.round(), y.round(), color);
    }

    /// A panel behind widgets.
    pub fn panel(&mut self, r: &Rect) {
        self.draw.rect(r.x, r.y, r.w, r.h, theme::PANEL);
    }

    /// A title over a frame (centred, at `y`).
    pub fn title(&mut self, y: f32, text: &str) {
        let w = self.font.width(text) as f32;
        self.label((self.size.0 - w) / 2.0, y, text, theme::TEXT);
    }

    /// A button; true when clicked (or Enter while focused).
    pub fn button(&mut self, r: Rect, label: &str) -> bool {
        self.button_enabled(r, label, true)
    }

    pub fn button_enabled(&mut self, r: Rect, label: &str, enabled: bool) -> bool {
        let id = id_of(label, &r);
        let focused = enabled && self.focusable(id, &r);
        self.placed(&r, label);
        let hot = enabled && self.hot(&r);
        if hot && self.input.pressed {
            self.state.active = Some(id);
        }
        let held = self.state.active == Some(id) && self.input.down;
        let fill = if !enabled {
            theme::BUTTON.with_alpha(120)
        } else if held {
            theme::BUTTON_DOWN
        } else if hot {
            theme::BUTTON_HOT
        } else {
            theme::BUTTON
        };
        self.frame(&r, fill, focused);
        let color = if enabled { theme::TEXT } else { theme::DIM };
        self.text_centred(&r, label, color);
        let clicked = hot && self.input.released && self.state.active.is_none_or(|a| a == id);
        let pressed = enabled && (clicked || (focused && self.input.key(NavKey::Enter)));
        if pressed {
            self.state.clicks += 1;
        }
        pressed
    }

    /// A slider over `min..=max`; the label shows `text` (the value in words). True when changed.
    pub fn slider(
        &mut self,
        r: Rect,
        label: &str,
        value: &mut f32,
        min: f32,
        max: f32,
        text: &str,
    ) -> bool {
        let id = id_of(label, &r);
        let focused = self.focusable(id, &r);
        self.placed(&r, &format!("{label}: {text}"));
        let hot = self.hot(&r);
        if hot && self.input.pressed {
            self.state.active = Some(id);
        }
        let before = *value;
        if self.state.active == Some(id)
            && self.input.down
            && let Some(p) = self.input.pointer
        {
            let t = ((p.0 - r.x - 2.0) / (r.w - 4.0)).clamp(0.0, 1.0);
            *value = min + t * (max - min);
        }
        if focused {
            let step = (max - min) / 20.0;
            if self.input.key(NavKey::Left) {
                *value = (*value - step).max(min);
            }
            if self.input.key(NavKey::Right) {
                *value = (*value + step).min(max);
            }
        }
        if hot && self.input.scroll != 0.0 {
            *value = (*value + self.input.scroll * (max - min) / 40.0).clamp(min, max);
        }
        self.frame(&r, theme::FIELD, focused);
        let t = if max > min {
            ((*value - min) / (max - min)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.draw.rect(
            r.x + 1.0,
            r.y + 1.0,
            (r.w - 2.0) * t,
            r.h - 2.0,
            theme::FILL.with_alpha(150),
        );
        self.text_centred(&r, &format!("{label}: {text}"), theme::TEXT);
        *value != before
    }

    /// A switch; true when flipped.
    pub fn toggle(&mut self, r: Rect, label: &str, value: &mut bool) -> bool {
        let words = if *value {
            self.t("ui.on")
        } else {
            self.t("ui.off")
        };
        if self.button(r, &format!("{label}: {words}")) {
            *value = !*value;
            true
        } else {
            false
        }
    }

    /// One of several choices, stepped by clicking (right click or Left steps back). True when
    /// changed.
    pub fn cycle(&mut self, r: Rect, label: &str, choices: &[String], index: &mut usize) -> bool {
        if choices.is_empty() {
            return false;
        }
        let id = id_of(label, &r);
        let shown = format!("{label}: {}", choices[(*index).min(choices.len() - 1)]);
        let focused = self.state.focus == Some(id);
        let back = focused && self.input.key(NavKey::Left);
        let clicked = self.button(r, &shown) || (focused && self.input.key(NavKey::Right));
        if back {
            *index = (*index + choices.len() - 1) % choices.len();
            true
        } else if clicked {
            *index = (*index + 1) % choices.len();
            true
        } else {
            false
        }
    }

    /// A text field; true when its text changed. Clicking focuses it; typing edits it.
    pub fn text_field(&mut self, r: Rect, label: &str, value: &mut String, max_len: usize) -> bool {
        let id = id_of(label, &r);
        let focused = self.focusable(id, &r);
        let shown = if value.is_empty() {
            label
        } else {
            value.as_str()
        };
        let shown = shown.to_owned();
        self.placed(&r, &shown);
        if self.hot(&r) && self.input.pressed {
            self.state.focus = Some(id);
            self.state.cursor = usize::MAX;
        }
        let mut changed = false;
        if focused {
            let len = value.chars().count();
            let mut cur = self.state.cursor.min(len);
            for c in self.input.text.chars() {
                if !c.is_control() && value.chars().count() < max_len {
                    let at = value
                        .char_indices()
                        .nth(cur)
                        .map_or(value.len(), |(i, _)| i);
                    value.insert(at, c);
                    cur += 1;
                    changed = true;
                }
            }
            for k in &self.input.keys {
                match k {
                    NavKey::Backspace if cur > 0 => {
                        let at = value
                            .char_indices()
                            .nth(cur - 1)
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        value.remove(at);
                        cur -= 1;
                        changed = true;
                    }
                    NavKey::Delete if cur < value.chars().count() => {
                        let at = value.char_indices().nth(cur).map(|(i, _)| i).unwrap_or(0);
                        value.remove(at);
                        changed = true;
                    }
                    NavKey::Left => cur = cur.saturating_sub(1),
                    NavKey::Right => cur = (cur + 1).min(value.chars().count()),
                    NavKey::Home => cur = 0,
                    NavKey::End => cur = value.chars().count(),
                    _ => {}
                }
            }
            self.state.cursor = cur;
        }
        self.frame(&r, theme::FIELD, focused);
        let shown = if value.is_empty() && !focused {
            label.to_owned()
        } else {
            value.clone()
        };
        let color = if value.is_empty() {
            theme::DIM
        } else {
            theme::TEXT
        };
        let ty = (r.y + (r.h - 7.0) / 2.0).round();
        self.draw
            .text_shadowed(self.font, &shown, r.x + 4.0, ty, color);
        if focused {
            let before: String = value.chars().take(self.state.cursor).collect();
            let cx = r.x
                + 4.0
                + self.font.width(&before) as f32
                + if before.is_empty() { 0.0 } else { 1.0 };
            self.draw.rect(cx, ty - 1.0, 1.0, CELL as f32, theme::FOCUS);
        }
        changed
    }

    /// A list of rows `row_h` high in `r`, scrolled by the wheel; returns the index of a row
    /// clicked, and calls `row` to draw each visible one (with whether it is selected).
    pub fn list(
        &mut self,
        r: Rect,
        id_label: &str,
        count: usize,
        row_h: f32,
        selected: Option<usize>,
        mut row: impl FnMut(&mut Ui<'_>, Rect, usize, bool),
    ) -> Option<usize> {
        let id = id_of(id_label, &r);
        self.placed(&r, "");
        let content = count as f32 * row_h;
        let max = (content - r.h).max(0.0);
        let mut off = self.state.scroll.get(&id).copied().unwrap_or(0.0);
        if self.hot(&r) && self.input.scroll != 0.0 {
            off -= self.input.scroll * row_h * 2.0;
        }
        off = off.clamp(0.0, max);
        self.state.scroll.insert(id, off);
        self.draw
            .rect(r.x, r.y, r.w, r.h, theme::FIELD.with_alpha(160));
        let mut clicked = None;
        let first = (off / row_h).floor() as usize;
        for i in first..count {
            let y = r.y + i as f32 * row_h - off;
            if y + row_h > r.y + r.h + 0.5 {
                break;
            }
            let rr = Rect::new(r.x, y, r.w, row_h);
            let hot = self.hot(&rr);
            let sel = selected == Some(i);
            if sel || hot {
                let c = if sel {
                    theme::BUTTON_HOT
                } else {
                    theme::BUTTON
                };
                self.draw.rect(rr.x, rr.y, rr.w, rr.h, c);
            }
            if hot && self.input.released && !self.blocked {
                clicked = Some(i);
                self.state.clicks += 1;
            }
            row(self, rr, i, sel);
        }
        if max > 0.0 {
            // The scroll bar.
            let bar_h = (r.h * r.h / content).max(8.0);
            let bar_y = r.y + (r.h - bar_h) * off / max;
            self.draw.rect(
                r.x + r.w - 3.0,
                bar_y,
                2.0,
                bar_h,
                theme::EDGE.with_alpha(160),
            );
        }
        clicked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame<R>(state: &mut UiState, input: &UiInput, f: impl FnOnce(&mut Ui<'_>) -> R) -> R {
        let font = Font::new();
        let lang = Lang::from_pairs(&[("ui.on", "on"), ("ui.off", "off")]);
        let mut draw = DrawList::new(1);
        let mut ui = Ui::begin(&font, &lang, &mut draw, input, state, (320.0, 240.0));
        f(&mut ui)
    }

    #[test]
    fn a_click_is_a_press_and_release_on_the_button() {
        let mut st = UiState::default();
        let r = Rect::new(10.0, 10.0, 100.0, 20.0);
        let at = Some((50.0, 20.0));
        let press = UiInput {
            pointer: at,
            pressed: true,
            down: true,
            ..Default::default()
        };
        assert!(!frame(&mut st, &press, |ui| ui.button(r, "Play")));
        let release = UiInput {
            pointer: at,
            released: true,
            ..Default::default()
        };
        assert!(frame(&mut st, &release, |ui| ui.button(r, "Play")));
        // Released elsewhere: no click.
        let away = UiInput {
            pointer: Some((200.0, 200.0)),
            released: true,
            ..Default::default()
        };
        assert!(!frame(&mut st, &away, |ui| ui.button(r, "Play")));
    }

    #[test]
    fn keys_move_focus_and_type() {
        let mut st = UiState::default();
        let a = Rect::new(0.0, 0.0, 100.0, 20.0);
        let b = Rect::new(0.0, 30.0, 100.0, 20.0);
        let mut name = String::new();
        let none = UiInput::default();
        // A first frame to learn the widgets.
        frame(&mut st, &none, |ui| {
            ui.button(a, "A");
            ui.text_field(b, "Name", &mut name, 16);
        });
        let tab = UiInput {
            keys: vec![NavKey::Tab],
            ..Default::default()
        };
        frame(&mut st, &tab, |ui| {
            ui.button(a, "A");
            ui.text_field(b, "Name", &mut name, 16);
        });
        let tab2 = tab.clone();
        frame(&mut st, &tab2, |ui| {
            ui.button(a, "A");
            ui.text_field(b, "Name", &mut name, 16);
        });
        let typing = UiInput {
            text: "Ada".into(),
            ..Default::default()
        };
        frame(&mut st, &typing, |ui| {
            ui.button(a, "A");
            ui.text_field(b, "Name", &mut name, 16)
        });
        assert_eq!(name, "Ada");
        let back = UiInput {
            keys: vec![NavKey::Backspace],
            ..Default::default()
        };
        frame(&mut st, &back, |ui| {
            ui.button(a, "A");
            ui.text_field(b, "Name", &mut name, 16)
        });
        assert_eq!(name, "Ad");
    }

    /// Twenty rows in an area that shows five: the rest are cut away, and the wheel or the keys
    /// bring them into sight.
    #[test]
    fn a_long_column_scrolls_within_its_area() {
        let mut st = UiState::recording();
        let area = Rect::new(10.0, 20.0, 120.0, 100.0);
        let rows = |ui: &mut Ui<'_>, c: &mut Column| {
            for i in 0..20 {
                ui.button(c.row(16.0), &format!("Row {i}"));
            }
        };
        let none = UiInput::default();
        // The first frame learns how far the rows run; the second knows it scrolls.
        frame(&mut st, &none, |ui| ui.scroll(area, "list", 4.0, rows));
        assert!(frame(&mut st, &none, |ui| ui.scroll(area, "list", 4.0, rows)));
        let shown = |st: &UiState| {
            st.layout
                .as_ref()
                .unwrap()
                .iter()
                .filter(|p| area.holds(&p.rect))
                .map(|p| p.text.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(shown(&st).first().map(String::as_str), Some("Row 0"));
        assert_eq!(st.layout.as_ref().unwrap().len(), 20, "every row laid out");
        // Clicks past the area's edge reach nothing.
        let below = UiInput {
            pointer: Some((50.0, 130.0)),
            pressed: true,
            down: true,
            ..Default::default()
        };
        frame(&mut st, &below, |ui| ui.scroll(area, "list", 4.0, rows));
        assert_eq!(st.active, None);
        // The wheel scrolls toward the end.
        let wheel = UiInput {
            pointer: Some((50.0, 60.0)),
            scroll: -20.0,
            ..Default::default()
        };
        frame(&mut st, &wheel, |ui| ui.scroll(area, "list", 4.0, rows));
        frame(&mut st, &none, |ui| ui.scroll(area, "list", 4.0, rows));
        assert_eq!(
            shown(&st).last().map(String::as_str),
            Some("Row 19"),
            "{:?}",
            shown(&st)
        );
        // The keys take the focus to the first row, out of sight above: the column follows it.
        let down = UiInput {
            keys: vec![NavKey::Down],
            ..Default::default()
        };
        frame(&mut st, &down, |ui| ui.scroll(area, "list", 4.0, rows));
        frame(&mut st, &none, |ui| ui.scroll(area, "list", 4.0, rows));
        assert!(
            shown(&st).contains(&"Row 0".to_owned()),
            "the focused row in sight: {:?}",
            shown(&st)
        );
    }

    #[test]
    fn drawing_is_cut_to_the_clip() {
        let mut d = DrawList::new(2);
        d.set_clip(Some([0.0, 0.0, 10.0, 10.0]));
        d.rect(5.0, 5.0, 10.0, 10.0, Rgba::WHITE);
        let xs: Vec<f32> = d.vertices.iter().map(|v| v.pos[0]).collect();
        assert!(xs.iter().all(|&x| (10.0..=20.0).contains(&x)), "{xs:?}");
        let n = d.vertices.len();
        d.rect(20.0, 20.0, 5.0, 5.0, Rgba::WHITE);
        assert_eq!(d.vertices.len(), n, "wholly outside: nothing drawn");
    }

    #[test]
    fn sliders_follow_the_pointer() {
        let mut st = UiState::default();
        let r = Rect::new(0.0, 0.0, 104.0, 16.0);
        let mut v = 0.0;
        let grab = UiInput {
            pointer: Some((52.0, 8.0)),
            pressed: true,
            down: true,
            ..Default::default()
        };
        assert!(frame(&mut st, &grab, |ui| ui
            .slider(r, "FOV", &mut v, 0.0, 100.0, "")));
        assert!((v - 50.0).abs() < 1.0, "{v}");
    }
}

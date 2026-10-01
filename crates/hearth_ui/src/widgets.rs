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
    /// Presses since the owner last took them (for the click's sound).
    pub clicks: u32,
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
        Self {
            font,
            lang,
            draw,
            input,
            state,
            size,
            blocked: false,
        }
    }

    /// The words for a key.
    pub fn t(&self, key: &str) -> String {
        self.lang.get(key).to_owned()
    }

    fn hot(&self, r: &Rect) -> bool {
        !self.blocked && self.input.pointer.is_some_and(|p| r.contains(p))
    }

    fn focusable(&mut self, id: u64) -> bool {
        self.state.order.push(id);
        self.state.focus == Some(id)
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
        let focused = enabled && self.focusable(id);
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
        let focused = self.focusable(id);
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
        let focused = self.focusable(id);
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

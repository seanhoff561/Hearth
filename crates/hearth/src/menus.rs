//! The screens (v1 §11, the minimal set for V2-3; the full flow is V2-15): the title, the worlds
//! (pick one or make a new one), pause, and the options — video, controls with rebinding,
//! language, accessibility. Every screen is drawn each frame with the widgets of `hearth_ui`
//! and says what the player chose.

use std::path::{Path, PathBuf};

use hearth_core::options::{DisplayMode, GraphicsPreset, Options, Quality};
use hearth_input::{ActionId, CaptureResult, InputKey, KeyBindings, RebindCapture};
use hearth_ui::widgets::theme;
use hearth_ui::{Column, Rect, Ui};

/// A world on disk.
#[derive(Debug, Clone)]
pub struct WorldEntry {
    /// Its folder in the saves (the name the server opens).
    pub folder: String,
    pub name: String,
    pub last_played_unix: u64,
}

/// The worlds in a saves folder, most recently played first.
pub fn list_worlds(saves: &Path) -> Vec<WorldEntry> {
    let mut out: Vec<WorldEntry> = std::fs::read_dir(saves)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().join("level.json").exists())
                .filter_map(|e| {
                    let folder = e.file_name().to_str()?.to_owned();
                    let (_, meta, _) = hearth_save::WorldDir::open(&e.path()).ok()?;
                    Some(WorldEntry {
                        folder,
                        name: meta.name,
                        last_played_unix: meta.last_played_unix,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|w| std::cmp::Reverse(w.last_played_unix));
    out
}

pub enum Screen {
    Title,
    Worlds {
        list: Vec<WorldEntry>,
        selected: Option<usize>,
    },
    NewWorld {
        name: String,
        seed: String,
    },
    Pause,
    Options,
    Video,
    Controls {
        capturing: Option<ActionId>,
        capture: RebindCapture,
    },
    Accessibility,
}

/// What the player chose.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction {
    /// Play a world (its folder; the seed is used if it is new).
    Play {
        folder: String,
        seed: u64,
    },
    Resume,
    QuitToTitle,
    QuitGame,
    /// The options changed: save and apply them.
    OptionsChanged,
    LanguageChanged,
}

/// What the screens edit and need.
pub struct MenuContext<'a> {
    pub options: &'a mut Options,
    pub bindings: &'a mut KeyBindings,
    pub saves: PathBuf,
    pub in_game: bool,
    pub languages: &'a [String],
}

/// The open screens, the top one shown.
pub struct Menus {
    stack: Vec<Screen>,
}

const W: f32 = 220.0;
const ROW: f32 = 18.0;

impl Menus {
    pub fn title() -> Self {
        Self {
            stack: vec![Screen::Title],
        }
    }

    pub fn none() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn is_open(&self) -> bool {
        !self.stack.is_empty()
    }

    pub fn open(&mut self, s: Screen) {
        self.stack.push(s);
    }

    pub fn close_all(&mut self) {
        self.stack.clear();
    }

    /// Back one screen (Escape). Closing the pause screen resumes the game.
    pub fn back(&mut self) -> Option<MenuAction> {
        match self.stack.last() {
            Some(Screen::Title) | None => None,
            Some(Screen::Pause) => {
                self.stack.pop();
                Some(MenuAction::Resume)
            }
            Some(_) => {
                self.stack.pop();
                None
            }
        }
    }

    /// Whether the controls screen is waiting for a key.
    pub fn capturing(&self) -> bool {
        matches!(
            self.stack.last(),
            Some(Screen::Controls {
                capturing: Some(_),
                ..
            })
        )
    }

    /// A key while capturing a binding; returns whether the options changed.
    pub fn capture(&mut self, key: InputKey, bindings: &mut KeyBindings) -> bool {
        if let Some(Screen::Controls { capturing, capture }) = self.stack.last_mut()
            && let Some(action) = *capturing
        {
            match capture.press(key) {
                CaptureResult::Pending => false,
                CaptureResult::Done(binding) => {
                    bindings.set(action, binding);
                    *capturing = None;
                    *capture = RebindCapture::new();
                    true
                }
            }
        } else {
            false
        }
    }

    /// Draws the top screen and returns what was chosen.
    pub fn ui(&mut self, ui: &mut Ui<'_>, cx: &mut MenuContext<'_>) -> Vec<MenuAction> {
        let mut out = Vec::new();
        let Some(top) = self.stack.last_mut() else {
            return out;
        };
        let mut push: Option<Screen> = None;
        let mut pop = false;
        let size = ui.size;
        // A veil over the world behind the menus.
        if cx.in_game {
            ui.draw
                .rect(0.0, 0.0, size.0, size.1, theme::PANEL.with_alpha(120));
        } else {
            ui.draw
                .rect(0.0, 0.0, size.0, size.1, hearth_ui::Rgba([14, 18, 24, 255]));
        }
        let x = ((size.0 - W) / 2.0).round();
        match top {
            Screen::Title => {
                let t = ui.t("game.title").to_uppercase();
                let tw = ui.font.width(&t) as f32 * 4.0;
                ui.draw.text_sized(
                    ui.font,
                    &t,
                    ((size.0 - tw) / 2.0).round(),
                    (size.1 * 0.2).round(),
                    4.0,
                    theme::TEXT,
                );
                let sub = ui.t("menu.title.subtitle");
                ui.title((size.1 * 0.2 + 42.0).round(), &sub);
                let mut c = Column::new(x, (size.1 * 0.48).round(), W);
                if ui.button(c.row(ROW), &ui.t("menu.title.play")) {
                    push = Some(Screen::Worlds {
                        list: list_worlds(&cx.saves),
                        selected: None,
                    });
                }
                if ui.button(c.row(ROW), &ui.t("menu.options")) {
                    push = Some(Screen::Options);
                }
                if ui.button(c.row(ROW), &ui.t("menu.title.quit")) {
                    out.push(MenuAction::QuitGame);
                }
            }
            Screen::Worlds { list, selected } => {
                ui.title(16.0, &ui.t("menu.worlds.title"));
                let list_r = Rect::new(x - 40.0, 34.0, W + 80.0, (size.1 - 100.0).max(60.0));
                let entries = list.clone();
                let clicked = ui.list(
                    list_r,
                    "worlds",
                    entries.len(),
                    20.0,
                    *selected,
                    |ui, r, i, _| {
                        let w = &entries[i];
                        ui.label(r.x + 4.0, r.y + 2.0, &w.name, theme::TEXT);
                        ui.label(r.x + 4.0, r.y + 11.0, &w.folder, theme::DIM);
                    },
                );
                if entries.is_empty() {
                    let hint = ui.t("menu.worlds.none");
                    ui.text_centred(&list_r, &hint, theme::DIM);
                }
                if let Some(i) = clicked {
                    if *selected == Some(i) {
                        // A second click plays it.
                        out.push(MenuAction::Play {
                            folder: entries[i].folder.clone(),
                            seed: 0,
                        });
                    }
                    *selected = Some(i);
                }
                let mut c = Column::new(x - 40.0, size.1 - 60.0, W + 80.0);
                let row = c.row(ROW);
                let (a, rest) = row.split_left((W + 80.0 - 8.0) / 3.0, 4.0);
                let (b, d) = rest.split_left((W + 80.0 - 8.0) / 3.0, 4.0);
                if ui.button_enabled(a, &ui.t("menu.worlds.play"), selected.is_some())
                    && let Some(i) = *selected
                {
                    out.push(MenuAction::Play {
                        folder: entries[i].folder.clone(),
                        seed: 0,
                    });
                }
                if ui.button(b, &ui.t("menu.worlds.new")) {
                    push = Some(Screen::NewWorld {
                        name: String::new(),
                        seed: String::new(),
                    });
                }
                if ui.button(d, &ui.t("menu.back")) {
                    pop = true;
                }
            }
            Screen::NewWorld { name, seed } => {
                ui.title(30.0, &ui.t("menu.new_world.title"));
                let mut c = Column::new(x, 60.0, W);
                ui.label(x, c.y, &ui.t("menu.new_world.name"), theme::DIM);
                c.space(10.0);
                ui.text_field(c.row(ROW), &ui.t("menu.new_world.name_hint"), name, 32);
                c.space(6.0);
                ui.label(x, c.y, &ui.t("menu.new_world.seed"), theme::DIM);
                c.space(10.0);
                ui.text_field(c.row(ROW), &ui.t("menu.new_world.seed_hint"), seed, 20);
                c.space(10.0);
                let folder = folder_name(name);
                let exists = cx.saves.join(&folder).join("level.json").exists();
                if exists {
                    ui.label(x, c.y, &ui.t("menu.new_world.exists"), theme::WARN);
                    c.space(10.0);
                }
                if ui.button_enabled(c.row(ROW), &ui.t("menu.new_world.create"), !exists) {
                    out.push(MenuAction::Play {
                        folder,
                        seed: parse_seed(seed),
                    });
                }
                if ui.button(c.row(ROW), &ui.t("menu.back")) {
                    pop = true;
                }
            }
            Screen::Pause => {
                ui.title((size.1 * 0.3).round(), &ui.t("menu.pause.title"));
                let mut c = Column::new(x, (size.1 * 0.3 + 20.0).round(), W);
                if ui.button(c.row(ROW), &ui.t("menu.pause.resume")) {
                    out.push(MenuAction::Resume);
                }
                if ui.button(c.row(ROW), &ui.t("menu.options")) {
                    push = Some(Screen::Options);
                }
                if ui.button(c.row(ROW), &ui.t("menu.pause.quit")) {
                    out.push(MenuAction::QuitToTitle);
                }
            }
            Screen::Options => {
                ui.title(30.0, &ui.t("menu.options"));
                let mut c = Column::new(x, 56.0, W);
                if ui.button(c.row(ROW), &ui.t("menu.options.video")) {
                    push = Some(Screen::Video);
                }
                if ui.button(c.row(ROW), &ui.t("menu.options.controls")) {
                    push = Some(Screen::Controls {
                        capturing: None,
                        capture: RebindCapture::new(),
                    });
                }
                if ui.button(c.row(ROW), &ui.t("menu.options.accessibility")) {
                    push = Some(Screen::Accessibility);
                }
                let names: Vec<String> = cx
                    .languages
                    .iter()
                    .map(|code| language_name(ui, code))
                    .collect();
                let mut i = cx
                    .languages
                    .iter()
                    .position(|l| *l == cx.options.language)
                    .unwrap_or(0);
                if ui.cycle(c.row(ROW), &ui.t("menu.options.language"), &names, &mut i) {
                    cx.options.language = cx.languages[i].clone();
                    out.push(MenuAction::LanguageChanged);
                    out.push(MenuAction::OptionsChanged);
                }
                c.space(8.0);
                if ui.button(c.row(ROW), &ui.t("menu.done")) {
                    pop = true;
                }
            }
            Screen::Video => {
                ui.title(12.0, &ui.t("menu.options.video"));
                let mut changed = false;
                let v = &mut cx.options.video;
                let wide = W + 120.0;
                let mut c = Column::new(((size.0 - wide) / 2.0).round(), 28.0, wide);
                c.gap = 3.0;
                let presets = [
                    GraphicsPreset::Fast,
                    GraphicsPreset::Fancy,
                    GraphicsPreset::Fabulous,
                    GraphicsPreset::Custom,
                ];
                let names: Vec<String> = ["fast", "fancy", "fabulous", "custom"]
                    .iter()
                    .map(|k| ui.t(&format!("menu.video.preset.{k}")))
                    .collect();
                let mut i = presets.iter().position(|p| *p == v.graphics).unwrap_or(3);
                if ui.cycle(c.row(ROW), &ui.t("menu.video.preset"), &names, &mut i) {
                    v.apply_preset(presets[i]);
                    changed = true;
                }
                let mut rd = v.render_distance as f32;
                let text = format!("{}", rd.round());
                if ui.slider(
                    c.row(ROW),
                    &ui.t("menu.video.render_distance"),
                    &mut rd,
                    2.0,
                    32.0,
                    &text,
                ) {
                    v.render_distance = rd.round() as u32;
                    changed = true;
                }
                let mut vd = v.vertical_render_distance as f32;
                let text = format!("{}", vd.round());
                if ui.slider(
                    c.row(ROW),
                    &ui.t("menu.video.vertical_distance"),
                    &mut vd,
                    2.0,
                    16.0,
                    &text,
                ) {
                    v.vertical_render_distance = vd.round() as u32;
                    changed = true;
                }
                let mut ld = v.lod_distance as f32;
                let lod_text = if ld < 1.0 {
                    ui.t("ui.off")
                } else {
                    format!("{}", ld.round())
                };
                if ui.slider(
                    c.row(ROW),
                    &ui.t("menu.video.lod_distance"),
                    &mut ld,
                    0.0,
                    1024.0,
                    &lod_text,
                ) {
                    v.lod_distance = ((ld / 32.0).round() * 32.0) as u32;
                    changed = true;
                }
                let qualities = [Quality::Low, Quality::Medium, Quality::High];
                let qnames: Vec<String> = ["low", "medium", "high"]
                    .iter()
                    .map(|k| ui.t(&format!("menu.quality.{k}")))
                    .collect();
                let mut qi = qualities
                    .iter()
                    .position(|q| *q == v.lod_detail)
                    .unwrap_or(1);
                if ui.cycle(c.row(ROW), &ui.t("menu.video.lod_detail"), &qnames, &mut qi) {
                    v.lod_detail = qualities[qi];
                    v.refresh_preset();
                    changed = true;
                }
                let mut wi = qualities
                    .iter()
                    .position(|q| *q == v.shader.water)
                    .unwrap_or(1);
                if ui.cycle(c.row(ROW), &ui.t("menu.video.water"), &qnames, &mut wi) {
                    v.shader.water = qualities[wi];
                    v.refresh_preset();
                    changed = true;
                }
                let mut fov = v.fov;
                let text = format!("{}°", fov.round());
                if ui.slider(
                    c.row(ROW),
                    &ui.t("menu.video.fov"),
                    &mut fov,
                    30.0,
                    110.0,
                    &text,
                ) {
                    v.fov = fov.round();
                    changed = true;
                }
                let scales = [0.5f32, 0.67, 0.75, 1.0, 1.25, 1.5, 2.0];
                let snames: Vec<String> = scales
                    .iter()
                    .map(|s| format!("{}%", (s * 100.0).round()))
                    .collect();
                let mut si = scales
                    .iter()
                    .position(|s| (s - v.render_scale).abs() < 0.01)
                    .unwrap_or(3);
                if ui.cycle(
                    c.row(ROW),
                    &ui.t("menu.video.render_scale"),
                    &snames,
                    &mut si,
                ) {
                    v.render_scale = scales[si];
                    changed = true;
                }
                let gnames: Vec<String> = std::iter::once(ui.t("menu.video.gui_scale.auto"))
                    .chain((1..=6).map(|n| n.to_string()))
                    .collect();
                let mut gi = (v.gui_scale as usize).min(6);
                if ui.cycle(c.row(ROW), &ui.t("menu.video.gui_scale"), &gnames, &mut gi) {
                    v.gui_scale = gi as u32;
                    changed = true;
                }
                let mut vsync = v.vsync;
                if ui.toggle(c.row(ROW), &ui.t("menu.video.vsync"), &mut vsync) {
                    v.vsync = vsync;
                    changed = true;
                }
                let mut fps = if v.max_framerate == 0 {
                    300.0
                } else {
                    v.max_framerate as f32
                };
                let fps_text = if fps >= 300.0 {
                    ui.t("menu.video.unlimited")
                } else {
                    format!("{}", fps.round())
                };
                if ui.slider(
                    c.row(ROW),
                    &ui.t("menu.video.max_fps"),
                    &mut fps,
                    30.0,
                    300.0,
                    &fps_text,
                ) {
                    v.max_framerate = if fps >= 299.5 { 0 } else { fps.round() as u32 };
                    changed = true;
                }
                let modes = [
                    DisplayMode::Windowed,
                    DisplayMode::Borderless,
                    DisplayMode::Exclusive,
                ];
                let mnames: Vec<String> = ["windowed", "borderless", "exclusive"]
                    .iter()
                    .map(|k| ui.t(&format!("menu.video.display.{k}")))
                    .collect();
                let mut mi = modes.iter().position(|m| *m == v.display_mode).unwrap_or(0);
                if ui.cycle(c.row(ROW), &ui.t("menu.video.display"), &mnames, &mut mi) {
                    v.display_mode = modes[mi];
                    changed = true;
                }
                c.space(4.0);
                if ui.button(c.row(ROW), &ui.t("menu.done")) {
                    pop = true;
                }
                if changed {
                    out.push(MenuAction::OptionsChanged);
                }
            }
            Screen::Controls { capturing, .. } => {
                ui.title(10.0, &ui.t("menu.options.controls"));
                let mut changed = false;
                let wide = W + 140.0;
                let left = ((size.0 - wide) / 2.0).round();
                let mut c = Column::new(left, 24.0, wide);
                c.gap = 3.0;
                let ctl = &mut cx.options.controls;
                let row = c.row(ROW);
                let (a, b) = row.split_left((wide - 4.0) / 2.0, 4.0);
                let mut sens = ctl.mouse_sensitivity;
                let text = format!("{}%", (sens * 200.0).round());
                if ui.slider(
                    a,
                    &ui.t("menu.controls.sensitivity"),
                    &mut sens,
                    0.0,
                    1.0,
                    &text,
                ) {
                    ctl.mouse_sensitivity = sens;
                    changed = true;
                }
                let mut inv = ctl.invert_y;
                if ui.toggle(b, &ui.t("menu.controls.invert_y"), &mut inv) {
                    ctl.invert_y = inv;
                    changed = true;
                }
                let row = c.row(ROW);
                let (a, b) = row.split_left((wide - 4.0) / 2.0, 4.0);
                let mut ts = ctl.toggle_sneak;
                if ui.toggle(a, &ui.t("menu.controls.toggle_sneak"), &mut ts) {
                    ctl.toggle_sneak = ts;
                    changed = true;
                }
                let mut tr = ctl.toggle_sprint;
                if ui.toggle(b, &ui.t("menu.controls.toggle_sprint"), &mut tr) {
                    ctl.toggle_sprint = tr;
                    changed = true;
                }
                // The key bindings.
                let ids: Vec<ActionId> = cx.bindings.registry().ids().collect();
                let list_h = (size.1 - c.y - 30.0).max(40.0);
                let list_r = Rect::new(left, c.y, wide, list_h);
                let bindings = &*cx.bindings;
                let waiting = *capturing;
                let press_key = ui.t("menu.controls.press_key");
                let unbound = ui.t("menu.controls.unbound");
                let clicked = ui.list(
                    list_r,
                    "bindings",
                    ids.len(),
                    12.0,
                    waiting.and_then(|w| ids.iter().position(|i| *i == w)),
                    |ui, r, i, _| {
                        let id = ids[i];
                        let def = bindings.registry().def(id);
                        ui.label(r.x + 4.0, r.y + 2.0, ui.lang.get(&def.id), theme::TEXT);
                        let b = if waiting == Some(id) {
                            press_key.clone()
                        } else {
                            bindings
                                .get(id)
                                .map_or(unbound.clone(), |b| b.display_name())
                        };
                        let conflict = !bindings.conflicts_of(id).is_empty();
                        let bw = ui.font.width(&b) as f32;
                        ui.label(
                            r.x + r.w - bw - 8.0,
                            r.y + 2.0,
                            &b,
                            if conflict { theme::WARN } else { theme::DIM },
                        );
                    },
                );
                if let Some(i) = clicked {
                    *capturing = Some(ids[i]);
                }
                let mut c2 = Column::new(left, list_r.y + list_r.h + 6.0, wide);
                let row = c2.row(ROW);
                let (a, b) = row.split_left((wide - 4.0) / 2.0, 4.0);
                if ui.button(a, &ui.t("menu.controls.reset_all")) {
                    cx.bindings.reset_all();
                    changed = true;
                }
                if ui.button(b, &ui.t("menu.done")) {
                    pop = true;
                }
                if changed {
                    out.push(MenuAction::OptionsChanged);
                }
            }
            Screen::Accessibility => {
                ui.title(30.0, &ui.t("menu.options.accessibility"));
                let a = &mut cx.options.accessibility;
                let mut c = Column::new(x, 56.0, W);
                let mut op = a.text_background_opacity;
                let text = format!("{}%", (op * 100.0).round());
                if ui.slider(
                    c.row(ROW),
                    &ui.t("menu.accessibility.text_background"),
                    &mut op,
                    0.0,
                    1.0,
                    &text,
                ) {
                    a.text_background_opacity = op;
                    out.push(MenuAction::OptionsChanged);
                }
                c.space(8.0);
                if ui.button(c.row(ROW), &ui.t("menu.done")) {
                    pop = true;
                }
            }
        }
        if pop {
            self.stack.pop();
        }
        if let Some(s) = push {
            self.stack.push(s);
        }
        out
    }
}

/// A world's folder from its name.
pub fn folder_name(name: &str) -> String {
    let f: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if f.is_empty() { "world".into() } else { f }
}

/// A seed from what was typed: a number as it is, words hashed, nothing random.
pub fn parse_seed(s: &str) -> u64 {
    let s = s.trim();
    if s.is_empty() {
        return std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64);
    }
    s.parse::<u64>().unwrap_or_else(|_| {
        s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ b as u64).wrapping_mul(0x1b3)
        })
    })
}

fn language_name(ui: &Ui<'_>, code: &str) -> String {
    let key = format!("language.{code}");
    if ui.lang.has(&key) {
        ui.t(&key)
    } else {
        code.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_and_folders_from_what_was_typed() {
        assert_eq!(parse_seed("42"), 42);
        assert_eq!(parse_seed("hearth"), parse_seed("hearth"));
        assert_ne!(parse_seed("hearth"), parse_seed("Hearth"));
        assert_eq!(folder_name("My World!"), "My_World_");
        assert_eq!(folder_name("   "), "world");
    }
}

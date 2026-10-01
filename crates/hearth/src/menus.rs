//! The screens (v1 §11, the minimal set for V2-3; the full flow is V2-15): the title, the
//! character (people kept as profiles, made with a turning preview), the worlds (pick one or
//! make a new one with a person), pause, and the options — video, sound, controls with
//! rebinding, language, accessibility. Every screen is drawn each frame with the widgets of `hearth_ui`
//! and says what the player chose.

use std::path::{Path, PathBuf};

use hearth_character::appearance::{MAX_HEIGHT_M, MIN_HEIGHT_M, from_hsl, skin_presets, to_hsl};
use hearth_character::{
    Appearance, BodyType, EyeColor, FacialHair, HAIR_COLORS, HairStyle, Loincloth,
};
use hearth_core::options::{DisplayMode, GraphicsPreset, Options, Quality};
use hearth_input::{ActionId, CaptureResult, InputKey, KeyBindings, RebindCapture};
use hearth_render::figure::PreviewLight;
use hearth_ui::widgets::theme;
use hearth_ui::{Column, Rect, Ui};

/// A world on disk.
#[derive(Debug, Clone)]
pub struct WorldEntry {
    /// Its folder in the saves (the name the server opens).
    pub folder: String,
    pub name: String,
    pub last_played_unix: u64,
    /// Ended with its character's death (permadeath).
    pub ended: bool,
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
                        ended: meta.ended,
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
        /// Which of the death rules (Legacy, Permadeath, Hardy).
        death: usize,
        /// How knowledge is gained (Discovery, Guided, Open).
        knowledge: usize,
    },
    /// After death: what the world's rules allow.
    Death,
    /// The journal (J): the tab open and how far it is scrolled.
    Journal {
        tab: u8,
        scroll: i32,
    },
    /// Knapping a stone by hand.
    Knapping(Box<crate::knapping_ui::KnapScreen>),
    /// What the player carries (Tab): a thing lifted onto the pointer, and whether it is turned.
    Inventory {
        lifted: Option<crate::inventory_ui::Lifted>,
        turned: bool,
    },
    Pause,
    Options,
    Video,
    Sound,
    Controls {
        capturing: Option<ActionId>,
        capture: RebindCapture,
    },
    Accessibility,
    /// Making people: which way the preview faces (radians), its light, and a drag in
    /// progress (the pointer's last x).
    Character {
        yaw: f32,
        light: usize,
        drag: Option<f32>,
    },
}

/// The person on the character screen, for the app to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    /// Where (interface pixels).
    pub rect: Rect,
    pub appearance: Appearance,
    pub yaw: f32,
    pub light: PreviewLight,
}

/// What the player chose.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction {
    /// Play a world (its folder; the seed is used if it is new).
    Play {
        folder: String,
        seed: u64,
        death_rules: hearth_save::DeathRules,
        knowledge: hearth_save::KnowledgeMode,
    },
    /// Live on after death (as this person, under Legacy).
    LiveOn(Option<Appearance>),
    /// Knapping is over: do the process, by hand (with the quality reached) or as usual.
    Knapped {
        process: String,
        aim: hearth_protocol::AimAt,
        hand: Option<f32>,
    },
    /// Move a carried thing.
    Shift {
        from: hearth_items::Path,
        count: Option<u16>,
        to: hearth_items::Target,
    },
    /// Put a carried thing down on the ground.
    PutDown {
        from: hearth_items::Path,
        count: Option<u16>,
    },
    Resume,
    QuitToTitle,
    QuitGame,
    /// The options changed: save and apply them.
    OptionsChanged,
    LanguageChanged,
    /// The people changed: save them.
    ProfilesChanged,
}

/// What the screens edit and need.
pub struct MenuContext<'a> {
    pub options: &'a mut Options,
    pub bindings: &'a mut KeyBindings,
    pub saves: PathBuf,
    pub in_game: bool,
    pub languages: &'a [String],
    /// The sound output devices there are.
    pub audio_devices: &'a [String],
    pub profiles: &'a mut crate::profiles::Profiles,
    /// The player's death, when there is one to face.
    pub death: Option<DeathInfo>,
    /// What the player carries, in a world.
    pub inventory: Option<crate::inventory_ui::InventoryView<'a>>,
    /// What the player knows, in a world.
    pub journal: Option<crate::journal_ui::JournalView<'a>>,
}

/// What the death screen says.
#[derive(Debug, Clone, PartialEq)]
pub struct DeathInfo {
    pub words: String,
    pub rules: hearth_save::DeathRules,
    /// The life's tale, when it ended the world.
    pub summary: Option<Vec<String>>,
}

const KNOWLEDGE_MODES: [hearth_save::KnowledgeMode; 3] = [
    hearth_save::KnowledgeMode::Discovery,
    hearth_save::KnowledgeMode::Guided,
    hearth_save::KnowledgeMode::Open,
];

const DEATH_RULES: [hearth_save::DeathRules; 3] = [
    hearth_save::DeathRules::Legacy,
    hearth_save::DeathRules::Permadeath,
    hearth_save::DeathRules::Hardy,
];

/// The open screens, the top one shown.
pub struct Menus {
    stack: Vec<Screen>,
    /// The character screen's person this frame.
    preview: Option<Preview>,
}

const W: f32 = 220.0;
const ROW: f32 = 18.0;

impl Menus {
    pub fn title() -> Self {
        Self {
            stack: vec![Screen::Title],
            preview: None,
        }
    }

    pub fn none() -> Self {
        Self {
            stack: Vec::new(),
            preview: None,
        }
    }

    /// The person shown on the character screen this frame, if it is open.
    pub fn preview(&self) -> Option<&Preview> {
        self.preview.as_ref()
    }

    fn character() -> Screen {
        Screen::Character {
            yaw: 0.4,
            light: 0,
            drag: None,
        }
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
            Some(Screen::Title) | Some(Screen::Death) | None => None,
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

    /// Whether the inventory is the screen shown.
    pub fn inventory_open(&self) -> bool {
        matches!(self.stack.last(), Some(Screen::Inventory { .. }))
    }

    pub fn journal_open(&self) -> bool {
        matches!(self.stack.last(), Some(Screen::Journal { .. }))
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
        self.preview = None;
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
                if ui.button(c.row(ROW), &ui.t("menu.title.character")) {
                    push = Some(Self::character());
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
                        let name = if w.ended {
                            format!("{} {}", w.name, ui.t("menu.worlds.ended"))
                        } else {
                            w.name.clone()
                        };
                        ui.label(r.x + 4.0, r.y + 2.0, &name, theme::TEXT);
                        ui.label(r.x + 4.0, r.y + 11.0, &w.folder, theme::DIM);
                    },
                );
                if entries.is_empty() {
                    let hint = ui.t("menu.worlds.none");
                    ui.text_centred(&list_r, &hint, theme::DIM);
                }
                if let Some(i) = clicked {
                    if *selected == Some(i) && !entries[i].ended {
                        // A second click plays it.
                        out.push(MenuAction::Play {
                            folder: entries[i].folder.clone(),
                            seed: 0,
                            death_rules: Default::default(),
                            knowledge: Default::default(),
                        });
                    }
                    *selected = Some(i);
                }
                let mut c = Column::new(x - 40.0, size.1 - 60.0, W + 80.0);
                let row = c.row(ROW);
                let (a, rest) = row.split_left((W + 80.0 - 8.0) / 3.0, 4.0);
                let (b, d) = rest.split_left((W + 80.0 - 8.0) / 3.0, 4.0);
                let playable = selected.is_some_and(|i| !entries[i].ended);
                if ui.button_enabled(a, &ui.t("menu.worlds.play"), playable)
                    && let Some(i) = *selected
                {
                    out.push(MenuAction::Play {
                        folder: entries[i].folder.clone(),
                        seed: 0,
                        death_rules: Default::default(),
                        knowledge: Default::default(),
                    });
                }
                if ui.button(b, &ui.t("menu.worlds.new")) {
                    push = Some(Screen::NewWorld {
                        name: String::new(),
                        seed: String::new(),
                        death: 0,
                        knowledge: 0,
                    });
                }
                if ui.button(d, &ui.t("menu.back")) {
                    pop = true;
                }
            }
            Screen::NewWorld {
                name,
                seed,
                death,
                knowledge,
            } => {
                ui.title(30.0, &ui.t("menu.new_world.title"));
                let mut c = Column::new(x, 60.0, W);
                ui.label(x, c.y, &ui.t("menu.new_world.name"), theme::DIM);
                c.space(10.0);
                ui.text_field(c.row(ROW), &ui.t("menu.new_world.name_hint"), name, 32);
                c.space(6.0);
                ui.label(x, c.y, &ui.t("menu.new_world.seed"), theme::DIM);
                c.space(10.0);
                ui.text_field(c.row(ROW), &ui.t("menu.new_world.seed_hint"), seed, 20);
                c.space(6.0);
                // Who lives there.
                let unnamed = ui.t("menu.character.unnamed");
                let people: Vec<String> = (0..cx.profiles.list.len())
                    .map(|i| cx.profiles.name(i, &unnamed))
                    .collect();
                let row = c.row(ROW);
                let (pick, edit) = row.split_left(W - 64.0, 4.0);
                let mut who = cx.profiles.selected;
                if ui.cycle(pick, &ui.t("menu.new_world.character"), &people, &mut who) {
                    cx.profiles.selected = who;
                    out.push(MenuAction::ProfilesChanged);
                }
                if ui.button(edit, &ui.t("menu.new_world.edit_character")) {
                    push = Some(Self::character());
                }
                let rules: Vec<String> = ["rules.legacy", "rules.permadeath", "rules.hardy"]
                    .iter()
                    .map(|k| ui.t(k))
                    .collect();
                ui.cycle(c.row(ROW), &ui.t("menu.new_world.death"), &rules, death);
                let modes: Vec<String> = [
                    "menu.knowledge.discovery",
                    "menu.knowledge.guided",
                    "menu.knowledge.open",
                ]
                .iter()
                .map(|k| ui.t(k))
                .collect();
                ui.cycle(
                    c.row(ROW),
                    &ui.t("menu.new_world.knowledge"),
                    &modes,
                    knowledge,
                );
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
                        death_rules: DEATH_RULES[(*death).min(DEATH_RULES.len() - 1)],
                        knowledge: KNOWLEDGE_MODES[(*knowledge).min(KNOWLEDGE_MODES.len() - 1)],
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
                if ui.button(c.row(ROW), &ui.t("menu.options.sound")) {
                    push = Some(Screen::Sound);
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
                {
                    use hearth_core::options::AntiAliasing;
                    let modes = [AntiAliasing::Off, AntiAliasing::Taa];
                    let names = [ui.t("menu.video.aa.off"), ui.t("menu.video.aa.taa")];
                    let mut ai = usize::from(v.anti_aliasing == AntiAliasing::Taa);
                    if ui.cycle(
                        c.row(ROW),
                        &ui.t("menu.video.anti_aliasing"),
                        &names,
                        &mut ai,
                    ) {
                        v.anti_aliasing = modes[ai];
                        v.refresh_preset();
                        changed = true;
                    }
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
            Screen::Sound => {
                ui.title(30.0, &ui.t("menu.options.sound"));
                let s = &mut cx.options.sound;
                let wide = W + 120.0;
                let mut c = Column::new(((size.0 - wide) / 2.0).round(), 56.0, wide);
                c.gap = 3.0;
                let mut changed = false;
                // The categories that have sounds so far.
                for (key, value) in [
                    ("master", &mut s.master),
                    ("weather", &mut s.weather),
                    ("players", &mut s.players),
                    ("ambient", &mut s.ambient),
                    ("ui", &mut s.ui),
                ] {
                    let mut v = *value;
                    let text = if v <= 0.0 {
                        ui.t("ui.off")
                    } else {
                        format!("{}%", (v * 100.0).round())
                    };
                    let label = ui.t(&format!("menu.sound.{key}"));
                    if ui.slider(c.row(ROW), &label, &mut v, 0.0, 1.0, &text) {
                        *value = v;
                        changed = true;
                    }
                }
                let mut names = vec![ui.t("menu.sound.device.default")];
                names.extend(cx.audio_devices.iter().cloned());
                let mut i = cx
                    .audio_devices
                    .iter()
                    .position(|d| *d == s.device)
                    .map_or(0, |p| p + 1);
                if ui.cycle(c.row(ROW), &ui.t("menu.sound.device"), &names, &mut i) {
                    s.device = match i {
                        0 => String::new(),
                        i => cx.audio_devices[i - 1].clone(),
                    };
                    changed = true;
                }
                let mut captions = s.subtitles;
                if ui.toggle(c.row(ROW), &ui.t("menu.sound.subtitles"), &mut captions) {
                    s.subtitles = captions;
                    changed = true;
                }
                c.space(8.0);
                if ui.button(c.row(ROW), &ui.t("menu.done")) {
                    pop = true;
                }
                if changed {
                    out.push(MenuAction::OptionsChanged);
                }
            }
            Screen::Death => {
                death_screen(ui, cx, &mut out, &mut push);
            }
            Screen::Knapping(k) => {
                if let Some(done) = crate::knapping_ui::knapping_screen(ui, k) {
                    use crate::knapping_ui::KnapDone;
                    let hand = match done {
                        KnapDone::Finished(q) => Some(Some(q)),
                        KnapDone::Habit => Some(None),
                        KnapDone::Leave => None,
                    };
                    if let Some(hand) = hand {
                        out.push(MenuAction::Knapped {
                            process: k.process.clone(),
                            aim: k.aim,
                            hand,
                        });
                    }
                    pop = true;
                }
            }
            Screen::Journal { tab, scroll } => match &cx.journal {
                Some(view) => {
                    pop |= crate::journal_ui::journal_screen(ui, view, tab, scroll);
                }
                None => pop = true,
            },
            Screen::Inventory { lifted, turned } => match &cx.inventory {
                Some(view) => {
                    crate::inventory_ui::inventory_screen(ui, view, lifted, turned, &mut out);
                }
                None => pop = true,
            },
            Screen::Character { yaw, light, drag } => {
                let (preview, changed, done) = character_screen(ui, cx.profiles, yaw, light, drag);
                self.preview = Some(preview);
                if changed {
                    out.push(MenuAction::ProfilesChanged);
                }
                pop |= done;
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
                if ui.toggle(
                    c.row(ROW),
                    &ui.t("menu.accessibility.guided_hud"),
                    &mut a.guided_hud,
                ) {
                    out.push(MenuAction::OptionsChanged);
                }
                if ui.toggle(
                    c.row(ROW),
                    &ui.t("menu.accessibility.reduce_motion"),
                    &mut a.reduce_motion,
                ) {
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

/// The character screen: the people on the left, the one chosen turning in the middle, what
/// they look like on the right. Returns the preview, whether the people changed, and whether
/// the screen is done.
fn character_screen(
    ui: &mut Ui<'_>,
    profiles: &mut crate::profiles::Profiles,
    yaw: &mut f32,
    light: &mut usize,
    drag: &mut Option<f32>,
) -> (Preview, bool, bool) {
    let size = ui.size;
    let mut changed = false;
    let mut done = false;
    ui.title(8.0, &ui.t("menu.character.title"));
    // The people.
    let list_w = 120.0;
    let list_r = Rect::new(8.0, 24.0, list_w, (size.1 - 24.0 - 52.0).max(40.0));
    let unnamed = ui.t("menu.character.unnamed");
    let names: Vec<String> = (0..profiles.list.len())
        .map(|i| profiles.name(i, &unnamed))
        .collect();
    if let Some(i) = ui.list(
        list_r,
        "people",
        names.len(),
        14.0,
        Some(profiles.selected),
        |ui, r, i, _| {
            ui.label(r.x + 3.0, r.y + 3.0, &names[i], theme::TEXT);
        },
    ) && i != profiles.selected
    {
        profiles.selected = i;
        changed = true;
    }
    let mut c = Column::new(8.0, size.1 - 46.0, list_w);
    let (a, b) = c.row(ROW).split_left((list_w - 4.0) / 2.0, 4.0);
    if ui.button(a, &ui.t("menu.character.new")) {
        let n = profiles.list.len();
        profiles.list.push(if n % 2 == 1 {
            Appearance::female()
        } else {
            Appearance::default()
        });
        profiles.selected = n;
        changed = true;
    }
    if ui.button_enabled(b, &ui.t("menu.character.delete"), profiles.list.len() > 1) {
        profiles.list.remove(profiles.selected);
        profiles.selected = profiles.selected.min(profiles.list.len() - 1);
        changed = true;
    }
    if ui.button(c.row(ROW), &ui.t("menu.done")) {
        done = true;
    }
    // What they look like.
    let cw = 200.0;
    let mut c = Column::new(size.0 - cw - 8.0, 24.0, cw);
    c.gap = 2.0;
    let a = profiles.current_mut();
    let before = a.clone();
    ui.text_field(c.row(ROW), &ui.t("menu.character.name"), &mut a.name, 32);
    let bodies = [BodyType::Female, BodyType::Male];
    let body_names: Vec<String> = ["character.body.female", "character.body.male"]
        .iter()
        .map(|k| ui.t(k))
        .collect();
    let mut i = bodies.iter().position(|b| *b == a.body).unwrap_or(1);
    if ui.cycle(
        c.row(ROW),
        &ui.t("menu.character.body"),
        &body_names,
        &mut i,
    ) {
        a.body = bodies[i];
    }
    let text = format!("{:.2} m", a.height_m);
    ui.slider(
        c.row(ROW),
        &ui.t("menu.character.height"),
        &mut a.height_m,
        MIN_HEIGHT_M,
        MAX_HEIGHT_M,
        &text,
    );
    let build = ["slight", "lean", "average", "sturdy", "heavy"][((a.build * 5.0) as usize).min(4)];
    let text = ui.t(&format!("character.build.{build}"));
    ui.slider(
        c.row(ROW),
        &ui.t("menu.character.build"),
        &mut a.build,
        0.0,
        1.0,
        &text,
    );
    let presets = skin_presets();
    let nearest = presets
        .iter()
        .enumerate()
        .min_by(|x, y| {
            (x.1 - a.skin_tone)
                .abs()
                .total_cmp(&(y.1 - a.skin_tone).abs())
        })
        .map_or(0, |(i, _)| i);
    let text = ui
        .lang
        .format("character.skin.tone", &[("n", &(nearest + 1).to_string())]);
    ui.slider(
        c.row(ROW),
        &ui.t("menu.character.skin"),
        &mut a.skin_tone,
        0.0,
        1.0,
        &text,
    );
    let under = if a.undertone < -0.33 {
        "cool"
    } else if a.undertone > 0.33 {
        "warm"
    } else {
        "neutral"
    };
    let text = ui.t(&format!("character.undertone.{under}"));
    ui.slider(
        c.row(ROW),
        &ui.t("menu.character.undertone"),
        &mut a.undertone,
        -1.0,
        1.0,
        &text,
    );
    let styles: Vec<String> = HairStyle::ALL.iter().map(|h| ui.t(h.key())).collect();
    let mut i = HairStyle::ALL
        .iter()
        .position(|h| *h == a.hair)
        .unwrap_or(0);
    if ui.cycle(c.row(ROW), &ui.t("menu.character.hair"), &styles, &mut i) {
        a.hair = HairStyle::ALL[i];
    }
    // Natural colours, then the colour finely.
    let mut colours: Vec<String> = HAIR_COLORS.iter().map(|(k, _)| ui.t(k)).collect();
    let matching = HAIR_COLORS.iter().position(|(_, c)| *c == a.hair_color);
    colours.push(ui.t("character.hair_color.custom"));
    let mut i = matching.unwrap_or(HAIR_COLORS.len());
    if ui.cycle(
        c.row(ROW),
        &ui.t("menu.character.hair_color"),
        &colours,
        &mut i,
    ) && i < HAIR_COLORS.len()
    {
        a.hair_color = HAIR_COLORS[i].1;
    }
    let [mut h, mut s, mut l] = to_hsl(a.hair_color);
    let text = format!("{h:.0}°");
    let mut fine = ui.slider(
        c.row(ROW),
        &ui.t("menu.character.hue"),
        &mut h,
        0.0,
        360.0,
        &text,
    );
    let text = format!("{:.0}%", s * 100.0);
    fine |= ui.slider(
        c.row(ROW),
        &ui.t("menu.character.saturation"),
        &mut s,
        0.0,
        1.0,
        &text,
    );
    let text = format!("{:.0}%", l * 100.0);
    fine |= ui.slider(
        c.row(ROW),
        &ui.t("menu.character.lightness"),
        &mut l,
        0.02,
        0.95,
        &text,
    );
    if fine {
        a.hair_color = from_hsl([h, s, l]);
    }
    let facial: Vec<String> = FacialHair::ALL.iter().map(|f| ui.t(f.key())).collect();
    let mut i = FacialHair::ALL
        .iter()
        .position(|f| *f == a.facial_hair)
        .unwrap_or(0);
    if ui.cycle(
        c.row(ROW),
        &ui.t("menu.character.facial_hair"),
        &facial,
        &mut i,
    ) {
        a.facial_hair = FacialHair::ALL[i];
    }
    let eyes: Vec<String> = EyeColor::ALL.iter().map(|e| ui.t(e.key())).collect();
    let mut i = EyeColor::ALL.iter().position(|e| *e == a.eyes).unwrap_or(0);
    if ui.cycle(c.row(ROW), &ui.t("menu.character.eyes"), &eyes, &mut i) {
        a.eyes = EyeColor::ALL[i];
    }
    let cloths = [Loincloth::Hide, Loincloth::PlantFibre];
    let cloth_names: Vec<String> = ["character.loincloth.hide", "character.loincloth.fibre"]
        .iter()
        .map(|k| ui.t(k))
        .collect();
    let mut i = cloths.iter().position(|x| *x == a.loincloth).unwrap_or(0);
    if ui.cycle(
        c.row(ROW),
        &ui.t("menu.character.loincloth"),
        &cloth_names,
        &mut i,
    ) {
        a.loincloth = cloths[i];
    }
    if *a != before {
        changed = true;
    }
    // The person between, turned by dragging across them.
    let px = list_w + 16.0;
    let pw = (size.0 - cw - 16.0 - px).max(40.0);
    let rect = Rect::new(px, 24.0, pw, (size.1 - 24.0 - 30.0).max(40.0));
    match ui.input.pointer {
        Some(p) if ui.input.down && (drag.is_some() || rect.contains(p)) => {
            if let Some(last) = *drag {
                *yaw += (p.0 - last) * 0.03;
            }
            *drag = Some(p.0);
        }
        _ => *drag = None,
    }
    let lights: Vec<String> = PreviewLight::ALL.iter().map(|l| ui.t(l.key())).collect();
    let lr = Rect::new((px + pw / 2.0 - 90.0).round(), size.1 - 26.0, 180.0, ROW);
    ui.cycle(lr, &ui.t("menu.character.light"), &lights, light);
    let preview = Preview {
        rect,
        appearance: profiles.current().clone(),
        yaw: *yaw,
        light: PreviewLight::ALL[(*light).min(PreviewLight::ALL.len() - 1)],
    };
    (preview, changed, done)
}

/// After death: how it happened, and what the world's rules allow — live on as someone new
/// (Legacy), live again (Hardy), or the tale of the life that ended the world (permadeath).
fn death_screen(
    ui: &mut Ui<'_>,
    cx: &mut MenuContext<'_>,
    out: &mut Vec<MenuAction>,
    push: &mut Option<Screen>,
) {
    let size = ui.size;
    let Some(d) = cx.death.clone() else {
        // Alive again: nothing to face.
        return;
    };
    let x = ((size.0 - W) / 2.0).round();
    ui.title((size.1 * 0.22).round(), &d.words);
    let mut c = Column::new(x, (size.1 * 0.22 + 20.0).round(), W);
    let rules = match d.rules {
        hearth_save::DeathRules::Legacy => "body.death.legacy",
        hearth_save::DeathRules::Hardy => "body.death.hardy",
        hearth_save::DeathRules::Permadeath => "body.death.permadeath",
    };
    for line in ui.font.wrap(&ui.t(rules), (W + 80.0) as u32) {
        let lw = ui.font.width(&line) as f32;
        ui.label(((size.0 - lw) / 2.0).round(), c.y, &line, theme::DIM);
        c.space(hearth_ui::font::LINE as f32);
    }
    c.space(8.0);
    match d.rules {
        hearth_save::DeathRules::Legacy => {
            let unnamed = ui.t("menu.character.unnamed");
            let people: Vec<String> = (0..cx.profiles.list.len())
                .map(|i| cx.profiles.name(i, &unnamed))
                .collect();
            let mut who = cx.profiles.selected;
            if ui.cycle(c.row(ROW), &ui.t("menu.death.as"), &people, &mut who) {
                cx.profiles.selected = who;
                out.push(MenuAction::ProfilesChanged);
            }
            if ui.button(c.row(ROW), &ui.t("menu.death.live_on")) {
                out.push(MenuAction::LiveOn(Some(cx.profiles.current().clone())));
            }
            if ui.button(c.row(ROW), &ui.t("menu.death.someone_new")) {
                *push = Some(Menus::character());
            }
        }
        hearth_save::DeathRules::Hardy => {
            if ui.button(c.row(ROW), &ui.t("menu.death.live_again")) {
                out.push(MenuAction::LiveOn(None));
            }
        }
        hearth_save::DeathRules::Permadeath => {
            for line in d.summary.iter().flatten() {
                let lw = ui.font.width(line) as f32;
                ui.label(((size.0 - lw) / 2.0).round(), c.y, line, theme::TEXT);
                c.space(hearth_ui::font::LINE as f32 + 1.0);
            }
            c.space(6.0);
        }
    }
    c.space(6.0);
    if ui.button(c.row(ROW), &ui.t("menu.death.to_title")) {
        out.push(MenuAction::QuitToTitle);
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

//! The screens (v1 §11, the minimal set for V2-3; the full flow is V2-15): the title, the worlds
//! (pick one, or make a new one and say what is wished of the birth in it: a name, a daughter or
//! a son — never looks, which come from the parents: V2.1 Addendum A), the birth shown, pause,
//! death, and the options — video, sound, controls with rebinding, language, accessibility. Every screen is drawn each frame with the widgets of `hearth_ui`
//! and says what the player chose.

use std::path::{Path, PathBuf};

use hearth_core::options::{DisplayMode, GraphicsPreset, Options, Quality};
use hearth_input::{ActionId, CaptureResult, InputKey, KeyBindings, RebindCapture};
use hearth_ui::widgets::theme;
use hearth_ui::{Column, Rect, Ui};

use crate::profiles::Born;

/// A question the Worlds screen is asking about the world chosen.
#[derive(Debug, Clone, PartialEq)]
pub enum WorldsAsk {
    /// Its new name, being typed.
    Rename(String),
    /// Whether to delete it (to the trash).
    Delete,
    /// Its game mode changed to one less strict (Amendment P §2): which, in Create World's
    /// order.
    Mode(usize),
}

/// What Creative's time and weather panel shows chosen (indices into its choices).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TimeWeather {
    pub hour: usize,
    pub speed: usize,
    pub weather: usize,
    pub wind: usize,
    pub temperature: usize,
}

/// The panel's choices: the hour to go on to, the time's speed (multiples of lived time; 0
/// stopped), the weather held, the wind (m/s) and the air's temperature (°C).
pub const HOURS: [Option<f64>; 7] = [
    None,
    Some(6.0),
    Some(9.0),
    Some(12.0),
    Some(16.0),
    Some(19.0),
    Some(23.0),
];
pub const SPEEDS: [f64; 5] = [1.0, 0.0, 10.0, 60.0, 600.0];
pub const WEATHERS: [&str; 6] = ["as_it_is", "clear", "cloudy", "rain", "downpour", "snow"];
pub const WINDS: [Option<f64>; 5] = [None, Some(0.0), Some(5.0), Some(12.0), Some(25.0)];
pub const TEMPERATURES: [Option<f64>; 6] = [
    None,
    Some(-15.0),
    Some(0.0),
    Some(10.0),
    Some(20.0),
    Some(32.0),
];

/// The weather the panel's choices hold (none: the weather goes its own way).
pub fn weather_hold(t: &TimeWeather) -> Option<hearth_env::weather::WeatherHold> {
    use hearth_env::weather::WeatherHold;
    let mut h = match WEATHERS[t.weather.min(WEATHERS.len() - 1)] {
        "clear" => WeatherHold {
            humidity: Some(0.35),
            precip_mm_h: Some(0.0),
            ..Default::default()
        },
        "cloudy" => WeatherHold {
            humidity: Some(0.85),
            precip_mm_h: Some(0.0),
            ..Default::default()
        },
        "rain" => WeatherHold {
            humidity: Some(0.95),
            precip_mm_h: Some(3.0),
            ..Default::default()
        },
        "downpour" => WeatherHold {
            humidity: Some(1.0),
            precip_mm_h: Some(25.0),
            ..Default::default()
        },
        "snow" => WeatherHold {
            humidity: Some(0.95),
            precip_mm_h: Some(2.0),
            temperature_c: Some(-4.0),
            ..Default::default()
        },
        _ => WeatherHold::default(),
    };
    if let Some(w) = WINDS[t.wind.min(WINDS.len() - 1)] {
        h.wind_speed_m_s = Some(w);
    }
    if let Some(c) = TEMPERATURES[t.temperature.min(TEMPERATURES.len() - 1)] {
        h.temperature_c = Some(c);
    }
    (h != WeatherHold::default()).then_some(h)
}

pub enum Screen {
    Title,
    /// The worlds on disk (Amendment P §4.2): the one chosen, a question asked about it, and
    /// what came of the last thing done.
    Worlds {
        list: Vec<crate::worlds::WorldInfo>,
        selected: Option<usize>,
        ask: Option<WorldsAsk>,
        note: Option<String>,
    },
    /// The worlds deleted in the last thirty days.
    Trash {
        list: Vec<crate::worlds::Trashed>,
        selected: Option<usize>,
    },
    /// Create World (Amendment P §4.3): its name and seed, the era, and — shown on asking — the
    /// world's shape.
    NewWorld {
        name: String,
        seed: String,
        /// Which of the eras there are to play (V2.1 §15.3).
        era: usize,
        /// Whether More options are shown, the planet's size among the presets, and the rest of
        /// the world's shape.
        more: bool,
        size: usize,
        shape: crate::server::WorldShape,
        /// Which of the game modes (Amendment P §2), in Create World's order.
        mode: usize,
    },
    /// The planet being made: what is being done, and how far it has come (0–1).
    Making {
        stage: String,
        share: f32,
    },
    /// Where to begin, chosen on the globe (Amendment P §4.3): the world asked for, and the
    /// place chosen (latitude, longitude in radians).
    Birthplace {
        choice: NewWorldChoice,
        chosen: Option<(f32, f32)>,
    },
    /// After death: a new life, or the world begun again.
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
    /// Creative's time and weather (Amendment P §3.1): each a choice among a few, set as it is
    /// chosen.
    TimeWeather(TimeWeather),
    /// Creative's inventory (Amendment P §3.1).
    Creative(crate::creative_ui::CreativeScreen),
    /// Creative's clear view and its parts (Amendment P §3.2).
    ClearView,
    Options,
    /// The video settings, by the group shown.
    Video {
        tab: usize,
    },
    Sound,
    Controls {
        capturing: Option<ActionId>,
        capture: RebindCapture,
    },
    Accessibility,
}

/// A new world as Create World asked for it.
#[derive(Debug, Clone, PartialEq)]
pub struct NewWorldChoice {
    pub folder: String,
    pub seed: u64,
    /// Its era's id.
    pub era: String,
    pub size: hearth_math::PlanetSize,
    pub shape: crate::server::WorldShape,
    /// Its game mode's id (Amendment P §2).
    pub mode: String,
}

/// The planet sizes Create World offers, the standard first among them by its place.
const SIZES: [(hearth_math::PlanetSize, &str); 5] = [
    (hearth_math::PlanetSize::Small, "menu.new_world.size.small"),
    (
        hearth_math::PlanetSize::Standard,
        "menu.new_world.size.standard",
    ),
    (hearth_math::PlanetSize::Large, "menu.new_world.size.large"),
    (hearth_math::PlanetSize::Huge, "menu.new_world.size.huge"),
    (hearth_math::PlanetSize::Vast, "menu.new_world.size.vast"),
];

impl Screen {
    /// A fresh Create World.
    pub fn new_world() -> Self {
        Screen::NewWorld {
            name: String::new(),
            seed: String::new(),
            era: 0,
            more: false,
            size: 1,
            shape: Default::default(),
            mode: usize::MAX,
        }
    }

    /// The Worlds screen, with the worlds in `saves`.
    pub fn worlds(saves: &Path) -> Self {
        Screen::Worlds {
            list: crate::worlds::list(saves),
            selected: None,
            ask: None,
            note: None,
        }
    }
}

/// What the player chose.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction {
    /// Play a world (its folder; the seed is used if it is new).
    Play {
        folder: String,
        seed: u64,
        knowledge: hearth_save::KnowledgeMode,
        /// A new world's era (its id).
        era: String,
        /// A new world's planet size and shape.
        size: hearth_math::PlanetSize,
        shape: crate::server::WorldShape,
        /// Where a new world's first life begins (world x, z), chosen on the globe.
        birthplace: Option<glam::DVec2>,
        /// A new world's game mode (a saved world keeps its own).
        mode: Option<String>,
    },
    /// Make a new world's planet, then choose where to begin on it.
    CreateWorld(NewWorldChoice),
    /// Creative: a thing of the inventory taken, placed or summoned.
    Creative(crate::creative_ui::CreativeAct),
    /// Creative's instant actions on or off.
    Instant(bool),
    /// Creative's clear view as set.
    ClearView(crate::clear_view::ClearView),
    /// Creative: on to an hour of the day (local).
    GoToHour(f64),
    /// Creative: time at a multiple of lived time (0: stopped).
    TimeSpeed(f64),
    /// Creative: the weather held so (none: let it go).
    Weather(Option<hearth_env::weather::WeatherHold>),
    /// A world's game mode changed (toward less strict; the world list is read again).
    ChangeMode {
        folder: String,
        mode: String,
    },
    /// Making the planet given up: back to Create World.
    CancelCreate,
    /// After death: a new life near where the player last lived, or (`elsewhere`) about a place
    /// picked on the globe (Amendment E §6.6).
    NewLife {
        elsewhere: bool,
    },
    /// After death: begin this world again (the old one archived).
    Restart,
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
    /// Eat one of a carried thing.
    Eat(hearth_items::Path),
    Resume,
    QuitToTitle,
    QuitGame,
    /// The options changed: save and apply them.
    OptionsChanged,
    LanguageChanged,
    /// Who the player begins as changed: save it.
    ProfilesChanged,
    /// Spectate (Creative): the player's body put aside, the eye free.
    Watch,
    /// Save the world now (it goes on).
    Save,
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
    /// Who the player begins as.
    pub profiles: &'a mut crate::profiles::Profiles,
    /// The player's death, when there is one to face.
    pub death: Option<DeathInfo>,
    /// What the player carries, in a world.
    pub inventory: Option<crate::inventory_ui::InventoryView<'a>>,
    /// What the player knows, in a world.
    pub journal: Option<crate::journal_ui::JournalView<'a>>,
    /// The eras a new world may be made in (id, name, how its people live), playable first.
    pub eras: Vec<(String, String, String)>,
    /// The game modes (id, name, summary) in Create World's order (Amendment P §2).
    pub modes: Vec<(String, String, String)>,
    /// The globe of a new world's planet, while its birthplace is chosen.
    pub globe: Option<GlobeContext<'a>>,
    /// The time of day and the year in words, in a world.
    pub time_words: Option<String>,
    /// Whether watching the world is open (Creative, or a world of no mode), and whether the
    /// world is in Creative (Amendment P §2).
    pub may_watch: bool,
    pub creative: bool,
    /// Creative's inventory: everything there is (empty outside Creative), and whether its
    /// actions are instant.
    pub catalog: &'a [crate::creative::Entry],
    pub instant: bool,
    /// Creative's clear view as it is.
    pub clear_view: crate::clear_view::ClearView,
}

/// The globe a birthplace is chosen on, and the planet it shows.
pub struct GlobeContext<'a> {
    pub picker: &'a mut crate::globe::GlobePicker,
    pub terrain: std::sync::Arc<hearth_worldgen::region::Terrain>,
}

/// What the death screen says: how the player died, and what a new life keeps.
#[derive(Debug, Clone, PartialEq)]
pub struct DeathInfo {
    pub words: String,
    pub after_death: hearth_save::AfterDeath,
}

/// The open screens, the top one shown.
pub struct Menus {
    stack: Vec<Screen>,
}

const W: f32 = 220.0;
const ROW: f32 = 18.0;
/// Where a page's rows begin, under its title.
const PAGE_TOP: f32 = 28.0;

/// A page of rows (Amendment P §4.1): its title, its rows in a column `wide` across from `top`
/// that scrolls when the window is too short for them, and its footer (Done, Back) at the
/// window's bottom, always in the same place. Returns the footer's row.
fn page(
    ui: &mut Ui<'_>,
    title: &str,
    id: &str,
    wide: f32,
    top: f32,
    gap: f32,
    body: impl FnOnce(&mut Ui<'_>, &mut Column),
) -> Rect {
    page_footed(ui, title, id, wide, top, gap, 1, body)
}

/// A page whose footer has `rows` rows (the first returned; the next each `ROW + 4` below).
#[allow(clippy::too_many_arguments)]
fn page_footed(
    ui: &mut Ui<'_>,
    title: &str,
    id: &str,
    wide: f32,
    top: f32,
    gap: f32,
    rows: usize,
    body: impl FnOnce(&mut Ui<'_>, &mut Column),
) -> Rect {
    let size = ui.size;
    ui.title(12.0, title);
    let x = ((size.0 - wide) / 2.0).round();
    let room = (size.1 - top - rows as f32 * (ROW + 4.0) - 12.0).max(ROW);
    let area = Rect::new(x, top, wide, room);
    ui.scroll(area, id, gap, body);
    Rect::new(x, (area.y + area.h + 8.0).round(), wide, ROW)
}

impl Menus {
    pub fn title() -> Self {
        Self {
            stack: vec![Screen::Title],
        }
    }

    pub fn none() -> Self {
        Self { stack: Vec::new() }
    }

    /// The screen shown, to update.
    pub fn top_mut(&mut self) -> Option<&mut Screen> {
        self.stack.last_mut()
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
        let Some(top) = self.stack.last_mut() else {
            return out;
        };
        let mut push: Option<Screen> = None;
        let mut pop = false;
        let size = ui.size;
        // A veil over the world behind the menus (the globe a birthplace is chosen on shows).
        if matches!(top, Screen::Birthplace { .. }) {
        } else if cx.in_game {
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
                    push = Some(Screen::worlds(&cx.saves));
                }
                if ui.button(c.row(ROW), &ui.t("menu.options")) {
                    push = Some(Screen::Options);
                }
                if ui.button(c.row(ROW), &ui.t("menu.title.quit")) {
                    out.push(MenuAction::QuitGame);
                }
            }
            Screen::Worlds {
                list,
                selected,
                ask,
                note,
            } => {
                ui.title(12.0, &ui.t("menu.worlds.title"));
                let wide = (W + 120.0).min(size.0 - 16.0);
                let xw = ((size.0 - wide) / 2.0).round();
                // Three rows of buttons below the list, and a line for what came of the last.
                let foot_h = 3.0 * (ROW + 4.0) + 12.0;
                let list_r = Rect::new(
                    xw,
                    PAGE_TOP,
                    wide,
                    (size.1 - PAGE_TOP - foot_h - 8.0).max(30.0),
                );
                let entries = list.clone();
                let eras = cx.eras.clone();
                let modes = cx.modes.clone();
                let bare = |id: &str| id.rsplit(':').next().unwrap_or(id).to_owned();
                // A world's mode's place in Create World's order (less strict first); a world from
                // before the modes was played as Realistic.
                let mode_of = |w: &crate::worlds::WorldInfo| {
                    let id = w
                        .mode
                        .clone()
                        .unwrap_or_else(|| crate::modes::DEFAULT.to_owned());
                    modes.iter().position(|m| bare(&m.0) == bare(&id))
                };
                let in_creative = ui.t("menu.worlds.played_in_creative");
                let played_label = ui.t("menu.worlds.played");
                let clicked = ui.list(
                    list_r,
                    "worlds",
                    entries.len(),
                    22.0,
                    *selected,
                    |ui, r, i, _| {
                        let w = &entries[i];
                        let era = eras
                            .iter()
                            .find(|e| e.0 == w.era)
                            .map_or_else(|| w.era.clone(), |e| e.1.clone());
                        let name = if w.ended {
                            format!("{} {}", w.name, ui.t("menu.worlds.ended"))
                        } else {
                            w.name.clone()
                        };
                        let (y, m, d, _, _) = crate::worlds::civil(w.last_played_unix);
                        let mode = mode_of(w).map_or_else(String::new, |i| {
                            if w.played_in_creative && !bare(&modes[i].0).eq("creative") {
                                format!("{} ({in_creative}) · ", modes[i].1)
                            } else {
                                format!("{} · ", modes[i].1)
                            }
                        });
                        let mut second = format!(
                            "{mode}{era} · {played_label} {} · {y:04}-{m:02}-{d:02}",
                            crate::worlds::played_words(w.played_s)
                        );
                        if let Some((who, age)) = &w.character {
                            let who = if who.is_empty() {
                                ui.t("menu.born.you")
                            } else {
                                who.clone()
                            };
                            second = format!("{who}, {:.0} · {second}", age.floor());
                        }
                        let fit = |ui: &Ui<'_>, t: String| {
                            let mut t = t;
                            while ui.font.width(&t) as f32 > r.w - 12.0 && t.pop().is_some() {}
                            t
                        };
                        let name = fit(ui, name);
                        let second = fit(ui, second);
                        ui.label(r.x + 4.0, r.y + 2.0, &name, theme::TEXT);
                        ui.label(r.x + 4.0, r.y + 12.0, &second, theme::DIM);
                    },
                );
                if entries.is_empty() {
                    let hint = ui.t("menu.worlds.none");
                    ui.text_centred(&list_r, &hint, theme::DIM);
                }
                if let Some(i) = clicked {
                    if *selected == Some(i) && !entries[i].ended && ask.is_none() {
                        // A second click plays it.
                        out.push(MenuAction::Play {
                            folder: entries[i].folder.clone(),
                            seed: 0,
                            knowledge: Default::default(),
                            era: crate::eras::WILD_EARTH.to_owned(),
                            size: hearth_math::PlanetSize::Standard,
                            shape: Default::default(),
                            birthplace: None,
                            mode: None,
                        });
                    }
                    *selected = Some(i);
                    *ask = None;
                }
                let chosen = selected.and_then(|i| entries.get(i)).cloned();
                let mut c = Column::new(xw, list_r.y + list_r.h + 6.0, wide);
                let thirds = |r: Rect| {
                    let w = ((r.w - 8.0) / 3.0).floor();
                    let (a, rest) = r.split_left(w, 4.0);
                    let (b, d) = rest.split_left(w, 4.0);
                    (a, b, d)
                };
                let saves = cx.saves.clone();
                let mut reload = false;
                match (ask.clone(), &chosen) {
                    (Some(WorldsAsk::Rename(mut name)), Some(w)) => {
                        ui.text_field(c.row(ROW), &ui.t("menu.worlds.new_name"), &mut name, 32);
                        let (a, b, _) = thirds(c.row(ROW));
                        if ui.button_enabled(
                            a,
                            &ui.t("menu.worlds.rename"),
                            !name.trim().is_empty(),
                        ) {
                            *note = crate::worlds::rename(&saves, &w.folder, &name)
                                .err()
                                .map(|e| e.to_string());
                            *ask = None;
                            reload = true;
                        } else if ui.button(b, &ui.t("menu.cancel")) {
                            *ask = None;
                        } else {
                            *ask = Some(WorldsAsk::Rename(name));
                        }
                    }
                    (Some(WorldsAsk::Mode(to)), Some(w)) => {
                        // Only toward less strict: the modes before its own.
                        let now = mode_of(w).unwrap_or(modes.len());
                        let names: Vec<String> =
                            modes.iter().take(now).map(|m| m.1.clone()).collect();
                        let mut to = to.min(names.len().saturating_sub(1));
                        if names.is_empty() {
                            ui.label(xw, c.y, &ui.t("menu.worlds.mode_none"), theme::DIM);
                            c.space(ROW);
                        } else {
                            ui.cycle(c.row(ROW), &ui.t("menu.new_world.mode"), &names, &mut to);
                            let warn = if bare(&modes[to].0) == "creative" {
                                ui.t("menu.worlds.mode_warn_creative")
                            } else {
                                ui.t("menu.worlds.mode_warn")
                            };
                            for l in ui.font.wrap(&warn, wide as u32).into_iter().take(2) {
                                ui.label(xw, c.y, &l, theme::WARN);
                                c.space(hearth_ui::font::LINE as f32);
                            }
                        }
                        let (a, b, _) = thirds(c.row(ROW));
                        if ui.button_enabled(a, &ui.t("menu.worlds.change"), !names.is_empty()) {
                            out.push(MenuAction::ChangeMode {
                                folder: w.folder.clone(),
                                mode: modes[to].0.clone(),
                            });
                            *ask = None;
                        } else if ui.button(b, &ui.t("menu.cancel")) {
                            *ask = None;
                        } else {
                            *ask = Some(WorldsAsk::Mode(to));
                        }
                    }
                    (Some(WorldsAsk::Delete), Some(w)) => {
                        let q = ui
                            .lang
                            .format("menu.worlds.delete_ask", &[("name", &w.name)]);
                        for l in ui.font.wrap(&q, wide as u32).into_iter().take(2) {
                            ui.label(xw, c.y, &l, theme::WARN);
                            c.space(hearth_ui::font::LINE as f32);
                        }
                        c.space(2.0);
                        let (a, b, _) = thirds(c.row(ROW));
                        if ui.button(a, &ui.t("menu.worlds.delete")) {
                            *note = crate::worlds::delete(
                                &saves,
                                &w.folder,
                                hearth_save::meta::unix_now(),
                            )
                            .err()
                            .map(|e| e.to_string());
                            *ask = None;
                            *selected = None;
                            reload = true;
                        }
                        if ui.button(b, &ui.t("menu.cancel")) {
                            *ask = None;
                        }
                    }
                    _ => {
                        *ask = None;
                        let some = chosen.is_some();
                        let playable = chosen.as_ref().is_some_and(|w| !w.ended);
                        let (a, b, d) = thirds(c.row(ROW));
                        if ui.button_enabled(a, &ui.t("menu.worlds.play"), playable)
                            && let Some(w) = &chosen
                        {
                            out.push(MenuAction::Play {
                                folder: w.folder.clone(),
                                seed: 0,
                                knowledge: Default::default(),
                                era: crate::eras::WILD_EARTH.to_owned(),
                                size: hearth_math::PlanetSize::Standard,
                                shape: Default::default(),
                                birthplace: None,
                                mode: None,
                            });
                        }
                        if ui.button(b, &ui.t("menu.worlds.new")) {
                            push = Some(Screen::new_world());
                        }
                        if ui.button(d, &ui.t("menu.back")) {
                            pop = true;
                        }
                        let (a, b, d) = thirds(c.row(ROW));
                        if ui.button_enabled(a, &ui.t("menu.worlds.rename"), some)
                            && let Some(w) = &chosen
                        {
                            *ask = Some(WorldsAsk::Rename(w.name.clone()));
                        }
                        if ui.button_enabled(b, &ui.t("menu.worlds.duplicate"), some)
                            && let Some(w) = &chosen
                        {
                            *note = Some(match crate::worlds::duplicate(&saves, &w.folder) {
                                Ok(f) => {
                                    ui.lang.format("menu.worlds.duplicated", &[("folder", &f)])
                                }
                                Err(e) => e.to_string(),
                            });
                            reload = true;
                        }
                        if ui.button_enabled(d, &ui.t("menu.worlds.back_up"), some)
                            && let Some(w) = &chosen
                        {
                            *note = Some(
                                match crate::worlds::back_up(
                                    &saves,
                                    &w.folder,
                                    hearth_save::meta::unix_now(),
                                ) {
                                    Ok(p) => ui.lang.format(
                                        "menu.worlds.backed_up",
                                        &[(
                                            "folder",
                                            &p.file_name()
                                                .map(|f| f.to_string_lossy().into_owned())
                                                .unwrap_or_default(),
                                        )],
                                    ),
                                    Err(e) => e.to_string(),
                                },
                            );
                        }
                        let row = c.row(ROW);
                        let q = ((row.w - 12.0) / 4.0).floor();
                        let (a, rest) = row.split_left(q, 4.0);
                        let (m, rest) = rest.split_left(q, 4.0);
                        let (b, d) = rest.split_left(q, 4.0);
                        if ui.button_enabled(a, &ui.t("menu.worlds.delete"), some) {
                            *ask = Some(WorldsAsk::Delete);
                        }
                        let softer = chosen.as_ref().and_then(mode_of).is_some_and(|i| i > 0);
                        if ui.button_enabled(m, &ui.t("menu.worlds.mode"), softer) {
                            *ask = Some(WorldsAsk::Mode(0));
                        }
                        if ui.button_enabled(b, &ui.t("menu.worlds.open_folder"), some)
                            && let Some(w) = &chosen
                        {
                            *note = crate::worlds::open_folder(&saves.join(&w.folder))
                                .err()
                                .map(|e| e.to_string());
                        }
                        if ui.button(d, &ui.t("menu.worlds.trash")) {
                            push = Some(Screen::Trash {
                                list: crate::worlds::trashed(&saves),
                                selected: None,
                            });
                        }
                    }
                }
                if let Some(n) = note.as_ref() {
                    let n = n.clone();
                    ui.label(xw, c.y + 2.0, &n, theme::DIM);
                }
                if reload {
                    *list = crate::worlds::list(&saves);
                    if selected.is_some_and(|i| i >= list.len()) {
                        *selected = None;
                    }
                }
            }
            Screen::Trash { list, selected } => {
                ui.title(12.0, &ui.t("menu.trash.title"));
                let wide = (W + 120.0).min(size.0 - 16.0);
                let xw = ((size.0 - wide) / 2.0).round();
                let foot_h = 2.0 * (ROW + 4.0) + 8.0;
                let list_r = Rect::new(
                    xw,
                    PAGE_TOP,
                    wide,
                    (size.1 - PAGE_TOP - foot_h - 8.0).max(30.0),
                );
                let entries = list.clone();
                let deleted = ui.t("menu.trash.deleted");
                if let Some(i) = ui.list(
                    list_r,
                    "trash",
                    entries.len(),
                    22.0,
                    *selected,
                    |ui, r, i, _| {
                        let t = &entries[i];
                        let (y, m, d, _, _) = crate::worlds::civil(t.deleted_unix);
                        ui.label(r.x + 4.0, r.y + 2.0, &t.name, theme::TEXT);
                        ui.label(
                            r.x + 4.0,
                            r.y + 12.0,
                            &format!("{deleted} {y:04}-{m:02}-{d:02}"),
                            theme::DIM,
                        );
                    },
                ) {
                    *selected = Some(i);
                }
                if entries.is_empty() {
                    let hint = ui.t("menu.trash.none");
                    ui.text_centred(&list_r, &hint, theme::DIM);
                }
                let mut c = Column::new(xw, list_r.y + list_r.h + 6.0, wide);
                let row = c.row(ROW);
                let w3 = ((wide - 8.0) / 3.0).floor();
                let (a, rest) = row.split_left(w3, 4.0);
                let (b, d) = rest.split_left(w3, 4.0);
                let saves = cx.saves.clone();
                if ui.button_enabled(a, &ui.t("menu.trash.restore"), selected.is_some())
                    && let Some(t) = selected.and_then(|i| entries.get(i))
                {
                    let _ = crate::worlds::restore(&saves, &t.entry);
                    *list = crate::worlds::trashed(&saves);
                    *selected = None;
                }
                if ui.button_enabled(b, &ui.t("menu.trash.empty"), !entries.is_empty()) {
                    crate::worlds::empty_trash(&saves, hearth_save::meta::unix_now(), true);
                    list.clear();
                    *selected = None;
                }
                if ui.button(d, &ui.t("menu.back")) {
                    pop = true;
                    // The worlds below, as they are now.
                    if let Some(Screen::Worlds { list, .. }) = self.stack.iter_mut().rev().nth(1) {
                        *list = crate::worlds::list(&saves);
                    }
                }
            }
            Screen::NewWorld {
                name,
                seed,
                era,
                more,
                size: size_i,
                shape,
                mode,
            } => {
                let title = ui.t("menu.new_world.title");
                let wide = W + 60.0;
                let folder = folder_name(name);
                let exists = cx.saves.join(&folder).join("level.json").exists();
                let eras = cx.eras.clone();
                let modes = cx.modes.clone();
                if *mode >= modes.len() {
                    // Realistic, unless chosen otherwise.
                    *mode = modes
                        .iter()
                        .position(|m| {
                            m.0.rsplit(':').next() == crate::modes::DEFAULT.rsplit(':').next()
                        })
                        .unwrap_or(0);
                }
                let wish = &mut *cx.profiles;
                let before = wish.clone();
                let footer =
                    page_footed(ui, &title, "new_world", wide, PAGE_TOP, 4.0, 1, |ui, c| {
                        let x = c.x;
                        let line = hearth_ui::font::LINE as f32;
                        ui.label(x, c.y, &ui.t("menu.new_world.name"), theme::DIM);
                        c.space(line);
                        ui.text_field(c.row(ROW), &ui.t("menu.new_world.name_hint"), name, 32);
                        if exists {
                            ui.label(x, c.y, &ui.t("menu.new_world.exists"), theme::WARN);
                            c.space(line);
                        }
                        ui.label(x, c.y, &ui.t("menu.new_world.seed"), theme::DIM);
                        c.space(line);
                        ui.text_field(c.row(ROW), &ui.t("menu.new_world.seed_hint"), seed, 20);
                        // Who the player begins as: a name (or none), a woman, a man or as chance
                        // has it (Amendment E §6.1; the character creator comes with E5).
                        ui.text_field(
                            c.row(ROW),
                            &ui.t("menu.new_world.your_name"),
                            &mut wish.name,
                            32,
                        );
                        born_choice(ui, c.row(ROW), &mut wish.born);
                        // When the world is: its era, and how its people live then (V2.1 §15.3).
                        if !eras.is_empty() {
                            let names: Vec<String> = eras.iter().map(|e| e.1.clone()).collect();
                            *era = (*era).min(names.len() - 1);
                            ui.cycle(c.row(ROW), &ui.t("menu.new_world.era"), &names, era);
                            for l in ui.font.wrap(&eras[*era].2, c.w as u32) {
                                ui.label(x, c.y, &l, theme::DIM);
                                c.space(line);
                            }
                            c.space(2.0);
                        }
                        // How the world is played: Creative, Easy or Realistic (Amendment P §2).
                        if !modes.is_empty() {
                            let names: Vec<String> = modes.iter().map(|m| m.1.clone()).collect();
                            ui.cycle(c.row(ROW), &ui.t("menu.new_world.mode"), &names, mode);
                            for l in ui.font.wrap(&modes[*mode].2, c.w as u32) {
                                ui.label(x, c.y, &l, theme::DIM);
                                c.space(line);
                            }
                            c.space(2.0);
                        }
                        // The world's shape, asked only when wanted.
                        let label = if *more {
                            ui.t("menu.new_world.fewer")
                        } else {
                            ui.t("menu.new_world.more")
                        };
                        if ui.button(c.row(ROW), &label) {
                            *more = !*more;
                        }
                        if *more {
                            let names: Vec<String> = SIZES.iter().map(|(_, k)| ui.t(k)).collect();
                            ui.cycle(c.row(ROW), &ui.t("menu.new_world.size"), &names, size_i);
                            let scales = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];
                            let snames: Vec<String> = scales
                                .iter()
                                .map(|v| format!("{:.0}%", v * 100.0))
                                .collect();
                            let mut si = scales
                                .iter()
                                .position(|v| {
                                    (v - shape.vertical_scale.unwrap_or(1.0)).abs() < 1e-6
                                })
                                .unwrap_or(2);
                            if ui.cycle(
                                c.row(ROW),
                                &ui.t("menu.new_world.vertical"),
                                &snames,
                                &mut si,
                            ) {
                                shape.vertical_scale = Some(scales[si]);
                            }
                            let days = [24u32, 36, 48, 72, 96, 120];
                            let dnames: Vec<String> = days
                                .iter()
                                .map(|d| {
                                    ui.lang
                                        .format("menu.new_world.minutes", &[("n", &d.to_string())])
                                })
                                .collect();
                            let mut di = days
                                .iter()
                                .position(|d| Some(*d) == shape.day_length_min)
                                .unwrap_or(2);
                            if ui.cycle(
                                c.row(ROW),
                                &ui.t("menu.new_world.day_length"),
                                &dnames,
                                &mut di,
                            ) {
                                shape.day_length_min = Some(days[di]);
                            }
                            let seasons = [4u32, 6, 8, 12, 16, 30];
                            let pnames: Vec<String> =
                                seasons.iter().map(|d| d.to_string()).collect();
                            let mut pi = seasons
                                .iter()
                                .position(|d| Some(*d) == shape.days_per_season)
                                .unwrap_or(2);
                            if ui.cycle(
                                c.row(ROW),
                                &ui.t("menu.new_world.season_days"),
                                &pnames,
                                &mut pi,
                            ) {
                                shape.days_per_season = Some(seasons[pi]);
                            }
                            use hearth_content::schema::Season;
                            let all = [
                                Season::Spring,
                                Season::Summer,
                                Season::Autumn,
                                Season::Winter,
                            ];
                            let names: Vec<String> = ["spring", "summer", "autumn", "winter"]
                                .iter()
                                .map(|k| ui.t(&format!("season.{k}")))
                                .collect();
                            let mut wi = all
                                .iter()
                                .position(|s| Some(*s) == shape.starting_season)
                                .unwrap_or(0);
                            if ui.cycle(c.row(ROW), &ui.t("menu.new_world.season"), &names, &mut wi)
                            {
                                shape.starting_season = Some(all[wi]);
                            }
                        }
                    });
                if *wish != before {
                    out.push(MenuAction::ProfilesChanged);
                }
                let (a, b) = footer.split_left((footer.w - 4.0) / 2.0, 4.0);
                if ui.button_enabled(a, &ui.t("menu.new_world.create"), !exists) {
                    out.push(MenuAction::CreateWorld(NewWorldChoice {
                        folder,
                        seed: parse_seed(seed),
                        era: eras
                            .get(*era)
                            .map_or_else(|| crate::eras::WILD_EARTH.to_owned(), |e| e.0.clone()),
                        size: SIZES[(*size_i).min(SIZES.len() - 1)].0,
                        shape: *shape,
                        mode: modes
                            .get(*mode)
                            .map_or_else(|| crate::modes::DEFAULT.to_owned(), |m| m.0.clone()),
                    }));
                }
                if ui.button(b, &ui.t("menu.back")) {
                    pop = true;
                }
            }
            Screen::Making { stage, share } => {
                // The planet being made, what is being done, and how far it has come.
                ui.title((size.1 * 0.35).round(), &ui.t("menu.making.title"));
                let wide = (W + 120.0).min(size.0 - 16.0);
                let xw = ((size.0 - wide) / 2.0).round();
                let y = (size.1 * 0.35 + 20.0).round();
                let words = stage.clone();
                ui.text_centred(&Rect::new(xw, y, wide, 10.0), &words, theme::TEXT);
                let bar = Rect::new(xw, y + 16.0, wide, 6.0);
                ui.draw.rect(bar.x, bar.y, bar.w, bar.h, theme::FIELD);
                ui.draw.rect(
                    bar.x,
                    bar.y,
                    bar.w * share.clamp(0.0, 1.0),
                    bar.h,
                    theme::FILL,
                );
                let cancel = Rect::new(((size.0 - W) / 2.0).round(), y + 34.0, W, ROW);
                if ui.button(cancel, &ui.t("menu.cancel")) {
                    out.push(MenuAction::CancelCreate);
                    pop = true;
                }
            }
            Screen::Birthplace { choice, chosen } => {
                birthplace_screen(ui, cx, choice, chosen, &mut out, &mut pop);
            }
            Screen::Pause => {
                let title = ui.t("menu.pause.title");
                let when = cx.time_words.clone();
                let footer = page(ui, &title, "pause", W, PAGE_TOP, 4.0, |ui, c| {
                    if let Some(w) = &when {
                        ui.text_centred(&Rect::new(c.x, c.y, c.w, 10.0), w, theme::DIM);
                        c.space(14.0);
                    }
                    if ui.button(c.row(ROW), &ui.t("menu.pause.resume")) {
                        out.push(MenuAction::Resume);
                    }
                    if ui.button(c.row(ROW), &ui.t("menu.options")) {
                        push = Some(Screen::Options);
                    }
                    if cx.may_watch && ui.button(c.row(ROW), &ui.t("menu.pause.watch")) {
                        out.push(MenuAction::Watch);
                    }
                    if cx.creative && ui.button(c.row(ROW), &ui.t("menu.pause.time_weather")) {
                        push = Some(Screen::TimeWeather(TimeWeather::default()));
                    }
                    if cx.creative && ui.button(c.row(ROW), &ui.t("menu.pause.clear_view")) {
                        push = Some(Screen::ClearView);
                    }
                    if ui.button(c.row(ROW), &ui.t("menu.pause.save")) {
                        out.push(MenuAction::Save);
                    }
                });
                if ui.button(footer, &ui.t("menu.pause.quit")) {
                    out.push(MenuAction::QuitToTitle);
                }
            }
            Screen::Creative(st) => {
                let mut instant = cx.instant;
                let (act, next) =
                    crate::creative_ui::creative_screen(ui, cx.catalog, st, &mut instant);
                if instant != cx.instant {
                    out.push(MenuAction::Instant(instant));
                }
                if let Some(a) = act {
                    out.push(MenuAction::Creative(a));
                }
                match next {
                    crate::creative_ui::Next::Stay => {}
                    crate::creative_ui::Next::Close => out.push(MenuAction::Resume),
                    crate::creative_ui::Next::Carried => {
                        pop = true;
                        push = Some(Screen::Inventory {
                            lifted: None,
                            turned: false,
                        });
                    }
                }
            }
            Screen::ClearView => {
                let title = ui.t("menu.clear_view.title");
                let mut v = cx.clear_view;
                let footer = page(ui, &title, "clear_view", W, PAGE_TOP, 4.0, |ui, c| {
                    ui.toggle(c.row(ROW), &ui.t("menu.clear_view.on"), &mut v.on);
                    ui.toggle(c.row(ROW), &ui.t("menu.clear_view.light"), &mut v.light);
                    ui.toggle(c.row(ROW), &ui.t("menu.clear_view.air"), &mut v.air);
                    ui.toggle(c.row(ROW), &ui.t("menu.clear_view.weather"), &mut v.weather);
                    ui.toggle(c.row(ROW), &ui.t("menu.clear_view.plain"), &mut v.plain);
                });
                if v != cx.clear_view {
                    out.push(MenuAction::ClearView(v));
                }
                if ui.button(footer, &ui.t("menu.done")) {
                    pop = true;
                }
            }
            Screen::TimeWeather(t) => {
                let title = ui.t("menu.time_weather.title");
                let when = cx.time_words.clone();
                let footer = page(
                    ui,
                    &title,
                    "time_weather",
                    W + 40.0,
                    PAGE_TOP,
                    4.0,
                    |ui, c| {
                        if let Some(w) = &when {
                            ui.text_centred(&Rect::new(c.x, c.y, c.w, 10.0), w, theme::DIM);
                            c.space(14.0);
                        }
                        let hours: Vec<String> = HOURS
                            .iter()
                            .map(|h| match h {
                                None => ui.t("menu.time_weather.now"),
                                Some(h) => format!("{:02}:00", *h as u32),
                            })
                            .collect();
                        if ui.cycle(
                            c.row(ROW),
                            &ui.t("menu.time_weather.hour"),
                            &hours,
                            &mut t.hour,
                        ) && let Some(h) = HOURS[t.hour]
                        {
                            out.push(MenuAction::GoToHour(h));
                        }
                        let speeds: Vec<String> = SPEEDS
                            .iter()
                            .map(|s| match *s {
                                0.0 => ui.t("menu.time_weather.stopped"),
                                1.0 => ui.t("menu.time_weather.lived"),
                                s => format!("×{s:.0}"),
                            })
                            .collect();
                        if ui.cycle(
                            c.row(ROW),
                            &ui.t("menu.time_weather.speed"),
                            &speeds,
                            &mut t.speed,
                        ) {
                            out.push(MenuAction::TimeSpeed(SPEEDS[t.speed]));
                        }
                        let weathers: Vec<String> = WEATHERS
                            .iter()
                            .map(|w| ui.t(&format!("menu.time_weather.{w}")))
                            .collect();
                        let mut changed = ui.cycle(
                            c.row(ROW),
                            &ui.t("menu.time_weather.weather"),
                            &weathers,
                            &mut t.weather,
                        );
                        let winds: Vec<String> = WINDS
                            .iter()
                            .map(|w| {
                                w.map_or_else(
                                    || ui.t("menu.time_weather.as_it_is"),
                                    |w| format!("{w:.0} m/s"),
                                )
                            })
                            .collect();
                        changed |= ui.cycle(
                            c.row(ROW),
                            &ui.t("menu.time_weather.wind"),
                            &winds,
                            &mut t.wind,
                        );
                        let temps: Vec<String> = TEMPERATURES
                            .iter()
                            .map(|w| {
                                w.map_or_else(
                                    || ui.t("menu.time_weather.as_it_is"),
                                    |w| format!("{w:.0} °C"),
                                )
                            })
                            .collect();
                        changed |= ui.cycle(
                            c.row(ROW),
                            &ui.t("menu.time_weather.temperature"),
                            &temps,
                            &mut t.temperature,
                        );
                        if changed {
                            out.push(MenuAction::Weather(weather_hold(t)));
                        }
                    },
                );
                if ui.button(footer, &ui.t("menu.done")) {
                    pop = true;
                }
            }
            Screen::Options => {
                let title = ui.t("menu.options");
                let footer = page(ui, &title, "options", W, PAGE_TOP, 4.0, |ui, c| {
                    if ui.button(c.row(ROW), &ui.t("menu.options.video")) {
                        push = Some(Screen::Video { tab: 0 });
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
                    // Advanced: the debug screen's full reading in every mode (Amendment P §2).
                    if ui.toggle(
                        c.row(ROW),
                        &ui.t("menu.options.developer"),
                        &mut cx.options.developer_mode,
                    ) {
                        out.push(MenuAction::OptionsChanged);
                    }
                    // Deleted worlds, gone for good (Amendment P §4.2).
                    let trashed = crate::worlds::trashed(&cx.saves).len();
                    let label = ui
                        .lang
                        .format("menu.options.empty_trash", &[("n", &trashed.to_string())]);
                    if ui.button_enabled(c.row(ROW), &label, trashed > 0) {
                        crate::worlds::empty_trash(&cx.saves, hearth_save::meta::unix_now(), true);
                    }
                });
                if ui.button(footer, &ui.t("menu.done")) {
                    pop = true;
                }
            }
            Screen::Video { tab } => {
                let mut changed = false;
                let wide = W + 120.0;
                let xw = ((size.0 - wide) / 2.0).round();
                // The settings in groups (P §4.1): the screen, the quality of the image, how far
                // the eye sees.
                let tabs: Vec<String> = ["display", "quality", "distance"]
                    .iter()
                    .map(|k| ui.t(&format!("menu.video.tab.{k}")))
                    .collect();
                ui.tabs(Rect::new(xw, PAGE_TOP, wide, ROW), &tabs, tab);
                let title = ui.t("menu.options.video");
                let id = ["video_display", "video_quality", "video_distance"][(*tab).min(2)];
                let v = &mut cx.options.video;
                let qualities = [Quality::Low, Quality::Medium, Quality::High];
                let footer = page(ui, &title, id, wide, PAGE_TOP + ROW + 6.0, 3.0, |ui, c| {
                    let qnames: Vec<String> = ["low", "medium", "high"]
                        .iter()
                        .map(|k| ui.t(&format!("menu.quality.{k}")))
                        .collect();
                    match *tab {
                        0 => {
                            let modes = [
                                DisplayMode::Windowed,
                                DisplayMode::Borderless,
                                DisplayMode::Exclusive,
                            ];
                            let mnames: Vec<String> = ["windowed", "borderless", "exclusive"]
                                .iter()
                                .map(|k| ui.t(&format!("menu.video.display.{k}")))
                                .collect();
                            let mut mi =
                                modes.iter().position(|m| *m == v.display_mode).unwrap_or(0);
                            if ui.cycle(c.row(ROW), &ui.t("menu.video.display"), &mnames, &mut mi) {
                                v.display_mode = modes[mi];
                                changed = true;
                            }
                            let gnames: Vec<String> =
                                std::iter::once(ui.t("menu.video.gui_scale.auto"))
                                    .chain((1..=6).map(|n| n.to_string()))
                                    .collect();
                            let mut gi = (v.gui_scale as usize).min(6);
                            if ui.cycle(c.row(ROW), &ui.t("menu.video.gui_scale"), &gnames, &mut gi)
                            {
                                v.gui_scale = gi as u32;
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
                        }
                        1 => {
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
                            use hearth_core::options::AntiAliasing;
                            let aa = [AntiAliasing::Off, AntiAliasing::Taa];
                            let aa_names = [ui.t("menu.video.aa.off"), ui.t("menu.video.aa.taa")];
                            let mut ai = usize::from(v.anti_aliasing == AntiAliasing::Taa);
                            if ui.cycle(
                                c.row(ROW),
                                &ui.t("menu.video.anti_aliasing"),
                                &aa_names,
                                &mut ai,
                            ) {
                                v.anti_aliasing = aa[ai];
                                v.refresh_preset();
                                changed = true;
                            }
                            let mut qi = qualities
                                .iter()
                                .position(|q| *q == v.lod_detail)
                                .unwrap_or(1);
                            if ui.cycle(
                                c.row(ROW),
                                &ui.t("menu.video.lod_detail"),
                                &qnames,
                                &mut qi,
                            ) {
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
                        }
                        _ => {
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
                        }
                    }
                });
                if ui.button(footer, &ui.t("menu.done")) {
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
                let title = ui.t("menu.options.sound");
                let s = &mut cx.options.sound;
                let devices = cx.audio_devices;
                let mut changed = false;
                let footer = page(ui, &title, "sound", W + 120.0, PAGE_TOP, 3.0, |ui, c| {
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
                    names.extend(devices.iter().cloned());
                    let mut i = devices
                        .iter()
                        .position(|d| *d == s.device)
                        .map_or(0, |p| p + 1);
                    if ui.cycle(c.row(ROW), &ui.t("menu.sound.device"), &names, &mut i) {
                        s.device = match i {
                            0 => String::new(),
                            i => devices[i - 1].clone(),
                        };
                        changed = true;
                    }
                    let mut captions = s.subtitles;
                    if ui.toggle(c.row(ROW), &ui.t("menu.sound.subtitles"), &mut captions) {
                        s.subtitles = captions;
                        changed = true;
                    }
                });
                if ui.button(footer, &ui.t("menu.done")) {
                    pop = true;
                }
                if changed {
                    out.push(MenuAction::OptionsChanged);
                }
            }
            Screen::Death => {
                death_screen(ui, cx, &mut out);
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
            Screen::Accessibility => {
                let title = ui.t("menu.options.accessibility");
                let a = &mut cx.options.accessibility;
                let mut changed = false;
                let footer = page(ui, &title, "accessibility", W, PAGE_TOP, 4.0, |ui, c| {
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
                        changed = true;
                    }
                    changed |= ui.toggle(
                        c.row(ROW),
                        &ui.t("menu.accessibility.guided_hud"),
                        &mut a.guided_hud,
                    );
                    changed |= ui.toggle(
                        c.row(ROW),
                        &ui.t("menu.accessibility.reduce_motion"),
                        &mut a.reduce_motion,
                    );
                });
                if changed {
                    out.push(MenuAction::OptionsChanged);
                }
                if ui.button(footer, &ui.t("menu.done")) {
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

/// Where to be born (Amendment P §4.3): the planet's globe behind the screen (the app draws
/// it), turned by dragging and zoomed by the wheel; the place under the pointer told in plain
/// words; a click chooses (a sea's point is born on its nearest coast), "Recommended" the place
/// the world finds best for a first life, "Surprise me" anywhere on land.
fn birthplace_screen(
    ui: &mut Ui<'_>,
    cx: &mut MenuContext<'_>,
    choice: &NewWorldChoice,
    chosen: &mut Option<(f32, f32)>,
    out: &mut Vec<MenuAction>,
    pop: &mut bool,
) {
    let size = ui.size;
    ui.title(10.0, &ui.t("menu.birthplace.title"));
    let wide = (W + 160.0).min(size.0 - 16.0);
    let xw = ((size.0 - wide) / 2.0).round();
    // The buttons along the bottom: two rows.
    let foot = Rect::new(
        xw,
        size.1 - 2.0 * (ROW + 4.0) - 6.0,
        wide,
        2.0 * (ROW + 4.0),
    );
    let scale = ui.draw.scale;
    let pointer = ui.input.pointer;
    let mut said: Option<String> = None;
    if let Some(g) = cx.globe.as_mut() {
        let over_globe = pointer.is_some_and(|p| p.1 > 24.0 && p.1 < foot.y);
        if let Some(p) = pointer {
            g.picker.cursor_moved(glam::Vec2::new(p.0, p.1) * scale);
        }
        if over_globe {
            if ui.input.pressed {
                g.picker.button(true);
            }
            if ui.input.released
                && let Some(ll) = g.picker.button(false)
            {
                *chosen = Some(ll);
            }
            if ui.input.scroll != 0.0 {
                g.picker.view.zoom_by(ui.input.scroll.round() as i32);
            }
        }
        let shown = g.picker.hovered().or(*chosen);
        said = shown.map(|(lat, lon)| crate::globe::describe(&g.terrain, lat, lon));
    }
    let mut y = 24.0;
    for l in said
        .iter()
        .flat_map(|s| ui.font.wrap(s, wide as u32))
        .take(4)
    {
        let lw = ui.font.width(&l) as f32;
        ui.label(((size.0 - lw) / 2.0).round(), y, &l, theme::TEXT);
        y += hearth_ui::font::LINE as f32;
    }
    let w3 = ((wide - 8.0) / 3.0).floor();
    let row = Rect::new(foot.x, foot.y, wide, ROW);
    let (a, rest) = row.split_left(w3, 4.0);
    let (b, d) = rest.split_left(w3, 4.0);
    let recommended = ui.button(a, &ui.t("menu.birthplace.recommended"));
    let surprise = ui.button(b, &ui.t("menu.birthplace.surprise"));
    if recommended || surprise {
        let random = surprise;
        if let Some(g) = cx.globe.as_mut() {
            let (x, z) = g.terrain.find_spawn(random);
            let at = glam::DVec3::new(x as f64, 0.0, z as f64);
            let (lat, lon) = crate::globe::lat_lon(g.terrain.planet(), at);
            g.picker.view.lat = lat;
            g.picker.view.lon = lon;
            *chosen = Some((lat, lon));
        }
    }
    if ui.button(d, &ui.t("menu.back")) {
        out.push(MenuAction::CancelCreate);
        *pop = true;
    }
    let row = Rect::new(foot.x, foot.y + ROW + 4.0, wide, ROW);
    if ui.button_enabled(row, &ui.t("menu.birthplace.born_here"), chosen.is_some())
        && let (Some((lat, lon)), Some(g)) = (*chosen, cx.globe.as_ref())
    {
        let (x, z) = crate::globe::world_xz(g.terrain.planet(), lat, lon);
        out.push(MenuAction::Play {
            folder: choice.folder.clone(),
            seed: choice.seed,
            knowledge: Default::default(),
            era: choice.era.clone(),
            size: choice.size,
            shape: choice.shape,
            birthplace: Some(glam::DVec2::new(x as f64 + 0.5, z as f64 + 0.5)),
            mode: Some(choice.mode.clone()),
        });
    }
}

/// A woman, a man, or as chance has it.
fn born_choice(ui: &mut Ui<'_>, r: Rect, born: &mut Born) {
    let names: Vec<String> = Born::ALL.iter().map(|b| ui.t(b.key())).collect();
    let mut i = Born::ALL.iter().position(|b| b == born).unwrap_or(0);
    if ui.cycle(r, &ui.t("menu.new_world.born"), &names, &mut i) {
        *born = Born::ALL[i];
    }
}

/// After death (Amendment E §6.6): how it happened and what a new life keeps; begin a new life
/// near where the player last lived or elsewhere on the globe, or begin the world again.
fn death_screen(ui: &mut Ui<'_>, cx: &mut MenuContext<'_>, out: &mut Vec<MenuAction>) {
    let size = ui.size;
    let Some(d) = cx.death.clone() else {
        // Alive again: nothing to face.
        return;
    };
    let wide = (W + 120.0).min(size.0 - 16.0);
    let footer = page_footed(ui, &d.words, "death", wide, PAGE_TOP, 4.0, 2, |ui, c| {
        let size = ui.size;
        let said = match d.after_death {
            hearth_save::AfterDeath::TheirsOnly => "body.death.theirs_only",
            hearth_save::AfterDeath::HeadStart => "body.death.head_start",
            hearth_save::AfterDeath::KeepEverything => "body.death.keep_everything",
        };
        for line in ui.font.wrap(&ui.t(said), c.w as u32) {
            let lw = ui.font.width(&line) as f32;
            ui.label(((size.0 - lw) / 2.0).round(), c.y, &line, theme::DIM);
            c.space(hearth_ui::font::LINE as f32);
        }
        c.space(8.0);
        if ui.button(c.row(ROW), &ui.t("menu.death.new_life")) {
            out.push(MenuAction::NewLife { elsewhere: false });
        }
        if ui.button(c.row(ROW), &ui.t("menu.death.new_life_elsewhere")) {
            out.push(MenuAction::NewLife { elsewhere: true });
        }
    });
    if ui.button(footer, &ui.t("menu.death.to_title")) {
        out.push(MenuAction::QuitToTitle);
    }
    let again = Rect::new(footer.x, footer.y + ROW + 4.0, footer.w, ROW);
    if ui.button(again, &ui.t("menu.death.again")) {
        out.push(MenuAction::Restart);
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

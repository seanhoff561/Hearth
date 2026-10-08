//! The windowed application: event loop, window management (display modes, remembered
//! placement), input routing and the frame loop.

use std::sync::Arc;
use std::time::{Duration, Instant};

use hearth_core::options::{DisplayMode, Options, PresentModePref};
use hearth_core::paths::GameDirs;
use hearth_input::{InputKey, InputOptions, InputState, Key, KeyBindings, MouseButton, builtin};
use hearth_render::{PresentPreference, Renderer};
use hearth_ui::NavKey;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowId};

use crate::client::Client;
use crate::content_state::ContentState;
use crate::frame_limiter::FrameLimiter;
use crate::gamepad::{Gamepads, Polled, Press};
use crate::interface::Interface;
use crate::menus::{MenuAction, MenuContext, Menus, Screen};
use crate::profiles::Profiles;

/// Command-line configuration of a run.
#[derive(Debug, Clone, Default)]
pub struct LaunchConfig {
    pub dirs: Option<GameDirs>,
    /// Exit cleanly (saving options) after this long; used by automated smoke tests.
    pub quit_after: Option<Duration>,
    /// Seed of a new world.
    pub seed: Option<u64>,
    /// A world to play at once (its save folder), skipping the title screen.
    pub world: Option<String>,
}

struct Running {
    window: Arc<Window>,
    renderer: Renderer,
    /// The world being played, if any (none at the title).
    client: Option<Client>,
    interface: Interface,
    menus: Menus,
    /// Mouse captured for looking around.
    captured: bool,
    last_frame: Instant,
    title_timer: Instant,
    title_frames: u32,
    log_timer: Instant,
}

/// Top-level application state.
pub struct App {
    dirs: GameDirs,
    options: Options,
    bindings: KeyBindings,
    input: InputState,
    running: Option<Running>,
    limiter: FrameLimiter,
    /// Fullscreen kind F11 switches to from windowed mode.
    last_fullscreen: DisplayMode,
    start: Instant,
    exit_requested: bool,
    fatal_error: Option<String>,
    quit_after: Option<Duration>,
    frames_rendered: u64,
    seed: u64,
    /// A world named on the command line, played at once.
    world: Option<String>,
    content: ContentState,
    /// The languages there are words for.
    languages: Vec<String>,
    pads: Gamepads,
    /// The sound output (none if there is no device), the device asked for, the devices
    /// there are, and when the output was last checked and last told the surroundings.
    audio: Option<hearth_audio::Audio>,
    audio_asked: String,
    audio_devices: Vec<String>,
    audio_checked: Instant,
    ambience_sent: Instant,
    /// Who the player begins as.
    profiles: Profiles,
    /// A new world's planet being made, and then its globe while the birthplace is chosen
    /// (Amendment P §4.3).
    making: Option<Making>,
    choosing: Option<Choosing>,
}

/// A new world's planet size, shape and birthplace (a saved world keeps its own).
#[derive(Debug, Clone)]
struct NewShape {
    size: hearth_math::PlanetSize,
    shape: crate::server::WorldShape,
    birthplace: Option<glam::DVec2>,
    /// Its game mode (Amendment P §2).
    mode: Option<String>,
}

impl Default for NewShape {
    fn default() -> Self {
        Self {
            size: hearth_math::PlanetSize::Standard,
            shape: Default::default(),
            birthplace: None,
            mode: None,
        }
    }
}

/// A new world's planet being made on its own thread: what it is doing and how far it has come.
struct Making {
    choice: crate::menus::NewWorldChoice,
    progress: Arc<std::sync::Mutex<(f32, String)>>,
    handle: std::thread::JoinHandle<anyhow::Result<Arc<hearth_worldgen::region::Terrain>>>,
}

/// A new world's globe, its birthplace being chosen.
struct Choosing {
    terrain: Arc<hearth_worldgen::region::Terrain>,
    picker: crate::globe::GlobePicker,
}

impl App {
    pub fn new(
        dirs: GameDirs,
        quit_after: Option<Duration>,
        seed: u64,
        world: Option<String>,
    ) -> Self {
        if let Err(e) = dirs.ensure_created() {
            log::warn!(
                "could not create game directory {}: {e}",
                dirs.root.display()
            );
        }
        let options = Options::load_or_default(&dirs.options_file());
        let mut bindings = KeyBindings::from_map(
            hearth_input::ActionRegistry::with_builtins(),
            &options.controls.key_bindings,
        );
        bindings.apply_pad_map(&options.controls.pad_bindings);
        let mut input = InputState::new(bindings.registry().len());
        input.set_options(InputOptions {
            toggle_sneak: options.controls.toggle_sneak,
            toggle_sprint: options.controls.toggle_sprint,
        });
        let profiles = Profiles::load_or_migrate(&dirs.root);
        let last_fullscreen = match options.video.display_mode {
            DisplayMode::Exclusive => DisplayMode::Exclusive,
            _ => DisplayMode::Borderless,
        };
        Self {
            dirs,
            limiter: FrameLimiter::new(),
            options,
            bindings,
            input,
            running: None,
            last_fullscreen,
            start: Instant::now(),
            exit_requested: false,
            fatal_error: None,
            quit_after,
            frames_rendered: 0,
            seed,
            world,
            content: ContentState::load(vec![crate::scene::data_pack_dir()]),
            languages: Interface::languages(),
            pads: Gamepads::new(),
            audio: None,
            audio_asked: String::new(),
            audio_devices: Vec::new(),
            audio_checked: Instant::now(),
            ambience_sent: Instant::now(),
            profiles,
            making: None,
            choosing: None,
        }
    }

    fn save_profiles(&self) {
        if let Err(e) = self
            .profiles
            .save(&self.dirs.root.join(crate::profiles::FILE))
        {
            log::error!("could not save the wishes for a birth: {e}");
        }
    }

    /// Opens the sound output the options name (or the default), with their volumes.
    fn open_audio(&mut self) {
        // Close the old stream first: some devices take one at a time.
        self.audio = None;
        self.audio_asked = self.options.sound.device.clone();
        let seed = self.start.elapsed().as_nanos() as u32 ^ 0x5eed;
        match hearth_audio::Audio::open(&self.options.sound.device, seed) {
            Ok(a) => self.audio = Some(a),
            Err(e) => log::warn!("no sound: {e}"),
        }
        self.send_volumes();
    }

    fn send_volumes(&self) {
        let Some(audio) = &self.audio else { return };
        let s = &self.options.sound;
        let mut buses = [1.0; hearth_audio::BUSES];
        for (bus, v) in [
            (hearth_audio::Bus::Weather, s.weather),
            (hearth_audio::Bus::Blocks, s.blocks),
            (hearth_audio::Bus::Players, s.players),
            (hearth_audio::Bus::Ambient, s.ambient),
            (hearth_audio::Bus::Hostile, s.hostile),
            (hearth_audio::Bus::Friendly, s.friendly),
            (hearth_audio::Bus::Ui, s.ui),
            (hearth_audio::Bus::Music, s.music),
        ] {
            buses[bus.index()] = v;
        }
        audio.send(hearth_audio::Command::Volumes {
            master: s.master,
            buses,
        });
    }

    /// This frame's sounds: the world's, the interface's clicks, the surroundings (20 times a
    /// second), and a new output if the device was lost.
    fn sound(&mut self, dt: f64) {
        if self.audio_checked.elapsed().as_secs_f64() >= 2.0 {
            self.audio_checked = Instant::now();
            if self.audio.as_ref().is_some_and(|a| a.failed()) {
                log::info!("sound output lost: opening it again");
                self.open_audio();
            }
        }
        let Some(run) = &mut self.running else {
            return;
        };
        let clicks = std::mem::take(&mut run.interface.state.clicks);
        let Some(audio) = &self.audio else {
            if let Some(c) = &mut run.client {
                c.take_sounds().for_each(drop);
            }
            return;
        };
        if clicks > 0 {
            audio.send(hearth_audio::Command::Play {
                sound: hearth_audio::Sound::Click,
                bus: hearth_audio::Bus::Ui,
                gain: 1.0,
                pan: 0.0,
            });
        }
        let since = self.ambience_sent.elapsed().as_secs_f64();
        match &mut run.client {
            Some(c) => {
                for cmd in c.take_sounds() {
                    audio.send(cmd);
                }
                let ambience = c.ambience(dt);
                if since >= 0.05 {
                    self.ambience_sent = Instant::now();
                    audio.send(hearth_audio::Command::Ambience(ambience));
                }
            }
            None if since >= 0.05 => {
                self.ambience_sent = Instant::now();
                audio.send(hearth_audio::Command::Ambience(hearth_audio::Ambience {
                    heart_bpm: 62.0,
                    breaths_per_min: 12.0,
                    ..Default::default()
                }));
            }
            None => {}
        }
    }

    fn present_preference(&self) -> PresentPreference {
        match self.options.video.present_mode {
            PresentModePref::Auto => PresentPreference::Auto {
                vsync: self.options.video.vsync,
            },
            PresentModePref::Immediate => PresentPreference::Immediate,
            PresentModePref::Mailbox => PresentPreference::Mailbox,
            PresentModePref::Fifo => PresentPreference::Fifo,
        }
    }

    fn fullscreen_for(&self, window: &Window, mode: DisplayMode) -> Option<Fullscreen> {
        let monitor = match &self.options.video.monitor {
            Some(name) => window
                .available_monitors()
                .find(|m| m.name().as_deref() == Some(name.as_str())),
            None => None,
        }
        .or_else(|| window.current_monitor());
        match mode {
            DisplayMode::Windowed => None,
            DisplayMode::Borderless => Some(Fullscreen::Borderless(monitor)),
            DisplayMode::Exclusive => {
                let monitor = monitor?;
                let wanted = self.options.video.exclusive_resolution;
                let wanted_hz = self.options.video.exclusive_refresh_millihertz;
                let mut modes: Vec<_> = monitor.video_modes().collect();
                // Highest resolution, then highest refresh rate first.
                modes.sort_by_key(|m| {
                    std::cmp::Reverse((
                        m.size().width,
                        m.size().height,
                        m.refresh_rate_millihertz(),
                    ))
                });
                let chosen = modes
                    .iter()
                    .find(|m| {
                        wanted.is_none_or(|[w, h]| m.size().width == w && m.size().height == h)
                            && wanted_hz.is_none_or(|hz| m.refresh_rate_millihertz() == hz)
                    })
                    .or_else(|| {
                        modes.iter().find(|m| {
                            wanted.is_none_or(|[w, h]| m.size().width == w && m.size().height == h)
                        })
                    })
                    .or(modes.first())
                    .cloned();
                match chosen {
                    Some(m) => Some(Fullscreen::Exclusive(m)),
                    None => Some(Fullscreen::Borderless(Some(monitor))),
                }
            }
        }
    }

    fn apply_display_mode(&mut self) {
        let Some(run) = &self.running else { return };
        let fs = self.fullscreen_for(&run.window, self.options.video.display_mode);
        run.window.set_fullscreen(fs);
    }

    fn toggle_fullscreen(&mut self) {
        self.options.video.display_mode = match self.options.video.display_mode {
            DisplayMode::Windowed => self.last_fullscreen,
            other => {
                self.last_fullscreen = other;
                DisplayMode::Windowed
            }
        };
        self.apply_display_mode();
        self.save_options();
    }

    fn save_options(&mut self) {
        self.options.controls.key_bindings = self.bindings.to_map();
        self.options.controls.pad_bindings = self.bindings.pad_map();
        if let Err(e) = self.options.save(&self.dirs.options_file()) {
            log::error!("failed to save options: {e}");
        }
    }

    fn remember_window_placement(&mut self) {
        let Some(run) = &self.running else { return };
        if self.options.video.display_mode != DisplayMode::Windowed {
            return;
        }
        self.options.window.maximized = run.window.is_maximized();
        if !self.options.window.maximized {
            let size = run.window.inner_size();
            if size.width > 0 && size.height > 0 {
                self.options.window.size = [size.width, size.height];
            }
            if let Ok(pos) = run.window.outer_position() {
                self.options.window.position = Some([pos.x, pos.y]);
            }
        }
    }

    fn handle_key(&mut self, key: InputKey, pressed: bool) {
        let Some(run) = &mut self.running else {
            return;
        };
        // A binding being captured takes every key, pressed and let go (a modifier let go
        // alone binds itself).
        if run.menus.capturing() {
            if run.menus.capture(key, pressed, &mut self.bindings) {
                self.save_options();
            }
            return;
        }
        if run.menus.is_open() {
            if !pressed {
                // What was held in play is let go (a tap it held back doesn't fire under a
                // menu).
                self.input.release(key, &self.bindings);
                if key == InputKey::Mouse(MouseButton::Left) {
                    run.interface.button(false);
                }
                return;
            }
            match key {
                InputKey::Mouse(MouseButton::Left) => run.interface.button(true),
                InputKey::Mouse(MouseButton::Right) => run.interface.alt_button(),
                InputKey::Keyboard(k) => {
                    let nav = match k {
                        Key::Up => Some(NavKey::Up),
                        Key::Down => Some(NavKey::Down),
                        Key::Left => Some(NavKey::Left),
                        Key::Right => Some(NavKey::Right),
                        Key::Enter | Key::NumpadEnter => Some(NavKey::Enter),
                        Key::Tab => Some(NavKey::Tab),
                        Key::Backspace => Some(NavKey::Backspace),
                        Key::Delete => Some(NavKey::Delete),
                        Key::Home => Some(NavKey::Home),
                        Key::End => Some(NavKey::End),
                        _ => None,
                    };
                    let inventory_key = self.bindings.get(builtin::INVENTORY)
                        == Some(hearth_input::Binding::key(k));
                    let journal_key =
                        self.bindings.get(builtin::JOURNAL) == Some(hearth_input::Binding::key(k));
                    if (inventory_key && run.menus.inventory_open())
                        || (journal_key && run.menus.journal_open())
                    {
                        run.menus.close_all();
                    } else if let Some(n) = nav {
                        run.interface.key(n);
                    } else if k == Key::Escape {
                        let action = run.menus.back();
                        if let Some(a) = action {
                            self.menu_actions(vec![a]);
                        }
                    } else if k == Key::F11 {
                        self.toggle_fullscreen();
                    }
                }
                _ => {}
            }
            return;
        }
        let globe_open = run.client.as_ref().is_some_and(|c| c.globe.open);
        if pressed {
            let activated: Vec<_> = self.input.press(key, &self.bindings).to_vec();
            self.fire(activated);
            if key == InputKey::Mouse(MouseButton::Left) {
                if globe_open {
                    if let Some(c) = self.running.as_mut().and_then(|r| r.client.as_mut()) {
                        c.globe_button(true);
                    }
                } else if self.running.as_ref().is_some_and(|r| r.client.is_some()) {
                    self.set_captured(true);
                }
            }
        } else {
            // A tap on a modifier key alone comes as it is let go.
            let tapped = self.input.release(key, &self.bindings);
            self.fire(tapped);
            if key == InputKey::Mouse(MouseButton::Left)
                && globe_open
                && let Some(c) = self.running.as_mut().and_then(|r| r.client.as_mut())
            {
                c.globe_button(false);
            }
        }
    }

    /// Does what the actions just pressed (or tapped) ask.
    fn fire(&mut self, actions: Vec<hearth_input::ActionId>) {
        let mut release_mouse = false;
        for action in actions {
            log::debug!("action {}", self.bindings.registry().def(action).id);
            if action == builtin::FULLSCREEN {
                self.toggle_fullscreen();
            } else if action == builtin::PAUSE {
                let Some(run) = &mut self.running else {
                    continue;
                };
                match &mut run.client {
                    Some(c) if c.globe.open => c.globe.close(),
                    // Spectating: back to the body.
                    Some(c) if c.watching_alive() => c.step_in(),
                    Some(c) => {
                        c.pause(true);
                        run.menus.open(Screen::Pause);
                    }
                    None => {}
                }
                release_mouse = true;
            } else if action == builtin::DEBUG_RELOAD_RESOURCES {
                self.content.reload();
            } else if let Some(run) = &mut self.running
                && let Some(p) = &mut run.client
            {
                let watching = p.watching.is_some();
                if action == builtin::DEBUG {
                    p.toggle_debug();
                } else if action == builtin::WORLD_MAP {
                    release_mouse |= p.toggle_globe();
                } else if watching && action == builtin::WATCH_FASTER {
                    p.watch_faster(1);
                } else if watching && action == builtin::WATCH_SLOWER {
                    p.watch_faster(-1);
                } else if watching && action == builtin::INTERACT {
                    p.watch_follow();
                } else if action == builtin::DEBUG_TIME_FORWARD && p.may_watch() {
                    p.skip_hours(1.0);
                } else if action == builtin::DEBUG_TIME_BACK && p.may_watch() {
                    p.skip_hours(-1.0);
                } else if action == builtin::DEBUG_SEASON_FORWARD && p.may_watch() {
                    p.skip_hours(24.0 * p.calendar.days_per_season as f64);
                } else if action == builtin::DEBUG_TIME_WARP && p.may_watch() {
                    // Off → one game hour per real second → off.
                    let warp = if p.time_warp > 0.0 {
                        0.0
                    } else {
                        p.calendar.ticks_per_day() / 24.0
                    };
                    p.set_time_warp(warp);
                } else if action == builtin::NO_CLIP && p.creative() {
                    // Through the ground: flying, and nothing stops the body.
                    p.no_clip = !p.no_clip;
                    p.flying |= p.no_clip;
                } else if action == builtin::DEBUG_FREE_CAMERA && p.may_watch() {
                    p.toggle_free_camera();
                } else if action == builtin::TOGGLE_PERSPECTIVE {
                    p.toggle_perspective();
                } else if action == builtin::SLEEP {
                    p.toggle_rest();
                } else if action == builtin::SHOUT {
                    p.shout();
                } else if action == builtin::BODY_PANEL {
                    p.toggle_body_panel();
                } else if action == builtin::BUILDER_VIEW {
                    p.toggle_builder_view();
                } else if action == builtin::INVENTORY && p.creative() && !p.dead() {
                    // Creative's inventory first; its Carried button the things carried.
                    run.menus.open(Screen::Creative(Default::default()));
                    release_mouse = true;
                } else if action == builtin::PICK_BLOCK && p.creative() {
                    if let Some((tab, query)) = p.pick() {
                        run.menus
                            .open(Screen::Creative(crate::creative_ui::CreativeScreen {
                                tab,
                                query,
                                selected: Some(0),
                                ..Default::default()
                            }));
                        release_mouse = true;
                    }
                } else if action == builtin::CLEAR_VIEW && (p.creative() || p.developer) {
                    p.clear_view.on = !p.clear_view.on;
                } else if action == builtin::CREATIVE_REMOVE {
                    p.remove_looked();
                } else if action == builtin::SPECTATE && p.creative() {
                    // Spectate, and back: the body brought to where the eye is.
                    if p.watching_alive() {
                        p.resume_here();
                    } else {
                        p.observe();
                    }
                } else if action == builtin::INVENTORY && p.can_handle() {
                    run.menus.open(Screen::Inventory {
                        lifted: None,
                        turned: false,
                    });
                    release_mouse = true;
                } else if action == builtin::JOURNAL && p.crafting.is_some() {
                    run.menus.open(Screen::Journal { tab: 0, scroll: 0 });
                    release_mouse = true;
                }
            }
        }
        if release_mouse {
            self.set_captured(false);
        }
    }

    /// Starts playing a world.
    /// The eras a new world may be made in: (id, name, how its people live), playable ones in
    /// their order.
    fn eras(&self) -> Vec<(String, String, String)> {
        let Some(c) = self.content.content.as_deref() else {
            return Vec::new();
        };
        let mut eras: Vec<&hearth_content::schema::era::Era> =
            c.eras.iter().filter(|e| e.available).collect();
        eras.sort_by_key(|e| e.order);
        eras.into_iter()
            .map(|e| (e.id.clone(), e.name.clone(), e.description.clone()))
            .collect()
    }

    /// The game modes (id, name, summary) in Create World's order (Amendment P §2).
    fn modes(&self) -> Vec<(String, String, String)> {
        let Some(c) = self.content.content.as_deref() else {
            return Vec::new();
        };
        crate::modes::all(c)
            .into_iter()
            .map(|m| (m.id.clone(), m.name.clone(), m.summary.clone()))
            .collect()
    }

    fn play(
        &mut self,
        folder: &str,
        seed: u64,
        knowledge: hearth_save::KnowledgeMode,
        era: &str,
        new: NewShape,
    ) {
        let Some(run) = &mut self.running else {
            return;
        };
        log::info!("playing world {folder:?}");
        let mut spec = Client::default_world(
            folder,
            seed,
            Some(self.dirs.cache()),
            Some(self.dirs.saves()),
            self.profiles.appearance(seed),
            knowledge,
            era,
        );
        spec.planet = new.size;
        spec.shape = new.shape;
        spec.birthplace = new.birthplace;
        spec.mode = new.mode;
        self.making = None;
        self.choosing = None;
        let mut client = Client::new(
            spec,
            &self.options,
            run.renderer.color_format(),
            self.content.content.as_deref(),
        );
        client.apply_options(&self.options);
        run.client = Some(client);
        run.menus.close_all();
    }

    /// Starts making a new world's planet (cached where the world will find it), its progress
    /// shown until its globe opens to choose the birthplace.
    fn make_planet(&mut self, choice: crate::menus::NewWorldChoice) {
        let progress = Arc::new(std::sync::Mutex::new((0.0f32, String::new())));
        let settings = choice.shape.planet(choice.seed, choice.size);
        let cache = self
            .dirs
            .cache()
            .join(crate::scene::planet_cache_name(&settings));
        let told = progress.clone();
        let handle = std::thread::Builder::new()
            .name("new planet".into())
            .spawn(move || {
                use hearth_worldgen::planet::PlanetGrid;
                let tell = |f: f32, stage: &str| {
                    if let Ok(mut p) = told.lock() {
                        *p = (f, stage.to_owned());
                    }
                };
                let grid = match PlanetGrid::load(&cache) {
                    Ok(g) if cache.exists() => g,
                    _ => {
                        let g = PlanetGrid::build(&settings, &tell);
                        if let Some(d) = cache.parent() {
                            std::fs::create_dir_all(d).ok();
                        }
                        if let Err(e) = g.save(&cache) {
                            log::warn!("could not cache planet: {e}");
                        }
                        g
                    }
                };
                Ok(Arc::new(hearth_worldgen::region::Terrain::new(Arc::new(
                    grid,
                ))))
            });
        match handle {
            Ok(handle) => {
                if let Some(run) = &mut self.running {
                    run.menus.open(Screen::Making {
                        stage: String::new(),
                        share: 0.0,
                    });
                }
                self.making = Some(Making {
                    choice,
                    progress,
                    handle,
                });
            }
            Err(e) => log::error!("could not start making the planet: {e}"),
        }
    }

    /// Follows the planet being made; when it is done, opens its globe to choose the birthplace.
    fn follow_making(&mut self) {
        let Some(run) = &mut self.running else {
            return;
        };
        let Some(m) = &self.making else {
            return;
        };
        if !m.handle.is_finished() {
            let (share, stage) = m.progress.lock().map(|p| p.clone()).unwrap_or_default();
            if let Some(Screen::Making { stage: s, share: f }) = run.menus.top_mut() {
                *s = if stage.is_empty() {
                    String::new()
                } else {
                    format!("{stage}…")
                };
                *f = share;
            }
            return;
        }
        let Some(m) = self.making.take() else {
            return;
        };
        match m.handle.join() {
            Ok(Ok(terrain)) => {
                let (x, z) = terrain.find_spawn(false);
                let (lat, lon) = crate::globe::lat_lon(
                    terrain.planet(),
                    glam::DVec3::new(x as f64, 0.0, z as f64),
                );
                let mut picker = crate::globe::GlobePicker::default();
                picker.open(&terrain, lat, lon);
                if matches!(run.menus.top_mut(), Some(Screen::Making { .. })) {
                    run.menus.back();
                    run.menus.open(Screen::Birthplace {
                        choice: m.choice,
                        chosen: None,
                    });
                    self.choosing = Some(Choosing { terrain, picker });
                }
            }
            Ok(Err(e)) => log::error!("the planet could not be made: {e}"),
            Err(_) => log::error!("the planet's thread failed"),
        }
    }

    /// Does what the menus asked.
    fn menu_actions(&mut self, actions: Vec<MenuAction>) {
        for a in actions {
            match a {
                MenuAction::Play {
                    folder,
                    seed,
                    knowledge,
                    era,
                    size,
                    shape,
                    birthplace,
                    mode,
                } => self.play(
                    &folder,
                    seed,
                    knowledge,
                    &era,
                    NewShape {
                        size,
                        shape,
                        birthplace,
                        mode,
                    },
                ),
                MenuAction::CreateWorld(choice) => self.make_planet(choice),
                MenuAction::ChangeMode { folder, mode } => {
                    let saves = self.dirs.saves();
                    let content = self.content.content.clone();
                    let note = content.as_deref().and_then(|c| {
                        let to = crate::server::mode_rules(c, Some(&mode))?;
                        let from = crate::worlds::info(&saves.join(&folder)).map(|w| {
                            crate::server::mode_rules(
                                c,
                                Some(w.mode.as_deref().unwrap_or(crate::modes::DEFAULT)),
                            )
                        });
                        match crate::worlds::set_mode(&saves, &folder, from.flatten().as_ref(), &to)
                        {
                            Ok(true) => None,
                            Ok(false) => Some("That mode is stricter than the world's.".to_owned()),
                            Err(e) => Some(e.to_string()),
                        }
                    });
                    if let Some(run) = &mut self.running
                        && let Some(crate::menus::Screen::Worlds { list, note: n, .. }) =
                            run.menus.top_mut()
                    {
                        *list = crate::worlds::list(&saves);
                        *n = note;
                    }
                }
                MenuAction::CancelCreate => {
                    self.making = None;
                    self.choosing = None;
                }
                MenuAction::Creative(act) => {
                    if let Some(c) = self.running.as_mut().and_then(|r| r.client.as_mut()) {
                        c.creative_act(act);
                    }
                }
                MenuAction::ClearView(v) => {
                    if let Some(c) = self.running.as_mut().and_then(|r| r.client.as_mut()) {
                        c.clear_view = v;
                    }
                }
                MenuAction::Instant(on) => {
                    if let Some(c) = self.running.as_mut().and_then(|r| r.client.as_mut()) {
                        c.set_instant(on);
                    }
                }
                MenuAction::GoToHour(h) => {
                    if let Some(c) = self.running.as_mut().and_then(|r| r.client.as_mut()) {
                        c.go_to_hour(h);
                    }
                }
                MenuAction::TimeSpeed(x) => {
                    if let Some(c) = self.running.as_mut().and_then(|r| r.client.as_mut()) {
                        c.time_speed(x);
                    }
                }
                MenuAction::Weather(hold) => {
                    if let Some(c) = self.running.as_mut().and_then(|r| r.client.as_mut()) {
                        c.hold_weather(hold);
                    }
                }
                MenuAction::Watch => {
                    if let Some(run) = &mut self.running {
                        run.menus.close_all();
                        if let Some(c) = &mut run.client {
                            c.pause(false);
                            c.observe();
                        }
                    }
                }
                MenuAction::Save => {
                    if let Some(c) = self.running.as_ref().and_then(|r| r.client.as_ref()) {
                        c.save_now();
                    }
                }
                MenuAction::Knapped { process, aim, hand } => {
                    if let Some(run) = &mut self.running
                        && let Some(c) = &mut run.client
                    {
                        c.act_by_hand(process, aim, hand);
                    }
                    self.set_captured(true);
                }
                MenuAction::Shift { from, count, to } => {
                    if let Some(c) = self.running.as_mut().and_then(|r| r.client.as_mut()) {
                        c.shift(from, count, to);
                    }
                }
                MenuAction::PutDown { from, count } => {
                    if let Some(c) = self.running.as_mut().and_then(|r| r.client.as_mut()) {
                        c.put_down_from(from, count);
                    }
                }
                MenuAction::Eat(from) => {
                    if let Some(c) = self.running.as_mut().and_then(|r| r.client.as_mut()) {
                        c.eat(from);
                    }
                }
                MenuAction::Restart => {
                    // The world begun again from its seed and settings, the old one archived.
                    let spec = self
                        .running
                        .as_mut()
                        .and_then(|r| r.client.take())
                        .map(|c| {
                            // Begun again in the mode it was played in.
                            let mut spec = c.world_spec().clone();
                            spec.mode = c.rules.as_ref().map(|r| r.id.clone()).or(spec.mode);
                            spec
                        });
                    if let Some(spec) = spec {
                        let saves = self.dirs.saves();
                        let old = saves.join(&spec.name);
                        if old.exists() {
                            let mut n = 1;
                            while saves.join(format!("{} (life {n})", spec.name)).exists() {
                                n += 1;
                            }
                            if let Err(e) = std::fs::rename(
                                &old,
                                saves.join(format!("{} (life {n})", spec.name)),
                            ) {
                                log::error!("could not archive the world: {e}");
                            }
                        }
                        self.play(
                            &spec.name,
                            spec.seed,
                            spec.knowledge,
                            &spec.era,
                            NewShape {
                                size: spec.planet,
                                shape: spec.shape,
                                birthplace: spec.birthplace,
                                mode: spec.mode.clone(),
                            },
                        );
                    }
                }
                MenuAction::NewLife { elsewhere } => {
                    if let Some(run) = &mut self.running {
                        run.menus.close_all();
                        if let Some(c) = &mut run.client {
                            c.new_life(elsewhere);
                        }
                    }
                }
                MenuAction::Resume => {
                    if let Some(run) = &mut self.running {
                        run.menus.close_all();
                        if let Some(c) = &mut run.client {
                            c.pause(false);
                        }
                    }
                }
                MenuAction::QuitToTitle => {
                    self.set_captured(false);
                    if let Some(run) = &mut self.running {
                        // Dropping the client stops its server, which saves.
                        run.client = None;
                        run.menus = Menus::title();
                    }
                }
                MenuAction::QuitGame => self.exit_requested = true,
                MenuAction::ProfilesChanged => self.save_profiles(),
                MenuAction::LanguageChanged => {
                    if let Some(run) = &mut self.running {
                        run.interface.set_language(&self.options.language);
                    }
                }
                MenuAction::OptionsChanged => {
                    self.input.set_options(InputOptions {
                        toggle_sneak: self.options.controls.toggle_sneak,
                        toggle_sprint: self.options.controls.toggle_sprint,
                    });
                    let present = self.present_preference();
                    if let Some(run) = &mut self.running {
                        run.renderer.set_present(present);
                        if let Some(c) = &mut run.client {
                            c.apply_options(&self.options);
                        }
                    }
                    self.apply_display_mode();
                    if self.options.sound.device != self.audio_asked {
                        self.open_audio();
                    } else {
                        self.send_volumes();
                    }
                    self.save_options();
                }
            }
        }
    }

    /// Captures (hides and locks) or releases the mouse cursor.
    fn set_captured(&mut self, captured: bool) {
        let Some(run) = &mut self.running else { return };
        if run.captured == captured {
            return;
        }
        if captured {
            let grabbed = run
                .window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| run.window.set_cursor_grab(CursorGrabMode::Confined));
            if let Err(e) = grabbed {
                log::warn!("could not capture the mouse: {e}");
                return;
            }
            run.window.set_cursor_visible(false);
        } else {
            let _ = run.window.set_cursor_grab(CursorGrabMode::None);
            run.window.set_cursor_visible(true);
        }
        run.captured = captured;
    }

    /// What the controller did this frame: its buttons go where keys go (to a binding being
    /// captured, or to play), and its steps move through an open menu.
    fn pad_input(&mut self, polled: Polled) {
        let Some(run) = &self.running else {
            return;
        };
        // As things were before this frame's buttons (Start may open the pause screen).
        let menu = run.menus.is_open() && !run.menus.capturing();
        for (b, pressed) in polled.buttons {
            self.handle_key(InputKey::Pad(b), pressed);
        }
        let Some(run) = &mut self.running else {
            return;
        };
        if !menu || !run.menus.is_open() {
            return;
        }
        for p in polled.nav {
            let nav = match p {
                Press::Up => NavKey::Up,
                Press::Down => NavKey::Down,
                Press::Left => NavKey::Left,
                Press::Right => NavKey::Right,
                Press::Enter => NavKey::Enter,
                Press::Back => {
                    if let Some(a) = run.menus.back() {
                        self.menu_actions(vec![a]);
                        return;
                    }
                    continue;
                }
            };
            run.interface.key(nav);
        }
    }

    fn frame(&mut self) {
        self.follow_making();
        let sensitivity = self.options.controls.mouse_sensitivity;
        let invert = self.options.controls.invert_y;
        let pad_sensitivity = self.options.controls.controller_sensitivity;
        let polled = self.pads.poll(self.start.elapsed().as_secs_f64());
        if !polled.buttons.is_empty() || !polled.nav.is_empty() {
            self.pad_input(polled);
        }
        let pad = self.pads.pad;
        let mut actions = Vec::new();
        let mut frame_dt = 0.0;
        let eras = self.eras();
        let modes = self.modes();
        if let Some(run) = &mut self.running {
            let now = Instant::now();
            let dt = (now - run.last_frame).as_secs_f64().min(0.25);
            run.last_frame = now;
            frame_dt = dt;
            let menu_open = run.menus.is_open();
            let look = run.captured.then(|| {
                let (dx, dy) = self.input.mouse_delta();
                (dx, if invert { -dy } else { dy })
            });
            if let Some(c) = &mut run.client {
                c.pump(&run.renderer.ctx);
                if c.dead() && !run.menus.is_open() {
                    run.menus.open(Screen::Death);
                    if run.captured {
                        run.captured = false;
                        let _ = run.window.set_cursor_grab(CursorGrabMode::None);
                        run.window.set_cursor_visible(true);
                    }
                }
                if !menu_open {
                    c.update(
                        dt,
                        &mut self.input,
                        look,
                        sensitivity,
                        &pad,
                        pad_sensitivity,
                    );
                    // Knapping by hand opens its screen.
                    if let Some(k) = c.knap_request.take() {
                        run.menus.open(Screen::Knapping(Box::new(k)));
                        if run.captured {
                            run.captured = false;
                            let _ = run.window.set_cursor_grab(CursorGrabMode::None);
                            run.window.set_cursor_visible(true);
                        }
                    }
                }
            }
            let client = &mut run.client;
            let choosing = &mut self.choosing;
            let interface = &mut run.interface;
            let menus = &mut run.menus;
            let options = &mut self.options;
            let bindings = &mut self.bindings;
            let saves = self.dirs.saves();
            let languages = &self.languages;
            let audio_devices = &self.audio_devices;
            let profiles = &mut self.profiles;
            let format = run.renderer.color_format();
            if run.renderer.render_with(|ctx, enc, targets| {
                match client.as_mut() {
                    Some(c) => c.render(ctx, enc, targets, dt as f32),
                    None => clear(enc, targets.color),
                }
                // A new world's globe, its birthplace being chosen, under the screen.
                if client.is_none()
                    && let Some(ch) = choosing.as_mut()
                {
                    let at = ch.picker.view;
                    ch.picker.render(
                        ctx,
                        enc,
                        targets.color,
                        targets.size,
                        format,
                        (at.lat, at.lon),
                    );
                }
                let gui = options.video.gui_scale;
                let backdrop = options.accessibility.text_background_opacity;
                interface.frame(ctx, enc, targets.color, format, targets.size, gui, |ui| {
                    if let Some(c) = client.as_ref() {
                        c.hud(ui, backdrop);
                    }
                    let mut cx = MenuContext {
                        options,
                        bindings,
                        saves,
                        in_game: client.is_some(),
                        languages,
                        audio_devices,
                        profiles,
                        death: client.as_ref().and_then(|c| c.death_info(ui.lang)),
                        inventory: client.as_ref().and_then(|c| c.inventory_view()),
                        journal: client.as_ref().and_then(|c| c.journal_view()),
                        eras: eras.clone(),
                        modes: modes.clone(),
                        time_words: client.as_ref().and_then(|c| c.time_words(ui.lang)),
                        may_watch: client.as_ref().is_none_or(|c| c.may_watch()),
                        creative: client.as_ref().is_some_and(|c| c.creative()),
                        catalog: client.as_ref().map_or(&[][..], |c| &c.catalog[..]),
                        instant: client.as_ref().is_none_or(|c| c.instant),
                        clear_view: client.as_ref().map(|c| c.clear_view).unwrap_or_default(),
                        globe: choosing.as_mut().map(|ch| crate::menus::GlobeContext {
                            picker: &mut ch.picker,
                            terrain: ch.terrain.clone(),
                        }),
                    };
                    actions = menus.ui(ui, &mut cx);
                });
            }) {
                self.frames_rendered += 1;
                run.title_frames += 1;
            }
            let elapsed = run.title_timer.elapsed().as_secs_f64();
            // The globe describes the place under the cursor: keep up with it.
            let globe = run.client.as_ref().is_some_and(|c| c.globe.open);
            let period = if globe { 0.1 } else { 0.5 };
            if elapsed >= period {
                let fps = run.title_frames as f64 / elapsed;
                let status = match &mut run.client {
                    Some(c) => {
                        c.fps = fps;
                        c.title_status(fps)
                    }
                    None => format!("{fps:.0} fps"),
                };
                run.window
                    .set_title(&format!("{} | {status}", hearth_core::window_title()));
                if run.log_timer.elapsed().as_secs() >= 5 {
                    log::info!("{status}");
                    run.log_timer = Instant::now();
                }
                run.title_timer = Instant::now();
                run.title_frames = 0;
            }
        }
        self.sound(frame_dt);
        if !actions.is_empty() {
            self.menu_actions(actions);
        }
        self.input.end_frame();
    }

    /// Error that terminated the app, if any.
    pub fn fatal_error(&self) -> Option<&str> {
        self.fatal_error.as_deref()
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.running.is_some() {
            return;
        }
        let ws = &self.options.window;
        let mut attrs = Window::default_attributes()
            .with_title(hearth_core::window_title())
            .with_inner_size(PhysicalSize::new(ws.size[0].max(320), ws.size[1].max(240)))
            .with_min_inner_size(PhysicalSize::new(320, 240))
            .with_maximized(ws.maximized);
        if let Some([x, y]) = ws.position {
            attrs = attrs.with_position(PhysicalPosition::new(x, y));
        }
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.fatal_error = Some(format!("could not create window: {e}"));
                event_loop.exit();
                return;
            }
        };
        let renderer = match Renderer::new(window.clone(), self.present_preference()) {
            Ok(r) => r,
            Err(e) => {
                self.fatal_error = Some(format!("could not initialise graphics: {e}"));
                event_loop.exit();
                return;
            }
        };
        self.running = Some(Running {
            window,
            renderer,
            client: None,
            interface: Interface::new(&self.options.language),
            menus: Menus::title(),
            captured: false,
            last_frame: Instant::now(),
            title_timer: Instant::now(),
            title_frames: 0,
            log_timer: Instant::now(),
        });
        let t = Instant::now();
        self.open_audio();
        self.audio_devices = hearth_audio::output_devices();
        log::info!(
            "sound opened in {:.0} ms ({} output devices)",
            t.elapsed().as_secs_f64() * 1000.0,
            self.audio_devices.len()
        );
        if let Some(world) = self.world.clone() {
            self.play(
                &world,
                self.seed,
                hearth_save::KnowledgeMode::default(),
                crate::eras::WILD_EARTH,
                NewShape::default(),
            );
        }
        self.apply_display_mode();
        event_loop.set_control_flow(ControlFlow::Poll);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.exit_requested = true;
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(run) = &mut self.running {
                    run.renderer.resize(size.width, size.height);
                }
                self.remember_window_placement();
            }
            WindowEvent::Moved(_) => self.remember_window_placement(),
            WindowEvent::Focused(false) => {
                self.input.release_all();
                if let Some(run) = &mut self.running {
                    run.menus.capture_reset();
                }
                self.set_captured(false);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed
                    && let Some(text) = &event.text
                    && let Some(run) = &mut self.running
                    && run.menus.is_open()
                {
                    run.interface.typed(text);
                }
                if let PhysicalKey::Code(code) = event.physical_key
                    && let Some(key) = Key::from_winit(code)
                {
                    self.handle_key(
                        InputKey::Keyboard(key),
                        event.state == ElementState::Pressed,
                    );
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.handle_key(
                    InputKey::Mouse(MouseButton::from_winit(button)),
                    state == ElementState::Pressed,
                );
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(run) = &mut self.running {
                    run.interface
                        .pointer_moved(position.x as f32, position.y as f32);
                    if let Some(c) = &mut run.client {
                        c.globe
                            .cursor_moved(glam::Vec2::new(position.x as f32, position.y as f32));
                    }
                }
            }
            WindowEvent::CursorLeft { .. } => {
                if let Some(run) = &mut self.running {
                    run.interface.pointer_left();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(p) => p.y / 40.0,
                };
                match &mut self.running {
                    Some(run) if run.menus.is_open() => run.interface.scroll(lines as f32),
                    _ => self.input.add_scroll(lines),
                }
            }
            WindowEvent::RedrawRequested => self.frame(),
            _ => {}
        }
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.input.add_mouse_motion(delta.0, delta.1);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.exit_requested {
            event_loop.exit();
            return;
        }
        if self.quit_after.is_some_and(|d| self.start.elapsed() >= d) {
            log::info!(
                "auto-quit after {:.1}s, {} frames rendered",
                self.start.elapsed().as_secs_f64(),
                self.frames_rendered
            );
            self.exit_requested = true;
            event_loop.exit();
            return;
        }
        let cap = if self.options.video.vsync || self.options.video.unlimited_framerate() {
            None
        } else {
            Some(self.options.video.max_framerate)
        };
        self.limiter.wait(cap);
        if let Some(run) = &self.running {
            run.window.request_redraw();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.remember_window_placement();
        self.save_options();
    }
}

/// The game directory: the explicit one if given, else the standard resolution order.
pub fn resolve_dirs(explicit: Option<GameDirs>) -> GameDirs {
    explicit.unwrap_or_else(|| {
        GameDirs::resolve(
            None,
            directories::ProjectDirs::from("", "", hearth_core::GAME_NAME)
                .map(|d| d.data_dir().to_path_buf()),
        )
    })
}

/// Runs the game until the window is closed.
pub fn run(config: LaunchConfig) -> anyhow::Result<()> {
    let dirs = resolve_dirs(config.dirs);
    log::info!("game directory: {}", dirs.root.display());
    let event_loop = EventLoop::new()?;
    let mut app = App::new(
        dirs,
        config.quit_after,
        config.seed.unwrap_or(1),
        config.world,
    );
    event_loop.run_app(&mut app)?;
    if let Some(err) = app.fatal_error() {
        anyhow::bail!("{err}");
    }
    Ok(())
}

/// Sleep granularity helper used by the limiter; exposed for tests.
pub(crate) fn frame_budget(fps: u32) -> Duration {
    Duration::from_secs_f64(1.0 / fps.max(1) as f64)
}

/// Clears a frame with nothing to draw but the interface.
fn clear(enc: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
    let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: 0.005,
                    g: 0.007,
                    b: 0.011,
                    a: 1.0,
                }),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
}

//! The windowed application: event loop, window management (display modes, remembered
//! placement), input routing and the frame loop.

use std::sync::Arc;
use std::time::{Duration, Instant};

use hearth_core::options::{DisplayMode, Options, PresentModePref};
use hearth_core::paths::GameDirs;
use hearth_input::{InputKey, InputOptions, InputState, Key, KeyBindings, MouseButton, builtin};
use hearth_render::{PresentPreference, Renderer};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowId};

use crate::content_state::ContentState;
use crate::frame_limiter::FrameLimiter;
use crate::preview::Preview;

/// Command-line configuration of a run.
#[derive(Debug, Clone, Default)]
pub struct LaunchConfig {
    pub dirs: Option<GameDirs>,
    /// Exit cleanly (saving options) after this long; used by automated smoke tests.
    pub quit_after: Option<Duration>,
    /// Seed of the preview world.
    pub seed: Option<u64>,
}

struct Running {
    window: Arc<Window>,
    renderer: Renderer,
    preview: Preview,
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
    content: ContentState,
}

impl App {
    pub fn new(dirs: GameDirs, quit_after: Option<Duration>, seed: u64) -> Self {
        if let Err(e) = dirs.ensure_created() {
            log::warn!(
                "could not create game directory {}: {e}",
                dirs.root.display()
            );
        }
        let options = Options::load_or_default(&dirs.options_file());
        let bindings = KeyBindings::from_map(
            hearth_input::ActionRegistry::with_builtins(),
            &options.controls.key_bindings,
        );
        let mut input = InputState::new(bindings.registry().len());
        input.set_options(InputOptions {
            toggle_sneak: options.controls.toggle_sneak,
            toggle_sprint: options.controls.toggle_sprint,
        });
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
            content: ContentState::load(vec![crate::scene::data_pack_dir()]),
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
        if pressed {
            let activated: Vec<_> = self.input.press(key, &self.bindings).to_vec();
            for action in activated {
                log::debug!("action {}", self.bindings.registry().def(action).id);
                if action == builtin::FULLSCREEN {
                    self.toggle_fullscreen();
                } else if action == builtin::PAUSE {
                    self.set_captured(false);
                } else if action == builtin::DEBUG_RELOAD_RESOURCES {
                    self.content.reload();
                } else if let Some(run) = &mut self.running {
                    let p = &mut run.preview;
                    if action == builtin::DEBUG_TIME_FORWARD {
                        p.skip_hours(1.0);
                    } else if action == builtin::DEBUG_TIME_BACK {
                        p.skip_hours(-1.0);
                    } else if action == builtin::DEBUG_SEASON_FORWARD {
                        p.skip_hours(24.0 * p.calendar.days_per_season as f64);
                    } else if action == builtin::DEBUG_TIME_WARP {
                        // Off → one game hour per real second → off.
                        p.time_warp = if p.time_warp > 0.0 {
                            0.0
                        } else {
                            p.calendar.ticks_per_day() / 24.0
                        };
                    }
                }
            }
            if key == InputKey::Mouse(MouseButton::Left) {
                self.set_captured(true);
            }
        } else {
            self.input.release(key, &self.bindings);
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

    fn frame(&mut self) {
        let sensitivity = self.options.controls.mouse_sensitivity;
        if let Some(run) = &mut self.running {
            let now = Instant::now();
            let dt = (now - run.last_frame).as_secs_f64().min(0.25);
            run.last_frame = now;
            let look = run.captured.then(|| self.input.mouse_delta());
            run.preview.pump(&run.renderer.ctx);
            run.preview.update(dt, &mut self.input, look, sensitivity);
            let preview = &mut run.preview;
            if run
                .renderer
                .render_with(|ctx, enc, targets| preview.render(ctx, enc, targets, dt as f32))
            {
                self.frames_rendered += 1;
                run.title_frames += 1;
            }
            let elapsed = run.title_timer.elapsed().as_secs_f64();
            if elapsed >= 0.5 {
                let fps = run.title_frames as f64 / elapsed;
                run.window.set_title(&format!(
                    "{} | {}",
                    hearth_core::window_title(),
                    run.preview.title_status(fps)
                ));
                if run.log_timer.elapsed().as_secs() >= 5 {
                    log::info!("{}", run.preview.title_status(fps));
                    run.log_timer = Instant::now();
                }
                run.title_timer = Instant::now();
                run.title_frames = 0;
            }
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
        let preview = Preview::new(
            Preview::default_world(self.seed, Some(self.dirs.cache())),
            &self.options,
            renderer.color_format(),
            self.content.content.as_deref(),
        );
        self.running = Some(Running {
            window,
            renderer,
            preview,
            captured: false,
            last_frame: Instant::now(),
            title_timer: Instant::now(),
            title_frames: 0,
            log_timer: Instant::now(),
        });
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
                self.set_captured(false);
            }
            WindowEvent::KeyboardInput { event, .. } => {
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
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(p) => p.y / 40.0,
                };
                self.input.add_scroll(lines);
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
    let mut app = App::new(dirs, config.quit_after, config.seed.unwrap_or(1));
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

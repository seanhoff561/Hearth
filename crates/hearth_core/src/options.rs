//! Player options persisted to `options.toml`.
//!
//! Every field has a default so that older or hand-edited files load cleanly: unknown keys are
//! ignored and missing keys take their defaults. Values are clamped into their valid ranges on
//! load ([`Options::sanitize`]) so a bad edit can never put the game into an invalid state.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Current schema version of the options file.
pub const OPTIONS_VERSION: u32 = 1;

/// Errors from loading or saving options.
#[derive(Debug, thiserror::Error)]
pub enum OptionsError {
    #[error("failed to read or write options file: {0}")]
    Io(#[from] std::io::Error),
    #[error("options file is not valid TOML: {0}")]
    Parse(String),
    #[error("failed to serialize options: {0}")]
    Serialize(String),
}

/// All persisted player options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub version: u32,
    pub video: VideoOptions,
    pub controls: ControlOptions,
    pub sound: SoundOptions,
    pub accessibility: AccessibilityOptions,
    pub window: WindowState,
    pub performance: PerformanceOptions,
    /// Language code, e.g. `en_us`.
    pub language: String,
    /// Developer mode (Amendment P §2): the debug screen (F3) shows everything in every mode,
    /// not only how the game performs, and Creative's clear view is open.
    pub developer_mode: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            version: OPTIONS_VERSION,
            video: VideoOptions::default(),
            controls: ControlOptions::default(),
            sound: SoundOptions::default(),
            accessibility: AccessibilityOptions::default(),
            window: WindowState::default(),
            performance: PerformanceOptions::default(),
            language: "en_us".to_owned(),
            developer_mode: false,
        }
    }
}

impl Options {
    /// Parses options from TOML text and sanitizes the result.
    pub fn from_toml_str(text: &str) -> Result<Self, OptionsError> {
        let mut opts: Options = toml_parse(text).map_err(|e| OptionsError::Parse(e.to_string()))?;
        opts.sanitize();
        Ok(opts)
    }

    /// Serializes to pretty TOML.
    pub fn to_toml_string(&self) -> Result<String, OptionsError> {
        toml_serialize(self).map_err(|e| OptionsError::Serialize(e.to_string()))
    }

    /// Loads options from `path`. A missing file yields defaults; a corrupt file is reported
    /// as an error so the caller can back it up before overwriting.
    pub fn load(path: &Path) -> Result<Self, OptionsError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::from_toml_str(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Loads options, falling back to defaults (and keeping a `.bak` copy of the unreadable
    /// file) if the file is corrupt.
    pub fn load_or_default(path: &Path) -> Self {
        match Self::load(path) {
            Ok(o) => o,
            Err(err) => {
                log::warn!("could not load {}: {err}; using defaults", path.display());
                let backup = path.with_extension("toml.bak");
                if let Err(e) = std::fs::copy(path, &backup) {
                    log::warn!("could not back up broken options file: {e}");
                }
                Self::default()
            }
        }
    }

    /// Writes options atomically (write to a temp file, then rename).
    pub fn save(&self, path: &Path) -> Result<(), OptionsError> {
        let text = self.to_toml_string()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Clamps every numeric option into its valid range and repairs inconsistent state.
    pub fn sanitize(&mut self) {
        self.version = OPTIONS_VERSION;
        self.video.sanitize();
        self.controls.sanitize();
        self.sound.sanitize();
        self.accessibility.sanitize();
        if self.language.trim().is_empty() {
            self.language = "en_us".to_owned();
        }
    }
}

// The `toml` crate is only needed by this module; keep the calls in one place.
fn toml_parse<T: for<'de> Deserialize<'de>>(text: &str) -> Result<T, impl std::fmt::Display> {
    toml::from_str(text)
}

fn toml_serialize<T: Serialize>(value: &T) -> Result<String, impl std::fmt::Display> {
    toml::to_string_pretty(value)
}

/// Overall graphics quality preset. Changing any preset-controlled sub-setting switches the
/// preset to `Custom`. The names of the original game's presets load as their equivalents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphicsPreset {
    #[serde(alias = "fast")]
    Low,
    #[serde(alias = "fancy")]
    Medium,
    #[serde(alias = "fabulous")]
    High,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentModePref {
    /// Pick based on the VSync toggle (FIFO when on, Mailbox → Immediate when off).
    Auto,
    Immediate,
    Mailbox,
    Fifo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayMode {
    Windowed,
    Borderless,
    Exclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AntiAliasing {
    /// Off (an options file's `fxaa`, never built, loads as off).
    #[serde(alias = "fxaa")]
    Off,
    Taa,
}

/// Generic four-step quality level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Off,
    Low,
    Medium,
    High,
}

/// Shader quality group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShaderOptions {
    pub water: Quality,
}

impl Default for ShaderOptions {
    fn default() -> Self {
        Self {
            water: Quality::Medium,
        }
    }
}

/// How the game uses the machine (E4.1 §4.6, §4.7, §6): each 0 is chosen from the machine.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PerformanceOptions {
    /// Worker threads generating the world about the player; 0: the machine's cores less those
    /// kept free.
    pub workers: u32,
    /// Cores kept free for the main thread, rendering and sound; 0: one, two from eight cores.
    pub reserved_cores: u32,
    /// The caches' share of the machine's memory (%); 0: forty.
    pub memory_share: u32,
    /// Whether the first run's look at the machine set the video settings to suit it.
    pub machine_checked: bool,
}

/// Video settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoOptions {
    pub graphics: GraphicsPreset,
    /// Full-detail horizontal radius in chunks (2–32).
    pub render_distance: u32,
    /// Full-detail cubes above and below the player (4–32).
    pub vertical_render_distance: u32,
    /// Level-of-detail terrain radius in chunks; 0 disables LOD (0–4096).
    pub lod_distance: u32,
    /// Video memory budget for LOD tiles in MiB.
    pub lod_vram_budget_mb: u32,
    /// Detail of rough distant terrain: how far its steps may stray on screen (`lod_error_px`).
    pub lod_detail: Quality,
    /// Frame rate cap (10–260); 0 means unlimited.
    pub max_framerate: u32,
    pub vsync: bool,
    pub present_mode: PresentModePref,
    pub display_mode: DisplayMode,
    /// Exclusive-fullscreen resolution; `None` uses the monitor's current mode.
    pub exclusive_resolution: Option<[u32; 2]>,
    /// Exclusive-fullscreen refresh rate in millihertz; `None` picks the highest available.
    pub exclusive_refresh_millihertz: Option<u32>,
    /// Monitor name to use for fullscreen modes; `None` uses the window's current monitor.
    pub monitor: Option<String>,
    /// GUI scale; 0 = auto.
    pub gui_scale: u32,
    /// Vertical field of view in degrees (30–110).
    pub fov: f32,
    pub view_bobbing: bool,
    pub anti_aliasing: AntiAliasing,
    /// Internal render resolution scale (0.5–2.0): below 1 the frame is upscaled (FSR 1),
    /// above 1 filtered down. 1 in every preset.
    pub render_scale: f32,
    pub shader: ShaderOptions,
}

impl Default for VideoOptions {
    fn default() -> Self {
        let mut v = Self {
            graphics: GraphicsPreset::Medium,
            render_distance: 12,
            vertical_render_distance: 8,
            lod_distance: 256,
            lod_vram_budget_mb: 1024,
            lod_detail: Quality::Medium,
            max_framerate: 120,
            vsync: true,
            present_mode: PresentModePref::Auto,
            display_mode: DisplayMode::Windowed,
            exclusive_resolution: None,
            exclusive_refresh_millihertz: None,
            monitor: None,
            gui_scale: 0,
            fov: 70.0,
            view_bobbing: true,
            anti_aliasing: AntiAliasing::Off,
            render_scale: 1.0,
            shader: ShaderOptions::default(),
        };
        v.apply_preset(GraphicsPreset::Medium);
        v
    }
}

/// The subset of video settings a graphics preset controls.
#[derive(Debug, Clone, PartialEq)]
struct PresetValues {
    lod_detail: Quality,
    anti_aliasing: AntiAliasing,
    water: Quality,
}

impl PresetValues {
    fn for_preset(preset: GraphicsPreset) -> Self {
        let (lod_detail, anti_aliasing, water) = match preset {
            GraphicsPreset::Low => (Quality::Low, AntiAliasing::Off, Quality::Low),
            GraphicsPreset::Medium | GraphicsPreset::Custom => {
                (Quality::Medium, AntiAliasing::Off, Quality::Medium)
            }
            GraphicsPreset::High => (Quality::High, AntiAliasing::Taa, Quality::High),
        };
        Self {
            lod_detail,
            anti_aliasing,
            water,
        }
    }

    fn of(v: &VideoOptions) -> Self {
        Self {
            lod_detail: v.lod_detail,
            anti_aliasing: v.anti_aliasing,
            water: v.shader.water,
        }
    }
}

impl VideoOptions {
    /// Applies a preset's sub-settings. `Custom` leaves everything as is.
    pub fn apply_preset(&mut self, preset: GraphicsPreset) {
        self.graphics = preset;
        if preset == GraphicsPreset::Custom {
            return;
        }
        let p = PresetValues::for_preset(preset);
        self.lod_detail = p.lod_detail;
        self.anti_aliasing = p.anti_aliasing;
        self.shader.water = p.water;
    }

    /// Returns the preset whose sub-settings exactly match the current values, or `Custom`.
    pub fn detect_preset(&self) -> GraphicsPreset {
        let current = PresetValues::of(self);
        [
            GraphicsPreset::Low,
            GraphicsPreset::Medium,
            GraphicsPreset::High,
        ]
        .into_iter()
        .find(|p| PresetValues::for_preset(*p) == current)
        .unwrap_or(GraphicsPreset::Custom)
    }

    /// Call after changing any preset-controlled sub-setting: switches the preset label to
    /// `Custom` when the values no longer match the selected preset.
    pub fn refresh_preset(&mut self) {
        self.graphics = self.detect_preset();
    }

    /// True if the framerate is uncapped.
    pub fn unlimited_framerate(&self) -> bool {
        self.max_framerate == 0
    }

    /// Vertical error of distant terrain allowed on screen, in pixels: rough land gets finer
    /// tiles until the steps between its columns stray no more than this from what finer columns
    /// would show. 0 keeps the distance rule alone (columns about 3–6 pixels wide).
    pub fn lod_error_px(&self) -> f64 {
        match self.lod_detail {
            Quality::Off => 0.0,
            Quality::Low => 4.0,
            Quality::Medium => 2.0,
            Quality::High => 1.0,
        }
    }

    fn sanitize(&mut self) {
        self.render_distance = self.render_distance.clamp(2, 32);
        self.vertical_render_distance = self.vertical_render_distance.clamp(4, 32);
        self.lod_distance = self.lod_distance.min(4096);
        self.lod_vram_budget_mb = self.lod_vram_budget_mb.clamp(64, 16384);
        if self.max_framerate != 0 {
            self.max_framerate = self.max_framerate.clamp(10, 260);
        }
        self.fov = finite_clamp(self.fov, 30.0, 110.0, 70.0);
        self.render_scale = finite_clamp(self.render_scale, 0.5, 2.0, 1.0);
        if self.graphics != GraphicsPreset::Custom {
            // A hand-edited file may claim a preset whose values it doesn't match.
            self.refresh_preset();
        }
    }
}

fn finite_clamp(v: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        fallback
    }
}

/// Mouse, keyboard and movement-style options. Key bindings are stored as a map from action id
/// to binding string and interpreted by `hearth_input`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ControlOptions {
    /// Mouse sensitivity 0–1 (0.5 is the familiar default).
    pub mouse_sensitivity: f32,
    pub invert_y: bool,
    pub toggle_sneak: bool,
    pub toggle_sprint: bool,
    /// Controller look sensitivity 0–1.
    pub controller_sensitivity: f32,
    /// Action id → binding string, e.g. `"key.forward" = "w"`, `"key.drop_stack" = "ctrl+x"`.
    /// Missing entries use the action's default binding.
    pub key_bindings: BTreeMap<String, String>,
    /// Action id → controller button, e.g. `"key.jump" = "pad.south"`, beside the keys.
    pub pad_bindings: BTreeMap<String, String>,
    /// A thing put away to free a hand comes back to it afterwards (P §5.2).
    pub return_to_hand: bool,
    /// The name of what is looked at by the crosshair (P §5.1).
    pub name_tags: NameTags,
    /// What each hand would do, by the crosshair.
    pub hand_hints: bool,
}

/// When the name of what is looked at shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NameTags {
    Off,
    /// A moment after the look comes to rest on something, then it fades.
    #[default]
    Brief,
    Always,
}

impl Default for ControlOptions {
    fn default() -> Self {
        Self {
            mouse_sensitivity: 0.5,
            invert_y: false,
            toggle_sneak: false,
            toggle_sprint: false,
            controller_sensitivity: 0.5,
            key_bindings: BTreeMap::new(),
            pad_bindings: BTreeMap::new(),
            return_to_hand: true,
            name_tags: NameTags::Brief,
            hand_hints: true,
        }
    }
}

impl ControlOptions {
    fn sanitize(&mut self) {
        self.mouse_sensitivity = finite_clamp(self.mouse_sensitivity, 0.0, 1.0, 0.5);
        self.controller_sensitivity = finite_clamp(self.controller_sensitivity, 0.0, 1.0, 0.5);
    }
}

/// Sound volume categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoundCategory {
    Master,
    Music,
    Weather,
    Blocks,
    Hostile,
    Friendly,
    Players,
    Ambient,
    Ui,
}

impl SoundCategory {
    pub const ALL: [SoundCategory; 9] = [
        SoundCategory::Master,
        SoundCategory::Music,
        SoundCategory::Weather,
        SoundCategory::Blocks,
        SoundCategory::Hostile,
        SoundCategory::Friendly,
        SoundCategory::Players,
        SoundCategory::Ambient,
        SoundCategory::Ui,
    ];
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundOptions {
    pub master: f32,
    pub music: f32,
    pub weather: f32,
    pub blocks: f32,
    pub hostile: f32,
    pub friendly: f32,
    pub players: f32,
    pub ambient: f32,
    pub ui: f32,
    /// Output device name; empty = system default.
    pub device: String,
    pub subtitles: bool,
}

impl Default for SoundOptions {
    fn default() -> Self {
        Self {
            master: 1.0,
            music: 0.6,
            weather: 1.0,
            blocks: 1.0,
            hostile: 1.0,
            friendly: 1.0,
            players: 1.0,
            ambient: 1.0,
            ui: 1.0,
            device: String::new(),
            subtitles: false,
        }
    }
}

impl SoundOptions {
    pub fn volume(&self, cat: SoundCategory) -> f32 {
        match cat {
            SoundCategory::Master => self.master,
            SoundCategory::Music => self.music,
            SoundCategory::Weather => self.weather,
            SoundCategory::Blocks => self.blocks,
            SoundCategory::Hostile => self.hostile,
            SoundCategory::Friendly => self.friendly,
            SoundCategory::Players => self.players,
            SoundCategory::Ambient => self.ambient,
            SoundCategory::Ui => self.ui,
        }
    }

    pub fn volume_mut(&mut self, cat: SoundCategory) -> &mut f32 {
        match cat {
            SoundCategory::Master => &mut self.master,
            SoundCategory::Music => &mut self.music,
            SoundCategory::Weather => &mut self.weather,
            SoundCategory::Blocks => &mut self.blocks,
            SoundCategory::Hostile => &mut self.hostile,
            SoundCategory::Friendly => &mut self.friendly,
            SoundCategory::Players => &mut self.players,
            SoundCategory::Ambient => &mut self.ambient,
            SoundCategory::Ui => &mut self.ui,
        }
    }

    /// Effective linear gain of a category (category volume × master).
    pub fn effective(&self, cat: SoundCategory) -> f32 {
        if cat == SoundCategory::Master {
            self.master
        } else {
            self.master * self.volume(cat)
        }
    }

    fn sanitize(&mut self) {
        for cat in SoundCategory::ALL {
            let v = self.volume_mut(cat);
            *v = finite_clamp(*v, 0.0, 1.0, 1.0);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccessibilityOptions {
    /// Opacity of the background behind subtitle text (0–1).
    pub text_background_opacity: f32,
    /// Reduce camera shake, flashes and pulsing effects.
    pub reduce_motion: bool,
    /// Compact bars for food, water, warmth, rest, stamina and blood (v2 §9.9's "Guided" HUD)
    /// besides the body's sensations.
    pub guided_hud: bool,
    /// Work goes on from one click of the hand's button to the next, instead of while it is
    /// held (E §7.2).
    pub toggle_work: bool,
}

impl Default for AccessibilityOptions {
    fn default() -> Self {
        Self {
            text_background_opacity: 0.5,
            reduce_motion: false,
            guided_hud: false,
            toggle_work: false,
        }
    }
}

impl AccessibilityOptions {
    fn sanitize(&mut self) {
        self.text_background_opacity = finite_clamp(self.text_background_opacity, 0.0, 1.0, 0.5);
    }
}

/// Remembered window placement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowState {
    /// Outer position in physical pixels, if known.
    pub position: Option<[i32; 2]>,
    /// Inner size in physical pixels.
    pub size: [u32; 2],
    pub maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            position: None,
            size: [1600, 900],
            maximized: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_round_trips_through_toml() {
        let mut o = Options::default();
        o.controls
            .key_bindings
            .insert("key.forward".into(), "up".into());
        o.video.exclusive_resolution = Some([1920, 1080]);
        let text = o.to_toml_string().unwrap();
        let back = Options::from_toml_str(&text).unwrap();
        assert_eq!(o, back);
    }

    #[test]
    fn missing_and_unknown_keys_are_tolerated() {
        let text = r#"
            language = "en_us"
            some_future_key = 3
            [video]
            fov = 90.0
        "#;
        let o = Options::from_toml_str(text).unwrap();
        assert_eq!(o.video.fov, 90.0);
        assert_eq!(
            o.video.render_distance,
            VideoOptions::default().render_distance
        );
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        let text = r#"
            [video]
            render_distance = 99
            fov = 500.0
            max_framerate = 3
            [sound]
            master = 4.0
        "#;
        let o = Options::from_toml_str(text).unwrap();
        assert_eq!(o.video.render_distance, 32);
        assert_eq!(o.video.fov, 110.0);
        assert_eq!(o.video.max_framerate, 10);
        assert_eq!(o.sound.master, 1.0);
    }

    #[test]
    fn unlimited_framerate_is_zero() {
        let text = "[video]\nmax_framerate = 0\n";
        let o = Options::from_toml_str(text).unwrap();
        assert!(o.video.unlimited_framerate());
    }

    #[test]
    fn preset_switches_to_custom_on_change() {
        let mut v = VideoOptions::default();
        assert_eq!(v.graphics, GraphicsPreset::Medium);
        v.apply_preset(GraphicsPreset::High);
        assert_eq!(v.detect_preset(), GraphicsPreset::High);
        v.shader.water = Quality::Low;
        v.refresh_preset();
        assert_eq!(v.graphics, GraphicsPreset::Custom);
        v.apply_preset(GraphicsPreset::Low);
        assert_eq!(v.graphics, GraphicsPreset::Low);
        assert_eq!(v.detect_preset(), GraphicsPreset::Low);
    }

    #[test]
    fn the_original_preset_names_load_as_their_equivalents() {
        let o =
            Options::from_toml_str("[video]\ngraphics = \"fabulous\"\nanti_aliasing = \"fxaa\"\n")
                .unwrap();
        // A file's FXAA (never built) is off; with the other values left at their defaults the
        // preset is what the values say, not what the file claimed.
        assert_eq!(o.video.anti_aliasing, AntiAliasing::Off);
        assert_eq!(o.video.graphics, GraphicsPreset::Medium);
        let fast =
            "[video]\ngraphics = \"fast\"\nlod_detail = \"low\"\n[video.shader]\nwater = \"low\"\n";
        let o = Options::from_toml_str(fast).unwrap();
        assert_eq!(o.video.graphics, GraphicsPreset::Low);
    }

    #[test]
    fn save_and_load_file() {
        let dir = std::env::temp_dir().join(format!("hearth-opts-{}", std::process::id()));
        let path = dir.join("options.toml");
        let mut o = Options::default();
        o.video.fov = 95.0;
        o.save(&path).unwrap();
        let back = Options::load(&path).unwrap();
        assert_eq!(back.video.fov, 95.0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_file_gives_defaults() {
        let o = Options::load(Path::new("definitely/not/here/options.toml")).unwrap();
        assert_eq!(o, Options::default());
    }
}

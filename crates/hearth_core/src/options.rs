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
    pub chat: ChatOptions,
    pub accessibility: AccessibilityOptions,
    pub skin: SkinOptions,
    pub window: WindowState,
    /// Language code, e.g. `en_us`.
    pub language: String,
    /// Enabled resource packs, lowest priority first (the built-in pack is implicit and always
    /// at the bottom).
    pub resource_packs: Vec<String>,
    /// Show advanced tooltips (F3+H).
    pub advanced_tooltips: bool,
    /// Pause the game when the window loses focus.
    pub pause_on_lost_focus: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            version: OPTIONS_VERSION,
            video: VideoOptions::default(),
            controls: ControlOptions::default(),
            sound: SoundOptions::default(),
            chat: ChatOptions::default(),
            accessibility: AccessibilityOptions::default(),
            skin: SkinOptions::default(),
            window: WindowState::default(),
            language: "en_us".to_owned(),
            resource_packs: Vec::new(),
            advanced_tooltips: false,
            pause_on_lost_focus: true,
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
        self.chat.sanitize();
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
/// preset to `Custom`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphicsPreset {
    Fast,
    Fancy,
    Fabulous,
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
pub enum CloudMode {
    Off,
    Fast,
    Fancy,
    Volumetric,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticleMode {
    All,
    Decreased,
    Minimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkBuilderMode {
    Threaded,
    SemiBlocking,
    FullyBlocking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttackIndicator {
    Off,
    Crosshair,
    Hotbar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AntiAliasing {
    Off,
    Fxaa,
    Taa,
}

/// Generic four-step quality level used by the shader-quality group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Off,
    Low,
    Medium,
    High,
}

/// Shader quality group (water, shadows, volumetrics, sky, weather, exposure).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShaderOptions {
    pub water: Quality,
    pub shadows: Quality,
    pub volumetrics: Quality,
    pub sky: Quality,
    pub weather_effects: Quality,
    pub auto_exposure: bool,
    pub bloom: bool,
}

impl Default for ShaderOptions {
    fn default() -> Self {
        Self::for_preset(GraphicsPreset::Fancy)
    }
}

impl ShaderOptions {
    pub fn for_preset(preset: GraphicsPreset) -> Self {
        match preset {
            GraphicsPreset::Fast => Self {
                water: Quality::Low,
                shadows: Quality::Off,
                volumetrics: Quality::Off,
                sky: Quality::Low,
                weather_effects: Quality::Low,
                auto_exposure: true,
                bloom: false,
            },
            GraphicsPreset::Fancy | GraphicsPreset::Custom => Self {
                water: Quality::Medium,
                shadows: Quality::Medium,
                volumetrics: Quality::Low,
                sky: Quality::Medium,
                weather_effects: Quality::Medium,
                auto_exposure: true,
                bloom: true,
            },
            GraphicsPreset::Fabulous => Self {
                water: Quality::High,
                shadows: Quality::High,
                volumetrics: Quality::Medium,
                sky: Quality::High,
                weather_effects: Quality::High,
                auto_exposure: true,
                bloom: true,
            },
        }
    }
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
    /// Radius in chunks within which the world is simulated (5–32).
    pub simulation_distance: u32,
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
    /// 0.0 = Moody (strict realism), 1.0 = Bright (generous minimum exposure).
    pub brightness: f32,
    /// Vertical field of view in degrees (30–110).
    pub fov: f32,
    /// Strength of speed/sprint FOV changes (0–1).
    pub fov_effects: f32,
    pub view_bobbing: bool,
    /// Strength of nausea/underwater distortion effects (0–1).
    pub distortion_effects: f32,
    /// Strength of screen overlays such as damage tilt and darkness (0–1).
    pub screen_effects: f32,
    pub clouds: CloudMode,
    /// Cloud layer base height in blocks above sea level.
    pub cloud_height: i32,
    pub particles: ParticleMode,
    /// Mipmap levels (0–4).
    pub mipmap_levels: u32,
    /// Biome blend radius in blocks (0–15).
    pub biome_blend: u32,
    /// Entity render distance scale (0.5–5.0).
    pub entity_distance: f32,
    pub entity_shadows: bool,
    pub smooth_lighting: bool,
    pub chunk_builder: ChunkBuilderMode,
    pub attack_indicator: AttackIndicator,
    /// Anisotropic filtering samples (1, 2, 4, 8 or 16).
    pub anisotropic_filtering: u32,
    pub anti_aliasing: AntiAliasing,
    /// Internal render resolution scale (0.5–2.0): below 1 the frame is upscaled (FSR 1),
    /// above 1 filtered down. 1 in every preset.
    pub render_scale: f32,
    /// Bend distant terrain below the horizon according to planet curvature.
    pub planet_curvature: bool,
    /// Sway leaves, grass and crops in the wind.
    pub wind_sway: bool,
    /// Held light sources (torches, lanterns) light up the surroundings.
    pub dynamic_held_light: bool,
    pub shader: ShaderOptions,
}

impl Default for VideoOptions {
    fn default() -> Self {
        let mut v = Self {
            graphics: GraphicsPreset::Fancy,
            render_distance: 12,
            vertical_render_distance: 8,
            simulation_distance: 12,
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
            brightness: 0.5,
            fov: 70.0,
            fov_effects: 1.0,
            view_bobbing: true,
            distortion_effects: 1.0,
            screen_effects: 1.0,
            clouds: CloudMode::Fancy,
            cloud_height: 320,
            particles: ParticleMode::All,
            mipmap_levels: 4,
            biome_blend: 5,
            entity_distance: 1.0,
            entity_shadows: true,
            smooth_lighting: true,
            chunk_builder: ChunkBuilderMode::Threaded,
            attack_indicator: AttackIndicator::Crosshair,
            anisotropic_filtering: 4,
            anti_aliasing: AntiAliasing::Fxaa,
            render_scale: 1.0,
            planet_curvature: true,
            wind_sway: true,
            dynamic_held_light: true,
            shader: ShaderOptions::default(),
        };
        v.apply_preset(GraphicsPreset::Fancy);
        v
    }
}

/// The subset of video settings a graphics preset controls.
#[derive(Debug, Clone, PartialEq)]
struct PresetValues {
    lod_detail: Quality,
    clouds: CloudMode,
    particles: ParticleMode,
    entity_shadows: bool,
    smooth_lighting: bool,
    anti_aliasing: AntiAliasing,
    wind_sway: bool,
    shader: ShaderOptions,
}

impl PresetValues {
    fn for_preset(preset: GraphicsPreset) -> Self {
        match preset {
            GraphicsPreset::Fast => Self {
                lod_detail: Quality::Low,
                clouds: CloudMode::Fast,
                particles: ParticleMode::Decreased,
                entity_shadows: false,
                smooth_lighting: true,
                anti_aliasing: AntiAliasing::Off,
                wind_sway: false,
                shader: ShaderOptions::for_preset(preset),
            },
            GraphicsPreset::Fancy | GraphicsPreset::Custom => Self {
                lod_detail: Quality::Medium,
                clouds: CloudMode::Fancy,
                particles: ParticleMode::All,
                entity_shadows: true,
                smooth_lighting: true,
                anti_aliasing: AntiAliasing::Fxaa,
                wind_sway: true,
                shader: ShaderOptions::for_preset(GraphicsPreset::Fancy),
            },
            GraphicsPreset::Fabulous => Self {
                lod_detail: Quality::High,
                clouds: CloudMode::Volumetric,
                particles: ParticleMode::All,
                entity_shadows: true,
                smooth_lighting: true,
                anti_aliasing: AntiAliasing::Taa,
                wind_sway: true,
                shader: ShaderOptions::for_preset(preset),
            },
        }
    }

    fn of(v: &VideoOptions) -> Self {
        Self {
            lod_detail: v.lod_detail,
            clouds: v.clouds,
            particles: v.particles,
            entity_shadows: v.entity_shadows,
            smooth_lighting: v.smooth_lighting,
            anti_aliasing: v.anti_aliasing,
            wind_sway: v.wind_sway,
            shader: v.shader.clone(),
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
        self.clouds = p.clouds;
        self.particles = p.particles;
        self.entity_shadows = p.entity_shadows;
        self.smooth_lighting = p.smooth_lighting;
        self.anti_aliasing = p.anti_aliasing;
        self.wind_sway = p.wind_sway;
        self.shader = p.shader;
    }

    /// Returns the preset whose sub-settings exactly match the current values, or `Custom`.
    pub fn detect_preset(&self) -> GraphicsPreset {
        let current = PresetValues::of(self);
        [
            GraphicsPreset::Fast,
            GraphicsPreset::Fancy,
            GraphicsPreset::Fabulous,
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
        self.simulation_distance = self.simulation_distance.clamp(5, 32);
        self.lod_distance = self.lod_distance.min(4096);
        self.lod_vram_budget_mb = self.lod_vram_budget_mb.clamp(64, 16384);
        if self.max_framerate != 0 {
            self.max_framerate = self.max_framerate.clamp(10, 260);
        }
        self.brightness = finite_clamp(self.brightness, 0.0, 1.0, 0.5);
        self.fov = finite_clamp(self.fov, 30.0, 110.0, 70.0);
        self.fov_effects = finite_clamp(self.fov_effects, 0.0, 1.0, 1.0);
        self.distortion_effects = finite_clamp(self.distortion_effects, 0.0, 1.0, 1.0);
        self.screen_effects = finite_clamp(self.screen_effects, 0.0, 1.0, 1.0);
        self.cloud_height = self.cloud_height.clamp(64, 2048);
        self.mipmap_levels = self.mipmap_levels.min(4);
        self.biome_blend = self.biome_blend.min(15);
        self.entity_distance = finite_clamp(self.entity_distance, 0.5, 5.0, 1.0);
        self.anisotropic_filtering = match self.anisotropic_filtering {
            0 | 1 => 1,
            2 => 2,
            3 | 4 => 4,
            5..=8 => 8,
            _ => 16,
        };
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
    pub raw_input: bool,
    /// Scroll sensitivity multiplier (0.01–10).
    pub scroll_sensitivity: f32,
    /// Each scroll event moves exactly one step regardless of its magnitude.
    pub discrete_scrolling: bool,
    pub toggle_sneak: bool,
    pub toggle_sprint: bool,
    pub auto_jump: bool,
    /// Controller look sensitivity 0–1.
    pub controller_sensitivity: f32,
    /// Action id → binding string, e.g. `"key.forward" = "w"`, `"key.drop_stack" = "ctrl+x"`.
    /// Missing entries use the action's default binding.
    pub key_bindings: BTreeMap<String, String>,
}

impl Default for ControlOptions {
    fn default() -> Self {
        Self {
            mouse_sensitivity: 0.5,
            invert_y: false,
            raw_input: true,
            scroll_sensitivity: 1.0,
            discrete_scrolling: false,
            toggle_sneak: false,
            toggle_sprint: false,
            auto_jump: false,
            controller_sensitivity: 0.5,
            key_bindings: BTreeMap::new(),
        }
    }
}

impl ControlOptions {
    fn sanitize(&mut self) {
        self.mouse_sensitivity = finite_clamp(self.mouse_sensitivity, 0.0, 1.0, 0.5);
        self.scroll_sensitivity = finite_clamp(self.scroll_sensitivity, 0.01, 10.0, 1.0);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatVisibility {
    Shown,
    CommandsOnly,
    Hidden,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChatOptions {
    pub visibility: ChatVisibility,
    pub colors: bool,
    pub web_links: bool,
    /// Text opacity 0.1–1.
    pub opacity: f32,
    /// Chat text scale 0–1.
    pub scale: f32,
    /// Width in GUI pixels (40–320).
    pub width: u32,
    /// Height when focused in GUI pixels (20–180).
    pub height_focused: u32,
    /// Height when unfocused in GUI pixels (20–180).
    pub height_unfocused: u32,
    /// Extra spacing between lines 0–1.
    pub line_spacing: f32,
    /// Seconds before a message is shown (0–6).
    pub delay: f32,
}

impl Default for ChatOptions {
    fn default() -> Self {
        Self {
            visibility: ChatVisibility::Shown,
            colors: true,
            web_links: false,
            opacity: 1.0,
            scale: 1.0,
            width: 320,
            height_focused: 180,
            height_unfocused: 90,
            line_spacing: 0.0,
            delay: 0.0,
        }
    }
}

impl ChatOptions {
    fn sanitize(&mut self) {
        self.opacity = finite_clamp(self.opacity, 0.1, 1.0, 1.0);
        self.scale = finite_clamp(self.scale, 0.0, 1.0, 1.0);
        self.width = self.width.clamp(40, 320);
        self.height_focused = self.height_focused.clamp(20, 180);
        self.height_unfocused = self.height_unfocused.clamp(20, 180);
        self.line_spacing = finite_clamp(self.line_spacing, 0.0, 1.0, 0.0);
        self.delay = finite_clamp(self.delay, 0.0, 6.0, 0.0);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccessibilityOptions {
    /// Opacity of the background behind chat/subtitle text (0–1).
    pub text_background_opacity: f32,
    pub high_contrast: bool,
    /// Reduce camera shake, flashes and pulsing effects.
    pub reduce_motion: bool,
    /// How strongly darkness effects pulse (0–1).
    pub darkness_pulsing: f32,
    /// Hide lightning sky flashes.
    pub hide_lightning_flashes: bool,
    /// Scale of the damage tilt (0–1).
    pub damage_tilt: f32,
    /// Compact bars for food, water, warmth, rest, stamina and blood (v2 §9.9's "Guided" HUD)
    /// besides the body's sensations.
    pub guided_hud: bool,
}

impl Default for AccessibilityOptions {
    fn default() -> Self {
        Self {
            text_background_opacity: 0.5,
            high_contrast: false,
            reduce_motion: false,
            darkness_pulsing: 1.0,
            hide_lightning_flashes: false,
            damage_tilt: 1.0,
            guided_hud: false,
        }
    }
}

impl AccessibilityOptions {
    fn sanitize(&mut self) {
        self.text_background_opacity = finite_clamp(self.text_background_opacity, 0.0, 1.0, 0.5);
        self.darkness_pulsing = finite_clamp(self.darkness_pulsing, 0.0, 1.0, 1.0);
        self.damage_tilt = finite_clamp(self.damage_tilt, 0.0, 1.0, 1.0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MainHand {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkinOptions {
    /// Built-in skin id.
    pub skin: String,
    pub main_hand: MainHand,
}

impl Default for SkinOptions {
    fn default() -> Self {
        Self {
            skin: "hearth:wanderer".to_owned(),
            main_hand: MainHand::Right,
        }
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
            anisotropic_filtering = 7
            [sound]
            master = 4.0
        "#;
        let o = Options::from_toml_str(text).unwrap();
        assert_eq!(o.video.render_distance, 32);
        assert_eq!(o.video.fov, 110.0);
        assert_eq!(o.video.max_framerate, 10);
        assert_eq!(o.video.anisotropic_filtering, 8);
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
        assert_eq!(v.graphics, GraphicsPreset::Fancy);
        v.apply_preset(GraphicsPreset::Fabulous);
        assert_eq!(v.detect_preset(), GraphicsPreset::Fabulous);
        v.shader.shadows = Quality::Low;
        v.refresh_preset();
        assert_eq!(v.graphics, GraphicsPreset::Custom);
        v.apply_preset(GraphicsPreset::Fast);
        assert_eq!(v.graphics, GraphicsPreset::Fast);
        assert_eq!(v.detect_preset(), GraphicsPreset::Fast);
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

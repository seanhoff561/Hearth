//! GPU device creation and presentation-surface management.

use std::sync::Arc;

use winit::window::Window;

/// Errors raised while bringing up the GPU.
#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    #[error("failed to create a presentation surface: {0}")]
    Surface(#[from] wgpu::CreateSurfaceError),
    #[error("no compatible GPU adapter found: {0}")]
    Adapter(#[from] wgpu::RequestAdapterError),
    #[error("failed to create GPU device: {0}")]
    Device(#[from] wgpu::RequestDeviceError),
    #[error("the surface is not supported by the selected adapter")]
    UnsupportedSurface,
}

/// How frames are presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentPreference {
    /// Choose from the VSync flag.
    Auto {
        vsync: bool,
    },
    Immediate,
    Mailbox,
    Fifo,
}

/// Optional features the renderer uses when available. Everything has a fallback path.
#[derive(Debug, Clone, Copy, Default)]
pub struct GpuCapabilities {
    pub multi_draw_indirect_count: bool,
    pub indirect_first_instance: bool,
    pub timestamp_queries: bool,
    pub texture_compression_bc: bool,
    pub float32_filterable: bool,
    pub shader_f16: bool,
    pub max_anisotropy: u16,
}

/// Device, queue and adapter information shared by all renderer subsystems.
pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: wgpu::AdapterInfo,
    pub caps: GpuCapabilities,
}

impl GpuContext {
    fn optional_features(adapter: &wgpu::Adapter) -> (wgpu::Features, GpuCapabilities) {
        let available = adapter.features();
        let wanted = wgpu::Features::MULTI_DRAW_INDIRECT_COUNT
            | wgpu::Features::INDIRECT_FIRST_INSTANCE
            | wgpu::Features::TIMESTAMP_QUERY
            | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS
            | wgpu::Features::TEXTURE_COMPRESSION_BC
            | wgpu::Features::FLOAT32_FILTERABLE
            | wgpu::Features::SHADER_F16;
        let features = available & wanted;
        let caps = GpuCapabilities {
            multi_draw_indirect_count: features.contains(wgpu::Features::MULTI_DRAW_INDIRECT_COUNT),
            indirect_first_instance: features.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE),
            timestamp_queries: features.contains(wgpu::Features::TIMESTAMP_QUERY),
            texture_compression_bc: features.contains(wgpu::Features::TEXTURE_COMPRESSION_BC),
            float32_filterable: features.contains(wgpu::Features::FLOAT32_FILTERABLE),
            shader_f16: features.contains(wgpu::Features::SHADER_F16),
            max_anisotropy: 16,
        };
        (features, caps)
    }

    async fn create(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'static>>,
        force_fallback: bool,
    ) -> Result<Self, GpuError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: force_fallback,
                compatible_surface: surface,
                ..Default::default()
            })
            .await?;
        let info = adapter.get_info();
        log::info!(
            "GPU adapter: {} ({:?}, {:?}, driver {} {})",
            info.name,
            info.device_type,
            info.backend,
            info.driver,
            info.driver_info
        );
        let (features, caps) = Self::optional_features(&adapter);
        // Ask for the adapter's full limits so large storage buffers and texture arrays work.
        let limits = adapter.limits();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("hearth device"),
                required_features: features,
                required_limits: limits,
                memory_hints: wgpu::MemoryHints::Performance,
                ..Default::default()
            })
            .await?;
        device.on_uncaptured_error(Arc::new(|err| {
            log::error!("wgpu error: {err}");
        }));
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            info,
            caps,
        })
    }

    /// Creates a context able to present to `window`, returning the configured surface too.
    pub fn for_window(
        window: Arc<Window>,
        present: PresentPreference,
    ) -> Result<(Self, SurfaceState), GpuError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance.create_surface(window.clone())?;
        let ctx = pollster::block_on(Self::create(instance, Some(&surface), false))?;
        let size = window.inner_size();
        let state = SurfaceState::new(&ctx, surface, size.width, size.height, present)?;
        Ok((ctx, state))
    }

    /// Creates a context without any window, for offscreen rendering and tools. When
    /// `software` is true the fallback (CPU) adapter is requested.
    pub fn headless(software: bool) -> Result<Self, GpuError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        pollster::block_on(Self::create(instance, None, software))
    }
}

/// The window's swapchain configuration.
pub struct SurfaceState {
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    supported_present_modes: Vec<wgpu::PresentMode>,
}

impl SurfaceState {
    fn new(
        ctx: &GpuContext,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
        present: PresentPreference,
    ) -> Result<Self, GpuError> {
        let caps = surface.get_capabilities(&ctx.adapter);
        if caps.formats.is_empty() {
            return Err(GpuError::UnsupportedSurface);
        }
        let mut config = surface
            .get_default_config(&ctx.adapter, width.max(1), height.max(1))
            .ok_or(GpuError::UnsupportedSurface)?;
        // Prefer an sRGB swapchain: the renderer writes linear values and lets the hardware
        // encode.
        if let Some(fmt) = caps.formats.iter().copied().find(|f| f.is_srgb()) {
            config.format = fmt;
        }
        config.desired_maximum_frame_latency = 2;
        let mut s = Self {
            surface,
            config,
            supported_present_modes: caps.present_modes,
        };
        s.config.present_mode = s.pick_present_mode(present);
        s.surface.configure(&ctx.device, &s.config);
        log::info!(
            "surface configured: {}x{} {:?} {:?}",
            s.config.width,
            s.config.height,
            s.config.format,
            s.config.present_mode
        );
        Ok(s)
    }

    fn pick_present_mode(&self, pref: PresentPreference) -> wgpu::PresentMode {
        use wgpu::PresentMode as P;
        let supported = |m: P| self.supported_present_modes.contains(&m);
        let wanted: &[P] = match pref {
            PresentPreference::Auto { vsync: true } | PresentPreference::Fifo => &[P::Fifo],
            PresentPreference::Auto { vsync: false } => &[P::Mailbox, P::Immediate, P::Fifo],
            PresentPreference::Mailbox => &[P::Mailbox, P::Immediate, P::Fifo],
            PresentPreference::Immediate => &[P::Immediate, P::Mailbox, P::Fifo],
        };
        wanted
            .iter()
            .copied()
            .find(|m| supported(*m))
            .unwrap_or(P::Fifo)
    }

    /// Present modes the surface supports (for the advanced video menu).
    pub fn supported_present_modes(&self) -> &[wgpu::PresentMode] {
        &self.supported_present_modes
    }

    pub fn set_present(&mut self, ctx: &GpuContext, pref: PresentPreference) {
        let mode = self.pick_present_mode(pref);
        if mode != self.config.present_mode {
            self.config.present_mode = mode;
            self.surface.configure(&ctx.device, &self.config);
            log::info!("present mode: {mode:?}");
        }
    }

    pub fn resize(&mut self, ctx: &GpuContext, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if width != self.config.width || height != self.config.height {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&ctx.device, &self.config);
        }
    }

    /// Acquires the next swapchain image, reconfiguring on loss. Returns `None` when no frame
    /// can be drawn right now (minimised, occluded, timeout).
    pub fn acquire(&mut self, ctx: &GpuContext) -> Option<wgpu::SurfaceTexture> {
        use wgpu::CurrentSurfaceTexture as C;
        for _ in 0..2 {
            match self.surface.get_current_texture() {
                C::Success(t) => return Some(t),
                C::Suboptimal(t) => {
                    // Draw this frame, reconfigure before the next.
                    self.surface.configure(&ctx.device, &self.config);
                    return Some(t);
                }
                C::Outdated | C::Lost => {
                    self.surface.configure(&ctx.device, &self.config);
                }
                C::Timeout | C::Occluded => return None,
                C::Validation => {
                    log::error!("surface acquire validation error");
                    return None;
                }
            }
        }
        None
    }
}

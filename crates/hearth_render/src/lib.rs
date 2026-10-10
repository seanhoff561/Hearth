//! The Hearth renderer, built on wgpu.
//!
//! The renderer is organised as a set of passes sharing one [`GpuContext`]; see
//! `ARCHITECTURE.md` for the frame graph.

mod arena;
pub mod atlas;
pub mod body;
pub mod camera;
mod cull;
pub mod figure;
pub mod globe;
pub mod gpu;
pub mod joints;
pub mod lod;
pub mod mesh;
pub mod models;
pub mod offscreen;
pub mod outline;
pub mod post;
pub mod precip;
pub mod profiler;
pub mod scene;
pub mod shadow;
pub mod skin_lut;
pub mod sky;
pub mod smoke;
pub mod smooth;
pub mod taa;
pub mod terrain;
pub mod trees;
pub mod ui;
pub mod water;

pub use gpu::{GpuCapabilities, GpuContext, GpuError, PresentPreference, SurfaceState};

/// The window renderer. Owns the device, the swapchain and the main depth buffer.
pub struct Renderer {
    pub ctx: GpuContext,
    pub surface: SurfaceState,
    pub depth: offscreen::DepthTarget,
    /// How long the last frame's parts took on the calling thread.
    pub parts: FrameParts,
}

/// The calling thread's time in each part of a frame ([`Renderer::render_with`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameParts {
    /// Waiting for the surface's next image.
    pub acquire: std::time::Duration,
    /// Recording the frame's commands (the caller's drawing).
    pub draw: std::time::Duration,
    pub submit: std::time::Duration,
    pub present: std::time::Duration,
}

/// What a frame callback draws into.
#[derive(Clone, Copy)]
pub struct FrameTargets<'a> {
    pub color: &'a wgpu::TextureView,
    pub depth: &'a wgpu::TextureView,
    pub size: (u32, u32),
    pub format: wgpu::TextureFormat,
}

impl Renderer {
    pub fn new(
        window: std::sync::Arc<winit::window::Window>,
        present: PresentPreference,
    ) -> Result<Self, GpuError> {
        let (ctx, surface) = GpuContext::for_window(window, present)?;
        let depth = offscreen::DepthTarget::new(&ctx, surface.config.width, surface.config.height);
        Ok(Self {
            ctx,
            surface,
            depth,
            parts: FrameParts::default(),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface.resize(&self.ctx, width, height);
        let (w, h) = (self.surface.config.width, self.surface.config.height);
        if (w, h) != (self.depth.width, self.depth.height) {
            self.depth = offscreen::DepthTarget::new(&self.ctx, w, h);
        }
    }

    /// Colour format of the swapchain (pipelines drawing to the window must use it).
    pub fn color_format(&self) -> wgpu::TextureFormat {
        self.surface.config.format
    }

    /// Acquires the next swapchain image, lets `draw` record into `enc`, then submits and
    /// presents. Returns false if no frame was drawn (minimised or surface lost).
    pub fn render_with(
        &mut self,
        draw: impl FnOnce(&GpuContext, &mut wgpu::CommandEncoder, FrameTargets<'_>),
    ) -> bool {
        let t = std::time::Instant::now();
        let acquired = self.surface.acquire(&self.ctx);
        self.parts = FrameParts {
            acquire: t.elapsed(),
            ..FrameParts::default()
        };
        let Some(frame) = acquired else {
            return false;
        };
        let t = std::time::Instant::now();
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        draw(
            &self.ctx,
            &mut encoder,
            FrameTargets {
                color: &view,
                depth: &self.depth.view,
                size: (self.surface.config.width, self.surface.config.height),
                format: self.surface.config.format,
            },
        );
        let finished = encoder.finish();
        self.parts.draw = t.elapsed();
        let t = std::time::Instant::now();
        self.ctx.queue.submit(Some(finished));
        self.parts.submit = t.elapsed();
        let t = std::time::Instant::now();
        self.ctx.queue.present(frame);
        self.parts.present = t.elapsed();
        true
    }

    pub fn set_present(&mut self, pref: PresentPreference) {
        self.surface.set_present(&self.ctx, pref);
    }
}

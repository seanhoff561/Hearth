//! The Hearth renderer, built on wgpu.
//!
//! The renderer is organised as a set of passes sharing one [`GpuContext`]; see
//! `ARCHITECTURE.md` for the frame graph.

pub mod atlas;
pub mod camera;
mod cull;
pub mod globe;
pub mod gpu;
pub mod lod;
pub mod mesh;
pub mod models;
pub mod offscreen;
pub mod post;
pub mod precip;
pub mod profiler;
pub mod scene;
pub mod sky;
pub mod terrain;
pub mod ui;
pub mod water;

pub use gpu::{GpuCapabilities, GpuContext, GpuError, PresentPreference, SurfaceState};

/// Linear RGBA color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearColor {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

/// The window renderer. Owns the device, the swapchain and the main depth buffer.
pub struct Renderer {
    pub ctx: GpuContext,
    pub surface: SurfaceState,
    pub depth: offscreen::DepthTarget,
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
        let Some(frame) = self.surface.acquire(&self.ctx) else {
            return false;
        };
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
        self.ctx.queue.submit(Some(encoder.finish()));
        self.ctx.queue.present(frame);
        true
    }

    pub fn set_present(&mut self, pref: PresentPreference) {
        self.surface.set_present(&self.ctx, pref);
    }

    /// Clears the swapchain to `color` and presents. Returns false if no frame was drawn.
    pub fn render_clear(&mut self, color: LinearColor) -> bool {
        let Some(frame) = self.surface.acquire(&self.ctx) else {
            return false;
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: color.r,
                            g: color.g,
                            b: color.b,
                            a: color.a,
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
        self.ctx.queue.submit(Some(encoder.finish()));
        self.ctx.queue.present(frame);
        true
    }
}

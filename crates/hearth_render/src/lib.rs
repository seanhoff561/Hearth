//! The Hearth renderer, built on wgpu.
//!
//! The renderer is organised as a set of passes sharing one [`GpuContext`]; see
//! `ARCHITECTURE.md` for the frame graph.

pub mod gpu;

pub use gpu::{GpuCapabilities, GpuContext, GpuError, PresentPreference, SurfaceState};

/// Linear RGBA color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearColor {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

/// The window renderer. Owns the device and swapchain and draws each frame.
pub struct Renderer {
    pub ctx: GpuContext,
    pub surface: SurfaceState,
}

impl Renderer {
    pub fn new(
        window: std::sync::Arc<winit::window::Window>,
        present: PresentPreference,
    ) -> Result<Self, GpuError> {
        let (ctx, surface) = GpuContext::for_window(window, present)?;
        Ok(Self { ctx, surface })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface.resize(&self.ctx, width, height);
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

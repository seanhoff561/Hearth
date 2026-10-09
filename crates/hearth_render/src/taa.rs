//! Temporal anti-aliasing (`shaders/taa.wgsl`): the camera is jittered by a fraction of a pixel
//! each frame (a Halton (2, 3) sequence of eight), and each frame is blended into the history
//! of the last ones, reprojected through the depth buffer and clamped to the colours about each
//! pixel. The result replaces the frame's HDR image, so metering, tonemapping and scaling see
//! it as before.

use bytemuck::{Pod, Zeroable};
use glam::{DVec3, Mat4, Vec2};

use crate::gpu::GpuContext;
use crate::post::HDR_FORMAT;

/// Jitter offsets (pixels, −0.5..0.5) of the eight frames of the sequence.
fn halton(i: u32, base: u32) -> f32 {
    let mut f = 1.0;
    let mut r = 0.0;
    let mut i = i;
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }
    r
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Params {
    inv_view_proj: [[f32; 4]; 4],
    prev_view_proj: [[f32; 4]; 4],
    delta: [f32; 4],
    jitter: [f32; 4],
}

struct History {
    textures: [wgpu::Texture; 2],
    views: [wgpu::TextureView; 2],
    size: (u32, u32),
}

pub struct TaaRenderer {
    pipe: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    sampler: wgpu::Sampler,
    history: Option<History>,
    /// Which history texture this frame writes.
    cur: usize,
    frame: u32,
    /// The last frame's unjittered view-projection and camera, and whether there is one.
    prev: Option<(Mat4, DVec3)>,
}

impl TaaRenderer {
    pub fn new(ctx: &GpuContext) -> Self {
        let device = &ctx.device;
        let tex = |binding: u32, depth: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: if depth {
                    wgpu::TextureSampleType::Depth
                } else {
                    wgpu::TextureSampleType::Float { filterable: true }
                },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("taa layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                tex(1, false),
                tex(2, true),
                tex(3, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taa params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("taa sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("taa.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/taa.wgsl").into()),
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("taa pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("taa"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipe,
            layout,
            params,
            sampler,
            history: None,
            cur: 0,
            frame: 0,
            prev: None,
        }
    }

    /// This frame's jitter in pixels (−0.5..0.5 each way).
    pub fn jitter_px(&self) -> Vec2 {
        let i = self.frame % 8 + 1;
        Vec2::new(halton(i, 2) - 0.5, halton(i, 3) - 0.5)
    }

    /// Forgets the history (a cut: the next frame starts it afresh).
    pub fn reset(&mut self) {
        self.prev = None;
    }

    fn ensure(&mut self, ctx: &GpuContext, size: (u32, u32)) {
        if self.history.as_ref().is_some_and(|h| h.size == size) {
            return;
        }
        let make = |label: &str| {
            ctx.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size.0.max(1),
                    height: size.1.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HDR_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };
        let textures = [make("taa history a"), make("taa history b")];
        let views = [
            textures[0].create_view(&Default::default()),
            textures[1].create_view(&Default::default()),
        ];
        self.history = Some(History {
            textures,
            views,
            size,
        });
        self.prev = None;
    }

    /// Resolves this frame (`hdr`, its `depth`, rendered with `view_proj` jittered by
    /// `jitter_px` from the camera at `cam`, `unjittered` without) against the history and
    /// writes the result back into `hdr`.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        hdr: &wgpu::Texture,
        hdr_view: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        size: (u32, u32),
        view_proj: Mat4,
        unjittered: Mat4,
        cam: DVec3,
    ) {
        self.ensure(ctx, size);
        let Some(h) = &self.history else {
            return;
        };
        let (prev_vp, prev_cam, fresh) = match self.prev {
            Some((vp, c)) => (vp, c, false),
            None => (unjittered, cam, true),
        };
        // The camera's move since the last frame. Crossing the planet's seam its x jumps by the
        // circumference, a multiple of 4096 blocks; no frame moves it 2048 blocks, so the move is
        // its x folded into ±2048 (E4.1 §4.5).
        let mut delta = cam - prev_cam;
        delta.x = (delta.x + 2048.0).rem_euclid(4096.0) - 2048.0;
        let jitter = self.jitter_px();
        ctx.write_buffer(
            &self.params,
            0,
            bytemuck::bytes_of(&Params {
                inv_view_proj: view_proj.inverse().to_cols_array_2d(),
                prev_view_proj: prev_vp.to_cols_array_2d(),
                delta: [
                    delta.x as f32,
                    delta.y as f32,
                    delta.z as f32,
                    if fresh { 1.0 } else { 0.0 },
                ],
                jitter: [jitter.x, jitter.y, size.0 as f32, size.1 as f32],
            }),
        );
        let read = 1 - self.cur;
        let bind = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("taa bind"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(hdr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&h.views[read]),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("taa"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &h.views[self.cur],
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipe);
            pass.set_bind_group(0, &bind, &[]);
            pass.draw(0..3, 0..1);
        }
        enc.copy_texture_to_texture(
            h.textures[self.cur].as_image_copy(),
            hdr.as_image_copy(),
            wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
        );
        self.prev = Some((unjittered, cam));
        self.cur = read;
        self.frame = self.frame.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_jitter_sequence_covers_the_pixel() {
        let pts: Vec<(f32, f32)> = (1..=8).map(|i| (halton(i, 2), halton(i, 3))).collect();
        assert!(
            pts.iter()
                .all(|(x, y)| (0.0..1.0).contains(x) && (0.0..1.0).contains(y))
        );
        // Every quarter of the pixel is visited.
        for (qx, qy) in [(0.0, 0.0), (0.5, 0.0), (0.0, 0.5), (0.5, 0.5)] {
            assert!(
                pts.iter()
                    .any(|(x, y)| (qx..qx + 0.5).contains(x) && (qy..qy + 0.5).contains(y)),
                "quarter ({qx}, {qy}) of {pts:?}"
            );
        }
        let mean = pts.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
        assert!((mean.0 / 8.0 - 0.5).abs() < 0.1 && (mean.1 / 8.0 - 0.5).abs() < 0.1);
    }
}

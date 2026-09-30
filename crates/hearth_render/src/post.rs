//! HDR render target, highlight metering (`shaders/meter.wgsl`) and the final tonemap to the
//! display format (`shaders/post.wgsl`).

use bytemuck::{Pod, Zeroable};

use crate::gpu::GpuContext;

/// Format of the scene's HDR colour target.
pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The bright end of the frame (90th percentile, pre-exposed) is kept at or below this.
const METER_BRIGHT_TARGET: f32 = 1.2;
/// The metering darkens the frame by at most this factor.
const METER_MIN_SCALE: f32 = 1.0 / 16.0;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Params {
    p: [f32; 4],
}

struct HdrTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    tonemap_bind: wgpu::BindGroup,
    meter_bind: wgpu::BindGroup,
    size: (u32, u32),
}

pub struct PostProcess {
    params: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    pipe: wgpu::RenderPipeline,
    meter_params: wgpu::Buffer,
    meter_state: wgpu::Buffer,
    meter_layout: wgpu::BindGroupLayout,
    meter_pipe: wgpu::ComputePipeline,
    hdr: Option<HdrTarget>,
}

impl PostProcess {
    pub fn new(ctx: &GpuContext, output_format: wgpu::TextureFormat) -> Self {
        let device = &ctx.device;
        let uniform = |label: &str| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: std::mem::size_of::<Params>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let params = uniform("post params");
        let meter_params = uniform("meter params");
        let meter_state = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("meter state"),
            size: 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let entry = |binding: u32, visibility: wgpu::ShaderStages, ty: wgpu::BindingType| {
            wgpu::BindGroupLayoutEntry {
                binding,
                visibility,
                ty,
                count: None,
            }
        };
        let uniform_ty = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let hdr_ty = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let storage_ty = |read_only: bool| wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let fs = wgpu::ShaderStages::FRAGMENT;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post layout"),
            entries: &[
                entry(0, fs, uniform_ty),
                entry(1, fs, hdr_ty),
                entry(2, fs, storage_ty(true)),
            ],
        });
        let cs = wgpu::ShaderStages::COMPUTE;
        let meter_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("meter layout"),
            entries: &[
                entry(0, cs, uniform_ty),
                entry(1, cs, hdr_ty),
                entry(2, cs, storage_ty(false)),
            ],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("post.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/post.wgsl").into()),
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("post pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tonemap"),
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
                    format: output_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let meter_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("meter.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/meter.wgsl").into()),
        });
        let meter_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("meter pipeline layout"),
            bind_group_layouts: &[Some(&meter_layout)],
            immediate_size: 0,
        });
        let meter_pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("meter"),
            layout: Some(&meter_pl),
            module: &meter_module,
            entry_point: Some("meter_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        Self {
            params,
            layout,
            pipe,
            meter_params,
            meter_state,
            meter_layout,
            meter_pipe,
            hdr: None,
        }
    }

    /// The HDR target for a frame of `size`, (re)created when the size changes.
    pub fn hdr_view(&mut self, ctx: &GpuContext, size: (u32, u32)) -> &wgpu::TextureView {
        if self.hdr.as_ref().is_none_or(|h| h.size != size) {
            let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("hdr colour"),
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
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let bind = |label: &str, layout: &wgpu::BindGroupLayout, params: &wgpu::Buffer| {
                ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(label),
                    layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: params.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: self.meter_state.as_entire_binding(),
                        },
                    ],
                })
            };
            let tonemap_bind = bind("post bind", &self.layout, &self.params);
            let meter_bind = bind("meter bind", &self.meter_layout, &self.meter_params);
            self.hdr = Some(HdrTarget {
                _texture: texture,
                view,
                tonemap_bind,
                meter_bind,
                size,
            });
        }
        &self.hdr.as_ref().expect("created above").view
    }

    /// Measures the rendered HDR frame and adapts the highlight exposure over `dt` seconds
    /// (not finite = instantly). Record after the scene, before `render`.
    pub fn meter(&self, ctx: &GpuContext, enc: &mut wgpu::CommandEncoder, dt: f32) {
        let Some(hdr) = &self.hdr else {
            return;
        };
        let instant = !dt.is_finite();
        ctx.queue.write_buffer(
            &self.meter_params,
            0,
            bytemuck::bytes_of(&Params {
                p: [
                    if instant { 0.0 } else { dt },
                    if instant { 1.0 } else { 0.0 },
                    METER_BRIGHT_TARGET,
                    METER_MIN_SCALE,
                ],
            }),
        );
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("meter"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.meter_pipe);
        pass.set_bind_group(0, &hdr.meter_bind, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }

    /// Tonemaps the HDR target into `output`.
    pub fn render(
        &self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        exposure: f32,
        night: f32,
    ) {
        let Some(hdr) = &self.hdr else {
            return;
        };
        ctx.queue.write_buffer(
            &self.params,
            0,
            bytemuck::bytes_of(&Params {
                p: [exposure, night, 0.0, 0.0],
            }),
        );
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("tonemap"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                depth_slice: None,
                resolve_target: None,
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
        pass.set_bind_group(0, &hdr.tonemap_bind, &[]);
        pass.draw(0..3, 0..1);
    }
}

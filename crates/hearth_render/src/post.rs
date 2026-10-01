//! HDR render target, highlight metering (`shaders/meter.wgsl`) and the final tonemap to the
//! display format (`shaders/post.wgsl`). A frame rendered at another size than the output's
//! (the render scale) is tonemapped at its own size, then upscaled with FSR 1's EASU and RCAS
//! passes or filtered down.

use bytemuck::{Pod, Zeroable};

use crate::gpu::GpuContext;
use crate::profiler::GpuTimer;

/// Format of the scene's HDR colour target.
pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The bright end of the frame (90th percentile, pre-exposed) is kept at or below this.
const METER_BRIGHT_TARGET: f32 = 1.2;
/// The metering darkens the frame by at most this factor.
const METER_MIN_SCALE: f32 = 1.0 / 16.0;
/// Format of the tonemapped (perceptual) frame the upscaler reads and writes.
const LDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgb10a2Unorm;
/// RCAS sharpening, in stops below the strongest (FSR 1's usual 0.2).
const RCAS_STOPS: f32 = 0.2;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Params {
    p: [f32; 4],
}

/// The tonemap's parameters (`Params` in `post.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct ToneParams {
    /// x: extra exposure, y: night factor.
    p: [f32; 4],
    /// Under water: x the water surface above the camera (blocks), 0 when not under water.
    water: [f32; 4],
    /// rgb: light the water scatters toward the eye (pre-exposed).
    inscatter: [f32; 4],
    /// Clip space to camera-relative world space.
    inv_view_proj: [[f32; 4]; 4],
}

/// The camera under water: the surface above it (blocks) and the light the water scatters
/// toward the eye (pre-exposed).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Underwater {
    pub surface_above: f32,
    pub inscatter: glam::Vec3,
}

/// The upscaling passes' parameters (`Scale` in `post.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct ScaleParams {
    /// Source texels per output pixel, and the source's size.
    ratio: [f32; 2],
    size: [f32; 2],
    sharp: [f32; 4],
}

/// The targets of a frame rendered at another size than the output's.
struct Scaled {
    render: (u32, u32),
    output: (u32, u32),
    /// The tonemapped frame at the render size, read by EASU (or the resampling).
    _ldr: wgpu::Texture,
    ldr: wgpu::TextureView,
    from_ldr: wgpu::BindGroup,
    /// When upscaling: EASU's output at the output size, read by RCAS.
    up: Option<(wgpu::Texture, wgpu::TextureView, wgpu::BindGroup)>,
}

struct HdrTarget {
    texture: wgpu::Texture,
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
    /// The depth buffer the tonemap reads for the water seen from below (group 1), and the
    /// view it was made for.
    depth_layout: wgpu::BindGroupLayout,
    depth_bind: Option<(wgpu::TextureView, wgpu::BindGroup)>,
    /// Rendering at another size: tonemapping to the perceptual copy, EASU, RCAS, and the
    /// down-filter, with their layout, sampler and targets.
    tonemap_pipe: wgpu::RenderPipeline,
    easu_pipe: wgpu::RenderPipeline,
    rcas_pipe: wgpu::RenderPipeline,
    resample_pipe: wgpu::RenderPipeline,
    scale_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    scaled: Option<Scaled>,
}

impl PostProcess {
    pub fn new(ctx: &GpuContext, output_format: wgpu::TextureFormat) -> Self {
        let device = &ctx.device;
        let uniform = |label: &str, size: usize| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let params = uniform("post params", std::mem::size_of::<ToneParams>());
        let meter_params = uniform("meter params", std::mem::size_of::<Params>());
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
        let depth_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post depth layout"),
            entries: &[entry(
                0,
                fs,
                wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
            )],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("post pipeline layout"),
            bind_group_layouts: &[Some(&layout), Some(&depth_layout)],
            immediate_size: 0,
        });
        let scale_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("upscale layout"),
            entries: &[
                entry(3, fs, uniform_ty),
                entry(
                    4,
                    fs,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(
                    5,
                    fs,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                ),
            ],
        });
        let scale_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("upscale pipeline layout"),
            bind_group_layouts: &[Some(&scale_layout)],
            immediate_size: 0,
        });
        let pipeline = |label: &str,
                        layout: &wgpu::PipelineLayout,
                        entry_point: &str,
                        format: wgpu::TextureFormat| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
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
                    entry_point: Some(entry_point),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipe = pipeline("tonemap", &pl, "fs_main", output_format);
        let tonemap_pipe = pipeline("tonemap to scale", &pl, "fs_tonemap", LDR_FORMAT);
        let easu_pipe = pipeline("easu", &scale_pl, "fs_easu", LDR_FORMAT);
        let rcas_pipe = pipeline("rcas", &scale_pl, "fs_rcas", output_format);
        let resample_pipe = pipeline("resample", &scale_pl, "fs_resample", output_format);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("upscale sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
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
            depth_layout,
            depth_bind: None,
            tonemap_pipe,
            easu_pipe,
            rcas_pipe,
            resample_pipe,
            scale_layout,
            sampler,
            scaled: None,
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
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
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
                texture,
                view,
                tonemap_bind,
                meter_bind,
                size,
            });
        }
        &self.hdr.as_ref().expect("created above").view
    }

    /// The HDR target's texture (to copy what is drawn so far).
    pub fn hdr_texture(&self) -> Option<&wgpu::Texture> {
        self.hdr.as_ref().map(|h| &h.texture)
    }

    /// Measures the rendered HDR frame and adapts the highlight exposure over `dt` seconds
    /// (not finite = instantly). Record after the scene, before `render`.
    pub fn meter(&self, ctx: &GpuContext, enc: &mut wgpu::CommandEncoder, dt: f32) {
        let Some(hdr) = &self.hdr else {
            return;
        };
        let instant = !dt.is_finite();
        ctx.write_buffer(
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

    /// Tonemaps the HDR target into `output` (`size` pixels): directly at the same size; else
    /// through a tonemapped copy at the HDR target's size, upscaled by EASU and sharpened by
    /// RCAS, or filtered down when larger. Under water (`underwater`), what is seen is dimmed
    /// along the view through the water and replaced by the light it scatters; `depth` is the
    /// scene's depth and `inv_view_proj` takes clip space back to camera-relative space.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        size: (u32, u32),
        exposure: f32,
        night: f32,
        depth: &wgpu::TextureView,
        underwater: Option<Underwater>,
        inv_view_proj: glam::Mat4,
        mut timer: Option<&mut GpuTimer>,
    ) {
        let mut mark = |enc: &mut wgpu::CommandEncoder, label: &'static str| {
            if let Some(t) = timer.as_deref_mut() {
                t.mark(enc, label);
            }
        };
        let Some(render) = self.hdr.as_ref().map(|h| h.size) else {
            return;
        };
        let (above, inscatter) = underwater.map_or((0.0, glam::Vec3::ZERO), |u| {
            (u.surface_above.max(1e-3), u.inscatter)
        });
        ctx.write_buffer(
            &self.params,
            0,
            bytemuck::bytes_of(&ToneParams {
                p: [exposure, night, 0.0, 0.0],
                water: [
                    if underwater.is_some() { above } else { 0.0 },
                    0.0,
                    0.0,
                    0.0,
                ],
                inscatter: inscatter.extend(0.0).to_array(),
                inv_view_proj: inv_view_proj.to_cols_array_2d(),
            }),
        );
        if self.depth_bind.as_ref().is_none_or(|(v, _)| v != depth) {
            let bind = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("post depth bind"),
                layout: &self.depth_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(depth),
                }],
            });
            self.depth_bind = Some((depth.clone(), bind));
        }
        if render != size
            && self
                .scaled
                .as_ref()
                .is_none_or(|s| (s.render, s.output) != (render, size))
        {
            self.scaled = Some(self.scaled_targets(ctx, render, size));
        }
        let hdr = self.hdr.as_ref().expect("checked above");
        let depth_bind = &self.depth_bind.as_ref().expect("made above").1;
        if render == size {
            fullscreen(
                enc,
                "tonemap",
                output,
                &self.pipe,
                &hdr.tonemap_bind,
                Some(depth_bind),
            );
            mark(enc, "tonemap");
            return;
        }
        let s = self.scaled.as_ref().expect("made above");
        fullscreen(
            enc,
            "tonemap",
            &s.ldr,
            &self.tonemap_pipe,
            &hdr.tonemap_bind,
            Some(depth_bind),
        );
        mark(enc, "tonemap");
        match &s.up {
            Some((_, up, from_up)) => {
                fullscreen(enc, "easu", up, &self.easu_pipe, &s.from_ldr, None);
                mark(enc, "upscale (easu)");
                fullscreen(enc, "rcas", output, &self.rcas_pipe, from_up, None);
                mark(enc, "sharpen (rcas)");
            }
            None => {
                fullscreen(
                    enc,
                    "resample",
                    output,
                    &self.resample_pipe,
                    &s.from_ldr,
                    None,
                );
                mark(enc, "filter down");
            }
        }
    }

    /// The targets and bind groups for a frame rendered at `render` and shown at `output`.
    fn scaled_targets(&self, ctx: &GpuContext, render: (u32, u32), output: (u32, u32)) -> Scaled {
        let device = &ctx.device;
        let texture = |label: &str, (w, h): (u32, u32)| {
            let t = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: w.max(1),
                    height: h.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: LDR_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let v = t.create_view(&Default::default());
            (t, v)
        };
        // Each pass reads its source with its own ratio and size.
        let bind = |label: &str, view: &wgpu::TextureView, from: (u32, u32), to: (u32, u32)| {
            let params = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: std::mem::size_of::<ScaleParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            ctx.write_buffer(
                &params,
                0,
                bytemuck::bytes_of(&ScaleParams {
                    ratio: [
                        from.0 as f32 / to.0.max(1) as f32,
                        from.1 as f32 / to.1.max(1) as f32,
                    ],
                    size: [from.0 as f32, from.1 as f32],
                    sharp: [(-RCAS_STOPS).exp2(), 0.0, 0.0, 0.0],
                }),
            );
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &self.scale_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: params.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            })
        };
        let (ldr_texture, ldr) = texture("tonemapped at render size", render);
        let from_ldr = bind("upscale from render size", &ldr, render, output);
        let up = (render.0 < output.0 || render.1 < output.1).then(|| {
            let (t, v) = texture("upscaled", output);
            let b = bind("sharpen", &v, output, output);
            (t, v, b)
        });
        Scaled {
            render,
            output,
            _ldr: ldr_texture,
            ldr,
            from_ldr,
            up,
        }
    }
}

/// Records a full-screen pass of `pipe` into `target`.
fn fullscreen(
    enc: &mut wgpu::CommandEncoder,
    label: &str,
    target: &wgpu::TextureView,
    pipe: &wgpu::RenderPipeline,
    bind: &wgpu::BindGroup,
    bind1: Option<&wgpu::BindGroup>,
) {
    let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
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
    pass.set_pipeline(pipe);
    pass.set_bind_group(0, bind, &[]);
    if let Some(b) = bind1 {
        pass.set_bind_group(1, b, &[]);
    }
    pass.draw(0..3, 0..1);
}

//! Bodies: boxes from `hearth_character` drawn as instances of the unit cube — in the world,
//! lit like the terrain (into the scene's HDR target), and on their own for the character
//! screen's preview (straight to the display, under a choice of lights).

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use hearth_character::FigureInstance;

use crate::gpu::GpuContext;
use crate::terrain::DEPTH_FORMAT;

const INSTANCE_BYTES: u64 = std::mem::size_of::<FigureInstance>() as u64;

/// A storage buffer of instances that grows as needed.
struct Instances {
    layout: wgpu::BindGroupLayout,
    buffer: wgpu::Buffer,
    bind: wgpu::BindGroup,
    capacity: u64,
    count: u32,
}

impl Instances {
    fn new(ctx: &GpuContext, label: &str) -> Self {
        let layout = ctx
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let (buffer, bind) = Self::alloc(ctx, &layout, 256);
        Self {
            layout,
            buffer,
            bind,
            capacity: 256,
            count: 0,
        }
    }

    fn alloc(
        ctx: &GpuContext,
        layout: &wgpu::BindGroupLayout,
        capacity: u64,
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("figure instances"),
            size: capacity * INSTANCE_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("figure instances"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        (buffer, bind)
    }

    fn set(&mut self, ctx: &GpuContext, instances: &[FigureInstance]) {
        let n = instances.len() as u64;
        if n > self.capacity {
            let capacity = n.next_power_of_two();
            let (buffer, bind) = Self::alloc(ctx, &self.layout, capacity);
            self.buffer = buffer;
            self.bind = bind;
            self.capacity = capacity;
        }
        if n > 0 {
            ctx.queue
                .write_buffer(&self.buffer, 0, bytemuck::cast_slice(instances));
        }
        self.count = n as u32;
    }
}

fn pipeline(
    ctx: &GpuContext,
    label: &str,
    module: &wgpu::ShaderModule,
    layouts: &[Option<&wgpu::BindGroupLayout>],
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = ctx
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(label),
            bind_group_layouts: layouts,
            immediate_size: 0,
        });
    ctx.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some("fs_main"),
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
}

/// The texture the animals' coats are read from (one texel, white, until there are coats).
struct Skins {
    layout: wgpu::BindGroupLayout,
    bind: wgpu::BindGroup,
}

impl Skins {
    fn new(ctx: &GpuContext) -> Self {
        let layout = ctx
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("figure coats"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            });
        let bind = Self::upload(ctx, &layout, 1, 1, &[[255, 255, 255, 255]]);
        Self { layout, bind }
    }

    fn upload(
        ctx: &GpuContext,
        layout: &wgpu::BindGroupLayout,
        width: u32,
        height: u32,
        texels: &[[u8; 4]],
    ) -> wgpu::BindGroup {
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("figure coats"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        ctx.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(texels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("figure coats"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            }],
        })
    }
}

/// Bodies in the world.
pub struct FigureRenderer {
    pipeline: wgpu::RenderPipeline,
    instances: Instances,
    skins: Skins,
}

impl FigureRenderer {
    /// `globals` is the terrain's bind group 0 layout (its lighting and camera).
    pub fn new(ctx: &GpuContext, globals: &wgpu::BindGroupLayout) -> Self {
        let module = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("figure.wgsl"),
                source: wgpu::ShaderSource::Wgsl(
                    concat!(
                        include_str!("shaders/common.wgsl"),
                        include_str!("shaders/figure.wgsl")
                    )
                    .into(),
                ),
            });
        let instances = Instances::new(ctx, "figure instances");
        let skins = Skins::new(ctx);
        let pipeline = pipeline(
            ctx,
            "figures",
            &module,
            &[Some(globals), Some(&instances.layout), Some(&skins.layout)],
            crate::post::HDR_FORMAT,
        );
        Self {
            pipeline,
            instances,
            skins,
        }
    }

    /// This frame's boxes (camera-relative).
    pub fn set(&mut self, ctx: &GpuContext, instances: &[FigureInstance]) {
        self.instances.set(ctx, instances);
    }

    /// The texture of the animals' coats (sRGB texels, row by row).
    pub fn set_coats(&mut self, ctx: &GpuContext, width: u32, height: u32, texels: &[[u8; 4]]) {
        assert_eq!(texels.len(), (width * height) as usize, "coat texture size");
        self.skins.bind =
            Skins::upload(ctx, &self.skins.layout, width.max(1), height.max(1), texels);
    }

    pub fn count(&self) -> u32 {
        self.instances.count
    }

    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, globals: &'a wgpu::BindGroup) {
        if self.instances.count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, globals, &[]);
        pass.set_bind_group(1, &self.instances.bind, &[]);
        pass.set_bind_group(2, &self.skins.bind, &[]);
        pass.draw(0..36, 0..self.instances.count);
    }
}

/// The light a preview is seen in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewLight {
    #[default]
    Daylight,
    Overcast,
    Dusk,
    Firelight,
    /// A full moon's light.
    Night,
}

impl PreviewLight {
    pub const ALL: [PreviewLight; 5] = [
        PreviewLight::Daylight,
        PreviewLight::Overcast,
        PreviewLight::Dusk,
        PreviewLight::Firelight,
        PreviewLight::Night,
    ];

    pub fn key(self) -> &'static str {
        match self {
            PreviewLight::Daylight => "character.light.daylight",
            PreviewLight::Overcast => "character.light.overcast",
            PreviewLight::Dusk => "character.light.dusk",
            PreviewLight::Firelight => "character.light.firelight",
            PreviewLight::Night => "character.light.night",
        }
    }

    /// Direction to the light (in front of the figure, which faces +Z), its illuminance and
    /// the sky's and the ground's (lux, RGB).
    pub(crate) fn lighting(self) -> (Vec3, Vec3, Vec3, Vec3) {
        match self {
            PreviewLight::Daylight => (
                Vec3::new(0.45, 0.75, 0.5).normalize(),
                Vec3::new(95_000.0, 90_000.0, 82_000.0),
                Vec3::new(14_000.0, 17_000.0, 23_000.0),
                Vec3::new(9_000.0, 8_500.0, 7_500.0),
            ),
            PreviewLight::Overcast => (
                Vec3::Y,
                Vec3::ZERO,
                Vec3::new(16_000.0, 16_500.0, 17_500.0),
                Vec3::new(3_000.0, 3_000.0, 3_000.0),
            ),
            PreviewLight::Dusk => (
                Vec3::new(-0.6, 0.12, 0.79).normalize(),
                Vec3::new(1_800.0, 900.0, 420.0),
                Vec3::new(260.0, 300.0, 420.0),
                Vec3::new(120.0, 90.0, 70.0),
            ),
            PreviewLight::Firelight => (
                Vec3::new(0.5, 0.1, 0.86).normalize(),
                Vec3::new(60.0, 37.0, 18.0),
                Vec3::new(0.3, 0.3, 0.45),
                Vec3::new(6.0, 3.5, 1.6),
            ),
            // A full moon high (some 0.25 lux), seen bluish by the dark-adapted eye.
            PreviewLight::Night => (
                Vec3::new(-0.3, 0.8, 0.52).normalize(),
                Vec3::new(0.21, 0.24, 0.32),
                Vec3::new(0.03, 0.035, 0.05),
                Vec3::new(0.02, 0.02, 0.022),
            ),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct PreviewUniform {
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 4],
    light: [f32; 4],
    sky: [f32; 4],
    ground: [f32; 4],
    exposure: [f32; 4],
}

/// People alone, for a screen that shows them (the birth screen).
pub struct FigurePreview {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind0: wgpu::BindGroup,
    instances: Instances,
    depth: Option<crate::offscreen::DepthTarget>,
    srgb: bool,
}

impl FigurePreview {
    /// `format` is the display's (an sRGB format takes linear light).
    pub fn new(ctx: &GpuContext, format: wgpu::TextureFormat) -> Self {
        let module = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("figure_preview.wgsl"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("shaders/figure_preview.wgsl").into(),
                ),
            });
        let layout0 = ctx
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("figure preview"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let uniform = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("figure preview"),
            size: std::mem::size_of::<PreviewUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind0 = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("figure preview"),
            layout: &layout0,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let instances = Instances::new(ctx, "figure preview instances");
        let pipeline = pipeline(
            ctx,
            "figure preview",
            &module,
            &[Some(&layout0), Some(&instances.layout)],
            format,
        );
        Self {
            pipeline,
            uniform,
            bind0,
            instances,
            depth: None,
            srgb: format.is_srgb(),
        }
    }

    /// Draws `instances` (placed in the figure's frame: feet at the origin, facing +Z) into
    /// `rect` (x, y, width, height in pixels) of `target` (`size` pixels), seen from the front
    /// at the height of its middle, framing `height_m` up and `half_width_m` to each side, and
    /// lit by `light`. The instances go up with the queue:
    /// one preview per submission.
    pub fn render(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        rect: [u32; 4],
        instances: &[FigureInstance],
        height_m: f32,
        half_width_m: f32,
        light: PreviewLight,
    ) {
        let [x, y, w, h] = rect;
        if w == 0 || h == 0 || x + w > size.0 || y + h > size.1 {
            return;
        }
        if self
            .depth
            .as_ref()
            .is_none_or(|d| (d.width, d.height) != size)
        {
            self.depth = Some(crate::offscreen::DepthTarget::new(ctx, size.0, size.1));
        }
        self.instances.set(ctx, instances);
        let fov = 28f32.to_radians();
        let aspect = w as f32 / h as f32;
        // Far enough back that the bodies fit the height with a margin, and their width (with
        // arms out) the width.
        let half = (fov / 2.0).tan();
        let fit = (height_m * 0.58 / half).max(half_width_m.max(0.55) / (half * aspect));
        let center = Vec3::new(0.0, height_m * 0.5, 0.0);
        let eye = center + Vec3::new(0.0, height_m * 0.05, fit);
        let view = glam::camera::rh::view::look_at_mat4(eye, center, Vec3::Y);
        let proj = glam::camera::rh::proj::directx::perspective_infinite_reverse(fov, aspect, 0.05);
        let (dir, sun, sky, ground) = light.lighting();
        // Exposed for the light on a face turned toward it, as the eye would adapt (though
        // not all the way to moonlight: the night stays dim).
        let level = (sun.y + sky.y).max(0.6);
        // A display without an sRGB format gets its gamma from the shader.
        let encode = if self.srgb { 0.0 } else { 1.0 };
        let u = PreviewUniform {
            view_proj: (proj * view).to_cols_array_2d(),
            light_dir: dir.extend(0.0).to_array(),
            light: sun.extend(0.0).to_array(),
            sky: sky.extend(0.0).to_array(),
            ground: ground.extend(0.0).to_array(),
            exposure: [1.2 * std::f32::consts::PI / level, encode, 0.0, 0.0],
        };
        ctx.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        let depth = &self.depth.as_ref().expect("depth").view;
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("figure preview"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(x as f32, y as f32, w as f32, h as f32, 0.0, 1.0);
        pass.set_scissor_rect(x, y, w, h);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind0, &[]);
        pass.set_bind_group(1, &self.instances.bind, &[]);
        pass.draw(0..36, 0..self.instances.count);
    }
}

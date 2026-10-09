//! The thing looked at, highlighted by its own shape (Amendment T §2.3): never a box about it.
//!
//! What the eyes rest on is drawn again into a mask the size of the scene (`outline.wgsl`) —
//! a thing's or an animal's boxes as they are drawn, a block's own quads alpha-tested as the
//! terrain draws them (only the texels of its fruit and flowers when those are what is looked
//! at, swaying and losing its leaves as the terrain's do), or a soft disc over the ground a dig
//! there would take or over the water about the point looked at — then a soft glow is laid over
//! the finished frame along the mask's edges (`outline_glow.wgsl`). The mask is drawn with the
//! unjittered camera and against the scene's depth, so only what of the thing is seen glows and
//! it does not shimmer.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use hearth_character::FigureInstance;
use hearth_world::RenderLayer;

use crate::gpu::GpuContext;
use crate::models::ModelQuad;
use crate::terrain::DEPTH_FORMAT;

/// The mask: red where the thing is seen, green over the patch of ground or water.
const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Unorm;

/// How far toward the eye the thing is drawn for its mask (a scale on its distance, its pixels
/// unchanged): enough that its own surface (drawn with the jitter) does not hide it, too little
/// to show it through anything more than a centimetre or so before it.
const PULL: f32 = 0.994;

/// The glow's colour (linear, a warm white) and strength.
const GLOW: [f32; 4] = [1.0, 0.86, 0.62, 1.0];

/// What is highlighted, as its own shape (camera-relative).
#[derive(Debug, Clone)]
pub enum Highlight {
    /// A thing lying, an animal or a carcass: its boxes as drawn.
    Boxes(Vec<FigureInstance>),
    /// A block: its quads (block-local) at `origin`, its corner; only its fruit and flowers when
    /// `fruit`; `climate` its column's climate code (its foliage's season).
    Block {
        quads: Vec<MaskQuad>,
        origin: Vec3,
        fruit: bool,
        climate: u32,
    },
    /// The ground a dig there takes: its bowl's middle and radius (the patch is where the bowl
    /// meets what is seen).
    Ground { centre: Vec3, radius: f32 },
    /// Water about a point of its surface.
    Water { at: Vec3, radius: f32 },
}

impl Highlight {
    /// A box about all of it.
    fn bounds(&self) -> Option<(Vec3, Vec3)> {
        match self {
            Highlight::Boxes(parts) => parts
                .iter()
                .map(|p| {
                    let c = Vec3::new(p.rows[0][3], p.rows[1][3], p.rows[2][3]);
                    let half = Vec3::new(
                        p.rows[0][0].abs() + p.rows[0][1].abs() + p.rows[0][2].abs(),
                        p.rows[1][0].abs() + p.rows[1][1].abs() + p.rows[1][2].abs(),
                        p.rows[2][0].abs() + p.rows[2][1].abs() + p.rows[2][2].abs(),
                    ) * 0.5;
                    (c - half, c + half)
                })
                .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1))),
            // A plant sways a few centimetres beyond its block.
            Highlight::Block { origin, .. } => {
                Some((*origin - Vec3::splat(0.1), *origin + Vec3::splat(1.1)))
            }
            Highlight::Ground { centre, radius } => Some((
                *centre - Vec3::splat(*radius + 0.1),
                *centre + Vec3::splat(*radius + 0.1),
            )),
            Highlight::Water { at, radius } => Some((
                *at - Vec3::new(*radius, 0.1, *radius),
                *at + Vec3::new(*radius, 0.1, *radius),
            )),
        }
    }
}

/// A quad of a block for its mask: its corners (block-local) with their texel u, each corner's
/// texel v; its texture and how it sways; its tint kind and face (`outline.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct MaskQuad {
    pub corners: [[f32; 4]; 4],
    pub v: [f32; 4],
    /// layer:12 | frames−1:4 | frame time−1:4 | sway:2 (1 the upper corners, 2 all) | cutout:1
    pub layer: u32,
    /// Tint kind (4 bits, `mesh::TINT_*` with its variant) | the face's direction (3, 7 none).
    pub flags: u32,
    pub pad: [u32; 2],
}

impl MaskQuad {
    /// A quad of a block's own shape (`BlockModels::each_quad`) drawn on `layer`; `tint_kind` its
    /// tint's kind in its column; `cube` when it is a full cube's face (a cube of leaves sways
    /// whole, a plant only at its top).
    pub fn new(q: &ModelQuad, layer: RenderLayer, tint_kind: u32, cube: bool) -> Self {
        let t = &q.tex.tex;
        let sway = match (q.waving, cube) {
            (false, _) => 0u32,
            (true, false) => 1,
            (true, true) => 2,
        };
        let cutout = layer == RenderLayer::Cutout;
        let dir = q.dir.map_or(7, |d| d.index() as u32);
        Self {
            corners: std::array::from_fn(|c| [q.pos[c].x, q.pos[c].y, q.pos[c].z, q.uv[c].x]),
            v: std::array::from_fn(|c| q.uv[c].y),
            layer: (t.layer as u32 & 0xfff)
                | ((t.frames.max(1) as u32 - 1) & 15) << 12
                | ((t.frame_time.max(1) as u32 - 1) & 15) << 16
                | sway << 20
                | (cutout as u32) << 22,
            flags: (tint_kind & 15) | dir << 4,
            pad: [0; 2],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct MaskUniform {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    origin: [f32; 4],
    disc: [f32; 4],
    params: [f32; 4],
    climate: [u32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct GlowUniform {
    color: [f32; 4],
    size: [f32; 4],
}

/// A storage buffer that grows as needed.
struct Storage {
    buffer: wgpu::Buffer,
    capacity: u64,
}

impl Storage {
    fn alloc(ctx: &GpuContext, capacity: u64) -> wgpu::Buffer {
        ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("outline storage"),
            size: capacity,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn new(ctx: &GpuContext, capacity: u64) -> Self {
        Self {
            buffer: Self::alloc(ctx, capacity),
            capacity,
        }
    }

    /// Writes `data`; true when the buffer was made anew (its bind group must be too).
    fn set(&mut self, ctx: &GpuContext, data: &[u8]) -> bool {
        let n = data.len() as u64;
        let grown = n > self.capacity;
        if grown {
            self.capacity = n.next_power_of_two();
            self.buffer = Self::alloc(ctx, self.capacity);
        }
        if n > 0 {
            ctx.write_buffer(&self.buffer, 0, data);
        }
        grown
    }
}

/// The mask at the scene's size, and the glow's view of it.
struct Target {
    size: (u32, u32),
    view: wgpu::TextureView,
    glow_bind: wgpu::BindGroup,
}

/// What is drawn this frame.
#[derive(Debug, Clone, Copy)]
enum Draw {
    Boxes(u32),
    Quads(u32),
    Disc,
}

#[derive(Debug, Clone, Copy)]
struct Frame {
    draw: Draw,
    /// The parts of the mask and of the output the highlight can reach (x, y, w, h).
    mask_rect: [u32; 4],
    glow_rect: [u32; 4],
}

pub struct OutlineRenderer {
    uniform: wgpu::Buffer,
    glow_uniform: wgpu::Buffer,
    mask_layout: wgpu::BindGroupLayout,
    disc_layout: wgpu::BindGroupLayout,
    glow_layout: wgpu::BindGroupLayout,
    boxes_pipe: wgpu::RenderPipeline,
    quads_pipe: wgpu::RenderPipeline,
    disc_pipe: wgpu::RenderPipeline,
    glow_pipe: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    parts: Storage,
    quads: Storage,
    mask_bind: wgpu::BindGroup,
    target: Option<Target>,
    disc_bind: Option<(wgpu::TextureView, wgpu::BindGroup)>,
    frame: Option<Frame>,
}

impl OutlineRenderer {
    /// `globals` is the terrain's bind group 0 layout (its camera, textures and wind); `output`
    /// the format of the frame the glow is laid on.
    pub fn new(
        ctx: &GpuContext,
        globals: &wgpu::BindGroupLayout,
        output: wgpu::TextureFormat,
    ) -> Self {
        let device = &ctx.device;
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
        let storage_ty = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let vf = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let mask_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("outline mask"),
            entries: &[
                entry(0, vf, uniform_ty),
                entry(1, wgpu::ShaderStages::VERTEX, storage_ty),
                entry(2, wgpu::ShaderStages::VERTEX, storage_ty),
            ],
        });
        let disc_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("outline disc"),
            entries: &[entry(
                0,
                wgpu::ShaderStages::FRAGMENT,
                wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
            )],
        });
        let glow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("outline glow"),
            entries: &[
                entry(0, wgpu::ShaderStages::FRAGMENT, uniform_ty),
                entry(
                    1,
                    wgpu::ShaderStages::FRAGMENT,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(
                    2,
                    wgpu::ShaderStages::FRAGMENT,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                ),
            ],
        });
        let mask_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("outline.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                concat!(
                    include_str!("shaders/common.wgsl"),
                    include_str!("shaders/outline.wgsl")
                )
                .into(),
            ),
        });
        let glow_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("outline_glow.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/outline_glow.wgsl").into()),
        });
        let layout = |label: &str, groups: &[Option<&wgpu::BindGroupLayout>]| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: groups,
                immediate_size: 0,
            })
        };
        let mask_pl = layout("outline mask", &[Some(globals), Some(&mask_layout)]);
        let disc_pl = layout(
            "outline disc",
            &[Some(globals), Some(&mask_layout), Some(&disc_layout)],
        );
        let glow_pl = layout("outline glow", &[Some(&glow_layout)]);
        let pipeline = |label: &str,
                        pl: &wgpu::PipelineLayout,
                        module: &wgpu::ShaderModule,
                        (vs, fs): (&str, &str),
                        depth: bool,
                        target: wgpu::ColorTargetState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(pl),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                // Both sides: a plant's quads are seen from either, and a box's far side is
                // hidden by the scene's depth.
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: depth.then(|| wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(target)],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let channel = |write_mask: wgpu::ColorWrites| wgpu::ColorTargetState {
            format: MASK_FORMAT,
            blend: None,
            write_mask,
        };
        let boxes_pipe = pipeline(
            "outline boxes",
            &mask_pl,
            &mask_module,
            ("vs_box", "fs_seen"),
            true,
            channel(wgpu::ColorWrites::RED),
        );
        let quads_pipe = pipeline(
            "outline quads",
            &mask_pl,
            &mask_module,
            ("vs_quad", "fs_quad"),
            true,
            channel(wgpu::ColorWrites::RED),
        );
        let disc_pipe = pipeline(
            "outline disc",
            &disc_pl,
            &mask_module,
            ("vs_full", "fs_disc"),
            false,
            channel(wgpu::ColorWrites::GREEN),
        );
        let glow_pipe = pipeline(
            "outline glow",
            &glow_pl,
            &glow_module,
            ("vs_full", "fs_glow"),
            false,
            wgpu::ColorTargetState {
                format: output,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            },
        );
        let uniform = |label: &str, size: usize| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let mask_uniform = uniform("outline mask", std::mem::size_of::<MaskUniform>());
        let glow_uniform = uniform("outline glow", std::mem::size_of::<GlowUniform>());
        let parts = Storage::new(ctx, 64 * std::mem::size_of::<FigureInstance>() as u64);
        let quads = Storage::new(ctx, 32 * std::mem::size_of::<MaskQuad>() as u64);
        let mask_bind = Self::mask_bind(ctx, &mask_layout, &mask_uniform, &parts, &quads);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("outline mask"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            uniform: mask_uniform,
            glow_uniform,
            mask_layout,
            disc_layout,
            glow_layout,
            boxes_pipe,
            quads_pipe,
            disc_pipe,
            glow_pipe,
            sampler,
            parts,
            quads,
            mask_bind,
            target: None,
            disc_bind: None,
            frame: None,
        }
    }

    fn mask_bind(
        ctx: &GpuContext,
        layout: &wgpu::BindGroupLayout,
        uniform: &wgpu::Buffer,
        parts: &Storage,
        quads: &Storage,
    ) -> wgpu::BindGroup {
        ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("outline mask"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: parts.buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: quads.buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Whether anything is highlighted this frame.
    pub fn active(&self) -> bool {
        self.frame.is_some()
    }

    /// Takes this frame's highlight (none: nothing is drawn): `view_proj` the camera's without
    /// the jitter, `inv_view_proj` the scene's back (its depth read back), `render` the scene's
    /// size and `output` the frame's.
    pub fn prepare(
        &mut self,
        ctx: &GpuContext,
        highlight: Option<&Highlight>,
        view_proj: Mat4,
        inv_view_proj: Mat4,
        render: (u32, u32),
        output: (u32, u32),
    ) {
        self.frame = None;
        let Some(h) = highlight else {
            return;
        };
        let mut u = MaskUniform {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            origin: [0.0; 4],
            disc: [0.0; 4],
            params: [0.0, PULL, render.0 as f32, render.1 as f32],
            climate: [0; 4],
        };
        let mut grown = false;
        let draw = match h {
            Highlight::Boxes(parts) => {
                if parts.is_empty() {
                    return;
                }
                grown |= self.parts.set(ctx, bytemuck::cast_slice(parts));
                Draw::Boxes(parts.len() as u32)
            }
            Highlight::Block {
                quads,
                origin,
                fruit,
                climate,
            } => {
                if quads.is_empty() {
                    return;
                }
                grown |= self.quads.set(ctx, bytemuck::cast_slice(quads));
                u.origin = [origin.x, origin.y, origin.z, if *fruit { 1.0 } else { 0.0 }];
                u.climate[0] = *climate;
                Draw::Quads(quads.len() as u32)
            }
            Highlight::Ground { centre, radius } => {
                u.disc = [centre.x, centre.y, centre.z, *radius];
                Draw::Disc
            }
            Highlight::Water { at, radius } => {
                u.disc = [at.x, at.y, at.z, *radius];
                u.params[0] = 1.0;
                Draw::Disc
            }
        };
        if grown {
            self.mask_bind = Self::mask_bind(
                ctx,
                &self.mask_layout,
                &self.uniform,
                &self.parts,
                &self.quads,
            );
        }
        ctx.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        // The glow's lines widen with the frame (1.5 and 4 pixels at 1080 lines).
        let scale = (output.1 as f32 / 1080.0).max(1.0);
        ctx.write_buffer(
            &self.glow_uniform,
            0,
            bytemuck::bytes_of(&GlowUniform {
                color: GLOW,
                size: [output.0 as f32, output.1 as f32, scale, 0.0],
            }),
        );
        let bounds = h.bounds();
        let rect = |size: (u32, u32), margin: f32| match bounds {
            Some((lo, hi)) => screen_rect(view_proj, lo, hi, size, margin),
            None => [0, 0, size.0.max(1), size.1.max(1)],
        };
        self.frame = Some(Frame {
            draw,
            mask_rect: rect(render, 2.0),
            glow_rect: rect(output, 6.0 * scale + 2.0),
        });
    }

    /// Draws the mask (against the scene's `depth`, with the terrain's `globals`) and lays the
    /// glow over `output`; nothing when nothing is highlighted.
    pub fn render(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        globals: &wgpu::BindGroup,
        depth: &wgpu::TextureView,
        output: &wgpu::TextureView,
        render: (u32, u32),
    ) {
        let Some(frame) = self.frame else {
            return;
        };
        if self.target.as_ref().is_none_or(|t| t.size != render) {
            self.target = Some(self.make_target(ctx, render));
        }
        if self.disc_bind.as_ref().is_none_or(|(v, _)| v != depth) {
            let bind = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("outline disc"),
                layout: &self.disc_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(depth),
                }],
            });
            self.disc_bind = Some((depth.clone(), bind));
        }
        let target = self.target.as_ref().expect("made above");
        let clear = wgpu::RenderPassColorAttachment {
            view: &target.view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        };
        {
            // The ground's and the water's disc read the depth, so draw with none attached.
            let disc = matches!(frame.draw, Draw::Disc);
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("outline mask"),
                color_attachments: &[Some(clear)],
                depth_stencil_attachment: (!disc).then_some(
                    wgpu::RenderPassDepthStencilAttachment {
                        view: depth,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    },
                ),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, globals, &[]);
            pass.set_bind_group(1, &self.mask_bind, &[]);
            match frame.draw {
                Draw::Boxes(n) => {
                    pass.set_pipeline(&self.boxes_pipe);
                    pass.draw(0..36, 0..n);
                }
                Draw::Quads(n) => {
                    pass.set_pipeline(&self.quads_pipe);
                    pass.draw(0..n * 6, 0..1);
                }
                Draw::Disc => {
                    let [x, y, w, h] = frame.mask_rect;
                    pass.set_scissor_rect(x, y, w, h);
                    pass.set_pipeline(&self.disc_pipe);
                    pass.set_bind_group(2, &self.disc_bind.as_ref().expect("made above").1, &[]);
                    pass.draw(0..3, 0..1);
                }
            }
        }
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("outline glow"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let [x, y, w, h] = frame.glow_rect;
        pass.set_scissor_rect(x, y, w, h);
        pass.set_pipeline(&self.glow_pipe);
        pass.set_bind_group(0, &target.glow_bind, &[]);
        pass.draw(0..3, 0..1);
    }

    fn make_target(&self, ctx: &GpuContext, size: (u32, u32)) -> Target {
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("outline mask"),
            size: wgpu::Extent3d {
                width: size.0.max(1),
                height: size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: MASK_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let glow_bind = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("outline glow"),
            layout: &self.glow_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.glow_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        Target {
            size,
            view,
            glow_bind,
        }
    }
}

/// The pixels (x, y, width, height) of a frame of `size` that the box `lo`–`hi`
/// (camera-relative) covers, `margin` pixels more each way; the whole frame when part of it is
/// behind the eye.
fn screen_rect(view_proj: Mat4, lo: Vec3, hi: Vec3, size: (u32, u32), margin: f32) -> [u32; 4] {
    let (w, h) = (size.0.max(1) as f32, size.1.max(1) as f32);
    let mut min = glam::Vec2::splat(f32::MAX);
    let mut max = glam::Vec2::splat(f32::MIN);
    for k in 0..8 {
        let p = Vec3::new(
            if k & 1 == 0 { lo.x } else { hi.x },
            if k & 2 == 0 { lo.y } else { hi.y },
            if k & 4 == 0 { lo.z } else { hi.z },
        );
        let c: Vec4 = view_proj * p.extend(1.0);
        if c.w < 0.05 {
            return [0, 0, w as u32, h as u32];
        }
        let px = glam::Vec2::new((c.x / c.w * 0.5 + 0.5) * w, (0.5 - c.y / c.w * 0.5) * h);
        min = min.min(px);
        max = max.max(px);
    }
    let x0 = (min.x - margin).floor().clamp(0.0, w);
    let y0 = (min.y - margin).floor().clamp(0.0, h);
    let x1 = (max.x + margin).ceil().clamp(0.0, w);
    let y1 = (max.y + margin).ceil().clamp(0.0, h);
    if x1 <= x0 || y1 <= y0 {
        return [0, 0, 1, 1];
    }
    [x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_before_the_eye_covers_its_part_of_the_frame() {
        let proj =
            glam::camera::rh::proj::directx::perspective_infinite_reverse(1.2, 16.0 / 9.0, 0.05);
        let view = glam::camera::rh::view::look_at_mat4(Vec3::ZERO, Vec3::NEG_Z, Vec3::Y);
        let vp = proj * view;
        let r = screen_rect(
            vp,
            Vec3::new(-0.1, -0.1, -2.1),
            Vec3::new(0.1, 0.1, -1.9),
            (1600, 900),
            0.0,
        );
        // Small, about the middle.
        assert!(r[2] < 200 && r[3] < 200, "{r:?}");
        let mid = (r[0] + r[2] / 2, r[1] + r[3] / 2);
        assert!(mid.0.abs_diff(800) < 4 && mid.1.abs_diff(450) < 4, "{r:?}");
        // Partly behind the eye: all of it.
        let all = screen_rect(vp, Vec3::new(-1.0, -1.0, -1.0), Vec3::ONE, (1600, 900), 0.0);
        assert_eq!(all, [0, 0, 1600, 900]);
    }
}

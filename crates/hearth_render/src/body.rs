//! People's bodies as smooth skinned meshes (Amendment E §8, E7): the anatomy of
//! `hearth_character::anatomy` uploaded once per person, each vertex carried by up to four
//! joints of the 17-joint palette of its pose, and shaded as skin, lips and nails (light that
//! scatters under the skin, two specular lobes).

use bytemuck::{Pod, Zeroable};
use glam::{Affine3A, Mat4, Vec3};
use hearth_character::anatomy::Anatomy;
use hearth_character::rig::JOINTS;
use hearth_character::{Appearance, Pose, Rig};

use crate::figure::PreviewLight;
use crate::gpu::GpuContext;
use crate::terrain::DEPTH_FORMAT;

/// A skin vertex (36 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct BodyVertex {
    pos: [f32; 3],
    normal: [f32; 3],
    /// Four joint indices, one a byte.
    joints: u32,
    /// Their weights, unorm8 × 4.
    weights: u32,
    /// How much the skin is lips and nail there, unorm8 (and two bytes spare).
    tissue: u32,
}

/// One person's skin on the GPU.
pub struct GpuBody {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
    /// From the figure's frame to each joint's in the pose the skin was sculpted in.
    unbind: [Affine3A; JOINTS],
}

impl GpuBody {
    /// Uploads a body (`anatomy`'s output).
    pub fn new(ctx: &GpuContext, body: &Anatomy) -> Self {
        let verts: Vec<BodyVertex> = (0..body.positions.len())
            .map(|i| {
                // Weights to bytes summing to exactly 255 (the rounding's remainder to the
                // heaviest): a sum off by one moves the skin by a part in 255 of its distance
                // from the origin, centimetres at the head.
                let mut w = body.weights[i].map(|w| (w * 255.0).round() as i32);
                w[0] += 255 - w.iter().sum::<i32>();
                let w = w.map(|w| w.clamp(0, 255) as u8);
                let j = body.joints[i];
                BodyVertex {
                    pos: body.positions[i].to_array(),
                    normal: body.normals[i].to_array(),
                    joints: u32::from_le_bytes(j),
                    weights: u32::from_le_bytes(w),
                    tissue: u32::from_le_bytes([
                        (body.tissue[i][0] * 255.0).round() as u8,
                        (body.tissue[i][1] * 255.0).round() as u8,
                        0,
                        0,
                    ]),
                }
            })
            .collect();
        let vertices = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("body vertices"),
            size: (verts.len().max(1) * std::mem::size_of::<BodyVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.write_buffer(&vertices, 0, bytemuck::cast_slice(&verts));
        let indices = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("body indices"),
            size: (body.indices.len().max(1) * 4) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.write_buffer(&indices, 0, bytemuck::cast_slice(&body.indices));
        Self {
            vertices,
            indices,
            count: body.indices.len() as u32,
            unbind: body.bind.map(|b| b.inverse()),
        }
    }

    /// The skinning palette for a pose placed at `place`: each joint's pose times the inverse
    /// of its bind pose.
    pub fn palette(&self, rig: &Rig, pose: &Pose, place: Affine3A) -> [[[f32; 4]; 4]; JOINTS] {
        let joints = pose.joints(rig);
        std::array::from_fn(|i| {
            let m = place * joints[i] * self.unbind[i];
            Mat4::from(m).to_cols_array_2d()
        })
    }
}

/// How a person's skin is coloured (linear albedos).
#[derive(Debug, Clone, Copy)]
pub struct SkinLook {
    pub skin: [f32; 3],
    pub lips: [f32; 3],
    pub nail: [f32; 3],
}

impl SkinLook {
    pub fn of(a: &Appearance) -> Self {
        let skin = a.skin_linear();
        // Lips: the skin reddened by the blood near the surface; nails pale over pink.
        let lips = [skin[0] * 0.82, skin[1] * 0.46, skin[2] * 0.48];
        let nail = [
            skin[0] * 0.55 + 0.3,
            skin[1] * 0.55 + 0.25,
            skin[2] * 0.55 + 0.25,
        ];
        Self { skin, lips, nail }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct BodyUniform {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    light_dir: [f32; 4],
    light: [f32; 4],
    sky: [f32; 4],
    ground: [f32; 4],
    /// x: exposure; y: 1 to encode the display's gamma.
    exposure: [f32; 4],
    skin: [f32; 4],
    lips: [f32; 4],
    nail: [f32; 4],
    palette: [[[f32; 4]; 4]; JOINTS],
}

/// Bodies on their own (the creator's preview and the review), under a preview light.
pub struct BodyPreview {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    depth: Option<crate::offscreen::DepthTarget>,
    srgb: bool,
}

impl BodyPreview {
    pub fn new(ctx: &GpuContext, format: wgpu::TextureFormat) -> Self {
        let module = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("body.wgsl"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/body.wgsl").into()),
            });
        let layout = ctx
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("body"),
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
            label: Some("body"),
            size: std::mem::size_of::<BodyUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("body"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let pipeline_layout = ctx
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("body"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let attrs = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x3, 2 => Uint32, 3 => Unorm8x4, 4 => Unorm8x4
        ];
        let pipeline = ctx
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("body preview"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<BodyVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &attrs,
                    })],
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
                    module: &module,
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
            });
        Self {
            pipeline,
            uniform,
            bind,
            depth: None,
            srgb: format.is_srgb(),
        }
    }

    /// Draws `body` in `pose` into `rect` of `target`, seen from `eye` looking at `at` (the
    /// figure's frame), lit by `light`. One preview per submission.
    pub fn render(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        rect: [u32; 4],
        body: &GpuBody,
        palette: [[[f32; 4]; 4]; JOINTS],
        look: SkinLook,
        view: (Vec3, Vec3, f32),
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
        let (eye, at, fov_deg) = view;
        let aspect = w as f32 / h as f32;
        let view_m = glam::camera::rh::view::look_at_mat4(eye, at, Vec3::Y);
        let proj = glam::camera::rh::proj::directx::perspective_infinite_reverse(
            fov_deg.to_radians(),
            aspect,
            0.02,
        );
        let (dir, sun, sky, ground) = light.lighting();
        let level = (sun.y + sky.y).max(0.6);
        let u = BodyUniform {
            view_proj: (proj * view_m).to_cols_array_2d(),
            eye: eye.extend(0.0).to_array(),
            light_dir: dir.extend(0.0).to_array(),
            light: sun.extend(0.0).to_array(),
            sky: sky.extend(0.0).to_array(),
            ground: ground.extend(0.0).to_array(),
            exposure: [
                1.2 * std::f32::consts::PI / level,
                if self.srgb { 0.0 } else { 1.0 },
                0.0,
                0.0,
            ],
            skin: [look.skin[0], look.skin[1], look.skin[2], 0.0],
            lips: [look.lips[0], look.lips[1], look.lips[2], 0.0],
            nail: [look.nail[0], look.nail[1], look.nail[2], 0.0],
            palette,
        };
        ctx.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        let depth = &self.depth.as_ref().expect("depth").view;
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("body preview"),
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
        pass.set_bind_group(0, &self.bind, &[]);
        pass.set_vertex_buffer(0, body.vertices.slice(..));
        pass.set_index_buffer(body.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..body.count, 0, 0..1);
    }
}

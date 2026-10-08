//! People's bodies as smooth skinned meshes (Amendment E §8, E7): the anatomy of
//! `hearth_character::anatomy` uploaded once per person, each vertex carried by up to four
//! joints of the 17-joint palette of its pose, and shaded as skin, lips and nails (light that
//! scatters under the skin, two specular lobes).

use bytemuck::{Pod, Zeroable};
use glam::{Affine3A, Mat4, Vec3};
use hearth_character::anatomy::Anatomy;
use hearth_character::eyes::{EYE_PARTS, EyeMesh, EyeMotion, transforms};
use hearth_character::hair::{GUIDE_POINTS, GUIDES, HairMesh, HairSim};
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
    /// How much the skin is lips, nail and short hair there, and whether a garment's, unorm8.
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
                        (body.tissue[i][2] * 255.0).round() as u8,
                        (body.tissue[i][3] * 255.0).round() as u8,
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

/// A hair card vertex (48 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct HairVertex {
    pos: [f32; 3],
    tangent: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    /// Guide, strands, depth, along the guide.
    info: [u8; 4],
}

/// One person's hair on the GPU.
pub struct GpuHair {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
}

impl GpuHair {
    pub fn new(ctx: &GpuContext, hair: &HairMesh) -> Self {
        let verts: Vec<HairVertex> = (0..hair.positions.len())
            .map(|i| HairVertex {
                pos: hair.positions[i].to_array(),
                tangent: hair.tangents[i].to_array(),
                normal: hair.normals[i].to_array(),
                uv: hair.uv[i],
                info: hair.info[i],
            })
            .collect();
        let vertices = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hair vertices"),
            size: (verts.len().max(1) * std::mem::size_of::<HairVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.write_buffer(&vertices, 0, bytemuck::cast_slice(&verts));
        let indices = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hair indices"),
            size: (hair.indices.len().max(1) * 4) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.write_buffer(&indices, 0, bytemuck::cast_slice(&hair.indices));
        Self {
            vertices,
            indices,
            count: hair.indices.len() as u32,
        }
    }
}

/// The hair's look and motion for a frame.
#[derive(Debug, Clone, Copy)]
pub struct HairLook {
    /// Linear albedo.
    pub color: [f32; 3],
    /// How wet (0–1): darker, glossier.
    pub wet: f32,
    /// The guides' offsets (`HairSim::offsets`).
    pub guides: [[f32; 4]; GUIDES * GUIDE_POINTS],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct HairUniform {
    color: [f32; 4],
    guides: [[f32; 4]; GUIDES * GUIDE_POINTS],
}

/// An eye vertex (48 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct EyeVertex {
    pos: [f32; 3],
    normal: [f32; 3],
    local: [f32; 3],
    uv: [f32; 2],
    /// Part, surface, two spare.
    part: [u8; 4],
}

/// One person's eyes on the GPU.
pub struct GpuEyes {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
}

impl GpuEyes {
    pub fn new(ctx: &GpuContext, eyes: &EyeMesh) -> Self {
        let verts: Vec<EyeVertex> = (0..eyes.positions.len())
            .map(|i| EyeVertex {
                pos: eyes.positions[i].to_array(),
                normal: eyes.normals[i].to_array(),
                local: eyes.local[i].to_array(),
                uv: eyes.uv[i],
                part: [eyes.part[i][0], eyes.part[i][1], 0, 0],
            })
            .collect();
        let vertices = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("eye vertices"),
            size: (verts.len().max(1) * std::mem::size_of::<EyeVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.write_buffer(&vertices, 0, bytemuck::cast_slice(&verts));
        let indices = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("eye indices"),
            size: (eyes.indices.len().max(1) * 4) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.write_buffer(&indices, 0, bytemuck::cast_slice(&eyes.indices));
        Self {
            vertices,
            indices,
            count: eyes.indices.len() as u32,
        }
    }
}

/// The eyes' pose and colour for a frame.
#[derive(Debug, Clone, Copy)]
pub struct EyeLook {
    /// Each part's transform (`eyes::transforms`).
    pub parts: [Affine3A; EYE_PARTS],
    /// The iris's linear colour.
    pub iris: [f32; 3],
    /// The pupil's size (0 small, bright light – 1 wide, dark).
    pub pupil: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct EyeUniform {
    parts: [[[f32; 4]; 4]; EYE_PARTS],
    iris: [f32; 4],
}

/// How a person's skin is coloured (linear albedos).
#[derive(Debug, Clone, Copy)]
pub struct SkinLook {
    pub skin: [f32; 3],
    pub lips: [f32; 3],
    pub nail: [f32; 3],
    /// The hair's colour, for the short hair on the skin.
    pub hair: [f32; 3],
    /// What the body's life has done to the skin.
    pub state: SkinState,
    /// The garment's colour (linear) and kind (0 hide, 1 plant fibre).
    pub cloth: [f32; 4],
}

/// The skin's state as drawn (from the body simulation's `hearth_body::skin`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SkinState {
    /// Wet with water, 0–1.
    pub wet: f32,
    pub tan: f32,
    pub sunburn: f32,
    /// Pale from cold or blood lost, 0–1.
    pub pallor: f32,
    /// Flushed from heat or effort, 0–1.
    pub flush: f32,
    pub goosebumps: f32,
    /// Per joint: dirt, blood and scar (0–1 each) and whether clothing covers it from the sun.
    pub marks: [[f32; 4]; JOINTS],
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
        Self {
            skin,
            lips,
            nail,
            hair: a.hair_linear(),
            state: SkinState::default(),
            cloth: hearth_character::garment::loincloth_look(a).to_array(),
        }
    }
}

/// A person's meshes on the GPU: the skin (with its fitted garments), the hair, the eyes.
pub struct PersonMeshes {
    pub body: GpuBody,
    pub hair: Option<GpuHair>,
    pub eyes: Option<GpuEyes>,
}

impl PersonMeshes {
    /// Uploads a person's meshes (`hearth_character::person::meshes`).
    pub fn upload(ctx: &GpuContext, m: &hearth_character::person::Meshes) -> Self {
        Self {
            body: GpuBody::new(ctx, &m.body),
            hair: (!m.hair.indices.is_empty()).then(|| GpuHair::new(ctx, &m.hair)),
            eyes: m.eyes.as_ref().map(|e| GpuEyes::new(ctx, e)),
        }
    }
}

/// What moves on a person from frame to frame besides the pose: the hair's guides and the
/// eyes (gaze, saccades, blinks).
pub struct PersonMotion {
    hair: HairSim,
    eyes: Option<(EyeMesh, EyeMotion)>,
}

impl PersonMotion {
    pub fn new(a: &Appearance, m: &hearth_character::person::Meshes, seed: u64) -> Self {
        Self {
            hair: HairSim::new(&m.hair, a),
            eyes: m.eyes.clone().map(|e| (e, EyeMotion::new(seed))),
        }
    }

    /// The frame's data: the hair swung `dt` seconds by the head's motion and `wind` (m/s), the
    /// eyes moved toward `look` (yaw, pitch down; radians from the head), `blink` overriding
    /// the lids when set.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        a: &Appearance,
        palette: [[[f32; 4]; 4]; JOINTS],
        look: SkinLook,
        dt: f32,
        wind: Vec3,
        gaze: glam::Vec2,
        blink: Option<f32>,
        light: [f32; 2],
    ) -> PersonFrame {
        let head = Affine3A::from_mat4(Mat4::from_cols_array_2d(
            &palette[hearth_character::rig::Joint::Head.index()],
        ));
        self.hair.step(head, dt, wind, look.state.wet);
        let eyes = self.eyes.as_mut().map(|(mesh, motion)| {
            motion.step(dt, gaze);
            let shut = blink.unwrap_or(motion.blink());
            EyeLook {
                parts: transforms(mesh, head, motion.gaze(), shut),
                iris: hearth_character::appearance::srgb_to_linear(a.eyes.srgb()),
                pupil: 0.4,
            }
        });
        PersonFrame {
            palette,
            look,
            hair: Some(HairLook {
                color: a.hair_linear(),
                wet: look.state.wet,
                guides: self.hair.offsets(head),
            }),
            eyes,
            light,
        }
    }
}

/// A person's pose and looks for a frame.
#[derive(Debug, Clone, Copy)]
pub struct PersonFrame {
    pub palette: [[[f32; 4]; 4]; JOINTS],
    pub look: SkinLook,
    pub hair: Option<HairLook>,
    pub eyes: Option<EyeLook>,
    /// The sky's light and the firelight where the person is (0–1 each), for the world's
    /// lighting.
    pub light: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct PersonUniform {
    skin: [f32; 4],
    lips: [f32; 4],
    nail: [f32; 4],
    hair: [f32; 4],
    palette: [[[f32; 4]; 4]; JOINTS],
    state: [f32; 4],
    state2: [f32; 4],
    marks: [[f32; 4]; JOINTS],
    cloth: [f32; 4],
    light: [f32; 4],
}

impl PersonUniform {
    fn of(f: &PersonFrame) -> Self {
        let l = &f.look;
        let v = |c: [f32; 3]| [c[0], c[1], c[2], 0.0];
        Self {
            skin: v(l.skin),
            lips: v(l.lips),
            nail: v(l.nail),
            hair: v(l.hair),
            palette: f.palette,
            state: [l.state.wet, l.state.tan, l.state.sunburn, l.state.pallor],
            state2: [l.state.flush, l.state.goosebumps, 0.0, 0.0],
            marks: l.state.marks,
            cloth: l.cloth,
            light: [f.light[0], f.light[1], 0.0, 0.0],
        }
    }
}

/// One person's uniforms and their bind group (group 1 of the people's shaders).
struct PersonSlot {
    person: wgpu::Buffer,
    hair: wgpu::Buffer,
    eyes: wgpu::Buffer,
    bind: wgpu::BindGroup,
}

impl PersonSlot {
    fn new(ctx: &GpuContext, layout: &wgpu::BindGroupLayout) -> Self {
        let buffer = |label: &str, size: usize| {
            ctx.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let person = buffer("person", std::mem::size_of::<PersonUniform>());
        let hair = buffer("person hair", std::mem::size_of::<HairUniform>());
        let eyes = buffer("person eyes", std::mem::size_of::<EyeUniform>());
        let bind = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("person"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: person.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: hair.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: eyes.as_entire_binding(),
                },
            ],
        });
        Self {
            person,
            hair,
            eyes,
            bind,
        }
    }

    fn write(&self, ctx: &GpuContext, f: &PersonFrame) {
        ctx.queue
            .write_buffer(&self.person, 0, bytemuck::bytes_of(&PersonUniform::of(f)));
        if let Some(hl) = &f.hair {
            let hu = HairUniform {
                color: [hl.color[0], hl.color[1], hl.color[2], hl.wet],
                guides: hl.guides,
            };
            ctx.queue
                .write_buffer(&self.hair, 0, bytemuck::bytes_of(&hu));
        }
        if let Some(el) = &f.eyes {
            let eu = EyeUniform {
                parts: el.parts.map(|m| Mat4::from(m).to_cols_array_2d()),
                iris: [el.iris[0], el.iris[1], el.iris[2], el.pupil],
            };
            ctx.queue
                .write_buffer(&self.eyes, 0, bytemuck::bytes_of(&eu));
        }
    }
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// The people's three pipelines (skin, hair, eyes) for a view: its shader prefix and its
/// group 0 layout.
struct Pipelines {
    person_layout: wgpu::BindGroupLayout,
    skin: wgpu::RenderPipeline,
    hair: wgpu::RenderPipeline,
    eye: wgpu::RenderPipeline,
}

impl Pipelines {
    fn new(
        ctx: &GpuContext,
        view: &wgpu::BindGroupLayout,
        prefix: &str,
        format: wgpu::TextureFormat,
    ) -> Self {
        let person_layout = ctx
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("person"),
                entries: &[uniform_entry(0), uniform_entry(1), uniform_entry(2)],
            });
        let layout = ctx
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("person"),
                bind_group_layouts: &[Some(view), Some(&person_layout)],
                immediate_size: 0,
            });
        let make = |label: &str,
                    body: &str,
                    stride: usize,
                    attrs: &[wgpu::VertexAttribute],
                    cull: Option<wgpu::Face>| {
            let source = format!("{prefix}\n{}\n{body}", include_str!("shaders/person.wgsl"));
            let module = ctx
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(label),
                    source: wgpu::ShaderSource::Wgsl(source.into()),
                });
            ctx.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(label),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: &[Some(wgpu::VertexBufferLayout {
                            array_stride: stride as u64,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: attrs,
                        })],
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: cull,
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
                })
        };
        let skin_attrs = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x3, 2 => Uint32, 3 => Unorm8x4, 4 => Unorm8x4
        ];
        let hair_attrs = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x2, 4 => Uint8x4
        ];
        let eye_attrs = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x2, 4 => Uint8x4
        ];
        let skin = make(
            "body.wgsl",
            include_str!("shaders/body.wgsl"),
            std::mem::size_of::<BodyVertex>(),
            &skin_attrs,
            Some(wgpu::Face::Back),
        );
        let hair = make(
            "hair.wgsl",
            include_str!("shaders/hair.wgsl"),
            std::mem::size_of::<HairVertex>(),
            &hair_attrs,
            None,
        );
        let eye = make(
            "eye.wgsl",
            include_str!("shaders/eye.wgsl"),
            std::mem::size_of::<EyeVertex>(),
            &eye_attrs,
            None,
        );
        Self {
            person_layout,
            skin,
            hair,
            eye,
        }
    }

    /// Draws a person whose slot is bound at group 1.
    fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, m: &'a PersonMeshes, f: &PersonFrame) {
        pass.set_pipeline(&self.skin);
        pass.set_vertex_buffer(0, m.body.vertices.slice(..));
        pass.set_index_buffer(m.body.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..m.body.count, 0, 0..1);
        if let (Some(hair), Some(_)) = (&m.hair, &f.hair)
            && hair.count > 0
        {
            pass.set_pipeline(&self.hair);
            pass.set_vertex_buffer(0, hair.vertices.slice(..));
            pass.set_index_buffer(hair.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..hair.count, 0, 0..1);
        }
        if let (Some(eyes), Some(_)) = (&m.eyes, &f.eyes) {
            pass.set_pipeline(&self.eye);
            pass.set_vertex_buffer(0, eyes.vertices.slice(..));
            pass.set_index_buffer(eyes.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..eyes.count, 0, 0..1);
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct SceneUniform {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    light_dir: [f32; 4],
    light: [f32; 4],
    sky: [f32; 4],
    ground: [f32; 4],
    exposure: [f32; 4],
}

/// People on their own (the creator's preview and the review), under a preview light.
pub struct BodyPreview {
    pipelines: Pipelines,
    scene: wgpu::Buffer,
    scene_bind: wgpu::BindGroup,
    slot: PersonSlot,
    depth: Option<crate::offscreen::DepthTarget>,
    srgb: bool,
}

impl BodyPreview {
    pub fn new(ctx: &GpuContext, format: wgpu::TextureFormat) -> Self {
        let scene_layout = ctx
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("person preview"),
                entries: &[uniform_entry(0)],
            });
        let scene = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("person preview"),
            size: std::mem::size_of::<SceneUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let scene_bind = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("person preview"),
            layout: &scene_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: scene.as_entire_binding(),
            }],
        });
        let pipelines = Pipelines::new(
            ctx,
            &scene_layout,
            include_str!("shaders/person_preview.wgsl"),
            format,
        );
        let slot = PersonSlot::new(ctx, &pipelines.person_layout);
        Self {
            pipelines,
            scene,
            scene_bind,
            slot,
            depth: None,
            srgb: format.is_srgb(),
        }
    }

    /// Draws a person into `rect` of `target`, seen from `eye` looking at `at` (the figure's
    /// frame), lit by `light`. One preview per submission.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        rect: [u32; 4],
        meshes: &PersonMeshes,
        frame: &PersonFrame,
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
        let su = SceneUniform {
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
        };
        ctx.queue
            .write_buffer(&self.scene, 0, bytemuck::bytes_of(&su));
        self.slot.write(ctx, frame);
        let depth = &self.depth.as_ref().expect("depth").view;
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("person preview"),
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
        pass.set_bind_group(0, &self.scene_bind, &[]);
        pass.set_bind_group(1, &self.slot.bind, &[]);
        self.pipelines.draw(&mut pass, meshes, frame);
    }
}

/// People in the world: drawn into the scene's HDR target after the terrain, lit by its
/// globals (group 0) as the figures are.
pub struct PeopleRenderer {
    pipelines: Pipelines,
    slots: Vec<PersonSlot>,
    people: Vec<(std::sync::Arc<PersonMeshes>, PersonFrame)>,
}

impl PeopleRenderer {
    /// `globals` is the terrain's bind group 0 layout.
    pub fn new(ctx: &GpuContext, globals: &wgpu::BindGroupLayout) -> Self {
        let prefix = concat!(
            include_str!("shaders/common.wgsl"),
            include_str!("shaders/person_world.wgsl")
        );
        Self {
            pipelines: Pipelines::new(ctx, globals, prefix, crate::post::HDR_FORMAT),
            slots: Vec::new(),
            people: Vec::new(),
        }
    }

    /// This frame's people (camera-relative palettes).
    pub fn set(
        &mut self,
        ctx: &GpuContext,
        people: Vec<(std::sync::Arc<PersonMeshes>, PersonFrame)>,
    ) {
        while self.slots.len() < people.len() {
            self.slots
                .push(PersonSlot::new(ctx, &self.pipelines.person_layout));
        }
        for (slot, (_, frame)) in self.slots.iter().zip(&people) {
            slot.write(ctx, frame);
        }
        self.people = people;
    }

    pub fn count(&self) -> usize {
        self.people.len()
    }

    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, globals: &'a wgpu::BindGroup) {
        pass.set_bind_group(0, globals, &[]);
        for (slot, (meshes, frame)) in self.slots.iter().zip(&self.people) {
            pass.set_bind_group(1, &slot.bind, &[]);
            self.pipelines.draw(pass, meshes, frame);
        }
    }
}

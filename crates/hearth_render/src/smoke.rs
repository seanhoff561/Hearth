//! Smoke over fires in the vegetation (`shaders/smoke.wgsl`): up to 64 plumes, each a stream of
//! soft puffs generated on the GPU, rising, swelling and leaning with the wind. A far fire's
//! plume rises over a kilometre and is seen across the land.

use bytemuck::{Pod, Zeroable};
use glam::{DVec3, Vec3};

use crate::camera::Camera;
use crate::gpu::GpuContext;
use crate::post::HDR_FORMAT;

/// Plumes drawn at most.
pub const MAX_PLUMES: usize = 64;
/// Puffs in each plume.
const PUFFS: u32 = 80;

/// A plume of smoke: where it rises from and how thick it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmokePlume {
    pub at: DVec3,
    /// 0–1.
    pub strength: f32,
    /// A far fire's: it rises kilometres.
    pub far: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct PlumeGpu {
    pos: [f32; 4],
    params: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    right: [f32; 4],
    up: [f32; 4],
    wind: [f32; 4],
    ambient: [f32; 4],
    direct: [f32; 4],
    fire: [f32; 4],
    haze: [f32; 4],
    count: [u32; 4],
    plumes: [PlumeGpu; MAX_PLUMES],
}

/// The frame's light for the smoke (pre-exposed).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmokeLight {
    pub ambient: Vec3,
    pub direct: Vec3,
    pub fire: Vec3,
    pub haze: Vec3,
    /// Extinction of the haze (1/m).
    pub haze_extinction: f32,
}

pub struct SmokeRenderer {
    pipe: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind: wgpu::BindGroup,
    plumes: Vec<SmokePlume>,
    count: u32,
}

impl SmokeRenderer {
    pub fn new(ctx: &GpuContext) -> Self {
        let device = &ctx.device;
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("smoke uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("smoke layout"),
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
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("smoke bind"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("smoke.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/smoke.wgsl").into()),
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("smoke pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("smoke"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::terrain::DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                // Reverse-Z: nearer is greater.
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
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipe,
            uniforms,
            bind,
            plumes: Vec::new(),
            count: 0,
        }
    }

    /// Replaces the plumes (the nearest and thickest kept when there are too many).
    pub fn set_plumes(&mut self, plumes: Vec<SmokePlume>) {
        self.plumes = plumes;
    }

    pub fn plumes(&self) -> &[SmokePlume] {
        &self.plumes
    }

    /// Uploads the frame's plumes and light.
    pub fn prepare(
        &mut self,
        ctx: &GpuContext,
        camera: &Camera,
        aspect: f32,
        wind: Vec3,
        seconds: f32,
        light: &SmokeLight,
    ) {
        if self.plumes.is_empty() {
            self.count = 0;
            return;
        }
        let mut order: Vec<&SmokePlume> = self.plumes.iter().collect();
        order.sort_by(|a, b| {
            let wa = (a.at - camera.pos).length() / (0.2 + a.strength as f64);
            let wb = (b.at - camera.pos).length() / (0.2 + b.strength as f64);
            wa.total_cmp(&wb)
        });
        order.truncate(MAX_PLUMES);
        let mut plumes = [PlumeGpu::zeroed(); MAX_PLUMES];
        for (i, p) in order.iter().enumerate() {
            let rel = p.at - camera.pos;
            plumes[i] = PlumeGpu {
                pos: [
                    rel.x as f32,
                    rel.y as f32,
                    rel.z as f32,
                    p.strength.clamp(0.0, 1.0),
                ],
                params: [
                    if p.far { 1.0 } else { 0.0 },
                    // Its look by its place, x taken modulo 4096 (as every planet's circumference
                    // is): the same plume from either side of the seam.
                    (((p.at.x.floor() as i64).rem_euclid(4096) * 31 + p.at.z.floor() as i64)
                        .rem_euclid(9973)) as f32,
                    0.0,
                    0.0,
                ],
            };
        }
        let view = camera.view();
        let right = Vec3::new(view.x_axis.x, view.y_axis.x, view.z_axis.x);
        let up = Vec3::new(view.x_axis.y, view.y_axis.y, view.z_axis.y);
        let v4 = |v: Vec3, w: f32| [v.x, v.y, v.z, w];
        let u = Uniforms {
            view_proj: camera.view_proj(aspect).to_cols_array_2d(),
            right: v4(right, 0.0),
            up: v4(up, 0.0),
            wind: v4(wind, seconds),
            ambient: v4(light.ambient, 0.0),
            direct: v4(light.direct, 0.0),
            fire: v4(light.fire, 0.0),
            haze: v4(light.haze, light.haze_extinction),
            count: [order.len() as u32, PUFFS, 0, 0],
            plumes,
        };
        ctx.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));
        self.count = order.len() as u32 * PUFFS;
    }

    /// Draws into a pass whose depth target holds the scene.
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if self.count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipe);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.draw(0..self.count * 6, 0..1);
    }
}

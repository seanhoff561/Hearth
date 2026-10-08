//! Rain and snow (`shaders/precip.wgsl`): particles generated on the GPU in a box around the
//! camera, hidden under cover by a map of the highest sky-blocking block per column.

use bytemuck::{Pod, Zeroable};
use glam::{DVec2, Vec3};

use crate::camera::Camera;
use crate::gpu::GpuContext;
use crate::post::HDR_FORMAT;

/// Side of the sky-height map (columns) around the camera.
pub const HEIGHTS_SIZE: u32 = 128;
/// The box of falling particles around the camera (m).
const BOX: Vec3 = Vec3::new(48.0, 32.0, 48.0);
/// The period of the particles' clock (s): every fall speed is a whole number of boxes in it and
/// every wobble a whole number of turns, so the clock wraps unseen and stays precise however long
/// the world has run.
const PERIOD_S: f64 = 1000.0;
/// Particles at full intensity.
const MAX_PARTICLES: u32 = 40_000;
/// Stands for "nothing blocks the sky here".
const OPEN_SKY: i32 = i32::MIN / 2;

/// The highest sky-blocking block per column in a square around the camera.
#[derive(Debug, Clone, PartialEq)]
pub struct SkyHeights {
    /// World block (x, z) of the first entry.
    pub origin: (i32, i32),
    /// `HEIGHTS_SIZE`² heights (row-major by z); `None` columns are open.
    pub heights: Vec<i32>,
}

impl SkyHeights {
    /// Builds the map centred on a block column from a lookup of the highest sky-blocking Y.
    pub fn build(center_x: i32, center_z: i32, top: impl Fn(i32, i32) -> Option<i32>) -> Self {
        let n = HEIGHTS_SIZE as i32;
        let origin = (center_x - n / 2, center_z - n / 2);
        let mut heights = Vec::with_capacity((n * n) as usize);
        for z in 0..n {
            for x in 0..n {
                heights.push(top(origin.0 + x, origin.1 + z).unwrap_or(OPEN_SKY));
            }
        }
        Self { origin, heights }
    }
}

/// What is falling, from the weather.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Precipitation {
    /// How hard it rains or snows, 0..1.
    pub intensity: f32,
    /// Share of rain among the particles: 1 rain, 0 snow, between sleet.
    pub rain: f32,
    /// Wind (m/s, world axes).
    pub wind: Vec3,
    /// How far the air has carried the falling particles (m, world x and z), summed in real
    /// seconds (Amendment P §8): a change of wind changes how fast they drift, never where they
    /// are.
    pub drift: DVec2,
}

impl Default for Precipitation {
    fn default() -> Self {
        Self {
            intensity: 0.0,
            rain: 1.0,
            wind: Vec3::ZERO,
            drift: DVec2::ZERO,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    cam_mod: [f32; 4],
    cam: [f32; 4],
    rain: [f32; 4],
    wind: [f32; 4],
    box_size: [f32; 4],
    ambient: [f32; 4],
    direct: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    drift: [f32; 4],
}

pub struct PrecipRenderer {
    pipe: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    heights: wgpu::Texture,
    bind: wgpu::BindGroup,
    origin: (i32, i32),
    count: u32,
}

impl PrecipRenderer {
    pub fn new(ctx: &GpuContext) -> Self {
        let device = &ctx.device;
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("precip uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let heights = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sky heights"),
            size: wgpu::Extent3d {
                width: HEIGHTS_SIZE,
                height: HEIGHTS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Sint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("precip layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Sint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let view = heights.create_view(&Default::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("precip bind"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("precip.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/precip.wgsl").into()),
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("precip pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("precip"),
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
        let r = Self {
            pipe,
            uniforms,
            heights,
            bind,
            origin: (0, 0),
            count: 0,
        };
        // Until a map arrives nothing is covered.
        r.upload_heights(ctx, &vec![OPEN_SKY; (HEIGHTS_SIZE * HEIGHTS_SIZE) as usize]);
        r
    }

    fn upload_heights(&self, ctx: &GpuContext, heights: &[i32]) {
        ctx.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.heights,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(heights),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(HEIGHTS_SIZE * 4),
                rows_per_image: Some(HEIGHTS_SIZE),
            },
            wgpu::Extent3d {
                width: HEIGHTS_SIZE,
                height: HEIGHTS_SIZE,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Replaces the sky-height map.
    pub fn set_heights(&mut self, ctx: &GpuContext, h: &SkyHeights) {
        debug_assert_eq!(h.heights.len(), (HEIGHTS_SIZE * HEIGHTS_SIZE) as usize);
        self.origin = h.origin;
        self.upload_heights(ctx, &h.heights);
    }

    /// Uploads the frame's parameters: `seconds` is the world's clock in real seconds (the
    /// particles fall by it); `ambient` and `direct` are pre-exposed.
    pub fn prepare(
        &mut self,
        ctx: &GpuContext,
        camera: &Camera,
        aspect: f32,
        p: &Precipitation,
        seconds: f64,
        ambient: Vec3,
        direct: Vec3,
    ) {
        let intensity = p.intensity.clamp(0.0, 1.0);
        self.count = (MAX_PARTICLES as f32 * intensity.sqrt()) as u32;
        if self.count == 0 {
            return;
        }
        let pos = camera.pos;
        let modulo = |v: f64, m: f32| v.rem_euclid(m as f64) as f32;
        let fall = Vec3::new(p.wind.x, -8.5, p.wind.z);
        // Camera axes in world space: the rows of the view rotation.
        let view = camera.view();
        let right = Vec3::new(view.x_axis.x, view.y_axis.x, view.z_axis.x);
        let up = Vec3::new(view.x_axis.y, view.y_axis.y, view.z_axis.y);
        let u = Uniforms {
            view_proj: camera.view_proj(aspect).to_cols_array_2d(),
            cam_mod: [
                modulo(pos.x, BOX.x),
                modulo(pos.y, BOX.y),
                modulo(pos.z, BOX.z),
                seconds.rem_euclid(PERIOD_S) as f32,
            ],
            cam: [
                (pos.x - self.origin.0 as f64) as f32,
                pos.y as f32,
                (pos.z - self.origin.1 as f64) as f32,
                HEIGHTS_SIZE as f32,
            ],
            rain: [fall.x, fall.y, fall.z, p.rain.clamp(0.0, 1.0)],
            wind: [p.wind.x, p.wind.y, p.wind.z, 0.0],
            box_size: [BOX.x, BOX.y, BOX.z, 0.35 + 0.65 * intensity],
            ambient: [ambient.x, ambient.y, ambient.z, 0.0],
            direct: [direct.x, direct.y, direct.z, 0.0],
            right: [right.x, right.y, right.z, 0.0],
            up: [up.x, up.y, up.z, 0.0],
            drift: [
                modulo(p.drift.x, BOX.x),
                0.0,
                modulo(p.drift.y, BOX.z),
                PERIOD_S as f32,
            ],
        };
        ctx.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sky_heights_are_centred_and_default_open() {
        let h = SkyHeights::build(1000, -40, |x, z| (x == 1000 && z == -40).then_some(77));
        let n = HEIGHTS_SIZE as i32;
        assert_eq!(h.origin, (1000 - n / 2, -40 - n / 2));
        assert_eq!(h.heights.len(), (n * n) as usize);
        let centre = (n / 2 * n + n / 2) as usize;
        assert_eq!(h.heights[centre], 77);
        assert_eq!(h.heights[0], OPEN_SKY);
    }
}

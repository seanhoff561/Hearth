//! The sky: atmosphere lookup tables (transmittance and multiple scattering once, the
//! camera's sky view every frame) and the sky pass (sky, sun, moon, stars, clouds). See
//! `shaders/atmosphere.wgsl` and `shaders/sky.wgsl`.

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Vec2, Vec3};

use crate::gpu::GpuContext;
use crate::post::HDR_FORMAT;

const TRANSMITTANCE_SIZE: (u32, u32) = (256, 64);
const MULTISCATTER_SIZE: (u32, u32) = (32, 32);
/// Sky-view table size: azimuth × elevation (see `skyview_dir` in `atmosphere.wgsl`).
pub const SKYVIEW_SIZE: (u32, u32) = (256, 128);
const LUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Everything the sky needs for one frame. Illuminances are pre-exposed (lux × exposure).
#[derive(Debug, Clone, Copy)]
pub struct SkyParams {
    pub sun_dir: Vec3,
    pub sun_illuminance: f32,
    pub moon_dir: Vec3,
    pub moon_illuminance: f32,
    pub moon_phase: f32,
    /// Camera altitude above sea level (m).
    pub altitude: f32,
    /// Multiplier on aerosol (Mie) density: 1 clear, higher in haze.
    pub haze: f32,
    /// The stars' scintillation clock (real seconds, wrapped short for precision).
    pub seconds: f32,
    /// How turbulent the air is (0 calm … 1 a gale): the stars shiver more.
    pub turbulence: f32,
    /// Celestial → world rotation (stars).
    pub star_rotation: Mat3,
    pub star_visibility: f32,
    pub cloud_cover: f32,
    /// Cloud base height above the camera (blocks); ≤ 0 hides clouds.
    pub cloud_height: f32,
    pub cloud_offset: Vec2,
    pub cloud_churn: f32,
    pub exposure: f32,
    pub night: f32,
    /// Moon disc radiance scale (pre-exposed).
    pub moon_disc: f32,
    /// Direct sun + moon light at the camera and sky irradiance (pre-exposed, RGB).
    pub direct: Vec3,
    pub ambient: Vec3,
    /// Grey of an overcast sky (rgb, pre-exposed) and how far it replaces the clear sky (w).
    pub overcast: glam::Vec4,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    sun: [f32; 4],
    moon: [f32; 4],
    camera: [f32; 4],
    inv_view_proj: [[f32; 4]; 4],
    stars0: [f32; 4],
    stars1: [f32; 4],
    stars2: [f32; 4],
    clouds: [f32; 4],
    misc: [f32; 4],
    direct: [f32; 4],
    ambient: [f32; 4],
    overcast: [f32; 4],
}

fn lut(ctx: &GpuContext, label: &str, size: (u32, u32)) -> (wgpu::Texture, wgpu::TextureView) {
    let t = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: LUT_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let v = t.create_view(&Default::default());
    (t, v)
}

pub struct SkyRenderer {
    uniforms: wgpu::Buffer,
    /// Wraps in azimuth: for the sky-view table only.
    sampler: wgpu::Sampler,
    /// Clamps both ways: for the transmittance and multiple-scattering tables, whose edges are
    /// the horizon and the zenith (wrapping would blend them).
    _clamp_sampler: wgpu::Sampler,
    _transmittance: wgpu::Texture,
    _transmittance_view: wgpu::TextureView,
    _multiscatter: wgpu::Texture,
    multiscatter_view: wgpu::TextureView,
    _skyview: wgpu::Texture,
    pub skyview_view: wgpu::TextureView,
    skyview_pipe: wgpu::ComputePipeline,
    skyview_bind: wgpu::BindGroup,
    sky_pipe: wgpu::RenderPipeline,
    sky_bind: wgpu::BindGroup,
    /// The static LUTs are computed on the first update.
    static_done: bool,
    transmittance_pipe: wgpu::ComputePipeline,
    transmittance_bind: wgpu::BindGroup,
    multiscatter_pipe: wgpu::ComputePipeline,
    multiscatter_bind: wgpu::BindGroup,
    params: Option<SkyParams>,
    /// Haze the static tables were computed with.
    static_haze: f32,
}

impl SkyRenderer {
    pub fn new(ctx: &GpuContext) -> Self {
        let device = &ctx.device;
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sky uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sky lut sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let clamp_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sky lut clamp sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let (transmittance, transmittance_view) = lut(ctx, "transmittance lut", TRANSMITTANCE_SIZE);
        let (multiscatter, multiscatter_view) = lut(ctx, "multiscatter lut", MULTISCATTER_SIZE);
        let (skyview, skyview_view) = lut(ctx, "skyview lut", SKYVIEW_SIZE);
        // Placeholders for bindings a compute pass doesn't read (can't read and write one LUT).
        let (_p1, placeholder) = lut(ctx, "lut placeholder", (1, 1));
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("atmosphere.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/atmosphere.wgsl").into()),
        });
        let tex = |binding: u32, stage: wgpu::ShaderStages| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: stage,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let uniform_entry = |stage: wgpu::ShaderStages| wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: stage,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let cs = wgpu::ShaderStages::COMPUTE;
        let compute_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("atmosphere compute layout"),
            entries: &[
                uniform_entry(cs),
                tex(1, cs),
                tex(2, cs),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: cs,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: cs,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: LUT_FORMAT,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let compute_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("atmosphere compute"),
            bind_group_layouts: &[Some(&compute_layout)],
            immediate_size: 0,
        });
        let pipe = |entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&compute_pl),
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let bind =
            |read1: &wgpu::TextureView, read2: &wgpu::TextureView, out: &wgpu::TextureView| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("atmosphere bind"),
                    layout: &compute_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: uniforms.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(read1),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(read2),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::Sampler(&clamp_sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::TextureView(out),
                        },
                    ],
                })
            };
        let transmittance_pipe = pipe("transmittance_main");
        let multiscatter_pipe = pipe("multiscatter_main");
        let skyview_pipe = pipe("skyview_main");
        let transmittance_bind = bind(&placeholder, &placeholder, &transmittance_view);
        let multiscatter_bind = bind(&transmittance_view, &placeholder, &multiscatter_view);
        let skyview_bind = bind(&transmittance_view, &multiscatter_view, &skyview_view);

        // Sky pass.
        let sky_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sky.wgsl").into()),
        });
        let fs = wgpu::ShaderStages::FRAGMENT;
        let sky_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sky layout"),
            entries: &[
                uniform_entry(wgpu::ShaderStages::VERTEX_FRAGMENT),
                tex(1, fs),
                tex(2, fs),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: fs,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: fs,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sky_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sky bind"),
            layout: &sky_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&skyview_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&transmittance_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&clamp_sampler),
                },
            ],
        });
        let sky_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sky pipeline layout"),
            bind_group_layouts: &[Some(&sky_layout)],
            immediate_size: 0,
        });
        let sky_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&sky_pl),
            vertex: wgpu::VertexState {
                module: &sky_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            // Drawn after opaque terrain: only where nothing was drawn (depth still 0).
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::terrain::DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &sky_module,
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
            uniforms,
            sampler,
            _clamp_sampler: clamp_sampler,
            _transmittance: transmittance,
            _transmittance_view: transmittance_view,
            _multiscatter: multiscatter,
            multiscatter_view,
            _skyview: skyview,
            skyview_view,
            skyview_pipe,
            skyview_bind,
            sky_pipe,
            sky_bind,
            static_done: false,
            transmittance_pipe,
            transmittance_bind,
            multiscatter_pipe,
            multiscatter_bind,
            params: None,
            static_haze: 1.0,
        }
    }

    /// Sampler for the sky-view LUT (linear, wraps in azimuth).
    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    pub fn multiscatter_view(&self) -> &wgpu::TextureView {
        &self.multiscatter_view
    }

    /// Uploads parameters and records the LUT updates.
    pub fn update(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        p: &SkyParams,
        view_proj: Mat4,
    ) {
        let r = p.star_rotation;
        let u = Uniforms {
            sun: [p.sun_dir.x, p.sun_dir.y, p.sun_dir.z, p.sun_illuminance],
            moon: [p.moon_dir.x, p.moon_dir.y, p.moon_dir.z, p.moon_illuminance],
            camera: [p.altitude, p.haze, p.moon_phase, p.seconds],
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            // Rows of the celestial→world matrix = columns of its transpose (world→celestial).
            stars0: [r.x_axis.x, r.y_axis.x, r.z_axis.x, 0.0],
            stars1: [r.x_axis.y, r.y_axis.y, r.z_axis.y, 0.0],
            stars2: [r.x_axis.z, r.y_axis.z, r.z_axis.z, 0.0],
            clouds: [
                p.cloud_cover,
                p.cloud_height,
                p.cloud_offset.x,
                p.cloud_offset.y,
            ],
            misc: [p.star_visibility, p.exposure, p.night, p.moon_disc],
            direct: [p.direct.x, p.direct.y, p.direct.z, p.turbulence],
            ambient: [p.ambient.x, p.ambient.y, p.ambient.z, p.cloud_churn],
            overcast: p.overcast.to_array(),
        };
        ctx.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("atmosphere"),
            timestamp_writes: None,
        });
        let dispatch = |pass: &mut wgpu::ComputePass<'_>,
                        pipe: &wgpu::ComputePipeline,
                        bind: &wgpu::BindGroup,
                        size: (u32, u32)| {
            pass.set_pipeline(pipe);
            pass.set_bind_group(0, bind, &[]);
            pass.dispatch_workgroups(size.0.div_ceil(8), size.1.div_ceil(8), 1);
        };
        // Transmittance and multiple scattering depend on the aerosol density.
        if (p.haze - self.static_haze).abs() > 0.05 * self.static_haze.max(0.1) {
            self.static_done = false;
            self.static_haze = p.haze;
        }
        if !self.static_done {
            dispatch(
                &mut pass,
                &self.transmittance_pipe,
                &self.transmittance_bind,
                TRANSMITTANCE_SIZE,
            );
            dispatch(
                &mut pass,
                &self.multiscatter_pipe,
                &self.multiscatter_bind,
                MULTISCATTER_SIZE,
            );
            self.static_done = true;
        }
        dispatch(
            &mut pass,
            &self.skyview_pipe,
            &self.skyview_bind,
            SKYVIEW_SIZE,
        );
        self.params = Some(*p);
    }

    /// Reads the sky-view table back (row-major, `SKYVIEW_SIZE`; radiance, pre-exposed). For
    /// tests and tools: it waits for the GPU.
    pub fn read_skyview(&self, ctx: &GpuContext) -> Vec<[f32; 3]> {
        let (w, h) = SKYVIEW_SIZE;
        let bpr = w * 8;
        let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("skyview readback"),
            size: (bpr * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("skyview readback"),
            });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self._skyview,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        ctx.queue.submit(Some(enc.finish()));
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = ctx.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        let data = slice.get_mapped_range().expect("mapped skyview buffer");
        let out = data
            .as_chunks::<8>()
            .0
            .iter()
            .map(|t| {
                let c = |i: usize| f16_to_f32(u16::from_le_bytes([t[i], t[i + 1]]));
                [c(0), c(2), c(4)]
            })
            .collect();
        drop(data);
        buffer.unmap();
        out
    }

    /// Draws the sky into a pass whose depth target already holds the opaque geometry.
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_pipeline(&self.sky_pipe);
        pass.set_bind_group(0, &self.sky_bind, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// IEEE half → single precision.
fn f16_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = ((h >> 10) & 0x1f) as i32;
    let frac = (h & 0x3ff) as f32;
    match exp {
        0 => sign * frac * 2f32.powi(-24),
        31 => {
            if frac == 0.0 {
                sign * f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => sign * (1.0 + frac / 1024.0) * 2f32.powi(exp - 15),
    }
}

#[cfg(test)]
mod tests {
    use super::f16_to_f32;

    #[test]
    fn half_floats_decode() {
        assert_eq!(f16_to_f32(0x3c00), 1.0);
        assert_eq!(f16_to_f32(0xc000), -2.0);
        assert_eq!(f16_to_f32(0x7bff), 65504.0);
        assert!((f16_to_f32(0x0001) - 5.96e-8).abs() < 1e-9);
        assert!(f16_to_f32(0x7c00).is_infinite());
    }
}

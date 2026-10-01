//! Water surfaces (v1 §9.3): what the water shading reads besides the terrain's globals — the
//! scene behind the water (its colour, copied before the translucent pass, and the depth buffer,
//! read-only during it) for refraction and the water's depth, tiling wave normals, and the wind.

use bytemuck::{Pod, Zeroable};
use glam::Vec2;

use crate::gpu::GpuContext;
use crate::post::HDR_FORMAT;

/// Side of the tiling wave normal texture (texels).
const WAVE_SIZE: u32 = 256;

/// How the water is shaded: `Low` lays a translucent surface over the scene with the sky's
/// reflection; `Medium` refracts the scene behind it, absorbed by the water's depth, with foam on
/// the shores; `High` adds screen-space reflections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterQuality {
    Low = 0,
    Medium = 1,
    High = 2,
}

impl From<hearth_core::options::Quality> for WaterQuality {
    fn from(q: hearth_core::options::Quality) -> Self {
        use hearth_core::options::Quality;
        match q {
            Quality::Off | Quality::Low => WaterQuality::Low,
            Quality::Medium => WaterQuality::Medium,
            Quality::High => WaterQuality::High,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Params {
    /// xy: the direction the wind blows toward (unit, world x and z), z: wind speed (m/s), w:
    /// quality.
    wind: [f32; 4],
    /// xy: 1 / render size, z: camera near plane, w: unused.
    screen: [f32; 4],
}

/// The scene's colour copied before the translucent pass, at the render size.
struct SceneCopy {
    size: (u32, u32),
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

pub struct WaterRenderer {
    layout: wgpu::BindGroupLayout,
    /// The waves alone (parameters, wave texture, sampler), for the distant land, which draws
    /// while the depth buffer is written.
    waves_layout: wgpu::BindGroupLayout,
    waves_bind: wgpu::BindGroup,
    params: wgpu::Buffer,
    waves: wgpu::TextureView,
    wave_sampler: wgpu::Sampler,
    copy: Option<SceneCopy>,
    /// Stand-ins bound while there is no copy (Low quality): a black pixel and a far depth.
    blank_color: wgpu::TextureView,
    blank_depth: wgpu::TextureView,
    /// The bind group and the depth view it reads (rebuilt when either changes).
    bind: Option<(wgpu::TextureView, wgpu::BindGroup)>,
    pub quality: WaterQuality,
}

impl WaterRenderer {
    pub fn new(ctx: &GpuContext) -> Self {
        let device = &ctx.device;
        let fs = wgpu::ShaderStages::FRAGMENT;
        let entry = |binding: u32, ty: wgpu::BindingType| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: fs,
            ty,
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water layout"),
            entries: &[
                entry(
                    0,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                entry(
                    1,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(
                    2,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(
                    3,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(
                    4,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                ),
            ],
        });
        let waves_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water waves layout"),
            entries: &[
                entry(
                    0,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                entry(
                    3,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(
                    4,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                ),
            ],
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("water params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let waves = upload_waves(ctx);
        let wave_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("wave sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        });
        let blank = |label: &str, format: wgpu::TextureFormat, usage: wgpu::TextureUsages| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let blank_color = blank(
            "water: no scene copy",
            HDR_FORMAT,
            wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let blank_depth = blank(
            "water: no depth",
            crate::terrain::DEPTH_FORMAT,
            wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let waves_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water waves bind"),
            layout: &waves_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&waves),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&wave_sampler),
                },
            ],
        });
        Self {
            layout,
            waves_layout,
            waves_bind,
            params,
            waves,
            wave_sampler,
            copy: None,
            blank_color,
            blank_depth,
            bind: None,
            quality: WaterQuality::Medium,
        }
    }

    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    /// The waves' layout and bind group, for the distant land.
    pub fn waves_layout(&self) -> &wgpu::BindGroupLayout {
        &self.waves_layout
    }

    pub fn waves_bind(&self) -> &wgpu::BindGroup {
        &self.waves_bind
    }

    /// Whether the water reads the scene behind it (and so needs it copied and the depth buffer
    /// read-only in the translucent pass).
    pub fn reads_scene(&self) -> bool {
        self.quality != WaterQuality::Low
    }

    /// Sets the frame's parameters: the render size, the wind (direction it blows toward and
    /// speed) and the camera's near plane; keeps a scene copy of that size when needed.
    pub fn prepare(
        &mut self,
        ctx: &GpuContext,
        size: (u32, u32),
        wind_dir: Vec2,
        wind_speed_m_s: f32,
        near: f32,
    ) {
        if self.reads_scene() {
            if self.copy.as_ref().is_none_or(|c| c.size != size) {
                let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("scene behind the water"),
                    size: wgpu::Extent3d {
                        width: size.0.max(1),
                        height: size.1.max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: HDR_FORMAT,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                let view = texture.create_view(&Default::default());
                self.copy = Some(SceneCopy {
                    size,
                    texture,
                    view,
                });
                self.bind = None;
            }
        } else if self.copy.take().is_some() {
            self.bind = None;
        }
        let dir = wind_dir.try_normalize().unwrap_or(Vec2::X);
        let p = Params {
            wind: [
                dir.x,
                dir.y,
                wind_speed_m_s.max(0.0),
                self.quality as u32 as f32,
            ],
            screen: [
                1.0 / size.0.max(1) as f32,
                1.0 / size.1.max(1) as f32,
                near,
                0.0,
            ],
        };
        ctx.write_buffer(&self.params, 0, bytemuck::bytes_of(&p));
    }

    /// Copies the scene drawn so far (opaque terrain, distant land, sky) for the water to see
    /// through: the part of the screen `rect` (0–1, y down) covers, with a margin for the waves'
    /// refraction. Record between the sky and the translucent pass.
    pub fn copy_scene(
        &self,
        enc: &mut wgpu::CommandEncoder,
        color: &wgpu::Texture,
        rect: [f32; 4],
    ) {
        let Some(copy) = &self.copy else {
            return;
        };
        let (w, h) = copy.size;
        if color.width() != w || color.height() != h {
            return;
        }
        const MARGIN: f32 = 32.0;
        let x0 = (rect[0] * w as f32 - MARGIN).max(0.0) as u32;
        let y0 = (rect[1] * h as f32 - MARGIN).max(0.0) as u32;
        let x1 = ((rect[2] * w as f32 + MARGIN).ceil() as u32).min(w);
        let y1 = ((rect[3] * h as f32 + MARGIN).ceil() as u32).min(h);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        fn at(texture: &wgpu::Texture, x: u32, y: u32) -> wgpu::TexelCopyTextureInfo<'_> {
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            }
        }
        enc.copy_texture_to_texture(
            at(color, x0, y0),
            at(&copy.texture, x0, y0),
            wgpu::Extent3d {
                width: x1 - x0,
                height: y1 - y0,
                depth_or_array_layers: 1,
            },
        );
    }

    /// The bind group of this frame, reading `depth` (the depth buffer, read-only in the
    /// translucent pass).
    pub fn bind(&mut self, ctx: &GpuContext, depth: &wgpu::TextureView) -> &wgpu::BindGroup {
        let reads = self.reads_scene() && self.copy.is_some();
        let depth = if reads { depth } else { &self.blank_depth };
        if self.bind.as_ref().is_none_or(|(v, _)| v != depth) {
            let color = match &self.copy {
                Some(c) if reads => &c.view,
                _ => &self.blank_color,
            };
            let bind = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("water bind"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(color),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(depth),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&self.waves),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&self.wave_sampler),
                    },
                ],
            });
            self.bind = Some((depth.clone(), bind));
        }
        &self.bind.as_ref().expect("made above").1
    }
}

/// Slopes of a tiling field of waves (x and z), from a spectrum of wind-driven waves: wavelengths
/// from the whole tile to a few texels, travelling within ±70° of +x, longer ones taller (height
/// ∝ wavelength^1.3), with whole numbers of crests across the tile so it repeats seamlessly. The
/// slopes are scaled so the steepest is about 1.
pub fn wave_slopes(size: u32) -> Vec<[f32; 2]> {
    let n = size as usize;
    let mut slopes = vec![[0.0f32; 2]; n * n];
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut rand = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let tau = std::f64::consts::TAU;
    for _ in 0..48 {
        // Crests across the tile: 1 to 40, log-uniform.
        let k = (40f64.ln() * rand()).exp();
        let angle = (rand() - 0.5) * 2.0 * 70f64.to_radians();
        let (kx, kz) = (
            (k * angle.cos()).round() as i32,
            (k * angle.sin()).round() as i32,
        );
        if kx == 0 && kz == 0 {
            continue;
        }
        let len = ((kx * kx + kz * kz) as f64).sqrt();
        let amp = len.powf(-1.3) * (0.6 + 0.8 * rand());
        let phase = rand() * tau;
        for z in 0..n {
            for x in 0..n {
                let arg = tau * (kx as f64 * x as f64 + kz as f64 * z as f64) / n as f64 + phase;
                // Height a·sin(arg): its slope is a·k·cos(arg), with k in radians per texel.
                let d = amp * arg.cos() * tau / n as f64;
                let s = &mut slopes[z * n + x];
                s[0] += (d * kx as f64) as f32;
                s[1] += (d * kz as f64) as f32;
            }
        }
    }
    let max = slopes
        .iter()
        .map(|s| s[0].hypot(s[1]))
        .fold(0.0f32, f32::max)
        .max(1e-6);
    for s in &mut slopes {
        s[0] /= max;
        s[1] /= max;
    }
    slopes
}

/// The wave slopes as a mipmapped RG texture (slope × 0.5 + 0.5); coarser levels average the
/// slopes, so distant water flattens instead of glittering at random.
fn upload_waves(ctx: &GpuContext) -> wgpu::TextureView {
    let levels = WAVE_SIZE.ilog2() + 1;
    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("wave normals"),
        size: wgpu::Extent3d {
            width: WAVE_SIZE,
            height: WAVE_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rg16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut level = wave_slopes(WAVE_SIZE);
    let mut size = WAVE_SIZE as usize;
    for mip in 0..levels {
        let data: Vec<u16> = level
            .iter()
            .flat_map(|s| [half(s[0] * 0.5 + 0.5), half(s[1] * 0.5 + 0.5)])
            .collect();
        ctx.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: mip,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&data),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size as u32 * 4),
                rows_per_image: Some(size as u32),
            },
            wgpu::Extent3d {
                width: size as u32,
                height: size as u32,
                depth_or_array_layers: 1,
            },
        );
        if size == 1 {
            break;
        }
        let half_size = size / 2;
        let mut next = vec![[0.0f32; 2]; half_size * half_size];
        for z in 0..half_size {
            for x in 0..half_size {
                let mut acc = [0.0f32; 2];
                for (dx, dz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let s = level[(2 * z + dz) * size + 2 * x + dx];
                    acc[0] += s[0] * 0.25;
                    acc[1] += s[1] * 0.25;
                }
                next[z * half_size + x] = acc;
            }
        }
        level = next;
        size = half_size;
    }
    texture.create_view(&Default::default())
}

/// An f32 as IEEE half-precision bits (normal range, rounded toward zero; enough for slopes).
fn half(v: f32) -> u16 {
    let bits = v.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    if exp <= 0 {
        return sign;
    }
    if exp >= 31 {
        return sign | 0x7c00;
    }
    sign | ((exp as u16) << 10) | ((bits >> 13) & 0x3ff) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waves_tile_and_average_out() {
        let n = 64;
        let s = wave_slopes(n);
        let mean = s
            .iter()
            .fold([0.0f32; 2], |a, v| [a[0] + v[0], a[1] + v[1]]);
        assert!(mean[0].abs() / (n * n) as f32 <= 1e-3 && mean[1].abs() / (n * n) as f32 <= 1e-3);
        let max = s.iter().map(|v| v[0].hypot(v[1])).fold(0.0, f32::max);
        assert!((max - 1.0).abs() < 1e-4);
        // Seamless: the step across the tile's edge is like a step inside it.
        let step = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
        let n = n as usize;
        let inner: f32 = (0..n).map(|z| step(s[z * n + 1], s[z * n])).sum::<f32>() / n as f32;
        let edge: f32 = (0..n)
            .map(|z| step(s[z * n], s[z * n + n - 1]))
            .sum::<f32>()
            / n as f32;
        assert!(edge < inner * 2.0, "edge {edge} inner {inner}");
        assert_eq!(half(1.0), 0x3c00);
        assert_eq!(half(0.5), 0x3800);
        assert_eq!(half(-2.0), 0xc000);
    }
}

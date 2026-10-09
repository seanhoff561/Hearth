//! The planet as a globe (v2 §16; the minimal spawn picker until the world-creation screens):
//! the world map wrapped on a sphere the player turns, zooms and clicks, drawn over the whole
//! frame, in relief (Amendment T §2.4): the map's heights light the land and the sea's floor as
//! the globe turns, tinted by height and depth, its rivers, lakes and ice drawn; by biome, by
//! climate or as plain relief.

use std::f32::consts::{PI, TAU};

use glam::{Vec2, Vec3};

use crate::gpu::GpuContext;

/// What the globe's land is coloured by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlobeMode {
    /// The biomes' colours, a little tinted by height.
    #[default]
    Biomes,
    /// The climate: warmth in hue, rain in depth of colour.
    Climate,
    /// Plain relief: the land tinted by height alone (green lowlands to white peaks).
    Relief,
}

impl GlobeMode {
    pub const ALL: [GlobeMode; 3] = [GlobeMode::Biomes, GlobeMode::Climate, GlobeMode::Relief];

    /// The next mode round.
    pub fn next(self) -> Self {
        match self {
            GlobeMode::Biomes => GlobeMode::Climate,
            GlobeMode::Climate => GlobeMode::Relief,
            GlobeMode::Relief => GlobeMode::Biomes,
        }
    }
}

/// How the globe is turned and how near it is, and how it is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobeView {
    /// Latitude and longitude (radians) at the globe's centre.
    pub lat: f32,
    pub lon: f32,
    /// 1 fits the globe in the frame with a margin.
    pub zoom: f32,
    /// How much the relief's slopes are exaggerated in its light (1 true; the default modest).
    pub relief: f32,
    pub mode: GlobeMode,
}

impl Default for GlobeView {
    fn default() -> Self {
        Self {
            lat: 0.0,
            lon: 0.0,
            zoom: 1.0,
            relief: GlobeView::RELIEF,
            mode: GlobeMode::Biomes,
        }
    }
}

/// A unit vector for a latitude and longitude (radians): the planet's convention (north +Y,
/// longitude 0 along +X, increasing toward +Z).
pub fn sphere_point(lat: f32, lon: f32) -> Vec3 {
    let (sa, ca) = lat.sin_cos();
    let (so, co) = lon.sin_cos();
    Vec3::new(ca * co, sa, ca * so)
}

impl GlobeView {
    pub const MAX_ZOOM: f32 = 16.0;
    /// The relief's exaggeration by default, and its range.
    pub const RELIEF: f32 = 3.0;
    pub const RELIEF_RANGE: (f32, f32) = (1.0, 12.0);

    /// Exaggerates the relief more (positive steps) or less.
    pub fn relief_by(&mut self, steps: i32) {
        let (lo, hi) = Self::RELIEF_RANGE;
        self.relief = (self.relief * 1.5f32.powi(steps)).clamp(lo, hi);
    }

    /// The globe's radius in pixels.
    pub fn radius_px(&self, size: (u32, u32)) -> f32 {
        0.45 * size.0.min(size.1) as f32 * self.zoom
    }

    /// East, north and outward at the centre: the frame's right, up and toward the viewer.
    fn basis(&self) -> [Vec3; 3] {
        let (sa, ca) = self.lat.sin_cos();
        let (so, co) = self.lon.sin_cos();
        [
            Vec3::new(-so, 0.0, co),
            Vec3::new(-sa * co, ca, -sa * so),
            Vec3::new(ca * co, sa, ca * so),
        ]
    }

    /// The latitude and longitude (radians) under a pixel, if it shows the globe.
    pub fn pick(&self, px: Vec2, size: (u32, u32)) -> Option<(f32, f32)> {
        let half = 0.5 * Vec2::new(size.0 as f32, size.1 as f32);
        let q = (px - half) / self.radius_px(size) * Vec2::new(1.0, -1.0);
        let d2 = q.length_squared();
        if d2 > 1.0 {
            return None;
        }
        let [e, n, o] = self.basis();
        let p = e * q.x + n * q.y + o * (1.0 - d2).sqrt();
        Some((p.y.clamp(-1.0, 1.0).asin(), p.z.atan2(p.x)))
    }

    /// Where a point (latitude, longitude in radians) appears in the frame, if on the near
    /// side.
    pub fn project(&self, lat: f32, lon: f32, size: (u32, u32)) -> Option<Vec2> {
        let p = sphere_point(lat, lon);
        let [e, n, o] = self.basis();
        if p.dot(o) <= 0.0 {
            return None;
        }
        let half = 0.5 * Vec2::new(size.0 as f32, size.1 as f32);
        Some(half + Vec2::new(p.dot(e), -p.dot(n)) * self.radius_px(size))
    }

    /// Turns the globe by a drag of the cursor (pixels): the land at the centre follows the
    /// cursor (faster east–west near the poles, up to a limit).
    pub fn drag(&mut self, delta: Vec2, size: (u32, u32)) {
        let r = self.radius_px(size);
        let across = r * self.lat.cos().max(0.3);
        self.lon = (self.lon - delta.x / across + PI).rem_euclid(TAU) - PI;
        self.lat = (self.lat + delta.y / r).clamp(-1.4, 1.4);
    }

    /// Zooms by scroll steps (positive: nearer).
    pub fn zoom_by(&mut self, steps: i32) {
        self.zoom = (self.zoom * 1.25f32.powi(steps)).clamp(1.0, Self::MAX_ZOOM);
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GlobeParams {
    /// East, north and outward at the centre; w: the radius (px), frame width and height.
    right: [f32; 4],
    up: [f32; 4],
    out: [f32; 4],
    /// The camera's place and the point under the cursor (unit vectors); w: 1 when shown.
    camera: [f32; 4],
    cursor: [f32; 4],
    /// The relief's exaggeration, the mode (0 biomes, 1 climate, 2 relief), the planet's
    /// radius (m) and the relief's width (texels).
    look: [f32; 4],
    /// The finer relief's window (radians: south, north, west, east; none when north is not
    /// above south) and its width (texels).
    detail: [f32; 4],
    detail_size: [f32; 4],
}

/// A window of finer relief for the globe seen near (`hearth::globe::Detail`): its edges
/// (radians: south, north, west, east) and its heights (m, half floats), `size` square, rows
/// from the north.
#[derive(Debug, Clone, Copy)]
pub struct DetailLayer<'a> {
    pub window: [f32; 4],
    pub size: u32,
    pub elevation: &'a [u16],
}

/// The planet's map as the globe takes it (`hearth::globe::GlobeMap`): equirectangular, rows
/// from the north pole to the south, columns eastward from longitude 0.
#[derive(Debug, Clone, Copy)]
pub struct MapLayers<'a> {
    pub width: u32,
    pub height: u32,
    /// The ground's colour (sRGB) and its water (alpha: 0 land, 128 a lake, 255 the sea).
    pub color: &'a [[u8; 4]],
    /// The river, ice, temperature and rain (0–255 each).
    pub facts: &'a [[u8; 4]],
    /// The surface's height, or the water's floor's (m), as half floats: `relief_width` by half
    /// that (finer than the rest).
    pub relief_width: u32,
    pub elevation: &'a [u16],
}

/// Draws the globe from an equirectangular map of the planet.
pub struct GlobeRenderer {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    sampler: wgpu::Sampler,
    bind: wgpu::BindGroup,
    has_map: bool,
    /// The planet's radius (m) and the relief's width (texels).
    radius_m: f32,
    width: u32,
    /// The map's layers and the finer relief's, and that relief's window and size.
    maps: [wgpu::TextureView; 3],
    detail: wgpu::TextureView,
    window: Option<([f32; 4], u32)>,
}

impl GlobeRenderer {
    pub fn new(ctx: &GpuContext, output_format: wgpu::TextureFormat) -> Self {
        let device = &ctx.device;
        let fs = wgpu::ShaderStages::FRAGMENT;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globe layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: fs,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: fs,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: fs,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                texture_entry(3),
                texture_entry(4),
                texture_entry(5),
            ],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("globe.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/globe.wgsl").into()),
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("globe pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("globe"),
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
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globe params"),
            size: std::mem::size_of::<GlobeParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("globe sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        });
        // Until the map is made: an ocean 4 km deep.
        let blank = MapLayers {
            width: 1,
            height: 1,
            color: &[[20, 50, 110, 255]],
            facts: &[[0, 0, 128, 64]],
            relief_width: 2,
            elevation: &[crate::skin_lut::to_f16(-4000.0); 2],
        };
        let maps = upload_layers(ctx, &blank);
        let detail = upload_heights(ctx, "globe detail", 1, 1, &[0]);
        let bind = Self::bind_group(ctx, &layout, &params, &maps, &detail, &sampler);
        Self {
            pipeline,
            layout,
            params,
            sampler,
            bind,
            has_map: false,
            radius_m: 6.371e6,
            width: 1,
            maps,
            detail,
            window: None,
        }
    }

    fn bind_group(
        ctx: &GpuContext,
        layout: &wgpu::BindGroupLayout,
        params: &wgpu::Buffer,
        maps: &[wgpu::TextureView; 3],
        detail: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globe bind"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&maps[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&maps[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&maps[2]),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(detail),
                },
            ],
        })
    }

    /// Sets the map, and the radius (m) of the planet it is of.
    pub fn set_map(&mut self, ctx: &GpuContext, map: &MapLayers, radius_m: f32) {
        self.maps = upload_layers(ctx, map);
        self.rebind(ctx);
        self.has_map = true;
        self.radius_m = radius_m;
        self.width = map.relief_width;
    }

    /// Sets the finer relief drawn where the globe is seen near, or none.
    pub fn set_detail(&mut self, ctx: &GpuContext, detail: Option<DetailLayer>) {
        match detail {
            Some(d) => {
                self.detail = upload_heights(ctx, "globe detail", d.size, d.size, d.elevation);
                self.window = Some((d.window, d.size));
            }
            None => self.window = None,
        }
        self.rebind(ctx);
    }

    fn rebind(&mut self, ctx: &GpuContext) {
        self.bind = Self::bind_group(
            ctx,
            &self.layout,
            &self.params,
            &self.maps,
            &self.detail,
            &self.sampler,
        );
    }

    /// Whether a map has been set.
    pub fn has_map(&self) -> bool {
        self.has_map
    }

    /// Draws the globe over the whole target, with the camera's place (latitude, longitude)
    /// and the point under the cursor marked.
    pub fn render(
        &self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        view: &GlobeView,
        camera: Option<(f32, f32)>,
        cursor: Option<(f32, f32)>,
    ) {
        let [e, n, o] = view.basis();
        let mark = |p: Option<(f32, f32)>| match p {
            Some((lat, lon)) => sphere_point(lat, lon).extend(1.0).to_array(),
            None => [0.0; 4],
        };
        let params = GlobeParams {
            right: e.extend(view.radius_px(size)).to_array(),
            up: n.extend(size.0 as f32).to_array(),
            out: o.extend(size.1 as f32).to_array(),
            camera: mark(camera),
            cursor: mark(cursor),
            look: [
                view.relief,
                match view.mode {
                    GlobeMode::Biomes => 0.0,
                    GlobeMode::Climate => 1.0,
                    GlobeMode::Relief => 2.0,
                },
                self.radius_m,
                self.width as f32,
            ],
            detail: self.window.map_or([0.0; 4], |(w, _)| w),
            detail_size: [self.window.map_or(1.0, |(_, n)| n as f32), 0.0, 0.0, 0.0],
        };
        ctx.write_buffer(&self.params, 0, bytemuck::bytes_of(&params));
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("globe"),
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
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.draw(0..3, 0..1);
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

/// Uploads the map's three layers with their mipmaps (box-filtered).
fn upload_layers(ctx: &GpuContext, map: &MapLayers) -> [wgpu::TextureView; 3] {
    let (w, h) = (map.width as usize, map.height as usize);
    assert_eq!(map.color.len(), w * h, "map size");
    assert_eq!(map.facts.len(), w * h, "map size");
    let rw = map.relief_width;
    assert_eq!(map.elevation.len(), (rw * (rw / 2)) as usize, "relief size");
    [
        upload(
            ctx,
            "globe map",
            wgpu::TextureFormat::Rgba8UnormSrgb,
            map.width,
            map.height,
            map.color.to_vec(),
            |t| bytemuck::cast_slice(t).to_vec(),
        ),
        upload(
            ctx,
            "globe facts",
            wgpu::TextureFormat::Rgba8Unorm,
            map.width,
            map.height,
            map.facts.to_vec(),
            |t| bytemuck::cast_slice(t).to_vec(),
        ),
        upload_heights(ctx, "globe heights", rw, rw / 2, map.elevation),
    ]
}

/// Heights (half floats) as a filtered texture with its mipmaps, averaged as heights (in single
/// floats), not as their halves' bits.
fn upload_heights(
    ctx: &GpuContext,
    label: &str,
    width: u32,
    height: u32,
    elevation: &[u16],
) -> wgpu::TextureView {
    let heights: Vec<[f32; 1]> = elevation.iter().map(|&b| [f16_to_f32(b)]).collect();
    upload(
        ctx,
        label,
        wgpu::TextureFormat::R16Float,
        width,
        height,
        heights,
        |t| {
            t.iter()
                .flat_map(|v| crate::skin_lut::to_f16(v[0]).to_le_bytes())
                .collect()
        },
    )
}

/// A half float's value.
fn f16_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = ((h >> 10) & 0x1f) as i32;
    let m = (h & 0x3ff) as f32;
    sign * match e {
        0 => m * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    }
}

/// A texel that averages: four of them into one.
trait Mean: Copy {
    fn mean(q: [Self; 4]) -> Self;
}

impl Mean for [u8; 4] {
    fn mean(q: [Self; 4]) -> Self {
        std::array::from_fn(|k| ((q.iter().map(|t| t[k] as u32).sum::<u32>() + 2) / 4) as u8)
    }
}

impl Mean for [f32; 1] {
    fn mean(q: [Self; 4]) -> Self {
        [q.iter().map(|t| t[0]).sum::<f32>() * 0.25]
    }
}

/// Uploads one layer with its mipmaps, each level's texels the mean of four of the level above
/// (`bytes` turning a level into the texture's format).
fn upload<T: Mean>(
    ctx: &GpuContext,
    label: &str,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    texels: Vec<T>,
    bytes: impl Fn(&[T]) -> Vec<u8>,
) -> wgpu::TextureView {
    let levels = width.min(height).max(1).ilog2() + 1;
    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let texel_bytes = format.block_copy_size(None).unwrap_or(4);
    let mut level = texels;
    let (mut w, mut h) = (width as usize, height as usize);
    for mip in 0..levels {
        ctx.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: mip,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes(&level),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w as u32 * texel_bytes),
                rows_per_image: Some(h as u32),
            },
            wgpu::Extent3d {
                width: w as u32,
                height: h as u32,
                depth_or_array_layers: 1,
            },
        );
        if mip + 1 < levels {
            level = half_map(&level, w, h);
            (w, h) = ((w / 2).max(1), (h / 2).max(1));
        }
    }
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// The next mip level of a map: each texel the mean of four.
fn half_map<T: Mean>(texels: &[T], w: usize, h: usize) -> Vec<T> {
    let (hw, hh) = ((w / 2).max(1), (h / 2).max(1));
    let mut out = Vec::with_capacity(hw * hh);
    for y in 0..hh {
        for x in 0..hw {
            let at = |dx: usize, dy: usize| {
                texels[(2 * y + dy).min(h - 1) * w + (2 * x + dx).min(w - 1)]
            };
            out.push(T::mean([at(0, 0), at(1, 0), at(0, 1), at(1, 1)]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: (u32, u32) = (1600, 900);

    #[test]
    fn the_centre_shows_the_view_point_and_east_is_right() {
        let v = GlobeView {
            lat: 0.6,
            lon: -2.0,
            zoom: 1.5,
            ..GlobeView::default()
        };
        let c = Vec2::new(800.0, 450.0);
        let (lat, lon) = v.pick(c, SIZE).expect("centre");
        assert!((lat - 0.6).abs() < 1e-5 && (lon + 2.0).abs() < 1e-5);
        let (_, east) = v.pick(c + Vec2::new(20.0, 0.0), SIZE).expect("east");
        let (north, _) = v.pick(c - Vec2::new(0.0, 20.0), SIZE).expect("north");
        assert!(east > lon && north > lat);
        assert!(
            v.pick(Vec2::new(5.0, 5.0), SIZE).is_none(),
            "outside the globe"
        );
    }

    #[test]
    fn picking_inverts_projection() {
        let v = GlobeView {
            lat: -0.3,
            lon: 3.0,
            zoom: 2.0,
            ..GlobeView::default()
        };
        for &(lat, lon) in &[(-0.3f32, 3.0f32), (0.1, -3.1), (-0.9, 2.5), (0.4, 2.9)] {
            let px = v.project(lat, lon, SIZE).expect("near side");
            let (a, o) = v.pick(px, SIZE).expect("on the globe");
            let d = sphere_point(a, o).dot(sphere_point(lat, lon));
            assert!(d > 1.0 - 1e-6, "({lat}, {lon}) came back as ({a}, {o})");
        }
        assert!(v.project(0.3, 0.0, SIZE).is_none(), "far side");
    }

    #[test]
    fn dragging_moves_the_land_with_the_cursor() {
        let mut v = GlobeView {
            lat: 0.5,
            lon: 1.0,
            zoom: 1.0,
            ..GlobeView::default()
        };
        let start = Vec2::new(800.0, 450.0);
        let under = v.pick(start, SIZE).expect("on the globe");
        v.drag(Vec2::new(-30.0, 25.0), SIZE);
        let after = v
            .project(under.0, under.1, SIZE)
            .expect("still on the near side");
        assert!(
            after.distance(start + Vec2::new(-30.0, 25.0)) < 3.0,
            "{after}"
        );
    }
}

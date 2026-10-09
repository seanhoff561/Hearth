//! GPU terrain: all cube meshes live in a few large storage buffers (sub-allocated), drawn with
//! vertex pulling. Each frame the CPU finds candidate cubes (cave culling + frustum); then either
//! the GPU culls them further (two-phase Hi-Z occlusion, see `cull.rs`) and writes the indirect
//! draws itself, or (on adapters without indirect-count draws) the CPU builds the draw lists.
//! The cubes within each of the sun's near shadow cascades are drawn into it first, depth only
//! (`terrain/casters.rs`, `shadow.rs`).

use std::collections::VecDeque;

use bytemuck::{Pod, Zeroable};
use glam::{DVec3, Vec3};
use hearth_math::{BlockPos, CUBE_SIZE, CubePos, Direction, Planet};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::atlas::TextureArray;
use crate::camera::{Camera, Frustum};
use crate::cull::{GpuCuller, SlotRecord};
use crate::gpu::GpuContext;
use crate::mesh::{CubeMesh, GeneralQuad, PackedQuad};
use crate::profiler::GpuTimer;
use crate::shadow::{CASCADES, NEAR_CASCADES, ShadowMaps, ShadowQuality};

pub(crate) use crate::arena::Arena;
pub use crate::arena::RangeAllocator;
pub use casters::shadow_depth_state;
use casters::{ShadowPipelines, make_shadow_pipelines};

mod casters;

/// Translucent quads of cubes within this many cubes of the camera are re-sorted back to front
/// whenever the camera enters another block.
const RESORT_RADIUS: i32 = 2;

/// Quads per draw call are limited by the shared index buffer.
const MAX_QUADS_PER_DRAW: u32 = 16_384;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Where a cube's mesh lives on the GPU.
#[derive(Debug, Clone)]
struct GpuMesh {
    packed_off: u32,
    packed_len: u32,
    counts: [[u32; 6]; 2],
    general_off: u32,
    general_len: u32,
    model_counts: [u32; 2],
    trans_off: u32,
    trans_len: u32,
    visibility: u64,
    /// Culling slot (index into the GPU slot table).
    slot: u32,
    /// CPU copy of the translucent quads for re-sorting, and the camera block they were
    /// last sorted for.
    trans: Vec<GeneralQuad>,
    sorted_for: Option<BlockPos>,
    /// The box the translucent quads span (blocks from the cube's origin): the part of the
    /// screen the water reads is copied from it.
    trans_box: [Vec3; 2],
    /// The smooth ground: vertices, and indices in pairs (words of two `u16`s), and how many.
    smooth_v_off: u32,
    smooth_v_len: u32,
    smooth_i_off: u32,
    smooth_i_words: u32,
    smooth_indices: u32,
    /// When the cube first appeared (kept as its mesh is replaced), for its fade-in.
    born: std::time::Instant,
}

/// The box (blocks from the cube's origin) a set of general quads spans.
fn quad_box(quads: &[GeneralQuad]) -> [Vec3; 2] {
    let coord = |v: u32| (v as u16 as i16) as f32 / 256.0;
    let mut b = [Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)];
    for c in quads.iter().flat_map(|q| &q.corners) {
        let p = Vec3::new(coord(c[0]), coord(c[0] >> 16), coord(c[1]));
        b = [b[0].min(p), b[1].max(p)];
    }
    b
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct DrawArgs {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
}

/// A cube drawn this frame: its origin (camera-relative) and how far it has faded in (w, 0..1).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Instance {
    origin: [f32; 4],
}

/// A ground material as the shader draws it (Amendment S §4): two linear colours mixed by
/// noise at its grain, its roughness and how it is tinted.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct GroundMaterial {
    /// Linear albedo, and roughness.
    pub color: [f32; 4],
    /// The second colour, and the grain's size (m).
    pub color2: [f32; 4],
    /// 0: as coloured; 1: grass, tinted by place and season.
    pub tint: u32,
    /// How much its relief stands up in blending (0 flat … 1 coarse stones).
    pub relief: f32,
    /// The thickness of its beds (m), for bedded rock; 0 none.
    pub strata: f32,
    pub _pad: f32,
}

impl Default for GroundMaterial {
    fn default() -> Self {
        Self {
            color: [0.18, 0.16, 0.14, 0.9],
            color2: [0.12, 0.10, 0.09, 0.5],
            tint: 0,
            relief: 0.3,
            strata: 0.0,
            _pad: 0.0,
        }
    }
}

/// Per-frame lighting, fog and time. Light values are pre-exposed illuminances (lux ×
/// exposure), so the HDR target stays in a comfortable range day and night.
#[derive(Debug, Clone, Copy)]
pub struct FrameParams {
    /// Direct light of the dominant body (sun or moon) on a surface facing it.
    pub direct_light: Vec3,
    /// Direction toward the dominant body.
    pub light_dir: Vec3,
    /// Sky irradiance on an upward surface.
    pub sky_light: Vec3,
    /// Light that reaches everything (starlight, airglow).
    pub ambient_floor: f32,
    /// Firelight at block-light level 15.
    pub block_light: Vec3,
    /// Aerosol extinction of the air at sea level (per metre): grows with haze and humidity.
    pub haze_extinction: f32,
    /// Grey extinction of falling rain or snow (per metre).
    pub precip_extinction: f32,
    /// Blocks per real metre of height (the world's vertical scale).
    pub vertical_scale: f32,
    /// Aerial perspective on (off only for tests and comparisons).
    pub aerial_perspective: bool,
    /// Full-detail terrain area in world blocks (min x, min z, max x, max z); the LOD draws
    /// beyond it. `None`: full-detail terrain everywhere.
    pub near_area: Option<[f64; 4]>,
    pub seconds: f32,
    pub anim_ticks: f32,
    pub wind: f32,
    /// Year fraction (0 = March equinox) for seasonal colours.
    pub year_frac: f32,
    /// Grey of an overcast sky (rgb) and how far haze and fog take it instead of the clear
    /// sky's colour (w, 0..1).
    pub overcast: glam::Vec4,
    /// How wet the ground's surface is (0..1).
    pub wetness: f32,
}

impl Default for FrameParams {
    fn default() -> Self {
        Self {
            direct_light: Vec3::splat(2.0),
            light_dir: Vec3::new(0.3, 0.9, 0.2).normalize(),
            sky_light: Vec3::new(0.35, 0.45, 0.6),
            ambient_floor: 0.0,
            block_light: Vec3::new(1.0, 0.82, 0.6),
            haze_extinction: 4.44e-5,
            precip_extinction: 0.0,
            vertical_scale: 1.0,
            aerial_perspective: true,
            near_area: None,
            seconds: 0.0,
            anim_ticks: 0.0,
            wind: 1.0,
            year_frac: 0.3,
            overcast: glam::Vec4::ZERO,
            wetness: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    sun_light: [f32; 4],
    sky_light: [f32; 4],
    block_light: [f32; 4],
    fog: [f32; 4],
    params: [f32; 4],
    sun: [f32; 4],
    camera: [f32; 4],
    overcast: [f32; 4],
    near: [f32; 4],
    water_map: [f32; 4],
    /// The sun's shadow maps (`shadow.rs`): each cascade's view-projection from points relative
    /// to the camera now, into the map as it was drawn.
    shadow_vp: [[[f32; 4]; 4]; CASCADES],
    /// x: 1 when the maps hold the sun's shadows this frame; y: their size (texels); z: how far
    /// toward the sun the far cascade takes its casters; w: 1 while a cascade is drawn.
    shadow: [f32; 4],
    /// Each cascade's texel (blocks): the near ones, then the far one in the second's x.
    shadow_texel: [[f32; 4]; 2],
    /// The direction toward the light the maps are drawn for.
    shadow_light: [f32; 4],
    /// How far toward the sun each near cascade takes its casters (blocks).
    shadow_reach: [f32; 4],
}

/// Earth's mean radius (m): with the vertical scale, the radius of the planet's curvature.
pub const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// Statistics for the debug overlay.
#[derive(Debug, Clone, Copy, Default)]
pub struct TerrainStats {
    pub meshes: usize,
    /// Cubes that passed cave culling and the frustum test.
    pub visible_cubes: usize,
    /// CPU-built draws (all passes with CPU culling; translucent only with GPU culling).
    pub draws: usize,
    /// Quads submitted by the CPU path, or the upper bound before GPU occlusion culling.
    pub quads_drawn: u64,
    pub packed_bytes: u64,
    pub general_bytes: u64,
    /// The smooth ground's vertices and indices (bytes).
    pub smooth_bytes: u64,
    pub gpu_culling: bool,
    pub resorted: usize,
    /// Translucent quads drawn (always from CPU draw lists).
    pub translucent_quads: u64,
    /// The part of the screen (0–1, y down: min x, min y, max x, max y) the cubes with
    /// translucent quads cover, for copying no more of the scene than the water reads.
    pub translucent_rect: Option<[f32; 4]>,
    /// Draws and triangles of the near terrain cast into the shadow maps this frame.
    pub shadow_draws: usize,
    pub shadow_triangles: u64,
}

struct Pass {
    draws: Vec<DrawArgs>,
    buffer: wgpu::Buffer,
    capacity: usize,
}

impl Pass {
    fn new(device: &wgpu::Device, label: &str) -> Self {
        let capacity = 4096;
        Self {
            draws: Vec::with_capacity(capacity),
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: (capacity * std::mem::size_of::<DrawArgs>()) as u64,
                usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity,
        }
    }

    fn upload(&mut self, ctx: &GpuContext, label: &str) {
        if self.draws.len() > self.capacity {
            self.capacity = self.draws.len().next_power_of_two();
            self.buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: (self.capacity * std::mem::size_of::<DrawArgs>()) as u64,
                usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !self.draws.is_empty() {
            ctx.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.draws));
        }
    }
}

/// The terrain renderer.
pub struct TerrainRenderer {
    globals: wgpu::Buffer,
    layout0: wgpu::BindGroupLayout,
    bind0: wgpu::BindGroup,
    layout1: wgpu::BindGroupLayout,
    bind1: wgpu::BindGroup,
    packed: Arena,
    general: Arena,
    /// The smooth ground's vertices and its indices (Amendment S).
    smooth_v: Arena,
    smooth_i: Arena,
    /// The ground's materials' parameters (`GroundMaterial`, one per slot).
    ground: wgpu::Buffer,
    instances: wgpu::Buffer,
    instance_capacity: usize,
    instance_data: Vec<Instance>,
    index_buffer: wgpu::Buffer,
    pipes: Pipelines,
    passes: [Pass; 6],
    meshes: FxHashMap<CubePos, GpuMesh>,
    planet: Planet,
    pub stats: TerrainStats,
    bind_dirty: bool,
    multi_draw: bool,
    culler: Option<GpuCuller>,
    free_slots: Vec<u32>,
    next_slot: u32,
    /// Slot of each candidate this frame (candidate index = instance index).
    cand_slots: Vec<u32>,
    /// Scratch for translucent re-sorting.
    sort_keys: Vec<(f32, u32)>,
    sort_scratch: Vec<GeneralQuad>,
    /// Scratch for the visibility search, kept so frames allocate nothing.
    visible: Vec<(CubePos, Vec3)>,
    bfs_seen: FxHashSet<CubePos>,
    bfs_queue: VecDeque<(CubePos, Option<Direction>, u8)>,
    /// Horizontal render distance in cubes (for the visibility search).
    pub render_distance: i32,
    pub vertical_distance: i32,
    /// Use the cave-culling visibility graph.
    pub cave_culling: bool,
    /// Use GPU occlusion culling when the adapter supports it.
    pub gpu_culling: bool,
    /// How long a cube takes to fade in when it first appears (s; E4.1 §4.4), its pixels
    /// dithered in as the near ground's hand-off to the distant one is; 0, at once (pictures and
    /// tests).
    pub fade_in_s: f32,
    /// What the water surfaces read (the scene behind them, waves, wind).
    pub water: crate::water::WaterRenderer,
    /// The sun's shadows (R1a): the maps, and each cascade's globals and bind group 0 (the
    /// frame's with the light's view-projection; the maps themselves left out while drawn into).
    pub shadows: ShadowMaps,
    shadow_globals: Vec<wgpu::Buffer>,
    shadow_binds: Vec<wgpu::BindGroup>,
    /// The near cascades' casters: packed opaque and cutout, general opaque and cutout, the
    /// smooth ground.
    shadow_passes: Vec<[Pass; 5]>,
    shadow_pipes: ShadowPipelines,
    /// What bind group 0 is made of besides the globals and the shadow maps.
    bind0_parts: Bind0Parts,
    /// Each cube's instance this frame by its slot (`u32::MAX`: none yet; scratch).
    slot_instance: Vec<u32>,
    /// Redraw every cascade each frame (pictures: nothing kept from another frame).
    pub redraw_shadows: bool,
}

struct Pipelines {
    smooth: wgpu::RenderPipeline,
    packed_opaque: wgpu::RenderPipeline,
    packed_cutout: wgpu::RenderPipeline,
    general_opaque: wgpu::RenderPipeline,
    general_cutout: wgpu::RenderPipeline,
    translucent: wgpu::RenderPipeline,
}

/// The textures and samplers of bind group 0 besides the globals and the shadow maps.
struct Bind0Parts {
    blocks: wgpu::TextureView,
    sampler: wgpu::Sampler,
    skyview: wgpu::TextureView,
    sky_sampler: wgpu::Sampler,
}

/// Bind group 0: the globals, block textures, sky-view table, water and the shadow maps.
fn make_bind0(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    globals: &wgpu::Buffer,
    parts: &Bind0Parts,
    water: &crate::water::WaterRenderer,
    shadows: &ShadowMaps,
    shadow: &wgpu::TextureView,
) -> wgpu::BindGroup {
    let view = wgpu::BindingResource::TextureView;
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("terrain bind 0"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: view(&parts.blocks),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&parts.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: view(&parts.skyview),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(&parts.sky_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: view(water.heights_view()),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: view(water.caustics_view()),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: view(shadow),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: wgpu::BindingResource::Sampler(&shadows.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 9,
                resource: wgpu::BindingResource::Sampler(&shadows.depth_sampler),
            },
        ],
    })
}

impl TerrainRenderer {
    /// Terrain drawn into the HDR target; `sky` provides the sky-view table for aerial
    /// perspective.
    pub fn new(
        ctx: &GpuContext,
        atlas: &TextureArray,
        sky: &crate::sky::SkyRenderer,
        planet: Planet,
        mip_levels: u32,
        anisotropy: u16,
    ) -> Self {
        let color_format = crate::post::HDR_FORMAT;
        let device = &ctx.device;
        let texture = upload_texture_array(ctx, atlas, mip_levels);
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("block textures"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let aniso = anisotropy.clamp(1, 16);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("block sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: mip_levels as f32,
            compare: None,
            anisotropy_clamp: aniso,
            border_color: None,
        });
        let water = crate::water::WaterRenderer::new(ctx);
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrain globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terrain layout 0"),
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
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // The water surfaces around the camera, and the caustics.
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // The sun's shadow maps and their comparison sampler.
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });
        let storage = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terrain layout 1"),
            entries: &[
                storage(0),
                storage(1),
                storage(2),
                storage(3),
                wgpu::BindGroupLayoutEntry {
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ..storage(4)
                },
            ],
        });
        let bind0_parts = Bind0Parts {
            blocks: view,
            sampler,
            skyview: sky.skyview_view.clone(),
            sky_sampler: sky.sampler().clone(),
        };
        let shadows = ShadowMaps::new(ctx, ShadowQuality::Off);
        let bind0 = make_bind0(
            device,
            &layout0,
            &globals,
            &bind0_parts,
            &water,
            &shadows,
            &shadows.array_view,
        );
        let shadow_globals: Vec<wgpu::Buffer> = (0..CASCADES)
            .map(|_| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("shadow cascade globals"),
                    size: std::mem::size_of::<Globals>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        let shadow_binds = shadow_globals
            .iter()
            .map(|b| {
                make_bind0(
                    device,
                    &layout0,
                    b,
                    &bind0_parts,
                    &water,
                    &shadows,
                    &shadows.dummy_view,
                )
            })
            .collect();
        let shadow_passes = (0..NEAR_CASCADES)
            .map(|_| {
                [
                    Pass::new(device, "shadow packed"),
                    Pass::new(device, "shadow packed cutout"),
                    Pass::new(device, "shadow general"),
                    Pass::new(device, "shadow general cutout"),
                    Pass::new(device, "shadow smooth ground"),
                ]
            })
            .collect();
        let packed = Arena::new(device, "packed quads", 16, 1 << 20);
        let general = Arena::new(device, "general quads", 64, 1 << 17);
        let smooth_v = Arena::new(device, "smooth ground vertices", 24, 1 << 19);
        let smooth_i = Arena::with_usage(
            device,
            "smooth ground indices",
            4,
            1 << 20,
            wgpu::BufferUsages::INDEX,
        );
        let ground = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ground materials"),
            size: (256 * std::mem::size_of::<GroundMaterial>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.write_buffer(
            &ground,
            0,
            bytemuck::cast_slice(&[GroundMaterial::default(); 256]),
        );
        let instance_capacity = 16_384;
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrain instances"),
            size: (instance_capacity * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind1 = make_bind1(
            device,
            &layout1,
            [
                &packed.buffer,
                &general.buffer,
                &instances,
                &smooth_v.buffer,
                &ground,
            ],
        );
        // Shared quad index buffer: 0,1,2, 2,3,0 per quad.
        let mut indices = Vec::with_capacity(MAX_QUADS_PER_DRAW as usize * 6);
        for q in 0..MAX_QUADS_PER_DRAW {
            let b = q * 4;
            indices.extend_from_slice(&[b, b + 1, b + 2, b + 2, b + 3, b]);
        }
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("quad indices"),
            size: (indices.len() * 4) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.write_buffer(&index_buffer, 0, bytemuck::cast_slice(&indices));
        let module = terrain_module(device);
        // The pipelines made side by side: a software device compiles each on one core.
        let (pipes, shadow_pipes) = std::thread::scope(|scope| {
            let shadow = scope.spawn(|| make_shadow_pipelines(device, &module, &layout0, &layout1));
            let pipes = make_pipelines(
                device,
                &module,
                &layout0,
                &layout1,
                water.layout(),
                color_format,
            );
            (pipes, shadow.join().expect("the shadow pipelines"))
        });
        let passes = [
            Pass::new(device, "draws packed opaque"),
            Pass::new(device, "draws packed cutout"),
            Pass::new(device, "draws general opaque"),
            Pass::new(device, "draws general cutout"),
            Pass::new(device, "draws translucent"),
            Pass::new(device, "draws smooth ground"),
        ];
        Self {
            globals,
            layout0,
            bind0,
            layout1,
            bind1,
            packed,
            general,
            smooth_v,
            smooth_i,
            ground,
            instances,
            instance_capacity,
            instance_data: Vec::with_capacity(instance_capacity),
            index_buffer,
            pipes,
            passes,
            meshes: FxHashMap::default(),
            planet,
            stats: TerrainStats::default(),
            bind_dirty: false,
            multi_draw: ctx.caps.indirect_first_instance,
            // DX12 indirect-count draws don't add the base vertex / first instance to the
            // vertex and instance builtins (wgpu only patches those for plain multi-draws), and
            // Metal has no indirect-count draws, so GPU culling is Vulkan-only for now.
            culler: (ctx.caps.multi_draw_indirect_count
                && ctx.caps.indirect_first_instance
                && ctx.info.backend == wgpu::Backend::Vulkan)
                .then(|| GpuCuller::new(ctx)),
            free_slots: Vec::new(),
            next_slot: 0,
            cand_slots: Vec::new(),
            sort_keys: Vec::new(),
            sort_scratch: Vec::new(),
            visible: Vec::new(),
            bfs_seen: FxHashSet::default(),
            bfs_queue: VecDeque::new(),
            render_distance: 12,
            vertical_distance: 8,
            cave_culling: true,
            gpu_culling: true,
            fade_in_s: 0.0,
            water,
            shadows,
            shadow_globals,
            shadow_binds,
            shadow_passes,
            shadow_pipes,
            bind0_parts,
            slot_instance: Vec::new(),
            redraw_shadows: false,
        }
    }

    /// The per-frame globals, block textures and sky-view table (bind group 0), shared with
    /// the LOD renderer so both light and fade the same way.
    pub fn globals_bind(&self) -> (&wgpu::BindGroupLayout, &wgpu::BindGroup) {
        (&self.layout0, &self.bind0)
    }

    /// Sets the planet places wrap on, before any mesh is uploaded (those held are kept by their
    /// place on it).
    pub fn set_planet(&mut self, planet: Planet) {
        debug_assert!(self.meshes.is_empty());
        self.planet = planet;
    }

    /// True when this frame's opaque geometry is culled and drawn by the GPU.
    pub fn uses_gpu_culling(&self) -> bool {
        self.gpu_culling && self.culler.is_some()
    }

    /// Uploads (or replaces) a cube's mesh.
    pub fn upload(&mut self, ctx: &GpuContext, mesh: &CubeMesh) {
        let pos = self.planet.wrap_cube(mesh.pos);
        // A cube remeshed (its light, a change) stays as faded in as it was.
        let born = self
            .meshes
            .get(&pos)
            .map_or_else(std::time::Instant::now, |g| g.born);
        self.remove(mesh.pos);
        let slot = self.free_slots.pop().unwrap_or_else(|| {
            self.next_slot += 1;
            self.next_slot - 1
        });
        let mut g = GpuMesh {
            packed_off: 0,
            packed_len: mesh.quads.len() as u32,
            counts: mesh.quad_counts,
            general_off: 0,
            general_len: mesh.models.len() as u32,
            model_counts: mesh.model_counts,
            trans_off: 0,
            trans_len: mesh.translucent.len() as u32,
            visibility: mesh.visibility,
            slot,
            trans: mesh.translucent.clone(),
            sorted_for: None,
            trans_box: quad_box(&mesh.translucent),
            smooth_v_off: 0,
            smooth_v_len: 0,
            smooth_i_off: 0,
            smooth_i_words: 0,
            smooth_indices: 0,
            born,
        };
        if !mesh.smooth.is_empty() {
            let sm = &mesh.smooth;
            let words = sm.indices.len().div_ceil(2) as u32;
            let (voff, grew_v) = self.smooth_v.alloc(ctx, sm.vertices.len() as u32);
            let (ioff, grew_i) = if voff == u32::MAX {
                (u32::MAX, false)
            } else {
                self.smooth_i.alloc(ctx, words)
            };
            if voff != u32::MAX && ioff == u32::MAX {
                self.smooth_v.alloc.free(voff, sm.vertices.len() as u32);
            }
            if voff != u32::MAX && ioff != u32::MAX {
                self.bind_dirty |= grew_v || grew_i;
                g.smooth_v_off = voff;
                g.smooth_v_len = sm.vertices.len() as u32;
                g.smooth_i_off = ioff;
                g.smooth_i_words = words;
                g.smooth_indices = sm.indices.len() as u32;
                ctx.write_buffer(
                    &self.smooth_v.buffer,
                    voff as u64 * 24,
                    bytemuck::cast_slice(&sm.vertices),
                );
                let mut idx = sm.indices.clone();
                if idx.len() % 2 == 1 {
                    idx.push(0);
                }
                ctx.write_buffer(
                    &self.smooth_i.buffer,
                    ioff as u64 * 4,
                    bytemuck::cast_slice(&idx),
                );
            }
        }
        if g.packed_len > 0 {
            let (off, grew) = self.packed.alloc(ctx, g.packed_len);
            if off == u32::MAX {
                self.free_slots.push(slot);
                return;
            }
            self.bind_dirty |= grew;
            g.packed_off = off;
            ctx.write_buffer(
                &self.packed.buffer,
                off as u64 * 16,
                bytemuck::cast_slice::<PackedQuad, u8>(&mesh.quads),
            );
        }
        let glen = g.general_len + g.trans_len;
        if glen > 0 {
            let (off, grew) = self.general.alloc(ctx, glen);
            if off == u32::MAX {
                self.packed.alloc.free(g.packed_off, g.packed_len);
                self.free_slots.push(slot);
                return;
            }
            self.bind_dirty |= grew;
            g.general_off = off;
            g.trans_off = off + g.general_len;
            if g.general_len > 0 {
                ctx.write_buffer(
                    &self.general.buffer,
                    off as u64 * 64,
                    bytemuck::cast_slice::<GeneralQuad, u8>(&mesh.models),
                );
            }
            if g.trans_len > 0 {
                ctx.write_buffer(
                    &self.general.buffer,
                    g.trans_off as u64 * 64,
                    bytemuck::cast_slice::<GeneralQuad, u8>(&mesh.translucent),
                );
            }
        }
        if let Some(culler) = &mut self.culler {
            let mut counts = [0u32; 12];
            for (li, dirs) in g.counts.iter().enumerate() {
                counts[li * 6..li * 6 + 6].copy_from_slice(dirs);
            }
            culler.write_slot(
                ctx,
                slot,
                &SlotRecord {
                    packed_off: g.packed_off,
                    general_off: g.general_off,
                    model_opaque: g.model_counts[0],
                    model_cutout: g.model_counts[1],
                    counts,
                },
            );
        }
        self.meshes.insert(pos, g);
    }

    /// Removes a cube's mesh and frees its memory.
    pub fn remove(&mut self, pos: CubePos) {
        let pos = self.planet.wrap_cube(pos);
        if let Some(g) = self.meshes.remove(&pos) {
            self.packed.alloc.free(g.packed_off, g.packed_len);
            self.general
                .alloc
                .free(g.general_off, g.general_len + g.trans_len);
            if g.smooth_indices > 0 {
                self.smooth_v.alloc.free(g.smooth_v_off, g.smooth_v_len);
                self.smooth_i.alloc.free(g.smooth_i_off, g.smooth_i_words);
            }
            self.free_slots.push(g.slot);
        }
    }

    pub fn contains(&self, pos: CubePos) -> bool {
        self.meshes.contains_key(&self.planet.wrap_cube(pos))
    }

    /// Video memory the near terrain's meshes take (bytes, their buffers' whole size).
    pub fn bytes(&self) -> u64 {
        self.packed.bytes() + self.general.bytes() + self.smooth_v.bytes() + self.smooth_i.bytes()
    }

    /// Visible cubes via cave culling (BFS through connected faces) and frustum tests, into
    /// `out` (the search's sets are passed in so they keep their memory between frames).
    fn visible_cubes(
        &self,
        camera: &Camera,
        frustum: &Frustum,
        out: &mut Vec<(CubePos, Vec3)>,
        seen: &mut FxHashSet<CubePos>,
        queue: &mut VecDeque<(CubePos, Option<Direction>, u8)>,
    ) {
        let start = self.planet.wrap_cube(CubePos::containing(camera.pos));
        let rel = |c: CubePos| -> Vec3 {
            let min = c.min_block().as_dvec3();
            let d = self.planet.delta(camera.pos, min);
            d.as_vec3()
        };
        let in_range = |c: CubePos| -> bool {
            let d = self.planet.cube_delta(start, c);
            d.x.abs() <= self.render_distance
                && d.z.abs() <= self.render_distance
                && d.y.abs() <= self.vertical_distance
        };
        out.clear();
        seen.clear();
        queue.clear();
        if !self.cave_culling || !self.meshes.contains_key(&start) {
            for &pos in self.meshes.keys() {
                if !in_range(pos) {
                    continue;
                }
                let o = rel(pos);
                if frustum.intersects_aabb(o, o + Vec3::splat(CUBE_SIZE as f32)) {
                    out.push((pos, o));
                }
            }
            return;
        }
        // BFS from the camera's cube, entering each cube through a face and leaving through
        // faces connected to it; never stepping back toward the camera.
        seen.insert(start);
        queue.push_back((start, None, 0));
        while let Some((pos, entered_from, dirs_taken)) = queue.pop_front() {
            let o = rel(pos);
            if !frustum.intersects_aabb(o, o + Vec3::splat(CUBE_SIZE as f32)) && pos != start {
                continue;
            }
            let Some(mesh) = self.meshes.get(&pos) else {
                continue;
            };
            out.push((pos, o));
            for d in Direction::ALL {
                // Don't travel back against a direction already taken.
                if dirs_taken & (1 << d.opposite().index()) != 0 {
                    continue;
                }
                if let Some(e) = entered_from {
                    let bit = 1u64 << (e.index() * 6 + d.index());
                    if mesh.visibility & bit == 0 {
                        continue;
                    }
                }
                let n = self.planet.cube_neighbor(pos, d);
                if !in_range(n) || !seen.insert(n) {
                    continue;
                }
                queue.push_back((n, Some(d.opposite()), dirs_taken | (1 << d.index())));
            }
        }
    }

    /// Finds this frame's candidate cubes, builds the CPU draw lists (translucent always; opaque
    /// when not GPU-culled), re-sorts nearby translucent geometry, and uploads per-frame data.
    /// `size` is the render target size in pixels.
    pub fn prepare(
        &mut self,
        ctx: &GpuContext,
        camera: &Camera,
        size: (u32, u32),
        params: &FrameParams,
    ) {
        let aspect = size.0.max(1) as f32 / size.1.max(1) as f32;
        let vp = camera.view_proj(aspect);
        let frustum = Frustum::from_view_proj(vp);
        let mut visible = std::mem::take(&mut self.visible);
        let (mut seen, mut queue) = (
            std::mem::take(&mut self.bfs_seen),
            std::mem::take(&mut self.bfs_queue),
        );
        self.visible_cubes(camera, &frustum, &mut visible, &mut seen, &mut queue);
        (self.bfs_seen, self.bfs_queue) = (seen, queue);
        // Front to back for opaque (early-z); translucent goes back to front below. The search
        // order is deterministic, so an unstable sort (no scratch memory) is too.
        visible.sort_unstable_by(|a, b| {
            let da = (a.1 + Vec3::splat(8.0)).length_squared();
            let db = (b.1 + Vec3::splat(8.0)).length_squared();
            da.total_cmp(&db)
        });
        let gpu = self.uses_gpu_culling();
        let resorted = self.resort_translucent(ctx, camera, &visible);
        self.instance_data.clear();
        self.cand_slots.clear();
        for p in &mut self.passes {
            p.draws.clear();
        }
        let mut quads_drawn = 0u64;
        let mut translucent_quads = 0u64;
        let mut translucent_rect: Option<[f32; 4]> = None;
        for (inst, (pos, o)) in visible.iter().enumerate() {
            let m = &self.meshes[pos];
            let inst = inst as u32;
            let faded = if self.fade_in_s > 0.0 {
                (m.born.elapsed().as_secs_f32() / self.fade_in_s).min(1.0)
            } else {
                1.0
            };
            self.instance_data.push(Instance {
                origin: [o.x, o.y, o.z, faded],
            });
            self.cand_slots.push(m.slot);
            if m.smooth_indices > 0 {
                self.passes[5].draws.push(DrawArgs {
                    index_count: m.smooth_indices,
                    instance_count: 1,
                    first_index: m.smooth_i_off * 2,
                    base_vertex: m.smooth_v_off as i32,
                    first_instance: inst,
                });
            }
            if gpu {
                quads_drawn += (m.packed_len + m.general_len) as u64;
                continue;
            }
            // Per-direction culling: a face group can only be seen from its front side.
            let cam_local = -*o; // camera position relative to the cube min
            let facing = |d: Direction| -> bool {
                match d {
                    Direction::Up => cam_local.y > 0.0,
                    Direction::Down => cam_local.y < CUBE_SIZE as f32,
                    Direction::South => cam_local.z > 0.0,
                    Direction::North => cam_local.z < CUBE_SIZE as f32,
                    Direction::East => cam_local.x > 0.0,
                    Direction::West => cam_local.x < CUBE_SIZE as f32,
                }
            };
            let mut off = m.packed_off;
            for (li, counts) in m.counts.iter().enumerate() {
                for d in Direction::ALL {
                    let n = counts[d.index()];
                    if n > 0 && facing(d) {
                        push_draws(&mut self.passes[li].draws, off, n, inst);
                        quads_drawn += n as u64;
                    }
                    off += n;
                }
            }
            let mut goff = m.general_off;
            for li in 0..2 {
                let n = m.model_counts[li];
                if n > 0 {
                    push_draws(&mut self.passes[2 + li].draws, goff, n, inst);
                    quads_drawn += n as u64;
                }
                goff += n;
            }
        }
        // Translucent: back to front.
        for (inst, (pos, o)) in visible.iter().enumerate().rev() {
            let m = &self.meshes[pos];
            if m.trans_len > 0 {
                if let Some(r) = screen_rect(vp, *o + m.trans_box[0], *o + m.trans_box[1]) {
                    translucent_rect = Some(match translucent_rect {
                        Some(t) => [
                            t[0].min(r[0]),
                            t[1].min(r[1]),
                            t[2].max(r[2]),
                            t[3].max(r[3]),
                        ],
                        None => r,
                    });
                }
                push_draws(
                    &mut self.passes[4].draws,
                    m.trans_off,
                    m.trans_len,
                    inst as u32,
                );
                quads_drawn += m.trans_len as u64;
                translucent_quads += m.trans_len as u64;
            }
        }
        // The sun's shadows: the cascades due this frame and what casts into them.
        self.shadows
            .plan(camera.pos, params.light_dir, self.redraw_shadows);
        self.shadow_casters(camera.pos, &visible);
        // Upload instances.
        if self.instance_data.len() > self.instance_capacity {
            self.instance_capacity = self.instance_data.len().next_power_of_two();
            self.instances = ctx.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("terrain instances"),
                size: (self.instance_capacity * std::mem::size_of::<Instance>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.bind_dirty = true;
            if let Some(c) = &mut self.culler {
                c.invalidate();
            }
        }
        if self.bind_dirty {
            self.bind1 = make_bind1(
                &ctx.device,
                &self.layout1,
                [
                    &self.packed.buffer,
                    &self.general.buffer,
                    &self.instances,
                    &self.smooth_v.buffer,
                    &self.ground,
                ],
            );
            self.bind_dirty = false;
        }
        if !self.instance_data.is_empty() {
            ctx.write_buffer(
                &self.instances,
                0,
                bytemuck::cast_slice(&self.instance_data),
            );
        }
        for (i, p) in self.passes.iter_mut().enumerate() {
            p.upload(
                ctx,
                [
                    "packed opaque",
                    "packed cutout",
                    "general opaque",
                    "general cutout",
                    "translucent",
                    "smooth ground",
                ][i],
            );
        }
        for passes in &mut self.shadow_passes {
            for p in passes.iter_mut() {
                p.upload(ctx, "shadow casters");
            }
        }
        if gpu && let Some(c) = &mut self.culler {
            c.prepare(ctx, &self.cand_slots, vp, camera.near, size);
        }
        let cam = camera.pos;
        let v4 = |v: Vec3, w: f32| [v.x, v.y, v.z, w];
        let v = params.vertical_scale.max(1e-3);
        let near = match params.near_area {
            Some([x0, z0, x1, z1]) => [
                self.planet.delta_x(cam.x, x0) as f32,
                (z0 - cam.z) as f32,
                (self.planet.delta_x(cam.x, x0) + (x1 - x0)) as f32,
                (z1 - cam.z) as f32,
            ],
            // No LOD: the full-detail terrain covers everything.
            None => [-1e9, -1e9, 1e9, 1e9],
        };
        let globals = Globals {
            view_proj: vp.to_cols_array_2d(),
            sun_light: v4(params.direct_light, 0.0),
            sky_light: v4(params.sky_light, params.ambient_floor),
            block_light: v4(params.block_light, params.wetness),
            fog: [
                params.haze_extinction,
                params.precip_extinction,
                1.0 / v,
                (cam.y as f32).max(0.0) / v,
            ],
            params: [
                params.seconds,
                params.anim_ticks,
                (0.5 / (EARTH_RADIUS_M * v as f64)) as f32,
                if params.aerial_perspective { 1.0 } else { 0.0 },
            ],
            sun: v4(params.light_dir, params.wind),
            camera: [
                (cam.x.rem_euclid(4096.0)) as f32,
                (cam.y.rem_euclid(4096.0)) as f32,
                (cam.z.rem_euclid(4096.0)) as f32,
                params.year_frac,
            ],
            overcast: params.overcast.to_array(),
            near,
            // The map of water surfaces: its first column relative to the camera, the
            // camera's height, and whether there is a map.
            water_map: match self.water.map_origin() {
                Some((ox, oz)) => [
                    self.planet.delta_x(cam.x, ox as f64) as f32,
                    (oz as f64 - cam.z) as f32,
                    cam.y as f32,
                    1.0,
                ],
                None => [0.0; 4],
            },
            shadow_vp: self
                .shadows
                .sample_matrices(cam)
                .map(|m| m.to_cols_array_2d()),
            shadow: [
                if self.shadows.active { 1.0 } else { 0.0 },
                self.shadows.size as f32,
                crate::shadow::REACH_UP[NEAR_CASCADES],
                0.0,
            ],
            shadow_texel: [
                std::array::from_fn(|i| self.shadows.texel(i)),
                [self.shadows.texel(NEAR_CASCADES), 0.0, 0.0, 0.0],
            ],
            shadow_light: v4(self.shadows.light().unwrap_or(params.light_dir), 0.0),
            shadow_reach: std::array::from_fn(|i| crate::shadow::REACH_UP[i]),
        };
        ctx.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        // Each cascade drawn this frame: the frame's globals seen from the light.
        for i in self.shadows.due() {
            let cascade = Globals {
                view_proj: self.shadows.cascades[i].view_proj.to_cols_array_2d(),
                shadow: [globals.shadow[0], globals.shadow[1], globals.shadow[2], 1.0],
                ..globals
            };
            ctx.write_buffer(&self.shadow_globals[i], 0, bytemuck::bytes_of(&cascade));
        }
        self.stats = TerrainStats {
            meshes: self.meshes.len(),
            visible_cubes: visible.len(),
            draws: self.passes.iter().map(|p| p.draws.len()).sum(),
            quads_drawn,
            packed_bytes: self.packed.alloc.used() as u64 * 16,
            general_bytes: self.general.alloc.used() as u64 * 64,
            smooth_bytes: self.smooth_v.alloc.used() as u64 * 24
                + self.smooth_i.alloc.used() as u64 * 4,
            gpu_culling: gpu,
            resorted,
            translucent_quads,
            translucent_rect,
            shadow_draws: self
                .shadow_passes
                .iter()
                .flatten()
                .map(|p| p.draws.len())
                .sum(),
            shadow_triangles: self
                .shadow_passes
                .iter()
                .flatten()
                .flat_map(|p| &p.draws)
                .map(|d| d.index_count as u64 / 3)
                .sum(),
        };
        self.visible = visible;
    }

    /// Re-sorts the translucent quads of cubes near the camera back to front when the camera
    /// has moved to another block since their last sort. Returns how many cubes were re-sorted.
    fn resort_translucent(
        &mut self,
        ctx: &GpuContext,
        camera: &Camera,
        visible: &[(CubePos, Vec3)],
    ) -> usize {
        let cam_block = self.planet.wrap_block(BlockPos::containing(camera.pos));
        let cam_cube = cam_block.cube();
        let mut n = 0;
        for (pos, o) in visible {
            let d = self.planet.cube_delta(cam_cube, *pos);
            if d.x.abs() > RESORT_RADIUS || d.y.abs() > RESORT_RADIUS || d.z.abs() > RESORT_RADIUS {
                continue;
            }
            let Some(m) = self.meshes.get_mut(pos) else {
                continue;
            };
            if m.trans.len() < 2 || m.sorted_for == Some(cam_block) {
                continue;
            }
            // Camera in cube-local units (1/256 block, like the quad corners).
            let cam = -*o * 256.0;
            self.sort_keys.clear();
            for (i, q) in m.trans.iter().enumerate() {
                let mut c = Vec3::ZERO;
                for corner in &q.corners {
                    c += Vec3::new(
                        (corner[0] & 0xffff) as i16 as f32,
                        (corner[0] >> 16) as i16 as f32,
                        (corner[1] & 0xffff) as i16 as f32,
                    );
                }
                self.sort_keys
                    .push(((c * 0.25 - cam).length_squared(), i as u32));
            }
            // Farthest first.
            self.sort_keys.sort_unstable_by(|a, b| b.0.total_cmp(&a.0));
            self.sort_scratch.clear();
            self.sort_scratch
                .extend(self.sort_keys.iter().map(|(_, i)| m.trans[*i as usize]));
            std::mem::swap(&mut m.trans, &mut self.sort_scratch);
            ctx.write_buffer(
                &self.general.buffer,
                m.trans_off as u64 * 64,
                bytemuck::cast_slice::<GeneralQuad, u8>(&m.trans),
            );
            m.sorted_for = Some(cam_block);
            n += 1;
        }
        n
    }

    /// Records the opaque and cutout geometry into `enc` (GPU- or CPU-culled). Clears the
    /// targets first when `clear` is given. Translucent geometry is drawn afterwards with
    /// [`Self::draw_translucent`], after the sky. `timer` marks the passes.
    pub fn render_opaque(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        clear: Option<wgpu::Color>,
        mut timer: Option<&mut GpuTimer>,
    ) {
        let mut mark = |enc: &mut wgpu::CommandEncoder, label: &'static str| {
            if let Some(t) = timer.as_deref_mut() {
                t.mark(enc, label);
            }
        };
        if !self.uses_gpu_culling() {
            {
                let mut pass = begin_pass(enc, color, depth, clear);
                self.draw_opaque(&mut pass);
            }
            mark(enc, "terrain");
            return;
        }
        let count = self.cand_slots.len() as u32;
        let culler = self.culler.as_mut().expect("checked by uses_gpu_culling");
        // Phase 0: what was visible last frame.
        culler.cull(ctx, enc, &self.instances, 0, count);
        mark(enc, "cull 0");
        {
            let mut pass = begin_pass(enc, color, depth, clear);
            self.draw_gpu_phase(&mut pass, 0);
        }
        mark(enc, "terrain 0");
        // Phase 1: test everything against the depth so far and draw what phase 0 missed.
        let culler = self.culler.as_mut().expect("checked by uses_gpu_culling");
        culler.build_hzb(ctx, enc, depth);
        mark(enc, "hi-z");
        let culler = self.culler.as_mut().expect("checked by uses_gpu_culling");
        culler.cull(ctx, enc, &self.instances, 1, count);
        mark(enc, "cull 1");
        {
            let mut pass = begin_pass(enc, color, depth, None);
            self.draw_gpu_phase(&mut pass, 1);
        }
        mark(enc, "terrain 1");
    }

    /// The Hi-Z pyramid built this frame from the full-detail terrain's depth (after its first
    /// phase), when the terrain is GPU-culled: view, level-0 size and levels.
    pub(crate) fn hzb(&self) -> Option<(&wgpu::TextureView, [f32; 2], u32)> {
        if !self.uses_gpu_culling() {
            return None;
        }
        self.culler.as_ref().and_then(|c| c.hzb())
    }

    /// The GPU culling counters (draws, then quads, per phase and pass), when GPU culling
    /// runs; copied into the benchmark's statistics.
    pub fn cull_counters(&self) -> Option<&wgpu::Buffer> {
        if !self.uses_gpu_culling() {
            return None;
        }
        self.culler.as_ref().map(|c| &c.counts)
    }

    /// Opaque then translucent terrain in one call (no sky in between).
    pub fn render(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        clear: Option<wgpu::Color>,
    ) {
        self.render_opaque(ctx, enc, color, depth, clear, None);
        let water = self.water.bind(ctx, depth).clone();
        let mut pass = begin_pass(enc, color, depth, None);
        self.draw_translucent(&mut pass, &water);
    }

    fn draw_gpu_phase<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, phase: u32) {
        let Some(culler) = &self.culler else {
            return;
        };
        pass.set_bind_group(0, &self.bind0, &[]);
        pass.set_bind_group(1, &self.bind1, &[]);
        // The smooth ground is not occlusion-culled yet: drawn whole first in the first phase,
        // its depth hides what lies behind it from the second.
        if phase == 0 {
            self.draw_smooth(pass);
        }
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        let order = [
            (&self.pipes.packed_opaque, 0u32),
            (&self.pipes.general_opaque, 2),
            (&self.pipes.packed_cutout, 1),
            (&self.pipes.general_cutout, 3),
        ];
        for (pipe, i) in order {
            pass.set_pipeline(pipe);
            pass.multi_draw_indexed_indirect_count(
                &culler.draws,
                culler.region_offset(phase, i),
                &culler.counts,
                ((phase * 4 + i) * 4) as u64,
                culler.capacity,
            );
        }
    }

    /// Records the smooth ground's draws (CPU-listed, frustum and cave culled).
    fn draw_smooth<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_index_buffer(self.smooth_i.buffer.slice(..), wgpu::IndexFormat::Uint16);
        self.draw_pass(pass, &self.pipes.smooth, 5);
    }

    /// The buffer of the ground's materials (fixed for the renderer's life).
    pub(crate) fn ground_materials_buffer(&self) -> &wgpu::Buffer {
        &self.ground
    }

    /// The ground's materials (one per slot of `smooth::GroundMaterials`, at most 256).
    pub fn set_ground_materials(&mut self, ctx: &GpuContext, materials: &[GroundMaterial]) {
        let n = materials.len().min(256);
        ctx.write_buffer(&self.ground, 0, bytemuck::cast_slice(&materials[..n]));
    }

    /// GPU-culled draw counts of the last rendered frame per (phase, pass); stalls the GPU.
    /// Tests and diagnostics only.
    pub fn read_gpu_draw_counts(&self, ctx: &GpuContext) -> Option<[u32; 8]> {
        self.culler.as_ref().map(|c| c.read_counts(ctx))
    }

    /// Records the opaque and cutout passes from the CPU draw lists.
    fn draw_opaque<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_bind_group(0, &self.bind0, &[]);
        pass.set_bind_group(1, &self.bind1, &[]);
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        let order = [
            (&self.pipes.packed_opaque, 0usize),
            (&self.pipes.general_opaque, 2),
            (&self.pipes.packed_cutout, 1),
            (&self.pipes.general_cutout, 3),
        ];
        for (pipe, i) in order {
            self.draw_pass(pass, pipe, i);
        }
        self.draw_smooth(pass);
    }

    /// Records the translucent pass (after opaque geometry and the sky), with the water's bind
    /// group of this frame (`water.bind`).
    pub fn draw_translucent<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        water: &'a wgpu::BindGroup,
    ) {
        pass.set_bind_group(0, &self.bind0, &[]);
        pass.set_bind_group(1, &self.bind1, &[]);
        pass.set_bind_group(2, water, &[]);
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.draw_pass(pass, &self.pipes.translucent, 4);
    }

    fn draw_pass<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        pipe: &'a wgpu::RenderPipeline,
        i: usize,
    ) {
        self.draw_list(pass, pipe, &self.passes[i]);
    }

    fn draw_list<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        pipe: &'a wgpu::RenderPipeline,
        p: &'a Pass,
    ) {
        if p.draws.is_empty() {
            return;
        }
        pass.set_pipeline(pipe);
        if self.multi_draw {
            pass.multi_draw_indexed_indirect(&p.buffer, 0, p.draws.len() as u32);
        } else {
            for d in &p.draws {
                pass.draw_indexed(
                    0..d.index_count,
                    d.base_vertex,
                    d.first_instance..d.first_instance + 1,
                );
            }
        }
    }
}

/// The part of the screen (0–1, y down: min x, min y, max x, max y) a camera-relative box
/// covers: its corners in front of the camera and the points where its edges pass the
/// camera's plane (a box reaching behind the camera covers the screen only toward where it
/// lies); `None` when it lies wholly behind.
pub fn screen_rect(view_proj: glam::Mat4, lo: Vec3, hi: Vec3) -> Option<[f32; 4]> {
    // Clip w of points counted as in front (a block's thousandth from the camera).
    const FRONT: f32 = 1e-3;
    let clip: [glam::Vec4; 8] = std::array::from_fn(|c| {
        let p = Vec3::new(
            if c & 1 == 0 { lo.x } else { hi.x },
            if c & 2 == 0 { lo.y } else { hi.y },
            if c & 4 == 0 { lo.z } else { hi.z },
        );
        view_proj * p.extend(1.0)
    });
    let mut r = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    let mut add = |p: glam::Vec4| {
        let (u, v) = (p.x / p.w * 0.5 + 0.5, 0.5 - p.y / p.w * 0.5);
        r = [r[0].min(u), r[1].min(v), r[2].max(u), r[3].max(v)];
    };
    for (a, &pa) in clip.iter().enumerate() {
        if pa.w > FRONT {
            add(pa);
        }
        // The edges from this corner to the corners one step up an axis.
        for axis in [1, 2, 4] {
            if a & axis != 0 {
                continue;
            }
            let pb = clip[a | axis];
            if (pa.w > FRONT) != (pb.w > FRONT) {
                add(pa + (pb - pa) * ((pa.w - FRONT) / (pa.w - pb.w)));
            }
        }
    }
    (r[0] <= r[2]).then(|| {
        [
            r[0].clamp(0.0, 1.0),
            r[1].clamp(0.0, 1.0),
            r[2].clamp(0.0, 1.0),
            r[3].clamp(0.0, 1.0),
        ]
    })
}

/// A render pass over `color` that tests against `depth` without writing it, so the depth can
/// be read as a texture in the same pass (the water reads it).
pub fn begin_pass_read_depth<'e>(
    enc: &'e mut wgpu::CommandEncoder,
    color: &wgpu::TextureView,
    depth: &wgpu::TextureView,
) -> wgpu::RenderPass<'e> {
    enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("translucent"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth,
            depth_ops: None,
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

/// Begins a pass on the scene's colour and depth targets (clearing both when `clear` is set).
pub fn begin_pass<'e>(
    enc: &'e mut wgpu::CommandEncoder,
    color: &wgpu::TextureView,
    depth: &wgpu::TextureView,
    clear: Option<wgpu::Color>,
) -> wgpu::RenderPass<'e> {
    enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("terrain"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: clear.map_or(wgpu::LoadOp::Load, wgpu::LoadOp::Clear),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth,
            depth_ops: Some(wgpu::Operations {
                load: if clear.is_some() {
                    wgpu::LoadOp::Clear(0.0)
                } else {
                    wgpu::LoadOp::Load
                },
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

fn push_draws(draws: &mut Vec<DrawArgs>, offset: u32, count: u32, instance: u32) {
    let mut done = 0;
    while done < count {
        let n = (count - done).min(MAX_QUADS_PER_DRAW);
        draws.push(DrawArgs {
            index_count: n * 6,
            instance_count: 1,
            first_index: 0,
            base_vertex: ((offset + done) * 4) as i32,
            first_instance: instance,
        });
        done += n;
    }
}

/// Bind group 1: packed quads, general quads, instances, the smooth ground's vertices and its
/// materials.
fn make_bind1(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffers: [&wgpu::Buffer; 5],
) -> wgpu::BindGroup {
    let entries: Vec<wgpu::BindGroupEntry> = buffers
        .iter()
        .enumerate()
        .map(|(i, b)| wgpu::BindGroupEntry {
            binding: i as u32,
            resource: b.as_entire_binding(),
        })
        .collect();
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("terrain bind 1"),
        layout,
        entries: &entries,
    })
}

/// The terrain's shaders (`common.wgsl` and `water.wgsl` before them).
fn terrain_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("terrain.wgsl"),
        source: wgpu::ShaderSource::Wgsl(
            concat!(
                include_str!("shaders/common.wgsl"),
                include_str!("shaders/water.wgsl"),
                include_str!("shaders/terrain.wgsl")
            )
            .into(),
        ),
    })
}

fn make_pipelines(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    layout0: &wgpu::BindGroupLayout,
    layout1: &wgpu::BindGroupLayout,
    water: &wgpu::BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> Pipelines {
    let opaque_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("terrain pipeline layout"),
        bind_group_layouts: &[Some(layout0), Some(layout1)],
        immediate_size: 0,
    });
    // The translucent pass also reads the water's resources.
    let translucent_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("terrain translucent pipeline layout"),
        bind_group_layouts: &[Some(layout0), Some(layout1), Some(water)],
        immediate_size: 0,
    });
    let make = |label: &str,
                vs: &str,
                fs: &str,
                cull: Option<wgpu::Face>,
                blend: Option<wgpu::BlendState>,
                depth_write: bool| {
        let layout = if depth_write {
            &opaque_layout
        } else {
            &translucent_layout
        };
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some(vs),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: cull,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(depth_write),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(fs),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    // Each made on its own thread (`TerrainRenderer::new`).
    let make = &make;
    std::thread::scope(|scope| {
        let back = Some(wgpu::Face::Back);
        let smooth =
            scope.spawn(move || make("smooth ground", "vs_smooth", "fs_smooth", back, None, true));
        let packed_opaque =
            scope.spawn(move || make("packed opaque", "vs_packed", "fs_opaque", back, None, true));
        let packed_cutout =
            scope.spawn(move || make("packed cutout", "vs_packed", "fs_cutout", back, None, true));
        let general_opaque = scope.spawn(move || {
            make(
                "general opaque",
                "vs_general",
                "fs_opaque",
                back,
                None,
                true,
            )
        });
        let general_cutout = scope.spawn(move || {
            make(
                "general cutout",
                "vs_general",
                "fs_cutout",
                None,
                None,
                true,
            )
        });
        let translucent = make(
            "translucent",
            "vs_general",
            "fs_translucent",
            None,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            false,
        );
        let join = |h: std::thread::ScopedJoinHandle<'_, wgpu::RenderPipeline>| {
            h.join().expect("a terrain pipeline")
        };
        Pipelines {
            smooth: join(smooth),
            packed_opaque: join(packed_opaque),
            packed_cutout: join(packed_cutout),
            general_opaque: join(general_opaque),
            general_cutout: join(general_cutout),
            translucent,
        }
    })
}

/// Uploads the texture array with a full (alpha-aware) mip chain.
fn upload_texture_array(ctx: &GpuContext, atlas: &TextureArray, mip_levels: u32) -> wgpu::Texture {
    let size = atlas.size;
    let levels = (mip_levels + 1).min(size.ilog2() + 1);
    let layers = atlas.layer_count().max(1);
    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("block texture array"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: layers,
        },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for layer in 0..atlas.layer_count() {
        let mips = atlas.mips(layer as usize, levels);
        for (level, data) in mips.iter().enumerate() {
            let s = (size >> level).max(1);
            let bytes: Vec<u8> = data.iter().flatten().copied().collect();
            ctx.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * s),
                    rows_per_image: Some(s),
                },
                wgpu::Extent3d {
                    width: s,
                    height: s,
                    depth_or_array_layers: 1,
                },
            );
        }
    }
    texture
}

/// Camera-relative helper exposed for other passes.
pub fn relative(planet: &Planet, camera: DVec3, world: DVec3) -> Vec3 {
    planet.delta(camera, world).as_vec3()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_rects_clip_at_the_camera() {
        let camera = crate::camera::Camera {
            pitch: 20.0,
            ..Default::default()
        };
        let vp = camera.view_proj(16.0 / 9.0);
        // A pond ahead, 7° below the horizon (13° above the view's centre): a small rectangle
        // above the middle of the screen.
        let r = screen_rect(vp, Vec3::new(-4.0, -3.0, 20.0), Vec3::new(4.0, -2.9, 30.0))
            .expect("in view");
        assert!(
            r[0] > 0.4 && r[2] < 0.6 && r[1] > 0.3 && r[3] < 0.4,
            "{r:?}"
        );
        // A lake under the camera reaching behind it: from the bottom of the screen up to
        // its far shore, not the sky above.
        let r = screen_rect(
            vp,
            Vec3::new(-50.0, -3.0, -50.0),
            Vec3::new(50.0, -2.9, 50.0),
        )
        .expect("in view");
        assert_eq!((r[0], r[2], r[3]), (0.0, 1.0, 1.0), "{r:?}");
        // The horizon is at 0.24 of the screen's height; the far shore just below it.
        assert!(r[1] > 0.25, "the sky is left out: {r:?}");
        // Wholly behind the camera.
        assert!(
            screen_rect(vp, Vec3::new(-4.0, -3.0, -30.0), Vec3::new(4.0, 3.0, -20.0)).is_none()
        );
    }
}

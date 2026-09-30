//! GPU terrain: all cube meshes live in a few large storage buffers (sub-allocated), drawn with
//! vertex pulling. Each frame the CPU finds candidate cubes (cave culling + frustum); then either
//! the GPU culls them further (two-phase Hi-Z occlusion, see `cull.rs`) and writes the indirect
//! draws itself, or (on adapters without indirect-count draws) the CPU builds the draw lists.

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

/// Translucent quads of cubes within this many cubes of the camera are re-sorted back to front
/// whenever the camera enters another block.
const RESORT_RADIUS: i32 = 2;

/// Quads per draw call are limited by the shared index buffer.
const MAX_QUADS_PER_DRAW: u32 = 16_384;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// First-fit free-list allocator over `[0, capacity)`.
#[derive(Debug, Clone)]
pub struct RangeAllocator {
    capacity: u32,
    /// Free ranges sorted by offset: (offset, len).
    free: Vec<(u32, u32)>,
    used: u32,
}

impl RangeAllocator {
    pub fn new(capacity: u32) -> Self {
        Self {
            capacity,
            free: vec![(0, capacity)],
            used: 0,
        }
    }

    pub fn alloc(&mut self, len: u32) -> Option<u32> {
        if len == 0 {
            return Some(0);
        }
        let i = self.free.iter().position(|(_, l)| *l >= len)?;
        let (off, l) = self.free[i];
        if l == len {
            self.free.remove(i);
        } else {
            self.free[i] = (off + len, l - len);
        }
        self.used += len;
        Some(off)
    }

    pub fn free(&mut self, off: u32, len: u32) {
        if len == 0 {
            return;
        }
        self.used -= len;
        let i = self.free.partition_point(|(o, _)| *o < off);
        self.free.insert(i, (off, len));
        // Coalesce with neighbours.
        if i + 1 < self.free.len() && self.free[i].0 + self.free[i].1 == self.free[i + 1].0 {
            self.free[i].1 += self.free[i + 1].1;
            self.free.remove(i + 1);
        }
        if i > 0 && self.free[i - 1].0 + self.free[i - 1].1 == self.free[i].0 {
            self.free[i - 1].1 += self.free[i].1;
            self.free.remove(i);
        }
    }

    /// Extends the capacity (after the backing buffer grew).
    pub fn grow(&mut self, new_capacity: u32) {
        let extra = new_capacity - self.capacity;
        let old = self.capacity;
        self.capacity = new_capacity;
        self.used += extra;
        self.free(old, extra);
    }

    pub fn used(&self) -> u32 {
        self.used
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }
}

/// A storage buffer of fixed-size elements with a range allocator; grows by doubling.
pub(crate) struct Arena {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) alloc: RangeAllocator,
    stride: u64,
    label: &'static str,
}

impl Arena {
    pub(crate) fn new(
        device: &wgpu::Device,
        label: &'static str,
        stride: u64,
        capacity: u32,
    ) -> Self {
        Self {
            buffer: Self::make(device, label, stride, capacity),
            alloc: RangeAllocator::new(capacity),
            stride,
            label,
        }
    }

    fn make(device: &wgpu::Device, label: &str, stride: u64, capacity: u32) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: stride * capacity as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    /// Allocates `len` elements, growing the buffer if needed. Returns (offset, grew).
    pub(crate) fn alloc(&mut self, ctx: &GpuContext, len: u32) -> (u32, bool) {
        if let Some(off) = self.alloc.alloc(len) {
            return (off, false);
        }
        let max_elems = (ctx.device.limits().max_storage_buffer_binding_size / self.stride) as u32;
        let mut cap = self.alloc.capacity();
        while cap < max_elems
            && self.alloc.capacity() - self.alloc.used() + (cap - self.alloc.capacity()) < len
        {
            cap = (cap * 2).min(max_elems);
        }
        cap = (cap.max(self.alloc.capacity() * 2)).min(max_elems);
        if cap <= self.alloc.capacity() {
            log::error!("{} arena is full ({} elements)", self.label, cap);
            return (u32::MAX, false);
        }
        let new_buf = Self::make(&ctx.device, self.label, self.stride, cap);
        let mut enc = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("arena grow"),
            });
        enc.copy_buffer_to_buffer(
            &self.buffer,
            0,
            &new_buf,
            0,
            self.stride * self.alloc.capacity() as u64,
        );
        ctx.queue.submit(Some(enc.finish()));
        self.buffer = new_buf;
        self.alloc.grow(cap);
        log::info!(
            "{} arena grew to {} MiB",
            self.label,
            (self.stride * cap as u64) >> 20
        );
        let off = self.alloc.alloc(len).unwrap_or(u32::MAX);
        (off, true)
    }
}

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

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Instance {
    origin: [f32; 4],
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
    pub gpu_culling: bool,
    pub resorted: usize,
    /// Translucent quads drawn (always from CPU draw lists).
    pub translucent_quads: u64,
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
    instances: wgpu::Buffer,
    instance_capacity: usize,
    instance_data: Vec<Instance>,
    index_buffer: wgpu::Buffer,
    pipes: Pipelines,
    passes: [Pass; 5],
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
}

struct Pipelines {
    packed_opaque: wgpu::RenderPipeline,
    packed_cutout: wgpu::RenderPipeline,
    general_opaque: wgpu::RenderPipeline,
    general_cutout: wgpu::RenderPipeline,
    translucent: wgpu::RenderPipeline,
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
            entries: &[storage(0), storage(1), storage(2)],
        });
        let bind0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain bind 0"),
            layout: &layout0,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&sky.skyview_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(sky.sampler()),
                },
            ],
        });
        let packed = Arena::new(device, "packed quads", 16, 1 << 20);
        let general = Arena::new(device, "general quads", 64, 1 << 17);
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
            &packed.buffer,
            &general.buffer,
            &instances,
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
        let pipes = make_pipelines(device, &layout0, &layout1, color_format);
        let passes = [
            Pass::new(device, "draws packed opaque"),
            Pass::new(device, "draws packed cutout"),
            Pass::new(device, "draws general opaque"),
            Pass::new(device, "draws general cutout"),
            Pass::new(device, "draws translucent"),
        ];
        Self {
            globals,
            layout0,
            bind0,
            layout1,
            bind1,
            packed,
            general,
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
        }
    }

    /// The per-frame globals, block textures and sky-view table (bind group 0), shared with
    /// the LOD renderer so both light and fade the same way.
    pub fn globals_bind(&self) -> (&wgpu::BindGroupLayout, &wgpu::BindGroup) {
        (&self.layout0, &self.bind0)
    }

    /// True when this frame's opaque geometry is culled and drawn by the GPU.
    pub fn uses_gpu_culling(&self) -> bool {
        self.gpu_culling && self.culler.is_some()
    }

    /// Uploads (or replaces) a cube's mesh.
    pub fn upload(&mut self, ctx: &GpuContext, mesh: &CubeMesh) {
        self.remove(mesh.pos);
        let pos = self.planet.wrap_cube(mesh.pos);
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
        };
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
            self.free_slots.push(g.slot);
        }
    }

    pub fn contains(&self, pos: CubePos) -> bool {
        self.meshes.contains_key(&self.planet.wrap_cube(pos))
    }

    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
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
        for (inst, (pos, o)) in visible.iter().enumerate() {
            let m = &self.meshes[pos];
            let inst = inst as u32;
            self.instance_data.push(Instance {
                origin: [o.x, o.y, o.z, 0.0],
            });
            self.cand_slots.push(m.slot);
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
        for (inst, (pos, _)) in visible.iter().enumerate().rev() {
            let m = &self.meshes[pos];
            if m.trans_len > 0 {
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
                &self.packed.buffer,
                &self.general.buffer,
                &self.instances,
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
                ][i],
            );
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
            block_light: v4(params.block_light, 0.0),
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
        };
        ctx.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        self.stats = TerrainStats {
            meshes: self.meshes.len(),
            visible_cubes: visible.len(),
            draws: self.passes.iter().map(|p| p.draws.len()).sum(),
            quads_drawn,
            packed_bytes: self.packed.alloc.used() as u64 * 16,
            general_bytes: self.general.alloc.used() as u64 * 64,
            gpu_culling: gpu,
            resorted,
            translucent_quads,
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
        let mut pass = begin_pass(enc, color, depth, None);
        self.draw_translucent(&mut pass);
    }

    fn draw_gpu_phase<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, phase: u32) {
        let Some(culler) = &self.culler else {
            return;
        };
        pass.set_bind_group(0, &self.bind0, &[]);
        pass.set_bind_group(1, &self.bind1, &[]);
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
    }

    /// Records the translucent pass (after opaque geometry and the sky).
    pub fn draw_translucent<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_bind_group(0, &self.bind0, &[]);
        pass.set_bind_group(1, &self.bind1, &[]);
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.draw_pass(pass, &self.pipes.translucent, 4);
    }

    fn draw_pass<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        pipe: &'a wgpu::RenderPipeline,
        i: usize,
    ) {
        let p = &self.passes[i];
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

fn make_bind1(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    packed: &wgpu::Buffer,
    general: &wgpu::Buffer,
    instances: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("terrain bind 1"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: packed.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: general.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: instances.as_entire_binding(),
            },
        ],
    })
}

fn make_pipelines(
    device: &wgpu::Device,
    layout0: &wgpu::BindGroupLayout,
    layout1: &wgpu::BindGroupLayout,
    color_format: wgpu::TextureFormat,
) -> Pipelines {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("terrain.wgsl"),
        source: wgpu::ShaderSource::Wgsl(
            concat!(
                include_str!("shaders/common.wgsl"),
                include_str!("shaders/terrain.wgsl")
            )
            .into(),
        ),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("terrain pipeline layout"),
        bind_group_layouts: &[Some(layout0), Some(layout1)],
        immediate_size: 0,
    });
    let make = |label: &str,
                vs: &str,
                fs: &str,
                cull: Option<wgpu::Face>,
                blend: Option<wgpu::BlendState>,
                depth_write: bool| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
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
                module: &module,
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
    Pipelines {
        packed_opaque: make(
            "packed opaque",
            "vs_packed",
            "fs_opaque",
            Some(wgpu::Face::Back),
            None,
            true,
        ),
        packed_cutout: make(
            "packed cutout",
            "vs_packed",
            "fs_cutout",
            Some(wgpu::Face::Back),
            None,
            true,
        ),
        general_opaque: make(
            "general opaque",
            "vs_general",
            "fs_opaque",
            Some(wgpu::Face::Back),
            None,
            true,
        ),
        general_cutout: make(
            "general cutout",
            "vs_general",
            "fs_cutout",
            None,
            None,
            true,
        ),
        translucent: make(
            "translucent",
            "vs_general",
            "fs_translucent",
            None,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            false,
        ),
    }
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
    fn allocator_reuses_and_coalesces() {
        let mut a = RangeAllocator::new(100);
        let x = a.alloc(30).unwrap();
        let y = a.alloc(30).unwrap();
        let z = a.alloc(30).unwrap();
        assert_eq!((x, y, z), (0, 30, 60));
        assert!(a.alloc(20).is_none());
        a.free(y, 30);
        assert_eq!(a.alloc(20), Some(30));
        a.free(30, 20);
        a.free(x, 30);
        a.free(z, 30);
        assert_eq!(a.used(), 0);
        assert_eq!(a.alloc(100), Some(0), "fully coalesced");
        a.free(0, 100);
        a.grow(200);
        assert_eq!(a.alloc(200), Some(0));
    }
}

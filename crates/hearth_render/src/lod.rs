//! Distant terrain (v1 §8): LOD tile meshes (built by `hearth_lod`) drawn after the full-detail
//! terrain, with the same globals — lighting, aerial perspective, planet curvature, seasonal
//! tints — and a dithered handoff at the edge of the full-detail area.
//!
//! Every tile's quads live in one pooled storage buffer as 16-byte records (`hearth_lod::
//! LodQuad`) that the vertex shader expands into their four corners, and the frame's tiles are
//! drawn with one indirect multi-draw (one draw per tile where the adapter can't). A tile's
//! quads are grouped by the way they face, and only the groups that can face the camera are
//! drawn. Where the terrain is GPU-culled, the tiles inside the frustum are also tested on the
//! GPU against its Hi-Z pyramid (`shaders/lod_cull.wgsl`), so land hidden behind near terrain
//! is not drawn.

use bytemuck::{Pod, Zeroable};
use glam::{DVec3, Vec3};
use hearth_math::Planet;
use rustc_hash::FxHashMap;

use crate::camera::{Camera, Frustum};
use crate::gpu::GpuContext;
use crate::terrain::{Arena, DEPTH_FORMAT, EARTH_RADIUS_M, TerrainRenderer};

/// Bytes per LOD quad record (`hearth_lod::LodQuad`).
pub const QUAD_BYTES: u64 = 16;

/// The faces of a tile's quad groups, in storage order (as `hearth_lod::GROUP_FACES`): down,
/// north (−Z), west (−X), up, east (+X), south (+Z), in the terrain's face codes.
pub const GROUP_FACES: [u32; 6] = [0, 2, 4, 1, 5, 3];

struct GpuTile {
    /// First quad in the pool, and how many; how many in each group (`GROUP_FACES`).
    off: u32,
    quads: u32,
    groups: [u32; 6],
    origin: [i32; 2],
    size: i32,
    y: (i32, i32),
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

/// A tile for the GPU cull (`Cand` in `lod_cull.wgsl`): camera-relative bounds and its draw.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Cand {
    lo: [f32; 4],
    hi: [f32; 4],
    /// Index count, base vertex, first instance, unused.
    draw: [u32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct CullParams {
    view_proj: [[f32; 4]; 4],
    hzb_size: [f32; 2],
    hzb_mips: u32,
    count: u32,
    near: f32,
    occlusion: u32,
    pad: [u32; 2],
}

/// Culls the frame's tiles against the full-detail terrain's Hi-Z pyramid on the GPU.
struct LodCuller {
    pipe: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    cands: wgpu::Buffer,
    cand_capacity: usize,
    draws: wgpu::Buffer,
    count: wgpu::Buffer,
    /// The bind group and the Hi-Z view it reads (rebuilt when either changes).
    bind: Option<(wgpu::TextureView, wgpu::BindGroup)>,
    /// Bound when there is no pyramid this frame (occlusion off).
    no_hzb: wgpu::TextureView,
    data: Vec<Cand>,
    /// Culled on the GPU this frame.
    active: bool,
}

impl LodCuller {
    fn new(ctx: &GpuContext) -> Self {
        let device = &ctx.device;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lod_cull.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/lod_cull.wgsl").into()),
        });
        let buf = |binding: u32, ty: wgpu::BufferBindingType| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lod cull layout"),
            entries: &[
                buf(0, wgpu::BufferBindingType::Uniform),
                buf(1, wgpu::BufferBindingType::Storage { read_only: true }),
                buf(2, wgpu::BufferBindingType::Storage { read_only: false }),
                buf(3, wgpu::BufferBindingType::Storage { read_only: false }),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lod cull"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("lod cull"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("cull_lod"),
            compilation_options: Default::default(),
            cache: None,
        });
        let cand_capacity = 1024;
        let no_hzb = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("lod cull: no hi-z"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        Self {
            pipe,
            layout,
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("lod cull params"),
                size: std::mem::size_of::<CullParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            cands: Self::cands_buffer(device, cand_capacity),
            draws: Self::draws_buffer(device, cand_capacity),
            cand_capacity,
            count: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("lod cull count"),
                size: 4,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::INDIRECT
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            bind: None,
            no_hzb,
            data: Vec::new(),
            active: false,
        }
    }

    fn cands_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lod cull candidates"),
            size: (capacity * std::mem::size_of::<Cand>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn draws_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lod culled draws"),
            size: (capacity * std::mem::size_of::<DrawArgs>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::INDIRECT,
            mapped_at_creation: false,
        })
    }
}

/// What the LOD pass holds and drew.
#[derive(Debug, Clone, Copy, Default)]
pub struct LodStats {
    pub tiles: usize,
    /// Tiles inside the frustum (before occlusion culling on the GPU), and their draws (runs of
    /// quad groups facing the camera).
    pub drawn: usize,
    pub draws: usize,
    /// Quads of the groups facing the camera, in the frustum.
    pub quads: u64,
    pub bytes: u64,
    /// Tested against the near terrain's Hi-Z pyramid on the GPU.
    pub gpu_culled: bool,
}

/// The LOD renderer.
pub struct LodRenderer {
    /// The largest buffer the device binds (bytes): what the quads can grow to.
    max_bytes: u64,
    pipeline: wgpu::RenderPipeline,
    layout1: wgpu::BindGroupLayout,
    bind1: wgpu::BindGroup,
    /// All tiles' quad records.
    pool: Arena,
    origins: wgpu::Buffer,
    origins_capacity: usize,
    draw_buffer: wgpu::Buffer,
    draw_capacity: usize,
    index_buffer: wgpu::Buffer,
    index_quads: u32,
    tiles: FxHashMap<u64, GpuTile>,
    /// This frame's draws and tile origins (kept so frames allocate nothing).
    draws: Vec<DrawArgs>,
    origins_scratch: Vec<[f32; 4]>,
    /// Indirect multi-draws with a first instance are available.
    multi_draw: bool,
    /// Occlusion culling on the GPU (where indirect-count draws work, as for the terrain).
    culler: Option<LodCuller>,
    bind_dirty: bool,
    planet: Planet,
    pub stats: LodStats,
}

fn quad_indices(quads: u32) -> Vec<u32> {
    let mut v = Vec::with_capacity(quads as usize * 6);
    for q in 0..quads {
        let b = q * 4;
        v.extend_from_slice(&[b, b + 1, b + 2, b + 2, b + 3, b]);
    }
    v
}

impl LodRenderer {
    pub fn new(
        ctx: &GpuContext,
        terrain: &TerrainRenderer,
        planet: Planet,
        color_format: wgpu::TextureFormat,
    ) -> Self {
        let device = &ctx.device;
        let (layout0, _) = terrain.globals_bind();
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
            label: Some("lod layout 1"),
            entries: &[storage(0), storage(1)],
        });
        let origins_capacity = 1024;
        let origins = Self::origins_buffer(device, origins_capacity);
        let pool = Arena::new(device, "lod quads", QUAD_BYTES, 1 << 20);
        let bind1 = Self::bind(device, &layout1, &origins, &pool.buffer);
        let draw_capacity = 1024;
        let draw_buffer = Self::draw_buffer(device, draw_capacity);
        let index_quads = 4096;
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lod quad indices"),
            size: index_quads as u64 * 24,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.write_buffer(
            &index_buffer,
            0,
            bytemuck::cast_slice(&quad_indices(index_quads)),
        );
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lod.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                concat!(
                    include_str!("shaders/common.wgsl"),
                    include_str!("shaders/water.wgsl"),
                    include_str!("shaders/lod.wgsl")
                )
                .into(),
            ),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lod pipeline layout"),
            bind_group_layouts: &[
                Some(layout0),
                Some(&layout1),
                Some(terrain.water.waves_layout()),
            ],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lod"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_lod"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                // Column faces are wound both ways; skirts must show from either side.
                cull_mode: None,
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
                entry_point: Some("fs_lod"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            layout1,
            bind1,
            pool,
            origins,
            origins_capacity,
            draw_buffer,
            draw_capacity,
            index_buffer,
            index_quads,
            tiles: FxHashMap::default(),
            draws: Vec::new(),
            origins_scratch: Vec::new(),
            multi_draw: ctx.caps.indirect_first_instance,
            max_bytes: device.limits().max_storage_buffer_binding_size,
            culler: (ctx.caps.multi_draw_indirect_count
                && ctx.caps.indirect_first_instance
                && ctx.info.backend == wgpu::Backend::Vulkan)
                .then(|| LodCuller::new(ctx)),
            bind_dirty: false,
            planet,
            stats: LodStats::default(),
        }
    }

    fn origins_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lod tile origins"),
            size: (capacity * 16) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn draw_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lod draws"),
            size: (capacity * std::mem::size_of::<DrawArgs>()) as u64,
            usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn bind(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        origins: &wgpu::Buffer,
        quads: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lod bind 1"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: origins.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: quads.as_entire_binding(),
                },
            ],
        })
    }

    /// Uploads (or replaces) a tile: its id, minimum corner in world blocks (X canonical), side,
    /// height range and quad records (`QUAD_BYTES` each), grouped by the way they face, with the
    /// number in each group (`GROUP_FACES`).
    #[allow(clippy::too_many_arguments)]
    pub fn upload(
        &mut self,
        ctx: &GpuContext,
        id: u64,
        origin: [i32; 2],
        size: i32,
        y: (i32, i32),
        quads: &[u8],
        groups: [u32; 6],
    ) {
        self.remove(id);
        let n = (quads.len() as u64 / QUAD_BYTES) as u32;
        debug_assert_eq!(groups.iter().sum::<u32>(), n, "groups cover the quads");
        if n == 0 {
            return;
        }
        if n > self.index_quads {
            self.index_quads = n.next_power_of_two();
            self.index_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("lod quad indices"),
                size: self.index_quads as u64 * 24,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            ctx.write_buffer(
                &self.index_buffer,
                0,
                bytemuck::cast_slice(&quad_indices(self.index_quads)),
            );
        }
        let (off, grew) = self.pool.alloc(ctx, n);
        if off == u32::MAX {
            return;
        }
        self.bind_dirty |= grew;
        ctx.write_buffer(&self.pool.buffer, off as u64 * QUAD_BYTES, quads);
        self.tiles.insert(
            id,
            GpuTile {
                off,
                quads: n,
                groups,
                origin,
                size,
                y,
            },
        );
    }

    pub fn contains(&self, id: u64) -> bool {
        self.tiles.contains_key(&id)
    }

    pub fn remove(&mut self, id: u64) {
        if let Some(t) = self.tiles.remove(&id) {
            self.pool.alloc.free(t.off, t.quads);
        }
    }

    /// Keeps only the tiles `keep` accepts.
    pub fn retain(&mut self, keep: impl Fn(u64) -> bool) {
        let pool = &mut self.pool;
        self.tiles.retain(|id, t| {
            let k = keep(*id);
            if !k {
                pool.alloc.free(t.off, t.quads);
            }
            k
        });
    }

    /// The most the tiles' quads can take (bytes): the largest buffer the device binds.
    pub fn max_bytes(&self) -> u64 {
        self.max_bytes
    }

    /// Video memory the tiles' quads take (bytes).
    pub fn bytes(&self) -> u64 {
        self.pool.alloc.used() as u64 * QUAD_BYTES
    }

    /// Chooses this frame's draws among `show` (the tiles of the current selection): those
    /// uploaded and inside the frustum, placed relative to the camera by the shortest way around,
    /// each drawn as the runs of its quad groups that can face the camera.
    /// With `hzb` (the size and levels of this frame's Hi-Z pyramid of the near terrain), they
    /// are tested against it on the GPU as well (`cull`).
    pub fn prepare(
        &mut self,
        ctx: &GpuContext,
        camera: &Camera,
        aspect: f32,
        vertical_scale: f32,
        show: &[u64],
        hzb: Option<([f32; 2], u32)>,
    ) {
        let frustum = Frustum::from_view_proj(camera.view_proj(aspect));
        let curvature = (0.5 / (EARTH_RADIUS_M * vertical_scale.max(1e-3) as f64)) as f32;
        let cam: DVec3 = camera.pos;
        let mut origins = std::mem::take(&mut self.origins_scratch);
        origins.clear();
        self.draws.clear();
        if let Some(c) = self.culler.as_mut() {
            c.data.clear();
        }
        let (mut quads, mut drawn) = (0u64, 0usize);
        for &id in show {
            let Some(t) = self.tiles.get(&id) else {
                continue;
            };
            let ox = self.planet.delta_x(cam.x, t.origin[0] as f64) as f32;
            let oz = (t.origin[1] as f64 - cam.z) as f32;
            let size = t.size as f32;
            // The farthest corner sinks most under the planet's curvature.
            let far = (ox.abs() + size).powi(2) + (oz.abs() + size).powi(2);
            let sink = far * curvature;
            let (y0, y1) = ((t.y.0 as f64 - cam.y) as f32, (t.y.1 as f64 - cam.y) as f32);
            let min = Vec3::new(ox, y0 - sink, oz);
            let max = Vec3::new(ox + size, y1, oz + size);
            if !frustum.intersects_aabb(min, max) {
                continue;
            }
            // The groups that can face the camera (`GROUP_FACES`): a side group faces away when
            // the whole tile lies behind its faces' planes; tops face away when the camera is
            // below the tile, bottoms when it is above (the curvature tilts far faces away from
            // the camera, which only matters for bottoms: they may show a little higher).
            let facing = [
                y1 + sink > 0.0,
                oz + size > 0.0,
                ox + size > 0.0,
                y0 < 0.0,
                ox < 0.0,
                oz < 0.0,
            ];
            // Runs of consecutive groups facing the camera (empty groups join runs): at most 3.
            let mut runs = [(0u32, 0u32); 3];
            let (mut n_runs, mut open, mut at) = (0, false, t.off);
            for (g, &n) in t.groups.iter().enumerate() {
                if n == 0 {
                    continue;
                }
                if !facing[g] {
                    open = false;
                } else if open {
                    runs[n_runs - 1].1 += n;
                } else {
                    runs[n_runs] = (at, n);
                    n_runs += 1;
                    open = true;
                }
                at += n;
            }
            if n_runs == 0 {
                continue;
            }
            let slot = origins.len() as u32;
            for &(off, n) in &runs[..n_runs] {
                let draw = DrawArgs {
                    index_count: n * 6,
                    instance_count: 1,
                    first_index: 0,
                    base_vertex: (off * 4) as i32,
                    first_instance: slot,
                };
                if let Some(c) = self.culler.as_mut() {
                    // The bounds its vertices can reach, curvature included.
                    c.data.push(Cand {
                        lo: min.extend(0.0).to_array(),
                        hi: max.extend(0.0).to_array(),
                        draw: [
                            draw.index_count,
                            draw.base_vertex as u32,
                            draw.first_instance,
                            0,
                        ],
                    });
                }
                self.draws.push(draw);
                quads += n as u64;
            }
            origins.push([ox, -(cam.y as f32), oz, 0.0]);
            drawn += 1;
        }
        if origins.len() > self.origins_capacity {
            self.origins_capacity = origins.len().next_power_of_two();
            self.origins = Self::origins_buffer(&ctx.device, self.origins_capacity);
            self.bind_dirty = true;
        }
        if self.draws.len() > self.draw_capacity {
            self.draw_capacity = self.draws.len().next_power_of_two();
            self.draw_buffer = Self::draw_buffer(&ctx.device, self.draw_capacity);
        }
        if self.bind_dirty {
            self.bind1 = Self::bind(&ctx.device, &self.layout1, &self.origins, &self.pool.buffer);
            self.bind_dirty = false;
        }
        let gpu = self.culler.is_some() && !self.draws.is_empty();
        if let Some(c) = self.culler.as_mut() {
            c.active = gpu;
        }
        if !origins.is_empty() {
            ctx.write_buffer(&self.origins, 0, bytemuck::cast_slice(&origins));
            if gpu {
                self.prepare_cull(ctx, camera, aspect, hzb);
            } else if self.multi_draw {
                ctx.write_buffer(&self.draw_buffer, 0, bytemuck::cast_slice(&self.draws));
            }
        }
        self.origins_scratch = origins;
        self.stats = LodStats {
            tiles: self.tiles.len(),
            drawn,
            draws: self.draws.len(),
            quads,
            bytes: self.pool.alloc.used() as u64 * QUAD_BYTES,
            gpu_culled: gpu && hzb.is_some(),
        };
    }

    /// Uploads the candidates (every tile inside the frustum) and parameters of the GPU cull.
    fn prepare_cull(
        &mut self,
        ctx: &GpuContext,
        camera: &Camera,
        aspect: f32,
        hzb: Option<([f32; 2], u32)>,
    ) {
        let Some(c) = self.culler.as_mut() else {
            return;
        };
        if c.data.len() > c.cand_capacity {
            c.cand_capacity = c.data.len().next_power_of_two();
            c.cands = LodCuller::cands_buffer(&ctx.device, c.cand_capacity);
            c.draws = LodCuller::draws_buffer(&ctx.device, c.cand_capacity);
            c.bind = None;
        }
        ctx.write_buffer(&c.cands, 0, bytemuck::cast_slice(&c.data));
        let (hzb_size, hzb_mips) = hzb.unwrap_or(([1.0, 1.0], 1));
        let p = CullParams {
            view_proj: camera.view_proj(aspect).to_cols_array_2d(),
            hzb_size,
            hzb_mips,
            count: c.data.len() as u32,
            near: camera.near,
            occlusion: u32::from(hzb.is_some()),
            pad: [0; 2],
        };
        ctx.write_buffer(&c.params, 0, bytemuck::bytes_of(&p));
    }

    /// Records the GPU cull of this frame's tiles against the near terrain's Hi-Z pyramid (after
    /// the full-detail opaque terrain, before `draw`); frustum only without a pyramid.
    pub fn cull(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        hzb: Option<&wgpu::TextureView>,
    ) {
        let Some(c) = self.culler.as_mut().filter(|c| c.active) else {
            return;
        };
        let view = hzb.unwrap_or(&c.no_hzb);
        if c.bind.as_ref().is_none_or(|(v, _)| v != view) {
            let bind = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("lod cull bind"),
                layout: &c.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: c.params.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: c.cands.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: c.draws.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: c.count.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                ],
            });
            c.bind = Some((view.clone(), bind));
        }
        enc.clear_buffer(&c.count, 0, None);
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("lod cull"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&c.pipe);
        pass.set_bind_group(0, &c.bind.as_ref().expect("made above").1, &[]);
        pass.dispatch_workgroups((c.data.len() as u32).div_ceil(64), 1, 1);
    }

    /// Draws this frame's tiles (after the full-detail opaque terrain, before the sky).
    /// `waves`: the water's waves bind group (`WaterRenderer::waves_bind`).
    pub fn draw<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        bind0: &'a wgpu::BindGroup,
        waves: &'a wgpu::BindGroup,
    ) {
        if self.draws.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind0, &[]);
        pass.set_bind_group(1, &self.bind1, &[]);
        pass.set_bind_group(2, waves, &[]);
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        if let Some(c) = self.culler.as_ref().filter(|c| c.active) {
            pass.multi_draw_indexed_indirect_count(&c.draws, 0, &c.count, 0, c.data.len() as u32);
        } else if self.multi_draw {
            pass.multi_draw_indexed_indirect(&self.draw_buffer, 0, self.draws.len() as u32);
        } else {
            for d in &self.draws {
                pass.draw_indexed(
                    0..d.index_count,
                    d.base_vertex,
                    d.first_instance..d.first_instance + 1,
                );
            }
        }
    }
}

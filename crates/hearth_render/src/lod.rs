//! Distant terrain (v1 §8): LOD tile meshes (built by `hearth_lod`) drawn after the full-detail
//! terrain, with the same globals — lighting, aerial perspective, planet curvature, seasonal
//! tints — and a dithered handoff at the edge of the full-detail area.
//!
//! Every tile's quads live in one pooled storage buffer as 16-byte records (`hearth_lod::
//! LodQuad`) that the vertex shader expands into their four corners, and the frame's tiles are
//! drawn with one indirect multi-draw (one draw per tile where the adapter can't).

use bytemuck::{Pod, Zeroable};
use glam::{DVec3, Vec3};
use hearth_math::Planet;
use rustc_hash::FxHashMap;

use crate::camera::{Camera, Frustum};
use crate::gpu::GpuContext;
use crate::terrain::{Arena, DEPTH_FORMAT, EARTH_RADIUS_M, TerrainRenderer};

/// Bytes per LOD quad record (`hearth_lod::LodQuad`).
pub const QUAD_BYTES: u64 = 16;

struct GpuTile {
    /// First quad in the pool, and how many.
    off: u32,
    quads: u32,
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

/// What the LOD pass holds and drew.
#[derive(Debug, Clone, Copy, Default)]
pub struct LodStats {
    pub tiles: usize,
    pub drawn: usize,
    pub quads: u64,
    pub bytes: u64,
}

/// The LOD renderer.
pub struct LodRenderer {
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
                    include_str!("shaders/lod.wgsl")
                )
                .into(),
            ),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lod pipeline layout"),
            bind_group_layouts: &[Some(layout0), Some(&layout1)],
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
    /// height range and quad records (`QUAD_BYTES` each).
    pub fn upload(
        &mut self,
        ctx: &GpuContext,
        id: u64,
        origin: [i32; 2],
        size: i32,
        y: (i32, i32),
        quads: &[u8],
    ) {
        self.remove(id);
        let n = (quads.len() as u64 / QUAD_BYTES) as u32;
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

    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    /// Chooses this frame's draws among `show` (the tiles of the current selection): those
    /// uploaded and inside the frustum, placed relative to the camera by the shortest way around.
    pub fn prepare(
        &mut self,
        ctx: &GpuContext,
        camera: &Camera,
        aspect: f32,
        vertical_scale: f32,
        show: &[u64],
    ) {
        let frustum = Frustum::from_view_proj(camera.view_proj(aspect));
        let curvature = (0.5 / (EARTH_RADIUS_M * vertical_scale.max(1e-3) as f64)) as f32;
        let cam: DVec3 = camera.pos;
        let mut origins = std::mem::take(&mut self.origins_scratch);
        origins.clear();
        self.draws.clear();
        let mut quads = 0u64;
        for &id in show {
            let Some(t) = self.tiles.get(&id) else {
                continue;
            };
            let ox = self.planet.delta_x(cam.x, t.origin[0] as f64) as f32;
            let oz = (t.origin[1] as f64 - cam.z) as f32;
            let size = t.size as f32;
            // The farthest corner sinks most under the planet's curvature.
            let far = (ox.abs() + size).powi(2) + (oz.abs() + size).powi(2);
            let min = Vec3::new(ox, (t.y.0 as f64 - cam.y) as f32 - far * curvature, oz);
            let max = Vec3::new(ox + size, (t.y.1 as f64 - cam.y) as f32, oz + size);
            if !frustum.intersects_aabb(min, max) {
                continue;
            }
            self.draws.push(DrawArgs {
                index_count: t.quads * 6,
                instance_count: 1,
                first_index: 0,
                base_vertex: (t.off * 4) as i32,
                first_instance: origins.len() as u32,
            });
            origins.push([ox, -(cam.y as f32), oz, 0.0]);
            quads += t.quads as u64;
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
        if !origins.is_empty() {
            ctx.write_buffer(&self.origins, 0, bytemuck::cast_slice(&origins));
            if self.multi_draw {
                ctx.write_buffer(&self.draw_buffer, 0, bytemuck::cast_slice(&self.draws));
            }
        }
        self.origins_scratch = origins;
        self.stats = LodStats {
            tiles: self.tiles.len(),
            drawn: self.draws.len(),
            quads,
            bytes: self.pool.alloc.used() as u64 * QUAD_BYTES,
        };
    }

    /// Draws this frame's tiles (after the full-detail opaque terrain, before the sky).
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, bind0: &'a wgpu::BindGroup) {
        if self.draws.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind0, &[]);
        pass.set_bind_group(1, &self.bind1, &[]);
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        if self.multi_draw {
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

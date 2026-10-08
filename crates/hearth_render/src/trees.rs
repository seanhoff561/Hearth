//! Trees and woody shrubs drawn as meshes (Amendment S §7.1–7.2, S5): each tree's mesh
//! (`hearth_flora::mesh`, by species, stage, variant, what is left of it and detail) kept on the
//! GPU once, and every tree of it drawn as an instance with its place, its turn, its colours,
//! its climate and the light where it stands: the wood opaque, the leaves alpha-tested and drawn
//! from both sides (`shaders/tree.wgsl`), after the terrain with its globals.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use hearth_flora::mesh::{TreeMesh, TreeVertex};
use rustc_hash::FxHashMap;

use crate::gpu::GpuContext;
use crate::terrain::DEPTH_FORMAT;

/// One tree drawn (48 bytes; `TreeIn`'s instance part in `tree.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct TreeInstance {
    /// Where the skeleton's origin is, camera-relative.
    pub origin: [f32; 3],
    /// Bark colour (sRGB, packed) | flags and pattern << 24 (`BARK_*`).
    pub bark: u32,
    /// The turn in the ground's plane: x' = t0 x + t1 z, z' = t2 x + t3 z.
    pub turn: [f32; 4],
    /// Leaf colour (sRGB, packed) | tint kind << 24 (as the terrain's).
    pub leaf: u32,
    /// Climate code (24 bits, `hearth_env::tint`) | snow on it (0..255) << 24.
    pub climate: u32,
    /// Sky light level | block light level << 4 | its sway's phase << 8.
    pub light: u32,
    pub pad: u32,
}

/// The bark burned black (`TreeInstance::bark` flag, above the pattern's four bits).
pub const BARK_CHARRED: u32 = 0x10;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct FrameUniform {
    wind: [f32; 4],
}

struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    wood: u32,
    total: u32,
    bytes: u64,
    /// The frame it was last drawn in.
    used: u64,
}

/// What the tree pass drew.
#[derive(Debug, Clone, Copy, Default)]
pub struct TreeStats {
    pub meshes: usize,
    pub instances: usize,
    pub triangles: u64,
    pub bytes: u64,
}

pub struct TreeRenderer {
    wood: wgpu::RenderPipeline,
    leaves: wgpu::RenderPipeline,
    frame_buf: wgpu::Buffer,
    bind: wgpu::BindGroup,
    meshes: FxHashMap<u64, GpuMesh>,
    instances: wgpu::Buffer,
    capacity: usize,
    /// This frame's batches: mesh, first instance, count.
    batches: Vec<(u64, u32, u32)>,
    frame: u64,
    pub stats: TreeStats,
}

impl TreeRenderer {
    /// `globals` is the terrain's bind group 0 layout.
    pub fn new(
        ctx: &GpuContext,
        globals: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> Self {
        let device = &ctx.device;
        let layout1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("trees frame"),
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
        let frame_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("trees frame"),
            size: std::mem::size_of::<FrameUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("trees frame"),
            layout: &layout1,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buf.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("trees"),
            bind_group_layouts: &[Some(globals), Some(&layout1)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tree.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                concat!(
                    include_str!("shaders/common.wgsl"),
                    include_str!("shaders/tree.wgsl")
                )
                .into(),
            ),
        });
        let vertex_attrs = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x2, 2 => Snorm8x4, 3 => Uint8x4, 4 => Uint8x4
        ];
        let instance_attrs = wgpu::vertex_attr_array![
            5 => Float32x3, 6 => Uint32, 7 => Float32x4, 8 => Uint32, 9 => Uint32, 10 => Uint32
        ];
        let buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<TreeVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &vertex_attrs,
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<TreeInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &instance_attrs,
            }),
        ];
        let pipe = |label: &str, fs: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_tree"),
                    compilation_options: Default::default(),
                    buffers: &buffers,
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    // Mirrored trees turn their winding about; tubes are closed, cards two-sided.
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
                    entry_point: Some(fs),
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
        let capacity = 1024;
        Self {
            wood: pipe("trees: wood", "fs_wood"),
            leaves: pipe("trees: leaves", "fs_leaf"),
            frame_buf,
            bind,
            meshes: FxHashMap::default(),
            instances: Self::instance_buffer(device, capacity),
            capacity,
            batches: Vec::new(),
            frame: 0,
            stats: TreeStats::default(),
        }
    }

    fn instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tree instances"),
            size: (capacity * std::mem::size_of::<TreeInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Whether a mesh is kept.
    pub fn has(&self, key: u64) -> bool {
        self.meshes.contains_key(&key)
    }

    /// Keeps a mesh under a key (the caller's: species, stage, variant, remains, detail).
    pub fn upload(&mut self, ctx: &GpuContext, key: u64, mesh: &TreeMesh) {
        if mesh.indices.is_empty() {
            self.meshes.remove(&key);
            return;
        }
        let make = |label: &str, bytes: &[u8], usage: wgpu::BufferUsages| {
            let b = ctx.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: bytes.len() as u64,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            ctx.write_buffer(&b, 0, bytes);
            b
        };
        let vertices = make(
            "tree vertices",
            bytemuck::cast_slice(&mesh.vertices),
            wgpu::BufferUsages::VERTEX,
        );
        let indices = make(
            "tree indices",
            bytemuck::cast_slice(&mesh.indices),
            wgpu::BufferUsages::INDEX,
        );
        self.meshes.insert(
            key,
            GpuMesh {
                vertices,
                indices,
                wood: mesh.wood,
                total: mesh.indices.len() as u32,
                bytes: mesh.bytes() as u64,
                used: self.frame,
            },
        );
    }

    /// Lets go of the meshes not drawn in the last `frames` frames.
    pub fn forget_unused(&mut self, frames: u64) {
        let now = self.frame;
        self.meshes
            .retain(|_, m| now.saturating_sub(m.used) <= frames);
    }

    /// This frame's trees, as instances of the meshes kept (those whose mesh is not kept are
    /// left out), and the wind (m/s).
    pub fn prepare(&mut self, ctx: &GpuContext, batches: &[(u64, Vec<TreeInstance>)], wind: Vec3) {
        self.frame += 1;
        let speed = wind.length();
        let dir = if speed > 1e-3 { wind / speed } else { Vec3::X };
        ctx.write_buffer(
            &self.frame_buf,
            0,
            bytemuck::bytes_of(&FrameUniform {
                wind: [dir.x, dir.y, dir.z, speed],
            }),
        );
        self.batches.clear();
        let mut all: Vec<TreeInstance> = Vec::new();
        let mut triangles = 0u64;
        for (key, list) in batches {
            let Some(m) = self.meshes.get_mut(key) else {
                continue;
            };
            if list.is_empty() {
                continue;
            }
            m.used = self.frame;
            self.batches
                .push((*key, all.len() as u32, list.len() as u32));
            triangles += (m.total / 3) as u64 * list.len() as u64;
            all.extend_from_slice(list);
        }
        if all.len() > self.capacity {
            self.capacity = all.len().next_power_of_two();
            self.instances = Self::instance_buffer(&ctx.device, self.capacity);
        }
        if !all.is_empty() {
            ctx.write_buffer(&self.instances, 0, bytemuck::cast_slice(&all));
        }
        self.stats = TreeStats {
            meshes: self.meshes.len(),
            instances: all.len(),
            triangles,
            bytes: self.meshes.values().map(|m| m.bytes).sum(),
        };
    }

    /// Draws this frame's trees (after the terrain, in its pass).
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, globals: &'a wgpu::BindGroup) {
        if self.batches.is_empty() {
            return;
        }
        pass.set_bind_group(0, globals, &[]);
        pass.set_bind_group(1, &self.bind, &[]);
        pass.set_vertex_buffer(1, self.instances.slice(..));
        for (pipeline, leaves) in [(&self.wood, false), (&self.leaves, true)] {
            pass.set_pipeline(pipeline);
            for &(key, first, count) in &self.batches {
                let m = &self.meshes[&key];
                let range = if leaves { m.wood..m.total } else { 0..m.wood };
                if range.is_empty() {
                    continue;
                }
                pass.set_vertex_buffer(0, m.vertices.slice(..));
                pass.set_index_buffer(m.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(range, 0, first..first + count);
            }
        }
    }
}

//! GPU-driven occlusion culling for terrain: a Hi-Z depth pyramid and a compute pass that emits
//! indirect draws (see `shaders/cull.wgsl` for the two-phase scheme). Used when the adapter
//! supports indirect-count multi-draws; otherwise the terrain renderer builds draws on the CPU.

use bytemuck::{Pod, Zeroable};
use glam::Mat4;

use crate::gpu::GpuContext;

/// Draw regions: 2 phases × 4 opaque/cutout passes.
pub(crate) const REGIONS: u32 = 8;
const DRAW_ARGS_SIZE: u64 = 20;
const SLOT_SIZE: u64 = 64;

/// Per-cube draw layout, mirrored by `Slot` in the shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct SlotRecord {
    pub packed_off: u32,
    pub general_off: u32,
    pub model_opaque: u32,
    pub model_cutout: u32,
    pub counts: [u32; 12],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Params {
    view_proj: [[f32; 4]; 4],
    hzb_size: [f32; 2],
    hzb_mips: u32,
    count: u32,
    phase: u32,
    capacity: u32,
    occlusion: u32,
    near: f32,
}

struct Hzb {
    texture: wgpu::Texture,
    all: wgpu::TextureView,
    levels: Vec<wgpu::TextureView>,
    /// Depth size this pyramid was built for.
    depth_size: (u32, u32),
    /// Bind groups for levels 1.. (level 0 reads the depth target and is bound per frame).
    down_binds: Vec<wgpu::BindGroup>,
}

pub(crate) struct GpuCuller {
    cull_pipe: wgpu::ComputePipeline,
    cull_layout: wgpu::BindGroupLayout,
    hzb_depth_pipe: wgpu::ComputePipeline,
    hzb_down_pipe: wgpu::ComputePipeline,
    hzb_layout: wgpu::BindGroupLayout,
    params: [wgpu::Buffer; 2],
    slots: wgpu::Buffer,
    slot_capacity: u32,
    vis: wgpu::Buffer,
    cands: wgpu::Buffer,
    cand_capacity: u32,
    pub draws: wgpu::Buffer,
    /// Draws per region.
    pub capacity: u32,
    pub counts: wgpu::Buffer,
    hzb: Option<Hzb>,
    binds: Option<[wgpu::BindGroup; 2]>,
    /// Placeholders for bindings a pass doesn't use.
    dummy_hzb: wgpu::TextureView,
    dummy_depth: wgpu::TextureView,
}

impl GpuCuller {
    pub fn new(ctx: &GpuContext) -> Self {
        let device = &ctx.device;
        let cull_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cull.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/cull.wgsl").into()),
        });
        let hzb_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hzb.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/hzb.wgsl").into()),
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
        let ro = wgpu::BufferBindingType::Storage { read_only: true };
        let rw = wgpu::BufferBindingType::Storage { read_only: false };
        let cull_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cull layout"),
            entries: &[
                buf(0, wgpu::BufferBindingType::Uniform),
                buf(1, ro),
                buf(2, ro),
                buf(3, ro),
                buf(4, rw),
                buf(5, rw),
                buf(6, rw),
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
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
        let hzb_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hzb layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let pipe = |label: &str,
                    module: &wgpu::ShaderModule,
                    layout: &wgpu::BindGroupLayout,
                    entry: &str| {
            let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&pl),
                module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let cull_pipe = pipe("terrain cull", &cull_module, &cull_layout, "cull");
        let hzb_depth_pipe = pipe("hzb from depth", &hzb_module, &hzb_layout, "from_depth");
        let hzb_down_pipe = pipe("hzb downsample", &hzb_module, &hzb_layout, "downsample");
        let params = [0, 1].map(|i| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(if i == 0 {
                    "cull params phase 0"
                } else {
                    "cull params phase 1"
                }),
                size: std::mem::size_of::<Params>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        let slot_capacity = 4096;
        let cand_capacity = 4096;
        let capacity = 16_384;
        let placeholder = |label: &str, format: wgpu::TextureFormat| {
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
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        Self {
            slots: storage_buffer(device, "cull slots", SLOT_SIZE * slot_capacity as u64, true),
            vis: storage_buffer(device, "cull visibility", 4 * slot_capacity as u64, false),
            slot_capacity,
            cands: storage_buffer(device, "cull candidates", 4 * cand_capacity as u64, false),
            cand_capacity,
            draws: draw_buffer(device, capacity),
            capacity,
            counts: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("cull draw counts"),
                size: 4 * REGIONS as u64,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::INDIRECT
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            cull_pipe,
            cull_layout,
            hzb_depth_pipe,
            hzb_down_pipe,
            hzb_layout,
            params,
            hzb: None,
            binds: None,
            dummy_hzb: placeholder("hzb placeholder", wgpu::TextureFormat::R32Float),
            dummy_depth: placeholder("depth placeholder", crate::terrain::DEPTH_FORMAT),
        }
    }

    /// Writes a slot's draw layout and marks it "not visible last frame" so phase 1 decides.
    pub fn write_slot(&mut self, ctx: &GpuContext, slot: u32, rec: &SlotRecord) {
        if slot >= self.slot_capacity {
            let mut cap = self.slot_capacity;
            while cap <= slot {
                cap *= 2;
            }
            let slots = storage_buffer(&ctx.device, "cull slots", SLOT_SIZE * cap as u64, true);
            let mut enc = ctx
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("cull slots grow"),
                });
            enc.copy_buffer_to_buffer(
                &self.slots,
                0,
                &slots,
                0,
                SLOT_SIZE * self.slot_capacity as u64,
            );
            ctx.queue.submit(Some(enc.finish()));
            self.slots = slots;
            // Visibility restarts from "unknown" for everything; phase 1 re-tests all cubes.
            self.vis = storage_buffer(&ctx.device, "cull visibility", 4 * cap as u64, false);
            self.slot_capacity = cap;
            self.binds = None;
        }
        ctx.queue.write_buffer(
            &self.slots,
            slot as u64 * SLOT_SIZE,
            bytemuck::bytes_of(rec),
        );
        ctx.queue
            .write_buffer(&self.vis, slot as u64 * 4, bytemuck::bytes_of(&0u32));
    }

    /// Uploads this frame's candidates and parameters.
    pub fn prepare(
        &mut self,
        ctx: &GpuContext,
        cands: &[u32],
        view_proj: Mat4,
        near: f32,
        depth_size: (u32, u32),
    ) {
        let n = cands.len() as u32;
        if n > self.cand_capacity {
            self.cand_capacity = n.next_power_of_two();
            self.cands = storage_buffer(
                &ctx.device,
                "cull candidates",
                4 * self.cand_capacity as u64,
                false,
            );
            self.binds = None;
        }
        // Every candidate can emit at most 6 packed draws per layer; general draws are rarer.
        let need = (n * 6).max(4096);
        if need > self.capacity {
            self.capacity = need.next_power_of_two();
            self.draws = draw_buffer(&ctx.device, self.capacity);
            self.binds = None;
        }
        if !cands.is_empty() {
            ctx.queue
                .write_buffer(&self.cands, 0, bytemuck::cast_slice(cands));
        }
        self.ensure_hzb(ctx, depth_size);
        let hzb = self.hzb.as_ref().expect("created above");
        for phase in 0..2u32 {
            let p = Params {
                view_proj: view_proj.to_cols_array_2d(),
                hzb_size: [depth_size.0 as f32 / 2.0, depth_size.1 as f32 / 2.0],
                hzb_mips: hzb.levels.len() as u32,
                count: n,
                phase,
                capacity: self.capacity,
                occlusion: 1,
                near,
            };
            ctx.queue
                .write_buffer(&self.params[phase as usize], 0, bytemuck::bytes_of(&p));
        }
    }

    fn ensure_hzb(&mut self, ctx: &GpuContext, depth_size: (u32, u32)) {
        if self
            .hzb
            .as_ref()
            .is_some_and(|h| h.depth_size == depth_size)
        {
            return;
        }
        let w = depth_size.0.div_ceil(2).max(1);
        let h = depth_size.1.div_ceil(2).max(1);
        let mips = w.max(h).ilog2() + 1;
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hzb"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let levels: Vec<wgpu::TextureView> = (0..mips)
            .map(|l| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("hzb level"),
                    base_mip_level: l,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let all = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let down_binds = (1..mips as usize)
            .map(|l| {
                ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("hzb downsample"),
                    layout: &self.hzb_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&self.dummy_depth),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&levels[l - 1]),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&levels[l]),
                        },
                    ],
                })
            })
            .collect();
        self.hzb = Some(Hzb {
            texture,
            all,
            levels,
            depth_size,
            down_binds,
        });
        self.binds = None;
    }

    fn ensure_binds(&mut self, ctx: &GpuContext, instances: &wgpu::Buffer) {
        if self.binds.is_some() {
            return;
        }
        let hzb_view = self.hzb.as_ref().map(|h| &h.all).unwrap_or(&self.dummy_hzb);
        let make = |params: &wgpu::Buffer| {
            ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cull bind"),
                layout: &self.cull_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: params.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.slots.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.cands.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: instances.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: self.vis.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: self.draws.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: self.counts.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: wgpu::BindingResource::TextureView(hzb_view),
                    },
                ],
            })
        };
        self.binds = Some([make(&self.params[0]), make(&self.params[1])]);
    }

    /// Forces bind groups to be rebuilt (e.g. after the instance buffer was replaced).
    pub fn invalidate(&mut self) {
        self.binds = None;
    }

    /// Records a cull dispatch for `phase` (clearing the draw counters before phase 0).
    pub fn cull(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        instances: &wgpu::Buffer,
        phase: u32,
        count: u32,
    ) {
        self.ensure_binds(ctx, instances);
        if phase == 0 {
            enc.clear_buffer(&self.counts, 0, None);
        }
        if count == 0 {
            return;
        }
        let binds = self.binds.as_ref().expect("built above");
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("terrain cull"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.cull_pipe);
        pass.set_bind_group(0, &binds[phase as usize], &[]);
        pass.dispatch_workgroups(count.div_ceil(64), 1, 1);
    }

    /// Builds the Hi-Z pyramid from `depth` (after phase 0's opaque geometry).
    pub fn build_hzb(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        depth: &wgpu::TextureView,
    ) {
        let Some(hzb) = &self.hzb else {
            return;
        };
        let level0 = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hzb level 0"),
            layout: &self.hzb_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.dummy_hzb),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&hzb.levels[0]),
                },
            ],
        });
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("hzb build"),
            timestamp_writes: None,
        });
        let size = |l: u32| {
            let s = hzb.texture.size();
            ((s.width >> l).max(1), (s.height >> l).max(1))
        };
        pass.set_pipeline(&self.hzb_depth_pipe);
        pass.set_bind_group(0, &level0, &[]);
        let (w, h) = size(0);
        pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        pass.set_pipeline(&self.hzb_down_pipe);
        for (i, bind) in hzb.down_binds.iter().enumerate() {
            let (w, h) = size(i as u32 + 1);
            pass.set_bind_group(0, bind, &[]);
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }
    }

    /// Byte offset of a draw region in `draws`.
    pub fn region_offset(&self, phase: u32, pass: u32) -> u64 {
        (phase * 4 + pass) as u64 * self.capacity as u64 * DRAW_ARGS_SIZE
    }

    /// Reads the draw counters back (tests and screenshot verification only; stalls).
    pub fn read_counts(&self, ctx: &GpuContext) -> [u32; REGIONS as usize] {
        let staging = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cull counts readback"),
            size: 4 * REGIONS as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("counts readback"),
            });
        enc.copy_buffer_to_buffer(&self.counts, 0, &staging, 0, 4 * REGIONS as u64);
        ctx.queue.submit(Some(enc.finish()));
        let slice = staging.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = ctx.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        let mut out = [0u32; REGIONS as usize];
        if let Ok(data) = slice.get_mapped_range() {
            out.copy_from_slice(bytemuck::cast_slice(&data));
        }
        staging.unmap();
        out
    }
}

fn storage_buffer(device: &wgpu::Device, label: &str, size: u64, copy_src: bool) -> wgpu::Buffer {
    let mut usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
    if copy_src {
        usage |= wgpu::BufferUsages::COPY_SRC;
    }
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size.max(16),
        usage,
        mapped_at_creation: false,
    })
}

fn draw_buffer(device: &wgpu::Device, capacity: u32) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("cull draws"),
        size: REGIONS as u64 * capacity as u64 * DRAW_ARGS_SIZE,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::INDIRECT,
        mapped_at_creation: false,
    })
}

//! The near terrain drawn into the sun's shadow maps (`shadow.rs`): the caster lists, the
//! cascades' passes and bind groups, the depth-only pipelines.

use glam::{DVec3, Vec3};
use hearth_math::{CUBE_SIZE, CubePos, Direction};

use super::{DrawArgs, GpuMesh, Instance, Pass, TerrainRenderer, make_bind0, push_draws};
use crate::gpu::GpuContext;
use crate::shadow::{NEAR_CASCADES, SHADOW_FORMAT, ShadowMaps, ShadowQuality};

/// The depth-only pipelines drawing the near terrain into the shadow maps, by caster list.
pub(super) struct ShadowPipelines {
    packed: wgpu::RenderPipeline,
    packed_cutout: wgpu::RenderPipeline,
    general: wgpu::RenderPipeline,
    general_cutout: wgpu::RenderPipeline,
    smooth: wgpu::RenderPipeline,
}

impl TerrainRenderer {
    /// Sets the sun's shadows' quality (the maps made anew; bind group 0 with them).
    pub fn set_shadow_quality(&mut self, ctx: &GpuContext, quality: ShadowQuality) {
        if self.shadows.quality == quality {
            return;
        }
        let far = self.shadows.far_radius;
        self.shadows = ShadowMaps::new(ctx, quality);
        self.shadows.far_radius = far;
        self.bind0 = make_bind0(
            &ctx.device,
            &self.layout0,
            &self.globals,
            &self.bind0_parts,
            &self.water,
            &self.shadows,
            &self.shadows.array_view,
        );
        for (bind, globals) in self.shadow_binds.iter_mut().zip(&self.shadow_globals) {
            *bind = make_bind0(
                &ctx.device,
                &self.layout0,
                globals,
                &self.bind0_parts,
                &self.water,
                &self.shadows,
                &self.shadows.dummy_view,
            );
        }
    }

    /// The near cascades' caster lists for the cascades due this frame: every cube that can
    /// cast into one (those not drawn in the view given instances after theirs), its solid faces
    /// turned away from the light (the lit ones would only shade themselves), its plants and
    /// models whole (not into the widest cascade, whose texels are wider than their shadows),
    /// its smooth ground's faces turned away (culled in the pipeline). The cubes are looked up
    /// in the box about each cascade's caster region, not all of those loaded.
    pub(super) fn shadow_casters(&mut self, camera: DVec3, visible: &[(CubePos, Vec3)]) {
        for passes in &mut self.shadow_passes {
            for p in passes.iter_mut() {
                p.draws.clear();
            }
        }
        let mut due = [false; NEAR_CASCADES];
        for i in self.shadows.due().filter(|&i| i < NEAR_CASCADES) {
            due[i] = true;
        }
        let Some(light) = self.shadows.light() else {
            return;
        };
        if !due.contains(&true) {
            return;
        }
        let away: [bool; 6] =
            std::array::from_fn(|d| Direction::ALL[d].offset().as_vec3().dot(light) <= 0.0);
        // Each cube's instance by its slot: those in view have theirs already.
        self.slot_instance.clear();
        self.slot_instance.resize(self.next_slot as usize, u32::MAX);
        for (k, (pos, _)) in visible.iter().enumerate() {
            self.slot_instance[self.meshes[pos].slot as usize] = k as u32;
        }
        // Half a cube's diagonal: a cube whose middle lies this far outside a region may still
        // reach into it.
        let margin = CUBE_SIZE as f32 * 0.5 * 3f32.sqrt();
        for (i, &is_due) in due.iter().enumerate() {
            if !is_due {
                continue;
            }
            let mut caster = Caster {
                cascade: i,
                camera,
                away,
                models: i + 1 < NEAR_CASCADES,
                shadows: &self.shadows,
                planet: &self.planet,
                slot_instance: &mut self.slot_instance,
                instances: &mut self.instance_data,
                passes: &mut self.shadow_passes[i],
            };
            let (lo, hi) = self.shadows.reach_box(i, camera, margin);
            let c0 = CubePos::containing(camera + lo.as_dvec3());
            let c1 = CubePos::containing(camera + hi.as_dvec3());
            let span =
                (c1.x - c0.x + 1) as i64 * (c1.y - c0.y + 1) as i64 * (c1.z - c0.z + 1) as i64;
            if span >= self.meshes.len() as i64 {
                for (pos, m) in &self.meshes {
                    caster.add(*pos, m);
                }
            } else {
                for x in c0.x..=c1.x {
                    for z in c0.z..=c1.z {
                        for y in c0.y..=c1.y {
                            let pos = self.planet.wrap_cube(CubePos { x, y, z });
                            if let Some(m) = self.meshes.get(&pos) {
                                caster.add(pos, m);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Begins the depth-only pass drawing shadow cascade `i` (cleared to the far side).
    pub fn begin_shadow_pass<'e>(
        &self,
        enc: &'e mut wgpu::CommandEncoder,
        i: usize,
    ) -> wgpu::RenderPass<'e> {
        enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("shadow cascade"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.shadows.layer_views[i],
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }

    /// Bind group 0 as cascade `i` is drawn with: the frame's globals seen from the light.
    pub fn shadow_bind(&self, i: usize) -> &wgpu::BindGroup {
        &self.shadow_binds[i]
    }

    /// Records the near terrain's casters into near cascade `i` (in its pass).
    pub fn draw_shadow<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, i: usize) {
        let Some(passes) = self.shadow_passes.get(i) else {
            return;
        };
        pass.set_bind_group(0, &self.shadow_binds[i], &[]);
        pass.set_bind_group(1, &self.bind1, &[]);
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        let s = &self.shadow_pipes;
        for (pipe, p) in [
            (&s.packed, &passes[0]),
            (&s.general, &passes[2]),
            (&s.packed_cutout, &passes[1]),
            (&s.general_cutout, &passes[3]),
        ] {
            self.draw_list(pass, pipe, p);
        }
        pass.set_index_buffer(self.smooth_i.buffer.slice(..), wgpu::IndexFormat::Uint16);
        self.draw_list(pass, &s.smooth, &passes[4]);
    }
}

/// A near cascade's caster lists being made: what a cube casts into it.
struct Caster<'a> {
    cascade: usize,
    camera: DVec3,
    /// The face directions turned away from the light.
    away: [bool; 6],
    /// Plants and models cast too.
    models: bool,
    shadows: &'a ShadowMaps,
    planet: &'a hearth_math::Planet,
    slot_instance: &'a mut Vec<u32>,
    instances: &'a mut Vec<Instance>,
    passes: &'a mut [Pass; 5],
}

impl Caster<'_> {
    /// A cube's draws into the cascade, when it can cast there (given an instance if it is not
    /// drawn in the view).
    fn add(&mut self, pos: CubePos, m: &GpuMesh) {
        let side = Vec3::splat(CUBE_SIZE as f32);
        let o = self
            .planet
            .delta(self.camera, pos.min_block().as_dvec3())
            .as_vec3();
        if !self
            .shadows
            .may_cast(self.cascade, self.camera, o, o + side)
        {
            return;
        }
        let slot = m.slot as usize;
        if self.slot_instance[slot] == u32::MAX {
            self.slot_instance[slot] = self.instances.len() as u32;
            self.instances.push(Instance {
                origin: [o.x, o.y, o.z, 1.0],
            });
        }
        let inst = self.slot_instance[slot];
        let mut off = m.packed_off;
        for (li, counts) in m.counts.iter().enumerate() {
            for d in Direction::ALL {
                let n = counts[d.index()];
                if n > 0 && self.away[d.index()] {
                    push_draws(&mut self.passes[li].draws, off, n, inst);
                }
                off += n;
            }
        }
        if self.models {
            let mut goff = m.general_off;
            for li in 0..2 {
                let n = m.model_counts[li];
                if n > 0 {
                    push_draws(&mut self.passes[2 + li].draws, goff, n, inst);
                }
                goff += n;
            }
        }
        if m.smooth_indices > 0 {
            self.passes[4].draws.push(DrawArgs {
                index_count: m.smooth_indices,
                instance_count: 1,
                first_index: m.smooth_i_off * 2,
                base_vertex: m.smooth_v_off as i32,
                first_instance: inst,
            });
        }
    }
}

/// The depth state of everything drawn into the shadow maps: standard depth (0 nearest the
/// sun), pushed back a little by its slope so a surface does not shade itself.
pub fn shadow_depth_state() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: SHADOW_FORMAT,
        depth_write_enabled: Some(true),
        depth_compare: Some(wgpu::CompareFunction::Less),
        stencil: Default::default(),
        bias: wgpu::DepthBiasState {
            constant: 2,
            slope_scale: 1.5,
            clamp: 0.0,
        },
    }
}

pub(super) fn make_shadow_pipelines(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    layout0: &wgpu::BindGroupLayout,
    layout1: &wgpu::BindGroupLayout,
) -> ShadowPipelines {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("terrain shadow pipeline layout"),
        bind_group_layouts: &[Some(layout0), Some(layout1)],
        immediate_size: 0,
    });
    let make = |label: &str, vs: &str, fs: Option<&str>, cull: Option<wgpu::Face>| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
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
            depth_stencil: Some(shadow_depth_state()),
            multisample: Default::default(),
            fragment: fs.map(|fs| wgpu::FragmentState {
                module,
                entry_point: Some(fs),
                compilation_options: Default::default(),
                targets: &[],
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    // Solid faces are chosen by the way they face (`shadow_casters`); plants and models cast
    // from both sides; the smooth ground's faces turned toward the light are culled.
    ShadowPipelines {
        packed: make("shadow packed", "vs_packed", None, None),
        packed_cutout: make(
            "shadow packed cutout",
            "vs_packed",
            Some("fs_shadow_cutout"),
            None,
        ),
        general: make("shadow general", "vs_general", None, None),
        general_cutout: make(
            "shadow general cutout",
            "vs_general",
            Some("fs_shadow_cutout"),
            None,
        ),
        smooth: make(
            "shadow smooth ground",
            "vs_smooth",
            None,
            Some(wgpu::Face::Front),
        ),
    }
}

//! GPU occlusion culling must never change the image: render the same view with GPU culling
//! (three frames, so phase 0 reuses last frame's visibility) and with CPU culling, and compare.
//! Skipped when no adapter supports GPU culling.

use glam::DVec3;
use hearth::scene::LocalWorld;
use hearth_math::PlanetSize;
use hearth_render::GpuContext;
use hearth_render::atlas::TextureArray;
use hearth_render::camera::Camera;
use hearth_render::mesh::MeshOptions;
use hearth_render::models::BlockModels;
use hearth_render::offscreen::{OFFSCREEN_FORMAT, OffscreenTarget};
use hearth_render::scene::{Environment, SceneRenderer};

#[test]
fn gpu_culling_matches_cpu_culling() {
    let Ok(ctx) = GpuContext::headless(false) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let mut world = LocalWorld::create(11, PlanetSize::Tiny, 256, None).expect("world");
    let atlas = TextureArray::from_entries(&hearth_texgen::textures_for(Some(&world.content)));
    let (sx, sz) = world.terrain().find_spawn(false);
    let (x, z) = (sx as f64 + 0.5, sz as f64 + 0.5);
    let models = BlockModels::build(&world.reg, &atlas);
    let mut scene = SceneRenderer::new(&ctx, &atlas, OFFSCREEN_FORMAT, *world.map.planet(), 2, 1);
    if !scene.terrain.uses_gpu_culling() {
        eprintln!("skipped: adapter {} can't GPU-cull", ctx.info.name);
        return;
    }
    let (w, h) = (320, 180);
    let target = OffscreenTarget::new(&ctx, w, h);
    let env = Environment::default();
    // Several views: low in the terrain (lots of occlusion), and looking down from above.
    for (above, yaw, pitch) in [(1.7, 40.0, 5.0), (1.7, 220.0, -10.0), (40.0, 100.0, 45.0)] {
        let camera = Camera {
            pos: DVec3::new(x, world.surface_y(x, z) + above, z),
            yaw,
            pitch,
            fov_y: 70.0,
            near: 0.05,
        };
        let positions = world.load_area(camera.pos, 5, 1, None);
        for m in world.mesh(&models, &positions, MeshOptions::default()) {
            scene.terrain.upload(&ctx, &m);
        }
        scene.terrain.render_distance = 5;
        scene.terrain.vertical_distance = 16;
        let frame = |scene: &mut SceneRenderer| {
            scene.prepare(&ctx, &camera, (w, h), &env, f32::INFINITY);
            let mut enc = ctx
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("frame"),
                });
            scene.render(
                &ctx,
                &mut enc,
                &target.color_view,
                &target.depth.view,
                (w, h),
            );
            ctx.queue.submit(Some(enc.finish()));
            target.read_rgba(&ctx)
        };
        scene.terrain.gpu_culling = true;
        frame(&mut scene);
        frame(&mut scene);
        let gpu = frame(&mut scene);
        let counts = scene
            .terrain
            .read_gpu_draw_counts(&ctx)
            .expect("GPU culler");
        scene.terrain.gpu_culling = false;
        let cpu = frame(&mut scene);
        let differing = gpu
            .as_chunks::<4>()
            .0
            .iter()
            .zip(cpu.as_chunks::<4>().0)
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(
            differing, 0,
            "GPU culling changed {differing} pixels (view above={above} yaw={yaw})"
        );
        // Something must actually be on screen.
        assert!(scene.terrain.stats.visible_cubes > 0, "cubes in view");
        assert!(counts.iter().any(|&c| c > 0), "GPU culling drew something");
    }
}

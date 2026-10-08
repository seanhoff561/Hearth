//! The smooth ground drawn (Amendment S, S2): views of the generated world offscreen
//! (`bench-out/smooth_*.png`, to look at), the natural ground meshed through its fill and shaded
//! by its materials, and something of it on screen in each.

use glam::DVec3;
use hearth::scene::LocalWorld;
use hearth_math::PlanetSize;
use hearth_render::GpuContext;
use hearth_render::atlas::TextureArray;
use hearth_render::camera::Camera;
use hearth_render::mesh::MeshOptions;
use hearth_render::models::BlockModels;
use hearth_render::offscreen::{OFFSCREEN_FORMAT, OffscreenTarget, write_png};
use hearth_render::scene::{Environment, SceneRenderer};

#[test]
fn the_smooth_ground_is_drawn() {
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
    let ground = hearth::scene::ground_materials(&world.reg, &world.content).1;
    scene.terrain.set_ground_materials(&ctx, &ground);
    let (w, h) = (960, 540);
    let target = OffscreenTarget::new(&ctx, w, h);
    let env = Environment::default();
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench-out");
    std::fs::create_dir_all(&out).ok();
    for (k, (above, yaw, pitch)) in [(1.7, 40.0, 12.0), (6.0, 220.0, 25.0), (30.0, 100.0, 40.0)]
        .into_iter()
        .enumerate()
    {
        let camera = Camera {
            pos: DVec3::new(x, world.surface_y(x, z) + above, z),
            yaw,
            pitch,
            fov_y: 70.0,
            near: 0.05,
            jitter: glam::Vec2::ZERO,
        };
        let positions = world.load_area(camera.pos, 4, 1, None);
        let meshes = world.mesh(&models, &positions, MeshOptions::default());
        let smooth: usize = meshes.iter().map(|m| m.smooth.indices.len() / 3).sum();
        assert!(smooth > 1000, "{smooth} smooth triangles");
        for m in meshes {
            scene.terrain.upload(&ctx, &m);
        }
        scene.terrain.render_distance = 4;
        scene.terrain.vertical_distance = 8;
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
        let px = target.read_rgba(&ctx);
        write_png(&out.join(format!("smooth_{k}.png")), w, h, &px).expect("png");
        // Ground on screen: the lower half is not all sky.
        let lower = &px[(w * h * 2) as usize..];
        let varied = lower
            .as_chunks::<4>()
            .0
            .windows(2)
            .filter(|p| p[0] != p[1])
            .count();
        assert!(
            varied > 1000,
            "view {k}: the ground shows ({varied} changes)"
        );
    }
}

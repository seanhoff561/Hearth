//! GPU occlusion culling must never change the image: render the same view with GPU culling
//! (three frames, so phase 0 reuses last frame's visibility) and with CPU culling, and compare.
//! Skipped when no adapter supports GPU culling.

use glam::DVec3;
use hearth::scene::LocalWorld;
use hearth_math::PlanetSize;
use hearth_render::atlas::TextureArray;
use hearth_render::camera::Camera;
use hearth_render::mesh::MeshOptions;
use hearth_render::models::BlockModels;
use hearth_render::offscreen::{OFFSCREEN_FORMAT, OffscreenTarget};
use hearth_render::terrain::{FrameParams, TerrainRenderer};
use hearth_render::{GpuContext, LinearColor, render_terrain};

#[test]
fn gpu_culling_matches_cpu_culling() {
    let Ok(ctx) = GpuContext::headless(false) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let atlas = TextureArray::from_entries(&hearth_texgen::default_textures());
    let mut world = LocalWorld::create(11, PlanetSize::Tiny, 256, None).expect("world");
    let (sx, sz) = world.terrain().find_spawn(false);
    let (x, z) = (sx as f64 + 0.5, sz as f64 + 0.5);
    let models = BlockModels::build(&world.reg, &atlas);
    let mut terrain =
        TerrainRenderer::new(&ctx, &atlas, OFFSCREEN_FORMAT, *world.map.planet(), 2, 1);
    if !terrain.uses_gpu_culling() {
        eprintln!("skipped: adapter {} can't GPU-cull", ctx.info.name);
        return;
    }
    let (w, h) = (320, 180);
    let target = OffscreenTarget::new(&ctx, w, h);
    // Several views: low in the terrain (lots of occlusion), and looking down from above.
    for (above, yaw, pitch) in [(1.7, 40.0, 5.0), (1.7, 220.0, -10.0), (40.0, 100.0, 45.0)] {
        let camera = Camera {
            pos: DVec3::new(x, world.surface_y(x, z) + above, z),
            yaw,
            pitch,
            fov_y: 70.0,
            near: 0.05,
        };
        let positions = world.load_area(camera.pos, 5, 1);
        for m in world.mesh(&models, &positions, MeshOptions::default()) {
            terrain.upload(&ctx, &m);
        }
        terrain.render_distance = 5;
        terrain.vertical_distance = 16;
        let params = FrameParams {
            fog_start: 50.0,
            fog_end: 76.0,
            ..FrameParams::default()
        };
        let clear = LinearColor {
            r: 0.6,
            g: 0.7,
            b: 0.9,
            a: 1.0,
        };
        let frame = |terrain: &mut TerrainRenderer| {
            terrain.prepare(&ctx, &camera, (w, h), &params);
            render_terrain(&ctx, terrain, &target.color_view, &target.depth.view, clear);
            target.read_rgba(&ctx)
        };
        terrain.gpu_culling = true;
        frame(&mut terrain);
        frame(&mut terrain);
        let gpu = frame(&mut terrain);
        terrain.gpu_culling = false;
        let cpu = frame(&mut terrain);
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
        let sky = [153u8, 178, 229];
        let non_sky = cpu
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[..3] != sky)
            .count();
        assert!(non_sky > (w * h / 10) as usize, "terrain visible");
    }
}

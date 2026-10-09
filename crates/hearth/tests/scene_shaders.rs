//! Every pipeline of the world's scene builds without a validation error (a shader that does
//! not compile, or a vertex layout its shader does not match, fails here rather than when the
//! game starts). Skipped when no adapter exists.

use hearth::scene::LocalWorld;
use hearth_math::PlanetSize;
use hearth_render::GpuContext;
use hearth_render::atlas::TextureArray;
use hearth_render::offscreen::OFFSCREEN_FORMAT;
use hearth_render::scene::SceneRenderer;

#[test]
fn the_scenes_pipelines_build() {
    let Ok(ctx) = GpuContext::headless(false).or_else(|_| GpuContext::headless(true)) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let world = LocalWorld::create(11, PlanetSize::Tiny, 256, None).expect("world");
    let atlas = TextureArray::from_entries(&hearth_texgen::textures_for(Some(&world.content)));
    let scope = ctx.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut scene = SceneRenderer::new(&ctx, &atlas, OFFSCREEN_FORMAT, *world.map.planet(), 2, 1);
    // A tree's mesh and an instance of it, as the tree pass takes them.
    let sp = hearth_flora::Templates::from_content(&world.content);
    let oak = sp.index_of("hearth:english_oak").unwrap_or(0);
    let sk =
        hearth_flora::template::variant_skeleton(&sp.species[oak], hearth_flora::Stage::Young, 1);
    let mesh = hearth_flora::mesh::mesh(
        &sk,
        hearth_flora::mesh::Options::new(
            hearth_flora::mesh::Detail::Reduced,
            sp.species[oak].form.leaf,
            false,
            sp.species[oak].form.foliage_density,
        ),
        1,
    );
    scene.trees.upload(&ctx, 1, &mesh);
    let instance = hearth_render::trees::TreeInstance {
        origin: [0.0, -2.0, -12.0],
        bark: 0x40_5060,
        turn: [1.0, 0.0, 0.0, 1.0],
        leaf: 0x30_6020 | 2 << 24,
        climate: 0,
        light: 15,
        pad: 0,
    };
    scene
        .trees
        .prepare(&ctx, &[(1, vec![instance])], glam::Vec3::new(4.0, 0.0, 1.0));
    let err = pollster::block_on(scope.pop());
    assert!(err.is_none(), "{err:?}");
    assert_eq!(scene.trees.stats.instances, 1);
    assert!(scene.trees.stats.triangles > 1000);
}

/// The thing looked at glows about its own shape (T §2.3): a box highlighted before an empty
/// sky changes only the pixels just outside its silhouette (a line and a halo, never inside it,
/// never far off), and the patch of ground drawn as a disc changes nothing without ground.
#[test]
fn the_highlight_glows_about_its_own_shape() {
    use glam::{Affine3A, DVec3, Quat, Vec3};
    use hearth_render::camera::Camera;
    use hearth_render::offscreen::OffscreenTarget;
    use hearth_render::outline::Highlight;
    use hearth_render::scene::Environment;
    let Ok(ctx) = GpuContext::headless(false).or_else(|_| GpuContext::headless(true)) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let world = LocalWorld::create(11, PlanetSize::Tiny, 256, None).expect("world");
    let atlas = TextureArray::from_entries(&hearth_texgen::textures_for(Some(&world.content)));
    let mut scene = SceneRenderer::new(&ctx, &atlas, OFFSCREEN_FORMAT, *world.map.planet(), 2, 1);
    let (w, h) = (320u32, 180u32);
    let target = OffscreenTarget::new(&ctx, w, h);
    // High in the air, looking level: nothing but sky about the box.
    let camera = Camera {
        pos: DVec3::new(0.5, 900.0, 0.5),
        yaw: 0.0,
        pitch: 0.0,
        fov_y: 60.0,
        near: 0.05,
        jitter: glam::Vec2::ZERO,
    };
    let env = Environment::default();
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
    let plain = frame(&mut scene);
    // A box 0.4 m across, 2 m ahead (+Z), drawn only as the highlight.
    let place =
        Affine3A::from_scale_rotation_translation(Vec3::splat(0.4), Quat::IDENTITY, Vec3::Z * 2.0);
    let b = hearth_character::solid(place, [200, 200, 200], (15, 0));
    scene.highlight = Some(Highlight::Boxes(vec![b]));
    let lit = frame(&mut scene);
    // The box's silhouette: 0.2 m either side at 1.8 m (its near face), about the middle.
    let half = (0.2f32 / 1.8 / (30f32.to_radians()).tan() * h as f32 / 2.0).round() as i64;
    let (cx, cy) = (w as i64 / 2, h as i64 / 2);
    let mut changed = 0;
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            let i = ((y * w as i64 + x) * 4) as usize;
            let d = (0..3)
                .map(|k| (lit[i + k] as i32 - plain[i + k] as i32).abs())
                .max()
                .unwrap_or(0);
            if d <= 4 {
                continue;
            }
            changed += 1;
            // Outside the silhouette, within a few pixels of it.
            let (dx, dy) = ((x - cx).abs(), (y - cy).abs());
            let off = (dx - half).max(dy - half);
            assert!(
                (-1..=8).contains(&off),
                "changed at ({x}, {y}), {off} px from its edge"
            );
        }
    }
    assert!(
        changed > 4 * half as usize,
        "a line about it: {changed} pixels changed"
    );
    // The ground's patch where there is no ground: nothing.
    scene.highlight = Some(Highlight::Ground {
        centre: Vec3::new(0.0, -1.0, 2.0),
        radius: 0.6,
    });
    let none = frame(&mut scene);
    let moved = none
        .iter()
        .zip(&plain)
        .filter(|(a, b)| (**a as i32 - **b as i32).abs() > 4)
        .count();
    assert_eq!(moved, 0, "no ground, no patch");
}

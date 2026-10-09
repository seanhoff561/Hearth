//! The sun's shadows (R1a): a slab held above the ground in the sun shades the ground where its
//! shadow falls, and the open ground beside it is as lit as without the shadow maps (no surface
//! shades itself).

use glam::{DVec3, Vec3};
use hearth::scene::LocalWorld;
use hearth_character::FigureInstance;
use hearth_math::PlanetSize;
use hearth_render::GpuContext;
use hearth_render::atlas::TextureArray;
use hearth_render::camera::Camera;
use hearth_render::mesh::MeshOptions;
use hearth_render::models::BlockModels;
use hearth_render::offscreen::{OFFSCREEN_FORMAT, OffscreenTarget, write_png};
use hearth_render::scene::{Environment, SceneRenderer};
use hearth_render::shadow::ShadowQuality;

const W: u32 = 640;
const H: u32 = 400;

/// Mean luminance (0–1) of the pixels within `r` of a point on screen.
fn luma_at(px: &[u8], at: (i32, i32), r: i32) -> f64 {
    let (mut sum, mut n) = (0.0f64, 0.0f64);
    for y in at.1 - r..=at.1 + r {
        for x in at.0 - r..=at.0 + r {
            if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
                continue;
            }
            let i = ((y as u32 * W + x as u32) * 4) as usize;
            let c = |k: usize| px[i + k] as f64 / 255.0;
            sum += 0.2126 * c(0) + 0.7152 * c(1) + 0.0722 * c(2);
            n += 1.0;
        }
    }
    sum / n.max(1.0)
}

/// Where a point (camera-relative) falls on screen.
fn on_screen(camera: &Camera, p: Vec3) -> (i32, i32) {
    let c = camera.view_proj(W as f32 / H as f32) * p.extend(1.0);
    let (x, y) = (c.x / c.w, c.y / c.w);
    (
        ((x * 0.5 + 0.5) * W as f32) as i32,
        ((0.5 - y * 0.5) * H as f32) as i32,
    )
}

#[test]
fn a_box_in_the_sun_shades_the_ground_and_nothing_shades_itself() {
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
    scene.terrain.redraw_shadows = true;
    let env = Environment::default();
    let sun = env.sun_dir;
    // A slab 2 m across held 6 m over the ground, and where its middle's shadow falls (marched
    // down the sun's rays to the ground).
    let floor = |x: f64, z: f64| world.surface_y(x, z) + 1.0;
    let held = DVec3::new(x, floor(x, z) + 6.0, z);
    let mut shadow = held;
    for k in 0..4000 {
        shadow = held - sun.as_dvec3() * (k as f64 * 0.01);
        if shadow.y <= floor(shadow.x, shadow.z) {
            break;
        }
    }
    // Lit ground on the sun's side of the slab.
    let flat = Vec3::new(sun.x, 0.0, sun.z).normalize().as_dvec3();
    let open = DVec3::new(x, 0.0, z) + flat * 3.5;
    let open = DVec3::new(open.x, floor(open.x, open.z), open.z);
    // The camera above the shadow, looking down.
    let camera = Camera {
        pos: DVec3::new(shadow.x, shadow.y + 14.0, shadow.z),
        yaw: 30.0,
        pitch: 89.0,
        fov_y: 70.0,
        near: 0.05,
        jitter: glam::Vec2::ZERO,
    };
    let positions = world.load_area(camera.pos, 3, 1, None);
    for m in world.mesh(&models, &positions, MeshOptions::default()) {
        scene.terrain.upload(&ctx, &m);
    }
    scene.terrain.render_distance = 3;
    scene.terrain.vertical_distance = 8;
    let rel = |p: DVec3| (p - camera.pos).as_vec3();
    let c = rel(held);
    scene.figures.set(
        &ctx,
        &[FigureInstance {
            rows: [
                [2.0, 0.0, 0.0, c.x],
                [0.0, 0.3, 0.0, c.y],
                [0.0, 0.0, 2.0, c.z],
            ],
            color: [150, 120, 90, 255],
            light: [15, 0, 0, 0],
            pad: [0; 2],
        }],
    );
    let target = OffscreenTarget::new(&ctx, W, H);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench-out");
    std::fs::create_dir_all(&out).ok();
    let mut shot = |quality: ShadowQuality, name: &str| {
        scene.terrain.set_shadow_quality(&ctx, quality);
        scene.prepare(&ctx, &camera, (W, H), &env, f32::INFINITY);
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
            (W, H),
        );
        ctx.queue.submit(Some(enc.finish()));
        let px = target.read_rgba(&ctx);
        write_png(&out.join(name), W, H, &px).expect("png");
        px
    };
    let on = shot(ShadowQuality::Medium, "sun_shadows_on.png");
    let off = shot(ShadowQuality::Off, "sun_shadows_off.png");
    let s = on_screen(&camera, rel(shadow));
    let o = on_screen(&camera, rel(open));
    for p in [s, o] {
        assert!(
            (0..W as i32).contains(&p.0) && (0..H as i32).contains(&p.1),
            "{p:?} on screen"
        );
    }
    let (shade_on, shade_off) = (luma_at(&on, s, 4), luma_at(&off, s, 4));
    let (open_on, open_off) = (luma_at(&on, o, 4), luma_at(&off, o, 4));
    eprintln!("shadow {shade_on:.3} (without {shade_off:.3}); open {open_on:.3} ({open_off:.3})");
    // The sun is most of the light on open ground: its shadow is much darker.
    assert!(
        shade_on < 0.7 * shade_off,
        "the slab's shadow darkens the ground: {shade_on:.3} against {shade_off:.3}"
    );
    // The open ground is lit as it was (it does not shade itself).
    assert!(
        (open_on - open_off).abs() < 0.03 * open_off.max(0.05),
        "open ground unchanged: {open_on:.3} against {open_off:.3}"
    );
}

//! The sculpted, skinned bodies of E7 drawn offscreen (`bench-out/anatomy.png` to look at): four
//! people standing, then the same four mid-stride, to see the skin carried by the joints.

use glam::{Affine3A, Quat, Vec3};
use hearth_character::anatomy::anatomy;
use hearth_character::{Activity, Appearance, BodyType, Drive, Figure, HAIR_COLORS, HairStyle};
use hearth_render::GpuContext;
use hearth_render::body::{BodyPreview, GpuBody, SkinLook};
use hearth_render::figure::PreviewLight;
use hearth_render::offscreen::{OFFSCREEN_FORMAT, OffscreenTarget, write_png};

fn clear(enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
    let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: 0.05,
                    g: 0.06,
                    b: 0.08,
                    a: 1.0,
                }),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
}

#[test]
fn sculpted_bodies_stand_and_walk() {
    let Ok(ctx) = GpuContext::headless(false) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let people = [
        Appearance::default(),
        Appearance {
            hair: HairStyle::Coily,
            skin_tone: 0.85,
            hair_color: HAIR_COLORS[0].1,
            build: 0.8,
            ..Appearance::default()
        },
        Appearance {
            skin_tone: 0.1,
            undertone: -0.5,
            ..Appearance::female()
        },
        Appearance {
            body: BodyType::Female,
            height_m: 1.58,
            build: 0.9,
            skin_tone: 0.55,
            ..Appearance::female()
        },
    ];
    let (w, h) = (1280, 1440);
    let target = OffscreenTarget::new(&ctx, w, h);
    let mut preview = BodyPreview::new(&ctx, OFFSCREEN_FORMAT);
    let mut enc = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    clear(&mut enc, &target.color_view);
    ctx.queue.submit(Some(enc.finish()));
    for (k, a) in people.iter().enumerate() {
        let body = anatomy(a, 0.006);
        assert!(body.indices.len() > 30_000);
        let gpu = GpuBody::new(&ctx, &body);
        let mut f = Figure::starting(a.clone());
        let walk = Drive {
            activity: Activity::Walk,
            speed: 1.4,
            ..Drive::default()
        };
        let mut walking = f.animator.update(&f.rig, &walk, 0.05);
        for _ in 0..13 {
            walking = f.animator.update(&f.rig, &walk, 0.05);
        }
        let standing = f.animator.pose(&f.rig, Activity::Stand, &Drive::default());
        for (row, pose, turn) in [(0u32, &standing, 0.35f32), (1, &walking, 1.2)] {
            let place =
                Affine3A::from_rotation_translation(Quat::from_rotation_y(turn), Vec3::ZERO);
            let palette = gpu.palette(&f.rig, pose, place);
            let tall = a.height_m;
            let eye = Vec3::new(0.0, tall * 0.55, 3.2);
            let at = Vec3::new(0.0, tall * 0.5, 0.0);
            let mut enc = ctx
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            preview.render(
                &ctx,
                &mut enc,
                &target.color_view,
                (w, h),
                [k as u32 * 320, row * 720, 320, 720],
                &gpu,
                palette,
                SkinLook::of(a),
                (eye, at, 36.0),
                PreviewLight::Daylight,
            );
            ctx.queue.submit(Some(enc.finish()));
        }
    }
    let px = target.read_rgba(&ctx);
    for k in 0..4u32 {
        for row in 0..2u32 {
            let mut lit = 0;
            for y in (row * 720 + 150..row * 720 + 600).step_by(10) {
                for x in (k * 320 + 100..k * 320 + 220).step_by(10) {
                    let i = ((y * w + x) * 4) as usize;
                    if px[i] as u32 + px[i + 1] as u32 + px[i + 2] as u32 > 60 {
                        lit += 1;
                    }
                }
            }
            assert!(lit > 40, "body {k} row {row} barely drawn ({lit})");
        }
    }
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench-out");
    std::fs::create_dir_all(&out).ok();
    write_png(&out.join("anatomy.png"), w, h, &px).expect("png");
}

#[test]
fn close_up_face_hand_and_foot() {
    let Ok(ctx) = GpuContext::headless(false) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let a = Appearance::default();
    let body = anatomy(&a, 0.006);
    let gpu = GpuBody::new(&ctx, &body);
    let f = Figure::starting(a.clone());
    let pose = f.animator.pose(&f.rig, Activity::Stand, &Drive::default());
    let joints = pose.joints(&f.rig);
    let palette = gpu.palette(&f.rig, &pose, Affine3A::IDENTITY);
    use hearth_character::rig::Joint;
    let at = |j: Joint| Vec3::from(joints[j.index()].translation);
    let head = at(Joint::Head) + Vec3::new(0.0, 0.08, 0.0);
    let hand = at(Joint::WristL) + Vec3::new(0.0, -0.09, 0.0);
    let chest = at(Joint::Chest);
    let foot = at(Joint::AnkleL) + Vec3::new(0.0, -0.04, 0.08);
    let views = [
        (head + Vec3::new(0.0, 0.0, 0.6), head),
        (head + Vec3::new(0.45, 0.05, 0.4), head),
        (chest + Vec3::new(0.3, 0.1, 0.9), chest),
        (hand + Vec3::new(0.45, 0.05, 0.15), hand),
        (foot + Vec3::new(0.3, 0.3, 0.35), foot),
    ];
    let (w, h) = (1600, 360);
    let target = OffscreenTarget::new(&ctx, w, h);
    let mut preview = BodyPreview::new(&ctx, OFFSCREEN_FORMAT);
    let mut enc = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    clear(&mut enc, &target.color_view);
    ctx.queue.submit(Some(enc.finish()));
    for (k, (eye, look)) in views.into_iter().enumerate() {
        let mut enc = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        preview.render(
            &ctx,
            &mut enc,
            &target.color_view,
            (w, h),
            [k as u32 * 320, 0, 320, 360],
            &gpu,
            palette,
            SkinLook::of(&a),
            (eye, look, 30.0),
            PreviewLight::Daylight,
        );
        ctx.queue.submit(Some(enc.finish()));
    }
    let px = target.read_rgba(&ctx);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench-out");
    std::fs::create_dir_all(&out).ok();
    write_png(&out.join("anatomy_close.png"), w, h, &px).expect("png");
}

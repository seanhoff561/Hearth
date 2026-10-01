//! The character screen's preview drawn offscreen (`bench-out/character_*.png` to look at):
//! bodies of both kinds under each light, standing in the frame.

use glam::{Affine3A, Quat, Vec3};
use hearth_character::{
    Activity, Appearance, BodyType, Drive, EyeColor, FacialHair, Figure, HAIR_COLORS, HairStyle,
    Show, instances,
};
use hearth_render::GpuContext;
use hearth_render::figure::{FigurePreview, PreviewLight};
use hearth_render::offscreen::{OFFSCREEN_FORMAT, OffscreenTarget, write_png};

#[test]
fn bodies_stand_in_the_preview() {
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
            facial_hair: FacialHair::ShortBeard,
            ..Appearance::default()
        },
        Appearance {
            skin_tone: 0.1,
            undertone: -0.5,
            hair: HairStyle::LongWavy,
            hair_color: HAIR_COLORS[5].1,
            eyes: EyeColor::Green,
            ..Appearance::female()
        },
        Appearance {
            body: BodyType::Female,
            height_m: 1.58,
            build: 0.9,
            skin_tone: 0.55,
            hair: HairStyle::Braids,
            hair_color: HAIR_COLORS[1].1,
            ..Appearance::female()
        },
    ];
    let (w, h) = (1280, 720);
    let target = OffscreenTarget::new(&ctx, w, h);
    let mut preview = FigurePreview::new(&ctx, OFFSCREEN_FORMAT);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench-out");
    std::fs::create_dir_all(&out).ok();
    for light in PreviewLight::ALL {
        for (k, a) in people.iter().enumerate() {
            let mut enc = ctx
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            if k == 0 {
                // A dark backdrop.
                let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target.color_view,
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
            let f = Figure::new(a.clone());
            let pose = f.animator.pose(&f.rig, Activity::Stand, &Drive::default());
            let mut boxes = Vec::new();
            // Each turned a little toward the camera's right.
            let place =
                Affine3A::from_rotation_translation(Quat::from_rotation_y(0.35), Vec3::ZERO);
            instances(
                &f.rig,
                &f.palette,
                &pose,
                place,
                Show::default(),
                &mut boxes,
            );
            assert!(boxes.len() > 40);
            let x = k as u32 * 320;
            preview.render(
                &ctx,
                &mut enc,
                &target.color_view,
                (w, h),
                [x, 0, 320, 720],
                &boxes,
                1.9,
                light,
            );
            ctx.queue.submit(Some(enc.finish()));
        }
        let px = target.read_rgba(&ctx);
        // Something was drawn in each quarter, darker than the light and lighter than the
        // backdrop.
        for k in 0..4u32 {
            let mut lit = 0;
            for y in (200..600).step_by(10) {
                for x in (k * 320 + 100..k * 320 + 220).step_by(10) {
                    let i = ((y * w + x) * 4) as usize;
                    if px[i] as u32 + px[i + 1] as u32 + px[i + 2] as u32 > 60 {
                        lit += 1;
                    }
                }
            }
            assert!(lit > 50, "{light:?}: figure {k} barely drawn ({lit})");
        }
        let name = format!("character_{light:?}").to_lowercase();
        write_png(&out.join(format!("{name}.png")), w, h, &px).expect("png");
    }
}

//! The sculpted, skinned bodies of E7 drawn offscreen, to look at: `bench-out/anatomy.png` (four
//! people standing, then mid-stride), `anatomy_close.png` (face, chest, hand, foot) and
//! `hair_styles.png` (every style, with brows and beards).

use glam::{Affine3A, Quat, Vec3};
use hearth_character::anatomy::anatomy;
use hearth_character::hair::{HairSim, hair};
use hearth_character::rig::Joint;
use hearth_character::{
    Activity, Appearance, BodyType, Drive, Eyebrows, FacialHair, Figure, HAIR_COLORS, HairStyle,
    Pose,
};
use hearth_render::GpuContext;
use hearth_render::body::{BodyPreview, GpuBody, GpuHair, HairLook, Person, SkinLook};
use hearth_render::figure::PreviewLight;
use hearth_render::offscreen::{OFFSCREEN_FORMAT, OffscreenTarget, write_png};

fn clear(ctx: &GpuContext, view: &wgpu::TextureView) {
    let mut enc = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
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
    ctx.queue.submit(Some(enc.finish()));
}

/// A person ready to draw: skin and hair uploaded, the hair's guides settled in a pose.
struct Model {
    a: Appearance,
    figure: Figure,
    body: GpuBody,
    hair: GpuHair,
    sim: HairSim,
}

impl Model {
    fn new(ctx: &GpuContext, a: &Appearance, cell: f32) -> Self {
        let body = anatomy(a, cell);
        let mesh = hair(a);
        Self {
            a: a.clone(),
            figure: Figure::starting(a.clone()),
            body: GpuBody::new(ctx, &body),
            hair: GpuHair::new(ctx, &mesh),
            sim: HairSim::new(&mesh, a),
        }
    }

    /// Draws the model in `pose` placed by `place`, its hair settled over a second in `wind`.
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &mut self,
        ctx: &GpuContext,
        preview: &mut BodyPreview,
        target: &OffscreenTarget,
        size: (u32, u32),
        rect: [u32; 4],
        pose: &Pose,
        place: Affine3A,
        view: (Vec3, Vec3, f32),
        wind: Vec3,
    ) {
        let palette = self.body.palette(&self.figure.rig, pose, place);
        let head = glam::Mat4::from_cols_array_2d(&palette[Joint::Head.index()]);
        let head = Affine3A::from_mat4(head);
        for _ in 0..60 {
            self.sim.step(head, 1.0 / 60.0, wind, 0.0);
        }
        let person = Person {
            body: &self.body,
            palette,
            look: SkinLook::of(&self.a),
            hair: Some((
                &self.hair,
                HairLook {
                    color: self.a.hair_linear(),
                    wet: 0.0,
                    guides: self.sim.offsets(head),
                },
            )),
        };
        let mut enc = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        preview.render(
            ctx,
            &mut enc,
            &target.color_view,
            size,
            rect,
            &person,
            view,
            PreviewLight::Daylight,
        );
        ctx.queue.submit(Some(enc.finish()));
    }
}

#[test]
fn sculpted_bodies_stand_and_walk() {
    let Ok(ctx) = GpuContext::headless(false) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let people = [
        Appearance {
            facial_hair: FacialHair::ShortBeard,
            ..Appearance::default()
        },
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
            hair: HairStyle::LongWavy,
            hair_color: HAIR_COLORS[5].1,
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
    let (w, h) = (1280, 1440);
    let target = OffscreenTarget::new(&ctx, w, h);
    let mut preview = BodyPreview::new(&ctx, OFFSCREEN_FORMAT);
    clear(&ctx, &target.color_view);
    for (k, a) in people.iter().enumerate() {
        let mut model = Model::new(&ctx, a, 0.006);
        let walk = Drive {
            activity: Activity::Walk,
            speed: 1.4,
            ..Drive::default()
        };
        let f = &mut model.figure;
        let mut walking = f.animator.update(&f.rig, &walk, 0.05);
        for _ in 0..13 {
            walking = f.animator.update(&f.rig, &walk, 0.05);
        }
        let standing = f.animator.pose(&f.rig, Activity::Stand, &Drive::default());
        for (row, pose, turn) in [(0u32, &standing, 0.35f32), (1, &walking, 1.2)] {
            let place =
                Affine3A::from_rotation_translation(Quat::from_rotation_y(turn), Vec3::ZERO);
            let tall = a.height_m;
            let eye = Vec3::new(0.0, tall * 0.55, 3.2);
            let at = Vec3::new(0.0, tall * 0.5, 0.0);
            model.draw(
                &ctx,
                &mut preview,
                &target,
                (w, h),
                [k as u32 * 320, row * 720, 320, 720],
                pose,
                place,
                (eye, at, 36.0),
                Vec3::ZERO,
            );
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
    let a = Appearance {
        facial_hair: FacialHair::Stubble,
        ..Appearance::default()
    };
    let mut model = Model::new(&ctx, &a, 0.006);
    let f = &model.figure;
    let pose = f.animator.pose(&f.rig, Activity::Stand, &Drive::default());
    let joints = pose.joints(&f.rig);
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
    clear(&ctx, &target.color_view);
    for (k, (eye, look)) in views.into_iter().enumerate() {
        model.draw(
            &ctx,
            &mut preview,
            &target,
            (w, h),
            [k as u32 * 320, 0, 320, 360],
            &pose,
            Affine3A::IDENTITY,
            (eye, look, 30.0),
            Vec3::ZERO,
        );
    }
    let px = target.read_rgba(&ctx);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench-out");
    std::fs::create_dir_all(&out).ok();
    write_png(&out.join("anatomy_close.png"), w, h, &px).expect("png");
}

#[test]
fn every_hair_style() {
    let Ok(ctx) = GpuContext::headless(false) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let beards = [
        FacialHair::None,
        FacialHair::FullBeard,
        FacialHair::None,
        FacialHair::None,
        FacialHair::None,
        FacialHair::Goatee,
        FacialHair::None,
        FacialHair::None,
        FacialHair::Moustache,
        FacialHair::None,
        FacialHair::ShortBeard,
        FacialHair::None,
    ];
    let (w, h) = (1536, 640);
    let target = OffscreenTarget::new(&ctx, w, h);
    let mut preview = BodyPreview::new(&ctx, OFFSCREEN_FORMAT);
    clear(&ctx, &target.color_view);
    let styles: Vec<HairStyle> = HairStyle::ALL.into_iter().collect();
    for (k, style) in styles.iter().chain([&HairStyle::LongStraight]).enumerate() {
        let female = matches!(
            style,
            HairStyle::ShoulderLength
                | HairStyle::LongStraight
                | HairStyle::LongWavy
                | HairStyle::TiedBack
        );
        let base = if female {
            Appearance::female()
        } else {
            Appearance::default()
        };
        let a = Appearance {
            hair: *style,
            hair_color: HAIR_COLORS[k % HAIR_COLORS.len()].1,
            skin_tone: (k as f32 * 0.37) % 1.0,
            facial_hair: if female { FacialHair::None } else { beards[k] },
            eyebrows: Eyebrows::ALL[k % 4],
            ..base
        };
        // Coarser skin: only the head is looked at here.
        let mut model = Model::new(&ctx, &a, 0.008);
        let f = &model.figure;
        let pose = f.animator.pose(&f.rig, Activity::Stand, &Drive::default());
        let joints = pose.joints(&f.rig);
        let head = Vec3::from(joints[Joint::Head.index()].translation) + Vec3::new(0.0, 0.02, 0.0);
        // The last in a wind from the left, seen from behind.
        let windy = k == 11;
        let eye = if windy {
            head + Vec3::new(-0.5, 0.15, -0.9)
        } else {
            head + Vec3::new(0.55, 0.1, 0.85)
        };
        let wind = if windy {
            Vec3::new(6.0, 0.0, 0.0)
        } else {
            Vec3::ZERO
        };
        let x = (k % 6) as u32 * 256;
        let y = (k / 6) as u32 * 320;
        model.draw(
            &ctx,
            &mut preview,
            &target,
            (w, h),
            [x, y, 256, 320],
            &pose,
            Affine3A::IDENTITY,
            (eye, head - Vec3::new(0.0, 0.08, 0.0), 30.0),
            wind,
        );
    }
    let px = target.read_rgba(&ctx);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench-out");
    std::fs::create_dir_all(&out).ok();
    write_png(&out.join("hair_styles.png"), w, h, &px).expect("png");
}

/// One head, large, to look at closely: `HAIR_STYLE` (0–10) and `HAIR_BEARD` (0–5) choose.
#[test]
fn one_head() {
    let Ok(ctx) = GpuContext::headless(false) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let pick = |name: &str| {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0)
    };
    let a = Appearance {
        hair: HairStyle::ALL[pick("HAIR_STYLE") % HairStyle::ALL.len()],
        facial_hair: FacialHair::ALL[pick("HAIR_BEARD") % FacialHair::ALL.len()],
        hair_color: HAIR_COLORS[3].1,
        ..Appearance::default()
    };
    let mut model = Model::new(&ctx, &a, 0.006);
    let f = &model.figure;
    let pose = f.animator.pose(&f.rig, Activity::Stand, &Drive::default());
    let joints = pose.joints(&f.rig);
    let head = Vec3::from(joints[Joint::Head.index()].translation) + Vec3::new(0.0, 0.06, 0.0);
    let (w, h) = (1024, 512);
    let target = OffscreenTarget::new(&ctx, w, h);
    let mut preview = BodyPreview::new(&ctx, OFFSCREEN_FORMAT);
    clear(&ctx, &target.color_view);
    for (k, eye) in [Vec3::new(0.25, 0.08, 0.5), Vec3::new(-0.3, 0.25, -0.45)]
        .into_iter()
        .enumerate()
    {
        model.draw(
            &ctx,
            &mut preview,
            &target,
            (w, h),
            [k as u32 * 512, 0, 512, 512],
            &pose,
            Affine3A::IDENTITY,
            (head + eye, head, 30.0),
            Vec3::ZERO,
        );
    }
    let px = target.read_rgba(&ctx);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench-out");
    std::fs::create_dir_all(&out).ok();
    write_png(&out.join("hair_one.png"), w, h, &px).expect("png");
}

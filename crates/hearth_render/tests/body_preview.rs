//! The sculpted, skinned bodies of E7 drawn offscreen, to look at: `bench-out/anatomy.png` (four
//! people standing, then mid-stride), `anatomy_close.png` (face, chest, hand, foot) and
//! `hair_styles.png` (every style, with brows and beards).

use glam::{Affine3A, Quat, Vec3};
use hearth_character::person::{Detail, meshes};
use hearth_character::rig::Joint;
use hearth_character::{
    Activity, Appearance, BodyType, Drive, Eyebrows, FacialHair, Figure, HAIR_COLORS, HairStyle,
    Pose,
};
use hearth_render::GpuContext;
use hearth_render::body::{BodyPreview, PersonMeshes, PersonMotion, SkinLook, SkinState};
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

/// Whether a pixel differs from the backdrop's.
fn differs(c: &[u8], backdrop: &[u8]) -> bool {
    c.iter()
        .zip(backdrop)
        .map(|(a, b)| (*a as i32 - *b as i32).abs())
        .sum::<i32>()
        > 24
}

/// A person ready to draw: meshes uploaded, their motion running.
struct Model {
    a: Appearance,
    figure: Figure,
    meshes: PersonMeshes,
    motion: PersonMotion,
    /// How closed the lids are (overriding the blinks), for the close-ups.
    blink: Option<f32>,
    state: SkinState,
}

impl Model {
    fn new(ctx: &GpuContext, a: &Appearance, cell: f32) -> Self {
        let detail = if cell < 0.01 {
            Detail::Close
        } else {
            Detail::Near
        };
        let figure = Figure::starting(a.clone());
        let worn: Vec<&str> = figure.garbs.iter().map(|g| g.garment.as_str()).collect();
        let m = meshes(a, &worn, detail);
        Self {
            a: a.clone(),
            meshes: PersonMeshes::upload(ctx, &m),
            motion: PersonMotion::new(a, &m, 3),
            figure,
            blink: None,
            state: SkinState::default(),
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
        let palette = self.meshes.body.palette(&self.figure.rig, pose, place);
        let look = SkinLook {
            state: self.state,
            ..SkinLook::of(&self.a)
        };
        let mut frame = None;
        for _ in 0..60 {
            frame = Some(self.motion.frame(
                &self.a,
                palette,
                look,
                1.0 / 60.0,
                wind,
                glam::Vec2::ZERO,
                self.blink,
                [1.0, 0.0],
            ));
        }
        let frame = frame.expect("a frame");
        let mut enc = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        preview.render(
            ctx,
            &mut enc,
            &target.color_view,
            size,
            rect,
            &self.meshes,
            &frame,
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
                    if differs(&px[i..i + 3], &px[0..3]) {
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

#[test]
fn skin_states() {
    let Ok(ctx) = GpuContext::headless(false) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let a = Appearance {
        skin_tone: 0.15,
        hair: HairStyle::ShoulderLength,
        hair_color: HAIR_COLORS[4].1,
        ..Appearance::default()
    };
    let mut model = Model::new(&ctx, &a, 0.006);
    let joint = |j: Joint| j.index();
    let mut states = Vec::new();
    // Plain; wet; muddy legs and hands; sunburnt (but under the loincloth); a long tan;
    // pale, bleeding from the left forearm, a scar on the right thigh.
    states.push(SkinState::default());
    states.push(SkinState {
        wet: 1.0,
        ..SkinState::default()
    });
    let mut muddy = SkinState::default();
    for (j, d) in [
        (Joint::AnkleL, 1.0),
        (Joint::AnkleR, 1.0),
        (Joint::KneeL, 0.75),
        (Joint::KneeR, 0.75),
        (Joint::WristL, 0.8),
        (Joint::WristR, 0.8),
    ] {
        muddy.marks[joint(j)][0] = d;
    }
    states.push(muddy);
    let mut burnt = SkinState {
        sunburn: 0.9,
        ..SkinState::default()
    };
    burnt.marks[joint(Joint::Root)][3] = 1.0;
    states.push(burnt);
    let mut tanned = SkinState {
        tan: 0.8,
        ..SkinState::default()
    };
    tanned.marks[joint(Joint::Root)][3] = 1.0;
    states.push(tanned);
    let mut hurt = SkinState {
        pallor: 0.8,
        goosebumps: 1.0,
        ..SkinState::default()
    };
    hurt.marks[joint(Joint::ElbowL)][1] = 0.9;
    hurt.marks[joint(Joint::HipR)][2] = 1.0;
    states.push(hurt);
    let (w, h) = (1536, 720);
    let target = OffscreenTarget::new(&ctx, w, h);
    let mut preview = BodyPreview::new(&ctx, OFFSCREEN_FORMAT);
    clear(&ctx, &target.color_view);
    let f = &model.figure;
    let pose = f.animator.pose(&f.rig, Activity::Stand, &Drive::default());
    let place = Affine3A::from_rotation_y(0.3);
    for (k, state) in states.into_iter().enumerate() {
        model.state = state;
        let tall = a.height_m;
        model.draw(
            &ctx,
            &mut preview,
            &target,
            (w, h),
            [k as u32 * 256, 0, 256, 720],
            &pose,
            place,
            (
                Vec3::new(0.0, tall * 0.55, 3.0),
                Vec3::new(0.0, tall * 0.5, 0.0),
                36.0,
            ),
            Vec3::ZERO,
        );
    }
    let px = target.read_rgba(&ctx);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench-out");
    std::fs::create_dir_all(&out).ok();
    let lit = px.chunks(4).filter(|c| differs(&c[..3], &px[0..3])).count();
    assert!(lit > 50_000, "barely drawn: {lit}");
    write_png(&out.join("skin_states.png"), w, h, &px).expect("png");
}

/// The people's shaders compile and their pipelines build (a broken shader otherwise draws
/// nothing, silently).
#[test]
fn shaders_compile() {
    let Ok(ctx) = GpuContext::headless(false) else {
        return;
    };
    let scope = ctx.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let _p = BodyPreview::new(&ctx, OFFSCREEN_FORMAT);
    let err = pollster::block_on(scope.pop());
    assert!(err.is_none(), "{err:?}");
}

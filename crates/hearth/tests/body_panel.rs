//! The Body panel drawn offscreen (`bench-out/body_panel.png` to look at), for a body with a
//! splinted leg, a bandaged hand, a bruise and the cold on it.

use hearth::interface::Interface;
use hearth_body::{Body, BodyConfig, Rates, Side};
use hearth_content::Content;
use hearth_content::schema::body::BodyRegion;
use hearth_content::time::TimeScales;
use hearth_physics::Ability;
use hearth_protocol::BodyView;
use hearth_render::GpuContext;
use hearth_render::offscreen::{OFFSCREEN_FORMAT, OffscreenTarget, write_png};

#[test]
fn the_body_panel_names_and_draws_the_injuries() {
    let Ok(ctx) = GpuContext::headless(false) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let content = Content::load_base();
    let cfg = BodyConfig::with_rates(
        &content,
        Rates::authentic(),
        TimeScales::defaults(&content.time),
    );
    let mut body = Body::new(&cfg, 3);
    let leg = body
        .injure(&cfg, "fracture", BodyRegion::LowerLeg, Side::Left, 0.7)
        .expect("fracture");
    body.treat_injury(leg, "splint");
    body.treat_injury(leg, "bandage");
    let hand = body
        .injure(&cfg, "cut", BodyRegion::Hand, Side::Right, 0.3)
        .expect("cut");
    body.treat_injury(hand, "bandage");
    body.injure(&cfg, "bruise", BodyRegion::Chest, Side::Middle, 0.4)
        .expect("bruise");
    let view = BodyView {
        status: body.status(&cfg),
        ability: Ability::human(),
        asleep: false,
        lying: false,
        rate: 20.0,
        dead: None,
        injuries: body.injuries.clone(),
        illnesses: Vec::new(),
        exposure: hearth_body::Exposure::mild(),
    };
    let (w, h) = (1280, 720);
    let target = OffscreenTarget::new(&ctx, w, h);
    let mut iface = Interface::new("en_us");
    let mut enc = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    iface.frame(
        &ctx,
        &mut enc,
        &target.color_view,
        OFFSCREEN_FORMAT,
        (w, h),
        0,
        |ui| {
            ui.draw.rect(
                0.0,
                0.0,
                ui.size.0,
                ui.size.1,
                hearth_ui::Rgba([40, 60, 40, 255]),
            );
            hearth::body_panel::draw(ui, Some(&cfg), &view, 200);
        },
    );
    ctx.queue.submit(Some(enc.finish()));
    let px = target.read_rgba(&ctx);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench-out");
    std::fs::create_dir_all(&out).ok();
    write_png(&out.join("body_panel.png"), w, h, &px).expect("png");
    // Red where the injuries are: the left shin (on the viewer's right of the drawing).
    let reds = px
        .chunks(4)
        .filter(|p| p[0] > 150 && p[1] < 90 && p[2] < 90)
        .count();
    assert!(reds > 200, "injuries drawn ({reds} red pixels)");
}

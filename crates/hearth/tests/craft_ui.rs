//! The making screens drawn offscreen (`bench-out/journal.png`, `bench-out/knapping.png`,
//! `bench-out/offers.png`): the journal with discoveries, hunches and skills; a stone being
//! knapped by hand with a blow aimed; the list of what can be done by the crosshair.

use std::sync::Arc;

use hearth::crafting_ui::{Crafting, Do, News, Offer};
use hearth::interface::Interface;
use hearth::journal_ui::JournalView;
use hearth::knapping_ui::KnapScreen;
use hearth::menus::{MenuContext, Menus, Screen};
use hearth_content::Content;
use hearth_core::options::Options;
use hearth_craft::knap::{Aim, Knap};
use hearth_craft::{Crafts, Graph, KnowledgeState, Mode};
use hearth_input::{ActionRegistry, KeyBindings};
use hearth_items::Items;
use hearth_protocol::{AimAt, WorkView};
use hearth_render::GpuContext;
use hearth_render::offscreen::{OFFSCREEN_FORMAT, OffscreenTarget, write_png};

fn out_dir() -> std::path::PathBuf {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench-out");
    std::fs::create_dir_all(&out).ok();
    out
}

#[test]
fn the_making_screens_draw() {
    let Ok(ctx) = GpuContext::headless(false) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let content = Arc::new(Content::load_base());
    let items = Items::from_content(&content);
    let graph = Graph::from_content(&content);
    // A person who has learned a little and has a hunch or two.
    let mut k = KnowledgeState::default();
    for (t, tick) in [
        ("strike:stone", 100),
        ("strike:stone", 200),
        ("strike:stone", 300),
        ("strike:knappable", 300),
        ("strike:knappable", 400),
        ("strike:knappable", 500),
        ("strike:knappable", 600),
        ("use:flake", 70_000),
        ("twist:fibre", 80_000),
    ] {
        k.observe(&graph, t, tick, Mode::Discovery);
    }
    k.practice("knapping", 3.0, 600);
    k.first_made("hearth:flake/flint", "flint flake", 600);
    assert!(k.knows("hearth:sharp_flake"));

    let (w, h) = (1280u32, 720u32);
    let target = OffscreenTarget::new(&ctx, w, h);
    let mut iface = Interface::new("en_us");
    let mut options = Options::default();
    let mut bindings = KeyBindings::from_map(
        ActionRegistry::with_builtins(),
        &options.controls.key_bindings,
    );
    let mut profiles = hearth::profiles::Profiles::default();
    let languages = vec!["en_us".to_owned()];
    let draw = |iface: &mut Interface,
                menus: &mut Menus,
                options: &mut Options,
                bindings: &mut KeyBindings,
                profiles: &mut hearth::profiles::Profiles,
                name: &str| {
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
                let mut cx = MenuContext {
                    options,
                    bindings,
                    saves: std::env::temp_dir(),
                    in_game: false,
                    languages: &languages,
                    audio_devices: &[],
                    profiles,
                    death: None,
                    inventory: None,
                    journal: Some(JournalView {
                        knowledge: &k,
                        graph: &graph,
                        mode: Mode::Discovery,
                        ticks_per_day: 57_600.0,
                    }),
                };
                menus.ui(ui, &mut cx);
            },
        );
        ctx.queue.submit(Some(enc.finish()));
        let px = target.read_rgba(&ctx);
        write_png(&out_dir().join(name), w, h, &px).expect("png");
        px
    };
    // The journal, each tab.
    for (tab, name) in [
        (0u8, "journal.png"),
        (1, "journal_hunches.png"),
        (2, "journal_skills.png"),
    ] {
        let mut menus = Menus::none();
        menus.open(Screen::Journal { tab, scroll: 0 });
        let px = draw(
            &mut iface,
            &mut menus,
            &mut options,
            &mut bindings,
            &mut profiles,
            name,
        );
        assert!(px.chunks(4).any(|p| p[0] > 200), "something drawn");
    }
    // A flint stone half-knapped into a hand axe.
    let mut knap = Knap::new(Aim::HandAxe, 0.95, 3);
    for _ in 0..6 {
        let (at, inward) = knap
            .edge_near((knap.w as f32 * 0.85, knap.h as f32 * 0.2))
            .expect("edge");
        knap.strike(at, inward, 0.3);
    }
    let mut menus = Menus::none();
    menus.open(Screen::Knapping(Box::new(KnapScreen {
        knap,
        process: "hearth:make_hand_axe".into(),
        aim: AimAt::Nothing,
        title: "Shape a hand axe".into(),
        color: [70, 72, 78],
        press: None,
    })));
    draw(
        &mut iface,
        &mut menus,
        &mut options,
        &mut bindings,
        &mut profiles,
        "knapping.png",
    );

    // The list by the crosshair, and news.
    let crafts = Arc::new(Crafts::from_content(&content, &items));
    let mut c = Crafting::new(
        content.clone(),
        crafts,
        Arc::new(graph.clone()),
        Mode::Discovery,
    );
    c.offers = vec![
        Offer {
            words: "strike off a flake".into(),
            act: Some(Do::Process("hearth:strike_flake".into())),
            why: None,
            play_s: Some(4.0),
            material: Some("hearth:flint".into()),
        },
        Offer {
            words: "strike to test".into(),
            act: Some(Do::Process("hearth:test_nodule".into())),
            why: None,
            play_s: Some(3.0),
            material: Some("hearth:flint".into()),
        },
        Offer {
            words: "shape a hand axe".into(),
            act: None,
            why: Some("needs core".into()),
            play_s: None,
            material: None,
        },
    ];
    c.tell("Learned: Sharp flakes".into(), News::Learned);
    c.tell(
        "A chip flew off the flint, and its edge is keen.".into(),
        News::Hunch,
    );
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
                hearth_ui::Rgba([60, 80, 70, 255]),
            );
            c.draw(ui, 160)
        },
    );
    ctx.queue.submit(Some(enc.finish()));
    write_png(&out_dir().join("offers.png"), w, h, &target.read_rgba(&ctx)).expect("png");
    // Work under way.
    c.work = Some(WorkView {
        process: "hearth:make_hand_axe".into(),
        action: "thin and shape both faces".into(),
        done: 0.4,
        play_s_left: 52.0,
    });
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
                hearth_ui::Rgba([60, 80, 70, 255]),
            );
            c.draw(ui, 160)
        },
    );
    ctx.queue.submit(Some(enc.finish()));
    write_png(
        &out_dir().join("working.png"),
        w,
        h,
        &target.read_rgba(&ctx),
    )
    .expect("png");
}

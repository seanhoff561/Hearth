//! The screens drawn offscreen (to `bench-out/menu_*.png` to look at): each lays out within
//! the frame and answers clicks.

use hearth::interface::Interface;
use hearth::menus::{MenuAction, MenuContext, Menus, Screen};
use hearth_core::options::Options;
use hearth_input::{ActionRegistry, KeyBindings};
use hearth_render::GpuContext;
use hearth_render::offscreen::{OFFSCREEN_FORMAT, OffscreenTarget, write_png};

fn render(
    ctx: &GpuContext,
    iface: &mut Interface,
    menus: &mut Menus,
    options: &mut Options,
    bindings: &mut KeyBindings,
    name: &str,
) -> Vec<MenuAction> {
    let (w, h) = (1280, 720);
    let target = OffscreenTarget::new(ctx, w, h);
    let saves = std::env::temp_dir().join("hearth-menu-test-saves");
    let languages = vec!["en_us".to_owned()];
    let mut profiles = hearth::profiles::Profiles::default();
    let mut actions = Vec::new();
    let mut enc = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("menus"),
        });
    iface.frame(
        ctx,
        &mut enc,
        &target.color_view,
        OFFSCREEN_FORMAT,
        (w, h),
        0,
        |ui| {
            let mut cx = MenuContext {
                options,
                bindings,
                saves,
                in_game: false,
                languages: &languages,
                audio_devices: &[],
                profiles: &mut profiles,
                death: Some(hearth::menus::DeathInfo {
                    words: "You froze to death.".into(),
                    after_death: hearth_save::AfterDeath::TheirsOnly,
                    places: None,
                }),
                inventory: None,
                journal: None,
                eras: Vec::new(),
                modes: Vec::new(),
                globe: None,
                time_words: Some("Late afternoon, early autumn".into()),
                waiting: Some("Dry meat".into()),
                sleep_words: Some("You are not sleepy yet. Lying here, you would probably fall asleep in about 4 hours.".into()),
                may_watch: true,
                creative: true,
                catalog: &[],
                instant: true,
                clear_view: Default::default(),
            };
            actions = menus.ui(ui, &mut cx);
        },
    );
    ctx.queue.submit(Some(enc.finish()));
    let px = target.read_rgba(ctx);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench-out");
    std::fs::create_dir_all(&out).ok();
    write_png(&out.join(format!("menu_{name}.png")), w, h, &px).expect("png");
    actions
}

#[test]
fn the_screens_draw_and_answer() {
    let Ok(ctx) = GpuContext::headless(false) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let mut iface = Interface::new("en_us");
    let mut options = Options::default();
    let mut bindings = KeyBindings::from_map(
        ActionRegistry::with_builtins(),
        &options.controls.key_bindings,
    );
    let screens: Vec<(&str, Screen)> = vec![
        ("title", Screen::Title),
        (
            "worlds",
            Screen::Worlds {
                list: Vec::new(),
                selected: None,
                ask: None,
                note: None,
            },
        ),
        ("new_world", Screen::new_world()),
        ("options", Screen::Options),
        ("video", Screen::Video { tab: 0 }),
        ("sound", Screen::Sound),
        ("controls", Screen::controls()),
        ("pause", Screen::Pause),
        ("rest", Screen::Rest),
        ("death", Screen::Death),
        ("character_menu", Screen::Character(Default::default())),
    ];
    for (name, screen) in screens {
        let mut menus = Menus::none();
        let creator = matches!(screen, Screen::Character(_));
        menus.open(screen);
        render(
            &ctx,
            &mut iface,
            &mut menus,
            &mut options,
            &mut bindings,
            name,
        );
        // The creator shows its person within the frame, beside its rows.
        if creator {
            let p = menus.preview().expect("the creator's person");
            assert!(p.rect.w > 100.0 && p.rect.x + p.rect.w <= 640.0 && p.rect.h > 200.0);
            assert_eq!(p.appearance, hearth_character::Appearance::default());
        } else {
            assert!(menus.preview().is_none());
        }
    }
    // A click on the title's first button (Play) opens the worlds.
    let mut menus = Menus::title();
    // Auto interface scale at 720p is 2: the buttons sit at 48 % of 360 interface pixels.
    let (x, y) = (640.0, (360.0 * 0.48 + 9.0) * 2.0);
    iface.pointer_moved(x, y);
    iface.button(true);
    render(
        &ctx,
        &mut iface,
        &mut menus,
        &mut options,
        &mut bindings,
        "click1",
    );
    iface.button(false);
    render(
        &ctx,
        &mut iface,
        &mut menus,
        &mut options,
        &mut bindings,
        "click2",
    );
    render(
        &ctx,
        &mut iface,
        &mut menus,
        &mut options,
        &mut bindings,
        "after_click",
    );
    assert!(menus.is_open());
    // Escape on the worlds goes back to the title; on the title it does nothing.
    assert_eq!(menus.back(), None);
    assert_eq!(menus.back(), None);
}

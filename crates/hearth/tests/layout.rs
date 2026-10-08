//! The layout test (Amendment P §4.1): every screen laid out at every resolution, interface scale
//! and display mode, on the CPU. Every interactive widget must be reachable (on screen, or within
//! a scrolled area that is), none may overlap another, and no widget's text may be cut.

use hearth::menus::{MenuContext, Menus, Screen};
use hearth_core::options::Options;
use hearth_input::{ActionRegistry, KeyBindings, RebindCapture};
use hearth_ui::widgets::Placed;
use hearth_ui::{DrawList, Font, Rect, Ui, UiInput, UiState};

const RESOLUTIONS: [(f32, f32); 7] = [
    (1280.0, 720.0),
    (1600.0, 900.0),
    (1920.0, 1080.0),
    (2560.0, 1440.0),
    (3840.0, 2160.0),
    (2560.0, 1080.0),
    (1024.0, 768.0),
];

/// The screens, made afresh for each layout.
fn screens() -> Vec<(&'static str, Box<dyn Fn() -> Screen>)> {
    vec![
        ("title", Box::new(|| Screen::Title)),
        (
            "worlds",
            Box::new(|| Screen::Worlds {
                list: Vec::new(),
                selected: None,
                ask: None,
                note: None,
            }),
        ),
        ("new_world", Box::new(Screen::new_world)),
        (
            "making",
            Box::new(|| Screen::Making {
                stage: "Raising mountains…".into(),
                share: 0.4,
            }),
        ),
        (
            "birthplace",
            Box::new(|| Screen::Birthplace {
                choice: hearth::menus::NewWorldChoice {
                    folder: "Hearthstead".into(),
                    seed: 7,
                    era: hearth::eras::WILD_EARTH.into(),
                    size: hearth_math::PlanetSize::Standard,
                    shape: Default::default(),
                    mode: "hearth:realistic".into(),
                },
                chosen: None,
            }),
        ),
        (
            "new_world_more",
            Box::new(|| {
                let mut s = Screen::new_world();
                if let Screen::NewWorld { more, .. } = &mut s {
                    *more = true;
                }
                s
            }),
        ),
        ("options", Box::new(|| Screen::Options)),
        ("video_display", Box::new(|| Screen::Video { tab: 0 })),
        ("video_quality", Box::new(|| Screen::Video { tab: 1 })),
        ("video_distance", Box::new(|| Screen::Video { tab: 2 })),
        ("sound", Box::new(|| Screen::Sound)),
        (
            "controls",
            Box::new(|| Screen::Controls {
                capturing: None,
                capture: RebindCapture::new(),
            }),
        ),
        ("accessibility", Box::new(|| Screen::Accessibility)),
        ("conversation", Box::new(|| Screen::Conversation)),
        ("pause", Box::new(|| Screen::Pause)),
        (
            "time_weather",
            Box::new(|| Screen::TimeWeather(Default::default())),
        ),
        ("clear_view", Box::new(|| Screen::ClearView)),
        (
            "creative_animals",
            Box::new(|| {
                Screen::Creative(hearth::creative_ui::CreativeScreen {
                    tab: 2,
                    ..Default::default()
                })
            }),
        ),
        (
            "worlds_mode",
            Box::new(|| Screen::Worlds {
                list: vec![hearth::worlds::WorldInfo {
                    folder: "Hazel Valley".into(),
                    name: "Hazel Valley".into(),
                    era: "hearth:wild_earth".into(),
                    created_unix: 1_790_000_000,
                    last_played_unix: 1_790_100_000,
                    played_s: 7_200,
                    character: Some(("Ash".into(), 24.0)),
                    ended: false,
                    mode: Some("hearth:realistic".into()),
                    played_in_creative: false,
                }],
                selected: Some(0),
                ask: Some(hearth::menus::WorldsAsk::Mode(1)),
                note: None,
            }),
        ),
        (
            "creative",
            Box::new(|| Screen::Creative(Default::default())),
        ),
        ("death", Box::new(|| Screen::Death)),
        (
            "births",
            Box::new(|| Screen::Births {
                choices: (0..4)
                    .map(|i| hearth_protocol::BirthChoice {
                        title: format!("A family of the Kaanu ({i})"),
                        lines: vec![
                            "Your mother: Ama, 24 years old".into(),
                            "Your father: Tek, 29 years old".into(),
                            "Brothers and sisters: a sister of 4".into(),
                            "A band of 31, living in the savanna of the Afrotropical".into(),
                        ],
                    })
                    .collect(),
                selected: 0,
                born: hearth::profiles::Born::Chance,
            }),
        ),
        (
            "who_you_are",
            Box::new(|| Screen::WhoYouAre {
                lines: (0..12)
                    .map(|i| format!("A line of who you are now, the {i}th of a long briefing."))
                    .collect(),
            }),
        ),
        (
            "say",
            Box::new(|| Screen::Say {
                person: 1,
                whom: "Ama".into(),
                text: String::new(),
                options: Vec::new(),
            }),
        ),
        ("chronicle", Box::new(|| Screen::Chronicle { scroll: 0 })),
        ("born", Box::new(born)),
    ]
}

/// The birth screen of a birth in the tropics: the family as their genes made them.
fn born() -> Screen {
    use std::sync::OnceLock;
    static BORN: OnceLock<hearth_protocol::Born> = OnceLock::new();
    let shown = BORN.get_or_init(|| {
        let content = hearth_content::Content::load_base();
        let g = hearth_people::Genetics::from_content(&content).expect("the genetics");
        let b = hearth::born::draw(&g, 12.0, Some(true), 7).expect("a birth");
        let age = hearth::born::coming_of_age(&content);
        let you = hearth::born::player(&content, &b, "Ash", hearth_character::Loincloth::Hide, age);
        let family = hearth::born::household(&g, &b, age, 7);
        hearth::born::shown(&content, &b, &you, 12.0, Some(&family))
    });
    Screen::Born {
        born: Box::new(shown.clone()),
        sway: 0.0,
        light: 0,
    }
}

/// What a screen lays out at an interface size, after a frame to learn its scrolled areas.
fn lay_out(screen: &dyn Fn() -> Screen, size: (f32, f32)) -> Vec<Placed> {
    let font = Font::new();
    let lang = hearth::interface::lang();
    let mut options = Options::default();
    let mut bindings = KeyBindings::from_map(
        ActionRegistry::with_builtins(),
        &options.controls.key_bindings,
    );
    let mut profiles = hearth::profiles::Profiles::default();
    let languages = vec!["en_us".to_owned()];
    let mut menus = Menus::none();
    menus.open(screen());
    let mut state = UiState::recording();
    let input = UiInput::default();
    for _ in 0..2 {
        let mut draw = DrawList::new(1);
        let mut ui = Ui::begin(&font, &lang, &mut draw, &input, &mut state, size);
        let mut cx = MenuContext {
            options: &mut options,
            bindings: &mut bindings,
            saves: std::env::temp_dir().join("hearth-layout-test-saves"),
            in_game: true,
            languages: &languages,
            audio_devices: &[],
            profiles: &mut profiles,
            death: Some(hearth::menus::DeathInfo {
                words: "You froze to death.".into(),
                death: hearth_save::Death::default(),
                summary: None,
                story: vec![
                    "You lived 31 years.".into(),
                    "Your mother lives. Your father died before you.".into(),
                    "You had 2 children, 2 of them living.".into(),
                    "5 of your people mourn you.".into(),
                ],
                others: (0..6)
                    .map(|i| hearth_protocol::Other {
                        id: 11 + i,
                        words: format!("your kinsman, {} years", 20 + i),
                        family: true,
                        group: true,
                        near: true,
                        child: false,
                    })
                    .collect(),
            }),
            inventory: None,
            journal: None,
            eras: vec![(
                "hearth:upper_paleolithic".into(),
                "Upper Paleolithic".into(),
                "Modern people in bands, with blades, needles and art.".into(),
            )],
            modes: vec![
                ("hearth:creative".into(), "Creative".into(), "Build, explore and experiment freely. You can't be hurt, can fly, can place or summon anything, and can control time and weather.".into()),
                ("hearth:easy".into(), "Easy".into(), "The same world, more forgiving.".into()),
                ("hearth:realistic".into(), "Realistic".into(), "Life as it really is. Real needs, real dangers, real time. You only know what you learn, and when you die, life goes on through someone else.".into()),
            ],
            chronicle: (0..30)
                .map(|i| hearth_protocol::ChronicleEntry {
                    when: format!("Year {i}"),
                    text: "A band split in two".into(),
                    at: None,
                })
                .collect(),
            conversation_probe: None,
            conversation_models: Vec::new(),
            globe: None,
            time_words: Some("Late afternoon, the third day of autumn".into()),
                may_watch: true,
                creative: true,
                catalog: &[],
                instant: true,
                clear_view: Default::default(),
        };
        menus.ui(&mut ui, &mut cx);
    }
    state.layout.unwrap_or_default()
}

/// What is wrong with a layout in a frame of `size`.
fn faults(placed: &[Placed], size: (f32, f32)) -> Vec<String> {
    let frame = Rect::new(0.0, 0.0, size.0, size.1);
    let mut out = Vec::new();
    for p in placed {
        match p.area {
            None if !frame.holds(&p.rect) => {
                out.push(format!("“{}” off the window at {:?}", p.text, p.rect))
            }
            Some((a, ran)) => {
                if !frame.holds(&a) {
                    out.push(format!(
                        "“{}”'s scrolled area off the window: {a:?}",
                        p.text
                    ));
                }
                let span = Rect::new(a.x, a.y, a.w, ran.max(a.h));
                if !span.holds(&p.rect) {
                    out.push(format!(
                        "“{}” out of its scrolled area: {:?}",
                        p.text, p.rect
                    ));
                }
            }
            _ => {}
        }
        if p.text_w > p.rect.w - 2.0 {
            out.push(format!(
                "“{}” cut: {:.0} wide in {:.0}",
                p.text, p.text_w, p.rect.w
            ));
        }
    }
    for (i, a) in placed.iter().enumerate() {
        for b in &placed[i + 1..] {
            let same = match (a.area, b.area) {
                (None, None) => true,
                (Some((x, _)), Some((y, _))) => x == y,
                _ => false,
            };
            let hit = if same {
                a.rect.intersect(&b.rect)
            } else {
                // A widget outside a scrolled area must not lie over it.
                match (a.area, b.area) {
                    (None, Some((area, _))) => a.rect.intersect(&area),
                    (Some((area, _)), None) => b.rect.intersect(&area),
                    _ => None,
                }
            };
            if hit.is_some_and(|r| r.w * r.h > 1.0) {
                out.push(format!("“{}” overlaps “{}”", a.text, b.text));
            }
        }
    }
    out
}

#[test]
fn every_screen_fits_every_window() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for (name, screen) in screens() {
        for (w, h) in RESOLUTIONS {
            // Fullscreen, and a window with its title bar and the desktop's task bar.
            for (mode, (ww, wh)) in [("full", (w, h)), ("window", (w - 16.0, h - 72.0))] {
                for setting in 0..=4u32 {
                    let scale = hearth_ui::gui_scale(setting, wh as u32) as f32;
                    let size = ((ww / scale).floor(), (wh / scale).floor());
                    let placed = lay_out(screen.as_ref(), size);
                    assert!(!placed.is_empty(), "{name}: no widgets");
                    checked += 1;
                    for f in faults(&placed, size) {
                        failures.push(format!(
                            "{name} at {w}×{h} {mode}, scale {setting} ({}×{}): {f}",
                            size.0, size.1
                        ));
                    }
                }
            }
        }
    }
    failures.dedup();
    // A few of each screen's faults.
    let mut shown: Vec<String> = Vec::new();
    let mut per: std::collections::BTreeMap<&str, usize> = Default::default();
    for f in &failures {
        let name = f.split(' ').next().unwrap_or("");
        let n = per.entry(name).or_default();
        *n += 1;
        if *n <= 4 {
            shown.push(f.clone());
        }
    }
    assert!(
        failures.is_empty(),
        "{} faults in {checked} layouts ({per:?}):\n{}",
        failures.len(),
        shown.join("\n")
    );
}

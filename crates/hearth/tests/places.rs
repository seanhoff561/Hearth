//! Suggested places (Amendment E §6.3): a few, of different climates, far apart, survivable at
//! the starting date — and every claim on their cards true of the world about the spot,
//! checked again here from the world itself.

use std::sync::Arc;
use std::time::Instant;

use hearth::places::{Claim, Finder, Found, When};
use hearth_math::{BlockPos, PlanetSize};
use hearth_worldgen::{PlanetGrid, Terrain, WorldGenSettings, WorldGenerator};

fn finder() -> Finder {
    let s = WorldGenSettings {
        seed: 7,
        planet_size: PlanetSize::Standard,
        grid_resolution: 256,
    };
    let terrain = Arc::new(Terrain::new(Arc::new(PlanetGrid::build(&s, &|_, _| {}))));
    let reg = Arc::new(hearth_world::datapack::load_builtin_registry().expect("base pack"));
    let content = Arc::new(hearth_content::Content::load_base());
    let wg = Arc::new(WorldGenerator::new(terrain, &reg, &content).expect("generator"));
    Finder::new(wg, content, reg)
}

/// Where a claim points (world x, z).
fn spot(x: i32, z: i32, c: &Claim) -> (i32, i32) {
    let (dx, dz) = c.at.unwrap_or((0.0, 0.0));
    (x + dx.round() as i32, z + dz.round() as i32)
}

#[test]
fn suggested_places_are_varied_survivable_and_true() {
    let f = finder();
    let t0 = Instant::now();
    let places = f.suggest(When::Spring, 7);
    let took = t0.elapsed().as_secs_f64();
    println!("{} places in {took:.1} s", places.len());
    assert!((3..=5).contains(&places.len()), "{} places", places.len());
    assert!(took < 15.0, "suggesting took {took:.1} s");
    let wg = &f.wg;
    let lang = hearth::interface::lang();
    let mut climates = Vec::new();
    for p in &places {
        println!(
            "{} ({}) — {}; {}",
            hearth::places::words(&lang, &p.name),
            lang.get(p.difficulty.key()),
            hearth::places::words(&lang, &p.climate),
            hearth::places::words(&lang, &p.terrain),
        );
        for c in &p.look_for {
            println!("  + {}", hearth::places::claim_words(&lang, c));
        }
        for d in &p.watch_out {
            println!("  ! {}", hearth::places::danger_words(&lang, d));
        }
        let here = wg.terrain.sample(p.x, p.z);
        assert!(!here.is_underwater(), "a place on dry land");
        climates.push(here.climate);
        // Survivable at the date: no frost before dawn, no killing heat.
        assert!(
            p.low_c >= 2.0 && p.high_c <= 40.0,
            "{} – {}",
            p.low_c,
            p.high_c
        );
        // Fresh water first, and within reach.
        let water = &p.look_for[0];
        assert!(matches!(
            water.found,
            Found::River { .. } | Found::Lake | Found::Spring
        ));
        for c in &p.look_for {
            let (x, z) = spot(p.x, p.z, c);
            let x = wg.planet().wrap_x(x);
            match &c.found {
                Found::River { .. } => {
                    let s = wg.terrain.sample(x, z);
                    assert!(
                        s.river.is_some_and(|r| r.distance <= r.width * 0.5 + 2.0),
                        "a river where it says"
                    );
                }
                Found::Lake => {
                    let s = wg.terrain.sample(x, z);
                    assert!(s.lake && s.is_underwater(), "a lake where it says");
                }
                Found::Spring => {
                    let cell = hearth_worldgen::hydro::CELL;
                    let springs = wg.hydro.cell(wg, x.div_euclid(cell), z.div_euclid(cell));
                    assert!(
                        springs
                            .iter()
                            .any(|s| (s.x - x).abs() <= 1 && (s.z - z).abs() <= 1),
                        "a spring where it says"
                    );
                }
                Found::Sea => {}
                Found::Toolstone(rock) => {
                    // The stones lie on the ground there.
                    let s = wg.terrain.sample(x, z);
                    let y = s.height_i();
                    let found = [y - 1, y + 15].iter().any(|&cy| {
                        let cube = wg.generate_cube(BlockPos::new(x, cy, z).cube());
                        (0..hearth_math::CUBE_VOLUME).any(|i| {
                            let b = cube.get_index(i);
                            wg.blocks.is_loose_stone(b)
                        })
                    });
                    assert!(found, "{rock} stones on the ground where it says");
                }
                Found::Trees(names) => {
                    let trees = wg.features().trees_in(
                        wg,
                        &Default::default(),
                        (x - 20, z - 20),
                        (x + 20, z + 20),
                    );
                    assert!(!trees.is_empty(), "trees where it says ({names:?})");
                }
                Found::Fibre(n) | Found::Tinder(n) => assert!(!n.is_empty()),
                Found::Food(f) => assert!(!f.is_empty()),
            }
        }
    }
    // Different climates, far apart.
    climates.sort_by_key(|c| *c as u8);
    climates.dedup();
    assert!(climates.len() >= 3, "climates: {climates:?}");
}

/// The Birthplace screen with the places suggested on a world, drawn offscreen
/// (`bench-out/menu_birthplace_places.png`, to look at): the cards fit beside the globe.
#[test]
fn the_birthplace_screen_shows_the_places() {
    use hearth::menus::{MenuContext, Menus, Screen};
    use hearth_render::GpuContext;
    use hearth_render::offscreen::{OFFSCREEN_FORMAT, OffscreenTarget, write_png};
    let Ok(ctx) = GpuContext::headless(false) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let f = Arc::new(finder());
    let places = f.suggest(When::Spring, 7);
    let mut iface = hearth::interface::Interface::new("en_us");
    let mut options = hearth_core::options::Options::default();
    let mut bindings = hearth_input::KeyBindings::from_map(
        hearth_input::ActionRegistry::with_builtins(),
        &options.controls.key_bindings,
    );
    let mut profiles = hearth::profiles::Profiles::default();
    let mut picker = hearth::globe::GlobePicker::default();
    let mut menus = Menus::none();
    menus.open(Screen::Birthplace {
        choice: hearth::menus::NewWorldChoice {
            folder: "Hearthstead".into(),
            seed: 7,
            era: hearth::eras::WILD_EARTH.into(),
            size: PlanetSize::Standard,
            shape: Default::default(),
            mode: "hearth:realistic".into(),
        },
        chosen: None,
        shown: 1,
        anywhere: false,
        card: None,
    });
    let (w, h) = (1280, 720);
    let target = OffscreenTarget::new(&ctx, w, h);
    let languages = vec!["en_us".to_owned()];
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
                options: &mut options,
                bindings: &mut bindings,
                saves: std::env::temp_dir().join("hearth-places-test-saves"),
                in_game: false,
                languages: &languages,
                audio_devices: &[],
                profiles: &mut profiles,
                death: None,
                inventory: None,
                journal: None,
                eras: Vec::new(),
                modes: Vec::new(),
                globe: Some(hearth::menus::GlobeContext {
                    picker: &mut picker,
                    terrain: f.wg.terrain.clone(),
                    finder: Some(f.clone()),
                    places: &places,
                }),
                time_words: None,
                waiting: None,
                may_watch: false,
                creative: false,
                catalog: &[],
                instant: false,
                clear_view: Default::default(),
            };
            menus.ui(ui, &mut cx);
        },
    );
    ctx.queue.submit(Some(enc.finish()));
    let px = target.read_rgba(&ctx);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench-out");
    std::fs::create_dir_all(&out).ok();
    write_png(&out.join("menu_birthplace_places.png"), w, h, &px).expect("png");
}

/// On an Earth-sized planet (seed 7's grid as `bench relief` caches it; run by hand with
/// `--ignored`): suggesting adds at most some 15 s to making a world.
#[test]
#[ignore]
fn places_on_earth_take_under_15_s() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench-out/planets/planet_7_earth_2048.bin.zst");
    let Ok(grid) = PlanetGrid::load(&path) else {
        eprintln!("skipped: no cached Earth grid at {}", path.display());
        return;
    };
    let t0 = Instant::now();
    let terrain = Arc::new(Terrain::new(Arc::new(grid)));
    let reg = Arc::new(hearth_world::datapack::load_builtin_registry().expect("base pack"));
    let content = Arc::new(hearth_content::Content::load_base());
    let wg = Arc::new(WorldGenerator::new(terrain, &reg, &content).expect("generator"));
    let f = Finder::new(wg, content, reg);
    let made = t0.elapsed().as_secs_f64();
    let places = f.suggest(When::Spring, 7);
    let took = t0.elapsed().as_secs_f64();
    let lang = hearth::interface::lang();
    for p in &places {
        println!(
            "{} ({})",
            hearth::places::words(&lang, &p.name),
            lang.get(p.difficulty.key())
        );
        for c in &p.look_for {
            println!("  + {}", hearth::places::claim_words(&lang, c));
        }
        for d in &p.watch_out {
            println!("  ! {}", hearth::places::danger_words(&lang, d));
        }
    }
    println!(
        "generator {made:.1} s; {} places in {took:.1} s",
        places.len()
    );
    assert!((3..=5).contains(&places.len()));
    assert!(took < 15.0, "{took:.1} s");
}

/// Wild Earth has no people but players (Amendment E §6.1): no animal of the catalog is a
/// human or another hominin, and no content names one.
#[test]
fn no_humans_live_in_the_world() {
    let content = hearth_content::Content::load_base();
    let catalog = hearth_fauna::Catalog::new(&content);
    let human = |s: &str| {
        let s = s.to_lowercase();
        [
            "human",
            "homo ",
            "homo_",
            "hominin",
            "neanderthal",
            "erectus",
            "person",
            "people",
        ]
        .iter()
        .any(|w| s.contains(w))
    };
    for sp in &catalog.species {
        assert!(!human(&sp.id) && !human(&sp.name), "{}", sp.id);
    }
}

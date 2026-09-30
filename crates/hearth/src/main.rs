use std::path::PathBuf;

use hearth::LaunchConfig;
use hearth::screenshot::ShotSpec;
use hearth_core::paths::GameDirs;

/// Heap allocations are counted per thread (the benchmark checks the frame path).
#[global_allocator]
static ALLOCATOR: hearth::alloc_count::CountingAllocator = hearth::alloc_count::CountingAllocator;

const OPTIONS_HELP: &str = "\
COMMANDS:
    content lint               Validate all game data (schemas, references, reachability)
    content graph [--out DIR]  Export knowledge/process/food-web graphs (default docs/generated)
    content uncertain          List data entries marked for realism review
    content status             Implemented/planned counts per content domain
    bench [OPTIONS]            Rendering benchmark (see `hearth bench --help`)

OPTIONS:
    --game-dir <PATH>          Use PATH as the game directory (options, saves, screenshots)
    --seed <N>                 Seed of the preview world (default 1)
    --screenshot <SPEC>        Render a shot headlessly and exit (repeatable). SPEC is
                               comma-separated key=value: seed, planet, res, x, y, z, lat,
                               above, yaw, pitch, fov, w, h, dist, season, yf, hour, clouds,
                               snow (cover), rain|sleet|snowfall (mm/h), dry, out, software,
                               verify_cull
    --screenshot-list <FILE>   Render every shot listed in FILE (one SPEC per line)
    --software                 Prefer the software (CPU) adapter for screenshots
    --quit-after <SECONDS>     Exit cleanly after a delay (smoke tests)
    -h, --help                 Print this help";

fn print_help() {
    println!(
        "{} {}\n\nUSAGE:\n    hearth [OPTIONS]\n\n{OPTIONS_HELP}",
        hearth_core::GAME_NAME,
        hearth_core::GAME_VERSION
    );
}

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn,naga=warn"),
    )
    .init();

    let all: Vec<String> = std::env::args().skip(1).collect();
    if all.first().map(String::as_str) == Some("content") {
        std::process::exit(hearth::content_cli::run(&all[1..]));
    }
    if all.first().map(String::as_str) == Some("bench") {
        let dirs = hearth::resolve_dirs(None);
        std::process::exit(hearth::bench::run(&all[1..], Some(&dirs.cache())));
    }

    let mut config = LaunchConfig::default();
    let mut shots: Vec<ShotSpec> = Vec::new();
    let mut software = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--game-dir" => match args.next() {
                Some(p) => config.dirs = Some(GameDirs::new(PathBuf::from(p))),
                None => {
                    eprintln!("--game-dir needs a path");
                    std::process::exit(2);
                }
            },
            "--quit-after" => match args.next().and_then(|s| s.parse::<f64>().ok()) {
                Some(secs) if secs >= 0.0 => {
                    config.quit_after = Some(std::time::Duration::from_secs_f64(secs))
                }
                _ => {
                    eprintln!("--quit-after needs a number of seconds");
                    std::process::exit(2);
                }
            },
            "--screenshot" => match args.next().map(|s| ShotSpec::parse(&s)) {
                Some(Ok(spec)) => shots.push(spec),
                Some(Err(e)) => fail(&format!("bad --screenshot spec: {e}")),
                None => fail("--screenshot needs a spec, e.g. \"seed=1,yaw=30,out=shot.png\""),
            },
            "--screenshot-list" => match args.next() {
                Some(path) => match std::fs::read_to_string(&path)
                    .map_err(anyhow::Error::from)
                    .and_then(|t| ShotSpec::parse_list(&t))
                {
                    Ok(list) => shots.extend(list),
                    Err(e) => fail(&format!("cannot use shot list {path}: {e}")),
                },
                None => fail("--screenshot-list needs a file"),
            },
            "--software" => software = true,
            "--seed" => match args.next().and_then(|s| s.parse::<u64>().ok()) {
                Some(seed) => config.seed = Some(seed),
                None => fail("--seed needs a number"),
            },
            "-h" | "--help" => {
                print_help();
                return;
            }
            other => {
                eprintln!("unknown argument {other:?}; see --help");
                std::process::exit(2);
            }
        }
    }

    if !shots.is_empty() {
        for s in &mut shots {
            s.software |= software;
        }
        let dirs = hearth::resolve_dirs(config.dirs);
        if let Err(e) = hearth::screenshot::run(&shots, Some(&dirs.cache()), &dirs.screenshots()) {
            log::error!("{e:#}");
            eprintln!("screenshot failed: {e:#}");
            std::process::exit(1);
        }
        return;
    }

    if let Err(e) = hearth::run(config) {
        log::error!("{e:#}");
        eprintln!("{} crashed: {e:#}", hearth_core::GAME_NAME);
        std::process::exit(1);
    }
}

fn fail(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(2);
}

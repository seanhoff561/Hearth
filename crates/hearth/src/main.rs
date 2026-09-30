use std::path::PathBuf;

use hearth::LaunchConfig;
use hearth_core::paths::GameDirs;

fn print_help() {
    println!(
        "{name} {version}\n\nUSAGE:\n    hearth [OPTIONS]\n\nOPTIONS:\n    --game-dir <PATH>    Use PATH as the game directory (options, saves, screenshots)\n    -h, --help           Print this help",
        name = hearth_core::GAME_NAME,
        version = hearth_core::GAME_VERSION
    );
}

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn,naga=warn"),
    )
    .init();

    let mut config = LaunchConfig::default();
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

    if let Err(e) = hearth::run(config) {
        log::error!("{e:#}");
        eprintln!("{} crashed: {e:#}", hearth_core::GAME_NAME);
        std::process::exit(1);
    }
}

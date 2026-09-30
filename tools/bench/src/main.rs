//! Headless tools for Hearth: planet maps, benchmarks and screenshot drivers.

mod image;
mod worldmap;

fn usage() {
    println!(
        "bench <command> [options]\n\ncommands:\n  worldmap [--seed N] [--planet standard] [--res 1024] [--width 2048] [--out DIR] [--rarity rare|standard|common] [--slice lat=45] [--slice lon=-30]\n      Build the planet model and write Mercator/equirectangular PNG maps and slices."
    );
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        usage();
        return Ok(());
    };
    match cmd.as_str() {
        "worldmap" => worldmap::run(&args[1..]),
        "-h" | "--help" | "help" => {
            usage();
            Ok(())
        }
        other => {
            usage();
            anyhow::bail!("unknown command {other}")
        }
    }
}

//! Headless tools for Hearth: planet maps, benchmarks and screenshot drivers.

mod deposits;
mod r#gen;
mod image;
mod region;
mod smooth;
mod textures;
mod worldmap;

fn usage() {
    println!(
        "bench <command> [options]\n\ncommands:\n  worldmap [--seed N] [--planet earth] [--res 1024] [--width 2048] [--out DIR] [--slice lat=45] [--slice lon=-30]\n      Build the planet model and write Mercator/equirectangular PNG maps and slices.
  region [--seed N] [--res 1024] [--at X,Z] [--size 1024] [--scale 1] [--out FILE]
      Top-down block-resolution render of the surface around a point (default: spawn).
  deposits [--seed N] [--model ID] [--near X,Z] [--max-depth N] [--limit 10] [--coverage] [--springs] [--find BIOME|coral]
      List a world's deposit bodies (and springs) nearest a point, and the resource coverage of its
      continents.
  smooth [--out DIR] [--scenes a,b,...|shading] [--view 560x350] [--supersample 2] [--no-images]
      S0's smooth-terrain prototypes: the eight test scenes meshed by Surface Nets, Surface Nets
      with sharp features and Dual Contouring, measured and rendered; the shading prototype."
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
        "region" => region::run(&args[1..]),
        "gen" => r#gen::run(&args[1..]),
        "deposits" => deposits::run(&args[1..]),
        "textures" => textures::run(&args[1..]),
        "smooth" => smooth::run(&args[1..]),
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

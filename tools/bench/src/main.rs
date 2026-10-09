//! Headless tools for Hearth: planet maps, benchmarks and screenshot drivers.

mod deposits;
mod r#gen;
mod image;
mod realism;
mod region;
mod relief;
mod smooth;
mod textures;
mod worldmap;

fn usage() {
    println!(
        "bench <command> [options]\n\ncommands:\n  worldmap [--seed N] [--planet earth] [--res 1024] [--width 2048] [--out DIR] [--slice lat=45] [--slice lon=-30]\n      Build the planet model and write Mercator/equirectangular PNG maps and slices.
  hypso [--seed N] [--res 2048]
      An Earth grid's hypsometry against Earth's.
  region [--seed N] [--res 1024] [--at X,Z] [--size 1024] [--scale 1] [--out FILE]
      Top-down block-resolution render of the surface around a point (default: spawn).
  deposits [--seed N] [--model ID] [--near X,Z] [--max-depth N] [--limit 10] [--coverage] [--springs] [--find BIOME|coral]
      List a world's deposit bodies (and springs) nearest a point, and the resource coverage of its
      continents.
  relief [--seed N] [--res 2048] [--at X,Z] [--px 512] [--out DIR]
      Shaded relief maps of the Earth's refinement levels about a point (default: the highest
      uplift), with each level's time.
  smooth [--out DIR] [--scenes a,b,...|shading] [--view 560x350] [--supersample 2] [--no-images]
      S0's smooth-terrain prototypes: the eight test scenes meshed by Surface Nets, Surface Nets
      with sharp features and Dual Contouring, measured and rendered; the shading prototype.
  realism terrain|global|levels [--seed N] [--planet earth|standard] [--kinds hills,plains,...]
      The realism analysis (T §3.3, docs/review/realism/): the generator's ground beside real
      ground of the same kind (one-metre lidar, thirty-metre SRTM), over the whole land (random
      windows), and by refinement level; references from scripts/fetch-realism-refs.sh.
  realism weather | images DIR | sheet | photo-queries | photo-pick
      The weather's year at real places' normals; pictures' statistics; the suite's contact
      sheet beside its photographs (and the fetch script's two steps for them)."
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
        "relief" => relief::run(&args[1..]),
        "hypso" => relief::hypso(&args[1..]),
        "worldmap" => worldmap::run(&args[1..]),
        "region" => region::run(&args[1..]),
        "gen" => r#gen::run(&args[1..]),
        "deposits" => deposits::run(&args[1..]),
        "textures" => textures::run(&args[1..]),
        "smooth" => smooth::run(&args[1..]),
        "realism" => realism::run(&args[1..]),
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

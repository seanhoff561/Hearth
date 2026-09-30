//! `hearth content <command>`: tools for the data-driven content (v2 §3.2).
//!
//! * `lint` — load every pack, validate schemas, cross-references, reachability, habitats,
//!   food webs and units, and print the effort-from-scratch rollup. Exit code 1 on errors.
//!   `--coverage` also generates Standard worlds (`--seed N`, repeatable; default 1, 2, 3) and
//!   flags every Era 0–5 resource out of reach of a continent (v2 V2-2).
//! * `graph [--out DIR]` — write the knowledge graph, the process graph and the food webs as
//!   DOT, SVG and an HTML index (default `docs/generated`).
//! * `uncertain` — list entries marked `uncertain: true` for a realism review.
//! * `status` — implemented/planned counts per domain (the PROGRESS.md content table).
//!
//! `--data DIR` (repeatable) adds data packs after the base pack.

use std::path::PathBuf;

use hearth_content::{Content, LintContext, Severity, lint};
use hearth_worldgen::region::biome::Biome;

fn base_pack() -> PathBuf {
    crate::scene::data_pack_dir()
}

/// Runs a content subcommand; returns the process exit code.
pub fn run(args: &[String]) -> i32 {
    let mut packs = vec![base_pack()];
    let mut out = PathBuf::from("docs/generated");
    let mut command = None;
    let mut coverage = false;
    let mut seeds: Vec<u64> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--coverage" => coverage = true,
            "--seed" => match it.next().and_then(|s| s.parse().ok()) {
                Some(s) => seeds.push(s),
                None => {
                    eprintln!("--seed needs a number");
                    return 2;
                }
            },
            "--data" => match it.next() {
                Some(p) => packs.push(PathBuf::from(p)),
                None => {
                    eprintln!("--data needs a directory");
                    return 2;
                }
            },
            "--out" => match it.next() {
                Some(p) => out = PathBuf::from(p),
                None => {
                    eprintln!("--out needs a directory");
                    return 2;
                }
            },
            c if command.is_none() && !c.starts_with('-') => command = Some(c.to_owned()),
            other => {
                eprintln!("unknown argument {other:?}");
                return 2;
            }
        }
    }
    let Some(command) = command else {
        eprintln!(
            "usage: hearth content <lint|graph|uncertain|status> [--data DIR]... [--out DIR]"
        );
        return 2;
    };
    let (content, load_report) = Content::load(&packs);
    let print = |r: &hearth_content::Report, verbose: bool| {
        for d in r.sorted() {
            if verbose || d.severity != Severity::Info {
                println!("{d}");
            }
        }
    };
    let Some(content) = content else {
        print(&load_report, true);
        eprintln!("content failed to load ({} errors)", load_report.errors());
        return 1;
    };
    match command.as_str() {
        "lint" => {
            let ctx = LintContext {
                biomes: Some(Biome::ALL.iter().map(|b| b.name().to_owned()).collect()),
            };
            let mut report = load_report;
            report.extend(lint(&content, &ctx));
            if coverage {
                if seeds.is_empty() {
                    seeds = vec![1, 2, 3];
                }
                for &seed in &seeds {
                    if let Err(e) = coverage_report(&content, &packs, seed, &mut report) {
                        report.error("coverage", None, None, format!("seed {seed}: {e}"));
                    }
                }
            }
            print(&report, true);
            println!(
                "content lint: {} errors, {} warnings ({} packs)",
                report.errors(),
                report.warnings(),
                packs.len()
            );
            if report.errors() > 0 { 1 } else { 0 }
        }
        "graph" => {
            print(&load_report, false);
            match hearth_content::graph::write_all(&content, &out) {
                Ok(names) => {
                    println!(
                        "wrote {} graphs to {} (open index.html)",
                        names.len(),
                        out.display()
                    );
                    0
                }
                Err(e) => {
                    eprintln!("could not write graphs: {e}");
                    1
                }
            }
        }
        "uncertain" => {
            for (domain, id) in content.uncertain_entries() {
                println!("{domain}\t{id}");
            }
            0
        }
        "status" => {
            println!("| Domain | Implemented | Planned (data only) |");
            println!("|---|---|---|");
            for (domain, implemented, planned) in content.status_counts() {
                println!("| {domain} | {implemented} | {planned} |");
            }
            0
        }
        other => {
            eprintln!("unknown content command {other:?} (lint, graph, uncertain, status)");
            2
        }
    }
}

/// Generates a Standard world and reports the resources of Eras 0–5 that some continent
/// cannot reach: open gaps as warnings, gaps a substitute covers as notes.
fn coverage_report(
    content: &Content,
    packs: &[PathBuf],
    seed: u64,
    report: &mut hearth_content::Report,
) -> anyhow::Result<()> {
    use std::sync::Arc;
    let settings = hearth_worldgen::WorldGenSettings {
        seed,
        planet_size: hearth_math::PlanetSize::Standard,
        ..hearth_worldgen::WorldGenSettings::default()
    }
    .sanitized();
    let grid = hearth_worldgen::PlanetGrid::build(&settings, &|_, _| {});
    let terrain = Arc::new(hearth_worldgen::Terrain::new(Arc::new(grid)));
    let defs = hearth_world::datapack::load_block_defs(packs)?;
    let reg = hearth_world::BlockRegistry::build(defs).map_err(|e| anyhow::anyhow!(e))?;
    let generator = hearth_worldgen::WorldGenerator::new(terrain, &reg, content)?;
    let cov = hearth_worldgen::coverage::coverage(&generator, content, 5, 50.0);
    let where_ = |ci: usize| {
        let c = &cov.continents[ci];
        format!(
            "seed {seed}: continent {ci} ({:.0} km² around {}, {})",
            c.area_km2, c.center.0, c.center.1
        )
    };
    for gap in &cov.gaps {
        let msg = format!(
            "{} has no {} (era {}) within {:.0} km: 90% of it is {:.1} km away",
            where_(gap.continent),
            gap.resource,
            gap.era,
            gap.limit_km,
            gap.p90_km
        );
        match &gap.substitute {
            Some(s) => report.info("coverage", format!("{msg}; {s} stands in")),
            None => report.warning("coverage", None, None, msg),
        }
    }
    report.info(
        "coverage",
        format!(
            "seed {seed}: {} continents, {} gaps ({} open)",
            cov.continents.len(),
            cov.gaps.len(),
            cov.open_gaps().count()
        ),
    );
    Ok(())
}

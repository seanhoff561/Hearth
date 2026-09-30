//! `hearth content <command>`: tools for the data-driven content (v2 §3.2).
//!
//! * `lint` — load every pack, validate schemas, cross-references, reachability, habitats,
//!   food webs and units, and print the effort-from-scratch rollup. Exit code 1 on errors.
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
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
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

//! The game's loaded content and its hot reload (F3+T, v2 §3.2).
//!
//! Content is immutable once loaded and shared as `Arc<Content>`; reloading builds a fresh
//! `Content` from the same packs and swaps it in if it loads without errors. Systems that hold
//! derived data declare whether they can rebuild it live; those that can't are named in the
//! log so it's clear a restart (or world reload) is needed for their changes.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use hearth_content::{Content, LintContext, Report, Severity};
use hearth_worldgen::region::biome::Biome;

/// Systems built from content, and whether they can pick up a reload while running.
pub const RELOADABLE: &[(&str, bool)] = &[
    (
        "content tables (materials, species, processes, knowledge, balance, time)",
        true,
    ),
    (
        "block registry, textures and meshes (blocks still come from the v1 block pack until V2-2)",
        false,
    ),
    ("world generator", false),
];

pub struct ContentState {
    pub packs: Vec<PathBuf>,
    pub content: Option<Arc<Content>>,
    /// Bumped on every successful reload so systems can notice.
    pub generation: u64,
}

impl ContentState {
    pub fn load(packs: Vec<PathBuf>) -> Self {
        let mut s = Self {
            packs,
            content: None,
            generation: 0,
        };
        s.reload();
        s
    }

    /// Reloads all packs; keeps the previous content if the new one has errors.
    pub fn reload(&mut self) -> Report {
        let t0 = Instant::now();
        let (content, mut report) = Content::load(&self.packs);
        if let Some(c) = &content {
            report.extend(hearth_content::lint(
                c,
                &LintContext {
                    biomes: Some(Biome::ALL.iter().map(|b| b.name().to_owned()).collect()),
                },
            ));
        }
        for d in report.sorted() {
            match d.severity {
                Severity::Error => log::error!("{d}"),
                Severity::Warning => log::debug!("{d}"),
                Severity::Info => {}
            }
        }
        match content {
            Some(c) if report.errors() == 0 => {
                let counts: Vec<String> = c
                    .status_counts()
                    .iter()
                    .filter(|(_, i, p)| i + p > 0)
                    .map(|(d, i, p)| format!("{} {}", i + p, d.to_lowercase()))
                    .collect();
                log::info!(
                    "content {} in {:.0} ms: {} ({} warnings)",
                    if self.generation == 0 {
                        "loaded"
                    } else {
                        "reloaded"
                    },
                    t0.elapsed().as_secs_f64() * 1000.0,
                    counts.join(", "),
                    report.warnings()
                );
                self.content = Some(Arc::new(c));
                self.generation += 1;
                if self.generation > 1 {
                    for (system, live) in RELOADABLE {
                        if !live {
                            log::info!("not hot-reloadable, restart to apply: {system}");
                        }
                    }
                }
            }
            _ => log::error!(
                "content reload failed with {} errors; keeping the previous content",
                report.errors()
            ),
        }
        report
    }
}

//! `hearth history`: a world's deep past (V2.1 §15.1, H8) — its peoples at the era's date, its
//! lineages, what they know, and the chronicle — for reviewing an era without playing it.

use std::sync::Arc;

use hearth_content::Content;
use hearth_people::history::{Geography, Happening, History, Setup, deep};

/// A realm's name by its index.
pub fn realm_name(r: u8) -> &'static str {
    [
        "Palearctic",
        "Nearctic",
        "Afrotropical",
        "Indomalayan",
        "Neotropical",
        "Australasian",
        "Oceanian",
        "Antarctic",
    ]
    .get(r as usize)
    .copied()
    .unwrap_or("?")
}

fn short(id: &str) -> &str {
    id.rsplit(':').next().unwrap_or(id)
}

/// The chronicle's line for an event.
pub fn tell(h: &History, e: &hearth_people::history::Event, geo: Option<&Geography>) -> String {
    let sp = |k: u8| {
        h.species
            .get(k as usize)
            .map_or("?", |s| short(s))
            .replace('_', " ")
    };
    let what = match &e.what {
        Happening::Appeared { species } => format!("{} appears", sp(*species)),
        Happening::Reached { species, realm } => {
            format!("{} reaches the {}", sp(*species), realm_name(*realm))
        }
        Happening::Crossed {
            species, by_sea, ..
        } => {
            if *by_sea {
                format!("{} crosses the sea to a new land", sp(*species))
            } else {
                format!("{} walks over to a new land", sp(*species))
            }
        }
        Happening::Invented {
            species, technique, ..
        } => format!(
            "{} first know {}",
            sp(*species),
            h.techniques
                .get(*technique as usize)
                .map_or("?", |t| short(t))
                .replace('_', " ")
        ),
        Happening::Lost {
            species, technique, ..
        } => format!(
            "{} of a land lose {}",
            sp(*species),
            h.techniques
                .get(*technique as usize)
                .map_or("?", |t| short(t))
                .replace('_', " ")
        ),
        Happening::Split { lineage, daughter } => {
            format!("people {daughter} part from people {lineage}")
        }
        Happening::Met { a, b } => format!("{} and {} meet", sp(*a), sp(*b)),
        Happening::Left { species, realm } => {
            format!("{} leave the {}", sp(*species), realm_name(*realm))
        }
        Happening::Gone { species } => format!("the last {} die", sp(*species)),
    };
    let place = geo
        .and_then(|g| g.cells.get(e.cell as usize))
        .map(|c| format!(" ({})", realm_name(c.realm)))
        .unwrap_or_default();
    format!("{:>10.0} years ago  {what}{place}", e.ya)
}

/// Runs the command: `history [--seed N] [--era ID] [--size NAME] [--all]`.
pub fn run(args: &[String]) -> i32 {
    let mut seed = 1u64;
    let mut era = "upper_paleolithic".to_owned();
    let mut size = hearth_math::PlanetSize::Standard;
    let mut all = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => seed = it.next().and_then(|s| s.parse().ok()).unwrap_or(seed),
            "--era" => era = it.next().cloned().unwrap_or(era),
            "--size" => {
                if let Some(s) = it.next().and_then(|n| {
                    hearth_math::PlanetSize::PRESETS
                        .into_iter()
                        .find(|p| p.name() == n)
                }) {
                    size = s;
                }
            }
            "--all" => all = true,
            other => {
                eprintln!("unknown history option {other:?} (--seed, --era, --size, --all)");
                return 2;
            }
        }
    }
    let (content, report) = Content::load(&[crate::scene::data_pack_dir()]);
    let Some(content) = content else {
        for d in report.sorted() {
            eprintln!("{d}");
        }
        return 1;
    };
    let Some(e) = content.eras.iter().find(|e| short(&e.id) == short(&era)) else {
        eprintln!("no era {era:?}");
        return 2;
    };
    let graph = hearth_craft::Graph::from_content(&content);
    let Some(setup) = Setup::of_era(&content, &graph, e, seed) else {
        eprintln!("{} has no peoples to run through deep time", e.name);
        return 2;
    };
    let settings = hearth_worldgen::WorldGenSettings {
        seed,
        planet_size: size,
        ..hearth_worldgen::WorldGenSettings::default()
    }
    .sanitized();
    let t0 = std::time::Instant::now();
    let grid = hearth_worldgen::PlanetGrid::build(&settings, &|_, _| {});
    let terrain = Arc::new(hearth_worldgen::Terrain::new(Arc::new(grid)));
    let genetics = hearth_people::Genetics::from_content(&content);
    let sun = move |lat: f64| genetics.as_ref().map_or(1.0, |g| g.sunlight(lat));
    let geo = Geography::of_terrain(&terrain, &setup.settings, &sun);
    let made = t0.elapsed().as_secs_f64();
    let h = deep::run(&setup, &geo, &|_| {});
    println!(
        "{} on a {} planet (seed {seed}): {}×{} cells of {:.1} km; planet {made:.1} s, deep time {:.1} s; peoples {:.1}× as dense as real",
        e.name,
        size.name(),
        geo.n,
        geo.n,
        geo.cell_km(),
        t0.elapsed().as_secs_f64() - made,
        h.denser,
    );
    for (k, sp) in h.species.iter().enumerate() {
        let demes: Vec<_> = h.demes.iter().filter(|d| d.species as usize == k).collect();
        if demes.is_empty() {
            continue;
        }
        let mut lineages: Vec<u32> = demes.iter().map(|d| d.lineage).collect();
        lineages.sort_unstable();
        lineages.dedup();
        println!(
            "\n{}: {:.0} people in {} cells, {} peoples",
            short(sp).replace('_', " "),
            h.total(k),
            demes.len(),
            lineages.len()
        );
        let mut realms = [0.0f32; 8];
        for d in &demes {
            realms[geo.cells[d.cell as usize].realm as usize & 7] += d.people;
        }
        let by: Vec<String> = realms
            .iter()
            .enumerate()
            .filter(|(_, p)| **p > 0.0)
            .map(|(r, p)| format!("{} {p:.0}", realm_name(r as u8)))
            .collect();
        println!("  by realm: {}", by.join(", "));
        let n = demes.len() as f32;
        let mut known: Vec<(String, f32)> = h
            .techniques
            .iter()
            .enumerate()
            .map(|(i, t)| {
                (
                    short(t).replace('_', " "),
                    demes.iter().filter(|d| d.knows() & (1 << i) != 0).count() as f32 / n,
                )
            })
            .filter(|(_, s)| *s > 0.0)
            .collect();
        known.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let told: Vec<String> = known
            .iter()
            .map(|(t, s)| format!("{t} {:.0}%", s * 100.0))
            .collect();
        println!("  knows: {}", told.join(", "));
    }
    println!("\nCensus (people of each species):");
    for c in &h.census {
        if all || c.ya < 150_000.0 || (c.ya as i64) % 100_000 < 10_000 {
            let people: Vec<String> = c.people.iter().map(|p| format!("{p:>7.0}")).collect();
            println!(
                "{:>10.0} years ago  sea {:>5.0} m  {}",
                c.ya,
                c.sea_m,
                people.join(" ")
            );
        }
    }
    println!("\nChronicle:");
    let splits = h
        .chronicle
        .iter()
        .filter(|e| matches!(e.what, Happening::Split { .. }))
        .count();
    for e in &h.chronicle {
        if all || !matches!(e.what, Happening::Split { .. } | Happening::Lost { .. }) {
            println!("{}", tell(&h, e, Some(&geo)));
        }
    }
    println!("({splits} peoples parted; --all shows them and every technique lost)");
    0
}

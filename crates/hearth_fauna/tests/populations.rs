//! Populations (V2-7, docs/design/fauna.md): every species of a temperate wood holds within
//! plausible bounds over the decades, through food, predators, territories and cover; heavy
//! hunting thins the deer about a place, and they come back when it stops.

use std::sync::Arc;

use hearth_fauna::ecology::{Cause, Ecology, REGION_M};
use hearth_fauna::habitat::{Uniform, temperate_wood};
use hearth_fauna::species::Catalog;

fn catalog() -> Arc<Catalog> {
    Arc::new(Catalog::new(&hearth_content::Content::load_base()))
}

/// A uniform temperate wood of `n × n` regions.
fn wood(n: i64) -> Ecology {
    let cat = catalog();
    let land = Uniform {
        habitat: temperate_wood(&cat),
        cells_around: 4096,
    };
    let mut eco = Ecology::new(cat, 7, 0.0, &land);
    for j in 0..n {
        for i in 0..n {
            eco.ensure_region(&land, (i + 3, j), 0.0);
        }
    }
    eco
}

/// Each species' yearly means (of the counts at the end of each season) over `years`.
fn yearly(eco: &mut Ecology, species: &[usize], years: usize) -> Vec<Vec<f64>> {
    let mut out = vec![Vec::new(); species.len()];
    let start = eco.regions.values().map(|r| r.time).fold(0.0, f64::max);
    for y in 0..years {
        let mut sums = vec![0.0; species.len()];
        for q in 0..4 {
            eco.advance(start + y as f64 + (q + 1) as f64 / 4.0, 1.0 / 32.0);
            for (k, &s) in species.iter().enumerate() {
                sums[k] += eco.count(s) / 4.0;
            }
        }
        for (k, v) in sums.into_iter().enumerate() {
            out[k].push(v);
        }
    }
    out
}

#[test]
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn a_temperate_wood_holds_its_animals_for_decades() {
    let mut eco = wood(2);
    let cat = eco.catalog.clone();
    let present: Vec<usize> = (0..cat.len()).filter(|&s| eco.capacity(s) > 0.0).collect();
    let series = yearly(&mut eco, &present, 30);
    let mut failures = Vec::new();
    for (k, &s) in present.iter().enumerate() {
        let sp = &cat.species[s];
        let cap = eco.capacity(s);
        let v = &series[k];
        let tail = &v[v.len() / 3..];
        let mean = tail.iter().sum::<f64>() / tail.len() as f64;
        let (lo, hi) = tail
            .iter()
            .fold((f64::INFINITY, 0.0f64), |(a, b), x| (a.min(*x), b.max(*x)));
        println!(
            "{:<26} capacity {:>9.0}  mean {:>9.0} ({:>4.2})  range {:>9.0} .. {:>9.0}",
            sp.name,
            cap,
            mean,
            mean / cap,
            lo,
            hi
        );
        // Within a tenth and four times what the habitat holds, and never gone for long (a
        // rare predator may vanish a while and come back from the land beyond). A species the
        // wood holds fewer than five of (Wild Earth's wandering families, a person or so here)
        // is too few to judge by its mean.
        if cap < 5.0 {
            continue;
        }
        let gone = v.windows(8).any(|w| w.iter().all(|x| *x < 0.5));
        if mean < 0.1 * cap || mean > 4.0 * cap + 5.0 || gone {
            failures.push(format!("{}: mean {mean:.0} of {cap:.0}", sp.name));
        }
    }
    let mut deaths: Vec<_> = eco.deaths.iter().filter(|(_, n)| **n > 0.5).collect();
    deaths.sort_by(|a, b| a.0.cmp(b.0));
    for ((s, cause), n) in deaths {
        println!("  {:<26} {cause:?}: {n:.0}", cat.species[*s as usize].name);
    }
    assert!(failures.is_empty(), "out of bounds: {failures:?}");
}

#[test]
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn heavy_hunting_thins_the_deer_and_they_come_back() {
    let mut eco = wood(2);
    let deer = eco.catalog.index("red_deer").expect("red deer");
    // A hunting ground 10 km across in the middle of the four regions.
    let at = [4.0 * REGION_M, REGION_M];
    let radius = 5_000.0;
    let mut t = eco.regions.values().map(|r| r.time).fold(0.0, f64::max);
    let census = |eco: &mut Ecology, t: &mut f64, out: &mut Vec<f64>| {
        *t += 1.0;
        eco.advance(*t, 1.0 / 32.0);
        out.push(eco.count_within(deer, at, radius));
    };
    let mut before = Vec::new();
    for _ in 0..6 {
        census(&mut eco, &mut t, &mut before);
    }
    let base = before.iter().sum::<f64>() / before.len() as f64;
    // Six years of hunting half of them every year.
    let mut during = Vec::new();
    for _ in 0..6 {
        let n = (eco.count_within(deer, at, radius) * 0.5) as u32;
        assert!(eco.cull(deer, at, radius, n) > 0);
        census(&mut eco, &mut t, &mut during);
    }
    // Then it stops.
    let mut after = Vec::new();
    for _ in 0..18 {
        census(&mut eco, &mut t, &mut after);
    }
    println!("before {before:.0?}\nduring {during:.0?}\nafter {after:.0?}");
    let low = during.iter().copied().fold(f64::INFINITY, f64::min);
    let recovered = after[after.len() - 5..].iter().sum::<f64>() / 5.0;
    assert!(
        low < 0.5 * base,
        "hunting took them only to {low:.0} of {base:.0}"
    );
    assert!(
        recovered > 0.75 * base,
        "they came back only to {recovered:.0} of {base:.0}"
    );
    let hunted = eco.deaths.get(&(deer as u16, Cause::Hunting)).copied();
    assert!(hunted.unwrap_or(0.0) > 0.0);
}

//! H3 (V2.1 §7, §14.3): six bands of foragers twenty kilometres apart, lived two hundred years on their life
//! table land near the forager targets — a life expectancy at birth about thirty, most children
//! who outlive childhood living past sixty, four to six children to a woman who lives to
//! forty-five, three to four years between births — and the numbers about a home, once the land
//! there fills, hold near what it feeds.

mod common;

use common::*;
use glam::DVec3;
use hearth_body::{BodyConfig, Rates};
use hearth_content::TimeScales;
use hearth_people::person::{Event, Person};
use hearth_people::{Now, People, Species, SpeciesSet};

const YEARS: u32 = 200;

/// The species set with ours in it (its families come later in H3: data until then).
fn with_us() -> SpeciesSet {
    let b = base();
    let mut species = b.species.clone();
    let human = BodyConfig::with_rates(
        &b.content,
        Rates::authentic(),
        TimeScales::defaults(&b.content.time),
    );
    let us = b
        .content
        .species
        .iter()
        .find(|s| s.id.ends_with("homo_sapiens"))
        .expect("our species' profile");
    species
        .list
        .push(Species::of_profile(us, &b.graph, &human).with_routine(&b.content));
    species
}

#[test]
fn foragers_live_near_the_forager_targets() {
    let b = base();
    let species = with_us();
    let k = species.index_of("homo_sapiens").expect("our species");
    let mut w = Savanna::new();
    let mut p = People::new(11);
    let mut now = w.now();
    for (x, z) in [(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (2, 1)] {
        let at = DVec3::new(x as f64 * 20_000.0, GROUND, z as f64 * 20_000.0);
        p.spawn_band(
            &species,
            &b.graph,
            &b.items,
            &mut w,
            k,
            [9, 8, 10, 3],
            at,
            now,
        );
    }
    let start = now.day;
    let table = species.life.of("homo_sapiens").expect("a table");
    // How crowded the land is about the bands, as they feel it, on the whole.
    let felt = |p: &People| {
        let (mut sum, mut all) = (0.0, 0.0);
        for bi in 0..p.bands.len() {
            let n = p.members(p.bands[bi].id).filter(|q| q.alive()).count() as f64;
            sum += n * p.crowd(bi, table);
            all += n;
        }
        sum / all.max(1.0)
    };
    // How many live in the country the bands began in (and fifteen kilometres about it).
    let inside = |p: &People| {
        p.bands
            .iter()
            .filter(|b| (-15e3..55e3).contains(&b.home.x) && (-15e3..35e3).contains(&b.home.y))
            .map(|b| p.members(b.id).filter(|q| q.alive()).count())
            .sum::<usize>()
    };
    let mut crowding = Vec::new();
    let mut there = Vec::new();
    for year in 1..=YEARS {
        now = Now {
            day: start + year as f64 * YEAR_DAYS,
            ..now
        };
        p.live_course(&species, &b.items, &w, now);
        if year % 20 == 0 {
            let living = p.persons.iter().filter(|q| q.alive()).count();
            println!(
                "year {year}: {} bands, {living} living, {} in the first country, crowding {:.2}",
                p.bands.len(),
                inside(&p),
                felt(&p)
            );
            crowding.push(felt(&p));
            there.push(inside(&p) as f64);
        }
    }
    let years = |d: f64| d / YEAR_DAYS;
    let lived = |q: &Person| years(q.life.died.as_ref().map_or(now.day, |d| d.day) - q.life.born);
    // Those born from the thirtieth year to the hundredth, followed to their deaths.
    let cohort: Vec<&Person> = p
        .persons
        .iter()
        .filter(|q| (30.0..100.0).contains(&years(q.life.born - start)))
        .collect();
    let n = cohort.len() as f64;
    let e0 = cohort.iter().map(|q| lived(q)).sum::<f64>() / n;
    let l15 = cohort.iter().filter(|q| lived(q) >= 15.0).count() as f64 / n;
    let past15: Vec<&&Person> = cohort.iter().filter(|q| lived(q) >= 15.0).collect();
    let to60 = past15.iter().filter(|q| lived(q) >= 60.0).count() as f64 / past15.len() as f64;
    // Their women who lived to forty-five: how many they bore, how far apart.
    let bore = |q: &Person| -> Vec<f64> {
        q.life
            .events
            .iter()
            .filter(|e| matches!(e.event, Event::Bore { .. }))
            .map(|e| e.day)
            .collect()
    };
    let women: Vec<&&Person> = cohort
        .iter()
        .filter(|q| q.life.female && lived(q) >= 45.0)
        .collect();
    let ceb = women.iter().map(|q| bore(q).len()).sum::<usize>() as f64 / women.len() as f64;
    let gaps: Vec<f64> = women
        .iter()
        .flat_map(|q| {
            let mut d = bore(q);
            d.dedup();
            d.windows(2).map(|w| years(w[1] - w[0])).collect::<Vec<_>>()
        })
        .collect();
    let ibi = gaps.iter().sum::<f64>() / gaps.len() as f64;
    let living = p.persons.iter().filter(|q| q.alive()).count();
    println!(
        "{n} born: e0 {e0:.1}, l15 {l15:.2}, {to60:.2} of those to 60; {} women to 45 bore \
         {ceb:.2}, {ibi:.2} years apart; {} bands, {living} living",
        women.len(),
        p.bands.len()
    );
    assert!(n > 300.0, "too few born to judge");
    assert!(
        (27.0..37.0).contains(&e0),
        "life expectancy at birth {e0:.1}"
    );
    assert!((0.45..0.7).contains(&l15), "alive at fifteen {l15:.2}");
    assert!(to60 > 0.3, "of those past fifteen, to sixty {to60:.2}");
    assert!((4.0..6.5).contains(&ceb), "children to a woman {ceb:.2}");
    assert!((3.0..4.6).contains(&ibi), "years between births {ibi:.2}");
    // Neither died out nor overran the land.
    assert!(living > 150, "{living} living");
    for c in &crowding[crowding.len() - 3..] {
        assert!(*c < 1.4, "crowding {c:.2}");
    }
    // Once the country fills, its numbers grow no more (the last sixty years).
    let k = there.len();
    let growth = (there[k - 1] / there[k - 4]).ln() / 60.0;
    assert!(
        growth.abs() < 0.006,
        "growth {:.2} % a year",
        growth * 100.0
    );
}

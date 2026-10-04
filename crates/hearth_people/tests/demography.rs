//! H3 (V2.1 §7, §14.3): six bands of foragers twenty kilometres apart, lived two hundred years on their life
//! table land near the forager targets — a life expectancy at birth about thirty, most children
//! who outlive childhood living past sixty, four to six children to a woman who lives to
//! forty-five, three to four years between births — and the numbers about a home, once the land
//! there fills, hold near what it feeds. H8 (D198): the archaic peoples on theirs — dying younger,
//! weaned sooner, replacing themselves.

mod common;

use common::*;
use glam::DVec3;
use hearth_people::person::{Event, Person};
use hearth_people::{Now, People};

const YEARS: u32 = 200;

/// Two hundred years of six bands of a species twenty kilometres apart: the cohort born from the
/// thirtieth year to the hundredth followed to their deaths, and the bands at the end.
struct Lived {
    /// Born in the cohort; their life expectancy at birth; the share alive at fifteen; of those,
    /// the share to sixty.
    n: f64,
    e0: f64,
    l15: f64,
    to60: f64,
    /// Its women who lived to forty-five: how many, the children they bore, years between births.
    women: usize,
    ceb: f64,
    ibi: f64,
    /// The living at the end, in how many bands; those with a grandmother born in the run.
    living: usize,
    bands: usize,
    grand: usize,
    /// How crowded the bands felt each twentieth year, and how many lived in the first country.
    crowding: Vec<f64>,
    there: Vec<f64>,
}

fn lived(species_id: &str, seed: u64, founders: [u16; 4]) -> Lived {
    let b = base();
    let species = b.species.clone();
    let k = species.index_of(species_id).expect("the species");
    let mut w = Savanna::new();
    let mut p = People::new(seed);
    let mut now = w.now();
    for (x, z) in [(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (2, 1)] {
        let at = DVec3::new(x as f64 * 20_000.0, GROUND, z as f64 * 20_000.0);
        p.spawn_band(&species, &b.graph, &b.items, &mut w, k, founders, at, now);
    }
    let start = now.day;
    let table = species.life.of(species_id).expect("a table");
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
    // Families persist across generations: the living have grandmothers born in the run.
    let grand = p
        .persons
        .iter()
        .filter(|q| q.alive())
        .filter(|q| {
            q.life
                .mother
                .and_then(|m| p.get(m))
                .and_then(|m| m.life.mother)
                .and_then(|g| p.get(g))
                .is_some_and(|g| g.life.born > start)
        })
        .count();
    let out = Lived {
        n,
        e0,
        l15,
        to60,
        women: women.len(),
        ceb,
        ibi,
        living,
        bands: p.bands.len(),
        grand,
        crowding,
        there,
    };
    println!(
        "{species_id}: {} born: e0 {:.1}, l15 {:.2}, {:.2} of those to 60; {} women to 45 bore \
         {:.2}, {:.2} years apart; {} bands, {} living; {} living with a grandmother born in the \
         run",
        out.n,
        out.e0,
        out.l15,
        out.to60,
        out.women,
        out.ceb,
        out.ibi,
        out.bands,
        out.living,
        out.grand
    );
    out
}

/// Once the country fills, its numbers grow no more (the last sixty years).
fn growth(there: &[f64]) -> f64 {
    let k = there.len();
    (there[k - 1] / there[k - 4]).ln() / 60.0
}

#[test]
fn foragers_live_near_the_forager_targets() {
    let Lived {
        n,
        e0,
        l15,
        to60,
        ceb,
        ibi,
        living,
        grand,
        crowding,
        there,
        ..
    } = lived("homo_sapiens", 11, [9, 8, 10, 3]);
    assert!(grand > 20, "three generations and more");
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
    let g = growth(&there);
    assert!(g.abs() < 0.006, "growth {:.2} % a year", g * 100.0);
}

/// H8 (D198): *Homo erectus* and the Neanderthals on their own tables — a life expectancy at birth
/// near twenty, few past sixty, births closer than ours by an earlier weaning — keep going through
/// three generations and more, their numbers held by what their thin country feeds (the founders
/// are more than it feeds at their tables' three and two and a half to the hundred km², so their
/// numbers fall toward it).
#[test]
fn the_archaic_peoples_live_and_die_by_their_tables() {
    for (species, seed, founders, e0_near, spacing) in [
        ("homo_erectus", 12, [9, 8, 10, 3], 21.0, 2.4..4.2),
        ("homo_neanderthalensis", 13, [6, 5, 7, 3], 20.0, 2.2..4.0),
    ] {
        let l = lived(species, seed, founders);
        assert!(l.n > 150.0, "{species}: too few born to judge");
        assert!(
            (e0_near - 5.0..e0_near + 5.0).contains(&l.e0),
            "{species}: life expectancy at birth {:.1}",
            l.e0
        );
        assert!(
            l.to60 < 0.3,
            "{species}: of those past fifteen, to sixty {:.2}",
            l.to60
        );
        assert!(
            spacing.contains(&l.ibi),
            "{species}: years between births {:.2}",
            l.ibi
        );
        assert!(l.grand > 10, "{species}: three generations and more");
        assert!(l.living >= 30, "{species}: {} living", l.living);
        for c in &l.crowding[l.crowding.len() - 3..] {
            assert!(*c < 1.6, "{species}: crowding {c:.2}");
        }
    }
}

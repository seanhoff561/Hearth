//! H8 (V2.1 §15.1): deep time over a planet's own geography — peoples spreading from the cradle,
//! the sea crossed only within reach, a land bridge opened by the ice ages, the few and cut off
//! losing what the many keep, lineages making a tree, the same seed the same history.

mod common;

use std::sync::Arc;

use common::*;
use hearth_content::schema::history::HistorySettings;
use hearth_people::history::geo::{GeoCell, SAMPLES};
use hearth_people::history::{Geography, Happening, History, Kind, Setup, Technique, deep};
use hearth_worldgen::region::Terrain;
use hearth_worldgen::{PlanetGrid, WorldGenSettings};

fn settings() -> HistorySettings {
    base()
        .content
        .history
        .iter()
        .next()
        .expect("the history settings")
        .clone()
}

/// A Standard planet (the default world size) on a coarse planet grid.
fn planet(seed: u64) -> Arc<Terrain> {
    let s = WorldGenSettings {
        seed,
        grid_resolution: 256,
        ..WorldGenSettings::default()
    };
    Arc::new(Terrain::new(Arc::new(PlanetGrid::build(&s, &|_, _| {}))))
}

fn sun(lat: f64) -> f32 {
    base()
        .species
        .genetics
        .as_ref()
        .map_or(1.0, |g| g.sunlight(lat))
}

fn era_run(geo: &Geography, era: &str, seed: u64) -> (Setup, History) {
    let b = base();
    let era = b
        .content
        .eras
        .iter()
        .find(|e| e.id.ends_with(era))
        .expect("the era");
    let setup = Setup::of_era(&b.content, &b.graph, era, seed).expect("its peoples");
    let t0 = std::time::Instant::now();
    let h = deep::run(&setup, geo, &|_| {});
    println!(
        "{}: {} cells of {:.1} km, {:.2} s",
        era.id,
        geo.n * geo.n,
        geo.cell_km(),
        t0.elapsed().as_secs_f64()
    );
    (setup, h)
}

fn realm_name(r: u8) -> &'static str {
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

fn tell(h: &History, geo: &Geography) {
    for (k, sp) in h.species.iter().enumerate() {
        let demes: Vec<_> = h.demes.iter().filter(|d| d.species as usize == k).collect();
        let lineages: std::collections::BTreeSet<u32> = demes.iter().map(|d| d.lineage).collect();
        println!(
            "  {sp}: {:.0} people in {} cells, {} lineages",
            h.total(k),
            demes.len(),
            lineages.len()
        );
        let mut count = vec![0usize; h.techniques.len()];
        for d in &demes {
            for (i, c) in count.iter_mut().enumerate() {
                if d.knows() & (1 << i) != 0 {
                    *c += 1;
                }
            }
        }
        let known: Vec<String> = h
            .techniques
            .iter()
            .zip(&count)
            .filter(|(_, c)| **c > 0)
            .map(|(t, c)| format!("{}:{c}", t.rsplit(':').next().unwrap_or(t)))
            .collect();
        println!("    knows {}", known.join(" "));
    }
    for e in h
        .chronicle
        .iter()
        .filter(|e| !matches!(e.what, Happening::Split { .. }))
    {
        let what = match &e.what {
            Happening::Reached { species, realm } => {
                format!(
                    "{} reached the {}",
                    h.species[*species as usize],
                    realm_name(*realm)
                )
            }
            Happening::Invented {
                species, technique, ..
            } => format!(
                "{} invented {}",
                h.species[*species as usize], h.techniques[*technique as usize]
            ),
            other => format!("{other:?}"),
        };
        let g = &geo.cells[e.cell as usize];
        println!("  {:>9.0} ya  {what}  ({})", e.ya, realm_name(g.realm));
    }
    for c in h
        .census
        .iter()
        .filter(|c| c.ya < 120_000.0 || (c.ya as i64) % 100_000 < 10_000)
    {
        let people: Vec<String> = c.people.iter().map(|p| format!("{p:.0}")).collect();
        println!(
            "  census {:>9.0} ya  sea {:>5.0} m  {}",
            c.ya,
            c.sea_m,
            people.join(" / ")
        );
    }
    let splits = h
        .chronicle
        .iter()
        .filter(|e| matches!(e.what, Happening::Split { .. }))
        .count();
    println!("  {splits} lineage splits");
}

#[test]
fn deep_time_spreads_its_peoples_over_a_planet() {
    let terrain = planet(5);
    let geo = Geography::of_terrain(&terrain, &settings(), &sun);
    let land = geo.cells.iter().filter(|c| c.land_today() > 0.0).count();
    println!("{} cells, {land} with land", geo.cells.len());
    for r in 0..8u8 {
        let cells: Vec<_> = geo
            .cells
            .iter()
            .filter(|c| c.realm == r && c.land_today() > 0.0)
            .collect();
        if cells.is_empty() {
            continue;
        }
        let mean = |f: &dyn Fn(&&hearth_people::history::GeoCell) -> f32| {
            cells.iter().map(f).sum::<f32>() / cells.len() as f32
        };
        println!(
            "  {}: {} land cells, coldest month {:.1} °C, yield {:.2}, {} by rivers",
            realm_name(r),
            cells.len(),
            mean(&|c| c.coldest_c),
            mean(&|c| c.land_yield),
            cells.iter().filter(|c| c.river).count()
        );
    }
    for era in [
        "lower_paleolithic",
        "middle_paleolithic",
        "upper_paleolithic",
    ] {
        let (setup, h) = era_run(&geo, era, 5);
        tell(&h, &geo);
        // The era's peoples live.
        for k in setup.kinds.iter().enumerate().filter(|(_, k)| k.in_era) {
            assert!(h.total(k.0) > 0.0, "{} lives in {era}", k.1.id);
        }
        // The first people appeared in the cradle, and reached its realm before any other.
        let first = h
            .chronicle
            .iter()
            .find(|e| matches!(e.what, Happening::Appeared { .. }))
            .expect("a first people");
        assert_eq!(
            realm_name(geo.cells[first.cell as usize].realm),
            "Afrotropical"
        );
        // A tree of lineages: each from an older one.
        for l in &h.lineages {
            if let Some(p) = l.parent {
                let parent = h.lineage(p).expect("its parent");
                assert!(p < l.id && parent.since_ya >= l.since_ya);
            }
        }
    }
}

/// A made-up world of 10 km cells: a mainland (Afrotropical), and 40 km off across a strait whose
/// floor lies `strait_m` deep, an island (Australasian).
fn island_world(strait_m: f32) -> Geography {
    let n = 24;
    let mut cells = Vec::new();
    for j in 0..n {
        for i in 0..n {
            let (ground, realm, landmass) = if (2..14).contains(&i) && (6..18).contains(&j) {
                (50.0, 2, 0)
            } else if (18..21).contains(&i) && (10..13).contains(&j) {
                (20.0, 5, 1)
            } else if (14..18).contains(&i) && (10..13).contains(&j) {
                (strait_m, 5, 1)
            } else {
                (-500.0, 2, hearth_people::history::geo::NO_LANDMASS)
            };
            cells.push(GeoCell {
                ground: [ground; SAMPLES],
                temp_c: 25.0,
                coldest_c: 20.0,
                rain_mm: 1000.0,
                land_yield: 1.0,
                shelf_yield: 0.8,
                realm,
                landmass,
                river: false,
                sun: 0.9,
            });
        }
    }
    Geography {
        n,
        cell_m: 10_000.0,
        cells,
    }
}

/// One people appearing on the mainland 150,000 years ago knowing a chain of sixteen
/// techniques each resting on the last (and a raft if `raft`), the sea 80 m low until 100,000
/// years ago and today's from 90,000.
fn island_setup(raft: bool) -> Setup {
    let mut settings = settings();
    settings.curve = vec![
        (200_000.0, -80.0),
        (100_000.0, -80.0),
        (90_000.0, 0.0),
        (0.0, 0.0),
    ];
    let mut techniques: Vec<Technique> = (0..16)
        .map(|i| Technique {
            id: format!("test:t{i}"),
            years_bp: 1.0e6,
            requires: if i == 0 { 0 } else { 1 << (i - 1) },
            depth: i as u8,
            food: 1.0,
            fishing: 1.0,
            reach_km: 0.0,
        })
        .collect();
    if raft {
        techniques.push(Technique {
            id: "test:raft".to_owned(),
            years_bp: 1.0e6,
            requires: 0,
            depth: 0,
            food: 1.0,
            fishing: 1.0,
            reach_km: 120.0,
        });
    }
    let all = (1u128 << techniques.len()) - 1;
    Setup {
        seed: 9,
        era: "test".to_owned(),
        until_ya: 0.0,
        kinds: vec![Kind {
            id: "test:people".to_owned(),
            population: None,
            appears_ya: 150_000.0,
            gone_ya: None,
            realm: Some(2),
            from: None,
            growth: 0.006,
            spread_km_year: 1.0,
            lineage_people: 1500.0,
            people: 1.0e6,
            density: 0.03,
            cold_c: 0.0,
            repertoire: all,
            in_era: true,
        }],
        techniques,
        baseline: 0,
        cold: Vec::new(),
        settings,
    }
}

fn island_people(h: &History, geo: &Geography) -> f32 {
    h.demes
        .iter()
        .filter(|d| geo.cells[d.cell as usize].realm == 5)
        .map(|d| d.people)
        .sum()
}

#[test]
fn a_land_bridge_opens_with_the_ice_and_the_cut_off_lose_what_the_many_keep() {
    // A strait 60 m deep: dry land while the sea stands 80 m low.
    let geo = island_world(-60.0);
    let setup = island_setup(false);
    let h = deep::run(&setup, &geo, &|_| {});
    let crossed = h
        .chronicle
        .iter()
        .find(|e| matches!(e.what, Happening::Crossed { .. }))
        .expect("the island reached");
    println!("{crossed:?}; {:.0} on the island", island_people(&h, &geo));
    assert!(
        matches!(crossed.what, Happening::Crossed { by_sea: false, .. }),
        "over the land bridge"
    );
    assert!(crossed.ya > 100_000.0, "while the sea was low");
    assert!(island_people(&h, &geo) > 0.0, "and they stayed");
    // Cut off by the rising sea for ninety thousand years, the islanders have lost the deepest
    // techniques, which the mainland's people keep.
    let deepest =
        |d: &hearth_people::history::Deme| (0..16).rev().find(|&i| d.knows() & (1 << i) != 0);
    let (mut island, mut main) = (Vec::new(), Vec::new());
    for d in &h.demes {
        if geo.cells[d.cell as usize].realm == 5 {
            island.push(deepest(d));
        } else {
            main.push(deepest(d));
        }
    }
    println!(
        "deepest kept: island {island:?}, mainland {:?}",
        &main[..main.len().min(12)]
    );
    assert!(
        main.iter().all(|d| *d == Some(15)),
        "the mainland keeps all"
    );
    assert!(
        island.iter().all(|d| d.is_some_and(|d| d < 15)),
        "the island lost the deepest"
    );
    assert!(
        h.chronicle
            .iter()
            .any(|e| matches!(e.what, Happening::Lost { landmass: 1, .. })),
        "and the chronicle tells it"
    );
}

#[test]
fn the_sea_is_crossed_only_within_reach() {
    // A strait 200 m deep, never dry: 40 km of water.
    let geo = island_world(-200.0);
    let without = deep::run(&island_setup(false), &geo, &|_| {});
    assert_eq!(island_people(&without, &geo), 0.0, "no one drifts 40 km");
    let with = deep::run(&island_setup(true), &geo, &|_| {});
    let crossed = with
        .chronicle
        .iter()
        .find(|e| matches!(e.what, Happening::Crossed { .. }))
        .expect("rafts reach it");
    assert!(matches!(
        crossed.what,
        Happening::Crossed { by_sea: true, .. }
    ));
    assert!(island_people(&with, &geo) > 0.0);
}

#[test]
fn the_same_seed_makes_the_same_history() {
    let geo = island_world(-60.0);
    let a = deep::run(&island_setup(true), &geo, &|_| {});
    let b = deep::run(&island_setup(true), &geo, &|_| {});
    assert_eq!(a, b);
    let mut other = island_setup(true);
    other.seed = 10;
    let c = deep::run(&other, &geo, &|_| {});
    assert_ne!(a.demes, c.demes, "another seed, another history");
}

#[test]
fn the_recent_past_gives_the_people_lives_and_a_player_is_born_among_them() {
    use glam::{DVec2, DVec3};
    use hearth_fauna::ecology::Ecology;
    use hearth_fauna::habitat::{Uniform, temperate_wood};
    use hearth_fauna::species::Catalog;
    use hearth_people::{Now, People};
    let b = base();
    // The Upper Paleolithic's deep time over the made-up world (its mainland the cradle).
    let geo = island_world(-60.0);
    let era = b
        .content
        .eras
        .iter()
        .find(|e| e.id.ends_with("upper_paleolithic"))
        .expect("the era");
    let setup = Setup::of_era(&b.content, &b.graph, era, 21).expect("its peoples");
    let h = Arc::new(deep::run(&setup, &geo, &|_| {}));
    let sapiens = h.species_index("homo_sapiens").expect("our kind");
    println!(
        "{:.0} of our kind in {} lineages",
        h.total(sapiens),
        h.lineages
            .iter()
            .filter(|l| l.species as usize == sapiens)
            .count()
    );
    // The ecological cells of a planet as large, peopled by it.
    let mut catalog = Catalog::new(&b.content);
    catalog.peoples(&[("hearth:homo_sapiens".to_owned(), Some((25, 50)))]);
    let catalog = Arc::new(catalog);
    let land = Uniform {
        habitat: temperate_wood(&catalog),
        cells_around: (geo.circumference_m() / 256.0) as i64,
    };
    let mut eco = Ecology::new(catalog, 21, 0.0, &land);
    eco.peopling = Some(h.clone());
    // About the middle of the mainland.
    let (cx, cz) = geo.centre(12 * geo.n + 8);
    for dj in -2..=2 {
        for di in -2..=2 {
            let (ki, kj) = eco.region_key(cx + di as f64 * 8000.0, cz + dj as f64 * 8000.0);
            eco.ensure_region(&land, (ki, kj), 0.0);
        }
    }
    let mut people = People::new(21);
    people.history = Some(h.clone());
    let now = Now {
        tick: 0,
        hour: 10.0,
        day: 120.0 * YEAR_DAYS,
        year_days: YEAR_DAYS,
    };
    let at = DVec2::new(cx, cz);
    let bands = people.recent_history(
        &mut eco,
        &b.species,
        &b.graph,
        &b.items,
        &|_: DVec3| 0.0,
        at,
        40_000.0,
        100.0,
        now,
        &|_| {},
    );
    println!("{bands} bands lived a century");
    assert!(bands >= 2, "bands about the place");
    // Their culture is their people's of deep time, shared by bands of one lineage — or, for a
    // band that split off during the century, a daughter of it.
    let mut of_lineage = 0;
    for band in &people.bands {
        let d = band.deep.as_ref().expect("from deep time");
        let root = hearth_people::history::replay::culture_id(d.lineage);
        assert!(
            band.culture.id == root || band.culture.parent.is_some(),
            "band {} of culture {} (lineage {root})",
            band.id,
            band.culture.id
        );
        of_lineage += usize::from(band.culture.id == root);
        assert!(band.culture.language.is_some());
        assert!(!band.culture.knowledge.is_empty());
    }
    assert!(of_lineage >= 2, "bands sharing their lineage's culture");
    // A century lived: the dead, the born, and grandparents known.
    let dead = people.persons.iter().filter(|p| !p.alive()).count();
    let born = people
        .persons
        .iter()
        .filter(|p| p.life.born > now.day - 100.0 * YEAR_DAYS)
        .count();
    let with_grandmother = people
        .persons
        .iter()
        .filter(|p| p.alive())
        .filter(|p| {
            p.life
                .mother
                .and_then(|m| people.get(m))
                .and_then(|m| m.life.mother)
                .is_some()
        })
        .count();
    println!(
        "{dead} dead, {born} born in the century, {with_grandmother} living with a grandmother known"
    );
    assert!(dead > 0 && born > 0 && with_grandmother > 0);
    // Births offered among them, of as many bands as there are; a daughter born into the first.
    let options = people.birth_options(&b.species, at, 40_000.0, 0.0, &now, 4);
    println!("{} births offered: {options:?}", options.len());
    assert!((2..=4).contains(&options.len()));
    let mut bands_offered: Vec<u64> = options.iter().map(|o| o.band).collect();
    bands_offered.dedup();
    assert!(bands_offered.len() >= 2.min(bands), "of more than one band");
    let genetics = b.species.genetics.as_ref().expect("genetics");
    let (birth, household, me) = people
        .born_into(
            options[0],
            1,
            Some(true),
            0.0,
            genetics,
            &b.species,
            &b.graph,
            now,
        )
        .expect("born");
    assert!(birth.genome.female);
    let p = people.get(me).expect("the player's person");
    assert_eq!(p.life.mother, Some(options[0].mother));
    assert_eq!(p.life.father, Some(options[0].father));
    assert_eq!(p.player, Some(1));
    assert!(!p.tongues.is_empty(), "a mother tongue");
    println!(
        "born to a mother of {:.0} and a father of {:.0}, with {} brothers and sisters",
        household.mother_age,
        household.father_age,
        household.siblings.len()
    );
}

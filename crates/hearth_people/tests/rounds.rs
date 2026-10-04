//! H8 (V2.1 §15.3): an era's bands keep camp where their round goes season by season, coming back
//! to the same places; the bands of one people gather once a year in their season, meet one
//! another, and go home.

mod common;

use std::sync::Arc;

use common::*;
use glam::{DVec2, DVec3};
use hearth_content::schema::Season;
use hearth_content::schema::era::{Aggregation, EraPeople, RoundStop, Toward};
use hearth_fauna::ecology::Ecology;
use hearth_fauna::habitat::{Uniform, temperate_wood};
use hearth_fauna::species::Catalog;
use hearth_people::{Country, EraWays, Now, People};

/// A country whose places of each kind lie a fixed way from a band's home.
struct Compass;

fn offset(toward: Toward) -> DVec2 {
    match toward {
        Toward::Water => DVec2::new(1500.0, 0.0),
        Toward::Uplands => DVec2::new(0.0, 1500.0),
        Toward::Shelter => DVec2::new(-1500.0, 0.0),
    }
}

impl Country for Compass {
    fn camp_toward(
        &self,
        toward: Toward,
        from: DVec2,
        _within_m: f64,
        _taken: &[DVec2],
    ) -> Option<DVec3> {
        let at = from + offset(toward);
        Some(DVec3::new(at.x, 0.0, at.y))
    }
}

/// Twenty years of our kind's bands in temperate woods, thick enough for a few within a day or
/// two's walk, with a round and (if `gather`) an autumn gathering: the people, the bands founded
/// and the moment they were lived to (the middle of a summer).
fn lived(gather: bool) -> (People, usize, Now) {
    let b = base();
    let mut catalog = Catalog::new(&b.content);
    catalog.peoples(&[("hearth:homo_sapiens".to_owned(), Some((25, 50)))]);
    catalog.families_for(150);
    let catalog = Arc::new(catalog);
    let land = Uniform {
        habitat: temperate_wood(&catalog),
        cells_around: 4096,
    };
    let mut eco = Ecology::new(catalog, 5, 0.0, &land);
    let centre = (300_000.0, 0.0);
    for dj in -1..=1 {
        for di in -1..=1 {
            let key = eco.region_key(
                centre.0 + di as f64 * 16_000.0,
                centre.1 + dj as f64 * 16_000.0,
            );
            eco.ensure_region(&land, key, 0.0);
        }
    }
    let mut people = People::new(5);
    let sapiens = EraPeople {
        species: hearth_content::IdRef("hearth:homo_sapiens".into()),
        band_size: (25.0, 50.0),
        repertoire: Vec::new(),
        reach: None,
        hearth: true,
        bedding: true,
        round: vec![
            RoundStop {
                season: Season::Winter,
                toward: Toward::Shelter,
            },
            RoundStop {
                season: Season::Spring,
                toward: Toward::Water,
            },
            RoundStop {
                season: Season::Summer,
                toward: Toward::Uplands,
            },
        ],
        aggregation: gather.then_some(Aggregation {
            season: Season::Autumn,
            days: 20.0,
            reach_km: 60.0,
            bands: 4,
        }),
    };
    people.era = EraWays {
        peoples: vec![sapiens],
        year_offset: 0.0,
        country: Some(Arc::new(Compass)),
    };
    let now = Now {
        tick: 0,
        hour: 12.0,
        day: 40.0 * YEAR_DAYS + 0.37 * YEAR_DAYS,
        year_days: YEAR_DAYS,
    };
    let bands = people.recent_history(
        &mut eco,
        &b.species,
        &b.graph,
        &b.items,
        &|_: DVec3| 30.0,
        DVec2::new(centre.0, centre.1),
        40_000.0,
        20.0,
        now,
        &|_| {},
    );
    (people, bands, now)
}

/// The ties the living have to the living of other bands.
fn met(people: &People) -> usize {
    let band_of = |id: u64| people.get(id).filter(|q| q.alive()).map(|q| q.social.band);
    people
        .persons
        .iter()
        .filter(|p| p.alive())
        .map(|p| {
            p.social
                .ties
                .iter()
                .filter(|t| band_of(t.who).is_some_and(|b| b != p.social.band))
                .count()
        })
        .sum()
}

#[test]
fn bands_keep_their_rounds_camps_and_gather_in_their_season() {
    let (people, bands, now) = lived(true);
    println!("{bands} bands");
    assert!(bands >= 3, "bands about the place: {bands}");
    // Each band of the living keeps its summer camp, up on the uplands of its own country.
    let mut kept = 0;
    for band in people.bands.iter().filter(|b| !b.members.is_empty()) {
        assert_eq!(band.round.season, Some(Season::Summer), "band {}", band.id);
        let camp = band.camp.expect("a camp");
        let want = band.home + offset(Toward::Uplands);
        assert!(
            (DVec2::new(camp.x, camp.z) - want).length() < 1.0,
            "band {} camps at {camp:?}, not its uplands {want:?}",
            band.id
        );
        kept += 1;
    }
    assert!(kept >= 3);
    // Last autumn its people gathered, the nearest four bands at most at one camp, each band at
    // its own fire, for twenty days of the real year (as long a part of the game's).
    let gathered: Vec<_> = people
        .bands
        .iter()
        .filter_map(|b| b.round.gathering.map(|g| (b.id, g)))
        .filter(|(_, g)| g.bands >= 2 && now.day - g.from < YEAR_DAYS)
        .collect();
    println!("gathered last autumn: {gathered:?}");
    assert!(gathered.len() >= 2, "bands gathered");
    let mut camps: std::collections::BTreeMap<u64, usize> = Default::default();
    for (_, g) in &gathered {
        *camps.entry(g.host).or_default() += 1;
        assert!((g.until - g.from - 20.0 / 365.0 * YEAR_DAYS).abs() < 1e-9);
        assert!((2..=4).contains(&g.bands), "{} bands at one camp", g.bands);
    }
    assert!(camps.values().all(|&n| n <= 4), "gatherings: {camps:?}");
    // Gathering, people came to know those of other bands, as people kept apart did not.
    let (apart, _, _) = lived(false);
    let (with, without) = (met(&people), met(&apart));
    println!("{with} ties across bands, against {without} without gatherings");
    assert!(with > 2 * without.max(10), "people met at the gatherings");
}

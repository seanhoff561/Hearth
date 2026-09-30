//! Deposit statistics (v2 V2-2 acceptance): bodies lie only in their provinces, host rocks and
//! climates, at the frequencies the content gives, and the resources of the first eras are
//! within reach of every continent.

use std::sync::{Arc, OnceLock};

use hearth_content::Content;
use hearth_content::schema::Entry;
use hearth_content::schema::geology::DepositGeometry;
use hearth_math::PlanetSize;
use hearth_worldgen::deposits::{Body, CELL};
use hearth_worldgen::{PlanetGrid, Terrain, WorldGenSettings, WorldGenerator};
use rayon::prelude::*;

fn content() -> &'static Content {
    static C: OnceLock<Content> = OnceLock::new();
    C.get_or_init(Content::load_base)
}

fn generator() -> &'static WorldGenerator {
    static G: OnceLock<WorldGenerator> = OnceLock::new();
    G.get_or_init(|| {
        let s = WorldGenSettings {
            seed: 11,
            planet_size: PlanetSize::Standard,
            grid_resolution: 512,
            ..WorldGenSettings::default()
        };
        let terrain = Arc::new(Terrain::new(Arc::new(PlanetGrid::build(&s, &|_, _| {}))));
        let reg = hearth_world::datapack::load_builtin_registry().expect("base pack");
        WorldGenerator::new(terrain, &reg, content()).expect("all generator blocks exist")
    })
}

fn census() -> &'static [Body] {
    static B: OnceLock<Vec<Body>> = OnceLock::new();
    B.get_or_init(|| generator().deposits.census(generator()))
}

fn surface_bound(g: DepositGeometry) -> bool {
    matches!(
        g,
        DepositGeometry::Placer
            | DepositGeometry::Crust
            | DepositGeometry::Flow
            | DepositGeometry::Bog
    )
}

#[test]
fn bodies_lie_in_their_provinces_and_host_rocks() {
    let wg = generator();
    let d = &wg.deposits;
    let bodies = census();
    assert!(
        bodies.len() > 2000,
        "only {} bodies on a Standard planet",
        bodies.len()
    );
    let mut hosted = 0;
    for b in bodies {
        let m = &d.models[b.model as usize];
        let p = wg
            .geology
            .province_index(b.center[0] as f64 + 0.5, b.center[2] as f64 + 0.5);
        assert!(
            d.in_province(b, p),
            "{} at {:?} lies in {}",
            m.id,
            b.center,
            wg.geology.province(p).id
        );
        let hosts = d.hosts(b.model);
        if !hosts.is_empty() && !surface_bound(m.geometry) {
            let rock = wg
                .geology
                .column(b.center[0], b.center[2])
                .rock_at(b.center[1]);
            assert!(
                hosts.contains(&rock),
                "{} at {:?} is not in a host rock",
                m.id,
                b.center
            );
            hosted += 1;
        }
    }
    assert!(hosted > 500, "only {hosted} host-rock bodies checked");
}

#[test]
fn bodies_lie_in_their_climates() {
    let wg = generator();
    let d = &wg.deposits;
    let c = content();
    let mut checked = 0;
    for b in census() {
        let dep = c
            .deposits
            .iter()
            .nth(b.model as usize)
            .expect("model per deposit");
        if dep.conditions.is_empty() {
            continue;
        }
        let s = wg.terrain.sample(b.center[0], b.center[2]);
        for cond in &dep.conditions {
            let ok = match cond.as_str() {
                "arid" => s.precipitation < 450.0,
                "humid" => s.precipitation > 750.0,
                "warm" => s.temperature > 15.0,
                "cold" => s.temperature < 3.0,
                _ => true,
            };
            assert!(
                ok,
                "{} at {:?} breaks its condition {cond}",
                d.models[b.model as usize].id, b.center
            );
        }
        checked += 1;
    }
    assert!(checked > 100, "only {checked} conditioned bodies");
}

/// Models bound only by their provinces occur at their content frequency per km² of those
/// provinces (Poisson noise and the jittered province borders aside); the others never exceed it.
#[test]
fn bodies_occur_at_their_frequencies() {
    let wg = generator();
    let d = &wg.deposits;
    let c = content();
    let deposits: Vec<_> = c.deposits.iter().collect();
    let province_of = |id: &str| {
        wg.geology
            .provinces()
            .iter()
            .position(|p| p.id == id)
            .map(|i| i as u8)
    };
    let provinces: Vec<Vec<u8>> = deposits
        .iter()
        .map(|dep| {
            dep.provinces
                .iter()
                .filter_map(|p| province_of(p.as_str()))
                .collect()
        })
        .collect();
    let n = d.cells_around();
    // Share of each model's provinces in every scanned cell (8×8 points), counting only dry
    // ground for surface-bound models (they are not laid under water).
    const K: i32 = 8;
    let expected: Vec<f64> = (-n / 2..n - n / 2)
        .into_par_iter()
        .map(|cz| {
            let mut e = vec![0.0f64; deposits.len()];
            for cx in 0..n {
                if !d.has_land(wg, cx, cz) {
                    continue;
                }
                for k in 0..K * K {
                    let x = cx * CELL + (k % K) * (CELL / K) + CELL / K / 2;
                    let z = cz * CELL + (k / K) * (CELL / K) + CELL / K / 2;
                    let p = wg.geology.province_index(x as f64 + 0.5, z as f64 + 0.5);
                    let mut dry = None;
                    for (mi, dep) in deposits.iter().enumerate() {
                        if !provinces[mi].contains(&p) {
                            continue;
                        }
                        if surface_bound(dep.geometry)
                            && !*dry.get_or_insert_with(|| !wg.terrain.sample(x, z).is_underwater())
                        {
                            continue;
                        }
                        let per_cell =
                            dep.frequency_per_km2 as f64 * (CELL as f64 / 1000.0).powi(2);
                        e[mi] += per_cell / (K * K) as f64;
                    }
                }
            }
            e
        })
        .reduce(
            || vec![0.0; deposits.len()],
            |a, b| a.iter().zip(&b).map(|(x, y)| x + y).collect(),
        );
    let mut observed = vec![0usize; deposits.len()];
    for b in census() {
        observed[b.model as usize] += 1;
    }
    let mut compared = 0;
    for (mi, dep) in deposits.iter().enumerate() {
        let (obs, exp) = (observed[mi] as f64, expected[mi]);
        let sigma = exp.sqrt().max(1.0);
        if dep.host_rocks.is_empty() && dep.conditions.is_empty() {
            assert!(
                (obs - exp).abs() < 4.0 * sigma + 0.1 * exp,
                "{}: {obs} bodies, {exp:.1} expected",
                dep.id()
            );
            compared += 1;
        } else {
            assert!(
                obs < exp + 4.0 * sigma + 0.1 * exp,
                "{}: {obs} bodies, at most {exp:.1} expected",
                dep.id()
            );
        }
    }
    assert!(
        compared >= 5,
        "only {compared} models compared with their frequency"
    );
}

/// The resources of the first three eras (toolstone, fire stones, pigments, clay) are within
/// reach of every continent, directly or through a substitute.
#[test]
fn early_resources_reach_every_continent() {
    let wg = generator();
    let cov = hearth_worldgen::coverage::coverage_of(wg, content(), census(), 2, 50.0);
    assert!(!cov.continents.is_empty(), "no continents");
    let open: Vec<String> = cov
        .open_gaps()
        .map(|g| {
            format!(
                "continent {} lacks {} ({:.1} km)",
                g.continent, g.resource, g.p90_km
            )
        })
        .collect();
    assert!(open.is_empty(), "{open:#?}");
    // Every continent has its own toolstone.
    for (ci, reach) in cov.reach.iter().enumerate() {
        let knappable: usize = reach.iter().filter(|r| r.era == 0).map(|r| r.bodies).sum();
        assert!(knappable > 0, "continent {ci} has no toolstone of its own");
    }
}

/// Gravel washed in a placer reach leaves its heavy grains in the pan, richest in the middle;
/// gravel away from rivers leaves nothing.
#[test]
fn panning_finds_placer_grains() {
    let wg = generator();
    let d = &wg.deposits;
    let mut panned = 0;
    for b in census() {
        let m = &d.models[b.model as usize];
        if m.geometry != DepositGeometry::Placer
            || m.id != "hearth:gold_placers" && m.id != "hearth:cassiterite_placers"
        {
            continue;
        }
        let s = wg.terrain.sample(b.center[0], b.center[2]);
        if !s.river.is_some_and(|r| r.distance < r.width * 0.5 + 6.0) {
            continue;
        }
        let pan = d.pan(wg, b.center[0], b.center[2]);
        let grams = pan
            .grains
            .iter()
            .find(|(id, _)| *id == m.resource)
            .map_or(0.0, |g| g.1);
        assert!(
            grams > 0.0,
            "nothing of {} in a pan at {:?}: {pan:?}",
            m.resource,
            b.center
        );
        if m.resource == "hearth:native_gold" {
            assert!(grams < 1.0, "{grams} g of gold in one pan");
        }
        panned += 1;
    }
    assert!(panned >= 3, "only {panned} placers on a river bed to pan");
    // Dry land far from any river: nothing.
    let (sx, sz) = wg.terrain.find_spawn(false);
    let dry = (0..400)
        .map(|k| (sx + (k % 20) * 97, sz + (k / 20) * 97))
        .find(|&(x, z)| {
            let s = wg.terrain.sample(x, z);
            !s.is_underwater() && s.river.is_none_or(|r| r.distance > r.width + 40.0)
        })
        .expect("dry ground near spawn");
    assert!(d.pan(wg, dry.0, dry.1).grains.is_empty());
}

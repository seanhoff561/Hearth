//! Fifty-year runs of the land about a biome's heart (V2-10): the world of seed 7 on a vast
//! planet (on smaller ones the 3 × 3 regions about a heart hold several kinds of land, and the
//! standard planet has only patches of some biomes, D112), made once for every test of a file;
//! the regions about the heart; the species of the biome's ecosystems the land holds ten of at
//! least, judged over fifty years. And the same of a uniform land (`open_land`), for a realm
//! the generated world has none of a biome in.

// Each test file uses what it needs of these.
#![allow(dead_code)]

use std::sync::{Arc, OnceLock};

use hearth_fauna::ecology::{Cause, Ecology};
use hearth_fauna::habitat::{GenLand, Habitat, Land, TreeYields, Uniform};
use hearth_fauna::species::{Catalog, Forage};
use hearth_math::PlanetSize;
use hearth_worldgen::region::biome::Biome;
use hearth_worldgen::vegetation::Vegetation;
use hearth_worldgen::{ColumnSample, PlanetGrid, Terrain, WorldGenSettings, WorldGenerator};

pub struct World {
    pub wg: WorldGenerator,
    pub catalog: Arc<Catalog>,
    pub trees: TreeYields,
    pub veg: Vegetation,
}

/// The world of seed 7 on a vast planet, made once.
pub fn world() -> &'static World {
    static WORLD: OnceLock<World> = OnceLock::new();
    WORLD.get_or_init(|| {
        let s = WorldGenSettings {
            seed: 7,
            planet_size: PlanetSize::Vast,
            grid_resolution: 256,
            ..WorldGenSettings::default()
        };
        let terrain = Arc::new(Terrain::new(Arc::new(PlanetGrid::build(&s, &|_, _| {}))));
        let reg = hearth_world::datapack::load_builtin_registry().expect("base pack");
        let content = hearth_content::Content::load_base();
        let wg = WorldGenerator::new(terrain, &reg, &content).expect("generator blocks");
        let catalog = Arc::new(Catalog::new(&content));
        let trees = TreeYields::new(&wg, &content);
        World {
            wg,
            catalog,
            trees,
            veg: Vegetation::default(),
        }
    })
}

/// The populations of the regions about the heart of a biome (its own and the eight about it,
/// as the game keeps them about a player there), made; and the heart's own cell.
pub fn about_the_heart(
    biome: Biome,
    also: impl Fn(&ColumnSample) -> bool + Sync,
) -> (Ecology, Habitat) {
    let w = world();
    let land = GenLand {
        wg: &w.wg,
        veg: &w.veg,
        catalog: &w.catalog,
        trees: &w.trees,
    };
    let (x, z) =
        w.wg.terrain
            .find_biome(biome, 12_000.0, also)
            .unwrap_or_else(|| panic!("no {} on the planet", biome.name()));
    let s = w.wg.terrain.sample(x, z);
    println!(
        "{} at {x}, {z}: mean {:.1} °C, warmest month {:.1} °C, {:.0} mm, realm {:?}",
        biome.name(),
        s.temperature,
        s.t_warm,
        s.precipitation,
        s.realm
    );
    let h = land.habitat((
        (x as f64 / 256.0).floor() as i64,
        (z as f64 / 256.0).floor() as i64,
    ));
    let forage: Vec<String> = Forage::ALL
        .iter()
        .map(|k| format!("{} {:.0}", k.name(), h.forage[*k as usize]))
        .collect();
    println!(
        "  its cell: cover {:.2}, animals of the {:?}; {}",
        h.cover,
        h.fauna(),
        forage.join(", ")
    );
    let mut eco = Ecology::new(w.catalog.clone(), 7, 0.0, &land);
    let key = eco.region_key(x as f64, z as f64);
    for dj in -1..=1 {
        for di in -1..=1 {
            eco.ensure_region(&land, (key.0 + di, key.1 + dj), 0.0);
        }
    }
    (eco, h)
}

/// The populations of 2 × 2 regions of one habitat everywhere.
pub fn uniform(habitat: Habitat) -> Ecology {
    let catalog = world().catalog.clone();
    let land = Uniform {
        habitat,
        cells_around: 4096,
    };
    let mut eco = Ecology::new(catalog, 7, 0.0, &land);
    for j in 0..2 {
        for i in 0..2 {
            eco.ensure_region(&land, (i + 3, j), 0.0);
        }
    }
    eco
}

/// Fifty years of the regions: the names of the species of the biome's ecosystems the land
/// holds ten of at least (and two groups' worth of one that lives in groups), and those that left
/// the bounds — within a twentieth and four times what the land holds (its patches, on the
/// borders of realms and kinds of land, hold fewer than a whole stretch of it would), never gone
/// for long (one rare here may vanish a while and come back from the land beyond). The regions
/// about a place reach far: the species of other kinds of land in them are left to their own
/// tests.
pub fn fifty_years(eco: &mut Ecology, biome: Biome) -> (Vec<String>, Vec<String>) {
    let cat = eco.catalog.clone();
    let mine = cat.ecosystems_of_biome(biome.name());
    let present: Vec<usize> = (0..cat.len())
        .filter(|&s| {
            let sp = &cat.species[s];
            let groups = (sp.group.0 + sp.group.1) as f64;
            sp.habitats & mine != 0
                && eco.capacity(s) >= 10.0f64.max(if sp.grouped() { groups } else { 0.0 })
        })
        .collect();
    // What the regions' first years (made at equilibrium, settled for three) cost each.
    for &s in &present {
        let died: Vec<String> = [
            Cause::Natural,
            Cause::Hunger,
            Cause::Winter,
            Cause::Crowding,
            Cause::Predation,
            Cause::Lost,
        ]
        .iter()
        .map(|&c| {
            let n = eco.deaths.get(&(s as u16, c)).copied().unwrap_or(0.0);
            format!("{c:?} {n:.0}")
        })
        .collect();
        println!(
            "  settling: {:<24} now {:>9.0} of {:>9.0}; died {}",
            cat.species[s].name,
            eco.count(s),
            eco.capacity(s),
            died.join(", ")
        );
    }
    eco.deaths.clear();
    eco.fed.clear();
    let start = eco.regions.values().map(|r| r.time).fold(0.0, f64::max);
    let mut series = vec![Vec::new(); present.len()];
    for y in 0..50 {
        let mut sums = vec![0.0; present.len()];
        for q in 0..4 {
            eco.advance(start + y as f64 + (q + 1) as f64 / 4.0, 1.0 / 32.0);
            for (k, &s) in present.iter().enumerate() {
                sums[k] += eco.count(s) / 4.0;
            }
        }
        for (k, v) in sums.into_iter().enumerate() {
            series[k].push(v);
        }
    }
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
        let died = |c: Cause| eco.deaths.get(&(s as u16, c)).copied().unwrap_or(0.0) / 50.0;
        let fed = |k: u16| {
            eco.fed
                .get(&k)
                .map_or(f64::NAN, |(sum, n)| sum / n.max(1.0))
        };
        println!(
            "  {:<24} capacity {:>9.0}  mean {:>9.0} ({:>4.2})  range {:>9.0} .. {:>9.0}  \
             fed {:.2} hunt {:.2}  deaths/yr: natural {:.0} hunger {:.0} winter {:.0} \
             crowding {:.0} predation {:.0} lost {:.0}",
            sp.name,
            cap,
            mean,
            mean / cap,
            lo,
            hi,
            fed(s as u16),
            fed(1000 + s as u16),
            died(Cause::Natural),
            died(Cause::Hunger),
            died(Cause::Winter),
            died(Cause::Crowding),
            died(Cause::Predation),
            died(Cause::Lost),
        );
        let gone = v.windows(8).any(|w| w.iter().all(|x| *x < 0.5));
        if mean < 0.05 * cap || mean > 4.0 * cap + 5.0 || gone {
            failures.push(format!("{}: mean {mean:.0} of {cap:.0}", sp.name));
        }
    }
    let names = present.iter().map(|&s| cat.species[s].id.clone()).collect();
    (names, failures)
}

/// Whether one of the species (bare ids) is among those present.
fn has_one(present: &[String], ids: &[&str]) -> bool {
    ids.iter()
        .any(|id| present.iter().any(|p| p.ends_with(&format!(":{id}"))))
}

/// The land holds one at least of each list of species, and every species stayed in bounds.
pub fn check(biome: &str, present: &[String], failures: &[String], belong: &[&[&str]]) {
    let missing: Vec<String> = belong
        .iter()
        .filter(|ids| !has_one(present, ids))
        .map(|ids| ids.join(" or "))
        .collect();
    assert!(
        missing.is_empty(),
        "{biome} lacks {missing:?}; it holds {present:?}"
    );
    assert!(failures.is_empty(), "{biome}: out of bounds: {failures:?}");
}

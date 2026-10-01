//! Succession (V2-6, docs/design/flora.md): trees age with the calendar; cleared land grows
//! back through the pioneers to the shade-tolerant trees of the old forest; burned land stands
//! charred and bare a while; a felled tree leaves its stump and another takes its place.

use std::sync::{Arc, OnceLock};

use hearth_flora::Stage;
use hearth_math::{BlockPos, PlanetSize};
use hearth_world::BlockRegistry;
use hearth_worldgen::cubegen::features::{PlacedTree, Remains};
use hearth_worldgen::region::biome::Biome;
use hearth_worldgen::vegetation::{Disturbance, DisturbanceKind, Vegetation, VegetationSave};
use hearth_worldgen::{PlanetGrid, Terrain, WorldGenSettings, WorldGenerator};

fn registry() -> &'static BlockRegistry {
    static R: OnceLock<BlockRegistry> = OnceLock::new();
    R.get_or_init(|| hearth_world::datapack::load_builtin_registry().expect("base pack"))
}

fn generator() -> &'static WorldGenerator {
    static G: OnceLock<WorldGenerator> = OnceLock::new();
    G.get_or_init(|| {
        let s = WorldGenSettings {
            seed: 7,
            planet_size: PlanetSize::Standard,
            grid_resolution: 256,
            ..WorldGenSettings::default()
        };
        let terrain = Arc::new(Terrain::new(Arc::new(PlanetGrid::build(&s, &|_, _| {}))));
        let content = hearth_content::Content::load_base();
        WorldGenerator::new(terrain, registry(), &content).expect("generator blocks")
    })
}

/// A place in a closed forest of real species near the spawn: the centre of a square of
/// `2 x half` metres holding many canopy trees.
fn forest(wg: &WorldGenerator, half: i32) -> (i32, i32) {
    static SPOT: OnceLock<(i32, i32)> = OnceLock::new();
    *SPOT.get_or_init(|| {
        let (sx, sz) = wg.terrain.find_spawn(false);
        let veg = Vegetation::default();
        let mut near = Vec::new();
        for j in -60..=60 {
            for i in -60..=60 {
                let (x, z) = (wg.planet().wrap_x(sx + i * 256), sz + j * 256);
                let s = wg.terrain.sample(x, z);
                let woods = matches!(
                    s.biome,
                    Biome::BroadleafForest | Biome::MixedForest | Biome::BirchForest
                );
                if woods && s.tree_density > 0.6 && !s.is_underwater() {
                    near.push((i * i + j * j, x, z));
                }
            }
        }
        near.sort_unstable();
        for (_, x, z) in near.into_iter().take(60) {
            let trees =
                wg.features()
                    .trees_in(wg, &veg, (x - half, z - half), (x + half, z + half));
            let canopy = trees
                .iter()
                .filter(|t| !t.understory && t.stage >= Stage::Young)
                .count();
            let side = (2 * half) as f32;
            if canopy as f32 >= side * side / 150.0 {
                return (x, z);
            }
        }
        panic!("no forest near the spawn");
    })
}

fn pioneer(wg: &WorldGenerator, t: &PlacedTree) -> bool {
    wg.forest.niches[t.species].shade_tolerance < 0.3
}

fn tolerant(wg: &WorldGenerator, t: &PlacedTree) -> bool {
    wg.forest.niches[t.species].shade_tolerance >= 0.5
}

fn at(save: &VegetationSave, year: f64) -> Vegetation {
    Vegetation::new(save, generator().planet().circumference(), year)
}

/// The trees with their foot within `r` of a place.
fn within(wg: &WorldGenerator, veg: &Vegetation, (x, z): (i32, i32), r: i32) -> Vec<PlacedTree> {
    wg.features()
        .trees_in(wg, veg, (x - r, z - r), (x + r, z + r))
        .into_iter()
        .filter(|t| {
            let (dx, dz) = (t.foot[0] - x, t.foot[2] - z);
            dx * dx + dz * dz <= r * r
        })
        .collect()
}

#[test]
fn cleared_land_grows_back_through_pioneers_to_the_old_forest() {
    let wg = generator();
    let c = forest(wg, 30);
    println!("a forest at {c:?}");
    let save = VegetationSave {
        disturbances: vec![Disturbance {
            kind: DisturbanceKind::Cleared,
            year: 10.0,
            x: c.0,
            z: c.1,
            radius: 60.0,
            severity: 1.0,
            patches: Vec::new(),
        }],
    };
    let before = within(wg, &at(&save, 9.9), c, 45);
    assert!(before.len() > 40, "{} trees before", before.len());
    let mut shares = Vec::new();
    for year in [10.2, 11.5, 14.0, 20.0, 40.0, 80.0, 160.0, 320.0] {
        let trees = within(wg, &at(&save, year), c, 45);
        let living: Vec<&PlacedTree> = trees
            .iter()
            .filter(|t| t.remains == Remains::Living)
            .collect();
        let n = living.len().max(1) as f32;
        let p = living.iter().filter(|t| pioneer(wg, t)).count() as f32 / n;
        let s = living.iter().filter(|t| tolerant(wg, t)).count() as f32 / n;
        let mut stages = std::collections::BTreeMap::new();
        let mut species = std::collections::BTreeMap::new();
        for t in &living {
            *stages.entry(t.stage.name()).or_insert(0) += 1;
            *species
                .entry(wg.forest.templates.species[t.species].id.as_str())
                .or_insert(0) += 1;
        }
        println!(
            "year {year:5.1}: {:3} living, pioneers {:.2}, shade-tolerant {:.2}, {stages:?}\n    \
             {species:?}",
            living.len(),
            p,
            s
        );
        shares.push((year, living.len(), p, s));
    }
    // Nothing standing just after the clearing; the pioneers come within a few years and hold
    // the young stand; the shade-tolerant take over as the pioneers age and die.
    assert_eq!(shares[0].1, 0, "trees just after the clearing");
    let early = shares[2];
    assert!(
        early.1 > 10 && early.2 > 0.5,
        "pioneers within four years: {early:?}"
    );
    let young = shares[3];
    assert!(
        young.2 > 0.5,
        "the pioneers hold the young stand: {young:?}"
    );
    let old = shares[7];
    assert!(
        old.3 > 0.5 && old.3 > old.2,
        "the shade-tolerant take the old forest back: {old:?}"
    );
}

#[test]
fn a_burned_stand_stands_charred_and_bare_then_grows_back() {
    let wg = generator();
    let c = forest(wg, 30);
    let save = VegetationSave {
        disturbances: vec![Disturbance {
            kind: DisturbanceKind::Burned,
            year: 5.0,
            x: c.0,
            z: c.1,
            radius: 50.0,
            severity: 1.0,
            patches: Vec::new(),
        }],
    };
    let just = within(wg, &at(&save, 5.05), c, 35);
    let charred = just
        .iter()
        .filter(|t| t.remains == Remains::Charred)
        .count();
    let living = just.iter().filter(|t| t.remains == Remains::Living).count();
    assert!(
        charred > 10 && living == 0,
        "{charred} charred, {living} living"
    );
    // The ground is burned bare at first, and green again within a year.
    let burnt = wg.blocks.burnt_ground;
    let ground_at = |veg: &Vegetation| {
        let mut n = 0;
        for dz in (-20..20).step_by(4) {
            for dx in (-20..20).step_by(4) {
                let (x, z) = (c.0 + dx, c.1 + dz);
                let y = wg.terrain.sample(x, z).height_i() - 1;
                let p = BlockPos::new(x, y, z);
                let (cube, _) = wg.generate_cube_in(p.cube(), veg);
                if cube.get(p.local()) == burnt {
                    n += 1;
                }
            }
        }
        n
    };
    assert!(ground_at(&at(&save, 5.05)) > 50, "burned ground");
    assert_eq!(ground_at(&at(&save, 6.0)), 0, "green again");
    // Twenty years on, the charred trunks have fallen and a young stand of pioneers grows.
    let later = within(wg, &at(&save, 25.0), c, 35);
    let still = later
        .iter()
        .filter(|t| t.remains == Remains::Charred)
        .count();
    let young: Vec<&PlacedTree> = later
        .iter()
        .filter(|t| t.remains == Remains::Living)
        .collect();
    let pioneers = young.iter().filter(|t| pioneer(wg, t)).count();
    assert!(still == 0, "{still} charred trunks still standing");
    assert!(
        young.len() > 10 && pioneers * 2 > young.len(),
        "{} young trees, {pioneers} pioneers",
        young.len()
    );
}

#[test]
fn a_felled_tree_leaves_its_stump_and_another_takes_its_place() {
    let wg = generator();
    let c = forest(wg, 30);
    let veg = at(&VegetationSave::default(), 1.0);
    let tree = within(wg, &veg, c, 30)
        .into_iter()
        .find(|t| !t.understory && t.stage >= Stage::Young)
        .expect("a canopy tree");
    let foot = (tree.foot[0], tree.foot[2]);
    let felled = veg.with(Disturbance {
        kind: DisturbanceKind::Felled,
        year: 1.0,
        x: foot.0,
        z: foot.1,
        radius: tree.template.reach() as f32,
        severity: 1.0,
        patches: Vec::new(),
    });
    let save = felled.save();
    let site = |year: f64| {
        within(wg, &at(&save, year), foot, 0)
            .into_iter()
            .find(|t| (t.foot[0], t.foot[2]) == foot)
    };
    let before = site(0.9).expect("the tree before it was felled");
    assert_eq!(before.remains, Remains::Living);
    let stump = site(1.2).expect("its stump");
    assert_eq!(stump.remains, Remains::Stump);
    assert!(stump.blocks().all(|(p, _)| p.y <= stump.foot[1]));
    assert!(stump.blocks().count() > 0);
    let next = site(12.0).expect("a young tree in its place");
    assert_eq!(next.remains, Remains::Living);
    assert!(next.stage <= Stage::Pole, "{:?}", next.stage);
}

#[test]
fn trees_age_with_the_calendar_and_cubes_tell_when_they_change() {
    let wg = generator();
    let c = forest(wg, 30);
    let save = VegetationSave::default();
    let young = within(wg, &at(&save, 0.0), c, 40);
    let older = within(wg, &at(&save, 60.0), c, 40);
    let grown = young
        .iter()
        .filter(|a| {
            older
                .iter()
                .any(|b| b.foot == a.foot && !b.understory && !a.understory && b.stage > a.stage)
        })
        .count();
    assert!(
        grown > 5,
        "{grown} trees grew into a later stage in sixty years"
    );
    // A cube under the canopy says when its trees next change, and they do change then.
    let y = wg.terrain.sample(c.0, c.1).height_i() + 4;
    let p = BlockPos::new(c.0, y, c.1).cube();
    let veg = at(&save, 0.0);
    let (now, next) = wg.generate_cube_in(p, &veg);
    assert!(next.is_finite() && next > 0.0, "next change {next}");
    let (same, _) = wg.generate_cube_in(p, &veg.at_year(next - 1e-3));
    assert!(
        (0..hearth_math::CUBE_VOLUME).all(|i| same.get_index(i) == now.get_index(i)),
        "nothing changes before the year it said"
    );
}

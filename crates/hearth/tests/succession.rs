//! V2-6 acceptance: a cleared area goes through succession over simulated years, in a running
//! world: herbs at once, shrubs from the second year, pioneer trees a few years on, the trees
//! of the old forest as the pioneers age. The server grows the loaded terrain again as the
//! years pass; what is counted is what the player would see.

mod common;

use std::collections::BTreeMap;

use common::{World, temp};
use hearth_content::schema::flora::GrowthForm;
use hearth_flora::Stage;
use hearth_math::BlockPos;
use hearth_protocol::ToServer;
use hearth_worldgen::region::biome::Biome;
use hearth_worldgen::vegetation::{DisturbanceKind, Vegetation};

/// What grows in a circle about a place.
#[derive(Debug, Default)]
struct Census {
    /// Trunks and limbs of living trees.
    wood: u32,
    charred: u32,
    /// Foliage blocks by the shade tolerance of their species: pioneers (under 0.3), the
    /// shade-tolerant (0.5 and over), and those between.
    pioneer_leaves: u32,
    middle_leaves: u32,
    tolerant_leaves: u32,
    herbs: u32,
    shrubs: u32,
    grass: u32,
    burnt: u32,
    species: BTreeMap<String, u32>,
}

impl Census {
    fn leaves(&self) -> u32 {
        self.pioneer_leaves + self.middle_leaves + self.tolerant_leaves
    }
}

fn census(w: &World, (x, z): (i32, i32), r: i32) -> Census {
    // The blocks of the trees' foliage and of the understory's plants, from the content.
    let mut foliage: BTreeMap<String, (String, f32)> = BTreeMap::new();
    let mut under: BTreeMap<String, GrowthForm> = BTreeMap::new();
    for p in w.content.plants.iter() {
        if let Some(t) = &p.tree {
            let leaves = t
                .leaves
                .as_str()
                .rsplit(':')
                .next()
                .unwrap_or("")
                .to_owned();
            foliage.insert(leaves, (p.id.clone(), p.shade_tolerance));
        }
        if let Some(u) = &p.understory {
            let block = u.block.as_str().rsplit(':').next().unwrap_or("").to_owned();
            under.insert(block, p.form);
        }
    }
    let mut c = Census::default();
    for dz in -r..=r {
        for dx in -r..=r {
            if dx * dx + dz * dz > r * r {
                continue;
            }
            let (bx, bz) = (x + dx, z + dz);
            let ground = w.generator.terrain.sample(bx, bz).height_i();
            for y in ground - 2..ground + 45 {
                let Some(name) = w.block(BlockPos::new(bx, y, bz)) else {
                    continue;
                };
                if name == "air" {
                    continue;
                }
                if name.starts_with("charred_") {
                    c.charred += 1;
                } else if name.ends_with("_log") || name.ends_with("_branch") {
                    c.wood += 1;
                } else if let Some((id, shade)) = foliage.get(&name) {
                    if *shade < 0.3 {
                        c.pioneer_leaves += 1;
                    } else if *shade >= 0.5 {
                        c.tolerant_leaves += 1;
                    } else {
                        c.middle_leaves += 1;
                    }
                    *c.species.entry(id.clone()).or_insert(0) += 1;
                } else if let Some(form) = under.get(&name) {
                    if matches!(form, GrowthForm::Shrub | GrowthForm::Vine) {
                        c.shrubs += 1;
                    } else {
                        c.herbs += 1;
                    }
                } else if name == "burnt_ground" {
                    c.burnt += 1;
                } else if name.contains("grass") && name != "grass_block" {
                    c.grass += 1;
                }
            }
        }
    }
    c
}

/// A closed temperate wood of real species (no snow lying in the season the world starts in)
/// near the player: the centre of a square of 50 m holding many canopy trees.
fn forest(w: &World) -> (i32, i32) {
    let wg = &w.generator;
    let veg = Vegetation::default();
    let (px, pz) = (w.mover.pos.x as i32, w.mover.pos.z as i32);
    let mut near = Vec::new();
    for j in -64..=64 {
        for i in -64..=64 {
            let (x, z) = (wg.planet().wrap_x(px + i * 96), pz + j * 96);
            let s = wg.terrain.sample(x, z);
            let woods = matches!(
                s.biome,
                Biome::BroadleafForest | Biome::MixedForest | Biome::BirchForest
            );
            // Temperate: a warm broadleaf forest grows the tropics' trees, its understory without
            // the wood's shrubs.
            let temperate = (6.0..16.0).contains(&s.temperature);
            if woods && !s.is_underwater() && s.tree_density > 0.5 && temperate {
                near.push((i * i + j * j, x, z));
            }
        }
    }
    near.sort_unstable();
    for (_, x, z) in near.into_iter().take(80) {
        let canopy = wg
            .features()
            .trees_in(wg, &veg, (x - 25, z - 25), (x + 25, z + 25))
            .iter()
            .filter(|t| !t.understory && t.stage >= Stage::Young)
            .count();
        if canopy >= 14 {
            return (x, z);
        }
    }
    panic!("no temperate wood near the spawn");
}

#[test]
fn a_cleared_area_goes_through_succession_over_simulated_years() {
    let dir = temp("succession");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 7);
    let at = forest(&w);
    w.go(at.0 as f64 + 0.5, at.1 as f64 + 0.5);
    w.run(10);
    let r = 12;
    let before = census(&w, at, r);

    println!("before: {before:?}");
    assert!(
        before.wood > 20 && before.leaves() > 100,
        "a forest: {before:?}"
    );
    let days_per_year = w.content.time.days_per_season.default as f64 * 4.0;
    let year = |w: &World| w.ticks as f64 / w.ticks_per_day / days_per_year;
    // Cleared: trees and shrubs gone, herbs and grass at once.
    w.server.send(ToServer::Disturb {
        kind: DisturbanceKind::Cleared,
        x: at.0,
        z: at.1,
        radius: 40.0,
    });
    w.run(20);
    let mut table = vec![(year(&w), census(&w, at, r))];

    for target in [1.0, 3.0, 10.0, 40.0, 200.0] {
        // Whole years on: the same season as at the start.
        let hours = (target - year(&w)) * days_per_year * 24.0;
        let want = w.ticks + (hours / 24.0 * w.ticks_per_day) as u64;
        w.server.send(ToServer::SkipHours(hours));
        w.server.send(ToServer::Run(1));
        // The populations about the player are caught up through the skipped years as well
        // (some three tenths of a second a year in a debug build).
        assert!(w.until(180.0, |w| w.ticks >= want), "the clock moved on");
        // The terrain about the player grows again.
        w.run(30);
        table.push((target, census(&w, at, r)));
    }
    for (y, c) in &table {
        println!(
            "year {y:6.1}: wood {:4}, leaves {:4} (pioneers {:4}, between {:4}, shade-tolerant \
             {:4}), herbs {:4}, shrubs {:4}, grass {:4}\n             {:?}",
            c.wood,
            c.leaves(),
            c.pioneer_leaves,
            c.middle_leaves,
            c.tolerant_leaves,
            c.herbs,
            c.shrubs,
            c.grass,
            c.species
        );
    }
    let cleared = &table[0].1;
    assert!(
        cleared.wood == 0 && cleared.leaves() == 0 && cleared.shrubs == 0,
        "cleared: {cleared:?}"
    );
    assert!(
        cleared.herbs + cleared.grass > 20,
        "herbs at once: {cleared:?}"
    );
    let second = &table[1].1;
    assert_eq!(
        second.shrubs, 0,
        "no shrubs in the first two years: {second:?}"
    );
    let shrubs = &table[2].1;
    assert!(shrubs.shrubs > 0, "shrubs from the second year: {shrubs:?}");
    let pioneers = &table[3].1;
    assert!(
        pioneers.leaves() > 20 && pioneers.pioneer_leaves * 2 > pioneers.leaves(),
        "pioneer trees after a few years: {pioneers:?}"
    );
    let young = &table[4].1;
    assert!(young.wood > 20, "a young wood after forty years: {young:?}");
    let old = &table[5].1;
    assert!(
        old.tolerant_leaves > old.pioneer_leaves,
        "the trees of the old forest come back: {old:?}"
    );
    drop(w);
    let _ = std::fs::remove_dir_all(&dir);
}

//! V2-7 acceptance (PLAN.md): fifty years of the generated land about the spawn stay within
//! plausible bounds for every species it holds; heavy hunting there thins the deer and they come
//! back; and every attack on a person, in every kind of encounter, comes with a realistic cause
//! — told in words, and only where it can hold.

use std::sync::Arc;

use glam::DVec3;
use hearth_content::schema::fauna::BodyPlan;
use hearth_fauna::danger::Cause;
use hearth_fauna::ecology::{Cause as Death, Ecology};
use hearth_fauna::habitat::{GenLand, TreeYields};
use hearth_fauna::live::{Footing, Ground, Live, Now, Stage};
use hearth_fauna::mind::{Air, Presence};
use hearth_fauna::species::Catalog;
use hearth_fauna::wound::Blow;
use hearth_math::PlanetSize;
use hearth_worldgen::vegetation::Vegetation;
use hearth_worldgen::{PlanetGrid, Terrain, WorldGenSettings, WorldGenerator};

/// The world of seed 7, the populations of the regions about its spawn (its own and the eight
/// about it, as the game keeps them) made, the spawn's region's key and the spawn.
fn spawn_region() -> (Ecology, (i64, i64), [f64; 2]) {
    let s = WorldGenSettings {
        seed: 7,
        planet_size: PlanetSize::Standard,
        grid_resolution: 256,
    };
    let terrain = Arc::new(Terrain::new(Arc::new(PlanetGrid::build(&s, &|_, _| {}))));
    let reg = hearth_world::datapack::load_builtin_registry().expect("base pack");
    let content = hearth_content::Content::load_base();
    let wg = WorldGenerator::new(terrain, &reg, &content).expect("generator blocks");
    let catalog = Arc::new(Catalog::new(&content));
    let trees = TreeYields::new(&wg, &content);
    let veg = Vegetation::default();
    let land = GenLand {
        wg: &wg,
        veg: &veg,
        catalog: &catalog,
        trees: &trees,
    };
    let (sx, sz) = wg.terrain.find_spawn(false);
    let mut eco = Ecology::new(catalog.clone(), 7, 0.0, &land);
    let key = eco.region_key(sx as f64, sz as f64);
    for dj in -1..=1 {
        for di in -1..=1 {
            eco.ensure_region(&land, (key.0 + di, key.1 + dj), 0.0);
        }
    }
    (eco, key, [sx as f64, sz as f64])
}

#[test]
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn fifty_years_about_the_spawn_stay_within_plausible_bounds() {
    let (mut eco, _, _) = spawn_region();
    let cat = eco.catalog.clone();
    // The species the land holds fifty of at least, two groups' worth of one that lives in
    // groups, and the land of one's range at least (a wolf pack ranges over far more than these
    // regions, a handful of bobcats or bears is at the mercy of chance, and so is a lone herd of
    // onagers or a flock of bustards, the few dozen tapirs of a patch of rainforest in the mosaic
    // about the spawn, or saiga herds roaming ten times the steppe there is: each biome's own runs
    // judge its animals where it is wide, D131).
    let present: Vec<usize> = (0..cat.len())
        .filter(|&s| {
            let sp = &cat.species[s];
            let groups = (sp.group.0 + sp.group.1) as f64;
            let cap = eco.capacity(s);
            cap >= 50.0f64.max(if sp.grouped() { groups } else { 0.0 })
                && cap / sp.density.max(1e-6) as f64 >= sp.home_range_km2 as f64
        })
        .collect();
    let start = eco.regions.values().map(|r| r.time).fold(0.0, f64::max);
    eco.deaths.clear();
    eco.kills.clear();
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
        let died = |c: Death| eco.deaths.get(&(s as u16, c)).copied().unwrap_or(0.0) / 50.0;
        println!(
            "{:<26} capacity {:>9.0}  mean {:>9.0} ({:>4.2})  range {:>9.0} .. {:>9.0}  \
             deaths/yr: natural {:.0} hunger {:.0} winter {:.0} crowding {:.0} predation {:.0} \
             lost {:.0}",
            sp.name,
            cap,
            mean,
            mean / cap,
            lo,
            hi,
            died(Death::Natural),
            died(Death::Hunger),
            died(Death::Winter),
            died(Death::Crowding),
            died(Death::Predation),
            died(Death::Lost),
        );
        let mut by: Vec<(f64, &str)> = eco
            .kills
            .iter()
            .filter(|((_, prey), _)| *prey as usize == s)
            .map(|((hunter, _), n)| (*n / 50.0, cat.species[*hunter as usize].name.as_str()))
            .collect();
        by.sort_by(|a, b| b.0.total_cmp(&a.0));
        if !by.is_empty() {
            let top: Vec<String> = by
                .iter()
                .take(4)
                .map(|(n, name)| format!("{name} {n:.0}"))
                .collect();
            println!("      killed a year by {}", top.join(", "));
        }
        // Within a twentieth and four times what the land holds (its patches, on the borders of
        // realms and kinds of land, hold fewer than a whole wood would), never gone for long (one
        // rare here may vanish a while and come back from the land beyond).
        let gone = v.windows(8).any(|w| w.iter().all(|x| *x < 0.5));
        if mean < 0.05 * cap || mean > 4.0 * cap + 5.0 || gone {
            failures.push(format!("{}: mean {mean:.0} of {cap:.0}", sp.name));
        }
    }
    assert!(
        present.len() >= 20,
        "{} species about the spawn",
        present.len()
    );
    assert!(failures.is_empty(), "out of bounds: {failures:?}");
}

#[test]
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn heavy_hunting_about_the_spawn_thins_the_deer_and_they_come_back() {
    let (mut eco, _, spawn) = spawn_region();
    // A hunting ground 16 km across about the spawn, and the deer most of there.
    let at = spawn;
    let radius = 8_000.0;
    let deer = ["red_deer", "white_tailed_deer", "roe_deer"]
        .iter()
        .map(|id| eco.catalog.index(id).expect(id))
        .max_by(|a, b| {
            eco.count_within(*a, at, radius)
                .total_cmp(&eco.count_within(*b, at, radius))
        })
        .expect("deer");
    println!("hunting {} about the spawn", eco.catalog.species[deer].name);
    let mut t = eco.regions.values().map(|r| r.time).fold(0.0, f64::max);
    let census = |eco: &mut Ecology, t: &mut f64, out: &mut Vec<f64>| {
        *t += 1.0;
        eco.advance(*t, 1.0 / 32.0);
        out.push(eco.count_within(deer, at, radius));
    };
    let mut before = Vec::new();
    for _ in 0..5 {
        census(&mut eco, &mut t, &mut before);
    }
    let base = before.iter().sum::<f64>() / before.len() as f64;
    assert!(base > 20.0, "{base:.0} deer about the spawn");
    // Six years of hunting half of them every year, then it stops.
    let mut during = Vec::new();
    for _ in 0..6 {
        let n = (eco.count_within(deer, at, radius) * 0.5) as u32;
        eco.cull(deer, at, radius, n);
        census(&mut eco, &mut t, &mut during);
    }
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
}

/// Open level ground.
struct Flat;

impl Ground for Flat {
    fn footing(&self, _x: f64, _z: f64, _y: f64) -> Option<Footing> {
        Some(Footing::dry(64.0))
    }

    fn top(&self, x: f64, z: f64) -> Option<Footing> {
        self.footing(x, z, 64.0)
    }
}

/// An encounter: who is about (species, stage, female, where), the person (as they stand and
/// move, hurting one of them first or not), the moment, how long.
struct Encounter {
    name: &'static str,
    animals: Vec<(&'static str, Stage, bool, DVec3)>,
    person: Presence,
    walk: DVec3,
    /// How near the first of them the person walks (an adder is walked over).
    stop_m: f64,
    hurt_first: bool,
    now: Now,
    secs: f32,
}

#[test]
fn every_attack_on_a_person_comes_with_a_realistic_cause() {
    let content = hearth_content::Content::load_base();
    let cat = Arc::new(Catalog::new(&content));
    let land = hearth_fauna::habitat::Uniform {
        habitat: hearth_fauna::habitat::temperate_wood(&cat),
        cells_around: 4096,
    };
    let eco = Ecology::new(cat.clone(), 7, 0.0, &land);
    let night = |year_frac: f32| Now {
        air: Air {
            light: 0.04,
            ..Air::calm_day()
        },
        year_frac,
        ..Now::day(0.95)
    };
    let p = |x: f64, z: f64| DVec3::new(x + 0.5, 64.0, z + 0.5);
    let mut crouched = Presence::stalking(p(0.0, 22.0));
    crouched.vulnerable = 0.85;
    let encounters = vec![
        Encounter {
            name: "a sow and her cub come upon",
            animals: vec![
                ("brown_bear", Stage::Adult, true, p(0.0, 0.0)),
                ("brown_bear", Stage::Young, true, p(2.0, 0.0)),
            ],
            person: Presence::walking(p(0.0, 25.0)),
            walk: DVec3::new(0.0, 0.0, -1.2),
            stop_m: 3.0,
            hurt_first: false,
            now: Now::day(0.4),
            secs: 40.0,
        },
        Encounter {
            name: "wolves at the end of winter, the person crouched alone in the dark",
            animals: (0..3)
                .map(|k| {
                    (
                        "gray_wolf",
                        Stage::Adult,
                        k % 2 == 0,
                        p(k as f64 * 3.0, 0.0),
                    )
                })
                .collect(),
            person: crouched,
            walk: DVec3::ZERO,
            stop_m: 3.0,
            hurt_first: false,
            now: night(0.97),
            secs: 120.0,
        },
        Encounter {
            name: "a boar surprised",
            animals: vec![("wild_boar", Stage::Adult, false, p(0.0, 0.0))],
            person: Presence::walking(p(0.0, 2.5)),
            walk: DVec3::ZERO,
            stop_m: 3.0,
            hurt_first: false,
            now: Now::day(0.4),
            secs: 10.0,
        },
        Encounter {
            name: "an adder walked over",
            animals: vec![("common_european_adder", Stage::Adult, true, p(0.0, 0.0))],
            person: Presence::walking(p(0.0, 3.0)),
            walk: DVec3::new(0.0, 0.0, -1.3),
            stop_m: 0.0,
            hurt_first: false,
            now: Now::day(0.4),
            secs: 6.0,
        },
        Encounter {
            name: "a boar wounded and come up on",
            animals: vec![("wild_boar", Stage::Adult, false, p(0.0, 0.0))],
            person: Presence::walking(p(5.0, 0.0)),
            walk: DVec3::ZERO,
            stop_m: 3.0,
            hurt_first: true,
            now: Now::day(0.4),
            secs: 10.0,
        },
        Encounter {
            name: "a stag in the rut walked up to",
            animals: vec![("red_deer", Stage::Adult, false, p(0.0, 0.0))],
            person: Presence::walking(p(0.0, 25.0)),
            walk: DVec3::new(0.0, 0.0, -1.2),
            stop_m: 3.0,
            hurt_first: false,
            now: Now {
                year_frac: 0.6,
                ..Now::day(0.8)
            },
            secs: 25.0,
        },
        Encounter {
            name: "a red deer hind and calf grazing, walked past",
            animals: vec![
                ("red_deer", Stage::Adult, true, p(0.0, 0.0)),
                ("red_deer", Stage::Young, true, p(3.0, 0.0)),
            ],
            person: Presence::walking(p(30.0, 30.0)),
            walk: DVec3::new(-1.2, 0.0, -1.2),
            stop_m: 3.0,
            hurt_first: false,
            now: Now::day(0.4),
            secs: 40.0,
        },
    ];
    let mut by_cause: std::collections::BTreeMap<String, usize> = Default::default();
    for e in &encounters {
        for seed in 0..12 {
            let mut live = Live::new(2000 + seed);
            let mut ids = Vec::new();
            for &(id, stage, female, at) in &e.animals {
                let s = cat.index(id).expect(id) as u16;
                ids.push(live.place(s, stage, female, at, 0.0));
            }
            // A young one's mother is the grown female placed first.
            let mother = ids[0];
            for a in live.animals.iter_mut() {
                if a.stage == Stage::Young {
                    a.mother = Some(mother);
                }
                a.timer = 30.0;
            }
            if e.hurt_first {
                let a = &live.animals[0];
                let target = a.pos + DVec3::new(0.0, 0.5, -0.4);
                let from = e.person.pos + DVec3::Y * 1.5;
                let path = [from, target + (target - from).normalize() * 2.0];
                if let Some(hit) = live.hit_along(&cat, &path, e.now.year_frac) {
                    live.strike(
                        &cat,
                        &hit,
                        &Blow {
                            energy_j: 30.0,
                            piercing: 0.4,
                            ..Default::default()
                        },
                        "spear",
                        e.person.pos,
                        e.now.year_frac,
                    );
                }
            }
            let mut person = e.person;
            let mut t = 0.0;
            while t < e.secs {
                live.step(&eco, &Flat, Some(&person), &e.now, 0.05);
                for at in &live.attacks {
                    let a = live
                        .animals
                        .iter()
                        .find(|a| a.id == at.animal)
                        .expect("the attacker");
                    let sp = &cat.species[a.species as usize];
                    // Told in words, with its reason.
                    assert!(
                        at.words.contains(at.cause.words()),
                        "{}: {}",
                        e.name,
                        at.words
                    );
                    // Only where the cause can hold.
                    let fits = match at.cause {
                        Cause::Hunger => sp.danger.predatory,
                        Cause::DefendingYoung => {
                            a.female && live.animals.iter().any(|y| y.mother == Some(a.id))
                        }
                        Cause::SteppedNear => sp.danger.venomous && sp.plan == BodyPlan::Snake,
                        Cause::Provoked => a.hurt.since.is_some(),
                        Cause::Rut => !a.female && sp.rut.is_some(),
                        Cause::Surprise | Cause::Cornered => sp.danger.defensive,
                        Cause::DefendingKill => a.kill_at.is_some(),
                    };
                    assert!(fits, "{}: {:?} from a {}", e.name, at.cause, sp.name);
                    *by_cause.entry(format!("{:?}", at.cause)).or_default() += 1;
                }
                if (person.pos - live.animals[0].pos).length() > e.stop_m {
                    person.pos += e.walk * 0.05;
                }
                t += 0.05;
            }
        }
    }
    println!("attacks by cause: {by_cause:?}");
    for cause in [
        "DefendingYoung",
        "Hunger",
        "Surprise",
        "SteppedNear",
        "Provoked",
    ] {
        assert!(
            by_cause.contains_key(cause),
            "no attack for {cause}: {by_cause:?}"
        );
    }
}

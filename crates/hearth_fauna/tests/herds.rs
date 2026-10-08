//! Kept animals through the years (V2-12): a ewe beside a ram in the rut is in lamb and lambs in
//! the spring; her lambs are kept, follow her, grow up and are of a generation on; kept animals
//! save and come back; a grown wild-born one turns wary while a docile one stays calm.

use glam::DVec3;
use hearth_content::Content;
use hearth_fauna::Catalog;
use hearth_fauna::herd::{Breed, Kept, Tiding};
use hearth_fauna::live::{Live, Stage};

fn catalog() -> Catalog {
    Catalog::new(&Content::load_base())
}

/// The year's fraction (0 at the March equinox) of a time in years.
fn frac(years: f64) -> f32 {
    years.rem_euclid(1.0) as f32
}

#[test]
fn a_kept_flock_breeds_in_its_season_and_its_lambs_are_kept() {
    let cat = catalog();
    let sheep = cat.index("hearth:mouflon").expect("mouflon") as u16;
    let mut live = Live::new(5);
    let put = |live: &mut Live, female: bool, x: f64| {
        let id = live.place(sheep, Stage::Adult, female, DVec3::new(x, 0.0, 0.0), 0.0);
        let a = live
            .animals
            .iter_mut()
            .find(|a| a.id == id)
            .expect("placed");
        let mut k = Kept::caught(
            Breed {
                docile: 0.6,
                wool: 0.5,
                milk: 0.3,
            },
            -3.0,
        );
        k.tame = 0.8;
        k.tether = Some((a.pos, 4.0));
        a.kept = Some(k);
        id
    };
    let ram = put(&mut live, false, 0.0);
    let ewes: Vec<u64> = (0..6)
        .map(|i| put(&mut live, true, 2.0 + i as f64))
        .collect();
    // Two years by the week.
    let mut births = 0;
    let mut years = 0.0;
    live.tend(&cat, years, frac(years), false);
    while years < 2.0 {
        years += 1.0 / 52.0;
        for t in live.tend(&cat, years, frac(years), false) {
            if let Tiding::Born { young, .. } = t {
                births += young;
            }
        }
    }
    assert!(births >= 4, "lambs born: {births}");
    let lambs: Vec<_> = live
        .animals
        .iter()
        .filter(|a| a.kept.is_some() && a.id != ram && !ewes.contains(&a.id))
        .collect();
    assert_eq!(lambs.len() as u32, births);
    for l in &lambs {
        let k = l.kept.as_ref().expect("kept");
        assert_eq!(k.generation, 1, "a generation on");
        assert!(
            (0.3..0.95).contains(&k.breed.docile),
            "between its parents': {:?}",
            k.breed
        );
    }
    // Saved and back.
    let saved = live.kept_saved(&cat);
    let mut again = Live::new(6);
    again.restore_kept(&cat, saved.clone());
    assert_eq!(again.kept_saved(&cat).len(), saved.len());
}

#[test]
fn a_wild_born_lamb_raised_by_hand_turns_wary_as_it_grows() {
    let cat = catalog();
    let sheep = cat.index("hearth:mouflon").expect("mouflon") as u16;
    let mut live = Live::new(9);
    let mut ids = Vec::new();
    for (docile, x) in [(0.05, 0.0), (0.9, 5.0)] {
        let id = live.place(sheep, Stage::Young, true, DVec3::new(x, 0.0, 0.0), 0.0);
        let a = live
            .animals
            .iter_mut()
            .find(|a| a.id == id)
            .expect("placed");
        let mut k = Kept::caught(
            Breed {
                docile,
                wool: 0.1,
                milk: 0.1,
            },
            0.0,
        );
        k.hand_reared = docile < 0.5;
        a.kept = Some(k);
        ids.push(id);
    }
    let mut years = 0.2;
    live.tend(&cat, years, frac(years), false);
    while years < 3.0 {
        years += 1.0 / 32.0;
        live.tend(&cat, years, frac(years), false);
    }
    let tame = |id: u64| {
        live.animals
            .iter()
            .find(|a| a.id == id)
            .and_then(|a| a.kept.as_ref())
            .map(|k| (k.tame, k.shyness()))
            .expect("kept")
    };
    let (wild, calm) = (tame(ids[0]), tame(ids[1]));
    assert!(
        wild.0 < 0.5 && wild.1 > 0.25,
        "the wild-born turned wary: {wild:?}"
    );
    assert!(
        calm.0 > 0.8 && calm.1 < 0.05,
        "the docile one stayed calm: {calm:?}"
    );
}

/// The herder's choosing, fast (V2-12): eight wild lambs raised by hand, bred sixteen years by the
/// week with the acceptance's herder (`tests/acceptance_v2_12_herd.rs` in the game, which lives
/// the same years in a whole world, in an hour): the calmest and woolliest ewes and ram kept, the
/// most promising ram lambs and, once the flock has its ewes, the better half of the ewe lambs,
/// the rest culled. Over a dozen flocks, the line grown in the last years is calmer than the first
/// born in the keeping.
#[test]
fn choosing_the_calmest_lambs_breeds_a_calmer_line() {
    let cat = catalog();
    let mut gains = Vec::new();
    for seed in 1..=12 {
        let (t0, t1) = flock_bred(&cat, seed);
        println!("seed {seed}: first born tame {t0:.2}, from year 10 {t1:.2}");
        gains.push(t1 - t0);
    }
    gains.sort_by(f32::total_cmp);
    let median = gains[gains.len() / 2];
    // Some five generations in sixteen years: the grown line calmer by a tenth (its docility up
    // by half again), as the first tame generations of a selected line were.
    assert!(
        median > 0.08,
        "a docile lineage: median gain {median:.2} ({gains:.2?})"
    );
    assert!(
        gains.iter().filter(|g| **g > 0.0).count() >= 10,
        "{gains:.2?}"
    );
}

/// One flock bred sixteen years: the first born's grown temper and the late ones'.
fn flock_bred(cat: &Catalog, seed: u64) -> (f32, f32) {
    let sheep = cat.index("hearth:mouflon").expect("mouflon") as u16;
    let mut rng = hearth_math::hash::Rng::new(seed);
    let mut live = Live::new(seed);
    for i in 0..8 {
        let id = live.place(
            sheep,
            Stage::Young,
            i % 3 != 0,
            DVec3::new(i as f64, 0.0, 0.0),
            0.0,
        );
        let a = live
            .animals
            .iter_mut()
            .find(|a| a.id == id)
            .expect("placed");
        let mut k = Kept::caught(Breed::wild(&mut rng), -0.2);
        k.tether = Some((a.pos, 4.0));
        a.kept = Some(k);
    }
    const EWES: usize = 8;
    let score = |k: &Kept| 2.0 * k.tame + k.breed.wool;
    let mut first_seen: std::collections::HashMap<u64, f64> = Default::default();
    let mut grown_tame: std::collections::HashMap<u64, f32> = Default::default();
    let mut years = 0.0;
    live.tend(cat, years, frac(years), false);
    let founders: Vec<u64> = live.animals.iter().map(|a| a.id).collect();
    while years < 16.0 {
        years += 1.0 / 52.0;
        live.tend(cat, years, frac(years), false);
        for a in live
            .animals
            .iter_mut()
            .filter(|a| !a.dead && a.kept.is_some())
        {
            first_seen.entry(a.id).or_insert(years);
            // Tethered by the camp, near the ram.
            let k = a.kept.as_mut().expect("kept");
            k.tether = Some((DVec3::new((a.id % 10) as f64, 0.0, 0.0), 4.0));
            a.pos = DVec3::new((a.id % 10) as f64, 0.0, 0.0);
            if a.stage == Stage::Adult {
                grown_tame.insert(a.id, k.tame);
            }
        }
        // Once a season, the choosing.
        if (years * 52.0).round() as i64 % 13 != 0 {
            continue;
        }
        let view: Vec<(u64, bool, Stage, f32, f64)> = live
            .animals
            .iter()
            .filter(|a| !a.dead)
            .filter_map(|a| {
                a.kept
                    .as_ref()
                    .map(|k| (a.id, a.female, a.stage, score(k), years - first_seen[&a.id]))
            })
            .collect();
        let mut cull = Vec::new();
        let pick = |female: bool, stage: Stage, old: f64| {
            let mut v: Vec<_> = view
                .iter()
                .filter(|x| x.1 == female && x.2 == stage)
                .cloned()
                .collect();
            let young = v.iter().filter(|x| x.4 <= old).count();
            v.sort_by(|a, b| {
                (a.4 > old && young > 0)
                    .cmp(&(b.4 > old && young > 0))
                    .then(b.3.total_cmp(&a.3))
            });
            v
        };
        let ewes = pick(true, Stage::Adult, 6.0);
        let young_ewes = ewes.iter().filter(|x| x.4 <= 6.0).count();
        cull.extend(ewes.iter().skip(EWES).map(|x| x.0));
        if young_ewes >= EWES / 2 {
            cull.extend(ewes.iter().take(EWES).filter(|x| x.4 > 6.0).map(|x| x.0));
        }
        cull.extend(pick(false, Stage::Adult, 4.0).iter().skip(1).map(|x| x.0));
        cull.extend(
            pick(false, Stage::Juvenile, 99.0)
                .iter()
                .skip(3)
                .map(|x| x.0),
        );
        let ewe_lambs = pick(true, Stage::Juvenile, 99.0);
        let grown = ewes.len();
        if grown >= EWES {
            let keep = (ewe_lambs.len() / 2).max(1);
            cull.extend(ewe_lambs.iter().skip(keep).map(|x| x.0));
        }
        live.animals.retain(|a| !cull.contains(&a.id));
    }
    let mean = |pick: &dyn Fn(u64) -> bool| {
        let v: Vec<f32> = grown_tame
            .iter()
            .filter(|(id, _)| pick(**id))
            .map(|(_, t)| *t)
            .collect();
        (v.iter().sum::<f32>() / v.len().max(1) as f32, v.len())
    };
    let (t0, n0) = mean(&|id| !founders.contains(&id) && first_seen[&id] < 4.0);
    let (t1, n1) = mean(&|id| first_seen[&id] >= 10.0);
    assert!(
        n0 > 0 && n1 > 0,
        "grown sheep of the first and the late years"
    );
    (t0, t1)
}

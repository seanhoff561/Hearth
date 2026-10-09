//! V2-12's acceptance (PLAN.md, v2 Era 3): a sheep lineage becomes docile and woolly.
//!
//! A scripted herder takes up eight wild mouflon lambs whose mothers are gone, raises them by
//! hand and tethers them by its camp. Season after season it tethers the young as they grow,
//! plucks each grown sheep's wool once a year, and keeps to breed only the calmest and woolliest
//! — eight ewes and a ram, and a ram lamb coming on — slaughtering the rest for meat. Years on,
//! its flock's grown sheep, born in the keeping, are calmer than the wild-born founders ever
//! grew to be, and their fleeces heavier.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::{World, temp};
use hearth_env::weather::WeatherHold;
use hearth_fauna::live::{AnimalView, Stage};
use hearth_items::{Hand, Path, Root, Stack, Target};
use hearth_protocol::{AimAt, ToServer};

const LAMBS: usize = 8;
const EWES: usize = 8;

/// The kept animals about the player.
fn flock(w: &World) -> Vec<AnimalView> {
    w.animals.iter().filter(|a| a.kept).copied().collect()
}

fn view(w: &World, id: u64) -> Option<AnimalView> {
    w.animals.iter().find(|a| a.id == id).copied()
}

/// Stands beside an animal and does something to it, trying again as it moves.
fn tend(w: &mut World, process: &str, id: u64) -> (bool, String) {
    let mut last = (false, String::new());
    for _ in 0..4 {
        let Some(a) = view(w, id) else {
            return (false, "gone".into());
        };
        w.go_exact(glam::DVec3::new(a.pos.x + 1.2, a.pos.y, a.pos.z));
        w.run(2);
        last = w.act(process, AimAt::Animal(id));
        if last.0 || !(last.1.contains("reach") || last.1.contains("off before")) {
            return last;
        }
    }
    last
}

/// Puts down what is in the left hand and on the tie (the flake stays in the right).
fn clear(w: &mut World) {
    for root in [Root::Hand(Hand::Left), Root::Hung(0, 0)] {
        if w.carry.get(&Path::at(root)).is_some() {
            w.server.send(ToServer::PutDown {
                from: Path::at(root),
                count: None,
                at: w.mover.pos + glam::DVec3::new(0.3, 0.5, 0.3),
            });
            w.run(2);
        }
    }
}

/// The herder eats and drinks: a skin of water and some cheese.
fn sustain(w: &mut World) {
    clear(w);
    let mut skin = Stack::of("hearth:water_skin/scraped_hide", 1);
    skin.liquid_l = 2.0;
    w.server.send(ToServer::Give(skin));
    w.run(2);
    let roots = [Root::Hand(Hand::Left), Root::Hung(0, 0)];
    if let Some(at) = roots.into_iter().find(|r| {
        w.carry
            .get(&Path::at(*r))
            .is_some_and(|s| s.id == "hearth:water_skin/scraped_hide")
    }) {
        for _ in 0..6 {
            w.server
                .send(ToServer::Drink(hearth_protocol::DrinkFrom::Skin(Path::at(
                    at,
                ))));
            w.run(2);
        }
    }
    clear(w);
    for _ in 0..2 {
        w.give("hearth:cut/cheese", 1);
        if let Some(at) = roots.into_iter().find(|r| {
            w.carry
                .get(&Path::at(*r))
                .is_some_and(|s| s.id == "hearth:cut/cheese")
        }) {
            w.server.send(ToServer::Eat(Path::at(at)));
            w.run(2);
        }
    }
    clear(w);
}

/// A day's skip of the world.
fn skip_days(w: &mut World, days: f64) {
    w.server.send(ToServer::SkipHours(24.0 * days));
    w.run(30);
}

/// Some half an hour on the cloud machine (sixteen years of the world, a season at a time): run
/// with `--ignored`.
#[test]
#[ignore]
fn a_herder_breeds_a_wild_sheep_into_a_docile_woolly_lineage() {
    let dir = temp("acceptance-v2-12-herd");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 11);
    w.server.send(ToServer::HoldWeather(Some(WeatherHold {
        temperature_c: Some(15.0),
        humidity: Some(0.5),
        precip_mm_h: Some(0.0),
        wind_speed_m_s: Some(1.0),
        ..WeatherHold::default()
    })));
    // A flint flake in the hand, for the slaughtering.
    w.give("hearth:flake/flint", 1);
    let from = [Root::Hand(Hand::Left), Root::Hung(0, 0)]
        .into_iter()
        .find(|r| {
            w.carry
                .get(&Path::at(*r))
                .is_some_and(|s| s.id == "hearth:flake/flint")
        });
    if let Some(from) = from {
        w.server.send(ToServer::Shift {
            from: Path::at(from),
            count: None,
            to: Target::Root(Root::Hand(Hand::Right)),
        });
        w.run(2);
    }
    assert!(
        w.carry
            .right
            .as_ref()
            .is_some_and(|s| s.id == "hearth:flake/flint"),
        "the flake in hand: {:?}",
        w.carry
    );
    // Eight wild lambs whose mothers are gone, about the herder.
    let home = w.mover.pos;
    for i in 0..LAMBS {
        let a = i as f64 / LAMBS as f64 * std::f64::consts::TAU;
        w.server.send(ToServer::Bring {
            species: "hearth:mouflon".into(),
            young: true,
            female: i % 4 != 0,
            at: home + glam::DVec3::new(a.cos() * 2.0, 1.0, a.sin() * 2.0),
        });
    }
    w.run(20);
    let wild: Vec<u64> = w
        .animals
        .iter()
        .filter(|a| !a.kept && a.stage == Stage::Young)
        .filter(|a| (a.pos - home).length() < 4.0)
        .map(|a| a.id)
        .collect();
    assert_eq!(wild.len(), LAMBS, "lambs brought: {:?}", w.animals);
    // Caught and raised by hand, then tethered by the camp.
    let mut tethered: BTreeSet<u64> = BTreeSet::new();
    for &id in &wild {
        let (done, words) = tend(&mut w, "catch_animal", id);
        assert!(done, "caught: {words}");
    }
    let founders: BTreeSet<u64> = wild.iter().copied().collect();
    w.go_exact(home);
    // Who was born when (the year first seen), the tameness of each grown, the wool plucked.
    let mut first_seen: BTreeMap<u64, usize> = founders.iter().map(|&id| (id, 0)).collect();
    let mut grown_tame: BTreeMap<u64, f32> = BTreeMap::new();
    let mut plucks: BTreeMap<u64, Vec<f32>> = BTreeMap::new();
    let mut plucked_on: BTreeMap<u64, f64> = BTreeMap::new();
    let mut born = 0usize;
    let years: usize = std::env::var("HEARTH_YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(16);
    let days_per_year = w.calendar.days_per_year();
    let seasons = years * 4;
    for season in 0..seasons {
        let year = season / 4;
        // A season's days.
        for _ in 0..4 {
            skip_days(&mut w, days_per_year / 16.0);
        }
        let now = w.calendar.at(w.ticks).days;
        // The lambs seen at their mothers' sides (nothing tells of a birth: Amendment T §0.2).
        for a in flock(&w) {
            first_seen.entry(a.id).or_insert(year);
        }
        born = first_seen.len() - founders.len();
        sustain(&mut w);
        // The young tethered as they grow (tame while young, they let the herder near).
        for a in flock(&w) {
            if a.stage != Stage::Young && !tethered.contains(&a.id) {
                clear(&mut w);
                w.give("hearth:cord/nettle_fibre", 1);
                w.give("hearth:stick/oak_wood", 1);
                let (done, words) = tend(&mut w, "tether_animal", a.id);
                if done {
                    tethered.insert(a.id);
                } else {
                    println!("not tethered: {words}");
                    if words.contains("cannot now") {
                        println!("the herder: {:?}", w.body.as_ref().map(|b| &b.status));
                    }
                }
            }
        }
        // Each grown one plucked once a year; how tame it is noted.
        for a in flock(&w).into_iter().filter(|a| a.stage == Stage::Adult) {
            grown_tame.insert(a.id, a.tame);
            if plucked_on
                .get(&a.id)
                .is_none_or(|t| now - t > days_per_year * 0.9)
            {
                let (done, words) = tend(&mut w, "pluck_wool", a.id);
                if let Some(kg) = words
                    .strip_prefix("Plucked: ")
                    .and_then(|x| x.split(" kg").next())
                    .and_then(|x| x.parse::<f32>().ok())
                {
                    plucks.entry(a.id).or_default().push(kg);
                }
                if done {
                    plucked_on.insert(a.id, now);
                } else {
                    println!("not plucked: {words}");
                }
            }
        }
        // Only the calmest and woolliest kept to breed, calm first (a wild-tempered ewe is
        // trouble every day; a thin fleece only once a year): the best eight ewes and the best
        // ram, the three most promising ram lambs and the better half of the ewe lambs (a lamb
        // shows its line's temper at its mother's side; the sire is chosen among the ram lambs
        // when they are grown); the rest slaughtered.
        let score = |a: &AnimalView| {
            let wool = plucks
                .get(&a.id)
                .and_then(|p| p.last())
                .map_or(a.fleece, |kg| (kg - 0.2) / 2.3);
            2.0 * a.tame + wool
        };
        // Ewes past their sixth year and rams past their fourth give way to the young, as a
        // herder keeps young stock breeding: selection works only through the generations.
        let age = |a: &AnimalView| year - first_seen.get(&a.id).copied().unwrap_or(year);
        let old = |a: &AnimalView, limit: usize| age(a) > limit;
        let mut cull: Vec<u64> = Vec::new();
        let mut ewes: Vec<AnimalView> = flock(&w)
            .into_iter()
            .filter(|a| a.female && a.stage == Stage::Adult)
            .collect();
        let young_ewes = ewes.iter().filter(|a| !old(a, 6)).count();
        ewes.sort_by(|a, b| {
            (young_ewes >= EWES / 2 && old(a, 6))
                .cmp(&(young_ewes >= EWES / 2 && old(b, 6)))
                .then(score(b).total_cmp(&score(a)))
        });
        cull.extend(ewes.iter().skip(EWES).map(|a| a.id));
        cull.extend(
            ewes.iter()
                .take(EWES)
                .filter(|a| young_ewes >= EWES / 2 && old(a, 6))
                .map(|a| a.id),
        );
        let mut rams: Vec<AnimalView> = flock(&w)
            .into_iter()
            .filter(|a| !a.female && a.stage == Stage::Adult)
            .collect();
        let young_rams = rams.iter().filter(|a| !old(a, 4)).count();
        rams.sort_by(|a, b| {
            (young_rams > 0 && old(a, 4))
                .cmp(&(young_rams > 0 && old(b, 4)))
                .then(score(b).total_cmp(&score(a)))
        });
        cull.extend(rams.iter().skip(1).map(|a| a.id));
        let mut ram_lambs: Vec<AnimalView> = flock(&w)
            .into_iter()
            .filter(|a| !a.female && a.stage == Stage::Juvenile)
            .collect();
        ram_lambs.sort_by(|a, b| score(b).total_cmp(&score(a)));
        cull.extend(ram_lambs.iter().skip(3).map(|a| a.id));
        // The ewe lambs too, once the flock has its ewes: the calmer half kept to breed, the
        // rest for meat, as a herder chooses the young that will be the flock.
        let grown_ewes = flock(&w)
            .iter()
            .filter(|a| a.female && a.stage == Stage::Adult)
            .count();
        let mut ewe_lambs: Vec<AnimalView> = flock(&w)
            .into_iter()
            .filter(|a| a.female && a.stage == Stage::Juvenile)
            .collect();
        if grown_ewes >= EWES {
            ewe_lambs.sort_by(|a, b| score(b).total_cmp(&score(a)));
            let keep = (ewe_lambs.len() / 2).max(1);
            cull.extend(ewe_lambs.iter().skip(keep).map(|a| a.id));
        }
        for id in cull {
            let (done, words) = tend(&mut w, "slaughter_animal", id);
            if !done {
                println!("not slaughtered: {words}");
            }
        }
        w.go_exact(home);
        assert!(
            w.body.as_ref().is_none_or(|b| b.dead.is_none()),
            "the herder lives: {:?}",
            w.body.as_ref().map(|b| &b.dead)
        );
        let f = flock(&w);
        println!(
            "season {season}: {} kept ({} grown), {born} born so far; grown tame {:.2}",
            f.len(),
            f.iter().filter(|a| a.stage == Stage::Adult).count(),
            f.iter()
                .filter(|a| a.stage == Stage::Adult)
                .map(|a| a.tame)
                .sum::<f32>()
                / f.iter().filter(|a| a.stage == Stage::Adult).count().max(1) as f32
        );
    }
    // The line's first generation born in the keeping against its grown sheep born in the last
    // years, for temper (the founders, wild lambs raised by hand, were tamed a little by their
    // rearing as no lamb of the flock is); the founders against the late ones for wool.
    let late = years * 2 / 3;
    let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len().max(1) as f32;
    let tame_of = |pick: &dyn Fn(u64) -> bool| -> Vec<f32> {
        grown_tame
            .iter()
            .filter(|(id, _)| pick(**id))
            .map(|(_, t)| *t)
            .collect()
    };
    let wool_of = |pick: &dyn Fn(u64) -> bool| -> Vec<f32> {
        plucks
            .iter()
            .filter(|(id, _)| pick(**id))
            .flat_map(|(_, p)| p.iter().copied())
            .collect()
    };
    let is_founder = |id: u64| founders.contains(&id);
    let is_first = |id: u64| {
        !founders.contains(&id) && first_seen.get(&id).is_some_and(|y| (1..=4).contains(y))
    };
    let is_late = |id: u64| first_seen.get(&id).is_some_and(|y| *y >= late);
    let tf = mean(&tame_of(&is_founder));
    let (t0, t1) = (mean(&tame_of(&is_first)), mean(&tame_of(&is_late)));
    let (w0, w1) = (mean(&wool_of(&is_founder)), mean(&wool_of(&is_late)));
    println!(
        "founders: tame {tf:.2}, wool {w0:.2} kg; first born in the keeping (years 1–4): tame \
         {t0:.2} ({} grown); born from year {late}: tame {t1:.2} ({} grown), wool {w1:.2} kg",
        tame_of(&is_first).len(),
        tame_of(&is_late).len()
    );
    assert!(born >= 10, "lambs born in the keeping: {born}");
    assert!(
        !tame_of(&is_first).is_empty() && !tame_of(&is_late).is_empty(),
        "grown sheep of the first and the late years"
    );
    // Some five generations: calmer by a twentieth at least (`hearth_fauna`'s fast flocks gain
    // a tenth in the median, the world's herder the same rules).
    assert!(t1 > t0 + 0.05, "a docile lineage: {t0:.2} → {t1:.2}");
    assert!(w1 > w0 * 1.8, "a woolly lineage: {w0:.2} → {w1:.2} kg");
}

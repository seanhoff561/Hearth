//! V2-12's acceptance (PLAN.md, v2 Era 3): a bot domesticates a grain over simulated
//! generations and sees its yields rise.
//!
//! A scripted farmer breaks eight square metres of ground with a digging stick and sows them
//! with wild einkorn stripped from the hills. Year after year it sows in autumn, spreads dung,
//! weeds once, reaps the ripe crop with a flint sickle, threshes the sheaves and picks the
//! plumpest third of the grain to sow again. Its seed changes as the generations pass — the
//! ears come to hold their grain, the seed sprouts at once, the grain grows plumper — and what
//! a square metre gives rises.

mod common;

use common::{World, temp};
use hearth_env::weather::WeatherHold;
use hearth_items::{Carry, Hand, Lot, Path, Root, Stack, Target};
use hearth_math::BlockPos;
use hearth_protocol::{AimAt, ToServer};

const PLOTS: usize = 8;

/// Every stack of a kind carried, with its path.
fn carried(carry: &Carry, id: &str) -> Vec<(Path, Stack)> {
    fn walk(path: &Path, s: &Stack, id: &str, out: &mut Vec<(Path, Stack)>) {
        if s.id == id {
            out.push((path.clone(), s.clone()));
        }
        if let Some(c) = s.contents() {
            for (i, p) in c.items.iter().enumerate() {
                walk(&path.inner(i), &p.stack, id, out);
            }
        }
    }
    let mut out = Vec::new();
    let mut roots: Vec<(Root, &Stack)> = Vec::new();
    if let Some(s) = &carry.right {
        roots.push((Root::Hand(Hand::Right), s));
    }
    if let Some(s) = &carry.left {
        roots.push((Root::Hand(Hand::Left), s));
    }
    if let Some(s) = &carry.back {
        roots.push((Root::Back, s));
    }
    for (i, w) in carry.worn.iter().enumerate() {
        for (p, h) in w.hung.iter().enumerate() {
            if let Some(s) = h {
                roots.push((Root::Hung(i, p), s));
            }
        }
    }
    for (root, s) in roots {
        walk(&Path::at(root), s, id, &mut out);
    }
    out
}

/// Lays down what is at a path in front of the feet, where the year's grain is picked over.
fn lay_out(w: &mut World, path: Path) {
    w.server.send(ToServer::PutDown {
        from: path,
        count: None,
        at: w.mover.pos + glam::DVec3::new(0.0, 0.5, 1.5),
    });
    w.run(2);
}

/// Puts what is at a path aside, out of hand's reach (the culls are eaten, not sown).
fn put_aside(w: &mut World, path: Path) {
    w.server.send(ToServer::PutDown {
        from: path,
        count: None,
        at: w.mover.pos + glam::DVec3::new(0.0, 1.0, -2.7),
    });
    w.run(2);
}

/// Puts aside everything of a kind carried.
fn put_aside_all(w: &mut World, id: &str) {
    for (p, _) in carried(&w.carry, id).into_iter().rev() {
        put_aside(w, p);
    }
}

/// Takes a tool carried into the right hand (it works only in a hand).
fn in_hand(w: &mut World, id: &str) {
    if let Some((p, _)) = carried(&w.carry, id).into_iter().next() {
        w.server.send(ToServer::Shift {
            from: p,
            count: None,
            to: Target::Root(Root::Hand(Hand::Right)),
        });
        w.run(2);
    }
    assert!(
        w.carry.right.as_ref().is_some_and(|s| s.id == id),
        "{id} in hand: {:?}",
        w.carry
    );
}

/// The crop's stage on a plot (none: nothing stands there).
fn stage(w: &World, plot: BlockPos) -> Option<u8> {
    let s = w.mirror.block(plot.up())?;
    if w.reg.block_of(s).name.path() != "einkorn_crop" {
        return None;
    }
    w.reg.get(s, "stage").and_then(|v| v.parse().ok())
}

/// Days of the world, skipped.
fn skip_days(w: &mut World, days: f64) {
    w.server.send(ToServer::SkipHours(24.0 * days));
    w.run(30);
}

/// Upland out of reach of the river's spring floods: walks away from any water about until
/// there is none near.
fn upland(w: &mut World) {
    for _ in 0..10 {
        let wet = w.find(32, |n, _| n == "water" || n == "ice");
        if wet.is_empty() {
            return;
        }
        let n = wet.len() as f64;
        let (cx, cz) = wet.iter().fold((0.0, 0.0), |(x, z), p| {
            (x + p.x as f64 / n, z + p.z as f64 / n)
        });
        let away =
            glam::DVec2::new(w.mover.pos.x - cx, w.mover.pos.z - cz).normalize_or(glam::DVec2::X);
        let to = glam::DVec2::new(w.mover.pos.x, w.mover.pos.z) + away * 48.0;
        w.go(to.x, to.y);
    }
}

/// Squares of open ground about the player to till, within reach: the top of soil with air
/// above, the nearest first.
fn plots(w: &World) -> Vec<BlockPos> {
    let g = w.ground();
    let eye = w.mover.pos + glam::DVec3::new(0.0, 1.6, 0.0);
    let mut around: Vec<(i32, i32)> = (-3i32..=3)
        .flat_map(|dx| (-3i32..=3).map(move |dz| (dx, dz)))
        .filter(|&(dx, dz)| (dx, dz) != (0, 0))
        .collect();
    around.sort_by_key(|&(dx, dz)| dx * dx + dz * dz);
    let mut out = Vec::new();
    for (dx, dz) in around {
        {
            let mut p = BlockPos::new(g.x + dx, g.y + 3, g.z + dz);
            for _ in 0..7 {
                let solid = w
                    .mirror
                    .block(p)
                    .is_some_and(|s| !w.reg.collision_shape(s).is_empty());
                if solid {
                    let name = w.block(p).unwrap_or_default();
                    let soil = matches!(name.as_str(), "grass_block" | "podzol")
                        || name.contains("loam")
                        || name.contains("soil");
                    let centre = glam::DVec3::new(p.x as f64, p.y as f64, p.z as f64) + 0.5;
                    if soil
                        && w.block(p.up().up()).as_deref().is_none_or(|n| n == "air")
                        && (centre - eye).length() < 3.8
                    {
                        out.push(p);
                    }
                    break;
                }
                p = p.down();
            }
            if out.len() == PLOTS {
                return out;
            }
        }
    }
    out
}

/// Some fifty minutes on the cloud machine (each harvest is a year of the world, skipped a day
/// or two at a time): run with `--ignored`.
#[test]
#[ignore]
fn a_bot_domesticates_a_grain_over_the_generations_and_its_yields_rise() {
    let dir = temp("acceptance-v2-12-fields");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 11);
    // Mild, dry weather to work in (the year's rain is the climate's, in the crop's reckoning).
    w.server.send(ToServer::HoldWeather(Some(WeatherHold {
        temperature_c: Some(15.0),
        humidity: Some(0.5),
        precip_mm_h: Some(0.0),
        wind_speed_m_s: Some(1.0),
        ..WeatherHold::default()
    })));
    upland(&mut w);
    w.give("hearth:digging_stick/oak_wood", 1);
    let squares = plots(&w);
    assert_eq!(squares.len(), PLOTS, "ground to farm: {squares:?}");
    for &p in &squares {
        let (done, words) = w.act("till_soil", AimAt::Block { pos: p, top: true });
        assert!(done, "tilled: {words}");
    }
    put_aside_all(&mut w, "hearth:digging_stick/oak_wood");
    // A basket on the back for the grain and the sheaves.
    w.give("hearth:back_basket/reed", 1);
    let basket = carried(&w.carry, "hearth:back_basket/reed")
        .first()
        .map(|(p, _)| p.clone())
        .expect("a basket");
    w.server.send(ToServer::Shift {
        from: basket,
        count: None,
        to: Target::Root(Root::Back),
    });
    w.run(2);
    assert!(w.carry.back.is_some(), "the basket on the back");
    // Wild einkorn, stripped from the hills.
    w.give("hearth:grains/einkorn", (PLOTS * 3) as u16);
    // Twelve harvests (fewer to look into a failure: HEARTH_GENERATIONS).
    let generations: usize = std::env::var("HEARTH_GENERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(12);
    let mut yields: Vec<f32> = Vec::new();
    let mut lots: Vec<Lot> = Vec::new();
    for generation in 0..generations {
        // The year's grain laid out and picked over a scoop at a time: the plumpest third kept
        // for seed, the rest put aside to eat.
        for (p, _) in carried(&w.carry, "hearth:grains/einkorn").into_iter().rev() {
            lay_out(&mut w, p);
        }
        let spot = w.mover.pos + glam::DVec3::new(0.0, 0.0, 1.5);
        let mut picked: Vec<f32> = Vec::new();
        for _ in 0..PLOTS * 3 {
            let pile = w
                .lying
                .iter()
                .filter(|i| i.stack.id == "hearth:grains/einkorn" && i.stack.count >= 3)
                .filter(|i| {
                    let d = glam::DVec3::from_array(i.pos) - spot;
                    glam::DVec2::new(d.x, d.z).length() < 0.8
                })
                .max_by_key(|i| i.stack.count)
                .map(|i| (i.id, i.stack.lot));
            let Some((pile, lot)) = pile else {
                break;
            };
            let (done, words) = w.act("select_seed", AimAt::Thing(pile));
            assert!(done, "{words}");
            picked.extend(lot.map(|l| l.grain_mg));
            if carried(&w.carry, "hearth:grains/einkorn")
                .iter()
                .filter(|(_, s)| {
                    s.lot.is_some_and(|l| {
                        l.grain_mg > picked.iter().sum::<f32>() / picked.len() as f32
                    })
                })
                .map(|(_, s)| s.count as usize)
                .sum::<usize>()
                >= PLOTS
            {
                break;
            }
        }
        // What was picked out is plumper than the grain it was picked from; the rest aside.
        let mean = picked.iter().sum::<f32>() / picked.len().max(1) as f32;
        let held = carried(&w.carry, "hearth:grains/einkorn");
        let culls: Vec<Path> = held
            .iter()
            .filter(|(_, s)| s.lot.is_none_or(|l| l.grain_mg <= mean))
            .map(|(p, _)| p.clone())
            .collect();
        for p in culls.into_iter().rev() {
            put_aside(&mut w, p);
        }
        let seed = carried(&w.carry, "hearth:grains/einkorn")
            .iter()
            .filter_map(|(_, s)| s.lot.map(|l| (l, s.count as f32)))
            .reduce(|(a, n), (b, m)| (a.mixed(&b, n, m), n + m))
            .map(|(l, _)| l)
            .expect("seed to sow");
        lots.push(seed);
        // The leftover scoops picked up and put aside too.
        let left: Vec<u64> = w
            .lying
            .iter()
            .filter(|i| i.stack.id == "hearth:grains/einkorn")
            .filter(|i| {
                let d = glam::DVec3::from_array(i.pos) - spot;
                glam::DVec2::new(d.x, d.z).length() < 0.8
            })
            .map(|i| i.id)
            .collect();
        for id in left {
            if w.pick_up(id) {
                let back: Vec<Path> = carried(&w.carry, "hearth:grains/einkorn")
                    .into_iter()
                    .filter(|(_, s)| s.lot.is_none_or(|l| l.grain_mg <= mean))
                    .map(|(p, _)| p)
                    .collect();
                for p in back.into_iter().rev() {
                    put_aside(&mut w, p);
                }
            }
        }
        // Autumn: sow every plot there is seed for.
        let mut sown = Vec::new();
        for _ in 0..40 {
            let (done, words) = w.act(
                "sow_seed",
                AimAt::Block {
                    pos: squares[0],
                    top: true,
                },
            );
            if done {
                sown.push(squares[0]);
                break;
            }
            assert!(
                words.contains("sow in autumn")
                    || words.contains("no time for sowing")
                    || words.contains("too cold")
                    || words.contains("too late"),
                "{words}"
            );
            skip_days(&mut w, 1.0);
        }
        assert!(!sown.is_empty(), "the season for sowing came");
        for &p in &squares[1..] {
            if carried(&w.carry, "hearth:grains/einkorn").is_empty() {
                break;
            }
            let (done, _) = w.act("sow_seed", AimAt::Block { pos: p, top: true });
            if done {
                sown.push(p);
            }
        }
        // What seed is left over is eaten.
        put_aside_all(&mut w, "hearth:grains/einkorn");
        // Dung dug in, to keep the soil.
        for &p in &sown {
            w.give("hearth:dung_cake/dung", 5);
            w.act("manure_plot", AimAt::Block { pos: p, top: true });
            put_aside_all(&mut w, "hearth:dung_cake/dung");
        }
        // The year turns: weeded once in the spring, reaped when ripe.
        let mut weeded = false;
        for _ in 0..60 {
            skip_days(&mut w, 2.0);
            let st = stage(&w, sown[0]);
            if !weeded && st.is_some_and(|s| s >= 2) {
                for &p in &sown {
                    w.act(
                        "weed_plot",
                        AimAt::Block {
                            pos: p.up(),
                            top: true,
                        },
                    );
                }
                weeded = true;
            }
            if st == Some(4) {
                break;
            }
        }
        assert_eq!(stage(&w, sown[0]), Some(4), "ripe");
        w.give("hearth:sickle/flint", 1);
        in_hand(&mut w, "hearth:sickle/flint");
        let mut reaped = 0.0f32;
        for &p in &sown {
            let (_, words) = w.act(
                "reap_crop",
                AimAt::Block {
                    pos: p.up(),
                    top: true,
                },
            );
            if let Some(kg) = words
                .split('(')
                .nth(1)
                .and_then(|x| x.split(" kg").next())
                .and_then(|x| x.trim().parse::<f32>().ok())
            {
                reaped += kg;
            }
        }
        put_aside_all(&mut w, "hearth:sickle/flint");
        // Threshed, the straw put by.
        loop {
            if carried(&w.carry, "hearth:sheaf_of/einkorn").is_empty() {
                break;
            }
            let (done, words) = w.act("thresh_sheaf", AimAt::Nothing);
            assert!(done, "{words}");
            put_aside_all(&mut w, "hearth:handful/dry_grass");
        }
        let per_m2 = reaped / sown.len() as f32;
        println!(
            "generation {generation}: {} plots sown with {seed:?}; reaped {reaped:.2} kg, {per_m2:.3} kg a square metre",
            sown.len()
        );
        yields.push(per_m2);
    }
    let first: f32 = yields[..3].iter().sum::<f32>() / 3.0;
    let last: f32 = yields[generations - 3..].iter().sum::<f32>() / 3.0;
    println!("yields: {yields:?}");
    let (a, z) = (lots[0], lots[generations - 1]);
    assert!(
        z.grain_mg > a.grain_mg * 1.3,
        "plumper grain: {a:?} → {z:?}"
    );
    assert!(
        z.tough > a.tough * 5.0,
        "the ears come to hold their grain: {a:?} → {z:?}"
    );
    assert!(
        z.dormant < a.dormant * 0.5,
        "the seed sprouts at once: {a:?} → {z:?}"
    );
    assert!(last > 1.5 * first, "yields rise: {first:.3} → {last:.3}");
}

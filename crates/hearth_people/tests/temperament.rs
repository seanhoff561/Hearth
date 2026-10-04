//! H2 acceptance (V2.1 §5.1, §18): temperament tilts behaviour, modestly. Ten bands of
//! *Australopithecus* live a day in the savanna; how much of its time each grown one spends
//! grooming the others follows its sociability, and its extraversion behind that — a
//! correlation there to be found, but far from deciding (the needs of the body, the hour and
//! chance decide most of it), as studies of personality and everyday behaviour find.

mod common;

use common::*;
use glam::DVec3;
use hearth_fauna::live::Stage;
use hearth_people::{Doing, People, Tendency};

fn correlation(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len() as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let cov: f64 = a.iter().zip(b).map(|(x, y)| (x - ma) * (y - mb)).sum();
    let va: f64 = a.iter().map(|x| (x - ma) * (x - ma)).sum();
    let vb: f64 = b.iter().map(|y| (y - mb) * (y - mb)).sum();
    cov / (va * vb).sqrt()
}

#[test]
fn the_sociable_groom_more_but_not_always() {
    let b = base();
    let mut world = Savanna::new();
    let mut p = People::new(31);
    let k = b.species.index_of("australopithecus").expect("the hominin");
    // Ten bands about the marula, each in its own place.
    for i in 0..10 {
        let a = i as f64 * std::f64::consts::TAU / 10.0;
        let at = DVec3::new(20.0 + 30.0 * a.cos(), GROUND, 30.0 * a.sin());
        let now = world.now();
        p.spawn_band(
            &b.species,
            &b.graph,
            &b.items,
            &mut world,
            k,
            [4, 3, 1, 0],
            at,
            now,
        );
    }
    // A day, the grown ones' doings counted every few seconds.
    let mut counts: std::collections::BTreeMap<u64, (u32, u32)> = Default::default();
    let steps = (DAY_S / DT) as u64;
    for n in 0..steps {
        step(&mut p, &mut world, &[]);
        if !n.is_multiple_of(8) {
            continue;
        }
        let now = world.now();
        let sp = &b.species.list[k];
        for q in p.full() {
            if q.stage(sp, &now) != Stage::Adult {
                continue;
            }
            let c = counts.entry(q.id).or_default();
            c.1 += 1;
            if matches!(q.mind.doing, Doing::Grooming { .. }) {
                c.0 += 1;
            }
        }
    }
    let (mut share, mut sociable, mut extraverted) = (Vec::new(), Vec::new(), Vec::new());
    for (id, (groomed, seen)) in &counts {
        let q = p.get(*id).expect("them");
        share.push(*groomed as f64 / (*seen).max(1) as f64);
        sociable.push(q.psyche.tendency(Tendency::Sociability) as f64);
        extraverted.push(
            q.phenotype
                .as_ref()
                .map_or(0.0, |ph| ph.z("extraversion") as f64),
        );
    }
    let mean = share.iter().sum::<f64>() / share.len() as f64;
    let r = correlation(&sociable, &share);
    let rx = correlation(&extraverted, &share);
    println!(
        "{} grown ones; grooming {:.0}% of the time; with sociability r = {r:.2}, with extraversion r = {rx:.2}",
        share.len(),
        mean * 100.0
    );
    assert!(share.len() >= 60);
    assert!(
        (0.12..0.75).contains(&r),
        "sociability and grooming: r = {r:.2}, not a modest tilt"
    );
    assert!(rx > 0.0, "extraversion and grooming: r = {rx:.2}");
}

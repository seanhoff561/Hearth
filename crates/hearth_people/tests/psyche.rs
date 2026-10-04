//! H2 (V2.1 §5): the psyche. Tendencies lean on temperament; a fright seen spreads through those
//! near and fades; a death grieves the kin.

mod common;

use common::*;
use hearth_math::hash::Rng;
use hearth_people::{Feeling, Psyche, Tendency};

fn correlation(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len() as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let cov: f64 = a.iter().zip(b).map(|(x, y)| (x - ma) * (y - mb)).sum();
    let va: f64 = a.iter().map(|x| (x - ma) * (x - ma)).sum();
    let vb: f64 = b.iter().map(|y| (y - mb) * (y - mb)).sum();
    cov / (va * vb).sqrt()
}

#[test]
fn tendencies_lean_on_temperament() {
    let b = base();
    let g = b.species.genetics.as_ref().expect("the genetics");
    let pool = g.pool("homo_sapiens").expect("the human pool");
    let mut rng = Rng::new(41);
    let mut rows: Vec<[f64; 4]> = Vec::new();
    for k in 0..1500 {
        let x = g.founder(pool, 0.5, k % 2 == 0, &mut rng);
        let ph = g.phenotype(&x, &mut rng);
        let ps = Psyche::form(&b.species.psyche, Some(&ph), 0.0);
        rows.push([
            ps.tendency(Tendency::RiskTolerance) as f64,
            ph.z("emotionality") as f64,
            ps.tendency(Tendency::Sociability) as f64,
            ph.z("extraversion") as f64,
        ]);
    }
    let col = |i: usize| rows.iter().map(|r| r[i]).collect::<Vec<f64>>();
    let risk = correlation(&col(0), &col(1));
    let social = correlation(&col(2), &col(3));
    println!("risk tolerance ~ emotionality {risk:.2}; sociability ~ extraversion {social:.2}");
    assert!((-0.85..-0.3).contains(&risk), "{risk:.2}");
    assert!((0.3..0.85).contains(&social), "{social:.2}");
    // Every tendency in its range, and not everyone alike.
    let spread = col(0)
        .iter()
        .fold((1.0f64, 0.0f64), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
    assert!(spread.0 >= 0.1 && spread.1 <= 0.9 && spread.1 - spread.0 > 0.2);
}

#[test]
fn a_fright_spreads_through_those_near_and_fades() {
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    // One of them frightened by what the others did not see.
    let first = p.full().next().map(|q| q.id).expect("a person");
    let near: Vec<u64> = {
        let at = p.get(first).expect("first").place.pos;
        p.full()
            .filter(|q| q.id != first && (q.place.pos - at).length() < 8.0)
            .map(|q| q.id)
            .collect()
    };
    assert!(!near.is_empty(), "others stand near");
    if let Some(q) = p.persons.iter_mut().find(|q| q.id == first) {
        q.psyche.feel(Feeling::Fear, 1.0);
    }
    live(&mut p, &mut world, 4.0, &[]);
    let caught: Vec<f32> = near
        .iter()
        .map(|id| p.get(*id).expect("near").psyche.feeling(Feeling::Fear))
        .collect();
    println!("fear caught by those near: {caught:?}");
    assert!(caught.iter().all(|f| *f > 0.08), "{caught:?}");
    // In safety it is gone in some minutes.
    live(&mut p, &mut world, 400.0, &[]);
    let left = p
        .full()
        .map(|q| q.psyche.feeling(Feeling::Fear))
        .fold(0.0f32, f32::max);
    assert!(left < 0.05, "fear left: {left:.3}");
}

#[test]
fn a_death_grieves_the_kin() {
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    let (child, mother) = p
        .full()
        .find_map(|q| q.life.mother.map(|m| (q.id, m)))
        .expect("a child with its mother");
    if let Some(m) = p.persons.iter_mut().find(|q| q.id == mother) {
        m.body.kill(hearth_body::Death::Starvation);
    }
    live(&mut p, &mut world, 2.0, &[]);
    assert!(p.get(mother).is_some_and(|m| !m.alive()));
    let grief = p
        .get(child)
        .expect("the child")
        .psyche
        .feeling(Feeling::Grief);
    println!("the child's grief: {grief:.2}");
    assert!(grief > 0.4, "{grief:.2}");
    // A stranger to her (another band's, or no kin) is not grieved for as kin are.
    let mood = p.get(child).expect("the child").psyche.mood;
    live(&mut p, &mut world, 60.0, &[]);
    assert!(p.get(child).expect("the child").psyche.mood <= mood + 0.05);
}

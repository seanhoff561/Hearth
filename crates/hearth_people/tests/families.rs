//! H1 (V2.1 §4): a band's people carry genomes from its species' pool, its children the
//! meiosis of their mother's and their father's — a father not of the mother's close kin — and
//! the genes are saved with them.

mod common;

use common::*;

#[test]
fn a_band_carries_genes_and_its_children_their_parents() {
    let mut world = Savanna::new();
    let p = band(&mut world);
    let members: Vec<&hearth_people::Person> = p.full().collect();
    assert!(
        members
            .iter()
            .all(|q| q.genome.is_some() && q.phenotype.is_some())
    );
    // A child with both parents known has one copy of each locus from each of them.
    let mut checked = 0;
    for c in &members {
        let (Some(m), Some(f)) = (c.life.mother, c.life.father) else {
            continue;
        };
        let (Some(m), Some(f)) = (p.get(m), p.get(f)) else {
            continue;
        };
        // A forebear kept as a genealogy stub (H7) has let her genome go.
        let (Some(mg), Some(fg)) = (m.genome.as_ref(), f.genome.as_ref()) else {
            continue;
        };
        let cg = c.genome.as_ref().expect("genome");
        let n = cg.maternal.len();
        let from_mother = (0..n)
            .filter(|&i| cg.maternal[i] == mg.maternal[i] || cg.maternal[i] == mg.paternal[i])
            .count();
        assert!(
            from_mother * 100 >= n * 99,
            "{from_mother} of {n} from the mother"
        );
        let from_father = (0..n)
            .filter(|&i| {
                cg.paternal[i] == hearth_people::genome::NONE
                    || cg.paternal[i] == fg.maternal[i]
                    || cg.paternal[i] == fg.paternal[i]
            })
            .count();
        assert!(
            from_father * 100 >= n * 99,
            "{from_father} of {n} from the father"
        );
        // And the father is no close kinsman of the mother.
        let mut kin = hearth_people::Kinship::new(&p);
        assert!(kin.of(m.id, f.id) < 0.125);
        checked += 1;
    }
    println!("{checked} children checked against their parents");
    // Grown brothers and sisters of a forebear (H7) share more of their genes than unrelated
    // grown ones do: alike at a locus when they carry an allele in common.
    let alike = |a: &hearth_people::Person, b: &hearth_people::Person| {
        let (ga, gb) = (a.genome.as_ref().expect("genome"), b.genome.as_ref().expect("genome"));
        let n = ga.maternal.len();
        let shared = (0..n)
            .filter(|&i| {
                let x = [ga.maternal[i], ga.paternal[i]];
                let y = [gb.maternal[i], gb.paternal[i]];
                x.iter().any(|&v| v != hearth_people::genome::NONE && y.contains(&v))
            })
            .count();
        shared as f64 / n as f64
    };
    let stubbed = |q: &hearth_people::Person| {
        q.life
            .mother
            .and_then(|m| p.get(m))
            .filter(|m| m.life.stub)
            .map(|m| m.id)
    };
    let (mut sibs, mut others) = (Vec::new(), Vec::new());
    for (i, a) in members.iter().enumerate() {
        for b in &members[i + 1..] {
            let (Some(x), Some(y)) = (stubbed(a), stubbed(b)) else {
                continue;
            };
            if x == y {
                sibs.push(alike(a, b));
            } else {
                others.push(alike(a, b));
            }
        }
    }
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
    println!(
        "{} sibling pairs alike at {:.3} of loci, {} unrelated at {:.3}",
        sibs.len(),
        mean(&sibs),
        others.len(),
        mean(&others)
    );
    if !sibs.is_empty() && !others.is_empty() {
        assert!(mean(&sibs) > mean(&others) + 0.02);
    }
    // Folded and drawn out again, they are the same people with the same genes.
    let before: Vec<_> = p.persons.iter().map(|q| (q.id, q.genome.clone())).collect();
    let save = hearth_people::save::to_json(&p.to_save()).expect("json");
    let back = hearth_people::People::from_save(
        p.seed,
        hearth_people::save::from_json(&save).expect("read"),
    );
    let after: Vec<_> = back
        .persons
        .iter()
        .map(|q| (q.id, q.genome.clone()))
        .collect();
    assert_eq!(before, after);
}

#[test]
fn our_families_know_what_their_country_takes() {
    // Wild Earth's families (D164): the warm country's ways, and where it is cold fire, hide
    // wraps and a windbreak besides.
    let b = base();
    let us = &b.species.list[b.species.index_of("homo_sapiens").expect("our species")];
    let knows = |list: &[String], id: &str| list.iter().any(|k| k.ends_with(id));
    let (warm, warm_ways) = us.ways(false);
    let (cold, cold_ways) = us.ways(true);
    assert!(knows(&warm, "digging_stick") && knows(&warm, "sharp_flake"));
    assert!(!knows(&warm, "fire_keeping"), "no fire where it is warm");
    assert!(knows(&cold, "fire_keeping") && knows(&cold, "hide_wrap_clothing"));
    assert!(
        cold_ways.len() > warm_ways.len(),
        "the cold opens more to do"
    );
}

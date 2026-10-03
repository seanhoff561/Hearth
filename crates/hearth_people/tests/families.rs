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
        let (cg, mg, fg) = (
            c.genome.as_ref().expect("genome"),
            m.genome.as_ref().expect("genome"),
            f.genome.as_ref().expect("genome"),
        );
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

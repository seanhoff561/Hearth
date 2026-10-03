//! H0 (V2.1 §16): the developer's inspector shows a person's record, a section for each of its
//! components (H1 adds the genome and the phenotype).

mod common;

use common::*;
use hearth_people::inspect::report;

#[test]
fn the_inspector_shows_every_component() {
    let b = base();
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    live(&mut p, &mut world, 120.0, &[]);
    let grown = p
        .full()
        .find(|q| q.age(&world.now()) > 12.0)
        .map(|q| q.id)
        .expect("a grown one");
    let r = report(&p, &b.species, &b.graph, grown, &world.now()).expect("its record");
    println!("{}", r.title);
    for s in &r.sections {
        println!("  {}", s.name);
        for l in &s.lines {
            println!("    {l}");
        }
    }
    assert!(r.title.contains("Australopithecus"));
    let names: Vec<&str> = r.sections.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Life",
            "Body",
            "Genome",
            "Phenotype",
            "Mind",
            "Knowledge",
            "Social",
            "Possessions",
            "Place"
        ]
    );
    let knowledge = &r.sections[5].lines[0];
    assert!(r.sections[3].lines.iter().any(|l| l.starts_with("stature")));
    assert!(knowledge.contains("Cracking nuts"), "{knowledge}");
    assert!(report(&p, &b.species, &b.graph, 999_999, &world.now()).is_none());
}

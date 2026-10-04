//! H3 (V2.1 §7.4): a death in the band — the body lies where it fell until its people lay it to
//! rest, what it carried goes to its heir, and its kin grieve.

mod common;

use common::*;
use hearth_items::Stack;
use hearth_people::person::Event;
use hearth_people::psyche::Feeling;

#[test]
fn the_dead_lie_a_while_their_things_go_to_their_heir_and_kin_grieve() {
    let b = base();
    let mut world = Savanna::new();
    let mut p = band(&mut world);
    // A mother with a hammerstone in her hand; her children by her.
    let mother = p
        .full()
        .find(|q| p.full().any(|c| c.life.mother == Some(q.id)))
        .map(|q| q.id)
        .expect("a mother");
    let at = p.get(mother).expect("her").place.pos;
    if let Some(q) = p.persons.iter_mut().find(|q| q.id == mother) {
        q.possessions.carry.right = Some(Stack::of(&item("cobble/basalt"), 1));
        q.body.dead = Some(hearth_body::Death::Injury("a fall".into()));
    }
    step(&mut p, &mut world, &[]);
    let dead = p.get(mother).expect("her");
    assert!(dead.life.died.is_some(), "her death recorded");
    let heir = p
        .persons
        .iter()
        .find(|q| {
            q.life
                .events
                .iter()
                .any(|e| e.event == Event::Inherited { from: mother })
        })
        .expect("an heir");
    assert!(
        heir.possessions.carry.right.is_some() || heir.possessions.carry.left.is_some(),
        "the heir holds what she carried"
    );
    let grieving = p
        .full()
        .filter(|q| q.life.mother == Some(mother))
        .all(|q| q.psyche.feeling(Feeling::Grief) > 0.3);
    assert!(grieving, "her children grieve");
    // She lies where she fell until the day is out.
    let now = world.now();
    let seen = p.views(&b.species, &now, at, 50.0);
    assert!(
        seen.iter().any(|v| v.id == mother && v.dead),
        "her body lies there"
    );
    let later = hearth_people::Now {
        day: now.day + 1.5,
        ..now
    };
    let seen = p.views(&b.species, &later, at, 50.0);
    assert!(!seen.iter().any(|v| v.id == mother), "laid to rest");
}

//! A region set aside far from the player (E4.1 §4.7) keeps only what is saved of it, and
//! brought back is as it was: its land's part made again the same, its animals as they were,
//! living on as they would have.

use std::sync::Arc;

use hearth_fauna::ecology::Ecology;
use hearth_fauna::habitat::{Uniform, temperate_wood};
use hearth_fauna::species::Catalog;

#[test]
fn a_region_set_aside_comes_back_as_it_was() {
    let cat = Arc::new(Catalog::new(&hearth_content::Content::load_base()));
    let land = Uniform {
        habitat: temperate_wood(&cat),
        cells_around: 4096,
    };
    let mut eco = Ecology::new(cat.clone(), 7, 0.0, &land);
    let key = (3, 0);
    eco.ensure_region(&land, key, 0.0);
    eco.advance(0.25, 1.0 / 32.0);
    let r = eco.regions.remove(&key).expect("the region");
    // Set aside: a fraction of the memory.
    let mut aside = r.clone();
    aside.strip();
    println!(
        "{:.1} MB about the player, {:.1} MB set aside",
        r.bytes() as f64 / 1e6,
        aside.bytes() as f64 / 1e6
    );
    assert!(
        aside.bytes() * 3 < r.bytes(),
        "set aside, it keeps too much"
    );
    // Brought back: the same in every part.
    let mut back = eco.maker(eco.next_id);
    back.restore(&land, aside);
    let b = back.regions.get(&key).expect("brought back");
    assert_eq!(b.time, r.time);
    assert_eq!(b.habitat, r.habitat);
    assert_eq!(b.avail_mean, r.avail_mean);
    assert_eq!(b.quality, r.quality);
    assert_eq!(b.prey, r.prey);
    assert_eq!(b.capacity, r.capacity);
    assert_eq!(b.block_capacity, r.block_capacity);
    assert_eq!(b.young, r.young);
    assert_eq!(b.adults, r.adults);
    assert_eq!(b.cond, r.cond);
    assert_eq!(b.groups, r.groups);
    // And it lives on as it would have: a season more of each, the same numbers.
    eco.regions.insert(key, r);
    eco.advance(0.5, 1.0 / 32.0);
    back.advance(0.5, 1.0 / 32.0);
    let (a, b) = (&eco.regions[&key], &back.regions[&key]);
    assert_eq!(a.adults, b.adults);
    assert_eq!(a.groups, b.groups);
}

//! Construction pieces as members of a structure (V2-8 (b)): what the generated pieces weigh
//! and bear, from their materials and sizes.

use hearth_content::Content;
use hearth_content::building::{Member, piece_blocks};

fn member(c: &Content, id: &str) -> Member {
    piece_blocks(&c.construction, &c.materials, &c.forms)
        .into_iter()
        .find(|b| b.id == id)
        .unwrap_or_else(|| panic!("no {id}"))
        .member
}

#[test]
fn pieces_weigh_and_bear_what_their_materials_do() {
    let c = Content::load_base();
    for id in [
        "hearth:post/hazel_wood",
        "hearth:beam/hazel_wood",
        "hearth:bark_roof/birch_bark",
        "hearth:dry_stone/granite",
        "hearth:stone_lintel/granite",
        "hearth:brush_wall/hazel_wood",
    ] {
        println!("{id}: {:?}", member(&c, id));
    }
    // A hazel pole post: under a kilo; it buckles at about a tonne over a metre.
    let post = member(&c, "hearth:post/hazel_wood");
    assert!((5.0..12.0).contains(&post.weight), "{post:?}");
    let euler = std::f32::consts::PI.powi(2) * post.stiffness;
    assert!((3_000.0..15_000.0).contains(&euler), "{euler} N");
    // A course of dry stone weighs a tonne and bends not at all; brush likewise.
    let wall = member(&c, "hearth:dry_stone/granite");
    assert!((8_000.0..12_000.0).contains(&wall.weight), "{wall:?}");
    assert_eq!(wall.moment, 0.0);
    assert_eq!(member(&c, "hearth:brush_wall/hazel_wood").moment, 0.0);
    // A granite lintel bears its span; laid end to end, slabs carry nothing across the joint.
    let lintel = member(&c, "hearth:stone_lintel/granite");
    assert!(lintel.moment > 5_000.0, "{lintel:?}");
    assert_eq!(lintel.continuity, 0.0);
    // A bark roof bends as its two rafters do.
    let roof = member(&c, "hearth:bark_roof/birch_bark");
    assert!((100.0..1_000.0).contains(&roof.moment), "{roof:?}");
}

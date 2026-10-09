//! The seam (E4.1 §4.5): the player is wrapped around the planet, and the animals about them
//! are kept in the player's frame, so an animal a few metres across the seam is a few metres
//! off, not the planet's breadth.

use glam::DVec3;
use hearth_fauna::live::{Live, Stage};

#[test]
fn animals_across_the_seam_are_kept_in_the_players_frame() {
    let c = 65_536.0;
    let mut live = Live::new(3);
    let id = live.place(0, Stage::Adult, true, DVec3::new(c - 10.0, 60.0, 5.0), 0.0);
    let at = |live: &Live| {
        live.animals
            .iter()
            .find(|a| a.id == id)
            .expect("the animal")
            .pos
    };
    // The player just east of the seam: the animal ten metres west of it, fifteen metres off.
    live.reframe(DVec3::new(5.0, 60.0, 5.0), c);
    assert_eq!(at(&live).x, -10.0);
    // The player back west of the seam: the animal where it was.
    live.reframe(DVec3::new(c - 3.0, 60.0, 5.0), c);
    assert_eq!(at(&live).x, c - 10.0);
    // Near the player already: left alone.
    live.reframe(DVec3::new(c - 30.0, 60.0, 5.0), c);
    assert_eq!(at(&live).x, c - 10.0);
}

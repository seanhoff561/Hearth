//! Finding the way (V2-7 (e), docs/design/fauna.md "Finding the way"): a deer swims a river to
//! get away from a person, a squirrel runs up a tree, a crow flies to a tree's crown away from
//! the person, a trout keeps to its stream.

use std::sync::Arc;

use glam::DVec3;
use hearth_fauna::ecology::Ecology;
use hearth_fauna::habitat::{Uniform, temperate_wood};
use hearth_fauna::live::{Act, Cell, Footing, Ground, Live, Medium, Now, Stage};
use hearth_fauna::mind::Presence;
use hearth_fauna::species::Catalog;

/// Level ground at 64, a river across x 20..26 (its bed at 62.5, its surface at 63.8), and a
/// tree whose trunk stands at (`tree`, 0) up to 72 under a crown from 70 to 75.
struct Glade {
    tree: i32,
}

const RIVER: std::ops::Range<i32> = 20..26;

impl Glade {
    fn trunk(&self, x: i32, z: i32) -> bool {
        x == self.tree && z == 0
    }

    fn crown(&self, x: i32, z: i32) -> bool {
        (x - self.tree).abs() <= 2 && z.abs() <= 2
    }
}

impl Ground for Glade {
    fn footing(&self, x: f64, z: f64, _y: f64) -> Option<Footing> {
        let (ix, iz) = (x.floor() as i32, z.floor() as i32);
        if self.trunk(ix, iz) {
            return None;
        }
        if RIVER.contains(&ix) {
            return Some(Footing {
                y: 62.5,
                water: true,
                depth: 1.3,
            });
        }
        Some(Footing::dry(64.0))
    }

    fn top(&self, x: f64, z: f64) -> Option<Footing> {
        let (ix, iz) = (x.floor() as i32, z.floor() as i32);
        if self.trunk(ix, iz) {
            return Some(Footing::dry(64.0));
        }
        self.footing(x, z, 64.0)
    }

    fn cell(&self, x: i32, y: i32, z: i32) -> Option<Cell> {
        if self.trunk(x, z) && (64..72).contains(&y) {
            return Some(Cell::Trunk);
        }
        if self.crown(x, z) && (70..75).contains(&y) {
            return Some(Cell::Leaves);
        }
        if RIVER.contains(&x) {
            return Some(if y < 62 {
                Cell::Solid
            } else if y < 64 {
                Cell::Water
            } else {
                Cell::Open
            });
        }
        Some(if y < 64 { Cell::Solid } else { Cell::Open })
    }
}

fn eco() -> Ecology {
    let cat = Arc::new(Catalog::new(&hearth_content::Content::load_base()));
    let land = Uniform {
        habitat: temperate_wood(&cat),
        cells_around: 4096,
    };
    Ecology::new(cat, 7, 0.0, &land)
}

#[test]
fn a_deer_swims_a_river_to_get_away() {
    let eco = eco();
    let deer = eco.catalog.index("red_deer").expect("red deer") as u16;
    let ground = Glade { tree: -40 };
    let mut live = Live::new(3);
    let id = live.place(deer, Stage::Adult, true, DVec3::new(17.5, 64.0, 0.5), 1.0);
    // A person to the west of it.
    let player = DVec3::new(10.0, 64.0, 0.5);
    let mut swam = false;
    for _ in 0..600 {
        live.step(
            &eco,
            &ground,
            Some(&Presence::walking(player)),
            &Now::day(0.4),
            0.05,
        );
        let a = live.animals.iter().find(|a| a.id == id).expect("the deer");
        if a.medium == Medium::Water {
            swam = true;
            // Swimming: its back at the water's surface, under it its legs.
            assert!(a.pos.y < 63.8 && a.pos.y > 62.0, "at {}", a.pos.y);
        }
    }
    let a = live.animals.iter().find(|a| a.id == id).expect("the deer");
    assert!(swam, "it never swam");
    assert!(a.pos.x > 26.0, "across the river: {}", a.pos);
    assert_eq!(a.medium, Medium::Ground);
}

#[test]
fn a_squirrel_runs_up_a_tree() {
    let eco = eco();
    let squirrel = eco.catalog.index("red_squirrel").expect("red squirrel") as u16;
    let ground = Glade { tree: 0 };
    let mut live = Live::new(4);
    let id = live.place(
        squirrel,
        Stage::Adult,
        true,
        DVec3::new(-4.5, 64.0, 0.5),
        0.0,
    );
    let player = DVec3::new(-4.5, 64.0, 3.5);
    let mut up = 0.0f64;
    for _ in 0..400 {
        live.step(
            &eco,
            &ground,
            Some(&Presence::walking(player)),
            &Now::day(0.4),
            0.05,
        );
        let a = live
            .animals
            .iter()
            .find(|a| a.id == id)
            .expect("the squirrel");
        if a.medium == Medium::Tree {
            up = up.max(a.pos.y - 64.0);
            // On the trunk's side.
            let d = ((a.pos.x - 0.5).powi(2) + (a.pos.z - 0.5).powi(2)).sqrt();
            assert!(d < 1.0, "{d:.2} m from the trunk");
        }
    }
    assert!(up > 1.5, "up the tree {up:.1} m");
}

#[test]
fn a_crow_flies_to_a_tree_away_from_a_person() {
    let eco = eco();
    let crow = eco.catalog.index("carrion_crow").expect("carrion crow");
    let flight = eco.catalog.species[crow].flight_m() as f64;
    // A tree where the crow makes for, away from the person.
    let tree = (5.0 - (flight * 2.0 + 15.0)).round() as i32;
    let ground = Glade { tree };
    let mut live = Live::new(5);
    let id = live.place(
        crow as u16,
        Stage::Adult,
        true,
        DVec3::new(5.5, 64.0, 0.5),
        0.0,
    );
    let player = DVec3::new(5.5 + flight * 0.5, 64.0, 0.5);
    let mut flew = false;
    let mut highest = 0.0f64;
    let mut perched = None;
    for _ in 0..1200 {
        live.step(
            &eco,
            &ground,
            Some(&Presence::walking(player)),
            &Now::day(0.4),
            0.05,
        );
        let a = live.animals.iter().find(|a| a.id == id).expect("the crow");
        if a.medium == Medium::Air {
            flew = true;
            assert_eq!(a.act, Act::Fly);
            highest = highest.max(a.pos.y);
        }
        if a.medium == Medium::Tree && perched.is_none() {
            perched = Some(a.pos);
        }
    }
    assert!(flew, "it never flew");
    assert!(highest > 70.0, "over the trees: {highest:.1}");
    let at = perched.expect("it perched");
    assert!((at.y - 75.0).abs() < 0.01, "on the crown: {at}");
    assert!((at.x.floor() as i32 - tree).abs() <= 2, "{at}");
}

#[test]
fn a_trout_keeps_to_its_stream() {
    let eco = eco();
    let trout = eco.catalog.index("brown_trout").expect("brown trout") as u16;
    let ground = Glade { tree: -40 };
    let mut live = Live::new(6);
    let id = live.place(trout, Stage::Adult, true, DVec3::new(22.5, 63.0, 0.5), 0.0);
    if let Some(a) = live.animals.iter_mut().find(|a| a.id == id) {
        a.medium = Medium::Water;
    }
    // Someone at the water's edge, within its flight distance.
    let player = DVec3::new(20.5, 64.0, 0.5);
    let mut moved = 0.0f64;
    let mut last = DVec3::new(22.5, 63.0, 0.5);
    for _ in 0..600 {
        live.step(
            &eco,
            &ground,
            Some(&Presence::walking(player)),
            &Now::day(0.4),
            0.05,
        );
        let a = live.animals.iter().find(|a| a.id == id).expect("the trout");
        assert!(
            RIVER.contains(&(a.pos.x.floor() as i32)),
            "out of the water at {}",
            a.pos
        );
        assert!(a.pos.y > 62.5 && a.pos.y < 63.8, "at {}", a.pos.y);
        moved += (a.pos - last).length();
        last = a.pos;
    }
    assert!(moved > 2.0, "it darted off: {moved:.1} m");
}

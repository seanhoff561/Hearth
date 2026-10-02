//! Remains (V2-7 (h)): where the populations' large animals die — taken by hunters, of winter,
//! of age — something lies for a while, less of it each day as the hunters come back and the
//! scavengers find it; a person coming near finds it, and ravens over the fresh ones tell of
//! them from afar. A kill in the world lies as a body with what its hunter ate of it gone.

use std::sync::Arc;

use glam::DVec3;
use hearth_fauna::ecology::{Cause, Ecology};
use hearth_fauna::habitat::{Uniform, temperate_wood};
use hearth_fauna::live::{Footing, Ground, Live, Now, Stage};
use hearth_fauna::mind::Air;
use hearth_fauna::species::Catalog;

fn wood() -> (Ecology, Uniform) {
    let cat = Arc::new(Catalog::new(&hearth_content::Content::load_base()));
    let land = Uniform {
        habitat: temperate_wood(&cat),
        cells_around: 4096,
    };
    let mut eco = Ecology::new(cat, 11, 0.0, &land);
    eco.ensure_region(&land, (3, 0), 0.0);
    (eco, land)
}

#[test]
fn the_dead_lie_a_while_and_are_found() {
    let (mut eco, _) = wood();
    // A year, a few days at a time.
    let mut t = 0.0;
    let mut causes = std::collections::BTreeMap::new();
    let (mut seen, mut lying, mut checks) = (0usize, 0usize, 0usize);
    while t < 1.0 {
        t += 1.0 / 128.0;
        eco.advance(t, 1.0 / 32.0);
        for r in eco.regions.values() {
            for m in &r.remains {
                assert!(
                    t - m.time < 31.0 / 365.0 && m.time <= t + 1e-9,
                    "{m:?} at {t}"
                );
                assert!((0.0..=1.0).contains(&m.left));
            }
            seen = seen.max(r.remains.len());
            lying += r.remains.len();
            checks += 1;
        }
        for m in eco.regions.values().flat_map(|r| r.remains.iter()) {
            *causes.entry(format!("{:?}", m.cause)).or_insert(0usize) += 1;
        }
    }
    println!(
        "{:.0} remains lying in a region of 256 km² on average, at most {seen}; by cause \
         (summed over the year's checks): {causes:?}",
        lying as f64 / checks as f64
    );
    assert!(seen > 10, "{seen}");
    assert!(causes.contains_key("Predation") && causes.contains_key("Natural"));
    // A hunter's kill is eaten from; a death of age lies whole.
    let all: Vec<_> = eco
        .regions
        .values()
        .flat_map(|r| r.remains.iter().copied())
        .collect();
    assert!(
        all.iter()
            .filter(|m| m.cause == Cause::Predation)
            .all(|m| m.left < 1.0)
    );
    // Ravens over the fresh ones tell of them, not again within the hour.
    let m = *all
        .iter()
        .filter(|m| t - m.time < 3.0 / 365.0 && m.left_at(t) > 0.3)
        .max_by(|a, b| a.time.total_cmp(&b.time))
        .expect("a fresh one");
    let from = [m.at[0] + 900.0, m.at[1]];
    let told = eco.ravens(from, 2000.0, t, 4.0 / 365.0, 1.0 / 8760.0);
    assert!(told.iter().any(|x| x.at == m.at));
    assert!(
        eco.ravens(from, 2000.0, t, 4.0 / 365.0, 1.0 / 8760.0)
            .iter()
            .all(|x| x.at != m.at)
    );
    // Come near, it is found (and lies in the world from then on, not in the populations).
    let found = eco.take_remains(m.at, 50.0, t);
    let f = found.iter().find(|x| x.at == m.at).expect("found");
    assert!(f.left <= m.left && f.left > 0.0);
    assert!(
        eco.regions
            .values()
            .flat_map(|r| r.remains.iter())
            .all(|x| x.at != m.at)
    );
}

/// Level ground grown over with plants up to two metres (cover for a stalking cat).
struct Thicket;

impl Ground for Thicket {
    fn footing(&self, _x: f64, _z: f64, _y: f64) -> Option<Footing> {
        Some(Footing::dry(64.0))
    }

    fn top(&self, x: f64, z: f64) -> Option<Footing> {
        self.footing(x, z, 64.0)
    }

    fn cell(&self, _x: i32, y: i32, _z: i32) -> Option<hearth_fauna::live::Cell> {
        use hearth_fauna::live::Cell;
        Some(match y {
            ..64 => Cell::Solid,
            64..66 => Cell::Plant,
            _ => Cell::Open,
        })
    }
}

#[test]
fn a_kill_lies_with_what_the_hunter_ate_gone() {
    let (eco, _) = wood();
    let lynx = eco.catalog.index("eurasian_lynx").expect("lynx") as u16;
    let roe = eco.catalog.index("roe_deer").expect("roe") as u16;
    for seed in 0..10 {
        let mut live = Live::new(700 + seed);
        let cat = live.place(lynx, Stage::Adult, true, DVec3::new(0.5, 64.0, -30.5), 0.0);
        live.place(roe, Stage::Adult, true, DVec3::new(0.5, 64.0, 0.5), 0.0);
        for a in live.animals.iter_mut() {
            if a.id == cat {
                a.timer = 0.0;
            }
        }
        let now = Now {
            air: Air {
                wind: glam::DVec2::new(0.0, -1.0),
                wind_speed: 3.0,
                light: 1.0,
            },
            ..Now::day(0.3)
        };
        for _ in 0..2400 {
            live.step(&eco, &Thicket, None, &now, 0.05);
            let bodies = live.take_bodies(&eco.catalog);
            if let Some(b) = bodies.first() {
                assert_eq!(b.species, roe);
                assert_eq!(b.killed_by, Some(lynx));
                assert!(b.left > 0.3 && b.left < 0.95, "{} left", b.left);
                assert!(live.animals.iter().all(|a| !a.dead), "taken from the world");
                return;
            }
        }
    }
    panic!("no kill in ten ambushes");
}

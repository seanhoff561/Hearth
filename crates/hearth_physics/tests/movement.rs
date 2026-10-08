//! Movement in small block worlds: speeds, jumps, walls, steps and ledges, falls, water, low
//! tunnels, edges, ice, ladders and foliage.

use glam::{DVec2, DVec3};
use hearth_physics::testing::{Cell, Grid};
use hearth_physics::{Ability, Gait, Intent, Motion, Mover, Stance, step};

const DT: f64 = 1.0 / 60.0;

fn go(dir: (f64, f64), gait: Gait) -> Intent {
    Intent {
        wish: DVec2::new(dir.0, dir.1),
        gait,
        ..Intent::default()
    }
}

/// Runs `secs` of play with one intent; returns the reports.
fn run(g: &Grid, m: &mut Mover, i: &Intent, a: &Ability, secs: f64) -> Vec<hearth_physics::Report> {
    (0..(secs / DT).round() as usize)
        .map(|_| step(g, m, i, a, DT))
        .collect()
}

fn settled(g: &Grid, x: f64, z: f64) -> Mover {
    let mut m = Mover::new(DVec3::new(x, 0.5, z));
    run(g, &mut m, &Intent::default(), &Ability::human(), 1.0);
    assert!(m.on_ground);
    m
}

#[test]
fn gaits_reach_human_speeds() {
    let g = Grid::floor(80);
    for (gait, speed) in [(Gait::Walk, 1.4), (Gait::Jog, 3.0), (Gait::Sprint, 6.5)] {
        let mut m = settled(&g, 0.5, 0.5);
        let r = run(&g, &mut m, &go((1.0, 0.0), gait), &Ability::human(), 4.0);
        let last = r.last().expect("steps");
        assert!(
            (last.speed - speed).abs() < 0.05,
            "{gait:?}: {}",
            last.speed
        );
        assert!(m.on_ground && (m.pos.z - 0.5).abs() < 1e-9);
    }
    // Sprinting not allowed (exhausted): a jog.
    let mut m = settled(&g, 0.5, 0.5);
    let tired = Ability {
        sprint: false,
        ..Ability::human()
    };
    let r = run(&g, &mut m, &go((1.0, 0.0), Gait::Sprint), &tired, 3.0);
    assert!((r.last().expect("steps").speed - 3.0).abs() < 0.05);
}

#[test]
fn a_standing_jump_rises_about_half_a_metre() {
    let g = Grid::floor(8);
    let mut m = settled(&g, 0.5, 0.5);
    let jump = Intent {
        jump: true,
        ..Intent::default()
    };
    let mut peak: f64 = 0.0;
    step(&g, &mut m, &jump, &Ability::human(), DT);
    for _ in 0..90 {
        step(&g, &mut m, &Intent::default(), &Ability::human(), DT);
        peak = peak.max(m.pos.y);
    }
    assert!((peak - 0.45).abs() < 0.03, "{peak}");
    assert!(m.on_ground && m.pos.y.abs() < 1e-6);
}

#[test]
fn walls_stop_and_slide() {
    let mut g = Grid::floor(20);
    g.fill((3, 0, -20), (3, 1, 20), Cell::Solid);
    let mut m = settled(&g, 0.5, 0.5);
    run(
        &g,
        &mut m,
        &go((1.0, 0.0), Gait::Walk),
        &Ability::human(),
        4.0,
    );
    assert!(
        (m.pos.x - (3.0 - 0.25)).abs() < 1e-6,
        "stopped at the wall: {}",
        m.pos.x
    );
    // At an angle it slides along.
    let z0 = m.pos.z;
    run(
        &g,
        &mut m,
        &go((0.7, 0.7), Gait::Walk),
        &Ability::human(),
        2.0,
    );
    assert!(m.pos.z - z0 > 1.0 && m.pos.x < 2.76);
}

#[test]
fn steps_are_taken_scrambled_or_climbed() {
    let human = Ability::human();
    // Half a metre: a step in stride.
    let mut g = Grid::floor(20);
    g.fill((3, 0, -20), (20, 0, 20), Cell::Slab(0.5));
    let mut m = settled(&g, 0.5, 0.5);
    run(&g, &mut m, &go((1.0, 0.0), Gait::Walk), &human, 4.0);
    assert!(m.pos.x > 5.0 && (m.pos.y - 0.5).abs() < 1e-6, "{:?}", m.pos);

    // A full block: scrambled up while walking, slower than walking on.
    let mut g = Grid::floor(20);
    g.fill((3, 0, -20), (20, 0, 20), Cell::Solid);
    let mut m = settled(&g, 0.5, 0.5);
    let r = run(&g, &mut m, &go((1.0, 0.0), Gait::Walk), &human, 5.0);
    assert!((m.pos.y - 1.0).abs() < 1e-6, "up the block: {:?}", m.pos);
    assert!(r.iter().any(|r| r.straining));
    let flat_distance = 1.4 * 5.0;
    assert!(
        m.pos.x - 0.5 < flat_distance - 0.2,
        "slowed by the scramble"
    );

    // A ledge at head height (1.8 m): no walking up; climbed with a jump and both hands, in
    // about two seconds; not with a broken arm.
    let mut g = Grid::floor(20);
    g.fill((3, 0, -20), (20, 0, 20), Cell::Solid);
    g.fill((3, 1, -20), (20, 1, 20), Cell::Slab(0.8));
    let mut m = settled(&g, 0.5, 0.5);
    run(&g, &mut m, &go((1.0, 0.0), Gait::Walk), &human, 4.0);
    assert!(
        m.pos.y.abs() < 1e-6 && m.pos.x < 2.76,
        "the wall stops a walker"
    );
    let climb = Intent {
        jump: true,
        ..go((1.0, 0.0), Gait::Walk)
    };
    let one_arm = Ability {
        climb: false,
        ..human
    };
    let mut hurt = m;
    run(&g, &mut hurt, &climb, &one_arm, 3.0);
    assert!(
        hurt.pos.y < 0.6,
        "no pulling up with one arm: {:?}",
        hurt.pos
    );
    let r = run(&g, &mut m, &climb, &human, 2.2);
    assert!(r.iter().any(|r| r.motion == Motion::Climbing));
    run(&g, &mut m, &go((1.0, 0.0), Gait::Walk), &human, 1.0);
    assert!(
        (m.pos.y - 1.8).abs() < 1e-6 && m.pos.x > 3.0,
        "on top: {:?}",
        m.pos
    );

    // Two metres is above head height: out of reach.
    let mut g = Grid::floor(20);
    g.fill((3, 0, -20), (20, 1, 20), Cell::Solid);
    let mut m = settled(&g, 0.5, 0.5);
    run(&g, &mut m, &climb, &human, 4.0);
    assert!(m.pos.y < 0.6, "{:?}", m.pos);
}

/// The landing speed of a fall from `height` onto `cell` (or into water `depth` deep).
fn fall(height: f64, cell: Option<Cell>, water_depth: i32) -> f64 {
    let mut g = Grid::floor(10);
    if let Some(c) = cell {
        g.fill((-10, -1, -10), (10, -1, 10), c);
    }
    if water_depth > 0 {
        g.fill(
            (-10, -1 - water_depth, -10),
            (10, -2 - water_depth, 10),
            Cell::Solid,
        );
        g.fill((-10, -water_depth, -10), (10, -1, 10), Cell::Water);
    }
    let mut m = Mover::new(DVec3::new(0.5, height, 0.5));
    let mut landed: f64 = 0.0;
    for _ in 0..600 {
        let r = step(&g, &mut m, &Intent::default(), &Ability::human(), DT);
        if let Some(v) = r.landed {
            landed = landed.max(v);
        }
    }
    landed
}

#[test]
fn falls_land_at_their_speed_softened_by_snow_and_water() {
    let free = |h: f64| (2.0 * hearth_physics::GRAVITY * h).sqrt();
    // Air drag takes a few percent off over ten metres.
    let stone = fall(10.0, None, 0);
    assert!(
        stone < free(10.0) && stone > 0.95 * free(10.0),
        "{stone} vs {}",
        free(10.0)
    );
    let snow = fall(10.0, Some(Cell::Snow), 0);
    assert!((snow - 0.75 * stone).abs() < 0.3, "{snow}");
    // Into four metres of water: the entry passes on about a third.
    let water = fall(10.0, None, 4);
    assert!(water < 0.4 * stone && water > 0.25 * stone, "{water}");
    // A step down is no fall.
    assert!(fall(0.5, None, 0) < 3.5);
}

#[test]
fn swimmers_float_and_divers_run_out_of_breath() {
    let mut g = Grid::floor(20);
    g.fill((-20, -6, -20), (20, -6, 20), Cell::Solid);
    g.fill((-20, -5, -20), (20, -1, 20), Cell::Water);
    let human = Ability::human();
    let mut m = Mover::new(DVec3::new(0.5, -1.0, 0.5));
    let r = run(&g, &mut m, &Intent::default(), &human, 60.0);
    let last = r.last().expect("steps");
    assert_eq!(m.stance, Stance::Swimming);
    assert!(
        !last.eyes_under && last.airless_s == 0.0,
        "floating, face out"
    );
    // Diving down: the breath lasts 45 s, then the body is without air.
    let dive = Intent {
        descend: true,
        ..Intent::default()
    };
    let r = run(&g, &mut m, &dive, &human, 60.0);
    let last = r.last().expect("steps");
    assert!(last.eyes_under && last.airless_s > 10.0, "{last:?}");
    // An exhausted swimmer slips under.
    let spent = Ability {
        stamina: 0.0,
        ..human
    };
    let mut tired = Mover::new(DVec3::new(0.5, -1.0, 0.5));
    let r = run(&g, &mut tired, &Intent::default(), &spent, 30.0);
    assert!(r.last().expect("steps").eyes_under);
}

#[test]
fn a_boat_floats_its_paddler_dry_and_grounds_in_the_shallows() {
    // A lake shelving from dry ground (x < 0) to five metres deep (x ≥ 4).
    let mut g = Grid::floor(30);
    g.fill((-30, -6, -30), (30, -6, 30), Cell::Solid);
    g.fill((0, -5, -30), (30, -1, 30), Cell::Water);
    g.fill((0, -5, -30), (1, -2, 30), Cell::Solid);
    g.fill((-30, -5, -30), (-1, -1, 30), Cell::Solid);
    let boat = Ability {
        boat_m_s: 1.8,
        ..Ability::human()
    };
    // Out from the shore (wading the shallows, then afloat).
    let mut m = Mover::new(DVec3::new(-2.0, 0.5, 0.5));
    run(&g, &mut m, &Intent::default(), &boat, 1.0);
    let r = run(&g, &mut m, &go((1.0, 0.0), Gait::Walk), &boat, 12.0);
    let last = r.last().expect("steps");
    assert_eq!(last.motion, Motion::Paddling, "{m:?}");
    assert!(!m.wet && last.immersion == 0.0, "dry in the hull");
    assert!((last.speed - 1.8).abs() < 0.1, "paddled: {}", last.speed);
    assert!(m.pos.y > -0.3, "afloat at the surface: {}", m.pos.y);
    // Without a boat, the same water is swum.
    let mut swimmer = Mover::new(DVec3::new(10.5, -1.0, 0.5));
    run(&g, &mut swimmer, &Intent::default(), &Ability::human(), 5.0);
    assert_eq!(swimmer.stance, Stance::Swimming);
    // Back to the shore: it grounds in the shallows and is stepped out of.
    let r = run(&g, &mut m, &go((-1.0, 0.0), Gait::Walk), &boat, 20.0);
    let last = r.last().expect("steps");
    assert!(m.pos.x < 0.0 && m.on_ground, "ashore: {m:?}");
    assert_ne!(last.motion, Motion::Paddling);
}

#[test]
fn crawling_goes_where_standing_cannot() {
    let mut g = Grid::floor(20);
    // A tunnel one block high from x = 3 to x = 8.
    g.fill((3, 1, -20), (8, 1, 20), Cell::Solid);
    let mut m = settled(&g, 0.5, 0.5);
    run(
        &g,
        &mut m,
        &go((1.0, 0.0), Gait::Walk),
        &Ability::human(),
        3.0,
    );
    assert!(m.pos.x < 2.76, "standing does not fit: {}", m.pos.x);
    let crawl = Intent {
        crawl: true,
        ..go((1.0, 0.0), Gait::Walk)
    };
    run(&g, &mut m, &crawl, &Ability::human(), 10.0);
    assert!(m.pos.x > 5.0 && m.stance == Stance::Crawling);
    // No standing up under the ceiling.
    run(&g, &mut m, &Intent::default(), &Ability::human(), 0.5);
    assert_eq!(m.stance, Stance::Crawling);
}

#[test]
fn crouching_stops_at_the_edge() {
    let mut g = Grid::default();
    g.fill((-10, -1, -10), (3, -1, 10), Cell::Solid);
    let mut m = settled(&g, 0.5, 0.5);
    let crouch = Intent {
        crouch: true,
        ..go((1.0, 0.0), Gait::Walk)
    };
    run(&g, &mut m, &crouch, &Ability::human(), 8.0);
    assert!(
        m.on_ground && m.pos.y.abs() < 1e-6,
        "still on the ledge: {:?}",
        m.pos
    );
    // Walking off the same edge falls.
    run(
        &g,
        &mut m,
        &go((1.0, 0.0), Gait::Walk),
        &Ability::human(),
        3.0,
    );
    assert!(m.pos.y < -1.0);
}

#[test]
fn ice_slides() {
    let stop_distance = |cell: Cell| {
        let mut g = Grid::floor(60);
        g.fill((-60, -1, -60), (60, -1, 60), cell);
        let mut m = settled(&g, 0.5, 0.5);
        run(
            &g,
            &mut m,
            &go((1.0, 0.0), Gait::Jog),
            &Ability::human(),
            3.0,
        );
        let x0 = m.pos.x;
        run(&g, &mut m, &Intent::default(), &Ability::human(), 6.0);
        m.pos.x - x0
    };
    let stone = stop_distance(Cell::Solid);
    let ice = stop_distance(Cell::Ice);
    assert!(stone < 0.4 && ice > 3.0, "stone {stone}, ice {ice}");
}

#[test]
fn ladders_are_climbed() {
    let mut g = Grid::floor(10);
    g.fill((1, 0, 0), (1, 5, 0), Cell::Ladder);
    g.fill((2, 0, 0), (2, 5, 0), Cell::Solid);
    let mut m = settled(&g, 0.5, 0.5);
    // Into the ladder, holding up.
    let up = Intent {
        jump: true,
        ..go((1.0, 0.0), Gait::Walk)
    };
    let r = run(&g, &mut m, &up, &Ability::human(), 5.0);
    assert!(m.pos.y > 2.5, "{:?}", m.pos);
    assert!(r.iter().any(|r| r.motion == Motion::Climbing));
}

#[test]
fn nothing_falls_through_a_floor() {
    let g = Grid::floor(4);
    let mut m = Mover::new(DVec3::new(0.5, 3.0, 0.5));
    m.vel.y = -60.0;
    step(&g, &mut m, &Intent::default(), &Ability::human(), 0.5);
    assert!(m.pos.y.abs() < 1e-6 && m.on_ground, "{:?}", m.pos);
}

#[test]
fn foliage_is_passed_through_slowly() {
    // A wall of dense foliage two blocks high across the way: walked through, at half pace.
    let mut g = Grid::floor(40);
    g.fill((4, 0, -3), (12, 1, 3), Cell::Foliage(0.5));
    let mut m = settled(&g, 0.5, 0.5);
    let r = run(
        &g,
        &mut m,
        &go((1.0, 0.0), Gait::Walk),
        &Ability::human(),
        22.0,
    );
    assert!(m.pos.x > 14.0, "through the foliage: {}", m.pos.x);
    let slowest = r.iter().map(|x| x.speed).skip(60).fold(f64::MAX, f64::min);
    assert!((slowest - 0.7).abs() < 0.08, "half pace inside: {slowest}");
    assert!(
        (r.last().expect("steps").speed - 1.4).abs() < 0.05,
        "full pace beyond"
    );
}

/// Steady sprinting speed through a field of plants, the cells at the feet and above them.
fn sprint_speed(low: Option<Cell>, high: Option<Cell>) -> f64 {
    let mut g = Grid::floor(80);
    if let Some(c) = low {
        g.fill((2, 0, -3), (70, 0, 3), c);
    }
    if let Some(c) = high {
        g.fill((2, 1, -3), (70, 1, 3), c);
    }
    let mut m = settled(&g, 0.5, 0.5);
    let r = run(
        &g,
        &mut m,
        &go((1.0, 0.0), Gait::Sprint),
        &Ability::human(),
        8.0,
    );
    let tail: Vec<f64> = r.iter().rev().take(r.len() / 4).map(|x| x.speed).collect();
    tail.iter().sum::<f64>() / tail.len() as f64
}

/// Plants slow a body by how much of it they reach (Amendment P §10.1): grass at the ankles
/// costs a sprint almost nothing, waist-high grass a few percent, a shrub belt a quarter.
#[test]
fn plants_slow_a_sprint_by_how_much_of_the_body_they_reach() {
    let bare = sprint_speed(None, None);
    // Ankle-high grass as dense as steppe fescue (0.4 m, 0.1), and heather (0.5 m, 0.15).
    let fescue = sprint_speed(Some(Cell::Plant(0.1, 0.4)), None);
    let heather = sprint_speed(Some(Cell::Plant(0.15, 0.5)), None);
    assert!(
        fescue > bare * 0.98 && heather > bare * 0.98,
        "short grass: {bare:.2} → {fescue:.2}, {heather:.2} m/s"
    );
    // Waist-high grass (1 m, 0.2).
    let tall = sprint_speed(Some(Cell::Plant(0.2, 1.0)), None);
    assert!(
        tall < bare * 0.98 && tall > bare * 0.9,
        "waist-high grass: {bare:.2} → {tall:.2} m/s"
    );
    // A shrub belt 1.4 m tall (0.4).
    let shrubs = sprint_speed(Some(Cell::Plant(0.4, 1.0)), Some(Cell::Plant(0.4, 0.4)));
    assert!(
        shrubs < bare * 0.8 && shrubs > bare * 0.5,
        "shrubs: {bare:.2} → {shrubs:.2} m/s"
    );
}

/// A body brushing the edge of a thicket is slowed by the share of it in the leaves, not as
/// though it were inside.
#[test]
fn brushing_a_thicket_slows_less_than_pushing_through_it() {
    let edge = |z0: i32, z1: i32| {
        let mut g = Grid::floor(60);
        g.fill((2, 0, z0), (50, 1, z1), Cell::Foliage(0.5));
        let mut m = settled(&g, 0.5, 0.5);
        let r = run(
            &g,
            &mut m,
            &go((1.0, 0.0), Gait::Walk),
            &Ability::human(),
            8.0,
        );
        r.iter().rev().take(60).map(|x| x.speed).sum::<f64>() / 60.0
    };
    // The body's box spans z 0.25..0.75: foliage over z 0..1 covers all of it; over z 0.6..
    // (from the next block up, the box's edge only) next to none of it, as its own column is
    // clear.
    let inside = edge(-3, 3);
    let beside = edge(1, 3);
    assert!((inside - 0.7).abs() < 0.08, "inside: {inside}");
    assert!(beside > 1.35, "beside it: {beside}");
}

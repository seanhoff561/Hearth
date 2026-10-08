//! Movement on smooth natural ground (Amendment S §8.1–8.2, §13): walked up and down without
//! steps, slower up steep ground, the steepest walkable slope by footing, sliding beyond it,
//! cliffs as walls and ledges, no tunnelling at sprint and fall speeds, and the same result
//! every time.

use glam::{DVec2, DVec3};
use hearth_physics::testing::{Grid, Surface};
use hearth_physics::{Ability, Gait, Intent, Mover, Report, step};

const DT: f64 = 1.0 / 60.0;

fn go(dir: (f64, f64), gait: Gait) -> Intent {
    Intent {
        wish: DVec2::new(dir.0, dir.1),
        gait,
        ..Intent::default()
    }
}

fn run(g: &Grid, m: &mut Mover, i: &Intent, secs: f64) -> Vec<Report> {
    (0..(secs / DT).round() as usize)
        .map(|_| step(g, m, i, &Ability::human(), DT))
        .collect()
}

/// A body set down on the ground at (x, z) and left to settle.
fn standing(g: &Grid, x: f64, z: f64) -> Mover {
    let s = g.surface.expect("smooth ground");
    let mut m = Mover::new(DVec3::new(x, s.height(x, z) + 0.3, z));
    run(g, &mut m, &Intent::default(), 0.5);
    m
}

fn slope(grade: f64) -> Grid {
    Grid::smooth(Surface {
        grade: (grade, 0.0),
        ..Surface::default()
    })
}

#[test]
fn gentle_slopes_are_walked_without_steps() {
    // A rolling meadow: bumps of 0.6 m every 8 m on a 10 % rise.
    let g = Grid::smooth(Surface {
        grade: (0.1, 0.0),
        bump: 0.6,
        ..Surface::default()
    });
    let s = g.surface.expect("surface");
    let mut m = standing(&g, 0.3, 0.7);
    assert!(m.on_ground);
    let mut last = m.pos.y;
    for _ in 0..(8.0 / DT) as usize {
        step(
            &g,
            &mut m,
            &go((1.0, 0.0), Gait::Jog),
            &Ability::human(),
            DT,
        );
        assert!(m.on_ground, "left the ground at {}", m.pos);
        // On the surface to a centimetre or two (the feet's middle can stand a little above it,
        // on the higher side of a hump).
        let gap = m.pos.y - s.height(m.pos.x, m.pos.z);
        assert!(
            (-0.01..0.08).contains(&gap),
            "{gap} off the ground at {}",
            m.pos
        );
        // No step-hopping: the height changes smoothly frame to frame.
        assert!(
            (m.pos.y - last).abs() < 0.08,
            "a jump of {}",
            m.pos.y - last
        );
        last = m.pos.y;
    }
    // Jogging (3 m/s on the flat), slowed up the humps.
    assert!(m.pos.x > 12.0, "went {}", m.pos.x);
}

#[test]
fn uphill_is_slower_and_steep_downhill_too() {
    let speed_on = |grade: f64, dir: f64| {
        let g = slope(grade);
        let mut m = standing(&g, 0.5, 0.5);
        let r = run(&g, &mut m, &go((dir, 0.0), Gait::Walk), 4.0);
        let x0 = m.pos.x;
        let r2 = run(&g, &mut m, &go((dir, 0.0), Gait::Walk), 2.0);
        let _ = (r, r2);
        ((m.pos.x - x0) / 2.0).abs()
    };
    let flat = speed_on(0.0, 1.0);
    let up = speed_on(0.3, 1.0);
    let gentle_down = speed_on(0.05, -1.0);
    let steep_down = speed_on(0.5, -1.0);
    eprintln!("flat {flat:.2} up {up:.2} gentle down {gentle_down:.2} steep down {steep_down:.2}");
    assert!((flat - 1.4).abs() < 0.1);
    // Tobler: some 0.35 of the flat pace up a 30 % grade, fastest a little downhill.
    assert!(up < 0.45 * flat && up > 0.25 * flat, "{up}");
    assert!(gentle_down >= flat * 0.99);
    assert!(steep_down < 0.8 * flat);
}

#[test]
fn the_steepest_walkable_slope_depends_on_the_footing() {
    // Walking uphill for 5 s: how high does the body get?
    let climbs = |grade: f64, friction: f64| {
        let g = Grid::smooth(Surface {
            grade: (grade, 0.0),
            friction,
            ..Surface::default()
        });
        let mut m = standing(&g, 0.5, 0.5);
        let y0 = m.pos.y;
        run(&g, &mut m, &go((1.0, 0.0), Gait::Walk), 5.0);
        m.pos.y - y0
    };
    // Dry ground: a 35° slope (0.7) is walked, a 55° one (1.4) is not.
    assert!(climbs(0.7, 0.6) > 0.8, "{}", climbs(0.7, 0.6));
    assert!(climbs(1.4, 0.6) < 0.2, "{}", climbs(1.4, 0.6));
    // Wet clay (slipperier): the 35° slope is not.
    assert!(climbs(0.7, 0.75) < 0.2, "{}", climbs(0.7, 0.75));
    // Ice: not even a gentle one of 10°.
    assert!(climbs(0.18, 0.98) < 0.1, "{}", climbs(0.18, 0.98));
}

#[test]
fn ground_too_steep_to_stand_on_is_slid_down() {
    let g = Grid::smooth(Surface {
        grade: (1.2, 0.0),
        ..Surface::default()
    });
    let mut m = standing(&g, 5.0, 0.5);
    let x0 = m.pos.x;
    let r = run(&g, &mut m, &Intent::default(), 1.0);
    assert!(r.iter().any(|r| r.sliding));
    assert!(m.pos.x < x0 - 0.5, "slid {}", x0 - m.pos.x);
    // A slope that can be stood on holds still.
    let g = slope(0.4);
    let mut m = standing(&g, 5.0, 0.5);
    let x0 = m.pos.x;
    let r = run(&g, &mut m, &Intent::default(), 1.0);
    assert!(!r.iter().any(|r| r.sliding));
    assert!((m.pos.x - x0).abs() < 0.01);
    // On ice, a 10° slope is slid down.
    let g = Grid::smooth(Surface {
        grade: (0.18, 0.0),
        friction: 0.98,
        ..Surface::default()
    });
    let mut m = standing(&g, 5.0, 0.5);
    let x0 = m.pos.x;
    run(&g, &mut m, &Intent::default(), 2.0);
    assert!(m.pos.x < x0 - 0.3, "slid {}", x0 - m.pos.x);
}

#[test]
fn a_cliff_is_a_wall_and_its_top_a_ledge() {
    let g = Grid::smooth(Surface {
        cliff: Some((3.0, 1.6)),
        ..Surface::default()
    });
    let mut m = standing(&g, 0.5, 0.5);
    run(&g, &mut m, &go((1.0, 0.0), Gait::Jog), 3.0);
    assert!(m.pos.x < 3.0 && m.pos.x > 2.4, "at {}", m.pos.x);
    assert!(m.pos.y.abs() < 0.05);
    // Jumping at it pulls up onto the top.
    let climb = Intent {
        jump: true,
        ..go((1.0, 0.0), Gait::Walk)
    };
    run(&g, &mut m, &climb, 0.1);
    run(&g, &mut m, &go((1.0, 0.0), Gait::Walk), 3.0);
    assert!(
        m.pos.x > 3.2 && (m.pos.y - 1.6).abs() < 0.05,
        "at {}",
        m.pos
    );
    // A bank of half a metre is stepped up onto without stopping.
    let g = Grid::smooth(Surface {
        cliff: Some((3.0, 0.5)),
        ..Surface::default()
    });
    let mut m = standing(&g, 0.5, 0.5);
    run(&g, &mut m, &go((1.0, 0.0), Gait::Walk), 4.0);
    assert!(
        m.pos.x > 5.0 && (m.pos.y - 0.5).abs() < 0.05,
        "at {}",
        m.pos
    );
}

#[test]
fn nothing_tunnels_at_sprint_or_fall_speeds() {
    let g = Grid::smooth(Surface {
        grade: (0.2, 0.1),
        bump: 1.0,
        wavelength: 6.0,
        ..Surface::default()
    });
    let s = g.surface.expect("surface");
    // Dropped from 60 m (some 33 m/s at the ground).
    let mut m = Mover::new(DVec3::new(2.0, s.height(2.0, 3.0) + 60.0, 3.0));
    let r = run(&g, &mut m, &Intent::default(), 5.0);
    assert!(m.on_ground);
    // On the ground (the feet's middle may stand a little above it on a hump's higher side).
    let gap = m.pos.y - s.height(m.pos.x, m.pos.z);
    assert!((-0.01..0.15).contains(&gap), "{gap}");
    let landed = r.iter().filter_map(|r| r.landed).fold(0.0, f64::max);
    assert!(landed > 25.0, "landed at {landed}");
    // Sprinting over the bumps and down into the hollows for a while.
    let mut m = standing(&g, 0.5, 0.5);
    for k in 0..(10.0 / DT) as usize {
        let dir = if (k / 120) % 2 == 0 {
            (1.0, 0.3)
        } else {
            (-0.4, 1.0)
        };
        step(&g, &mut m, &go(dir, Gait::Sprint), &Ability::human(), DT);
        let under = s.height(m.pos.x, m.pos.z) - m.pos.y;
        assert!(under < 0.03, "{under} under the ground at {}", m.pos);
    }
}

#[test]
fn the_same_walk_comes_out_the_same() {
    let g = Grid::smooth(Surface {
        grade: (0.15, -0.05),
        bump: 0.8,
        ..Surface::default()
    });
    let walk = |seed: f64| {
        let mut m = standing(&g, 0.5, 0.5);
        let mut path = Vec::new();
        for k in 0..600 {
            let a = (k as f64 * 0.01 + seed).sin();
            step(&g, &mut m, &go((1.0, a), Gait::Jog), &Ability::human(), DT);
            path.push(m.pos);
        }
        path
    };
    assert_eq!(walk(0.0), walk(0.0));
}

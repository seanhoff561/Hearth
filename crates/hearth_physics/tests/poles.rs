//! The poles (E4.1 §4.5): a body that goes past a pole edge comes down the far side of the pole,
//! half the planet on, going the other way north–south, as it would over a real pole.

use glam::DVec3;
use hearth_math::{Aabb, BlockPos, Planet, PlanetSize};
use hearth_physics::{Ground, Mover, Terrain, fly};

/// Open air over a planet: nothing to collide with, its x wrapping and its poles crossed.
struct Air(Planet);

impl Terrain for Air {
    fn boxes(&self, _area: &Aabb, _out: &mut Vec<Aabb>) {}
    fn water(&self, _pos: BlockPos) -> f64 {
        0.0
    }
    fn ground(&self, _pos: BlockPos) -> Ground {
        Ground::default()
    }
    fn climbable(&self, _pos: BlockPos) -> bool {
        false
    }
    fn wrap_x(&self, x: f64) -> f64 {
        self.0.wrap_xf(x)
    }
    fn cross_pole(&self, p: DVec3) -> Option<DVec3> {
        self.0.cross_pole(p).map(|c| c.position)
    }
}

#[test]
fn a_body_past_a_pole_comes_down_the_far_side() {
    let planet = Planet::from_size(PlanetSize::Small).expect("planet");
    let (c, edge) = (planet.circumference_f64(), planet.pole_edge_z());
    let t = Air(planet);
    // North (−z), a metre short of the north edge at two metres a second, for a second.
    let mut m = Mover::new(DVec3::new(100.0, 50.0, -edge + 1.0));
    fly(&t, &mut m, DVec3::new(0.5, 0.0, -2.0), 1.0, true);
    // A metre past the edge: a metre inside it on the far side, half the planet on, going
    // south now and still east.
    assert!((m.pos.z - (-edge + 1.0)).abs() < 1e-9, "z {}", m.pos.z);
    assert!((m.pos.x - (100.5 + c / 2.0)).abs() < 1e-9, "x {}", m.pos.x);
    assert!(m.vel.z > 1.99 && m.vel.x > 0.49, "velocity {:?}", m.vel);
    // Over the south pole the same way, near the seam: x wraps around the planet.
    let mut m = Mover::new(DVec3::new(c - 10.0, 50.0, edge - 0.5));
    fly(&t, &mut m, DVec3::new(0.0, 0.0, 1.5), 1.0, true);
    assert!((m.pos.z - (edge - 1.0)).abs() < 1e-9, "z {}", m.pos.z);
    assert!((m.pos.x - (c / 2.0 - 10.0)).abs() < 1e-9, "x {}", m.pos.x);
    assert!(m.vel.z < -1.49, "velocity {:?}", m.vel);
    // Inside the edges nothing changes.
    let mut m = Mover::new(DVec3::new(5.0, 50.0, 0.0));
    fly(&t, &mut m, DVec3::new(0.0, 0.0, -3.0), 1.0, true);
    assert_eq!((m.pos.x, m.pos.z), (5.0, -3.0));
}

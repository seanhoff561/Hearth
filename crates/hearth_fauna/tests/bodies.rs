//! Bodies (V2-7 (d), docs/design/fauna.md "Bodies"): every species stands at its real
//! dimensions, every act poses without coming apart, feet stand on the ground and follow it,
//! the gaits change with speed, and the coats lie in the atlas without overlapping.

use glam::{Affine3A, Vec3};
use hearth_content::Content;
use hearth_fauna::anim::{Drive, Flat, Footing, Motion, pose, rest_pose};
use hearth_fauna::live::{Act, Medium, Stage};
use hearth_fauna::rig::{Frame, Rig, Slot};
use hearth_fauna::skin::Bodies;
use hearth_fauna::species::Catalog;
use hearth_texgen::coats::unwrap_size;

fn catalog() -> Catalog {
    Catalog::new(&Content::load_base())
}

/// The corners of a placed box.
fn corners(m: &Affine3A) -> [Vec3; 8] {
    std::array::from_fn(|k| {
        m.transform_point3(Vec3::new(
            if k & 1 == 0 { -0.5 } else { 0.5 },
            if k & 2 == 0 { -0.5 } else { 0.5 },
            if k & 4 == 0 { -0.5 } else { 0.5 },
        ))
    })
}

fn extent(rig: &Rig, placed: &[(usize, Affine3A)], part: impl Fn(usize) -> bool) -> (Vec3, Vec3) {
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    for (i, m) in placed {
        if !part(*i) {
            continue;
        }
        for c in corners(m) {
            lo = lo.min(c);
            hi = hi.max(c);
        }
    }
    let _ = rig;
    (lo, hi)
}

#[test]
fn every_species_stands_at_its_real_dimensions() {
    let cat = catalog();
    for sp in &cat.species {
        for male in [false, true] {
            let rig = Rig::of(sp, male);
            let rest = rest_pose(&rig);
            let placed = rest.boxes(&rig);
            assert!(!placed.is_empty(), "{}: no boxes", sp.name);
            let (lo, hi) = extent(&rig, &placed, |_| true);
            assert!(lo.is_finite() && hi.is_finite(), "{}", sp.name);
            match rig.frame {
                Frame::Quadruped => {
                    // Standing on the ground, the back at the shoulder height.
                    assert!(
                        lo.y.abs() < sp.shoulder_m * 0.08 + 0.01,
                        "{}: lowest {:.3}",
                        sp.name,
                        lo.y
                    );
                    let (_, torso_hi) = extent(&rig, &placed, |i| {
                        matches!(rig.boxes[i].slot, Slot::Chest | Slot::Hips)
                            && rig.boxes[i].part == hearth_texgen::coats::SkinPart::Body
                    });
                    let err = (torso_hi.y - sp.shoulder_m).abs() / sp.shoulder_m;
                    assert!(
                        err < 0.12,
                        "{}: back at {:.2} m, shoulder {:.2} m",
                        sp.name,
                        torso_hi.y,
                        sp.shoulder_m
                    );
                    // Nose to rump about its length (the tail and antlers aside).
                    let (blo, bhi) = extent(&rig, &placed, |i| {
                        !matches!(rig.boxes[i].slot, Slot::Tail(_))
                            && rig.boxes[i].gear == hearth_fauna::rig::Gear::None
                            && !matches!(rig.boxes[i].slot, Slot::Ear(_))
                    });
                    let len = bhi.z - blo.z;
                    let err = (len - sp.length_m).abs() / sp.length_m;
                    assert!(
                        err < 0.3,
                        "{}: {:.2} m nose to rump, length {:.2} m",
                        sp.name,
                        len,
                        sp.length_m
                    );
                }
                Frame::Bird | Frame::Frog => {
                    assert!(
                        lo.y.abs() < sp.shoulder_m * 0.15 + 0.005,
                        "{}: lowest {:.3}",
                        sp.name,
                        lo.y
                    );
                    let len = hi.z - lo.z;
                    assert!(
                        len > sp.length_m * 0.6 && len < sp.length_m * 1.5,
                        "{}: {len:.3} m long",
                        sp.name
                    );
                }
                _ => {
                    let len = hi.z - lo.z;
                    assert!(
                        len > sp.length_m * 0.7 && len < sp.length_m * 1.4,
                        "{}: {len:.3} m long",
                        sp.name
                    );
                }
            }
        }
    }
}

#[test]
fn every_act_poses_every_species_whole() {
    let cat = catalog();
    let acts = [
        Act::Graze,
        Act::Walk,
        Act::Rest,
        Act::Alert,
        Act::Flee,
        Act::Sleep,
        Act::Groom,
        Act::Drink,
        Act::Rear,
        Act::Attack,
        Act::Fly,
    ];
    for sp in &cat.species {
        let rig = Rig::of(sp, false);
        for act in acts {
            for stage in [Stage::Adult, Stage::Young] {
                let speed = match act {
                    Act::Walk => sp.walk_m_s,
                    Act::Flee => sp.run_m_s,
                    _ => 0.0,
                };
                let d = Drive {
                    act,
                    speed,
                    look: Some(Vec3::new(3.0, 1.0, 10.0)),
                    stage,
                    female: true,
                    year_frac: 0.3,
                    southern: false,
                    scale: if stage == Stage::Young { 0.4 } else { 1.0 },
                    medium: match act {
                        Act::Fly => Medium::Air,
                        _ => Medium::Ground,
                    },
                };
                let mut m = Motion::new(7);
                m.settle(&rig, &d);
                for _ in 0..30 {
                    m.update(&rig, &d, 0.05);
                    let p = pose(&rig, &m, &d, &Flat);
                    for (i, place) in p.boxes(&rig) {
                        for c in corners(&place) {
                            assert!(c.is_finite(), "{} {act:?}: box {i} at {c}", sp.name);
                            // Nothing far from the body (a stretched limb, a lost head).
                            let reach = sp.length_m.max(sp.shoulder_m) * 3.5 + 0.2;
                            assert!(
                                c.length() < reach * if act == Act::Fly { 3.0 } else { 1.0 },
                                "{} {act:?}: box {i} at {c}",
                                sp.name
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Ground that rises toward the front.
struct Slope(f32);

impl Footing for Slope {
    fn ground(&self, at: Vec3) -> Option<f32> {
        Some(at.z * self.0)
    }
}

#[test]
fn feet_stand_on_the_ground_and_the_body_follows_a_slope() {
    let cat = catalog();
    let sp = cat.get("hearth:red_deer").expect("red deer");
    let rig = Rig::of(sp, false);
    let d = Drive::standing();
    let mut m = Motion::new(1);
    m.settle(&rig, &d);
    for slope in [0.0f32, 0.3] {
        let ground = Slope(slope);
        let p = pose(&rig, &m, &d, &ground);
        for l in 0..4u8 {
            let foot = p.slots[Slot::Foot(l).index()].translation;
            let g = foot.z * slope;
            assert!(
                (foot.y - g).abs() < 0.03,
                "foot {l} at {:.3}, ground {g:.3} (slope {slope})",
                foot.y
            );
        }
        // The body pitches nose up on a rising slope.
        let fwd = p.slots[Slot::Chest.index()].matrix3.z_axis;
        if slope > 0.0 {
            assert!(fwd.y > 0.1, "the body follows the slope: {fwd}");
        }
    }
}

#[test]
fn feet_lift_and_fall_through_a_stride_and_the_gait_changes_with_speed() {
    let cat = catalog();
    let sp = cat.get("hearth:red_deer").expect("red deer");
    let rig = Rig::of(sp, false);
    let mut lifted = [false; 4];
    let mut down = [false; 4];
    let mut d = Drive::standing();
    d.speed = sp.walk_m_s;
    let mut m = Motion::new(3);
    m.settle(&rig, &d);
    for _ in 0..80 {
        m.update(&rig, &d, 0.03);
        let p = pose(&rig, &m, &d, &Flat);
        for l in 0..4u8 {
            let y = p.slots[Slot::Foot(l).index()].translation.y;
            if y > 0.02 {
                lifted[l as usize] = true;
            }
            if y.abs() < 0.005 {
                down[l as usize] = true;
            }
        }
    }
    assert!(
        lifted.iter().all(|&b| b) && down.iter().all(|&b| b),
        "lifted {lifted:?}, down {down:?}"
    );
    // Walking, trotting, galloping as it goes faster.
    let mut gaits = Vec::new();
    for speed in [1.0f32, 4.0, 12.0] {
        let mut d = Drive::standing();
        d.speed = speed;
        let mut m = Motion::new(3);
        m.settle(&rig, &d);
        gaits.push(m.gait);
    }
    assert_eq!(gaits, vec![0.0, 1.0, 2.0]);
}

#[test]
fn lying_down_brings_the_body_to_the_ground() {
    let cat = catalog();
    let sp = cat.get("hearth:red_deer").expect("red deer");
    let rig = Rig::of(sp, false);
    let mut d = Drive::standing();
    d.act = Act::Rest;
    let mut m = Motion::new(5);
    m.settle(&rig, &d);
    let p = pose(&rig, &m, &d, &Flat);
    let placed = p.boxes(&rig);
    let (lo, hi) = extent(&rig, &placed, |i| {
        matches!(rig.boxes[i].slot, Slot::Chest | Slot::Hips)
    });
    assert!(lo.y < 0.05, "the belly on the ground: {:.2}", lo.y);
    assert!(
        hi.y < sp.shoulder_m * 0.7,
        "lower than standing: {:.2}",
        hi.y
    );
}

#[test]
fn stags_carry_antlers_in_their_season_and_hinds_none() {
    let cat = catalog();
    let sp = cat.get("hearth:red_deer").expect("red deer");
    let antlers = |female: bool, year_frac: f32| {
        let rig = Rig::of(sp, !female);
        let mut d = Drive::standing();
        d.female = female;
        d.year_frac = year_frac;
        let mut m = Motion::new(9);
        m.settle(&rig, &d);
        let p = pose(&rig, &m, &d, &Flat);
        let n = p
            .boxes(&rig)
            .iter()
            .filter(|(i, _)| rig.boxes[*i].gear == hearth_fauna::rig::Gear::Antler)
            .count();
        (p.antlers, n)
    };
    // Autumn: the stag's antlers hard; early spring, just cast and growing again; never on a
    // hind.
    let (grown, n) = antlers(false, 0.6);
    assert_eq!(grown, 1.0);
    assert!(n > 6, "{n} boxes of antler");
    let (spring, _) = antlers(false, 0.03);
    assert!(spring < 0.2, "grown {spring} in early spring");
    assert_eq!(antlers(true, 0.6).1, 0);
}

#[test]
fn reindeer_cows_carry_antlers_a_season_later_than_the_bulls() {
    let cat = catalog();
    let sp = cat.get("hearth:reindeer").expect("reindeer");
    let antlers = |female: bool, year_frac: f32| {
        let rig = Rig::of(sp, !female);
        let mut d = Drive::standing();
        d.female = female;
        d.year_frac = year_frac;
        let mut m = Motion::new(4);
        m.settle(&rig, &d);
        pose(&rig, &m, &d, &Flat).antlers
    };
    // Late winter: the bulls have cast theirs, the cows keep them (to hold their digs to the
    // lichen); late summer, both carry them, the cows' smaller.
    assert_eq!(antlers(false, 0.95), 0.0, "a bull in late winter");
    assert!(antlers(true, 0.95) > 0.5, "a cow in late winter");
    let (bull, cow) = (antlers(false, 0.45), antlers(true, 0.45));
    assert_eq!(bull, 1.0);
    assert!(cow > 0.0 && cow < bull, "cow {cow}, bull {bull}");
}

#[test]
fn a_moose_carries_broad_palms() {
    let cat = catalog();
    let sp = cat.get("hearth:moose").expect("moose");
    let rig = Rig::of(sp, true);
    let mut d = Drive::standing();
    d.year_frac = 0.6;
    let mut m = Motion::new(2);
    m.settle(&rig, &d);
    let placed = pose(&rig, &m, &d, &Flat).boxes(&rig);
    let antler = |i: usize| rig.boxes[i].gear == hearth_fauna::rig::Gear::Antler;
    // The palms reach out to the sides, wider than the head is long, and are broad plates.
    let (lo, hi) = extent(&rig, &placed, antler);
    let span = (hi.x - lo.x).max(hi.z - lo.z);
    assert!(span > 1.0, "the antlers span {span:.2} m");
    let widest = rig
        .boxes
        .iter()
        .filter(|b| b.gear == hearth_fauna::rig::Gear::Antler)
        .map(|b| b.size.x.max(b.size.y))
        .fold(0.0f32, f32::max);
    assert!(
        widest > 0.3,
        "the broadest antler box is {widest:.2} m across"
    );
}

#[test]
fn the_coats_lie_in_the_atlas_without_overlapping() {
    let cat = catalog();
    let bodies = Bodies::new(&cat);
    let [w, h] = [bodies.atlas.w, bodies.atlas.h];
    assert!(w <= 4096 && h <= 4096, "atlas {w}x{h}");
    let mut owner = vec![u32::MAX; (w * h) as usize];
    for (si, skin) in bodies.skins.iter().enumerate() {
        for (vi, block) in skin.blocks.iter().enumerate() {
            let Some(at) = block else { continue };
            for (bi, (o, d)) in skin.boxes.iter().zip(&skin.dims).enumerate() {
                let [uw, uh] = unwrap_size(*d);
                for y in 0..uh {
                    for x in 0..uw {
                        let (px, py) = (at[0] + o[0] + x, at[1] + o[1] + y);
                        assert!(
                            px < w && py < h,
                            "{}: box {bi} off the atlas",
                            cat.species[si].name
                        );
                        let id = ((si * 4 + vi) * 1000 + bi) as u32;
                        let i = (py * w + px) as usize;
                        assert!(
                            owner[i] == u32::MAX,
                            "{}: box {bi} overlaps",
                            cat.species[si].name
                        );
                        owner[i] = id;
                    }
                }
            }
        }
    }
    // Painted: every face of every box (the unwraps' corners stay empty).
    let opaque = bodies.atlas.px.iter().filter(|p| p[3] == 255).count();
    let faces: u32 = bodies
        .skins
        .iter()
        .map(|s| {
            let per: u32 = s
                .dims
                .iter()
                .map(|[x, y, z]| 2 * (x * y + y * z + x * z))
                .sum();
            per * s.blocks.iter().flatten().count() as u32
        })
        .sum();
    assert_eq!(opaque as u32, faces, "painted pixels");
    println!("atlas {w}x{h}, {faces} pixels of coats");
}

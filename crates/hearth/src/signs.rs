//! The signs animals leave, as drawn (V2-7 (h), docs/design/fauna.md "Tracks and signs"): a
//! print shaped by the foot that made it (a cloven hoof's two halves, a paw's pad and toes, a
//! bear's broad sole, a hare's long hind feet, a bird's three toes), dark in earth and a
//! blue-grey hollow in snow; drops of blood, bright when fresh; droppings (a plant-eater's
//! pellets, a hunter's scat). Boxes on the ground for the figure pass.

use glam::{Affine3A, DVec3, Quat, Vec3};
use hearth_character::FigureInstance;
use hearth_content::schema::fauna::Foot;
use hearth_fauna::live::{Sign, SignKind};
use hearth_fauna::species::Catalog;

/// Signs are drawn within this distance of the eye.
pub const DRAWN_M: f64 = 40.0;

/// The boxes of the signs within sight of `view`, at the world's seconds `now`, lit by `light`
/// at a point.
pub fn instances(
    signs: &[Sign],
    now: f64,
    cat: &Catalog,
    view: DVec3,
    light: &dyn Fn(DVec3) -> (u8, u8),
) -> Vec<FigureInstance> {
    let mut out = Vec::new();
    let mut put = |at: DVec3, yaw: f32, size: Vec3, color: [u8; 3]| {
        let place = Affine3A::from_scale_rotation_translation(
            size,
            Quat::from_rotation_y(yaw),
            (at - view).as_vec3() + Vec3::Y * (size.y * 0.5 + 0.002),
        );
        out.push(hearth_character::solid(place, color, light(at)));
    };
    for s in signs {
        if (s.pos - view).length() > DRAWN_M {
            continue;
        }
        let Some(sp) = cat.species.get(s.species as usize) else {
            continue;
        };
        let q = Quat::from_rotation_y(s.yaw);
        let off = |x: f32, z: f32| s.pos + (q * Vec3::new(x, 0.0, z)).as_dvec3();
        match s.kind {
            SignKind::Print => {
                let Some(track) = sp.track else {
                    continue;
                };
                let l = (track.length_cm / 100.0).max(0.01);
                let color = if s.plain >= 0.95 {
                    [168, 180, 202]
                } else {
                    [66, 50, 36]
                };
                let thin = 0.006;
                match track.foot {
                    Foot::ClovenHoof => {
                        for x in [-0.22, 0.22] {
                            put(off(x * l, 0.0), s.yaw, Vec3::new(0.32 * l, thin, l), color);
                        }
                    }
                    Foot::PawClawed | Foot::PawRetracted => {
                        put(
                            off(0.0, -0.15 * l),
                            s.yaw,
                            Vec3::new(0.5 * l, thin, 0.45 * l),
                            color,
                        );
                        for (x, z) in [(-0.3, 0.2), (-0.1, 0.35), (0.1, 0.35), (0.3, 0.2)] {
                            put(
                                off(x * l, z * l),
                                s.yaw,
                                Vec3::new(0.16 * l, thin, 0.2 * l),
                                color,
                            );
                        }
                    }
                    Foot::Plantigrade => {
                        put(
                            off(0.0, -0.1 * l),
                            s.yaw,
                            Vec3::new(0.6 * l, thin, 0.7 * l),
                            color,
                        );
                        for k in 0..5 {
                            let x = -0.3 + 0.15 * k as f32;
                            put(
                                off(x * l, 0.38 * l),
                                s.yaw,
                                Vec3::new(0.11 * l, thin, 0.13 * l),
                                color,
                            );
                        }
                    }
                    Foot::Hopping => {
                        for x in [-0.25, 0.25] {
                            put(off(x * l, 0.0), s.yaw, Vec3::new(0.3 * l, thin, l), color);
                        }
                    }
                    Foot::BirdToes => {
                        for a in [-0.5f32, 0.0, 0.5] {
                            put(
                                off(a.sin() * 0.3 * l, a.cos() * 0.3 * l),
                                s.yaw + a,
                                Vec3::new(0.06 * l, thin, 0.6 * l),
                                color,
                            );
                        }
                    }
                    Foot::Hoof => {
                        put(off(0.0, 0.0), s.yaw, Vec3::new(0.85 * l, thin, l), color);
                    }
                    Foot::Pad => {
                        put(off(0.0, 0.0), s.yaw, Vec3::new(0.95 * l, thin, l), color);
                        for x in [-0.3, -0.1, 0.1, 0.3] {
                            put(
                                off(x * l, 0.5 * l),
                                s.yaw,
                                Vec3::new(0.12 * l, thin, 0.08 * l),
                                color,
                            );
                        }
                    }
                    Foot::Slither => {}
                }
            }
            SignKind::Blood => {
                let hours = (now - s.t) / 3600.0;
                let color = if hours < 2.0 {
                    [128, 14, 12]
                } else {
                    [74, 26, 18]
                };
                put(s.pos, s.yaw, Vec3::new(0.035, 0.004, 0.03), color);
            }
            SignKind::Droppings => {
                if sp.hunts() {
                    for k in 0..2 {
                        put(
                            off(0.03 * k as f32, 0.05 * k as f32),
                            s.yaw + k as f32,
                            Vec3::new(0.022, 0.02, 0.06),
                            [70, 56, 40],
                        );
                    }
                } else {
                    for k in 0..6 {
                        let a = k as f32 * 2.4;
                        put(
                            off(0.03 * a.cos(), 0.03 * a.sin()),
                            s.yaw,
                            Vec3::splat(0.013),
                            [44, 34, 24],
                        );
                    }
                }
            }
        }
    }
    out
}

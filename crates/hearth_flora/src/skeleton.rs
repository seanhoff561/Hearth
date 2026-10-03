//! The branching skeleton of one tree (after Weber and Penn's parametric trees): stems that
//! lean and taper, decurrent crowns forking into limbs, branches leaving the stem at the
//! species' angle (in whorls for conifers) as long as the crown's shape allows, twigs forking
//! from them, all bending under their weight as the species droops, and foliage at the ends.
//! Lengths are metres, y up, the foot of the first stem at the origin.

use glam::Vec3;
use hearth_content::schema::flora::{Crown, LeafKind};
use hearth_math::hash::{Rng, derive_seed, mix64};

use crate::growth::{Species, Stage};

/// A piece of wood: a tapered cylinder from `a` to `b`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Seg {
    pub a: Vec3,
    pub b: Vec3,
    /// Radius at `a` and at `b` (m).
    pub ra: f32,
    pub rb: f32,
    /// 0 a stem or limb, 1 a branch, 2 a twig, 3 a root.
    pub order: u8,
}

/// Foliage: a capsule from `a` to `b` of radius `r` (a sphere when `a == b`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blob {
    pub a: Vec3,
    pub b: Vec3,
    pub r: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Skeleton {
    pub wood: Vec<Seg>,
    pub foliage: Vec<Blob>,
    /// Height (m) and trunk diameter at breast height (m) it was grown at.
    pub height: f32,
    pub diameter: f32,
}

/// How wide the crown is at a relative height in it (0 its base, 1 its top), as a share of
/// its widest.
pub fn envelope(crown: Crown, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let dome = |centre: f32, half: f32| (1.0 - ((t - centre) / half).powi(2)).max(0.0).sqrt();
    match crown {
        Crown::Conical => (1.0 - t).powf(0.85).max(0.05),
        Crown::Ovoid | Crown::Weeping => dome(0.4, 0.62).max(0.25),
        Crown::Rounded => dome(0.5, 0.55).max(0.3),
        Crown::Spreading => dome(0.35, 0.75).max(0.3),
        Crown::Columnar => 0.55 + 0.45 * (std::f32::consts::PI * t).sin().max(0.0).sqrt(),
        Crown::Umbrella => {
            if t < 0.8 {
                0.25 + 0.75 * (t / 0.8).powf(1.5)
            } else {
                (1.0 - (t - 0.8) / 0.2).max(0.0).sqrt().max(0.2)
            }
        }
        Crown::MultiStemmed => 0.45 + 0.55 * t.sqrt() * (1.0 - 0.3 * t),
        // All its leaves are in the rosette at the top.
        Crown::Palm => {
            if t > 0.9 {
                1.0
            } else {
                0.05
            }
        }
    }
}

/// The parameters of one growth, adjusted for the tree's stage.
struct Plan {
    crown: Crown,
    height: f32,
    diameter: f32,
    crown_ratio: f32,
    /// Widest crown radius (m).
    radius: f32,
    apical: f32,
    angle: f32,
    droop: f32,
    whorled: bool,
    needles: bool,
    density: f32,
    flare: f32,
    /// The height (m) up the stem its stilt roots arch from.
    prop: f32,
    snag: bool,
}

impl Plan {
    fn new(sp: &Species, stage: Stage) -> Self {
        let f = &sp.form;
        let age = stage.age(sp);
        let height = sp.height_at(age).max(0.25);
        let diameter = sp.diameter_at(age).max(0.02);
        // Young trees carry their crown low and are narrow for their height; old trees are
        // broad and flat-topped, their leader lost among the limbs.
        let maturity = (height / sp.max_height_m).clamp(0.0, 1.0);
        let old = match stage {
            Stage::Old => 1.12,
            Stage::Ancient => 1.25,
            Stage::Snag => 1.1,
            _ => 1.0,
        };
        let crown_ratio = (0.95 + (f.crown_ratio - 0.95) * maturity).clamp(0.15, 0.97);
        let width = f.crown_width * (0.55 + 0.45 * maturity) * old;
        let apical = match stage {
            Stage::Old => f.apical_dominance * 0.85,
            Stage::Ancient => f.apical_dominance * 0.65,
            Stage::Seedling | Stage::Sapling => f.apical_dominance.max(0.7),
            _ => f.apical_dominance,
        };
        Self {
            crown: f.crown,
            height,
            diameter,
            crown_ratio,
            radius: (width * height / 2.0).max(0.35),
            apical,
            angle: f.branch_angle_deg.to_radians(),
            droop: f.droop,
            whorled: f.whorled,
            needles: f.leaf != LeafKind::Broad,
            density: f.foliage_density,
            flare: if stage >= Stage::Young {
                f.root_flare
            } else {
                0.0
            },
            prop: if stage >= Stage::Sapling {
                f.prop_roots * height
            } else {
                0.0
            },
            snag: stage == Stage::Snag,
        }
    }
}

/// A direction from azimuth and angle from the vertical.
fn dir(azimuth: f32, from_vertical: f32) -> Vec3 {
    let (s, c) = from_vertical.sin_cos();
    Vec3::new(s * azimuth.cos(), c, s * azimuth.sin())
}

/// Grows one tree of a species at a stage; `variant` picks one of its forms.
pub fn grow(sp: &Species, stage: Stage, variant: u32) -> Skeleton {
    let seed = mix64(derive_seed(variant as u64, &sp.id) ^ (stage.index() as u64 * 0x9e37_79b9));
    let mut rng = Rng::new(seed);
    let plan = Plan::new(sp, stage);
    let mut out = Skeleton {
        height: plan.height,
        diameter: plan.diameter,
        ..Skeleton::default()
    };
    let stems = if stage <= Stage::Sapling {
        1
    } else {
        sp.form.stems.max(1) as usize
    };
    for k in 0..stems {
        let (base, lean, height, diameter) = if stems == 1 {
            let lean = dir(
                rng.range_f32(0.0, std::f32::consts::TAU),
                rng.range_f32(0.0, 0.07),
            );
            (Vec3::ZERO, lean, plan.height, plan.diameter)
        } else {
            // Stems of a stool spread outward from the middle.
            let az = (k as f32 + rng.range_f32(-0.3, 0.3)) * std::f32::consts::TAU / stems as f32;
            let off = Vec3::new(az.cos(), 0.0, az.sin()) * rng.range_f32(0.2, 0.6);
            let lean = dir(az, rng.range_f32(0.15, 0.4));
            (
                off,
                lean,
                plan.height * rng.range_f32(0.75, 1.0),
                plan.diameter / (stems as f32).sqrt(),
            )
        };
        if plan.crown == Crown::Palm {
            palm(&plan, &mut rng, &mut out, base, lean, height, diameter);
        } else {
            stem(&plan, &mut rng, &mut out, base, lean, height, diameter);
        }
    }
    out
}

/// A palm: one stem, unbranched and hardly tapering, leaning as it rises and curving back
/// toward the light (a coconut's, which droops, far), crowned with fronds that arch out and
/// down from its top, the old ones hanging, the young ones standing up.
fn palm(
    plan: &Plan,
    rng: &mut Rng,
    out: &mut Skeleton,
    base: Vec3,
    lean: Vec3,
    height: f32,
    diameter: f32,
) {
    let r0 = diameter / 2.0;
    // The fronds, as long as the crown is broad (a young one's in proportion to it), rise about
    // half their length above the stem's top: the stem ends where the crown begins.
    let length = plan.radius.max(1.2).min(height * 0.55);
    let height = (height - length * 0.55 - 0.3).max(height * 0.3);
    let steps = (height / 0.9).ceil().max(1.0) as usize;
    let tilt = Vec3::new(lean.x, 0.0, lean.z).normalize_or_zero() * (0.15 + plan.droop * 0.6);
    let mut d = (Vec3::Y + tilt).normalize();
    let mut p = base;
    for i in 1..=steps {
        let t0 = (i - 1) as f32 / steps as f32;
        let t1 = i as f32 / steps as f32;
        // Curving back up toward the light.
        d = (d + Vec3::Y * 0.05).normalize();
        let q = p + d * (height / steps as f32);
        let ra = r0 * (1.0 - 0.25 * t0) * if i == 1 { 1.0 + plan.flare * 0.4 } else { 1.0 };
        out.wood.push(Seg {
            a: p,
            b: q,
            ra,
            rb: r0 * (1.0 - 0.25 * t1),
            order: 0,
        });
        p = q;
    }
    if plan.snag {
        return;
    }
    // The heart of the crown, and the fronds about it.
    out.foliage.push(Blob {
        a: p,
        b: p + Vec3::Y * 0.6,
        r: (r0 * 1.6).clamp(0.4, 0.9),
    });
    let n = 10 + rng.below(7) as usize;
    let az0 = rng.range_f32(0.0, std::f32::consts::TAU);
    for k in 0..n {
        let az = az0 + k as f32 * std::f32::consts::TAU / n as f32 + rng.range_f32(-0.2, 0.2);
        // Young fronds stand up, old ones lie out.
        let rise = rng.range_f32(0.0, 1.0);
        let mut fd = dir(az, 0.35 + 1.0 * (1.0 - rise));
        let mut q0 = p + Vec3::Y * 0.3;
        let segs = 3;
        for _ in 0..segs {
            let q1 = q0 + fd * (length * rng.range_f32(0.85, 1.1) / segs as f32);
            out.foliage.push(Blob {
                a: q0,
                b: q1,
                r: 0.3 + 0.15 * plan.density,
            });
            // Each frond bends down along its length.
            fd = (fd - Vec3::Y * (0.25 + 0.5 * plan.droop)).normalize();
            q0 = q1;
        }
    }
}

/// Radius (m) of a stem of foot radius `r0` at a share `t` of its height.
fn taper(r0: f32, t: f32) -> f32 {
    (r0 * (1.0 - 0.9 * t.clamp(0.0, 1.0)).powf(1.1)).max(0.008)
}

/// One stem: its leader (or forked limbs), branches and foliage.
fn stem(
    plan: &Plan,
    rng: &mut Rng,
    out: &mut Skeleton,
    base: Vec3,
    lean: Vec3,
    height: f32,
    diameter: f32,
) {
    let r0 = diameter / 2.0;
    let crown_base = height * (1.0 - plan.crown_ratio);
    // A broad crown forks into limbs; a strong leader runs to the top.
    let fork_t = if plan.apical < 0.5 {
        (crown_base / height + (1.0 - crown_base / height) * (0.12 + 0.5 * plan.apical))
            .clamp(0.2, 0.85)
    } else {
        1.0
    };
    let top = if plan.snag {
        rng.range_f32(0.6, 0.85)
    } else {
        1.0
    };
    let leader_end_t = fork_t.min(top);
    // The leader: a gently wandering polyline.
    let steps = ((height * leader_end_t) / 0.9).ceil().max(1.0) as usize;
    let mut p = base;
    let mut d = lean;
    let mut axis: Vec<(Vec3, f32, f32)> = vec![(p, 0.0, taper(r0, 0.0))];
    for i in 1..=steps {
        let t = leader_end_t * i as f32 / steps as f32;
        d = (d + Vec3::new(rng.range_f32(-0.04, 0.04), 0.0, rng.range_f32(-0.04, 0.04)))
            .normalize();
        // The leader straightens toward the light.
        d = (d + Vec3::Y * 0.08).normalize();
        let q = p + d * (height * leader_end_t / steps as f32);
        let (ra, rb) = (taper(r0, axis.last().map_or(0.0, |a| a.1)), taper(r0, t));
        // The foot flares into the ground.
        let ra = if i == 1 {
            ra * (1.0 + plan.flare * 0.6)
        } else {
            ra
        };
        out.wood.push(Seg {
            a: p,
            b: q,
            ra,
            rb,
            order: 0,
        });
        axis.push((q, t, rb));
        p = q;
    }
    if plan.flare > 0.0 {
        roots(plan, rng, out, base, r0);
    }
    if plan.prop > 0.0 {
        stilts(plan, rng, out, base, r0);
    }
    // Branches along the leader in the crown.
    branches_along(plan, rng, out, &axis, height, crown_base, base);
    if fork_t < top {
        // Limbs from the fork, each a leader of its own part of the crown.
        let n = 2 + rng.below(3) as usize;
        let (fork_p, _, fork_r) = *axis.last().expect("leader");
        let az0 = rng.range_f32(0.0, std::f32::consts::TAU);
        for k in 0..n {
            let az = az0 + k as f32 * std::f32::consts::TAU / n as f32 + rng.range_f32(-0.3, 0.3);
            let tilt = rng.range_f32(0.35, 0.7);
            let mut ld = dir(az, tilt);
            let length = (height * (top - fork_t)) / tilt.cos() * rng.range_f32(0.85, 1.05);
            let limb_steps = (length / 0.9).ceil().max(1.0) as usize;
            let r_limb = fork_r * rng.range_f32(0.6, 0.8);
            let mut q0 = fork_p;
            let mut laxis: Vec<(Vec3, f32, f32)> = vec![(q0, fork_t, r_limb)];
            // The limbs end under the crown's top, leaving room for their foliage.
            let ceiling = base.y + height * top - 1.0;
            for i in 1..=limb_steps {
                let s = i as f32 / limb_steps as f32;
                // Limbs bend up toward the light as they climb.
                ld = (ld + Vec3::Y * 0.06).normalize();
                let q1 = q0 + ld * (length / limb_steps as f32);
                if q1.y > ceiling && i > 1 {
                    break;
                }
                let ra = laxis.last().map_or(r_limb, |a| a.2);
                let rb = (r_limb * (1.0 - 0.9 * s)).max(0.008);
                out.wood.push(Seg {
                    a: q0,
                    b: q1,
                    ra,
                    rb,
                    order: 0,
                });
                let t = ((q1.y - base.y) / height).clamp(0.0, 1.0);
                laxis.push((q1, t, rb));
                q0 = q1;
            }
            branches_along(plan, rng, out, &laxis, height, crown_base, base);
            if !plan.snag {
                tuft(plan, rng, out, q0, ld);
            }
        }
    } else if !plan.snag {
        let (tip, _, _) = *axis.last().expect("leader");
        tuft(plan, rng, out, tip, d);
    }
}

/// Foliage at a leader's tip.
fn tuft(plan: &Plan, rng: &mut Rng, out: &mut Skeleton, tip: Vec3, d: Vec3) {
    if plan.needles {
        out.foliage.push(Blob {
            a: tip - d * (plan.height * 0.08).min(1.5),
            b: tip + d * 0.4,
            r: rng.range_f32(0.55, 0.8),
        });
    } else {
        // A crown's top sits over its leader's tip.
        let c = tip + d * 0.5;
        out.foliage.push(Blob {
            a: c,
            b: c,
            r: ((0.8 + 0.08 * plan.height).min(2.2) * rng.range_f32(0.85, 1.1)).max(1.0),
        });
    }
}

/// Stilt roots: arching out and down from the lower stem into the mud about it (a red
/// mangrove's cage), each a curve of segments thinning toward the ground.
fn stilts(plan: &Plan, rng: &mut Rng, out: &mut Skeleton, base: Vec3, r0: f32) {
    let n = 5 + rng.below(4) as usize;
    let az0 = rng.range_f32(0.0, std::f32::consts::TAU);
    for k in 0..n {
        let az = az0 + k as f32 * std::f32::consts::TAU / n as f32 + rng.range_f32(-0.35, 0.35);
        let out_dir = Vec3::new(az.cos(), 0.0, az.sin());
        let from = base + Vec3::Y * (plan.prop * rng.range_f32(0.45, 1.0)) + out_dir * r0;
        let to = base + out_dir * (r0 + plan.prop * rng.range_f32(0.7, 1.3)) - Vec3::Y * 0.3;
        // Out first, then down: a quadratic arch bowed up over its chord.
        let bend = Vec3::new(to.x, from.y + plan.prop * 0.15, to.z).lerp(from, 0.45);
        let steps = 4;
        let ra = (r0 * 0.35).max(0.06);
        let mut p = from;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let q = from * (1.0 - t) * (1.0 - t) + bend * 2.0 * t * (1.0 - t) + to * t * t;
            let t0 = (i - 1) as f32 / steps as f32;
            out.wood.push(Seg {
                a: p,
                b: q,
                ra: ra * (1.0 - 0.4 * t0),
                rb: ra * (1.0 - 0.4 * t),
                order: 3,
            });
            p = q;
        }
    }
}

/// Exposed roots at the foot of a flared trunk.
fn roots(plan: &Plan, rng: &mut Rng, out: &mut Skeleton, base: Vec3, r0: f32) {
    let n = 3 + rng.below(3) as usize;
    let az0 = rng.range_f32(0.0, std::f32::consts::TAU);
    for k in 0..n {
        let az = az0 + k as f32 * std::f32::consts::TAU / n as f32 + rng.range_f32(-0.4, 0.4);
        let out_dir = Vec3::new(az.cos(), -0.15, az.sin()).normalize();
        let len = r0 + plan.flare * rng.range_f32(0.8, 1.8) * (r0 * 2.0).clamp(0.5, 2.0);
        let a = base + Vec3::new(0.0, 0.25, 0.0);
        out.wood.push(Seg {
            a,
            b: a + out_dir * len,
            ra: (r0 * 0.45).max(0.05),
            rb: 0.05,
            order: 3,
        });
    }
}

/// First-order branches along an axis of (point, share of the tree's height, radius), with
/// their twigs and foliage.
fn branches_along(
    plan: &Plan,
    rng: &mut Rng,
    out: &mut Skeleton,
    axis: &[(Vec3, f32, f32)],
    height: f32,
    crown_base: f32,
    foot: Vec3,
) {
    if axis.len() < 2 {
        return;
    }
    let crown_len = (height - crown_base).max(0.3);
    let start_y = axis[0].0.y;
    let end_y = axis[axis.len() - 1].0.y;
    let lo = (foot.y + crown_base).max(start_y);
    if end_y <= lo {
        return;
    }
    // Spacing: whorls for conifers, a spiral otherwise; small trees have few branches.
    let (step, per) = if plan.whorled {
        ((height / 30.0).clamp(0.45, 0.9), 4 + rng.below(3) as usize)
    } else {
        ((0.75 - 0.25 * plan.density).max(0.35), 1)
    };
    let mut y = lo + rng.range_f32(0.0, step);
    let mut az = rng.range_f32(0.0, std::f32::consts::TAU);
    let golden = 2.399_963;
    while y < end_y - 0.2 {
        // The axis point and radius at this height.
        let (p, r_axis) = point_at(axis, y);
        let t = ((y - foot.y - crown_base) / crown_len).clamp(0.0, 1.0);
        let reach = plan.radius * envelope_of(plan, t);
        for k in 0..per {
            let a = if plan.whorled {
                az + k as f32 * std::f32::consts::TAU / per as f32 + rng.range_f32(-0.25, 0.25)
            } else {
                az
            };
            // Upper branches rise more steeply.
            let from_vertical = (plan.angle * (1.0 - 0.35 * t)).clamp(0.2, 1.75);
            let horizontal = from_vertical.sin().max(0.3);
            let mut length = (reach / horizontal * rng.range_f32(0.85, 1.1)).max(0.3);
            // Rising branches end under the crown's top, leaving room for their foliage.
            let rise = from_vertical.cos();
            if rise > 0.05 {
                let room = (foot.y + height - 1.2 - p.y).max(0.3);
                length = length.min(room / rise);
            }
            let r_branch = (r_axis * (0.3 + 0.45 * (length / plan.radius.max(0.5)).min(1.0)))
                .min(r_axis * 0.85)
                .max(0.01);
            let length = if plan.snag {
                length * rng.range_f32(0.15, 0.45)
            } else {
                length
            };
            branch(
                plan,
                rng,
                out,
                p,
                dir(a, from_vertical),
                length,
                r_branch,
                1,
            );
        }
        if !plan.whorled {
            az += golden + rng.range_f32(-0.2, 0.2);
        } else {
            az += rng.range_f32(0.3, 0.9);
        }
        y += step * rng.range_f32(0.8, 1.2);
    }
}

fn envelope_of(plan: &Plan, t: f32) -> f32 {
    envelope(plan.crown, t)
}

/// The point and radius of an axis at a height.
fn point_at(axis: &[(Vec3, f32, f32)], y: f32) -> (Vec3, f32) {
    for w in axis.windows(2) {
        let (a, b) = (w[0], w[1]);
        if y >= a.0.y && y <= b.0.y {
            let s = if (b.0.y - a.0.y).abs() < 1e-4 {
                0.0
            } else {
                (y - a.0.y) / (b.0.y - a.0.y)
            };
            return (a.0.lerp(b.0, s), a.2 + (b.2 - a.2) * s);
        }
    }
    let last = axis[axis.len() - 1];
    (last.0, last.2)
}

/// A branch of `order` (1 from the stem, 2 a twig) from `p` along `d`, bending under its
/// weight, with its own twigs and its foliage.
#[allow(clippy::too_many_arguments)]
fn branch(
    plan: &Plan,
    rng: &mut Rng,
    out: &mut Skeleton,
    p: Vec3,
    d: Vec3,
    length: f32,
    radius: f32,
    order: u8,
) {
    let seg_len = if order == 1 { 0.8 } else { 0.6 };
    let steps = (length / seg_len).ceil().max(1.0) as usize;
    let mut q0 = p;
    let mut dd = d;
    let mut r = radius;
    let mut points = Vec::with_capacity(steps + 1);
    points.push((q0, r));
    for i in 1..=steps {
        let s = i as f32 / steps as f32;
        // Weight bends it down along its length; the tips turn up a little toward the light.
        let sag = plan.droop * 0.35 * s;
        dd = (dd - Vec3::Y * sag
            + Vec3::new(
                rng.range_f32(-0.12, 0.12),
                rng.range_f32(-0.05, 0.08),
                rng.range_f32(-0.12, 0.12),
            ))
        .normalize();
        let q1 = q0 + dd * (length / steps as f32);
        let r1 = (radius * (1.0 - 0.85 * s)).max(0.006);
        out.wood.push(Seg {
            a: q0,
            b: q1,
            ra: r,
            rb: r1,
            order,
        });
        points.push((q1, r1));
        q0 = q1;
        r = r1;
    }
    if plan.snag {
        return;
    }
    if order == 1 && length > 1.2 {
        // Twigs from the outer two thirds, alternating sides.
        let mut side = if rng.below(2) == 0 { 1.0 } else { -1.0 };
        let mut along = length * rng.range_f32(0.3, 0.4);
        while along < length - 0.4 {
            let k = ((along / length) * steps as f32).floor() as usize;
            let (bp, br) = points[k.min(points.len() - 1)];
            let fwd = dd;
            let side_dir = fwd.cross(Vec3::Y).normalize_or(Vec3::X) * side;
            let twig_dir = (fwd * 0.6 + side_dir * 0.75 + Vec3::Y * 0.25).normalize();
            let twig_len = (length - along) * rng.range_f32(0.35, 0.55) + 0.4;
            branch(
                plan,
                rng,
                out,
                bp,
                twig_dir,
                twig_len,
                (br * 0.55).max(0.006),
                2,
            );
            side = -side;
            along += rng.range_f32(0.7, 1.2);
        }
    }
    // Foliage: along the outer part for needles, in clusters at the ends for broad leaves.
    if plan.needles {
        let from = points[(points.len() / 5).min(points.len() - 1)].0;
        out.foliage.push(Blob {
            a: from,
            b: q0,
            r: if order == 1 { 0.75 } else { 0.55 } * (0.8 + 0.3 * plan.density),
        });
    } else {
        let size = if order == 1 { 1.0 } else { 0.85 };
        let r = (size * (0.75 + 0.45 * (length / 3.0).min(1.0)) * (0.8 + 0.35 * plan.density))
            .clamp(0.6, 1.9);
        out.foliage.push(Blob { a: q0, b: q0, r });
        if order == 1 && length > 2.5 {
            // A second cluster midway out.
            let mid = points[points.len() / 2].0;
            out.foliage.push(Blob {
                a: mid,
                b: mid,
                r: r * 0.8,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_palm_is_one_stem_and_a_crown_of_fronds() {
        let mut sp = crate::growth::tests::oak();
        sp.form.crown = Crown::Palm;
        sp.form.crown_width = 0.45;
        sp.form.droop = 0.5;
        sp.max_height_m = 25.0;
        let t = grow(&sp, crate::growth::Stage::Mature, 3);
        // Wood is the stem alone, rising from the foot to the top.
        assert!(
            t.wood.iter().all(|s| s.order == 0),
            "a palm has no branches"
        );
        let top = t.wood.iter().map(|s| s.b.y).fold(0.0f32, f32::max);
        assert!(top > t.height * 0.75, "{top} of {}", t.height);
        // The crown above it, its fronds reaching about the palm's height.
        let crown = t
            .foliage
            .iter()
            .map(|b| b.a.y.max(b.b.y) + b.r)
            .fold(0.0f32, f32::max);
        assert!(
            (t.height * 0.9..t.height * 1.1).contains(&crown),
            "crown to {crown} of {}",
            t.height
        );
        // Its leaves are all up in the crown, spreading wide about the stem's top.
        assert!(t.foliage.len() >= 30, "{} pieces of frond", t.foliage.len());
        assert!(t.foliage.iter().all(|b| b.a.y > top - 4.0));
        let spread = t
            .foliage
            .iter()
            .map(|b| (b.b.x * b.b.x + b.b.z * b.b.z).sqrt())
            .fold(0.0f32, f32::max);
        assert!(spread > 3.0, "fronds reach {spread} m");
    }

    #[test]
    fn crown_envelopes_have_their_shapes() {
        // A spire is widest at its foot, an umbrella near its top, a dome in its lower middle.
        assert!(envelope(Crown::Conical, 0.0) > envelope(Crown::Conical, 0.8));
        assert!(envelope(Crown::Umbrella, 0.8) > envelope(Crown::Umbrella, 0.1));
        let s = |t| envelope(Crown::Spreading, t);
        assert!(s(0.35) >= s(0.0) && s(0.35) > s(1.0));
        for c in [
            Crown::Spreading,
            Crown::Ovoid,
            Crown::Rounded,
            Crown::Conical,
            Crown::Columnar,
            Crown::Weeping,
            Crown::Umbrella,
            Crown::MultiStemmed,
            Crown::Palm,
        ] {
            for k in 0..=10 {
                let e = envelope(c, k as f32 / 10.0);
                assert!((0.0..=1.0).contains(&e), "{c:?} {k}: {e}");
            }
        }
    }
}

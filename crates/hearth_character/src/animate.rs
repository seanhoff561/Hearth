//! Moving the frame: a pose for each thing a body does (standing, walking, jogging,
//! sprinting, wading, crouching, crawling, swimming, treading water, climbing, falling,
//! lying), the gaits driven by distance so the feet keep pace with the ground, cross-faded
//! when the activity changes.
//!
//! Joint angles are in degrees: flexion swings a limb forward (the knee's bends the shank
//! back), abduction takes a limb out to the side, lean tips the trunk or head forward.

use std::f32::consts::{PI, TAU};

use glam::{Affine3A, Quat, Vec3};

use crate::rig::{JOINTS, Joint, Rig};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Activity {
    #[default]
    Stand,
    Walk,
    Jog,
    Sprint,
    Wade,
    Crouch,
    Crawl,
    /// Swimming at the surface.
    Swim,
    /// Swimming under water.
    Dive,
    /// Treading water.
    Tread,
    /// Pulling up onto a ledge.
    Climb,
    Ladder,
    Fall,
    /// Lying on the back: asleep, unconscious or dead.
    Lie,
}

impl Activity {
    /// The length of one cycle of the gait (two steps, or one stroke) in metres; 0 for none.
    pub fn cycle_m(self) -> f32 {
        match self {
            Activity::Walk => 1.5,
            Activity::Jog => 2.3,
            Activity::Sprint => 3.2,
            Activity::Wade => 1.4,
            Activity::Crouch => 1.1,
            Activity::Crawl => 0.9,
            Activity::Swim | Activity::Dive => 1.1,
            Activity::Ladder => 1.1,
            _ => 0.0,
        }
    }

    /// Whether the body stands, kneels or lies on the ground (its lowest point is put on it).
    pub fn grounded(self) -> bool {
        matches!(
            self,
            Activity::Stand
                | Activity::Walk
                | Activity::Jog
                | Activity::Sprint
                | Activity::Wade
                | Activity::Crouch
                | Activity::Crawl
                | Activity::Lie
        )
    }
}

/// What the body is doing this frame.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Drive {
    pub activity: Activity,
    /// Horizontal speed (m/s).
    pub speed: f32,
    /// Vertical speed (m/s).
    pub vertical: f32,
    /// Where the eyes look: pitch (degrees, down positive) and turn from the body (degrees,
    /// left positive).
    pub look_pitch: f32,
    pub look_yaw: f32,
    /// A ledge climb's progress (0–1).
    pub climb: f32,
    /// Shivering (0–1): arms hug the body, a tremor.
    pub shiver: f32,
    pub breaths_per_min: f32,
    /// What the hands are doing with things.
    pub holding: Holding,
}

/// What the hands hold: a thing in either hand, a load in both arms, or a drag behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Holding {
    pub left: bool,
    pub right: bool,
    pub both: bool,
    pub dragging: bool,
}

/// Joint rotations and the root's height above the feet's ground (m).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub rot: [Quat; JOINTS],
    pub root_y: f32,
}

impl Pose {
    pub fn rest(rig: &Rig) -> Self {
        Self {
            rot: [Quat::IDENTITY; JOINTS],
            root_y: rig.rest[Joint::Root.index()].y,
        }
    }

    /// Each joint's placement in the figure's frame (feet's ground at y = 0).
    pub fn joints(&self, rig: &Rig) -> [Affine3A; JOINTS] {
        let mut out = [Affine3A::IDENTITY; JOINTS];
        for j in Joint::ALL {
            let i = j.index();
            out[i] = match j.parent() {
                None => Affine3A::from_rotation_translation(
                    self.rot[i],
                    Vec3::new(rig.rest[i].x, self.root_y, rig.rest[i].z),
                ),
                Some(p) => {
                    out[p.index()] * Affine3A::from_rotation_translation(self.rot[i], rig.rest[i])
                }
            };
        }
        out
    }

    /// The lowest point of the body's flesh, with the root at `root_y`.
    pub fn lowest(&self, rig: &Rig) -> f32 {
        let joints = self.joints(rig);
        let mut low = f32::INFINITY;
        for p in &rig.parts {
            let m = joints[p.joint.index()];
            let h = p.size / 2.0;
            for k in 0..8 {
                let c = Vec3::new(
                    if k & 1 == 0 { -h.x } else { h.x },
                    if k & 2 == 0 { -h.y } else { h.y },
                    if k & 4 == 0 { -h.z } else { h.z },
                );
                low = low.min(m.transform_point3(p.center + c).y);
            }
        }
        low
    }

    /// Between two poses (0 all `a`, 1 all `b`).
    pub fn blend(a: &Pose, b: &Pose, t: f32) -> Pose {
        Pose {
            rot: std::array::from_fn(|i| a.rot[i].slerp(b.rot[i], t)),
            root_y: a.root_y + (b.root_y - a.root_y) * t,
        }
    }
}

fn rx(deg: f32) -> Quat {
    Quat::from_rotation_x(deg.to_radians())
}

/// A limb hanging down swings forward (or an arm's elbow bends).
fn flex(deg: f32) -> Quat {
    rx(-deg)
}

/// The shank bends back.
fn knee(deg: f32) -> Quat {
    rx(deg)
}

/// Toes up.
fn dorsi(deg: f32) -> Quat {
    rx(-deg)
}

/// The trunk or the head tips forward.
fn lean(deg: f32) -> Quat {
    rx(deg)
}

/// A limb out to its side (`side` 1 left, −1 right).
fn abduct(side: f32, deg: f32) -> Quat {
    Quat::from_rotation_z((side * deg).to_radians())
}

/// A turn to the left.
fn twist(deg: f32) -> Quat {
    Quat::from_rotation_y(deg.to_radians())
}

/// An arm: flexion, then abduction, then the elbow.
#[derive(Clone, Copy)]
struct Arm {
    flex: f32,
    abduct: f32,
    elbow: f32,
}

/// A leg.
#[derive(Clone, Copy)]
struct Leg {
    hip: f32,
    abduct: f32,
    knee: f32,
    ankle: f32,
}

/// Seconds to cross-fade between activities.
const FADE_S: f32 = 0.25;

/// Animates one body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Animator {
    /// The gait's cycle (0–1): the left foot comes down at 0, the right at 0.5.
    pub phase: f32,
    t: f32,
    current: Activity,
    previous: Activity,
    fade: f32,
    /// Phase still to make up to meet a footstep heard (cycles).
    correction: f32,
}

impl Default for Animator {
    fn default() -> Self {
        Self {
            phase: 0.0,
            t: 0.0,
            current: Activity::Stand,
            previous: Activity::Stand,
            fade: 1.0,
            correction: 0.0,
        }
    }
}

impl Animator {
    pub fn activity(&self) -> Activity {
        self.current
    }

    /// A foot came down (as the footsteps heard say): the gait's phase eases to meet it.
    pub fn foot_down(&mut self, left: bool) {
        let strike = if left { 0.0 } else { 0.5 };
        let mut d = strike - self.phase.rem_euclid(1.0);
        if d > 0.5 {
            d -= 1.0;
        }
        if d < -0.5 {
            d += 1.0;
        }
        self.correction = d;
    }

    pub fn update(&mut self, rig: &Rig, drive: &Drive, dt: f32) -> Pose {
        let dt = dt.clamp(0.0, 0.25);
        self.t += dt;
        if drive.activity != self.current {
            self.previous = self.current;
            self.current = drive.activity;
            self.fade = 0.0;
        }
        self.fade = (self.fade + dt / FADE_S).min(1.0);
        let cycle = self.current.cycle_m();
        let advance = match self.current {
            Activity::Ladder => drive.vertical.abs() * dt / cycle,
            Activity::Swim | Activity::Dive => drive.speed.max(0.35) * dt / cycle,
            Activity::Tread => 0.8 * dt,
            _ if cycle > 0.0 => drive.speed * dt / cycle,
            _ => 0.0,
        };
        let fix = self.correction * (1.0 - (-dt / 0.15).exp());
        self.correction -= fix;
        self.phase = (self.phase + advance + fix).rem_euclid(1.0);
        let now = self.pose(rig, self.current, drive);
        if self.fade >= 1.0 {
            return now;
        }
        let before = self.pose(rig, self.previous, drive);
        let t = self.fade * self.fade * (3.0 - 2.0 * self.fade);
        Pose::blend(&before, &now, t)
    }

    /// The pose of an activity now.
    pub fn pose(&self, rig: &Rig, activity: Activity, d: &Drive) -> Pose {
        let female = rig.dims.bust_d > 0.0;
        // The arms hang just clear of the hips, the forearms angled out a little (the
        // elbow's carrying angle).
        let carry = if female { 9.0 } else { 6.0 };
        let rest_abduct = if female { 5.0 } else { 4.0 };
        let th = TAU * self.phase;
        let (left, right) = (th, th + PI);
        let mut trunk = 0.0;
        let mut root = Quat::IDENTITY;
        let mut chest = Quat::IDENTITY;
        let mut neck_lean = 0.0;
        let mut root_y = None;
        let rest_arm = Arm {
            flex: 0.0,
            abduct: rest_abduct,
            elbow: 8.0,
        };
        let rest_leg = Leg {
            hip: 0.0,
            abduct: 1.5,
            knee: 3.0,
            ankle: 0.0,
        };
        let (mut arms, mut legs): ([Arm; 2], [Leg; 2]) = match activity {
            Activity::Stand => {
                // Breathing and a slow shift of weight.
                let breath = (TAU * self.t * d.breaths_per_min.max(6.0) / 60.0).sin();
                chest = lean(-0.8 * breath);
                let sway = (TAU * self.t * 0.12).sin();
                root = twist(1.5 * sway);
                (
                    [
                        Arm {
                            flex: 1.0 * breath,
                            ..rest_arm
                        },
                        Arm {
                            flex: 1.0 * breath,
                            ..rest_arm
                        },
                    ],
                    [
                        Leg {
                            abduct: 1.5 + 1.0 * sway,
                            ..rest_leg
                        },
                        Leg {
                            abduct: 1.5 - 1.0 * sway,
                            ..rest_leg
                        },
                    ],
                )
            }
            Activity::Walk | Activity::Jog | Activity::Sprint | Activity::Wade => {
                let g = match activity {
                    Activity::Walk => &WALK,
                    Activity::Jog => &JOG,
                    Activity::Sprint => &SPRINT,
                    _ => &WADE,
                };
                let ql = self.phase;
                let qr = (self.phase + 0.5).rem_euclid(1.0);
                // An arm swings with the other side's leg: forward as that foot comes down.
                let arm = |q: f32| {
                    let fwd = (TAU * (q - 0.5)).cos();
                    Arm {
                        flex: 2.0 + g.arm * fwd,
                        abduct: if activity == Activity::Wade {
                            20.0
                        } else {
                            rest_abduct
                        },
                        elbow: g.elbow + g.elbow_swing * fwd.max(0.0),
                    }
                };
                trunk = g.lean;
                let fwd_l = (TAU * ql).cos();
                root = twist(-g.twist * fwd_l);
                chest = twist(g.twist * 1.8 * fwd_l);
                let (left_leg, right_leg) = (stride(g, ql), stride(g, qr));
                // The hips ride on the feet that are down; between strides of a run they fly
                // in an arc from the push to the landing.
                let support = |l: &Leg| support_y(rig, l);
                root_y = Some(match (ql < g.stance, qr < g.stance) {
                    (true, true) => support(&left_leg).max(support(&right_leg)),
                    (true, false) => support(&left_leg),
                    (false, true) => support(&right_leg),
                    (false, false) => {
                        let flight = (0.5 - g.stance).max(1e-3);
                        let since = ((ql - g.stance).min(qr - g.stance) / flight).clamp(0.0, 1.0);
                        let off = support(&stride(g, g.stance - 1e-4));
                        let on = support(&stride(g, 0.0));
                        off + (on - off) * since + 4.0 * g.flight_m * since * (1.0 - since)
                    }
                });
                ([arm(ql), arm(qr)], [left_leg, right_leg])
            }
            Activity::Crouch => {
                let moving = d.speed.abs() > 0.1;
                let amp = if moving { 1.0 } else { 0.0 };
                trunk = 25.0;
                let leg = |p: f32| {
                    let hip = 75.0 + 15.0 * amp * p.sin();
                    let knee = 112.0 + 20.0 * amp * (p + PI / 6.0).cos().max(0.0);
                    // Feet flat.
                    Leg {
                        hip,
                        abduct: 4.0,
                        knee,
                        ankle: knee - hip,
                    }
                };
                let arm = |p: f32| Arm {
                    flex: 30.0 + 10.0 * amp * p.sin(),
                    abduct: rest_abduct + 4.0,
                    elbow: 40.0,
                };
                ([arm(right), arm(left)], [leg(left), leg(right)])
            }
            Activity::Crawl => {
                // Low, on the belly (the crawling box is 0.6 m): forearms and knees pull and
                // push, a knee drawn up to the side with the other side's arm reaching on.
                root = lean(88.0);
                neck_lean = -50.0;
                let leg = |p: f32| Leg {
                    hip: 8.0 * p.sin().max(0.0),
                    abduct: 12.0 + 14.0 * p.sin().max(0.0),
                    knee: 15.0 + 35.0 * p.sin().max(0.0),
                    ankle: -35.0,
                };
                // Upper arms down to the elbows on the ground, forearms flat ahead.
                let arm = |p: f32| Arm {
                    flex: 145.0 + 12.0 * p.sin(),
                    abduct: 25.0,
                    elbow: 35.0 - 15.0 * p.sin(),
                };
                ([arm(right), arm(left)], [leg(left), leg(right)])
            }
            Activity::Swim | Activity::Dive => {
                // Breaststroke: arms sweep out and back from the glide, the legs kick.
                let under = activity == Activity::Dive;
                root = lean(if under { 86.0 } else { 74.0 });
                neck_lean = if under { -20.0 } else { -50.0 };
                root_y = Some(0.3);
                let arm = Arm {
                    flex: 130.0 + 40.0 * th.cos(),
                    abduct: 35.0 + 25.0 * th.sin(),
                    elbow: 45.0 - 45.0 * th.cos(),
                };
                let leg = Leg {
                    hip: 30.0 + 30.0 * th.sin().max(0.0),
                    abduct: 15.0 + 10.0 * th.sin(),
                    knee: 60.0 + 60.0 * th.sin(),
                    ankle: -30.0,
                };
                ([arm, arm], [leg, leg])
            }
            Activity::Tread => {
                // Upright, sculling with the hands, an eggbeater kick.
                root_y = Some(rig.rest[Joint::Root.index()].y);
                let scull = th.sin();
                let arm = Arm {
                    flex: 30.0,
                    abduct: 45.0 + 15.0 * scull,
                    elbow: 60.0,
                };
                let leg = |p: f32| Leg {
                    hip: 40.0 + 20.0 * p.sin(),
                    abduct: 15.0,
                    knee: 70.0 + 20.0 * p.cos(),
                    ankle: -20.0,
                };
                ([arm, arm], [leg(left), leg(right)])
            }
            Activity::Climb => {
                // Hands on the ledge pull the body up, then press it over; a knee comes up.
                let c = d.climb.clamp(0.0, 1.0);
                root_y = Some(rig.rest[Joint::Root.index()].y);
                trunk = 20.0;
                let (sf, el) = if c < 0.5 {
                    (170.0 - 280.0 * c, 30.0 + 180.0 * c)
                } else {
                    (30.0 - 60.0 * (c - 0.5), 120.0 - 220.0 * (c - 0.5))
                };
                let arm = Arm {
                    flex: sf,
                    abduct: 15.0,
                    elbow: el.max(5.0),
                };
                let lift = (PI * ((c - 0.2) / 0.7).clamp(0.0, 1.0)).sin();
                (
                    [arm, arm],
                    [
                        rest_leg_with(10.0, 15.0),
                        Leg {
                            hip: 80.0 * lift,
                            abduct: 3.0,
                            knee: 100.0 * lift,
                            ankle: 0.0,
                        },
                    ],
                )
            }
            Activity::Ladder => {
                root_y = Some(rig.rest[Joint::Root.index()].y);
                trunk = -4.0;
                let arm = |p: f32| Arm {
                    flex: 140.0 + 25.0 * p.sin(),
                    abduct: 12.0,
                    elbow: 60.0 - 30.0 * p.sin(),
                };
                let leg = |p: f32| Leg {
                    hip: 40.0 + 30.0 * p.sin(),
                    abduct: 4.0,
                    knee: 70.0 + 30.0 * p.sin(),
                    ankle: 0.0,
                };
                ([arm(left), arm(right)], [leg(right), leg(left)])
            }
            Activity::Fall => {
                root_y = Some(rig.rest[Joint::Root.index()].y);
                trunk = -5.0;
                let flail = (TAU * self.t * 1.5).sin();
                let arm = |s: f32| Arm {
                    flex: 20.0 + 10.0 * flail * s,
                    abduct: 70.0 + 10.0 * flail,
                    elbow: 25.0,
                };
                (
                    [arm(1.0), arm(-1.0)],
                    [rest_leg_with(25.0, 35.0), rest_leg_with(15.0, 30.0)],
                )
            }
            Activity::Lie => {
                root = lean(-90.0);
                (
                    [
                        Arm {
                            abduct: 15.0,
                            ..rest_arm
                        },
                        Arm {
                            abduct: 15.0,
                            ..rest_arm
                        },
                    ],
                    [rest_leg_with(4.0, 6.0), rest_leg_with(4.0, 6.0)],
                )
            }
        };
        // Things in the hands: held forward to be seen and used, a load carried in both arms
        // before the chest (leaning back against it), a drag pulled with the arms back and the
        // body leaning into it.
        let upright = matches!(
            activity,
            Activity::Stand
                | Activity::Walk
                | Activity::Jog
                | Activity::Sprint
                | Activity::Wade
                | Activity::Crouch
        );
        if upright {
            let h = d.holding;
            if h.dragging {
                for a in &mut arms {
                    a.flex = -25.0;
                    a.abduct = rest_abduct + 6.0;
                    a.elbow = 15.0;
                }
                trunk += 18.0;
            } else if h.both {
                for a in &mut arms {
                    a.flex = 38.0 + 0.2 * a.flex;
                    a.abduct = 12.0;
                    a.elbow = 85.0;
                }
                trunk -= 4.0;
            } else {
                for (held, a) in [h.left, h.right].into_iter().zip(arms.iter_mut()) {
                    if held {
                        a.flex = 22.0 + 0.4 * a.flex;
                        a.elbow = 70.0;
                    }
                }
            }
        }
        // Cold: arms wrapped around the body, and a tremor.
        let empty = d.holding == Holding::default();
        let hug = ((d.shiver - 0.35) / 0.4).clamp(0.0, 1.0)
            * if matches!(activity, Activity::Stand | Activity::Walk) && empty {
                1.0
            } else {
                0.0
            };
        let tremor = d.shiver * (TAU * self.t * 9.0).sin();
        if hug > 0.0 {
            for a in &mut arms {
                a.flex += (45.0 - a.flex) * hug;
                a.abduct += (-28.0 - a.abduct) * hug;
                a.elbow += (115.0 - a.elbow) * hug;
            }
        }
        for a in &mut arms {
            a.abduct += 1.5 * tremor;
        }
        for l in &mut legs {
            l.knee += 1.0 * tremor.abs();
        }
        let mut rot = [Quat::IDENTITY; JOINTS];
        let mut set = |j: Joint, q: Quat| rot[j.index()] = q;
        set(Joint::Root, root);
        set(Joint::Waist, lean(trunk * 0.5));
        set(Joint::Chest, lean(trunk * 0.5) * chest);
        // The head looks where the eyes look, against the trunk's lean.
        let pitch = (d.look_pitch - trunk).clamp(-60.0, 70.0) + neck_lean;
        let turn = d.look_yaw.clamp(-75.0, 75.0);
        set(Joint::Neck, twist(turn * 0.4) * lean(pitch * 0.35));
        set(
            Joint::Head,
            twist(turn * 0.6) * lean(pitch * 0.65 + 0.8 * tremor),
        );
        for (s, a, sh, el) in [
            (1.0, &arms[0], Joint::ShoulderL, Joint::ElbowL),
            (-1.0, &arms[1], Joint::ShoulderR, Joint::ElbowR),
        ] {
            set(sh, flex(a.flex) * abduct(s, a.abduct));
            set(el, flex(a.elbow) * abduct(s, carry));
        }
        for (s, l, hip, kn, an) in [
            (1.0, &legs[0], Joint::HipL, Joint::KneeL, Joint::AnkleL),
            (-1.0, &legs[1], Joint::HipR, Joint::KneeR, Joint::AnkleR),
        ] {
            set(hip, flex(l.hip) * abduct(s, l.abduct));
            set(kn, knee(l.knee));
            set(an, dorsi(l.ankle));
        }
        let mut pose = Pose {
            rot,
            root_y: rig.rest[Joint::Root.index()].y,
        };
        pose.root_y = match root_y {
            Some(y) => y,
            None if activity.grounded() => pose.root_y - pose.lowest(rig),
            None => pose.root_y,
        };
        pose
    }
}

/// A gait: how long a foot is down, and the angles of the legs and arms.
struct Gait {
    /// Share of the cycle a foot is on the ground.
    stance: f32,
    /// Hip flexion as the foot comes down, as it leaves, and at the top of the swing.
    hip_contact: f32,
    hip_off: f32,
    hip_peak: f32,
    /// Knee flexion as the foot comes down, the extra under load, bending before the foot
    /// leaves, and at the top of the swing.
    knee_contact: f32,
    knee_load: f32,
    knee_pre: f32,
    knee_swing: f32,
    /// Plantar flexion pushing off.
    push: f32,
    arm: f32,
    elbow: f32,
    elbow_swing: f32,
    lean: f32,
    twist: f32,
    /// Height of the arc between strides when both feet are off the ground (m).
    flight_m: f32,
}

const WALK: Gait = Gait {
    stance: 0.62,
    hip_contact: 22.0,
    hip_off: -13.0,
    hip_peak: 25.0,
    knee_contact: 4.0,
    knee_load: 12.0,
    knee_pre: 35.0,
    knee_swing: 62.0,
    push: 18.0,
    arm: 16.0,
    elbow: 12.0,
    elbow_swing: 15.0,
    lean: 2.0,
    twist: 4.0,
    flight_m: 0.0,
};
const JOG: Gait = Gait {
    stance: 0.4,
    hip_contact: 22.0,
    hip_off: -15.0,
    hip_peak: 40.0,
    knee_contact: 18.0,
    knee_load: 22.0,
    knee_pre: 0.0,
    knee_swing: 95.0,
    push: 25.0,
    arm: 30.0,
    elbow: 80.0,
    elbow_swing: 10.0,
    lean: 6.0,
    twist: 6.0,
    flight_m: 0.03,
};
const SPRINT: Gait = Gait {
    stance: 0.3,
    hip_contact: 25.0,
    hip_off: -18.0,
    hip_peak: 60.0,
    knee_contact: 20.0,
    knee_load: 20.0,
    knee_pre: 0.0,
    knee_swing: 125.0,
    push: 30.0,
    arm: 55.0,
    elbow: 90.0,
    elbow_swing: 10.0,
    lean: 12.0,
    twist: 8.0,
    flight_m: 0.035,
};
const WADE: Gait = Gait {
    stance: 0.65,
    hip_contact: 20.0,
    hip_off: -10.0,
    hip_peak: 30.0,
    knee_contact: 8.0,
    knee_load: 10.0,
    knee_pre: 25.0,
    knee_swing: 75.0,
    push: 10.0,
    arm: 12.0,
    elbow: 25.0,
    elbow_swing: 10.0,
    lean: 4.0,
    twist: 3.0,
    flight_m: 0.0,
};

/// A leg at `q` of its cycle: the foot comes down at 0, carries the body (flat, rolling onto
/// the toes at the end) until it leaves, then swings forward, knee bent, reaching past where
/// it will land and settling back to meet the ground.
fn stride(g: &Gait, q: f32) -> Leg {
    let q = q.rem_euclid(1.0);
    if q < g.stance {
        let u = q / g.stance;
        let hip = g.hip_contact + (g.hip_off - g.hip_contact) * u;
        // The knee gives under the load, straightens, then bends as the heel rises.
        let knee = g.knee_contact
            + g.knee_load * (PI * (u / 0.7).min(1.0)).sin()
            + g.knee_pre * smooth(0.7, 1.0, u);
        let heel_off = smooth(0.65, 1.0, u);
        Leg {
            hip,
            abduct: 1.5,
            knee,
            ankle: knee - hip - g.push * heel_off,
        }
    } else {
        let v = (q - g.stance) / (1.0 - g.stance);
        let hip = if v < 0.75 {
            g.hip_off + (g.hip_peak - g.hip_off) * (PI / 2.0 * v / 0.75).sin()
        } else {
            g.hip_peak + (g.hip_contact - g.hip_peak) * smooth(0.75, 1.0, v)
        };
        let knee_off = g.knee_contact + g.knee_pre;
        let knee = if v < 0.35 {
            knee_off + (g.knee_swing - knee_off) * smooth(0.0, 0.35, v)
        } else {
            g.knee_swing + (g.knee_contact - g.knee_swing) * smooth(0.35, 1.0, v)
        };
        // Toes come up off the push; the foot meets the ground flat.
        let ankle = (1.0 - smooth(0.0, 0.3, v)) * (-g.push) + smooth(0.0, 0.3, v) * 8.0;
        let land = smooth(0.85, 1.0, v);
        Leg {
            hip,
            abduct: 1.5,
            knee,
            ankle: ankle * (1.0 - land) + (g.knee_contact - g.hip_contact) * land,
        }
    }
}

/// The root's height that puts a leg's foot on the ground.
fn support_y(rig: &Rig, l: &Leg) -> f32 {
    let mut rot = [Quat::IDENTITY; JOINTS];
    rot[Joint::HipL.index()] = flex(l.hip) * abduct(1.0, l.abduct);
    rot[Joint::KneeL.index()] = knee(l.knee);
    rot[Joint::AnkleL.index()] = dorsi(l.ankle);
    let rest = rig.rest[Joint::Root.index()].y;
    let pose = Pose { rot, root_y: rest };
    let joints = pose.joints(rig);
    let mut low = f32::INFINITY;
    for p in rig
        .parts
        .iter()
        .filter(|p| matches!(p.joint, Joint::HipL | Joint::KneeL | Joint::AnkleL))
    {
        let m = joints[p.joint.index()];
        let h = p.size / 2.0;
        for k in 0..8 {
            let c = Vec3::new(
                if k & 1 == 0 { -h.x } else { h.x },
                if k & 2 == 0 { -h.y } else { h.y },
                if k & 4 == 0 { -h.z } else { h.z },
            );
            low = low.min(m.transform_point3(p.center + c).y);
        }
    }
    rest - low
}

/// Smoothstep from `e0` to `e1` (either way round).
fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn rest_leg_with(hip: f32, knee: f32) -> Leg {
    Leg {
        hip,
        abduct: 2.0,
        knee,
        ankle: 0.0,
    }
}

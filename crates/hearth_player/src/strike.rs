//! Blows struck by a body (E §3.2): a punch, a kick, or a blow of what is in the hand (a thrust,
//! a swing, a slash, a stab, a strike). Each is drawn back, struck and recovered from at the
//! speed of a real body, costs stamina, reaches as far as the arm (or the leg) and the weapon
//! do, and sweeps its striking end along a path that is tested against bodies segment by
//! segment, so a blow can miss. What it strikes takes a blow of the weapon's kind (blunt,
//! cutting or piercing) with the energy and the momentum the body puts into it; tired, it puts
//! in less, and more slowly.
//!
//! The figures are an untrained adult's, rounded: a straight punch lands about 0.15 s after it
//! starts with the fist at 6–7 m/s and some 40 J (an Olympic boxer's fist reaches 9 m/s:
//! Walilko, Viano and Bir 2005, Br J Sports Med 39:710–719); a front kick's foot reaches 7–10
//! m/s with some 120 J; a club or a hafted axe swung comes round at 15–20 m/s; a spear thrust
//! with the body behind it lands with about 150 J. Shoulder to fist is about 0.4 of the body's
//! height, hip to foot about 0.53.

use glam::DVec3;
use hearth_content::schema::item::Use;
use hearth_items::ItemKind;

/// The eye's height on a body, as a share of its stature.
pub const EYE_SHARE: f64 = 0.93;

/// A kind of blow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attack {
    /// The fist, straight from the shoulder (also the fist that holds a thing with no blow of
    /// its own).
    Punch,
    /// The foot, driven out from the hip.
    Kick,
    /// A point driven along the look with the body behind it.
    Thrust,
    /// An arc across the body.
    Swing,
    /// An edge drawn across in a short arc.
    Slash,
    /// A point, a short way.
    Stab,
    /// Brought down hard from overhead.
    Strike,
}

impl Attack {
    /// The blow a thing in the hand makes as its use (none for a use that isn't a blow; the
    /// fist for a thing with no use).
    pub fn of(primary: Option<Use>) -> Option<Attack> {
        Some(match primary {
            None => Attack::Punch,
            Some(Use::Thrust) => Attack::Thrust,
            Some(Use::Swing) => Attack::Swing,
            Some(Use::Slash) => Attack::Slash,
            Some(Use::Stab) => Attack::Stab,
            Some(Use::Strike) => Attack::Strike,
            Some(Use::HoldUp | Use::Draw) => return None,
        })
    }

    /// Its timing at full strength: drawn back, struck, recovered from (s). Heavier things are
    /// slower to draw back and bring round.
    pub fn timing(self, w: &Weapon) -> Timing {
        let (windup_s, strike_s, recover_s) = match self {
            Attack::Punch => (0.15, 0.10, 0.25),
            Attack::Kick => (0.25, 0.15, 0.40),
            // Driven from the ready, the spear already back.
            Attack::Thrust => (0.15, 0.12, 0.40),
            Attack::Swing => (0.35, 0.15, 0.45),
            Attack::Slash => (0.15, 0.12, 0.25),
            Attack::Stab => (0.15, 0.10, 0.25),
            Attack::Strike => (0.25, 0.10, 0.35),
        };
        let k = match self {
            Attack::Thrust | Attack::Swing | Attack::Strike => {
                (w.mass_kg as f64).sqrt().clamp(1.0, 1.6)
            }
            _ => 1.0,
        };
        Timing {
            windup_s: windup_s * k,
            strike_s: strike_s * k,
            recover_s: recover_s * k,
        }
    }

    /// The seconds of all-out effort it costs (the body's `all_out_s` of them spend all its
    /// stamina).
    pub fn effort_s(self, w: &Weapon) -> f64 {
        match self {
            Attack::Punch | Attack::Strike => 0.5,
            Attack::Kick | Attack::Thrust => 0.8,
            Attack::Swing => 0.6 + 0.4 * (w.mass_kg as f64).min(3.0),
            Attack::Slash | Attack::Stab => 0.4,
        }
    }

    /// The energy it lands with at full strength (J): a long spear driven with the body behind
    /// it, other things by the arm, the more for their weight.
    pub fn energy_j(self, w: &Weapon) -> f32 {
        let m = w.mass_kg;
        match self {
            Attack::Punch => 40.0,
            Attack::Kick => 120.0,
            Attack::Thrust if w.length_m >= 1.5 => 150.0,
            Attack::Thrust | Attack::Strike => 40.0 + 60.0 * m.min(1.5),
            Attack::Swing => 60.0 + 100.0 * m.min(2.5),
            Attack::Slash => 25.0 + 20.0 * m.min(0.5),
            Attack::Stab => 30.0 + 40.0 * m.min(0.5),
        }
    }

    /// The mass moving with the striking end (kg): the fist and forearm, the leg, the weapon
    /// and the arm, the body behind a long spear.
    fn moving_kg(self, w: &Weapon) -> f32 {
        match self {
            Attack::Punch => 2.0,
            Attack::Kick => 5.0,
            Attack::Thrust if w.length_m >= 1.5 => 10.0,
            Attack::Thrust | Attack::Strike => 2.0 + w.mass_kg,
            Attack::Swing => 1.5 + w.mass_kg,
            Attack::Slash | Attack::Stab => 1.5,
        }
    }

    /// How far it reaches from the eye (m): the arm, or the leg, and the weapon beyond the hand.
    pub fn reach_m(self, w: &Weapon, stature_m: f32) -> f64 {
        let arm = 0.4 * stature_m as f64;
        match self {
            Attack::Punch => arm,
            Attack::Kick => 0.53 * stature_m as f64,
            _ => arm + w.length_m as f64,
        }
    }

    /// The blow it lands with `strength` (0–1) of the body's, moving along `along`.
    pub fn blow(self, w: &Weapon, strength: f32, along: DVec3) -> Landed {
        let energy_j = self.energy_j(w) * strength;
        let (piercing, cutting) = match self {
            Attack::Punch | Attack::Kick => (0.0, 0.0),
            // A point; an edge brought to a point pierces a little.
            Attack::Thrust | Attack::Stab => (w.piercing.max(w.cutting * 0.5), 0.0),
            Attack::Swing | Attack::Slash => (0.0, w.cutting),
            Attack::Strike => (0.0, w.cutting * 0.6),
        };
        // p = √(2 m E): the moving mass at the speed its energy gives it.
        let momentum = (2.0 * self.moving_kg(w) * energy_j).sqrt() as f64;
        Landed {
            energy_j,
            piercing,
            cutting,
            push: along.normalize_or_zero() * momentum,
        }
    }

    /// The path its striking end sweeps while it strikes, for a body `stature_m` tall whose eye
    /// is at `eye` looking along `dir`, with the right hand (or foot) or the left: along the
    /// look for a punch, a thrust or a stab; across the body for a swing (from the hand's side
    /// to the other, falling a little) or a slash; down from overhead for a strike; out from the
    /// hip toward where the eye looks for a kick.
    pub fn path(
        self,
        w: &Weapon,
        stature_m: f32,
        eye: DVec3,
        dir: DVec3,
        right: bool,
    ) -> Vec<DVec3> {
        let f = dir.normalize_or(DVec3::Z);
        let flat = DVec3::new(f.x, 0.0, f.z).normalize_or(DVec3::Z);
        // The body's right: forward × up.
        let side = flat.cross(DVec3::Y).normalize_or(DVec3::X);
        let up = side.cross(f).normalize_or(DVec3::Y);
        let s = if right { 1.0 } else { -1.0 };
        let reach = self.reach_m(w, stature_m);
        let arc = |from: f64, to: f64, n: usize, pivot: DVec3, r: f64, plane: DVec3, rise: f64| {
            (0..=n)
                .map(|i| {
                    let a = (from + (to - from) * i as f64 / n as f64).to_radians();
                    pivot + (f * a.cos() + plane * a.sin()) * r + up * (rise * a.sin())
                })
                .collect::<Vec<_>>()
        };
        match self {
            Attack::Punch | Attack::Thrust | Attack::Stab => {
                vec![
                    eye + f * 0.25,
                    eye + f * (0.25 + reach) * 0.5,
                    eye + f * reach,
                ]
            }
            Attack::Kick => {
                let feet = eye.y - EYE_SHARE * stature_m as f64;
                let hip = eye - DVec3::Y * (0.4 * stature_m as f64) + side * (s * 0.1);
                // Toward what the look rests on a kick's length out, or nearer where it comes
                // down to the ground: no lower than the ground, no higher than the chest.
                let level = (f.x * f.x + f.z * f.z).sqrt().max(0.2);
                let mut along = (0.8 / level).min(3.0);
                if f.y < 0.0 {
                    along = along.min((eye.y - (feet + 0.15)) / -f.y);
                }
                let aim = eye + f * along;
                let aim = DVec3::new(aim.x, aim.y.clamp(feet + 0.05, hip.y + 0.45), aim.z);
                let out = (aim - hip).normalize_or(flat);
                vec![
                    hip + out * 0.2,
                    hip + out * (0.2 + reach) * 0.5,
                    hip + out * reach,
                ]
            }
            // From the hand's side, across and a little down.
            Attack::Swing => arc(70.0, -45.0, 8, eye - up * 0.15, reach, side * s, 0.15),
            Attack::Slash => arc(45.0, -45.0, 6, eye - up * 0.15, reach, side * s, 0.25),
            // From above the head, down to before the body.
            Attack::Strike => arc(
                65.0,
                -25.0,
                6,
                eye - up * 0.1 + side * (s * 0.15),
                reach * 0.95,
                up,
                0.0,
            ),
        }
    }
}

/// A blow's timing (s).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timing {
    pub windup_s: f64,
    pub strike_s: f64,
    pub recover_s: f64,
}

impl Timing {
    pub fn total(&self) -> f64 {
        self.windup_s + self.strike_s + self.recover_s
    }
}

/// What a blow is struck with: the thing in the hand, or none (the bare fist or foot).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Weapon {
    pub mass_kg: f32,
    /// How far its striking end lies beyond the hand (m).
    pub length_m: f32,
    /// How sharp its point is, and its edge (0–1).
    pub piercing: f32,
    pub cutting: f32,
}

impl Weapon {
    /// A thing in the hand as a weapon: a spear's reach as its data gives it, other things
    /// held near one end.
    pub fn of(kind: &ItemKind) -> Self {
        let longest = kind.size_m.iter().copied().fold(0.0, f32::max);
        Self {
            mass_kg: kind.mass_kg,
            length_m: kind.property("reach_m").unwrap_or(longest * 0.8),
            piercing: kind.property("piercing").unwrap_or(0.0),
            cutting: kind.property("sharp_edge").unwrap_or(0.0),
        }
    }
}

/// The blow that lands: its energy (J), its point and edge (0–1), and the momentum it carries
/// into what it strikes (kg m/s).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Landed {
    pub energy_j: f32,
    pub piercing: f32,
    pub cutting: f32,
    pub push: DVec3,
}

/// A blow under way: what, with what, which hand (or foot), along which way, how long since it
/// began, whether it has struck yet, and the strength put into it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Striking {
    pub attack: Attack,
    pub weapon: Weapon,
    /// The right hand (or foot), or the left.
    pub right: bool,
    pub dir: DVec3,
    pub t: f64,
    pub struck: bool,
    /// 0.4–1: a tired body strikes weaker and slower.
    pub strength: f32,
}

impl Striking {
    /// A blow begun with `stamina` (0–1) left.
    pub fn new(attack: Attack, weapon: Weapon, right: bool, dir: DVec3, stamina: f64) -> Self {
        Self {
            attack,
            weapon,
            right,
            dir: dir.normalize_or(DVec3::Z),
            t: 0.0,
            struck: false,
            strength: (0.4 + 0.6 * stamina.clamp(0.0, 1.0)) as f32,
        }
    }

    /// Its timing at the strength put into it.
    pub fn timing(&self) -> Timing {
        let t = self.attack.timing(&self.weapon);
        let slow = 1.0 + 0.5 * (1.0 - self.strength as f64);
        Timing {
            windup_s: t.windup_s * slow,
            strike_s: t.strike_s * slow,
            recover_s: t.recover_s * slow,
        }
    }

    /// On by `dt`: true at the moment it strikes (once), when its path is to be tested.
    pub fn advance(&mut self, dt: f64) -> bool {
        self.t += dt;
        if !self.struck && self.t >= self.timing().windup_s {
            self.struck = true;
            return true;
        }
        false
    }

    pub fn done(&self) -> bool {
        self.t >= self.timing().total()
    }

    /// Where in it the body is: 0–1 drawing back, 1–2 striking, 2–3 recovering, 3 done.
    pub fn phase(&self) -> f32 {
        let t = self.timing();
        let p = if self.t < t.windup_s {
            self.t / t.windup_s
        } else if self.t < t.windup_s + t.strike_s {
            1.0 + (self.t - t.windup_s) / t.strike_s
        } else {
            2.0 + ((self.t - t.windup_s - t.strike_s) / t.recover_s).min(1.0)
        };
        p as f32
    }

    /// The blow it lands.
    pub fn landed(&self) -> Landed {
        self.attack.blow(&self.weapon, self.strength, self.dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EYE: DVec3 = DVec3::new(0.0, 1.62, 0.0);

    fn spear() -> Weapon {
        Weapon {
            mass_kg: 1.4,
            length_m: 2.2,
            piercing: 0.85,
            cutting: 0.0,
        }
    }

    fn club() -> Weapon {
        Weapon {
            mass_kg: 1.0,
            length_m: 0.6,
            ..Weapon::default()
        }
    }

    #[test]
    fn blows_take_a_real_bodys_time() {
        let fist = Attack::Punch.timing(&Weapon::default());
        assert!(
            (0.12..0.2).contains(&fist.windup_s),
            "a punch lands in ~0.15 s"
        );
        assert!(
            (0.4..0.6).contains(&fist.total()),
            "and comes back within half a second"
        );
        let kick = Attack::Kick.timing(&Weapon::default());
        assert!(kick.total() > fist.total(), "a kick is slower than a punch");
        let light = Attack::Swing.timing(&club());
        let heavy = Attack::Swing.timing(&Weapon {
            mass_kg: 2.5,
            ..club()
        });
        assert!(
            heavy.total() > light.total(),
            "a heavier thing comes round slower"
        );
        // Tired, slower and weaker.
        let fresh = Striking::new(Attack::Punch, Weapon::default(), true, DVec3::Z, 1.0);
        let spent = Striking::new(Attack::Punch, Weapon::default(), true, DVec3::Z, 0.0);
        assert!(spent.timing().total() > fresh.timing().total());
        assert!(spent.landed().energy_j < fresh.landed().energy_j);
    }

    #[test]
    fn reach_is_the_arm_or_leg_and_the_weapon() {
        let bare = Weapon::default();
        let arm = Attack::Punch.reach_m(&bare, 1.75);
        assert!((0.65..0.75).contains(&arm), "{arm}");
        assert!(Attack::Kick.reach_m(&bare, 1.75) > arm);
        assert!(Attack::Thrust.reach_m(&spear(), 1.75) > 2.5);
        // Each path ends at its reach.
        for (a, w) in [
            (Attack::Punch, bare),
            (Attack::Thrust, spear()),
            (Attack::Swing, club()),
            (Attack::Slash, club()),
            (Attack::Strike, club()),
        ] {
            let path = a.path(&w, 1.75, EYE, DVec3::Z, true);
            let far = path.iter().map(|p| (*p - EYE).length()).fold(0.0, f64::max);
            let reach = a.reach_m(&w, 1.75);
            assert!(
                far <= reach + 0.3 && far >= reach * 0.75,
                "{a:?}: {far} of {reach}"
            );
        }
    }

    #[test]
    fn a_swing_crosses_the_body_from_the_hands_side() {
        let path = Attack::Swing.path(&club(), 1.75, EYE, DVec3::Z, true);
        // Facing +z the body's right is −x: a right-handed swing starts there and ends left.
        let (first, last) = (path[0], path[path.len() - 1]);
        assert!(first.x < -0.5 && last.x > 0.5, "{first} → {last}");
        assert!(path.iter().all(|p| p.z > -0.1), "all of it before the body");
        let left = Attack::Swing.path(&club(), 1.75, EYE, DVec3::Z, false);
        assert!(left[0].x > 0.5, "a left-handed one from the left");
        // A strike comes down from above the head.
        let down = Attack::Strike.path(&club(), 1.75, EYE, DVec3::Z, true);
        assert!(down[0].y > EYE.y + 0.5 && down[down.len() - 1].y < EYE.y);
        // A kick goes out low, toward the ground looked at.
        let kick = Attack::Kick.path(
            &Weapon::default(),
            1.75,
            EYE,
            DVec3::new(0.0, -1.0, 1.0),
            true,
        );
        assert!(kick.iter().all(|p| p.y < 1.2), "{kick:?}");
        assert!(kick[kick.len() - 1].z > 0.6);
    }

    #[test]
    fn blows_carry_their_kind_and_momentum() {
        let blade = Weapon {
            mass_kg: 0.05,
            length_m: 0.07,
            piercing: 0.0,
            cutting: 1.0,
        };
        let cut = Attack::Slash.blow(&blade, 1.0, DVec3::X);
        assert!(cut.cutting > 0.9 && cut.piercing == 0.0);
        let thrust = Attack::Thrust.blow(&spear(), 1.0, DVec3::Z);
        assert!(thrust.piercing > 0.8 && thrust.energy_j == 150.0);
        let punch = Attack::Punch.blow(&Weapon::default(), 1.0, DVec3::Z);
        assert!(punch.piercing == 0.0 && punch.cutting == 0.0);
        // A fist's 40 J moves about 2 kg at 6–7 m/s.
        let p = punch.push.length();
        assert!((11.0..15.0).contains(&p), "{p} kg m/s");
        assert!(
            Attack::Kick
                .blow(&Weapon::default(), 1.0, DVec3::Z)
                .push
                .length()
                > p
        );
    }

    #[test]
    fn a_blow_strikes_once_at_the_end_of_its_wind_up() {
        let mut s = Striking::new(Attack::Swing, club(), true, DVec3::Z, 1.0);
        let mut moments = 0;
        let mut t = 0.0;
        while !s.done() {
            if s.advance(0.05) {
                moments += 1;
                assert!((s.t - s.timing().windup_s).abs() <= 0.05);
            }
            t += 0.05;
            assert!(t < 3.0);
        }
        assert_eq!(moments, 1);
        assert!((2.9..=3.0).contains(&s.phase()));
    }
}

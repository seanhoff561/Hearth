//! Injuries and illness (v2 §9.6). An injury sits on a body region with a severity; it bleeds
//! (clotting over minutes, much less under pressure or a bandage), hurts, may become infected if
//! not cleaned within hours, and heals over its time on its scale — slower when the body is
//! starving, dry, cold, infected or the fracture unsplinted, faster at rest. An illness waits out
//! its onset, then runs its course; its effects (fever, water loss, weakness) act on the body,
//! and a course that would kill (rolled when caught) is turned by any of its treatments.

use hearth_content::schema::body::{BodyRegion, Illness, Injury};
use serde::{Deserialize, Serialize};

/// Which side of the body a region on a paired limb is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Left,
    Right,
    Middle,
}

/// Treatments that keep a wound from becoming infected.
pub const CLEANING: [&str; 5] = ["clean_water", "boiled_water", "honey", "resin", "plantain"];
/// Treatments that stem bleeding.
pub const STEMMING: [&str; 2] = ["pressure", "bandage"];
/// Herbs pressed on a wound that slow its bleeding a little (yarrow).
pub const HERBAL_STEMMING: [&str; 1] = ["yarrow"];
/// Real seconds after which an uncleaned wound may become infected.
pub const INFECTION_WINDOW_S: f64 = 6.0 * 3600.0;

/// An injury the body has.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InjuryState {
    /// Content id of the injury type.
    pub id: String,
    pub region: BodyRegion,
    pub side: Side,
    /// 0–1.
    pub severity: f32,
    /// Bleeding now (ml per real minute).
    pub bleeding_ml_min: f64,
    /// 0 fresh – 1 healed.
    pub healed: f64,
    /// Real seconds since it happened.
    pub age_s: f64,
    pub treatments: Vec<String>,
    pub infected: bool,
    /// Whether the chance of infection has been rolled.
    pub infection_rolled: bool,
}

impl InjuryState {
    pub fn new(kind: &Injury, region: BodyRegion, side: Side, severity: f32) -> Self {
        let s = severity.clamp(0.0, 1.0);
        let (lo, hi) = kind.bleeding_ml_per_min;
        Self {
            id: kind.id.clone(),
            region,
            side,
            severity: s,
            bleeding_ml_min: (lo + (hi - lo) * s) as f64,
            healed: 0.0,
            age_s: 0.0,
            treatments: Vec::new(),
            infected: false,
            infection_rolled: false,
        }
    }

    pub fn treated_with(&self, any: &[&str]) -> bool {
        self.treatments.iter().any(|t| any.contains(&t.as_str()))
    }

    /// Clotting time constant (real seconds): small wounds stop in minutes, deep ones barely.
    pub fn clotting_s(&self, kind: &Injury) -> f64 {
        let deep = kind.bleeding_ml_per_min.1 as f64 / 200.0;
        300.0 * (1.0 + 30.0 * deep * self.severity as f64 * self.severity as f64)
    }

    /// Pain now, 0–1.
    pub fn pain(&self, kind: &Injury) -> f64 {
        kind.pain as f64 * (0.5 + 0.5 * self.severity as f64) * (1.0 - self.healed).powf(0.7)
            + if self.infected { 0.2 } else { 0.0 }
    }
}

/// An illness the body has caught.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IllnessState {
    pub id: String,
    /// Seconds of play until the symptoms start (0 once they have).
    pub onset_s: f64,
    /// Seconds of play the illness still lasts after onset.
    pub left_s: f64,
    /// Rolled when caught: untreated, this course kills at its end.
    pub fatal: bool,
    pub treatments: Vec<String>,
}

impl IllnessState {
    pub fn active(&self) -> bool {
        self.onset_s <= 0.0
    }

    pub fn treated(&self, kind: &Illness) -> bool {
        self.treatments.iter().any(|t| kind.treatments.contains(t))
    }
}

/// How a region is used: legs carry, arms hold.
pub fn is_leg(r: BodyRegion) -> bool {
    matches!(
        r,
        BodyRegion::UpperLeg | BodyRegion::LowerLeg | BodyRegion::Foot
    )
}

pub fn is_arm(r: BodyRegion) -> bool {
    matches!(
        r,
        BodyRegion::UpperArm | BodyRegion::LowerArm | BodyRegion::Hand
    )
}

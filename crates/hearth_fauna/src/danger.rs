//! Danger (V2-7 (g), v2 §7.5): when and how an animal turns on a person, and why — always for a
//! reason a naturalist would give: a mother whose young the person has come too near; one
//! surprised too close to get away first; one cornered; a male in the rut; a hungry hunter in a
//! lean season, the person seeming small (crouched, alone, at night, hurt); a snake stepped
//! near; one at its kill. Most turn on a person with a charge that stops short (a bluff) unless
//! the person runs or keeps coming; some close (a bite, a mauling, a goring, a kick). A person
//! can turn it: fire keeps hunters off; standing tall, facing it and shouting turns most back;
//! running from a hunter sets it after them. One faced down or hurt is warier after.

use glam::DVec3;
use hearth_content::schema::body::BodyRegion;
use hearth_content::schema::fauna::BodyPlan;

use crate::mind::Presence;
use crate::species::Species;

/// Why an animal turned on a person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// The person came too near its young.
    DefendingYoung,
    /// The person came near its kill.
    DefendingKill,
    /// Come upon too close to get away first.
    Surprise,
    /// No way left to run.
    Cornered,
    /// A male in the rut.
    Rut,
    /// Hungry in a lean season, the person seeming small and alone.
    Hunger,
    /// Stepped near.
    SteppedNear,
    /// Hurt by the person.
    Provoked,
}

impl Cause {
    /// The reason in words, as the person might learn it.
    pub fn words(self) -> &'static str {
        match self {
            Cause::DefendingYoung => "you came too near its young",
            Cause::DefendingKill => "you came near its kill",
            Cause::Surprise => "you came upon it too close for it to get away",
            Cause::Cornered => "it had no way left to run",
            Cause::Rut => "it is the rut",
            Cause::Hunger => "it is hungry in a lean season, and you seemed small and alone",
            Cause::SteppedNear => "you stepped too near it",
            Cause::Provoked => "you hurt it",
        }
    }
}

/// An animal turned on a person: why, whether it means to close (else it stops short, a
/// bluff), whether it is still stalking, how long it has been at it, and when it may strike
/// again.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hostile {
    pub cause: Cause,
    pub contact: bool,
    pub stalking: bool,
    pub t: f32,
    pub cooldown: f32,
}

/// An attack on a person: by whom, why, and what it did (an injury as the body's data names
/// it, where, how bad; venom).
#[derive(Debug, Clone, PartialEq)]
pub struct Attack {
    pub animal: u64,
    pub species: u16,
    pub cause: Cause,
    pub injury: &'static str,
    pub region: BodyRegion,
    pub severity: f32,
    pub venom: bool,
    pub words: String,
}

/// A kill of one animal by another.
#[derive(Debug, Clone, PartialEq)]
pub struct Kill {
    pub predator: u16,
    pub prey: u16,
    pub at: DVec3,
    pub cause: Cause,
}

/// How a body hurts a person: the injury, where, how bad, and in words.
pub fn blow(sp: &Species, cause: Cause, roll: f32) -> (&'static str, BodyRegion, f32, String) {
    let heavy = (sp.mass_kg / 150.0).clamp(0.0, 1.0);
    let name = sp.name.to_lowercase();
    match sp.plan {
        BodyPlan::Bear => (
            "deep_wound",
            if roll < 0.5 {
                BodyRegion::UpperArm
            } else {
                BodyRegion::Chest
            },
            0.45 + heavy * 0.4,
            format!("The {name} knocks you down and mauls you"),
        ),
        BodyPlan::Ungulate if sp.id.ends_with("wild_boar") => (
            "deep_wound",
            if roll < 0.6 {
                BodyRegion::UpperLeg
            } else {
                BodyRegion::LowerLeg
            },
            0.4 + heavy * 0.3,
            format!("The {name} slashes at your legs with its tusks"),
        ),
        BodyPlan::Ungulate if cause == Cause::Rut || sp.shape.head_gear.is_some() => (
            "puncture",
            if roll < 0.5 {
                BodyRegion::Abdomen
            } else {
                BodyRegion::Chest
            },
            0.35 + heavy * 0.4,
            format!("The {name} drives into you head down"),
        ),
        BodyPlan::Ungulate => (
            "bruise",
            BodyRegion::Chest,
            0.3 + heavy * 0.3,
            format!("The {name} strikes you with its forefeet"),
        ),
        BodyPlan::Snake => (
            "bite",
            if roll < 0.7 {
                BodyRegion::LowerLeg
            } else {
                BodyRegion::Hand
            },
            0.2,
            format!("The {name} bites you"),
        ),
        BodyPlan::BirdPerching
        | BodyPlan::BirdGround
        | BodyPlan::Raptor
        | BodyPlan::Waterfowl
        | BodyPlan::Seabird => (
            "cut",
            BodyRegion::Head,
            0.15,
            format!("The {name} flies at your head"),
        ),
        _ => (
            "bite",
            if roll < 0.5 {
                BodyRegion::LowerLeg
            } else {
                BodyRegion::LowerArm
            },
            0.2 + heavy * 0.5,
            format!("The {name} bites you"),
        ),
    }
}

/// How far an animal guards its young from a person (m).
pub fn guard_m(sp: &Species) -> f64 {
    match sp.plan {
        BodyPlan::Bear => 30.0,
        _ => (8.0 + sp.mass_kg.sqrt() as f64).min(25.0),
    }
}

/// How likely a charge for a cause closes rather than stopping short.
pub fn closes(sp: &Species, cause: Cause) -> f32 {
    match cause {
        Cause::Surprise | Cause::Cornered | Cause::SteppedNear | Cause::Provoked => 1.0,
        Cause::Hunger => 0.9,
        _ => match sp.plan {
            BodyPlan::Bear => 0.3,
            BodyPlan::Ungulate if sp.id.ends_with("wild_boar") => 0.6,
            _ => 0.4,
        },
    }
}

/// Whether something holds that might turn an animal on a person now (each only for an animal
/// aware of the person, or startled, or a snake underfoot), and if so whether it did: the cause
/// (rolled once an encounter, at `aggression`: the species', by the world's setting, less what
/// it has learned to fear).
#[allow(clippy::too_many_arguments)]
pub fn provoked(
    sp: &Species,
    p: &Presence,
    d: f64,
    startled: bool,
    young_near: Option<f64>,
    at_kill: bool,
    cornered: bool,
    rut: bool,
    lean: f32,
    aggression: f32,
    roll: f32,
) -> Option<Option<Cause>> {
    let defensive = sp.danger.defensive;
    let rolled = |chance: f32, cause: Cause| Some((roll < chance).then_some(cause));
    if sp.danger.venomous && sp.plan == BodyPlan::Snake && d < 0.9 && p.noise > 0.02 {
        return rolled(0.5 + aggression, Cause::SteppedNear);
    }
    if cornered && d < 5.0 && defensive {
        return rolled(0.6 + aggression, Cause::Cornered);
    }
    if startled && defensive && sp.mass_kg >= 25.0 {
        return rolled((aggression * 2.0).min(0.9), Cause::Surprise);
    }
    if let Some(young) = young_near
        && young < guard_m(sp)
        && defensive
    {
        return rolled((aggression * 3.0).min(0.95), Cause::DefendingYoung);
    }
    if at_kill && d < 25.0 && defensive {
        return rolled((aggression * 3.0).min(0.9), Cause::DefendingKill);
    }
    if rut && d < 20.0 && defensive {
        return rolled((aggression * 1.5).min(0.8), Cause::Rut);
    }
    if sp.danger.predatory && lean > 0.3 && p.vulnerable > 0.4 && !p.by_fire {
        // Weighed again and again while it holds: rarely, the more so the leaner the season and
        // the smaller the person seems.
        return rolled(aggression * lean * p.vulnerable, Cause::Hunger);
    }
    None
}

/// Whether a person faces an animal down: upright, facing it, loud.
pub fn faced_down(p: &Presence, at: DVec3) -> bool {
    let to = glam::DVec2::new(at.x - p.pos.x, at.z - p.pos.z).normalize_or_zero();
    let facing = glam::DVec2::new(p.facing.sin() as f64, p.facing.cos() as f64);
    p.upright && to.dot(facing) > 0.7 && (p.shouting || p.noise > 0.6)
}

/// How lean the season is for a hunter (0 plenty … 1 the hungry end of winter), from the year's
/// fraction in its hemisphere.
pub fn lean(year_frac: f32, southern: bool) -> f32 {
    let f = if southern {
        (year_frac + 0.5).rem_euclid(1.0)
    } else {
        year_frac
    };
    // Leanest at the spring equinox (the end of winter), plentiful by summer's end.
    let from_spring = (f - 0.0).rem_euclid(1.0);
    let d = from_spring.min(1.0 - from_spring);
    (1.0 - d / 0.25).clamp(0.0, 1.0)
}

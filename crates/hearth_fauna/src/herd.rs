//! Kept animals (V2-12, v2 §11.3): wild young caught and raised by hand, tethered, bred, milked
//! and plucked — and what keeping them, and choosing which to breed from, does to their kind
//! over the generations.
//!
//! An animal's makeup ([`Breed`]) is three heritable shares, 0 the wild's and 1 the bred form's
//! at its fullest: how docile it is with people, how woolly its coat, how much milk it gives
//! beyond its young's need. A wild animal's are low and vary a little; a young one's lie between
//! its parents' with the spread their mixing gives, so breeding only from the calmest and the
//! woolliest moves a lineage along a little each generation, as it did in the first herds.
//!
//! How easy it is with people now ([`Kept::tame`]) is learned: a young one handled from birth is
//! tame whatever its makeup, and grows into what its makeup lets it be — a wild-born lamb raised
//! by hand turns wary as it grows, one of a docile line stays calm. What it fears is in its
//! flight: the distance it runs from a person shrinks with the square of what it is not tame.
//!
//! Breeding is abstracted (V2.1 ground rules): a kept female near a kept male of her kind in
//! the rut is in young or not, the likelier the more settled she is; her young are born in the
//! season her kind gives birth.

use glam::DVec3;
use hearth_math::hash::Rng;
use serde::{Deserialize, Serialize};

use crate::live::Stage;
use crate::species::Species;

/// What spread of makeup two parents' young show about the middle of theirs.
pub const SPREAD: f32 = 0.09;
/// How far a tether lets an animal graze from its stake (m).
pub const TETHER_M: f64 = 4.0;
/// How long a mother is in milk after she gives birth (years).
pub const IN_MILK_YEARS: f64 = 0.4;
/// How quickly what it is used to settles to what it can be (a year's share).
const SETTLE_PER_YEAR: f32 = 2.5;
/// A kept male fathers young on females within this distance in the rut (m).
const RUT_M: f64 = 60.0;

/// An animal's heritable makeup: 0 the wild's … 1 the bred form's at its fullest.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Breed {
    /// How docile it is with people: how tame it grows up, how little it runs or fights.
    pub docile: f32,
    /// How woolly its coat: the fleece it grows a year.
    pub wool: f32,
    /// How much milk it gives beyond its young's need.
    pub milk: f32,
}

impl Breed {
    /// A wild animal's makeup: a little variation about nothing bred.
    pub fn wild(rng: &mut Rng) -> Self {
        Self {
            docile: 0.2 * rng.next_f32(),
            wool: 0.1 * rng.next_f32(),
            milk: 0.1 * rng.next_f32(),
        }
    }

    /// A young one's makeup: between its parents', spread either way by their mixing.
    pub fn of_young(dam: &Breed, sire: &Breed, rng: &mut Rng) -> Self {
        let mut one =
            |a: f32, b: f32| (0.5 * (a + b) + SPREAD * rng.normal() as f32).clamp(0.0, 1.0);
        Self {
            docile: one(dam.docile, sire.docile),
            wool: one(dam.wool, sire.wool),
            milk: one(dam.milk, sire.milk),
        }
    }
}

/// A female in young: her mate's makeup and lineage, and when they are due (years).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Carrying {
    pub sire: Breed,
    pub sire_generation: u16,
    pub due: f64,
}

/// What keeping an animal adds to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Kept {
    pub breed: Breed,
    /// How easy it is with people now (0 wild … 1 hand-tame).
    pub tame: f32,
    /// Raised by hand from young.
    pub hand_reared: bool,
    /// Its stake and how far its tether lets it go (m).
    pub tether: Option<(DVec3, f64)>,
    /// Follows its keeper about (led on a halter; a young one raised by hand).
    pub follows: bool,
    /// When it was born (the world's years).
    pub born: f64,
    /// Generations since its line was caught wild (0: caught wild).
    pub generation: u16,
    pub carrying: Option<Carrying>,
    /// When it last gave birth (years).
    pub birth: Option<f64>,
    /// When it was last plucked or shorn, and last milked (years).
    pub plucked: Option<f64>,
    pub milked: Option<f64>,
    /// The year it was last in the rut.
    pub rut_year: Option<i64>,
}

impl Kept {
    /// A wild young one just caught, of an age.
    pub fn caught(breed: Breed, born: f64) -> Self {
        Self {
            breed,
            tame: 0.5,
            hand_reared: true,
            tether: None,
            follows: true,
            born,
            generation: 0,
            carrying: None,
            birth: None,
            plucked: None,
            milked: None,
            rut_year: None,
        }
    }

    /// Born in the keeping, of these parents.
    pub fn born_of(dam: &Kept, c: &Carrying, born: f64, rng: &mut Rng) -> Self {
        Self {
            breed: Breed::of_young(&dam.breed, &c.sire, rng),
            tame: 0.6,
            hand_reared: false,
            tether: None,
            follows: false,
            born,
            generation: dam.generation.max(c.sire_generation) + 1,
            carrying: None,
            birth: None,
            plucked: None,
            milked: None,
            rut_year: None,
        }
    }

    /// How easy with people it settles to at its age: handled young, tame; grown, what its
    /// makeup lets it be (a little more for one raised by hand).
    pub fn settles_to(&self, stage: Stage) -> f32 {
        match stage {
            Stage::Young | Stage::Juvenile => 0.85,
            Stage::Adult => {
                (0.3 + 0.65 * self.breed.docile + if self.hand_reared { 0.05 } else { 0.0 })
                    .min(0.97)
            }
        }
    }

    /// How far it runs from a person, as a share of its wild kind's flight.
    pub fn shyness(&self) -> f32 {
        let wild = 1.0 - self.tame;
        (wild * wild).max(0.005)
    }

    /// In milk (years since giving birth, if it is).
    pub fn in_milk(&self, years: f64) -> Option<f64> {
        self.birth
            .map(|b| years - b)
            .filter(|t| (0.0..IN_MILK_YEARS).contains(t))
    }
}

/// The fleece a kept animal of a species has grown (kg): its kind's range by its makeup, as
/// much of a year's growth as has come since it was last plucked.
pub fn fleece_kg(sp: &Species, k: &Kept, years: f64) -> f32 {
    let Some((wild, bred)) = sp.domestication.as_ref().and_then(|d| d.fleece_kg) else {
        return 0.0;
    };
    let grown = k
        .plucked
        .map_or(1.0, |p| ((years - p) as f32).clamp(0.0, 1.0));
    (wild + (bred - wild) * k.breed.wool) * grown
}

/// The milk a kept mother gives at a milking (kg): her kind's range by her makeup, falling
/// through her months in milk; none from one too wild to stand for it.
pub fn milk_kg(sp: &Species, k: &Kept, years: f64) -> f32 {
    let Some((wild, bred)) = sp.domestication.as_ref().and_then(|d| d.milk_kg_day) else {
        return 0.0;
    };
    let Some(t) = k.in_milk(years) else {
        return 0.0;
    };
    let fall = (1.0 - t / IN_MILK_YEARS) as f32;
    (wild + (bred - wild) * k.breed.milk) * (0.4 + 0.6 * fall)
}

/// The stage of a kept animal of an age (years): young in its first year, grown at its kind's
/// maturity (sooner in a docile line, as bred animals breed younger).
pub fn stage_at(sp: &Species, k: &Kept, age: f64) -> Stage {
    let mature = sp.life.maturity_years as f64 * (1.0 - 0.35 * k.breed.docile as f64);
    if age < 0.75_f64.min(mature) {
        Stage::Young
    } else if age < mature {
        Stage::Juvenile
    } else {
        Stage::Adult
    }
}

/// When young conceived now are born: the next time of year its kind gives birth, at least a
/// season off (years).
pub fn due_after(sp: &Species, years: f64, year_frac: f32) -> f64 {
    let mut wait = (sp.life.birth_frac - year_frac).rem_euclid(1.0) as f64;
    if wait < 0.3 {
        wait += 1.0;
    }
    years + wait
}

/// Whether its kind is in the rut at a time of year (`season` 0 spring … 3 winter): its rut's
/// season, or for a kind without one the season before its young are born.
pub fn in_rut(sp: &Species, season: usize) -> bool {
    match sp.rut {
        Some(s) => s as usize == season,
        None => {
            let birth = ((sp.life.birth_frac * 4.0).floor() as usize).min(3);
            (birth + 3) % 4 == season
        }
    }
}

/// What happened to the kept animals in a turn of the calendar.
#[derive(Debug, Clone, PartialEq)]
pub enum Tiding {
    /// Young were born to a mother of a species, so many.
    Born {
        species: u16,
        mother: u64,
        young: u32,
        at: DVec3,
    },
    /// One died of age.
    Died { species: u16, at: DVec3 },
}

/// A female's chance of being in young after a rut beside a male: her kind's births a year,
/// the less as she is unsettled by her keeping.
pub fn conceives(sp: &Species, k: &Kept) -> f32 {
    sp.life.births_per_year.min(1.0) * (0.35 + 0.65 * k.tame)
}

/// Within this distance of a kept male in the rut.
pub fn near_mate(a: DVec3, b: DVec3) -> bool {
    let d = a - b;
    (d.x * d.x + d.z * d.z).sqrt() < RUT_M
}

/// What it is used to, settled toward what it can be over `dt_years`.
pub fn settle(k: &mut Kept, stage: Stage, dt_years: f64) {
    let to = k.settles_to(stage);
    let step = (SETTLE_PER_YEAR * dt_years as f32).min(1.0);
    k.tame += (to - k.tame) * step;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breeding_from_the_calmest_makes_a_lineage_docile() {
        let mut rng = Rng::new(7);
        let mut flock: Vec<Breed> = (0..12).map(|_| Breed::wild(&mut rng)).collect();
        let start = flock.iter().map(|b| b.docile).sum::<f32>() / flock.len() as f32;
        for _ in 0..10 {
            // The calmest third bred from, each pair a few young.
            flock.sort_by(|a, b| b.docile.total_cmp(&a.docile));
            let parents = &flock[..4];
            let mut young = Vec::new();
            for i in 0..12 {
                young.push(Breed::of_young(
                    &parents[i % 4],
                    &parents[(i + 1) % 4],
                    &mut rng,
                ));
            }
            flock = young;
        }
        let end = flock.iter().map(|b| b.docile).sum::<f32>() / flock.len() as f32;
        assert!(end > start + 0.4, "{start} → {end}");
    }

    #[test]
    fn a_lamb_raised_by_hand_is_tame_and_turns_wary_as_it_grows_unless_its_line_is_docile() {
        let mut rng = Rng::new(3);
        let mut wild = Kept::caught(Breed::wild(&mut rng), 0.0);
        settle(&mut wild, Stage::Young, 1.0);
        assert!(wild.tame > 0.8);
        assert!(wild.shyness() < 0.05);
        settle(&mut wild, Stage::Adult, 2.0);
        assert!(wild.tame < 0.6, "{}", wild.tame);
        assert!(wild.shyness() > 0.15);
        let mut calm = wild.clone();
        calm.breed.docile = 0.9;
        calm.hand_reared = false;
        settle(&mut calm, Stage::Adult, 2.0);
        assert!(calm.shyness() < 0.02, "{}", calm.shyness());
    }
}

//! A player's birth (V2.1 Addendum A; H1, D173): until H3 puts the player into a family of the
//! world, their two parents are drawn from the gene pool of the place they begin — its
//! pigmentation set by its sunlight — and the player's genome is their child's by meiosis, a
//! daughter or a son as asked or as the father's gamete falls. Nothing of the child's looks is
//! chosen.

use hearth_math::hash::Rng;
use serde::{Deserialize, Serialize};

use crate::genome::{Genetics, Genome, Phenotype};

/// A birth: the parents' genomes and what they make of them, and the child's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Birth {
    pub mother: Genome,
    pub father: Genome,
    pub mother_phenotype: Phenotype,
    pub father_phenotype: Phenotype,
    pub genome: Genome,
    pub phenotype: Phenotype,
}

impl Birth {
    /// A child of two parents drawn from a species' pool at a latitude; `female` as asked, or by
    /// chance.
    pub fn draw(
        genetics: &Genetics,
        species: &str,
        latitude_deg: f64,
        female: Option<bool>,
        rng: &mut Rng,
    ) -> Option<Self> {
        let pool = genetics.pool(species)?;
        let sun = genetics.sunlight(latitude_deg);
        let mother = genetics.founder(pool, sun, true, rng);
        let father = genetics.founder(pool, sun, false, rng);
        let genome = genetics.child(&mother, &father, female, rng);
        Some(Self {
            mother_phenotype: genetics.phenotype(&mother, rng),
            father_phenotype: genetics.phenotype(&father, rng),
            phenotype: genetics.phenotype(&genome, rng),
            mother,
            father,
            genome,
        })
    }
}

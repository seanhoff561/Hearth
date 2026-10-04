//! People (`humans/`), V2.1 §2: species profiles — the ranges every person's components take.

use serde::{Deserialize, Serialize};

use super::{Range, entry};
use crate::IdRef;

/// What a species does, of what the people's minds know how to do (v2 §8.2): its capabilities,
/// which H2's routines and methods are gated by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Behavior {
    /// Picks fruit, nuts, pods and seeds, turns over stones for grubs.
    Forage,
    /// Digs roots and tubers with a stick.
    DigTubers,
    /// Cracks hard nuts with a hammerstone on an anvil.
    CrackNuts,
    /// Fishes termites from their mound with a twig.
    FishTermites,
    /// Cuts meat from a carcass and cracks its bones for the marrow.
    Scavenge,
    /// Strikes sharp flakes from a stone.
    KnapFlakes,
    /// Bends branches into a nest in a tree to sleep.
    TreeNest,
    /// Calls out at a hunter.
    AlarmCall,
    /// Faces a threat together: shouting, brandishing sticks, throwing stones.
    MobThreat,
    /// Runs up into the trees.
    FleeToTrees,
    /// Comes to tolerate a calm, patient person.
    Habituate,
}

/// How a species speaks (V2.1 §10.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    /// Calls and gestures only.
    Calls,
    /// A simple, gesture-rich proto-language.
    ProtoLanguage,
    /// Full language.
    Full,
}

/// How much a species teaches (V2.1 §11.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Teaching {
    None,
    /// Tolerating the young watching up close, now and then slowing to let them see.
    Minimal,
    /// Demonstrating and correcting.
    Real,
}

/// What a species can do with fire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FireUse {
    None,
    /// Keeping and carrying a fire taken from a wildfire.
    Keeping,
    /// Making fire at will.
    Making,
}

/// The body plan a species' figures are drawn by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodyPlan {
    /// Long arms, short legs, a funnel chest: a climber that walks upright.
    Australopith,
    /// Tall and long-legged, narrow-hipped.
    Erectus,
    /// Short, broad and barrel-chested, short in the forearm and shin.
    Neanderthal,
    /// A modern human's.
    Modern,
}

/// A species' coat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Coat {
    /// Hair over the body, as an ape's.
    Hair,
    /// Bare skin.
    Bare,
}

/// Which sex leaves its birth group to live in another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Disperser {
    Females,
    Males,
    Both,
}

/// A range by sex.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BySex {
    pub female: Range,
    pub male: Range,
}

impl BySex {
    pub fn of(&self, female: bool) -> Range {
        if female { self.female } else { self.male }
    }
}

/// A species' body: grown sizes by sex, its plan and coat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeciesBody {
    pub height_m: BySex,
    pub mass_kg: BySex,
    pub plan: BodyPlan,
    pub coat: Coat,
    #[serde(default)]
    pub climbs: bool,
}

/// What a species' minds can do (V2.1 §6.1, §10–11).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cognition {
    pub language: Language,
    /// How many steps deep its plans go.
    pub planning_depth: u8,
    /// Symbols: ornament, pigment, marks that mean.
    pub symbolic: bool,
    pub teaching: Teaching,
    pub fire: FireUse,
    /// The highest era of the knowledge graph it can reach.
    pub era_ceiling: u8,
}

/// The stages of a life (V2.1 §7), in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LifeStage {
    /// Carried and nursed.
    Infant,
    /// Weaned, kept close, playing.
    Child,
    /// Ranging with the others, learning their ways.
    Juvenile,
    /// Growing to full size.
    Adolescent,
    Adult,
    /// Slowing, still giving.
    Elder,
}

/// A species' life history (years unless said).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifeParams {
    pub gestation_days: f32,
    pub weaning_years: f32,
    /// Grown: a female's first birth, a male's full size.
    pub maturity_years: f32,
    /// The ages by which most of those who outlive childhood die, and the oldest few.
    pub adult_death_years: Range,
    pub birth_interval_years: f32,
    /// The age (years) each stage of its life begins, in order from infancy (none listed: infant
    /// to weaning, juvenile to maturity, then adult).
    #[serde(default)]
    pub stages: Vec<(LifeStage, f32)>,
    /// How it grows: (age, share of grown height, share of grown mass), from birth to maturity
    /// (none listed: mass from a newborn's to a grown one's, height as mass to the 0.4).
    #[serde(default)]
    pub growth: Vec<(f32, f32, f32)>,
}

impl LifeParams {
    /// The stage of life at an age.
    pub fn stage(&self, age_years: f64) -> LifeStage {
        let a = age_years as f32;
        if self.stages.is_empty() {
            return if a < self.weaning_years {
                LifeStage::Infant
            } else if a < self.maturity_years {
                LifeStage::Juvenile
            } else {
                LifeStage::Adult
            };
        }
        self.stages
            .iter()
            .take_while(|(_, from)| a >= *from)
            .last()
            .map_or(LifeStage::Infant, |(s, _)| *s)
    }

    /// The share of grown height and of grown mass at an age.
    pub fn grown_share(&self, age_years: f64) -> (f32, f32) {
        let a = age_years.max(0.0) as f32;
        if self.growth.is_empty() {
            let t = (a / self.maturity_years.max(1.0)).min(1.0);
            let mass = NEWBORN_MASS + (1.0 - NEWBORN_MASS) * t.powf(1.1);
            return (mass.powf(0.4), mass);
        }
        let first = self.growth[0];
        if a <= first.0 {
            return (first.1, first.2);
        }
        for w in self.growth.windows(2) {
            let ((a0, h0, m0), (a1, h1, m1)) = (w[0], w[1]);
            if a <= a1 {
                let t = if a1 > a0 { (a - a0) / (a1 - a0) } else { 1.0 };
                return (h0 + (h1 - h0) * t, m0 + (m1 - m0) * t);
            }
        }
        (1.0, 1.0)
    }
}

/// A newborn's share of a grown one's mass, where a species does not list its growth.
pub const NEWBORN_MASS: f32 = 0.05;

/// How a species lives together, before culture (V2.1 §8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SocialDefaults {
    pub group_size: Range,
    pub disperses: Disperser,
}

entry! {
    /// A species of person (V2.1 §2): the ranges its persons' components take, what it knows and
    /// does, where and when it lived.
    pub struct Species in "humans/species", schema 1, name name {
        pub name: String,
        #[serde(default)]
        pub scientific: Option<String>,
        pub body: SpeciesBody,
        pub cognition: Cognition,
        pub life: LifeParams,
        pub social: SocialDefaults,
        /// Ecosystems it lives in.
        pub habitat: Vec<IdRef>,
        /// Knowledge nodes its groups practise (what a grown one knows).
        pub knowledge: Vec<IdRef>,
        /// What its groups know besides where the winters are cold (keeping and making fire,
        /// warm clothes, shelter from the wind): Wild Earth's families' (D164).
        #[serde(default)]
        pub cold_knowledge: Vec<IdRef>,
        /// Its population lives in suitable land the world over, whatever the hominins' range
        /// setting (Wild Earth's wandering families, D164).
        #[serde(default)]
        pub worldwide: bool,
        /// What it does, of what the people's minds know how to do.
        pub behaviors: Vec<Behavior>,
        /// Its population in the ecological cells: the animal entry that says what it eats, how
        /// it lives and dies and how far it ranges (its bands are drawn out as persons near the
        /// player).
        #[serde(default)]
        pub population: Option<IdRef>,
        #[serde(default)]
        pub first_appearance_ya: Option<f64>,
        #[serde(default)]
        pub extinction_ya: Option<f64>,
    }
}

/// What a trait or locus is about. Physical groups may differ between gene pools where selection
/// explains it; behavioural ones never do (V2.1 ground rule 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TraitGroup {
    Pigmentation,
    HairForm,
    Body,
    Health,
    Metabolism,
    Temperament,
    Aptitude,
}

impl TraitGroup {
    /// A behavioural group: one set of frequencies for the whole species, in every pool.
    pub fn behavioural(self) -> bool {
        matches!(self, TraitGroup::Temperament | TraitGroup::Aptitude)
    }
}

/// A chromosome's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChromosomeKind {
    Autosome,
    X,
    Y,
}

entry! {
    /// A chromosome (V2.1 §4.1): its genetic length, sex-averaged.
    pub struct Chromosome in "humans/genetics/chromosomes", schema 1, name name {
        pub name: String,
        pub kind: ChromosomeKind,
        pub length_cm: f32,
    }
}

/// How a trait's value is read.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Shows {
    /// A z-score: 0 the species' mean, 1 a standard deviation.
    Z,
    /// A factor about 1, `1 + spread·z`.
    Factor(f32),
    /// Stature: the species profile's height range by sex.
    Stature,
}

/// A named locus's share in a trait.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MajorEffect {
    pub locus: IdRef,
    pub weight: f32,
}

entry! {
    /// A heritable trait and its genetic architecture (V2.1 §4.3).
    pub struct Trait in "humans/genetics/traits", schema 1, name name {
        pub name: String,
        pub group: TraitGroup,
        /// Loci of small effect laid out for it under the settings' seed.
        #[serde(default)]
        pub polygenic: u16,
        /// The share of its variation genes account for in the species' pool (narrow sense).
        pub heritability: f32,
        pub shows: Shows,
        /// The named loci of larger effect it takes, by weight.
        #[serde(default)]
        pub major: Vec<MajorEffect>,
        /// Its raising alleles' frequency where the sun is weakest and strongest (physical traits
        /// only).
        #[serde(default)]
        pub sunlight: Option<(f32, f32)>,
    }
}

/// An allele of a named locus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Allele {
    pub name: String,
    /// Its effect, in the units of the polygenic loci's (about 1 each).
    pub effect: f32,
    /// Its frequency in the species pool.
    pub frequency: f32,
}

/// How a named locus's alleles combine.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum Dominance {
    #[default]
    Additive,
    /// The allele's effect shows in any carrier.
    Dominant(String),
    /// The allele's effect shows only in a homozygote.
    Recessive(String),
}

/// An allele's frequency where the sun is weakest and strongest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sunlight {
    pub allele: String,
    pub weak: f32,
    pub strong: f32,
}

entry! {
    /// A named locus of larger effect (V2.1 §4.1).
    pub struct Locus in "humans/genetics/loci", schema 1, name name {
        pub name: String,
        pub chromosome: IdRef,
        pub position_cm: f32,
        pub group: TraitGroup,
        pub alleles: Vec<Allele>,
        #[serde(default)]
        pub dominance: Dominance,
        #[serde(default)]
        pub sunlight: Option<Sunlight>,
    }
}

/// A pool's frequencies for a named locus (one per allele, in order).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolLocus {
    pub locus: IdRef,
    pub frequencies: Vec<f32>,
}

/// A pool's frequency for a polygenic trait's raising alleles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolTrait {
    pub of: IdRef,
    pub raising: f32,
}

entry! {
    /// A species' gene pool (V2.1 §4.5): its physical loci's frequencies where they differ from
    /// the loci's own. Behavioural loci share one set of frequencies everywhere (ground rule 1).
    pub struct GenePool in "humans/genetics/pools", schema 1, name name {
        pub name: String,
        pub species: IdRef,
        /// Whether a place's sunlight sets its pigmentation.
        #[serde(default)]
        pub sunlight: bool,
        #[serde(default)]
        pub loci: Vec<PoolLocus>,
        #[serde(default)]
        pub traits: Vec<PoolTrait>,
    }
}

entry! {
    /// How genomes are laid out and passed on (V2.1 §4.1–4.2).
    pub struct GeneticsSettings in "humans/genetics/settings", schema 1, name name {
        pub name: String,
        pub architecture_seed: u64,
        pub mutation_rate: f32,
        pub female_map: f32,
        pub male_map: f32,
        pub recessive_loci: u16,
        pub recessive_frequency: Range,
        pub recessive_burden: f32,
        pub hla_loci: u16,
        pub hla_alleles: u16,
        pub sunlight_power: f32,
    }
}

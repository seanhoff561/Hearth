//! Genetics (V2.1 §4; H1). The architecture is laid out once from the content — the chromosomes
//! with their genetic lengths, the named loci of large effect, each trait's loci of small effect
//! under one seed, the rare recessive conditions, the immune region — and every person's
//! [`Genome`] is read by it: two copies of each locus, the mother's and the father's, made by
//! meiosis with crossing over and mutated now and then. A trait's value is its loci's effects
//! summed (with dominance where biology has it), on one scale for every pool (the loci's own
//! frequencies'), plus chance and (from H3) development: its [`Phenotype`]. Gene pools differ only
//! in physical loci (ground rule 1), and those differences show on the common scale.

use std::collections::BTreeMap;

use hearth_content::Content;
use hearth_content::schema::humans::{ChromosomeKind, Dominance, Shows, TraitGroup};
use hearth_math::hash::{Rng, derive_seed, hash2};
use serde::{Deserialize, Serialize};

/// An allele: an index into its locus's alleles.
pub type AlleleIx = u8;
/// No allele: a man's second copy of a locus on the X.
pub const NONE: AlleleIx = u8::MAX;

/// What a locus is for.
#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    /// A named locus of large effect (its content id).
    Named(String),
    /// One of a trait's loci of small effect (the trait's index).
    Polygenic(usize),
    /// A rare recessive condition: allele 1 is the broken one.
    Recessive,
    /// A locus of the immune region.
    Immune,
}

/// How a locus's alleles combine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dom {
    Additive,
    /// The allele's effect shows in any carrier.
    Dominant(AlleleIx),
    /// The allele's effect shows only in a homozygote (or a man's single X).
    Recessive(AlleleIx),
}

/// A locus of the architecture.
#[derive(Debug, Clone)]
pub struct LocusDef {
    pub role: Role,
    /// Its alleles' names (named loci; empty otherwise).
    pub names: Vec<String>,
    pub group: TraitGroup,
    pub chromosome: usize,
    pub position_cm: f32,
    /// Each allele's effect, and its frequency in the reference (species-wide) pool.
    pub effects: Vec<f32>,
    pub frequencies: Vec<f32>,
    pub dom: Dom,
    /// An allele's frequency where the sun is weakest and strongest.
    pub sunlight: Option<(AlleleIx, f32, f32)>,
}

/// A chromosome of the architecture, and its loci in order along it.
#[derive(Debug, Clone)]
pub struct ChromosomeDef {
    pub id: String,
    pub kind: ChromosomeKind,
    pub length_cm: f32,
    pub loci: Vec<usize>,
}

/// A trait of the architecture.
#[derive(Debug, Clone)]
pub struct TraitDef {
    pub id: String,
    pub group: TraitGroup,
    pub heritability: f32,
    pub shows: Shows,
    /// Its loci and their weights.
    pub loci: Vec<(usize, f32)>,
}

/// A trait's scale: the genetic mean and spread of the reference pool (the loci's own
/// frequencies), and chance's spread to meet its heritability there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Calibration {
    pub mean: f64,
    pub sd_genetic: f64,
    pub sd_chance: f64,
}

impl Calibration {
    fn total(&self) -> f64 {
        (self.sd_genetic * self.sd_genetic + self.sd_chance * self.sd_chance)
            .sqrt()
            .max(1e-9)
    }
}

/// A species' gene pool, resolved against the architecture.
#[derive(Debug, Clone)]
pub struct Pool {
    pub id: String,
    pub species: String,
    /// Whether a place's sunlight sets its pigmentation.
    pub sunlight: bool,
    /// Each locus's allele frequencies.
    pub frequencies: Vec<Vec<f32>>,
}

/// The architecture every genome is read by.
#[derive(Debug, Clone)]
pub struct Genetics {
    pub chromosomes: Vec<ChromosomeDef>,
    pub loci: Vec<LocusDef>,
    pub traits: Vec<TraitDef>,
    /// Each trait's scale, the same in every pool.
    pub calibration: Vec<Calibration>,
    pub pools: Vec<Pool>,
    pub mutation_rate: f32,
    pub female_map: f32,
    pub male_map: f32,
    pub recessive_burden: f32,
    pub sunlight_power: f32,
    /// A hash of the layout, recorded in every genome.
    pub architecture: u64,
}

/// A person's genome: the copy of each locus from the mother and from the father.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Genome {
    /// The layout it was made by.
    pub architecture: u64,
    pub female: bool,
    pub maternal: Vec<AlleleIx>,
    pub paternal: Vec<AlleleIx>,
}

/// A trait's value: what genes give, chance, and development — in its species pool's standard
/// deviations.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct TraitValue {
    pub genetic: f32,
    pub chance: f32,
    #[serde(default)]
    pub development: f32,
}

impl TraitValue {
    pub fn z(&self) -> f32 {
        self.genetic + self.chance + self.development
    }
}

/// What a person's genes, development and chance make of them.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Phenotype {
    pub traits: BTreeMap<String, TraitValue>,
    /// The two alleles (by name) at each named locus: what the genotype shows directly (blue
    /// eyes, red hair, digesting milk).
    pub named: BTreeMap<String, [String; 2]>,
    /// The recessive conditions it is homozygous for.
    pub conditions: u16,
    /// The share of the immune region's loci it is heterozygous at.
    pub immune_diversity: f32,
}

impl Phenotype {
    /// How many copies of an allele (by name) it carries at a named locus.
    pub fn copies(&self, locus: &str, allele: &str) -> u8 {
        self.named
            .iter()
            .find(|(k, _)| k.as_str() == locus || k.ends_with(&format!(":{locus}")))
            .map_or(0, |(_, a)| {
                a.iter().filter(|x| x.as_str() == allele).count() as u8
            })
    }

    /// A trait's value in standard deviations (0 where it has none).
    pub fn z(&self, id: &str) -> f32 {
        if let Some(v) = self.traits.get(id) {
            return v.z();
        }
        // A bare id: the trait of that path in any namespace, the game's first.
        let suffix = format!(":{id}");
        self.traits
            .get(&format!("hearth{suffix}"))
            .or_else(|| {
                self.traits
                    .iter()
                    .find(|(k, _)| k.ends_with(&suffix))
                    .map(|(_, v)| v)
            })
            .map_or(0.0, TraitValue::z)
    }

    /// A trait read as a factor about 1.
    pub fn factor(&self, id: &str, spread: f32) -> f32 {
        (1.0 + spread * self.z(id)).max(0.1)
    }
}

/// One draw of a standard normal from a stream.
fn normal(rng: &mut Rng) -> f64 {
    rng.normal()
}

/// A Poisson draw with mean `m`.
fn poisson(rng: &mut Rng, m: f64) -> u32 {
    let limit = (-m).exp();
    let mut k = 0;
    let mut p = rng.next_f64();
    while p > limit && k < 200 {
        k += 1;
        p *= rng.next_f64();
    }
    k
}

impl Genetics {
    /// The architecture of the content's genetics: its chromosomes, named loci, traits, recessive
    /// conditions and immune region, and its species' pools calibrated.
    pub fn from_content(c: &Content) -> Option<Self> {
        let settings = c.genetics.iter().next()?;
        let chromosomes: Vec<ChromosomeDef> = c
            .chromosomes
            .iter()
            .map(|ch| ChromosomeDef {
                id: ch.id.clone(),
                kind: ch.kind,
                length_cm: ch.length_cm,
                loci: Vec::new(),
            })
            .collect();
        let chromo = |id: &str| {
            chromosomes
                .iter()
                .position(|ch| ch.id == id || ch.id.ends_with(&format!(":{id}")))
        };
        let mut loci: Vec<LocusDef> = Vec::new();
        // The named loci.
        for l in c.loci.iter() {
            let Some(ci) = chromo(l.chromosome.as_str()) else {
                continue;
            };
            let names: Vec<&str> = l.alleles.iter().map(|a| a.name.as_str()).collect();
            let ix = |name: &str| names.iter().position(|n| *n == name).map(|i| i as AlleleIx);
            let dom = match &l.dominance {
                Dominance::Additive => Dom::Additive,
                Dominance::Dominant(a) => ix(a).map_or(Dom::Additive, Dom::Dominant),
                Dominance::Recessive(a) => ix(a).map_or(Dom::Additive, Dom::Recessive),
            };
            let sunlight = l
                .sunlight
                .as_ref()
                .and_then(|s| ix(&s.allele).map(|a| (a, s.weak, s.strong)));
            loci.push(LocusDef {
                role: Role::Named(l.id.clone()),
                names: l.alleles.iter().map(|a| a.name.clone()).collect(),
                group: l.group,
                chromosome: ci,
                position_cm: l.position_cm,
                effects: l.alleles.iter().map(|a| a.effect).collect(),
                frequencies: l.alleles.iter().map(|a| a.frequency).collect(),
                dom,
                sunlight,
            });
        }
        // Where loci of small effect fall: along the chromosomes by their lengths (not on Y).
        let total_cm: f32 = chromosomes
            .iter()
            .filter(|ch| ch.kind != ChromosomeKind::Y)
            .map(|ch| ch.length_cm)
            .sum();
        let place = |rng: &mut Rng| -> (usize, f32) {
            let mut at = rng.next_f32() * total_cm;
            for (i, ch) in chromosomes.iter().enumerate() {
                if ch.kind == ChromosomeKind::Y {
                    continue;
                }
                if at < ch.length_cm {
                    return (i, at);
                }
                at -= ch.length_cm;
            }
            (0, 0.0)
        };
        // The traits, and their loci of small effect.
        let mut traits: Vec<TraitDef> = Vec::new();
        for t in c.traits.iter() {
            let ti = traits.len();
            let mut rng = Rng::new(derive_seed(settings.architecture_seed, &t.id));
            let mut mine: Vec<(usize, f32)> = Vec::new();
            for _ in 0..t.polygenic {
                let (ci, at) = place(&mut rng);
                let size = (normal(&mut rng).abs() as f32).max(0.2);
                let p = 0.1 + 0.8 * rng.next_f32();
                let (p, sunlight) = match t.sunlight {
                    Some((weak, strong)) => (p, Some((1, weak, strong))),
                    None => (p, None),
                };
                mine.push((loci.len(), 1.0));
                loci.push(LocusDef {
                    role: Role::Polygenic(ti),
                    names: Vec::new(),
                    group: t.group,
                    chromosome: ci,
                    position_cm: at,
                    effects: vec![0.0, size],
                    frequencies: vec![1.0 - p, p],
                    dom: Dom::Additive,
                    sunlight,
                });
            }
            for m in &t.major {
                let id = m.locus.as_str();
                if let Some(li) = loci.iter().position(|l| {
                    matches!(&l.role, Role::Named(n) if n == id || n.ends_with(&format!(":{id}")))
                }) {
                    mine.push((li, m.weight));
                }
            }
            traits.push(TraitDef {
                id: t.id.clone(),
                group: t.group,
                heritability: t.heritability,
                shows: t.shows,
                loci: mine,
            });
        }
        // The rare recessive conditions.
        let mut rng = Rng::new(derive_seed(settings.architecture_seed, "recessive"));
        for _ in 0..settings.recessive_loci {
            let (ci, at) = place(&mut rng);
            let (lo, hi) = settings.recessive_frequency;
            let q = lo + (hi - lo) * rng.next_f32();
            loci.push(LocusDef {
                role: Role::Recessive,
                names: Vec::new(),
                group: TraitGroup::Health,
                chromosome: ci,
                position_cm: at,
                effects: vec![0.0, 1.0],
                frequencies: vec![1.0 - q, q],
                dom: Dom::Recessive(1),
                sunlight: None,
            });
        }
        // The immune region: a cluster on chromosome 6 (the HLA's place, 6p21).
        if let Some(c6) = chromo("chr6") {
            let mut rng = Rng::new(derive_seed(settings.architecture_seed, "immune"));
            let n = settings.hla_alleles.clamp(2, 200) as usize;
            for k in 0..settings.hla_loci {
                let weights: Vec<f32> = (0..n).map(|_| 0.5 + rng.next_f32()).collect();
                let sum: f32 = weights.iter().sum();
                loci.push(LocusDef {
                    role: Role::Immune,
                    names: Vec::new(),
                    group: TraitGroup::Health,
                    chromosome: c6,
                    position_cm: 45.0 + 0.4 * k as f32,
                    effects: vec![0.0; n],
                    frequencies: weights.iter().map(|w| w / sum).collect(),
                    dom: Dom::Additive,
                    sunlight: None,
                });
            }
        }
        let mut chromosomes = chromosomes;
        for (li, l) in loci.iter().enumerate() {
            chromosomes[l.chromosome].loci.push(li);
        }
        for ch in chromosomes.iter_mut() {
            ch.loci
                .sort_by(|a, b| loci[*a].position_cm.total_cmp(&loci[*b].position_cm));
        }
        let architecture = loci.iter().enumerate().fold(
            derive_seed(settings.architecture_seed, "genome") ^ loci.len() as u64,
            |h, (i, l)| {
                hash2(
                    h ^ ((l.chromosome as u64) << 32) ^ u64::from(l.position_cm.to_bits()),
                    (i as u64) ^ ((l.effects.len() as u64) << 40),
                )
            },
        );
        let mut g = Genetics {
            chromosomes,
            loci,
            traits,
            calibration: Vec::new(),
            pools: Vec::new(),
            mutation_rate: settings.mutation_rate,
            female_map: settings.female_map,
            male_map: settings.male_map,
            recessive_burden: settings.recessive_burden,
            sunlight_power: settings.sunlight_power,
            architecture,
        };
        // The scale: the loci's own frequencies.
        let reference: Vec<Vec<f32>> = g.loci.iter().map(|l| l.frequencies.clone()).collect();
        g.calibration = g.calibrate(&reference);
        // The pools: the loci's own frequencies, changed where a pool says (physical loci only).
        for p in c.gene_pools.iter() {
            let mut freqs: Vec<Vec<f32>> = g.loci.iter().map(|l| l.frequencies.clone()).collect();
            for pl in &p.loci {
                let id = pl.locus.as_str();
                if let Some(li) = g.loci.iter().position(|l| {
                    matches!(&l.role, Role::Named(n) if n == id || n.ends_with(&format!(":{id}")))
                }) && !g.loci[li].group.behavioural()
                    && pl.frequencies.len() == freqs[li].len()
                {
                    freqs[li] = pl.frequencies.clone();
                }
            }
            for pt in &p.traits {
                let id = pt.of.as_str();
                let Some(ti) = g
                    .traits
                    .iter()
                    .position(|t| t.id == id || t.id.ends_with(&format!(":{id}")))
                else {
                    continue;
                };
                if g.traits[ti].group.behavioural() {
                    continue;
                }
                for (li, l) in g.loci.iter().enumerate() {
                    if l.role == Role::Polygenic(ti) {
                        freqs[li] = vec![1.0 - pt.raising, pt.raising];
                    }
                }
            }
            g.pools.push(Pool {
                id: p.id.clone(),
                species: p.species.to_string(),
                sunlight: p.sunlight,
                frequencies: freqs,
            });
        }
        Some(g)
    }

    /// Each trait's scale under a set of frequencies: its loci's genetic mean and variance at
    /// Hardy–Weinberg proportions, and chance's spread to meet its heritability.
    fn calibrate(&self, freqs: &[Vec<f32>]) -> Vec<Calibration> {
        self.traits
            .iter()
            .map(|t| {
                let (mut mean, mut var) = (0.0f64, 0.0f64);
                for &(li, w) in &t.loci {
                    let (m, v) = locus_moments(&self.loci[li], &freqs[li]);
                    mean += w as f64 * m;
                    var += (w as f64) * (w as f64) * v;
                }
                let h2 = (t.heritability as f64).clamp(0.01, 0.999);
                Calibration {
                    mean,
                    sd_genetic: var.sqrt(),
                    sd_chance: (var * (1.0 - h2) / h2).sqrt(),
                }
            })
            .collect()
    }

    /// The pool of a species.
    pub fn pool(&self, species: &str) -> Option<&Pool> {
        self.pools.iter().find(|p| {
            p.species == species
                || p.species.ends_with(&format!(":{species}"))
                || species.ends_with(&format!(":{}", p.species))
        })
    }

    /// A place's sunlight, 0 at the poles to 1 at the equator, from its latitude.
    pub fn sunlight(&self, latitude_deg: f64) -> f32 {
        (latitude_deg.to_radians().cos().max(0.0) as f32).powf(self.sunlight_power)
    }

    /// A locus's allele frequencies in a pool at a place's sunlight.
    fn frequencies<'a>(&self, pool: &'a Pool, li: usize, sun: f32) -> std::borrow::Cow<'a, [f32]> {
        let l = &self.loci[li];
        match (pool.sunlight, l.sunlight) {
            (true, Some((a, weak, strong))) => {
                let f = weak + (strong - weak) * sun.clamp(0.0, 1.0);
                let n = l.effects.len();
                let mut v = vec![0.0; n];
                if n == 2 {
                    v[a as usize] = f;
                    v[1 - a as usize] = 1.0 - f;
                } else {
                    let rest = (1.0 - f) / (n - 1).max(1) as f32;
                    for (k, x) in v.iter_mut().enumerate() {
                        *x = if k == a as usize { f } else { rest };
                    }
                }
                std::borrow::Cow::Owned(v)
            }
            _ => std::borrow::Cow::Borrowed(&pool.frequencies[li]),
        }
    }

    /// An allele drawn from frequencies.
    fn draw(freqs: &[f32], rng: &mut Rng) -> AlleleIx {
        let mut u = rng.next_f32() * freqs.iter().sum::<f32>().max(1e-9);
        for (k, f) in freqs.iter().enumerate() {
            if u < *f {
                return k as AlleleIx;
            }
            u -= f;
        }
        (freqs.len().saturating_sub(1)) as AlleleIx
    }

    /// Whether a locus lies on the X.
    fn on_x(&self, li: usize) -> bool {
        self.chromosomes[self.loci[li].chromosome].kind == ChromosomeKind::X
    }

    /// The drift dimension of a locus whose frequencies differ by place (H8): a named locus
    /// its own, a polygenic trait's loci their trait's, so that a trait's mean drifts as one.
    fn drift_dim(&self, li: usize) -> usize {
        let h = match &self.loci[li].role {
            Role::Named(id) => id.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
                (h ^ b as u64).wrapping_mul(0x100_0000_01b3)
            }),
            Role::Polygenic(t) => (*t as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15),
            Role::Recessive | Role::Immune => 0,
        };
        (h % crate::history::POOL_DIMS as u64) as usize
    }

    /// A locus's allele frequencies in a people of deep time: its pool's at the sunlight it is
    /// adapted to, the place-shifted allele moved by its people's drift (a logit offset).
    fn frequencies_in<'a>(
        &self,
        pool: &'a Pool,
        li: usize,
        sun: f32,
        drift: &[f32],
    ) -> std::borrow::Cow<'a, [f32]> {
        let f = self.frequencies(pool, li, sun);
        let Some((a, _, _)) = self.loci[li].sunlight else {
            return f;
        };
        if drift.is_empty() || !pool.sunlight {
            return f;
        }
        let d = drift[self.drift_dim(li) % drift.len()];
        let mut v = f.into_owned();
        let a = a as usize;
        let p = v[a].clamp(1e-4, 1.0 - 1e-4);
        let q = 1.0 / (1.0 + (-((p / (1.0 - p)).ln() + d)).exp());
        let rest: f32 = v
            .iter()
            .enumerate()
            .filter(|(k, _)| *k != a)
            .map(|(_, x)| x)
            .sum();
        let others = (v.len() - 1).max(1) as f32;
        for (k, x) in v.iter_mut().enumerate() {
            *x = if k == a {
                q
            } else if rest > 0.0 {
                *x * (1.0 - q) / rest
            } else {
                (1.0 - q) / others
            };
        }
        std::borrow::Cow::Owned(v)
    }

    /// A founder: every locus drawn from a pool's frequencies at a place's sunlight.
    pub fn founder(&self, pool: &Pool, sun: f32, female: bool, rng: &mut Rng) -> Genome {
        self.founder_in(pool, sun, &[], female, rng)
    }

    /// A founder of a people of deep time (H8): every locus drawn from a pool's frequencies at
    /// the sunlight its people are adapted to, moved by their drift.
    pub fn founder_in(
        &self,
        pool: &Pool,
        sun: f32,
        drift: &[f32],
        female: bool,
        rng: &mut Rng,
    ) -> Genome {
        let n = self.loci.len();
        let mut maternal = Vec::with_capacity(n);
        let mut paternal = Vec::with_capacity(n);
        for li in 0..n {
            let f = self.frequencies_in(pool, li, sun, drift);
            maternal.push(Self::draw(&f, rng));
            paternal.push(if !female && self.on_x(li) {
                NONE
            } else {
                Self::draw(&f, rng)
            });
        }
        Genome {
            architecture: self.architecture,
            female,
            maternal,
            paternal,
        }
    }

    /// A child of two: a gamete from each by meiosis, mutated; a daughter or a son as asked, or
    /// by the father's gamete.
    pub fn child(
        &self,
        mother: &Genome,
        father: &Genome,
        sex: Option<bool>,
        rng: &mut Rng,
    ) -> Genome {
        let female = sex.unwrap_or_else(|| rng.next_f32() < 0.5);
        let maternal = self.gamete(mother, true, rng);
        let paternal = if female {
            self.gamete(father, true, rng)
        } else {
            // The Y: no loci; the autosomes as ever.
            let mut g = self.gamete(father, false, rng);
            for (li, a) in g.iter_mut().enumerate() {
                if self.on_x(li) {
                    *a = NONE;
                }
            }
            g
        };
        Genome {
            architecture: self.architecture,
            female,
            maternal,
            paternal,
        }
    }

    /// A gamete of a parent: each chromosome's two copies crossed over at chiasmata (at least
    /// one per pair, the rest by its length in Morgans for the parent's sex), the gamete taking
    /// each chiasma's crossing with a chance of a half; mutated. A father's X goes whole (it has
    /// no pair to cross with), when `x` asks for it.
    fn gamete(&self, parent: &Genome, x: bool, rng: &mut Rng) -> Vec<AlleleIx> {
        let mut out = vec![NONE; self.loci.len()];
        let map = if parent.female {
            self.female_map
        } else {
            self.male_map
        };
        for ch in &self.chromosomes {
            if ch.kind == ChromosomeKind::Y {
                continue;
            }
            let is_x = ch.kind == ChromosomeKind::X;
            if is_x && !parent.female {
                if x {
                    for &li in &ch.loci {
                        out[li] = self.mutate(li, parent.maternal[li], rng);
                    }
                }
                continue;
            }
            let morgans = (ch.length_cm * map / 100.0) as f64;
            let chiasmata = poisson(rng, 2.0 * morgans).max(1);
            let mut crossings: Vec<f32> = Vec::new();
            for _ in 0..chiasmata {
                let at = rng.next_f32() * ch.length_cm;
                if rng.next_f32() < 0.5 {
                    crossings.push(at);
                }
            }
            crossings.sort_by(f32::total_cmp);
            let mut from_mother = rng.next_f32() < 0.5;
            let mut next = 0;
            for &li in &ch.loci {
                let at = self.loci[li].position_cm;
                while next < crossings.len() && crossings[next] < at {
                    from_mother = !from_mother;
                    next += 1;
                }
                let a = if from_mother {
                    parent.maternal[li]
                } else {
                    parent.paternal[li]
                };
                out[li] = self.mutate(li, a, rng);
            }
        }
        out
    }

    /// An allele passed on, now and then mutated: to the other allele of a pair, else to another
    /// at random.
    fn mutate(&self, li: usize, a: AlleleIx, rng: &mut Rng) -> AlleleIx {
        if a == NONE || rng.next_f32() >= self.mutation_rate {
            return a;
        }
        let n = self.loci[li].effects.len() as u32;
        if n <= 1 {
            return a;
        }
        if n == 2 {
            return 1 - a;
        }
        let other = rng.below(n - 1) as AlleleIx;
        if other >= a { other + 1 } else { other }
    }

    /// A locus's genetic value for one with these two copies (a man's X counts twice: dosage
    /// compensation keeps the scale).
    fn value(&self, li: usize, m: AlleleIx, p: AlleleIx) -> f64 {
        let l = &self.loci[li];
        let e = |a: AlleleIx| l.effects.get(a as usize).copied().unwrap_or(0.0) as f64;
        let (a, b) = if p == NONE { (m, m) } else { (m, p) };
        match l.dom {
            Dom::Additive => e(a) + e(b),
            Dom::Dominant(d) => {
                if a == d || b == d {
                    e(d)
                } else {
                    0.0
                }
            }
            Dom::Recessive(r) => {
                if a == r && b == r {
                    e(r)
                } else {
                    0.0
                }
            }
        }
    }

    /// What a genome makes of one: each trait's genetic value and a draw of chance, in the
    /// reference pool's standard deviations; its recessive conditions and immune diversity.
    pub fn phenotype(&self, genome: &Genome, rng: &mut Rng) -> Phenotype {
        let mut traits = BTreeMap::new();
        for (ti, t) in self.traits.iter().enumerate() {
            let g: f64 = t
                .loci
                .iter()
                .map(|&(li, w)| w as f64 * self.value(li, genome.maternal[li], genome.paternal[li]))
                .sum();
            let c = self.calibration[ti];
            let total = c.total();
            traits.insert(
                t.id.clone(),
                TraitValue {
                    genetic: ((g - c.mean) / total) as f32,
                    chance: (normal(rng) * c.sd_chance / total) as f32,
                    development: 0.0,
                },
            );
        }
        let mut conditions = 0;
        let (mut hla, mut het) = (0, 0);
        let mut named = BTreeMap::new();
        for (li, l) in self.loci.iter().enumerate() {
            let (m, p) = (genome.maternal[li], genome.paternal[li]);
            if let Role::Named(id) = &l.role {
                let name = |a: AlleleIx| {
                    let a = if a == NONE { m } else { a };
                    l.names.get(a as usize).cloned().unwrap_or_default()
                };
                named.insert(id.clone(), [name(m), name(p)]);
            }
            match l.role {
                Role::Recessive if m == 1 && (p == 1 || p == NONE) => conditions += 1,
                Role::Immune => {
                    hla += 1;
                    if m != p && p != NONE {
                        het += 1;
                    }
                }
                _ => {}
            }
        }
        Phenotype {
            traits,
            named,
            conditions,
            immune_diversity: if hla > 0 {
                het as f32 / hla as f32
            } else {
                0.0
            },
        }
    }
}

/// A locus's genetic mean and variance at Hardy–Weinberg proportions under some frequencies.
fn locus_moments(l: &LocusDef, freqs: &[f32]) -> (f64, f64) {
    let n = l.effects.len();
    let p = |k: usize| freqs.get(k).copied().unwrap_or(0.0) as f64;
    let e = |k: usize| l.effects[k] as f64;
    match l.dom {
        Dom::Additive => {
            let m1: f64 = (0..n).map(|k| p(k) * e(k)).sum();
            let m2: f64 = (0..n).map(|k| p(k) * e(k) * e(k)).sum();
            (2.0 * m1, 2.0 * (m2 - m1 * m1))
        }
        Dom::Dominant(d) => {
            let q = p(d as usize);
            let carrier = 1.0 - (1.0 - q) * (1.0 - q);
            let v = e(d as usize);
            (carrier * v, carrier * (1.0 - carrier) * v * v)
        }
        Dom::Recessive(r) => {
            let q = p(r as usize);
            let homo = q * q;
            let v = e(r as usize);
            (homo * v, homo * (1.0 - homo) * v * v)
        }
    }
}

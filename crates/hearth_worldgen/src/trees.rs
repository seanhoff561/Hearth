//! The trees of the world: each species' growth templates (`hearth_flora`) and the block
//! states their parts are drawn with.

use hearth_content::Content;
use hearth_content::schema::flora::{ClimateEnvelope, TreeForm};
use hearth_flora::{Part, Templates};
use hearth_world::{BlockRegistry, BlockStateId};

use crate::planet::climate::ClimateClass;
use crate::region::biome::Biome;

/// The blocks one species is drawn with.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeciesBlocks {
    /// The log along x, y and z.
    pub log: [BlockStateId; 3],
    /// Limbs by thickness (2, 4, 8, 12 px) × 64 + the faces they join.
    pub branch: Vec<BlockStateId>,
    pub leaves: BlockStateId,
}

fn thickness_index(px: u8) -> usize {
    match px {
        0..=2 => 0,
        3..=4 => 1,
        5..=8 => 2,
        _ => 3,
    }
}

impl SpeciesBlocks {
    pub fn new(reg: &BlockRegistry, form: &TreeForm) -> Result<Self, String> {
        let state = |s: String| reg.parse_state(&s).map_err(|e| format!("{s}: {e}"));
        let log_id = form.log.as_str();
        let log = [
            state(format!("{log_id}[axis=x]"))?,
            state(format!("{log_id}[axis=y]"))?,
            state(format!("{log_id}[axis=z]"))?,
        ];
        let mut branch = Vec::with_capacity(256);
        for px in [2, 4, 8, 12] {
            for joins in 0u8..64 {
                let b = |bit: u8| if joins & bit != 0 { "true" } else { "false" };
                branch.push(state(format!(
                    "{}[thickness={px},down={},up={},north={},south={},west={},east={}]",
                    form.branch.as_str(),
                    b(hearth_flora::template::DOWN),
                    b(hearth_flora::template::UP),
                    b(hearth_flora::template::NORTH),
                    b(hearth_flora::template::SOUTH),
                    b(hearth_flora::template::WEST),
                    b(hearth_flora::template::EAST),
                ))?);
            }
        }
        let leaves = state(form.leaves.as_str().to_owned())?;
        Ok(Self {
            log,
            branch,
            leaves,
        })
    }

    /// The block a part of the tree is drawn with.
    #[inline]
    pub fn state(&self, p: Part) -> BlockStateId {
        match p {
            Part::Log { axis } => self.log[(axis as usize).min(2)],
            Part::Branch { thickness, joins } => {
                self.branch[thickness_index(thickness) * 64 + (joins & 63) as usize]
            }
            Part::Leaves => self.leaves,
        }
    }
}

/// Where a species grows: its climate, light and ground.
#[derive(Debug, Clone, PartialEq)]
pub struct Niche {
    pub climate: ClimateEnvelope,
    /// 0 needs full sun (a pioneer) … 1 grows in deep shade.
    pub shade_tolerance: f32,
    /// Drainage it likes: 0 waterlogged … 1 dry.
    pub drainage: Option<(f32, f32)>,
    pub conifer: bool,
}

/// The climate of a place, as the trees read it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaceClimate {
    pub mean_c: f32,
    pub warm_c: f32,
    pub cold_c: f32,
    pub precip_mm: f32,
    pub class: ClimateClass,
    pub biome: Biome,
    /// Wet ground (floodplains, wetlands).
    pub wet: bool,
}

/// Every tree species of the content: its templates, its blocks and its niche (same order).
#[derive(Debug)]
pub struct Forest {
    pub templates: Templates,
    pub blocks: Vec<SpeciesBlocks>,
    pub niches: Vec<Niche>,
}

/// The coarse climate class a Köppen code belongs to.
fn class_of(koppen: &str) -> Option<ClimateClass> {
    Some(match koppen {
        "Af" | "Am" => ClimateClass::TropicalRainforest,
        "Aw" | "As" => ClimateClass::TropicalSavanna,
        "BWh" => ClimateClass::HotDesert,
        "BSh" => ClimateClass::HotSteppe,
        "BWk" => ClimateClass::ColdDesert,
        "BSk" => ClimateClass::ColdSteppe,
        "Csa" | "Csb" | "Csc" => ClimateClass::Mediterranean,
        "Cfa" | "Cwa" => ClimateClass::HumidSubtropical,
        "Cfb" | "Cfc" | "Cwb" => ClimateClass::Oceanic,
        "Dfa" | "Dfb" | "Dwa" | "Dwb" | "Dsa" | "Dsb" => ClimateClass::HumidContinental,
        "Dfc" | "Dfd" | "Dwc" | "Dwd" | "Dsc" => ClimateClass::Subarctic,
        "ET" => ClimateClass::Tundra,
        "EF" => ClimateClass::IceCap,
        _ => return None,
    })
}

/// 1 inside the range, falling off over `soft` outside it.
fn fit(v: f32, lo: f32, hi: f32, soft: f32) -> f32 {
    let d = if v < lo {
        lo - v
    } else if v > hi {
        v - hi
    } else {
        0.0
    };
    (-(d / soft.max(1e-3)).powi(2)).exp()
}

impl Niche {
    /// How well a place suits it, 0–1.
    pub fn suits(&self, c: &PlaceClimate) -> f32 {
        let e = &self.climate;
        let mut s = 1.0;
        if let Some(r) = &e.temp_c {
            s *= fit(c.mean_c, r.0, r.1, 2.5);
        }
        if let Some(m) = e.min_coldest_month_c {
            s *= fit(c.cold_c, m, f32::MAX, 4.0);
        }
        if let Some(m) = e.min_warmest_month_c {
            s *= fit(c.warm_c, m, f32::MAX, 2.0);
        }
        if let Some(r) = &e.precip_mm {
            s *= fit(c.precip_mm, r.0, r.1, 0.3 * (r.1 - r.0).max(100.0));
        }
        if !e.koppen.is_empty() && !e.koppen.iter().any(|k| class_of(k) == Some(c.class)) {
            s *= 0.35;
        }
        // Wet ground for the trees of wet ground, and not for the rest.
        if let Some((lo, _)) = self.drainage {
            let wet_lover = lo < 0.1;
            s *= match (c.wet, wet_lover) {
                (true, true) => 2.5,
                (true, false) => 0.5,
                (false, true) => 0.25,
                (false, false) => 1.0,
            };
        }
        s
    }

    /// How much the biome's own trees favour it.
    pub fn affinity(&self, id: &str, b: Biome) -> f32 {
        let is = |names: &[&str]| names.iter().any(|n| id.ends_with(n));
        match b {
            Biome::BorealForest | Biome::SnowyTaiga => {
                if self.conifer {
                    3.0
                } else if is(&["birch", "aspen"]) {
                    1.5
                } else {
                    0.3
                }
            }
            Biome::BirchForest => {
                if is(&["birch"]) {
                    5.0
                } else {
                    0.6
                }
            }
            Biome::TemperateRainforest | Biome::MontaneForest => {
                if self.conifer {
                    2.5
                } else {
                    0.8
                }
            }
            Biome::Wetland => {
                if is(&["alder", "willow", "osier"]) {
                    4.0
                } else {
                    0.5
                }
            }
            Biome::MixedForest => 1.0,
            _ => {
                if self.conifer {
                    0.6
                } else {
                    1.2
                }
            }
        }
    }
}

impl Forest {
    /// The species whose blocks the registry has (others are left out, with a warning).
    pub fn new(reg: &BlockRegistry, content: &Content) -> Self {
        let mut species = Vec::new();
        let mut blocks = Vec::new();
        let mut niches = Vec::new();
        for p in content.plants.iter() {
            let (Some(sp), Some(form)) = (hearth_flora::Species::from_plant(p), p.tree.as_ref())
            else {
                continue;
            };
            match SpeciesBlocks::new(reg, form) {
                Ok(b) => {
                    niches.push(Niche {
                        climate: p.climate.clone(),
                        shade_tolerance: p.shade_tolerance,
                        drainage: p.soil.drainage,
                        conifer: !p.deciduous
                            && form.leaf != hearth_content::schema::flora::LeafKind::Broad,
                    });
                    species.push(sp);
                    blocks.push(b);
                }
                Err(e) => log::warn!("tree `{}` is left out: {e}", p.id),
            }
        }
        Self {
            templates: Templates::new(species),
            blocks,
            niches,
        }
    }

    /// The species of a place, drawn by `roll` (0–1) from those that fit it, weighted by how
    /// well; `shade` (0–1) favours the shade-tolerant (the understory). None where no species
    /// fits (the tropics, deserts and tundra wait for their species).
    pub fn choose(&self, c: &PlaceClimate, shade: f32, roll: f32) -> Option<usize> {
        let mut weights: smallvec::SmallVec<[f32; 32]> = smallvec::SmallVec::new();
        let mut best = 0.0f32;
        for (i, n) in self.niches.iter().enumerate() {
            let s = n.suits(c);
            best = best.max(s);
            let light = 1.0 - shade + shade * (0.2 + 1.6 * n.shade_tolerance);
            let id = &self.templates.species[i].id;
            weights.push(s * s * n.affinity(id, c.biome) * light);
        }
        if best < 0.25 {
            return None;
        }
        let total: f32 = weights.iter().sum();
        if total <= 0.0 {
            return None;
        }
        let mut r = roll.clamp(0.0, 0.9999) * total;
        for (i, w) in weights.iter().enumerate() {
            if r < *w {
                return Some(i);
            }
            r -= w;
        }
        weights.iter().rposition(|w| *w > 0.0)
    }
}

//! The trees of the world: each species' growth templates (`hearth_flora`) and the block
//! states their parts are drawn with.

use hearth_content::Content;
use hearth_content::schema::flora::{
    ClimateEnvelope, Dispersal, Ground, GrowthForm, Plant, TreeForm, Understory,
};
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
        let leaves = reg
            .parse_state(form.leaves.as_str())
            .map_err(|e| format!("{}: {e}", form.leaves.as_str()))?;
        Self::named(reg, form.log.as_str(), form.branch.as_str(), leaves)
    }

    /// The blocks of a log and a branch block, with `leaves` for the foliage.
    pub fn named(
        reg: &BlockRegistry,
        log_id: &str,
        branch_id: &str,
        leaves: BlockStateId,
    ) -> Result<Self, String> {
        let state = |s: String| reg.parse_state(&s).map_err(|e| format!("{s}: {e}"));
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
                    branch_id,
                    b(hearth_flora::template::DOWN),
                    b(hearth_flora::template::UP),
                    b(hearth_flora::template::NORTH),
                    b(hearth_flora::template::SOUTH),
                    b(hearth_flora::template::WEST),
                    b(hearth_flora::template::EAST),
                ))?);
            }
        }
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
    /// How quickly it takes opened ground, by how its seed travels and how fast it grows
    /// (about 1; the wind-sown, fast-growing pioneers 2 and more, heavy nuts a half).
    pub colonizes: f32,
    /// How fast it grows against the others (0.5 slow … 2 fast): in a gap of the old forest the
    /// quicker of the shade-tolerant reach the light first.
    pub pace: f32,
}

impl Niche {
    /// A plant's niche.
    pub fn of(p: &Plant, conifer: bool) -> Self {
        let travels = match p.dispersal {
            Dispersal::Wind | Dispersal::Spores => 1.5,
            Dispersal::Water => 1.0,
            Dispersal::Animal => 0.6,
            Dispersal::Explosive => 0.5,
            Dispersal::Gravity => 0.3,
        };
        let pace = (p.growth_m_per_year.unwrap_or(0.4) / 0.5).clamp(0.5, 2.0);
        Self {
            climate: p.climate.clone(),
            shade_tolerance: p.shade_tolerance,
            drainage: p.soil.drainage,
            conifer,
            colonizes: travels * pace,
            pace,
        }
    }
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

/// A plant of the understory as the generator places it.
#[derive(Debug, Clone)]
pub struct UnderPlant {
    pub id: String,
    /// Herb, shrub, fern or fungus: on cleared ground the herbs come at once, the shrubs from
    /// the second year.
    pub form: GrowthForm,
    pub niche: Niche,
    pub understory: Understory,
    /// Its block, and the upper half's for plants two blocks tall.
    pub lower: BlockStateId,
    pub upper: Option<BlockStateId>,
}

/// The ground under a place, as the understory reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaceGround {
    pub wet: bool,
    pub rich: bool,
    pub acid: bool,
    pub disturbed: bool,
    /// Shrubs have had time to grow (not in the first year after the ground was cleared).
    pub shrubs: bool,
}

impl Default for PlaceGround {
    fn default() -> Self {
        Self {
            wet: false,
            rich: false,
            acid: false,
            disturbed: false,
            shrubs: true,
        }
    }
}

impl PlaceGround {
    fn has(&self, g: Ground) -> bool {
        match g {
            Ground::Wet => self.wet,
            Ground::Rich => self.rich,
            Ground::Acid => self.acid,
            Ground::Disturbed => self.disturbed,
            // Lime is not yet told from the bedrock: neither for nor against.
            Ground::Lime => true,
        }
    }
}

/// Every tree species of the content: its templates, its blocks and its niche (same order);
/// and the plants of the understory.
#[derive(Debug)]
pub struct Forest {
    pub templates: Templates,
    pub blocks: Vec<SpeciesBlocks>,
    pub niches: Vec<Niche>,
    pub understory: Vec<UnderPlant>,
    /// The blocks trees killed by fire stand in (charred trunk and limbs), if the content has
    /// them.
    pub charred: Option<SpeciesBlocks>,
    /// The greatest height (m) any species reaches.
    pub tallest_m: f32,
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
                    niches.push(Niche::of(
                        p,
                        !p.deciduous && form.leaf != hearth_content::schema::flora::LeafKind::Broad,
                    ));
                    species.push(sp);
                    blocks.push(b);
                }
                Err(e) => log::warn!("tree `{}` is left out: {e}", p.id),
            }
        }
        let mut understory = Vec::new();
        for p in content.plants.iter() {
            let Some(u) = &p.understory else {
                continue;
            };
            let block = u.block.as_str();
            let (lower, upper) = match (
                reg.parse_state(&format!("{block}[half=lower]")),
                reg.parse_state(&format!("{block}[half=upper]")),
            ) {
                (Ok(l), Ok(h)) => (l, Some(h)),
                _ => match reg.parse_state(block) {
                    Ok(s) => (s, None),
                    Err(e) => {
                        log::warn!("plant `{}` is left out: {e}", p.id);
                        continue;
                    }
                },
            };
            understory.push(UnderPlant {
                id: p.id.clone(),
                form: p.form,
                niche: Niche::of(p, false),
                understory: u.clone(),
                lower,
                upper,
            });
        }
        let charred = match SpeciesBlocks::named(
            reg,
            "hearth:charred_log",
            "hearth:charred_branch",
            BlockStateId::AIR,
        ) {
            Ok(b) => Some(b),
            Err(e) => {
                if !species.is_empty() {
                    log::warn!("burned trees stand unburned: {e}");
                }
                None
            }
        };
        let tallest_m = species.iter().map(|s| s.max_height_m).fold(0.0, f32::max);
        Self {
            templates: Templates::new(species),
            blocks,
            niches,
            understory,
            charred,
            tallest_m,
        }
    }

    /// A plant of the understory for a column, or none: each species as likely as its climate,
    /// the light under the canopy (0 deep shade … 1 open), the ground and its abundance say,
    /// in patches of its own size where it grows in patches. `roll` and `patch_roll` (by
    /// species and patch) are the column's and the patches' draws.
    pub fn choose_under(
        &self,
        c: &PlaceClimate,
        ground: &PlaceGround,
        light: f32,
        roll: f32,
        patch_roll: impl Fn(usize, f32) -> f32,
    ) -> Option<usize> {
        let mut odds: smallvec::SmallVec<[f32; 32]> = smallvec::SmallVec::new();
        for (i, p) in self.understory.iter().enumerate() {
            let u = &p.understory;
            if !ground.shrubs && matches!(p.form, GrowthForm::Shrub | GrowthForm::Vine) {
                odds.push(0.0);
                continue;
            }
            let climate = p.niche.suits(c).min(1.0);
            let lit = fit(light, u.light.0, u.light.1, 0.15);
            let soil = if u.ground.is_empty() || u.ground.iter().any(|g| ground.has(*g)) {
                1.0
            } else {
                0.25
            };
            let fit = climate * lit * soil;
            let p = if fit < 0.15 {
                0.0
            } else if u.patch_m > 0.0 {
                // A patch is there or not; inside one, the plant is thick on the ground.
                if patch_roll(i, u.patch_m) < u.abundance * fit {
                    0.35 * fit
                } else {
                    0.0
                }
            } else {
                0.02 * u.abundance * fit
            };
            odds.push(p);
        }
        let total: f32 = odds.iter().sum::<f32>().min(0.7);
        if roll >= total {
            return None;
        }
        let sum: f32 = odds.iter().sum();
        let mut r = roll / total * sum;
        for (i, p) in odds.iter().enumerate() {
            if r < *p {
                return Some(i);
            }
            r -= p;
        }
        None
    }

    /// The species of a place, drawn by `roll` (0–1) from those that fit it, weighted by how
    /// well; `shade` (0–1) favours the shade-tolerant (the understory). None where no species
    /// fits (the tropics, deserts and tundra wait for their species).
    pub fn choose(&self, c: &PlaceClimate, shade: f32, roll: f32) -> Option<usize> {
        self.choose_by(c, roll, |n| {
            1.0 - shade + shade * (0.2 + 1.6 * n.shade_tolerance)
        })
    }

    /// The species that takes opened ground (a clearing, a burn), drawn by `roll`: the
    /// light-demanding, wind-sown, fast-growing pioneers most often, the shade-tolerant
    /// seldom.
    pub fn choose_open(&self, c: &PlaceClimate, roll: f32) -> Option<usize> {
        self.choose_by(c, roll, |n| (1.0 - n.shade_tolerance).powi(3) * n.colonizes)
    }

    /// The species that takes a gap in the old forest, drawn by `roll`: those that grow in
    /// shade, the quicker of them more often.
    pub fn choose_gap(&self, c: &PlaceClimate, roll: f32) -> Option<usize> {
        self.choose_by(c, roll, |n| (0.2 + 1.6 * n.shade_tolerance) * n.pace)
    }

    fn choose_by(
        &self,
        c: &PlaceClimate,
        roll: f32,
        light: impl Fn(&Niche) -> f32,
    ) -> Option<usize> {
        let mut weights: smallvec::SmallVec<[f32; 32]> = smallvec::SmallVec::new();
        let mut best = 0.0f32;
        for (i, n) in self.niches.iter().enumerate() {
            let s = n.suits(c);
            best = best.max(s);
            let id = &self.templates.species[i].id;
            weights.push(s * s * n.affinity(id, c.biome) * light(n));
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

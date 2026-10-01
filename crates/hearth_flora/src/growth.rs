//! Allometry and stages: how tall and how thick a tree of a species is at an age, and the stage
//! it is in (seedling to ancient, and dead standing).

use hearth_content::schema::flora::{Plant, TreeForm};

/// A tree or woody shrub as the growth model reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Species {
    pub id: String,
    pub max_height_m: f32,
    /// Trunk diameter at breast height of the oldest trees (m).
    pub max_diameter_m: f32,
    pub lifespan_years: f32,
    /// Height growth when young (m a year).
    pub growth_m_per_year: f32,
    pub deciduous: bool,
    pub form: TreeForm,
}

impl Species {
    /// The species of a plant with a growth form.
    pub fn from_plant(p: &Plant) -> Option<Self> {
        let form = p.tree.clone()?;
        Some(Self {
            id: p.id.clone(),
            max_height_m: p.max_height_m.max(0.5),
            max_diameter_m: p
                .max_trunk_diameter_m
                .unwrap_or(p.max_height_m / 40.0)
                .max(0.02),
            lifespan_years: p.lifespan_years.unwrap_or(150.0).max(5.0),
            growth_m_per_year: p.growth_m_per_year.unwrap_or(0.4).max(0.05),
            deciduous: p.deciduous,
            form,
        })
    }

    /// The rate of the height curve: half the full height is reached when growth at the young
    /// rate would have got there, Hmax ÷ (2 × early growth).
    fn k(&self) -> f32 {
        1.88 * self.growth_m_per_year / self.max_height_m
    }

    /// Height (m) at an age (years): a Chapman–Richards curve, H = Hmax (1 − e^(−k·a))^1.4.
    pub fn height_at(&self, age: f32) -> f32 {
        self.max_height_m * (1.0 - (-self.k() * age.max(0.0)).exp()).powf(1.4)
    }

    /// The age (years) at which it reaches a height (m).
    pub fn age_at_height(&self, h: f32) -> f32 {
        let f = (h / self.max_height_m).clamp(0.0, 0.999).powf(1.0 / 1.4);
        -(1.0 - f).ln() / self.k()
    }

    /// Trunk diameter at breast height (m) at an age: it goes on thickening after the height
    /// levels off, D = Dmax (1 − e^(−a/τ))^1.3 with τ a third of the lifespan.
    pub fn diameter_at(&self, age: f32) -> f32 {
        let tau = self.lifespan_years / 3.0;
        self.max_diameter_m * (1.0 - (-age.max(0.0) / tau).exp()).powf(1.3)
    }
}

/// The stages of a tree's life (v2 §6.2): its blocks change only from one to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Stage {
    /// Under 0.6 m.
    Seedling,
    /// To 3 m (or a third of the species' height).
    Sapling,
    /// To two fifths of its height.
    Pole,
    /// To three quarters.
    Young,
    Mature,
    /// Past half its lifespan.
    Old,
    /// Past four fifths.
    Ancient,
    /// Dead, standing: trunk and broken limbs, no foliage.
    Snag,
}

impl Stage {
    pub const LIVING: [Stage; 7] = [
        Stage::Seedling,
        Stage::Sapling,
        Stage::Pole,
        Stage::Young,
        Stage::Mature,
        Stage::Old,
        Stage::Ancient,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Stage::Seedling => "seedling",
            Stage::Sapling => "sapling",
            Stage::Pole => "pole",
            Stage::Young => "young",
            Stage::Mature => "mature",
            Stage::Old => "old",
            Stage::Ancient => "ancient",
            Stage::Snag => "snag",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_name(name: &str) -> Option<Stage> {
        [
            Stage::Seedling,
            Stage::Sapling,
            Stage::Pole,
            Stage::Young,
            Stage::Mature,
            Stage::Old,
            Stage::Ancient,
            Stage::Snag,
        ]
        .into_iter()
        .find(|s| s.name() == name)
    }

    /// The ages (years) at which a living tree passes into each next stage.
    fn bounds(sp: &Species) -> [f32; 6] {
        let h = sp.max_height_m;
        let a = |m: f32| sp.age_at_height(m);
        let seedling = a(0.6_f32.min(0.1 * h));
        let sapling = a(3.0_f32.min(0.33 * h)).max(seedling + 1.0);
        let pole = a(0.4 * h).max(sapling + 1.0);
        let young = a(0.75 * h).max(pole + 1.0);
        let old = (0.5 * sp.lifespan_years).max(young + 2.0);
        let ancient = (0.8 * sp.lifespan_years).max(old + 2.0);
        [seedling, sapling, pole, young, old, ancient]
    }

    /// The stage of a living tree of this age.
    pub fn of(sp: &Species, age: f32) -> Stage {
        let b = Self::bounds(sp);
        match b.iter().position(|edge| age < *edge) {
            Some(i) => Self::LIVING[i],
            None => Stage::Ancient,
        }
    }

    /// The age a stage is grown at (the middle of its span).
    pub fn age(self, sp: &Species) -> f32 {
        let b = Self::bounds(sp);
        match self {
            Stage::Seedling => b[0] * 0.6,
            Stage::Sapling => 0.5 * (b[0] + b[1]),
            Stage::Pole => 0.5 * (b[1] + b[2]),
            Stage::Young => 0.5 * (b[2] + b[3]),
            Stage::Mature => 0.5 * (b[3] + b[4]),
            Stage::Old => 0.5 * (b[4] + b[5]),
            Stage::Ancient => 0.5 * (b[5] + sp.lifespan_years),
            Stage::Snag => 0.5 * (b[4] + b[5]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hearth_content::schema::flora::{AutumnColor, BarkPattern, Crown, LeafKind};

    pub(crate) fn oak() -> Species {
        Species {
            id: "oak".into(),
            max_height_m: 35.0,
            max_diameter_m: 3.0,
            lifespan_years: 800.0,
            growth_m_per_year: 0.45,
            deciduous: true,
            form: TreeForm {
                crown: Crown::Spreading,
                crown_ratio: 0.65,
                crown_width: 0.85,
                apical_dominance: 0.25,
                branch_angle_deg: 55.0,
                droop: 0.1,
                whorled: false,
                stems: 1,
                root_flare: 0.6,
                leaf: LeafKind::Broad,
                foliage_density: 0.75,
                autumn: AutumnColor::Brown,
                bark: BarkPattern::Furrowed,
                log: hearth_content::IdRef("hearth:oak_log".into()),
                branch: hearth_content::IdRef("hearth:oak_branch".into()),
                leaves: hearth_content::IdRef("hearth:oak_leaves".into()),
            },
        }
    }

    #[test]
    fn an_oak_grows_as_oaks_do() {
        let oak = oak();
        // About 20–26 m and under half a metre through at 60; 30 m and more and over a metre
        // at 250; near its full height and over 2.5 m through when ancient.
        let (h60, d60) = (oak.height_at(60.0), oak.diameter_at(60.0));
        assert!((19.0..27.0).contains(&h60), "{h60} m at 60");
        assert!((0.25..0.6).contains(&d60), "{d60} m at 60");
        let (h250, d250) = (oak.height_at(250.0), oak.diameter_at(250.0));
        assert!(h250 > 30.0 && d250 > 1.0, "{h250} m, {d250} m at 250");
        assert!(oak.diameter_at(750.0) > 2.5);
        // The inverse holds.
        assert!((oak.age_at_height(h60) - 60.0).abs() < 0.5);
    }

    #[test]
    fn stages_follow_age_in_order() {
        let oak = oak();
        let mut last = Stage::Seedling;
        for age in [1.0, 5.0, 15.0, 40.0, 100.0, 300.0, 500.0, 700.0] {
            let s = Stage::of(&oak, age);
            assert!(s >= last, "{age}: {s:?} after {last:?}");
            last = s;
        }
        assert_eq!(Stage::of(&oak, 1.0), Stage::Seedling);
        assert_eq!(Stage::of(&oak, 700.0), Stage::Ancient);
        // Each stage is grown at an age that is in it.
        for s in Stage::LIVING {
            assert_eq!(Stage::of(&oak, s.age(&oak)), s, "{s:?}");
        }
    }
}

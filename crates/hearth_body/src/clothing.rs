//! What covers each part of the body: insulation, wind and water resistance per region, from
//! the garments worn (layers on one region add up).

use hearth_content::schema::body::{BodyRegion, Garment};
use serde::{Deserialize, Serialize};

/// Every body region, in a fixed order.
pub const REGIONS: [BodyRegion; 11] = [
    BodyRegion::Head,
    BodyRegion::Neck,
    BodyRegion::Chest,
    BodyRegion::Abdomen,
    BodyRegion::Pelvis,
    BodyRegion::UpperArm,
    BodyRegion::LowerArm,
    BodyRegion::Hand,
    BodyRegion::UpperLeg,
    BodyRegion::LowerLeg,
    BodyRegion::Foot,
];

/// Share of the skin's area on a region (both sides of the paired limbs together), after the
/// burn charts' rule of nines adjusted to these regions; they sum to 1.
pub fn region_area(r: BodyRegion) -> f64 {
    match r {
        BodyRegion::Head => 0.07,
        BodyRegion::Neck => 0.02,
        BodyRegion::Chest => 0.17,
        BodyRegion::Abdomen => 0.13,
        BodyRegion::Pelvis => 0.06,
        BodyRegion::UpperArm => 0.08,
        BodyRegion::LowerArm => 0.06,
        BodyRegion::Hand => 0.05,
        BodyRegion::UpperLeg => 0.17,
        BodyRegion::LowerLeg => 0.12,
        BodyRegion::Foot => 0.07,
    }
}

/// Index of a region in [`REGIONS`].
pub fn region_index(r: BodyRegion) -> usize {
    REGIONS
        .iter()
        .position(|&x| x == r)
        .expect("every region is listed")
}

/// What covers one region.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct RegionCover {
    /// Insulation (clo) of everything on the region.
    pub clo: f32,
    /// 0–1: how much of the wind's effect the cover keeps out.
    pub wind: f32,
    /// 0–1: how much rain the cover keeps off the skin.
    pub water: f32,
}

/// What a body wears, region by region.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Worn {
    pub regions: [RegionCover; 11],
}

impl Worn {
    /// Nothing at all.
    pub fn naked() -> Self {
        Self::default()
    }

    /// The garments' cover: insulation adds up over layers; wind and water resistance combine
    /// as each layer stops a share of what the outer ones let through.
    pub fn of<'a>(garments: impl IntoIterator<Item = &'a Garment>) -> Self {
        let mut w = Self::default();
        for g in garments {
            for &r in &g.regions {
                let c = &mut w.regions[region_index(r)];
                c.clo += g.clo;
                c.wind = 1.0 - (1.0 - c.wind) * (1.0 - g.wind.clamp(0.0, 1.0));
                c.water = 1.0 - (1.0 - c.water) * (1.0 - g.water.clamp(0.0, 1.0));
            }
        }
        w
    }

    pub fn cover(&self, r: BodyRegion) -> RegionCover {
        self.regions[region_index(r)]
    }

    /// The share of rain that reaches the skin, over the whole body.
    pub fn rain_through(&self) -> f64 {
        REGIONS
            .iter()
            .zip(&self.regions)
            .map(|(&r, c)| region_area(r) * (1.0 - c.water as f64))
            .sum()
    }

    /// Area-weighted insulation (clo), for display.
    pub fn mean_clo(&self) -> f64 {
        REGIONS
            .iter()
            .zip(&self.regions)
            .map(|(&r, c)| region_area(r) * c.clo as f64)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_cover_the_skin_once() {
        let total: f64 = REGIONS.iter().map(|&r| region_area(r)).sum();
        assert!((total - 1.0).abs() < 1e-9, "{total}");
        let naked = Worn::naked();
        assert!((naked.rain_through() - 1.0).abs() < 1e-9);
        assert_eq!(naked.mean_clo(), 0.0);
    }
}

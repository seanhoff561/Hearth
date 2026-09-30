//! Phenology plumbing (v2 §4.3): the seasonal state of vegetation at a place and time, derived
//! from the local seasonal temperature and moisture. V2-1 provides it for generic plant
//! functional types (it drives seasonal colours); V2-6 layers per-species calendars on top.

use crate::climate::Normals;

/// Generic plant functional types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlantType {
    /// Temperate/boreal deciduous broadleaf trees and shrubs.
    DeciduousBroadleaf,
    /// Evergreen conifers (spruce, pine, fir).
    EvergreenConifer,
    /// Grasses and herbs.
    Grass,
}

/// Seasonal state of vegetation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhenoState {
    /// Leaf cover 0 (bare) .. 1 (full).
    pub leaf: f64,
    /// 1 = fresh green, 0 = fully senescent (autumn colour, cured grass).
    pub greenness: f64,
    /// 1 at peak flowering.
    pub flowering: f64,
}

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Seasonal state of a plant type at a place and year fraction.
pub fn state(kind: PlantType, n: &Normals, year_frac: f64) -> PhenoState {
    let t = n.temperature(year_frac);
    // Warming or cooling: compare with a little later in the year.
    let rising = n.temperature(year_frac + 0.02) > t;
    let dry = n.is_dry_season(year_frac);
    match kind {
        PlantType::DeciduousBroadleaf => {
            // Leaves out as spring passes ~8 °C; colour turns as autumn falls through 12 °C,
            // leaves drop around 5 °C. Drought-deciduous in strong dry seasons.
            let (leaf, green) = if rising {
                let l = smoothstep(6.0, 10.0, t);
                (l, 1.0)
            } else {
                let colour = smoothstep(5.0, 13.0, t);
                let l = smoothstep(3.0, 7.0, t);
                (l, colour)
            };
            // Drought-deciduous only where the dry season is the winter (savanna, monsoon).
            let drought = if dry && n.winter_dry > n.summer_dry {
                0.35
            } else {
                1.0
            };
            PhenoState {
                leaf: leaf * drought,
                greenness: green,
                flowering: if rising {
                    smoothstep(6.0, 9.0, t) * (1.0 - smoothstep(11.0, 14.0, t))
                } else {
                    0.0
                },
            }
        }
        PlantType::EvergreenConifer => PhenoState {
            leaf: 1.0,
            // Needles dull a little in hard winters.
            greenness: 0.75 + 0.25 * smoothstep(-15.0, 5.0, t),
            flowering: if rising {
                smoothstep(4.0, 8.0, t) * (1.0 - smoothstep(10.0, 13.0, t))
            } else {
                0.0
            },
        },
        PlantType::Grass => {
            // Green while warm enough and moist; cures to straw in dry seasons and winter.
            let warm = smoothstep(2.0, 8.0, t);
            let moist = if dry { 0.15 } else { 1.0 };
            PhenoState {
                leaf: 1.0,
                greenness: warm * moist,
                flowering: if rising {
                    smoothstep(12.0, 16.0, t)
                } else {
                    0.0
                } * moist,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temperate_trees_leaf_out_and_turn() {
        let n = Normals::new(50.0, 9.0, 18.0, 750.0, 0.0, 0.0);
        let summer = state(PlantType::DeciduousBroadleaf, &n, 0.35);
        let winter = state(PlantType::DeciduousBroadleaf, &n, 0.85);
        let autumn = state(PlantType::DeciduousBroadleaf, &n, 0.62);
        assert!(summer.leaf > 0.95 && summer.greenness > 0.95);
        assert!(winter.leaf < 0.05, "bare in winter");
        assert!(autumn.greenness < 0.8, "autumn colour");
        let conifer = state(PlantType::EvergreenConifer, &n, 0.85);
        assert_eq!(conifer.leaf, 1.0, "evergreen");
    }

    #[test]
    fn savanna_grass_cures_in_the_dry_season() {
        let n = Normals::new(12.0, 26.0, 5.0, 900.0, 0.9, 0.0);
        let wet = state(PlantType::Grass, &n, 0.35);
        let dry = state(PlantType::Grass, &n, 0.85);
        assert!(wet.greenness > 0.8 && dry.greenness < 0.3);
    }
}

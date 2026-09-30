//! Biomes: Earth-like zones derived from the climate model, altitude (highland zones) and local
//! conditions (water, slope, coast). Grass/foliage/water tints come from climate colormaps, not
//! per-biome constants, so colour shifts smoothly across the world.

use serde::{Deserialize, Serialize};

use crate::planet::climate::ClimateClass;

/// All biomes. Stable numeric ids (used in saves and the LOD cache).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum Biome {
    // Oceans
    WarmShallows = 0,
    TemperateSea = 1,
    ColdSea = 2,
    PolarSea = 3,
    DeepOcean = 4,
    Trench = 5,
    // Water & coasts
    River = 6,
    Lake = 7,
    Wetland = 8,
    Beach = 9,
    StonyShore = 10,
    // Tropics and dry lands
    TropicalRainforest = 11,
    Savanna = 12,
    HotDesert = 13,
    DuneSea = 14,
    Mesa = 15,
    Oasis = 16,
    SaltFlat = 17,
    Steppe = 18,
    ColdDesert = 19,
    MediterraneanScrub = 20,
    // Temperate
    TemperatePlains = 21,
    BroadleafForest = 22,
    BirchForest = 23,
    TemperateRainforest = 24,
    MixedForest = 25,
    // Cold
    BorealForest = 26,
    SnowyTaiga = 27,
    Tundra = 28,
    IceSheet = 29,
    // Highlands
    MontaneForest = 30,
    Krummholz = 31,
    AlpineMeadow = 32,
    AlpineRock = 33,
    Glacier = 34,
    Volcanic = 35,
}

impl Biome {
    pub const ALL: [Biome; 36] = [
        Biome::WarmShallows,
        Biome::TemperateSea,
        Biome::ColdSea,
        Biome::PolarSea,
        Biome::DeepOcean,
        Biome::Trench,
        Biome::River,
        Biome::Lake,
        Biome::Wetland,
        Biome::Beach,
        Biome::StonyShore,
        Biome::TropicalRainforest,
        Biome::Savanna,
        Biome::HotDesert,
        Biome::DuneSea,
        Biome::Mesa,
        Biome::Oasis,
        Biome::SaltFlat,
        Biome::Steppe,
        Biome::ColdDesert,
        Biome::MediterraneanScrub,
        Biome::TemperatePlains,
        Biome::BroadleafForest,
        Biome::BirchForest,
        Biome::TemperateRainforest,
        Biome::MixedForest,
        Biome::BorealForest,
        Biome::SnowyTaiga,
        Biome::Tundra,
        Biome::IceSheet,
        Biome::MontaneForest,
        Biome::Krummholz,
        Biome::AlpineMeadow,
        Biome::AlpineRock,
        Biome::Glacier,
        Biome::Volcanic,
    ];

    pub fn from_u8(v: u8) -> Biome {
        Self::ALL
            .get(v as usize)
            .copied()
            .unwrap_or(Biome::TemperatePlains)
    }

    /// Stable id `hearth:<name>`.
    pub fn name(self) -> &'static str {
        match self {
            Biome::WarmShallows => "warm_shallows",
            Biome::TemperateSea => "temperate_sea",
            Biome::ColdSea => "cold_sea",
            Biome::PolarSea => "polar_sea",
            Biome::DeepOcean => "deep_ocean",
            Biome::Trench => "trench",
            Biome::River => "river",
            Biome::Lake => "lake",
            Biome::Wetland => "wetland",
            Biome::Beach => "beach",
            Biome::StonyShore => "stony_shore",
            Biome::TropicalRainforest => "tropical_rainforest",
            Biome::Savanna => "savanna",
            Biome::HotDesert => "hot_desert",
            Biome::DuneSea => "dune_sea",
            Biome::Mesa => "mesa",
            Biome::Oasis => "oasis",
            Biome::SaltFlat => "salt_flat",
            Biome::Steppe => "steppe",
            Biome::ColdDesert => "cold_desert",
            Biome::MediterraneanScrub => "mediterranean_scrub",
            Biome::TemperatePlains => "temperate_plains",
            Biome::BroadleafForest => "broadleaf_forest",
            Biome::BirchForest => "birch_forest",
            Biome::TemperateRainforest => "temperate_rainforest",
            Biome::MixedForest => "mixed_forest",
            Biome::BorealForest => "boreal_forest",
            Biome::SnowyTaiga => "snowy_taiga",
            Biome::Tundra => "tundra",
            Biome::IceSheet => "ice_sheet",
            Biome::MontaneForest => "montane_forest",
            Biome::Krummholz => "krummholz",
            Biome::AlpineMeadow => "alpine_meadow",
            Biome::AlpineRock => "alpine_rock",
            Biome::Glacier => "glacier",
            Biome::Volcanic => "volcanic",
        }
    }

    pub fn is_ocean(self) -> bool {
        (self as u8) <= Biome::Trench as u8
    }

    pub fn is_water(self) -> bool {
        self.is_ocean() || matches!(self, Biome::River | Biome::Lake)
    }

    /// Precipitation falls as snow here regardless of season.
    pub fn is_snowy(self) -> bool {
        matches!(
            self,
            Biome::SnowyTaiga | Biome::IceSheet | Biome::Glacier | Biome::PolarSea
        )
    }

    /// Map colour for the biome map.
    pub fn color(self) -> [u8; 3] {
        match self {
            Biome::WarmShallows => [64, 200, 210],
            Biome::TemperateSea => [40, 110, 180],
            Biome::ColdSea => [45, 85, 140],
            Biome::PolarSea => [170, 200, 230],
            Biome::DeepOcean => [15, 35, 95],
            Biome::Trench => [10, 10, 50],
            Biome::River => [60, 110, 230],
            Biome::Lake => [50, 100, 210],
            Biome::Wetland => [70, 120, 90],
            Biome::Beach => [240, 225, 160],
            Biome::StonyShore => [140, 140, 140],
            Biome::TropicalRainforest => [20, 110, 20],
            Biome::Savanna => [190, 180, 80],
            Biome::HotDesert => [240, 200, 120],
            Biome::DuneSea => [250, 215, 140],
            Biome::Mesa => [200, 110, 60],
            Biome::Oasis => [70, 170, 90],
            Biome::SaltFlat => [245, 245, 235],
            Biome::Steppe => [200, 190, 120],
            Biome::ColdDesert => [190, 170, 140],
            Biome::MediterraneanScrub => [160, 160, 80],
            Biome::TemperatePlains => [140, 200, 90],
            Biome::BroadleafForest => [60, 150, 50],
            Biome::BirchForest => [110, 170, 80],
            Biome::TemperateRainforest => [40, 100, 60],
            Biome::MixedForest => [60, 130, 70],
            Biome::BorealForest => [40, 90, 60],
            Biome::SnowyTaiga => [170, 200, 190],
            Biome::Tundra => [160, 170, 140],
            Biome::IceSheet => [235, 245, 255],
            Biome::MontaneForest => [50, 110, 70],
            Biome::Krummholz => [90, 120, 90],
            Biome::AlpineMeadow => [150, 180, 110],
            Biome::AlpineRock => [130, 125, 120],
            Biome::Glacier => [220, 235, 250],
            Biome::Volcanic => [80, 70, 70],
        }
    }
}

/// Local conditions used to pick a biome.
#[derive(Debug, Clone, Copy)]
pub struct BiomeInputs {
    pub climate: ClimateClass,
    /// Mean annual temperature at the surface (°C).
    pub temperature: f32,
    /// Warmest-month temperature at the surface (°C).
    pub t_warm: f32,
    pub precipitation: f32,
    /// Terrain height and water level in blocks.
    pub height: f32,
    pub water: f32,
    /// Blocks of terrain per block of distance.
    pub slope: f32,
    /// Height above the tree line / snow line (blocks, negative below).
    pub above_tree_line: f32,
    pub above_snow_line: f32,
    /// Ocean depth class inputs.
    pub ocean: bool,
    pub sea_temperature: f32,
    pub near_ocean: bool,
    pub river: bool,
    pub lake: bool,
    pub salt_flat: bool,
    pub volcanic: bool,
    /// Uniform variation noise in 0..1 for patchiness.
    pub variation: f32,
    /// Second variation noise in 0..1.
    pub variation2: f32,
    /// Vertical scale (blocks per metre) to express depths in real units.
    pub vertical_scale: f32,
}

/// Picks the biome for a column.
pub fn select(i: &BiomeInputs) -> Biome {
    let underwater = i.water.is_finite() && i.height < i.water - 0.5;
    if i.ocean && underwater {
        let depth_m = (i.water - i.height) / i.vertical_scale;
        if depth_m > 6000.0 {
            return Biome::Trench;
        }
        if depth_m > 1500.0 {
            return Biome::DeepOcean;
        }
        return if i.sea_temperature < -1.0 {
            Biome::PolarSea
        } else if i.sea_temperature < 9.0 {
            Biome::ColdSea
        } else if i.sea_temperature > 21.0 && depth_m < 120.0 {
            Biome::WarmShallows
        } else {
            Biome::TemperateSea
        };
    }
    if underwater && i.lake {
        return Biome::Lake;
    }
    if i.river && underwater {
        return Biome::River;
    }
    if i.salt_flat {
        return Biome::SaltFlat;
    }
    if i.volcanic && i.above_tree_line > 0.0 {
        return Biome::Volcanic;
    }
    // Highland zones on top of any climate.
    if i.above_snow_line > 0.0 {
        return if i.slope > 1.2 {
            Biome::AlpineRock
        } else {
            Biome::Glacier
        };
    }
    if i.climate == ClimateClass::IceCap {
        return Biome::IceSheet;
    }
    if i.above_tree_line > 0.0 && i.climate != ClimateClass::Tundra {
        let dry = i.precipitation < 350.0;
        return if i.slope > 1.0 || dry || i.above_tree_line > 120.0 {
            Biome::AlpineRock
        } else {
            Biome::AlpineMeadow
        };
    }
    // Coasts.
    let shore = i.near_ocean && i.height < i.water.max(0.0) + 3.0 && i.water.is_finite();
    if shore {
        let cold = i.temperature < 2.0;
        return if i.slope > 0.7 || cold {
            Biome::StonyShore
        } else {
            Biome::Beach
        };
    }
    // Wetlands: flat, wet, poorly drained lowlands next to water.
    let near_water = i.water.is_finite() && i.height < i.water + 3.0;
    if near_water && i.slope < 0.06 && i.precipitation > 700.0 && i.temperature > -2.0 {
        return Biome::Wetland;
    }
    if i.above_tree_line > -40.0 && i.above_tree_line <= 0.0 {
        let cold_climate = matches!(i.climate, ClimateClass::Subarctic | ClimateClass::Tundra);
        if !cold_climate && i.precipitation > 350.0 {
            return Biome::Krummholz;
        }
    }
    let montane = i.above_tree_line > -260.0 && i.above_tree_line <= -40.0;
    match i.climate {
        ClimateClass::Ocean => Biome::TemperateSea,
        ClimateClass::TropicalRainforest => Biome::TropicalRainforest,
        ClimateClass::TropicalSavanna => {
            if i.variation < 0.12 && i.precipitation > 1200.0 {
                Biome::TropicalRainforest
            } else {
                Biome::Savanna
            }
        }
        ClimateClass::HotDesert => {
            if near_water || i.river {
                Biome::Oasis
            } else if i.slope > 0.45 && i.variation2 > 0.35 {
                Biome::Mesa
            } else if i.variation > 0.45 && i.slope < 0.25 {
                Biome::DuneSea
            } else {
                Biome::HotDesert
            }
        }
        ClimateClass::HotSteppe => {
            if i.precipitation > 550.0 && i.variation > 0.6 {
                Biome::Savanna
            } else {
                Biome::Steppe
            }
        }
        ClimateClass::ColdDesert => Biome::ColdDesert,
        ClimateClass::ColdSteppe => Biome::Steppe,
        ClimateClass::Mediterranean => {
            if i.precipitation > 800.0 && i.variation > 0.55 {
                Biome::BroadleafForest
            } else {
                Biome::MediterraneanScrub
            }
        }
        ClimateClass::HumidSubtropical => {
            if montane {
                Biome::MontaneForest
            } else if i.variation < 0.22 {
                Biome::TemperatePlains
            } else {
                Biome::BroadleafForest
            }
        }
        ClimateClass::Oceanic => {
            if i.precipitation > 1700.0 && i.near_ocean_windward() {
                Biome::TemperateRainforest
            } else if montane {
                Biome::MontaneForest
            } else if i.variation < 0.28 {
                Biome::TemperatePlains
            } else if i.variation2 > 0.72 {
                Biome::BirchForest
            } else {
                Biome::BroadleafForest
            }
        }
        ClimateClass::HumidContinental => {
            if montane {
                Biome::MontaneForest
            } else if i.variation < 0.3 {
                Biome::TemperatePlains
            } else if i.t_warm < 19.5 || i.variation2 > 0.6 {
                Biome::MixedForest
            } else if i.variation2 < 0.18 {
                Biome::BirchForest
            } else {
                Biome::BroadleafForest
            }
        }
        ClimateClass::Subarctic => {
            if i.temperature < -6.0 {
                Biome::SnowyTaiga
            } else {
                Biome::BorealForest
            }
        }
        ClimateClass::Tundra => Biome::Tundra,
        ClimateClass::IceCap => Biome::IceSheet,
    }
}

impl BiomeInputs {
    /// Very wet mid-latitude coasts (windward): the oceanic rainforest condition.
    fn near_ocean_windward(&self) -> bool {
        self.precipitation > 1700.0
    }
}

/// Grass colour from climate (temperature × precipitation colormap). Returns linear-ish sRGB.
pub fn grass_color(temperature: f32, precipitation: f32) -> [u8; 3] {
    climate_color(temperature, precipitation, false)
}

/// Foliage colour from climate.
pub fn foliage_color(temperature: f32, precipitation: f32) -> [u8; 3] {
    climate_color(temperature, precipitation, true)
}

fn climate_color(t: f32, p: f32, foliage: bool) -> [u8; 3] {
    // Axes of the colormap: warmth 0..1 (cold → hot), wetness 0..1 (dry → wet).
    let warmth = ((t + 10.0) / 38.0).clamp(0.0, 1.0);
    let wet = (p / 2200.0).clamp(0.0, 1.0);
    // Corner colours: cold-dry, cold-wet, hot-dry, hot-wet.
    let (cd, cw, hd, hw): ([f32; 3], [f32; 3], [f32; 3], [f32; 3]) = if foliage {
        (
            [96.0, 161.0, 123.0],
            [72.0, 140.0, 110.0],
            [174.0, 164.0, 42.0],
            [26.0, 160.0, 24.0],
        )
    } else {
        (
            [128.0, 180.0, 151.0],
            [100.0, 170.0, 120.0],
            [191.0, 183.0, 85.0],
            [71.0, 205.0, 51.0],
        )
    };
    let lerp3 = |a: [f32; 3], b: [f32; 3], t: f32| {
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
        ]
    };
    let cold = lerp3(cd, cw, wet);
    let hot = lerp3(hd, hw, wet);
    let c = lerp3(cold, hot, warmth);
    [c[0] as u8, c[1] as u8, c[2] as u8]
}

/// Water colour (tint) from sea temperature and depth: turquoise tropical shallows, green-grey
/// cold water, deep navy offshore.
pub fn water_color(sea_temperature: f32, depth_blocks: f32) -> [u8; 3] {
    let warm = ((sea_temperature - 5.0) / 22.0).clamp(0.0, 1.0);
    let shallow = (1.0 - depth_blocks / 40.0).clamp(0.0, 1.0);
    let cold = [58.0f32, 90.0, 110.0];
    let temperate = [44.0f32, 94.0, 170.0];
    let tropical = [40.0f32, 170.0, 190.0];
    let lerp3 = |a: [f32; 3], b: [f32; 3], t: f32| {
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
        ]
    };
    let base = if warm < 0.5 {
        lerp3(cold, temperate, warm * 2.0)
    } else {
        lerp3(temperate, tropical, (warm - 0.5) * 2.0 * shallow + 0.0)
    };
    [base[0] as u8, base[1] as u8, base[2] as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(climate: ClimateClass) -> BiomeInputs {
        BiomeInputs {
            climate,
            temperature: 12.0,
            t_warm: 20.0,
            precipitation: 900.0,
            height: 40.0,
            water: f32::NEG_INFINITY,
            slope: 0.1,
            above_tree_line: -500.0,
            above_snow_line: -900.0,
            ocean: false,
            sea_temperature: 12.0,
            near_ocean: false,
            river: false,
            lake: false,
            salt_flat: false,
            volcanic: false,
            variation: 0.5,
            variation2: 0.5,
            vertical_scale: 0.25,
        }
    }

    #[test]
    fn ids_round_trip() {
        for b in Biome::ALL {
            assert_eq!(Biome::from_u8(b as u8), b);
        }
    }

    #[test]
    fn climate_to_biome() {
        assert_eq!(
            select(&inputs(ClimateClass::TropicalRainforest)),
            Biome::TropicalRainforest
        );
        assert_eq!(
            select(&inputs(ClimateClass::Subarctic)),
            Biome::BorealForest
        );
        assert_eq!(select(&inputs(ClimateClass::IceCap)), Biome::IceSheet);
        let mut high = inputs(ClimateClass::HumidContinental);
        high.above_tree_line = 30.0;
        assert_eq!(select(&high), Biome::AlpineMeadow);
        high.above_snow_line = 10.0;
        assert_eq!(select(&high), Biome::Glacier);
    }

    #[test]
    fn oceans_by_depth_and_temperature() {
        let mut o = inputs(ClimateClass::Ocean);
        o.ocean = true;
        o.water = 0.0;
        o.height = -10.0;
        o.sea_temperature = 26.0;
        assert_eq!(select(&o), Biome::WarmShallows);
        o.sea_temperature = 3.0;
        assert_eq!(select(&o), Biome::ColdSea);
        o.height = -900.0;
        assert_eq!(select(&o), Biome::DeepOcean);
        o.height = -2600.0;
        assert_eq!(select(&o), Biome::Trench);
    }

    #[test]
    fn climate_colormap_is_warm_and_wet_green() {
        let hot_wet = grass_color(27.0, 2500.0);
        let hot_dry = grass_color(27.0, 100.0);
        assert!(hot_wet[1] > hot_wet[0] && hot_dry[0] > hot_wet[0]);
    }
}

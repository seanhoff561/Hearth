//! Seasonal vegetation colours as GPU parameters (v2 §4.3, §21): meshes store a 24-bit climate
//! code per vertex instead of a baked colour, and the shader colours grass and leaves for the
//! current date. Changing the season never remeshes the world.
//!
//! Code layout (24 bits): annual mean temperature (8 bits, (T + 50) × 2.5), annual range / 2
//! (5 bits), precipitation (8 bits, sqrt scale up to 6,000 mm), dry-season type (2 bits),
//! southern hemisphere (1 bit). `shaders/terrain.wgsl` decodes the same layout; this module is
//! the CPU reference (tests, LOD, maps).

use crate::climate::Normals;
use crate::phenology::{self, PlantType};

/// How a place's year dries out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DryType {
    None = 0,
    /// Savanna/monsoon: dry winters.
    WinterDry = 1,
    /// Mediterranean: dry summers.
    SummerDry = 2,
    /// Deserts and steppes: grass is cured most of the year.
    Arid = 3,
}

impl DryType {
    /// The dry-season type of a place: arid climates are dry all year; otherwise the stronger
    /// of a marked winter or summer dry season (the same threshold as
    /// `Normals::is_dry_season`, so colours, phenology and weather agree).
    pub fn of(n: &Normals, arid: bool) -> Self {
        if arid {
            DryType::Arid
        } else if n.winter_dry > 0.3 && n.winter_dry >= n.summer_dry {
            DryType::WinterDry
        } else if n.summer_dry > 0.3 {
            DryType::SummerDry
        } else {
            DryType::None
        }
    }
}

pub fn encode(t_mean: f64, t_range: f64, precip: f64, dry: DryType, southern: bool) -> u32 {
    let t = ((t_mean + 50.0) * 2.5).round().clamp(0.0, 255.0) as u32;
    let r = (t_range / 2.0).round().clamp(0.0, 31.0) as u32;
    let p = ((precip.max(0.0) / 6000.0).sqrt() * 255.0)
        .round()
        .clamp(0.0, 255.0) as u32;
    t | (r << 8) | (p << 13) | ((dry as u32) << 21) | (u32::from(southern) << 23)
}

/// Decoded climate code.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Decoded {
    pub t_mean: f64,
    pub t_range: f64,
    pub precip: f64,
    pub dry: DryType,
    pub southern: bool,
}

pub fn decode(code: u32) -> Decoded {
    let t = (code & 255) as f64 / 2.5 - 50.0;
    let r = ((code >> 8) & 31) as f64 * 2.0;
    let p = ((code >> 13) & 255) as f64 / 255.0;
    let dry = match (code >> 21) & 3 {
        1 => DryType::WinterDry,
        2 => DryType::SummerDry,
        3 => DryType::Arid,
        _ => DryType::None,
    };
    Decoded {
        t_mean: t,
        t_range: r,
        precip: p * p * 6000.0,
        dry,
        southern: (code >> 23) & 1 == 1,
    }
}

impl Decoded {
    /// Normals equivalent to the code (latitude only carries the hemisphere).
    pub fn normals(&self) -> Normals {
        let (winter_dry, summer_dry) = match self.dry {
            DryType::WinterDry => (0.9, 0.0),
            DryType::SummerDry => (0.0, 0.9),
            _ => (0.0, 0.0),
        };
        Normals::new(
            if self.southern { -45.0 } else { 45.0 },
            self.t_mean,
            self.t_range,
            self.precip,
            winter_dry,
            summer_dry,
        )
    }
}

fn lerp3(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Summer colour from the temperature × precipitation colormap (0..1 sRGB).
pub fn summer_color(t_mean: f64, precip: f64, foliage: bool) -> [f64; 3] {
    let warmth = ((t_mean + 10.0) / 38.0).clamp(0.0, 1.0);
    let wet = (precip / 2200.0).clamp(0.0, 1.0);
    let (cd, cw, hd, hw) = if foliage {
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
    let c = lerp3(lerp3(cd, cw, wet), lerp3(hd, hw, wet), warmth);
    [c[0] / 255.0, c[1] / 255.0, c[2] / 255.0]
}

/// Straw colour of cured grass and the brown of dead leaves.
pub const CURED: [f64; 3] = [0.78, 0.68, 0.42];
pub const DEAD_LEAVES: [f64; 3] = [0.47, 0.35, 0.22];
pub const AUTUMN_OAK: [f64; 3] = [0.70, 0.45, 0.16];
pub const AUTUMN_BIRCH: [f64; 3] = [0.88, 0.72, 0.22];

/// Reference seasonal grass colour for a code and year fraction.
pub fn grass(code: u32, year_frac: f64) -> [f64; 3] {
    let d = decode(code);
    let n = d.normals();
    let base = summer_color(d.t_mean, d.precip, false);
    let s = phenology::state(PlantType::Grass, &n, year_frac);
    let green = if d.dry == DryType::Arid {
        s.greenness * 0.35
    } else {
        s.greenness
    };
    lerp3(CURED, base, green)
}

/// Reference seasonal colour of deciduous leaves (`birch` picks the yellow autumn).
pub fn deciduous(code: u32, year_frac: f64, birch: bool) -> [f64; 3] {
    let d = decode(code);
    let n = d.normals();
    let base = if birch {
        [0.50, 0.65, 0.33]
    } else {
        summer_color(d.t_mean, d.precip, true)
    };
    let s = phenology::state(PlantType::DeciduousBroadleaf, &n, year_frac);
    let autumn = if birch { AUTUMN_BIRCH } else { AUTUMN_OAK };
    let turned = lerp3(autumn, base, s.greenness);
    lerp3(DEAD_LEAVES, turned, s.leaf.clamp(0.0, 1.0).sqrt())
}

/// Reference seasonal colour of evergreen needles.
pub fn evergreen(code: u32, year_frac: f64) -> [f64; 3] {
    let d = decode(code);
    let s = phenology::state(PlantType::EvergreenConifer, &d.normals(), year_frac);
    let base = [0.38, 0.60, 0.38];
    lerp3([0.30, 0.38, 0.28], base, s.greenness)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip() {
        let c = encode(12.3, 18.0, 850.0, DryType::SummerDry, true);
        assert!(c < 1 << 24);
        let d = decode(c);
        assert!((d.t_mean - 12.3).abs() < 0.25);
        assert!((d.t_range - 18.0).abs() < 1.01);
        assert!((d.precip - 850.0).abs() / 850.0 < 0.05);
        assert_eq!(d.dry, DryType::SummerDry);
        assert!(d.southern);
    }

    #[test]
    fn seasons_change_the_colours() {
        let c = encode(9.0, 18.0, 750.0, DryType::None, false);
        let summer = grass(c, 0.35);
        let winter = grass(c, 0.85);
        assert!(summer[1] > summer[0] + 0.1, "green in summer {summer:?}");
        assert!(winter[0] > winter[1] - 0.15, "cured in winter {winter:?}");
        let autumn = deciduous(c, 0.62, false);
        assert!(autumn[0] > 0.45, "leaves turn {autumn:?}");
        let birch = deciduous(c, 0.62, true);
        assert!(
            birch[1] > autumn[1],
            "birches go yellow {birch:?} vs {autumn:?}"
        );
        // The southern hemisphere is half a year out of phase.
        let s = encode(9.0, 18.0, 750.0, DryType::None, true);
        assert!((grass(s, 0.85)[1] - summer[1]).abs() < 0.02);
    }
}

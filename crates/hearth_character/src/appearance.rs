//! How a person looks (v2 §9.1): body, height and build, skin tone and undertone, hair and
//! facial hair, eyes, a name, and the loincloth everyone starts in. Purely cosmetic: the body
//! model is one reference adult (D66).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BodyType {
    Female,
    #[default]
    Male,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HairStyle {
    #[default]
    ShortCrop,
    Buzzed,
    ShoulderLength,
    LongStraight,
    LongWavy,
    Curly,
    Coily,
    Braids,
    Locs,
    TiedBack,
    Bald,
}

impl HairStyle {
    pub const ALL: [HairStyle; 11] = [
        HairStyle::ShortCrop,
        HairStyle::Buzzed,
        HairStyle::ShoulderLength,
        HairStyle::LongStraight,
        HairStyle::LongWavy,
        HairStyle::Curly,
        HairStyle::Coily,
        HairStyle::Braids,
        HairStyle::Locs,
        HairStyle::TiedBack,
        HairStyle::Bald,
    ];

    /// The language key of its name.
    pub fn key(self) -> &'static str {
        match self {
            HairStyle::ShortCrop => "character.hair.short_crop",
            HairStyle::Buzzed => "character.hair.buzzed",
            HairStyle::ShoulderLength => "character.hair.shoulder_length",
            HairStyle::LongStraight => "character.hair.long_straight",
            HairStyle::LongWavy => "character.hair.long_wavy",
            HairStyle::Curly => "character.hair.curly",
            HairStyle::Coily => "character.hair.coily",
            HairStyle::Braids => "character.hair.braids",
            HairStyle::Locs => "character.hair.locs",
            HairStyle::TiedBack => "character.hair.tied_back",
            HairStyle::Bald => "character.hair.bald",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FacialHair {
    #[default]
    None,
    Stubble,
    Moustache,
    Goatee,
    ShortBeard,
    FullBeard,
}

impl FacialHair {
    pub const ALL: [FacialHair; 6] = [
        FacialHair::None,
        FacialHair::Stubble,
        FacialHair::Moustache,
        FacialHair::Goatee,
        FacialHair::ShortBeard,
        FacialHair::FullBeard,
    ];

    pub fn key(self) -> &'static str {
        match self {
            FacialHair::None => "character.facial.none",
            FacialHair::Stubble => "character.facial.stubble",
            FacialHair::Moustache => "character.facial.moustache",
            FacialHair::Goatee => "character.facial.goatee",
            FacialHair::ShortBeard => "character.facial.short_beard",
            FacialHair::FullBeard => "character.facial.full_beard",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EyeColor {
    #[default]
    Brown,
    DarkBrown,
    Hazel,
    Amber,
    Green,
    Blue,
    Grey,
}

impl EyeColor {
    pub const ALL: [EyeColor; 7] = [
        EyeColor::Brown,
        EyeColor::DarkBrown,
        EyeColor::Hazel,
        EyeColor::Amber,
        EyeColor::Green,
        EyeColor::Blue,
        EyeColor::Grey,
    ];

    pub fn key(self) -> &'static str {
        match self {
            EyeColor::Brown => "character.eyes.brown",
            EyeColor::DarkBrown => "character.eyes.dark_brown",
            EyeColor::Hazel => "character.eyes.hazel",
            EyeColor::Amber => "character.eyes.amber",
            EyeColor::Green => "character.eyes.green",
            EyeColor::Blue => "character.eyes.blue",
            EyeColor::Grey => "character.eyes.grey",
        }
    }

    /// The iris (sRGB).
    pub fn srgb(self) -> [u8; 3] {
        match self {
            EyeColor::Brown => [99, 57, 24],
            EyeColor::DarkBrown => [52, 30, 16],
            EyeColor::Hazel => [124, 98, 52],
            EyeColor::Amber => [165, 110, 40],
            EyeColor::Green => [86, 120, 72],
            EyeColor::Blue => [78, 118, 164],
            EyeColor::Grey => [130, 140, 150],
        }
    }
}

/// What the loincloth is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Loincloth {
    #[default]
    Hide,
    PlantFibre,
}

impl Loincloth {
    pub fn srgb(self) -> [u8; 3] {
        match self {
            Loincloth::Hide => [128, 92, 60],
            Loincloth::PlantFibre => [140, 128, 84],
        }
    }
}

/// The natural hair colours (sRGB) and their language keys.
pub const HAIR_COLORS: [(&str, [u8; 3]); 11] = [
    ("character.hair_color.black", [28, 24, 22]),
    ("character.hair_color.dark_brown", [59, 42, 32]),
    ("character.hair_color.brown", [90, 59, 38]),
    ("character.hair_color.light_brown", [138, 99, 66]),
    ("character.hair_color.auburn", [124, 55, 31]),
    ("character.hair_color.red", [165, 71, 31]),
    ("character.hair_color.strawberry_blonde", [197, 140, 90]),
    ("character.hair_color.blonde", [214, 179, 122]),
    ("character.hair_color.platinum", [232, 220, 194]),
    ("character.hair_color.grey", [154, 150, 146]),
    ("character.hair_color.white", [230, 228, 224]),
];

/// Skin tones across the natural human range, lightest to darkest (sRGB swatches of a ten-step
/// scale, ordered by lightness); the tone slider moves continuously between them.
const SKIN: [[u8; 3]; 10] = [
    [246, 237, 228],
    [247, 234, 208],
    [243, 231, 219],
    [234, 218, 186],
    [215, 189, 150],
    [160, 126, 86],
    [130, 92, 67],
    [96, 65, 52],
    [58, 49, 42],
    [41, 36, 32],
];

/// The tone of each of the ten swatches, as presets.
pub fn skin_presets() -> [f32; 10] {
    std::array::from_fn(|i| i as f32 / 9.0)
}

/// A person's look.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// Optional.
    pub name: String,
    pub body: BodyType,
    /// Standing height (m), 0.8–1.95: a child's to a tall grown one's.
    pub height_m: f32,
    /// 0 slight – 0.5 average – 1 heavy.
    pub build: f32,
    /// 0 lightest – 1 darkest.
    pub skin_tone: f32,
    /// −1 cool (pink) – 0 neutral – 1 warm (golden).
    pub undertone: f32,
    pub hair: HairStyle,
    /// sRGB.
    pub hair_color: [u8; 3],
    pub facial_hair: FacialHair,
    pub eyes: EyeColor,
    pub loincloth: Loincloth,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            name: String::new(),
            body: BodyType::Male,
            height_m: 1.75,
            build: 0.5,
            skin_tone: 0.5,
            undertone: 0.2,
            hair: HairStyle::ShortCrop,
            hair_color: HAIR_COLORS[1].1,
            facial_hair: FacialHair::None,
            eyes: EyeColor::Brown,
            loincloth: Loincloth::Hide,
        }
    }
}

/// The shortest a figure stands: a small child (until H3's children have their own proportions,
/// a child is a grown body made small).
pub const MIN_HEIGHT_M: f32 = 0.8;
pub const MAX_HEIGHT_M: f32 = 1.95;

impl Appearance {
    /// A first look for a female body.
    pub fn female() -> Self {
        Self {
            body: BodyType::Female,
            height_m: 1.63,
            hair: HairStyle::LongStraight,
            ..Self::default()
        }
    }

    /// Values brought into their ranges.
    pub fn sanitized(mut self) -> Self {
        self.height_m = if self.height_m.is_finite() {
            self.height_m.clamp(MIN_HEIGHT_M, MAX_HEIGHT_M)
        } else {
            1.75
        };
        let unit = |v: f32, d: f32| if v.is_finite() { v.clamp(0.0, 1.0) } else { d };
        self.build = unit(self.build, 0.5);
        self.skin_tone = unit(self.skin_tone, 0.5);
        self.undertone = if self.undertone.is_finite() {
            self.undertone.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.name = self.name.chars().take(32).collect();
        self
    }

    /// The skin's albedo (linear RGB).
    pub fn skin_linear(&self) -> [f32; 3] {
        skin_linear(self.skin_tone, self.undertone)
    }

    /// The hair's albedo (linear RGB).
    pub fn hair_linear(&self) -> [f32; 3] {
        srgb_to_linear(self.hair_color)
    }
}

pub fn srgb_to_linear(c: [u8; 3]) -> [f32; 3] {
    c.map(|v| {
        let s = v as f32 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    })
}

pub fn linear_to_srgb(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| {
        let v = v.clamp(0.0, 1.0);
        let s = if v <= 0.003_130_8 {
            v * 12.92
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0).round() as u8
    })
}

/// Relative luminance of a linear colour.
pub fn luminance(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// The skin's linear albedo for a tone (0 lightest – 1 darkest) and an undertone (−1 cool –
/// 1 warm): between the scale's swatches in linear light, the undertone tilting the hue at the
/// same luminance.
pub fn skin_linear(tone: f32, undertone: f32) -> [f32; 3] {
    let t = tone.clamp(0.0, 1.0) * 9.0;
    let i = (t.floor() as usize).min(8);
    let f = t - i as f32;
    // The swatches are colours as seen; skin reflects about 0.6 of red light at the lightest
    // and 0.04 at the darkest.
    let albedo = |c: [u8; 3]| srgb_to_linear(c).map(|v| 0.025 + 0.63 * v);
    let a = albedo(SKIN[i]);
    let b = albedo(SKIN[i + 1]);
    let c: [f32; 3] = std::array::from_fn(|k| a[k] + (b[k] - a[k]) * f);
    let u = undertone.clamp(-1.0, 1.0);
    let tilt = if u < 0.0 {
        // Cool: rosier, less yellow.
        [1.0 + 0.06 * -u, 1.0 - 0.04 * -u, 1.0 + 0.03 * -u]
    } else {
        // Warm: golden.
        [1.0 + 0.04 * u, 1.0 + 0.01 * u, 1.0 - 0.14 * u]
    };
    let tilted: [f32; 3] = std::array::from_fn(|k| c[k] * tilt[k]);
    let keep = luminance(c) / luminance(tilted).max(1e-6);
    tilted.map(|v| v * keep)
}

/// Hue (0–360), saturation and lightness (0–1) of an sRGB colour.
pub fn to_hsl(c: [u8; 3]) -> [f32; 3] {
    let [r, g, b] = c.map(|v| v as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d < 1e-6 {
        return [0.0, 0.0, l];
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    [h, s.clamp(0.0, 1.0), l]
}

/// The sRGB colour of a hue (0–360), saturation and lightness (0–1).
pub fn from_hsl([h, s, l]: [f32; 3]) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s.clamp(0.0, 1.0);
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l.clamp(0.0, 1.0) - c / 2.0;
    [r, g, b].map(|v| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_round_trips() {
        for (_, c) in HAIR_COLORS {
            let back = from_hsl(to_hsl(c));
            for k in 0..3 {
                assert!(
                    (back[k] as i32 - c[k] as i32).abs() <= 1,
                    "{c:?} -> {back:?}"
                );
            }
        }
    }
}

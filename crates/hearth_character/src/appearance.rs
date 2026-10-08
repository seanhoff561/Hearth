//! How a person looks (v2 §9.1, Amendment E §6.2): body, height and build, skin tone,
//! undertone and freckles, the face's shape, hair (style, length, colour), eyebrows and facial
//! hair, eyes, a name, and the loincloth everyone starts in. Purely cosmetic: the body model is
//! one reference adult (D66), and how a person looks changes nothing they can do.

use hearth_math::hash::Rng;
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

/// How heavy the eyebrows are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Eyebrows {
    Fine,
    #[default]
    Natural,
    Thick,
    Bushy,
}

impl Eyebrows {
    pub const ALL: [Eyebrows; 4] = [
        Eyebrows::Fine,
        Eyebrows::Natural,
        Eyebrows::Thick,
        Eyebrows::Bushy,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Eyebrows::Fine => "character.brows.fine",
            Eyebrows::Natural => "character.brows.natural",
            Eyebrows::Thick => "character.brows.thick",
            Eyebrows::Bushy => "character.brows.bushy",
        }
    }

    /// How thick, against a natural brow's.
    pub fn weight(self) -> f32 {
        match self {
            Eyebrows::Fine => 0.6,
            Eyebrows::Natural => 1.0,
            Eyebrows::Thick => 1.4,
            Eyebrows::Bushy => 1.8,
        }
    }
}

/// The face's shape: each feature from −1 (narrow, small, low) through 0 (the average) to 1
/// (wide, large, strong).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Face {
    pub jaw: f32,
    pub cheekbones: f32,
    pub brow: f32,
    pub nose: f32,
    pub eyes: f32,
    pub lips: f32,
    pub ears: f32,
}

impl Face {
    /// The sliders' language keys, in their order.
    pub const KEYS: [&'static str; 7] = [
        "character.face.jaw",
        "character.face.cheekbones",
        "character.face.brow",
        "character.face.nose",
        "character.face.eyes",
        "character.face.lips",
        "character.face.ears",
    ];

    /// Each feature in the sliders' order.
    pub fn features_mut(&mut self) -> [&mut f32; 7] {
        [
            &mut self.jaw,
            &mut self.cheekbones,
            &mut self.brow,
            &mut self.nose,
            &mut self.eyes,
            &mut self.lips,
            &mut self.ears,
        ]
    }

    fn sanitized(mut self) -> Self {
        for v in self.features_mut() {
            *v = if v.is_finite() {
                v.clamp(-1.0, 1.0)
            } else {
                0.0
            };
        }
        self
    }
}

/// Faces to begin from (their language keys), before the sliders.
pub const FACE_PRESETS: [(&str, Face); 6] = [
    (
        "character.face.preset.oval",
        Face {
            jaw: 0.0,
            cheekbones: 0.0,
            brow: 0.0,
            nose: 0.0,
            eyes: 0.0,
            lips: 0.0,
            ears: 0.0,
        },
    ),
    (
        "character.face.preset.round",
        Face {
            jaw: -0.3,
            cheekbones: 0.4,
            brow: -0.3,
            nose: -0.2,
            eyes: 0.2,
            lips: 0.2,
            ears: 0.0,
        },
    ),
    (
        "character.face.preset.square",
        Face {
            jaw: 0.8,
            cheekbones: 0.2,
            brow: 0.4,
            nose: 0.1,
            eyes: -0.1,
            lips: 0.0,
            ears: 0.1,
        },
    ),
    (
        "character.face.preset.long",
        Face {
            jaw: 0.1,
            cheekbones: -0.2,
            brow: 0.2,
            nose: 0.6,
            eyes: -0.2,
            lips: -0.2,
            ears: 0.3,
        },
    ),
    (
        "character.face.preset.heart",
        Face {
            jaw: -0.7,
            cheekbones: 0.6,
            brow: -0.1,
            nose: -0.3,
            eyes: 0.4,
            lips: 0.3,
            ears: -0.1,
        },
    ),
    (
        "character.face.preset.angular",
        Face {
            jaw: 0.5,
            cheekbones: 0.9,
            brow: 0.7,
            nose: 0.4,
            eyes: -0.3,
            lips: -0.3,
            ears: 0.0,
        },
    ),
];

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
    /// Standing height (m), within an adult's range ([`MIN_HEIGHT_M`]–[`MAX_HEIGHT_M`]).
    pub height_m: f32,
    /// 0 slight – 0.5 average – 1 heavy.
    pub build: f32,
    /// 0 lightest – 1 darkest.
    pub skin_tone: f32,
    /// −1 cool (pink) – 0 neutral – 1 warm (golden).
    pub undertone: f32,
    /// 0 none – 1 many.
    pub freckles: f32,
    pub face: Face,
    pub hair: HairStyle,
    /// 0 shortest – 0.5 the style's own – 1 longest (for the styles that hang).
    pub hair_length: f32,
    /// sRGB.
    pub hair_color: [u8; 3],
    pub eyebrows: Eyebrows,
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
            freckles: 0.0,
            face: Face::default(),
            hair: HairStyle::ShortCrop,
            hair_length: 0.5,
            hair_color: HAIR_COLORS[1].1,
            eyebrows: Eyebrows::Natural,
            facial_hair: FacialHair::None,
            eyes: EyeColor::Brown,
            loincloth: Loincloth::Hide,
        }
    }
}

/// An adult's standing height (m), shortest to tallest (Amendment E §6.2: adults only).
pub const MIN_HEIGHT_M: f32 = 1.45;
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
        self.freckles = unit(self.freckles, 0.0);
        self.hair_length = unit(self.hair_length, 0.5);
        self.face = self.face.sanitized();
        self.name = self.name.chars().take(32).collect();
        self
    }

    /// Someone drawn from the natural variation of adults (the name and body kept): height and
    /// build about the body's means, skin across the human range, and the lighter hair and eyes
    /// and the freckles found together, as they are, with the lightest skins (the pigmentation
    /// genes act on all three; red hair goes with freckles).
    pub fn randomized(&self, seed: u64) -> Self {
        let mut r = Rng::new(seed ^ 0x5eed_face);
        let mut normal = |mean: f32, sd: f32| {
            let (u, v) = (r.next_f32().max(1e-6), r.next_f32());
            mean + sd * (-2.0 * u.ln()).sqrt() * (std::f32::consts::TAU * v).cos()
        };
        let female = self.body == BodyType::Female;
        // Adult heights (sd some 7 cm) and builds.
        let height_m = normal(if female { 1.63 } else { 1.76 }, 0.07);
        let build = normal(0.5, 0.17);
        let mut face = Face::default();
        for v in face.features_mut() {
            *v = normal(0.0, 0.4);
        }
        let mut r = Rng::new(seed ^ 0x0dd_c010);
        let skin_tone = r.next_f32();
        let undertone = r.range_f32(-0.6, 0.8);
        // How far the lightest pigmentation reaches at this tone: none past the middle.
        let light = smoothstep(0.5, 0.05, skin_tone);
        let pick = |r: &mut Rng, weights: &[f32]| {
            let total: f32 = weights.iter().sum();
            let mut x = r.next_f32() * total;
            for (i, w) in weights.iter().enumerate() {
                if x < *w {
                    return i;
                }
                x -= w;
            }
            weights.len() - 1
        };
        // Eyes: brown, dark brown, hazel, amber, green, blue, grey.
        let dark = smoothstep(0.4, 0.8, skin_tone);
        let eyes = EyeColor::ALL[pick(
            &mut r,
            &[
                0.45,
                0.2 + 0.8 * dark,
                0.08 + 0.12 * light,
                0.03,
                0.15 * light,
                0.6 * light,
                0.12 * light,
            ],
        )];
        // Hair: black, dark brown, brown, light brown, auburn, red, strawberry blonde, blonde,
        // platinum; grey and white with age.
        let hair_i = pick(
            &mut r,
            &[
                0.35 + 0.9 * dark,
                0.35,
                0.25 + 0.1 * light,
                0.35 * light,
                0.06 * light,
                0.06 * light,
                0.04 * light,
                0.35 * light,
                0.05 * light,
                0.04,
                0.015,
            ],
        );
        let red = (4..=6).contains(&hair_i);
        let freckles = if red {
            r.range_f32(0.45, 1.0)
        } else if r.next_f32() < 0.35 * light {
            r.range_f32(0.1, 0.6)
        } else {
            0.0
        };
        let hair = loop {
            let h = HairStyle::ALL[r.below(HairStyle::ALL.len() as u32) as usize];
            // Few women are bald.
            if h != HairStyle::Bald || !female || r.next_f32() < 0.1 {
                break h;
            }
        };
        let facial_hair = if female {
            FacialHair::None
        } else {
            FacialHair::ALL[pick(&mut r, &[0.4, 0.15, 0.08, 0.07, 0.15, 0.15])]
        };
        let eyebrows =
            Eyebrows::ALL[pick(&mut r, &[0.2 + 0.2 * female as u8 as f32, 0.5, 0.25, 0.1])];
        Self {
            name: self.name.clone(),
            body: self.body,
            height_m,
            build,
            skin_tone,
            undertone,
            freckles,
            face,
            hair,
            hair_length: r.next_f32(),
            hair_color: HAIR_COLORS[hair_i].1,
            eyebrows,
            facial_hair,
            eyes,
            loincloth: self.loincloth,
        }
        .sanitized()
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

/// 0 at `a`, 1 at `b` (either way round), smooth between.
fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
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
    fn an_appearance_round_trips_and_old_ones_load() {
        let a = Appearance {
            name: "Ash".into(),
            freckles: 0.7,
            face: FACE_PRESETS[4].1,
            hair_length: 0.9,
            eyebrows: Eyebrows::Thick,
            ..Appearance::female()
        }
        .randomized(3);
        let text = serde_json::to_string(&a).expect("to json");
        let back: Appearance = serde_json::from_str(&text).expect("from json");
        assert_eq!(back, a);
        assert_eq!(back.sanitized(), a, "already in range");
        // One saved before the face and freckles: the average face, none.
        let old: Appearance = serde_json::from_str(r#"{"name":"Bo","skin_tone":0.3}"#).unwrap();
        assert_eq!(old.face, Face::default());
        assert_eq!(old.freckles, 0.0);
        assert_eq!(old.hair_length, 0.5);
    }

    #[test]
    fn randomize_draws_adults_with_pigmentation_as_it_goes_together() {
        let base = Appearance::default();
        let people: Vec<Appearance> = (0..4000).map(|s| base.randomized(s)).collect();
        let light_eyes =
            |a: &Appearance| matches!(a.eyes, EyeColor::Blue | EyeColor::Green | EyeColor::Grey);
        let fair_hair = |a: &Appearance| HAIR_COLORS[3..9].iter().any(|(_, c)| *c == a.hair_color);
        let share = |set: &[&Appearance], f: &dyn Fn(&Appearance) -> bool| {
            set.iter().filter(|a| f(a)).count() as f32 / set.len().max(1) as f32
        };
        let darker: Vec<&Appearance> = people.iter().filter(|a| a.skin_tone > 0.6).collect();
        let lightest: Vec<&Appearance> = people.iter().filter(|a| a.skin_tone < 0.15).collect();
        assert!(darker.len() > 1000 && lightest.len() > 300);
        // No blue eyes or blond hair with dark skin; common with the lightest.
        assert_eq!(share(&darker, &light_eyes), 0.0);
        assert_eq!(share(&darker, &fair_hair), 0.0);
        assert!(
            share(&lightest, &light_eyes) > 0.3,
            "{}",
            share(&lightest, &light_eyes)
        );
        assert!(share(&lightest, &fair_hair) > 0.3);
        // Red hair goes with freckles.
        let red: Vec<&Appearance> = people
            .iter()
            .filter(|a| HAIR_COLORS[4..7].iter().any(|(_, c)| *c == a.hair_color))
            .collect();
        assert!(!red.is_empty());
        assert!(red.iter().all(|a| a.freckles >= 0.45));
        assert!(share(&darker, &|a| a.freckles > 0.0) == 0.0);
        // Adults of the body's heights; the name and body kept.
        let mean = people.iter().map(|a| a.height_m).sum::<f32>() / people.len() as f32;
        assert!((1.73..1.79).contains(&mean), "{mean}");
        assert!(
            people
                .iter()
                .all(|a| a.body == base.body && a == &a.clone().sanitized())
        );
        assert_ne!(base.randomized(1), base.randomized(2));
        assert_eq!(base.randomized(1), base.randomized(1));
    }

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

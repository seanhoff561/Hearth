//! How a person looks (V2.1 §4.3; H1): their figure's appearance read from their phenotype — the
//! same reading for every person and every player, so families look alike.

use hearth_content::schema::humans::BodyPlan;
use serde::{Deserialize, Serialize};

use crate::genome::Phenotype;

/// The eye colours a phenotype can give (as the figure draws them).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Eyes {
    #[default]
    DarkBrown,
    Brown,
    Amber,
    Hazel,
    Green,
    Blue,
    Grey,
}

/// What a person's figure is drawn from: their looks as their genes, development and age make
/// them (a message's payload: serializable, D166).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Look {
    /// 0 lightest – 1 darkest.
    pub skin_tone: f32,
    /// −1 cool – 1 warm.
    pub undertone: f32,
    /// sRGB.
    pub hair_color: [u8; 3],
    /// 0 straight – 1 coily.
    pub hair_curl: f32,
    pub eyes: Eyes,
    /// 0 none – 1 a full beard (a grown man's).
    pub beard: f32,
    /// 0 slight – 1 heavy.
    pub build: f32,
}

impl Default for Look {
    /// One of no recorded genes: dark-skinned and dark-haired, as the tropics' people and apes.
    fn default() -> Self {
        Self {
            skin_tone: 0.72,
            undertone: 0.0,
            hair_color: [74, 54, 38],
            hair_curl: 0.4,
            eyes: Eyes::DarkBrown,
            beard: 0.0,
            build: 0.5,
        }
    }
}

fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// The look a phenotype gives one of a body plan, a sex and an age (years; grown at
/// `maturity`). Hair darkens from a child's to an adult's and greys in old age (H3 refines).
pub fn look(p: &Phenotype, plan: BodyPlan, female: bool, age: f32, maturity: f32) -> Look {
    let skin = p.z("skin_pigment");
    let hair = p.z("hair_darkness");
    let red = p.copies("red_hair", "red") == 2;
    let blue = p.copies("eye_major", "blue") == 2;
    let green = p.copies("eye_green", "green") > 0;
    let light = p.z("eye_lightness");
    // Skin: on the common scale, from the far north's palest (about −2.5 at 65°, a tone of 0.2)
    // to the equator's darkest (about +6, 0.8), with room either side for the people of each
    // place to differ.
    let skin_tone = clamp01(0.36 + 0.07 * skin);
    let undertone = (0.2 - 0.03 * skin).clamp(-1.0, 1.0);
    // Hair: from platinum through blonde and brown to black by its darkness; red where the
    // red-hair locus is homozygous; lighter in a child.
    let child = (1.0 - age / maturity.max(1.0)).clamp(0.0, 1.0) * 0.35;
    let dark = clamp01(0.62 + 0.2 * hair - child);
    let base = if dark < 0.25 {
        lerp3([232.0, 220.0, 194.0], [214.0, 179.0, 122.0], dark / 0.25)
    } else if dark < 0.5 {
        lerp3(
            [214.0, 179.0, 122.0],
            [138.0, 99.0, 66.0],
            (dark - 0.25) / 0.25,
        )
    } else if dark < 0.75 {
        lerp3([138.0, 99.0, 66.0], [59.0, 42.0, 32.0], (dark - 0.5) / 0.25)
    } else {
        lerp3([59.0, 42.0, 32.0], [28.0, 24.0, 22.0], (dark - 0.75) / 0.25)
    };
    let base = if red {
        lerp3([197.0, 110.0, 50.0], [124.0, 55.0, 31.0], dark)
    } else {
        base
    };
    let grey = clamp01((age - 50.0) / 25.0);
    let c = lerp3(base, [154.0, 150.0, 146.0], grey);
    let hair_color = [c[0] as u8, c[1] as u8, c[2] as u8];
    // Eyes: blue in a homozygote of the major locus (green with the modifier, grey when its
    // lightness runs high); otherwise brown, hazel with the modifier, lighter or darker by the
    // polygenic lightness.
    let eyes = match (blue, green) {
        (true, true) => Eyes::Green,
        (true, false) if light > 1.0 => Eyes::Grey,
        (true, false) => Eyes::Blue,
        (false, true) => Eyes::Hazel,
        (false, false) if light > 1.2 => Eyes::Amber,
        (false, false) if light < -0.4 => Eyes::DarkBrown,
        (false, false) => Eyes::Brown,
    };
    let grown_man = !female && age >= maturity;
    let beard = if grown_man {
        clamp01(0.5 + 0.25 * p.z("beard_density"))
    } else {
        0.0
    };
    let _ = plan;
    Look {
        skin_tone,
        undertone,
        hair_color,
        hair_curl: clamp01(0.4 + 0.22 * p.z("hair_curl")),
        eyes,
        beard,
        build: clamp01(0.5 + 0.15 * p.z("build")),
    }
}

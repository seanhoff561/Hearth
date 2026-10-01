//! Food (v2 §9.3, §11.5): what eating a thing gives the body, and how things go off.
//!
//! Spoiling runs on the day scale: a material keeps `keeps_days` at 20 °C, two and a half
//! times faster for every ten degrees warmer and as much slower for every ten colder; frozen,
//! almost not at all; wet things go off faster. Spoiled food risks food poisoning.

use hearth_content::Content;
use hearth_content::schema::material::Material;
use hearth_items::{ItemKind, Stack};

/// What eating a thing gives.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Bite {
    pub kcal: f32,
    pub protein_g: f32,
    pub fat_g: f32,
    pub carb_g: f32,
    /// Water it brings (l).
    pub water_l: f32,
    /// Its bulk in the stomach (l).
    pub volume_l: f32,
    /// Days of fresh-food vitamins.
    pub fresh_days: f32,
    /// Causes of illness it risks, with their chance.
    pub risks: Vec<(String, f32)>,
}

/// The food in `kg` of a material, as it is now (`decay` 0–1).
pub fn bite(m: &Material, kg: f32, decay: f32) -> Option<Bite> {
    let n = m.nutrition.as_ref()?;
    let mut risks: Vec<(String, f32)> = Vec::new();
    if let Some((cause, per_kg)) = &n.risk {
        risks.push((cause.clone(), (per_kg * kg).min(1.0)));
    }
    if decay > 0.5 {
        // Going off: a growing chance of poisoning, near certain when spoiled.
        risks.push(("spoiled_food".into(), ((decay - 0.5) * 1.8).min(0.9)));
    }
    let kcal = m.kcal_per_kg.unwrap_or_else(|| n.kcal()) * kg;
    Some(Bite {
        kcal,
        protein_g: n.protein_g * kg,
        fat_g: n.fat_g * kg,
        carb_g: n.carb_g * kg,
        water_l: n.water * kg,
        volume_l: kg / m.density_kg_m3.max(100.0) * 1000.0,
        fresh_days: n.fresh_days * kg * (1.0 - decay),
        risks,
    })
}

/// The food in one unit of a stack (one cut, one handful), if it is food.
pub fn bite_of(c: &Content, kind: &ItemKind, stack: &Stack) -> Option<Bite> {
    let m = c.materials.get(kind.material.as_deref()?)?;
    bite(m, kind.mass_kg, stack.decay)
}

/// Days a kind of thing keeps at 20 °C: its own `keeps_days` property, or its material's.
pub fn keeps_days(c: &Content, kind: &ItemKind) -> Option<f32> {
    kind.property("keeps_days").or_else(|| {
        kind.material
            .as_deref()
            .and_then(|m| c.materials.get(m))
            .and_then(|m| m.keeps_days)
    })
}

/// How much of the way to spoiled a thing goes in an hour at `temp_c`, wet `wet` (0–1).
pub fn decay_per_hour(keeps_days: f32, temp_c: f32, wet: f32) -> f32 {
    let base = 1.0 / (keeps_days.max(0.01) * 24.0);
    let warmth = if temp_c <= -2.0 {
        0.02
    } else {
        2.5f32.powf((temp_c - 20.0) / 10.0)
    };
    base * warmth * (1.0 + wet.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meat_keeps_two_days_warm_and_over_a_week_cold() {
        let warm = 1.0 / decay_per_hour(2.0, 20.0, 0.0) / 24.0;
        let cold = 1.0 / decay_per_hour(2.0, 4.0, 0.0) / 24.0;
        let frozen = 1.0 / decay_per_hour(2.0, -10.0, 0.0) / 24.0;
        assert!((warm - 2.0).abs() < 0.01);
        assert!(cold > 8.0 && cold < 12.0, "{cold} days at 4 °C");
        assert!(frozen > 90.0, "{frozen} days frozen");
    }
}

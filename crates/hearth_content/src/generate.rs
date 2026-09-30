//! "Generate, don't enumerate": items are form × material with mass and properties derived
//! from the material, so a new wood or rock gets its logs, billets and tools without code.

use crate::content::{Origin, Table};
use crate::diag::Report;
use crate::schema::item::{Item, ItemForm, PropertyValue, Stacking};
use crate::schema::material::Material;
use crate::schema::{Entry, Status};

/// A concrete item type (explicit or generated).
#[derive(Debug, Clone, PartialEq)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    pub form: Option<String>,
    pub material: Option<String>,
    pub mass_kg: f32,
    pub volume_l: f32,
    pub footprint: (u8, u8),
    pub stacking: Stacking,
    pub properties: Vec<(String, f32)>,
    pub tags: Vec<String>,
    pub status: Status,
}

impl ItemDef {
    pub fn property(&self, name: &str) -> Option<f32> {
        self.properties
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| *v)
    }
}

/// The id of form `form` in material `material`: `ns:form/material` (the material's namespace
/// is kept when it differs: `ns:form/other_ns.material`).
pub fn generated_id(form: &str, material: &str) -> String {
    let (fns, _) = form.split_once(':').unwrap_or(("hearth", form));
    let (mns, mpath) = material.split_once(':').unwrap_or(("hearth", material));
    if fns == mns {
        format!("{form}/{mpath}")
    } else {
        format!("{form}/{mns}.{mpath}")
    }
}

/// Reads a numeric material field by name.
pub fn material_field(m: &Material, field: &str) -> Option<f32> {
    Some(match field {
        "density_kg_m3" => m.density_kg_m3,
        "hardness_mohs" => m.hardness_mohs?,
        "knapping" => m.knapping?,
        "elastic_modulus_gpa" => m.elastic_modulus_gpa?,
        "compressive_mpa" => m.strength.compressive_mpa?,
        "tensile_mpa" => m.strength.tensile_mpa?,
        "bending_mpa" => m.strength.bending_mpa?,
        "fuel_mj_kg" => m.fuel_mj_kg?,
        "workability" => m.workability?,
        "durability" => m.durability?,
        "porosity" => m.porosity?,
        "kcal_per_kg" => m.kcal_per_kg?,
        _ => return None,
    })
}

/// Names of the fields [`material_field`] understands.
pub const MATERIAL_FIELDS: &[&str] = &[
    "density_kg_m3",
    "hardness_mohs",
    "knapping",
    "elastic_modulus_gpa",
    "compressive_mpa",
    "tensile_mpa",
    "bending_mpa",
    "fuel_mj_kg",
    "workability",
    "durability",
    "porosity",
    "kcal_per_kg",
];

pub fn generate_items(
    forms: &Table<ItemForm>,
    materials: &Table<Material>,
    explicit: &Table<Item>,
    report: &mut Report,
) -> Table<ItemDef> {
    let mut out: Table<ItemDef> = Table::default();
    for (item, origin) in explicit.iter_with_origin() {
        out.upsert(
            item.id.clone(),
            ItemDef {
                id: item.id.clone(),
                name: item.name.clone(),
                form: None,
                material: item.material.as_ref().map(|m| m.to_string()),
                mass_kg: item.mass_kg,
                volume_l: item.volume_l,
                footprint: item.footprint,
                stacking: item.stacking,
                properties: item.properties.clone(),
                tags: item.tags.clone(),
                status: item.status,
            },
            origin.clone(),
        );
    }
    for (form, origin) in forms.iter_with_origin() {
        for p in &form.properties {
            if let PropertyValue::FromMaterial { field, .. } = &p.value
                && !MATERIAL_FIELDS.contains(&field.as_str())
            {
                report.error(
                    "bad-field",
                    Some(origin.file.clone()),
                    origin.line,
                    format!(
                        "form `{}` reads unknown material field `{field}`",
                        form.id()
                    ),
                );
            }
        }
        let volume_m3 = form.size_m[0] * form.size_m[1] * form.size_m[2] * form.fill;
        for m in materials.iter() {
            if !form.materials.matches(m.id(), m) {
                continue;
            }
            let mut properties = Vec::with_capacity(form.properties.len());
            for p in &form.properties {
                let v = match &p.value {
                    PropertyValue::Const(v) => Some(*v),
                    PropertyValue::FromMaterial { field, scale } => {
                        material_field(m, field).map(|v| v * scale)
                    }
                };
                if let Some(v) = v {
                    properties.push((p.name.clone(), v));
                }
            }
            let mut tags = form.tags.clone();
            tags.extend(m.tags.iter().cloned());
            let id = generated_id(form.id(), m.id());
            out.upsert(
                id.clone(),
                ItemDef {
                    id,
                    name: form.name.replace("{material}", &m.name),
                    form: Some(form.id().to_owned()),
                    material: Some(m.id().to_owned()),
                    mass_kg: volume_m3 * m.density_kg_m3,
                    volume_l: volume_m3 * 1000.0,
                    footprint: form.footprint,
                    stacking: form.stacking,
                    properties,
                    tags,
                    status: form.status(),
                },
                Origin {
                    file: origin.file.clone(),
                    line: origin.line,
                    pack: origin.pack,
                },
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_keep_foreign_namespaces() {
        assert_eq!(
            generated_id("hearth:flake", "hearth:flint"),
            "hearth:flake/flint"
        );
        assert_eq!(
            generated_id("hearth:flake", "geo:jasper"),
            "hearth:flake/geo.jasper"
        );
    }
}

/// What a generated natural block is (its template and behaviour).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NaturalKind {
    /// Bedrock and outcrops of a rock type.
    Rock,
    /// Cohesive earth: soil horizons, clays, peat.
    Soil,
    /// Loose grains that slide and fall (sands, gravel, ash): materials tagged `falls`.
    Loose,
    /// A few loose stones of a rock lying on the ground (`<rock>_cobbles`).
    Cobbles,
}

/// A block generated from the content tables: one per rock type and one per earth material
/// (soils, clays, sediments and every material a soil horizon is made of), so a new rock or
/// soil in data gets its block (and texture, from the material's appearance) without code.
#[derive(Debug, Clone, PartialEq)]
pub struct NaturalBlock {
    /// Namespaced block id (the rock's id).
    pub id: String,
    pub name: String,
    /// Namespaced material id.
    pub material: String,
    pub kind: NaturalKind,
    /// Break-time factor (≈1.5 for ordinary stone), from the material's strength.
    pub hardness: f32,
    pub map_color: [u8; 3],
}

/// Break-time factor of a material: compressive strength where known, else Mohs hardness.
fn hardness_of(m: &Material) -> f32 {
    let h = match (m.strength.compressive_mpa, m.hardness_mohs) {
        (Some(ucs), _) => 0.4 + ucs / 75.0,
        (None, Some(mohs)) => 0.3 + 0.3 * mohs,
        _ => 1.5,
    };
    h.clamp(0.3, 4.0)
}

/// The natural blocks the content defines: every rock type, then every earth material.
pub fn natural_blocks(c: &crate::Content) -> Vec<NaturalBlock> {
    use crate::schema::material::MaterialCategory as Cat;
    let mut out = Vec::new();
    for rock in c.rocks.iter() {
        let Some(m) = c.materials.get(rock.material.as_str()) else {
            continue;
        };
        out.push(NaturalBlock {
            id: rock.id().to_owned(),
            name: rock.name.clone(),
            material: rock.material.as_str().to_owned(),
            kind: NaturalKind::Rock,
            hardness: hardness_of(m),
            map_color: m.appearance.color.0,
        });
        // Loose stones of true rocks (not coal, salt or gypsum).
        if m.category == Cat::Rock {
            out.push(NaturalBlock {
                id: format!("{}_cobbles", rock.id()),
                name: format!("{} cobbles", rock.name),
                material: rock.material.as_str().to_owned(),
                kind: NaturalKind::Cobbles,
                hardness: 0.2,
                map_color: m.appearance.color.0,
            });
        }
    }
    // Earth materials: by category, and whatever a soil horizon at least half a block thick is
    // made of (peat).
    let mut earths: Vec<&str> = c
        .materials
        .iter()
        .filter(|m| matches!(m.category, Cat::Soil | Cat::Clay | Cat::Sediment))
        .map(|m| m.id())
        .collect();
    for soil in c.soils.iter() {
        for h in &soil.horizons {
            if h.thickness_m.1 >= 0.5 && !earths.contains(&h.material.as_str()) {
                earths.push(h.material.as_str());
            }
        }
    }
    for id in earths {
        if out.iter().any(|b| b.id == id) {
            continue;
        }
        let Some(m) = c.materials.get(id) else {
            continue;
        };
        let falls = m.tags.iter().any(|t| t == "falls");
        let frozen = m.tags.iter().any(|t| t == "frozen");
        out.push(NaturalBlock {
            id: id.to_owned(),
            name: m.name.clone(),
            material: id.to_owned(),
            kind: if falls {
                NaturalKind::Loose
            } else {
                NaturalKind::Soil
            },
            hardness: 0.25 + m.density_kg_m3 / 5000.0 + if frozen { 1.0 } else { 0.0 },
            map_color: m.appearance.color.0,
        });
    }
    out
}

#[cfg(test)]
mod natural_tests {
    use super::*;

    #[test]
    fn every_rock_and_earth_gets_a_block() {
        let c = crate::Content::load_base();
        let blocks = natural_blocks(&c);
        let rocks = blocks
            .iter()
            .filter(|b| b.kind == NaturalKind::Rock)
            .count();
        assert_eq!(rocks, c.rocks.len());
        assert!(
            blocks
                .iter()
                .any(|b| b.id == "hearth:granite_cobbles" && b.kind == NaturalKind::Cobbles)
        );
        assert!(
            !blocks
                .iter()
                .any(|b| b.id == "hearth:bituminous_coal_cobbles"),
            "no coal cobbles"
        );
        let kind = |id: &str| blocks.iter().find(|b| b.id == id).map(|b| b.kind);
        assert_eq!(kind("hearth:quartz_sand"), Some(NaturalKind::Loose));
        assert_eq!(kind("hearth:loam"), Some(NaturalKind::Soil));
        assert_eq!(
            kind("hearth:peat"),
            Some(NaturalKind::Soil),
            "thick soil horizons get blocks"
        );
        assert_eq!(
            kind("hearth:leaf_litter"),
            None,
            "thin litter is part of the turf"
        );
        let chalk = blocks
            .iter()
            .find(|b| b.id == "hearth:chalk")
            .expect("chalk");
        let granite = blocks
            .iter()
            .find(|b| b.id == "hearth:granite")
            .expect("granite");
        assert!(
            chalk.hardness < 1.0 && granite.hardness > 2.0,
            "{chalk:?} {granite:?}"
        );
    }
}

//! "Generate, don't enumerate": items are form × material with mass and properties derived
//! from the material, so a new wood or rock gets its logs, billets and tools without code.

use crate::content::{Origin, Table};
use crate::diag::Report;
use crate::schema::body::Garment;
use crate::schema::item::{ContainerSpec, Item, ItemForm, PropertyValue, Stacking, Use};
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
    /// Bounding box (m) and colour (sRGB, the material's) as a thing in the world.
    pub size_m: [f32; 3],
    pub color: [u8; 3],
    pub container: Option<ContainerSpec>,
    pub hangs_on: Vec<String>,
    /// The garment it is, when worn.
    pub garment: Option<String>,
    /// What it does in the hand with nothing aimed at (E §3.2).
    pub primary: Option<Use>,
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
    garments: &Table<Garment>,
    report: &mut Report,
) -> Table<ItemDef> {
    let mut out: Table<ItemDef> = Table::default();
    for (item, origin) in explicit.iter_with_origin() {
        let material = item
            .material
            .as_ref()
            .and_then(|m| materials.get(m.as_str()));
        let side = (item.volume_l.max(0.001) / 1000.0).cbrt();
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
                size_m: item.size_m.unwrap_or([side; 3]),
                color: material.map_or([140, 120, 100], |m| m.appearance.color.0),
                container: item.container,
                hangs_on: item.hangs_on.clone(),
                garment: None,
                primary: item.primary,
            },
            origin.clone(),
        );
    }
    // Garments are things too, made of whatever their materials allow.
    for (g, origin) in garments.iter_with_origin() {
        for m in materials.iter() {
            if !g.materials.matches(m.id(), m) {
                continue;
            }
            let id = generated_id(g.id(), m.id());
            let volume_l = g.mass_kg / m.density_kg_m3.max(1.0) * 1000.0 * 2.0;
            let side = (volume_l / 1000.0).cbrt();
            out.upsert(
                id.clone(),
                ItemDef {
                    id,
                    name: g.name.replace("{material}", &m.name),
                    form: None,
                    material: Some(m.id().to_owned()),
                    mass_kg: g.mass_kg,
                    volume_l,
                    footprint: g.footprint,
                    stacking: Stacking::Single,
                    properties: Vec::new(),
                    tags: vec!["garment".to_owned()],
                    status: g.status,
                    size_m: [side * 1.6, side * 0.4, side * 1.6],
                    color: m.appearance.color.0,
                    container: None,
                    hangs_on: Vec::new(),
                    garment: Some(g.id().to_owned()),
                    primary: None,
                },
                Origin {
                    file: origin.file.clone(),
                    line: origin.line,
                    pack: origin.pack,
                },
            );
        }
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
                    size_m: form.size_m,
                    color: m.appearance.color.0,
                    container: form.container,
                    hangs_on: form.hangs_on.clone(),
                    garment: None,
                    primary: form.primary,
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
    /// A few loose stones of a rock lying on the ground (`<rock>_cobbles`), or loose pieces of
    /// an ore (`<mineral>_float`).
    Cobbles,
    /// Rock carrying an ore mineral (`<mineral>_ore`): textured as a rock matrix flecked with
    /// the mineral.
    Ore,
    /// Stream gravel with heavy grains of a mineral (`<mineral>_placer`), for panning.
    Placer,
    /// An earthy crust or efflorescence of a mineral on the ground (`<mineral>_crust`):
    /// limonite, sulfur, cinnabar, saltpetre, bog ore.
    Crust,
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
    // Pigment earths that deposits dig (ochres, manganese black).
    for d in c.deposits.iter() {
        if let Some((id, NaturalKind::Soil)) = resource_block(c, d.resource.as_str())
            && let Some(m) = c.materials.get(&id)
            && m.category == Cat::Pigment
            && !earths.contains(&m.id())
        {
            earths.push(m.id());
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
    // Deposits: the blocks their bodies, placers, float and indicators are made of.
    let mut wanted: Vec<(String, NaturalKind, String)> = Vec::new();
    for d in c.deposits.iter() {
        let resource = d.resource.as_str();
        if let Some((id, kind)) = body_block(c, resource, d.geometry) {
            wanted.push((id, kind, material_of(c, resource).unwrap_or_default()));
        }
        if d.geometry == crate::schema::geology::DepositGeometry::Placer
            && let Some((id, kind)) = placer_block(c, resource)
        {
            wanted.push((id, kind, material_of(c, resource).unwrap_or_default()));
        }
        for ind in &d.indicators {
            let shown = ind.mineral.as_ref().map_or(resource, |m| m.as_str());
            if ind.kind == crate::schema::geology::IndicatorKind::Float {
                if let Some(id) = float_block(c, shown) {
                    wanted.push((
                        id,
                        NaturalKind::Cobbles,
                        material_of(c, shown).unwrap_or_default(),
                    ));
                }
            } else if let Some((id, kind)) = resource_block(c, shown) {
                wanted.push((id, kind, material_of(c, shown).unwrap_or_default()));
            }
        }
    }
    for (id, kind, material) in wanted {
        if material.is_empty() || out.iter().any(|b| b.id == id) {
            continue;
        }
        let Some(m) = c.materials.get(&material) else {
            continue;
        };
        let hardness = match kind {
            NaturalKind::Ore => hardness_of(m).max(1.8),
            NaturalKind::Rock => hardness_of(m),
            NaturalKind::Placer => 0.6,
            NaturalKind::Crust => 0.5,
            NaturalKind::Cobbles => 0.2,
            NaturalKind::Soil | NaturalKind::Loose => 0.25 + m.density_kg_m3 / 5000.0,
        };
        let name = match kind {
            NaturalKind::Ore => format!("{} ore", m.name),
            NaturalKind::Placer => format!("{}-bearing gravel", m.name),
            NaturalKind::Crust => format!("{} crust", m.name),
            NaturalKind::Cobbles if !id.ends_with("_cobbles") => format!("{} pieces", m.name),
            NaturalKind::Cobbles => format!("{} cobbles", m.name),
            _ => m.name.clone(),
        };
        out.push(NaturalBlock {
            id,
            name,
            material,
            kind,
            hardness,
            map_color: m.appearance.color.0,
        });
    }
    out
}

/// The material a resource (rock, mineral or material id) is made of.
pub fn material_of(c: &crate::Content, resource: &str) -> Option<String> {
    if let Some(r) = c.rocks.get(resource) {
        return Some(r.material.as_str().to_owned());
    }
    if let Some(m) = c.minerals.get(resource) {
        return Some(m.material.as_str().to_owned());
    }
    c.materials.get(resource).map(|m| m.id().to_owned())
}

/// The block a deposit body of `resource` is made of: the rock itself, an earth's block, a
/// rock-like mineral's own block (flint) or the mineral's ore block (`<mineral>_ore`).
pub fn resource_block(c: &crate::Content, resource: &str) -> Option<(String, NaturalKind)> {
    use crate::schema::material::MaterialCategory as Cat;
    if c.rocks.get(resource).is_some() {
        return Some((resource.to_owned(), NaturalKind::Rock));
    }
    let earth = |m: &Material| {
        let falls = m.tags.iter().any(|t| t == "falls");
        (
            m.id().to_owned(),
            if falls {
                NaturalKind::Loose
            } else {
                NaturalKind::Soil
            },
        )
    };
    // Minerals first: a mineral's id is usually also its material's.
    if let Some(mineral) = c.minerals.get(resource) {
        let m = c.materials.get(mineral.material.as_str())?;
        return Some(match m.category {
            Cat::Rock => (resource.to_owned(), NaturalKind::Rock),
            Cat::Soil | Cat::Clay | Cat::Sediment | Cat::Pigment => earth(m),
            _ => (format!("{resource}_ore"), NaturalKind::Ore),
        });
    }
    let m = c.materials.get(resource)?;
    match m.category {
        Cat::Soil | Cat::Clay | Cat::Sediment | Cat::Pigment => Some(earth(m)),
        _ => None,
    }
}

/// The block a deposit body of `resource` with a geometry is made of: as
/// [`resource_block`], except that crusts and bog ores of an ore mineral are an earthy crust of
/// it (`<mineral>_crust`) rather than rock flecked with it.
pub fn body_block(
    c: &crate::Content,
    resource: &str,
    geometry: crate::schema::geology::DepositGeometry,
) -> Option<(String, NaturalKind)> {
    use crate::schema::geology::DepositGeometry as G;
    match resource_block(c, resource)? {
        (_, NaturalKind::Ore) if matches!(geometry, G::Crust | G::Bog) => {
            Some((format!("{resource}_crust"), NaturalKind::Crust))
        }
        other => Some(other),
    }
}

/// What a placer of `resource` leaves in a stream bed: cobbles of a rock, or gravel with the
/// mineral's heavy grains.
pub fn placer_block(c: &crate::Content, resource: &str) -> Option<(String, NaturalKind)> {
    match resource_block(c, resource)? {
        (id, NaturalKind::Rock) => Some((format!("{id}_cobbles"), NaturalKind::Cobbles)),
        (_, NaturalKind::Ore) => Some((format!("{resource}_placer"), NaturalKind::Placer)),
        _ => None,
    }
}

/// Loose pieces of `resource` lying on the surface (float): cobbles of a rock, or pieces of
/// an ore mineral.
pub fn float_block(c: &crate::Content, resource: &str) -> Option<String> {
    match resource_block(c, resource)? {
        (id, NaturalKind::Rock) => Some(format!("{id}_cobbles")),
        (_, NaturalKind::Ore) => Some(format!("{resource}_float")),
        _ => None,
    }
}

#[cfg(test)]
mod natural_tests {
    use super::*;

    #[test]
    fn every_rock_and_earth_gets_a_block() {
        let c = crate::Content::load_base();
        let blocks = natural_blocks(&c);
        for rock in c.rocks.iter() {
            assert!(
                blocks
                    .iter()
                    .any(|b| b.id == rock.id() && b.kind == NaturalKind::Rock),
                "{} has a block",
                rock.id()
            );
        }
        assert!(
            blocks
                .iter()
                .any(|b| b.id == "hearth:granite_cobbles" && b.kind == NaturalKind::Cobbles)
        );
        assert!(
            !blocks.iter().any(|b| b.id == "hearth:rock_gypsum_cobbles"),
            "no loose stones of gypsum (no float indicator asks for them)"
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
        // Deposits bring their ores, placers, float and indicators.
        assert_eq!(kind("hearth:chalcopyrite_ore"), Some(NaturalKind::Ore));
        assert_eq!(
            kind("hearth:malachite_ore"),
            Some(NaturalKind::Ore),
            "stains"
        );
        assert_eq!(kind("hearth:gossan_ore"), Some(NaturalKind::Ore), "gossans");
        assert_eq!(kind("hearth:native_gold_placer"), Some(NaturalKind::Placer));
        assert_eq!(
            kind("hearth:sulfur_crust"),
            Some(NaturalKind::Crust),
            "fumarole crusts"
        );
        assert_eq!(
            kind("hearth:goethite_crust"),
            Some(NaturalKind::Crust),
            "limonite crusts"
        );
        assert_eq!(
            kind("hearth:flint"),
            Some(NaturalKind::Rock),
            "flint nodules are flint"
        );
        assert_eq!(
            kind("hearth:flint_cobbles"),
            Some(NaturalKind::Cobbles),
            "flint float"
        );
        assert_eq!(
            kind("hearth:red_ochre"),
            Some(NaturalKind::Soil),
            "ochre earth"
        );
        // Every deposit's body resolves to a block.
        for d in c.deposits.iter() {
            let (id, _) = body_block(&c, d.resource.as_str(), d.geometry)
                .unwrap_or_else(|| panic!("{} has no block", d.resource));
            assert!(kind(&id).is_some(), "{} has a block {id}", d.id);
        }
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

//! The kinds of items: one for each of the content's items, looked up by id.

use hearth_content::Content;
use hearth_content::schema::body::{BodyRegion, ClothingLayer};
use hearth_content::schema::item::{ContainerSpec, Stacking};
use rustc_hash::FxHashMap;

/// A kind of item.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemKind {
    pub id: String,
    pub name: String,
    pub material: Option<String>,
    pub mass_kg: f32,
    pub volume_l: f32,
    /// Grid cells (wide, high).
    pub footprint: (u8, u8),
    pub stacking: Stacking,
    /// As a thing in the world: its box (m) and colour (sRGB).
    pub size_m: [f32; 3],
    pub color: [u8; 3],
    pub tags: Vec<String>,
    pub properties: Vec<(String, f32)>,
    pub container: Option<ContainerSpec>,
    /// Attachment points it hangs from.
    pub hangs_on: Vec<String>,
    /// How it is worn, if it is a garment.
    pub wear: Option<Wear>,
}

/// A garment's place on the body.
#[derive(Debug, Clone, PartialEq)]
pub struct Wear {
    /// The garment's content id.
    pub garment: String,
    pub layer: ClothingLayer,
    pub regions: Vec<BodyRegion>,
    /// Attachment points it gives (`tie`, `belt_loop`).
    pub attachments: Vec<String>,
}

impl ItemKind {
    /// How many share one grid cell.
    pub fn per_cell(&self) -> u16 {
        match self.stacking {
            Stacking::Count(n) => n.max(1),
            _ => 1,
        }
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn property(&self, name: &str) -> Option<f32> {
        self.properties
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| *v)
    }

    /// Small enough to tuck into a loincloth's tie: one cell and half a kilogram at most.
    pub fn small(&self) -> bool {
        self.footprint == (1, 1) && self.mass_kg <= 0.5
    }

    /// Whether it can hang from an attachment point of this kind.
    pub fn hangs_from(&self, point: &str) -> bool {
        self.hangs_on.iter().any(|p| p == point) || (point == "tie" && self.small())
    }
}

/// Every kind of item there is.
#[derive(Debug, Clone, Default)]
pub struct Items {
    kinds: Vec<ItemKind>,
    index: FxHashMap<String, usize>,
}

impl Items {
    pub fn from_content(c: &Content) -> Self {
        let mut kinds: Vec<ItemKind> = c
            .items
            .iter()
            .map(|d| ItemKind {
                id: d.id.clone(),
                name: d.name.clone(),
                material: d.material.clone(),
                mass_kg: d.mass_kg,
                volume_l: d.volume_l,
                footprint: (d.footprint.0.max(1), d.footprint.1.max(1)),
                stacking: d.stacking,
                size_m: d.size_m,
                color: d.color,
                tags: d.tags.clone(),
                properties: d.properties.clone(),
                container: d.container,
                hangs_on: d.hangs_on.clone(),
                wear: d
                    .garment
                    .as_deref()
                    .and_then(|g| c.garments.get(g))
                    .map(|g| Wear {
                        garment: g.id.to_string(),
                        layer: g.layer,
                        regions: g.regions.clone(),
                        attachments: g.attachments.clone(),
                    }),
            })
            .collect();
        kinds.sort_by(|a, b| a.id.cmp(&b.id));
        let index = kinds
            .iter()
            .enumerate()
            .map(|(i, k)| (k.id.clone(), i))
            .collect();
        Self { kinds, index }
    }

    pub fn get(&self, id: &str) -> Option<&ItemKind> {
        self.index.get(id).map(|&i| &self.kinds[i])
    }

    pub fn iter(&self) -> impl Iterator<Item = &ItemKind> {
        self.kinds.iter()
    }

    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    /// The kind of a garment of this material (`hearth:loincloth` of `hearth:rawhide`).
    pub fn garment(&self, garment: &str, material: &str) -> Option<&ItemKind> {
        self.iter().find(|k| {
            k.wear.as_ref().is_some_and(|w| w.garment == garment)
                && k.material.as_deref() == Some(material)
        })
    }
}

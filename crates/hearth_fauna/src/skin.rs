//! Coats on the bodies (V2-7, v2 §7.2): every species' boxes unwrapped side by side in one
//! texture, and its coats painted there by `hearth_texgen::coats` from the boxes as they lie at
//! rest — the female's, the winter coat's where it differs, the male's where his colour does,
//! and the young's where they wear a pattern of their own (a fawn's spots, a piglet's stripes).
//! A body's boxes are drawn with their unwraps' places in the texture.

use glam::Affine3A;
use hearth_content::schema::fauna::{BodyPlan, Coat, CoatPattern};
use hearth_math::hash::hash2;
use hearth_texgen::coats::{CoatRecipe, Pattern, SkinBox, paint_coat, unwrap_size};
use hearth_texgen::paint::Tex;

use crate::anim::{Pose, rest_pose};
use crate::live::Stage;
use crate::rig::Rig;
use crate::species::{Catalog, Species};

/// The coats a species may have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Female = 0,
    Winter = 1,
    Male = 2,
    Young = 3,
    /// A bred fleece (V2-12: a woolly line's coat).
    Fleece = 4,
}

const VARIANTS: usize = 5;

/// Where a species' coats lie in the atlas: each box's unwrap within a coat's block and its
/// size in pixels, and where each coat's block is.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeciesSkin {
    pub boxes: Vec<[u32; 2]>,
    pub dims: Vec<[u32; 3]>,
    pub size: [u32; 2],
    pub blocks: [Option<[u32; 2]>; VARIANTS],
}

/// Every species' bodies (a female's and a male's) and their coats in one texture.
#[derive(Debug, Clone)]
pub struct Bodies {
    pub rigs: Vec<[Rig; 2]>,
    pub skins: Vec<SpeciesSkin>,
    pub atlas: Tex,
}

/// The atlas's width.
const ATLAS_W: u32 = 1024;

/// Packs rectangles in rows across a width: each one's place, and the height used.
fn shelf(sizes: &[[u32; 2]], width: u32) -> (Vec<[u32; 2]>, u32) {
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse((sizes[i][1], sizes[i][0])));
    let mut out = vec![[0, 0]; sizes.len()];
    let (mut x, mut y, mut row) = (0u32, 0u32, 0u32);
    for i in order {
        let [w, h] = sizes[i];
        if x + w > width && x > 0 {
            y += row;
            x = 0;
            row = 0;
        }
        out[i] = [x, y];
        x += w;
        row = row.max(h);
    }
    (out, y + row)
}

impl Bodies {
    pub fn new(cat: &Catalog) -> Self {
        let rigs: Vec<[Rig; 2]> = cat
            .species
            .iter()
            .map(|sp| [Rig::of(sp, false), Rig::of(sp, true)])
            .collect();
        // Each species' boxes in a block (the male's body, which carries everything).
        let mut skins = Vec::with_capacity(rigs.len());
        let mut blocks: Vec<(usize, Variant, [u32; 2])> = Vec::new();
        for (si, pair) in rigs.iter().enumerate() {
            let rig = &pair[1];
            let dims: Vec<[u32; 3]> = rig.boxes.iter().map(|b| rig.pixels(b)).collect();
            let sizes: Vec<[u32; 2]> = dims.iter().map(|d| unwrap_size(*d)).collect();
            let area: u32 = sizes.iter().map(|s| s[0] * s[1]).sum();
            let widest = sizes.iter().map(|s| s[0]).max().unwrap_or(1);
            let width = ((area as f32 * 1.3).sqrt().ceil() as u32)
                .max(widest)
                .max(16);
            let (boxes, height) = shelf(&sizes, width);
            let size = [
                boxes
                    .iter()
                    .zip(&sizes)
                    .map(|(p, s)| p[0] + s[0])
                    .max()
                    .unwrap_or(1),
                height.max(1),
            ];
            let sp = &cat.species[si];
            for v in variants_of(sp) {
                blocks.push((si, v, size));
            }
            skins.push(SpeciesSkin {
                boxes,
                dims,
                size,
                blocks: [None; VARIANTS],
            });
        }
        // The blocks across the atlas.
        let block_sizes: Vec<[u32; 2]> = blocks.iter().map(|b| b.2).collect();
        let width = ATLAS_W.max(block_sizes.iter().map(|s| s[0]).max().unwrap_or(1));
        let (places, height) = shelf(&block_sizes, width);
        for ((si, v, _), at) in blocks.iter().zip(&places) {
            skins[*si].blocks[*v as usize] = Some(*at);
        }
        let mut atlas = Tex::new(width, height.max(1));
        // The coats, painted on the bodies at rest.
        for (si, pair) in rigs.iter().enumerate() {
            let rig = &pair[1];
            let sp = &cat.species[si];
            let rest = rest_pose(rig);
            let placed = rest.boxes(rig);
            for v in variants_of(sp) {
                let Some(origin) = skins[si].blocks[v as usize] else {
                    continue;
                };
                let skin = &skins[si];
                let boxes: Vec<SkinBox> = placed
                    .iter()
                    .map(|(i, place)| SkinBox {
                        origin: [origin[0] + skin.boxes[*i][0], origin[1] + skin.boxes[*i][1]],
                        dims: skin.dims[*i],
                        rest: *place,
                        part: rig.boxes[*i].part,
                    })
                    .collect();
                paint_coat(&mut atlas, &boxes, &recipe(sp, rig, v));
            }
        }
        Self { rigs, skins, atlas }
    }

    /// A species' body for a sex.
    pub fn rig(&self, species: usize, female: bool) -> &Rig {
        &self.rigs[species][if female { 0 } else { 1 }]
    }

    /// Where an animal's coat lies in the atlas: a grown male's own where his colour differs,
    /// the winter coat in winter, the young's pattern on the young of the year (in its first
    /// summer), otherwise the female's.
    pub fn coat_of(
        &self,
        species: usize,
        female: bool,
        stage: Stage,
        winter: bool,
        fleece: f32,
    ) -> [u32; 2] {
        let s = &self.skins[species];
        let mut order = Vec::with_capacity(4);
        // A woolly line's fleece, grown (V2-12).
        if fleece > 0.5 {
            order.push(Variant::Fleece);
        }
        if stage == Stage::Adult && !female {
            order.push(Variant::Male);
        }
        if winter {
            order.push(Variant::Winter);
        } else if stage == Stage::Young {
            order.push(Variant::Young);
        }
        order.push(Variant::Female);
        order
            .iter()
            .find_map(|v| s.blocks[*v as usize])
            .unwrap_or([0, 0])
    }

    /// The boxes of a posed body with their coat's places: each box's placement (the unit cube
    /// to the body's frame) and the two words the figure shader reads its coat from.
    pub fn skinned(
        &self,
        species: usize,
        rig: &Rig,
        pose: &Pose,
        coat: [u32; 2],
    ) -> Vec<(Affine3A, [u32; 2])> {
        let skin = &self.skins[species];
        pose.boxes(rig)
            .into_iter()
            .map(|(i, place)| {
                let [u, v] = skin.boxes[i];
                let [w, h, d] = skin.dims[i];
                let at = (coat[0] + u) | ((coat[1] + v) << 16);
                let dims = w | (h << 8) | (d << 16) | (1 << 31);
                (place, [at, dims])
            })
            .collect()
    }
}

/// Whether a coat is the winter one at a time of year: from late autumn to early spring.
pub fn winter_coat(year_frac: f32, southern: bool) -> bool {
    let f = if southern {
        (year_frac + 0.5).rem_euclid(1.0)
    } else {
        year_frac
    };
    !(0.05..0.6).contains(&f)
}

/// The coats a species wears.
fn variants_of(sp: &Species) -> Vec<Variant> {
    let mut v = vec![Variant::Female];
    if let Some(c) = &sp.coat {
        if c.winter.is_some() {
            v.push(Variant::Winter);
        }
        if c.male.is_some() {
            v.push(Variant::Male);
        }
        if c.young.is_some() {
            v.push(Variant::Young);
        }
    }
    if fleece_coat(sp).is_some() {
        v.push(Variant::Fleece);
    }
    v
}

/// A bred fleece's colour, for a kind whose bred form grows one.
fn fleece_coat(sp: &Species) -> Option<[u8; 3]> {
    let d = sp.domestication.as_ref()?;
    d.fleece_kg?;
    d.bred_coat.map(|c| c.0)
}

fn pattern_of(p: CoatPattern) -> Pattern {
    match p {
        CoatPattern::Plain => Pattern::Plain,
        CoatPattern::Spotted => Pattern::Spotted,
        CoatPattern::Striped => Pattern::Striped,
        CoatPattern::Grizzled => Pattern::Grizzled,
        CoatPattern::Masked => Pattern::Masked,
        CoatPattern::Speckled => Pattern::Speckled,
        CoatPattern::Pied => Pattern::Pied,
        CoatPattern::ZigZag => Pattern::ZigZag,
        CoatPattern::Banded => Pattern::Banded,
        CoatPattern::FlankBand => Pattern::FlankBand,
    }
}

/// The recipe of a species' coat.
fn recipe(sp: &Species, rig: &Rig, v: Variant) -> CoatRecipe {
    let grey = Coat {
        base: hearth_content::schema::Color([120, 110, 100]),
        belly: hearth_content::schema::Color([170, 160, 150]),
        points: None,
        rump: None,
        pattern: CoatPattern::Plain,
        marking: None,
        face: None,
        legs: None,
        tail_tip: None,
        winter: None,
        male: None,
        young: None,
    };
    let c = sp.coat.unwrap_or(grey);
    let fleece = fleece_coat(sp).filter(|_| v == Variant::Fleece);
    let back = match v {
        Variant::Winter => c.winter.unwrap_or(c.base).0,
        Variant::Male => c.male.unwrap_or(c.base).0,
        Variant::Fleece => fleece.unwrap_or(c.base.0),
        _ => c.base.0,
    };
    // A fleece covers the body's markings: one colour over back and belly, the face and legs
    // their own.
    if let Some(f) = fleece {
        return CoatRecipe {
            kind: rig.kind,
            back: f,
            belly: f,
            points: f,
            rump: None,
            marking: None,
            face: c.face.map(|p| p.0),
            legs: c.legs.map(|p| p.0),
            tail_tip: None,
            pattern: Pattern::Plain,
            hooves: sp.plan == BodyPlan::Ungulate,
            seed: hash2(hash2(0xc0a7, sp.index as u64), v as u64),
        };
    }
    let pattern = match (v, c.young) {
        (Variant::Young, Some(p)) => p,
        _ => c.pattern,
    };
    // The young's spots and stripes are pale.
    let marking = match (v, pattern) {
        (Variant::Young, CoatPattern::Spotted | CoatPattern::Striped) => {
            Some(c.marking.map_or([238, 232, 218], |m| m.0))
        }
        _ => c.marking.map(|m| m.0),
    };
    CoatRecipe {
        kind: rig.kind,
        back,
        belly: c.belly.0,
        points: c.points.map_or(back, |p| p.0),
        rump: c.rump.map(|p| p.0),
        marking,
        face: c.face.map(|p| p.0),
        legs: c.legs.map(|p| p.0),
        tail_tip: c.tail_tip.map(|p| p.0),
        pattern: pattern_of(pattern),
        hooves: sp.plan == BodyPlan::Ungulate,
        seed: hash2(hash2(0xc0a7, sp.index as u64), v as u64),
    }
}

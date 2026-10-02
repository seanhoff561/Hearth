//! Construction pieces in the world (V2-8 (a), docs/design/building.md): generated from the
//! `construction/` pieces and the materials they may be made of, not enumerated. Each piece and
//! material is a block (`hearth:post/hazel_wood`) shaped as the piece sits in a block — a post
//! upright in the middle, a beam across the top, a panel or a wall at the side it faces, a layer
//! on the floor, a roof rising toward the way it faces — and each piece has a way of putting it
//! up (from what it uses, taking real time, needing what knowledge it needs) and of taking it
//! down again for most of what it took.

use crate::content::Table;
use crate::id::IdRef;
use crate::schema::knowledge::Knowledge;
use crate::schema::material::Material;
use crate::schema::process::{BlockMatch, Effect, Match, Output, Process, Quality, Target};
use crate::schema::station::{ConstructionPiece, PieceShape};
use crate::schema::{Duration, Entry, Status};

/// The block of a piece in a material (`hearth:post/hazel_wood`).
pub fn piece_block_id(piece: &str, material: &str) -> String {
    let (ns, path) = piece.split_once(':').unwrap_or(("hearth", piece));
    let m = material.split_once(':').map_or(material, |(_, p)| p);
    format!("{ns}:{path}/{m}")
}

/// The piece and material a block's name (with or without its namespace) names, if it is a
/// piece's block: (`post`, `hazel_wood`).
pub fn piece_of(block: &str) -> Option<(&str, &str)> {
    let path = block.split_once(':').map_or(block, |(_, p)| p);
    path.split_once('/')
}

/// The process that puts a piece up, and that takes it down.
pub fn place_id(piece: &str) -> String {
    let (ns, path) = piece.split_once(':').unwrap_or(("hearth", piece));
    format!("{ns}:place_{path}")
}

pub fn take_down_id(piece: &str) -> String {
    let (ns, path) = piece.split_once(':').unwrap_or(("hearth", piece));
    format!("{ns}:take_down_{path}")
}

/// How thick a piece looks in its block (sixteenths): its thinnest side, at least one (a post
/// or a beam at least two, to read as a member at all: a sapling pole is under a sixteenth).
pub fn thickness_px(p: &ConstructionPiece) -> f64 {
    let thin = p.size_m.iter().copied().fold(f32::INFINITY, f32::min);
    let least = match p.shape {
        PieceShape::Post | PieceShape::Beam => 2.0,
        _ => 1.0,
    };
    (thin as f64 * 16.0).round().clamp(least, 16.0)
}

/// A piece's boxes in its block (sixteenths), as it sits facing north: a post upright in the
/// middle, a beam across the top from north to south, a panel at the north side, a layer on the
/// floor, a roof rising in eight steps toward the north, a wall the north half.
pub fn boxes(shape: PieceShape, t: f64) -> Vec<[f64; 6]> {
    let (a, b) = (8.0 - t / 2.0, 8.0 + t / 2.0);
    match shape {
        PieceShape::Post => vec![[a, 0.0, a, b, 16.0, b]],
        PieceShape::Beam => vec![[a, 16.0 - t, 0.0, b, 16.0, 16.0]],
        PieceShape::Panel => vec![[0.0, 0.0, 0.0, 16.0, 16.0, t]],
        PieceShape::Layer => vec![[0.0, 0.0, 0.0, 16.0, t, 16.0]],
        PieceShape::Roof => {
            let step = 2.0;
            (0..8)
                .map(|k| {
                    let k = k as f64;
                    let z1 = 16.0 - step * k;
                    let y0 = step * k;
                    [0.0, y0, z1 - step, 16.0, (y0 + t.max(step)).min(16.0), z1]
                })
                .collect()
        }
        PieceShape::Wall => vec![[0.0, 0.0, 0.0, 16.0, 16.0, 8.0]],
        PieceShape::Block => vec![[0.0, 0.0, 0.0, 16.0, 16.0, 16.0]],
    }
}

/// Whether a piece's block turns with the way it faces (a post and a whole block do not).
pub fn faces(shape: PieceShape) -> bool {
    !matches!(shape, PieceShape::Post | PieceShape::Block)
}

/// What is at a place beside a piece, as the piece bears on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bearing {
    /// Air, or what gives way (grass, snow lying).
    Nothing,
    /// The ground, or anything solid that is not a piece (a log, a boulder).
    Ground,
    /// A piece of this shape.
    Piece(PieceShape),
}

/// Whether a piece of `shape` put up rests on what is below it and beside it: the stages of
/// building, a frame before what hangs on it. A post stands on the ground or on what stands; a
/// wall or a whole block on the ground, a wall or a beam (a lintel over a door); a panel on
/// anything below or lashed to a post beside it; a beam (lying across the top of its block) from
/// a post, a wall, the ground or another beam beside it; a roof or a layer on anything below, or
/// from the frame, the bank or the slope beside it. Whether it is strong enough there is the
/// frame's reckoning (V2-8 (b)).
pub fn rests(shape: PieceShape, below: Bearing, beside: [Bearing; 4]) -> bool {
    use Bearing::{Ground, Nothing, Piece};
    use PieceShape as S;
    let by = |ok: fn(Bearing) -> bool| beside.iter().any(|b| ok(*b));
    match shape {
        S::Post => matches!(below, Ground | Piece(S::Post | S::Wall | S::Block)),
        S::Wall | S::Block => matches!(below, Ground | Piece(S::Wall | S::Block | S::Beam)),
        S::Panel => below != Nothing || by(|b| matches!(b, Piece(S::Post | S::Panel))),
        S::Beam => by(|b| matches!(b, Ground | Piece(S::Post | S::Wall | S::Block | S::Beam))),
        S::Roof | S::Layer => below != Nothing || by(|b| b != Nothing),
    }
}

/// A piece's block, as the world's blocks are made from it.
#[derive(Debug, Clone, PartialEq)]
pub struct PieceBlock {
    pub id: String,
    pub piece: String,
    pub material: String,
    pub shape: PieceShape,
    pub boxes: Vec<[f64; 6]>,
    pub map_color: [u8; 3],
    /// How it sounds underfoot and struck.
    pub sound: &'static str,
    /// Resistance to taking down by force.
    pub hardness: f32,
    pub flammable: bool,
    pub status: Status,
}

/// The blocks of every piece in every material it may be made of.
pub fn piece_blocks(
    pieces: &Table<ConstructionPiece>,
    materials: &Table<Material>,
) -> Vec<PieceBlock> {
    use crate::schema::material::MaterialCategory as Cat;
    let mut out = Vec::new();
    for p in pieces.iter() {
        let t = thickness_px(p);
        for m in materials.iter().filter(|m| p.materials.matches(m.id(), m)) {
            let sound = match m.category {
                Cat::Wood | Cat::Bark => "wood",
                Cat::Rock => "stone",
                Cat::Clay | Cat::Soil | Cat::Sediment => "soil",
                _ if m.tags.iter().any(|t| t == "hide") => "wool",
                _ if m.tags.iter().any(|t| t == "snow") => "snow",
                _ => "grass",
            };
            let flammable = matches!(m.category, Cat::Wood | Cat::Bark)
                || m.fuel_mj_kg.is_some_and(|f| f > 5.0);
            out.push(PieceBlock {
                id: piece_block_id(p.id(), m.id()),
                piece: p.id().to_owned(),
                material: m.id().to_owned(),
                shape: p.shape,
                boxes: boxes(p.shape, t),
                map_color: m.appearance.color.0,
                sound,
                hardness: if sound == "stone" { 2.0 } else { 0.8 },
                flammable,
                status: p.status,
            });
        }
    }
    out
}

/// The noun of a piece ("post" from "{material} post").
fn noun(p: &ConstructionPiece) -> String {
    p.name.replace("{material}", "").trim().to_owned()
}

/// "A" or "an" by the sound of a word.
fn article(word: &str) -> &'static str {
    crate::butchery::article(word)
}

/// Generates every piece's ways of putting it up and taking it down, adding them to
/// `processes`; the knowledge a piece needs lists its putting up as what it enables.
pub fn generate(
    pieces: &Table<ConstructionPiece>,
    processes: &mut Table<Process>,
    knowledge: &mut Table<Knowledge>,
) {
    for (p, origin) in pieces.iter_with_origin() {
        if p.inputs.is_empty() {
            continue;
        }
        let n = noun(p);
        let action = p
            .action
            .clone()
            .unwrap_or_else(|| format!("put up {} {n}", article(&n)));
        let mut name = action.clone();
        if let Some(c) = name.get_mut(0..1) {
            c.make_ascii_uppercase();
        }
        let put_up = Process {
            id: place_id(p.id()),
            name,
            action,
            verb: Some("build".into()),
            target: Some(Target::Ground),
            effect: Effect::Place,
            inputs: p.inputs.clone(),
            tools: p.tools.clone(),
            station: None,
            conditions: Vec::new(),
            duration: p.build,
            knowledge: p.knowledge.clone(),
            skill: Some("building".into()),
            outputs: Vec::new(),
            byproducts: Vec::new(),
            failures: Vec::new(),
            teaches: Vec::new(),
            attended: true,
            mets: p.mets,
            wear: 0.0,
            harvests: None,
            treats: None,
            places: Some(IdRef(p.id().to_owned())),
            status: p.status,
            notes: None,
            realism_source: None,
            uncertain: false,
        };
        // Taken down, most of what went into it comes back (a little is broken or lost).
        let outputs = p
            .inputs
            .iter()
            .map(|i| Output {
                item: match &i.item {
                    Match::Form { form, .. } => Match::Form {
                        form: form.clone(),
                        materials: None,
                    },
                    other => other.clone(),
                },
                amount: ((i.amount * 0.8).floor().max(1.0), i.amount),
                chance: 1.0,
                quality: Quality::default(),
                seasons: Vec::new(),
            })
            .collect();
        let path = p.id().split_once(':').map_or(p.id(), |(_, p)| p);
        let take_down = Process {
            id: take_down_id(p.id()),
            name: format!("Take down the {n}"),
            action: format!("take down the {n}"),
            verb: Some("take".into()),
            target: Some(Target::Block(BlockMatch::Prefix(format!("{path}/")))),
            effect: Effect::Remove,
            inputs: Vec::new(),
            tools: Vec::new(),
            station: None,
            conditions: Vec::new(),
            duration: Duration {
                hours: p.build.hours * 0.4,
                scale: p.build.scale,
            },
            knowledge: None,
            skill: None,
            outputs,
            byproducts: Vec::new(),
            failures: Vec::new(),
            teaches: Vec::new(),
            attended: true,
            mets: p.mets,
            wear: 0.0,
            harvests: None,
            treats: None,
            places: None,
            status: p.status,
            notes: None,
            realism_source: None,
            uncertain: false,
        };
        if let Some(k) = p
            .knowledge
            .as_ref()
            .and_then(|k| knowledge.get_mut(k.as_str()))
        {
            let id = IdRef(put_up.id.clone());
            if !k.enables.contains(&id) {
                k.enables.push(id);
            }
        }
        processes.upsert(put_up.id.clone(), put_up, origin.clone());
        processes.upsert(take_down.id.clone(), take_down, origin.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        let id = piece_block_id("hearth:post", "hearth:hazel_wood");
        assert_eq!(id, "hearth:post/hazel_wood");
        assert_eq!(piece_of(&id), Some(("post", "hazel_wood")));
        assert_eq!(piece_of("hearth:granite"), None);
        assert_eq!(place_id("hearth:post"), "hearth:place_post");
    }

    #[test]
    fn a_frame_goes_up_before_what_hangs_on_it() {
        use Bearing::{Ground, Nothing, Piece};
        let none = [Nothing; 4];
        assert!(rests(PieceShape::Post, Ground, none));
        assert!(!rests(PieceShape::Post, Nothing, none), "a post in the air");
        // A beam from a post beside it, not in the air nor on a post's top.
        let post = [Nothing, Piece(PieceShape::Post), Nothing, Nothing];
        assert!(rests(PieceShape::Beam, Nothing, post));
        assert!(!rests(PieceShape::Beam, Piece(PieceShape::Post), none));
        // A roof from the beam beside it, or on the ground; not alone in the air.
        let beam = [Piece(PieceShape::Beam), Nothing, Nothing, Nothing];
        assert!(rests(PieceShape::Roof, Nothing, beam));
        assert!(rests(PieceShape::Roof, Ground, none));
        assert!(!rests(PieceShape::Roof, Nothing, none));
        // A dry-stone wall on the ground or a lintel, not on a roof.
        assert!(rests(PieceShape::Wall, Piece(PieceShape::Beam), none));
        assert!(!rests(PieceShape::Wall, Piece(PieceShape::Roof), none));
    }

    #[test]
    fn a_roof_rises_toward_the_way_it_faces() {
        let b = boxes(PieceShape::Roof, 2.0);
        assert_eq!(b.len(), 8);
        // The step at the south is lowest, the one at the north highest.
        let south = b.iter().find(|x| x[5] == 16.0).expect("south");
        let north = b.iter().find(|x| x[2] == 0.0).expect("north");
        assert!(north[1] > south[1]);
    }
}

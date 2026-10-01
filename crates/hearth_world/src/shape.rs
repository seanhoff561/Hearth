//! Block shapes: collision and outline boxes in block-local coordinates (0..1), computed per
//! block state from a shape family and the state's property values.

use glam::DVec3;
use hearth_math::{Aabb, Direction};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// Index into the shared shape table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ShapeId(pub u16);

impl ShapeId {
    pub const EMPTY: ShapeId = ShapeId(0);
    pub const FULL: ShapeId = ShapeId(1);
}

/// A shape made of up to a few boxes in block-local space.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Shape {
    pub boxes: SmallVec<[Aabb; 4]>,
}

impl Shape {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn full() -> Self {
        Self::from_px(&[[0.0, 0.0, 0.0, 16.0, 16.0, 16.0]])
    }

    /// Builds a shape from boxes given in sixteenths of a block (`[x0, y0, z0, x1, y1, z1]`).
    pub fn from_px(boxes: &[[f64; 6]]) -> Self {
        Self {
            boxes: boxes
                .iter()
                .map(|b| {
                    Aabb::from_coords(
                        b[0] / 16.0,
                        b[1] / 16.0,
                        b[2] / 16.0,
                        b[3] / 16.0,
                        b[4] / 16.0,
                        b[5] / 16.0,
                    )
                })
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty()
    }

    /// True if the shape is exactly the unit cube.
    pub fn is_full_cube(&self) -> bool {
        self.boxes.len() == 1 && self.boxes[0].min == DVec3::ZERO && self.boxes[0].max == DVec3::ONE
    }

    /// Bounding box of all boxes (unit cube if empty).
    pub fn bounds(&self) -> Aabb {
        let mut it = self.boxes.iter();
        match it.next() {
            Some(first) => it.fold(*first, |acc, b| acc.union(b)),
            None => Aabb::from_coords(0.0, 0.0, 0.0, 1.0, 1.0, 1.0),
        }
    }

    /// Maximum Y of the shape (0 if empty).
    pub fn top(&self) -> f64 {
        self.boxes.iter().map(|b| b.max.y).fold(0.0, f64::max)
    }

    /// True if the face on side `d` is fully covered by the union of the boxes touching that
    /// side (a "sturdy face" for support checks). Exact for shapes on a 1/16 grid.
    pub fn covers_face(&self, d: Direction) -> bool {
        let mut covered = [false; 256];
        for b in &self.boxes {
            let touches = match d {
                Direction::Down => b.min.y <= 0.0,
                Direction::Up => b.max.y >= 1.0,
                Direction::North => b.min.z <= 0.0,
                Direction::South => b.max.z >= 1.0,
                Direction::West => b.min.x <= 0.0,
                Direction::East => b.max.x >= 1.0,
            };
            if !touches {
                continue;
            }
            // The two axes spanning the face.
            let (u0, u1, v0, v1) = match d.axis() {
                hearth_math::Axis::Y => (b.min.x, b.max.x, b.min.z, b.max.z),
                hearth_math::Axis::X => (b.min.z, b.max.z, b.min.y, b.max.y),
                hearth_math::Axis::Z => (b.min.x, b.max.x, b.min.y, b.max.y),
            };
            let cell = |v: f64| (v * 16.0).round().clamp(0.0, 16.0) as usize;
            for v in cell(v0)..cell(v1) {
                for u in cell(u0)..cell(u1) {
                    covered[v * 16 + u] = true;
                }
            }
        }
        covered.iter().all(|c| *c)
    }

    /// Rotates the shape around the vertical axis so that a shape authored facing North faces
    /// `facing`.
    pub fn rotated(&self, facing: Direction) -> Shape {
        let rot = |p: DVec3| -> DVec3 {
            match facing {
                Direction::North => p,
                Direction::South => DVec3::new(1.0 - p.x, p.y, 1.0 - p.z),
                Direction::East => DVec3::new(1.0 - p.z, p.y, p.x),
                Direction::West => DVec3::new(p.z, p.y, 1.0 - p.x),
                _ => p,
            }
        };
        Shape {
            boxes: self
                .boxes
                .iter()
                .map(|b| Aabb::new(rot(b.min), rot(b.max)))
                .collect(),
        }
    }

    fn with(mut self, other: &Shape) -> Shape {
        self.boxes.extend(other.boxes.iter().copied());
        self
    }
}

/// Shape families known to the engine. Data files pick one per block; the concrete shape of
/// each state is derived from its property values.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    /// Full cube.
    #[default]
    Full,
    /// No collision and a full-cube outline (e.g. unknown/structure-void style).
    Empty,
    /// `type` = bottom | top | double.
    Slab,
    /// `facing`, `half`, `shape`.
    Stairs,
    /// Boolean `north/east/south/west` connections; 1.5 blocks tall collision.
    Fence,
    /// `facing`, `open`.
    FenceGate,
    /// Boolean connections; thin panes.
    Pane,
    /// `facing`, `open`, `hinge`.
    Door,
    /// `facing`, `open`, `half`.
    Trapdoor,
    /// `layers` 1..8.
    SnowLayer,
    /// 15/16 tall.
    Farmland,
    /// 1/16 tall.
    Carpet,
    /// 9/16 tall.
    Bed,
    /// Inset by 1/16 horizontally.
    Cactus,
    /// Inset by 1/16, 14/16 tall.
    Chest,
    /// Small box (`hanging` optional).
    Lantern,
    /// Thin panel against the wall behind `facing`.
    Ladder,
    /// `bites` 0..6.
    Cake,
    /// 14/16 tall collision (mud).
    Sunken,
    /// 1.5/16 tall pad.
    LilyPad,
    /// A tree's limb: `thickness` 2–16 px, joined to its `north/south/east/west/up/down`
    /// neighbours. Limbs 8 px and thicker are solid; thinner ones are passed through.
    Branch,
    /// Custom boxes in sixteenths.
    Boxes(Vec<[f64; 6]>),
}

/// Read-only access to a state's property values while computing its shape.
pub trait PropertyLookup {
    fn value(&self, name: &str) -> Option<&str>;
    fn flag(&self, name: &str) -> bool {
        self.value(name) == Some("true")
    }
    fn int(&self, name: &str) -> i64 {
        self.value(name).and_then(|v| v.parse().ok()).unwrap_or(0)
    }
    fn dir(&self, name: &str) -> Direction {
        self.value(name)
            .and_then(Direction::from_name)
            .unwrap_or(Direction::North)
    }
}

/// A limb's boxes: the core and an arm to each joined face.
fn branch_shape(props: &dyn PropertyLookup) -> Shape {
    let t = props.int("thickness").clamp(1, 16) as f64;
    let (a, b) = (8.0 - t / 2.0, 8.0 + t / 2.0);
    let mut boxes = vec![[a, a, a, b, b, b]];
    for (name, arm) in [
        ("down", [a, 0.0, a, b, a, b]),
        ("up", [a, b, a, b, 16.0, b]),
        ("north", [a, a, 0.0, b, b, a]),
        ("south", [a, a, b, b, b, 16.0]),
        ("west", [0.0, a, a, a, b, b]),
        ("east", [b, a, a, 16.0, b, b]),
    ] {
        if props.flag(name) {
            boxes.push(arm);
        }
    }
    Shape::from_px(&boxes)
}

/// Computes (collision, outline) shapes for one state.
pub fn shapes_for(kind: &ShapeKind, props: &dyn PropertyLookup) -> (Shape, Shape) {
    match kind {
        ShapeKind::Full => (Shape::full(), Shape::full()),
        ShapeKind::Empty => (Shape::empty(), Shape::full()),
        ShapeKind::Slab => {
            let s = match props.value("type") {
                Some("top") => Shape::from_px(&[[0.0, 8.0, 0.0, 16.0, 16.0, 16.0]]),
                Some("double") => Shape::full(),
                _ => Shape::from_px(&[[0.0, 0.0, 0.0, 16.0, 8.0, 16.0]]),
            };
            (s.clone(), s)
        }
        ShapeKind::Stairs => {
            let s = stairs_shape(props);
            (s.clone(), s)
        }
        ShapeKind::Fence => {
            let post = Shape::from_px(&[[6.0, 0.0, 6.0, 10.0, 24.0, 10.0]]);
            let mut collision = post;
            let mut outline = Shape::from_px(&[[6.0, 0.0, 6.0, 10.0, 16.0, 10.0]]);
            for d in Direction::HORIZONTAL {
                if props.flag(d.name()) {
                    let arm = Shape::from_px(&[[7.0, 0.0, 0.0, 9.0, 24.0, 6.0]]).rotated(d);
                    let arm_outline = Shape::from_px(&[[7.0, 6.0, 0.0, 9.0, 15.0, 6.0]]).rotated(d);
                    collision = collision.with(&arm);
                    outline = outline.with(&arm_outline);
                }
            }
            (collision, outline)
        }
        ShapeKind::FenceGate => {
            let facing = props.dir("facing");
            let base = Shape::from_px(&[[0.0, 0.0, 6.0, 16.0, 24.0, 10.0]]).rotated(facing);
            let outline = Shape::from_px(&[[0.0, 0.0, 6.0, 16.0, 16.0, 10.0]]).rotated(facing);
            if props.flag("open") {
                (Shape::empty(), outline)
            } else {
                (base, outline)
            }
        }
        ShapeKind::Pane => {
            let mut s = Shape::from_px(&[[7.0, 0.0, 7.0, 9.0, 16.0, 9.0]]);
            for d in Direction::HORIZONTAL {
                if props.flag(d.name()) {
                    s = s.with(&Shape::from_px(&[[7.0, 0.0, 0.0, 9.0, 16.0, 7.0]]).rotated(d));
                }
            }
            (s.clone(), s)
        }
        ShapeKind::Door => {
            let facing = props.dir("facing");
            // Closed: a 3/16 panel on the side opposite to `facing` (the door faces outward).
            let side = if props.flag("open") {
                if props.value("hinge") == Some("right") {
                    facing.rotate_ccw()
                } else {
                    facing.rotate_cw()
                }
            } else {
                facing
            };
            let s = Shape::from_px(&[[0.0, 0.0, 13.0, 16.0, 16.0, 16.0]]).rotated(side);
            (s.clone(), s)
        }
        ShapeKind::Trapdoor => {
            let s = if props.flag("open") {
                Shape::from_px(&[[0.0, 0.0, 13.0, 16.0, 16.0, 16.0]]).rotated(props.dir("facing"))
            } else if props.value("half") == Some("top") {
                Shape::from_px(&[[0.0, 13.0, 0.0, 16.0, 16.0, 16.0]])
            } else {
                Shape::from_px(&[[0.0, 0.0, 0.0, 16.0, 3.0, 16.0]])
            };
            (s.clone(), s)
        }
        ShapeKind::SnowLayer => {
            let layers = props.int("layers").clamp(1, 8) as f64;
            let collision = if layers <= 1.0 {
                Shape::empty()
            } else {
                Shape::from_px(&[[0.0, 0.0, 0.0, 16.0, (layers - 1.0) * 2.0, 16.0]])
            };
            let outline = Shape::from_px(&[[0.0, 0.0, 0.0, 16.0, layers * 2.0, 16.0]]);
            (collision, outline)
        }
        ShapeKind::Farmland => {
            let s = Shape::from_px(&[[0.0, 0.0, 0.0, 16.0, 15.0, 16.0]]);
            (s.clone(), s)
        }
        ShapeKind::Carpet => {
            let s = Shape::from_px(&[[0.0, 0.0, 0.0, 16.0, 1.0, 16.0]]);
            (s.clone(), s)
        }
        ShapeKind::Bed => {
            let s = Shape::from_px(&[[0.0, 0.0, 0.0, 16.0, 9.0, 16.0]]);
            (s.clone(), s)
        }
        ShapeKind::Cactus => (
            Shape::from_px(&[[1.0, 0.0, 1.0, 15.0, 15.0, 15.0]]),
            Shape::from_px(&[[1.0, 0.0, 1.0, 15.0, 16.0, 15.0]]),
        ),
        ShapeKind::Chest => {
            let s = Shape::from_px(&[[1.0, 0.0, 1.0, 15.0, 14.0, 15.0]]);
            (s.clone(), s)
        }
        ShapeKind::Branch => {
            let s = branch_shape(props);
            if props.int("thickness") >= 8 {
                (s.clone(), s)
            } else {
                (Shape::empty(), s)
            }
        }
        ShapeKind::Lantern => {
            let s = if props.flag("hanging") {
                Shape::from_px(&[[5.0, 1.0, 5.0, 11.0, 10.0, 11.0]])
            } else {
                Shape::from_px(&[[5.0, 0.0, 5.0, 11.0, 9.0, 11.0]])
            };
            (s.clone(), s)
        }
        ShapeKind::Ladder => {
            let s =
                Shape::from_px(&[[0.0, 0.0, 13.0, 16.0, 16.0, 16.0]]).rotated(props.dir("facing"));
            (s.clone(), s)
        }
        ShapeKind::Cake => {
            let bites = props.int("bites").clamp(0, 6) as f64;
            let s = Shape::from_px(&[[1.0 + 2.0 * bites, 0.0, 1.0, 15.0, 8.0, 15.0]]);
            (s.clone(), s)
        }
        ShapeKind::Sunken => (
            Shape::from_px(&[[0.0, 0.0, 0.0, 16.0, 14.0, 16.0]]),
            Shape::full(),
        ),
        ShapeKind::LilyPad => {
            let s = Shape::from_px(&[[1.0, 0.0, 1.0, 15.0, 1.5, 15.0]]);
            (s.clone(), s)
        }
        ShapeKind::Boxes(boxes) => {
            let s = Shape::from_px(boxes);
            (s.clone(), s)
        }
    }
}

fn stairs_shape(props: &dyn PropertyLookup) -> Shape {
    let facing = props.dir("facing");
    let top = props.value("half") == Some("top");
    let (base_y, step_y) = if top { (8.0, 0.0) } else { (0.0, 8.0) };
    let base = Shape::from_px(&[[0.0, base_y, 0.0, 16.0, base_y + 8.0, 16.0]]);
    // Step pieces authored for facing North: the full-height back is on the north half.
    let north_half = [0.0, step_y, 0.0, 16.0, step_y + 8.0, 8.0];
    let west_quarter_of_north = [0.0, step_y, 0.0, 8.0, step_y + 8.0, 8.0];
    let east_quarter_of_north = [8.0, step_y, 0.0, 16.0, step_y + 8.0, 8.0];
    let south_west_quarter = [0.0, step_y, 8.0, 8.0, step_y + 8.0, 16.0];
    let south_east_quarter = [8.0, step_y, 8.0, 16.0, step_y + 8.0, 16.0];
    let step = match props.value("shape") {
        Some("outer_left") => Shape::from_px(&[west_quarter_of_north]),
        Some("outer_right") => Shape::from_px(&[east_quarter_of_north]),
        Some("inner_left") => Shape::from_px(&[north_half, south_west_quarter]),
        Some("inner_right") => Shape::from_px(&[north_half, south_east_quarter]),
        _ => Shape::from_px(&[north_half]),
    };
    base.with(&step.rotated(facing))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct Props(HashMap<&'static str, &'static str>);
    impl PropertyLookup for Props {
        fn value(&self, name: &str) -> Option<&str> {
            self.0.get(name).copied()
        }
    }
    fn props(p: &[(&'static str, &'static str)]) -> Props {
        Props(p.iter().copied().collect())
    }

    #[test]
    fn slab_and_full() {
        let (c, _) = shapes_for(&ShapeKind::Full, &props(&[]));
        assert!(c.is_full_cube());
        let (c, _) = shapes_for(&ShapeKind::Slab, &props(&[("type", "top")]));
        assert_eq!(c.boxes[0].min.y, 0.5);
        assert!(c.covers_face(Direction::Up));
        assert!(!c.covers_face(Direction::Down));
    }

    #[test]
    fn rotation_maps_north_panel() {
        let north = Shape::from_px(&[[0.0, 0.0, 0.0, 16.0, 16.0, 2.0]]);
        let east = north.rotated(Direction::East);
        assert!((east.boxes[0].max.x - 1.0).abs() < 1e-12);
        assert!((east.boxes[0].min.x - 14.0 / 16.0).abs() < 1e-12);
        let south = north.rotated(Direction::South);
        assert!((south.boxes[0].min.z - 14.0 / 16.0).abs() < 1e-12);
        let west = north.rotated(Direction::West);
        assert!(
            west.boxes[0].min.x.abs() < 1e-12 && (west.boxes[0].max.x - 2.0 / 16.0).abs() < 1e-12
        );
    }

    #[test]
    fn stairs_have_step_on_facing_side() {
        let (s, _) = shapes_for(
            &ShapeKind::Stairs,
            &props(&[
                ("facing", "east"),
                ("half", "bottom"),
                ("shape", "straight"),
            ]),
        );
        let step = &s.boxes[1];
        assert!(step.min.x >= 0.5 && step.min.y >= 0.5);
        assert!(s.covers_face(Direction::Down));
        assert!(s.covers_face(Direction::East));
    }

    #[test]
    fn fences_are_tall_and_snow_layers_scale() {
        let (c, o) = shapes_for(&ShapeKind::Fence, &props(&[("north", "true")]));
        assert!(c.top() > 1.4 && o.top() <= 1.0);
        assert_eq!(c.boxes.len(), 2);
        let (c1, o1) = shapes_for(&ShapeKind::SnowLayer, &props(&[("layers", "1")]));
        assert!(c1.is_empty());
        assert!((o1.top() - 0.125).abs() < 1e-12);
        let (c8, _) = shapes_for(&ShapeKind::SnowLayer, &props(&[("layers", "8")]));
        assert!((c8.top() - 0.875).abs() < 1e-12);
    }
}

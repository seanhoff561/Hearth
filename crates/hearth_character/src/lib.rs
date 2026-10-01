//! The player's person (V2-3, v2 §9.1–9.2): how they look (`appearance`, saved as profiles), a
//! blocky but realistically proportioned frame (`rig`, from anthropometric segment lengths),
//! how it moves (`animate`: the gaits, crouching, crawling, swimming, climbing, falling and
//! lying, procedurally), and the boxes the renderer draws (`instances`).

pub mod animate;
pub mod appearance;
pub mod instances;
pub mod rig;

pub use animate::{Activity, Animator, Drive, Pose};
pub use appearance::{
    Appearance, BodyType, EyeColor, FacialHair, HAIR_COLORS, HairStyle, Loincloth,
};
pub use instances::{FigureInstance, Palette, Show, instances};
pub use rig::{EYE, Joint, Region, Rig, Stuff};

/// A person ready to draw: their rig and colours, rebuilt when their appearance changes.
#[derive(Debug, Clone, PartialEq)]
pub struct Figure {
    pub appearance: Appearance,
    pub rig: Rig,
    pub palette: Palette,
    pub animator: Animator,
}

impl Figure {
    pub fn new(appearance: Appearance) -> Self {
        let appearance = appearance.sanitized();
        Self {
            rig: Rig::new(&appearance),
            palette: Palette::of(&appearance),
            appearance,
            animator: Animator::default(),
        }
    }

    /// Takes a changed appearance (keeps the animation going).
    pub fn set_appearance(&mut self, appearance: &Appearance) {
        let a = appearance.clone().sanitized();
        if a != self.appearance {
            self.rig = Rig::new(&a);
            self.palette = Palette::of(&a);
            self.appearance = a;
        }
    }

    /// The eye point in the figure's frame for a pose.
    pub fn eye(&self, pose: &Pose) -> glam::Vec3 {
        let head = pose.joints(&self.rig)[Joint::Head.index()];
        head.transform_point3(self.rig.eye_in_head())
    }
}

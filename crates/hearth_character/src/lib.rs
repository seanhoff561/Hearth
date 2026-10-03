//! The player's person (V2-3, v2 §9.1–9.2): how they look (`appearance`, saved as profiles), a
//! blocky but realistically proportioned frame (`rig`, from anthropometric segment lengths),
//! how it moves (`animate`: the gaits, crouching, crawling, swimming, climbing, falling and
//! lying, procedurally), and the boxes the renderer draws (`instances`).

pub mod animate;
pub mod appearance;
pub mod instances;
pub mod rig;

pub use animate::{Activity, Animator, Drive, Holding, Pose};
pub use appearance::{
    Appearance, BodyType, EyeColor, FacialHair, HAIR_COLORS, HairStyle, Loincloth,
};
pub use instances::{FigureInstance, Palette, Show, instances, skinned, solid};
pub use rig::{EYE, Garb, Joint, Region, Rig, Stuff, starting_garbs};

/// A person ready to draw: their rig and colours, rebuilt when their appearance changes.
#[derive(Debug, Clone, PartialEq)]
pub struct Figure {
    pub appearance: Appearance,
    pub rig: Rig,
    pub palette: Palette,
    pub animator: Animator,
    /// What they wear.
    pub garbs: Vec<Garb>,
}

impl Figure {
    pub fn new(appearance: Appearance) -> Self {
        let appearance = appearance.sanitized();
        Self {
            rig: Rig::new(&appearance),
            palette: Palette::of(&appearance),
            appearance,
            animator: Animator::default(),
            garbs: Vec::new(),
        }
    }

    /// Takes a changed appearance (keeps the animation going and what is worn).
    pub fn set_appearance(&mut self, appearance: &Appearance) {
        let a = appearance.clone().sanitized();
        if a != self.appearance {
            let garbs = std::mem::take(&mut self.garbs);
            self.rig = Rig::new(&a);
            self.palette = Palette::of(&a);
            self.appearance = a;
            self.dress(&garbs);
        }
    }

    /// Dresses the person (when what they wear changed).
    pub fn dress(&mut self, garbs: &[Garb]) {
        if garbs != self.garbs.as_slice() {
            self.rig.dress(garbs);
            self.garbs = garbs.to_vec();
        }
    }

    /// An australopith of this stature and colouring, in its coat of hair ([`Rig::hominin`]).
    pub fn hominin(appearance: Appearance) -> Self {
        let mut a = appearance;
        let height = if a.height_m.is_finite() {
            a.height_m.clamp(0.8, 1.6)
        } else {
            1.25
        };
        a = a.sanitized();
        a.height_m = height;
        Self {
            rig: Rig::hominin(&a),
            palette: Palette::of(&a),
            appearance: a,
            animator: Animator::default(),
            garbs: Vec::new(),
        }
    }

    /// A person dressed as they start (a loincloth; a chest band for a female body).
    pub fn starting(appearance: Appearance) -> Self {
        let garbs = starting_garbs(&appearance);
        let mut f = Self::new(appearance);
        f.dress(&garbs);
        f
    }

    /// Where a hand holds things in the figure's frame (the middle of the palm, the thing's
    /// length along the forearm's line).
    pub fn hand(&self, pose: &Pose, left: bool) -> glam::Affine3A {
        let j = if left { Joint::WristL } else { Joint::WristR };
        let h = self.rig.dims.stature;
        pose.joints(&self.rig)[j.index()]
            * glam::Affine3A::from_translation(glam::Vec3::new(0.0, -0.05 * h, 0.012 * h))
    }

    /// The eye point in the figure's frame for a pose.
    pub fn eye(&self, pose: &Pose) -> glam::Vec3 {
        let head = pose.joints(&self.rig)[Joint::Head.index()];
        head.transform_point3(self.rig.eye_in_head())
    }
}

//! People drawn as sculpted, skinned bodies (Amendment E §8, E7): a person's meshes kept for
//! their appearance, the garments fitted to them and the detail they are seen at (built on a
//! worker thread, the last ones drawn meanwhile), their hair and eyes moving frame by frame,
//! their skin as the body simulation has left it. Garments not fitted as meshes are drawn as
//! the rig's boxes over the body.

use std::sync::{Arc, mpsc};

use glam::{Affine3A, Vec2, Vec3};
use hearth_body::harm::Side;
use hearth_body::skin::Skin;
use hearth_character::garment::FITTED;
use hearth_character::person::{Detail, Meshes, meshes};
use hearth_character::rig::{JOINTS, Joint};
use hearth_character::{Appearance, Figure, Garb, Pose, Rig};
use hearth_content::schema::body::BodyRegion;
use hearth_protocol::BodyView;
use hearth_render::GpuContext;
use hearth_render::body::{PersonFrame, PersonMeshes, PersonMotion, SkinLook, SkinState};

/// What a person's meshes are made from.
#[derive(Debug, Clone, PartialEq)]
struct Key {
    appearance: Appearance,
    worn: Vec<String>,
    detail: Detail,
}

/// One person shown as a sculpted body.
pub struct Person {
    shown: Option<(Key, Arc<PersonMeshes>)>,
    motion: Option<PersonMotion>,
    pending: Option<(Key, mpsc::Receiver<Meshes>)>,
    /// The rig dressed in the garments not fitted as meshes (drawn as boxes), and those garbs.
    boxes: Option<(Vec<Garb>, Rig)>,
    seed: u64,
}

/// The garments of these that are fitted as meshes.
fn fitted(garbs: &[Garb]) -> Vec<String> {
    garbs
        .iter()
        .map(|g| g.garment.clone())
        .filter(|g| FITTED.contains(&g.as_str()))
        .collect()
}

impl Person {
    pub fn new(seed: u64) -> Self {
        Self {
            shown: None,
            motion: None,
            pending: None,
            boxes: None,
            seed,
        }
    }

    fn take(&mut self, ctx: &GpuContext, key: Key, m: Meshes) {
        let hair_changed = self
            .shown
            .as_ref()
            .is_none_or(|(k, _)| k.appearance != key.appearance);
        if hair_changed || self.motion.is_none() {
            self.motion = Some(PersonMotion::new(&key.appearance, &m, self.seed));
        }
        self.shown = Some((key, Arc::new(PersonMeshes::upload(ctx, &m))));
    }

    /// Keeps the meshes for this figure at this detail: when they change, starts building them
    /// on a worker thread (still drawing the last ones), and takes them up when built. With
    /// `wait`, builds them here and now (screenshots).
    pub fn keep(&mut self, ctx: &GpuContext, figure: &Figure, detail: Detail, wait: bool) {
        let key = Key {
            appearance: figure.appearance.clone(),
            worn: fitted(&figure.garbs),
            detail,
        };
        if let Some((k, rx)) = &self.pending {
            match rx.try_recv() {
                Ok(m) => {
                    let k = k.clone();
                    self.pending = None;
                    self.take(ctx, k, m);
                }
                Err(mpsc::TryRecvError::Disconnected) => self.pending = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        // One build at a time: while one is under way, a newer wish waits for it (sliders
        // dragged make many).
        let current = self.shown.as_ref().is_some_and(|(k, _)| *k == key)
            || (self.pending.is_some() && !wait);
        if !current {
            let worn: Vec<String> = key.worn.clone();
            let a = key.appearance.clone();
            if wait {
                let refs: Vec<&str> = worn.iter().map(String::as_str).collect();
                let m = meshes(&a, &refs, detail);
                self.take(ctx, key, m);
            } else {
                let (tx, rx) = mpsc::channel();
                std::thread::spawn(move || {
                    let refs: Vec<&str> = worn.iter().map(String::as_str).collect();
                    let _ = tx.send(meshes(&a, &refs, detail));
                });
                self.pending = Some((key, rx));
            }
        }
        let others: Vec<Garb> = figure
            .garbs
            .iter()
            .filter(|g| !FITTED.contains(&g.garment.as_str()))
            .cloned()
            .collect();
        if self.boxes.as_ref().is_none_or(|(g, _)| *g != others) {
            let mut rig = figure.rig.clone();
            rig.dress(&others);
            self.boxes = Some((others, rig));
        }
    }

    /// Whether there are meshes to draw.
    pub fn ready(&self) -> bool {
        self.shown.is_some()
    }

    /// The appearance the meshes shown were built from (the creator's benchmark).
    pub fn shown_appearance(&self) -> Option<&Appearance> {
        self.shown.as_ref().map(|(k, _)| &k.appearance)
    }

    /// The rig to draw the unfitted garments' boxes from (with `Show::clothes_only`).
    pub fn garment_rig(&self) -> Option<&Rig> {
        self.boxes
            .as_ref()
            .filter(|(g, _)| !g.is_empty())
            .map(|(_, r)| r)
    }

    /// This frame's drawing of the person in `pose`, placed by `place` (camera-relative), its
    /// hair and eyes moved on by `dt`; in first person (`hide_head`) without the head, hair and
    /// eyes.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        figure: &Figure,
        pose: &Pose,
        place: Affine3A,
        state: SkinState,
        dt: f32,
        wind: Vec3,
        light: [f32; 2],
        hide_head: bool,
    ) -> Option<(Arc<PersonMeshes>, PersonFrame)> {
        let (key, meshes) = self.shown.as_ref()?;
        let motion = self.motion.as_mut()?;
        let a = &key.appearance;
        let mut palette = meshes.body.palette(&figure.rig, pose, place);
        let look = SkinLook {
            state,
            ..SkinLook::of(a)
        };
        let mut frame = motion.frame(a, palette, look, dt, wind, Vec2::ZERO, None, light);
        if hide_head {
            // The head and neck drawn down to a point inside the top of the chest (not at the
            // eye, where a funnel would reach up to the camera); the eye, looking down, comes
            // to where the neck was.
            let chest = place * pose.joints(&figure.rig)[Joint::Chest.index()];
            let at = chest.transform_point3(Vec3::new(0.0, 0.08, -0.02));
            let point = [[0.0; 4], [0.0; 4], [0.0; 4], [at.x, at.y, at.z, 1.0]];
            palette[Joint::Head.index()] = point;
            palette[Joint::Neck.index()] = point;
            frame.palette = palette;
            frame.hair = None;
            frame.eyes = None;
        }
        Some((meshes.clone(), frame))
    }
}

/// The joints that carry a body region on a side.
fn region_joints(region: BodyRegion, side: Side) -> Vec<Joint> {
    let pair = |l: Joint, r: Joint| match side {
        Side::Left => vec![l],
        Side::Right => vec![r],
        Side::Middle => vec![l, r],
    };
    match region {
        BodyRegion::Head => vec![Joint::Head],
        BodyRegion::Neck => vec![Joint::Neck],
        BodyRegion::Chest => vec![Joint::Chest],
        BodyRegion::Abdomen => vec![Joint::Waist],
        BodyRegion::Pelvis => vec![Joint::Root],
        BodyRegion::UpperArm => pair(Joint::ShoulderL, Joint::ShoulderR),
        BodyRegion::LowerArm => pair(Joint::ElbowL, Joint::ElbowR),
        BodyRegion::Hand => pair(Joint::WristL, Joint::WristR),
        BodyRegion::UpperLeg => pair(Joint::HipL, Joint::HipR),
        BodyRegion::LowerLeg => pair(Joint::KneeL, Joint::KneeR),
        BodyRegion::Foot => pair(Joint::AnkleL, Joint::AnkleR),
    }
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The skin as drawn, from the skin the body simulation keeps and the body's state: wet, the
/// sun's tan and burn, pale with cold or blood lost, flushed with heat or effort, goosebumps
/// when cold; dirt on the legs and hands, blood and scars where they are; the skin under the
/// fitted garments shaded from the sun.
pub fn skin_state(skin: &Skin, view: Option<&BodyView>, garbs: &[Garb]) -> SkinState {
    let mut s = SkinState {
        tan: skin.tan,
        sunburn: skin.burn,
        ..SkinState::default()
    };
    if let Some(v) = view {
        let st = &v.status;
        s.wet = st.wet.clamp(0.0, 1.0);
        s.pallor = (st.blood_lost * 2.5)
            .max(0.6 * smooth(30.0, 25.0, st.skin_c))
            .min(1.0);
        s.flush = (st.effects.sweating * 0.8)
            .max(smooth(35.5, 38.0, st.skin_c))
            .min(1.0);
        s.goosebumps = st.effects.shivering.max(smooth(31.0, 28.0, st.skin_c));
    }
    let mut marks = [[0.0f32; 4]; JOINTS];
    for (j, d) in [
        (Joint::AnkleL, 1.0),
        (Joint::AnkleR, 1.0),
        (Joint::KneeL, 0.6),
        (Joint::KneeR, 0.6),
    ] {
        marks[j.index()][0] = skin.dirt_legs * d;
    }
    for j in [Joint::WristL, Joint::WristR] {
        marks[j.index()][0] = skin.dirt_hands;
    }
    for m in &skin.blood {
        for j in region_joints(m.region, m.side) {
            marks[j.index()][1] = marks[j.index()][1].max(m.amount);
        }
    }
    for m in &skin.scars {
        for j in region_joints(m.region, m.side) {
            marks[j.index()][2] = marks[j.index()][2].max(m.amount);
        }
    }
    for g in garbs {
        for &r in &g.regions {
            for j in region_joints(r, Side::Middle) {
                marks[j.index()][3] = 1.0;
            }
        }
    }
    s.marks = marks;
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use hearth_body::skin::Mark;

    #[test]
    fn the_skin_drawn_follows_the_body() {
        let skin = Skin {
            tan: 0.4,
            burn: 0.2,
            dirt_legs: 0.8,
            blood: vec![Mark {
                region: BodyRegion::LowerArm,
                side: Side::Left,
                amount: 0.7,
            }],
            ..Skin::default()
        };
        let garbs = hearth_character::starting_garbs(&Appearance::default());
        let s = skin_state(&skin, None, &garbs);
        assert_eq!(s.tan, 0.4);
        assert!(s.marks[Joint::AnkleL.index()][0] > 0.7);
        assert_eq!(s.marks[Joint::ElbowL.index()][1], 0.7);
        assert_eq!(s.marks[Joint::ElbowR.index()][1], 0.0);
        // The loincloth shades the hips.
        assert_eq!(s.marks[Joint::Root.index()][3], 1.0);
        assert_eq!(s.marks[Joint::Chest.index()][3], 0.0);
    }
}

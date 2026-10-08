//! People drawn as sculpted, skinned bodies (Amendment E §8, E7): a person's meshes kept for
//! their appearance's shape, the garments fitted to them and the detail they are seen at (built
//! on a worker thread, the last ones drawn meanwhile: a new shape first at the coarse detail, in a
//! quarter of a second, then at the detail wanted), their colours drawn as they are each frame,
//! their hair and eyes moving frame by frame, their skin as the body simulation has left it.
//! Garments not fitted as meshes are drawn as the rig's boxes over the body.

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

/// What a person's meshes are made from: the appearance's shape (its colours are drawn each
/// frame, E4.1 §4.3), the garments fitted, the detail.
#[derive(Debug, Clone, PartialEq)]
struct Key {
    shape: Appearance,
    worn: Vec<String>,
    detail: Detail,
}

impl Key {
    /// Whether the meshes of `other` are of the same person as these (at any detail).
    fn same_shape(&self, other: &Key) -> bool {
        self.shape == other.shape && self.worn == other.worn
    }
}

/// An appearance as far as the meshes go: its colours (skin, undertone, freckles, hair and
/// eyes) left out, as every frame draws them from the appearance as it is.
fn shape_of(a: &Appearance) -> Appearance {
    let d = Appearance::default();
    Appearance {
        skin_tone: d.skin_tone,
        undertone: d.undertone,
        freckles: d.freckles,
        hair_color: d.hair_color,
        eyes: d.eyes,
        ..a.clone()
    }
}

/// How coarse a detail is (the coarsest first).
fn coarseness(d: Detail) -> u8 {
    match d {
        Detail::Far => 2,
        Detail::Near => 1,
        Detail::Close => 0,
    }
}

/// Built meshes kept for a person: the latest few settings, so one gone back to shows at once.
const KEPT: usize = 6;

/// Meshes built: what from, on the GPU, and as built (their hair's guides start its motion).
struct Built {
    key: Key,
    gpu: Arc<PersonMeshes>,
    meshes: Arc<Meshes>,
}

/// One person shown as a sculpted body.
pub struct Person {
    /// The meshes drawn, and the number of the build they came from.
    shown: Option<(u64, Arc<Built>)>,
    motion: Option<PersonMotion>,
    /// Builds under way, numbered in the order begun: at most a coarse one of the newest shape
    /// and one at the detail wanted.
    pending: Vec<(u64, Key, mpsc::Receiver<Meshes>)>,
    /// The meshes built lately, the newest last.
    kept: Vec<Arc<Built>>,
    /// The next build's number.
    next: u64,
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
            pending: Vec::new(),
            kept: Vec::new(),
            next: 0,
            boxes: None,
            seed,
        }
    }

    /// The key of the meshes drawn.
    fn shown_key(&self) -> Option<&Key> {
        self.shown.as_ref().map(|(_, b)| &b.key)
    }

    /// Draws these meshes from now on (numbered `n`).
    fn show(&mut self, n: u64, built: Arc<Built>) {
        let hair_changed = self.shown_key().is_none_or(|k| k.shape != built.key.shape);
        if hair_changed || self.motion.is_none() {
            self.motion = Some(PersonMotion::new(
                &built.key.shape,
                &built.meshes,
                self.seed,
            ));
        }
        self.shown = Some((n, built));
    }

    /// Uploads meshes built and keeps them.
    fn keep_built(&mut self, ctx: &GpuContext, key: Key, m: Meshes) -> Arc<Built> {
        let built = Arc::new(Built {
            gpu: Arc::new(PersonMeshes::upload(ctx, &m)),
            meshes: Arc::new(m),
            key,
        });
        self.kept.retain(|b| b.key != built.key);
        self.kept.push(built.clone());
        if self.kept.len() > KEPT {
            self.kept.remove(0);
        }
        built
    }

    /// Starts building the meshes of `key` on a worker thread.
    fn build(&mut self, key: Key) {
        let (tx, rx) = mpsc::channel();
        let (a, worn, detail) = (key.shape.clone(), key.worn.clone(), key.detail);
        let spawned = std::thread::Builder::new()
            .name("person".into())
            .spawn(move || {
                let refs: Vec<&str> = worn.iter().map(String::as_str).collect();
                let _ = tx.send(meshes(&a, &refs, detail));
            });
        match spawned {
            Ok(_) => {
                self.pending.push((self.next, key, rx));
                self.next += 1;
            }
            Err(e) => log::error!("could not build a person: {e}"),
        }
    }

    /// Keeps the meshes for this figure at this detail (E4.1 §4.3). Its colours need none: they
    /// are drawn as they are each frame. A new shape is built on a worker thread, first at the
    /// coarse detail (a quarter of a second) and then at the detail wanted, the last meshes
    /// drawn meanwhile (they follow the rig, so a height changed shows at once, roughly); while
    /// a slider is dragged each build done is shown and the newest shape built next. A setting
    /// gone back to is shown at once from the meshes kept. With `wait`, builds them here and now
    /// (screenshots).
    pub fn keep(&mut self, ctx: &GpuContext, figure: &Figure, detail: Detail, wait: bool) {
        let wish = Key {
            shape: shape_of(&figure.appearance),
            worn: fitted(&figure.garbs),
            detail,
        };
        // Builds done: kept, and shown if begun after what is shown (a newer shape, or a finer
        // detail of it).
        let mut i = 0;
        while i < self.pending.len() {
            match self.pending[i].2.try_recv() {
                Ok(m) => {
                    let (n, k, _) = self.pending.swap_remove(i);
                    let built = self.keep_built(ctx, k, m);
                    if self.shown.as_ref().is_none_or(|(s, _)| n > *s) {
                        self.show(n, built);
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending.swap_remove(i);
                }
                Err(mpsc::TryRecvError::Empty) => i += 1,
            }
        }
        // A setting built before: at once, at the finest detail kept up to the one wanted.
        if self.shown_key() != Some(&wish) {
            let kept = self
                .kept
                .iter()
                .filter(|b| {
                    b.key.same_shape(&wish) && coarseness(b.key.detail) >= coarseness(detail)
                })
                .min_by_key(|b| coarseness(b.key.detail))
                .cloned();
            if let Some(b) = kept
                && self.shown_key() != Some(&b.key)
            {
                let n = self.next;
                self.next += 1;
                self.show(n, b);
            }
        }
        let shown_shape = self.shown_key().is_some_and(|k| k.same_shape(&wish));
        let current = self.shown_key() == Some(&wish);
        if wait {
            if !current {
                let refs: Vec<&str> = wish.worn.iter().map(String::as_str).collect();
                let m = meshes(&wish.shape, &refs, detail);
                let n = self.next;
                self.next += 1;
                let built = self.keep_built(ctx, wish, m);
                self.show(n, built);
            }
        } else if !current {
            let coarse_busy = self
                .pending
                .iter()
                .any(|(_, p, _)| p.detail == Detail::Far && detail != Detail::Far);
            let fine_busy = self.pending.iter().any(|(_, p, _)| p.detail == detail);
            if shown_shape || detail == Detail::Far {
                // The shape shown: on to the detail wanted.
                if !fine_busy {
                    self.build(wish);
                }
            } else if !coarse_busy {
                // A new shape: coarse first, the newest each time one is done.
                self.build(Key {
                    detail: Detail::Far,
                    ..wish
                });
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

    /// The detail of the meshes shown if they are of this appearance's shape (the creator's
    /// benchmark).
    pub fn shows(&self, a: &Appearance) -> Option<Detail> {
        self.shown_key()
            .filter(|k| k.shape == shape_of(a))
            .map(|k| k.detail)
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
        let meshes = self.shown.as_ref().map(|(_, b)| b.gpu.clone())?;
        let motion = self.motion.as_mut()?;
        // The colours as they are now, whatever shape the meshes shown were built for.
        let a = &figure.appearance;
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
        Some((meshes, frame))
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

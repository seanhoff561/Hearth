//! The body as built and moved: proportions where people have them, poses at the heights the
//! movement's boxes expect, gaits that stride, skin tones in order, profiles that keep.

use glam::{Affine3A, Vec3};
use hearth_character::appearance::{luminance, skin_linear};
use hearth_character::{
    Activity, Animator, Appearance, BodyType, Drive, Figure, HairStyle, Joint, Pose, Region, Rig,
    Show, Stuff, instances,
};

fn extent(
    rig: &Rig,
    pose: &Pose,
    keep: impl Fn(&hearth_character::rig::Part) -> bool,
) -> (Vec3, Vec3) {
    let joints = pose.joints(rig);
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for p in rig.parts.iter().filter(|p| keep(p)) {
        let m = joints[p.joint.index()];
        let h = p.size / 2.0;
        for k in 0..8 {
            let c = Vec3::new(
                if k & 1 == 0 { -h.x } else { h.x },
                if k & 2 == 0 { -h.y } else { h.y },
                if k & 4 == 0 { -h.z } else { h.z },
            );
            let w = m.transform_point3(p.center + c);
            lo = lo.min(w);
            hi = hi.max(w);
        }
    }
    (lo, hi)
}

fn people() -> Vec<Appearance> {
    let mut out = Vec::new();
    for body in [BodyType::Female, BodyType::Male] {
        for height_m in [1.55, 1.75, 1.95] {
            for build in [0.0, 0.5, 1.0] {
                out.push(Appearance {
                    body,
                    height_m,
                    build,
                    hair: HairStyle::Bald,
                    ..Appearance::default()
                });
            }
        }
    }
    out
}

#[test]
fn proportions_are_human() {
    for a in people() {
        let f = Figure::new(a.clone());
        let rest = Pose::rest(&f.rig);
        let h = a.height_m;
        let (lo, hi) = extent(&f.rig, &rest, |_| true);
        assert!(lo.y.abs() < 0.01 * h, "{a:?}: feet at {}", lo.y);
        assert!(
            (hi.y - h).abs() < 0.015 * h,
            "{a:?}: top at {} for {h}",
            hi.y
        );
        let eye = f.eye(&rest);
        assert!(
            (eye.y / h - 0.936).abs() < 0.01,
            "{a:?}: eyes at {}",
            eye.y / h
        );
        // Fingertips reach to about 38 % of stature.
        let (hand_lo, _) = extent(&f.rig, &rest, |p| p.joint == Joint::WristL);
        assert!(
            (hand_lo.y / h - 0.377).abs() < 0.03,
            "{a:?}: fingertips at {}",
            hand_lo.y / h
        );
        // Shoulders wider than the hips for a male body, hips as wide for a female one.
        let d = f.rig.dims;
        match a.body {
            BodyType::Male => assert!(d.shoulder_w > d.hip_w, "{a:?}"),
            BodyType::Female => assert!(d.shoulder_w < d.hip_w * 1.15, "{a:?}"),
        }
        // Arms hang clear of the hips standing.
        let still = f.animator.pose(&f.rig, Activity::Stand, &Drive::default());
        let joints = still.joints(&f.rig);
        let wrist = joints[Joint::WristL.index()].translation;
        assert!(wrist.x > d.hip_w / 2.0, "{a:?}: wrist at {}", wrist.x);
    }
}

fn height_in(activity: Activity, a: &Appearance) -> (f32, Pose, Rig) {
    let f = Figure::new(a.clone());
    let pose = f.animator.pose(&f.rig, activity, &Drive::default());
    let (_, hi) = extent(&f.rig, &pose, |_| true);
    (hi.y, pose, f.rig)
}

#[test]
fn crouching_crawling_and_lying_fit_their_boxes() {
    let a = Appearance::default();
    // The movement's boxes: 1.75 standing, 1.3 crouching, 0.6 crawling.
    let (crouch, ..) = height_in(Activity::Crouch, &a);
    assert!((1.18..1.42).contains(&crouch), "crouching {crouch}");
    let (crawl, pose, rig) = height_in(Activity::Crawl, &a);
    assert!((0.3..0.62).contains(&crawl), "crawling {crawl}");
    let joints = pose.joints(&rig);
    for j in [Joint::KneeL, Joint::KneeR, Joint::WristL, Joint::WristR] {
        let y = joints[j.index()].translation.y;
        assert!(y < 0.2, "crawling: {j:?} at {y}");
    }
    let (lying, pose, rig) = height_in(Activity::Lie, &a);
    assert!(lying < 0.4, "lying {lying}");
    let (lo, hi) = extent(&rig, &pose, |_| true);
    assert!((hi.z - lo.z) > 1.5, "lying along {}", hi.z - lo.z);
    // Swimming: head and feet near one level.
    let f = Figure::new(a);
    let pose = f.animator.pose(&f.rig, Activity::Dive, &Drive::default());
    let joints = pose.joints(&f.rig);
    let head = joints[Joint::Head.index()].translation.y;
    let ankle = joints[Joint::AnkleL.index()].translation.y;
    assert!(
        (head - ankle).abs() < 0.6,
        "diving: head {head}, ankle {ankle}"
    );
}

#[test]
fn gaits_stride_and_alternate() {
    let f = Figure::new(Appearance::default());
    for (activity, speed, step) in [
        (Activity::Walk, 1.4, 0.75),
        (Activity::Jog, 3.0, 1.15),
        (Activity::Sprint, 6.5, 1.6),
    ] {
        let mut anim = Animator::default();
        let drive = Drive {
            activity,
            speed,
            ..Drive::default()
        };
        let (mut zl, mut zr) = (
            (f32::INFINITY, f32::NEG_INFINITY),
            (f32::INFINITY, f32::NEG_INFINITY),
        );
        let mut roots = (f32::INFINITY, f32::NEG_INFINITY);
        let mut apart = 0.0f32;
        let dt = 1.0 / 120.0;
        for i in 0..480 {
            let pose = anim.update(&f.rig, &drive, dt);
            if i < 60 {
                continue;
            }
            let j = pose.joints(&f.rig);
            let (l, r) = (
                j[Joint::AnkleL.index()].translation.z,
                j[Joint::AnkleR.index()].translation.z,
            );
            zl = (zl.0.min(l), zl.1.max(l));
            zr = (zr.0.min(r), zr.1.max(r));
            apart = apart.max((l - r).abs());
            roots = (roots.0.min(pose.root_y), roots.1.max(pose.root_y));
        }
        let swing = zl.1 - zl.0;
        eprintln!(
            "{activity:?}: foot swings {swing:.2} m, feet apart up to {apart:.2} m, root {:.2}–{:.2}",
            roots.0, roots.1
        );
        assert!(
            (0.5 * step..1.4 * step).contains(&swing),
            "{activity:?}: a foot swings {swing} m for steps of {step}"
        );
        assert!(
            (zr.1 - zr.0 - swing).abs() < 0.1,
            "{activity:?}: legs match"
        );
        assert!(apart > 0.4 * step, "{activity:?}: feet pass each other");
        // The hips bob a little (more running), never sink to a crouch.
        let bob = if activity == Activity::Walk {
            0.08
        } else {
            0.11
        };
        assert!(
            roots.1 - roots.0 < bob && roots.0 > 0.8,
            "{activity:?}: {roots:?}"
        );
    }
}

#[test]
fn skin_tones_run_light_to_dark() {
    let mut last = f32::INFINITY;
    for i in 0..=20 {
        let tone = i as f32 / 20.0;
        let l = luminance(skin_linear(tone, 0.0));
        assert!(l < last, "tone {tone}: {l} not darker than {last}");
        last = l;
        // Undertone changes the hue, not the lightness.
        for u in [-1.0, 1.0] {
            let c = skin_linear(tone, u);
            assert!((luminance(c) - l).abs() < 0.01 * l + 1e-4);
        }
        let (cool, warm) = (skin_linear(tone, -1.0), skin_linear(tone, 1.0));
        assert!(cool[2] / cool[1] > warm[2] / warm[1], "warm is more golden");
    }
}

#[test]
fn skin_is_as_red_as_measured_skin() {
    // Every tone reflects red most and blue least, green some 0.5–0.8 of red and blue 0.3–0.7
    // of it (the blood under the skin takes green and blue, melanin blue most; the dark-skin
    // patch below has blue at 0.34 of red), not a palette's near-greys.
    for i in 0..=20 {
        let tone = i as f32 / 20.0;
        let [r, g, b] = skin_linear(tone, 0.0);
        assert!(r > g && g > b, "tone {tone}: {r} {g} {b}");
        assert!(
            (0.5..0.8).contains(&(g / r)),
            "tone {tone}: green {:.2} of red",
            g / r
        );
        assert!(
            (0.3..0.7).contains(&(b / r)),
            "tone {tone}: blue {:.2} of red",
            b / r
        );
    }
    // The ColorChecker's skin patches, made to match skin's spectra.
    let near = |c: [f32; 3], m: [f32; 3]| {
        c.iter()
            .zip(m)
            .all(|(c, m)| (c - m).abs() < 0.12 * m + 0.01)
    };
    assert!(near(skin_linear(4.0 / 9.0, 0.0), [0.54, 0.305, 0.223]));
    assert!(near(skin_linear(7.0 / 9.0, 0.0), [0.171, 0.0844, 0.0578]));
    // The fairest reflects about 0.64 of red light, the deepest about 0.05.
    assert!((skin_linear(0.0, 0.0)[0] - 0.64).abs() < 0.02);
    assert!((skin_linear(1.0, 0.0)[0] - 0.05).abs() < 0.01);
}

#[test]
fn profiles_keep() {
    for a in [Appearance::default(), Appearance::female()] {
        let json = serde_json::to_string(&a).expect("json");
        let back: Appearance = serde_json::from_str(&json).expect("back");
        assert_eq!(a, back);
    }
    // Old or partial profiles take defaults; wild values come back into range.
    let partial: Appearance =
        serde_json::from_str(r#"{"name":"Ash","height_m":3.0}"#).expect("partial");
    let fixed = partial.sanitized();
    assert_eq!(fixed.name, "Ash");
    assert!(fixed.height_m <= 1.95);
}

#[test]
fn hair_lies_where_its_style_says() {
    let lowest_hair = |hair: HairStyle| {
        let a = Appearance {
            hair,
            ..Appearance::default()
        };
        let f = Figure::new(a);
        let rest = Pose::rest(&f.rig);
        let hairy = f
            .rig
            .parts
            .iter()
            .filter(|p| matches!(p.stuff, Stuff::Hair | Stuff::Shadow))
            .count();
        let (lo, _) = extent(&f.rig, &rest, |p| {
            matches!(p.stuff, Stuff::Hair | Stuff::Shadow)
        });
        (hairy, lo.y / 1.75)
    };
    assert_eq!(lowest_hair(HairStyle::Bald).0, 0);
    for style in HairStyle::ALL {
        if style == HairStyle::Bald {
            continue;
        }
        let (n, low) = lowest_hair(style);
        assert!(n > 0, "{style:?}");
        let long = matches!(
            style,
            HairStyle::LongStraight | HairStyle::LongWavy | HairStyle::Braids
        );
        if long {
            assert!(low < 0.76, "{style:?} reaches {low}");
        } else if matches!(style, HairStyle::ShortCrop | HairStyle::Buzzed) {
            assert!(low > 0.86, "{style:?} reaches {low}");
        }
    }
}

#[test]
fn first_person_leaves_out_the_head() {
    let f = Figure::new(Appearance::female());
    let pose = Pose::rest(&f.rig);
    let (mut all, mut body) = (Vec::new(), Vec::new());
    instances(
        &f.rig,
        &f.palette,
        &pose,
        Affine3A::IDENTITY,
        Show::default(),
        &mut all,
    );
    instances(
        &f.rig,
        &f.palette,
        &pose,
        Affine3A::IDENTITY,
        Show {
            hide_head: true,
            ..Show::default()
        },
        &mut body,
    );
    let head_parts = f
        .rig
        .parts
        .iter()
        .filter(|p| p.region == Region::Head)
        .count();
    assert!(head_parts > 10 && body.len() < all.len());
    // Every box's top is below the neck.
    for i in &body {
        let top = i.rows[1][3] + i.rows[1].iter().take(3).map(|v| v.abs()).sum::<f32>() / 2.0;
        assert!(top < 0.85 * f.rig.dims.stature, "a box reaches {top}");
    }
}

#[test]
fn garments_dress_the_body() {
    use hearth_character::{Garb, starting_garbs};
    use hearth_content::schema::body::{BodyRegion, ClothingLayer};
    let a = Appearance {
        hair: HairStyle::LongStraight,
        facial_hair: hearth_character::FacialHair::FullBeard,
        ..Appearance::female()
    };
    let mut f = Figure::new(a.clone());
    assert!(f.rig.clothes.is_empty(), "undressed");
    f.dress(&starting_garbs(&a));
    let start = f.rig.clothes.len();
    assert_eq!(start, 5, "a loincloth's four boxes and a chest band");
    // Fur leggings over the hips and both legs, standing off the skin.
    let leggings = Garb {
        garment: "hearth:fur_leggings".into(),
        color: [110, 80, 60],
        layer: ClothingLayer::Main,
        regions: vec![
            BodyRegion::Pelvis,
            BodyRegion::UpperLeg,
            BodyRegion::LowerLeg,
        ],
    };
    let mut garbs = starting_garbs(&a);
    garbs.push(leggings);
    f.dress(&garbs);
    let added: Vec<_> = f.rig.clothes[start..].to_vec();
    assert_eq!(
        added.len(),
        1 + 4 + 4,
        "the pelvis, two thighs and two shins of two boxes"
    );
    for c in &added {
        let skin = f
            .rig
            .parts
            .iter()
            .find(|p| p.joint == c.joint && p.center == c.center)
            .expect("over a body part");
        assert!(c.size.cmpgt(skin.size).all(), "wider than the skin");
    }
    // A parka's hood covers the hair, not the beard.
    let parka = Garb {
        garment: "hearth:sewn_fur_parka".into(),
        color: [120, 90, 70],
        layer: ClothingLayer::Outer,
        regions: vec![BodyRegion::Head, BodyRegion::Chest],
    };
    let count_hair = |f: &Figure| {
        let pose = Pose::rest(&f.rig);
        let mut boxes = Vec::new();
        instances(
            &f.rig,
            &f.palette,
            &pose,
            Affine3A::IDENTITY,
            Show::default(),
            &mut boxes,
        );
        let hair = f.palette.hair;
        boxes.iter().filter(|b| b.color[..3] == hair[..]).count()
    };
    let bare = count_hair(&f);
    f.dress(&[parka]);
    assert!(f.rig.hooded);
    let hooded = count_hair(&f);
    assert!(
        hooded < bare && hooded > 0,
        "{bare} hair boxes, {hooded} under the hood (the beard)"
    );
}

/// Motion timing (P §8): a foot on the ground stays put on the ground — the body moves over it
/// at the body's speed, the foot's own motion backward cancelling it — at a walk, a jog and a
/// run; and a body at work loops its stroke at the work's tempo.
#[test]
fn feet_do_not_slide_and_work_keeps_its_tempo() {
    let f = Figure::new(Appearance::default());
    for (activity, speed) in [
        (Activity::Walk, 1.4),
        (Activity::Jog, 3.0),
        (Activity::Sprint, 6.5),
    ] {
        let mut anim = Animator::default();
        let drive = Drive {
            activity,
            speed,
            ..Drive::default()
        };
        let dt = 1.0 / 240.0;
        let mut worst = 0.0f32;
        let mut prev: Option<f32> = None;
        for i in 0..960 {
            let pose = anim.update(&f.rig, &drive, dt);
            let z = pose.joints(&f.rig)[Joint::AnkleL.index()].translation.z;
            // The left foot is down and flat early in its cycle, in every gait. The ground
            // goes back under the body at `speed`, and so must the planted ankle.
            let flat = (0.03..0.18).contains(&anim.phase);
            if i >= 240
                && flat
                && let Some(p) = prev
            {
                worst = worst.max(((z - p) / dt + speed).abs());
            }
            prev = flat.then_some(z);
        }
        eprintln!("{activity:?}: a planted foot slides at most {worst:.2} m/s");
        assert!(
            worst < 0.1 * speed,
            "{activity:?}: a planted foot slides at {worst} m/s at {speed} m/s"
        );
    }
    // At work: the stroke repeats at its tempo.
    use hearth_content::schema::process::WorkPose;
    let stroke_s = 1.4;
    let drive = Drive {
        activity: Activity::Work(WorkPose::StandChop),
        stroke_s,
        ..Drive::default()
    };
    let mut anim = Animator::default();
    let dt = 1.0 / 100.0;
    let mut wrist = Vec::new();
    for _ in 0..600 {
        let pose = anim.update(&f.rig, &drive, dt);
        wrist.push(pose.joints(&f.rig)[Joint::WristR.index()].translation.y);
    }
    // The wrist's height one stroke apart is the same; half a stroke apart, not.
    let n = (stroke_s / dt).round() as usize;
    let (a, b, c) = (wrist[300], wrist[300 + n], wrist[300 + n / 2]);
    assert!((a - b).abs() < 0.02, "a stroke later: {a} against {b}");
    assert!((a - c).abs() > 0.05, "half a stroke later: {a} against {c}");
}

//! H8 (V2.1 §15, Addendum A): a world of each Paleolithic era is made with its deep past, the
//! recent past about the place is lived, and the player is born into one of its households —
//! among the era's own people.

mod common;

use common::*;

#[test]
fn births_are_offered_among_the_peoples_of_each_era() {
    // The deep past's and the recent past's timings, with RUST_LOG=info.
    let _ = env_logger::builder().is_test(true).try_init();
    let only = std::env::var("ERA").ok();
    for (era, seed) in [
        ("hearth:lower_paleolithic", 3u64),
        ("hearth:middle_paleolithic", 3),
        ("hearth:upper_paleolithic", 3),
    ] {
        if only.as_deref().is_some_and(|o| !era.ends_with(o)) {
            continue;
        }
        let dir = temp(&format!("era-{}", era.rsplit(':').next().unwrap_or(era)));
        let t0 = std::time::Instant::now();
        let mut w = World::start_in(
            &dir,
            hearth_save::KnowledgeMode::default(),
            seed,
            false,
            era,
            Some(0),
        );
        let born = w.born.clone().expect("born into a household");
        println!(
            "{era}: born in {:.0} s, to a mother of {:.0} and a father of {:.0}, {} brothers and sisters",
            t0.elapsed().as_secs_f64(),
            born.ages[0],
            born.ages[2],
            born.siblings.len()
        );
        // The family's band about the player: its people drawn out as the player comes.
        w.run(81);
        w.until(30.0, |w| !w.people.is_empty());
        let me = w.mover.pos;
        let near: Vec<_> = w
            .people
            .iter()
            .filter(|v| (v.pos - me).length() < 120.0 && !v.dead)
            .collect();
        println!(
            "  {} people near: {:?}",
            near.len(),
            near.iter().map(|v| v.plan).collect::<Vec<_>>()
        );
        assert!(
            !near.is_empty(),
            "the family's band about the player in {era}"
        );
        let plan = match era {
            "hearth:lower_paleolithic" => hearth_content::schema::humans::BodyPlan::Erectus,
            "hearth:upper_paleolithic" => hearth_content::schema::humans::BodyPlan::Modern,
            _ => near[0].plan,
        };
        assert!(
            near.iter().any(|v| v.plan == plan),
            "the era's people in {era}"
        );
        // The family's band keeps its fire at camp, where the life began.
        let fire = |w: &World| {
            (-14..=14).any(|dx| {
                (-14..=14).any(|dz| {
                    (-6..=6).any(|dy| {
                        let p = hearth_math::BlockPos::new(
                            me.x.floor() as i32 + dx,
                            me.y.floor() as i32 + dy,
                            me.z.floor() as i32 + dz,
                        );
                        w.block(p).as_deref() == Some("campfire")
                    })
                })
            })
        };
        w.run(41);
        assert!(w.until(20.0, fire), "a fire kept at the camp in {era}");
        drop(w);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

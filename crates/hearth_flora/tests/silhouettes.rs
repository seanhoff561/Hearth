//! The species grow into their own shapes and sizes: heights and girths by stage, crowns as
//! broad or narrow as their kind, trunks that hold together, and foliage only on the living.

use hearth_flora::{Part, Stage, Templates};

fn templates() -> Templates {
    Templates::from_content(&hearth_content::Content::load_base())
}

/// Width of the crown (blocks across, the larger of the two directions) ÷ height.
fn width_ratio(t: &hearth_flora::TreeTemplate) -> f32 {
    let w = (t.max[0] - t.min[0] + 1).max(t.max[2] - t.min[2] + 1) as f32;
    w / (t.max[1] + 1).max(1) as f32
}

#[test]
fn every_species_grows_every_stage() {
    let ts = templates();
    assert!(ts.species.len() >= 25, "{} tree species", ts.species.len());
    for (i, sp) in ts.species.iter().enumerate() {
        for stage in Stage::LIVING {
            let t = ts.get(i, stage, 0);
            assert!(!t.blocks.is_empty(), "{} {stage:?} has no blocks", sp.id);
            let top = t.max[1] + 1;
            let h = t.height_m;
            // The blocks reach about the height it is grown at (a block of foliage over it).
            assert!(
                (top as f32) >= h * 0.7 - 1.0 && (top as f32) <= h * 1.15 + 2.5,
                "{} {stage:?}: {top} blocks tall for {h:.1} m",
                sp.id
            );
            assert!(
                t.blocks.iter().any(|(_, p)| *p == Part::Leaves),
                "{} {stage:?} has no foliage",
                sp.id
            );
        }
        let snag = ts.get(i, Stage::Snag, 0);
        assert!(
            snag.blocks.iter().all(|(_, p)| *p != Part::Leaves),
            "{} snag has foliage",
            sp.id
        );
    }
}

#[test]
fn crowns_are_as_broad_or_narrow_as_their_kind() {
    let ts = templates();
    let ratio = |id: &str| {
        let i = ts.index_of(id).expect(id);
        let rs: Vec<f32> = (0..4)
            .map(|v| width_ratio(&ts.get(i, Stage::Mature, v)))
            .collect();
        rs.iter().sum::<f32>() / rs.len() as f32
    };
    let (oak, spruce, birch, aspen) = (
        ratio("english_oak"),
        ratio("norway_spruce"),
        ratio("silver_birch"),
        ratio("aspen"),
    );
    println!(
        "width ÷ height: oak {oak:.2}, spruce {spruce:.2}, birch {birch:.2}, aspen {aspen:.2}"
    );
    assert!(oak > 0.55, "an oak spreads: {oak:.2}");
    assert!(spruce < 0.45, "a spruce is a spire: {spruce:.2}");
    assert!(
        oak > birch && birch > aspen * 0.8,
        "{oak:.2} {birch:.2} {aspen:.2}"
    );
}

#[test]
fn a_mature_oak_has_a_trunk_of_logs_and_limbs_that_join_it() {
    let ts = templates();
    let i = ts.index_of("english_oak").expect("oak");
    let t = ts.get(i, Stage::Old, 0);
    // An old oak is over a metre through: its trunk fills more than one block at the foot.
    let foot_logs = t
        .blocks
        .iter()
        .filter(|(c, p)| c[1] == 1 && matches!(p, Part::Log { .. }))
        .count();
    assert!(foot_logs >= 2, "{foot_logs} logs across the trunk");
    // All wood is joined to the foot (through logs and limbs, face to face).
    let wood: std::collections::HashSet<[i16; 3]> = t
        .blocks
        .iter()
        .filter(|(_, p)| !matches!(p, Part::Leaves))
        .map(|(c, _)| *c)
        .collect();
    let start = *wood
        .iter()
        .find(|c| c[1] == 0 && c[0].abs() <= 1 && c[2].abs() <= 1)
        .expect("wood at the foot");
    let mut seen = std::collections::HashSet::from([start]);
    let mut todo = vec![start];
    while let Some(c) = todo.pop() {
        for d in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ] {
            let n = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
            if wood.contains(&n) && seen.insert(n) {
                todo.push(n);
            }
        }
    }
    let joined = seen.len() as f32 / wood.len() as f32;
    assert!(joined > 0.97, "{:.1} % of the wood joined", joined * 100.0);
}

#[test]
fn templates_are_the_same_every_time() {
    let a = templates();
    let b = templates();
    let i = a.index_of("scots_pine").expect("pine");
    assert_eq!(*a.get(i, Stage::Mature, 3), *b.get(i, Stage::Mature, 3));
    assert_ne!(*a.get(i, Stage::Mature, 3), *a.get(i, Stage::Mature, 4));
}

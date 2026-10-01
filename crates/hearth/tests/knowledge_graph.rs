//! V2-5 acceptance, the graph: with the world's real blocks, every implemented node of Eras
//! 0–2 can be discovered from a fresh start through triggers something emits, every
//! implemented process can be performed, and the effort to reach a technology from scratch
//! rises by era.

use hearth_content::lint::{effort, reachability_in};
use hearth_content::schema::Status;
use hearth_content::{Content, LintContext, Severity, lint};

fn blocks() -> Vec<(String, Option<String>)> {
    hearth::content_state::world_blocks(&[hearth::scene::data_pack_dir()]).expect("blocks")
}

#[test]
fn every_era_0_to_2_node_is_reachable_by_discovery_and_effort_rises() {
    let content = Content::load_base();
    let blocks = blocks();
    let report = lint(
        &content,
        &LintContext {
            biomes: None,
            blocks: Some(blocks.clone()),
        },
    );
    let errors: Vec<String> = report
        .sorted()
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.to_string())
        .collect();
    assert!(errors.is_empty(), "lint errors:\n{}", errors.join("\n"));

    let reach = reachability_in(&content, Some(&blocks));
    for k in content.knowledge.iter() {
        if k.era <= 2 && k.status == Status::Implemented {
            assert!(
                reach.knowledge.contains(&k.id),
                "{} cannot be discovered from scratch",
                k.id
            );
        }
    }
    for p in content.processes.iter() {
        if p.status == Status::Implemented {
            assert!(reach.processes.contains(&p.id), "{} cannot be done", p.id);
        }
    }
    let implemented = content
        .knowledge
        .iter()
        .filter(|k| k.era <= 2 && k.status == Status::Implemented)
        .count();
    assert!(implemented >= 50, "{implemented} Era 0–2 nodes implemented");

    // The mean steps from scratch to reach a node, by era, rise.
    let efforts = effort(&content, &reach);
    let mean = |era: u8| {
        let v: Vec<f64> = efforts
            .iter()
            .filter(|e| e.era == era && e.implemented)
            .map(|e| e.steps as f64)
            .collect();
        v.iter().sum::<f64>() / v.len().max(1) as f64
    };
    let (e0, e1, e2) = (mean(0), mean(1), mean(2));
    println!("mean steps from scratch: era 0 {e0:.1}, era 1 {e1:.1}, era 2 {e2:.1}");
    assert!(
        e0 < e1 && e1 < e2,
        "effort rises by era: {e0:.1}, {e1:.1}, {e2:.1}"
    );
}

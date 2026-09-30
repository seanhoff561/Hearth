//! The base data pack must load and lint without errors.

use hearth_content::{Content, LintContext, Severity, lint};

#[test]
fn base_content_loads_and_lints_clean() {
    let (content, report) = Content::load(&[Content::base_pack_dir()]);
    let msgs: Vec<String> = report.sorted().iter().map(|d| d.to_string()).collect();
    assert_eq!(report.errors(), 0, "load errors:\n{}", msgs.join("\n"));
    let content = content.expect("content");
    for (domain, implemented, planned) in content.status_counts() {
        eprintln!("{domain}: {implemented} implemented, {planned} planned");
    }
    assert!(content.materials.len() > 50, "materials loaded");
    let lint = lint(&content, &LintContext::default());
    let errors: Vec<String> = lint
        .sorted()
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.to_string())
        .collect();
    for d in lint.sorted() {
        eprintln!("{d}");
    }
    assert!(errors.is_empty(), "lint errors:\n{}", errors.join("\n"));
}

//! H9 (Addendum B §2.4): what a player keeps of what their earlier lives knew when they live on
//! as another person — only the new person's knowledge, a head start (legends), or everything —
//! and the old journal kept as notes from a past life, unlocking nothing.

use std::collections::BTreeSet;

use hearth_content::Content;
use hearth_craft::{Graph, Kept, KnowledgeState, NoteKind};

#[test]
fn what_is_kept_after_death_is_as_the_world_says() {
    let content = Content::load_base();
    let graph = Graph::from_content(&content);
    // An earlier life knew all there is; the new person knows nothing.
    let earlier = KnowledgeState::open(&graph, 0);
    let past: BTreeSet<String> = earlier.known.keys().cloned().collect();
    assert!(past.len() > 20, "the graph has techniques to know");
    let mut old = KnowledgeState::default();
    old.first_made("spear", "spear", 5);
    let theirs = KnowledgeState::default();

    let only = theirs
        .clone()
        .lived_on(&old.journal, &past, Kept::TheirsOnly, &graph, 100);
    assert!(only.known.is_empty(), "knows only what they knew");
    assert!(only.legends.is_empty());
    assert!(
        only.journal.iter().any(|n| n.kind == NoteKind::PastLife),
        "the old journal kept as notes from a past life"
    );

    let head = theirs
        .clone()
        .lived_on(&old.journal, &past, Kept::HeadStart, &graph, 100);
    assert!(head.known.is_empty(), "a head start is not knowing");
    assert_eq!(head.legends, past, "what was known comes back as legends");

    let all = theirs.lived_on(&old.journal, &past, Kept::Everything, &graph, 100);
    let known: BTreeSet<String> = all.known.keys().cloned().collect();
    assert_eq!(known, past, "everything known again");
    assert!(all.skills.is_empty(), "skills are the new person's own");
}

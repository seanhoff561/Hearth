//! H1 (V2.1 §1.1, §18): ground rule 1 in the content lint. A gene pool may set its physical loci
//! (where selection explains it) but never a behavioural locus or trait, and no behavioural trait
//! or locus may follow the sun.

use hearth_content::schema::humans::{PoolLocus, PoolTrait};
use hearth_content::{Content, IdRef, LintContext, Severity, lint};

fn errors(c: &Content, code: &str) -> usize {
    lint(c, &LintContext::default())
        .sorted()
        .iter()
        .filter(|d| d.severity == Severity::Error && d.code == code)
        .count()
}

#[test]
fn no_population_may_differ_in_temperament() {
    let mut c = Content::load_base();
    assert_eq!(errors(&c, "behavioural-pool"), 0);
    assert_eq!(errors(&c, "behavioural-sunlight"), 0);
    // A pool giving its people their own extraversion: refused.
    let pool = c
        .gene_pools
        .get_mut("hearth:homo_sapiens")
        .expect("the human pool");
    pool.traits.push(PoolTrait {
        of: IdRef("hearth:extraversion".into()),
        raising: 0.7,
    });
    assert_eq!(errors(&c, "behavioural-pool"), 1);
    // Its own skin is allowed.
    let pool = c
        .gene_pools
        .get_mut("hearth:homo_sapiens")
        .expect("the human pool");
    pool.traits.pop();
    pool.loci.push(PoolLocus {
        locus: IdRef("hearth:skin_a".into()),
        frequencies: vec![0.5, 0.5],
    });
    assert_eq!(errors(&c, "behavioural-pool"), 0);
    // A behavioural trait following the sun: refused.
    let t = c.traits.get_mut("hearth:openness").expect("openness");
    t.sunlight = Some((0.3, 0.7));
    assert_eq!(errors(&c, "behavioural-sunlight"), 1);
}

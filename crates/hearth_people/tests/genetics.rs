//! H1 (V2.1 §4, §18): the genetics. Mendel's ratios on a single locus; recombination against
//! distance along a chromosome, and crossovers per meiosis by sex; the mutation rate;
//! heritabilities from offspring–midparent regressions and sibling correlations, as family
//! studies measure them; inbreeding's cost; pools that differ in skin but not in temperament
//! (ground rule 1).

use std::sync::OnceLock;

use hearth_content::Content;
use hearth_math::hash::Rng;
use hearth_people::genome::{Genetics, Genome, Phenotype, Role};

fn genetics() -> &'static Genetics {
    static G: OnceLock<Genetics> = OnceLock::new();
    G.get_or_init(|| Genetics::from_content(&Content::load_base()).expect("the genetics"))
}

fn named(g: &Genetics, id: &str) -> usize {
    g.loci
        .iter()
        .position(|l| matches!(&l.role, Role::Named(n) if n.ends_with(id)))
        .unwrap_or_else(|| panic!("no locus {id}"))
}

/// A founder of the human pool at a sunlight.
fn human(g: &Genetics, sun: f32, female: bool, rng: &mut Rng) -> Genome {
    let pool = g.pool("homo_sapiens").expect("the human pool");
    g.founder(pool, sun, female, rng)
}

#[test]
fn the_architecture_is_a_few_hundred_loci_on_23_pairs() {
    let g = genetics();
    assert_eq!(g.chromosomes.len(), 24, "22 autosomes, X and Y");
    assert!((300..=600).contains(&g.loci.len()), "{} loci", g.loci.len());
    let cm: f32 = g
        .chromosomes
        .iter()
        .filter(|c| c.kind == hearth_content::schema::humans::ChromosomeKind::Autosome)
        .map(|c| c.length_cm)
        .sum();
    assert!((3300.0..3800.0).contains(&cm), "{cm} cM of autosomes");
    // The same content lays out the same architecture.
    let again = Genetics::from_content(&Content::load_base()).expect("again");
    assert_eq!(g.architecture, again.architecture);
}

#[test]
fn two_brown_eyed_carriers_have_a_blue_eyed_child_one_time_in_four() {
    let g = genetics();
    let eye = named(g, "eye_major");
    let mut rng = Rng::new(11);
    let mut mother = human(g, 0.5, true, &mut rng);
    let mut father = human(g, 0.5, false, &mut rng);
    // Both carriers: brown over blue.
    for p in [&mut mother, &mut father] {
        p.maternal[eye] = 0;
        p.paternal[eye] = 1;
    }
    let n = 8000;
    let (mut bb, mut bl, mut ll) = (0, 0, 0);
    for _ in 0..n {
        let c = g.child(&mother, &father, None, &mut rng);
        match (c.maternal[eye], c.paternal[eye]) {
            (1, 1) => ll += 1,
            (0, 0) => bb += 1,
            _ => bl += 1,
        }
    }
    let share = |k: i32| k as f64 / n as f64;
    println!("brown/brown {bb}, carriers {bl}, blue/blue {ll}");
    assert!((share(ll) - 0.25).abs() < 0.02, "blue-eyed {}", share(ll));
    assert!((share(bl) - 0.5).abs() < 0.025, "carriers {}", share(bl));
    assert!((share(bb) - 0.25).abs() < 0.02);
}

#[test]
fn recombination_grows_with_distance_and_crossovers_with_the_sex_map() {
    let g = genetics();
    let mut rng = Rng::new(5);
    let n = 4000;
    // Two loci on chromosome 1 at a distance, and a pair on different chromosomes: a parent with
    // a marker on its mother's copy only at both; recombinants in its gametes.
    let c1 = &g.chromosomes[0];
    let at = |li: usize| g.loci[li].position_cm;
    let pairs: Vec<(usize, usize)> = {
        let l = &c1.loci;
        let mut out = Vec::new();
        for &(lo, hi) in &[(5.0, 25.0), (40.0, 80.0), (20.0, 200.0)] {
            let a = *l.iter().find(|&&x| at(x) > lo).expect("a locus");
            let b = *l.iter().find(|&&x| at(x) > hi).expect("a locus");
            out.push((a, b));
        }
        out
    };
    for (a, b) in pairs {
        let d = (at(b) - at(a)) as f64 * g.female_map as f64;
        let mut mother = human(g, 0.5, true, &mut rng);
        let father = human(g, 0.5, false, &mut rng);
        for li in [a, b] {
            mother.maternal[li] = 0;
            mother.paternal[li] = 1;
        }
        let mut recombinant = 0;
        for _ in 0..n {
            let c = g.child(&mother, &father, Some(true), &mut rng);
            if c.maternal[a] != c.maternal[b] {
                recombinant += 1;
            }
        }
        let r = recombinant as f64 / n as f64;
        let haldane = 0.5 * (1.0 - (-2.0 * d / 100.0).exp());
        println!("{d:.0} cM apart: {r:.3} recombinant (Haldane {haldane:.3})");
        assert!((r - haldane).abs() < 0.06, "{r:.3} against {haldane:.3}");
    }
}

#[test]
fn heritabilities_land_near_their_targets() {
    let g = genetics();
    let mut rng = Rng::new(17);
    let pool = g.pool("homo_sapiens").expect("the pool");
    let pairs = 2500;
    let mut rows: Vec<(Phenotype, Phenotype, Phenotype, Phenotype)> = Vec::new();
    for _ in 0..pairs {
        let m = g.founder(pool, 0.5, true, &mut rng);
        let f = g.founder(pool, 0.5, false, &mut rng);
        let a = g.child(&m, &f, None, &mut rng);
        let b = g.child(&m, &f, None, &mut rng);
        let ph = |x: &Genome, rng: &mut Rng| g.phenotype(x, rng);
        rows.push((
            ph(&m, &mut rng),
            ph(&f, &mut rng),
            ph(&a, &mut rng),
            ph(&b, &mut rng),
        ));
    }
    for (trait_id, h2) in [
        ("stature", 0.8),
        ("honesty_humility", 0.4),
        ("impulsivity", 0.45),
        ("spatial_aptitude", 0.5),
    ] {
        let z = |p: &Phenotype| p.z(trait_id) as f64;
        // Offspring on midparent: the slope is the narrow-sense heritability.
        let xs: Vec<f64> = rows.iter().map(|r| (z(&r.0) + z(&r.1)) / 2.0).collect();
        let ys: Vec<f64> = rows.iter().map(|r| z(&r.2)).collect();
        let slope = covariance(&xs, &ys) / covariance(&xs, &xs);
        // Full siblings correlate at half of it.
        let a: Vec<f64> = rows.iter().map(|r| z(&r.2)).collect();
        let b: Vec<f64> = rows.iter().map(|r| z(&r.3)).collect();
        let sibs = covariance(&a, &b) / (covariance(&a, &a) * covariance(&b, &b)).sqrt();
        println!("{trait_id}: midparent slope {slope:.2}, siblings {sibs:.2} (target {h2})");
        assert!((slope - h2).abs() < 0.08, "{trait_id}: slope {slope:.2}");
        assert!(
            (sibs - h2 / 2.0).abs() < 0.07,
            "{trait_id}: siblings {sibs:.2}"
        );
    }
}

fn covariance(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len() as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - ma) * (y - mb))
        .sum::<f64>()
        / (n - 1.0)
}

#[test]
fn children_of_siblings_carry_more_recessive_conditions() {
    let g = genetics();
    let pool = g.pool("homo_sapiens").expect("the pool");
    let mut rng = Rng::new(23);
    let n = 3000;
    let (mut outbred, mut inbred) = (0u32, 0u32);
    for _ in 0..n {
        let m = g.founder(pool, 0.5, true, &mut rng);
        let f = g.founder(pool, 0.5, false, &mut rng);
        // A brother and sister of those two, and their child; and a child of strangers.
        let sister = g.child(&m, &f, Some(true), &mut rng);
        let brother = g.child(&m, &f, Some(false), &mut rng);
        let c = g.child(&sister, &brother, None, &mut rng);
        inbred += g.phenotype(&c, &mut rng).conditions as u32;
        let s = g.founder(pool, 0.5, true, &mut rng);
        let t = g.founder(pool, 0.5, false, &mut rng);
        let d = g.child(&s, &t, None, &mut rng);
        outbred += g.phenotype(&d, &mut rng).conditions as u32;
    }
    println!("recessive conditions: {inbred} among children of siblings, {outbred} of strangers");
    assert!(inbred > outbred * 4 + 20, "{inbred} against {outbred}");
}

#[test]
fn pools_differ_in_skin_but_not_in_temperament() {
    let g = genetics();
    let pool = g.pool("homo_sapiens").expect("the pool");
    let mut rng = Rng::new(29);
    let n = 2000;
    let mean = |sun: f32, trait_id: &str, rng: &mut Rng| -> f64 {
        (0..n)
            .map(|k| {
                let x = g.founder(pool, sun, k % 2 == 0, rng);
                g.phenotype(&x, rng).z(trait_id) as f64
            })
            .sum::<f64>()
            / n as f64
    };
    let dark = mean(0.95, "skin_pigment", &mut rng) - mean(0.05, "skin_pigment", &mut rng);
    println!("skin: {dark:.2} sd darker under strong sun");
    assert!(dark > 1.5, "skin by sunlight: {dark:.2}");
    for t in g.traits.iter().filter(|t| t.group.behavioural()) {
        let d = mean(0.95, &t.id, &mut rng) - mean(0.05, &t.id, &mut rng);
        assert!(d.abs() < 0.12, "{} differs by {d:.2} between pools", t.id);
    }
}

#[test]
fn a_child_is_the_sex_asked_for_and_mutation_runs_at_its_rate() {
    let g = genetics();
    let mut rng = Rng::new(31);
    let m = human(g, 0.5, true, &mut rng);
    let f = human(g, 0.5, false, &mut rng);
    for _ in 0..50 {
        assert!(g.child(&m, &f, Some(true), &mut rng).female);
        assert!(!g.child(&m, &f, Some(false), &mut rng).female);
    }
    // Mutations: a homozygous parent's alleles changed in its gametes.
    let mut parent = m.clone();
    for li in 0..g.loci.len() {
        parent.paternal[li] = parent.maternal[li];
    }
    let mut changed = 0u64;
    let mut passed = 0u64;
    for _ in 0..8000 {
        let c = g.child(&parent, &f, Some(true), &mut rng);
        for li in 0..g.loci.len() {
            passed += 1;
            if c.maternal[li] != parent.maternal[li] {
                changed += 1;
            }
        }
    }
    let rate = changed as f64 / passed as f64;
    println!("mutation rate {rate:.5} (data {})", g.mutation_rate);
    assert!((rate - g.mutation_rate as f64).abs() < g.mutation_rate as f64 * 0.2);
}

#[test]
fn a_twin_study_finds_the_heritabilities() {
    // Identical twins share one genome; fraternal twins are two children of one couple; each
    // twin is their genes and their own chance. Falconer's 2(r_MZ − r_DZ) recovers h², and the
    // identical twins correlate at h² itself.
    let g = genetics();
    let pool = g.pool("homo_sapiens").expect("the pool");
    let mut rng = Rng::new(37);
    let n = 2500;
    let mut mz: Vec<(Phenotype, Phenotype)> = Vec::new();
    let mut dz: Vec<(Phenotype, Phenotype)> = Vec::new();
    for _ in 0..n {
        let m = g.founder(pool, 0.5, true, &mut rng);
        let f = g.founder(pool, 0.5, false, &mut rng);
        let one = g.child(&m, &f, None, &mut rng);
        mz.push((g.phenotype(&one, &mut rng), g.phenotype(&one, &mut rng)));
        let a = g.child(&m, &f, None, &mut rng);
        let b = g.child(&m, &f, None, &mut rng);
        dz.push((g.phenotype(&a, &mut rng), g.phenotype(&b, &mut rng)));
    }
    for (trait_id, h2) in [
        ("stature", 0.8),
        ("extraversion", 0.4),
        ("verbal_aptitude", 0.5),
    ] {
        let r = |pairs: &[(Phenotype, Phenotype)]| {
            let a: Vec<f64> = pairs.iter().map(|p| p.0.z(trait_id) as f64).collect();
            let b: Vec<f64> = pairs.iter().map(|p| p.1.z(trait_id) as f64).collect();
            covariance(&a, &b) / (covariance(&a, &a) * covariance(&b, &b)).sqrt()
        };
        let (r_mz, r_dz) = (r(&mz), r(&dz));
        let falconer = 2.0 * (r_mz - r_dz);
        println!(
            "{trait_id}: identical {r_mz:.2}, fraternal {r_dz:.2}, Falconer {falconer:.2} (target {h2})"
        );
        assert!(
            (r_mz - h2).abs() < 0.06,
            "{trait_id}: identical twins {r_mz:.2}"
        );
        assert!(
            (falconer - h2).abs() < 0.16,
            "{trait_id}: Falconer {falconer:.2}"
        );
    }
}

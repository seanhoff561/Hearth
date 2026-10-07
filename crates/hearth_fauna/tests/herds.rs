//! Kept animals through the years (V2-12): a ewe beside a ram in the rut is in lamb and lambs in
//! the spring; her lambs are kept, follow her, grow up and are of a generation on; kept animals
//! save and come back; a grown wild-born one turns wary while a docile one stays calm.

use glam::DVec3;
use hearth_content::Content;
use hearth_fauna::Catalog;
use hearth_fauna::herd::{Breed, Kept, Tiding};
use hearth_fauna::live::{Live, Stage};

fn catalog() -> Catalog {
    Catalog::new(&Content::load_base())
}

/// The year's fraction (0 at the March equinox) of a time in years.
fn frac(years: f64) -> f32 {
    years.rem_euclid(1.0) as f32
}

#[test]
fn a_kept_flock_breeds_in_its_season_and_its_lambs_are_kept() {
    let cat = catalog();
    let sheep = cat.index("hearth:mouflon").expect("mouflon") as u16;
    let mut live = Live::new(5);
    let put = |live: &mut Live, female: bool, x: f64| {
        let id = live.place(sheep, Stage::Adult, female, DVec3::new(x, 0.0, 0.0), 0.0);
        let a = live
            .animals
            .iter_mut()
            .find(|a| a.id == id)
            .expect("placed");
        let mut k = Kept::caught(
            Breed {
                docile: 0.6,
                wool: 0.5,
                milk: 0.3,
            },
            -3.0,
        );
        k.tame = 0.8;
        k.tether = Some((a.pos, 4.0));
        a.kept = Some(k);
        id
    };
    let ram = put(&mut live, false, 0.0);
    let ewes: Vec<u64> = (0..6)
        .map(|i| put(&mut live, true, 2.0 + i as f64))
        .collect();
    // Two years by the week.
    let mut births = 0;
    let mut years = 0.0;
    live.tend(&cat, years, frac(years), false);
    while years < 2.0 {
        years += 1.0 / 52.0;
        for t in live.tend(&cat, years, frac(years), false) {
            if let Tiding::Born { young, .. } = t {
                births += young;
            }
        }
    }
    assert!(births >= 4, "lambs born: {births}");
    let lambs: Vec<_> = live
        .animals
        .iter()
        .filter(|a| a.kept.is_some() && a.id != ram && !ewes.contains(&a.id))
        .collect();
    assert_eq!(lambs.len() as u32, births);
    for l in &lambs {
        let k = l.kept.as_ref().expect("kept");
        assert_eq!(k.generation, 1, "a generation on");
        assert!(
            (0.3..0.95).contains(&k.breed.docile),
            "between its parents': {:?}",
            k.breed
        );
    }
    // Saved and back.
    let saved = live.kept_saved(&cat);
    let mut again = Live::new(6);
    again.restore_kept(&cat, saved.clone());
    assert_eq!(again.kept_saved(&cat).len(), saved.len());
}

#[test]
fn a_wild_born_lamb_raised_by_hand_turns_wary_as_it_grows() {
    let cat = catalog();
    let sheep = cat.index("hearth:mouflon").expect("mouflon") as u16;
    let mut live = Live::new(9);
    let mut ids = Vec::new();
    for (docile, x) in [(0.05, 0.0), (0.9, 5.0)] {
        let id = live.place(sheep, Stage::Young, true, DVec3::new(x, 0.0, 0.0), 0.0);
        let a = live
            .animals
            .iter_mut()
            .find(|a| a.id == id)
            .expect("placed");
        let mut k = Kept::caught(
            Breed {
                docile,
                wool: 0.1,
                milk: 0.1,
            },
            0.0,
        );
        k.hand_reared = docile < 0.5;
        a.kept = Some(k);
        ids.push(id);
    }
    let mut years = 0.2;
    live.tend(&cat, years, frac(years), false);
    while years < 3.0 {
        years += 1.0 / 32.0;
        live.tend(&cat, years, frac(years), false);
    }
    let tame = |id: u64| {
        live.animals
            .iter()
            .find(|a| a.id == id)
            .and_then(|a| a.kept.as_ref())
            .map(|k| (k.tame, k.shyness()))
            .expect("kept")
    };
    let (wild, calm) = (tame(ids[0]), tame(ids[1]));
    assert!(
        wild.0 < 0.5 && wild.1 > 0.25,
        "the wild-born turned wary: {wild:?}"
    );
    assert!(
        calm.0 > 0.8 && calm.1 < 0.05,
        "the docile one stayed calm: {calm:?}"
    );
}

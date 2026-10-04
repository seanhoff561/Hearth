//! The player's birth (V2.1 Addendum A; H1, D173; H3, D180). The player's two parents are drawn
//! from the gene pool of the place they begin — its pigmentation set by the place's sunlight —
//! and the player is their child by meiosis: a daughter or a son as asked, or as the father's
//! gamete falls, looking as those genes make them. Their household (their parents' ages, their
//! brothers and sisters) is drawn with it and lives in the world as one of Wild Earth's families.
//! Nothing of their looks is chosen; the family is shown when the player is born.

use hearth_character::{Appearance, Loincloth};
use hearth_content::Content;
use hearth_math::hash::{Rng, derive_seed};
use hearth_people::{Birth, Genetics, Household, Phenotype};
use hearth_protocol::{Born, Wish};

/// The species the player is (the human profile in every era, until the eras' peoples are
/// theirs to be born among: H8).
pub const PLAYER_SPECIES: &str = "hearth:homo_sapiens";
/// The player's age (years) when they take up a life without a family (born again under the
/// Legacy rule, until death's choices replace it).
pub const GROWN_YEARS: f64 = 20.0;
/// Their parents' ages then.
const MOTHER_YEARS: f32 = 44.0;
const FATHER_YEARS: f32 = 47.0;

/// How one of the player's species with a phenotype looks, of a sex and an age, as their figure
/// draws them.
pub fn appearance(content: &Content, ph: &Phenotype, female: bool, age: f32) -> Appearance {
    use hearth_content::schema::humans::BodyPlan;
    let profile = content.species.get(PLAYER_SPECIES);
    let (plan, maturity) = profile.map_or((BodyPlan::Modern, 18.0), |s| {
        (s.body.plan, s.life.maturity_years)
    });
    let look = hearth_people::look(ph, plan, female, age, maturity);
    let grown = profile.map_or(1.7, |s| {
        hearth_people::grown_height_m(s, female, ph.z("stature"))
    });
    let height = grown * profile.map_or(1.0, |s| s.life.grown_share(age as f64).0);
    let mut a = crate::people::appearance_of(&look, female, height);
    a.grown = (age / maturity.max(1.0)).clamp(0.0, 1.0);
    a
}

/// A birth at a latitude: two parents of the place's pool and their child, a daughter or a son
/// as wished (or by chance), drawn from `seed`. None where the content has no human pool.
pub fn draw(
    genetics: &Genetics,
    latitude_deg: f64,
    female: Option<bool>,
    seed: u64,
) -> Option<Birth> {
    let mut rng = Rng::new(derive_seed(seed, "birth"));
    Birth::draw(genetics, PLAYER_SPECIES, latitude_deg, female, &mut rng)
}

/// The age the player's people count the young grown (their life table's; 16 without one).
pub fn coming_of_age(content: &Content) -> f64 {
    hearth_people::life::Tables::from_content(content)
        .of(PLAYER_SPECIES)
        .map_or(16.0, |t| t.coming_of_age as f64)
}

/// The household a birth is into, drawn from the world's seed: the player `age` years old.
pub fn household(genetics: &Genetics, birth: &Birth, age: f64, seed: u64) -> Household {
    let mut rng = Rng::new(derive_seed(seed, "household"));
    Household::draw(birth, genetics, age, &mut rng)
}

/// The player as a birth makes them at an age, with the name and the loincloth they wished for.
pub fn player(
    content: &Content,
    birth: &Birth,
    name: &str,
    loincloth: Loincloth,
    age: f64,
) -> Appearance {
    let mut a = appearance(content, &birth.phenotype, birth.genome.female, age as f32);
    a.name = name.trim().chars().take(32).collect();
    a.loincloth = loincloth;
    a.sanitized()
}

/// The player before any birth (a world without genetics): the wish's name, sex and loincloth on
/// the default person.
pub fn unborn(wish: &Wish) -> Appearance {
    let mut a = if wish.female == Some(true) {
        Appearance::female()
    } else {
        Appearance::default()
    };
    a.name = wish.name.trim().chars().take(32).collect();
    a.loincloth = wish.loincloth;
    a.sanitized()
}

/// The birth as the player is shown it: their mother and father as they look now, the player,
/// and their brothers and sisters, at the ages of the household (without one, the parents in
/// their forties).
pub fn shown(
    content: &Content,
    birth: &Birth,
    you: &Appearance,
    latitude_deg: f64,
    household: Option<&Household>,
) -> Born {
    let (mother, father, age) = household.map_or((MOTHER_YEARS, FATHER_YEARS, GROWN_YEARS), |h| {
        (h.mother_age as f32, h.father_age as f32, h.age)
    });
    let siblings = household.map_or_else(Vec::new, |h| {
        h.siblings
            .iter()
            .map(|s| {
                let a = appearance(content, &s.phenotype, s.genome.female, s.age as f32);
                (s.age as f32, a)
            })
            .collect()
    });
    Born {
        mother: appearance(content, &birth.mother_phenotype, true, mother),
        father: appearance(content, &birth.father_phenotype, false, father),
        you: you.clone(),
        latitude_deg,
        ages: [mother, age as f32, father],
        siblings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_birth_follows_its_wish_and_its_place() {
        let content = Content::load_base();
        let g = Genetics::from_content(&content).expect("genetics");
        let wish = Wish {
            name: " Ash ".into(),
            female: Some(true),
            loincloth: Loincloth::PlantFibre,
        };
        let b = draw(&g, 5.0, wish.female, 7).expect("a birth");
        assert!(b.genome.female, "a daughter, as wished");
        let a = player(&content, &b, &wish.name, wish.loincloth, GROWN_YEARS);
        assert_eq!(a.name, "Ash");
        assert_eq!(a.body, hearth_character::BodyType::Female);
        assert_eq!(a.loincloth, Loincloth::PlantFibre);
        assert!((1.35..1.95).contains(&a.height_m), "{}", a.height_m);
        // The same world and wish: the same birth.
        assert_eq!(draw(&g, 5.0, wish.female, 7), Some(b.clone()));
        // Under the tropical sun the skin runs darker than in the far north, on average.
        let mean_skin = |lat: f64| {
            (0..40)
                .map(|k| {
                    let b = draw(&g, lat, None, 100 + k).expect("a birth");
                    player(&content, &b, "", Loincloth::Hide, GROWN_YEARS).skin_tone
                })
                .sum::<f32>()
                / 40.0
        };
        let (south, north) = (mean_skin(3.0), mean_skin(62.0));
        assert!(south > north + 0.3, "tropics {south:.2}, north {north:.2}");
        let seen = shown(&content, &b, &a, 5.0, None);
        assert_eq!(seen.mother.body, hearth_character::BodyType::Female);
        assert_eq!(seen.father.body, hearth_character::BodyType::Male);
        // A household of the birth: the parents older than their eldest by eighteen years or
        // more, its children of the parents' genes.
        let h = household(&g, &b, 0.0, 7);
        let eldest = h.siblings.iter().map(|s| s.age).fold(0.0, f64::max);
        assert!(h.mother_age >= eldest + 18.0, "{h:?}");
        let seen = shown(&content, &b, &a, 5.0, Some(&h));
        assert_eq!(seen.siblings.len(), h.siblings.len());
    }
}

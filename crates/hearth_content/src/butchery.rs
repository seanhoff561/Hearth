//! Carcasses and butchering by species (V2-7 (h), docs/design/fauna.md "Carcasses and
//! butchering"): generated from each species' data, not enumerated. Every species whose
//! butchering yields are known has a carcass for a grown animal, a grown male's too where the
//! sexes differ in size or the males carry what the females do not (antlers, tusks), and a young
//! one's where it is big enough to bother with, each of its real mass. Each carcass has two ways
//! of working it: butchering proper (skinning and jointing a mammal, plucking and drawing a bird,
//! gutting and filleting a fish, skinning a snake), which takes knowing how, and hacking at it,
//! which wastes half the meat and the hide. The yields are the species' fractions of the live
//! mass; the fat by the season (fattest in autumn, leanest at winter's end); antlers, horns and
//! tusks as the animal carries them. The work grows with the animal's size.

use crate::content::Table;
use crate::generate::ItemDef;
use crate::id::IdRef;
use crate::schema::fauna::{Animal, BodyPlan, Yields, common_name};
use crate::schema::item::Stacking;
use crate::schema::knowledge::Knowledge;
use crate::schema::material::Material;
use crate::schema::process::{Failure, Input, Match, Output, Process, Quality, Target, ToolReq};
use crate::schema::{Duration, Entry, Season, TimeScale};

/// The smallest carcass worth working, kg (a red squirrel; songbirds, mice and frogs are eaten
/// whole or not at all).
pub const MIN_KG: f32 = 0.25;
/// A young animal against a grown female: a calf in its first autumn, a yearling.
pub const YOUNG_SHARE: f32 = 0.4;
/// Males this much heavier than females have carcasses of their own.
const DIMORPHIC: f32 = 1.2;

/// Which animal of a species a carcass is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Carcass {
    /// A grown animal: a female, or either where the sexes are alike.
    Grown,
    /// A grown male, where the sexes differ.
    Male,
    Young,
}

impl Carcass {
    pub const ALL: [Carcass; 3] = [Carcass::Grown, Carcass::Male, Carcass::Young];

    fn suffix(self) -> &'static str {
        match self {
            Carcass::Grown => "_carcass",
            Carcass::Male => "_carcass_male",
            Carcass::Young => "_carcass_young",
        }
    }
}

/// The mass of a grown animal of a sex: the species' range is of both sexes about its middle,
/// a male heavier than a female by the species' dimorphism (1 where they are alike, below 1
/// where the females are the bigger, as in owls).
pub fn grown_mass(range: (f32, f32), dimorphism: f32, male: bool) -> f32 {
    let mean = 0.5 * (range.0 + range.1);
    let r = dimorphism.clamp(0.3, 3.0);
    let female = 2.0 * mean / (1.0 + r);
    if male { female * r } else { female }
}

/// The mass of an animal's carcass, kg.
pub fn carcass_mass(a: &Animal, which: Carcass) -> f32 {
    let d = a.dimorphism.unwrap_or(1.0);
    match which {
        Carcass::Grown => grown_mass(a.mass_kg, d, false),
        Carcass::Male => grown_mass(a.mass_kg, d, true),
        Carcass::Young => grown_mass(a.mass_kg, d, false) * YOUNG_SHARE,
    }
}

/// The id of a species' carcass (`hearth:red_deer_carcass_male`).
pub fn carcass_id(species: &str, which: Carcass) -> String {
    format!("{species}{}", which.suffix())
}

/// The species and animal a carcass's id names.
pub fn carcass_of(item: &str) -> Option<(&str, Carcass)> {
    // The longer suffixes first: `_carcass` ends neither of them.
    for which in [Carcass::Male, Carcass::Young, Carcass::Grown] {
        if let Some(species) = item.strip_suffix(which.suffix()) {
            return Some((species, which));
        }
    }
    None
}

/// The id of the process that butchers a carcass properly, and of hacking at it.
pub fn butcher_id(carcass: &str) -> String {
    let (ns, path) = carcass.split_once(':').unwrap_or(("hearth", carcass));
    format!("{ns}:butcher_{}", path.replacen("_carcass", "", 1))
}

pub fn hack_id(carcass: &str) -> String {
    let (ns, path) = carcass.split_once(':').unwrap_or(("hearth", carcass));
    format!("{ns}:hack_at_{}", path.replacen("_carcass", "", 1))
}

/// "a" or "an" before a word, by its sound.
pub fn article(word: &str) -> &'static str {
    let w = word.to_lowercase();
    let vowel = w.starts_with(['a', 'e', 'i', 'o', 'u'])
        && !w.starts_with("eu")
        && !w.starts_with("uni")
        && !w.starts_with("use")
        && !w.starts_with("one");
    if vowel { "an" } else { "a" }
}

/// How a body is worked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Body {
    Mammal,
    Bird,
    Fish,
    Reptile,
}

fn body_of(plan: BodyPlan) -> Option<Body> {
    Some(match plan {
        BodyPlan::BirdPerching
        | BodyPlan::BirdGround
        | BodyPlan::Raptor
        | BodyPlan::Waterfowl
        | BodyPlan::Seabird
        | BodyPlan::Penguin => Body::Bird,
        BodyPlan::FishFusiform | BodyPlan::FishFlat | BodyPlan::Eel => Body::Fish,
        BodyPlan::Snake | BodyPlan::Lizard | BodyPlan::Turtle | BodyPlan::Crocodilian => {
            Body::Reptile
        }
        BodyPlan::Insect | BodyPlan::Crab | BodyPlan::Amphibian => return None,
        _ => Body::Mammal,
    })
}

/// The carcasses a species has: none for one too small to work (or without known yields).
pub fn carcasses(a: &Animal) -> Vec<Carcass> {
    let Some(y) = &a.yields else {
        return Vec::new();
    };
    let Some(body) = body_of(a.body_plan) else {
        return Vec::new();
    };
    if carcass_mass(a, Carcass::Grown) < MIN_KG {
        return Vec::new();
    }
    let mut out = vec![Carcass::Grown];
    if a.dimorphism.unwrap_or(1.0) >= DIMORPHIC || y.extras.iter().any(|e| e.males_only) {
        out.push(Carcass::Male);
    }
    if matches!(body, Body::Mammal | Body::Bird) && carcass_mass(a, Carcass::Young) >= MIN_KG {
        out.push(Carcass::Young);
    }
    out
}

/// The name of a species' animal in running text ("young red deer").
fn animal_words(a: &Animal, which: Carcass) -> String {
    let name = common_name(&a.name);
    match which {
        Carcass::Grown => name,
        Carcass::Male => format!("male {name}"),
        Carcass::Young => format!("young {name}"),
    }
}

/// How long butchering a carcass of `kg` takes a practised hand, hours: minutes for a hare,
/// a couple of hours for a red deer, most of a day for an aurochs.
fn hours_for(body: Body, kg: f32) -> f32 {
    let mammal = 0.15 + 0.045 * kg.max(0.0).powf(0.8);
    mammal
        * match body {
            Body::Mammal => 1.0,
            Body::Bird => 0.8,
            Body::Fish => 0.5,
            Body::Reptile => 0.6,
        }
}

/// Generates the carcasses of every species and the ways of working them, adding to `items`
/// and `processes`; the knowledge they need lists them among what it enables.
pub fn generate(
    animals: &Table<Animal>,
    materials: &Table<Material>,
    items: &mut Table<ItemDef>,
    processes: &mut Table<Process>,
    knowledge: &mut Table<Knowledge>,
) {
    let butchery = IdRef::qualify("hearth:butchery");
    let mut enabled: Vec<IdRef> = Vec::new();
    for (a, origin) in animals.iter_with_origin() {
        let (Some(y), Some(body)) = (&a.yields, body_of(a.body_plan)) else {
            continue;
        };
        for which in carcasses(a) {
            let id = carcass_id(a.id(), which);
            let kg = carcass_mass(a, which);
            items.upsert(
                id.clone(),
                carcass_item(a, which, &id, kg, body),
                origin.clone(),
            );
            let words = animal_words(a, which);
            for proper in [true, false] {
                let p = work(a, y, which, &id, kg, body, &words, proper, materials);
                if proper {
                    enabled.push(IdRef(p.id.clone()));
                }
                processes.upsert(p.id.clone(), p, origin.clone());
            }
        }
    }
    if let Some(k) = knowledge.get_mut(butchery.as_str()) {
        for p in enabled {
            if !k.enables.contains(&p) {
                k.enables.push(p);
            }
        }
    }
}

fn carcass_item(a: &Animal, which: Carcass, id: &str, kg: f32, body: Body) -> ItemDef {
    let words = animal_words(a, which);
    let shoulder = a.shoulder_height_m.unwrap_or(match body {
        Body::Fish => a.length_m * 0.25,
        Body::Reptile => a.length_m * 0.05,
        _ => a.length_m * 0.5,
    });
    let s = if which == Carcass::Young {
        YOUNG_SHARE.cbrt()
    } else {
        1.0
    };
    // Lying on its side: as wide as it stands high, as high as its body is deep.
    let size_m = match body {
        Body::Fish => [a.length_m * 0.25, a.length_m * 0.1, a.length_m],
        Body::Reptile => [0.15, shoulder.max(0.03), a.length_m * 0.5],
        _ => [shoulder * 0.9, shoulder * 0.4, a.length_m],
    }
    .map(|v| (v * s).max(0.03));
    let footprint = match kg {
        k if k < 0.6 => (1, 2),
        k if k < 2.0 => (2, 2),
        k if k < 8.0 => (3, 2),
        k if k < 30.0 => (4, 3),
        _ => (6, 4),
    };
    ItemDef {
        id: id.to_owned(),
        name: match body {
            Body::Fish => format!("whole {words}"),
            _ => format!("{words} carcass"),
        },
        form: None,
        material: None,
        mass_kg: kg,
        volume_l: kg,
        footprint,
        stacking: Stacking::Single,
        properties: vec![(
            "keeps_days".into(),
            if body == Body::Fish { 1.0 } else { 2.0 },
        )],
        tags: vec!["carcass".into(), "natural".into()],
        status: a.status,
        size_m,
        color: a.coat.as_ref().map_or([120, 90, 60], |c| c.base.0),
        container: None,
        hangs_on: Vec::new(),
        garment: None,
    }
}

/// A material output of `kg` give or take (`lo`, `hi` as fractions of it), in some seasons.
fn out(material: &str, kg: f32, lo: f32, hi: f32, seasons: Vec<Season>) -> Option<Output> {
    (kg * hi > 1e-4).then(|| Output {
        item: Match::Material(IdRef::qualify(material)),
        amount: (kg * lo, kg * hi),
        chance: 1.0,
        quality: Quality::default(),
        seasons,
    })
}

#[allow(clippy::too_many_arguments)]
fn work(
    a: &Animal,
    y: &Yields,
    which: Carcass,
    carcass: &str,
    kg: f32,
    body: Body,
    words: &str,
    proper: bool,
    materials: &Table<Material>,
) -> Process {
    let an = article(words);
    let meat = y
        .meat_material
        .as_ref()
        .map_or("hearth:meat", |m| m.as_str());
    let hide = y
        .hide_material
        .as_ref()
        .map_or("hearth:rawhide", |m| m.as_str());
    // What it gives, and what hacking at it wastes: half the meat, the hide, the sinew.
    let waste = if proper { 1.0 } else { 0.5 };
    let mut outputs: Vec<Output> = Vec::new();
    outputs.extend(out(
        meat,
        kg * y.meat,
        0.85 * waste,
        1.0 * waste,
        Vec::new(),
    ));
    if proper {
        outputs.extend(out(hide, kg * y.hide, 0.9, 1.0, Vec::new()));
    }
    // A fish's or a snake's bones are too fine to keep.
    if matches!(body, Body::Mammal | Body::Bird) {
        outputs.extend(out(
            "hearth:bone",
            kg * y.bone,
            0.8 * waste,
            1.0 * waste,
            Vec::new(),
        ));
    }
    let mut byproducts: Vec<Output> = Vec::new();
    // Fat by the season: fattest in autumn, leanest at the end of winter.
    let fat = kg * y.fat * waste;
    for (season, lo, hi) in [
        (Season::Autumn, 1.2, 1.6),
        (Season::Summer, 0.8, 1.1),
        (Season::Winter, 0.5, 0.8),
        (Season::Spring, 0.2, 0.5),
    ] {
        byproducts.extend(out("hearth:animal_fat", fat, lo, hi, vec![season]));
    }
    byproducts.extend(out(
        "hearth:offal",
        kg * y.organs,
        0.8 * waste,
        waste,
        Vec::new(),
    ));
    if proper {
        byproducts.extend(out("hearth:sinew", kg * y.sinew, 0.7, 1.0, Vec::new()));
        // Antlers, horns, tusks and feathers: from a grown animal that carries them.
        for e in &y.extras {
            let carries = match which {
                Carcass::Young => false,
                Carcass::Male => true,
                Carcass::Grown => !e.males_only,
            };
            if carries && materials.contains(&e.material) {
                byproducts.push(Output {
                    item: Match::Material(e.material.clone()),
                    amount: e.mass_kg,
                    chance: 1.0,
                    quality: Quality::default(),
                    seasons: e.seasons.clone(),
                });
            }
        }
    }
    let (name, action) = match (body, proper) {
        (Body::Mammal, true) => (
            format!("Butcher {an} {words}"),
            format!("skin and joint the {words}"),
        ),
        (Body::Bird, true) => (
            format!("Pluck and draw {an} {words}"),
            format!("pluck and draw the {words}"),
        ),
        (Body::Fish, true) => (
            format!("Gut and fillet {an} {words}"),
            format!("gut and fillet the {words}"),
        ),
        (Body::Reptile, true) => (
            format!("Skin and gut {an} {words}"),
            format!("skin and gut the {words}"),
        ),
        (Body::Bird, false) => (
            format!("Tear at {an} {words}"),
            format!("tear the meat from the {words}"),
        ),
        (_, false) => (
            format!("Hack at {an} {words}"),
            format!("hack meat from the {words}"),
        ),
    };
    let small = kg < 2.0 || matches!(body, Body::Bird | Body::Fish);
    Process {
        id: if proper {
            butcher_id(carcass)
        } else {
            hack_id(carcass)
        },
        name,
        action,
        verb: Some("cut".into()),
        target: Some(Target::Thing(Match::Item(IdRef(carcass.to_owned())))),
        effect: Default::default(),
        inputs: vec![Input {
            item: Match::Item(IdRef(carcass.to_owned())),
            amount: 1.0,
            consumed: true,
        }],
        tools: vec![ToolReq {
            property: "sharp_edge".into(),
            min: match (proper, small) {
                (false, _) => 0.2,
                (true, true) => 0.3,
                (true, false) => 0.5,
            },
        }],
        station: None,
        conditions: Vec::new(),
        duration: Duration {
            hours: hours_for(body, kg),
            scale: TimeScale::Day,
        },
        knowledge: proper.then(|| IdRef::qualify("hearth:butchery")),
        skill: proper.then(|| "butchery".to_owned()),
        outputs,
        byproducts,
        failures: if proper {
            vec![Failure {
                chance: 0.2,
                min_chance: 0.02,
                outcome: "The knife slips and cuts your hand.".into(),
                loses_inputs: false,
                injury: Some("cut".into()),
            }]
        } else {
            Vec::new()
        },
        teaches: Vec::new(),
        attended: true,
        // Turning and lifting a big carcass is hard work.
        mets: if kg >= 50.0 { 4.5 } else { 3.5 },
        // A big animal dulls an edge: flakes are used up on a deer.
        wear: (0.05 * (kg / 15.0).powf(0.6)).clamp(0.01, 0.6),
        harvests: None,
        treats: None,
        status: a.status,
        notes: (!proper)
            .then(|| "Without knowing how, half the meat and the hide are wasted.".into()),
        realism_source: Some(
            "yields: the species' fractions of the live mass; times from experimental butchery \
             (a roe deer within the hour, a red deer a couple of hours); review"
                .into(),
        ),
        uncertain: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carcass_ids_round_trip() {
        for which in Carcass::ALL {
            let id = carcass_id("hearth:red_deer", which);
            assert_eq!(carcass_of(&id), Some(("hearth:red_deer", which)));
        }
        assert_eq!(carcass_of("hearth:flake/flint"), None);
        assert_eq!(
            butcher_id("hearth:red_deer_carcass_male"),
            "hearth:butcher_red_deer_male"
        );
        assert_eq!(
            hack_id("hearth:roe_deer_carcass"),
            "hearth:hack_at_roe_deer"
        );
    }

    #[test]
    fn sexes_share_the_range() {
        let (f, m) = (
            grown_mass((90.0, 240.0), 1.5, false),
            grown_mass((90.0, 240.0), 1.5, true),
        );
        assert!((m / f - 1.5).abs() < 1e-4);
        assert!(((f + m) / 2.0 - 165.0).abs() < 1e-3);
        assert_eq!(grown_mass((3.0, 5.0), 1.0, true), 4.0);
    }

    #[test]
    fn articles_by_sound() {
        assert_eq!(article("aurochs"), "an");
        assert_eq!(article("Eurasian lynx"), "a");
        assert_eq!(article("American black bear"), "an");
        assert_eq!(article("young red deer"), "a");
    }
}

//! The species as the simulation reads them: their content entries with defaults filled in
//! (from body mass, where a pack leaves life history or ranging out), and their foods sorted
//! into the kinds of forage a place produces, the animals they hunt and carrion.

use hearth_content::Content;
use hearth_content::schema::fauna::{
    Activity, Animal, BodyPlan, Dispersers, PopulationModel, SeasonalBehavior, Social,
};
use hearth_content::schema::flora::GrowthForm;
use hearth_worldgen::realms::{RealmSet, set_of};
use rustc_hash::FxHashMap;

/// What a plant-eater eats, grouped as places produce it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Forage {
    /// Grasses and herbs.
    Graze = 0,
    /// Leaves, shoots and twigs of shrubs and young trees within reach.
    Browse = 1,
    /// Nuts and acorns.
    Mast = 2,
    /// Berries and fruit.
    Fruit = 3,
    /// Seeds of grasses and herbs.
    Seeds = 4,
    /// Earthworms, grubs and insects.
    Invertebrates = 5,
    Fungi = 6,
    /// Nectar and pollen, and sweet sap.
    Nectar = 7,
    /// The insect larvae, snails and small crustaceans of fresh water.
    Aquatic = 8,
}

pub const FORAGE_KINDS: usize = 9;

impl Forage {
    pub const ALL: [Forage; FORAGE_KINDS] = [
        Forage::Graze,
        Forage::Browse,
        Forage::Mast,
        Forage::Fruit,
        Forage::Seeds,
        Forage::Invertebrates,
        Forage::Fungi,
        Forage::Nectar,
        Forage::Aquatic,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Forage::Graze => "graze",
            Forage::Browse => "browse",
            Forage::Mast => "mast",
            Forage::Fruit => "fruit",
            Forage::Seeds => "seeds",
            Forage::Invertebrates => "invertebrates",
            Forage::Fungi => "fungi",
            Forage::Nectar => "nectar",
            Forage::Aquatic => "aquatic invertebrates",
        }
    }
}

/// Breeding, growing up and dying (all filled in).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Life {
    pub maturity_years: f32,
    pub lifespan_years: f32,
    pub litter: (f32, f32),
    pub births_per_year: f32,
    /// The year fraction (0 at the March equinox, in the north) most young are born.
    pub birth_frac: f32,
    pub adult_survival: f32,
    pub young_survival: f32,
    pub birth_mass_kg: f32,
}

impl Life {
    pub fn litter_mean(&self) -> f32 {
        0.5 * (self.litter.0 + self.litter.1)
    }
}

/// A species as the simulation reads it.
#[derive(Debug, Clone)]
pub struct Species {
    pub index: usize,
    /// `namespace:path`.
    pub id: String,
    pub name: String,
    pub plan: BodyPlan,
    /// Mean adult mass, kg.
    pub mass_kg: f32,
    pub mass_range: (f32, f32),
    pub model: PopulationModel,
    pub social: Social,
    /// Usual group size (1 for solitary animals).
    pub group: (u16, u16),
    pub territorial: bool,
    pub home_range_km2: f32,
    pub dispersal_km: f32,
    pub dispersers: Dispersers,
    /// Keeps to cover, 0–1.
    pub cover: f32,
    /// Individuals (colonies for a colony species) per km² of good habitat (of water for
    /// fish).
    pub density: f32,
    /// Bits of the catalog's ecosystems it lives in.
    pub habitats: u32,
    pub realms: RealmSet,
    /// Lives in water (its density is per km² of water).
    pub aquatic: bool,
    /// Its density counts colonies (honey bees).
    pub colony: bool,
    /// Food a day: dry matter for plant-eaters, fresh for the rest, kg.
    pub need_kg: f32,
    /// Preference (0–1) for each kind of forage.
    pub forage: [f32; FORAGE_KINDS],
    /// The animals it hunts (species index, preference 0–1).
    pub prey: Vec<(usize, f32)>,
    /// Preference for carrion.
    pub carrion: f32,
    pub life: Life,
    pub hibernates: bool,
    /// Cold-blooded: eats with the warmth (fish, frogs, snakes, insects).
    pub ectotherm: bool,
    pub activity: Activity,
    /// Yearly growth at low density from the content (a check on the simulation).
    pub growth_rate: Option<f32>,
}

impl Species {
    /// Simulated as groups that keep their members.
    pub fn grouped(&self) -> bool {
        self.model == PopulationModel::Groups
    }

    /// Eats plants (or fungi, or invertebrates) at all.
    pub fn forages(&self) -> bool {
        self.forage.iter().any(|p| *p > 0.0)
    }

    /// Hunts.
    pub fn hunts(&self) -> bool {
        !self.prey.is_empty()
    }

    /// The share of its food a hunter takes from plants (and worms and grubs) when the hunt
    /// goes well, from its preferences; all of it for the rest (carrion is anyone's who eats
    /// it).
    pub fn forage_share(&self) -> f32 {
        if !self.hunts() {
            return 1.0;
        }
        let f = self.forage.iter().copied().fold(0.0f32, f32::max);
        let p = self.prey.iter().map(|(_, w)| *w).fold(0.0f32, f32::max);
        if f + p <= 0.0 { 1.0 } else { f / (f + p) }
    }

    /// The share of a year's young that counts in a census: all of a mammal's or bird's
    /// litter, the eggs and larvae of fish and frogs by what they weigh.
    pub fn census_young(&self) -> f32 {
        (2.0 * self.young_appetite()).min(1.0)
    }

    /// What a young one eats against an adult over its first year: little for the tiny young
    /// of fish and frogs, half for the rest.
    pub fn young_appetite(&self) -> f32 {
        (4.0 * self.life.birth_mass_kg / self.mass_kg).clamp(0.02, 0.5)
    }

    /// The radius of its home range, m.
    pub fn range_radius_m(&self) -> f64 {
        ((self.home_range_km2 as f64 / std::f64::consts::PI).sqrt() * 1000.0).max(50.0)
    }
}

/// An ecosystem as the habitat reads it: the biomes it occupies.
#[derive(Debug, Clone)]
pub struct EcosystemDef {
    pub id: String,
    pub biomes: Vec<String>,
}

/// All species of the content, and the ecosystems they live in.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub species: Vec<Species>,
    pub ecosystems: Vec<EcosystemDef>,
    by_id: FxHashMap<String, usize>,
}

/// The year fraction (0 at the March equinox) of a day of the year (0 = 1 January).
pub fn frac_of_day(day: u16) -> f32 {
    ((day as f32 - 79.0) / 365.0).rem_euclid(1.0)
}

/// What a food is, as the habitat produces it.
enum FoodKind {
    Forage(Forage),
    Prey(String),
    Carrion,
    Unknown,
}

fn food_kind(c: &Content, a: &Animal, id: &str) -> FoodKind {
    use hearth_content::schema::fauna::DietKind;
    if c.animals.get(id).is_some() {
        return FoodKind::Prey(id.to_owned());
    }
    // A name that is both a plant and its fruit or nut (bilberry, crab apple): browsers and
    // grazers eat the plant, the rest its fruit.
    let eats_plants = matches!(
        a.diet.kind,
        DietKind::Browser | DietKind::Grazer | DietKind::MixedFeeder
    );
    if let Some(p) = c.plants.get(id)
        && (eats_plants || c.materials.get(id).is_none())
    {
        // Bees take a plant's flowers, seed-eaters a tree's seeds, fruit-eaters its fruit.
        if a.body_plan == BodyPlan::Insect {
            return FoodKind::Forage(Forage::Nectar);
        }
        let woody = matches!(
            p.form,
            GrowthForm::Tree | GrowthForm::Shrub | GrowthForm::Vine
        );
        match a.diet.kind {
            DietKind::Granivore | DietKind::Insectivore if woody => {
                return FoodKind::Forage(Forage::Mast);
            }
            DietKind::Frugivore => return FoodKind::Forage(Forage::Fruit),
            _ => {}
        }
        return FoodKind::Forage(match p.form {
            GrowthForm::Grass
            | GrowthForm::Herb
            | GrowthForm::Fern
            | GrowthForm::Sedge
            | GrowthForm::Aquatic
            | GrowthForm::Moss
            | GrowthForm::Lichen
            | GrowthForm::Alga
            | GrowthForm::Succulent => Forage::Graze,
            GrowthForm::Fungus => Forage::Fungi,
            GrowthForm::Tree | GrowthForm::Shrub | GrowthForm::Vine => Forage::Browse,
        });
    }
    if let Some(m) = c.materials.get(id) {
        let has = |t: &str| m.tags.iter().any(|x| x == t);
        return if has("meat") {
            FoodKind::Carrion
        } else if has("nut") {
            FoodKind::Forage(Forage::Mast)
        } else if has("fruit") {
            FoodKind::Forage(Forage::Fruit)
        } else if has("grain") || has("seed") {
            FoodKind::Forage(Forage::Seeds)
        } else if has("insect") || has("invertebrate") {
            // What a fish takes of the kind is what lives in the water.
            if matches!(
                a.body_plan,
                BodyPlan::FishFusiform | BodyPlan::FishFlat | BodyPlan::Eel
            ) {
                FoodKind::Forage(Forage::Aquatic)
            } else {
                FoodKind::Forage(Forage::Invertebrates)
            }
        } else if has("mushroom") {
            FoodKind::Forage(Forage::Fungi)
        } else if has("sweet") {
            FoodKind::Forage(Forage::Nectar)
        } else {
            FoodKind::Forage(Forage::Graze)
        };
    }
    FoodKind::Unknown
}

impl Catalog {
    /// The species and ecosystems of the content.
    pub fn new(c: &Content) -> Self {
        let mut ecosystems: Vec<EcosystemDef> = c
            .ecosystems
            .iter()
            .map(|e| EcosystemDef {
                id: e.id.clone(),
                biomes: e.biomes.clone(),
            })
            .collect();
        ecosystems.sort_by(|a, b| a.id.cmp(&b.id));
        ecosystems.truncate(32);
        let eco_bit = |id: &str| -> u32 {
            ecosystems
                .iter()
                .position(|e| e.id == id || e.id.ends_with(&format!(":{id}")))
                .map_or(0, |i| 1 << i)
        };
        let mut animals: Vec<&Animal> = c.animals.iter().collect();
        animals.sort_by(|a, b| a.id.cmp(&b.id));
        let by_id: FxHashMap<String, usize> = animals
            .iter()
            .enumerate()
            .map(|(i, a)| (a.id.clone(), i))
            .collect();
        let species = animals
            .iter()
            .enumerate()
            .map(|(i, a)| species_of(c, a, i, &by_id, &eco_bit))
            .collect();
        Self {
            species,
            ecosystems,
            by_id,
        }
    }

    pub fn get(&self, id: &str) -> Option<&Species> {
        self.index(id).map(|i| &self.species[i])
    }

    /// The index of a species by `namespace:path` or bare path.
    pub fn index(&self, id: &str) -> Option<usize> {
        self.by_id
            .get(id)
            .or_else(|| self.by_id.get(&format!("hearth:{id}")))
            .copied()
    }

    pub fn len(&self) -> usize {
        self.species.len()
    }

    pub fn is_empty(&self) -> bool {
        self.species.is_empty()
    }

    /// The bits of the ecosystems that occupy a biome (by its generator name).
    pub fn ecosystems_of_biome(&self, biome: &str) -> u32 {
        self.ecosystems
            .iter()
            .enumerate()
            .filter(|(_, e)| e.biomes.iter().any(|b| b == biome))
            .fold(0, |m, (i, _)| m | (1 << i))
    }
}

fn species_of(
    c: &Content,
    a: &Animal,
    index: usize,
    by_id: &FxHashMap<String, usize>,
    eco_bit: &dyn Fn(&str) -> u32,
) -> Species {
    let mass = 0.5 * (a.mass_kg.0 + a.mass_kg.1).max(1e-6);
    let model = a.ranging.and_then(|r| r.model).unwrap_or(if mass >= 5.0 {
        PopulationModel::Groups
    } else {
        PopulationModel::Density
    });
    let group = match a.social {
        Social::Solitary => (1, 1),
        Social::Pair => (2, 2),
        Social::Family => (2, 4),
        Social::Herd { size }
        | Social::Pack { size }
        | Social::Pride { size }
        | Social::Flock { size }
        | Social::School { size }
        | Social::Colony { size } => (size.0.max(1.0) as u16, size.1.max(1.0) as u16),
    };
    // Defaults from body mass where the pack gives no life history (allometric rules of
    // thumb: bigger animals mature later, live longer and have fewer young).
    let m4 = mass.powf(0.25);
    let life = match a.life {
        Some(l) => Life {
            maturity_years: l.maturity_years.max(0.05),
            lifespan_years: l.lifespan_years.max(0.2),
            litter: (l.litter.0.max(1.0), l.litter.1.max(l.litter.0.max(1.0))),
            births_per_year: l.births_per_year.max(0.05),
            birth_frac: frac_of_day(l.birth_day),
            adult_survival: l.adult_survival.clamp(0.01, 0.999),
            young_survival: l.young_survival.clamp(0.0001, 0.999),
            birth_mass_kg: l.birth_mass_kg.max(1e-7),
        },
        None => Life {
            maturity_years: (0.4 * m4).max(0.1),
            lifespan_years: (4.0 * m4).max(1.0),
            litter: (1.0, (1.0 + 4.0 / m4).round()),
            births_per_year: 1.0,
            birth_frac: 0.2,
            adult_survival: (1.0 - 0.35 / m4).clamp(0.3, 0.95),
            young_survival: 0.5,
            birth_mass_kg: 0.06 * mass.powf(0.9),
        },
    };
    let (home, territorial, dispersal, dispersers, cover) = match a.ranging {
        Some(r) => (
            r.home_range_km2.max(1e-5),
            r.territorial,
            r.dispersal_km.max(0.05),
            r.dispersers,
            r.cover,
        ),
        // Home ranges grow with mass, and more for meat-eaters.
        None => (
            0.02 * mass.powf(1.0),
            false,
            (2.0 * mass.powf(0.4)).max(0.2),
            Dispersers::Males,
            0.4,
        ),
    };
    let mut forage = [0.0f32; FORAGE_KINDS];
    let mut prey = Vec::new();
    let mut carrion = 0.0f32;
    for f in &a.diet.foods {
        match food_kind(c, a, f.food.as_str()) {
            FoodKind::Forage(k) => {
                let p = &mut forage[k as usize];
                *p = p.max(f.preference);
            }
            FoodKind::Prey(id) => {
                if let Some(&j) = by_id.get(&id)
                    && j != index
                {
                    prey.push((j, f.preference));
                }
            }
            FoodKind::Carrion => carrion = carrion.max(f.preference),
            FoodKind::Unknown => {}
        }
    }
    let need = a
        .diet
        .daily_food_kg
        .unwrap_or_else(|| 0.04 * mass.powf(0.75))
        .max(1e-6);
    let hibernates = a
        .seasonal
        .iter()
        .any(|s| matches!(s, SeasonalBehavior::Hibernation));
    let habitats = a.habitat.iter().fold(0, |m, h| m | eco_bit(h.as_str()));
    let aquatic = matches!(
        a.body_plan,
        BodyPlan::FishFusiform | BodyPlan::FishFlat | BodyPlan::Eel
    );
    Species {
        index,
        id: a.id.clone(),
        name: a.name.clone(),
        plan: a.body_plan,
        mass_kg: mass,
        mass_range: a.mass_kg,
        model,
        social: a.social,
        group,
        territorial,
        home_range_km2: home,
        dispersal_km: dispersal,
        dispersers,
        cover,
        density: a.density_per_km2.unwrap_or(1.0).max(0.0),
        habitats,
        realms: set_of(&a.realms),
        aquatic,
        colony: matches!(a.social, Social::Colony { .. }),
        need_kg: need,
        forage,
        prey,
        carrion,
        life,
        hibernates,
        ectotherm: matches!(
            a.body_plan,
            BodyPlan::FishFusiform
                | BodyPlan::FishFlat
                | BodyPlan::Eel
                | BodyPlan::Lizard
                | BodyPlan::Crocodilian
                | BodyPlan::Turtle
                | BodyPlan::Snake
                | BodyPlan::Amphibian
                | BodyPlan::Crab
                | BodyPlan::Insect
        ),
        activity: a.activity,
        growth_rate: a.growth_rate,
    }
}

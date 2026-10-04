//! Species (V2.1 §2): a species profile as its persons are made — bodies of its size and build,
//! by sex, in its coat; how it grows, ages and lives; what its bands know and the techniques that
//! knowledge opens; what it does.

use std::borrow::Cow;

use hearth_body::BodyConfig;
use hearth_body::clothing::Worn;
use hearth_content::Content;
use hearth_content::schema::Status;
use hearth_content::schema::humans::LifeStage;
use hearth_content::schema::humans::{
    Behavior, BodyPlan, Coat, Cognition, LifeParams, SocialDefaults,
};
use hearth_craft::Graph;
use hearth_fauna::live::Stage;

/// A species, as its persons are made and lived.
#[derive(Debug, Clone)]
pub struct Species {
    pub id: String,
    pub name: String,
    pub plan: BodyPlan,
    /// A grown female's and male's height (m) and mass (kg): the middle of the profile's ranges.
    pub height_m: [f32; 2],
    pub mass_kg: [f32; 2],
    /// The spread (one standard deviation) of grown heights about the middle (a quarter of the
    /// profile's range).
    pub height_sd_m: [f32; 2],
    pub climbs: bool,
    pub cognition: Cognition,
    pub life: LifeParams,
    pub social: SocialDefaults,
    pub behaviors: Vec<Behavior>,
    /// What its bands know and practise (knowledge nodes)…
    pub knowledge: Vec<String>,
    /// What its bands know besides where the winters are cold, and the processes that opens.
    pub cold_knowledge: Vec<String>,
    pub cold_techniques: Vec<String>,
    /// …and the processes that knowledge opens: its bands' techniques.
    pub techniques: Vec<String>,
    /// Its population in the ecological cells (an animal species), if it has one.
    pub population: Option<String>,
    /// Its bodies: the player's physiology at a grown female's and a grown male's size.
    pub bodies: [BodyConfig; 2],
    /// Its coat of hair, or bare skin, covering the body as clothes do.
    pub coat: Worn,
    /// The shape of its days (V2.1 §6.1): when it sleeps, forages, rests, works, keeps company.
    pub routine: Vec<hearth_content::schema::mind::Block>,
}

fn sex(female: bool) -> usize {
    if female { 0 } else { 1 }
}

impl Species {
    pub fn does(&self, b: Behavior) -> bool {
        self.behaviors.contains(&b)
    }

    /// The physiology of one of its grown females or males.
    pub fn body(&self, female: bool) -> &BodyConfig {
        &self.bodies[sex(female)]
    }

    /// How grown one of an age is: the share of a grown one's mass, from a newborn's to all of
    /// it at maturity (its growth curve's).
    pub fn growth(&self, age_years: f64) -> f32 {
        self.life.grown_share(age_years).1
    }

    /// Its stage of life at an age.
    pub fn life_stage(&self, age_years: f64) -> LifeStage {
        self.life.stage(age_years)
    }

    /// The physiology of one of a sex, an age and a stature: a grown one's, or a growing one's
    /// by its size — its metabolism, skin, gaits, blood and stomach.
    pub fn body_at(&self, female: bool, age_years: f64, z: f32) -> Cow<'_, BodyConfig> {
        let base = self.body(female);
        if age_years >= self.life.maturity_years as f64 {
            return Cow::Borrowed(base);
        }
        let mass = self.mass_kg(female, age_years) as f64;
        let height = self.height_m(female, age_years, z) as f64;
        let mut c = body_of(base, mass, height);
        let share = (mass / base.mass_kg) as f32;
        c.params.blood_l = base.params.blood_l * share;
        c.params.stomach_capacity_l = base.params.stomach_capacity_l * share;
        Cow::Owned(c)
    }

    /// The mass (kg) of one of its persons of a sex and age.
    pub fn mass_kg(&self, female: bool, age_years: f64) -> f32 {
        self.mass_kg[sex(female)] * self.growth(age_years)
    }

    /// The standing height (m) of one of a sex and age (height goes as mass to the 0.4), of a
    /// stature `z` standard deviations from the species' middle.
    pub fn height_m(&self, female: bool, age_years: f64, z: f32) -> f32 {
        let grown = self.height_m[sex(female)] + z.clamp(-3.5, 3.5) * self.height_sd_m[sex(female)];
        grown * self.life.grown_share(age_years).0
    }

    /// The age class of an age, as the ecological cells count them: the young of the year, the
    /// young not yet grown, the grown.
    pub fn stage(&self, age_years: f64) -> Stage {
        if age_years < 1.0 {
            Stage::Young
        } else if age_years < self.life.maturity_years as f64 {
            Stage::Juvenile
        } else {
            Stage::Adult
        }
    }

    /// What a band of it knows, and the processes that opens: its own, and where the winters are
    /// cold what the cold asks besides.
    pub fn ways(&self, cold: bool) -> (Vec<String>, Vec<String>) {
        let mut knowledge = self.knowledge.clone();
        let mut techniques = self.techniques.clone();
        if cold {
            knowledge.extend(self.cold_knowledge.iter().cloned());
            techniques.extend(self.cold_techniques.iter().cloned());
        }
        knowledge.sort();
        knowledge.dedup();
        techniques.sort();
        techniques.dedup();
        (knowledge, techniques)
    }

    /// Whether one of an age is weaned (and has begun to learn its band's ways).
    pub fn weaned(&self, age_years: f64) -> bool {
        age_years >= self.life.weaning_years as f64
    }
}

/// A grown one's height (m) by a species profile (implemented or not: a player's): the middle
/// of its range for the sex, `z` standard deviations of a quarter of the range from it.
pub fn grown_height_m(
    profile: &hearth_content::schema::humans::Species,
    female: bool,
    z: f32,
) -> f32 {
    let r = if female {
        profile.body.height_m.female
    } else {
        profile.body.height_m.male
    };
    (r.0 + r.1) / 2.0 + z.clamp(-3.5, 3.5) * (r.1 - r.0) / 4.0
}

/// The player's physiology for a body of `mass_kg` and `height_m`: its skin area by DuBois, its
/// resting metabolism by Kleiber's three-quarter power, its gaits by its legs' length (the walk
/// that costs least goes as the square root of the leg).
pub fn body_of(base: &BodyConfig, mass_kg: f64, height_m: f64) -> BodyConfig {
    let mut c = base.clone();
    let legs = (height_m / base.height_m).sqrt() as f32;
    c.params.walk_m_s *= legs;
    c.params.jog_m_s *= legs;
    c.params.sprint_m_s *= legs;
    c.bmr_w = base.bmr_w * (mass_kg / base.mass_kg).powf(0.75);
    c.area_m2 = 0.007184 * mass_kg.powf(0.425) * (height_m * 100.0).powf(0.725);
    c.mass_kg = mass_kg;
    c.height_m = height_m;
    c
}

/// A coat of hair over all the body: about half a clo of insulation (a chimpanzee's), keeping
/// off some of the wind and a little of the rain.
fn hair() -> Worn {
    let mut w = Worn::naked();
    for r in w.regions.iter_mut() {
        r.clo = 0.5;
        r.wind = 0.3;
        r.water = 0.2;
    }
    w
}

impl Species {
    /// A species from its profile (implemented or not), its bodies made from the player's
    /// `base`.
    pub fn of_profile(
        s: &hearth_content::schema::humans::Species,
        graph: &Graph,
        base: &BodyConfig,
    ) -> Species {
        let node = |id: &str| {
            graph
                .node(id)
                .or_else(|| graph.node(&format!("hearth:{id}")))
        };
        let mid = |r: (f32, f32)| (r.0 + r.1) / 2.0;
        let height_m = [mid(s.body.height_m.female), mid(s.body.height_m.male)];
        let spread = |r: (f32, f32)| (r.1 - r.0) / 4.0;
        let height_sd_m = [spread(s.body.height_m.female), spread(s.body.height_m.male)];
        let mass_kg = [mid(s.body.mass_kg.female), mid(s.body.mass_kg.male)];
        let resolve = |list: &[hearth_content::IdRef]| -> Vec<String> {
            list.iter()
                .filter_map(|k| node(k.as_str()).map(|n| n.id.clone()))
                .collect()
        };
        let opens = |known: &[String]| -> Vec<String> {
            let mut t: Vec<String> = known
                .iter()
                .filter_map(|k| node(k))
                .flat_map(|n| n.enables.iter().cloned())
                .collect();
            t.sort();
            t.dedup();
            t
        };
        let knowledge = resolve(&s.knowledge);
        let techniques = opens(&knowledge);
        let cold_knowledge = resolve(&s.cold_knowledge);
        let cold_techniques = opens(&cold_knowledge);
        let bodies = [0, 1].map(|k| body_of(base, mass_kg[k] as f64, height_m[k] as f64));
        Species {
            id: s.id.clone(),
            name: s.name.clone(),
            plan: s.body.plan,
            height_m,
            mass_kg,
            height_sd_m,
            climbs: s.body.climbs,
            cognition: s.cognition.clone(),
            life: s.life.clone(),
            social: s.social.clone(),
            behaviors: s.behaviors.clone(),
            knowledge,
            techniques,
            cold_knowledge,
            cold_techniques,
            population: s.population.as_ref().map(|p| p.to_string()),
            bodies,
            coat: match s.body.coat {
                Coat::Hair => hair(),
                Coat::Bare => Worn::naked(),
            },
            routine: Vec::new(),
        }
    }

    /// With its routine from the content (a species without one keeps the default day: asleep
    /// from dusk to dawn).
    pub fn with_routine(mut self, c: &Content) -> Self {
        let key = |id: &str| id.rsplit(':').next().unwrap_or(id).to_owned();
        self.routine = c
            .routines
            .iter()
            .filter(|r| key(r.species.as_str()) == key(&self.id))
            .flat_map(|r| r.blocks.iter().cloned())
            .collect();
        self
    }

    /// Whether its people sleep at a local hour.
    pub fn asleep_at(&self, hour: f32) -> bool {
        use hearth_content::schema::mind::Routinely;
        let mut sleep = self
            .routine
            .iter()
            .filter(|b| b.doing == Routinely::Sleep)
            .peekable();
        if sleep.peek().is_none() {
            let h = hour.rem_euclid(24.0);
            return !(6.0..18.5).contains(&h);
        }
        sleep.any(|b| b.holds(hour))
    }

    /// How strongly its routine pulls toward something at a local hour.
    pub fn routinely(&self, doing: hearth_content::schema::mind::Routinely, hour: f32) -> f32 {
        self.routine
            .iter()
            .filter(|b| b.doing == doing && b.holds(hour))
            .map(|b| b.weight)
            .sum()
    }
}

/// Every species whose persons the game lives, and the genetic architecture they are read by.
#[derive(Debug, Clone, Default)]
pub struct SpeciesSet {
    pub list: Vec<Species>,
    pub genetics: Option<crate::genome::Genetics>,
    /// How temperament shows in behaviour, and the feelings' ways.
    pub psyche: crate::psyche::PsycheDefs,
    /// The life tables of the species that have them.
    pub life: crate::life::Tables,
    /// The peoples' norms and their sanctions (until cultures carry their own, H5).
    pub norms: crate::repute::Norms,
}

impl SpeciesSet {
    /// The implemented species profiles of the content, with bodies made from the player's
    /// `base`.
    pub fn from_content(c: &Content, graph: &Graph, base: &BodyConfig) -> Self {
        let list = c
            .species
            .iter()
            .filter(|s| s.status == Status::Implemented)
            .map(|s| Species::of_profile(s, graph, base).with_routine(c))
            .collect();
        Self {
            list,
            genetics: crate::genome::Genetics::from_content(c),
            psyche: crate::psyche::PsycheDefs::from_content(c),
            life: crate::life::Tables::from_content(c),
            norms: crate::repute::Norms::from_content(c),
        }
    }

    /// The index of a species by `namespace:path` or bare path.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.list
            .iter()
            .position(|k| k.id == id || k.id.ends_with(&format!(":{id}")))
    }

    pub fn get(&self, id: &str) -> Option<&Species> {
        self.index_of(id).map(|i| &self.list[i])
    }
}

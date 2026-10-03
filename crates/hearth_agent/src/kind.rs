//! Kinds of agent: a hominin species as its agents are made — a body of its size and build in
//! its coat, what its groups know and the techniques that knowledge opens, what it does.

use hearth_body::BodyConfig;
use hearth_body::clothing::Worn;
use hearth_content::Content;
use hearth_content::schema::Status;
use hearth_content::schema::era::Behavior;
use hearth_craft::Graph;

/// A kind of agent.
#[derive(Debug, Clone)]
pub struct Kind {
    pub id: String,
    pub name: String,
    /// A grown one's height (m) and mass (kg): the middle of its species' range.
    pub height_m: f32,
    pub mass_kg: f32,
    /// How many live together.
    pub group_size: (u16, u16),
    pub behaviors: Vec<Behavior>,
    /// What its groups know and practise (knowledge nodes)…
    pub knowledge: Vec<String>,
    /// …and the processes that knowledge opens: its culture's techniques.
    pub techniques: Vec<String>,
    /// Its population in the ecological cells (an animal species), if it has one.
    pub population: Option<String>,
    /// Its body: the player's physiology at its size and build.
    pub body: BodyConfig,
    /// Its coat of hair, covering the body as clothes do.
    pub coat: Worn,
}

impl Kind {
    pub fn does(&self, b: Behavior) -> bool {
        self.behaviors.contains(&b)
    }
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

/// Every kind of agent the content has.
#[derive(Debug, Clone, Default)]
pub struct Kinds {
    pub kinds: Vec<Kind>,
}

impl Kinds {
    /// The implemented hominins of the content, with bodies made from the player's `base`.
    pub fn from_content(c: &Content, graph: &Graph, base: &BodyConfig) -> Self {
        let node = |id: &str| {
            graph
                .node(id)
                .or_else(|| graph.node(&format!("hearth:{id}")))
        };
        let kinds = c
            .hominins
            .iter()
            .filter(|h| h.status == Status::Implemented)
            .map(|h| {
                let mass = (h.mass_kg.0 + h.mass_kg.1) as f64 / 2.0;
                let height = (h.height_m.0 + h.height_m.1) as f64 / 2.0;
                let knowledge: Vec<String> = h
                    .knowledge
                    .iter()
                    .filter_map(|k| node(k.as_str()).map(|n| n.id.clone()))
                    .collect();
                let mut techniques: Vec<String> = knowledge
                    .iter()
                    .filter_map(|k| node(k))
                    .flat_map(|n| n.enables.iter().cloned())
                    .collect();
                techniques.sort();
                techniques.dedup();
                Kind {
                    id: h.id.clone(),
                    name: h.name.clone(),
                    height_m: height as f32,
                    mass_kg: mass as f32,
                    group_size: (h.group_size.0 as u16, h.group_size.1 as u16),
                    behaviors: h.behaviors.clone(),
                    knowledge,
                    techniques,
                    population: h.population.as_ref().map(|p| p.to_string()),
                    body: body_of(base, mass, height),
                    coat: hair(),
                }
            })
            .collect();
        Self { kinds }
    }

    /// The index of a kind by `namespace:path` or bare path.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.kinds
            .iter()
            .position(|k| k.id == id || k.id.ends_with(&format!(":{id}")))
    }

    pub fn get(&self, id: &str) -> Option<&Kind> {
        self.index_of(id).map(|i| &self.kinds[i])
    }
}

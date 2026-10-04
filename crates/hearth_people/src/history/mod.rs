//! Deep time (V2.1 §15.1; H8, D191–D194): a fast abstract simulation of a world's peoples from
//! the first of them to the era's date, over the planet's own geography — peoples as demes on a
//! history grid, each with its numbers, its culture and language (a lineage), its gene pool and
//! what it knows; growth by what the land and the climate allow and what it knows, spread over
//! land and across water within its reach, the ice ages pushing it out of the north and letting
//! it back; lineages splitting, meeting and borrowing; techniques invented, spread and lost; and
//! a chronicle of what happened. Run once when the world is made and kept with it.
//! `docs/design/humans/history.md`.

pub mod climate;
pub mod deep;
pub mod geo;
pub mod replay;

use hearth_content::Content;
use hearth_content::schema::era::Era;
use hearth_content::schema::history::HistorySettings;
use hearth_craft::Graph;
use serde::{Deserialize, Serialize};

pub use climate::Climate;
pub use geo::{GeoCell, Geography};

/// Bumped when the saved history's form changes (an older one is made again).
pub const VERSION: u32 = 1;
/// The most techniques deep time follows (a mask's bits).
pub const TECHNIQUES: usize = 128;
/// A gene pool's drifting dimensions: the physical loci and traits that differ by place.
pub const POOL_DIMS: usize = 12;

/// Which of the techniques a deme knows, a bit each.
pub type Mask = u128;

/// Bits of a mask, saved as two words (JSON has no 128-bit numbers).
pub fn to_words(m: Mask) -> [u64; 2] {
    [m as u64, (m >> 64) as u64]
}

pub fn from_words(w: [u64; 2]) -> Mask {
    w[0] as Mask | ((w[1] as Mask) << 64)
}

/// A people's gene pool in deep time (§4.5): the sunlight its physical frequencies are adapted
/// to, its share of another species' ancestry, and how far drift has moved each of its
/// physical loci and traits (logit offsets of their frequencies). Behavioural loci have no place
/// here (ground rule 1).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Pool {
    pub sun: f32,
    pub archaic: f32,
    pub drift: [f32; POOL_DIMS],
}

/// A people of a cell at the era's date.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Deme {
    pub cell: u32,
    /// Its species (an index of [`History::species`]).
    pub species: u8,
    /// How many (persons).
    pub people: f32,
    /// Its culture and language.
    pub lineage: u32,
    /// What it knows (bits of [`History::techniques`]).
    pub know: [u64; 2],
    pub pool: Pool,
}

impl Deme {
    pub fn knows(&self) -> Mask {
        from_words(self.know)
    }
}

/// A culture with its language, and where it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lineage {
    pub id: u32,
    pub species: u8,
    /// The lineage it split from.
    pub parent: Option<u32>,
    /// When it began (years ago), and where.
    pub since_ya: f64,
    pub cell: u32,
    /// Its people at their most.
    pub peak: f32,
    /// The lineages it was most in contact with, and how much (person-years, of the fewer).
    pub contacts: Vec<(u32, f32)>,
}

/// What happened, for the chronicle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Happening {
    /// A species came to be.
    Appeared { species: u8 },
    /// A species reached a realm for the first time.
    Reached { species: u8, realm: u8 },
    /// A species reached a landmass for the first time, over the sea (else over land, a land
    /// bridge where the sea had stood).
    Crossed {
        species: u8,
        landmass: u32,
        by_sea: bool,
    },
    /// A technique invented for the first time by a species, by a lineage.
    Invented {
        species: u8,
        technique: u16,
        lineage: u32,
    },
    /// A technique no longer known anywhere on a landmass where a species had known it.
    Lost {
        species: u8,
        technique: u16,
        landmass: u32,
    },
    /// A lineage split off from another.
    Split { lineage: u32, daughter: u32 },
    /// Two species met for the first time.
    Met { a: u8, b: u8 },
    /// A species no longer lived in a realm it had lived in.
    Left { species: u8, realm: u8 },
    /// A species was gone.
    Gone { species: u8 },
}

/// An event of the chronicle: when (years ago), where (a cell) and what.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub ya: f64,
    pub cell: u32,
    pub what: Happening,
}

/// A world's deep past, as it stands at its era's date.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct History {
    pub version: u32,
    pub seed: u64,
    pub era: String,
    /// The era's date (years ago).
    pub years_ago: f64,
    /// The grid: cells along the circumference, a cell's edge (blocks).
    pub n: usize,
    pub cell_m: f64,
    /// How much denser than real its peoples live, on a planet smaller than a people's country.
    pub denser: f32,
    /// The most years of a lineage's branch its culture and language are replayed for.
    pub branch_years: f64,
    /// The species of person in it (ids), the population each lives as in the ecological cells.
    pub species: Vec<String>,
    pub populations: Vec<Option<String>>,
    /// The techniques it follows (knowledge ids), a bit each.
    pub techniques: Vec<String>,
    /// Each cell's share of land at the era's sea level.
    pub land: Vec<f32>,
    pub demes: Vec<Deme>,
    pub lineages: Vec<Lineage>,
    pub chronicle: Vec<Event>,
    /// Each species' numbers, every ten thousand years.
    pub census: Vec<Census>,
}

/// A census of deep time: when (years ago), each species' people, the sea's level then.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Census {
    pub ya: f64,
    pub people: Vec<f32>,
    pub sea_m: f32,
}

/// A species of person as deep time runs it.
#[derive(Debug, Clone, PartialEq)]
pub struct Kind {
    pub id: String,
    pub population: Option<String>,
    /// When it appears and is gone (years ago).
    pub appears_ya: f64,
    pub gone_ya: Option<f64>,
    /// Where it appears (a realm), and from which kind's people there (none: the cradle).
    pub realm: Option<u8>,
    pub from: Option<usize>,
    pub growth: f32,
    pub spread_km_year: f32,
    pub lineage_people: f32,
    /// Its numbers at their height on Earth.
    pub people: f32,
    /// Persons a km² of open savanna–woodland knowing nothing that wins more food, and how much
    /// colder a winter its body bears than ours (°C).
    pub density: f32,
    pub cold_c: f32,
    /// The techniques it may come to know.
    pub repertoire: Mask,
    /// One of the era's own peoples (else only an ancestor of one).
    pub in_era: bool,
}

/// A technique as deep time follows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Technique {
    pub id: String,
    /// The record's first sign of it (years ago): it is not invented before.
    pub years_bp: f64,
    pub requires: Mask,
    pub depth: u8,
    /// A factor on the land's yield (food won), on water's (fishing gear), and a reach across
    /// the sea (km, craft).
    pub food: f32,
    pub fishing: f32,
    pub reach_km: f32,
}

/// What a run needs: the era's peoples and their ancestors, the techniques, the settings.
#[derive(Debug, Clone)]
pub struct Setup {
    pub seed: u64,
    pub era: String,
    pub until_ya: f64,
    pub kinds: Vec<Kind>,
    pub techniques: Vec<Technique>,
    /// What every people of the era knows besides.
    pub baseline: Mask,
    /// The coldest month each set of techniques lets a people winter in.
    pub cold: Vec<(Mask, f32)>,
    pub settings: HistorySettings,
}

fn key(id: &str) -> &str {
    id.rsplit(':').next().unwrap_or(id)
}

/// The worldgen realm by its name.
pub fn realm_index(name: &str) -> Option<u8> {
    hearth_worldgen::realms::Realm::parse(name).map(|r| r as u8)
}

impl Setup {
    /// The run for a world of `era` (none for an era without peoples, as Wild Earth).
    pub fn of_era(c: &Content, graph: &Graph, era: &Era, seed: u64) -> Option<Self> {
        if era.peoples.is_empty() {
            return None;
        }
        let settings = c
            .history
            .iter()
            .find(|h| h.status == hearth_content::schema::Status::Implemented)?
            .clone();
        let until_ya = era.years_bp.unwrap_or(0.0);
        // The era's peoples and those they come from.
        let mut ids: Vec<String> = Vec::new();
        for p in &era.peoples {
            let mut id = Some(p.species.to_string());
            while let Some(s) = id {
                if ids.iter().any(|x| key(x) == key(&s)) {
                    break;
                }
                let sp = c.species.iter().find(|x| key(&x.id) == key(&s))?;
                ids.push(sp.id.clone());
                id = sp
                    .origin
                    .as_ref()
                    .and_then(|o| o.from.as_ref())
                    .map(|f| f.to_string());
            }
        }
        // Oldest first.
        ids.sort_by(|a, b| {
            let ya = |id: &str| {
                c.species
                    .iter()
                    .find(|x| key(&x.id) == key(id))
                    .and_then(|s| s.first_appearance_ya)
                    .unwrap_or(0.0)
            };
            ya(b).total_cmp(&ya(a)).then(a.cmp(b))
        });
        let lore = crate::learning::Lore::from_graph(graph);
        // What each may know, by name: the era's list or graph eras, else its profile's.
        let mut wants: Vec<Vec<String>> = Vec::new();
        for id in &ids {
            let sp = c.species.iter().find(|x| key(&x.id) == key(id))?;
            let ep = era
                .peoples
                .iter()
                .find(|p| key(p.species.as_str()) == key(id));
            let ceiling = sp.cognition.era_ceiling;
            let mut list: Vec<String> = match ep {
                Some(p) if p.reach.is_some() => {
                    let reach = p.reach.unwrap_or(0).min(ceiling);
                    c.knowledge
                        .iter()
                        .filter(|k| k.era <= reach)
                        .map(|k| k.id.clone())
                        .collect()
                }
                Some(p) if !p.repertoire.is_empty() => {
                    p.repertoire.iter().map(|k| k.to_string()).collect()
                }
                _ => sp
                    .knowledge
                    .iter()
                    .chain(&sp.cold_knowledge)
                    .map(|k| k.to_string())
                    .collect(),
            };
            list.retain(|k| {
                c.knowledge
                    .iter()
                    .find(|x| key(&x.id) == key(k))
                    .is_some_and(|x| x.era <= ceiling)
            });
            wants.push(list);
        }
        // The techniques: every one wanted, with what it rests on.
        let mut tech: Vec<String> = Vec::new();
        let mut stack: Vec<String> = wants.iter().flatten().cloned().collect();
        while let Some(t) = stack.pop() {
            if tech.iter().any(|x| key(x) == key(&t)) {
                continue;
            }
            let Some(k) = c.knowledge.iter().find(|x| key(&x.id) == key(&t)) else {
                continue;
            };
            tech.push(k.id.clone());
            stack.extend(k.requires.iter().map(|r| r.to_string()));
        }
        tech.sort_by(|a, b| {
            let d = |id: &str| {
                lore.nodes
                    .iter()
                    .find(|n| key(&n.id) == key(id))
                    .map_or(0, |n| n.depth)
            };
            d(a).cmp(&d(b)).then(a.cmp(b))
        });
        if tech.len() > TECHNIQUES {
            log::warn!(
                "deep time follows {} of {} techniques",
                TECHNIQUES,
                tech.len()
            );
            tech.truncate(TECHNIQUES);
        }
        let bit = |id: &str| -> Mask {
            tech.iter()
                .position(|x| key(x) == key(id))
                .map_or(0, |i| 1 << i)
        };
        let mask = |list: &[String]| list.iter().fold(0 as Mask, |m, k| m | bit(k));
        let techniques: Vec<Technique> = tech
            .iter()
            .map(|id| {
                let k = c.knowledge.iter().find(|x| x.id == *id);
                let gives = |list: &[hearth_content::schema::history::KnownGives]| {
                    list.iter()
                        .find(|g| key(g.knowledge.as_str()) == key(id))
                        .map(|g| g.gives)
                };
                Technique {
                    id: id.clone(),
                    years_bp: k.map_or(0.0, |k| k.history.years_bp),
                    requires: k
                        .map_or(0, |k| k.requires.iter().fold(0, |m, r| m | bit(r.as_str()))),
                    depth: lore
                        .nodes
                        .iter()
                        .find(|n| key(&n.id) == key(id))
                        .map_or(0, |n| n.depth),
                    food: gives(&settings.food).unwrap_or(1.0),
                    fishing: gives(&settings.fishing).unwrap_or(1.0),
                    reach_km: gives(&settings.craft).unwrap_or(0.0),
                }
            })
            .collect();
        // Each wanted technique with all it rests on.
        let closure = |m: Mask| {
            let mut m = m;
            loop {
                let more = techniques
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| m & (1 << i) != 0)
                    .fold(m, |acc, (_, t)| acc | t.requires);
                if more == m {
                    return m;
                }
                m = more;
            }
        };
        let mut kinds = Vec::new();
        for (k, id) in ids.iter().enumerate() {
            let sp = c.species.iter().find(|x| key(&x.id) == key(id))?;
            let deep = sp.deep.as_ref()?;
            let origin = sp.origin.as_ref();
            kinds.push(Kind {
                id: sp.id.clone(),
                population: sp.population.as_ref().map(|p| p.to_string()),
                appears_ya: sp.first_appearance_ya.unwrap_or(until_ya),
                gone_ya: sp.extinction_ya,
                realm: origin.and_then(|o| realm_index(&o.realm)),
                from: origin
                    .and_then(|o| o.from.as_ref())
                    .and_then(|f| ids.iter().position(|x| key(x) == key(f.as_str()))),
                growth: deep.growth,
                spread_km_year: deep.spread_km_year,
                lineage_people: deep.lineage_people,
                people: deep.people,
                density: deep.density,
                cold_c: deep.cold_c,
                repertoire: closure(mask(&wants[k])),
                in_era: era
                    .peoples
                    .iter()
                    .any(|p| key(p.species.as_str()) == key(id)),
            });
        }
        // The cold steps its techniques can reach (a step needing one not followed is none).
        let cold = settings
            .cold
            .iter()
            .filter(|s| s.needs.iter().all(|k| bit(k.as_str()) != 0))
            .map(|s| {
                (
                    s.needs.iter().fold(0 as Mask, |m, k| m | bit(k.as_str())),
                    s.coldest_c,
                )
            })
            .collect();
        let baseline = era
            .knowledge_baseline
            .iter()
            .fold(0 as Mask, |m, k| m | bit(k.as_str()));
        Some(Self {
            seed,
            era: era.id.clone(),
            until_ya,
            kinds,
            techniques,
            baseline,
            cold,
            settings,
        })
    }

    /// The bit of a technique.
    pub fn bit(&self, id: &str) -> Mask {
        self.techniques
            .iter()
            .position(|t| key(&t.id) == key(id))
            .map_or(0, |i| 1 << i)
    }
}

impl History {
    /// The cell of a world position.
    pub fn cell_at(&self, x: f64, z: f64) -> usize {
        let c = self.cell_m * self.n as f64;
        let i = ((x.rem_euclid(c)) / self.cell_m).floor() as i64;
        let j = ((z + c * 0.5) / self.cell_m).floor() as i64;
        let n = self.n as i64;
        (j.clamp(0, n - 1) * n + i.clamp(0, n - 1)) as usize
    }

    /// A species' index by its id or its population's.
    pub fn species_index(&self, id: &str) -> Option<usize> {
        self.species
            .iter()
            .position(|s| key(s) == key(id))
            .or_else(|| {
                self.populations
                    .iter()
                    .position(|p| p.as_deref().is_some_and(|p| key(p) == key(id)))
            })
    }

    /// The demes of a cell.
    pub fn demes_in(&self, cell: usize) -> impl Iterator<Item = &Deme> {
        let c = cell as u32;
        let from = self.demes.partition_point(|d| d.cell < c);
        self.demes[from..].iter().take_while(move |d| d.cell == c)
    }

    /// The deme of a species at a world position (the nearest cell it lives in, within a few).
    pub fn deme_at(&self, species: usize, x: f64, z: f64) -> Option<&Deme> {
        let here = self.cell_at(x, z);
        let n = self.n as i64;
        let (i, j) = ((here % self.n) as i64, (here / self.n) as i64);
        let mut best: Option<(i64, &Deme)> = None;
        for r in 0..=3i64 {
            for dj in -r..=r {
                for di in -r..=r {
                    if di.abs().max(dj.abs()) != r || !(0..n).contains(&(j + dj)) {
                        continue;
                    }
                    let c = ((j + dj) * n + (i + di).rem_euclid(n)) as usize;
                    for d in self.demes_in(c).filter(|d| d.species as usize == species) {
                        let dist = di * di + dj * dj;
                        if best
                            .is_none_or(|(b, bd)| dist < b || (dist == b && d.people > bd.people))
                        {
                            best = Some((dist, d));
                        }
                    }
                }
            }
            if best.is_some() {
                break;
            }
        }
        best.map(|(_, d)| d)
    }

    /// Persons a km² of land of a species (by its id or its population's) at a world position.
    pub fn density(&self, id: &str, x: f64, z: f64) -> Option<f32> {
        let s = self.species_index(id)?;
        let cell = self.cell_at(x, z);
        let km2 = (self.cell_m / 1000.0).powi(2) as f32;
        let land = self.land.get(cell).copied().unwrap_or(0.0);
        let people: f32 = self
            .demes_in(cell)
            .filter(|d| d.species as usize == s)
            .map(|d| d.people)
            .sum();
        Some(if land > 0.0 {
            people / (km2 * land)
        } else {
            0.0
        })
    }

    /// The people of a species on the planet.
    pub fn total(&self, species: usize) -> f64 {
        self.demes
            .iter()
            .filter(|d| d.species as usize == species)
            .map(|d| d.people as f64)
            .sum()
    }

    pub fn lineage(&self, id: u32) -> Option<&Lineage> {
        let k = self.lineages.partition_point(|l| l.id < id);
        self.lineages.get(k).filter(|l| l.id == id)
    }

    /// The techniques of a mask, by id.
    pub fn known(&self, m: Mask) -> Vec<String> {
        self.techniques
            .iter()
            .enumerate()
            .filter(|(i, _)| m & (1 << i) != 0)
            .map(|(_, t)| t.clone())
            .collect()
    }

    /// Saved, compressed.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        let json = serde_json::to_vec(self).map_err(std::io::Error::other)?;
        let packed = zstd::encode_all(&json[..], 9)?;
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(path, packed)
    }

    /// Loaded, if it is there and of this version.
    pub fn load(path: &std::path::Path) -> Option<Self> {
        let packed = std::fs::read(path).ok()?;
        let json = zstd::decode_all(&packed[..]).ok()?;
        let h: Self = serde_json::from_slice(&json).ok()?;
        (h.version == VERSION).then_some(h)
    }
}

impl hearth_fauna::ecology::Peopling for History {
    fn density(&self, population: &str, x: f64, z: f64) -> Option<f32> {
        History::density(self, population, x, z)
    }
}

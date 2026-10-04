//! A world's era (V2.1 §15.3; H8): which peoples live in it, its deep past — run when the world
//! is made and kept with it — and where a life begins among its people.

use std::path::Path;
use std::sync::Arc;

use glam::{DVec2, DVec3};
use hearth_content::Content;
use hearth_content::schema::era::{Era, Toward};
use hearth_craft::Graph;
use hearth_people::history::{Geography, History, Setup, deep};

use crate::scene::LocalWorld;

/// The deep past's file in a world's folder.
pub const FILE: &str = "history.json.zst";
/// Wild Earth: the era of worlds made before eras.
pub const WILD_EARTH: &str = "hearth:wild_earth";

fn key(id: &str) -> &str {
    id.rsplit(':').next().unwrap_or(id)
}

/// An era by id (Wild Earth for an unknown one).
pub fn era_of<'a>(content: &'a Content, id: &str) -> Option<&'a Era> {
    content
        .eras
        .iter()
        .find(|e| key(&e.id) == key(id))
        .or_else(|| content.eras.iter().find(|e| key(&e.id) == key(WILD_EARTH)))
}

/// The populations of the peoples living in an era, each with its bands' size where the era
/// says: its peoples (H8); for Wild Earth, its hominins and the wandering families (D164).
pub fn peoples(content: &Content, era: Option<&Era>) -> Vec<(String, Option<(u16, u16)>)> {
    let species = |id: &str| content.species.iter().find(|s| key(&s.id) == key(id));
    match era {
        Some(e) if !e.peoples.is_empty() => e
            .peoples
            .iter()
            .filter_map(|p| {
                let pop = species(p.species.as_str())?.population.as_ref()?;
                let size = (p.band_size.0.round() as u16, p.band_size.1.round() as u16);
                Some((pop.to_string(), Some(size)))
            })
            .collect(),
        _ => content
            .species
            .iter()
            .filter(|s| {
                s.status == hearth_content::schema::Status::Implemented
                    && (s.worldwide
                        || era.is_some_and(|e| {
                            e.species.iter().any(|x| key(x.as_str()) == key(&s.id))
                        }))
            })
            .filter_map(|s| s.population.as_ref().map(|p| (p.to_string(), None)))
            .collect(),
    }
}

/// A world's deep past (V2.1 §15.1): read from `file` if it is there (of this era and seed), else
/// run over the planet and kept there. `None` for an era without peoples.
pub fn history(
    lw: &LocalWorld,
    content: &Content,
    graph: &Graph,
    era: &Era,
    seed: u64,
    file: Option<&Path>,
    progress: &(dyn Fn(f32) + Sync),
) -> Option<Arc<History>> {
    let setup = Setup::of_era(content, graph, era, seed)?;
    if let Some(h) = file.and_then(History::load)
        && h.seed == seed
        && key(&h.era) == key(&era.id)
    {
        return Some(Arc::new(h));
    }
    let t0 = std::time::Instant::now();
    let genetics = hearth_people::Genetics::from_content(content);
    let sun = move |lat: f64| genetics.as_ref().map_or(1.0, |g| g.sunlight(lat));
    let geo = Geography::of_terrain(lw.terrain(), &setup.settings, &sun);
    let h = deep::run(&setup, &geo, progress);
    log::info!(
        "deep time to {} ran in {:.1} s: {} demes, {} lineages, {} events",
        era.name,
        t0.elapsed().as_secs_f64(),
        h.demes.len(),
        h.lineages.len(),
        h.chronicle.len()
    );
    if let Some(f) = file
        && let Err(e) = h.save(f)
    {
        log::warn!("could not keep the deep past: {e}");
    }
    Some(Arc::new(h))
}

/// Where a life begins in an era's world: `spawn` if one of the era's peoples lives well within
/// a day's walk of it — as many there as in half the places they live, not a thin frontier — else
/// the middle of the nearest cell where one does.
pub fn spawn_among(h: &History, era: &Era, spawn: DVec2) -> DVec2 {
    let in_era: Vec<usize> = era
        .peoples
        .iter()
        .filter_map(|p| h.species_index(p.species.as_str()))
        .collect();
    let mut peopled: Vec<f32> = h
        .demes
        .iter()
        .filter(|d| in_era.contains(&(d.species as usize)) && d.people >= 0.5)
        .map(|d| d.people)
        .collect();
    peopled.sort_by(f32::total_cmp);
    let median = peopled.get(peopled.len() / 2).copied().unwrap_or(0.5);
    let lives = |d: &hearth_people::history::Deme| {
        in_era.contains(&(d.species as usize)) && d.people >= median.max(0.5)
    };
    let n = h.n;
    let centre = |cell: usize| {
        let c = h.cell_m * n as f64;
        DVec2::new(
            ((cell % n) as f64 + 0.5) * h.cell_m,
            -c * 0.5 + ((cell / n) as f64 + 0.5) * h.cell_m,
        )
    };
    let wrap = h.cell_m * n as f64;
    let dist = |a: DVec2, b: DVec2| {
        let dx = (a.x - b.x).rem_euclid(wrap);
        dx.min(wrap - dx).hypot(a.y - b.y)
    };
    let best = h
        .demes
        .iter()
        .filter(|d| lives(d))
        .map(|d| (dist(centre(d.cell as usize), spawn), d.cell))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    match best {
        Some((d, _)) if d <= 20_000.0 => spawn,
        Some((_, cell)) => centre(cell as usize),
        None => spawn,
    }
}

/// A place's ground in 3D (the land's top there).
pub fn ground(lw: &LocalWorld, at: DVec2) -> DVec3 {
    DVec3::new(at.x, lw.surface_y(at.x, at.y), at.y)
}

/// The land as an era's peoples choose their camps in it (V2.1 §15.3; H8): read from the
/// terrain's columns about a band's home, each kind of place once (a round's camps are its
/// people's habit, the same places year after year) — the best few kept, so that a band finds
/// the best not taken by another's camp.
pub struct Lands {
    terrain: Arc<hearth_worldgen::region::Terrain>,
    found: std::sync::Mutex<rustc_hash::FxHashMap<(i64, i64, u8), Vec<DVec3>>>,
}

/// The best places of a kind kept for a band's home.
const KEPT: usize = 12;

impl Lands {
    pub fn new(terrain: Arc<hearth_worldgen::region::Terrain>) -> Self {
        Self {
            terrain,
            found: Default::default(),
        }
    }

    /// The ground's height where a camp may be made: dry — a block over any water there and two
    /// over the sea's level (the samples round a shore otherwise than its blocks), and so a few
    /// steps about it — and not steep.
    fn dry(&self, x: f64, z: f64) -> Option<f32> {
        let t = &self.terrain;
        let dry_at = |x: f64, z: f64| {
            let s = t.sample(x.floor() as i32, z.floor() as i32);
            let wet = s.is_underwater()
                || s.ocean
                || s.lake
                || (s.water.is_finite() && s.height_i() <= s.water_i() + 1)
                || s.height < 2.0
                || s.river
                    .as_ref()
                    .is_some_and(|r| r.distance < r.width * 0.5 + 2.0);
            (!wet && s.slope < 0.8).then_some(s.height)
        };
        let h = dry_at(x, z)?;
        [(6.0, 0.0), (-6.0, 0.0), (0.0, 6.0), (0.0, -6.0)]
            .iter()
            .all(|(dx, dz)| dry_at(x + dx, z + dz).is_some())
            .then_some(h)
    }

    /// The best camps of a kind about a place, best first (see [`hearth_people::Country`]).
    fn seek(&self, toward: Toward, from: DVec2, within_m: f64) -> Vec<DVec3> {
        let t = &self.terrain;
        let sample = |x: f64, z: f64| t.sample(x.floor() as i32, z.floor() as i32);
        let wet = |s: &hearth_worldgen::region::ColumnSample| {
            s.is_underwater() || s.river.as_ref().is_some_and(|r| r.distance < r.width * 0.5)
        };
        // Places on rings about the home, within its range.
        let reach = within_m.clamp(300.0, 12_000.0) * 0.8;
        let mut found: Vec<(f32, DVec3)> = Vec::new();
        for ring in 1..=5 {
            let r = reach * ring as f64 / 5.0;
            let n = 8 + 4 * ring;
            for k in 0..n {
                let a = (k as f64 + 0.5 * (ring % 2) as f64) / n as f64 * std::f64::consts::TAU;
                let (x, z) = (from.x + a.cos() * r, from.y + a.sin() * r);
                let s = sample(x, z);
                if s.slope > 0.6 || self.dry(x, z).is_none() {
                    continue;
                }
                // The country about it: how high it stands over it, and water near.
                let mut around = 0.0f32;
                let mut water_m: Option<f64> = None;
                for (j, d) in [(0, 40.0), (1, 90.0), (2, 160.0), (3, 300.0)] {
                    for q in 0..8 {
                        let b = (q as f64 + 0.25 * j as f64) / 8.0 * std::f64::consts::TAU;
                        let c = sample(x + b.cos() * d, z + b.sin() * d);
                        if wet(&c) {
                            water_m = Some(water_m.map_or(d, |w: f64| w.min(d)));
                        }
                        if j == 3 {
                            around += c.height / 8.0;
                        }
                    }
                }
                let rise = s.height - around;
                let open = 1.0 - s.tree_density.clamp(0.0, 1.0);
                let score = match toward {
                    // By a river, a lake or the shore: the nearer the water the better.
                    Toward::Water => match water_m {
                        Some(w) => 2.0 - (w / 160.0) as f32 + 0.2 * open,
                        None => continue,
                    },
                    // Up on the open high ground, looking out over the country.
                    Toward::Uplands => {
                        if s.tree_density > 0.6 {
                            continue;
                        }
                        (rise / 20.0).clamp(-1.0, 2.0) + 0.6 * open
                    }
                    // Down in the woods' shelter, out of the wind, water not far.
                    Toward::Shelter => {
                        (-rise / 20.0).clamp(-1.0, 1.5)
                            + 1.2 * s.tree_density.clamp(0.0, 1.0)
                            + if water_m.is_some_and(|w| w <= 160.0) {
                                0.4
                            } else {
                                0.0
                            }
                    }
                } - 0.3 * (r / reach) as f32;
                found.push((score, DVec3::new(x, s.height as f64, z)));
            }
        }
        found.sort_by(|a, b| b.0.total_cmp(&a.0));
        found.into_iter().take(KEPT).map(|(_, at)| at).collect()
    }
}

impl hearth_people::Country for Lands {
    fn camp_toward(
        &self,
        toward: Toward,
        from: DVec2,
        within_m: f64,
        taken: &[DVec2],
    ) -> Option<DVec3> {
        let k = (
            (from.x / 100.0).round() as i64,
            (from.y / 100.0).round() as i64,
            toward as u8,
        );
        let cached = self.found.lock().ok().and_then(|f| f.get(&k).cloned());
        let best = match cached {
            Some(b) => b,
            None => {
                let b = self.seek(toward, from, within_m);
                if let Ok(mut f) = self.found.lock() {
                    f.insert(k, b.clone());
                }
                b
            }
        };
        let free = |p: &DVec3| {
            taken.iter().all(|t| {
                (DVec2::new(p.x, p.z) - *t).length() >= hearth_people::rounds::CAMPS_APART_M
            })
        };
        // All taken: none (the band stays where it is).
        best.iter().find(|p| free(p)).copied()
    }

    fn dry_ground(
        &self,
        near: DVec2,
        within_m: f64,
        taken: &[DVec2],
        apart_m: f64,
    ) -> Option<DVec3> {
        let dry = |x: f64, z: f64| {
            let free = taken
                .iter()
                .all(|t| (DVec2::new(x, z) - *t).length() >= apart_m);
            if !free {
                return None;
            }
            self.dry(x, z).map(|h| DVec3::new(x, h as f64, z))
        };
        if let Some(at) = dry(near.x, near.y) {
            return Some(at);
        }
        // Rings outward, nearest first, as close as the camps may be.
        let step = (apart_m * 0.5).clamp(5.0, 40.0);
        let mut r = step;
        while r <= within_m {
            let n = ((std::f64::consts::TAU * r / step).ceil() as usize).max(8);
            for k in 0..n {
                let a = k as f64 / n as f64 * std::f64::consts::TAU;
                if let Some(at) = dry(near.x + a.cos() * r, near.y + a.sin() * r) {
                    return Some(at);
                }
            }
            r += step;
        }
        None
    }
}

/// How an era's peoples live through the year in a world (none outside the eras), and the land
/// they live in (every world's: a band split off goes to dry land in Wild Earth too).
pub fn ways(
    era: Option<&Era>,
    calendar: &hearth_env::Calendar,
    terrain: Arc<hearth_worldgen::region::Terrain>,
) -> hearth_people::EraWays {
    hearth_people::EraWays {
        peoples: era.map(|e| e.peoples.clone()).unwrap_or_default(),
        year_offset: calendar.year_offset,
        country: Some(Arc::new(Lands::new(terrain))),
    }
}

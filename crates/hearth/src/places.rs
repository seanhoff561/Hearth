//! Where a life may begin (Amendment E §6.3): a few suggested places on a new world, chosen for
//! variety and for being survivable in a loincloth at the starting date, each with what to look
//! for and what to watch out for there — every claim found by sampling the generated world about
//! the spot (its rivers, lakes and springs, the stones lying on the ground, the trees and plants
//! growing there, the animals that live there, the weather of the season). A place whose claims
//! cannot be found is passed over.

use std::sync::Arc;

use hearth_content::Content;
use hearth_content::schema::Season;
use hearth_content::schema::flora::{Edibility, PartKind};
use hearth_env::Normals;
use hearth_fauna::habitat::{GenLand, TreeYields};
use hearth_fauna::{Catalog, Land};
use hearth_math::BlockPos;
use hearth_world::BlockRegistry;
use hearth_worldgen::WorldGenerator;
use hearth_worldgen::planet::climate::ClimateClass;
use hearth_worldgen::region::ColumnSample;
use hearth_worldgen::vegetation::Vegetation;

/// How hard a place is to begin in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Difficulty {
    Gentle,
    Challenging,
    Harsh,
}

impl Difficulty {
    pub fn key(self) -> &'static str {
        match self {
            Difficulty::Gentle => "places.difficulty.gentle",
            Difficulty::Challenging => "places.difficulty.challenging",
            Difficulty::Harsh => "places.difficulty.harsh",
        }
    }
}

/// Words to show: a language key and what fills it.
#[derive(Debug, Clone, PartialEq)]
pub struct Words {
    pub key: &'static str,
    pub args: Vec<(&'static str, String)>,
}

impl Words {
    fn new(key: &'static str, args: &[(&'static str, String)]) -> Self {
        Self {
            key,
            args: args.to_vec(),
        }
    }
}

/// What a claim says is there, found where.
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    /// Fresh water: a river or stream (its width, m), a lake, a spring.
    River {
        width_m: f32,
    },
    Lake,
    Spring,
    /// The sea (salt: not to drink).
    Sea,
    /// Stones good for knapping lying on the ground (the rock's name).
    Toolstone(String),
    /// Trees (their names, most common first).
    Trees(Vec<String>),
    /// Plants giving fibre for cordage.
    Fibre(Vec<String>),
    /// Food in season (plant and part).
    Food(Vec<(String, PartKind)>),
    /// Tinder.
    Tinder(Vec<String>),
}

/// A claim about a place: what is there and how far which way (none: at the spot).
#[derive(Debug, Clone, PartialEq)]
pub struct Claim {
    pub found: Found,
    pub at: Option<(f64, f64)>,
}

/// A danger of a place.
#[derive(Debug, Clone, PartialEq)]
pub enum Danger {
    /// Animals that may hunt a person (their names).
    Predators(Vec<String>),
    /// Of those, the ones of the water's edge.
    AtTheWater(Vec<String>),
    Venomous(Vec<String>),
    /// Animals that defend themselves hard.
    Defensive(Vec<String>),
    /// Nights this cold before dawn (°C).
    ColdNights(f64),
    /// Afternoons this hot (°C).
    Heat(f64),
    /// High: the air this much thinner (m above the sea).
    Height(f64),
    /// The only water near is the sea's.
    SaltWater,
}

/// A suggested place.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    /// Where (world x, z; the ground there).
    pub x: i32,
    pub z: i32,
    /// "A chalk coast in a mild, wet climate."
    pub name: Words,
    /// Its climate and the season there now.
    pub climate: Words,
    /// Its land: the biome and height.
    pub terrain: Words,
    pub difficulty: Difficulty,
    pub look_for: Vec<Claim>,
    pub watch_out: Vec<Danger>,
    /// The night's low and the day's high now (°C).
    pub low_c: f64,
    pub high_c: f64,
}

/// When the first life begins: a share of the year from the March equinox, by place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum When {
    /// Spring wherever it is (a spring dawn).
    Spring,
    /// This moment of the year everywhere.
    YearFrac(f64),
}

impl When {
    /// The year's share at a latitude, and its season there.
    fn at(self, lat_deg: f64) -> (f64, Season) {
        let southern = lat_deg < 0.0;
        let yf = match self {
            When::Spring => 20.0 / 365.24 + if southern { 0.5 } else { 0.0 },
            When::YearFrac(f) => f,
        };
        let local = if southern { (yf + 0.5).fract() } else { yf };
        let season = match (local * 4.0) as u32 {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Autumn,
            _ => Season::Winter,
        };
        (yf, season)
    }
}

/// How far about a spot its claims are sought (m).
const REACH_M: f64 = 1500.0;
/// The coldest night and hottest afternoon a person in a loincloth gets through, sheltering
/// and with a fire to make (°C): suggested places stay within them.
const COLDEST_C: f64 = 2.0;
const HOTTEST_C: f64 = 40.0;
/// Suggested places lie this far apart at least (km).
const APART_KM: f64 = 1500.0;

/// The world as the suggestions read it.
pub struct Finder {
    pub wg: Arc<WorldGenerator>,
    content: Arc<Content>,
    reg: Arc<BlockRegistry>,
    catalog: Catalog,
    yields: TreeYields,
    veg: Vegetation,
}

impl Finder {
    pub fn new(wg: Arc<WorldGenerator>, content: Arc<Content>, reg: Arc<BlockRegistry>) -> Self {
        let catalog = Catalog::new(&content);
        let yields = TreeYields::new(&wg, &content);
        let veg = Vegetation::default();
        Self {
            wg,
            content,
            reg,
            catalog,
            yields,
            veg,
        }
    }

    /// Three to five places for a first life at `when`, of different climates and lands, each
    /// verified (fewer only if the planet has no more).
    pub fn suggest(&self, when: When, seed: u64) -> Vec<Place> {
        let g = &*self.wg.terrain.grid;
        let n = g.n();
        let mut rng = hearth_math::hash::Rng::new(seed ^ 0x0051_ace5);
        // Land cells to begin in, scored: fresh water at hand, a mild season, low and gentle.
        let mut cands: Vec<(f32, ClimateClass, f64, f64)> = Vec::new();
        for _ in 0..6000 {
            let i = rng.below(n as u32) as usize;
            let j = rng.below(n as u32) as usize;
            let idx = g.geom.idx(i, j);
            let e = g.elevation.data[idx];
            if !(3.0..2500.0).contains(&e) {
                continue;
            }
            let class = g.climate_at(idx);
            if matches!(
                class,
                ClimateClass::Ocean | ClimateClass::IceCap | ClimateClass::Tundra
            ) {
                continue;
            }
            let (x, z) = g.geom.world_xz(i, j);
            let lat = self.wg.planet().latitude_deg(z);
            let (yf, _) = when.at(lat);
            let normals = Normals::sample(g, x, z);
            let (low, high) = day_range(&normals, yf);
            if low < COLDEST_C || high > HOTTEST_C {
                continue;
            }
            let river = g.flags[idx] & hearth_worldgen::planet::flags::RIVER != 0;
            let coast = g.field_at(&g.coast, idx) < 0.02;
            let mild = 1.0 - ((low + high) / 2.0 - 18.0).abs() as f32 / 20.0;
            // Coasts, plenty as they are, no better than inland.
            let score =
                mild + if river { 1.0 } else { 0.0 } - if coast { 0.2 } else { 0.0 } - e / 2000.0
                    + rng.next_f32() * 0.3;
            cands.push((score, class, x, z));
        }
        cands.sort_by(|a, b| b.0.total_cmp(&a.0));
        // One of each climate first, the best of each, far apart; then the next best.
        let mut out: Vec<Place> = Vec::new();
        let mut tried_classes: Vec<ClimateClass> = Vec::new();
        let far = |out: &[Place], x: f64, z: f64| {
            out.iter()
                .all(|p| self.distance_km((p.x as f64, p.z as f64), (x, z)) >= APART_KM)
        };
        for pass in 0..2 {
            for &(_, class, x, z) in &cands {
                if out.len() >= 5 {
                    break;
                }
                if pass == 0 && tried_classes.contains(&class) {
                    continue;
                }
                if !far(&out, x, z) {
                    continue;
                }
                let (sx, sz) = self.wg.terrain.spawn_near(x as i32, z as i32);
                if let Some(p) = self.verify(sx, sz, when) {
                    if pass == 0 {
                        tried_classes.push(class);
                    }
                    out.push(p);
                }
            }
        }
        out
    }

    /// Great-circle distance between two world positions (km), across the seam.
    fn distance_km(&self, a: (f64, f64), b: (f64, f64)) -> f64 {
        let p = self.wg.planet();
        let (la, lb) = (p.latitude(a.1), p.latitude(b.1));
        let c = p.circumference() as f64;
        let (oa, ob) = (
            a.0 / c * std::f64::consts::TAU,
            b.0 / c * std::f64::consts::TAU,
        );
        let cos = la.sin() * lb.sin() + la.cos() * lb.cos() * (oa - ob).cos();
        cos.clamp(-1.0, 1.0).acos() * hearth_math::planet::EARTH_RADIUS_M / 1000.0
    }

    /// A place's card, every claim found about it; none where it has no fresh water near, or a
    /// season too cold or hot to begin in.
    pub fn verify(&self, x: i32, z: i32, when: When) -> Option<Place> {
        let wg = &*self.wg;
        let terrain = &wg.terrain;
        let planet = wg.planet();
        let per_m = terrain.vertical_scale() as f64;
        let lat = planet.latitude_deg(z as f64);
        let (yf, season) = when.at(lat);
        let normals = Normals::sample(&terrain.grid, x as f64, z as f64);
        let (low, high) = day_range(&normals, yf);
        let here = terrain.sample(x, z);
        if here.is_underwater() {
            return None;
        }
        let height_m = here.height as f64 / per_m;
        // Rings about the spot: the water, the land's shape.
        let mut water: Option<Claim> = None;
        let mut sea: Option<Claim> = None;
        let (mut lowest, mut highest) = (f32::MAX, f32::MIN);
        let mut cliffs = 0.0f32;
        let keep = |c: &mut Option<Claim>, found: Found, at: Option<(f64, f64)>| {
            let d = |c: &Claim| c.at.map_or(0.0, |(dx, dz)| dx.hypot(dz));
            let new = Claim { found, at };
            if c.as_ref().is_none_or(|o| d(&new) < d(o)) {
                *c = Some(new);
            }
        };
        for r in [0.0, 60.0, 150.0, 300.0, 500.0, 800.0, 1100.0, REACH_M] {
            let dirs = if r == 0.0 { 1 } else { 16 };
            for k in 0..dirs {
                let a = k as f64 / dirs as f64 * std::f64::consts::TAU;
                let (dx, dz) = (r * a.sin(), -r * a.cos());
                let (sx, sz) = (planet.wrap_x(x + dx as i32), z + dz as i32);
                let s = terrain.sample(sx, sz);
                let at = (r > 0.0).then_some((dx, dz));
                if s.is_underwater() {
                    if s.ocean {
                        keep(&mut sea, Found::Sea, at);
                    } else if s.lake {
                        keep(&mut water, Found::Lake, at);
                    }
                }
                if let Some(rv) = &s.river
                    && rv.distance <= rv.width * 0.5 + 2.0
                {
                    let width_m = rv.width / per_m as f32;
                    keep(&mut water, Found::River { width_m }, at);
                }
                if !s.is_underwater() {
                    lowest = lowest.min(s.height);
                    highest = highest.max(s.height);
                }
                cliffs = cliffs.max(s.cliffiness);
            }
        }
        // Springs of fresh water within some 750 m.
        let cell = hearth_worldgen::hydro::CELL;
        let span = 3;
        for cz in z.div_euclid(cell) - span..=z.div_euclid(cell) + span {
            for cx in x.div_euclid(cell) - span..=x.div_euclid(cell) + span {
                for sp in wg.hydro.cell(wg, cx, cz).iter() {
                    if sp.kind != hearth_worldgen::hydro::SpringKind::Fresh {
                        continue;
                    }
                    let dx = wrap_dx(planet, sp.x - x);
                    let dz = (sp.z - z) as f64;
                    if dx.hypot(dz) <= REACH_M {
                        keep(&mut water, Found::Spring, Some((dx, dz)));
                    }
                }
            }
        }
        // Fresh water is the first thing a place must have.
        let water = water?;
        if water.at.is_some_and(|(dx, dz)| dx.hypot(dz) > REACH_M) {
            return None;
        }
        // What grows and lies about: the ground of the spot and of points on rings out to some
        // 600 m, block by block, and the trees standing there.
        let mut points = vec![(0.0, 0.0)];
        for r in [80.0, 250.0, 600.0] {
            for k in 0..8 {
                let a = k as f64 / 8.0 * std::f64::consts::TAU;
                points.push((r * a.sin(), -r * a.cos()));
            }
        }
        let mut stones: Vec<(String, (f64, f64))> = Vec::new();
        let mut plants: Vec<(String, (f64, f64))> = Vec::new();
        let mut trees: Vec<(usize, (f64, f64))> = Vec::new();
        let under: Vec<(hearth_world::BlockStateId, &str)> = wg
            .forest
            .understory
            .iter()
            .flat_map(|u| {
                std::iter::once((u.lower, u.id.as_str())).chain(u.upper.map(|b| (b, u.id.as_str())))
            })
            .collect();
        for &(dx, dz) in &points {
            let (px, pz) = (planet.wrap_x(x + dx as i32), z + dz as i32);
            let s = terrain.sample(px, pz);
            if s.is_underwater() {
                continue;
            }
            let y = s.height_i();
            for cy in [y - 1, y + 15] {
                let pos = BlockPos::new(px, cy, pz).cube();
                let (cube, _) = wg.generate_cube_in(pos, &self.veg);
                for i in 0..hearth_math::CUBE_VOLUME {
                    let b = cube.get_index(i);
                    if b == hearth_world::BlockStateId::AIR {
                        continue;
                    }
                    if wg.blocks.is_loose_stone(b) {
                        let name = self.reg.block_of(b).name.to_string();
                        let rock = name
                            .rsplit(':')
                            .next()
                            .unwrap_or(&name)
                            .trim_end_matches("_cobbles")
                            .to_owned();
                        if self.knappable(&rock) && !stones.iter().any(|s| s.0 == rock) {
                            stones.push((rock, (dx, dz)));
                        }
                    } else if let Some((_, id)) = under.iter().find(|(s, _)| *s == b)
                        && !plants.iter().any(|p| p.0 == *id)
                    {
                        plants.push(((*id).to_owned(), (dx, dz)));
                    }
                }
            }
            let half = 20;
            for t in wg.features().trees_in(
                wg,
                &self.veg,
                (px - half, pz - half),
                (px + half, pz + half),
            ) {
                if t.remains == hearth_worldgen::cubegen::features::Remains::Living {
                    trees.push((t.species, (dx, dz)));
                }
            }
        }
        let mut look_for = vec![water.clone()];
        if let Some((rock, at)) = stones.first() {
            look_for.push(Claim {
                found: Found::Toolstone(self.material_name(rock)),
                at: some_at(*at),
            });
        }
        // Trees by how many stand about, with where the most common grows nearest.
        let species = &wg.forest.templates.species;
        let mut counts: Vec<(usize, usize, (f64, f64))> = Vec::new();
        for &(sp, at) in &trees {
            match counts.iter_mut().find(|c| c.0 == sp) {
                Some(c) => {
                    c.1 += 1;
                    if at.0.hypot(at.1) < c.2.0.hypot(c.2.1) {
                        c.2 = at;
                    }
                }
                None => counts.push((sp, 1, at)),
            }
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        if let Some(first) = counts.first() {
            let names: Vec<String> = counts
                .iter()
                .take(3)
                .map(|c| self.plant_name(&species[c.0].id))
                .collect();
            look_for.push(Claim {
                found: Found::Trees(names),
                at: some_at(first.2),
            });
        }
        // Of everything growing about, what gives fibre, food in this season, and tinder.
        let mut growing: Vec<(String, (f64, f64))> = counts
            .iter()
            .map(|c| (species[c.0].id.clone(), c.2))
            .collect();
        growing.extend(plants.iter().cloned());
        let uses = |id: &str, word: &str| {
            self.content
                .plants
                .get(id)
                .is_some_and(|p| p.parts.iter().any(|q| q.uses.iter().any(|u| u == word)))
        };
        let pick = |f: &dyn Fn(&str) -> bool| -> Option<(Vec<String>, (f64, f64))> {
            let found: Vec<&(String, (f64, f64))> = growing.iter().filter(|g| f(&g.0)).collect();
            let first = found.first()?;
            Some((
                found
                    .iter()
                    .take(3)
                    .map(|g| self.plant_name(&g.0))
                    .collect(),
                first.1,
            ))
        };
        if let Some((names, at)) = pick(&|id| uses(id, "cordage")) {
            look_for.push(Claim {
                found: Found::Fibre(names),
                at: some_at(at),
            });
        }
        let food: Vec<(String, PartKind, (f64, f64))> = growing
            .iter()
            .filter_map(|(id, at)| {
                let p = self.content.plants.get(id)?;
                let part = p.parts.iter().find(|q| {
                    q.edibility == Edibility::Edible
                        && q.seasons.contains(&season)
                        && q.uses.iter().any(|u| u == "food")
                })?;
                Some((self.plant_name(id), part.part, *at))
            })
            .collect();
        if let Some(first) = food.first() {
            look_for.push(Claim {
                found: Found::Food(food.iter().take(3).map(|f| (f.0.clone(), f.1)).collect()),
                at: some_at(first.2),
            });
        }
        if let Some((names, at)) = pick(&|id| uses(id, "tinder")) {
            look_for.push(Claim {
                found: Found::Tinder(names),
                at: some_at(at),
            });
        }
        // The animals of the place that a person must mind.
        let land = GenLand {
            wg,
            veg: &self.veg,
            catalog: &self.catalog,
            trees: &self.yields,
        };
        let cell_m = hearth_fauna::habitat::CELL_M;
        let mut h = land.habitat((
            (x as f64 / cell_m).floor() as i64,
            (z as f64 / cell_m).floor() as i64,
        ));
        h.fauna = hearth_fauna::ecology::fauna_realm(&self.catalog, &h) as u8;
        let mut watch_out = Vec::new();
        let (mut hunters, mut waterside, mut venomous, mut defensive) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for sp in &self.catalog.species {
            if !hearth_fauna::ecology::lives_here(sp, &h) {
                continue;
            }
            let d = &sp.danger;
            if d.predatory && sp.mass_kg >= 20.0 {
                if sp.aquatic || sp.waterside {
                    waterside.push(sp.name.clone());
                } else {
                    hunters.push(sp.name.clone());
                }
            } else if d.venomous && d.aggression > 0.0 && sp.mass_kg >= 0.05 {
                venomous.push(sp.name.clone());
            } else if d.defensive && d.aggression >= 0.3 && sp.mass_kg >= 100.0 {
                defensive.push(sp.name.clone());
            }
        }
        let dangers = hunters.len() + waterside.len();
        for (list, make) in [
            (hunters, Danger::Predators as fn(Vec<String>) -> Danger),
            (waterside, Danger::AtTheWater),
            (venomous, Danger::Venomous),
            (defensive, Danger::Defensive),
        ] {
            if !list.is_empty() {
                watch_out.push(make(list.into_iter().take(4).collect()));
            }
        }
        if low < 8.0 {
            watch_out.push(Danger::ColdNights(low));
        }
        if high > 33.0 {
            watch_out.push(Danger::Heat(high));
        }
        if height_m > 2500.0 {
            watch_out.push(Danger::Height(height_m));
        }
        if matches!(
            water.found,
            Found::Lake | Found::River { .. } | Found::Spring
        ) && water.at.is_some_and(|(dx, dz)| dx.hypot(dz) > 800.0)
            && sea.is_some()
        {
            watch_out.push(Danger::SaltWater);
        }
        // How hard: the cold or heat, the hunters, how far the water, what there is to eat.
        let water_m = water.at.map_or(0.0, |(dx, dz)| dx.hypot(dz));
        let fed = look_for.iter().any(|c| matches!(c.found, Found::Food(_)));
        let mut hard = 0;
        hard += if low < 5.0 {
            2
        } else if low < 9.0 {
            1
        } else {
            0
        };
        hard += if high > 36.0 {
            2
        } else if high > 32.0 {
            1
        } else {
            0
        };
        hard += dangers.min(3);
        hard += if water_m > 800.0 { 1 } else { 0 };
        hard += if fed { 0 } else { 1 };
        hard += if height_m > 2500.0 { 1 } else { 0 };
        let difficulty = match hard {
            0..=1 => Difficulty::Gentle,
            2..=3 => Difficulty::Challenging,
            _ => Difficulty::Harsh,
        };
        let relief_m = (highest - lowest).max(0.0) as f64 / per_m;
        let name = self.name(&here, x, z, &water, sea.as_ref(), relief_m, cliffs);
        let climate = Words::new(
            "places.climate",
            &[
                ("climate", here.climate.name().to_owned()),
                ("season", season_key(season).to_owned()),
                ("high", format!("{high:.0}")),
                ("low", format!("{low:.0}")),
            ],
        );
        let terrain_words = Words::new(
            "places.terrain",
            &[
                ("biome", here.biome.name().replace('_', " ")),
                ("height", format!("{:.0}", height_m.max(0.0))),
            ],
        );
        Some(Place {
            x,
            z,
            name,
            climate,
            terrain: terrain_words,
            difficulty,
            look_for,
            watch_out,
            low_c: low,
            high_c: high,
        })
    }

    /// A place's name in plain words: its land (with its rock where the rock shows) and its
    /// climate.
    #[allow(clippy::too_many_arguments)]
    fn name(
        &self,
        here: &ColumnSample,
        x: i32,
        z: i32,
        water: &Claim,
        sea: Option<&Claim>,
        relief_m: f64,
        cliffs: f32,
    ) -> Words {
        use hearth_worldgen::region::biome::Biome;
        let near = |c: &Claim, m: f64| c.at.is_none_or(|(dx, dz)| dx.hypot(dz) <= m);
        let land = if sea.is_some_and(|s| near(s, 800.0)) {
            "coast"
        } else if here.biome == Biome::Wetland {
            "marsh"
        } else if relief_m > 600.0 {
            "mountainside"
        } else if matches!(water.found, Found::River { .. }) && near(water, 600.0) {
            if relief_m > 80.0 {
                "valley"
            } else {
                "riverside"
            }
        } else if matches!(water.found, Found::Lake) && near(water, 600.0) {
            "lakeshore"
        } else if relief_m > 150.0 {
            "hills"
        } else {
            "plain"
        };
        let cover = if here.tree_density > 0.45 {
            "wooded"
        } else if here.biome == Biome::Savanna {
            "savanna"
        } else if here.tree_density < 0.1 {
            "open"
        } else {
            ""
        };
        // The rock, where it shows: in cliffs and on hills and coasts.
        let rock = if cliffs > 0.3 || matches!(land, "coast" | "hills" | "valley" | "mountainside")
        {
            let col = self.wg.geology.column(x, z);
            let top = col.rock_at(here.height_i() - 3);
            let name = self.reg.block_of(top).name.to_string();
            let id = name.rsplit(':').next().unwrap_or(&name).to_owned();
            self.content
                .rocks
                .get(&format!("hearth:{id}"))
                .map(|r| r.name.clone())
                .filter(|n| {
                    [
                        "chalk",
                        "limestone",
                        "granite",
                        "sandstone",
                        "basalt",
                        "red sandstone",
                    ]
                    .contains(&n.as_str())
                })
        } else {
            None
        };
        let mut words = Vec::new();
        if !cover.is_empty() {
            words.push(cover.to_owned());
        }
        if let Some(r) = rock {
            words.push(r);
        }
        words.push(land.to_owned());
        let what = words.join(" ");
        let article = if what.starts_with(['a', 'e', 'i', 'o', 'u']) {
            "places.an"
        } else {
            "places.a"
        };
        Words::new(
            "places.name",
            &[
                ("a", article.to_owned()),
                ("what", what),
                ("climate", climate_words(here.climate).to_owned()),
            ],
        )
    }

    fn knappable(&self, rock: &str) -> bool {
        self.content
            .materials
            .get(&format!("hearth:{rock}"))
            .is_some_and(|m| m.tags.iter().any(|t| t == "knappable"))
    }

    fn material_name(&self, rock: &str) -> String {
        self.content
            .materials
            .get(&format!("hearth:{rock}"))
            .map_or_else(|| rock.replace('_', " "), |m| m.name.clone())
    }

    fn plant_name(&self, id: &str) -> String {
        self.content.plants.get(id).map_or_else(
            || id.rsplit(':').next().unwrap_or(id).replace('_', " "),
            |p| p.name.clone(),
        )
    }
}

/// The coldest of the night (about dawn) and the warmest of the afternoon on a day of the year
/// (°C).
fn day_range(n: &Normals, yf: f64) -> (f64, f64) {
    let mean = n.temperature(yf);
    let half = 0.5 * n.diurnal_range();
    (mean - half, mean + half)
}

/// An east–west offset (blocks) across the seam, the short way.
fn wrap_dx(planet: &hearth_math::Planet, dx: i32) -> f64 {
    let c = planet.circumference();
    let d = dx.rem_euclid(c);
    (if d > c / 2 { d - c } else { d }) as f64
}

fn some_at(at: (f64, f64)) -> Option<(f64, f64)> {
    (at.0.hypot(at.1) > 30.0).then_some(at)
}

fn season_key(s: Season) -> &'static str {
    match s {
        Season::Spring => "spring",
        Season::Summer => "summer",
        Season::Autumn => "autumn",
        Season::Winter => "winter",
    }
}

/// A climate in a word or two.
fn climate_words(c: ClimateClass) -> &'static str {
    match c {
        ClimateClass::TropicalRainforest => "hot, wet",
        ClimateClass::TropicalSavanna => "hot, seasonally wet",
        ClimateClass::HotDesert | ClimateClass::HotSteppe => "hot, dry",
        ClimateClass::ColdDesert | ClimateClass::ColdSteppe => "dry",
        ClimateClass::Mediterranean => "warm, dry-summer",
        ClimateClass::HumidSubtropical => "warm, humid",
        ClimateClass::Oceanic => "mild, wet",
        ClimateClass::HumidContinental => "cold-winter",
        ClimateClass::Subarctic => "cold",
        ClimateClass::Tundra | ClimateClass::IceCap => "polar",
        ClimateClass::Ocean => "maritime",
    }
}

/// A distance and a way in words: "350 m to the north-east".
pub fn way(lang: &hearth_ui::Lang, at: Option<(f64, f64)>) -> String {
    let Some((dx, dz)) = at else {
        return lang.get("places.here").to_owned();
    };
    let d = dx.hypot(dz);
    let dist = if d < 1000.0 {
        format!("{:.0} m", (d / 50.0).round() * 50.0)
    } else {
        format!("{:.1} km", d / 1000.0)
    };
    // North is toward −z.
    let bearing = dx.atan2(-dz).to_degrees().rem_euclid(360.0);
    let dirs = ["n", "ne", "e", "se", "s", "sw", "w", "nw"];
    let dir = dirs[((bearing + 22.5) / 45.0) as usize % 8];
    lang.format(
        "places.way",
        &[
            ("dist", &dist),
            ("dir", lang.get(&format!("places.dir.{dir}"))),
        ],
    )
}

/// A claim in words.
pub fn claim_words(lang: &hearth_ui::Lang, c: &Claim) -> String {
    let list = |v: &[String]| v.join(", ");
    let (key, what) = match &c.found {
        Found::River { width_m } if *width_m < 4.0 => ("places.find.stream", String::new()),
        Found::River { .. } => ("places.find.river", String::new()),
        Found::Lake => ("places.find.lake", String::new()),
        Found::Spring => ("places.find.spring", String::new()),
        Found::Sea => ("places.find.sea", String::new()),
        Found::Toolstone(r) => ("places.find.toolstone", r.clone()),
        Found::Trees(t) => ("places.find.trees", list(t)),
        Found::Fibre(t) => ("places.find.fibre", list(t)),
        Found::Food(f) => (
            "places.find.food",
            f.iter()
                .map(|(n, p)| {
                    lang.format(
                        "places.food_part",
                        &[("plant", n), ("part", lang.get(part_key(*p)))],
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
        ),
        Found::Tinder(t) => ("places.find.tinder", list(t)),
    };
    lang.format(key, &[("what", &what), ("where", &way(lang, c.at))])
}

/// A danger in words.
pub fn danger_words(lang: &hearth_ui::Lang, d: &Danger) -> String {
    let list = |v: &[String]| v.join(", ");
    match d {
        Danger::Predators(v) => lang.format("places.danger.predators", &[("what", &list(v))]),
        Danger::AtTheWater(v) => lang.format("places.danger.water", &[("what", &list(v))]),
        Danger::Venomous(v) => lang.format("places.danger.venomous", &[("what", &list(v))]),
        Danger::Defensive(v) => lang.format("places.danger.defensive", &[("what", &list(v))]),
        Danger::ColdNights(c) => lang.format("places.danger.cold", &[("c", &format!("{c:.0}"))]),
        Danger::Heat(c) => lang.format("places.danger.heat", &[("c", &format!("{c:.0}"))]),
        Danger::Height(m) => lang.format("places.danger.height", &[("m", &format!("{m:.0}"))]),
        Danger::SaltWater => lang.get("places.danger.salt").to_owned(),
    }
}

/// Words with their arguments filled.
pub fn words(lang: &hearth_ui::Lang, w: &Words) -> String {
    let args: Vec<(&str, String)> = w
        .args
        .iter()
        .map(|(k, v)| {
            // Arguments that are themselves words (a season, an article) are looked up.
            let key = format!("places.season.{v}");
            if *k == "season" && lang.has(&key) {
                (*k, lang.get(&key).to_owned())
            } else if *k == "a" {
                (*k, lang.get(v).to_owned())
            } else {
                (*k, v.clone())
            }
        })
        .collect();
    let refs: Vec<(&str, &str)> = args.iter().map(|(k, v)| (*k, v.as_str())).collect();
    lang.format(w.key, &refs).trim().replace("  ", " ")
}

fn part_key(p: PartKind) -> &'static str {
    match p {
        PartKind::Fruit => "places.part.fruit",
        PartKind::Nut => "places.part.nut",
        PartKind::Seed => "places.part.seed",
        PartKind::Root => "places.part.root",
        PartKind::Tuber => "places.part.tuber",
        PartKind::Bulb => "places.part.bulb",
        PartKind::Leaf => "places.part.leaf",
        PartKind::Shoot => "places.part.shoot",
        PartKind::Flower => "places.part.flower",
        PartKind::Bark => "places.part.bark",
        PartKind::InnerBark => "places.part.inner_bark",
        PartKind::Sap => "places.part.sap",
        PartKind::Resin => "places.part.resin",
        PartKind::Fibre => "places.part.fibre",
        PartKind::Wood => "places.part.wood",
        PartKind::Cap => "places.part.cap",
        PartKind::Whole => "places.part.whole",
    }
}

//! Biogeographic realms (v2 §7.4): when the world is made, its landmasses are grouped into
//! realms by their isolation and climate, as Earth's are, and each realm draws its plants and
//! animals from the matching real assemblage, so that the continents feel different.
//!
//! Each landmass of at least 4 % of the land is a continent; the rest are islands. The
//! continents are ranked by size: the largest is the Old World, its land north of 23.5° the
//! Palearctic and its tropics the Afrotropical; the next is the New World, the Nearctic and the
//! Neotropical; then in turn (the Indomalayan tropics third). South of 23.5° a continent's land
//! is its tropics' realm, or the Australasian where it has none; south of 60° the Antarctic. An
//! island near a continent (within 1.5 % of the circumference) belongs to its realm; a remote
//! one is Oceanian. Islands are poorer (`island_at`): fewer and smaller animals.

use std::collections::VecDeque;

use crate::planet::{PlanetGrid, flags};

/// A biogeographic realm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Realm {
    Palearctic = 0,
    Nearctic = 1,
    Afrotropical = 2,
    Indomalayan = 3,
    Neotropical = 4,
    Australasian = 5,
    Oceanian = 6,
    Antarctic = 7,
}

impl Realm {
    pub const ALL: [Realm; 8] = [
        Realm::Palearctic,
        Realm::Nearctic,
        Realm::Afrotropical,
        Realm::Indomalayan,
        Realm::Neotropical,
        Realm::Australasian,
        Realm::Oceanian,
        Realm::Antarctic,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Realm::Palearctic => "Palearctic",
            Realm::Nearctic => "Nearctic",
            Realm::Afrotropical => "Afrotropical",
            Realm::Indomalayan => "Indomalayan",
            Realm::Neotropical => "Neotropical",
            Realm::Australasian => "Australasian",
            Realm::Oceanian => "Oceanian",
            Realm::Antarctic => "Antarctic",
        }
    }

    pub fn parse(s: &str) -> Option<Realm> {
        Realm::ALL.into_iter().find(|r| r.name() == s)
    }

    /// The realm's bit in a [`RealmSet`].
    #[inline]
    pub fn bit(self) -> u16 {
        1 << self as u8
    }

    fn from_u8(v: u8) -> Realm {
        Realm::ALL[(v as usize).min(7)]
    }
}

/// A set of realms as bits; empty means "everywhere" for a species with no realms listed.
pub type RealmSet = u16;

/// The realms named, as a set (unknown names are ignored).
pub fn set_of(names: &[String]) -> RealmSet {
    names
        .iter()
        .filter_map(|n| Realm::parse(n))
        .fold(0, |m, r| m | r.bit())
}

/// Whether a species of realms `set` is native where the realm is `here`.
#[inline]
pub fn native(set: RealmSet, here: Realm) -> bool {
    set == 0 || set & here.bit() != 0
}

/// The realm whose species stand in where a realm has none of its own for a climate (a
/// southern temperate forest, a tropical forest outside Africa): the Afrotropical in the
/// tropics, the Palearctic elsewhere. Never a mix of two realms' species.
pub fn stand_in(class: crate::planet::climate::ClimateClass) -> Realm {
    use crate::planet::climate::ClimateClass as C;
    match class {
        C::TropicalRainforest | C::TropicalSavanna => Realm::Afrotropical,
        _ => Realm::Palearctic,
    }
}

/// A connected landmass.
#[derive(Debug, Clone, PartialEq)]
pub struct Landmass {
    pub area_km2: f64,
    pub continent: bool,
}

/// The realms of a world, on its planet grid.
#[derive(Debug, Clone)]
pub struct Realms {
    n: usize,
    cell: f64,
    half: f64,
    /// Per grid cell: the realm (at sea, the nearest land's).
    realm: Vec<u8>,
    /// Per grid cell: the landmass (at sea, the nearest land's).
    mass: Vec<u32>,
    pub landmasses: Vec<Landmass>,
    /// The land area of each realm present, km².
    pub areas: Vec<(Realm, f64)>,
}

/// Landmasses of this share of the land or more are continents.
const CONTINENT_SHARE: f64 = 0.04;
/// Islands nearer a continent than this share of the circumference belong to its realm.
const NEAR_SHARE: f64 = 0.015;

impl Realms {
    pub fn new(grid: &PlanetGrid) -> Self {
        let g = &grid.geom;
        let n = g.n;
        let land: Vec<bool> = (0..n * n)
            .map(|i| grid.flags[i] & flags::OCEAN == 0)
            .collect();
        // Landmasses, 8-connected, wrapping east–west.
        let mut mass = vec![u32::MAX; n * n];
        let mut landmasses: Vec<Landmass> = Vec::new();
        let mut cells_of: Vec<Vec<usize>> = Vec::new();
        let mut queue = VecDeque::new();
        for start in 0..n * n {
            if !land[start] || mass[start] != u32::MAX {
                continue;
            }
            let id = landmasses.len() as u32;
            let mut area = 0.0;
            let mut cells = Vec::new();
            mass[start] = id;
            queue.push_back(start);
            while let Some(c) = queue.pop_front() {
                let (i, j) = g.ij(c);
                let side = g.phys_cell(j) / 1000.0;
                area += side * side;
                cells.push(c);
                for (di, dj) in crate::planet::grid::GridGeom::OFFSETS8 {
                    if let Some(nb) = g.neighbor(i, j, di, dj)
                        && land[nb]
                        && mass[nb] == u32::MAX
                    {
                        mass[nb] = id;
                        queue.push_back(nb);
                    }
                }
            }
            landmasses.push(Landmass {
                area_km2: area,
                continent: false,
            });
            cells_of.push(cells);
        }
        let total: f64 = landmasses.iter().map(|l| l.area_km2).sum();
        let largest = landmasses.iter().map(|l| l.area_km2).fold(0.0f64, f64::max);
        for l in &mut landmasses {
            l.continent = l.area_km2 >= CONTINENT_SHARE * total || l.area_km2 >= largest;
        }
        // Zones of the continents: (landmass, 0 north / 1 tropics / 2 south) and their areas.
        const TROPIC: f64 = 23.5;
        let zone_of = |j: usize| -> usize {
            let lat = g.lat[j].to_degrees();
            if lat >= TROPIC {
                0
            } else if lat > -TROPIC {
                1
            } else {
                2
            }
        };
        let mut zone_area = vec![[0.0f64; 3]; landmasses.len()];
        for (m, cells) in cells_of.iter().enumerate() {
            if !landmasses[m].continent {
                continue;
            }
            for &c in cells {
                let (_, j) = g.ij(c);
                let side = g.phys_cell(j) / 1000.0;
                zone_area[m][zone_of(j)] += side * side;
            }
        }
        // The continents by size: the largest is the Old World (the Palearctic north of the
        // tropics, the Afrotropical between them), the next the New World (the Nearctic and the
        // Neotropical), and so on in turn, the Indomalayan tropics third.
        let mut ranked: Vec<usize> = (0..landmasses.len())
            .filter(|&m| landmasses[m].continent)
            .collect();
        ranked.sort_by(|a, b| {
            landmasses[*b]
                .area_km2
                .total_cmp(&landmasses[*a].area_km2)
                .then(a.cmp(b))
        });
        let mut zone_realm = vec![[Realm::Oceanian; 3]; landmasses.len()];
        for (k, &m) in ranked.iter().enumerate() {
            let tropics = [Realm::Afrotropical, Realm::Neotropical, Realm::Indomalayan][k % 3];
            zone_realm[m] = [
                [Realm::Palearctic, Realm::Nearctic][k % 2],
                tropics,
                if zone_area[m][1] > 0.0 {
                    tropics
                } else {
                    Realm::Australasian
                },
            ];
        }
        // Land cells of the continents take their zone's realm.
        let mut realm = vec![u8::MAX; n * n];
        for (m, cells) in cells_of.iter().enumerate() {
            if !landmasses[m].continent {
                continue;
            }
            for &c in cells {
                let (_, j) = g.ij(c);
                realm[c] = if g.lat[j].to_degrees() < -60.0 {
                    Realm::Antarctic
                } else {
                    zone_realm[m][zone_of(j)]
                } as u8;
            }
        }
        // Everything else takes the nearest continent's realm, by a breadth-first spread from
        // the continents; islands farther than NEAR_SHARE are Oceanian.
        let step_km = g.cell / 1000.0;
        let near_steps = (NEAR_SHARE * g.c / 1000.0 / step_km).ceil().max(1.0) as u32;
        let mut dist = vec![u32::MAX; n * n];
        queue.clear();
        for c in 0..n * n {
            if realm[c] != u8::MAX {
                dist[c] = 0;
                queue.push_back(c);
            }
        }
        let mut nearest = realm.clone();
        while let Some(c) = queue.pop_front() {
            let (i, j) = g.ij(c);
            for (di, dj) in crate::planet::grid::GridGeom::OFFSETS8 {
                if let Some(nb) = g.neighbor(i, j, di, dj)
                    && dist[nb] == u32::MAX
                {
                    dist[nb] = dist[c] + 1;
                    nearest[nb] = nearest[c];
                    queue.push_back(nb);
                }
            }
        }
        for (m, cells) in cells_of.iter().enumerate() {
            if landmasses[m].continent {
                continue;
            }
            // An island is as near as its nearest cell.
            let near = cells.iter().map(|&c| dist[c]).min().unwrap_or(u32::MAX);
            let r = if near <= near_steps {
                cells
                    .iter()
                    .min_by_key(|&&c| dist[c])
                    .map_or(Realm::Oceanian as u8, |&c| nearest[c])
            } else {
                Realm::Oceanian as u8
            };
            for &c in cells {
                realm[c] = r;
            }
        }
        let mut areas: Vec<(Realm, f64)> = Vec::new();
        for cells in &cells_of {
            for &c in cells {
                let (_, j) = g.ij(c);
                let side = g.phys_cell(j) / 1000.0;
                let r = Realm::from_u8(realm[c]);
                match areas.iter_mut().find(|(x, _)| *x == r) {
                    Some(e) => e.1 += side * side,
                    None => areas.push((r, side * side)),
                }
            }
        }
        areas.sort_by(|a, b| b.1.total_cmp(&a.1));
        // The sea takes the nearest land's realm and landmass (coasts finer than the grid).
        queue.clear();
        let mut seen = vec![false; n * n];
        for c in 0..n * n {
            if land[c] {
                seen[c] = true;
                queue.push_back(c);
            }
        }
        while let Some(c) = queue.pop_front() {
            let (i, j) = g.ij(c);
            for (di, dj) in crate::planet::grid::GridGeom::OFFSETS8 {
                if let Some(nb) = g.neighbor(i, j, di, dj)
                    && !seen[nb]
                {
                    seen[nb] = true;
                    realm[nb] = realm[c];
                    mass[nb] = mass[c];
                    queue.push_back(nb);
                }
            }
        }
        for r in &mut realm {
            if *r == u8::MAX {
                *r = Realm::Oceanian as u8;
            }
        }
        Self {
            n,
            cell: g.cell,
            half: g.c * 0.5,
            realm,
            mass,
            landmasses,
            areas,
        }
    }

    /// One realm for the whole world (tests, worlds without a planet grid).
    pub fn uniform(r: Realm) -> Self {
        Self {
            n: 1,
            cell: f64::MAX,
            half: 0.0,
            realm: vec![r as u8],
            mass: vec![0],
            landmasses: vec![Landmass {
                area_km2: f64::INFINITY,
                continent: true,
            }],
            areas: vec![(r, f64::INFINITY)],
        }
    }

    fn index(&self, x: f64, z: f64) -> usize {
        if self.n == 1 {
            return 0;
        }
        let n = self.n as i64;
        let i = ((x / self.cell).floor() as i64).rem_euclid(n);
        let j = (((z + self.half) / self.cell).floor() as i64).clamp(0, n - 1);
        (j * n + i) as usize
    }

    /// The realm at a world position.
    pub fn realm_at(&self, x: f64, z: f64) -> Realm {
        Realm::from_u8(self.realm[self.index(x, z)])
    }

    /// The landmass at (or nearest) a world position.
    pub fn landmass_at(&self, x: f64, z: f64) -> Option<&Landmass> {
        self.landmasses.get(self.mass[self.index(x, z)] as usize)
    }

    /// Whether a world position is on an island (poorer in species than a continent).
    pub fn island_at(&self, x: f64, z: f64) -> bool {
        self.landmass_at(x, z).is_some_and(|l| !l.continent)
    }
}

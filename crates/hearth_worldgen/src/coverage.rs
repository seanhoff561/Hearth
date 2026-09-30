//! Resource coverage (v2 V2-2 acceptance): how far each continent's land is from every
//! resource of the early eras.
//!
//! A census of every deposit body on the planet is laid on the grid of placement cells; the
//! bodies that can be worked in the resource's era (not too deep) are its sources. Each
//! resource gets a distance field (Dijkstra over the cells, X wrapping) and each continent — a
//! connected landmass of some size — the distance within which 90% of its land lies from the
//! nearest body. A resource farther than its era's reach is a gap. A gap is covered when a
//! substitute is within reach: a resource yielding the same metal, a mineral of the same
//! formula (pyrite and marcasite), or one sharing a use tag (a tag ending in `_ore`, or one that
//! item forms and processes select materials by).

use std::cmp::Ordering;
use std::collections::{BTreeSet, BinaryHeap};

use hearth_content::Content;
use hearth_content::schema::process::Match;
use rayon::prelude::*;

use crate::cubegen::WorldGenerator;
use crate::deposits::{Body, CELL};

/// How far a resource of an era may lie from a continent's land and still count as reachable
/// (km): toolstone, pigments, clay and salt within a long day's walk, later needs within a
/// journey. Set at Standard size and scaled with the planet.
pub fn reach_limit_km(era: u8, circumference: i32) -> f64 {
    let km = match era {
        0..=2 => 5.0,
        3 => 8.0,
        _ => 12.0,
    };
    km * circumference as f64 / 65_536.0
}

/// How much ground may lie over a body for it to be worked in an era (blocks): surface finds
/// and shallow pits at first, Neolithic flint mines, then Bronze and Iron Age shafts.
pub fn access_depth(era: u8) -> i32 {
    match era {
        0..=2 => 4,
        3 => 12,
        _ => 40,
    }
}

/// A landmass.
#[derive(Debug, Clone)]
pub struct Continent {
    pub area_km2: f64,
    /// The land block farthest from the sea (x, z).
    pub center: (i32, i32),
}

/// How one resource can be reached from one continent.
#[derive(Debug, Clone)]
pub struct Reach {
    pub resource: String,
    pub era: u8,
    /// Workable bodies (see [`access_depth`]) centred on the continent.
    pub bodies: usize,
    /// 90% of the continent's land lies within this distance of a body (km; infinite when
    /// the planet has none).
    pub p90_km: f64,
}

/// A resource out of reach of a continent.
#[derive(Debug, Clone)]
pub struct Gap {
    pub continent: usize,
    pub resource: String,
    pub era: u8,
    pub p90_km: f64,
    pub limit_km: f64,
    /// A resource of the same use that is within reach.
    pub substitute: Option<String>,
}

/// Coverage of one world.
#[derive(Debug, Clone, Default)]
pub struct Coverage {
    /// Largest first.
    pub continents: Vec<Continent>,
    /// Per continent, one entry per resource (the same order for every continent).
    pub reach: Vec<Vec<Reach>>,
    pub gaps: Vec<Gap>,
    /// Bodies per deposit model on the whole planet.
    pub bodies_per_model: Vec<usize>,
    /// Land area of the planet (km², counted by placement cell).
    pub land_km2: f64,
}

impl Coverage {
    /// Gaps no substitute covers.
    pub fn open_gaps(&self) -> impl Iterator<Item = &Gap> {
        self.gaps.iter().filter(|g| g.substitute.is_none())
    }
}

/// Coverage of the resources of eras up to `max_era` on the landmasses of at least
/// `min_continent_km2`, from a census of the whole planet.
pub fn coverage(
    wg: &WorldGenerator,
    content: &Content,
    max_era: u8,
    min_continent_km2: f64,
) -> Coverage {
    let bodies = wg.deposits.census(wg);
    coverage_of(wg, content, &bodies, max_era, min_continent_km2)
}

/// Coverage from an existing census.
pub fn coverage_of(
    wg: &WorldGenerator,
    content: &Content,
    bodies: &[Body],
    max_era: u8,
    min_continent_km2: f64,
) -> Coverage {
    let grid = &wg.terrain.grid;
    let circumference = grid.planet().circumference();
    let n = wg.deposits.cells_around() as usize;
    let half = (n / 2) as i32;
    let cell_km = CELL as f64 / 1000.0;
    let cell_km2 = cell_km * cell_km;
    // Land cells (by their centre) and landmasses.
    let land: Vec<bool> = (0..n * n)
        .into_par_iter()
        .map(|k| {
            let x = ((k % n) as f64 + 0.5) * CELL as f64;
            let z = ((k / n) as f64 - half as f64 + 0.5) * CELL as f64;
            grid.elevation.data[grid.cell_at(x, z)] > 0.0
        })
        .collect();
    let (label, sizes) = landmasses(n, &land);
    let mut order: Vec<usize> = (0..sizes.len())
        .filter(|&l| sizes[l] as f64 * cell_km2 >= min_continent_km2)
        .collect();
    order.sort_by(|&a, &b| sizes[b].cmp(&sizes[a]));
    let mut continent_of_label = vec![usize::MAX; sizes.len()];
    for (ci, &l) in order.iter().enumerate() {
        continent_of_label[l] = ci;
    }
    let continent_at = |k: usize| -> Option<usize> {
        let l = label[k];
        (l != u32::MAX && continent_of_label[l as usize] != usize::MAX)
            .then(|| continent_of_label[l as usize])
    };
    let mut cells: Vec<Vec<u32>> = vec![Vec::new(); order.len()];
    for k in 0..n * n {
        if let Some(ci) = continent_at(k) {
            cells[ci].push(k as u32);
        }
    }
    // Centres: the cell of each continent farthest from the sea.
    let sea: Vec<u32> = (0..n * n).filter(|&k| !land[k]).map(|k| k as u32).collect();
    let inland = distance_field(n, &sea);
    let continents: Vec<Continent> = cells
        .iter()
        .map(|cs| {
            let k = cs
                .iter()
                .copied()
                .max_by(|&a, &b| inland[a as usize].total_cmp(&inland[b as usize]))
                .unwrap_or(0) as usize;
            Continent {
                area_km2: cs.len() as f64 * cell_km2,
                center: (
                    ((k % n) as i32) * CELL + CELL / 2,
                    ((k / n) as i32 - half) * CELL + CELL / 2,
                ),
            }
        })
        .collect();
    // Resources of the eras asked for, and where their bodies are.
    let models = &wg.deposits.models;
    let mut resources: Vec<(String, u8)> = Vec::new();
    let mut model_resource = vec![usize::MAX; models.len()];
    for (mi, (d, m)) in content.deposits.iter().zip(models).enumerate() {
        if m.era > max_era {
            continue;
        }
        let r = d.resource.as_str();
        model_resource[mi] = match resources.iter().position(|(id, _)| id == r) {
            Some(ri) => {
                resources[ri].1 = resources[ri].1.min(m.era);
                ri
            }
            None => {
                resources.push((r.to_owned(), m.era));
                resources.len() - 1
            }
        };
    }
    let mut sources = vec![Vec::new(); resources.len()];
    let mut bodies_per_model = vec![0; models.len()];
    let mut on_continent = vec![vec![0usize; resources.len()]; continents.len()];
    for b in bodies {
        bodies_per_model[b.model as usize] += 1;
        let ri = model_resource[b.model as usize];
        if ri == usize::MAX || b.top_depth() > access_depth(resources[ri].1) {
            continue;
        }
        let i = b.center[0].div_euclid(CELL).rem_euclid(n as i32) as usize;
        let j = (b.center[2].div_euclid(CELL) + half).clamp(0, n as i32 - 1) as usize;
        let k = j * n + i;
        sources[ri].push(k as u32);
        if let Some(ci) = continent_at(k) {
            on_continent[ci][ri] += 1;
        }
    }
    // Per resource and continent: the distance 90% of the land lies within.
    let p90: Vec<Vec<f64>> = sources
        .par_iter()
        .map(|src| {
            let d = distance_field(n, src);
            cells
                .iter()
                .map(|cs| {
                    let mut v: Vec<f32> = cs.iter().map(|&k| d[k as usize]).collect();
                    let idx = ((v.len() as f64 * 0.9).ceil() as usize).clamp(1, v.len()) - 1;
                    let (_, x, _) = v.select_nth_unstable_by(idx, |a, b| a.total_cmp(b));
                    *x as f64 * cell_km
                })
                .collect()
        })
        .collect();
    let reach = (0..continents.len())
        .map(|ci| {
            resources
                .iter()
                .enumerate()
                .map(|(ri, (r, era))| Reach {
                    resource: r.clone(),
                    era: *era,
                    bodies: on_continent[ci][ri],
                    p90_km: p90[ri][ci],
                })
                .collect()
        })
        .collect();
    // Gaps, and the substitutes that cover them.
    let names: Vec<&str> = resources.iter().map(|(r, _)| r.as_str()).collect();
    let subs = substitutes(content, &names);
    let mut gaps = Vec::new();
    for (ci, _) in continents.iter().enumerate() {
        for (ri, (r, era)) in resources.iter().enumerate() {
            let limit = reach_limit_km(*era, circumference);
            if p90[ri][ci] <= limit {
                continue;
            }
            let substitute = subs[ri]
                .iter()
                .copied()
                .filter(|&sj| p90[sj][ci] <= limit)
                .min_by(|&a, &b| p90[a][ci].total_cmp(&p90[b][ci]))
                .map(|sj| resources[sj].0.clone());
            gaps.push(Gap {
                continent: ci,
                resource: r.clone(),
                era: *era,
                p90_km: p90[ri][ci],
                limit_km: limit,
                substitute,
            });
        }
    }
    Coverage {
        continents,
        reach,
        gaps,
        bodies_per_model,
        land_km2: land.iter().filter(|l| **l).count() as f64 * cell_km2,
    }
}

/// Connected land (4-neighbours, X wrapping): a label per cell (`u32::MAX` for sea) and the
/// size of each landmass.
fn landmasses(n: usize, land: &[bool]) -> (Vec<u32>, Vec<usize>) {
    let mut label = vec![u32::MAX; n * n];
    let mut sizes = Vec::new();
    let mut stack = Vec::new();
    for start in 0..n * n {
        if !land[start] || label[start] != u32::MAX {
            continue;
        }
        let id = sizes.len() as u32;
        label[start] = id;
        stack.push(start);
        let mut size = 0;
        while let Some(k) = stack.pop() {
            size += 1;
            let (i, j) = (k % n, k / n);
            let mut next = [
                Some(j * n + (i + 1) % n),
                Some(j * n + (i + n - 1) % n),
                None,
                None,
            ];
            if j + 1 < n {
                next[2] = Some(k + n);
            }
            if j > 0 {
                next[3] = Some(k - n);
            }
            for kk in next.into_iter().flatten() {
                if land[kk] && label[kk] == u32::MAX {
                    label[kk] = id;
                    stack.push(kk);
                }
            }
        }
        sizes.push(size);
    }
    (label, sizes)
}

#[derive(Clone, Copy, PartialEq)]
struct Node(f32, u32);

impl Eq for Node {}

impl Ord for Node {
    // Reversed: the heap pops the nearest node first.
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.total_cmp(&self.0).then(o.1.cmp(&self.1))
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// Distance (in cells) from every cell to the nearest source, over 8-neighbour steps with X
/// wrapping (at most 8% longer than the straight line).
fn distance_field(n: usize, sources: &[u32]) -> Vec<f32> {
    use std::f32::consts::SQRT_2;
    const STEPS: [(isize, isize, f32); 8] = [
        (1, 0, 1.0),
        (-1, 0, 1.0),
        (0, 1, 1.0),
        (0, -1, 1.0),
        (1, 1, SQRT_2),
        (1, -1, SQRT_2),
        (-1, 1, SQRT_2),
        (-1, -1, SQRT_2),
    ];
    let mut d = vec![f32::INFINITY; n * n];
    let mut heap = BinaryHeap::new();
    for &s in sources {
        if d[s as usize] > 0.0 {
            d[s as usize] = 0.0;
            heap.push(Node(0.0, s));
        }
    }
    while let Some(Node(dist, k)) = heap.pop() {
        if dist > d[k as usize] {
            continue;
        }
        let (i, j) = (k as usize % n, k as usize / n);
        for (di, dj, w) in STEPS {
            let jj = j as isize + dj;
            if jj < 0 || jj >= n as isize {
                continue;
            }
            let kk = jj as usize * n + (i as isize + di).rem_euclid(n as isize) as usize;
            let nd = dist + w;
            if nd < d[kk] {
                d[kk] = nd;
                heap.push(Node(nd, kk as u32));
            }
        }
    }
    d
}

/// The tags item forms and processes select materials by.
fn use_tags(c: &Content) -> BTreeSet<String> {
    let mut tags: BTreeSet<String> = c
        .forms
        .iter()
        .flat_map(|f| f.materials.tags.iter().cloned())
        .collect();
    for p in c.processes.iter() {
        for input in &p.inputs {
            match &input.item {
                Match::Form {
                    materials: Some(f), ..
                } => tags.extend(f.tags.iter().cloned()),
                Match::Tag(t) => {
                    tags.insert(t.clone());
                }
                _ => {}
            }
        }
    }
    tags
}

/// For each resource, the resources that can stand in for it: those that yield its main
/// metal, minerals of the same formula, or those whose material shares a use tag with its
/// material.
fn substitutes(c: &Content, resources: &[&str]) -> Vec<Vec<usize>> {
    let used = use_tags(c);
    let formula: Vec<Option<&str>> = resources
        .iter()
        .map(|r| c.minerals.get(r).and_then(|m| m.formula.as_deref()))
        .collect();
    let info: Vec<(Option<String>, Vec<String>, Vec<String>)> = resources
        .iter()
        .map(|r| {
            let yields: Vec<String> = c
                .minerals
                .get(r)
                .map(|m| {
                    m.yields
                        .iter()
                        .map(|y| y.material.as_str().to_owned())
                        .collect()
                })
                .unwrap_or_default();
            let tags = hearth_content::generate::material_of(c, r)
                .and_then(|m| c.materials.get(m.as_str()))
                .map(|m| {
                    m.tags
                        .iter()
                        .filter(|t| t.ends_with("_ore") || used.contains(*t))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            (yields.first().cloned(), yields, tags)
        })
        .collect();
    (0..resources.len())
        .map(|i| {
            let (main, _, tags) = &info[i];
            (0..resources.len())
                .filter(|&j| {
                    j != i
                        && (main.as_ref().is_some_and(|m| info[j].1.contains(m))
                            || formula[i].is_some() && formula[i] == formula[j]
                            || tags.iter().any(|t| info[j].2.contains(t)))
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_field_wraps_and_measures() {
        let n = 16;
        let d = distance_field(n, &[(5 * n) as u32]);
        assert_eq!(d[5 * n], 0.0);
        // One step west of column 0 is column 15 (wrap).
        assert_eq!(d[5 * n + 15], 1.0);
        assert!((d[7 * n + 2] - 2.0 * std::f32::consts::SQRT_2).abs() < 1e-5);
        assert!(distance_field(n, &[]).iter().all(|v| v.is_infinite()));
    }

    #[test]
    fn landmasses_join_across_the_seam() {
        let n = 8;
        let mut land = vec![false; n * n];
        for k in [2 * n, 2 * n + 7, 3 * n + 7, 6 * n + 3] {
            land[k] = true;
        }
        let (label, sizes) = landmasses(n, &land);
        assert_eq!(sizes.len(), 2);
        assert_eq!(label[2 * n], label[3 * n + 7]);
        assert_ne!(label[2 * n], label[6 * n + 3]);
        assert_eq!(label[0], u32::MAX);
    }

    #[test]
    fn substitutes_follow_metals_and_use_tags() {
        let c = Content::load_base();
        let names = [
            "hearth:malachite",
            "hearth:native_copper",
            "hearth:galena",
            "hearth:native_silver",
            "hearth:flint",
            "hearth:chert",
            "hearth:red_ochre",
            "hearth:laterite",
            "hearth:bog_iron",
            "hearth:pyrite",
            "hearth:marcasite",
        ];
        let subs = substitutes(&c, &names);
        let has = |i: usize, j: usize| subs[i].contains(&j);
        assert!(
            has(0, 1) && has(1, 0),
            "copper minerals stand in for each other"
        );
        assert!(has(3, 2), "silver comes from galena");
        assert!(!has(2, 3), "native silver is no source of lead");
        assert!(has(4, 5) && has(5, 4), "knappable stones");
        assert!(has(7, 8) && has(8, 7), "iron ores");
        assert!(subs[6].is_empty(), "nothing replaces red ochre");
        assert!(has(9, 10) && has(10, 9), "polymorphs of FeS₂");
    }
}

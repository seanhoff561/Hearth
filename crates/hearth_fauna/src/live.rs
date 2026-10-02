//! Animals in the world (v2 §7.4): near the player a population's groups become animals that
//! stand and move on the ground — a herd of red deer with its calves, a sow and her piglets —
//! and the small species of the cells about the player are drawn from their numbers; as the
//! player leaves they fold back into their groups and cells, the dead staying dead.
//!
//! What they do here is the plainest life: grazing and wandering about the group, resting by
//! the hours of their kind, fleeing a person who comes within their flight distance. Their
//! minds (senses, needs, the utility AI, herds and packs) replace it in V2-7 (f).

use glam::{DVec2, DVec3};
use hearth_content::schema::fauna::Activity;
use hearth_math::hash::{Rng, derive_seed, hash2};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::ecology::{Ecology, REGION_LEN, dist};
use crate::habitat::CELL_M;
use crate::species::Species;

/// Where an animal can stand.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Footing {
    /// The height of the ground's surface (m).
    pub y: f64,
    /// Water stands over it.
    pub water: bool,
}

/// The ground animals walk on: the loaded terrain.
pub trait Ground {
    /// The ground near height `y` at (x, z), searching a few metres up and down; None where the
    /// terrain is not loaded or there is no footing.
    fn footing(&self, x: f64, z: f64, y: f64) -> Option<Footing>;
    /// The highest ground at (x, z) (under the trees' foliage); None where not loaded.
    fn top(&self, x: f64, z: f64) -> Option<Footing>;
}

/// An animal's age.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stage {
    /// Born this year.
    Young,
    /// Older, not yet grown.
    Juvenile,
    Adult,
}

/// What an animal is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Act {
    /// Feeding where it stands, head down.
    Graze,
    /// Walking to somewhere near.
    Walk,
    /// Lying down.
    Rest,
    /// Head up, looking (something is near).
    Alert,
    /// Running away.
    Flee,
    /// Lying asleep.
    Sleep,
    /// Licking or nibbling its coat.
    Groom,
    /// Head down at water.
    Drink,
    /// Up on the hind legs.
    Rear,
    /// Lunging, biting, butting or striking.
    Attack,
    /// On the wing.
    Fly,
}

/// An animal in the world.
#[derive(Debug, Clone, PartialEq)]
pub struct Animal {
    pub id: u64,
    pub species: u16,
    /// The group it belongs to (large species).
    pub group: Option<u64>,
    /// The cell it was drawn from (small species): region key and local cell.
    pub cell: Option<((i64, i64), usize)>,
    pub stage: Stage,
    pub female: bool,
    /// The middle of its feet.
    pub pos: DVec3,
    /// Facing, radians (0 toward +z, turning toward +x).
    pub yaw: f32,
    /// Ground speed now, m/s.
    pub speed: f32,
    pub act: Act,
    /// Seconds left in the act.
    pub timer: f32,
    /// Where it is walking or running to.
    pub goal: Option<DVec2>,
    /// The gait's phase: strides taken.
    pub stride: f32,
    pub dead: bool,
}

/// An animal as the client draws it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AnimalView {
    pub id: u64,
    pub species: u16,
    pub stage: Stage,
    pub female: bool,
    pub pos: DVec3,
    pub yaw: f32,
    pub speed: f32,
    pub act: Act,
    pub stride: f32,
}

/// The animals near the player.
#[derive(Debug, Clone)]
pub struct Live {
    pub animals: Vec<Animal>,
    next_id: u64,
    seed: u64,
    rng: Rng,
    /// Animals of small species taken out of each cell (region, slot, cell): adults, young.
    drawn: FxHashMap<((i64, i64), usize, usize), (u32, u32)>,
}

/// Within this distance of the player groups become animals.
pub const NEAR_M: f64 = 112.0;
/// Beyond this they fold back.
pub const FAR_M: f64 = 150.0;
/// Small species are drawn from the cells within this distance.
pub const SMALL_NEAR_M: f64 = 80.0;
/// The most of one small species drawn from a cell.
const SMALL_PER_CELL: u32 = 3;

/// The smallest animal drawn into the world here (smaller ones wait for their own ways of
/// being seen: voles in the grass, songbirds in the trees).
const SMALL_MIN_KG: f32 = 0.2;

fn hdist(a: DVec3, b: DVec3) -> f64 {
    ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

/// Whether a small species walks the ground where the player can meet it (not flying birds,
/// fish, or the very small).
pub fn walks(sp: &Species) -> bool {
    use hearth_content::schema::fauna::BodyPlan as B;
    sp.mass_kg >= SMALL_MIN_KG
        && !sp.aquatic
        && !sp.colony
        && !matches!(
            sp.plan,
            B::BirdPerching | B::Raptor | B::Waterfowl | B::Seabird | B::Insect | B::Snake
        )
}

/// Whether an animal of its kind is up and about at the hour (0–1, local solar time).
pub fn awake(activity: Activity, hour: f32) -> bool {
    let h = hour * 24.0;
    match activity {
        Activity::Diurnal => (6.0..20.0).contains(&h),
        Activity::Nocturnal => !(7.0..19.0).contains(&h),
        Activity::Crepuscular => !(10.0..16.0).contains(&h) && !(1.0..4.0).contains(&h),
        Activity::Cathemeral => true,
    }
}

impl Live {
    pub fn new(seed: u64) -> Self {
        Self {
            animals: Vec::new(),
            next_id: 1,
            seed,
            rng: Rng::new(derive_seed(seed, "live animals")),
            drawn: FxHashMap::default(),
        }
    }

    fn id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Brings the groups and small animals near `player` into the world.
    pub fn materialize(&mut self, eco: &mut Ecology, ground: &dyn Ground, player: DVec3) {
        let cat = eco.catalog.clone();
        let wrap = eco.cells_around as f64 * CELL_M;
        let at = [player.x, player.z];
        let mut keys: Vec<(i64, i64)> = eco.regions.keys().copied().collect();
        keys.sort_unstable();
        for key in keys {
            let r = eco.regions.get_mut(&key).expect("region");
            // The groups.
            for g in r.groups.iter_mut() {
                if g.live || dist(g.pos, at, wrap) > NEAR_M || g.size() == 0 {
                    continue;
                }
                let sp = &cat.species[g.species as usize];
                // Somewhere to stand for every member, or the group waits.
                let Some(centre) = ground.top(g.pos[0], g.pos[1]) else {
                    continue;
                };
                if centre.water && !sp.aquatic {
                    continue;
                }
                let spread = (g.size() as f64).sqrt() * (2.0 + sp.mass_kg.sqrt() as f64 * 0.25);
                let mut members: Vec<(Stage, bool)> = Vec::new();
                members.extend((0..g.females).map(|_| (Stage::Adult, true)));
                members.extend((0..g.males).map(|_| (Stage::Adult, false)));
                for _ in 0..g.juveniles {
                    let f = self.rng.next_f32() < 0.5;
                    members.push((Stage::Juvenile, f));
                }
                for _ in 0..g.young {
                    let f = self.rng.next_f32() < 0.5;
                    members.push((Stage::Young, f));
                }
                let mut placed = Vec::new();
                for (stage, female) in members {
                    let mut spot = None;
                    for _ in 0..6 {
                        let a = self.rng.next_f64() * std::f64::consts::TAU;
                        let d = spread * self.rng.next_f64().sqrt();
                        let (x, z) = (g.pos[0] + a.cos() * d, g.pos[1] + a.sin() * d);
                        if let Some(f) = ground.footing(x, z, centre.y)
                            && !f.water
                        {
                            spot = Some(DVec3::new(x, f.y, z));
                            break;
                        }
                    }
                    let pos = spot.unwrap_or(DVec3::new(g.pos[0], centre.y, g.pos[1]));
                    placed.push((stage, female, pos));
                }
                g.live = true;
                for (stage, female, pos) in placed {
                    let id = self.next_id;
                    self.next_id += 1;
                    let yaw = self.rng.next_f32() * std::f32::consts::TAU;
                    self.animals.push(Animal {
                        id,
                        species: g.species,
                        group: Some(g.id),
                        cell: None,
                        stage,
                        female,
                        pos,
                        yaw,
                        speed: 0.0,
                        act: Act::Graze,
                        timer: 2.0 + self.rng.next_f32() * 10.0,
                        goal: None,
                        stride: 0.0,
                        dead: false,
                    });
                }
            }
            // The small species of the cells near the player.
            for (slot, &si) in r.pool_species.iter().enumerate() {
                let sp = &cat.species[si as usize];
                if !walks(sp) {
                    continue;
                }
                for c in 0..REGION_LEN {
                    let centre = r.cell_centre(c);
                    if dist(centre, at, wrap) > SMALL_NEAR_M + CELL_M * 0.71 {
                        continue;
                    }
                    // The share of the cell within reach (a lattice of its points).
                    let inside: Vec<[f64; 2]> = (0..16)
                        .map(|k| {
                            let (u, v) = ((k % 4) as f64 + 0.5, (k / 4) as f64 + 0.5);
                            [
                                centre[0] + (u / 4.0 - 0.5) * CELL_M,
                                centre[1] + (v / 4.0 - 0.5) * CELL_M,
                            ]
                        })
                        .filter(|p| dist(*p, at, wrap) <= SMALL_NEAR_M)
                        .collect();
                    if inside.is_empty() {
                        continue;
                    }
                    let k = (key, slot, c);
                    if self.drawn.contains_key(&k) {
                        continue;
                    }
                    // How many are about this cell's part near the player: its numbers, the
                    // share of the cell within reach, drawn the same way each day.
                    let i = slot * REGION_LEN + c;
                    let (adults, young) = (r.adults[i], r.young[i] * sp.census_young());
                    let near_share = inside.len() as f32 / 16.0;
                    let h = hash2(
                        hash2(self.seed, key.0 as u64 ^ (key.1 as u64) << 20),
                        (c * 64 + slot) as u64,
                    );
                    let u = hearth_math::hash::unit_f32(h);
                    let draw = |n: f32| -> u32 {
                        let n = n * near_share;
                        (n.floor() + if u < n.fract() { 1.0 } else { 0.0 }) as u32
                    };
                    let na = draw(adults).min(SMALL_PER_CELL);
                    let ny = draw(young).min(SMALL_PER_CELL.saturating_sub(na));
                    if na + ny == 0 {
                        self.drawn.insert(k, (0, 0));
                        continue;
                    }
                    let mut spots = Vec::new();
                    for _ in 0..(na + ny) {
                        let p = inside[self.rng.below(inside.len() as u32) as usize];
                        let x = p[0] + (self.rng.next_f64() - 0.5) * CELL_M / 4.0;
                        let z = p[1] + (self.rng.next_f64() - 0.5) * CELL_M / 4.0;
                        if let Some(f) = ground.top(x, z)
                            && !f.water
                        {
                            spots.push(DVec3::new(x, f.y, z));
                        }
                    }
                    if spots.len() < (na + ny) as usize {
                        // Not loaded yet: wait.
                        continue;
                    }
                    r.adults[i] = (r.adults[i] - na as f32).max(0.0);
                    r.young[i] = (r.young[i] - ny as f32).max(0.0);
                    self.drawn.insert(k, (na, ny));
                    for (n, pos) in spots.into_iter().enumerate() {
                        let id = self.next_id;
                        self.next_id += 1;
                        let yaw = self.rng.next_f32() * std::f32::consts::TAU;
                        let female = self.rng.next_f32() < 0.5;
                        self.animals.push(Animal {
                            id,
                            species: si,
                            group: None,
                            cell: Some((key, c)),
                            stage: if (n as u32) < na {
                                Stage::Adult
                            } else {
                                Stage::Young
                            },
                            female,
                            pos,
                            yaw,
                            speed: 0.0,
                            act: Act::Graze,
                            timer: 2.0 + self.rng.next_f32() * 8.0,
                            goal: None,
                            stride: 0.0,
                            dead: false,
                        });
                    }
                }
            }
        }
    }

    /// Folds the groups and small animals far from `player` back into their numbers: the
    /// living into their group (by age and sex) or cell, the dead not at all.
    pub fn fold(&mut self, eco: &mut Ecology, player: DVec3) {
        let wrap = eco.cells_around as f64 * CELL_M;
        // Groups whose animals are all far (or gone).
        let mut groups: FxHashMap<u64, (bool, Vec<usize>)> = FxHashMap::default();
        for (k, a) in self.animals.iter().enumerate() {
            if let Some(g) = a.group {
                let e = groups.entry(g).or_insert((true, Vec::new()));
                e.1.push(k);
                if !a.dead && hdist(a.pos, player) <= FAR_M {
                    e.0 = false;
                }
            }
        }
        let mut remove = vec![false; self.animals.len()];
        for r in eco.regions.values_mut() {
            for g in r.groups.iter_mut().filter(|g| g.live) {
                let Some((far, members)) = groups.get(&g.id) else {
                    // A live group with no animals (all taken): fold it empty.
                    if dist(g.pos, [player.x, player.z], wrap) > FAR_M {
                        g.live = false;
                        g.young = 0;
                        g.juveniles = 0;
                        g.females = 0;
                        g.males = 0;
                    }
                    continue;
                };
                if !*far {
                    continue;
                }
                let (mut young, mut juv, mut f, mut m) = (0u16, 0u16, 0u16, 0u16);
                let (mut cx, mut cz, mut n) = (0.0f64, 0.0f64, 0.0f64);
                for &k in members {
                    let a = &self.animals[k];
                    remove[k] = true;
                    if a.dead {
                        continue;
                    }
                    match (a.stage, a.female) {
                        (Stage::Young, _) => young += 1,
                        (Stage::Juvenile, _) => juv += 1,
                        (Stage::Adult, true) => f += 1,
                        (Stage::Adult, false) => m += 1,
                    }
                    cx += a.pos.x;
                    cz += a.pos.z;
                    n += 1.0;
                }
                g.young = young;
                g.juveniles = juv;
                g.females = f;
                g.males = m;
                if n > 0.0 {
                    g.pos = [(cx / n).rem_euclid(wrap), cz / n];
                }
                g.live = false;
            }
        }
        // Small animals far from the player go back to their cells.
        let mut back: FxHashMap<((i64, i64), usize, usize), (u32, u32)> = FxHashMap::default();
        for (k, a) in self.animals.iter().enumerate() {
            let Some((key, c)) = a.cell else {
                continue;
            };
            let Some(r) = eco.regions.get(&key) else {
                remove[k] = true;
                continue;
            };
            let Some(slot) = r.pool_species.iter().position(|s| *s == a.species) else {
                remove[k] = true;
                continue;
            };
            let cell_at = r.cell_centre(c);
            if dist(cell_at, [player.x, player.z], wrap) <= FAR_M && !a.dead {
                // Its cell is still near: it stays while it is.
                if hdist(a.pos, player) <= FAR_M {
                    continue;
                }
            }
            remove[k] = true;
            let e = back.entry((key, slot, c)).or_default();
            if !a.dead {
                if a.stage == Stage::Adult {
                    e.0 += 1;
                } else {
                    e.1 += 1;
                }
            }
        }
        for ((key, slot, c), (na, ny)) in back {
            if let Some(r) = eco.regions.get_mut(&key) {
                let i = slot * REGION_LEN + c;
                r.adults[i] += na as f32;
                r.young[i] += ny as f32;
            }
            self.drawn.remove(&(key, slot, c));
        }
        // Cells far from the player are drawn afresh next time.
        self.drawn.retain(|(key, _, c), _| {
            eco.regions
                .get(key)
                .is_some_and(|r| dist(r.cell_centre(*c), [player.x, player.z], wrap) <= FAR_M)
        });
        let mut k = 0;
        self.animals.retain(|_| {
            let keep = !remove[k];
            k += 1;
            keep
        });
    }

    /// One step of `dt` seconds: grazing, wandering, resting by the hour of their kind,
    /// fleeing a person who comes too near.
    /// Every animal onto the ground under it (where the ground has changed or loaded since).
    pub fn settle(&mut self, ground: &dyn Ground) {
        for a in &mut self.animals {
            if let Some(f) = ground.footing(a.pos.x, a.pos.z, a.pos.y) {
                a.pos.y = f.y;
            }
        }
    }

    pub fn step(
        &mut self,
        eco: &Ecology,
        ground: &dyn Ground,
        player: Option<DVec3>,
        hour: f32,
        dt: f32,
    ) {
        let cat = eco.catalog.clone();
        // Where each group's members are, about.
        let mut centres: FxHashMap<u64, (DVec2, f64)> = FxHashMap::default();
        for a in &self.animals {
            if let (Some(g), false) = (a.group, a.dead) {
                let e = centres.entry(g).or_insert((DVec2::ZERO, 0.0));
                e.0 += DVec2::new(a.pos.x, a.pos.z);
                e.1 += 1.0;
            }
        }
        for a in self.animals.iter_mut() {
            if a.dead {
                continue;
            }
            let sp = &cat.species[a.species as usize];
            let walk = sp.walk_speed();
            let run = sp.run_speed();
            let flight = sp.flight_m();
            // A person within its flight distance: away, as fast as it goes.
            if let Some(p) = player {
                let d = hdist(a.pos, p);
                if d < flight as f64 {
                    let away = DVec2::new(a.pos.x - p.x, a.pos.z - p.z).normalize_or_zero();
                    let away = if away == DVec2::ZERO { DVec2::X } else { away };
                    a.goal = Some(DVec2::new(a.pos.x, a.pos.z) + away * (flight as f64 * 1.5));
                    a.act = Act::Flee;
                    a.timer = 6.0;
                } else if d < flight as f64 * 1.6 && a.act != Act::Flee {
                    a.act = Act::Alert;
                    a.timer = a.timer.max(2.0);
                    a.yaw = turn_toward(
                        a.yaw,
                        (p.x - a.pos.x) as f32,
                        (p.z - a.pos.z) as f32,
                        4.0 * dt,
                    );
                }
            }
            a.timer -= dt;
            if a.timer <= 0.0 {
                let up = awake(sp.activity, hour);
                let r = self.rng.next_f32();
                let centre = a
                    .group
                    .and_then(|g| centres.get(&g))
                    .map(|(s, n)| *s / *n)
                    .unwrap_or(DVec2::new(a.pos.x, a.pos.z));
                (a.act, a.timer, a.goal) = if !up {
                    // Out of its hours: asleep mostly, now and then awake where it lies.
                    let act = if r < 0.75 { Act::Sleep } else { Act::Rest };
                    (act, 20.0 + r * 40.0, None)
                } else if r < 0.5 {
                    (Act::Graze, 4.0 + r * 12.0, None)
                } else if r < 0.58 {
                    (Act::Groom, 3.0 + r * 4.0, None)
                } else {
                    // Somewhere near the group's middle.
                    let reach = 6.0 + sp.mass_kg.sqrt() as f64;
                    let ang = self.rng.next_f64() * std::f64::consts::TAU;
                    let goal =
                        centre + DVec2::new(ang.cos(), ang.sin()) * reach * self.rng.next_f64();
                    (Act::Walk, 20.0, Some(goal))
                };
            }
            // Moving toward the goal.
            let target_speed = match a.act {
                Act::Walk => walk,
                Act::Flee => run,
                _ => 0.0,
            };
            let mut moved = 0.0f32;
            if let (Some(goal), true) = (a.goal, target_speed > 0.0) {
                let to = goal - DVec2::new(a.pos.x, a.pos.z);
                let d = to.length();
                if d < 0.5 {
                    a.goal = None;
                    a.act = if a.act == Act::Flee {
                        Act::Alert
                    } else {
                        Act::Graze
                    };
                    a.timer = 3.0 + self.rng.next_f32() * 6.0;
                } else {
                    a.yaw = turn_toward(a.yaw, to.x as f32, to.y as f32, 5.0 * dt);
                    a.speed += (target_speed - a.speed) * (1.0 - (-4.0 * dt).exp());
                    let dir = DVec2::new(a.yaw.sin() as f64, a.yaw.cos() as f64);
                    let step = dir * (a.speed * dt) as f64;
                    let (nx, nz) = (a.pos.x + step.x, a.pos.z + step.y);
                    let climb = sp.climb_m() as f64;
                    match ground.footing(nx, nz, a.pos.y) {
                        Some(f) if !f.water || sp.aquatic => {
                            if (f.y - a.pos.y).abs() <= climb {
                                a.pos = DVec3::new(nx, f.y, nz);
                                moved = step.length() as f32;
                            } else {
                                // Too steep that way: turn and try elsewhere.
                                a.goal = None;
                                a.act = Act::Graze;
                                a.timer = 1.0;
                                a.yaw += std::f32::consts::FRAC_PI_2;
                            }
                        }
                        _ => {
                            a.goal = None;
                            a.act = Act::Graze;
                            a.timer = 1.0;
                            a.yaw += std::f32::consts::PI * 0.75;
                        }
                    }
                }
            } else {
                a.speed *= (-6.0 * dt).exp();
            }
            a.stride += moved / sp.stride_m().max(0.05);
        }
    }

    /// The animals as the client draws them.
    pub fn views(&self) -> Vec<AnimalView> {
        self.animals
            .iter()
            .filter(|a| !a.dead)
            .map(|a| AnimalView {
                id: a.id,
                species: a.species,
                stage: a.stage,
                female: a.female,
                pos: a.pos,
                yaw: a.yaw,
                speed: a.speed,
                act: a.act,
                stride: a.stride,
            })
            .collect()
    }

    /// Kills an animal (hunted, taken by a predator).
    pub fn kill(&mut self, id: u64) -> Option<&Animal> {
        let a = self.animals.iter_mut().find(|a| a.id == id)?;
        a.dead = true;
        a.speed = 0.0;
        Some(a)
    }

    /// Puts an animal of a species into the world (tests, screenshots): not of any group or
    /// cell, so it is gone when it folds.
    pub fn place(&mut self, species: u16, stage: Stage, female: bool, pos: DVec3, yaw: f32) -> u64 {
        let id = self.id();
        self.animals.push(Animal {
            id,
            species,
            group: None,
            cell: None,
            stage,
            female,
            pos,
            yaw,
            speed: 0.0,
            act: Act::Graze,
            timer: 5.0,
            goal: None,
            stride: 0.0,
            dead: false,
        });
        id
    }
}

/// Turns a facing toward the direction (dx, dz) by at most `max` radians.
fn turn_toward(yaw: f32, dx: f32, dz: f32, max: f32) -> f32 {
    if dx == 0.0 && dz == 0.0 {
        return yaw;
    }
    let want = dx.atan2(dz);
    let mut d = (want - yaw).rem_euclid(std::f32::consts::TAU);
    if d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    yaw + d.clamp(-max, max)
}

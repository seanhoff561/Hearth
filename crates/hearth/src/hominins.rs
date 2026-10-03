//! The hominins about the player (V2-11 (c)): the agents of `hearth_agent`, drawn out of the
//! ecology's hominin groups as the player comes near and folded back as the player goes, living
//! in the world as the game has it — the loaded ground, its trees and water, the things lying
//! about, the fruit and nut trees in their season, the weather on their bodies, the fauna's
//! hunters about them — their calls heard with the animals', their nests bent into the trees.
//! Where a group is drawn out for the first time, its site lies under the nearest nut tree: an
//! anvil with its hammers and the cobbles of the place, and the flakes and cores of its work
//! before (V2-11 (d)).

use glam::DVec3;
use hearth_agent::Things;
use hearth_agent::live::{AgentView, AgentWorld, FoodHere, Hominins, Now, Person};
use hearth_agent::{Kind, Kinds};
use hearth_body::{BodyConfig, Exposure};
use hearth_character::{Activity, Drive};
use hearth_content::Content;
use hearth_content::schema::flora::PartKind;
use hearth_craft::engine::Surroundings;
use hearth_craft::{Crafts, Graph};
use hearth_fauna::live::Ground;
use hearth_items::{Items, Stack, WorldItems};
use hearth_math::BlockPos;

use crate::fauna::{Fauna, MapGround};
use crate::scene::LocalWorld;

/// A fruit or nut tree about the agents: where its crown stands over the ground, and what it
/// gives now (its fruit in season; the stones of its nuts lying under it all year).
#[derive(Debug, Clone)]
struct FoodTree {
    at: DVec3,
    fruit: Option<String>,
    nuts: Option<String>,
}

/// How far from a food tree's trunk its food lies (m).
const UNDER_TREE_M: f64 = 4.5;
/// The trees about the player are looked over again when it has gone this far (m), or every
/// game hour.
const TREES_REFRESH_M: f64 = 60.0;
/// How far about the player the food trees are looked for (m).
const TREES_M: i32 = 160;
/// What the open savanna's ground gives a forager a minute (kg): seeds and grubs, thinly.
const GROUND_KG_MIN: f32 = 0.02;
/// How far from a group's place the nut tree of its site may stand (m): within the reach in
/// which a group drawn out knows its anvils.
const SITE_M: f64 = 70.0;
/// No second site is laid within this of an anvil already lying (m).
const SITE_APART_M: f64 = 25.0;

/// The hominins about the player.
pub struct Agents {
    pub live: Hominins,
    pub kinds: Kinds,
    trees: Vec<FoodTree>,
    trees_at: Option<(DVec3, u64)>,
    /// They were in sight at the last sending.
    pub shown: bool,
}

/// The world as the agents live in it, for one tick.
struct World<'a> {
    ground: MapGround<'a>,
    things: OnTheGround<'a>,
    items: &'a Items,
    trees: &'a [FoodTree],
    hunters: Vec<DVec3>,
    exposure: Exposure,
    around: Surroundings,
    calls: Vec<(DVec3, bool)>,
    nests: Vec<BlockPos>,
}

/// The things lying in the world, as the agents reach them.
struct OnTheGround<'a> {
    items: &'a mut WorldItems,
    lw: &'a LocalWorld,
    changed: bool,
}

impl Things for OnTheGround<'_> {
    fn near(&self, at: DVec3, reach: f64) -> Vec<(u64, Stack)> {
        self.items
            .items
            .iter()
            .filter(|w| (DVec3::from_array(w.pos) - at).length() <= reach)
            .map(|w| (w.id, w.stack.clone()))
            .collect()
    }

    fn take(&mut self, id: u64, count: Option<u16>) -> Option<Stack> {
        self.changed = true;
        let w = self.items.get_mut(id)?;
        match count {
            Some(n) if n < w.stack.count => {
                w.stack.count -= n;
                let mut part = w.stack.clone();
                part.count = n;
                part.inside = None;
                Some(part)
            }
            _ => self.items.take(id).map(|w| w.stack),
        }
    }

    fn lay(&mut self, at: DVec3, stack: Stack) -> u64 {
        self.changed = true;
        let p = crate::server::rest_on(self.lw, at);
        self.items.add(stack, p.to_array(), 0.0)
    }

    fn place(&self, id: u64) -> Option<DVec3> {
        self.items.get(id).map(|w| DVec3::from_array(w.pos))
    }
}

impl AgentWorld for World<'_> {
    fn ground(&self) -> &dyn Ground {
        &self.ground
    }

    fn things(&mut self) -> &mut dyn Things {
        &mut self.things
    }

    fn food_at(&self, at: DVec3) -> Option<FoodHere> {
        let tree = self
            .trees
            .iter()
            .filter(|t| t.fruit.is_some() || t.nuts.is_some())
            .find(|t| (t.at - at).length() < UNDER_TREE_M);
        if let Some(t) = tree {
            return Some(FoodHere {
                material: t
                    .fruit
                    .clone()
                    .unwrap_or_else(|| "insect_larvae".to_owned()),
                kg_min: if t.fruit.is_some() {
                    0.3
                } else {
                    GROUND_KG_MIN
                },
                nuts: t.nuts.clone(),
            });
        }
        // The open ground: seeds and grubs, thinly; not in the water.
        let f = self.ground.footing(at.x, at.z, at.y)?;
        (!f.water).then(|| FoodHere {
            material: "insect_larvae".to_owned(),
            kg_min: GROUND_KG_MIN,
            nuts: None,
        })
    }

    fn food_near(&self, at: DVec3, within: f64) -> Option<DVec3> {
        self.trees
            .iter()
            .filter(|t| t.fruit.is_some() || t.nuts.is_some())
            .map(|t| t.at)
            .filter(|p| (*p - at).length() < within)
            .min_by(|a, b| (*a - at).length().total_cmp(&(*b - at).length()))
    }

    fn exposure(&self, _at: DVec3, in_tree: bool) -> Exposure {
        let mut e = self.exposure;
        if in_tree {
            // In the crown's shade, on a nest of leaves.
            e.radiant_w_m2 *= 0.4;
            e.ground_clo = 0.6;
        }
        e
    }

    fn surroundings(&self, _at: DVec3) -> Surroundings {
        self.around.clone()
    }

    fn hunters_near(&self, at: DVec3, within: f64) -> Vec<DVec3> {
        self.hunters
            .iter()
            .copied()
            .filter(|h| (*h - at).length() < within)
            .collect()
    }

    fn call(&mut self, at: DVec3, alarm: bool) {
        self.calls.push((at, alarm));
    }

    fn nest(&mut self, at: DVec3) -> Option<DVec3> {
        let lw = self.things.lw;
        let nest = lw.reg.parse_state("hearth:leaf_nest").ok()?;
        let p = nest_spot(lw, at, nest).filter(|p| !self.nests.contains(p))?;
        self.nests.push(p);
        // On it: its top is five sixteenths up the block.
        Some(DVec3::new(
            p.x as f64 + 0.5,
            p.y as f64 + 5.0 / 16.0,
            p.z as f64 + 0.5,
        ))
    }

    fn settle(&mut self, at: DVec3, _kind: &Kind) {
        // Under the nut tree nearest the group's place.
        let Some(tree) = self
            .trees
            .iter()
            .filter(|t| t.nuts.is_some())
            .map(|t| t.at)
            .filter(|p| (p.x - at.x).hypot(p.z - at.z) < SITE_M)
            .min_by(|a, b| (*a - at).length().total_cmp(&(*b - at).length()))
        else {
            return;
        };
        // One laid before (or the player's anvil there): nothing more.
        let laid = self.things.items.items.iter().any(|w| {
            w.stack.id.contains("anvil_stone/")
                && (DVec3::from_array(w.pos) - tree).length() < SITE_APART_M
        });
        if laid {
            return;
        }
        let Some(stones) = self.stones_at(tree) else {
            return;
        };
        let mut rng = hearth_math::hash::Rng::new(
            (tree.x.floor() as i64 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
                ^ (tree.z.floor() as i64 as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f),
        );
        // The anvil a few metres out from the trunk, its hammers and cobbles by it, the flakes
        // and a spent core of the work done there before scattered about.
        let a = rng.next_f64() * std::f64::consts::TAU;
        let spot = tree + DVec3::new(a.cos() * 3.0, 0.0, a.sin() * 3.0);
        let near = |r: f64, rng: &mut hearth_math::hash::Rng| {
            let b = rng.next_f64() * std::f64::consts::TAU;
            let d = r * (0.4 + 0.6 * rng.next_f64());
            spot + DVec3::new(b.cos() * d, 0.0, b.sin() * d)
        };
        let mut laid: Vec<(DVec3, Stack)> = vec![(spot, Stack::of(&stones.anvil, 1))];
        for _ in 0..2 {
            laid.push((near(0.8, &mut rng), Stack::of(&stones.hammer, 1)));
        }
        for _ in 0..3 {
            laid.push((near(1.4, &mut rng), Stack::of(&stones.cobble, 1)));
        }
        laid.push((near(1.8, &mut rng), Stack::of(&stones.core, 1)));
        for _ in 0..5 {
            laid.push((near(2.2, &mut rng), Stack::of(&stones.flake, 1)));
        }
        for (p, stack) in laid {
            self.things.lay(p, stack);
        }
    }
}

/// The stones of a hominin site.
struct Stones {
    anvil: String,
    hammer: String,
    cobble: String,
    core: String,
    flake: String,
}

impl World<'_> {
    /// The stones of a site at a place: the place's own rock where it makes them (an anvil,
    /// hammers where it is hard, cobbles to knap where it breaks well), else what river gravels
    /// give (quartzite) and the rift's basalt for the anvil.
    fn stones_at(&self, at: DVec3) -> Option<Stones> {
        use hearth_content::generate::generated_id;
        let lw = self.things.lw;
        let rock = lw.generator.rock_at(
            at.x.floor() as i32,
            at.y.floor() as i32 - 3,
            at.z.floor() as i32,
        );
        let local = lw.reg.block_of(rock).def.material.clone();
        let tagged = |m: &str, tag: &str| {
            lw.content
                .materials
                .get(m)
                .is_some_and(|mat| mat.tags.iter().any(|t| t == tag))
        };
        let item = |form: &str, m: &str| {
            let id = generated_id(form, m);
            self.items.get(&id).map(|_| id)
        };
        let of = |form: &str, tag: &str, fallback: &str| {
            local
                .as_deref()
                .filter(|m| tag.is_empty() || tagged(m, tag))
                .and_then(|m| item(form, m))
                .or_else(|| item(form, fallback))
        };
        Some(Stones {
            anvil: of("hearth:anvil_stone", "", "hearth:basalt")?,
            hammer: of("hearth:cobble", "hammerstone", "hearth:quartzite")?,
            cobble: of("hearth:cobble", "knappable", "hearth:quartzite")?,
            core: of("hearth:core", "knappable", "hearth:quartzite")?,
            flake: of("hearth:flake", "knappable", "hearth:quartzite")?,
        })
    }
}

impl Agents {
    pub fn new(content: &Content, graph: &Graph, body: &BodyConfig, seed: u64) -> Self {
        Self {
            live: Hominins::new(seed),
            kinds: Kinds::from_content(content, graph, body),
            trees: Vec::new(),
            trees_at: None,
            shown: false,
        }
    }

    /// The fruit and nut trees about a place, and what each gives at this time of the year.
    fn look_for_trees(&mut self, lw: &LocalWorld, at: DVec3, year_frac: f32, tick: u64) {
        let fresh = self.trees_at.is_some_and(|(p, t)| {
            (p - at).length() < TREES_REFRESH_M && tick.saturating_sub(t) < 2400
        });
        if fresh {
            return;
        }
        self.trees_at = Some((at, tick));
        let (x, z) = (at.x.floor() as i32, at.z.floor() as i32);
        let placed = lw.generator.features().trees_in(
            &lw.generator,
            &lw.vegetation,
            (x - TREES_M, z - TREES_M),
            (x + TREES_M, z + TREES_M),
        );
        let forest = &lw.generator.forest;
        self.trees = placed
            .iter()
            .filter(|t| t.remains == hearth_worldgen::cubegen::features::Remains::Living)
            .filter_map(|t| {
                let id = &forest.templates.species.get(t.species)?.id;
                let plant = lw.content.plants.get(id)?;
                let in_season = plant.phenology.fruiting.is_some_and(|r| {
                    let (a, b) = (r.0, r.1);
                    if a <= b {
                        (a..=b).contains(&year_frac)
                    } else {
                        year_frac >= a || year_frac <= b
                    }
                });
                let mut fruit = None;
                let mut nuts = None;
                for part in &plant.parts {
                    let m = part.material.to_string();
                    match part.part {
                        PartKind::Fruit if in_season => fruit = Some(m),
                        // Hard nuts lie under the tree long after it fruits.
                        PartKind::Nut => nuts = Some(m),
                        _ => {}
                    }
                }
                (fruit.is_some() || nuts.is_some()).then(|| FoodTree {
                    at: DVec3::new(
                        t.foot[0] as f64 + 0.5,
                        t.foot[1] as f64,
                        t.foot[2] as f64 + 0.5,
                    ),
                    fruit,
                    nuts,
                })
            })
            .collect();
    }

    /// A tick of `dt` seconds of play: every 40 ticks the groups near the player are drawn out
    /// and those far folded back; every tick the agents live, with the person among them (if
    /// any) to notice. Blocks it changed (nests) go into `changed`; true when the things lying
    /// about changed.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        lw: &mut LocalWorld,
        fauna: &mut Fauna,
        world_items: &mut WorldItems,
        items: &Items,
        crafts: &Crafts,
        graph: &Graph,
        changed: &mut Vec<BlockPos>,
        exposure: Exposure,
        around: Surroundings,
        year_frac: f32,
        player: DVec3,
        person: Option<Person>,
        now: Now,
        dt: f32,
    ) -> bool {
        let every = now.tick.is_multiple_of(40);
        if self.live.groups.is_empty() && !every {
            return false;
        }
        // The fruit and nut trees about them, while there are agents to feed at them or groups
        // about to be drawn out.
        if every && (!self.live.groups.is_empty() || coming(fauna, player)) {
            self.look_for_trees(lw, player, year_frac, now.tick);
        }
        // The hunters about: the fauna's animals that take a hominin.
        let hominin_species: Vec<usize> = fauna
            .eco
            .catalog
            .species
            .iter()
            .enumerate()
            .filter(|(_, s)| s.hominin)
            .map(|(i, _)| i)
            .collect();
        let hunters: Vec<DVec3> = fauna
            .live
            .animals
            .iter()
            .filter(|a| !a.dead && (a.pos - player).length() < 260.0)
            .filter(|a| {
                fauna.eco.catalog.species[a.species as usize]
                    .prey
                    .iter()
                    .any(|(p, _)| hominin_species.contains(p))
            })
            .map(|a| a.pos)
            .collect();
        let content = lw.content.clone();
        let (calls, nests, items_changed) = {
            let mut world = World {
                ground: MapGround {
                    map: &lw.map,
                    reg: &lw.reg,
                    lw,
                    cells: &fauna.cells,
                },
                things: OnTheGround {
                    items: world_items,
                    lw,
                    changed: false,
                },
                items,
                trees: &self.trees,
                hunters,
                exposure,
                around,
                calls: Vec::new(),
                nests: Vec::new(),
            };
            if now.tick.is_multiple_of(40) {
                self.live.fold(&mut fauna.eco, player);
                self.live.materialize(
                    &mut fauna.eco,
                    &self.kinds,
                    graph,
                    items,
                    &mut world,
                    player,
                    now.tick,
                );
            }
            if !self.live.agents.is_empty() {
                self.live.step(
                    &self.kinds,
                    crafts,
                    &content,
                    items,
                    &mut world,
                    person,
                    now,
                    dt,
                );
            }
            (world.calls, world.nests, world.things.changed)
        };
        // Their calls, heard as the animals' are: the population's alarm.
        for (at, alarm) in calls {
            let Some(sp) = hominin_species.first().copied() else {
                break;
            };
            let when = if alarm {
                hearth_content::schema::fauna::CallWhen::Alarm
            } else {
                hearth_content::schema::fauna::CallWhen::Contact
            };
            if let Some(call) = hearth_fauna::voices::call_for(&fauna.eco.catalog.species[sp], when)
            {
                fauna.live.calls.push(hearth_fauna::voices::Called {
                    species: sp as u16,
                    call,
                    pos: at,
                });
            }
        }
        // Their nests, bent into the crowns beside the trunk each climbed.
        if !nests.is_empty()
            && let Ok(nest) = lw.reg.parse_state("hearth:leaf_nest")
        {
            let reg = lw.reg.clone();
            for p in nests {
                lw.map.set_block(p, nest, &reg);
                lw.edits.set(p, nest);
                changed.push(p);
            }
        }
        items_changed
    }

    /// What the person watching sees done this tick, and whether they stand by a scatter of
    /// knapped stone: the triggers to hear.
    pub fn watched(
        &self,
        crafts: &Crafts,
        graph: &Graph,
        world_items: &WorldItems,
        eye: DVec3,
        yaw: f32,
    ) -> Vec<String> {
        if self.live.agents.is_empty() {
            return Vec::new();
        }
        let mut out = hearth_agent::seen(&self.live.done, graph, crafts, eye, yaw);
        let lying = world_items
            .items
            .iter()
            .map(|w| (DVec3::from_array(w.pos), w.stack.id.as_str(), w.stack.count));
        if hearth_agent::scatter_near(lying, eye) {
            out.push(STUDY_SCATTER.to_owned());
        }
        out
    }

    /// The agents as the client draws them.
    pub fn views(&self) -> Vec<AgentView> {
        self.live.views(&self.kinds)
    }
}

/// Where a nest goes for one up a tree at `at`: beside its trunk at its height or a block up, in
/// the leaves (bent over into it) or the open air against the trunk, a branch or the leaves;
/// none where there is a nest already, or nowhere fit.
fn nest_spot(lw: &LocalWorld, at: DVec3, nest: hearth_world::BlockStateId) -> Option<BlockPos> {
    let reg = &lw.reg;
    let base = BlockPos::containing(at + DVec3::Y * 0.5);
    let leafy = |p: BlockPos| {
        lw.map
            .block(p)
            .is_some_and(|s| reg.block_of(s).name.path().ends_with("_leaves"))
    };
    let woody = |p: BlockPos| {
        lw.map.block(p).is_some_and(|s| {
            let n = reg.block_of(s).name.path();
            n.ends_with("_log") || n.ends_with("_branch")
        })
    };
    let open = |p: BlockPos| {
        lw.map
            .block(p)
            .is_some_and(|s| s.is_air() || reg.block_of(s).def.replaceable)
    };
    let ring = [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];
    let mut spots = Vec::new();
    for dy in [0, 1, -1] {
        for (dx, dz) in ring {
            spots.push(BlockPos::new(base.x + dx, base.y + dy, base.z + dz));
        }
    }
    // One made here before: none again.
    if spots.iter().any(|p| lw.map.block(*p) == Some(nest)) {
        return None;
    }
    let held = |p: BlockPos| {
        [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dz)| {
            let q = BlockPos::new(p.x + dx, p.y, p.z + dz);
            woody(q) || leafy(q)
        }) || woody(BlockPos::new(p.x, p.y - 1, p.z))
    };
    spots
        .iter()
        .copied()
        .find(|p| leafy(*p))
        .or_else(|| spots.iter().copied().find(|p| open(*p) && held(*p)))
}

/// The trigger of studying a scatter of knapped stone.
const STUDY_SCATTER: &str = "study:tool_scatter";

/// Whether a hominin group not yet drawn out is near enough the player to be soon.
fn coming(fauna: &Fauna, player: DVec3) -> bool {
    fauna.eco.regions.values().any(|r| {
        r.groups.iter().any(|g| {
            !g.live
                && fauna.eco.catalog.species[g.species as usize].hominin
                && (g.pos[0] - player.x).hypot(g.pos[1] - player.z)
                    < hearth_agent::live::NEAR_M + 20.0
        })
    })
}

/// How a hominin looks: its height and sex, dark-skinned under a coat of brown hair.
pub fn looks(v: &AgentView) -> hearth_character::Appearance {
    hearth_character::Appearance {
        body: if v.female {
            hearth_character::BodyType::Female
        } else {
            hearth_character::BodyType::Male
        },
        height_m: v.height_m,
        build: 0.7,
        skin_tone: 0.72,
        undertone: 0.0,
        hair: hearth_character::HairStyle::ShortCrop,
        hair_color: [74, 54, 38],
        ..hearth_character::Appearance::default()
    }
}

/// How a hominin's figure moves for what it is doing: walking and running, climbing its tree,
/// crouched at its work or its food, lying asleep in its nest, standing to watch or to call.
pub fn drive(v: &AgentView) -> Drive {
    use hearth_agent::Doing;
    let in_tree = v.medium == hearth_fauna::live::Medium::Tree;
    let activity = match &v.doing {
        Doing::Sleeping => Activity::Lie,
        _ if in_tree && v.speed > 0.05 => Activity::Ladder,
        Doing::Working { .. } | Doing::Feeding | Doing::Drinking | Doing::Nesting => {
            Activity::Crouch
        }
        Doing::Grooming { .. } | Doing::Resting => Activity::Crouch,
        _ if v.speed > 3.0 => Activity::Sprint,
        _ if v.speed > 1.8 => Activity::Jog,
        _ if v.speed > 0.1 => Activity::Walk,
        _ => Activity::Stand,
    };
    Drive {
        activity,
        speed: v.speed,
        vertical: if in_tree { v.speed } else { 0.0 },
        look_pitch: 0.0,
        look_yaw: 0.0,
        climb: 0.0,
        shiver: 0.0,
        breaths_per_min: 16.0,
        holding: hearth_character::Holding::default(),
    }
}

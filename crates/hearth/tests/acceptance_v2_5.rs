//! V2-5 acceptance: a scripted bot, starting with nothing but a loincloth in a world where
//! knowledge comes only by discovery, gets fire, a stone-tipped spear, sewn hide clothing and
//! dried meat. It is given no knowledge and no things: every technique comes from what it does
//! and sees (the knowledge's own routes), and everything from the world. The world's clock
//! runs as the bot asks (lockstep); the bot works from a camp by the spawn, fetching what it
//! needs and carrying it back a handful at a time (it steps from place to place rather than
//! walking). It lives as a person must: it drinks at the river it saw from afar when thirsty
//! (and finds it again as the summer lowers it), keeps its fire through the night from a bed of
//! grass beside it, lights it again by drilling when it goes out, and dresses for the autumn.
//! About twenty days of the world pass in a couple of minutes; watch with `--nocapture`.
//!
//! Lightning is summoned at a tree near the camp (a natural event a test may force); kills turn
//! up on their own every day or two.

mod common;

use common::{World, temp};
use glam::DVec3;
use hearth_items::{Hand, Path, Root, Target};
use hearth_math::BlockPos;
use hearth_protocol::{AimAt, DrinkFrom, ToServer};

struct Bot {
    w: World,
    camp: DVec3,
    /// When next to drink and eat.
    upkeep_at: u64,
    /// Where water was found, and whether it has been looked for.
    water: Option<BlockPos>,
    looked_for_water: bool,
    /// The camp's hearth, once laid.
    hearth: Option<BlockPos>,
    /// Blocks a process got nothing more from (this year), or could not reach.
    spent: std::collections::HashSet<(BlockPos, String)>,
    /// Water that could not be drunk from (out of reach).
    bad_water: Vec<BlockPos>,
    /// When the fire was last fed.
    fed_at: u64,
    /// Lighting the fire again (the work that takes is not interrupted to tend it).
    relighting: bool,
    /// The bed beside the hearth, once heaped up.
    bed: Option<BlockPos>,
    /// The tree the lightning set burning (while it burns, fire can be carried from it).
    wildfire: Option<BlockPos>,
    /// Tending the fire now (gathering wood for it does not tend it again).
    tending: bool,
}

fn id(s: &str) -> String {
    format!("hearth:{s}")
}

impl Bot {
    fn say(&self, s: &str) {
        println!(
            "[day {:.2}] {s}",
            self.w.ticks as f64 / self.w.ticks_per_day
        );
    }

    fn knows(&self, node: &str) -> bool {
        self.w.knowledge.knows(&id(node))
    }

    fn home(&mut self) {
        let c = self.camp;
        self.w.go_exact(c);
    }

    fn near_camp(&self, p: [f64; 3]) -> bool {
        (DVec3::from_array(p) - self.camp).length() < 2.4
    }

    /// Things lying at camp passing `pred`, nearest first.
    fn at_camp(&self, pred: impl Fn(&str) -> bool) -> Vec<u64> {
        let mut v: Vec<(f64, u64)> = self
            .w
            .lying
            .iter()
            .filter(|l| l.work.is_none() && pred(&l.stack.id) && self.near_camp(l.pos))
            .map(|l| ((DVec3::from_array(l.pos) - self.camp).length(), l.id))
            .collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        v.into_iter().map(|(_, id)| id).collect()
    }

    /// How many of a kind (by part of the id: `stick/`, `cut/meat`) are at camp or carried.
    fn count(&self, part: &str) -> u32 {
        let mut n: u32 = self
            .w
            .lying
            .iter()
            .filter(|l| l.work.is_none() && l.stack.id.contains(part) && self.near_camp(l.pos))
            .map(|l| l.stack.count as u32)
            .sum();
        let mut c = self.w.carry.clone();
        c.for_each_mut(&mut |s| {
            if s.id.contains(part) {
                n += s.count as u32;
            }
        });
        n
    }

    /// Carries everything lying within 3 m of `site` to camp, a handful at a time.
    fn haul(&mut self, site: DVec3) {
        for round in 0..60 {
            if round == 59 {
                self.say("could not haul everything home");
            }
            let here: Vec<u64> = self
                .w
                .lying
                .iter()
                .filter(|l| {
                    l.work.is_none()
                        && (DVec3::from_array(l.pos) - site).length() < 3.0
                        && !self.near_camp(l.pos)
                })
                .map(|l| l.id)
                .collect();
            if here.is_empty() {
                return;
            }
            self.w.go_exact(site);
            for id in here.into_iter().take(3) {
                self.w.pick_up(id);
            }
            self.home();
            self.w.put_down_all();
            self.w.let_go();
        }
    }

    /// Does `process` to the nearest blocks passing `block` until it has worked `times`,
    /// carrying what they give home: about camp (within `r`), then farther afield (from points
    /// 50 and 90 m out) when that is not enough. Blocks that give no more are remembered.
    fn gather(
        &mut self,
        r: i32,
        block: impl Fn(&str, Option<&str>) -> bool,
        process: &str,
        times: usize,
    ) -> usize {
        let mut centres = vec![(0.0, 0.0)];
        for ring in [50.0, 90.0] {
            for k in 0..8 {
                let a = k as f64 * std::f64::consts::FRAC_PI_4;
                centres.push((a.cos() * ring, a.sin() * ring));
            }
        }
        let mut done = 0;
        for (i, (dx, dz)) in centres.into_iter().enumerate() {
            if done >= times {
                break;
            }
            let reach = if i == 0 {
                self.home();
                r
            } else {
                self.w.go(self.camp.x + dx, self.camp.z + dz);
                r.min(44)
            };
            let found: Vec<BlockPos> = self
                .w
                .find(reach, &block)
                .into_iter()
                .filter(|p| !self.spent.contains(&(*p, process.to_owned())))
                .filter(|p| self.w.stand_by(*p).is_some())
                .collect();
            done += self.gather_from(found, process, times - done);
        }
        self.home();
        done
    }

    /// Does `process` to the blocks `found`, in turn, until it has worked `times`.
    fn gather_from(&mut self, found: Vec<BlockPos>, process: &str, times: usize) -> usize {
        let mut done = 0;
        for (tried, p) in found.into_iter().enumerate() {
            if done >= times || tried > times * 4 + 8 {
                break;
            }
            // The fire is not left to die while gathering.
            if tried % 3 == 2 {
                self.tend_fire();
            }
            self.w.put_down_all();
            self.w.go_to_block(p);
            let site = self.w.mover.pos;
            for _ in 0..3 {
                if done >= times {
                    break;
                }
                let (ok, words) = self.w.act(process, AimAt::Block { pos: p, top: true });
                if !ok {
                    if words.contains("no more")
                        || words.contains("Not here")
                        || words.contains("Out of reach")
                    {
                        self.spent.insert((p, process.to_owned()));
                        break;
                    }
                    continue;
                }
                done += 1;
                if self.w.block(p).is_none_or(|b| b == "air" || b == "spoil") {
                    break;
                }
            }
            self.home();
            self.w.put_down_all();
            self.haul(site);
        }
        done
    }

    /// Empties the hands at camp and takes up, for each tool property, the best thing at camp
    /// with it (not one `avoid` names).
    fn take_tools(&mut self, props: &[(String, f32)], avoid: &dyn Fn(&str) -> bool) {
        self.home();
        self.w.put_down_all();
        let items = self.w.items.clone();
        for (prop, min) in props {
            let held = [&self.w.carry.right, &self.w.carry.left]
                .into_iter()
                .flatten()
                .any(|s| s.property(&items, prop).is_some_and(|v| v >= *min));
            if held {
                continue;
            }
            let best = self
                .w
                .lying
                .iter()
                .filter(|l| self.near_camp(l.pos) && l.work.is_none() && !avoid(&l.stack.id))
                .filter_map(|l| l.stack.property(&items, prop).map(|v| (v, l.id)))
                .filter(|(v, _)| *v >= *min)
                .max_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, id)) = best {
                self.w.hold(id);
            }
        }
    }

    /// Does a process at camp (or aimed at something there) with the tools it needs in hand.
    fn work(&mut self, process: &str, aim: AimAt) -> bool {
        self.work_avoiding(process, aim, &|_| false)
    }

    fn work_avoiding(&mut self, process: &str, aim: AimAt, avoid: &dyn Fn(&str) -> bool) -> bool {
        self.upkeep();
        let tools: Vec<(String, f32)> = self
            .w
            .content
            .processes
            .get(&id(process))
            .map(|p| {
                p.tools
                    .iter()
                    .map(|t| (t.property.clone(), t.min))
                    .collect()
            })
            .unwrap_or_default();
        self.take_tools(&tools, avoid);
        let (ok, words) = self.w.act(process, aim);
        if !ok {
            self.say(&format!("{process}: {words}"));
        }
        self.w.put_down_all();
        ok
    }

    /// Repeats a process until a node is known (or `max` tries).
    fn until_known(&mut self, node: &str, process: &str, aim: AimAt, max: usize) {
        for _ in 0..max {
            if self.knows(node) {
                break;
            }
            self.work(process, aim);
        }
        assert!(
            self.knows(node),
            "{node} not learned by {process}: learned {:?}",
            self.w.learned
        );
        self.say(&format!("knows {node}"));
    }

    /// Water: the nearest fresh water. Rivers and lakes are seen across the land (as on the
    /// horizon); the nearest are gone to, and their water, if it is there this season, open to
    /// the sky and within reach, is tasted.
    fn find_water(&mut self) -> Option<BlockPos> {
        if self.water.is_some() || self.looked_for_water {
            return self.water;
        }
        self.looked_for_water = true;
        self.say("looks for fresh water");
        let g = self.w.generator.clone();
        let (cx, cz) = (self.camp.x.floor() as i32, self.camp.z.floor() as i32);
        let reach = 1500;
        let mut seen: Vec<(i64, i32, i32)> = Vec::new();
        for gz in ((cz - reach) >> 4)..=((cz + reach) >> 4) {
            for gx in ((cx - reach) >> 4)..=((cx + reach) >> 4) {
                let (x0, z0) = (gx * 16, gz * 16);
                let (dx, dz) = ((x0 + 8 - cx) as i64, (z0 + 8 - cz) as i64);
                if dx * dx + dz * dz > (reach as i64).pow(2) {
                    continue;
                }
                let col = g.column(hearth_math::ColumnPos::new(gx, gz));
                for lz in (0..16).step_by(4) {
                    for lx in (0..16).step_by(4) {
                        let s = col.at(lx, lz);
                        if s.is_underwater() && !s.ocean && (s.river.is_some() || s.lake) {
                            let (x, z) = (x0 + lx as i32, z0 + lz as i32);
                            let d = ((x - cx) as i64).pow(2) + ((z - cz) as i64).pow(2);
                            seen.push((d, x, z));
                        }
                    }
                }
            }
        }
        seen.sort();
        self.say(&format!(
            "{} stretches of river and lake in sight",
            seen.len()
        ));
        let mut tried: Vec<(i32, i32)> = Vec::new();
        let mut salty: Vec<BlockPos> = Vec::new();
        for (_, x, z) in seen {
            if tried.len() >= 40 {
                break;
            }
            if tried
                .iter()
                .any(|(tx, tz)| (tx - x).pow(2) + (tz - z).pow(2) < 40 * 40)
            {
                continue;
            }
            tried.push((x, z));
            self.w.go(x as f64 + 0.5, z as f64 + 0.5);
            if let Some(p) = self.taste_water_here(&mut salty) {
                self.water = Some(p);
                self.say(&format!(
                    "found fresh water at {p:?}, {:.0} m from camp ({} places tried)",
                    ((p.x - cx) as f64).hypot((p.z - cz) as f64),
                    tried.len()
                ));
                self.home();
                return self.water;
            }
        }
        self.say(&format!(
            "found no fresh water ({} places tried)",
            tried.len()
        ));
        self.home();
        None
    }

    /// Tastes the water about here (open to the sky, within reach); the first fresh found.
    fn taste_water_here(&mut self, salty: &mut Vec<BlockPos>) -> Option<BlockPos> {
        for _ in 0..4 {
            let bad = self.bad_water.clone();
            let near = |list: &[BlockPos], p: &BlockPos, r: i32| {
                list.iter()
                    .any(|q| (q.x - p.x).pow(2) + (q.z - p.z).pow(2) < r * r)
            };
            // Water under the open sky (not a pool in a cave).
            let open = |w: &World, p: &BlockPos| {
                (1..=12).all(|dy| {
                    w.block(BlockPos::new(p.x, p.y + dy, p.z))
                        .is_some_and(|b| b == "air" || b == "water")
                })
            };
            let p = self
                .w
                .find(24, |n, _| n == "water")
                .into_iter()
                .filter(|p| !near(salty, p, 48) && !near(&bad, p, 16))
                .filter(|p| open(&self.w, p))
                .find(|p| self.w.stand_by(*p).is_some())?;
            self.w.go_to_block(p);
            let before = self.w.acted.len();
            self.w
                .server
                .send(ToServer::Drink(DrinkFrom::Water(AimAt::Block {
                    pos: p,
                    top: true,
                })));
            self.w.run(2);
            let words = self.w.acted[before..]
                .last()
                .map(|(_, _, words)| words.clone())
                .unwrap_or_default();
            if words.contains("salty") {
                salty.push(p);
            } else if words == "You drink." || words.contains("cannot drink more") {
                return Some(p);
            } else {
                // Out of reach, or not water after all.
                self.bad_water.push(p);
            }
        }
        None
    }

    /// The hour where the camp is, and how the body is.
    fn hour(&self) -> f32 {
        self.w.body.as_ref().map_or(12.0, |b| b.exposure.local_hour)
    }

    /// Pieces of `kind` (`stick/`, `pole/`...) lying at camp that burn.
    fn lying_fuel(&self, kind: &str) -> u32 {
        let items = &self.w.items;
        self.w
            .lying
            .iter()
            .filter(|l| {
                l.work.is_none()
                    && l.stack.id.contains(kind)
                    && items
                        .get(&l.stack.id)
                        .is_some_and(|k| k.has_tag("fuelwood"))
                    && self.near_camp(l.pos)
            })
            .map(|l| l.stack.count as u32)
            .sum()
    }

    /// Of `kind`, how many are kept back from the fire: three sticks and three poles (to drill
    /// fire, to make things).
    fn kept(kind: &str) -> u32 {
        match kind {
            "stick/" | "pole/" => 3,
            _ => 0,
        }
    }

    /// Sticks, poles and log sections at camp to spare for the fire (twigs burn too briefly
    /// to count).
    fn fuel(&self) -> u32 {
        ["stick/", "pole/", "log_section/"]
            .iter()
            .map(|k| self.lying_fuel(k).saturating_sub(Self::kept(k)))
            .sum()
    }

    /// Fuel for the fire: wood at camp, more gathered when it runs low.
    fn firewood(&mut self) {
        if self.fuel() < 8 {
            let wood = |_: &str, m: Option<&str>| m.is_some_and(|m| m.ends_with("_wood"));
            self.gather(40, wood, "break_deadwood", 6);
            self.gather(40, wood, "pull_dead_pole", 4);
            self.gather(40, |n, _| n == "dead_bush", "break_dead_bush", 3);
        }
    }

    /// Lays `n` pieces of wood on the fire: sticks first, and poles for a long burn (a few kept
    /// back for making things).
    fn feed(&mut self, n: usize) {
        let Some(hearth) = self.hearth else {
            return;
        };
        self.firewood();
        let items = self.w.items.clone();
        let burns = move |id: &str| items.get(id).is_some_and(|k| k.has_tag("fuelwood"));
        // Embers or a cold hearth take kindling first: a twig catches from a few embers.
        let kindle = matches!(self.fire_state().as_deref(), Some("embers") | Some("out"));
        for i in 0..n + kindle as usize {
            self.home();
            self.w.put_down_all();
            let long = n >= 3;
            // Embers want kindling (the sticks kept for making things too, rather than let the
            // fire go out), then whatever there is.
            let first = kindle && i == 0;
            let order: &[&str] = if first {
                &["twig/", "stick/", "pole/", "log_section/"]
            } else if long {
                &["pole/", "stick/", "log_section/", "twig/"]
            } else {
                &["stick/", "log_section/", "pole/", "twig/"]
            };
            let mut laid = false;
            for kind in order {
                let spare = if first { 0 } else { Self::kept(kind) };
                if self.lying_fuel(kind) > spare && self.take(|id| id.contains(kind) && burns(id)) {
                    laid = true;
                    break;
                }
            }
            if !laid {
                self.say("no wood to spare for the fire");
                break;
            }
            let (ok, words) = self.w.act(
                "feed_fire",
                AimAt::Block {
                    pos: hearth,
                    top: false,
                },
            );
            if !ok {
                self.say(&format!("feed_fire: {words}"));
                break;
            }
        }
        self.w.put_down_all();
        self.fed_at = self.w.ticks;
        if n >= 3 {
            self.say(&format!(
                "fed the fire {n}: {:?}, {} pieces left",
                self.fire_state(),
                self.fuel()
            ));
        }
    }

    /// Keeps the fire: relit when out, revived from embers, a piece laid on as it burns low.
    /// Whether it needed anything.
    fn tend_fire(&mut self) -> bool {
        if self.hearth.is_none() || self.relighting || self.tending {
            return false;
        }
        self.tending = true;
        let tended = self.tend_fire_now();
        self.tending = false;
        tended
    }

    fn tend_fire_now(&mut self) -> bool {
        let since_h = self.w.ticks.saturating_sub(self.fed_at) as f64 / self.w.ticks_per_day * 24.0;
        match self.fire_state().as_deref() {
            Some("out") => self.relight(),
            Some("embers") => self.feed(2),
            Some("low") if since_h > 0.4 => self.feed(1),
            _ => return false,
        }
        true
    }

    /// The fire out: wood laid on the cold hearth and lit again, by drilling once that is
    /// known (rubbing sticks together teaches it).
    fn relight(&mut self) {
        let Some(hearth) = self.hearth else {
            return;
        };
        self.say("the fire is out: lights it again");
        self.relighting = true;
        self.feed(2);
        self.relight_tries(hearth);
        self.relighting = false;
    }

    fn relight_tries(&mut self, hearth: BlockPos) {
        for _ in 0..16 {
            if matches!(self.fire_state().as_deref(), Some("low") | Some("high")) {
                self.say("the fire burns again");
                return;
            }
            if !self.knows("fire_by_friction_hand_drill") {
                // Fire carried from the burning tree, while it burns.
                if self
                    .wildfire
                    .is_none_or(|f| self.w.block(f).as_deref() != Some("flames"))
                {
                    self.wildfire = self.lightning();
                }
                if let Some(f) = self.wildfire
                    && self.firebrand(f)
                {
                    let (ok, words) = self.w.act(
                        "light_fire",
                        AimAt::Block {
                            pos: hearth,
                            top: false,
                        },
                    );
                    self.w.put_down_all();
                    if !ok {
                        self.say(&format!("light_fire: {words}"));
                    }
                    continue;
                }
                self.work("rub_sticks", AimAt::Nothing);
                if self.knows("fire_by_friction_hand_drill") {
                    self.say("knows fire by hand drill");
                }
                continue;
            }
            if self.count("handful/dry_grass") == 0 {
                self.gather(40, |n, _| n == "short_dry_grass", "pluck_dry_grass", 6);
            }
            if self.count("stick/") < 2 {
                let wood = |_: &str, m: Option<&str>| m.is_some_and(|m| m.ends_with("_wood"));
                self.gather(40, wood, "break_deadwood", 3);
            }
            self.w.put_down_all();
            self.take_tools(&[], &|_| false);
            let (ok, words) = self.w.act(
                "hand_drill_fire",
                AimAt::Block {
                    pos: hearth,
                    top: false,
                },
            );
            if !ok {
                self.say(&format!("hand_drill_fire: {words}"));
                if words.contains("needs") {
                    // Rain: wait it out.
                    self.w.wait_hours(1.0);
                }
            }
        }
        self.say(&format!("the fire is still {:?}", self.fire_state()));
    }

    /// Whether the hearth burns.
    fn fire_state(&self) -> Option<String> {
        let h = self.hearth?;
        self.w
            .mirror
            .block(h)
            .and_then(|s| self.w.reg.get(s, "fire").map(str::to_owned))
    }

    /// A line on the night: the fire, the air, the warmth reaching the body and the body.
    fn night_note(&self) {
        let Some(b) = self.w.body.as_ref() else {
            return;
        };
        self.say(&format!(
            "  night {:.0} h at {:.1} m from the hearth: hearth {:?}, fire {:?}, air {:.0} °C, \
             wind {:.1} m/s, radiant {:.0} W/m², bed {:.1} clo, core {:.1} °C, skin {:.1} °C, \
             asleep {}, fuel {}",
            b.exposure.local_hour,
            self.hearth.map_or(-1.0, |h| (self.w.mover.pos
                - DVec3::new(h.x as f64 + 0.5, h.y as f64, h.z as f64 + 0.5))
            .length()),
            self.hearth.and_then(|h| self.w.block(h)),
            self.fire_state(),
            b.exposure.air_c,
            b.exposure.wind_m_s,
            b.exposure.radiant_w_m2,
            b.exposure.ground_clo,
            b.status.core_c,
            b.status.skin_c,
            b.asleep,
            self.fuel()
        ));
    }

    fn core(&self) -> f32 {
        self.w.body.as_ref().map_or(37.0, |b| b.status.core_c)
    }

    /// Chilled: warms by the fire until the core is warm again.
    fn warm_up(&mut self) {
        if self.core() >= 36.0 || self.hearth.is_none() {
            return;
        }
        self.say(&format!(
            "chilled (core {:.1} °C): warms by the fire",
            self.core()
        ));
        for _ in 0..12 {
            if self.core() >= 36.6 {
                break;
            }
            if !matches!(self.fire_state().as_deref(), Some("high")) {
                self.feed(2);
            }
            self.home();
            self.w.wait_hours(0.25);
        }
    }

    fn thirsty(&self) -> bool {
        use hearth_body::Thirst;
        self.w.body.as_ref().is_some_and(|b| {
            matches!(
                b.status.thirst,
                Thirst::Thirsty | Thirst::VeryThirsty | Thirst::Parched | Thirst::Dying
            )
        })
    }

    /// Goes to the water and drinks when thirsty (or `anyway`): mouthfuls until the stomach
    /// will take no more, and again as it empties, until the thirst is gone.
    fn drink_up(&mut self, anyway: bool) {
        if !anyway && !self.thirsty() {
            return;
        }
        let Some(p) = self.find_water() else {
            return;
        };
        self.w.put_down_all();
        self.w.go_to_block(p);
        let before = self.w.acted.len();
        for round in 0..4 {
            if round > 0 {
                if !self.thirsty() {
                    break;
                }
                // The stomach empties a drink in some twenty minutes.
                self.w.wait_hours(0.3);
            }
            for _ in 0..12 {
                let n = self.w.acted.len();
                self.w
                    .server
                    .send(ToServer::Drink(DrinkFrom::Water(AimAt::Block {
                        pos: p,
                        top: true,
                    })));
                self.w.run(2);
                if !self.w.acted[n..]
                    .iter()
                    .any(|(p, ok, _)| p == "drink" && *ok)
                {
                    break;
                }
            }
        }
        let drunk = self.w.acted[before..]
            .iter()
            .filter(|(p, ok, _)| p == "drink" && *ok)
            .count();
        // Not water any more (nothing said), or out of reach: looked for again.
        let gone = self.w.acted[before..].iter().all(|(p, _, _)| p != "drink");
        let unreachable = self.w.acted[before..]
            .iter()
            .any(|(p, _, words)| p == "drink" && words.contains("out of reach"));
        if unreachable {
            self.bad_water.push(p);
        }
        if gone || unreachable {
            let around: Vec<String> = (-1..=1)
                .map(|dy| format!("{:?}", self.w.block(BlockPos::new(p.x, p.y + dy, p.z))))
                .collect();
            self.say(&format!(
                "the water at {p:?} is gone ({around:?}): looks again"
            ));
            self.water = None;
            self.looked_for_water = false;
        } else if !anyway || drunk < 4 {
            self.say(&format!(
                "drinks {drunk} mouthfuls ({:?})",
                self.w.body.as_ref().map(|b| b.status.thirst)
            ));
        }
        self.home();
    }

    /// Drinks, eats, keeps the fire and sleeps when it is time.
    fn upkeep(&mut self) {
        self.tend_fire();
        self.warm_up();
        self.drink_up(false);
        self.eat_up();
        let night = !(6.0..19.5).contains(&self.hour());
        if self.w.ticks < self.upkeep_at && !night {
            return;
        }
        self.upkeep_at = self.w.ticks + (self.w.ticks_per_day / 3.0) as u64;
        self.drink_up(true);
        self.home();
        let food = self.at_camp(|id| {
            id.ends_with("cooked_meat") || id.ends_with("nut_kernel") || id.ends_with("blackberry")
        });
        for id in food.into_iter().take(3) {
            self.w.put_down_all();
            if self.w.pick_up(id) {
                self.w
                    .server
                    .send(ToServer::Eat(Path::at(Root::Hand(Hand::Right))));
                self.w.run(2);
            }
        }
        self.w.put_down_all();
        // At night, or weary: the fire well fed, sleep beside it on the bed.
        let weary = self.w.body.as_ref().is_some_and(|b| {
            matches!(
                b.status.tiredness,
                hearth_body::Tiredness::Tired | hearth_body::Tiredness::Exhausted
            )
        });
        if (night || weary) && self.hearth.is_some() {
            self.feed(3);
            // On the bed beside the hearth.
            if let Some(bed) = self.bed
                && self.w.block(bed).as_deref() == Some("grass_bed")
            {
                self.w.go_exact(DVec3::new(
                    bed.x as f64 + 0.5,
                    bed.y as f64 + 0.2,
                    bed.z as f64 + 0.5,
                ));
            } else {
                self.home();
            }
            self.say(&format!(
                "sleeps by the fire ({:.0} h, air {:.0} °C, core {:.1} °C)",
                self.hour(),
                self.w.body.as_ref().map_or(0.0, |b| b.exposure.air_c),
                self.w.body.as_ref().map_or(0.0, |b| b.status.core_c)
            ));
            self.w.server.send(ToServer::Sleep(true));
            for k in 0..120 {
                self.w.run(400);
                if k % 5 == 4 {
                    self.night_note();
                }
                let woke = self.w.body.as_ref().is_some_and(|b| !b.asleep && k > 2);
                // Up in the night to tend the fire.
                let since_h =
                    self.w.ticks.saturating_sub(self.fed_at) as f64 / self.w.ticks_per_day * 24.0;
                let wants = match self.fire_state().as_deref() {
                    Some("out") | Some("embers") => true,
                    Some("low") => since_h > 0.4,
                    _ => false,
                };
                if wants {
                    let lay = self.w.mover.pos;
                    self.w.server.send(ToServer::Sleep(false));
                    self.w.run(2);
                    self.tend_fire();
                    self.w.go_exact(lay);
                    self.w.server.send(ToServer::Sleep(true));
                    self.w.run(2);
                }
                if woke && (6.0..19.5).contains(&self.hour()) {
                    break;
                }
            }
            self.w.server.send(ToServer::Sleep(false));
            self.w.run(2);
            self.say(&format!(
                "wakes ({:.0} h, core {:.1} °C)",
                self.hour(),
                self.w.body.as_ref().map_or(0.0, |b| b.status.core_c)
            ));
        }
        assert!(
            self.w.body.as_ref().is_none_or(|b| b.dead.is_none()),
            "died: {:?}",
            self.w.body.as_ref().and_then(|b| b.dead.clone())
        );
    }

    /// Keeps at least `n` knappable cobbles at camp.
    fn cobbles(&mut self, n: u32, knappable: &dyn Fn(&str) -> bool) {
        for _ in 0..6 {
            let have: u32 = self
                .w
                .lying
                .iter()
                .filter(|l| self.near_camp(l.pos) && knappable(&l.stack.id))
                .map(|l| l.stack.count as u32)
                .sum();
            if have >= n {
                return;
            }
            let content = self.w.content.clone();
            let want = ((n - have) as usize).div_ceil(2);
            self.gather(
                48,
                move |name, _| {
                    name.strip_suffix("_cobbles").is_some_and(|rock| {
                        content
                            .materials
                            .get(&format!("hearth:{rock}"))
                            .is_some_and(|m| m.tags.iter().any(|t| t == "knappable"))
                    })
                },
                "gather_stones",
                want,
            );
        }
    }

    /// Resin from conifers seen across the land (dark spruce and pine stands, as on the
    /// horizon): the nearest are gone to and their trunks tapped.
    fn resin_afar(&mut self) {
        let g = self.w.generator.clone();
        let (cx, cz) = (self.camp.x.floor() as i32, self.camp.z.floor() as i32);
        let planet = g.planet();
        let mut seen: Vec<(i64, i32, i32)> = Vec::new();
        for r in (64..1500).step_by(48) {
            let n = (r as f64 * std::f64::consts::TAU / 48.0).ceil() as i32;
            for k in 0..n {
                let a = k as f64 * std::f64::consts::TAU / n as f64;
                let (x, z) = (
                    cx + (a.cos() * r as f64) as i32,
                    cz + (a.sin() * r as f64) as i32,
                );
                let s = g.terrain.sample(planet.wrap_x(x), z);
                if s.is_underwater() || s.tree_density < 0.3 {
                    continue;
                }
                let Some((sp, _, _)) =
                    g.features()
                        .expected_canopy(&g, &Default::default(), &s, x, z, 0.5)
                else {
                    continue;
                };
                let id = &g.forest.templates.species[sp].id;
                if id.ends_with("spruce") || id.ends_with("pine") {
                    seen.push(((r as i64).pow(2), x, z));
                }
            }
            if seen.len() >= 6 {
                break;
            }
        }
        self.say(&format!("{} conifer stands in sight", seen.len()));
        for (_, x, z) in seen.into_iter().take(6) {
            self.w.go(x as f64 + 0.5, z as f64 + 0.5);
            let trunks: Vec<BlockPos> = self
                .w
                .find(44, |_, m| {
                    m.is_some_and(|m| m.ends_with("spruce_wood") || m.ends_with("pine_wood"))
                })
                .into_iter()
                .filter(|p| self.w.stand_by(*p).is_some())
                .take(6)
                .collect();
            for p in trunks {
                self.w.go_to_block(p);
                let (ok, _) = self
                    .w
                    .act("collect_resin", AimAt::Block { pos: p, top: true });
                if ok {
                    // Carried home.
                    self.home();
                    self.w.put_down_all();
                    return;
                }
            }
        }
        self.home();
    }

    /// Lightning at a tree 12–30 m from camp (a natural event the test may force): where the
    /// flames are.
    fn lightning(&mut self) -> Option<BlockPos> {
        self.home();
        let camp = BlockPos::containing(self.camp);
        let tree = self
            .w
            .find(30, |n, _| n.ends_with("_log"))
            .into_iter()
            .find(|p| {
                let (dx, dz) = ((p.x - camp.x) as f64, (p.z - camp.z) as f64);
                (12.0..30.0).contains(&(dx * dx + dz * dz).sqrt())
            })?;
        self.w.server.send(ToServer::Strike {
            x: tree.x,
            z: tree.z,
        });
        self.w.run(2);
        let fire = (0..20).find_map(|_| {
            self.w.run(2);
            self.w.find(40, |n, _| n == "flames").into_iter().next()
        });
        if let Some(f) = fire {
            self.say(&format!("lightning: fire at {f:?}"));
        }
        fire
    }

    /// Eats when hungry: what is cooked or ready at camp, and meat charred on the fire when
    /// there is nothing else.
    fn eat_up(&mut self) {
        use hearth_body::Hunger;
        let hungry = |b: &Bot| {
            b.w.body.as_ref().is_some_and(|b| {
                matches!(
                    b.status.hunger,
                    Hunger::Hungry | Hunger::VeryHungry | Hunger::Starving
                )
            })
        };
        let ready = |id: &str| {
            id.ends_with("cooked_meat")
                || id.ends_with("dried_meat")
                || id.ends_with("nut_kernel")
                || id.ends_with("blackberry")
                || id.ends_with("raspberry")
                || id.ends_with("wild_strawberry")
        };
        for _ in 0..8 {
            if !hungry(self) {
                break;
            }
            self.home();
            self.w.put_down_all();
            let food = self.at_camp(ready).into_iter().next();
            match food {
                Some(id) => {
                    if self.w.pick_up(id) {
                        self.w
                            .server
                            .send(ToServer::Eat(Path::at(Root::Hand(Hand::Right))));
                        self.w.run(2);
                    }
                }
                None => {
                    let burning =
                        matches!(self.fire_state().as_deref(), Some("low") | Some("high"));
                    if self.count("cut/meat") == 0 || !burning {
                        break;
                    }
                    let Some(hearth) = self.hearth else {
                        break;
                    };
                    if !self
                        .w
                        .act(
                            "char_meat",
                            AimAt::Block {
                                pos: hearth,
                                top: false,
                            },
                        )
                        .0
                    {
                        break;
                    }
                }
            }
        }
        self.w.put_down_all();
    }

    /// Brings a fresh kill lying in the world to camp (dragged).
    fn fetch_carcass(&mut self) -> bool {
        for _ in 0..40 {
            if let Some((cid, pos)) = self.look_for_carcass() {
                self.w.put_down_all();
                self.w.go_exact(pos + DVec3::new(1.0, 0.0, 0.0));
                self.w.server.send(ToServer::Drag(cid));
                self.w.run(2);
                self.home();
                self.w.let_go();
                return true;
            }
            // None yet: the day goes on at camp.
            self.pass_time(4.0);
        }
        false
    }

    /// A fresh kill lying about, looked for from camp and from points 70 m out.
    fn look_for_carcass(&mut self) -> Option<(u64, DVec3)> {
        let mut spots = vec![(0.0, 0.0)];
        for k in 0..4 {
            let a = k as f64 * std::f64::consts::FRAC_PI_2;
            spots.push((a.cos() * 70.0, a.sin() * 70.0));
        }
        for (dx, dz) in spots {
            self.w.go(self.camp.x + dx, self.camp.z + dz);
            self.w.run(2);
            let found = self
                .w
                .lying
                .iter()
                .filter(|l| l.stack.id.ends_with("small_carcass") && !self.near_camp(l.pos))
                .filter(|l| l.stack.decay < 0.5)
                .map(|l| (l.id, DVec3::from_array(l.pos)))
                .next();
            if found.is_some() {
                return found;
            }
        }
        self.home();
        None
    }

    /// Lets `hours` pass at camp, keeping up as they go (the fire, warmth, water and sleep).
    fn pass_time(&mut self, hours: f64) {
        let end = self.w.ticks + (hours / 24.0 * self.w.ticks_per_day) as u64;
        while self.w.ticks < end {
            self.home();
            self.w.wait_hours(0.5);
            self.upkeep();
        }
    }

    /// Puts on a garment lying at camp (by part of its id); whether it is worn.
    fn put_on(&mut self, part: &str) -> bool {
        self.home();
        self.w.put_down_all();
        if !self.take(|id| id.contains(part)) {
            return false;
        }
        let hand = if self
            .w
            .carry
            .right
            .as_ref()
            .is_some_and(|s| s.id.contains(part))
        {
            Hand::Right
        } else {
            Hand::Left
        };
        self.w.server.send(ToServer::Shift {
            from: Path::at(Root::Hand(hand)),
            count: None,
            to: Target::Root(Root::Worn(0)),
        });
        self.w.run(2);
        let worn = self.w.carry.worn.iter().any(|w| w.stack.id.contains(part));
        self.say(&format!("puts on the {part}: {worn}"));
        self.w.put_down_all();
        worn
    }

    /// A kill fetched to camp and butchered.
    /// A kill fetched and butchered, its hide scraped while it is fresh.
    fn hide_from_a_kill(&mut self) {
        self.butcher_one();
        if self.count("sheet/rawhide") > 0 {
            let scrape = if self.knows("hide_scraping") {
                "scrape_hide"
            } else {
                "scrape_hide_crudely"
            };
            self.work(scrape, AimAt::Nothing);
        }
    }

    fn butcher_one(&mut self) {
        if !self.fetch_carcass() {
            self.say("no kill turned up");
            return;
        }
        if let Some(c) = self.carcass_here() {
            self.work("butcher_small_game", AimAt::Thing(c));
        }
    }

    /// A carcass at camp, aimed at.
    fn carcass_here(&self) -> Option<u64> {
        self.at_camp(|id| id.ends_with("small_carcass"))
            .into_iter()
            .next()
    }

    /// A place on the ground beside the camp to lay something out (its ground block).
    fn site(&self, dx: i32, dz: i32) -> BlockPos {
        let c = BlockPos::containing(self.camp);
        // From the height of the camp's feet down: under trees, not the leaves above.
        let col = DVec3::new(
            (c.x + dx) as f64 + 0.5,
            self.camp.y + 0.5,
            (c.z + dz) as f64 + 0.5,
        );
        self.w.ground_under(col).expect("ground beside camp")
    }

    /// Takes a thing at camp into a free hand.
    fn take(&mut self, pred: impl Fn(&str) -> bool) -> bool {
        match self.at_camp(pred).into_iter().next() {
            Some(id) => self.w.hold(id),
            None => false,
        }
    }

    /// Carries a burning stick from the natural fire at `fire` home (with a stick to burn).
    fn firebrand(&mut self, fire: BlockPos) -> bool {
        self.home();
        self.w.put_down_all();
        if !self.take(|id| id.contains("stick/")) {
            return false;
        }
        self.w.go_to_block(fire);
        let (ok, words) = self.w.act(
            "take_firebrand",
            AimAt::Block {
                pos: fire,
                top: false,
            },
        );
        if !ok {
            self.say(&format!("take_firebrand: {words}"));
        }
        self.home();
        ok
    }
}

#[test]
fn from_nothing_to_fire_spear_clothing_and_dried_meat_by_discovery() {
    let dir = temp("acceptance-v2-5");
    let w = World::start(&dir, hearth_save::KnowledgeMode::Discovery, 7);
    // A camp in the lee of trees near where the world began, with room for a hearth and a bed.
    let start = w.mover.pos;
    let camp = {
        let shelter = |p: BlockPos| -> usize {
            let mut n = 0;
            for dz in -3..=3 {
                for dx in -3..=3 {
                    for dy in 1..=4 {
                        let q = BlockPos::new(p.x + dx, p.y + dy, p.z + dz);
                        if w.block(q)
                            .is_some_and(|b| b.ends_with("_leaves") || b.ends_with("_log"))
                        {
                            n += 1;
                        }
                    }
                }
            }
            n
        };
        let open = |p: BlockPos| {
            w.solid(p)
                && !w.solid(p.up())
                && !w.solid(p.up().up())
                && w.block(p.up()).is_none_or(|b| !b.ends_with("_log"))
        };
        let mut best: Option<(usize, BlockPos)> = None;
        let c = BlockPos::containing(start);
        for dz in -20..=20 {
            for dx in -20..=20 {
                for dy in -4..=4 {
                    let g = BlockPos::new(c.x + dx, c.y + dy, c.z + dz);
                    let room = open(g)
                        && open(BlockPos::new(g.x + 2, g.y, g.z))
                        && open(BlockPos::new(g.x + 1, g.y, g.z + 1))
                        && open(BlockPos::new(g.x - 2, g.y, g.z));
                    if room {
                        let sh = shelter(g);
                        if best.is_none_or(|b| sh > b.0) {
                            best = Some((sh, g));
                        }
                    }
                }
            }
        }
        match best {
            Some((_, g)) => DVec3::new(g.x as f64 + 0.5, g.y as f64 + 1.0, g.z as f64 + 0.5),
            None => start,
        }
    };
    let mut bot = Bot {
        w,
        camp,
        upkeep_at: 0,
        water: None,
        looked_for_water: false,
        hearth: None,
        spent: Default::default(),
        bad_water: Vec::new(),
        fed_at: 0,
        relighting: false,
        bed: None,
        wildfire: None,
        tending: false,
    };
    let content = bot.w.content.clone();
    let items = bot.w.items.clone();
    let knappable = move |sid: &str| {
        items
            .get(sid)
            .and_then(|k| k.material.as_deref())
            .and_then(|m| content.materials.get(m))
            .is_some_and(|m| m.tags.iter().any(|t| t == "knappable"))
            && sid.contains("cobble/")
    };
    let wood = |_: &str, m: Option<&str>| m.is_some_and(|m| m.ends_with("_wood"));
    bot.home();
    bot.say(&format!(
        "Start: nothing known, a loincloth; camp at {:?}",
        bot.camp
    ));
    assert!(bot.w.knowledge.known.is_empty());

    // ---- Stone: knocking stones teaches what stone does. ----
    bot.cobbles(10, &knappable);
    bot.gather(40, |n, _| n.ends_with("_cobbles"), "pry_flat_stone", 1);
    for _ in 0..16 {
        if bot.knows("stone_as_hammer") && bot.knows("sharp_flake") {
            break;
        }
        bot.work("knock_stones", AimAt::Nothing);
    }
    assert!(
        bot.knows("stone_as_hammer") && bot.knows("sharp_flake"),
        "{:?}",
        bot.w.learned
    );
    bot.say("knows stone as a hammer, sharp flakes");

    // ---- Wood and grass, before the evening. ----
    bot.gather(40, wood, "break_deadwood", 4);
    bot.gather(40, wood, "pull_dead_pole", 8);
    bot.gather(40, |n, _| n == "short_dry_grass", "pluck_dry_grass", 12);
    bot.gather(40, |n, _| n == "tall_dry_grass", "pluck_tall_dry_grass", 6);
    bot.gather(40, |n, _| n == "tall_grass", "pull_dead_grass", 6);
    bot.say(&format!(
        "{} handfuls of dry grass",
        bot.count("handful/dry_grass")
    ));

    // ---- Fire: lightning sets a tree burning; a burning stick carried off teaches keeping fire.
    // A tree a little way from camp (a burning tree is no neighbour to sleep by).
    let camp_block = BlockPos::containing(bot.camp);
    let tree = bot
        .w
        .find(30, |n, _| n.ends_with("_log"))
        .into_iter()
        .find(|p| {
            let (dx, dz) = ((p.x - camp_block.x) as f64, (p.z - camp_block.z) as f64);
            (12.0..30.0).contains(&(dx * dx + dz * dz).sqrt())
        })
        .expect("a tree near camp");
    bot.w.server.send(ToServer::Strike {
        x: tree.x,
        z: tree.z,
    });
    bot.w.run(2);
    let fire = (0..20)
        .find_map(|_| {
            bot.w.run(2);
            bot.w.find(40, |n, _| n == "flames").into_iter().next()
        })
        .expect("the struck tree burns");
    bot.say(&format!("lightning: fire at {fire:?}"));
    bot.wildfire = Some(fire);
    for _ in 0..6 {
        if bot.knows("fire_keeping") {
            break;
        }
        bot.firebrand(fire);
        bot.w.put_down_all();
    }
    assert!(bot.knows("fire_keeping"), "{:?}", bot.w.learned);
    bot.say("knows keeping fire");
    // Lay a fire at camp and light it with a burning stick from the tree.
    let ground = bot.site(2, 0);
    let hearth = ground.up();
    assert!(
        bot.work(
            "build_campfire",
            AimAt::Block {
                pos: ground,
                top: true
            }
        ),
        "laid a fire"
    );
    let rain = |bot: &Bot| bot.w.body.as_ref().map_or(0.0, |b| b.exposure.rain_mm_h);
    // A small new fire drowns in rain: wait for a dry spell.
    for _ in 0..24 {
        if rain(&bot) <= 1.5 {
            break;
        }
        bot.say(&format!("rain ({:.1} mm/h): waits", rain(&bot)));
        bot.w.wait_hours(1.0);
    }
    let mut lit = false;
    for _ in 0..6 {
        if bot.firebrand(fire) {
            let (ok, words) = bot.w.act(
                "light_fire",
                AimAt::Block {
                    pos: hearth,
                    top: false,
                },
            );
            bot.w.put_down_all();
            if ok {
                lit = true;
                break;
            }
            bot.say(&format!("light_fire: {words}"));
        }
    }
    assert!(lit, "the campfire is lit");
    bot.hearth = Some(hearth);
    bot.w.run(200);
    let burning = bot.fire_state();
    bot.say(&format!(
        "the new fire: {burning:?} (rain {:.1} mm/h, wind {:.1} m/s)",
        rain(&bot),
        bot.w.body.as_ref().map_or(0.0, |b| b.exposure.wind_m_s)
    ));
    bot.say(&format!("GOAL fire: the campfire burns ({burning:?})"));
    assert!(matches!(burning.as_deref(), Some("low") | Some("high")));
    bot.feed(2);
    // A bed of dry grass beside it (anyone can heap one up).
    let bed = bot.site(1, 1);
    if !bot.work(
        "build_grass_bed",
        AimAt::Block {
            pos: bed,
            top: true,
        },
    ) {
        bot.gather(48, |n, _| n == "short_dry_grass", "pluck_dry_grass", 20);
        bot.work(
            "build_grass_bed",
            AimAt::Block {
                pos: bed,
                top: true,
            },
        );
    }
    bot.bed = Some(bed.up());
    bot.say(&format!(
        "a bed beside the fire: {:?}",
        bot.w.block(bed.up())
    ));

    // ---- Knapping onward. ----
    for _ in 0..3 {
        bot.work("test_nodule", AimAt::Nothing);
    }
    bot.until_known("chopper", "strike_flake", AimAt::Nothing, 10);
    bot.until_known("flake_cutting", "whittle_wood", AimAt::Nothing, 10);
    bot.until_known("wooden_spear", "whittle_wood", AimAt::Nothing, 10);
    bot.cobbles(8, &knappable);
    bot.until_known("hand_axe", "make_chopper", AimAt::Nothing, 10);
    for _ in 0..14 {
        if bot.knows("prepared_core") {
            break;
        }
        if bot.count("core/flint") == 0 {
            bot.cobbles(4, &knappable);
            for _ in 0..3 {
                bot.work("test_nodule", AimAt::Nothing);
            }
        }
        bot.work("make_hand_axe", AimAt::Nothing);
    }
    assert!(bot.knows("prepared_core"), "{:?}", bot.w.learned);
    bot.say("knows prepared cores");
    for _ in 0..12 {
        if bot.knows("blade_technology") && bot.knows("hafting_binding") {
            break;
        }
        if bot.count("core/flint") == 0 {
            bot.cobbles(4, &knappable);
            for _ in 0..3 {
                bot.work("test_nodule", AimAt::Nothing);
            }
        }
        bot.work("knap_point", AimAt::Nothing);
    }
    assert!(bot.knows("blade_technology"), "{:?}", bot.w.learned);
    assert!(bot.knows("hafting_binding"), "{:?}", bot.w.learned);
    bot.say("knows blades and hafting");
    let feed = |bot: &mut Bot| bot.feed(2);

    // ---- Carcasses: hacking at them teaches butchery; then butchered properly. ----
    for _ in 0..4 {
        if bot.knows("butchery") {
            break;
        }
        assert!(bot.fetch_carcass(), "a kill turns up");
        let c = bot.carcass_here().expect("carcass at camp");
        bot.work("hack_at_carcass", AimAt::Thing(c));
    }
    assert!(bot.knows("butchery"), "{:?}", bot.w.learned);
    bot.say("knows butchery");
    // A first kill butchered, its hide scraped while it is fresh (raw hides keep only a few
    // days): crudely at first, which teaches scraping. More kills are fetched while the meat
    // dries and the clothes are sewn.
    for _ in 0..1 {
        assert!(bot.fetch_carcass(), "a kill turns up");
        let c = bot.carcass_here().expect("carcass at camp");
        bot.work("butcher_small_game", AimAt::Thing(c));
        feed(&mut bot);
        if bot.count("sheet/rawhide") > 0 {
            let scrape = if bot.knows("hide_scraping") {
                "scrape_hide"
            } else {
                "scrape_hide_crudely"
            };
            bot.work(scrape, AimAt::Nothing);
        }
    }
    bot.say(&format!(
        "{} hides ({} scraped), {} meat, {} bone, {} sinew",
        bot.count("sheet/rawhide"),
        bot.count("sheet/scraped_hide"),
        bot.count("cut/meat"),
        bot.count("piece/bone"),
        bot.count("hank/sinew")
    ));
    // Meat held in the flames teaches roasting; cooked meat to eat.
    for _ in 0..3 {
        bot.work(
            "char_meat",
            AimAt::Block {
                pos: hearth,
                top: false,
            },
        );
    }

    // ---- Hides: scraping teaches scrapers and hide work; a hide worn, wraps. ----
    for _ in 0..6 {
        if bot.knows("hide_scraping") {
            break;
        }
        if bot.count("sheet/rawhide") == 0 {
            bot.butcher_one();
        }
        bot.work("scrape_hide_crudely", AimAt::Nothing);
    }
    assert!(
        bot.knows("scraper") && bot.knows("hide_scraping"),
        "{:?}",
        bot.w.learned
    );
    bot.until_known("hide_wrap_clothing", "drape_hide", AimAt::Nothing, 6);
    // Raw hides keep only a few days; scraped, they keep for weeks until there is a needle.
    for _ in 0..6 {
        if bot.count("sheet/rawhide") == 0 || bot.count("sheet/scraped_hide") >= 3 {
            break;
        }
        bot.work("scrape_hide", AimAt::Nothing);
    }

    // ---- Cord: rolling sinew fibres teaches cordage. ----
    bot.until_known("cordage_basic", "roll_fibres", AimAt::Nothing, 8);
    for _ in 0..3 {
        bot.work("twist_sinew", AimAt::Nothing);
    }

    // ---- Drying: meat hung in the smoke teaches drying; a rack, and three dry days. ----
    // Fresh meat (what lay at camp from the first kill may have gone off).
    if bot.count("cut/meat") < 3 {
        bot.hide_from_a_kill();
    }
    for _ in 0..6 {
        if bot.knows("drying_and_smoking") {
            break;
        }
        feed(&mut bot);
        bot.work(
            "hang_meat",
            AimAt::Block {
                pos: hearth,
                top: false,
            },
        );
    }
    assert!(bot.knows("drying_and_smoking"), "{:?}", bot.w.learned);
    // Cord for the rack's lashings (what the clothes left over may not be enough).
    for _ in 0..8 {
        if bot.count("cord/") >= 4 {
            break;
        }
        if bot.count("hank/sinew") == 0 {
            bot.butcher_one();
        }
        bot.work("twist_sinew", AimAt::Nothing);
    }
    let rack_ground = bot.site(-2, 0);
    let rack = rack_ground.up();
    assert!(
        bot.work(
            "build_drying_rack",
            AimAt::Block {
                pos: rack_ground,
                top: true
            }
        ),
        "a drying rack"
    );
    let mut hung = false;
    for _ in 0..4 {
        if bot.work(
            "dry_meat",
            AimAt::Block {
                pos: rack,
                top: false,
            },
        ) {
            hung = true;
            break;
        }
        // Rain: wait for it to pass.
        bot.pass_time(2.0);
    }
    assert!(hung, "meat hung to dry");
    bot.say("meat hung on the rack to dry");
    for _ in 0..30 {
        if bot.count("cut/dried_meat") > 0
            || bot
                .w
                .lying
                .iter()
                .any(|l| l.stack.id.ends_with("dried_meat"))
        {
            break;
        }
        // While it dries: kills for their hides (the clothes want five or six).
        if bot.count("sheet/scraped_hide") + bot.count("sheet/rawhide") < 6 {
            bot.hide_from_a_kill();
        } else {
            bot.pass_time(6.0);
        }
    }
    assert!(
        bot.w
            .lying
            .iter()
            .any(|l| l.stack.id.ends_with("dried_meat")),
        "dried meat on the rack"
    );
    bot.say("GOAL dried meat");

    // ---- Bone: scratching bone teaches burins and bone work; awls; needles. ----
    bot.until_known("burin", "scratch_bone", AimAt::Nothing, 6);
    bot.until_known("bone_antler_working", "scratch_bone", AimAt::Nothing, 6);
    // A blade for the burin (a nodule may shatter, a blade may snap: try again).
    for _ in 0..8 {
        if bot.count("burin/flint") > 0 {
            break;
        }
        if bot.count("blade/flint") == 0 {
            if bot.count("core/flint") == 0 {
                bot.cobbles(2, &knappable);
                bot.work("test_nodule", AimAt::Nothing);
            }
            bot.work("knap_blades", AimAt::Nothing);
        }
        bot.work("make_burin", AimAt::Nothing);
    }
    // A hide to pierce (those of the last kills may have torn or gone off).
    if bot.count("sheet/rawhide") + bot.count("sheet/scraped_hide") == 0 {
        bot.butcher_one();
    }
    bot.until_known("awl", "pierce_hide", AimAt::Nothing, 6);
    for _ in 0..4 {
        if bot.knows("eyed_needle") {
            break;
        }
        bot.work("make_awl", AimAt::Nothing);
    }
    assert!(bot.knows("eyed_needle"), "{:?}", bot.w.learned);
    for _ in 0..12 {
        if bot.count("needle/bone") > 0 {
            break;
        }
        bot.work("make_needle", AimAt::Nothing);
    }
    assert!(bot.count("needle/bone") > 0, "a needle");
    bot.say("a bone needle");

    // ---- Sewing: stitching hides together teaches sewn clothing; moccasins. ----
    for _ in 0..10 {
        if bot.count("sheet/scraped_hide") >= 3 {
            break;
        }
        if bot.count("sheet/rawhide") == 0 {
            bot.butcher_one();
        }
        bot.work("scrape_hide", AimAt::Nothing);
    }
    bot.say(&format!(
        "{} scraped hides",
        bot.count("sheet/scraped_hide")
    ));
    if bot.count("cord/sinew") < 2 {
        for _ in 0..3 {
            bot.work("twist_sinew", AimAt::Nothing);
        }
    }
    for _ in 0..8 {
        if bot.knows("sewn_clothing") {
            break;
        }
        bot.work("stitch_hides", AimAt::Nothing);
    }
    assert!(bot.knows("sewn_clothing"), "{:?}", bot.w.learned);
    let mut sewn = false;
    for _ in 0..6 {
        if bot.work("sew_moccasins", AimAt::Nothing) {
            sewn = true;
            break;
        }
    }
    assert!(sewn && bot.count("moccasins/scraped_hide") > 0, "moccasins");
    bot.say("GOAL clothing: sewn moccasins");
    bot.put_on("moccasins");
    // Autumn nights are cold: a cape of two hides.
    for _ in 0..6 {
        if bot.count("sheet/scraped_hide") >= 2 {
            break;
        }
        if bot.count("sheet/rawhide") == 0 {
            bot.butcher_one();
        }
        bot.work("scrape_hide", AimAt::Nothing);
    }
    if bot.work("make_hide_cape", AimAt::Nothing) {
        bot.put_on("hide_cape");
    }

    // Clothes first, against the autumn nights; then the spear.
    // ---- Spear: a pole whittled, a point knapped, bound and glued. ----
    bot.work("whittle_spear", AimAt::Nothing);
    if !bot.knows("stone_tipped_spear") {
        bot.work("whittle_spear", AimAt::Nothing);
    }
    assert!(bot.knows("stone_tipped_spear"), "{:?}", bot.w.learned);
    // Resin from the spruces and pines.
    let resinous = |_: &str, m: Option<&str>| {
        m.is_some_and(|m| m.ends_with("spruce_wood") || m.ends_with("pine_wood"))
    };
    for _ in 0..3 {
        if bot.count("pine_resin") > 0 {
            break;
        }
        bot.gather(48, resinous, "collect_resin", 2);
    }
    if bot.count("pine_resin") == 0 {
        bot.resin_afar();
    }
    bot.say(&format!("{} resin", bot.count("pine_resin")));
    if bot.count("point/flint") == 0 {
        bot.cobbles(2, &knappable);
        bot.work("test_nodule", AimAt::Nothing);
        bot.work("knap_point", AimAt::Nothing);
    }
    let mut spear = false;
    for _ in 0..3 {
        if bot.work("haft_spear", AimAt::Nothing) {
            spear = true;
            break;
        }
    }
    assert!(
        spear && bot.count("stone_tipped_spear/") > 0,
        "a hafted spear"
    );
    bot.say("GOAL spear: a stone-tipped spear");

    bot.say(&format!("learned: {:?}", bot.w.learned));
    // Everything learned came by a route (no knowledge was given).
    for (node, learned) in &bot.w.knowledge.known {
        assert!(learned.route.is_some(), "{node} came by no route");
    }
}

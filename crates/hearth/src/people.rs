//! The people about the player (V2.1, H0): the persons of `hearth_people`, their bands drawn out
//! of the ecology's groups as the player comes near — the same persons each time — and folded
//! back as the player goes; living in the world as the game has it — the loaded ground, its trees
//! and water, the things lying about, the fruit and nut trees in their season, the weather on
//! their bodies, the fauna's hunters about them — their calls heard with the animals', their
//! nests bent into the trees; saved with the world. Where a band is drawn out for the first time,
//! its site lies under the nearest nut tree: an anvil with its hammers and the cobbles of the
//! place, and the flakes and cores of its work before.

use std::path::Path;

use glam::DVec3;
use hearth_body::{BodyConfig, Exposure};
use hearth_character::{Activity, Drive, Figure};
use hearth_content::Content;
use hearth_content::schema::flora::PartKind;
use hearth_content::schema::humans::BodyPlan;
use hearth_craft::engine::Surroundings;
use hearth_craft::{Crafts, Graph};
use hearth_fauna::live::Ground;
use hearth_items::{Items, Stack, WorldItems};
use hearth_math::BlockPos;
use hearth_people::inspect::Report;
use hearth_people::{
    FoodHere, Now, People, PersonView, PlayerSeen, Senses, Species, SpeciesSet, Things,
    World as PeopleWorld,
};

use crate::fauna::{Fauna, MapGround};
use crate::scene::LocalWorld;

/// The people's file in a world's folder.
const FILE: &str = "people.json.zst";

/// A fruit or nut tree about the people: where its crown stands over the ground, and what it
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
/// How far from a band's place the nut tree of its site may stand (m): within the reach in which
/// a band drawn out knows its anvils.
const SITE_M: f64 = 70.0;
/// No second site is laid within this of an anvil already lying (m).
const SITE_APART_M: f64 = 25.0;
/// How far about a player its client is told of the people (m).
const VIEW_M: f64 = 200.0;

/// The people about the player, and everyone ever met.
pub struct PeopleNear {
    pub live: People,
    pub species: SpeciesSet,
    trees: Vec<FoodTree>,
    /// Where the players were and the tick when the trees were last looked over.
    trees_at: Option<(Vec<DVec3>, u64)>,
    /// They were in sight at the last sending.
    pub shown: bool,
    /// The moment of the last tick.
    now: Option<Now>,
    /// The player's person, and how many of its life's events the player has been told.
    told: Option<(u64, usize)>,
    /// The day of the last speech the player has been told of.
    heard_to: f64,
}

/// The world as the people live in it, for one tick.
struct Surrounds<'a> {
    ground: MapGround<'a>,
    things: OnTheGround<'a>,
    items: &'a Items,
    trees: &'a [FoodTree],
    hunters: Vec<DVec3>,
    exposure: Exposure,
    /// The fires' radiant heat at a place (W/m², as a body there takes it).
    warmth: &'a (dyn Fn(DVec3) -> f32 + Sync),
    around: Surroundings,
    calls: Vec<(DVec3, bool)>,
    nests: Vec<BlockPos>,
}

/// The things lying in the world, as the people reach them.
struct OnTheGround<'a> {
    items: &'a mut WorldItems,
    lw: &'a LocalWorld,
    changed: bool,
}

impl Things for OnTheGround<'_> {
    fn near(&self, at: DVec3, reach: f64) -> Vec<(u64, Stack)> {
        // What a player put down is theirs: the people leave it be (V2.1 §8.4).
        self.items
            .items
            .iter()
            .filter(|w| w.owner.is_none())
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

impl Senses for Surrounds<'_> {
    fn ground(&self) -> &(dyn Ground + Sync) {
        &self.ground
    }

    fn things_near(&self, at: DVec3, reach: f64) -> Vec<(u64, Stack)> {
        self.things.near(at, reach)
    }

    fn place_of(&self, id: u64) -> Option<DVec3> {
        self.things.place(id)
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

    fn warmth(&self, at: DVec3) -> f32 {
        (self.warmth)(at)
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

    fn latitude(&self, at: DVec3) -> f64 {
        self.things.lw.map.planet().latitude_deg(at.z)
    }
}

impl PeopleWorld for Surrounds<'_> {
    fn things(&mut self) -> &mut dyn Things {
        &mut self.things
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

    fn settle(&mut self, at: DVec3, _species: &Species) {
        // Under the nut tree nearest the band's place.
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

/// The stones of a band's site.
struct Stones {
    anvil: String,
    hammer: String,
    cobble: String,
    core: String,
    flake: String,
}

impl Surrounds<'_> {
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

impl PeopleNear {
    /// The people of a world: as saved in `dir`, or none met yet.
    pub fn new(
        content: &Content,
        graph: &Graph,
        body: &BodyConfig,
        seed: u64,
        dir: Option<&Path>,
    ) -> Self {
        let live = dir
            .and_then(|d| load(&d.join(FILE)))
            .map_or_else(|| People::new(seed), |s| People::from_save(seed, s));
        Self {
            live,
            species: SpeciesSet::from_content(content, graph, body),
            trees: Vec::new(),
            trees_at: None,
            shown: false,
            now: None,
            told: None,
            heard_to: f64::NEG_INFINITY,
        }
    }

    /// An era's people about a place, as a new life finds them (H8; for pictures and tools as the
    /// server does it for a player): the recent past lived there (a century of households), and
    /// player 0 born into the first household offered — `age` years ago — the nearest anywhere if
    /// none lives about it. The camp of the player's band (none: no household to be born into).
    #[allow(clippy::too_many_arguments)]
    pub fn born_in_era(
        &mut self,
        eco: &mut hearth_fauna::ecology::Ecology,
        graph: &Graph,
        items: &Items,
        genetics: &hearth_people::Genetics,
        latitude: &dyn Fn(DVec3) -> f64,
        at: glam::DVec2,
        age: f64,
        now: Now,
    ) -> Option<DVec3> {
        let reach = hearth_people::sim::HOUSEHOLD_M;
        self.live.recent_history(
            eco,
            &self.species,
            graph,
            items,
            latitude,
            at,
            reach,
            100.0,
            now,
            &|_| {},
        );
        let mut offered = self
            .live
            .birth_options(&self.species, at, reach, age, &now, 4);
        if offered.is_empty() {
            offered = self
                .live
                .birth_options(&self.species, at, f64::INFINITY, age, &now, 4);
        }
        let o = *offered.first()?;
        self.live
            .born_into(o, 0, None, age, genetics, &self.species, graph, now)?;
        self.now = Some(now);
        let band = self.live.bands.iter().find(|b| b.id == o.band)?;
        Some(
            band.camp
                .unwrap_or_else(|| DVec3::new(band.home.x, 0.0, band.home.y)),
        )
    }

    /// A household offered for a birth, as the player is told it (H8, Addendum A): the mother
    /// and the father by name and age, their children, the grandparents living, the band and its
    /// people, the country it lives in and what its people know — never how anyone looks.
    pub fn birth_choice(
        &self,
        o: &hearth_people::BirthOption,
        lw: &LocalWorld,
        graph: &Graph,
        now: &Now,
    ) -> hearth_protocol::BirthChoice {
        let live = &self.live;
        let name = |id: u64| {
            live.get(id)
                .map(|p| p.name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "one of them".to_owned())
        };
        let age = |id: u64| live.get(id).map_or(0.0, |p| p.age(now));
        let band = live.bands.iter().find(|b| b.id == o.band);
        let sp = band.and_then(|b| self.species.get(&b.species));
        let people = band
            .and_then(|b| {
                let l = b.culture.language.as_ref()?;
                let d = sp?.language.as_ref()?;
                l.say(d, "people")
            })
            .map(|w| {
                let mut c = w.chars();
                c.next()
                    .map(|f| f.to_uppercase().chain(c).collect::<String>())
                    .unwrap_or_default()
            });
        let kind = sp.map_or_else(|| "people".to_owned(), |s| s.name.clone());
        let title = match &people {
            Some(p) => format!("A family of the {p} ({kind})"),
            None => format!("A family of {kind}"),
        };
        // A parent since dead (a player starting grown may be an orphan of the band).
        let parent = |who: &str, id: u64| match live.get(id).and_then(|p| p.life.died.as_ref()) {
            Some(d) => format!(
                "{who}: {}, who died at {:.0}",
                name(id),
                live.get(id)
                    .map_or(0.0, |p| (d.day - p.life.born) / now.year_days.max(1.0))
            ),
            None => format!("{who}: {}, {:.0} years old", name(id), age(id)),
        };
        let mut lines = vec![
            parent("Your mother", o.mother),
            parent("Your father", o.father),
        ];
        let mut children: Vec<(f64, bool)> = live
            .persons
            .iter()
            .filter(|p| p.life.mother == Some(o.mother) && p.alive())
            .map(|p| (p.age(now), p.life.female))
            .collect();
        children.sort_by(|a, b| b.0.total_cmp(&a.0));
        lines.push(if children.is_empty() {
            "You would be their first child".to_owned()
        } else {
            let told: Vec<String> = children
                .iter()
                .map(|(a, f)| format!("a {} of {:.0}", if *f { "sister" } else { "brother" }, a))
                .collect();
            format!("Brothers and sisters: {}", told.join(", "))
        });
        let grand = [o.mother, o.father]
            .iter()
            .filter_map(|&p| live.get(p))
            .flat_map(|p| [p.life.mother, p.life.father])
            .flatten()
            .filter(|g| live.get(*g).is_some_and(|g| g.alive()))
            .count();
        if grand > 0 {
            lines.push(format!(
                "{grand} grandparent{} living",
                if grand == 1 { "" } else { "s" }
            ));
        }
        if let Some(b) = band {
            let n = live.members(b.id).filter(|p| p.alive()).count();
            let at = b
                .camp
                .unwrap_or_else(|| DVec3::new(b.home.x, 0.0, b.home.y));
            let s = lw.terrain().sample(at.x as i32, at.z as i32);
            let biome = s.biome.name().replace('_', " ");
            lines.push(format!(
                "A band of {n}, living in the {biome} of the {}",
                s.realm.name()
            ));
            // What its people know that most marks them: their deepest techniques.
            let lore = &self.species.lore;
            let mut known: Vec<(u8, String)> = b
                .culture
                .knowledge
                .iter()
                .filter_map(|k| {
                    let depth = lore
                        .nodes
                        .iter()
                        .find(|n| &n.id == k)
                        .map_or(0, |n| n.depth);
                    graph.node(k).map(|n| (depth, n.name.to_lowercase()))
                })
                .collect();
            known.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            let told: Vec<String> = known.into_iter().take(5).map(|k| k.1).collect();
            if !told.is_empty() {
                lines.push(format!("They know {}", told.join(", ")));
            }
        }
        hearth_protocol::BirthChoice { title, lines }
    }

    /// A player hands one of the people what it holds: a gift (V2.1 §8.7), taken into their
    /// keeping — or given back, when they have no room or the player is no one among them.
    /// Words for the player.
    pub fn give_to(
        &mut self,
        player: u64,
        person: u64,
        stack: Stack,
        items: &Items,
        content: &Content,
    ) -> Result<String, Stack> {
        let (Some(now), Some(me)) = (self.now, self.live.player_person(player).map(|p| p.id))
        else {
            return Err(stack);
        };
        let what = items.get(&stack.id).map_or_else(
            || "it".to_owned(),
            |k| format!("the {}", k.name.to_lowercase()),
        );
        self.live
            .gift(me, person, stack, &self.species, items, content, now)?;
        Ok(format!("You hold out {what}, and it is taken."))
    }

    /// What the people near a player have said since the player was last told, as the player
    /// makes it out (V2.1 §10.3) — and the player's person learning their words by hearing them.
    pub fn heard_by(&mut self, player: u64) -> Vec<hearth_protocol::HeardLine> {
        let Some((me, at)) = self.live.player_person(player).map(|p| (p.id, p.place.pos)) else {
            return Vec::new();
        };
        let since = self.heard_to;
        let new: Vec<hearth_people::Said> = self
            .live
            .said
            .iter()
            .filter(|s| {
                s.day > since
                    && s.speaker != me
                    && (s.at - at).length() < hearth_people::speech::HEARD_M
            })
            .cloned()
            .collect();
        self.heard_to = self.live.said.iter().map(|s| s.day).fold(since, f64::max);
        let mut out = Vec::new();
        for s in &new {
            let defs = self
                .live
                .get(s.speaker)
                .and_then(|q| self.species.get(&q.species))
                .and_then(|sp| sp.language.as_ref());
            if let (Some(d), Some(listener)) = (defs, self.live.get(me))
                && let Some(h) = self.live.heard(d, listener, s)
            {
                out.push(hearth_protocol::HeardLine {
                    speaker: h.speaker,
                    to_you: h.to_you,
                    spoken: h.spoken,
                    sense: h.sense,
                    understood: h.understood,
                    gesture: h.gesture.map(|g| gesture_words(g).to_owned()),
                });
            }
            self.live.learn_from(me, s);
        }
        out
    }

    /// What has befallen the player's person among others since the player was last told, in
    /// words (and whether it went well): met and greeted by strangers, warned off, taken in,
    /// quarrelled with.
    pub fn news_for(&mut self, player: u64) -> Vec<(String, bool)> {
        let Some(p) = self.live.player_person(player) else {
            return Vec::new();
        };
        let n = p.life.events.len();
        let from = match self.told {
            Some((id, k)) if id == p.id => k.min(n),
            _ => n,
        };
        let id = p.id;
        let words: Vec<(String, bool)> = p.life.events[from..]
            .iter()
            .filter_map(|e| {
                use hearth_people::Event;
                match e.event {
                    Event::Greeted { .. } => Some((
                        "One of a band not your own comes to meet you, and greets you: you are \
                         their guest."
                            .to_owned(),
                        true,
                    )),
                    Event::Unwelcome { .. } => Some((
                        "One of a band not your own shouts at you to go: you are not welcome here."
                            .to_owned(),
                        false,
                    )),
                    Event::TakenIn { .. } => Some((
                        "They have taken you in: you are one of them now.".to_owned(),
                        true,
                    )),
                    Event::Quarrelled { .. } => {
                        Some(("Someone has it out with you, angry.".to_owned(), false))
                    }
                    _ => None,
                }
            })
            .collect();
        self.told = Some((id, n));
        words
    }

    /// Saves the people into `dir` with the animals: every band folded back into its numbers in
    /// a copy, as if the player were far away, the records waiting for the player's return.
    pub fn save(&self, fauna: &Fauna, dir: &Path) {
        let mut people = self.live.clone();
        let species = &self.species;
        let now = self.now;
        fauna.save_with(dir, |eco| {
            if let Some(now) = now {
                people.fold_all(eco, species, now);
            }
        });
        let json = match hearth_people::save::to_json(&people.to_save()) {
            Ok(j) => j,
            Err(e) => {
                log::error!("people not saved: {e}");
                return;
            }
        };
        match zstd::encode_all(json.as_slice(), 3) {
            Ok(z) => {
                if let Err(e) = std::fs::write(dir.join(FILE), z) {
                    log::error!("people not saved: {e}");
                }
            }
            Err(e) => log::error!("people not saved: {e}"),
        }
    }

    /// The fruit and nut trees about each player, and what each gives at this time of the year.
    fn look_for_trees(&mut self, lw: &LocalWorld, players: &[DVec3], year_frac: f32, tick: u64) {
        let fresh = self.trees_at.as_ref().is_some_and(|(was, t)| {
            was.len() == players.len()
                && was
                    .iter()
                    .zip(players)
                    .all(|(a, b)| (*a - *b).length() < TREES_REFRESH_M)
                && tick.saturating_sub(*t) < 2400
        });
        if fresh {
            return;
        }
        self.trees_at = Some((players.to_vec(), tick));
        let mut placed = Vec::new();
        for at in players {
            let (x, z) = (at.x.floor() as i32, at.z.floor() as i32);
            for t in lw.generator.features().trees_in(
                &lw.generator,
                &lw.vegetation,
                (x - TREES_M, z - TREES_M),
                (x + TREES_M, z + TREES_M),
            ) {
                if !placed
                    .iter()
                    .any(|q: &hearth_worldgen::cubegen::features::PlacedTree| q.foot == t.foot)
                {
                    placed.push(t);
                }
            }
        }
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

    /// A tick of `dt` seconds of play: every 40 ticks the bands near any of the regions lived in
    /// full (the players' places) are drawn out and those far from all of them folded back; every
    /// tick the people in full live, with the players among them to notice. Blocks it changed
    /// (nests) go into `changed`; true when the things lying about changed.
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
        warmth: &(dyn Fn(DVec3) -> f32 + Sync),
        around: Surroundings,
        year_frac: f32,
        regions: &[DVec3],
        players: &[PlayerSeen],
        family: Option<(&hearth_people::Birth, &hearth_people::Household)>,
        now: Now,
        dt: f32,
    ) -> bool {
        self.now = Some(now);
        let at = regions;
        let every = now.tick.is_multiple_of(40);
        let out = self.live.full().next().is_some();
        if !out && !every {
            return false;
        }
        // The fruit and nut trees about them, while there are people to feed at them or bands
        // about to be drawn out.
        if every && (out || coming(fauna, at)) {
            self.look_for_trees(lw, at, year_frac, now.tick);
        }
        // The hunters about: the fauna's animals that take a person.
        let people_species: Vec<usize> = fauna
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
            .filter(|a| !a.dead && at.iter().any(|p| (a.pos - *p).length() < 260.0))
            .filter(|a| {
                fauna.eco.catalog.species[a.species as usize]
                    .prey
                    .iter()
                    .any(|(p, _)| people_species.contains(p))
            })
            .map(|a| a.pos)
            .collect();
        let content = lw.content.clone();
        let (calls, nests, items_changed) = {
            let mut world = Surrounds {
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
                warmth,
                around,
                calls: Vec::new(),
                nests: Vec::new(),
            };
            if every {
                // The family the player is born into, set down where they begin once the land
                // about is known (V2.1 Addendum A).
                if let Some((birth, household)) = family
                    && self.live.player_person(0).is_none()
                    && let Some(&p) = at.first()
                {
                    found_family(
                        &mut self.live,
                        &self.species,
                        &mut fauna.eco,
                        graph,
                        items,
                        &mut world,
                        birth,
                        household,
                        p,
                        now,
                    );
                }
                self.live.fold(&mut fauna.eco, &self.species, at, now);
                self.live.draw_out(
                    &mut fauna.eco,
                    &self.species,
                    graph,
                    items,
                    &mut world,
                    at,
                    now,
                );
            }
            if self.live.full().next().is_some() {
                self.live.step(
                    &self.species,
                    crafts,
                    graph,
                    &content,
                    items,
                    &mut world,
                    players,
                    now,
                    dt,
                );
            }
            (world.calls, world.nests, world.things.changed)
        };
        // Their calls, heard as the animals' are: the population's alarm.
        for (at, alarm) in calls {
            let Some(sp) = people_species.first().copied() else {
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

    /// What the player watching sees done this tick, and whether they stand by a scatter of
    /// knapped stone: the triggers to hear.
    pub fn watched(
        &self,
        crafts: &Crafts,
        graph: &Graph,
        world_items: &WorldItems,
        eye: DVec3,
        yaw: f32,
    ) -> Vec<String> {
        if self.live.full().next().is_none() {
            return Vec::new();
        }
        let mut out = hearth_people::seen(&self.live.done, graph, crafts, eye, yaw);
        let lying = world_items
            .items
            .iter()
            .map(|w| (DVec3::from_array(w.pos), w.stack.id.as_str(), w.stack.count));
        if hearth_people::scatter_near(lying, eye) {
            out.push(STUDY_SCATTER.to_owned());
        }
        out
    }

    /// What the people at their work in front of a player, who would teach the player, show it
    /// this tick (V2.1 §11.2): each work's knowledge and the insight toward it.
    pub fn lessons(
        &mut self,
        player: u64,
        crafts: &Crafts,
        eye: DVec3,
        yaw: f32,
        dt: f32,
    ) -> Vec<(String, f32)> {
        let Some(now) = self.now else {
            return Vec::new();
        };
        self.live
            .lessons_for(player, crafts, &self.species, eye, yaw, &now, dt)
    }

    /// The camps of the bands lived in full whose people keep a fire there (V2.1 §15.3; H8):
    /// where each is, whether any of its people are about it to tend the fire, and its beds —
    /// one a household, of furs where they know fur bedding — where its era has them.
    pub fn camps(&self) -> Vec<crate::workshop::CampKept> {
        let knows = |b: &hearth_people::Band, key: &str| {
            b.culture
                .knowledge
                .iter()
                .any(|k| k.rsplit(':').next() == Some(key))
        };
        let mut out = Vec::new();
        for b in &self.live.bands {
            if b.tier != hearth_people::person::Tier::Full {
                continue;
            }
            let (Some(ways), Some(at)) = (self.live.era.of(&b.species), b.camp) else {
                continue;
            };
            if !ways.hearth || !knows(b, "fire_keeping") {
                continue;
            }
            let members: Vec<&hearth_people::Person> =
                self.live.members(b.id).filter(|p| p.alive()).collect();
            if members.is_empty() {
                continue;
            }
            let tend = members.iter().any(|p| (p.place.pos - at).length() < 20.0);
            let mut households: Vec<u64> =
                members.iter().filter_map(|p| p.social.household).collect();
            households.sort_unstable();
            households.dedup();
            out.push(crate::workshop::CampKept {
                at,
                tend,
                beds: if ways.bedding {
                    households.len().clamp(1, 10) as u8
                } else {
                    0
                },
                fur: knows(b, "fur_bedding"),
            });
        }
        out
    }

    /// The people in full about a player, as that player's client draws them.
    pub fn views(&self, near: DVec3) -> Vec<PersonView> {
        self.now.map_or_else(Vec::new, |now| {
            self.live.views(&self.species, &now, near, VIEW_M)
        })
    }

    /// A person's record for the inspector.
    pub fn inspect(&self, id: u64, graph: &Graph) -> Option<Report> {
        let now = self.now?;
        hearth_people::inspect::report(&self.live, &self.species, graph, id, &now)
    }
}

/// The people's save in a world's folder.
fn load(path: &Path) -> Option<hearth_people::PeopleSave> {
    let bytes = std::fs::read(path).ok()?;
    let json = match zstd::decode_all(bytes.as_slice()) {
        Ok(j) => j,
        Err(e) => {
            log::error!("the people's save is unreadable ({e}); starting without them");
            return None;
        }
    };
    match hearth_people::save::from_json(&json) {
        Ok(s) => Some(s),
        Err(e) => {
            log::error!("the people's save is unreadable ({e}); starting without them");
            None
        }
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

/// Whether a band not yet drawn out is near enough any player to be soon.
fn coming(fauna: &Fauna, players: &[DVec3]) -> bool {
    fauna.eco.regions.values().any(|r| {
        r.groups.iter().any(|g| {
            !g.live
                && fauna.eco.catalog.species[g.species as usize].hominin
                && players.iter().any(|p| {
                    (g.pos[0] - p.x).hypot(g.pos[1] - p.z) < hearth_people::sim::NEAR_M + 20.0
                })
        })
    })
}

/// A person's figure: its species' body plan at its height and sex.
pub fn figure(v: &PersonView) -> Figure {
    match v.plan {
        BodyPlan::Australopith => Figure::hominin(looks(v)),
        BodyPlan::Erectus | BodyPlan::Neanderthal | BodyPlan::Modern => Figure::starting(looks(v)),
    }
}

/// How a person looks: their look (from their phenotype, H1) on a figure of their height and
/// sex; an australopith's hair a coat over all of it, not a style or a beard.
pub fn looks(v: &PersonView) -> hearth_character::Appearance {
    let mut a = appearance_of(&v.look, v.female, v.height_m);
    a.grown = v.grown;
    // A baby's hair is short and fine.
    if v.grown < 0.12 {
        a.hair = hearth_character::HairStyle::ShortCrop;
    }
    match v.plan {
        BodyPlan::Australopith => {
            a.hair = hearth_character::HairStyle::ShortCrop;
            a.facial_hair = hearth_character::FacialHair::None;
            a.build = 0.7;
        }
        BodyPlan::Erectus => a.plan = hearth_character::Plan::Erectus,
        BodyPlan::Neanderthal => {
            a.plan = hearth_character::Plan::Neanderthal;
            a.build = (a.build + 0.25).min(1.0);
        }
        BodyPlan::Modern => {}
    }
    a
}

/// A figure's appearance from a look: skin, hair (its colour, and a style its curl and sex
/// suggest until cultures dress it, H5), eyes, beard and build.
pub fn appearance_of(
    look: &hearth_people::Look,
    female: bool,
    height_m: f32,
) -> hearth_character::Appearance {
    use hearth_character::{EyeColor, FacialHair, HairStyle};
    use hearth_people::Eyes;
    let curl = look.hair_curl;
    let hair = match (female, curl) {
        (true, c) if c < 0.3 => HairStyle::LongStraight,
        (true, c) if c < 0.55 => HairStyle::LongWavy,
        (false, c) if c < 0.55 => HairStyle::ShortCrop,
        (_, c) if c < 0.8 => HairStyle::Curly,
        _ => HairStyle::Coily,
    };
    let eyes = match look.eyes {
        Eyes::DarkBrown => EyeColor::DarkBrown,
        Eyes::Brown => EyeColor::Brown,
        Eyes::Amber => EyeColor::Amber,
        Eyes::Hazel => EyeColor::Hazel,
        Eyes::Green => EyeColor::Green,
        Eyes::Blue => EyeColor::Blue,
        Eyes::Grey => EyeColor::Grey,
    };
    let facial_hair = match look.beard {
        b if b > 0.65 => FacialHair::FullBeard,
        b if b > 0.45 => FacialHair::ShortBeard,
        b if b > 0.25 => FacialHair::Stubble,
        _ => FacialHair::None,
    };
    hearth_character::Appearance {
        body: if female {
            hearth_character::BodyType::Female
        } else {
            hearth_character::BodyType::Male
        },
        height_m,
        build: look.build,
        skin_tone: look.skin_tone,
        undertone: look.undertone,
        hair,
        hair_color: look.hair_color,
        facial_hair,
        eyes,
        ..hearth_character::Appearance::default()
    }
}

/// How a person's figure moves for what it is doing: walking and running, climbing its tree,
/// crouched at its work or its food, lying asleep in its nest, standing to watch or to call.
pub fn drive(v: &PersonView) -> Drive {
    use hearth_people::Doing;
    let in_tree = v.medium == hearth_fauna::live::Medium::Tree;
    let mut activity = match &v.doing {
        _ if v.dead => Activity::Lie,
        Doing::Sleeping => Activity::Lie,
        _ if in_tree && v.speed > 0.05 => Activity::Ladder,
        Doing::Working { .. }
        | Doing::Feeding
        | Doing::Drinking
        | Doing::Nesting
        | Doing::Taking { .. } => Activity::Crouch,
        Doing::Grooming { .. }
        | Doing::Resting
        | Doing::Carried { .. }
        | Doing::Imitating { .. }
        | Doing::Tending { .. } => Activity::Crouch,
        _ if v.speed > 3.0 => Activity::Sprint,
        _ if v.speed > 1.8 => Activity::Jog,
        _ if v.speed > 0.1 => Activity::Walk,
        _ => Activity::Stand,
    };
    // What it feels shows in how it holds itself: cowering and trembling in fear, bristling
    // in anger, the head bowed in grief, hung and turned aside in shame, up in joy, turned
    // away in disgust, inclined toward the others in affection (the head's pitch in degrees,
    // down positive, and turn).
    let (mut pitch, mut turn, mut shiver, mut breaths) = (0.0, 0.0, 0.0, 16.0);
    if let Some((display, felt)) = v.shows {
        use hearth_content::schema::psyche::Display;
        match display {
            Display::Cower => {
                if activity == Activity::Stand {
                    activity = Activity::Crouch;
                }
                pitch = -6.0;
                shiver = 0.5 * felt;
                breaths = 28.0;
            }
            Display::Bristle => {
                pitch = -12.0 * felt;
                breaths = 24.0;
            }
            Display::Slump => {
                pitch = 35.0 * felt;
                breaths = 11.0;
            }
            Display::Hang => {
                pitch = 40.0 * felt;
                turn = 30.0 * felt;
            }
            Display::Bright => {
                pitch = -15.0 * felt;
                breaths = 18.0;
            }
            Display::Recoil => {
                pitch = -18.0 * felt;
                turn = 45.0 * felt;
            }
            Display::Warm => {
                pitch = 8.0 * felt;
                turn = 15.0 * felt;
            }
            Display::None => {}
        }
    }
    Drive {
        activity,
        speed: v.speed,
        vertical: if in_tree { v.speed } else { 0.0 },
        look_pitch: pitch,
        look_yaw: turn,
        climb: 0.0,
        shiver,
        breaths_per_min: breaths,
        holding: hearth_character::Holding::default(),
    }
}

/// Founds the family the player is born into at their place, its numbers a group of the
/// ecological cells so that it folds and wakes as any band does.
#[allow(clippy::too_many_arguments)]
fn found_family(
    live: &mut People,
    species: &SpeciesSet,
    eco: &mut hearth_fauna::ecology::Ecology,
    graph: &Graph,
    items: &Items,
    world: &mut dyn hearth_people::World,
    birth: &hearth_people::Birth,
    household: &hearth_people::Household,
    p: DVec3,
    now: Now,
) {
    if !eco.regions.contains_key(&eco.region_key(p.x, p.z)) {
        return;
    }
    let kind = crate::born::PLAYER_SPECIES;
    let founding = hearth_people::Founding {
        birth,
        household,
        species: kind,
        player: 0,
        at: p,
        cold: hearth_people::sim::cold_country(eco, p.x, p.z),
    };
    let Some((band, _)) = live.found_family(species, graph, items, world, founding, now) else {
        return;
    };
    let Some(sp) = species.get(kind) else {
        return;
    };
    let n = live.numbers(band, sp, &now);
    let group = eco.catalog.index(kind).and_then(|si| {
        eco.place_group(
            si as u16,
            [p.x, p.z],
            [n.young, n.juveniles, n.females, n.males],
        )
    });
    if let Some(b) = live.bands.iter_mut().find(|b| b.id == band) {
        b.population_group = group;
    }
}

/// Within this of where one died is "near" (m).
const NEAR_DEATH_M: f64 = 2_000.0;
/// The most offered to live on as.
const OTHERS_MOST: usize = 48;

/// What the player's own record adds to the story of their life.
pub struct LifeFacts<'a> {
    pub name: &'a str,
    pub walked_km: f64,
    pub farthest_km: f64,
    /// What they knew (the knowledge's names).
    pub known: Vec<String>,
    /// Where they died.
    pub at: DVec3,
    /// Who the world lets them live on as (Addendum B §2.3).
    pub scope: hearth_save::InhabitScope,
}

/// Who another is to a person, in words ("your sister", "a man of your band"), and how near:
/// a partner first, then children, brothers and sisters, parents, the band, others.
fn kin_words(of: &hearth_people::Person, q: &hearth_people::Person) -> (u8, String) {
    let sex = |f: bool, a: &str, b: &str| if f { a.to_owned() } else { b.to_owned() };
    if of.social.bond == Some(q.id) || q.social.bond == Some(of.id) {
        (0, "your partner".into())
    } else if q.life.mother == Some(of.id) || q.life.father == Some(of.id) {
        (1, sex(q.life.female, "your daughter", "your son"))
    } else if of.life.mother == Some(q.id) {
        (3, "your mother".into())
    } else if of.life.father == Some(q.id) {
        (3, "your father".into())
    } else if of.life.mother.is_some() && q.life.mother == of.life.mother {
        (2, sex(q.life.female, "your sister", "your brother"))
    } else if q.social.band == of.social.band {
        (
            4,
            sex(q.life.female, "a woman of your band", "a man of your band"),
        )
    } else {
        (
            5,
            sex(
                q.life.female,
                "a woman of another family",
                "a man of another family",
            ),
        )
    }
}

impl PeopleNear {
    /// The player's life told at its end (Addendum B §2), from their person's record and their
    /// own; and who of their people they could live on as — the living grown of their kin and
    /// band, and of others near where they died — told only by who they are to the dead.
    pub fn life_story(&self, facts: &LifeFacts<'_>) -> hearth_protocol::Story {
        let mut lines = Vec::new();
        let mut others = Vec::new();
        let Some(now) = self.now else {
            return hearth_protocol::Story { lines, others };
        };
        let name = if facts.name.trim().is_empty() {
            "You".to_owned()
        } else {
            facts.name.trim().to_owned()
        };
        if let Some(p) = self.live.player_person(0) {
            let years = (p.life.died.as_ref().map_or(now.day, |d| d.day) - p.life.born)
                / now.year_days.max(1.0);
            lines.push(if years < 1.0 {
                format!("{name} lived {:.0} days.", years * now.year_days)
            } else {
                format!("{name} lived {:.0} years.", years.floor())
            });
            let alive = |id: Option<u64>| id.and_then(|id| self.live.get(id)).map(|q| q.alive());
            let parent = |id: Option<u64>, who: &str| match alive(id) {
                Some(true) => format!("{who} lives."),
                Some(false) => format!("{who} died before you."),
                None => format!("{who} you never knew."),
            };
            lines.push(format!(
                "{} {}",
                parent(p.life.mother, "Your mother"),
                parent(p.life.father, "Your father")
            ));
            if let Some(b) = p.social.bond.and_then(|b| self.live.get(b)) {
                let (_, words) = kin_words(p, b);
                lines.push(format!("You lived with {words}."));
            }
            let children: Vec<&hearth_people::Person> = self
                .live
                .persons
                .iter()
                .filter(|q| q.life.mother == Some(p.id) || q.life.father == Some(p.id))
                .collect();
            if !children.is_empty() {
                let living = children.iter().filter(|c| c.alive()).count();
                let word = if children.len() == 1 {
                    "child"
                } else {
                    "children"
                };
                lines.push(format!(
                    "You had {} {word}, {living} of them living.",
                    children.len()
                ));
            }
            let mourners = self
                .live
                .persons
                .iter()
                .filter(|q| q.alive() && q.social.band == p.social.band && q.id != p.id)
                .count();
            if mourners > 0 {
                lines.push(format!("{mourners} of your people mourn you."));
            }
            others = self.others(p, facts.at, &now, facts.scope, OTHERS_MOST);
        }
        if !facts.known.is_empty() {
            let mut known = facts.known.clone();
            known.sort();
            let shown: Vec<String> = known.iter().take(8).cloned().collect();
            let more = known.len().saturating_sub(shown.len());
            lines.push(if more > 0 {
                format!("You knew {}, and {more} more.", shown.join(", "))
            } else {
                format!("You knew {}.", shown.join(", "))
            });
        }
        lines.push(format!(
            "You walked {:.1} km in all, as far as {:.1} km from where your life began.",
            facts.walked_km, facts.farthest_km
        ));
        hearth_protocol::Story { lines, others }
    }

    /// Those one who has died (`dead`, at `at`) may live on as (Addendum B §2.2–2.3): living
    /// people of the world lived in full or as households, never another player's, nor anyone
    /// fighting, fleeing, badly hurt and dying, or just giving birth; as the world's scope allows
    /// — anyone, kin, group and region, or kin only. Their family first, then their group, then
    /// those near, children among them (their childhood lived on from their age).
    pub fn others(
        &self,
        dead: &hearth_people::Person,
        at: DVec3,
        now: &Now,
        scope: hearth_save::InhabitScope,
        most: usize,
    ) -> Vec<hearth_protocol::Other> {
        use hearth_people::person::Tier;
        use hearth_save::InhabitScope;
        if scope == InhabitScope::None {
            return Vec::new();
        }
        let fighting: Vec<u64> = self.live.quarrels.iter().flat_map(|q| [q.a, q.b]).collect();
        let not_now = |q: &hearth_people::Person| {
            fighting.contains(&q.id)
                || matches!(
                    q.mind.doing,
                    hearth_people::Doing::Fleeing { .. } | hearth_people::Doing::Mobbing { .. }
                )
                || q.body
                    .injuries
                    .iter()
                    .any(|i| (i.severity >= 0.6 && i.healed < 0.5) || i.bleeding_ml_min > 20.0)
                || q.life.events.iter().any(|e| {
                    matches!(e.event, hearth_people::person::Event::Bore { .. })
                        && now.day - e.day < 1.0
                })
        };
        let mut found: Vec<(u8, f64, u64, hearth_protocol::Other)> = self
            .live
            .persons
            .iter()
            .filter(|q| q.alive() && q.player.is_none() && q.id != dead.id)
            .filter(|q| matches!(q.tier, Tier::Full | Tier::Household))
            .filter(|q| !not_now(q))
            .filter_map(|q| {
                let family = hearth_people::kin::kin_of(&self.live, dead.id, q.id).is_some()
                    || dead.social.bond == Some(q.id);
                let group = q.social.band == dead.social.band;
                let d = {
                    let dx = q.place.pos.x - at.x;
                    let dz = q.place.pos.z - at.z;
                    dx.hypot(dz)
                };
                let near = d <= NEAR_DEATH_M;
                let region = d <= hearth_people::sim::HOUSEHOLD_M;
                let allowed = match scope {
                    InhabitScope::Anyone => true,
                    InhabitScope::KinGroupRegion => family || group || region,
                    InhabitScope::KinOnly => family,
                    InhabitScope::None => false,
                };
                if !allowed {
                    return None;
                }
                let maturity = self
                    .species
                    .get(&q.species)
                    .map_or(18.0, |sp| sp.life.maturity_years as f64);
                let age = q.age(now);
                let child = age < maturity;
                let (rank, who) = kin_words(dead, q);
                let years = age.floor() as u32;
                let words = if family || group {
                    format!("{who}, {years} years")
                } else {
                    // A stranger, by their household.
                    let household = q
                        .social
                        .household
                        .map_or(1, |h| self.live.household_of(h).len());
                    let kind = match (child, q.life.female) {
                        (true, true) => "a girl",
                        (true, false) => "a boy",
                        (false, true) => "a woman",
                        (false, false) => "a man",
                    };
                    format!(
                        "{kind} of {years}, of a household of {household} {}",
                        if near { "near" } else { "away" }
                    )
                };
                let order = if family {
                    rank
                } else if group {
                    4
                } else if near {
                    5
                } else {
                    6
                };
                Some((
                    order,
                    d,
                    q.id,
                    hearth_protocol::Other {
                        id: q.id,
                        words,
                        family,
                        group,
                        near,
                        child,
                    },
                ))
            })
            .collect();
        found.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
        found.into_iter().take(most).map(|(_, _, _, o)| o).collect()
    }

    /// Whether a dead player may live on as a person now (see [`Self::others`]).
    pub fn may_live_as(
        &self,
        player: u64,
        id: u64,
        at: DVec3,
        scope: hearth_save::InhabitScope,
    ) -> bool {
        let (Some(now), Some(me)) = (self.now, self.live.player_person(player)) else {
            return false;
        };
        self.others(me, at, &now, scope, usize::MAX)
            .iter()
            .any(|o| o.id == id)
    }

    /// Who the player is, having taken up a person's life (Addendum B §2): the briefing.
    pub fn who_you_are(&self, id: u64, known: &[String]) -> Vec<String> {
        let mut lines = Vec::new();
        let (Some(now), Some(p)) = (self.now, self.live.get(id)) else {
            return lines;
        };
        lines.push(format!(
            "You are a {} of {:.0} years, of a family of the people here.",
            if p.life.female { "woman" } else { "man" },
            p.age(&now).floor()
        ));
        let mut kin: Vec<(u8, String)> = self
            .live
            .persons
            .iter()
            .filter(|q| q.alive() && q.id != p.id)
            .map(|q| (q, kin_words(p, q)))
            .filter(|(_, (rank, _))| *rank <= 3)
            .map(|(q, (rank, words))| (rank, format!("{words}, {:.0}", q.age(&now).floor())))
            .collect();
        kin.sort();
        if kin.is_empty() {
            lines.push("None of your close kin live.".into());
        } else {
            let words: Vec<String> = kin.into_iter().map(|(_, w)| w).collect();
            lines.push(format!("Your kin: {}.", words.join("; ")));
        }
        let lost = p
            .life
            .events
            .iter()
            .filter(|e| matches!(e.event, hearth_people::person::Event::Mourned { .. }))
            .count();
        if lost > 0 {
            lines.push(format!("You have mourned {lost} of your kin."));
        }
        if !known.is_empty() {
            let mut known = known.to_vec();
            known.sort();
            lines.push(format!("You know {}.", known.join(", ")));
        }
        lines.push("Those who know you may find you not yourself.".into());
        lines
    }
}

/// A gesture in words.
fn gesture_words(g: hearth_people::Gesture) -> &'static str {
    use hearth_people::Gesture;
    match g {
        Gesture::Point => "pointing",
        Gesture::Show => "showing",
        Gesture::Offer => "holding something out",
        Gesture::Beckon => "beckoning",
        Gesture::Shoo => "waving you off",
        Gesture::ThreatDisplay => "drawn up, threatening",
        Gesture::Submission => "head bowed",
        Gesture::Embrace => "arms open",
        Gesture::Hands => "hands held out",
    }
}

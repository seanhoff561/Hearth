//! Making things in a running world (V2-5): processes done to blocks and things, stations laid
//! and lit, fire that cooks, digging that leaves a spoil pile, eating and drinking, and the
//! knowledge that doing things teaches. The world runs its ticks as fast as they go.

use std::sync::Arc;
use std::time::{Duration, Instant};

use glam::DVec3;
use hearth::server::{Server, View, WorldSpec};
use hearth_craft::KnowledgeState;
use hearth_items::{Carry, Items, Stack, WorldItem};
use hearth_math::BlockPos;
use hearth_protocol::{AimAt, ToClient, ToServer};
use hearth_render::atlas::TextureArray;
use hearth_world::{BlockRegistry, CubeMap};

/// A test's view of a running world.
pub struct World {
    pub server: Server,
    pub reg: Arc<BlockRegistry>,
    pub mirror: CubeMap,
    pub items: Arc<Items>,
    pub carry: Carry,
    pub knowledge: KnowledgeState,
    pub lying: Vec<WorldItem>,
    pub feet: DVec3,
    pub ticks: u64,
    pub ticks_per_day: f64,
    /// What came of things done: (process, done, words).
    pub acted: Vec<(String, bool, String)>,
    pub learned: Vec<String>,
    pub working: bool,
}

impl World {
    pub fn start(dir: &std::path::Path, knowledge: hearth_save::KnowledgeMode, seed: u64) -> Self {
        let spec = WorldSpec {
            name: "test".into(),
            seed,
            planet: hearth_math::PlanetSize::Tiny,
            cache_dir: None,
            saves_dir: Some(dir.to_path_buf()),
            appearance: hearth_character::Appearance::default(),
            death_rules: hearth_save::DeathRules::default(),
            knowledge,
        };
        let atlas = Arc::new(TextureArray::from_entries(&hearth_texgen::textures_for(
            None,
        )));
        let server = Server::start(
            spec,
            atlas,
            View {
                radius: 2,
                vertical: 2,
            },
        );
        let t0 = Instant::now();
        let ready = loop {
            assert!(t0.elapsed() < Duration::from_secs(180), "no world");
            match server.poll() {
                Some(ToClient::Ready(r)) => break r,
                Some(_) => {}
                None => std::thread::sleep(Duration::from_millis(5)),
            }
        };
        server.send(ToServer::Fast(true));
        let mut w = World {
            server,
            mirror: CubeMap::new(ready.planet),
            reg: ready.reg.clone(),
            items: ready.items.clone(),
            carry: Carry::default(),
            knowledge: KnowledgeState::default(),
            lying: Vec::new(),
            feet: ready.player.pos,
            ticks: ready.ticks,
            ticks_per_day: ready.calendar.ticks_per_day(),
            acted: Vec::new(),
            learned: Vec::new(),
            working: false,
        };
        // The ground about the player streamed in.
        let feet = w.feet;
        w.until(60.0, |w| {
            w.mirror
                .block(BlockPos::containing(feet - DVec3::new(0.0, 0.5, 0.0)))
                .is_some()
        });
        w
    }

    /// Takes in what the server says.
    pub fn pump(&mut self) {
        while let Some(m) = self.server.poll() {
            match m {
                ToClient::Cube(p, c) => self.mirror.insert_cube(p, c, &self.reg),
                ToClient::Unload(p) => {
                    self.mirror.remove_cube(p);
                }
                ToClient::Carried(c) => self.carry = c,
                ToClient::Items(v) => self.lying = v,
                ToClient::Knowledge(k) => self.knowledge = *k,
                ToClient::Clock(t) => self.ticks = t,
                ToClient::Placed(m) => self.feet = m.pos,
                ToClient::Work(w) => self.working = w.is_some(),
                ToClient::Acted(a) => self.acted.push((a.process, a.done, a.words)),
                ToClient::Learned {
                    name,
                    discovered: true,
                    ..
                } => self.learned.push(name),
                _ => {}
            }
        }
    }

    /// Pumps until `pred` holds (or `secs` pass); whether it did.
    pub fn until(&mut self, secs: f64, mut pred: impl FnMut(&Self) -> bool) -> bool {
        let t0 = Instant::now();
        loop {
            self.pump();
            if pred(self) {
                return true;
            }
            if t0.elapsed() > Duration::from_secs_f64(secs) {
                return false;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Does a process and waits for what came of it.
    pub fn act(&mut self, process: &str, aim: AimAt) -> (bool, String) {
        let n = self.acted.len();
        self.server.send(ToServer::Act {
            process: format!("hearth:{process}"),
            aim,
            hand: None,
        });
        let id = format!("hearth:{process}");
        let ok = self.until(120.0, |w| w.acted[n..].iter().any(|(p, _, _)| *p == id));
        assert!(ok, "{process}: nothing came of it ({:?})", &self.acted[n..]);
        let (_, done, words) = self.acted[n..]
            .iter()
            .rev()
            .find(|(p, _, _)| *p == id)
            .cloned()
            .expect("acted");
        (done, words)
    }

    /// Gives `n` of a kind, one at a time (what cannot be carried is put down by the feet).
    pub fn give(&mut self, id: &str, n: u16) {
        assert!(self.items.get(id).is_some(), "no {id}");
        for _ in 0..n {
            let (had, lay) = (self.has(id), self.lying.len());
            self.server.send(ToServer::Give(Stack::one(id)));
            self.until(5.0, |w| w.has(id) > had || w.lying.len() > lay);
        }
    }

    /// How many of a kind are carried.
    pub fn has(&self, id: &str) -> u32 {
        let mut n = 0;
        let mut c = self.carry.clone();
        c.for_each_mut(&mut |s| {
            if s.id == id {
                n += s.count as u32;
            }
        });
        n
    }

    pub fn block(&self, p: BlockPos) -> Option<String> {
        self.mirror
            .block(p)
            .map(|s| self.reg.block_of(s).name.path().to_owned())
    }

    /// The first solid block under the player's feet.
    pub fn ground(&self) -> BlockPos {
        let mut p = BlockPos::containing(self.feet);
        for _ in 0..6 {
            let solid = self
                .mirror
                .block(p)
                .is_some_and(|s| !self.reg.collision_shape(s).is_empty());
            if solid {
                return p;
            }
            p = p.down();
        }
        p
    }
}

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("hearth-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn knocking_stones_teaches_and_open_knowledge_builds_a_fire_that_cooks() {
    let dir = temp("workshop");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Discovery, 11);
    // Knocking a flint cobble with another stone, a few times, teaches what stone can do.
    w.give("hearth:cobble/flint", 1);
    w.give("hearth:cobble/granite", 1);
    for _ in 0..6 {
        w.act("knock_stones", AimAt::Nothing);
    }
    w.until(5.0, |w| w.knowledge.knows("hearth:sharp_flake"));
    assert!(
        w.knowledge.knows("hearth:stone_as_hammer"),
        "learned: {:?}",
        w.learned
    );
    assert!(w.knowledge.knows("hearth:sharp_flake"), "{:?}", w.learned);
    // Knowledge not yet had is refused.
    let (done, words) = w.act("make_hand_axe", AimAt::Nothing);
    assert!(!done, "{words}");
    drop(w);
    let _ = std::fs::remove_dir_all(&dir);

    // With all knowledge open: lay a fire, light it, roast meat on it.
    let dir = temp("workshop-open");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::Open, 11);
    let ground = w.ground();
    // A place to build: open ground next to the player.
    let site = [(2, 0), (-2, 0), (0, 2), (0, -2), (2, 2), (-2, -2)]
        .iter()
        .map(|(dx, dz)| {
            // The top solid block of that column near the feet.
            let mut p = BlockPos::new(ground.x + dx, ground.y + 2, ground.z + dz);
            for _ in 0..5 {
                let solid = w
                    .mirror
                    .block(p)
                    .is_some_and(|s| !w.reg.collision_shape(s).is_empty());
                if solid {
                    return Some(p);
                }
                p = p.down();
            }
            None
        })
        .find_map(|p| p)
        .expect("ground to build on");
    w.give("hearth:stick/oak_wood", 4);
    w.give("hearth:handful/dry_grass", 2);
    let (done, words) = w.act(
        "build_campfire",
        AimAt::Block {
            pos: site,
            top: true,
        },
    );
    assert!(done, "laid: {words}");
    let hearth = site.up();
    assert!(
        w.until(10.0, |w| w.block(hearth).as_deref() == Some("campfire")),
        "a campfire stands: {:?}",
        w.block(hearth)
    );
    w.give("hearth:ember", 1);
    let lit = loop {
        let (done, words) = w.act(
            "light_fire",
            AimAt::Block {
                pos: hearth,
                top: false,
            },
        );
        if done {
            break true;
        }
        assert!(words.contains("ember dies"), "{words}");
        w.give("hearth:ember", 1);
    };
    assert!(lit);
    let burning = w.until(30.0, |w| {
        w.mirror
            .block(hearth)
            .is_some_and(|s| matches!(w.reg.get(s, "fire"), Some("low") | Some("high")))
    });
    assert!(burning, "the fire burns");
    // A few minutes for the hearth to heat.
    let t = w.ticks + (w.ticks_per_day / 288.0) as u64;
    w.until(30.0, |w| w.ticks >= t);
    // Fed for the hour a novice takes to roast.
    for _ in 0..4 {
        w.give("hearth:stick/oak_wood", 1);
        let (done, words) = w.act(
            "feed_fire",
            AimAt::Block {
                pos: hearth,
                top: false,
            },
        );
        assert!(done, "fed: {words}");
    }
    w.give("hearth:cut/meat", 2);
    let (done, words) = w.act(
        "roast_meat",
        AimAt::Block {
            pos: hearth,
            top: false,
        },
    );
    assert!(done, "{words}");
    assert!(w.has("hearth:cut/cooked_meat") > 0, "roasted: {words}");
    // Eating it.
    let path = hearth_items::Path::at(hearth_items::Root::Hand(hearth_items::Hand::Right));
    if w.carry
        .right
        .as_ref()
        .is_some_and(|s| s.id.contains("cooked_meat"))
    {
        w.server.send(ToServer::Eat(path));
        assert!(w.until(10.0, |w| w.acted.iter().any(|(p, d, _)| p == "eat" && *d)));
    }
    // Digging by hand: the earth goes, and a spoil pile rises beside the hole.
    let soil = w.ground();
    let before = w.block(soil);
    let (done, words) = w.act(
        "dig_by_hand",
        AimAt::Block {
            pos: soil,
            top: true,
        },
    );
    if done {
        assert!(
            w.until(10.0, |w| w.block(soil).as_deref() != before.as_deref()),
            "dug out"
        );
        let spoil = w.until(10.0, |w| {
            (-2..=2).any(|dx: i32| {
                (-2..=2).any(|dz: i32| {
                    (-3..=3).any(|dy: i32| {
                        w.block(BlockPos::new(soil.x + dx, soil.y + dy, soil.z + dz))
                            .as_deref()
                            == Some("spoil")
                    })
                })
            })
        });
        assert!(spoil, "a spoil pile");
    } else {
        panic!("could not dig here: {words} ({before:?})");
    }
    drop(w);
    let _ = std::fs::remove_dir_all(&dir);
}

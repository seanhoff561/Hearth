//! A test's view of a running world: the server run fast, the blocks it sends mirrored, what
//! the player carries, knows and has done; and a player's hands for scripted bots.

#![allow(dead_code)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use glam::DVec3;
use hearth::server::{Server, View, WorldSpec};
use hearth_content::Content;
use hearth_craft::KnowledgeState;
use hearth_items::{Carry, Hand, Items, Path, Root, Stack, WorldItem};
use hearth_math::BlockPos;
use hearth_physics::{Motion, Mover};
use hearth_protocol::{AimAt, Moved, ToClient, ToServer};
use hearth_render::atlas::TextureArray;
use hearth_world::{BlockRegistry, CubeMap};

pub struct World {
    pub server: Server,
    pub reg: Arc<BlockRegistry>,
    pub content: Arc<Content>,
    pub mirror: CubeMap,
    pub items: Arc<Items>,
    pub carry: Carry,
    pub knowledge: KnowledgeState,
    pub lying: Vec<WorldItem>,
    pub mover: Mover,
    pub ticks: u64,
    pub ticks_per_day: f64,
    /// The world's calendar (its seasons and their start).
    pub calendar: hearth_env::Calendar,
    /// What came of things done: (process, done, words).
    pub acted: Vec<(String, bool, String)>,
    pub learned: Vec<String>,
    pub working: bool,
    /// The body as the server last told it.
    pub body: Option<hearth_protocol::BodyView>,
    /// The world's terrain as the client has it (for the landscape seen from afar).
    pub generator: Arc<hearth_worldgen::WorldGenerator>,
    /// The animals near the player as the server last told of them.
    pub animals: Vec<hearth_fauna::live::AnimalView>,
    /// The people near the player as the server last told of them.
    pub people: Vec<hearth_people::PersonView>,
    /// What the people near said, as the player made it out.
    pub heard: Vec<hearth_protocol::HeardLine>,
    /// The childhood as last told, and the moments told of so far (their names).
    pub childhood: Option<hearth_protocol::ChildhoodView>,
    pub moments: Vec<String>,
    /// How the player looks, as last told.
    pub appearance: Option<hearth_character::Appearance>,
    /// The last census of the groups about the player.
    pub census: Option<Vec<(u16, glam::DVec2, u32)>>,
    /// The signs animals left about the player, as last told.
    pub signs: Vec<hearth_fauna::live::Sign>,
    /// The calls heard.
    pub calls: Vec<hearth_fauna::voices::Called>,
    /// Whether the server said it saved since asked.
    pub saved: bool,
    /// The births offered, and the birth shown (H8).
    pub births: Vec<hearth_protocol::BirthChoice>,
    pub born: Option<hearth_protocol::Born>,
    /// The developer's inspector's last record of the person it looks at.
    pub inspected: Option<hearth_people::inspect::Report>,
    /// Watching (H9): the followed life, the chronicle and the overlay as last told; what the
    /// player knows of the one it looks at.
    pub life_of: Option<(u64, Vec<String>)>,
    pub chronicle: Option<Vec<hearth_protocol::ChronicleEntry>>,
    pub overlay: Option<Option<hearth_protocol::OverlayMap>>,
    pub regarded: Option<(u64, Vec<String>)>,
    /// The conversation backend (H10): heard lines phrased (their ids and words), typed words to
    /// choose an act for, and how the backend is.
    pub phrased: Vec<(u64, String)>,
    pub clarify: Option<(u64, String, Vec<(hearth_people::player::Ask, String)>)>,
    pub conversing: Option<(bool, bool, Option<String>)>,
}

/// Copies a directory and all in it.
pub fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let dest = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &dest)?;
        } else {
            std::fs::copy(e.path(), dest)?;
        }
    }
    Ok(())
}

pub fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("hearth-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

impl World {
    pub fn start(dir: &std::path::Path, knowledge: hearth_save::KnowledgeMode, seed: u64) -> Self {
        Self::start_with(dir, knowledge, seed, false)
    }

    /// A world whose player lives their childhood (or begins grown).
    pub fn start_with(
        dir: &std::path::Path,
        knowledge: hearth_save::KnowledgeMode,
        seed: u64,
        childhood: bool,
    ) -> Self {
        Self::start_in(
            dir,
            knowledge,
            seed,
            childhood,
            hearth::eras::WILD_EARTH,
            None,
        )
    }

    /// A world of an era (H8), its player born into the household `birth` of those offered, or
    /// left to choose.
    pub fn start_in(
        dir: &std::path::Path,
        knowledge: hearth_save::KnowledgeMode,
        seed: u64,
        childhood: bool,
        era: &str,
        birth: Option<usize>,
    ) -> Self {
        let spec = WorldSpec {
            name: "test".into(),
            seed,
            // An era's world on the default planet: a Tiny one holds only a band or two of its
            // peoples (D196). Wild Earth's tests keep the Tiny planet they were written on.
            planet: if era == hearth::eras::WILD_EARTH {
                hearth_math::PlanetSize::Tiny
            } else {
                hearth_math::PlanetSize::Standard
            },
            cache_dir: None,
            saves_dir: Some(dir.to_path_buf()),
            wish: hearth_protocol::Wish {
                female: Some(false),
                ..Default::default()
            },
            death: hearth_save::Death::default(),
            knowledge,
            childhood,
            era: era.to_owned(),
            birth,
            shape: Default::default(),
            birthplace: None,
            mode: None,
        };
        Self::start_spec(spec)
    }

    /// A world of Wild Earth on the Tiny planet in a game mode (Amendment P §2), begun grown.
    #[allow(dead_code)]
    pub fn start_mode(dir: &std::path::Path, mode: &str, seed: u64) -> Self {
        let spec = WorldSpec {
            name: "test".into(),
            seed,
            planet: hearth_math::PlanetSize::Tiny,
            cache_dir: None,
            saves_dir: Some(dir.to_path_buf()),
            wish: hearth_protocol::Wish {
                female: Some(false),
                ..Default::default()
            },
            death: hearth_save::Death::default(),
            knowledge: hearth_save::KnowledgeMode::Discovery,
            childhood: false,
            era: hearth::eras::WILD_EARTH.to_owned(),
            birth: None,
            shape: Default::default(),
            birthplace: None,
            mode: Some(mode.to_owned()),
        };
        Self::start_spec(spec)
    }

    /// A world of a spec.
    pub fn start_spec(spec: WorldSpec) -> Self {
        let era_birth = spec.era != hearth::eras::WILD_EARTH && spec.birth.is_some();
        let atlas = Arc::new(TextureArray::from_entries(&hearth_texgen::textures_for(
            None,
        )));
        let server = Server::start(
            spec,
            atlas,
            View {
                radius: 3,
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
        // The clock moves only when the test runs it.
        server.send(ToServer::Run(0));
        let mut w = World {
            server,
            mirror: CubeMap::new(ready.planet),
            reg: ready.reg.clone(),
            content: ready.content.clone(),
            items: ready.items.clone(),
            carry: Carry::default(),
            knowledge: KnowledgeState::default(),
            lying: Vec::new(),
            mover: ready.player,
            ticks: ready.ticks,
            ticks_per_day: ready.calendar.ticks_per_day(),
            calendar: ready.calendar,
            acted: Vec::new(),
            learned: Vec::new(),
            working: false,
            body: None,
            generator: ready.generator.clone(),
            animals: Vec::new(),
            people: Vec::new(),
            heard: Vec::new(),
            childhood: None,
            moments: Vec::new(),
            appearance: Some(ready.appearance.clone()),
            census: None,
            signs: Vec::new(),
            calls: Vec::new(),
            saved: false,
            births: Vec::new(),
            born: None,
            inspected: None,
            life_of: None,
            chronicle: None,
            overlay: None,
            regarded: None,
            phrased: Vec::new(),
            clarify: None,
            conversing: None,
        };
        // A life born into an era's household begins where that household lives (H8).
        if era_birth {
            // The recent past is lived first: a century of the place's households (minutes in
            // a debug build on a busy machine).
            w.until(900.0, |w| w.born.is_some());
        }
        let feet = w.mover.pos;
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
                ToClient::Placed(m) => self.mover = m,
                ToClient::Work(w) => self.working = w.is_some(),
                ToClient::Body(b) => self.body = Some(*b),
                ToClient::Animals(v) => self.animals = v,
                ToClient::People(v) => self.people = v,
                ToClient::Childhood(v) => {
                    if let Some(c) = &v
                        && !c.passing
                        && self.moments.last() != Some(&c.name)
                    {
                        self.moments.push(c.name.clone());
                    }
                    self.childhood = v;
                }
                ToClient::Person(a) => self.appearance = Some(a),
                ToClient::Census(c) => self.census = Some(c),
                ToClient::Signs { signs, .. } => self.signs = signs,
                ToClient::Calls(c) => self.calls.extend(c),
                ToClient::Saved => self.saved = true,
                ToClient::Births(b) => self.births = b,
                ToClient::Heard(lines) => self.heard.extend(lines),
                ToClient::Born(b) => self.born = Some(*b),
                ToClient::Inspected(r) => self.inspected = r.map(|r| *r),
                ToClient::LifeOf(l) => self.life_of = l,
                ToClient::Chronicle(c) => self.chronicle = Some(c),
                ToClient::Overlay(o) => self.overlay = Some(o),
                ToClient::Regarded(r) => self.regarded = r,
                ToClient::Phrased { line, text } => self.phrased.push((line, text)),
                ToClient::Clarify {
                    person,
                    text,
                    options,
                } => self.clarify = Some((person, text, options)),
                ToClient::Conversing {
                    on,
                    free_text,
                    trouble,
                } => self.conversing = Some((on, free_text, trouble)),
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
    #[track_caller]
    pub fn until(&mut self, secs: f64, mut pred: impl FnMut(&Self) -> bool) -> bool {
        let t0 = Instant::now();
        let at = std::panic::Location::caller();
        loop {
            self.pump();
            if pred(self) {
                return true;
            }
            if t0.elapsed() > Duration::from_secs_f64(secs) {
                if secs >= 1.0 {
                    println!("  (waited {secs} s in vain at {at})");
                }
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// Runs the world on `n` ticks (more pass when the world is warped: sleep, long work).
    pub fn run(&mut self, n: u64) {
        let t = self.ticks + n;
        let t0 = Instant::now();
        self.server.send(ToServer::Run(n));
        assert!(
            self.until(600.0, |w| w.ticks >= t),
            "the clock stopped at {} (wanted {t})",
            self.ticks
        );
        if t0.elapsed() > Duration::from_secs(5) {
            println!(
                "  (running {n} ticks took {:.1} s)",
                t0.elapsed().as_secs_f64()
            );
        }
    }

    /// Lets `hours` of the world's time pass.
    pub fn wait_hours(&mut self, hours: f64) {
        let t = self.ticks + (hours / 24.0 * self.ticks_per_day) as u64;
        while self.ticks < t {
            let step = (t - self.ticks).min(2000);
            self.run(step);
        }
    }

    /// Does a process and waits for what came of it.
    pub fn act(&mut self, process: &str, aim: AimAt) -> (bool, String) {
        let id = if process.contains(':') {
            process.to_owned()
        } else {
            format!("hearth:{process}")
        };
        let n = self.acted.len();
        self.server.send(ToServer::Act {
            process: id.clone(),
            aim,
            hand: None,
        });
        // The work goes on as the clock runs.
        let mut ok = false;
        for _ in 0..2000 {
            self.run(20);
            if self.acted[n..].iter().any(|(p, _, _)| *p == id) {
                ok = true;
                break;
            }
            if !self.working && self.acted[n..].iter().any(|(p, d, _)| p.is_empty() && !*d) {
                // Stopped (walked off, fell asleep): say so.
                break;
            }
        }
        assert!(
            ok,
            "{id}: nothing came of it ({:?}); working {}, lying {:?}, asleep {:?}, dead {:?}",
            &self.acted[n..],
            self.working,
            self.body.as_ref().map(|b| b.lying),
            self.body.as_ref().map(|b| b.asleep),
            self.body.as_ref().map(|b| b.dead.clone()),
        );
        let (_, done, words) = self.acted[n..]
            .iter()
            .rev()
            .find(|(p, _, _)| *p == id)
            .cloned()
            .expect("acted");
        // The carried things and those lying about as they are now (the server tells them
        // before the clock).
        self.run(2);
        (done, words)
    }

    /// Runs the world until the populations about the player are made (on workers): their
    /// groups no more for a few looks a second apart. Returns the census of the groups.
    pub fn census_about(&mut self) -> Vec<(u16, glam::DVec2, u32)> {
        let mut last = (0usize, 0u32);
        for _ in 0..300 {
            self.run(40);
            std::thread::sleep(std::time::Duration::from_millis(300));
            self.census = None;
            self.server.send(ToServer::Census);
            if !self.until(30.0, |w| w.census.is_some()) {
                continue;
            }
            let n = self.census.as_ref().map_or(0, |c| c.len());
            if n > 0 && n == last.0 {
                last.1 += 1;
                if last.1 >= 3 {
                    break;
                }
            } else {
                last = (n, 0);
            }
        }
        self.census.clone().unwrap_or_default()
    }

    /// Gives one of a kind (development); what cannot be carried is put down by the feet.
    pub fn give(&mut self, id: &str, n: u16) {
        assert!(self.items.get(id).is_some(), "no {id}");
        for _ in 0..n {
            self.server.send(ToServer::Give(Stack::one(id)));
            self.run(2);
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

    /// How many of a kind are carried or lying within `r` of the player.
    pub fn at_hand(&self, pred: impl Fn(&str) -> bool, r: f64) -> u32 {
        let mut n = 0;
        let mut c = self.carry.clone();
        c.for_each_mut(&mut |s| {
            if pred(&s.id) {
                n += s.count as u32;
            }
        });
        for w in &self.lying {
            if pred(&w.stack.id) && (DVec3::from_array(w.pos) - self.mover.pos).length() < r {
                n += w.stack.count as u32;
            }
        }
        n
    }

    pub fn block(&self, p: BlockPos) -> Option<String> {
        self.mirror
            .block(p)
            .map(|s| self.reg.block_of(s).name.path().to_owned())
    }

    pub fn solid(&self, p: BlockPos) -> bool {
        self.mirror
            .block(p)
            .is_some_and(|s| !self.reg.collision_shape(s).is_empty())
    }

    /// Whether a thing can be built into a block: air, or what gives way to it (grass; not a
    /// plant with something to gather on it, nor water).
    pub fn free(&self, p: BlockPos) -> bool {
        self.mirror.block(p).is_none_or(|s| {
            s.is_air() || {
                let def = &self.reg.block_of(s).def;
                def.replaceable && def.fluid.is_none()
            }
        })
    }

    /// The first solid block under a point.
    pub fn ground_under(&self, at: DVec3) -> Option<BlockPos> {
        let mut p = BlockPos::containing(at);
        for _ in 0..12 {
            if self.solid(p) {
                return Some(p);
            }
            p = p.down();
        }
        None
    }

    /// The first solid block under the player's feet.
    pub fn ground(&self) -> BlockPos {
        self.ground_under(self.mover.pos)
            .unwrap_or(BlockPos::containing(self.mover.pos).down())
    }

    /// Stands the player on the ground at a column (as if walked there) and waits for the
    /// terrain about it.
    pub fn go(&mut self, x: f64, z: f64) {
        // The column's top solid block, from well above.
        let top = (0..96)
            .map(|k| {
                BlockPos::new(
                    x.floor() as i32,
                    self.mover.pos.y as i32 + 40 - k,
                    z.floor() as i32,
                )
            })
            .find(|p| self.solid(*p));
        let feet = match top {
            Some(p) => DVec3::new(x, p.y as f64 + 1.0, z),
            // Not loaded yet: about the generated ground there (a plateau may stand far above).
            None => {
                let ground = self
                    .generator
                    .terrain
                    .sample(x.floor() as i32, z.floor() as i32)
                    .height;
                DVec3::new(x, ground as f64 + 1.0, z)
            }
        };
        let mut m = self.mover;
        m.pos = feet;
        m.vel = DVec3::ZERO;
        self.mover = m;
        self.server.send(ToServer::Moved(Moved {
            mover: m,
            landed: None,
            motion: Motion::Still,
            speed: 0.0,
            straining: false,
            immersion: 0.0,
            airless_s: 0.0,
            yaw: 0.0,
        }));
        // Terrain streams in around the new place (a few seconds).
        let around = BlockPos::containing(feet);
        self.until(8.0, |w| {
            w.mirror.block(around).is_some()
                && w.mirror
                    .block(BlockPos::new(around.x + 16, around.y, around.z))
                    .is_some()
        });
        if top.is_none() {
            // Not yet known where the ground is: stand on it now.
            if let Some(g) = self.ground_under(feet + DVec3::new(0.0, 30.0, 0.0)) {
                self.go_exact(DVec3::new(x, g.y as f64 + 1.0, z));
            }
        }
    }

    /// Saves the world now and copies it to `to` (a moment kept for screenshots); the world
    /// goes on.
    pub fn keep(&mut self, from: &std::path::Path, to: &std::path::Path) {
        self.saved = false;
        self.server.send(ToServer::Save);
        assert!(self.until(60.0, |w| w.saved), "not saved");
        let src = from.join("test");
        let _ = std::fs::remove_dir_all(to);
        copy_dir(&src, to).expect("copy the saved world");
    }

    /// Turns the player to face a way (radians: 0 toward +z, south, turning toward +x, east).
    pub fn turn(&mut self, yaw: f32) {
        self.server.send(ToServer::Moved(Moved {
            mover: self.mover,
            landed: None,
            motion: Motion::Still,
            speed: 0.0,
            straining: false,
            immersion: 0.0,
            airless_s: 0.0,
            yaw,
        }));
        self.run(1);
    }

    /// Puts the player exactly here.
    pub fn go_exact(&mut self, feet: DVec3) {
        let mut m = self.mover;
        m.pos = feet;
        m.vel = DVec3::ZERO;
        self.mover = m;
        self.server.send(ToServer::Moved(Moved {
            mover: m,
            landed: None,
            motion: Motion::Still,
            speed: 0.0,
            straining: false,
            immersion: 0.0,
            airless_s: 0.0,
            yaw: 0.0,
        }));
        self.run(1);
    }

    /// Blocks within `r` (horizontally) of the player whose name and material pass `pred`,
    /// nearest first.
    pub fn find(&self, r: i32, pred: impl Fn(&str, Option<&str>) -> bool) -> Vec<BlockPos> {
        let c = BlockPos::containing(self.mover.pos);
        let mut out: Vec<(i64, BlockPos)> = Vec::new();
        for dy in -12..=16 {
            for dz in -r..=r {
                for dx in -r..=r {
                    let p = BlockPos::new(c.x + dx, c.y + dy, c.z + dz);
                    let Some(s) = self.mirror.block(p) else {
                        continue;
                    };
                    if s.is_air() {
                        continue;
                    }
                    let b = self.reg.block_of(s);
                    if pred(b.name.path(), b.def.material.as_deref()) {
                        let d = (dx * dx + dz * dz + dy * dy) as i64;
                        out.push((d, p));
                    }
                }
            }
        }
        out.sort_by_key(|(d, p)| (*d, *p));
        out.into_iter().map(|(_, p)| p).collect()
    }

    /// A place to stand beside a block with it in reach (4 m of the eyes), if there is one.
    pub fn stand_by(&self, p: BlockPos) -> Option<DVec3> {
        for (dx, dz) in [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (-1, -1),
            (1, -1),
            (-1, 1),
        ] {
            let (x, z) = (p.x + dx, p.z + dz);
            for dy in (-3..=2).rev() {
                let g = BlockPos::new(x, p.y + dy, z);
                if self.solid(g) && !self.solid(g.up()) && !self.solid(g.up().up()) {
                    let feet = DVec3::new(x as f64 + 0.5, g.y as f64 + 1.0, z as f64 + 0.5);
                    let eye = feet + DVec3::new(0.0, 1.6, 0.0);
                    let c = DVec3::new(p.x as f64 + 0.5, p.y as f64 + 0.5, p.z as f64 + 0.5);
                    if (c - eye).length() <= 3.8 {
                        return Some(feet);
                    }
                }
            }
        }
        None
    }

    /// Stands beside a block, within reach of it.
    pub fn go_to_block(&mut self, p: BlockPos) {
        if self.stand_by(p).is_none() {
            // Far off (not streamed in yet): come near first, and wait for the terrain about the
            // block to stream in, then find a footing.
            self.go(p.x as f64 + 0.5, p.z as f64 + 0.5);
            self.until(10.0, |w| {
                [p, p.up(), p.up().up(), p.down(), p.down().down()]
                    .iter()
                    .all(|q| w.mirror.block(*q).is_some())
            });
        }
        match self.stand_by(p) {
            Some(feet) => self.go_exact(feet),
            None => self.go(p.x as f64 + 0.5, p.z as f64 + 1.5),
        }
    }

    /// Picks up the thing lying with this id; whether it is now carried or dragged.
    pub fn pick_up(&mut self, id: u64) -> bool {
        self.server.send(ToServer::PickUp(id));
        self.run(2);
        let ok = !self.lying.iter().any(|l| l.id == id);
        if !ok {
            let what = self
                .lying
                .iter()
                .find(|l| l.id == id)
                .map(|l| l.stack.id.clone());
            println!("  (could not pick up {what:?})");
        }
        ok
    }

    /// Picks up a thing and holds it in a free hand (not on the tie or in a container).
    pub fn hold(&mut self, id: u64) -> bool {
        let Some(kind) = self
            .lying
            .iter()
            .find(|l| l.id == id)
            .map(|l| l.stack.id.clone())
        else {
            return false;
        };
        if !self.pick_up(id) {
            return false;
        }
        let in_hand = [&self.carry.right, &self.carry.left]
            .into_iter()
            .flatten()
            .any(|s| s.id == kind);
        if in_hand {
            return true;
        }
        let hand = if self.carry.right.is_none() {
            Hand::Right
        } else if self.carry.left.is_none() {
            Hand::Left
        } else {
            return false;
        };
        // From wherever it was stowed.
        let tie = Path::at(Root::Hung(0, 0));
        if self.carry.get(&tie).is_some_and(|s| s.id == kind) {
            self.server.send(ToServer::Shift {
                from: tie,
                count: Some(1),
                to: hearth_items::Target::Root(Root::Hand(hand)),
            });
            self.run(2);
        }
        [&self.carry.right, &self.carry.left]
            .into_iter()
            .flatten()
            .any(|s| s.id == kind)
    }

    /// Puts down everything in the hands (and on the tie) by the feet.
    pub fn put_down_all(&mut self) {
        for root in [
            Root::Hand(Hand::Right),
            Root::Hand(Hand::Left),
            Root::Hung(0, 0),
        ] {
            if self.carry.get(&Path::at(root)).is_some() {
                self.server.send(ToServer::PutDown {
                    from: Path::at(root),
                    count: None,
                    at: self.mover.pos + DVec3::new(0.3, 0.5, 0.3),
                });
                self.run(2);
            }
        }
    }

    /// Lets go of what is dragged, here.
    pub fn let_go(&mut self) {
        if self.carry.dragging.is_some() {
            self.server
                .send(ToServer::LetGo(self.mover.pos + DVec3::new(0.0, 0.5, 1.0)));
            self.run(2);
        }
    }
}

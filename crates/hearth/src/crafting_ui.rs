//! The player's side of making things (V2-5): what can be done with what is in hand and what
//! the eyes rest on, listed by the crosshair (the mouse wheel chooses, the primary action does
//! it); the work under way; discoveries, hunches and what came of things done, as they come.
//!
//! The list comes from the same process engine the server runs (`hearth_craft`), read from
//! the client's view of the world: the blocks it mirrors, what the player carries and knows.

use std::sync::Arc;

use glam::DVec3;
use hearth_content::Content;
use hearth_content::schema::station::Capability;
use hearth_content::triggers::key;
use hearth_craft::engine::offers;
use hearth_craft::food::bite_of;
use hearth_craft::{
    Aimed, Bench, Crafts, FireSeen, FireState, Graph, KnowledgeState, Lack, Mode, Surroundings,
};
use hearth_items::{Carry, Hand, Items, Path, Root, WorldItem};
use hearth_protocol::{AimAt, DrinkFrom, WorkView};
use hearth_ui::{Rgba, Ui};
use hearth_world::{BlockRegistry, CubeMap};

/// What choosing an offer does.
#[derive(Debug, Clone, PartialEq)]
pub enum Do {
    Process(String),
    Eat(Path),
    Drink(DrinkFrom),
    Fill(Path),
}

/// One line of the list by the crosshair.
#[derive(Debug, Clone, PartialEq)]
pub struct Offer {
    pub words: String,
    /// What doing it sends (none when something is lacking).
    pub act: Option<Do>,
    /// What is lacking, in words.
    pub why: Option<String>,
    /// Seconds of play it takes.
    pub play_s: Option<f64>,
    /// What it works (the material in play).
    pub material: Option<String>,
}

/// Kinds of news.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum News {
    Learned,
    Hunch,
    Done,
    Failed,
}

/// What the client sees to list offers from.
pub struct Seen<'a> {
    pub reg: &'a BlockRegistry,
    pub mirror: &'a CubeMap,
    pub items: &'a Items,
    pub carry: &'a Carry,
    pub world_items: &'a [WorldItem],
    pub aim: AimAt,
    /// The face of the block looked at (where a piece put up goes beside it).
    pub face: Option<hearth_math::Direction>,
    pub feet: DVec3,
    pub around: Surroundings,
    /// Seconds of play in a game day and a game year.
    pub day_s: f64,
    pub year_s: f64,
    /// The animal looked at, as a process sees it (V2-12).
    pub animal: Option<Aimed>,
}

/// The client's making and knowing.
pub struct Crafting {
    pub content: Arc<Content>,
    pub crafts: Arc<Crafts>,
    pub graph: Arc<Graph>,
    pub mode: Mode,
    pub knowledge: KnowledgeState,
    pub offers: Vec<Offer>,
    pub chosen: usize,
    pub work: Option<WorkView>,
    /// News and its age (s).
    pub news: Vec<(String, f64, News)>,
    /// Seconds until the list is drawn up again.
    refresh_in: f64,
    /// What was last looked at, and for how long (s); the aim told to the server.
    looking: (AimAt, f64),
    pub told_look: AimAt,
    /// A throw being wound up (s).
    pub charge: Option<f64>,
}

impl Crafting {
    pub fn new(content: Arc<Content>, crafts: Arc<Crafts>, graph: Arc<Graph>, mode: Mode) -> Self {
        Self {
            content,
            crafts,
            graph,
            mode,
            knowledge: KnowledgeState::default(),
            offers: Vec::new(),
            chosen: 0,
            work: None,
            news: Vec::new(),
            refresh_in: 0.0,
            looking: (AimAt::Nothing, 0.0),
            told_look: AimAt::Nothing,
            charge: None,
        }
    }

    /// Adds a piece of news (kept for a few seconds).
    pub fn tell(&mut self, words: String, kind: News) {
        if words.trim().is_empty() {
            return;
        }
        self.news.push((words, 0.0, kind));
        if self.news.len() > 5 {
            self.news.remove(0);
        }
    }

    /// Ages the news; true when the aim has rested long enough on something new to tell the
    /// server what is seen.
    pub fn age(&mut self, dt: f64, aim: AimAt) -> bool {
        for n in &mut self.news {
            n.1 += dt;
        }
        self.news.retain(|n| n.1 < 6.0);
        self.refresh_in -= dt;
        if self.looking.0 == aim {
            self.looking.1 += dt;
        } else {
            self.looking = (aim, 0.0);
        }
        self.looking.1 > 0.4 && self.told_look != aim && aim != AimAt::Nothing
    }

    /// Whether the list is due to be drawn up again (it is, a few times a second).
    pub fn due(&mut self) -> bool {
        if self.refresh_in <= 0.0 {
            self.refresh_in = 0.2;
            return true;
        }
        false
    }

    /// What is looked at, as a process sees it.
    pub fn aimed(&self, s: &Seen) -> Option<Aimed> {
        // Room for a piece where it would go: beside the face looked at.
        let room = || {
            let aim = match (s.aim, s.face) {
                (AimAt::Block { pos, .. }, Some(face)) => AimAt::Beside { pos, face },
                (aim, _) => aim,
            };
            crate::building::spot(s.mirror, s.reg, aim)
                .is_some_and(|at| crate::building::room(s.mirror, s.reg, at, s.feet))
        };
        match s.aim {
            AimAt::Nothing => None,
            AimAt::Animal(_) => s.animal.clone(),
            AimAt::Thing(id) => Some(Aimed::Thing(id)),
            AimAt::Beside { pos, .. } => {
                let block = s.reg.block_of(s.mirror.block(pos)?);
                Some(Aimed::Block {
                    name: block.name.to_string(),
                    material: block.def.material.clone(),
                    ground: false,
                    room: room(),
                })
            }
            AimAt::Block { pos, top } => {
                let state = s.mirror.block(pos)?;
                let block = s.reg.block_of(state);
                let name = block.name.to_string();
                if key(&name) == "flames" {
                    return Some(Aimed::Fire(FireSeen {
                        lit: true,
                        temp_c: 800.0,
                    }));
                }
                if let Some(st) = self.crafts.station_of_block(&self.content, &name) {
                    let burns = self.content.workstations.get(st).is_some_and(|w| {
                        w.provides
                            .iter()
                            .any(|c| matches!(c, Capability::MaxTempC(_)))
                    });
                    let state_of = s
                        .reg
                        .get(state, "fire")
                        .map(FireState::from_name)
                        .or_else(|| {
                            s.reg.get(state, "lit").map(|l| {
                                if l == "true" {
                                    FireState::Low
                                } else {
                                    FireState::Out
                                }
                            })
                        })
                        .unwrap_or(FireState::Out);
                    return Some(Aimed::Station {
                        id: st.to_owned(),
                        fire: burns.then(|| FireSeen {
                            lit: state_of.lit(),
                            temp_c: state_of.seen_temp_c(s.around.air_c),
                        }),
                    });
                }
                if block.def.fluid.is_some() || s.reg.get(state, "waterlogged") == Some("true") {
                    return Some(Aimed::Water);
                }
                let solid = !s.reg.collision_shape(state).is_empty();
                let open = s.mirror.block(pos.up()).is_none_or(|a| {
                    a.is_air() || {
                        let d = &s.reg.block_of(a).def;
                        d.replaceable && d.fluid.is_none()
                    }
                });
                Some(Aimed::Block {
                    name,
                    material: block.def.material.clone(),
                    ground: top && solid && open,
                    room: room(),
                })
            }
        }
    }

    /// Draws up the list of what can be done now.
    pub fn refresh(&mut self, s: &Seen) {
        let aimed = self.aimed(s);
        let at = match s.aim {
            AimAt::Block { pos, .. } | AimAt::Beside { pos, .. } => {
                DVec3::new(pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5)
            }
            AimAt::Thing(id) => s
                .world_items
                .iter()
                .find(|w| w.id == id)
                .map_or(s.feet, |w| DVec3::from_array(w.pos)),
            AimAt::Animal(_) | AimAt::Nothing => s.feet,
        };
        let lying = s.world_items.iter().filter(|w| {
            let p = DVec3::from_array(w.pos);
            w.work.is_none() && ((p - s.feet).length() < 2.5 || (p - at).length() < 1.5)
        });
        let bench = Bench::new(
            &self.content,
            s.items,
            s.carry,
            lying.map(|w| (w.id, &w.stack)),
            aimed.clone(),
            s.around.clone(),
        );
        let k = &self.knowledge;
        let graph = &self.graph;
        let open = self.mode == Mode::Open;
        let may = |r: &hearth_craft::Recipe| {
            open || k.may_attempt(r.def.knowledge.as_ref().map(|x| x.as_str()))
        };
        let skill = |r: &hearth_craft::Recipe| {
            r.def
                .skill
                .clone()
                .or_else(|| {
                    r.def
                        .knowledge
                        .as_ref()
                        .and_then(|n| graph.node(n.as_str()))
                        .and_then(|n| n.skill.clone())
                })
                .map_or(0.0, |n| k.skill(&n))
        };
        let mut out: Vec<Offer> = Vec::new();
        // Eating and drinking come first.
        let right = Path::at(Root::Hand(Hand::Right));
        if let Some(stack) = &s.carry.right
            && let Some(kind) = s.items.get(&stack.id)
        {
            if bite_of(&self.content, kind, stack).is_some() {
                out.push(Offer {
                    words: format!("eat the {}", kind.name.to_lowercase()),
                    act: Some(Do::Eat(right.clone())),
                    why: None,
                    play_s: None,
                    material: None,
                });
            }
            if let Some(c) = kind.container
                && c.liquid_l > 0.0
            {
                if stack.liquid_l >= 0.1 {
                    out.push(Offer {
                        words: "drink from the skin".into(),
                        act: Some(Do::Drink(DrinkFrom::Skin(right.clone()))),
                        why: None,
                        play_s: None,
                        material: None,
                    });
                }
                if matches!(aimed, Some(Aimed::Water)) && stack.liquid_l < c.liquid_l {
                    out.push(Offer {
                        words: "fill the skin".into(),
                        act: Some(Do::Fill(right.clone())),
                        why: None,
                        play_s: None,
                        material: None,
                    });
                }
            }
        }
        if matches!(aimed, Some(Aimed::Water)) {
            out.push(Offer {
                words: "drink".into(),
                act: Some(Do::Drink(DrinkFrom::Water(s.aim))),
                why: None,
                play_s: None,
                material: None,
            });
        }
        let mut lacking = 0;
        for (i, p) in offers(&self.crafts, &bench, &may, &skill) {
            let def = &self.crafts.recipes[i].def;
            match p {
                Ok(plan) => {
                    let play_s = match plan.scale {
                        hearth_content::schema::TimeScale::Day => {
                            plan.hours as f64 / 24.0 * s.day_s
                        }
                        hearth_content::schema::TimeScale::Year => {
                            plan.hours as f64 / (365.25 * 24.0) * s.year_s
                        }
                    };
                    out.push(Offer {
                        words: def.action.clone(),
                        act: Some(Do::Process(def.id.clone())),
                        why: None,
                        play_s: def.attended.then_some(play_s),
                        material: plan.material.clone(),
                    })
                }
                Err(l) if lacking < 3 => {
                    lacking += 1;
                    out.push(Offer {
                        words: def.action.clone(),
                        act: None,
                        why: Some(lack_words(&l)),
                        play_s: None,
                        material: None,
                    });
                }
                Err(_) => {}
            }
        }
        if self.offers.len() != out.len() {
            self.chosen = 0;
        }
        self.offers = out;
        if self.chosen >= self.offers.len() {
            self.chosen = 0;
        }
    }

    /// The chosen offer, if it can be done.
    pub fn chosen(&self) -> Option<&Offer> {
        self.offers.get(self.chosen).filter(|o| o.act.is_some())
    }

    /// The treatment of a wound offered now that lays this kind of thing on it (by its
    /// material): what using it with nothing aimed at does (E §3.2).
    pub fn treating_with(&self, kind: &hearth_items::ItemKind) -> Option<String> {
        let m = kind.material.as_deref()?;
        self.offers.iter().find_map(|o| {
            let Some(Do::Process(id)) = &o.act else {
                return None;
            };
            let def = &self.crafts.recipes[self.crafts.index_of(id)?].def;
            let with_it = def.inputs.iter().any(|i| {
                matches!(&i.item, hearth_content::schema::process::Match::Material(x)
                    if hearth_content::IdRef::qualify(x.as_str()).as_str() == m)
            });
            (def.treats.is_some() && with_it).then(|| id.clone())
        })
    }

    /// The first offer that can be done.
    pub fn first_act(&self) -> Option<Do> {
        self.offers.iter().find_map(|o| o.act.clone())
    }

    /// Moves the choice by whole steps of the wheel.
    pub fn choose(&mut self, steps: i32) {
        let n = self.offers.len() as i32;
        if n > 0 {
            self.chosen = (self.chosen as i32 - steps).rem_euclid(n) as usize;
        }
    }

    /// The list, the work under way, a throw's wind-up and the news, drawn about the
    /// crosshair.
    pub fn draw(&self, ui: &mut Ui<'_>, veil: u8) {
        let (w, h) = ui.size;
        let lh = hearth_ui::font::LINE as f32;
        let cx = w / 2.0;
        let mut y = (h / 2.0 + 20.0).round();
        if let Some(work) = &self.work {
            let words = format!("{} — {}", work.action, seconds(work.play_s_left));
            let lw = ui.font.width(&words) as f32;
            ui.draw.rect(
                cx - lw / 2.0 - 3.0,
                y - 2.0,
                lw + 6.0,
                lh + 8.0,
                Rgba([0, 0, 0, veil]),
            );
            ui.label(
                (cx - lw / 2.0).round(),
                y,
                &words,
                Rgba([235, 230, 210, 235]),
            );
            let bw = lw.max(60.0);
            ui.draw.rect(
                cx - bw / 2.0,
                y + lh + 1.0,
                bw,
                3.0,
                Rgba([60, 60, 60, 200]),
            );
            ui.draw.rect(
                cx - bw / 2.0,
                y + lh + 1.0,
                bw * work.done.clamp(0.0, 1.0),
                3.0,
                Rgba([230, 200, 120, 230]),
            );
        } else if !self.offers.is_empty() {
            let lines: Vec<(String, Rgba)> = self
                .offers
                .iter()
                .enumerate()
                .take(7)
                .map(|(i, o)| {
                    let chosen = i == self.chosen;
                    let mut text = format!("{} {}", if chosen { ">" } else { " " }, o.words);
                    if let Some(t) = o.play_s
                        && t >= 1.0
                    {
                        text.push_str(&format!("  ({})", seconds(t)));
                    }
                    if let Some(why) = &o.why {
                        text.push_str(&format!(" — {why}"));
                    }
                    let c = match (o.act.is_some(), chosen) {
                        (true, true) => Rgba([250, 240, 200, 245]),
                        (true, false) => Rgba([220, 220, 215, 220]),
                        (false, _) => Rgba([150, 150, 150, 200]),
                    };
                    (text, c)
                })
                .collect();
            let width = lines
                .iter()
                .map(|(t, _)| ui.font.width(t))
                .max()
                .unwrap_or(0) as f32;
            ui.draw.rect(
                cx - width / 2.0 - 3.0,
                y - 2.0,
                width + 6.0,
                lines.len() as f32 * lh + 3.0,
                Rgba([0, 0, 0, veil / 2]),
            );
            for (t, c) in lines {
                ui.label((cx - width / 2.0).round(), y, &t, c);
                y += lh;
            }
        }
        if let Some(c) = self.charge {
            let f = (c / 1.0).clamp(0.0, 1.0) as f32;
            ui.draw.rect(
                cx - 20.0,
                h / 2.0 - 12.0,
                40.0,
                2.0,
                Rgba([60, 60, 60, 200]),
            );
            ui.draw.rect(
                cx - 20.0,
                h / 2.0 - 12.0,
                40.0 * f,
                2.0,
                Rgba([240, 220, 160, 230]),
            );
        }
        // News, from the top.
        let mut ny = (h * 0.16).round();
        for (words, age, kind) in &self.news {
            let a = ((1.0 - ((age - 4.5).max(0.0) / 1.5)) * 240.0).clamp(0.0, 240.0) as u8;
            let c = match kind {
                News::Learned => Rgba([250, 220, 120, a]),
                News::Hunch => Rgba([200, 210, 240, a]),
                News::Done => Rgba([225, 225, 220, a]),
                News::Failed => Rgba([235, 170, 150, a]),
            };
            let lw = ui.font.width(words) as f32;
            ui.draw.rect(
                cx - lw / 2.0 - 3.0,
                ny - 2.0,
                lw + 6.0,
                lh + 3.0,
                Rgba([0, 0, 0, (veil as u32 * a as u32 / 255) as u8]),
            );
            ui.label((cx - lw / 2.0).round(), ny, words, c);
            ny += lh + 4.0;
        }
    }
}

fn lack_words(l: &Lack) -> String {
    match l {
        Lack::Knowledge => "not known".into(),
        Lack::Target => "not here".into(),
        Lack::Tool(p) => format!("needs {p} in hand"),
        Lack::Input(w) => format!("needs {w}"),
        Lack::Condition(c) => format!("needs {c}"),
    }
}

/// A span of play in words.
fn seconds(s: f64) -> String {
    if s < 90.0 {
        format!("{:.0} s", s.max(1.0))
    } else {
        format!("{:.0} min", s / 60.0)
    }
}

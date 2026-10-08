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
    /// Lie down to rest or sleep (the Rest screen).
    Rest,
    /// Look closely at work left to itself (E §7.5).
    Inspect(u64),
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
    /// The tools it is done with, in words.
    pub with: Option<String>,
}

/// What a hand does when its button is pressed (Amendment P §5.2).
#[derive(Debug, Clone, PartialEq)]
pub enum HandDo {
    Do(Do),
    /// Lift the thing looked at.
    PickUp(u64),
    /// A blow at what is looked at.
    Blow,
    /// With nothing looked at, the thing held's own use (E §3.2).
    Use,
}

/// A hand's use now: its hint's words, what it does, and whether it puts away what it holds
/// first.
#[derive(Debug, Clone, PartialEq)]
pub struct HandChoice {
    pub hint: String,
    pub act: HandDo,
    pub stow: bool,
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
    /// What the left and the right hand would do now.
    pub hands: [Option<HandChoice>; 2],
    /// The process preferred for a target and a thing held (learned from the action menu).
    pub prefer: std::collections::BTreeMap<String, String>,
    /// What is looked at, as the last list was drawn up for it.
    pub aimed_now: Option<Aimed>,
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
            hands: [None, None],
            prefer: Default::default(),
            aimed_now: None,
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

    /// How long the look has rested on what it is on (s).
    pub fn looked_s(&self) -> f64 {
        self.looking.1
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
        self.aimed_now = aimed.clone();
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
                    with: None,
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
                        with: None,
                    });
                }
                if matches!(aimed, Some(Aimed::Water)) && stack.liquid_l < c.liquid_l {
                    out.push(Offer {
                        words: "fill the skin".into(),
                        act: Some(Do::Fill(right.clone())),
                        why: None,
                        play_s: None,
                        material: None,
                        with: None,
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
                with: None,
            });
        }
        // Work left to itself: look closely at it (E §7.5).
        if let AimAt::Thing(id) = s.aim
            && s.world_items.iter().any(|w| w.id == id && w.work.is_some())
        {
            out.push(Offer {
                words: "look closely".into(),
                act: Some(Do::Inspect(id)),
                why: None,
                play_s: None,
                material: None,
                with: None,
            });
        }
        // Looking at nothing (at oneself, or the open ground): rest or sleep (P §5.3).
        if aimed.is_none() {
            out.push(Offer {
                words: "lie down to rest or sleep".into(),
                act: Some(Do::Rest),
                why: None,
                play_s: None,
                material: None,
                with: None,
            });
        }
        let mut lacking = 0;
        for (i, p) in offers(&self.crafts, &bench, &may, &skill) {
            let def = &self.crafts.recipes[i].def;
            match p {
                Ok(plan) => {
                    let play_s = plan.hours as f64 * 3600.0;
                    let tools: Vec<String> = plan
                        .tools
                        .iter()
                        .filter_map(|(src, _)| match src {
                            hearth_craft::engine::Source::Carried(p) => s.carry.get(p),
                            hearth_craft::engine::Source::Lying(_) => None,
                        })
                        .filter_map(|st| s.items.get(&st.id))
                        .map(|k| k.name.to_lowercase())
                        .collect();
                    out.push(Offer {
                        words: def.action.clone(),
                        act: Some(Do::Process(def.id.clone())),
                        why: None,
                        play_s: def.attended.then_some(play_s),
                        material: plan.material.clone(),
                        with: (!tools.is_empty()).then(|| tools.join(", ")),
                    })
                }
                Err(l) if lacking < 12 => {
                    lacking += 1;
                    out.push(Offer {
                        words: def.action.clone(),
                        act: None,
                        why: Some(lack_words(&l)),
                        play_s: None,
                        material: None,
                        with: None,
                    });
                }
                Err(_) => {}
            }
        }
        // Each hand's use, with its own tools.
        let hidden: Vec<String> = hearth_craft::knowledge::hidden_looks(&self.content, k)
            .into_iter()
            .flat_map(|(_, m)| m)
            .collect();
        for (i, hand) in [Hand::Left, Hand::Right].into_iter().enumerate() {
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
            )
            .by_hand(Some(hand));
            let held = match hand {
                Hand::Left => s.carry.left.as_ref(),
                Hand::Right => s.carry.right.as_ref(),
            };
            let content = &self.content;
            let known_food = |kind: &hearth_items::ItemKind, stack: &hearth_items::Stack| {
                bite_of(content, kind, stack).is_some()
                    && kind.material.as_ref().is_none_or(|m| !hidden.contains(m))
            };
            let prefer_key = prefer_key(&aimed, held);
            let ask = hearth_craft::intent::Ask {
                may: &may,
                skill: &skill,
                known_food: &known_food,
                prefer: self.prefer.get(&prefer_key).map(|s| s.as_str()),
            };
            let path = Path::at(Root::Hand(hand));
            self.hands[i] =
                hearth_craft::intent::resolve(content, &self.crafts, &bench, held, &ask).and_then(
                    |u| {
                        use hearth_craft::intent::HandAct;
                        let act = match u.act {
                            HandAct::Process(r) => {
                                HandDo::Do(Do::Process(self.crafts.recipes[r].def.id.clone()))
                            }
                            HandAct::PickUp => match s.aim {
                                AimAt::Thing(id) => HandDo::PickUp(id),
                                _ => return None,
                            },
                            HandAct::Drink if matches!(aimed, Some(Aimed::Water)) => {
                                HandDo::Do(Do::Drink(DrinkFrom::Water(s.aim)))
                            }
                            HandAct::Drink => HandDo::Do(Do::Drink(DrinkFrom::Skin(path.clone()))),
                            HandAct::Fill => HandDo::Do(Do::Fill(path.clone())),
                            HandAct::Eat => HandDo::Do(Do::Eat(path.clone())),
                            HandAct::Blow if aimed.is_none() => HandDo::Use,
                            HandAct::Blow => HandDo::Blow,
                        };
                        Some(HandChoice {
                            hint: u.hint,
                            act,
                            stow: u.stow,
                        })
                    },
                );
        }
        if self.offers.len() != out.len() {
            self.chosen = 0;
        }
        self.offers = out;
        if self.chosen >= self.offers.len() {
            self.chosen = 0;
        }
    }

    /// The use of the left (`0`) or right (`1`) hand now.
    pub fn hand(&self, i: usize) -> Option<&HandChoice> {
        self.hands.get(i).and_then(|h| h.as_ref())
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

    /// The work under way, the action menu when it is open (P §5.3), a throw's wind-up and the
    /// news, drawn about the crosshair.
    pub fn draw(&self, ui: &mut Ui<'_>, veil: u8, menu: bool) {
        let (w, h) = ui.size;
        let lh = hearth_ui::font::LINE as f32;
        let cx = w / 2.0;
        let mut y = (h / 2.0 + 20.0).round();
        if let Some(work) = &self.work {
            // What is being done, and how long it has left (no bar: E §9.1).
            let words = format!("{} — {}", work.action, seconds(work.play_s_left));
            let lw = ui.font.width(&words) as f32;
            ui.draw.rect(
                cx - lw / 2.0 - 3.0,
                y - 2.0,
                lw + 6.0,
                lh + 3.0,
                Rgba([0, 0, 0, veil]),
            );
            ui.label(
                (cx - lw / 2.0).round(),
                y,
                &words,
                Rgba([235, 230, 210, 235]),
            );
        } else if menu {
            // Every action for what is looked at: what it is, with what, about how long; greyed
            // with what it lacks. The wheel chooses; each hand's button does it with that hand.
            let mut lines: Vec<(String, Rgba)> = vec![(
                ui.lang.get("menu.actions.how").to_owned(),
                Rgba([200, 190, 160, 230]),
            )];
            if self.offers.is_empty() {
                lines.push((
                    ui.lang.get("menu.actions.none").to_owned(),
                    Rgba([170, 170, 165, 220]),
                ));
            }
            let first = self.chosen.saturating_sub(5);
            for (i, o) in self.offers.iter().enumerate().skip(first).take(11) {
                let chosen = i == self.chosen;
                let mut text = format!("{} {}", if chosen { ">" } else { " " }, o.words);
                if let Some(with) = &o.with {
                    text.push_str(&format!(" ({with})"));
                }
                if let Some(t) = o.play_s
                    && t >= 1.0
                {
                    text.push_str(&format!(" — {}", about(t)));
                }
                if let Some(why) = &o.why {
                    text.push_str(&format!(" — {why}"));
                }
                let c = match (o.act.is_some(), chosen) {
                    (true, true) => Rgba([250, 240, 200, 245]),
                    (true, false) => Rgba([220, 220, 215, 220]),
                    (false, _) => Rgba([150, 150, 150, 200]),
                };
                lines.push((text, c));
            }
            let width = lines
                .iter()
                .map(|(t, _)| ui.font.width(t))
                .max()
                .unwrap_or(0) as f32;
            ui.draw.rect(
                cx - width / 2.0 - 4.0,
                y - 3.0,
                width + 8.0,
                lines.len() as f32 * lh + 5.0,
                Rgba([0, 0, 0, veil.max(150)]),
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
        // What came of things done: a short line under the top, fading. Discoveries and
        // hunches: a quiet note at the top right by a small journal (P §5.4), one line each.
        let mut ny = (h * 0.16).round();
        let mut ty = 8.0;
        for (words, age, kind) in &self.news {
            let a = ((1.0 - ((age - 4.5).max(0.0) / 1.5)) * 240.0).clamp(0.0, 240.0) as u8;
            let shade = Rgba([0, 0, 0, (veil as u32 * a as u32 / 255) as u8]);
            match kind {
                News::Learned | News::Hunch => {
                    let c = if *kind == News::Learned {
                        Rgba([250, 220, 120, a])
                    } else {
                        Rgba([200, 210, 240, a])
                    };
                    let line = ui
                        .font
                        .wrap(words, (w * 0.35) as u32)
                        .into_iter()
                        .next()
                        .unwrap_or_default();
                    let lw = ui.font.width(&line) as f32;
                    let x = (w - lw - 18.0).round();
                    ui.draw.rect(x - 14.0, ty - 2.0, lw + 18.0, lh + 3.0, shade);
                    // The journal: a closed book.
                    ui.draw
                        .rect(x - 11.0, ty + 1.0, 7.0, 9.0, Rgba([150, 110, 70, a]));
                    ui.draw
                        .rect(x - 10.0, ty + 2.0, 1.0, 7.0, Rgba([230, 215, 180, a]));
                    ui.label(x, ty, &line, c);
                    ty += lh + 4.0;
                }
                News::Done | News::Failed => {
                    let c = if *kind == News::Done {
                        Rgba([225, 225, 220, a])
                    } else {
                        Rgba([235, 170, 150, a])
                    };
                    // A long account (what a close look sees) wraps.
                    for line in ui.font.wrap(words, (w * 0.6) as u32) {
                        let lw = ui.font.width(&line) as f32;
                        ui.draw
                            .rect(cx - lw / 2.0 - 3.0, ny - 2.0, lw + 6.0, lh + 3.0, shade);
                        ui.label((cx - lw / 2.0).round(), ny, &line, c);
                        ny += lh + 1.0;
                    }
                    ny += 3.0;
                }
            }
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

/// The key a preferred rule is kept under: what is looked at and what the hand holds.
pub fn prefer_key(aimed: &Option<Aimed>, held: Option<&hearth_items::Stack>) -> String {
    let target = match aimed {
        None => "nothing".to_owned(),
        Some(Aimed::Block { name, .. }) => name.clone(),
        Some(Aimed::Water) => "water".to_owned(),
        Some(Aimed::Thing(_)) => "thing".to_owned(),
        Some(Aimed::Station { id, .. }) => id.clone(),
        Some(Aimed::Fire(_)) => "fire".to_owned(),
        Some(Aimed::Animal { species, .. }) => species.clone(),
    };
    format!("{target}|{}", held.map_or("", |s| s.id.as_str()))
}

/// The hands' learned uses kept at `path` (none yet: none).
pub fn load_prefer(path: &std::path::Path) -> std::collections::BTreeMap<String, String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Keeps the hands' learned uses at `path`.
pub fn save_prefer(path: &std::path::Path, prefer: &std::collections::BTreeMap<String, String>) {
    let text = serde_json::to_string_pretty(prefer).unwrap_or_default();
    if let Err(e) = std::fs::write(path, text) {
        log::warn!("could not keep the hands' uses: {e}");
    }
}

/// A duration in rough words: "about 10 minutes".
fn about(s: f64) -> String {
    let m = s / 60.0;
    if m < 1.5 {
        "about a minute".into()
    } else if m < 55.0 {
        format!(
            "about {} minutes",
            ((m / 5.0).round() * 5.0).max(2.0) as u32
        )
    } else if m < 90.0 {
        "about an hour".into()
    } else {
        format!("about {} hours", (m / 60.0).round() as u32)
    }
}
